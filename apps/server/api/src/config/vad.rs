use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct VadConfig {
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub num_threads: Option<i32>,
}

impl Default for VadConfig {
    fn default() -> Self {
        Self {
            path: Some(String::from("data/vad/model/onnx-community/silero-vad/")),
            num_threads: Some(4),
        }
    }
}
