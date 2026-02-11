pub mod model;

use crate::config;
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
}

impl AsrFactory {
    pub fn new(default_instance: Arc<Mutex<Box<dyn Asr>>>) -> Self {
        Self { default_instance }
    }

    pub async fn init() -> &'static Self {
        INSTANCE.get_or_init(|| -> Self { Self::new(Arc::new(Mutex::new(Self::create_model()))) })
    }

    pub fn global() -> &'static AsrFactory {
        INSTANCE.get().unwrap()
    }

    pub fn default(&self) -> Arc<Mutex<Box<dyn Asr>>> {
        self.default_instance.clone()
    }

    pub fn create_model() -> Box<dyn Asr> {
        let config = config::get();
        let config = AsrConfig {
            path: config.asr_path.clone(),
        };
        Box::new(
            AsrWhisper::new(config.path.clone().expect("asr path is empty").to_string()).unwrap(),
        )
    }
}
