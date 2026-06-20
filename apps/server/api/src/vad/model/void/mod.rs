use async_trait::async_trait;

use crate::{common::ModelError, vad::Vad};

#[derive(Clone)]
pub struct VadVoid {}

impl VadVoid {
    pub fn new() -> core::result::Result<Self, ModelError> {
        Ok(Self {})
    }
}

#[async_trait]
impl Vad for VadVoid {
    async fn accept_waveform(&mut self, _samples: &[f32]) -> Result<f32, ModelError> {
        Ok(1.0)
    }

    async fn is_speech(&mut self) -> bool {
        true
    }

    async fn clear(&mut self) {}

    async fn window_size(&self) -> usize {
        512
    }
}
