use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RoundMode {
    Auto,
    Manual,
    RealTime,
    Text,
}

impl RoundMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            RoundMode::Auto => "auto",
            RoundMode::Manual => "manual",
            RoundMode::RealTime => "realtime",
            RoundMode::Text => "text",
        }
    }
}

pub enum FrameDirection {
    Inbound,
    Outbound,
}

pub struct RoundStartContext {
    pub round_id: String,
    pub session_id: Option<String>,
    pub client_info: Option<JsonValue>,
    pub mode: RoundMode,
}

pub struct AsrContext {
    pub round_id: String,
    pub voice_pcm: Vec<f32>,
    pub sample_rate: u32,
    pub text: String,
    pub confidence: f32,
}

pub struct TextInputContext {
    pub round_id: String,
    pub text: String,
}

pub struct LlmDeltaContext {
    pub round_id: String,
    pub text: String,
}

pub struct TtsDeltaContext {
    pub round_id: String,
    pub text: String,
    pub raw_pcm: Option<(Vec<f32>, u32)>,
}

pub struct FrameContext {
    pub round_id: Option<String>,
    pub session_id: Option<String>,
    pub seq: u64,
    pub direction: FrameDirection,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RoundEndReason {
    Completed,
    Interrupted,
}

pub struct RoundEndContext {
    pub round_id: String,
    pub reason: RoundEndReason,
}
