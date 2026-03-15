use async_trait::async_trait;
use earshot::Detector;

use crate::{
    common::ModelError,
    vad::{SpeechSegment, Vad},
};

pub struct VadEarshot {
    detector: Detector,
    samples: Vec<f32>,
    is_speech: bool,
    start: i32,
    total_accept_waveform_samples_len: i32,
    /// unit ms
    min_silence_duration: f32,
    /// unit ms
    current_silence_duration: f32,
    prediction_list: Vec<f32>,
}

impl VadEarshot {
    pub fn new() -> core::result::Result<Self, ModelError> {
        let detector = Detector::default();
        Ok(Self {
            detector,
            samples: Vec::new(),
            is_speech: false,
            start: -1,
            total_accept_waveform_samples_len: 0,
            min_silence_duration: 1000.0,
            current_silence_duration: 0.0,
            prediction_list: Vec::new(),
        })
    }
}

#[async_trait]
impl Vad for VadEarshot {
    async fn accept_waveform(&mut self, samples: Vec<f32>) -> Result<(), ModelError> {
        let threshold = 0.5;
        let sample_rate: i64 = 16000;
        let score = self.detector.predict_f32(&samples);
        // debug!("Score: {}", score);
        // Score is between 0-1; 0 = no voice, 1 = voice.
        if !self.is_speech {
            if score > threshold {
                self.prediction_list.push(score);
            } else {
                self.clear().await;
            }

            if !self.prediction_list.is_empty() {
                self.samples.append(&mut samples.clone());
            }

            // avoid some noise trigger speech detect
            if self.prediction_list.len() >= 5 && !self.is_speech {
                self.is_speech = true;
                if self.start < 0 {
                    self.start = self.total_accept_waveform_samples_len;
                }
            }
        } else if score >= threshold {
            self.samples.append(&mut samples.clone());
        } else {
            if self.is_speech && self.current_silence_duration <= self.min_silence_duration {
                self.samples.append(&mut samples.clone());
            } else {
                self.clear().await;
            }
            self.current_silence_duration += (samples.len() as f32 / sample_rate as f32) * 1000.0;
        }
        // info!("vad len = {}", self.prediction_list.len());
        self.total_accept_waveform_samples_len += samples.len() as i32;
        Ok(())
    }

    async fn front(&mut self) -> SpeechSegment {
        let samples = self.samples.to_vec();
        let start = self.start;
        SpeechSegment { start, samples }
    }

    async fn is_empty(&mut self) -> bool {
        self.samples.is_empty()
    }

    async fn is_speech(&mut self) -> bool {
        self.is_speech
    }

    async fn pop(&mut self) {
        self.samples.clear();
    }

    async fn clear(&mut self) {
        self.detector.reset();
        self.samples.clear();
        self.start = -1;
        self.is_speech = false;
        self.current_silence_duration = 0.0;
        self.total_accept_waveform_samples_len = 0;
        self.prediction_list.clear();
    }

    async fn window_size(&self) -> usize {
        256
    }
}
