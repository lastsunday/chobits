use serde::Deserialize;

#[derive(Debug, Default, Deserialize, Clone)]
pub struct LlmConfig {
    model: Option<String>,
    tokens: Option<String>,
}

impl LlmConfig {
    pub fn new() -> Self {
        Self {
            model: Some(String::from("data/llm/model.gguf")),
            tokens: Some(String::from("data/llm/tokenizer.json")),
        }
    }

    pub fn model(&self) -> &str {
        self.model.as_deref().unwrap_or_default()
    }

    pub fn tokens(&self) -> &str {
        self.tokens.as_deref().unwrap_or_default()
    }
}
