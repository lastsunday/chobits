use crate::{
    config,
    ws::{asr::Asr, state::State, vad::Vad},
};
use service::chobits::message::listen::ListenMode;
use std::{rc::Rc, sync::Arc};
use tokio::sync::Mutex;

pub struct Listener<V, A>
where
    V: Vad + 'static,
    A: Asr + 'static,
{
    session_id: String,
    temp_voice_data: Arc<Mutex<Vec<f32>>>,
    voice_data: Arc<Mutex<Vec<f32>>>,
    vad: Arc<Mutex<V>>,
    asr: Arc<Mutex<A>>,
    state: Arc<Mutex<State>>,
    listen_mode: Option<ListenMode>,
}

impl<V, A> Listener<V, A>
where
    V: Vad + Send,
    A: Asr + Send,
{
    pub fn new(
        session_id: String,
        vad: Arc<Mutex<V>>,
        asr: Arc<Mutex<A>>,
        state: Arc<Mutex<State>>,
    ) -> Self {
        Self {
            session_id,
            vad,
            asr,
            temp_voice_data: Arc::new(Mutex::new(Vec::new())),
            voice_data: Arc::new(Mutex::new(Vec::new())),
            state,
            listen_mode: None,
        }
    }

    pub fn listen(&mut self, data: Rc<&[u8]>) {
        //TODO: session not use
        let _session_id = self.session_id.clone();
        let data = data.to_vec();
        let temp_voice_data = self.temp_voice_data.clone();
        let voice_data = self.voice_data.clone();
        let vad = self.vad.clone();
        let state = self.state.clone();
        tokio::spawn(async move {
            let audio_config = config::get().audio();
            //tracing::info!("voice len = {}", data.len());
            let sample_rate: u32 = audio_config.input_sample_rate();
            let channel = audio_config.input_channel();
            let frame_duration = audio_config.input_frame_duration();
            // 16000Hz * 1 channel * 60 ms / 1000 = 960 samples -> frameSize
            let frame_size =
                ((sample_rate as u64 * channel as u64 * frame_duration) / 1000) as usize;
            let mut samples = vec![0f32; frame_size];
            let mut decoder = opus::Decoder::new(sample_rate, opus::Channels::Mono).unwrap();
            decoder.decode_float(&data, &mut samples, false).unwrap();
            let mut temp_voice_data = temp_voice_data.lock().await;
            temp_voice_data.append(&mut samples);
            let mut vad = vad.lock().await;
            let window_size = 512;
            while temp_voice_data.len() > window_size {
                let window: Vec<f32> = temp_voice_data.drain(..window_size).collect();
                if let Err(e) = vad.accept_waveform(window.to_vec()).await {
                    tracing::error!("accept_waveform error = {}", e.to_string());
                    return;
                }
                if vad.is_speech().await {
                    let mut state = state.lock().await;
                    state.update_last_activity_time();
                    state.update_last_speaking_time();
                    drop(state);
                    let mut segment = vad.front().await;
                    vad.pop().await;
                    let mut voice_data = voice_data.lock().await;
                    voice_data.append(&mut segment.samples);
                    let start_sec = (segment.start as f32) / sample_rate as f32;
                    let duration_sec = (voice_data.len() as f32) / sample_rate as f32;
                    tracing::info!("start={}s duration={}s", start_sec, duration_sec);
                    drop(voice_data);
                }
            }
        });
    }

    pub async fn reset(&mut self, listen_mode: Option<ListenMode>) {
        let state = self.state.clone();
        let mut state = state.lock().await;
        state.reset();
        self.listen_mode = listen_mode;
    }

    pub fn get_listen_mode(&self) -> Option<ListenMode> {
        self.listen_mode.clone()
    }

    pub async fn clear(&mut self) {
        let vad = self.vad.clone();
        let mut vad = vad.lock().await;
        let voice_data = self.voice_data.clone();
        let mut voice_data = voice_data.lock().await;
        let state = self.state.clone();
        let mut state = state.lock().await;
        vad.clear().await;
        voice_data.clear();
        state.last_speaking_time = None;
    }

    pub async fn get_result(&mut self) -> Option<String> {
        //TODO:session id not use
        let _session_id = self.session_id.clone();
        let voice_data = self.voice_data.clone();
        let voice_data = voice_data.lock().await;
        let audio_config = config::get().audio();
        let sample_rate: u32 = audio_config.input_sample_rate();
        let asr = self.asr.clone();
        let mut asr = asr.lock().await;
        tracing::info!("voice_data len = {}", voice_data.len());
        let result = asr.transcribe(sample_rate, &voice_data).await;
        tracing::info!("recognizer result = {:?}", result);
        match result {
            Ok(result) => Some(result.text),
            Err(e) => {
                tracing::error!("get result error = {}", e.to_string());
                None
            }
        }
    }
}
