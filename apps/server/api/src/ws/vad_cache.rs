use sherpa_rs::vad::VadConfig;
use std::sync::{Arc, OnceLock};
use tokio::sync::Mutex;

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
        let config = VadConfig {
            //wget https://huggingface.co/deepghs/silero-vad-onnx/resolve/main/silero_vad.onnx
            model: "silero_vad.onnx".into(),
            min_silence_duration: 0.1,
            min_speech_duration: 0.25,
            max_speech_duration: 8.0,
            threshold: 0.5,
            window_size: 512_i32,
            num_threads: Some(4),
            ..Default::default()
        };
        let vad_instance = sherpa_rs::vad::Vad::new(config, 8.0).unwrap();
        let vad = SherpaVad::new(Arc::new(Mutex::new(vad_instance)));
        VAD_INSTANCE.get_or_init(|| -> Self { Self::new(vad) })
    }

    pub fn global() -> &'static VadCache {
        VAD_INSTANCE.get().unwrap()
    }
}
