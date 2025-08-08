use serde::Deserialize;

#[derive(Debug, Default, Deserialize, Clone)]
pub struct TtsConfig {
    model: Option<String>,
    voice: Option<String>,
}

impl TtsConfig {
    pub fn new() -> Self {
        Self {
            model: Some(String::from("data/tts/model.onnx")),
            voice: Some(String::from("data/tts/voice.bin")),
        }
    }

    pub fn model(&self) -> &str {
        self.model.as_deref().unwrap_or_default()
    }

    pub fn voice(&self) -> &str {
        self.voice.as_deref().unwrap_or_default()
    }
}
