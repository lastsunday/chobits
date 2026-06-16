use serde::Deserialize;

#[derive(Debug, Deserialize, Clone, Default)]
pub struct VadConfig {
    #[serde(default)]
    pub model: Option<super::VadModel>,
    #[serde(default)]
    pub variant: Option<String>,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub num_threads: Option<i32>,
}
