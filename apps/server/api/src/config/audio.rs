use serde::Deserialize;

#[derive(Debug, Default, Deserialize, Clone)]
pub struct AudioConfig {
    input_sample_rate: Option<u32>,
    /// unit: ms
    input_frame_duration: Option<u64>,
    input_channel: Option<u32>,
    output_sample_rate: Option<u32>,
    output_channel: Option<u32>,
    /// unit: ms
    output_frame_duration: Option<u64>,
}

impl AudioConfig {
    pub fn new() -> Self {
        Self {
            input_sample_rate: Some(16000),
            input_channel: Some(1),
            input_frame_duration: Some(60_u64),
            output_sample_rate: Some(16000),
            output_channel: Some(1),
            output_frame_duration: Some(60_u64),
        }
    }

    pub fn input_sample_rate(&self) -> u32 {
        self.input_sample_rate.unwrap_or_default()
    }

    pub fn input_channel(&self) -> u32 {
        self.input_channel.unwrap_or_default()
    }

    pub fn input_frame_duration(&self) -> u64 {
        self.input_frame_duration.unwrap_or_default()
    }

    pub fn output_sample_rate(&self) -> u32 {
        self.output_sample_rate.unwrap_or_default()
    }

    pub fn output_channel(&self) -> u32 {
        self.output_channel.unwrap_or_default()
    }

    pub fn output_frame_duration(&self) -> u64 {
        self.output_frame_duration.unwrap_or_default()
    }
}
