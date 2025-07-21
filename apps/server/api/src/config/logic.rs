use serde::Deserialize;

#[derive(Debug, Default, Deserialize, Clone)]
pub struct LogicConfig {
    /// unit: ms
    close_connection_no_voice_time: Option<i64>,
}

impl LogicConfig {
    pub fn new() -> Self {
        Self {
            close_connection_no_voice_time: Some(30000),
        }
    }

    pub fn close_connection_no_voice_time(&self) -> i64 {
        self.close_connection_no_voice_time.unwrap_or(30000)
    }
}
