use crate::{config, ws::asr::AsrWhisper};
use std::sync::OnceLock;

static INSTANCE: OnceLock<AsrCache> = OnceLock::new();

pub struct AsrCache {
    pub instance: AsrWhisper,
}

impl AsrCache {
    pub fn new(instance: AsrWhisper) -> Self {
        Self { instance }
    }

    pub async fn init() -> &'static Self {
        let vad = Self::create();
        INSTANCE.get_or_init(|| -> Self { Self::new(vad) })
    }

    pub fn global() -> &'static AsrCache {
        INSTANCE.get().unwrap()
    }

    pub fn create() -> AsrWhisper {
        let app_config = config::get();
        let config = app_config.asr();
        AsrWhisper::new(
            config.model().to_string(),
            config.config().to_string(),
            config.tokens().to_string(),
        )
        .unwrap()
    }
}
