pub mod tts_cache;

use crate::config;
use crate::ws::common::ModelError;
use futures::Stream;
use futures::executor::block_on;
use kokoro_tts::KokoroTts;
use std::thread;
use std::{cmp, sync::Arc};
use tokio::sync::{Mutex, mpsc::channel};
use tokio_stream::StreamExt;
use tokio_stream::wrappers::ReceiverStream;

pub trait Tts {
    fn output_stream(
        &self,
        text_stream: impl Stream<Item = core::result::Result<String, ModelError>>
        + Unpin
        + Send
        + 'static,
    ) -> impl Stream<Item = core::result::Result<TtsData, TtsError>> + Unpin + Send + 'static;
}

pub struct TtsData {
    pub audio: Vec<Vec<u8>>,
    pub text: String,
}

#[derive(Debug, thiserror::Error)]
pub enum TtsError {
    #[error("init error")]
    Init,
    #[error("encode error")]
    Encode,
    #[error("text error")]
    Text,
}

#[derive(Clone)]
pub struct TtsKokoro {
    instance: Arc<Mutex<KokoroTts>>,
    encoder: Arc<Mutex<opus::Encoder>>,
}

impl TtsKokoro {
    pub fn new(instance: Arc<Mutex<KokoroTts>>) -> Self {
        let audio_config = config::get().audio();
        let sample_rate = audio_config.output_sample_rate();
        let encoder = opus::Encoder::new(
            sample_rate,
            opus::Channels::Mono,
            opus::Application::LowDelay,
        )
        .unwrap();
        Self {
            instance,
            encoder: Arc::new(Mutex::new(encoder)),
        }
    }
}
impl Tts for TtsKokoro {
    fn output_stream(
        &self,
        mut text_stream: impl Stream<Item = core::result::Result<String, ModelError>>
        + Unpin
        + Send
        + 'static,
    ) -> impl Stream<Item = core::result::Result<TtsData, TtsError>> + Unpin + Send + 'static {
        let (tx, rx) = channel(10);
        let instance = self.instance.clone();
        let encoder = self.encoder.clone();
        thread::spawn(move || {
            block_on(async move {
                while let Some(text) = text_stream.next().await {
                    let instance = instance.clone();
                    let tx = tx.clone();
                    match text {
                        Ok(text) => {
                            tracing::info!("[TTS] receive, text = {}", text);
                            let instance = instance.lock().await;
                            match instance
                                .synth(text.clone(), kokoro_tts::Voice::Zf059(1))
                                .await
                            {
                                Ok((sample, _took)) => {
                                    let audio_config = config::get().audio();
                                    let sample_rate = audio_config.output_sample_rate();
                                    let channel = audio_config.output_channel();
                                    let frame_duration = audio_config.output_frame_duration();
                                    let len = sample.len();
                                    let size = calcalute_tts_packet_size(
                                        sample_rate,
                                        channel,
                                        frame_duration,
                                    );
                                    let count = len / size;
                                    let mut audio: Vec<Vec<u8>> = Vec::new();
                                    for n in 1..count {
                                        let start = (n - 1) * size;
                                        let end = cmp::min(n * size, len);
                                        let mut encoder = encoder.lock().await;
                                        let packet = encoder
                                            .encode_vec_float(&sample[start..end], size)
                                            .unwrap();
                                        audio.push(packet);
                                    }
                                    let data = TtsData {
                                        audio,
                                        text: text.to_string(),
                                    };
                                    if let Err(e) = tx.send(Ok(data)).await {
                                        tracing::error!("output packet error = {}", e);
                                        break;
                                    } else {
                                        tracing::info!("[TTS] encode and send audio success");
                                    }
                                }
                                Err(e) => {
                                    tracing::error!("tts synth error = {}", e.to_string())
                                }
                            }
                        }
                        Err(_e) => {
                            if let Err(e) = tx.send(Err(TtsError::Text)).await {
                                tracing::error!("send error failure = {}", e);
                            }
                        }
                    }
                }
                drop(tx);
            })
        });
        ReceiverStream::new(rx)
    }
}

pub fn calcalute_tts_packet_size(sample_rate: u32, channel: u32, delay_millis: u64) -> usize {
    sample_rate as usize * channel as usize * delay_millis as usize / 1000
}
