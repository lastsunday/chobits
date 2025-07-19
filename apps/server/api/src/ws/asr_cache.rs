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
        let config = SenseVoiceConfig {
            model: "sherpa-onnx-sense-voice-zh-en-ja-ko-yue-2024-07-17/model.onnx".into(),
            tokens: "sherpa-onnx-sense-voice-zh-en-ja-ko-yue-2024-07-17/tokens.txt".into(),
            language: String::from("auto"),
            num_threads: Some(4),
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
