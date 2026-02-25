use std::sync::Arc;

use async_trait::async_trait;
use qwen_asr::{context::QwenCtx, transcribe};
use tokio::sync::Mutex;

use crate::{
    asr::{Asr, RecognizerResult},
    common::ModelError,
};

pub struct AsrQwen3 {
    ctx: Arc<Mutex<QwenCtx>>,
}

impl AsrQwen3 {
    pub fn new(path: String) -> Result<Self, ModelError> {
        let ctx = QwenCtx::load(path.as_str());
        match ctx {
            Some(ctx) => Ok(Self {
                ctx: Arc::new(Mutex::new(ctx)),
            }),
            None => Err(ModelError::ModelFileNotFound(format!("path = {}", path))),
        }
    }
}

#[async_trait]
impl Asr for AsrQwen3 {
    async fn transcribe(
        &mut self,
        _sample_rate: u32,
        samples: &[f32],
    ) -> Result<RecognizerResult, ModelError> {
        let ctx = self.ctx.clone();
        let ctx = &mut ctx.lock().await;
        let text = transcribe::transcribe_audio(ctx, samples);
        match text {
            Some(text) => Ok(RecognizerResult { text, prob: 1.0 }),
            None => Err(ModelError::Asr(String::from("asr transcribe failure"))),
        }
    }
}
