use crate::common::ModelError;
use crate::tts::{Tts, TtsData, TtsError, encode_sample_to_tts_packet};
use aha::models::voxcpm::config::VoxCPMConfig;
use aha::models::voxcpm::generate::VoxCPMGenerate;
use async_trait::async_trait;
use candle_core::Tensor;
use futures::Stream;
use futures::executor::block_on;
use resampler::{ResamplerFft, SampleRate};
use std::thread;
use std::{pin::Pin, sync::Arc};
use tokio::sync::{Mutex, mpsc::channel};
use tokio_stream::StreamExt;
use tokio_stream::wrappers::ReceiverStream;
use tracing::error;

pub struct TtsVoxCPM {
    instance: Arc<Mutex<VoxCPMGenerate>>,
    gen_sample_rate: u32,
    encoder: Arc<Mutex<opus_rs::OpusEncoder>>,
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
        let config_path = path.to_string() + "/config.json";
        let config: VoxCPMConfig = serde_json::from_slice(&std::fs::read(config_path)?)?;
        let gen_sample_rate = {
            match config.audio_vae_config {
                Some(config) => config.sample_rate,
                None => 16000,
            }
        };
        let encoder = opus_rs::OpusEncoder::new(
            encode_sample_rate as i32,
            1,
            opus_rs::Application::RestrictedLowDelay,
        )
        .map_err(|e| anyhow::anyhow!("opus encoder: {}", e))?;
        Ok(Self {
            instance: Arc::new(Mutex::new(instance)),
            encoder: Arc::new(Mutex::new(encoder)),
            gen_sample_rate: gen_sample_rate as u32,
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
        let gen_sample_rate = self.gen_sample_rate;
        thread::spawn(move || {
            block_on(async move {
                while let Some(text) = text_stream.next().await {
                    // let instance = instance.clone();
                    let tx = tx.clone();
                    match &text {
                        Ok(text) => {
                            // debug!("[TTS] receive, text = {}", text);
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
                                            let gen_sample_rate =
                                                match SampleRate::try_from(gen_sample_rate) {
                                                    Ok(sample_rate) => sample_rate,
                                                    Err(_) => {
                                                        let msg = format!(
                                                            "encode sample rate convert failure,{}",
                                                            encode_sample_rate
                                                        );
                                                        error!(msg);
                                                        if let Err(e) = tx
                                                            .send(Err(TtsError::Encode(msg)))
                                                            .await
                                                        {
                                                            error!("send error failure = {}", e);
                                                        }
                                                        return;
                                                    }
                                                };

                                            let sample_rate =
                                                match SampleRate::try_from(encode_sample_rate) {
                                                    Ok(sample_rate) => sample_rate,
                                                    Err(_) => {
                                                        let msg = format!(
                                                            "encode sample rate convert failure,{}",
                                                            encode_sample_rate
                                                        );
                                                        error!(msg);
                                                        if let Err(e) = tx
                                                            .send(Err(TtsError::Encode(msg)))
                                                            .await
                                                        {
                                                            error!("send error failure = {}", e);
                                                        }
                                                        return;
                                                    }
                                                };
                                            // resample
                                            let mut resampler = ResamplerFft::new(
                                                encode_channel as usize,
                                                gen_sample_rate,
                                                sample_rate,
                                            );
                                            let input_size = resampler.chunk_size_input();
                                            let output_size = resampler.chunk_size_output();
                                            let mut resample = vec![];
                                            let iter = sample.chunks(input_size);
                                            for chunk in iter {
                                                let mut output = vec![0.0f32; output_size];
                                                let chunk: &[f32] = {
                                                    if chunk.len() != input_size {
                                                        let mut chunk = chunk.to_vec();
                                                        chunk.resize(input_size, 0.0f32);
                                                        &chunk.to_owned()[..]
                                                    } else {
                                                        chunk
                                                    }
                                                };
                                                resampler.resample(chunk, &mut output).unwrap();
                                                resample.append(&mut output);
                                            }
                                            let mut encoder = encoder.lock().await;
                                            let audio = encode_sample_to_tts_packet(
                                                resample,
                                                &mut encoder,
                                                encode_sample_rate,
                                                encode_channel,
                                                encode_frame_duration,
                                            );

                                            let data = TtsData {
                                                audio: Some(audio),
                                                text: text.to_string(),
                                            };
                                            if let Err(e) = tx.send(Ok(data)).await {
                                                error!("output packet error = {}", e);
                                                break;
                                            } else {
                                                // debug!("[TTS] encode and send audio success");
                                            }
                                        }
                                        Err(e) => {
                                            error!(
                                                "tts tensor to sample error = {}",
                                                e.to_string()
                                            );
                                            if let Err(e) =
                                                tx.send(Err(TtsError::Encode(e.to_string()))).await
                                            {
                                                error!("send error failure = {}", e);
                                            }
                                            break;
                                        }
                                    }
                                }
                                Err(e) => {
                                    error!("tts synth error = {}", e.to_string());
                                    if let Err(e) =
                                        tx.send(Err(TtsError::Encode(e.to_string()))).await
                                    {
                                        error!("send error failure = {}", e);
                                    }
                                    break;
                                }
                            }
                        }
                        Err(e) => {
                            error!("tts text stream error = {}", e.to_string());
                            if let Err(e) = tx.send(Err(TtsError::Text(e.to_string()))).await {
                                error!("send error failure = {}", e);
                            }
                            break;
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
