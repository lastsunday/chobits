use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct LogicConfig {
    /// unit: ms
    #[serde(default)]
    pub close_connection_no_voice_time: Option<i64>,
    /// unit: ms
    #[serde(default)]
    pub silence_voice_timeout: Option<i64>,
    #[serde(default)]
    pub system_prompt: Option<String>,
    #[serde(default)]
    pub max_prompt_len: Option<u64>,
}
