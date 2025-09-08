use serde::Deserialize;

#[derive(Debug, Default, Deserialize, Clone)]
pub struct LogicConfig {
    /// unit: ms
    close_connection_no_voice_time: Option<i64>,
    /// unit: ms
    silence_voice_timeout: Option<i64>,
    system_prompt: Option<String>,
    system_wake_prompt: Option<String>,
    system_listen_unclear_prompt: Option<String>,
}

impl LogicConfig {
    pub fn new() -> Self {
        Self {
            close_connection_no_voice_time: Some(30000),
            silence_voice_timeout: Some(1200),
            system_prompt: Some(String::from(
                "你是一个助手，所有回答必须使用纯文本自然语言，禁止使用任何Markdown符号如#、-、*等。",
            )),
            system_wake_prompt: Some(String::from(
                "你是一个助手，所有回答必须使用纯文本自然语言，禁止使用任何Markdown符号如#、-、*等。现在用户向你打招呼，请有礼貌作出回应。",
            )),
            system_listen_unclear_prompt: Some(String::from(
                "你是一个助手，所有回答必须使用纯文本自然语言，禁止使用任何Markdown符号如#、-、*等。现在用户向你对话，但你没有听清楚用户的问题，请有礼貌作出回应。",
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

    pub fn system_wake_prompt(&self) -> &str {
        self.system_wake_prompt.as_deref().unwrap_or_default()
    }

    pub fn system_listen_unclear_prompt(&self) -> &str {
        self.system_listen_unclear_prompt
            .as_deref()
            .unwrap_or_default()
    }
}
