use serde::Deserialize;

#[derive(Debug, Deserialize, Clone, Default)]
pub struct LlmConfig {
    #[serde(default)]
    pub model: Option<super::LlmModel>,
    #[serde(default)]
    pub path: Option<String>,
}
