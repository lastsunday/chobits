use crate::{config, ws::llm::LlmQwen};
use std::sync::{Arc, OnceLock};
use tokio::sync::Mutex;

static INSTANCE: OnceLock<LlmCache> = OnceLock::new();

pub struct LlmCache {
    pub instance: Box<LlmQwen>,
}

impl LlmCache {
    pub fn new(instance: Box<LlmQwen>) -> Self {
        Self { instance }
    }

    pub async fn init() -> &'static Self {
        let app_config = config::get();
        let llm_config = app_config.llm();
        let llm = LlmQwen::new(
            llm_config.model().to_string(),
            llm_config.tokens().to_string(),
        )
        .unwrap();
        INSTANCE.get_or_init(|| -> Self { Self::new(Box::new(llm)) })
    }

    pub fn global() -> &'static LlmCache {
        INSTANCE.get().unwrap()
    }
}
