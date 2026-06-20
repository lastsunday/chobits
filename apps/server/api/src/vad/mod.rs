pub mod model;
use crate::config::VadModel;
use crate::config::vad::VadConfig;
use crate::vad::model::earshot::VadEarshot;
use crate::{common::ModelError, vad::model::void::VadVoid};
use async_trait::async_trait;
use std::sync::{Arc, OnceLock};

#[async_trait]
pub trait Vad: Send + Sync {
    /// Feed audio frame (window_size samples). Returns speech probability [0, 1].
    async fn accept_waveform(&mut self, samples: &[f32]) -> Result<f32, ModelError>;
    /// Whether the state machine currently considers speech active.
    async fn is_speech(&mut self) -> bool;
    /// Reset all internal state.
    async fn clear(&mut self);
    /// Number of samples expected per frame.
    async fn window_size(&self) -> usize;
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
            VadModel::Earshot => Box::new(VadEarshot::new(config).unwrap()),
        }
    }
}
