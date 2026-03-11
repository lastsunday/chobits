#[cfg(feature = "mkl")]
extern crate intel_mkl_src;

#[cfg(feature = "accelerate")]
extern crate accelerate_src;

pub mod model;
use crate::config::vad::VadConfig;
use crate::{common::ModelError, vad::model::void::VadVoid};
use async_trait::async_trait;
use model::silero::VadSilero;
use std::sync::{Arc, OnceLock};

#[async_trait]
pub trait Vad: Send + Sync {
    async fn accept_waveform(&mut self, samples: Vec<f32>) -> Result<(), ModelError>;
    async fn front(&mut self) -> SpeechSegment;
    async fn is_empty(&mut self) -> bool;
    async fn is_speech(&mut self) -> bool;
    async fn pop(&mut self);
    async fn clear(&mut self);
}

#[derive(Debug)]
pub struct SpeechSegment {
    pub start: i32,
    pub samples: Vec<f32>,
}

static VAD_INSTANCE: OnceLock<VadFactory> = OnceLock::new();

#[derive(Default)]
pub struct VadFactory {
    pub config: Arc<VadConfig>,
}

impl VadFactory {
    pub fn new(config: Arc<VadConfig>) -> Self {
        Self { config }
    }

    pub async fn init(config: Arc<VadConfig>) -> &'static Self {
        VAD_INSTANCE.get_or_init(|| -> Self { Self::new(config) })
    }

    pub fn global() -> &'static VadFactory {
        VAD_INSTANCE.get().unwrap()
    }

    pub fn create_model(config: &VadConfig) -> Box<dyn Vad> {
        match config.model.as_ref().expect("vad model empty") {
            crate::config::VadModel::Silero => {
                Box::new(VadSilero::new(config.path.clone().expect("vad path is empty")).unwrap())
            }
            crate::config::VadModel::Void => Box::new(VadVoid::new().unwrap()),
        }
    }
}
