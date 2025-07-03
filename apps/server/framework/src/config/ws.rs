use serde::Deserialize;

#[derive(Debug, Default, Deserialize)]
pub struct WebSocketConfig {
    schema: Option<String>,
}

impl WebSocketConfig {
    pub fn new() -> Self {
        Self {
            schema: Some(String::from("ws")),
        }
    }

    pub fn schema(&self) -> &str {
        self.schema.as_deref().unwrap_or("ws")
    }
}
