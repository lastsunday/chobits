use crate::ws::WsErrorCode;
use crate::{
    asr::Asr, common::ModelError, config::audio::AudioConfig, vad::Vad, ws::frame::FrameResult,
};
use async_trait::async_trait;
use chrono::Local;
use framework::err;
use framework::error::AppError;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::sync::mpsc::Sender;

/// Maximum prefix padding in samples (300ms at 16kHz).
const PREFIX_SAMPLES_MAX: usize = 4800;

#[async_trait]
pub trait Listener: Send + Sync {
    async fn listen(&mut self, data: &[u8]);
    fn set_state(&mut self, state: ListenState);
    fn get_state(&self) -> ListenState;
    async fn get_result(&mut self) -> core::result::Result<ListenResult, ModelError>;
    async fn reset(&mut self, silence_voice_timeout: Option<i64>);
    async fn set_sender(&mut self, tx: Sender<Result<FrameResult, AppError>>);

    async fn get_voice_data(&self) -> Vec<f32> {
        Vec::new()
    }

    async fn get_raw_pcm(&self) -> Vec<f32> {
        Vec::new()
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ListenState {
    Idle,
    /// is_speech
    Listening(bool),
    End,
}

#[derive(Debug, Clone)]
pub struct ListenResult {
    pub text: String,
    pub prob: f32,
}

pub struct DefaultListener {
    temp_voice_data: Arc<Mutex<Vec<f32>>>,
    voice_data: Arc<Mutex<Vec<f32>>>,
    vad: Arc<Mutex<Box<dyn Vad>>>,
    asr: Arc<Mutex<Box<dyn Asr>>>,
    decoder: Arc<Mutex<opus::Decoder>>,
    pub state: ListenState,
    silence_voice_timeout: Option<i64>,
    latest_speaking_time: Option<i64>,
    audio_config: Arc<AudioConfig>,
    error_tx: Option<Sender<Result<FrameResult, AppError>>>,
    /// Ring buffer for prefix padding (~300ms of raw audio).
    prefix_buffer: Vec<f32>,
    /// Whether prefix has been flushed for current speech turn.
    prefix_flushed: bool,
    /// Accumulates ALL decoded PCM (diagnostic only).
    total_pcm: Arc<Mutex<Vec<f32>>>,
}

impl DefaultListener {
    pub fn new(
        vad: Arc<Mutex<Box<dyn Vad>>>,
        asr: Arc<Mutex<Box<dyn Asr>>>,
        audio_config: Arc<AudioConfig>,
    ) -> Self {
        let sample_rate = audio_config
            .input_sample_rate
            .expect("input sample rate is empty");
        let decoder = Arc::new(Mutex::new(
            opus::Decoder::new(sample_rate, opus::Channels::Mono).unwrap(),
        ));
        Self {
            vad,
            asr,
            temp_voice_data: Arc::new(Mutex::new(Vec::new())),
            voice_data: Arc::new(Mutex::new(Vec::new())),
            decoder,
            state: ListenState::Idle,
            silence_voice_timeout: None,
            latest_speaking_time: None,
            audio_config,
            error_tx: None,
            prefix_buffer: Vec::with_capacity(PREFIX_SAMPLES_MAX),
            prefix_flushed: false,
            total_pcm: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

#[async_trait]
impl Listener for DefaultListener {
    async fn listen(&mut self, data: &[u8]) {
        if self.state == ListenState::Idle {
            self.state = ListenState::Listening(false);
        }
        if let ListenState::Listening(_) = self.state {
            let data = data.to_vec();
            let temp_voice_data = self.temp_voice_data.clone();
            let voice_data = self.voice_data.clone();
            let vad = self.vad.clone();
            let sample_rate = self
                .audio_config
                .input_sample_rate
                .expect("input sample rate is empty");
            let channel = self
                .audio_config
                .input_channel
                .expect("input channel is empty");
            let frame_duration = self
                .audio_config
                .input_frame_duration
                .expect("input frame duration is empty");
            let frame_size =
                ((sample_rate as u64 * channel as u64 * frame_duration) / 1000) as usize;
            let mut samples = vec![0f32; frame_size];
            let mut decoder = self.decoder.lock().await;
            let len = match decoder.decode_float(&data, &mut samples, false) {
                Ok(len) => len,
                Err(e) => {
                    tracing::error!(
                        "Opus decode error: {e}, data_len={}, first_bytes={:02x?}",
                        data.len(),
                        &data[..data.len().min(8)]
                    );
                    return;
                }
            };
            for s in samples[..len].iter_mut() {
                *s = s.clamp(-1.0, 1.0);
            }
            let mut total_pcm = self.total_pcm.lock().await;
            total_pcm.extend_from_slice(&samples[..len]);
            drop(total_pcm);
            let mut temp_voice_data = temp_voice_data.lock().await;
            temp_voice_data.append(&mut samples[..len].to_vec());
            let mut vad = vad.lock().await;
            let window_size = vad.window_size().await;
            while temp_voice_data.len() > window_size {
                let window: Vec<f32> = temp_voice_data.drain(..window_size).collect();

                // 1. Maintain ring buffer for prefix padding.
                self.prefix_buffer.extend(&window);
                if self.prefix_buffer.len() > PREFIX_SAMPLES_MAX {
                    let excess = self.prefix_buffer.len() - PREFIX_SAMPLES_MAX;
                    self.prefix_buffer.drain(..excess);
                }

                // 2. VAD decision only (no longer accumulates audio internally).
                if let Err(e) = vad.accept_waveform(&window).await {
                    tracing::error!("accept_waveform error = {}", e.to_string());
                    return;
                }

                // 3. Audio management in Listener.
                if vad.is_speech().await {
                    self.state = ListenState::Listening(true);
                    self.latest_speaking_time = Some(Local::now().timestamp_millis());
                    let mut voice_data = voice_data.lock().await;
                    if !self.prefix_flushed {
                        // First speech frame in this turn — flush prefix (includes current window).
                        voice_data.append(&mut self.prefix_buffer);
                        self.prefix_buffer = Vec::with_capacity(PREFIX_SAMPLES_MAX);
                        self.prefix_flushed = true;
                    } else {
                        // Subsequent speech frames.
                        voice_data.extend_from_slice(&window);
                    }
                } else {
                    self.prefix_flushed = false;
                }
            }
        }
        if let (Some(silence_voice_timeout), Some(latest_speaking_time)) =
            (self.silence_voice_timeout, self.latest_speaking_time)
        {
            let offset_time = Local::now().timestamp_millis() - latest_speaking_time;
            if offset_time >= silence_voice_timeout {
                self.state = ListenState::End;
            }
        }
    }

    fn set_state(&mut self, state: ListenState) {
        self.state = state;
    }

    fn get_state(&self) -> ListenState {
        self.state
    }

    async fn get_result(&mut self) -> core::result::Result<ListenResult, ModelError> {
        let voice_data = self.voice_data.clone();
        let voice_data = voice_data.lock().await;
        if voice_data.is_empty() {
            return Ok(ListenResult {
                text: "".to_string(),
                prob: 1.0,
            });
        }
        let sample_rate: u32 = self
            .audio_config
            .input_sample_rate
            .expect("input sample rate is empty");
        let asr = self.asr.clone();
        let mut asr = asr.lock().await;
        let result = asr.transcribe(sample_rate, &voice_data).await;
        match result {
            Ok(transcript) => Ok(ListenResult {
                text: transcript.text,
                prob: transcript.prob,
            }),
            Err(e) => {
                tracing::error!("{:?}", e);
                if let Some(tx) = &self.error_tx {
                    let _ = tx
                        .send(Err(err!(WsErrorCode::AsrFailure).with_extra(e.to_string())))
                        .await;
                }
                Err(e)
            }
        }
    }

    async fn reset(&mut self, silence_voice_timeout: Option<i64>) {
        self.state = ListenState::Idle;
        self.silence_voice_timeout = silence_voice_timeout;
        self.latest_speaking_time = None;
        let temp_voice_data = self.temp_voice_data.clone();
        let mut temp_voice_data = temp_voice_data.lock().await;
        temp_voice_data.clear();
        let voice_data = self.voice_data.clone();
        let mut voice_data = voice_data.lock().await;
        voice_data.clear();
        let vad = self.vad.clone();
        let mut vad = vad.lock().await;
        vad.clear().await;
        self.prefix_buffer.clear();
        self.prefix_flushed = false;
        self.total_pcm.lock().await.clear();
    }

    async fn set_sender(&mut self, tx: Sender<Result<FrameResult, AppError>>) {
        self.error_tx = Some(tx);
    }

    async fn get_voice_data(&self) -> Vec<f32> {
        self.voice_data.lock().await.clone()
    }

    async fn get_raw_pcm(&self) -> Vec<f32> {
        self.total_pcm.lock().await.clone()
    }
}
