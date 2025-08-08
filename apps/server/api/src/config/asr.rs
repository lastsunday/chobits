use serde::Deserialize;

#[derive(Debug, Default, Deserialize, Clone)]
pub struct AsrConfig {
    model: Option<String>,
    tokens: Option<String>,
    config: Option<String>,
}

impl AsrConfig {
    pub fn new() -> Self {
        Self {
            model: Some(String::from("data/asr/model.safetensors")),
            tokens: Some(String::from("data/asr/tokenizer.json")),
            config: Some(String::from("data/asr/config.json")),
        }
    }

    pub fn model(&self) -> &str {
        self.model.as_deref().unwrap_or_default()
    }

    pub fn tokens(&self) -> &str {
        self.tokens.as_deref().unwrap_or_default()
    }

    pub fn config(&self) -> &str {
        self.config.as_deref().unwrap_or_default()
    }
}
