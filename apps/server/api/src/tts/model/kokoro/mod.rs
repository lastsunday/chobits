use crate::common::ModelError;
use crate::tts::{Tts, TtsData, TtsError, encode_sample_to_tts_packet};
use async_trait::async_trait;
use futures::Stream;
use futures::executor::block_on;
use kokoro_tts::KokoroTts;
use std::pin::Pin;
use std::sync::Arc;
use std::thread;
use tokio::sync::{Mutex, mpsc::channel};
use tokio_stream::StreamExt;
use tokio_stream::wrappers::ReceiverStream;

#[derive(Clone)]
pub struct TtsKokoro {
    instance: Arc<Mutex<KokoroTts>>,
    encoder: Arc<Mutex<opus::Encoder>>,
    encode_sample_rate: u32,
    encode_channel: u32,
    encode_frame_duration: u64,
}

impl TtsKokoro {
    pub async fn new(
        path: &str,
        encode_sample_rate: u32,
        encode_channel: u32,
        encode_frame_duration: u64,
    ) -> Result<Self, anyhow::Error> {
        let encoder = opus::Encoder::new(
            encode_sample_rate,
            opus::Channels::Mono,
            opus::Application::LowDelay,
        )?;
        let instance =
            KokoroTts::new(format!("{}model.onnx", path), format!("{}voice.bin", path)).await?;
        Ok(Self {
            instance: Arc::new(Mutex::new(instance)),
            encoder: Arc::new(Mutex::new(encoder)),
            encode_sample_rate,
            encode_channel,
            encode_frame_duration,
        })
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
        let encode_sample_rate = self.encode_sample_rate;
        let encode_channel = self.encode_channel;
        let encode_frame_duration = self.encode_frame_duration;
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
                                    let mut encoder = encoder.lock().await;
                                    let audio = encode_sample_to_tts_packet(
                                        sample,
                                        &mut encoder,
                                        encode_sample_rate,
                                        encode_channel,
                                        encode_frame_duration,
                                    );
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
                                    tracing::error!("tts synth error = {}", e.to_string());
                                    if let Err(e) =
                                        tx.send(Err(TtsError::Encode(e.to_string()))).await
                                    {
                                        tracing::error!("send error failure = {}", e);
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            tracing::error!("tts text stream error = {}", e.to_string());
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
