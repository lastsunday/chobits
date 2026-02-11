pub mod chat;
pub mod client;
pub mod model;

use crate::{
    config::{self, llm::LlmConfig},
    llm::model::{minicpm4::Minicpm4, qwen3::LlmQwen},
};
use async_trait::async_trait;
use rig::{
    completion::{CompletionError, CompletionRequest, ToolDefinition},
    message::Message,
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

    fn calculate_system_prompt_len(&self, system_prompt: &Option<String>) -> u64;

    fn calculate_tools_prompt_len(&self, tools: &[ToolDefinition]) -> u64;

    fn calculate_message_prompt_len(&self, message: &Message) -> u64;
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

    fn calculate_system_prompt_len(&self, _system_prompt: &Option<String>) -> u64 {
        todo!()
    }

    fn calculate_tools_prompt_len(&self, _tools: &[ToolDefinition]) -> u64 {
        todo!()
    }

    fn calculate_message_prompt_len(&self, _message: &Message) -> u64 {
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
        let config = config::get();
        let llm_config = LlmConfig {
            model: config.llm_model.clone(),
            path: config.llm_path.clone(),
        };
        match llm_config.model.as_ref().expect("llm model is empty") {
            config::LlmModel::Qwen3 => Box::new(
                LlmQwen::new(llm_config.path.as_ref().expect("llm path is empty")).unwrap(),
            ),
            config::LlmModel::MiniCPM4 => Box::new(
                Minicpm4::new(llm_config.path.as_ref().expect("llm path is empty")).unwrap(),
            ),
        }
    }

    pub fn global() -> &'static LlmFactory {
        INSTANCE.get().unwrap()
    }
}
