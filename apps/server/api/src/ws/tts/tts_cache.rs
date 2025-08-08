use crate::config;
use crate::ws::tts::TtsKokoro;
use kokoro_tts::KokoroTts;
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
        let tts_instance = KokoroTts::new(tts_config.model(), tts_config.voice())
            .await
            .unwrap();
        let tts = TtsKokoro::new(Arc::new(Mutex::new(tts_instance)));
        TTS_INSTANCE.get_or_init(|| -> Self { Self::new(Box::new(tts)) })
    }

    pub fn global() -> &'static TtsCache {
        TTS_INSTANCE.get().unwrap()
    }
}
