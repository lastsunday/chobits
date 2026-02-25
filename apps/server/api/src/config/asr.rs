use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct AsrConfig {
    #[serde(default)]
    pub path: Option<String>,
}
