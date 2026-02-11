pub mod model;

use self::model::voxcpm::TtsVoxCPM;
use crate::common::ModelError;
use crate::config;
use crate::config::audio::AudioConfig;
use crate::config::tts::TtsConfig;
use async_trait::async_trait;
use futures::Stream;
use model::kokoro::TtsKokoro;
use std::pin::Pin;
use std::sync::OnceLock;
use std::{cmp, sync::Arc};

#[async_trait]
pub trait Tts: Send + Sync {
    async fn stream(
        &self,
        text_stream: Pin<
            Box<dyn Stream<Item = core::result::Result<String, ModelError>> + Send + Sync>,
        >,
    ) -> Pin<Box<dyn Stream<Item = core::result::Result<TtsData, TtsError>> + Send + Sync>>;
}

pub struct TtsData {
    pub audio: Vec<Vec<u8>>,
    pub text: String,
}

#[derive(Debug, thiserror::Error)]
pub enum TtsError {
    #[error("init error")]
    Init,
    #[error("encode error {0}")]
    Encode(String),
    #[error("text error {0}")]
    Text(String),
}

static INSTANCE: OnceLock<TtsFactory> = OnceLock::new();

pub struct TtsFactory {
    default_instance: Arc<Box<dyn Tts>>,
}

impl TtsFactory {
    pub fn new(default_instance: Arc<Box<dyn Tts>>) -> Self {
        Self { default_instance }
    }

    pub async fn init() -> Result<&'static Self, anyhow::Error> {
        let tts = Self::create_model().await?;
        Ok(INSTANCE.get_or_init(|| -> Self { Self::new(Arc::new(tts)) }))
    }

    pub fn default(&self) -> Arc<Box<dyn Tts>> {
        self.default_instance.clone()
    }

    pub async fn create_model() -> Result<Box<dyn Tts>, anyhow::Error> {
        let config = config::get();
        let tts_config = TtsConfig {
            model: config.tts_model.clone(),
            path: config.tts_path.clone(),
            reference_prompt_text: config.tts_reference_prompt_text.clone(),
            reference_prompt_wav_path: config.tts_reference_prompt_wav_path.clone(),
        };
        let audio_config = AudioConfig {
            input_sample_rate: config.audio_input_sample_rate,
            input_frame_duration: config.audio_input_frame_duration,
            input_channel: config.audio_input_channel,
            output_sample_rate: config.audio_output_sample_rate,
            output_channel: config.audio_output_channel,
            output_frame_duration: config.audio_output_frame_duration,
        };
        match tts_config.model.clone().expect("tts model is empty") {
            config::TtsModel::Kokoro => Ok(Box::new(
                TtsKokoro::new(
                    &tts_config.path.clone().expect("tts path is empty"),
                    audio_config
                        .output_sample_rate
                        .expect("tts output sample rate is empty"),
                    audio_config
                        .output_channel
                        .expect("tts output channel is empty"),
                    audio_config
                        .output_frame_duration
                        .expect("tts output frame duration is empty"),
                )
                .await?,
            )),
            config::TtsModel::Voxcpm => Ok(Box::new(
                TtsVoxCPM::new(
                    &tts_config.path.clone().expect("tts path is empty"),
                    audio_config
                        .output_sample_rate
                        .expect("tts output sample rate is empty"),
                    audio_config
                        .output_channel
                        .expect("tts output channel is empty"),
                    audio_config
                        .output_frame_duration
                        .expect("tts output frame duration is empty"),
                    tts_config.reference_prompt_text.clone(),
                    tts_config.reference_prompt_wav_path.clone(),
                )
                .await?,
            )),
        }
    }

    pub fn global() -> &'static TtsFactory {
        INSTANCE.get().unwrap()
    }
}

pub fn encode_sample_to_tts_packet(
    sample: Vec<f32>,
    encoder: &mut opus::Encoder,
    encode_sample_rate: u32,
    encode_channel: u32,
    encode_frame_duration: u64,
) -> Vec<Vec<u8>> {
    let len = sample.len();
    let size = calcalute_tts_packet_size(encode_sample_rate, encode_channel, encode_frame_duration);
    let count = len / size;
    let mut audio: Vec<Vec<u8>> = Vec::new();
    for n in 1..count {
        let start = (n - 1) * size;
        let end = cmp::min(n * size, len);
        let packet = encoder.encode_vec_float(&sample[start..end], size).unwrap();
        audio.push(packet);
    }
    audio
}

pub fn calcalute_tts_packet_size(sample_rate: u32, channel: u32, delay_millis: u64) -> usize {
    sample_rate as usize * channel as usize * delay_millis as usize / 1000
}
