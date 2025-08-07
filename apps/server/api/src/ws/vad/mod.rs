#[cfg(feature = "mkl")]
extern crate intel_mkl_src;

#[cfg(feature = "accelerate")]
extern crate accelerate_src;

pub mod vad_cache;

use candle_core::{DType, Device, Tensor};
use candle_onnx::onnx::ModelProto;

use crate::ws::common::{ModelError, device};

pub trait Vad: Send + Sync {
    fn accept_waveform(
        &mut self,
        samples: Vec<f32>,
    ) -> impl Future<Output = Result<(), ModelError>> + Send;
    fn front(&mut self) -> impl Future<Output = SpeechSegment> + Send;
    fn is_empty(&mut self) -> impl Future<Output = bool> + Send;
    fn is_speech(&mut self) -> impl Future<Output = bool> + Send;
    fn pop(&mut self) -> impl Future<Output = ()> + Send;
    fn clear(&mut self) -> impl Future<Output = ()> + Send;
}

#[derive(Debug)]
pub struct SpeechSegment {
    pub start: i32,
    pub samples: Vec<f32>,
}

#[derive(Clone)]
pub struct VadSilero {
    model: ModelProto,
    device: Device,
    // TODO: need refacotr to Vec<Vec<f32>> to adapter front and pop operation
    samples: Vec<f32>,
    is_speech: bool,
    start: i32,
    total_accept_waveform_samples_len: i32,
    /// unit ms
    min_silence_duration: f32,
    /// unit ms
    current_silence_duration: f32,
}

impl VadSilero {
    pub fn new(model_path: String) -> core::result::Result<Self, ModelError> {
        let start = std::time::Instant::now();
        let device = device(true).unwrap();
        let model = candle_onnx::read_file(model_path.clone())?;
        tracing::info!("loaded the model in {:?}", start.elapsed());
        tracing::info!("model built");
        Ok(Self {
            model,
            device: device.clone(),
            samples: Vec::new(),
            is_speech: false,
            start: -1,
            total_accept_waveform_samples_len: 0,
            min_silence_duration: 1000.0,
            current_silence_duration: 0.0,
        })
    }
}

struct State {
    frame_size: usize,
    sample_rate: Tensor,
    state: Tensor,
    context: Tensor,
}

impl Vad for VadSilero {
    async fn accept_waveform(&mut self, samples: Vec<f32>) -> Result<(), ModelError> {
        //TODO: setting
        let sample_rate: i64 = 16000;
        let frame_size: usize = 512;
        let context_size: usize = 64;
        let threshold = 0.5;

        let device = &self.device;
        let model = &self.model;
        let mut state = State {
            frame_size,
            sample_rate: Tensor::new(sample_rate, device)
                .map_err(|e| ModelError::Tensor(format!("tensor sample rate {}", e.to_string())))?,
            state: Tensor::zeros((2, 1, 128), DType::F32, device)
                .map_err(|e| ModelError::Tensor(format!("tensor state {}", e.to_string())))?,
            context: Tensor::zeros((1, context_size), DType::F32, device)
                .map_err(|e| ModelError::Tensor(format!("context {}", e.to_string())))?,
        };
        let mut res = vec![];
        if samples.len() < state.frame_size {
            return Ok(());
        }
        let next_context = Tensor::from_slice(
            &samples[state.frame_size - context_size..],
            (1, context_size),
            device,
        )?;
        let chunk = Tensor::from_vec(samples.clone(), (1, state.frame_size), device)
            .map_err(|e| ModelError::Tensor(format!("from vec error {}", e.to_string())))?;
        let chunk = Tensor::cat(&[&state.context, &chunk], 1)
            .map_err(|e| ModelError::Tensor(format!("cat error {}", e.to_string())))?;
        let inputs = std::collections::HashMap::from_iter([
            ("input".to_string(), chunk),
            ("sr".to_string(), state.sample_rate.clone()),
            ("state".to_string(), state.state.clone()),
        ]);
        let out = candle_onnx::simple_eval(model, inputs)
            .map_err(|e| ModelError::Tensor(format!("simple eval {}", e.to_string())))?;
        let out_names = &model.graph.as_ref().unwrap().output;
        let output = out.get(&out_names[0].name).unwrap().clone();
        state.state = out.get(&out_names[1].name).unwrap().clone();
        assert_eq!(state.state.dims(), &[2, 1, 128]);
        state.context = next_context;
        let output = output.flatten_all()?.to_vec1::<f32>()?;
        assert_eq!(output.len(), 1);
        let output = output[0];
        res.push(output);
        let res_len = res.len() as f32;
        let prediction = res.iter().sum::<f32>() / res_len;
        if prediction >= threshold {
            self.current_silence_duration = 0.0;
            self.samples.append(&mut samples.clone());
            self.is_speech = true;
            if self.start < 0 {
                self.start = self.total_accept_waveform_samples_len;
            }
        } else {
            if self.is_speech && self.current_silence_duration <= self.min_silence_duration {
                self.samples.append(&mut samples.clone());
            } else {
                self.is_speech = false;
                self.start = -1;
                self.samples.clear();
            }
            self.current_silence_duration +=
                (samples.len() as f32 / sample_rate as f32) as f32 * 1000.0;
        }
        self.total_accept_waveform_samples_len += samples.len() as i32;
        Ok(())
    }

    async fn front(&mut self) -> SpeechSegment {
        //TODO: get one samples list
        let samples = self.samples.to_vec();
        let start = self.start;
        SpeechSegment {
            start: start,
            samples: samples,
        }
    }

    async fn is_empty(&mut self) -> bool {
        self.samples.is_empty()
    }

    async fn is_speech(&mut self) -> bool {
        self.is_speech
    }

    async fn pop(&mut self) {
        //TODO: pop one samples
        self.samples.clear();
    }

    async fn clear(&mut self) {
        self.samples.clear();
        self.start = -1;
        self.is_speech = false;
        self.current_silence_duration = 0.0;
        self.total_accept_waveform_samples_len = 0;
    }
}
