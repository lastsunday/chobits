use serde::Deserialize;

#[derive(Debug, Default, Deserialize, Clone)]
pub struct AsrConfig {
    model: Option<String>,
    tokens: Option<String>,
    language: Option<String>,
    num_threads: Option<i32>,
}

impl AsrConfig {
    pub fn new() -> Self {
        Self {
            model: Some(String::from(
                "data/asr/sherpa-onnx-sense-voice-zh-en-ja-ko-yue-2024-07-17/model.onnx",
            )),
            tokens: Some(String::from(
                "data/asr/sherpa-onnx-sense-voice-zh-en-ja-ko-yue-2024-07-17/tokens.txt",
            )),
            language: Some(String::from("auto")),
            num_threads: Some(4),
        }
    }

    pub fn model(&self) -> &str {
        self.model
            .as_deref()
            .unwrap_or("data/asr/sherpa-onnx-sense-voice-zh-en-ja-ko-yue-2024-07-17/model.onnx")
    }

    pub fn tokens(&self) -> &str {
        self.tokens
            .as_deref()
            .unwrap_or("data/asr/sherpa-onnx-sense-voice-zh-en-ja-ko-yue-2024-07-17/tokens.txt")
    }

    pub fn language(&self) -> &str {
        self.language.as_deref().unwrap_or("auto")
    }

    pub fn num_threads(&self) -> i32 {
        self.num_threads.unwrap_or(4_i32)
    }
}
