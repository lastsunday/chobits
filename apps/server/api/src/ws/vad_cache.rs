use core::f32;
use sherpa_rs::vad::VadConfig;
use std::sync::{Arc, OnceLock};
use tokio::sync::Mutex;

use crate::config;
use crate::ws::vad::SherpaVad;

static VAD_INSTANCE: OnceLock<VadCache> = OnceLock::new();

pub struct VadCache {
    pub instance: SherpaVad,
}

impl VadCache {
    pub fn new(instance: SherpaVad) -> Self {
        Self { instance }
    }

    pub async fn init() -> &'static Self {
        let vad = Self::create_vad();
        VAD_INSTANCE.get_or_init(|| -> Self { Self::new(vad) })
    }

    pub fn global() -> &'static VadCache {
        VAD_INSTANCE.get().unwrap()
    }

    pub fn create_vad() -> SherpaVad {
        let app_config = config::get();
        let vad_config = app_config.vad();
        let config = VadConfig {
            //wget https://huggingface.co/deepghs/silero-vad-onnx/resolve/main/silero_vad.onnx
            model: vad_config.model().into(),
            min_silence_duration: 1.0,
            min_speech_duration: 0.001,
            max_speech_duration: f32::INFINITY,
            threshold: 0.1,
            window_size: 512_i32,
            num_threads: Some(vad_config.num_threads()),
            ..Default::default()
        };
        let vad_instance = sherpa_rs::vad::Vad::new(config, 8.0).unwrap();
        SherpaVad::new(Arc::new(Mutex::new(vad_instance)))
    }
}
