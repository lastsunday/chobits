use crate::common::ModelError;
use crate::config::audio::AudioConfig;
use crate::config::tts::TtsConfig;
use crate::tts::{Tts, TtsData, TtsError, encode_sample_to_tts_packet};
use async_trait::async_trait;
use futures::Stream;
use futures::executor::block_on;
use serde::Deserialize;
use std::pin::Pin;
use std::sync::Arc;
use std::thread;
use tokio::sync::Mutex;
use tokio::sync::mpsc::channel;
use tokio_stream::StreamExt;
use tokio_stream::wrappers::ReceiverStream;
use tracing::error;

#[derive(Deserialize, Default)]
#[serde(default)]
struct PocketTtsOptions {
    voice: Option<String>,
    temperature: Option<f32>,
    lsd_decode_steps: Option<usize>,
    eos_threshold: Option<f32>,
    model_variant: Option<String>,
}

pub struct TtsPocketTTS {
    model: Arc<pocket_tts::TTSModel>,
    voice_state: Option<pocket_tts::ModelState>,
    encoder: Arc<Mutex<opus_rs::OpusEncoder>>,
    encode_sample_rate: u32,
    encode_channel: u32,
    encode_frame_duration: u64,
}

impl TtsPocketTTS {
    pub async fn new(
        tts_config: &TtsConfig,
        audio_config: &AudioConfig,
    ) -> Result<Self, anyhow::Error> {
        let opts: PocketTtsOptions = tts_config
            .options
            .as_ref()
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();

        let model_dir = tts_config
            .path
            .clone()
            .unwrap_or_else(|| "data/tts/model/pocket-tts".into());

        let config = std::fs::read(format!("{model_dir}/b6369a24.yaml"))?;
        let weights = std::fs::read(format!("{model_dir}/tts_b6369a24.safetensors"))?;
        let tokenizer = std::fs::read(format!("{model_dir}/tokenizer.model"))?;

        let mut model = pocket_tts::TTSModel::load_from_bytes(&config, &weights, &tokenizer)?;
        model.temp = opts.temperature.unwrap_or(0.7);
        model.lsd_decode_steps = opts.lsd_decode_steps.unwrap_or(1);
        model.eos_threshold = opts.eos_threshold.unwrap_or(-4.0);

        let voice_state = match &tts_config.reference_prompt_wav_path {
            Some(path) => Some(model.get_voice_state(path)?),
            None => None,
        };

        let encode_sample_rate = audio_config
            .output_sample_rate
            .expect("tts output sample rate is empty");
        let encode_channel = audio_config
            .output_channel
            .expect("tts output channel is empty");
        let encode_frame_duration = audio_config
            .output_frame_duration
            .expect("tts output frame duration is empty");

        let encoder = opus_rs::OpusEncoder::new(
            encode_sample_rate as i32,
            encode_channel as usize,
            opus_rs::Application::RestrictedLowDelay,
        )
        .map_err(|e| anyhow::anyhow!("opus encoder: {}", e))?;

        Ok(Self {
            model: Arc::new(model),
            voice_state,
            encoder: Arc::new(Mutex::new(encoder)),
            encode_sample_rate,
            encode_channel,
            encode_frame_duration,
        })
    }
}

#[async_trait]
impl Tts for TtsPocketTTS {
    async fn stream(
        &self,
        mut text_stream: Pin<
            Box<dyn Stream<Item = core::result::Result<String, ModelError>> + Send + Sync>,
        >,
    ) -> Pin<Box<dyn Stream<Item = core::result::Result<TtsData, TtsError>> + Send + Sync>> {
        let (tx, rx) = channel(10);
        let model = self.model.clone();
        let voice_state = self.voice_state.clone();
        let encoder = self.encoder.clone();
        let encode_sample_rate = self.encode_sample_rate;
        let encode_channel = self.encode_channel;
        let encode_frame_duration = self.encode_frame_duration;

        thread::spawn(move || {
            block_on(async move {
                while let Some(text) = text_stream.next().await {
                    let tx = tx.clone();
                    match &text {
                        Ok(text) => {
                            let vs = voice_state
                                .as_ref()
                                .cloned()
                                .unwrap_or_default();

                            let iter = model.generate_stream_owned(text, &vs);
                            let mut encoder = encoder.lock().await;
                            let mut audio_packets = Vec::new();
                            for chunk in iter {
                                match chunk {
                                    Ok(tensor) => {
                                        let samples = tensor.to_vec1::<f32>().unwrap_or_default();
                                        let mut packets = encode_sample_to_tts_packet(
                                            samples,
                                            &mut encoder,
                                            encode_sample_rate,
                                            encode_channel,
                                            encode_frame_duration,
                                        );
                                        audio_packets.append(&mut packets);
                                    }
                                    Err(e) => {
                                        error!("pocket-tts stream error = {}", e);
                                        if let Err(e) =
                                            tx.send(Err(TtsError::Encode(e.to_string()))).await
                                        {
                                            error!("send error = {}", e);
                                        }
                                        break;
                                    }
                                }
                            }

                            let data = TtsData {
                                audio: Some(audio_packets),
                                text: text.to_string(),
                            };
                            if let Err(e) = tx.send(Ok(data)).await {
                                error!("output packet error = {}", e);
                                break;
                            }
                        }
                        Err(e) => {
                            error!("tts text stream error = {}", e.to_string());
                            if let Err(e) = tx.send(Err(TtsError::Text(e.to_string()))).await {
                                error!("send error = {}", e);
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
