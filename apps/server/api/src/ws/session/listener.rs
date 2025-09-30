use crate::{
    config,
    ws::{asr::Asr, common::ModelError, vad::Vad},
};
use chrono::Local;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::info;

pub trait Listener {
    fn listen(&mut self, data: &[u8]) -> impl std::future::Future<Output = ()> + Send;
    fn set_state(&mut self, state: ListenState);
    fn get_state(&self) -> ListenState;
    fn get_result(
        &mut self,
    ) -> impl std::future::Future<Output = core::result::Result<ListenResult, ModelError>> + Send;
    fn reset(
        &mut self,
        silence_voice_timeout: Option<i64>,
    ) -> impl std::future::Future<Output = ()> + Send;
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

#[derive(Debug)]
pub struct DefaultListener<V, A>
where
    V: Vad + 'static,
    A: Asr + 'static,
{
    temp_voice_data: Arc<Mutex<Vec<f32>>>,
    voice_data: Arc<Mutex<Vec<f32>>>,
    vad: Arc<Mutex<V>>,
    asr: Arc<Mutex<A>>,
    decoder: opus::Decoder,
    pub state: ListenState,
    silence_voice_timeout: Option<i64>,
    latest_speaking_time: Option<i64>,
}

impl<V, A> DefaultListener<V, A>
where
    V: Vad + 'static,
    A: Asr + 'static,
{
    pub fn new(vad: Arc<Mutex<V>>, asr: Arc<Mutex<A>>) -> Self {
        let audio_config = config::get().audio();
        let sample_rate: u32 = audio_config.input_sample_rate();
        let decoder = opus::Decoder::new(sample_rate, opus::Channels::Mono).unwrap();
        Self {
            vad,
            asr,
            temp_voice_data: Arc::new(Mutex::new(Vec::new())),
            voice_data: Arc::new(Mutex::new(Vec::new())),
            decoder,
            state: ListenState::Idle,
            silence_voice_timeout: None,
            latest_speaking_time: None,
        }
    }
}

impl<V, A> Listener for DefaultListener<V, A>
where
    V: Vad + 'static,
    A: Asr + 'static,
{
    async fn listen(&mut self, data: &[u8]) {
        if self.state == ListenState::Idle {
            self.state = ListenState::Listening(false);
        }
        if let ListenState::Listening(_) = self.state {
            let data = data.to_vec();
            let temp_voice_data = self.temp_voice_data.clone();
            let voice_data = self.voice_data.clone();
            let vad = self.vad.clone();
            let audio_config = config::get().audio();
            let sample_rate: u32 = audio_config.input_sample_rate();
            let channel = audio_config.input_channel();
            let frame_duration = audio_config.input_frame_duration();
            // 16000Hz * 1 channel * 60 ms / 1000 = 960 samples -> frameSize
            let frame_size =
                ((sample_rate as u64 * channel as u64 * frame_duration) / 1000) as usize;
            let mut samples = vec![0f32; frame_size];
            let len = self
                .decoder
                .decode_float(&data, &mut samples, false)
                .unwrap();
            let mut temp_voice_data = temp_voice_data.lock().await;
            temp_voice_data.append(&mut samples[..len].to_vec());
            let mut vad = vad.lock().await;
            let window_size = 512;
            while temp_voice_data.len() > window_size {
                let window: Vec<f32> = temp_voice_data.drain(..window_size).collect();
                if let Err(e) = vad.accept_waveform(window.to_vec()).await {
                    tracing::error!("accept_waveform error = {}", e.to_string());
                    return;
                }
                if vad.is_speech().await {
                    self.state = ListenState::Listening(true);
                    self.latest_speaking_time = Some(Local::now().timestamp_millis());
                    let mut segment = vad.front().await;
                    vad.pop().await;
                    let mut voice_data = voice_data.lock().await;
                    voice_data.append(&mut segment.samples);
                    // let start_sec = (segment.start as f32) / sample_rate as f32;
                    // let duration_sec = (voice_data.len() as f32) / sample_rate as f32;
                    // tracing::info!("start={}s duration={}s", start_sec, duration_sec);
                }
            }
        }
        if let (Some(silence_voice_timeout), Some(latest_speaking_time)) =
            (self.silence_voice_timeout, self.latest_speaking_time)
        {
            let offset_time = Local::now().timestamp_millis() - latest_speaking_time;
            if offset_time >= silence_voice_timeout {
                info!(
                    "offset_time = {},silence_voice_timeout = {}",
                    offset_time, silence_voice_timeout
                );
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
        let audio_config = config::get().audio();
        let sample_rate: u32 = audio_config.input_sample_rate();
        let asr = self.asr.clone();
        let mut asr = asr.lock().await;
        // the follow code want to output wav file to test
        // use wavers::write;
        // let file_name = Utc::now().format("%Y%m%d%H%M%S").to_string();
        // let fp = format!("./asr_result{}.wav", file_name);
        // let sr: i32 = 16000;
        // write(fp, &voice_data, sr, 1);
        let result = asr.transcribe(sample_rate, &voice_data).await?;
        tracing::info!("recognizer result = {:?}", result);
        Ok(ListenResult {
            text: result.text,
            prob: result.prob,
        })
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
    }
}
