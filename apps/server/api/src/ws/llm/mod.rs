pub mod chat;
pub mod client;
pub mod models;

use crate::config;
use async_trait::async_trait;
use rig::{
    completion::{CompletionError, CompletionRequest, CompletionResponse},
    streaming::StreamingCompletionResponse,
};
use std::sync::{Arc, OnceLock};

#[async_trait]
pub trait Model: Send + Sync {
    async fn completion(
        &self,
        request: CompletionRequest,
    ) -> Result<CompletionResponse<rig::providers::openai::CompletionResponse>, CompletionError>;

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
    async fn completion(
        &self,
        _request: CompletionRequest,
    ) -> Result<CompletionResponse<rig::providers::openai::CompletionResponse>, CompletionError>
    {
        todo!()
    }

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
    default_client: Arc<client::Client>,
}

impl LlmFactory {
    pub fn new(default_client: Arc<client::Client>) -> Self {
        Self { default_client }
    }

    pub async fn init() -> &'static Self {
        let llm = LlmFactory::create_model();
        let client = client::ClientBuilder::new()
            .with_client(Arc::new(llm))
            .build();
        INSTANCE.get_or_init(|| -> Self { Self::new(Arc::new(client)) })
    }

    // TODO: modify to create client?
    pub fn get_client(&self) -> Arc<client::Client> {
        self.default_client.clone()
    }

    pub fn create_model() -> Box<dyn Model> {
        let app_config = config::get();
        let llm_config = app_config.llm();
        let llm = models::qwen3::LlmQwen::new(
            llm_config.model().to_string(),
            llm_config.tokens().to_string(),
        )
        .unwrap();
        Box::new(llm)
    }

    pub fn global() -> &'static LlmFactory {
        INSTANCE.get().unwrap()
    }
}
