pub mod asr_cache;

use sherpa_rs::sense_voice::{SenseVoiceRecognizer, SenseVoiceRecognizerResult};
use std::sync::Arc;
use tokio::sync::Mutex;

pub trait Asr: Send + Sync {
    fn transcribe(
        &mut self,
        sample_rate: u32,
        samples: &[f32],
    ) -> impl Future<Output = RecognizerResult> + Send;
}

#[derive(Debug, Clone)]
pub struct RecognizerResult {
    pub lang: String,
    pub text: String,
    pub timestamps: Vec<f32>,
    pub tokens: Vec<String>,
}

#[derive(Clone)]
pub struct SenseVoiceAsr {
    instance: Arc<Mutex<SenseVoiceRecognizer>>,
}

impl SenseVoiceAsr {
    pub fn new(instance: Arc<Mutex<SenseVoiceRecognizer>>) -> Self {
        Self { instance }
    }
}

impl Asr for SenseVoiceAsr {
    async fn transcribe(&mut self, sample_rate: u32, samples: &[f32]) -> RecognizerResult {
        let mut instance = self.instance.lock().await;
        let SenseVoiceRecognizerResult {
            lang,
            text,
            timestamps,
            tokens,
        } = instance.transcribe(sample_rate, samples);
        RecognizerResult {
            lang,
            text,
            timestamps,
            tokens,
        }
    }
}
