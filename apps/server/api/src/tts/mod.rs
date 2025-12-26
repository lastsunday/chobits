pub mod model;

use self::model::voxcpm::TtsVoxCPM;
use crate::common::ModelError;
use crate::config;
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
    #[error("text error")]
    Text,
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
        let app_config = config::get();
        let tts_config = app_config.tts();
        let audio_config = app_config.audio();
        match tts_config.model() {
            config::tts::Model::Kokoro => Ok(Box::new(
                TtsKokoro::new(
                    tts_config.path(),
                    audio_config.output_sample_rate(),
                    audio_config.output_channel(),
                    audio_config.output_frame_duration(),
                )
                .await?,
            )),
            config::tts::Model::Voxcpm => Ok(Box::new(
                TtsVoxCPM::new(
                    tts_config.path(),
                    audio_config.output_sample_rate(),
                    audio_config.output_channel(),
                    audio_config.output_frame_duration(),
                    Some(tts_config.reference_prompt_text().to_string()),
                    Some(tts_config.reference_prompt_wav_path().to_string()),
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
