use serde::Deserialize;

#[derive(Debug, Deserialize, Clone, Default)]
pub struct WsConfig {
    #[serde(default)]
    pub schema: Option<String>,
}
