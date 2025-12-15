use crate::ws::common::ModelError;
use crate::ws::tts::{Tts, TtsData, TtsError, encode_sample_to_tts_packet};
use aha::models::voxcpm::generate::VoxCPMGenerate;
use async_trait::async_trait;
use candle_core::Tensor;
use futures::Stream;
use futures::executor::block_on;
use std::thread;
use std::{pin::Pin, sync::Arc};
use tokio::sync::{Mutex, mpsc::channel};
use tokio_stream::StreamExt;
use tokio_stream::wrappers::ReceiverStream;

pub struct TtsVoxCPM {
    instance: Arc<Mutex<VoxCPMGenerate>>,
    encoder: Arc<Mutex<opus::Encoder>>,
    encode_sample_rate: u32,
    encode_channel: u32,
    encode_frame_duration: u64,
    //参照音频字幕
    reference_prompt_text: Option<String>,
    //参照音频路径
    reference_prompt_wav_path: Option<String>,
}

impl TtsVoxCPM {
    pub async fn new(
        path: &str,
        encode_sample_rate: u32,
        encode_channel: u32,
        encode_frame_duration: u64,
        reference_prompt_text: Option<String>,
        reference_prompt_wav_path: Option<String>,
    ) -> Result<Self, anyhow::Error> {
        let instance = VoxCPMGenerate::init(path, None, None)?;
        let encoder = opus::Encoder::new(
            encode_sample_rate,
            opus::Channels::Mono,
            opus::Application::LowDelay,
        )?;
        Ok(Self {
            instance: Arc::new(Mutex::new(instance)),
            encoder: Arc::new(Mutex::new(encoder)),
            encode_sample_rate,
            encode_channel,
            encode_frame_duration,
            reference_prompt_text,
            reference_prompt_wav_path,
        })
    }
}

#[async_trait]
impl Tts for TtsVoxCPM {
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
        let reference_prompt_text = self.reference_prompt_text.clone();
        let reference_prompt_wav_path = self.reference_prompt_wav_path.clone();
        thread::spawn(move || {
            block_on(async move {
                while let Some(text) = text_stream.next().await {
                    let instance = instance.clone();
                    let tx = tx.clone();
                    match &text {
                        Ok(text) => {
                            tracing::info!("[TTS] receive, text = {}", text);
                            let mut instance = instance.lock().await;
                            match instance.generate_with_prompt_simple(
                                text.to_string(),
                                reference_prompt_text.clone(),
                                reference_prompt_wav_path.clone(),
                            ) {
                                Ok(tensor) => {
                                    let sample = tensor_to_sample(&tensor);
                                    match sample {
                                        Ok(sample) => {
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
                                                tracing::info!(
                                                    "[TTS] encode and send audio success"
                                                );
                                            }
                                        }
                                        Err(e) => {
                                            tracing::error!(
                                                "tts tensor to sample error = {}",
                                                e.to_string()
                                            );
                                            if let Err(e) = tx.send(Err(TtsError::Encode)).await {
                                                tracing::error!("send error failure = {}", e);
                                            }
                                        }
                                    }
                                }
                                Err(e) => {
                                    tracing::error!("tts synth error = {}", e.to_string());
                                    if let Err(e) = tx.send(Err(TtsError::Encode)).await {
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

fn tensor_to_sample(audio: &Tensor) -> anyhow::Result<Vec<f32>> {
    let audio = audio.squeeze(0)?;
    let audio_vec = audio.to_vec1::<f32>()?;
    Ok(audio_vec)
}
