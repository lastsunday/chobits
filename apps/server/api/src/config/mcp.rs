use serde::Deserialize;

#[derive(Debug, Default, Deserialize, Clone)]
pub struct McpConfig {
    uri_list: Option<Vec<String>>,
}

impl McpConfig {
    pub fn new() -> Self {
        Self {
            uri_list: Some(vec![String::from("http://127.0.0.1:3000/mcp")]),
        }
    }

    pub fn uri_list(&self) -> Vec<String> {
        self.uri_list.clone().unwrap_or_default()
    }
}
