pub mod asr_cache;
pub mod whisper;

use candle_core::{Device, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::whisper::{self as m, Config, audio};
use tokenizers::Tokenizer;

use crate::ws::{
    asr::whisper::{Model, Task, decoder::Decoder, multilingual::detect_language},
    common::{ModelError, device},
};

pub trait Asr: Send + Sync {
    fn transcribe(
        &mut self,
        sample_rate: u32,
        samples: &[f32],
    ) -> impl Future<Output = Result<RecognizerResult, ModelError>> + Send;
}

#[derive(Debug, Clone)]
pub struct RecognizerResult {
    pub text: String,
    pub language: String,
    pub prob: f32,
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

impl Asr for AsrWhisper {
    async fn transcribe(
        &mut self,
        _sample_rate: u32,
        samples: &[f32],
    ) -> Result<RecognizerResult, ModelError> {
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
