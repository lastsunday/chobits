use serde::Deserialize;

#[derive(Debug, Deserialize, Clone, Default)]
pub struct McpConfig {
    #[serde(default)]
    pub uri_list: Option<Vec<String>>,
}
