use async_trait::async_trait;

use crate::{
    asr::{Asr, RecognizerResult},
    common::ModelError,
};

pub struct AsrVoid {}

impl AsrVoid {
    pub fn new() -> Result<Self, ModelError> {
        Ok(Self {})
    }
}

#[async_trait]
impl Asr for AsrVoid {
    async fn transcribe(
        &mut self,
        _sample_rate: u32,
        _samples: &[f32],
    ) -> Result<RecognizerResult, ModelError> {
        Ok(RecognizerResult {
            text: String::new(),
            prob: 1.0,
        })
    }
}
