use serde::Deserialize;

#[derive(Debug, Default, Deserialize, Clone)]
pub struct LogicConfig {
    /// unit: ms
    close_connection_no_voice_time: Option<i64>,
    /// unit: ms
    silence_voice_timeout: Option<i64>,
    system_prompt: Option<String>,
}

impl LogicConfig {
    pub fn new() -> Self {
        Self {
            close_connection_no_voice_time: Some(30000),
            silence_voice_timeout: Some(1200),
            system_prompt: Some(String::from(
                "你是一个助手，所有回答必须使用纯文本自然语言，禁止使用任何Markdown符号如#、-、*等并且数字使用中文字代替。",
            )),
        }
    }

    pub fn close_connection_no_voice_time(&self) -> i64 {
        self.close_connection_no_voice_time.unwrap()
    }

    pub fn silence_voice_timeout(&self) -> i64 {
        self.silence_voice_timeout.unwrap()
    }

    pub fn system_prompt(&self) -> &str {
        self.system_prompt.as_deref().unwrap_or_default()
    }
}
