use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct LlmConfig {
    #[serde(default)]
    pub model: Option<super::LlmModel>,
    #[serde(default)]
    pub path: Option<String>,
}
