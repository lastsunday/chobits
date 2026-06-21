use async_trait::async_trait;
use serde_json::Value as JsonValue;

pub enum FrameDirection {
    Inbound,
    Outbound,
}

pub struct RoundStartContext {
    pub round_id: String,
    pub user_id: Option<String>,
    pub client_info: Option<JsonValue>,
}

pub struct AsrContext {
    pub round_id: String,
    pub voice_pcm: Vec<f32>,
    pub sample_rate: u32,
    pub text: String,
    pub confidence: f32,
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
    pub round_id: String,
    pub seq: u64,
    pub direction: FrameDirection,
    pub detail: String,
}

pub struct RoundEndContext {
    pub round_id: String,
}

#[async_trait]
pub trait SessionObserver: Send + Sync {
    fn on_round_start(&self, ctx: &RoundStartContext);

    fn on_asr(&self, ctx: &AsrContext);

    fn on_llm_delta(&self, ctx: &LlmDeltaContext);

    fn on_tts_delta(&self, ctx: &TtsDeltaContext);

    fn on_frame(&self, ctx: &FrameContext);

    async fn on_round_end(&self, ctx: &RoundEndContext) -> Result<(), anyhow::Error>;
}
