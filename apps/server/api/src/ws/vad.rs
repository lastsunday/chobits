use std::sync::Arc;

use tokio::sync::Mutex;

pub trait Vad: Send + Sync {
    fn accept_waveform(&mut self, samples: Vec<f32>) -> impl Future<Output = ()> + Send;
    fn front(&mut self) -> impl Future<Output = SpeechSegment> + Send;
    fn is_empty(&mut self) -> impl Future<Output = bool> + Send;
    fn is_speech(&mut self) -> impl Future<Output = bool> + Send;
    fn pop(&mut self) -> impl Future<Output = ()> + Send;
}

#[derive(Debug)]
pub struct SpeechSegment {
    pub start: i32,
    pub samples: Vec<f32>,
}

#[derive(Clone)]
pub struct SherpaVad {
    instance: Arc<Mutex<sherpa_rs::vad::Vad>>,
}

impl SherpaVad {
    pub fn new(instance: Arc<Mutex<sherpa_rs::vad::Vad>>) -> Self {
        Self { instance }
    }
}

impl Vad for SherpaVad {
    async fn accept_waveform(&mut self, samples: Vec<f32>) {
        let mut instance = self.instance.lock().await;
        instance.accept_waveform(samples);
    }

    async fn front(&mut self) -> SpeechSegment {
        let mut instance = self.instance.lock().await;
        let result = instance.front();
        SpeechSegment {
            start: result.start,
            samples: result.samples,
        }
    }

    async fn is_empty(&mut self) -> bool {
        let mut instance = self.instance.lock().await;
        instance.is_empty()
    }

    async fn is_speech(&mut self) -> bool {
        let mut instance = self.instance.lock().await;
        instance.is_speech()
    }

    async fn pop(&mut self) {
        let mut instance = self.instance.lock().await;
        instance.pop()
    }
}
