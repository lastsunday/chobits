use serde::Deserialize;

#[derive(Debug, Default, Deserialize, Clone)]
pub struct TtsConfig {
    model: Option<Model>,
    path: Option<String>,
}

impl TtsConfig {
    pub fn new() -> Self {
        Self {
            model: Some(Model::Kokoro),
            path: Some(String::from("data/tts/mzdk100/kokoro/")),
        }
    }

    pub fn path(&self) -> &str {
        self.path.as_deref().unwrap_or_default()
    }

    pub fn model(&self) -> Model {
        self.model.clone().unwrap_or_default()
    }
}
#[derive(Clone, Debug, Deserialize, PartialEq, Default)]
pub enum Model {
    #[default]
    Kokoro,
    Voxcpm,
}
