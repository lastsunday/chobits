use tokio::sync::Mutex;

use super::Llm;
use super::models::LlmQwen;
use crate::config;
use std::sync::{Arc, OnceLock};

static INSTANCE: OnceLock<LlmCache> = OnceLock::new();

pub struct LlmCache {
    pub instance: Arc<Mutex<Box<dyn Llm>>>,
}

impl LlmCache {
    pub fn new(instance: Arc<Mutex<Box<dyn Llm>>>) -> Self {
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
        INSTANCE.get_or_init(|| -> Self { Self::new(Arc::new(Mutex::new(Box::new(llm)))) })
    }

    pub fn global() -> &'static LlmCache {
        INSTANCE.get().unwrap()
    }
}
