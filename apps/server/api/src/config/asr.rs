use serde::Deserialize;

#[derive(Debug, Default, Deserialize, Clone)]
pub struct AsrConfig {
    path: Option<String>,
}

impl AsrConfig {
    pub fn new() -> Self {
        Self {
            path: Some(String::from("data/asr/model/openai/whisper-small/")),
        }
    }

    pub fn path(&self) -> &str {
        self.path.as_deref().unwrap_or_default()
    }
}
