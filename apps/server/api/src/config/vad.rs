use serde::Deserialize;

#[derive(Debug, Default, Deserialize, Clone)]
pub struct VadConfig {
    model: Option<String>,
    num_threads: Option<i32>,
}

impl VadConfig {
    pub fn new() -> Self {
        Self {
            model: Some(String::from("data/vad/silero_vad.onnx")),
            num_threads: Some(4),
        }
    }

    pub fn model(&self) -> &str {
        self.model.as_deref().unwrap_or_default()
    }

    pub fn num_threads(&self) -> i32 {
        self.num_threads.unwrap_or_default()
    }
}
