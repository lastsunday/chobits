pub mod model;

use crate::{common::ModelError, config::asr::AsrConfig};
use async_trait::async_trait;
use model::whisper::AsrWhisper;
use std::sync::{Arc, OnceLock};
use tokio::sync::Mutex;

#[async_trait]
pub trait Asr: Send + Sync {
    async fn transcribe(
        &mut self,
        sample_rate: u32,
        samples: &[f32],
    ) -> Result<RecognizerResult, ModelError>;
}

#[derive(Debug, Clone)]
pub struct RecognizerResult {
    pub text: String,
    pub language: String,
    pub prob: f32,
}

static INSTANCE: OnceLock<AsrFactory> = OnceLock::new();

pub struct AsrFactory {
    default_instance: Arc<Mutex<Box<dyn Asr>>>,
    pub config: Arc<AsrConfig>,
}

impl AsrFactory {
    pub fn new(default_instance: Arc<Mutex<Box<dyn Asr>>>, config: Arc<AsrConfig>) -> Self {
        Self {
            default_instance,
            config,
        }
    }

    pub async fn init(config: Arc<AsrConfig>) -> &'static Self {
        INSTANCE.get_or_init(|| -> Self {
            Self::new(Arc::new(Mutex::new(Self::create_model(&config))), config)
        })
    }

    pub fn global() -> &'static AsrFactory {
        INSTANCE.get().unwrap()
    }

    pub fn default(&self) -> Arc<Mutex<Box<dyn Asr>>> {
        self.default_instance.clone()
    }

    pub fn create_model(config: &AsrConfig) -> Box<dyn Asr> {
        let config = AsrConfig {
            path: config.path.clone(),
        };
        Box::new(
            AsrWhisper::new(config.path.clone().expect("asr path is empty").to_string()).unwrap(),
        )
    }
}
