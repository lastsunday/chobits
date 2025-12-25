pub mod chat;
pub mod client;
pub mod models;

use crate::{
    config,
    llm::models::{minicpm4::Minicpm4, qwen3::LlmQwen},
};
use async_trait::async_trait;
use rig::{
    completion::{CompletionError, CompletionRequest},
    streaming::StreamingCompletionResponse,
};
use std::sync::{Arc, OnceLock};

#[async_trait]
pub trait Model: Send + Sync {
    async fn stream(
        &self,
        request: CompletionRequest,
    ) -> Result<
        StreamingCompletionResponse<rig::providers::openai::streaming::StreamingCompletionResponse>,
        CompletionError,
    >;
}

#[derive(Default, Clone)]
pub struct DummyModel {}

#[async_trait]
impl Model for DummyModel {
    async fn stream(
        &self,
        _request: CompletionRequest,
    ) -> Result<
        StreamingCompletionResponse<rig::providers::openai::streaming::StreamingCompletionResponse>,
        CompletionError,
    > {
        todo!()
    }
}

static INSTANCE: OnceLock<LlmFactory> = OnceLock::new();

pub struct LlmFactory {
    default_llm: Arc<Box<dyn Model>>,
}

impl LlmFactory {
    pub fn new(default_llm: Arc<Box<dyn Model>>) -> Self {
        Self { default_llm }
    }

    pub async fn init() -> &'static Self {
        let llm = LlmFactory::create_model();
        INSTANCE.get_or_init(|| -> Self { Self::new(Arc::new(llm)) })
    }

    pub fn default(&self) -> Arc<Box<dyn Model>> {
        self.default_llm.clone()
    }

    pub fn create_model() -> Box<dyn Model> {
        let app_config = config::get();
        let llm_config = app_config.llm();
        match llm_config.model() {
            config::llm::Model::Qwen3 => Box::new(LlmQwen::new(llm_config.path()).unwrap()),
            config::llm::Model::MiniCPM4 => Box::new(Minicpm4::new(llm_config.path()).unwrap()),
        }
    }

    pub fn global() -> &'static LlmFactory {
        INSTANCE.get().unwrap()
    }
}
