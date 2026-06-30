use crate::{common::ModelError, vad::Vad};

#[derive(Clone)]
pub struct VadVoid {}

impl VadVoid {
    pub fn new() -> core::result::Result<Self, ModelError> {
        Ok(Self {})
    }
}

impl Vad for VadVoid {
    fn accept_waveform(&mut self, _samples: &[f32]) -> Result<f32, ModelError> {
        Ok(1.0)
    }

    fn is_speech(&mut self) -> bool {
        true
    }

    fn clear(&mut self) {}

    fn window_size(&self) -> usize {
        512
    }
}
