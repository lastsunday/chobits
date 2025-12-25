use std::sync::OnceLock;

use crate::{config, vad::VadSilero};

static VAD_INSTANCE: OnceLock<VadCache> = OnceLock::new();

pub struct VadCache {
    pub instance: VadSilero,
}

impl VadCache {
    pub fn new(instance: VadSilero) -> Self {
        Self { instance }
    }

    pub async fn init() -> &'static Self {
        let vad = Self::create_vad();
        VAD_INSTANCE.get_or_init(|| -> Self { Self::new(vad) })
    }

    pub fn global() -> &'static VadCache {
        VAD_INSTANCE.get().unwrap()
    }

    pub fn create_vad() -> VadSilero {
        let app_config = config::get();
        let vad_config = app_config.vad();
        VadSilero::new(String::from(vad_config.model())).unwrap()
    }
}
