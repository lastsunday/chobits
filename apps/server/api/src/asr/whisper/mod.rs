pub mod decoder;
pub mod multilingual;

use async_trait::async_trait;
use candle_core::{Device, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::whisper::{self as m, Config, audio};
use tokenizers::Tokenizer;

use self::decoder::Decoder;
use crate::{
    asr::{Asr, RecognizerResult, whisper::multilingual::detect_language},
    common::{ModelError, device},
};

#[derive(Debug, Clone)]
pub enum Model {
    Normal(m::model::Whisper),
    Quantized(m::quantized_model::Whisper),
}

// Maybe we should use some traits rather than doing the dispatch for all these.
impl Model {
    pub fn config(&self) -> &Config {
        match self {
            Self::Normal(m) => &m.config,
            Self::Quantized(m) => &m.config,
        }
    }

    pub fn encoder_forward(&mut self, x: &Tensor, flush: bool) -> candle_core::Result<Tensor> {
        match self {
            Self::Normal(m) => m.encoder.forward(x, flush),
            Self::Quantized(m) => m.encoder.forward(x, flush),
        }
    }

    pub fn decoder_forward(
        &mut self,
        x: &Tensor,
        xa: &Tensor,
        flush: bool,
    ) -> candle_core::Result<Tensor> {
        match self {
            Self::Normal(m) => m.decoder.forward(x, xa, flush),
            Self::Quantized(m) => m.decoder.forward(x, xa, flush),
        }
    }

    pub fn decoder_final_linear(&self, x: &Tensor) -> candle_core::Result<Tensor> {
        match self {
            Self::Normal(m) => m.decoder.final_linear(x),
            Self::Quantized(m) => m.decoder.final_linear(x),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Task {
    Transcribe,
    Translate,
}

pub fn token_id(tokenizer: &Tokenizer, token: &str) -> candle_core::Result<u32> {
    match tokenizer.token_to_id(token) {
        None => candle_core::bail!("no token-id for {token}"),
        Some(id) => Ok(id),
    }
}

#[derive(Clone)]
pub struct AsrWhisper {
    device: Device,
    model: Model,
    tokenizer: Tokenizer,
    config: Config,
    mel_filters: Vec<f32>,
}

impl AsrWhisper {
    pub fn new(
        model_path: String,
        config_path: String,
        tokenizer_path: String,
    ) -> Result<Self, ModelError> {
        let start = std::time::Instant::now();
        let device = device(false)?;
        let config: Config =
            serde_json::from_str(&std::fs::read_to_string(config_path.clone()).map_err(|_e| {
                ModelError::ModelFileNotFound(format!(
                    "config file not found path = {}",
                    config_path.clone()
                ))
            })?)
            .map_err(|_e| {
                ModelError::ModelInitFailure(format!(
                    "file convert json failure path = {}",
                    config_path
                ))
            })?;
        let tokenizer = Tokenizer::from_file(tokenizer_path.clone()).map_err(|_e| {
            ModelError::TokenFileNotFound(format!(
                "token file not found,path = {} ",
                tokenizer_path.clone()
            ))
        })?;
        let mel_bytes = include_bytes!("melfilters128.bytes").as_slice();
        let mut mel_filters = vec![0f32; mel_bytes.len() / 4];
        <byteorder::LittleEndian as byteorder::ByteOrder>::read_f32_into(
            mel_bytes,
            &mut mel_filters,
        );
        let vb = unsafe { VarBuilder::from_mmaped_safetensors(&[model_path], m::DTYPE, &device)? };
        let model = Model::Normal(m::model::Whisper::load(&vb, config.clone())?);
        tracing::info!("loaded the model in {:?}", start.elapsed());
        tracing::info!("model built");
        Ok(Self {
            device: device.clone(),
            model,
            tokenizer,
            config,
            mel_filters,
        })
    }
}

#[async_trait]
impl Asr for AsrWhisper {
    async fn transcribe(
        &mut self,
        _sample_rate: u32,
        samples: &[f32],
    ) -> Result<super::RecognizerResult, ModelError> {
        let device = &self.device;
        let mel_filters = &self.mel_filters;
        let config = &self.config;
        let model = &mut self.model;
        let tokenizer = &self.tokenizer;

        // TODO: setting
        let seed: u64 = 299792458;
        let task = Some(Task::Transcribe);
        let timestamps = false;
        let verbose = false;

        let mel = audio::pcm_to_mel(config, samples, mel_filters);
        let mel_len = mel.len();
        let mel = Tensor::from_vec(
            mel,
            (1, config.num_mel_bins, mel_len / config.num_mel_bins),
            device,
        )?;
        tracing::info!("loaded mel: {:?}", mel.dims());

        let language_token = Some(detect_language(model, tokenizer, &mel)?);

        match Decoder::new(
            model,
            tokenizer,
            seed,
            device,
            Some(language_token.clone().unwrap().0),
            task,
            timestamps,
            verbose,
        ) {
            Ok(mut dc) => {
                let result = dc.run(&mel);
                tracing::info!("result = {:?}", result);
                match result {
                    Ok(result) => {
                        let text = result
                            .into_iter()
                            .map(|item| item.dr.text)
                            .collect::<String>();
                        Ok(RecognizerResult {
                            text,
                            language: language_token.clone().unwrap().1.clone(),
                            prob: language_token.clone().unwrap().2,
                        })
                    }
                    Err(e) => Err(ModelError::Decoder(format!("decoder run error = {}", e))),
                }
            }
            Err(e) => Err(ModelError::Decoder(format!("decoder new error = {}", e))),
        }
    }
}
