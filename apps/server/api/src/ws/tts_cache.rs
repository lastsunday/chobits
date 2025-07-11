use std::sync::{Arc, OnceLock};

use kokoro_tts::KokoroTts;
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
        let tts_instance = KokoroTts::new("kokoro-v1.1-zh.onnx", "voices-v1.1-zh.bin")
            .await
            .unwrap();
        let tts = TtsKokoro::new(Arc::new(Mutex::new(tts_instance)));
        TTS_INSTANCE.get_or_init(|| -> Self { Self::new(Box::new(tts)) })
    }

    pub fn global() -> &'static TtsCache {
        TTS_INSTANCE.get().unwrap()
    }
}
