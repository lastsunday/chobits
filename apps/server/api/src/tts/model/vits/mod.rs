use std::pin::Pin;
use std::sync::Arc;
use async_trait::async_trait;
use futures::Stream;
use rubato::{FftFixedIn, Resampler};
use sherpa_onnx::{
    GenerationConfig, OfflineTts, OfflineTtsConfig, OfflineTtsModelConfig,
    OfflineTtsVitsModelConfig,
};
use tokio::sync::mpsc::channel;
use tokio_stream::StreamExt;
use tokio_stream::wrappers::ReceiverStream;
use tracing::{debug, error};


use crate::common::ModelError;
use crate::config::audio::AudioConfig;
use crate::config::tts::TtsConfig;
use crate::tts::{Tts, TtsData, TtsError, encode_sample_to_tts_packet};

pub struct TtsVits {
    tts: Arc<OfflineTts>,
    output_sample_rate: u32,
    output_channel: u32,
    output_frame_duration: u64,
    speed: f32,
    sid: i32,
}

impl TtsVits {
    pub async fn new(
        tts_config: &TtsConfig,
        audio_config: &AudioConfig,
    ) -> Result<Self, anyhow::Error> {
        let path = tts_config
            .path
            .as_deref()
            .unwrap_or("data/tts/model/vits/melo-tts-zh_en/");
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

        let noise_scale = opts
            .and_then(|o| o.get("noise_scale"))
            .and_then(|v| v.as_f64())
            .unwrap_or(0.667) as f32;

        let noise_scale_w = opts
            .and_then(|o| o.get("noise_scale_w"))
            .and_then(|v| v.as_f64())
            .unwrap_or(0.8) as f32;

        let length_scale = opts
            .and_then(|o| o.get("length_scale"))
            .and_then(|v| v.as_f64())
            .unwrap_or(1.0) as f32;

        let speed = opts
            .and_then(|o| o.get("speed"))
            .and_then(|v| v.as_f64())
            .unwrap_or(1.0) as f32;

        let sid = opts
            .and_then(|o| o.get("sid"))
            .and_then(|v| v.as_i64())
            .unwrap_or(0) as i32;

        let dict_dir = opts
            .and_then(|o| o.get("dict_dir"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let model = opts
            .and_then(|o| o.get("model"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .or_else(|| {
                let dir = std::path::Path::new(path);
                std::fs::read_dir(dir).ok().and_then(|mut entries| {
                    entries.find_map(|entry| {
                        entry.ok().and_then(|e| {
                            let p = e.path();
                            if p.extension().is_some_and(|ext| ext == "onnx") {
                                p.to_str().map(|s| s.to_string())
                            } else {
                                None
                            }
                        })
                    })
                })
            });

        let model_path =
            model.ok_or_else(|| anyhow::anyhow!("VITS model file (.onnx) not found in {path}"))?;

        let tokens = opts
            .and_then(|o| o.get("tokens"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("{path}tokens.txt"));

        let lexicon = opts
            .and_then(|o| o.get("lexicon"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("{path}lexicon.txt"));

        let data_dir = opts
            .and_then(|o| o.get("data_dir"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .or_else(|| {
                let candidate = format!("{path}espeak-ng-data");
                if std::path::Path::new(&candidate).is_dir() {
                    Some(candidate)
                } else {
                    None
                }
            });

        let rule_fsts = {
            let p = std::path::Path::new(path);
            let mut files: Vec<String> = Vec::new();
            if let Ok(entries) = std::fs::read_dir(p) {
                for entry in entries.flatten() {
                    let ep = entry.path();
                    if ep.extension().is_some_and(|ext| ext == "fst") {
                        files.push(ep.to_string_lossy().into_owned());
                    }
                }
            }
            files.sort();
            if files.is_empty() { None } else { Some(files.join(",")) }
        };

        let rule_fars = {
            let p = std::path::Path::new(path).join("rule.far");
            if p.is_file() { Some(p.to_string_lossy().into_owned()) } else { None }
        };

        let vits_config = OfflineTtsVitsModelConfig {
            model: Some(model_path),
            tokens: Some(tokens),
            lexicon: Some(lexicon),
            data_dir,
            dict_dir,
            noise_scale,
            noise_scale_w,
            length_scale,
        };

        let config = OfflineTtsConfig {
            model: OfflineTtsModelConfig {
                vits: vits_config,
                num_threads,
                debug,
                ..Default::default()
            },
            rule_fsts,
            rule_fars,
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

        Ok(Self {
            tts: Arc::new(tts),
            output_sample_rate,
            output_channel,
            output_frame_duration,
            speed,
            sid,
        })
    }
}

#[async_trait]
impl Tts for TtsVits {
    async fn stream(
        &self,
        text_stream: Pin<
            Box<dyn Stream<Item = core::result::Result<String, ModelError>> + Send + Sync>,
        >,
    ) -> Pin<Box<dyn Stream<Item = core::result::Result<TtsData, TtsError>> + Send + Sync>> {
        let (tx, rx) = channel::<core::result::Result<TtsData, TtsError>>(10);

        let tts = self.tts.clone();
        let output_sample_rate = self.output_sample_rate;
        let output_channel = self.output_channel;
        let output_frame_duration = self.output_frame_duration;
        let speed = self.speed;
        let sid = self.sid;

        tokio::spawn(async move {
            let mut pinned = text_stream;
            let encode_sr = output_sample_rate;
            let channels = match output_channel {
                2 => 2_usize,
                _ => 1_usize,
            };
            let mut encoder =
                match opus_rs::OpusEncoder::new(encode_sr as i32, channels, opus_rs::Application::Audio) {
                    Ok(e) => e,
                    Err(err) => {
                        error!("[VitsTTS] opus encoder creation error = {}", err);
                        return;
                    }
                };

            while let Some(text_result) = pinned.next().await {
                let text = match text_result {
                    Ok(t) => t,
                    Err(e) => {
                        error!("[VitsTTS] text stream error = {}", e);
                        let _ = tx.send(Err(TtsError::Text(e.to_string()))).await;
                        break;
                    }
                };

                debug!("[VitsTTS] generating audio for text = {}", text);

                let tts_clone = tts.clone();
                let text_clone = text.clone();
                let result = tokio::task::spawn_blocking(move || {
                    let gen_config = GenerationConfig {
                        speed,
                        sid,
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
                        error!("[VitsTTS] generation failed for text = {}", text);
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
