pub mod model;

use self::model::mute::TtsMute;
use self::model::pocket::TtsPocket;
use crate::config;
use crate::common::ModelError;
use crate::config::audio::AudioConfig;
use crate::config::tts::TtsConfig;
use async_trait::async_trait;
use futures::Stream;
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
    pub audio: Option<Vec<Vec<u8>>>,
    pub text: String,
    pub raw_pcm: Option<(Vec<f32>, i32)>,
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
    pub tts_config: Arc<TtsConfig>,
    pub audio_config: Arc<AudioConfig>,
}

impl TtsFactory {
    pub fn new(
        default_instance: Arc<Box<dyn Tts>>,
        tts_config: Arc<TtsConfig>,
        audio_config: Arc<AudioConfig>,
    ) -> Self {
        Self {
            default_instance,
            tts_config,
            audio_config,
        }
    }

    pub async fn init(
        tts_config: Arc<TtsConfig>,
        audio_config: Arc<AudioConfig>,
    ) -> Result<&'static Self, anyhow::Error> {
        let tts = Self::create_model(&tts_config, &audio_config).await?;
        Ok(INSTANCE.get_or_init(|| -> Self { Self::new(Arc::new(tts), tts_config, audio_config) }))
    }

    pub fn default(&self) -> Arc<Box<dyn Tts>> {
        self.default_instance.clone()
    }

    pub async fn create_model(
        tts_config: &TtsConfig,
        audio_config: &AudioConfig,
    ) -> Result<Box<dyn Tts>, anyhow::Error> {
        match tts_config.model.clone().expect("tts model is empty") {
            config::TtsModel::Mute => Ok(Box::new(TtsMute::new().await?)),
            config::TtsModel::PocketTts => {
                Ok(Box::new(TtsPocket::new(tts_config, audio_config).await?))
            }
        }
    }

    pub fn global() -> &'static TtsFactory {
        INSTANCE.get().unwrap()
    }
}

use crate::common::ModelErrorCode;
use framework::err;
use framework::error::AppError;

impl From<TtsError> for AppError {
    fn from(value: TtsError) -> Self {
        err!(ModelErrorCode::Tts).with_extra(value.to_string())
    }
}

pub fn encode_sample_to_tts_packet(
    sample: Vec<f32>,
    encoder: &mut opus_rs::OpusEncoder,
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
        let mut output = vec![0u8; 4000];
        let out_len = encoder.encode(&sample[start..end], size, &mut output).unwrap();
        output.truncate(out_len);
        audio.push(output);
    }
    audio
}

pub fn calcalute_tts_packet_size(sample_rate: u32, channel: u32, delay_millis: u64) -> usize {
    sample_rate as usize * channel as usize * delay_millis as usize / 1000
}
