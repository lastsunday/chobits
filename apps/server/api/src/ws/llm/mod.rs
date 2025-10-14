pub mod models;

use crate::config;
use crate::ws::common::ModelError;
use futures::Stream;
use models::LlmQwen;
use std::pin::Pin;
use std::sync::{Arc, OnceLock};

pub trait Llm: Send + Sync {
    fn chat(
        &self,
        system_prompt: String,
        text: String,
    ) -> Pin<Box<dyn Stream<Item = Result<String, ModelError>> + Send>>;
}

static INSTANCE: OnceLock<LlmFactory> = OnceLock::new();

pub struct LlmFactory {
    default_llm: Arc<Box<dyn Llm>>,
}

impl LlmFactory {
    pub fn new(default_llm: Arc<Box<dyn Llm>>) -> Self {
        Self { default_llm }
    }

    pub async fn init() -> &'static Self {
        let app_config = config::get();
        let llm_config = app_config.llm();
        let llm = LlmQwen::new(
            llm_config.model().to_string(),
            llm_config.tokens().to_string(),
        )
        .unwrap();
        INSTANCE.get_or_init(|| -> Self { Self::new(Arc::new(Box::new(llm))) })
    }

    pub fn get_llm(&self) -> Arc<Box<dyn Llm>> {
        self.default_llm.clone()
    }

    pub fn global() -> &'static LlmFactory {
        INSTANCE.get().unwrap()
    }
}
