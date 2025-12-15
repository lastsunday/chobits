use crate::config;
use crate::ws::common::ModelError;
use crate::ws::tts::{Tts, TtsData, TtsError};
use async_trait::async_trait;
use futures::Stream;
use futures::executor::block_on;
use kokoro_tts::KokoroTts;
use std::pin::Pin;
use std::thread;
use std::{cmp, sync::Arc};
use tokio::sync::{Mutex, mpsc::channel};
use tokio_stream::StreamExt;
use tokio_stream::wrappers::ReceiverStream;

#[derive(Clone)]
pub struct TtsKokoro {
    instance: Arc<Mutex<KokoroTts>>,
    encoder: Arc<Mutex<opus::Encoder>>,
}

impl TtsKokoro {
    pub async fn new(path: &str) -> Self {
        let audio_config = config::get().audio();
        let sample_rate = audio_config.output_sample_rate();
        let encoder = opus::Encoder::new(
            sample_rate,
            opus::Channels::Mono,
            opus::Application::LowDelay,
        )
        .unwrap();

        let instance = KokoroTts::new(format!("{}model.onnx", path), format!("{}voice.bin", path))
            .await
            .unwrap();
        Self {
            instance: Arc::new(Mutex::new(instance)),
            encoder: Arc::new(Mutex::new(encoder)),
        }
    }
}

#[async_trait]
impl Tts for TtsKokoro {
    async fn stream(
        &self,
        mut text_stream: Pin<
            Box<dyn Stream<Item = core::result::Result<String, ModelError>> + Send + Sync>,
        >,
    ) -> Pin<Box<dyn Stream<Item = core::result::Result<TtsData, TtsError>> + Send + Sync>> {
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
        Box::pin(ReceiverStream::new(rx))
    }
}

pub fn calcalute_tts_packet_size(sample_rate: u32, channel: u32, delay_millis: u64) -> usize {
    sample_rate as usize * channel as usize * delay_millis as usize / 1000
}
