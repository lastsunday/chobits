use crate::config;
use sherpa_rs::sense_voice::{SenseVoiceConfig, SenseVoiceRecognizer};
use std::sync::{Arc, OnceLock};
use tokio::sync::Mutex;

use crate::ws::asr::SenseVoiceAsr;

static ASR_INSTANCE: OnceLock<AsrCache> = OnceLock::new();

pub struct AsrCache {
    pub instance: SenseVoiceAsr,
}

impl AsrCache {
    pub fn new(instance: SenseVoiceAsr) -> Self {
        Self { instance }
    }

    pub async fn init() -> &'static Self {
        let app_config = config::get();
        let asr_config = app_config.asr();
        let config = SenseVoiceConfig {
            model: asr_config.model().into(),
            tokens: asr_config.tokens().into(),
            language: asr_config.language().into(),
            num_threads: Some(asr_config.num_threads()),
            provider: Some(String::from("cpu")),
            ..Default::default()
        };
        let asr_instance = SenseVoiceRecognizer::new(config).unwrap();
        let asr = SenseVoiceAsr::new(Arc::new(Mutex::new(asr_instance)));
        ASR_INSTANCE.get_or_init(|| -> Self { Self::new(asr) })
    }

    pub fn global() -> &'static AsrCache {
        ASR_INSTANCE.get().unwrap()
    }
}
