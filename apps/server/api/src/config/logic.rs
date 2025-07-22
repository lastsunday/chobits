use serde::Deserialize;

#[derive(Debug, Default, Deserialize, Clone)]
pub struct LogicConfig {
    /// unit: ms
    close_connection_no_voice_time: Option<i64>,
    /// unit: ms
    silence_voice_timeout: Option<i64>,
}

impl LogicConfig {
    pub fn new() -> Self {
        Self {
            close_connection_no_voice_time: Some(30000),
            silence_voice_timeout: Some(1200),
        }
    }

    pub fn close_connection_no_voice_time(&self) -> i64 {
        self.close_connection_no_voice_time.unwrap_or(30000)
    }

    pub fn silence_voice_timeout(&self) -> i64 {
        self.silence_voice_timeout.unwrap_or(1200)
    }
}
