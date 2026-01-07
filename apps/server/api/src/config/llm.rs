use serde::Deserialize;

#[derive(Debug, Deserialize, Default, Clone)]
pub struct LlmConfig {
    model: Option<Model>,
    path: Option<String>,
}

impl LlmConfig {
    pub fn new() -> Self {
        Self {
            model: Some(Model::Qwen3),
            path: Some(String::from("data/llm/model/unsloth/Qwen3-1.7B-GGUF/")),
            // path: Some(String::from("data/llm/model/unsloth/Qwen3-4B-GGUF/")),
            // model: Some(Model::MiniCPM4),
            // path: Some(String::from("data/llm/model/openbmb/MiniCPM4-0.5B/")),
        }
    }

    pub fn path(&self) -> &str {
        self.path.as_deref().unwrap_or_default()
    }

    pub fn model(&self) -> Model {
        self.model.clone().unwrap_or_default()
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Default)]
pub enum Model {
    #[default]
    Qwen3,
    MiniCPM4,
}
