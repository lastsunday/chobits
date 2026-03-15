use async_trait::async_trait;

use crate::{
    common::ModelError,
    vad::{SpeechSegment, Vad},
};

#[derive(Clone)]
pub struct VadVoid {}

impl VadVoid {
    pub fn new() -> core::result::Result<Self, ModelError> {
        Ok(Self {})
    }
}

#[async_trait]
impl Vad for VadVoid {
    async fn accept_waveform(&mut self, _samples: Vec<f32>) -> Result<(), ModelError> {
        Ok(())
    }

    async fn front(&mut self) -> SpeechSegment {
        SpeechSegment {
            start: 0,
            samples: vec![],
        }
    }

    async fn is_empty(&mut self) -> bool {
        true
    }

    async fn is_speech(&mut self) -> bool {
        true
    }

    async fn pop(&mut self) {}

    async fn clear(&mut self) {}

    async fn window_size(&self) -> usize {
        512
    }
}
