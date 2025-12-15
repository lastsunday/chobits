pub mod model;

use crate::config;
use crate::ws::common::ModelError;
use async_trait::async_trait;
use futures::Stream;
use model::kokoro::TtsKokoro;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::OnceLock;

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
    #[error("encode error")]
    Encode,
    #[error("text error")]
    Text,
}

static INSTANCE: OnceLock<TtsFactory> = OnceLock::new();

pub struct TtsFactory {
    pub default_tts: Arc<Box<dyn Tts>>,
}

impl TtsFactory {
    pub fn new(default_tts: Arc<Box<dyn Tts>>) -> Self {
        Self { default_tts }
    }

    pub async fn init() -> &'static Self {
        let tts = Self::create_model().await;
        INSTANCE.get_or_init(|| -> Self { Self::new(Arc::new(tts)) })
    }

    pub async fn create_model() -> Box<dyn Tts> {
        let app_config = config::get();
        let tts_config = app_config.tts();

        match tts_config.model() {
            config::tts::Model::Kokoro => Box::new(TtsKokoro::new(tts_config.path()).await),
            config::tts::Model::Voxcpm => todo!(),
        }
    }

    pub fn global() -> &'static TtsFactory {
        INSTANCE.get().unwrap()
    }
}
