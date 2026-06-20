pub mod model;
use crate::config::VadModel;
use crate::config::vad::VadConfig;
use crate::vad::model::earshot::VadEarshot;
use crate::{common::ModelError, vad::model::void::VadVoid};
use async_trait::async_trait;
use std::sync::{Arc, OnceLock};

#[async_trait]
pub trait Vad: Send + Sync {
    async fn accept_waveform(&mut self, samples: Vec<f32>) -> Result<(), ModelError>;
    async fn front(&mut self) -> SpeechSegment;
    async fn is_empty(&mut self) -> bool;
    async fn is_speech(&mut self) -> bool;
    async fn pop(&mut self);
    async fn clear(&mut self);
    async fn window_size(&self) -> usize;
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

    pub fn config() -> Arc<VadConfig> {
        VAD_INSTANCE.get().unwrap().config.clone()
    }

    pub fn create_model(config: &VadConfig) -> Box<dyn Vad> {
        match config.model.as_ref().expect("vad model empty") {
            VadModel::Void => Box::new(VadVoid::new().unwrap()),
            VadModel::Earshot => Box::new(VadEarshot::new().unwrap()),
        }
    }
}
