use async_trait::async_trait;
use futures::Stream;
use rubato::{FftFixedIn, Resampler};
use sherpa_onnx::{
    GenerationConfig, OfflineTts, OfflineTtsConfig, OfflineTtsModelConfig,
    OfflineTtsPocketModelConfig, Wave,
};
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::mpsc::channel;
use tokio_stream::StreamExt;
use tokio_stream::wrappers::ReceiverStream;
use tokio_util::sync::CancellationToken;
use tracing::error;

use crate::common::ModelError;
use crate::config::audio::AudioConfig;
use crate::config::tts::TtsConfig;
use crate::tts::{Tts, TtsData, TtsError, encode_sample_to_tts_packet};

pub struct TtsPocket {
    tts: Arc<OfflineTts>,
    reference_audio: Vec<f32>,
    reference_sample_rate: i32,
    reference_prompt_text: Option<String>,
    output_sample_rate: u32,
    output_channel: u32,
    output_frame_duration: u64,
    speed: f32,
}

impl TtsPocket {
    pub async fn new(
        tts_config: &TtsConfig,
        audio_config: &AudioConfig,
    ) -> Result<Self, anyhow::Error> {
        let path = tts_config
            .path
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("tts path must be set in TtsConfig"))?;
        if !path.ends_with('/') {
            return Err(anyhow::anyhow!("tts path must end with '/'"));
        }

        let opts = tts_config.options.as_ref();

        let num_threads = opts
            .and_then(|o| o.get("num_threads"))
            .and_then(|v| v.as_i64())
            .unwrap_or(2) as i32;

