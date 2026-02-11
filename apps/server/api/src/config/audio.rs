use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct AudioConfig {
    #[serde(default)]
    pub input_sample_rate: Option<u32>,
    /// unit: ms
    #[serde(default)]
    pub input_frame_duration: Option<u64>,
    #[serde(default)]
    pub input_channel: Option<u32>,
    #[serde(default)]
    pub output_sample_rate: Option<u32>,
    #[serde(default)]
    pub output_channel: Option<u32>,
    /// unit: ms
    #[serde(default)]
    pub output_frame_duration: Option<u64>,
}
