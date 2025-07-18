pub use sherpa_rs::tts::{KokoroTts, KokoroTtsConfig};
use sherpa_rs::{OnnxConfig, get_default_provider};
use std::sync::{Arc, OnceLock};
use tokio::sync::Mutex;

use crate::ws::tts::TtsKokoro;

static TTS_INSTANCE: OnceLock<TtsCache> = OnceLock::new();

pub struct TtsCache {
    pub instance: Box<TtsKokoro>,
}

impl TtsCache {
    pub fn new(instance: Box<TtsKokoro>) -> Self {
        Self { instance }
    }

    pub async fn init() -> &'static Self {
        let config = KokoroTtsConfig {
            model: "./kokoro-multi-lang-v1_1/model.onnx".to_string(),
            voices: "./kokoro-multi-lang-v1_1/voices.bin".into(),
            tokens: "./kokoro-multi-lang-v1_1/tokens.txt".into(),
            data_dir: "./kokoro-multi-lang-v1_1/espeak-ng-data".into(),
            dict_dir: "./kokoro-multi-lang-v1_1/dict".into(),
            lexicon:
                "./kokoro-multi-lang-v1_1/lexicon-us-en.txt,./kokoro-multi-lang-v1_1/lexicon-zh.txt"
                    .into(),
            length_scale: 1.0,
            onnx_config: OnnxConfig {
                provider: get_default_provider(),
                debug: false,
                num_threads: 8,
            },
            ..Default::default()
        };
        let tts_instance = KokoroTts::new(config);
        let tts = TtsKokoro::new(Arc::new(Mutex::new(tts_instance)));
        TTS_INSTANCE.get_or_init(|| -> Self { Self::new(Box::new(tts)) })
    }

    pub fn global() -> &'static TtsCache {
        TTS_INSTANCE.get().unwrap()
    }
}