        let debug = opts
            .and_then(|o| o.get("debug"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let voice_embedding_cache_capacity = opts
            .and_then(|o| o.get("voice_embedding_cache_capacity"))
            .and_then(|v| v.as_i64())
            .unwrap_or(50) as i32;

        let speed = opts
            .and_then(|o| o.get("speed"))
            .and_then(|v| v.as_f64())
            .unwrap_or(1.0) as f32;

        let opt_path = |key: &str, default_name: &str| -> Option<String> {
            opts.and_then(|o| o.get(key))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .or_else(|| Some(format!("{path}{default_name}")))
        };

        let config = OfflineTtsConfig {
            model: OfflineTtsModelConfig {
                pocket: OfflineTtsPocketModelConfig {
                    lm_flow: opt_path("lm_flow", "lm_flow.int8.onnx"),
                    lm_main: opt_path("lm_main", "lm_main.int8.onnx"),
                    encoder: opt_path("encoder", "encoder.onnx"),
                    decoder: opt_path("decoder", "decoder.int8.onnx"),
                    text_conditioner: opt_path("text_conditioner", "text_conditioner.onnx"),
                    vocab_json: opt_path("vocab_json", "vocab.json"),
                    token_scores_json: opt_path("token_scores_json", "token_scores.json"),
                    voice_embedding_cache_capacity,
                },
                num_threads,
                debug,
                ..Default::default()
            },
            ..Default::default()
        };

        let tts = OfflineTts::create(&config)
            .ok_or_else(|| anyhow::anyhow!("Failed to create OfflineTts"))?;

        let output_sample_rate = audio_config
            .output_sample_rate
            .ok_or_else(|| anyhow::anyhow!("AudioConfig.output_sample_rate is required"))?;
        let output_channel = audio_config
            .output_channel
            .ok_or_else(|| anyhow::anyhow!("AudioConfig.output_channel is required"))?;
        let output_frame_duration = audio_config
            .output_frame_duration
            .ok_or_else(|| anyhow::anyhow!("AudioConfig.output_frame_duration is required"))?;

        let reference_prompt_text = tts_config.reference_prompt_text.clone();

        let (reference_audio, reference_sample_rate) =
            if let Some(wav_path) = &tts_config.reference_prompt_wav_path {
                match Wave::read(wav_path) {
                    Some(wave) => {
                        let samples = wave.samples().to_vec();
                        let sr = wave.sample_rate();
                        (samples, sr)
                    }
                    None => {
                        error!("Failed to load reference audio: {wav_path}");
                        (Vec::new(), output_sample_rate as i32)
                    }
                }
            } else {
                (Vec::new(), output_sample_rate as i32)
            };

        Ok(Self {
            tts: Arc::new(tts),
            reference_audio,
            reference_sample_rate,
            reference_prompt_text,
            output_sample_rate,
            output_channel,
            output_frame_duration,
            speed,
        })
    }
}

#[async_trait]
impl Tts for TtsPocket {
    async fn stream(
        &self,
        text_stream: Pin<
            Box<dyn Stream<Item = core::result::Result<String, ModelError>> + Send + Sync>,
        >,
        cancel: CancellationToken,
    ) -> Pin<Box<dyn Stream<Item = core::result::Result<TtsData, TtsError>> + Send + Sync>> {
        let (tx, rx) = channel::<core::result::Result<TtsData, TtsError>>(10);

        let tts = self.tts.clone();
        let reference_audio = self.reference_audio.clone();
        let reference_sample_rate = self.reference_sample_rate;
        let reference_prompt_text = self.reference_prompt_text.clone();
        let output_sample_rate = self.output_sample_rate;
        let output_channel = self.output_channel;
        let output_frame_duration = self.output_frame_duration;
        let speed = self.speed;

        tokio::spawn(async move {
            let mut pinned = text_stream;
            let encode_sr = output_sample_rate;
            let channels = match output_channel {
                2 => 2_usize,
                _ => 1_usize,
            };
            let opus_channels = if channels == 2 {
                opus::Channels::Stereo
            } else {
                opus::Channels::Mono
            };
            let mut encoder =
                match opus::Encoder::new(encode_sr, opus_channels, opus::Application::Audio) {
                    Ok(e) => e,
                    Err(err) => {
                        error!("[PocketTTS] opus encoder creation error = {}", err);
                        return;
                    }
                };

            while let Some(text_result) = pinned.next().await {
                if cancel.is_cancelled() {
                    break;
                }
                let text = match text_result {
                    Ok(t) => t,
                    Err(e) => {
                        error!("[PocketTTS] text stream error = {}", e);
                        let _ = tx.send(Err(TtsError::Text(e.to_string()))).await;
                        break;
                    }
                };

                if cancel.is_cancelled() {
                    break;
                }
                let tts_clone = tts.clone();
                let text_clone = text.clone();
                let reference_audio_clone = reference_audio.clone();
                let reference_prompt_text_clone = reference_prompt_text.clone();
                let result = tokio::task::spawn_blocking(move || {
                    let mut extra = HashMap::new();
                    extra.insert(
                        "max_reference_audio_len".to_string(),
                        serde_json::json!(10.0),
                    );
                    let gen_config = GenerationConfig {
                        num_steps: 2,
                        speed,
                        reference_audio: if !reference_audio_clone.is_empty() {
                            Some(reference_audio_clone)
                        } else {
                            None
                        },
                        reference_sample_rate,
                        reference_text: reference_prompt_text_clone,
                        extra: Some(extra),
                        ..Default::default()
                    };
                    let audio = tts_clone.generate_with_config(
                        &text_clone,
                        &gen_config,
                        None::<fn(&[f32], f32) -> bool>,
                    );
                    match audio {
                        Some(a) => {
                            let samples = a.samples().to_vec();
                            let sr = a.sample_rate();
                            Some((samples, sr))
                        }
                        None => None,
                    }
                })
                .await;

                let (pcm_samples, pcm_sample_rate) = match result {
                    Ok(Some((s, sr))) => (s, sr),
                    _ => {
                        error!("[PocketTTS] generation failed for text = {}", text);
                        continue;
                    }
                };

                let (opus_pcm, _opus_sr) = if pcm_sample_rate != encode_sr as i32 {
                    let chunk_size = 4096.min(pcm_samples.len());
                    let mut resampler = FftFixedIn::<f32>::new(
                        pcm_sample_rate as usize,
                        encode_sr as usize,
                        chunk_size,
                        1,
                        1,
                    )
                    .expect("Failed to create resampler");
                    let mut all_output = Vec::new();
                    for chunk in pcm_samples.chunks(chunk_size) {
                        let out = if chunk.len() < chunk_size {
                            resampler
                                .process_partial(Some(&[chunk][..]), None)
                                .expect("Resampling failed")
                        } else {
                            resampler
                                .process(&[chunk], None)
                                .expect("Resampling failed")
                        };
                        all_output.extend_from_slice(&out[0]);
                    }
                    // flush resampler internal delay tail
                    if let Ok(tail) = resampler.process_partial(None::<&[&[f32]]>, None) {
                        all_output.extend_from_slice(&tail[0]);
                    }
                    (all_output, encode_sr as i32)
                } else {
                    (pcm_samples.clone(), pcm_sample_rate)
                };

                let audio_packets = encode_sample_to_tts_packet(
                    opus_pcm,
                    &mut encoder,
                    encode_sr,
                    output_channel,
                    output_frame_duration,
                );

                let data = TtsData {
                    audio: Some(audio_packets),
                    text: text.clone(),
                    raw_pcm: Some((pcm_samples, pcm_sample_rate)),
                };

                if tx.send(Ok(data)).await.is_err() {
                    break;
                }
            }
        });

        Box::pin(ReceiverStream::new(rx))
    }
}
