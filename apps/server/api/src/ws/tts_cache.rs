use crate::config;
use crate::ws::tts::TtsKokoro;
pub use sherpa_rs::tts::{KokoroTts, KokoroTtsConfig};
use sherpa_rs::{OnnxConfig, get_default_provider};
use std::sync::{Arc, OnceLock};
use tokio::sync::Mutex;

static TTS_INSTANCE: OnceLock<TtsCache> = OnceLock::new();

pub struct TtsCache {
    pub instance: Box<TtsKokoro>,
}

impl TtsCache {
    pub fn new(instance: Box<TtsKokoro>) -> Self {
        Self { instance }
    }

    pub async fn init() -> &'static Self {
        let app_config = config::get();
        let tts_config = app_config.tts();
        let config = KokoroTtsConfig {
            model: tts_config.model().into(),
            voices: tts_config.voices().into(),
            tokens: tts_config.tokens().into(),
            data_dir: tts_config.data_dir().into(),
            dict_dir: tts_config.data_dir().into(),
            lexicon: tts_config.lexicon().into(),
            length_scale: 1.0,
            onnx_config: OnnxConfig {
                provider: get_default_provider(),
                debug: false,
                num_threads: tts_config.num_threads(),
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
