use crate::ws::{sender::Sender, tts::Tts};
use axum::extract::ws::Message;
use futures_util::Sink;
use service::chobits::message::stt::SttMessage;
use sherpa_rs::{sense_voice::SenseVoiceRecognizer, vad::Vad};
use std::{rc::Rc, sync::Arc};
use tokio::sync::Mutex;

pub struct Listener<W, T>
where
    W: Sink<Message> + Unpin + 'static,
    T: Tts + 'static,
{
    session_id: String,
    sender: Arc<Mutex<Sender<W, T>>>,
    voice_data: Arc<Mutex<Vec<f32>>>,
    vad: Arc<Mutex<Vad>>,
    recognizer: Arc<Mutex<SenseVoiceRecognizer>>,
}

impl<W, T> Listener<W, T>
where
    W: Sink<Message> + Unpin + Send,
    T: Tts + Send,
{
    pub fn new(
        session_id: String,
        sender: Arc<Mutex<Sender<W, T>>>,
        vad: Arc<Mutex<Vad>>,
        recognizer: Arc<Mutex<SenseVoiceRecognizer>>,
    ) -> Self {
        Self {
            session_id,
            sender,
            vad,
            recognizer,
            voice_data: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn listen(&mut self, data: Rc<&[u8]>) {
        let session_id = self.session_id.clone();
        let data = data.to_vec();
        let sender = self.sender.clone();
        let voice_data = self.voice_data.clone();
        let vad = self.vad.clone();
        let recognizer = self.recognizer.clone();
        tokio::spawn(async move {
            //tracing::info!("voice len = {}", data.len());
            let sample_rate: u32 = 16000;
            // 16000Hz * 1 channel * 60 ms / 1000 = 960 samples -> frameSize
            let frame_size = (sample_rate * 60 / 1000) as usize;
            let mut samples = vec![0f32; frame_size];
            let mut decoder = opus::Decoder::new(sample_rate, opus::Channels::Mono).unwrap();
            decoder.decode_float(&data, &mut samples, false).unwrap();
            let mut voice_data = voice_data.lock().await;
            voice_data.append(&mut samples);
            let mut vad = vad.lock().await;
            let window_size = 512;
            while voice_data.len() > window_size {
                let window: Vec<f32> = voice_data.drain(..window_size).collect();
                vad.accept_waveform(window.to_vec());
                if vad.is_speech() {
                    while !vad.is_empty() {
                        let segment = vad.front();
                        let start_sec = (segment.start as f32) / sample_rate as f32;
                        let duration_sec = (segment.samples.len() as f32) / sample_rate as f32;
                        tracing::info!("start={}s duration={}s", start_sec, duration_sec);
                        let mut recognizer = recognizer.lock().await;
                        let result = recognizer.transcribe(sample_rate, &segment.samples);
                        let data =
                            SttMessage::new(Some(session_id.clone()), Some(result.text.clone()));
                        let mut sender = sender.lock().await;
                        match sender.send_json_text(&data).await {
                            Ok(_) => (),
                            Err(error) => {
                                tracing::info!("send tts message error {}", error);
                            }
                        }
                        tracing::info!("recognizer result = {:?}", result);
                        vad.pop();
                    }
                }
            }
        });
    }
}
