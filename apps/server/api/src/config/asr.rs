use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct AsrConfig {
    #[serde(default)]
    pub model: Option<super::AsrModel>,
    #[serde(default)]
    pub path: Option<String>,
}
