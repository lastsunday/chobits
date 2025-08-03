#[cfg(feature = "mkl")]
extern crate intel_mkl_src;

#[cfg(feature = "accelerate")]
extern crate accelerate_src;

pub mod llm_cache;
pub mod models;
pub mod token_output_stream;

use candle_core::{Device, Result, Tensor, quantized::gguf_file};
use futures::Stream;
use tokenizers::{Tokenizer, tokenizer};

use candle_transformers::generation::{LogitsProcessor, Sampling};

use candle_core::utils::{cuda_is_available, metal_is_available};
use models::quantized_qwen3::ModelWeights as Qwen3;
use token_output_stream::TokenOutputStream;

use tokio::sync::{Mutex, mpsc::channel};
use tokio_stream::wrappers::ReceiverStream;

pub trait Llm {
    fn chat(&self, text: String) -> impl Stream<Item = String> + Unpin + Send;
}

#[derive(Clone)]
pub struct LlmQwen {
    model: Qwen3,
    tokenizer: Tokenizer,
    device: Device,
}

#[derive(Debug, thiserror::Error)]
pub enum LlmError {
    #[error("model file not found path = {0}")]
    ModelFileNotFound(String),
    #[error("token file not found path = {0}")]
    TokenFileNotFound(String),
    #[error("model init failure path = {0}")]
    ModelInitFailure(String),
    #[error("token init failure path = {0}")]
    TokenInitFailure(String),
}

impl LlmQwen {
    pub fn new(model_path: String, token_path: String) -> core::result::Result<Self, LlmError> {
        let mut file = std::fs::File::open(model_path.clone())
            .map_err(|_e| LlmError::ModelFileNotFound(model_path.clone()))?;
        let start = std::time::Instant::now();
        let device = device(false).unwrap();

        let model = {
            let model = gguf_file::Content::read(&mut file)
                .map_err(|_e| LlmError::ModelInitFailure(model_path.clone()))?;
            let mut total_size_in_bytes = 0;
            for (_, tensor) in model.tensor_infos.iter() {
                let elem_count = tensor.shape.elem_count();
                total_size_in_bytes +=
                    elem_count * tensor.ggml_dtype.type_size() / tensor.ggml_dtype.block_size();
            }
            tracing::info!(
                "loaded {:?} tensors ({}) in {:.2}s",
                model.tensor_infos.len(),
                &format_size(total_size_in_bytes),
                start.elapsed().as_secs_f32(),
            );
            Qwen3::from_gguf(model, &mut file, &device)
                .map_err(|_e| LlmError::ModelInitFailure(model_path.clone()))?
        };
        tracing::info!("model built");
        let tokenizer = Tokenizer::from_file(token_path.clone())
            .map_err(|_e| LlmError::TokenInitFailure(token_path.clone()))?;
        Ok(Self {
            model,
            tokenizer,
            device: device.clone(),
        })
    }
}

impl Llm for LlmQwen {
    fn chat(&self, text: String) -> impl Stream<Item = String> + Unpin + Send {
        let tokenizer = self.tokenizer.clone();
        let mut model = self.model.clone();
        let device = self.device.clone();
        let (tx, rx) = channel(1);
        tokio::spawn(async move {
            let mut tos = TokenOutputStream::new(tokenizer);
            let prompt_str =
                format!("<|im_start|>user\n{text} /no_think<|im_end|>\n<|im_start|>assistant\n");
            tracing::info!("formatted prompt: {}", &prompt_str);

            let tokens = tos
                .tokenizer()
                .encode(prompt_str, true)
                .map_err(anyhow::Error::msg)
                .unwrap();

            let tokens = tokens.get_ids();

            // TODO:setting
            let to_sample = 999;

            let mut all_tokens = vec![];

            let mut logits_processor = {
                // TODO: setting
                let sampling = Sampling::All { temperature: 0.8 };
                LogitsProcessor::from_sampling(299792458, sampling)
            };

            let start_prompt_processing = std::time::Instant::now();

            let input = Tensor::new(tokens, &device).unwrap().unsqueeze(0).unwrap();
            let logits = model.forward(&input, 0).unwrap();
            let logits = logits.squeeze(0).unwrap();
            let mut next_token = logits_processor.sample(&logits).unwrap();

            let prompt_dt = start_prompt_processing.elapsed();

            all_tokens.push(next_token);

            if let Some(t) = tos.next_token(next_token).unwrap() {
                match tx.send(t).await {
                    Ok(_) => (),
                    Err(error) => {
                        tracing::info!("output text error = {}", error);
                    }
                }
            }

            let eos_token = *tos.tokenizer().get_vocab(true).get("<|im_end|>").unwrap();

            let start_post_prompt = std::time::Instant::now();

            let mut sampled = 0;
            for index in 0..to_sample {
                let input = Tensor::new(&[next_token], &device)
                    .unwrap()
                    .unsqueeze(0)
                    .unwrap();
                let logits = model.forward(&input, tokens.len() + index).unwrap();
                let logits = logits.squeeze(0).unwrap();
                // TODO: setting
                let logits = {
                    // TODO: setting
                    let start_at = all_tokens.len().saturating_sub(64);
                    // TODO: setting
                    candle_transformers::utils::apply_repeat_penalty(
                        &logits,
                        1.1,
                        &all_tokens[start_at..],
                    )
                    .unwrap()
                };
                next_token = logits_processor.sample(&logits).unwrap();
                all_tokens.push(next_token);
                if let Some(t) = tos.next_token(next_token).unwrap() {
                    match tx.send(t).await {
                        Ok(_) => (),
                        Err(error) => {
                            tracing::info!("output text error = {}", error);
                            break;
                        }
                    }
                }
                sampled += 1;
                if next_token == eos_token {
                    break;
                };
            }

            if let Some(rest) = tos.decode_rest().map_err(candle_core::Error::msg).unwrap() {
                match tx.send(rest).await {
                    Ok(_) => (),
                    Err(error) => {
                        tracing::info!("output text error = {}", error);
                    }
                }
            }

            let dt = start_post_prompt.elapsed();
            tracing::info!(
                "\n\n{:4} prompt tokens processed: {:.2} token/s",
                tokens.len(),
                tokens.len() as f64 / prompt_dt.as_secs_f64(),
            );
            tracing::info!(
                "{sampled:4} tokens generated: {:.2} token/s",
                sampled as f64 / dt.as_secs_f64(),
            );
            drop(tx);
        });
        ReceiverStream::new(rx)
    }
}

pub fn device(cpu: bool) -> Result<Device> {
    if cpu {
        Ok(Device::Cpu)
    } else if cuda_is_available() {
        Ok(Device::new_cuda(0)?)
    } else if metal_is_available() {
        Ok(Device::new_metal(0)?)
    } else {
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        {
            println!(
                "Running on CPU, to run on GPU(metal), build this example with `--features metal`"
            );
        }
        #[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
        {
            tracing::info!(
                "Running on CPU, to run on GPU, build this example with `--features cuda`"
            );
        }
        Ok(Device::Cpu)
    }
}

fn format_size(size_in_bytes: usize) -> String {
    if size_in_bytes < 1_000 {
        format!("{size_in_bytes}B")
    } else if size_in_bytes < 1_000_000 {
        format!("{:.2}KB", size_in_bytes as f64 / 1e3)
    } else if size_in_bytes < 1_000_000_000 {
        format!("{:.2}MB", size_in_bytes as f64 / 1e6)
    } else {
        format!("{:.2}GB", size_in_bytes as f64 / 1e9)
    }
}
