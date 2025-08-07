#[cfg(feature = "mkl")]
extern crate intel_mkl_src;

#[cfg(feature = "accelerate")]
extern crate accelerate_src;

pub mod llm_cache;
pub mod models;
pub mod token_output_stream;
use candle_core::{Device, Tensor, quantized::gguf_file};
use candle_transformers::generation::{LogitsProcessor, Sampling};
use futures::{Stream, executor::block_on};
use models::quantized_qwen3::ModelWeights as Qwen3;
use regex::Regex;
use std::thread;
use token_output_stream::TokenOutputStream;
use tokenizers::Tokenizer;
use tokio::sync::mpsc::channel;
use tokio_stream::wrappers::ReceiverStream;

use crate::ws::{
    common::{ModelError, device, format_size},
    util::llm::{filter, filter_think},
};

pub trait Llm {
    fn chat(
        &self,
        system_prompt: String,
        text: String,
    ) -> impl Stream<Item = core::result::Result<String, ModelError>> + Unpin + Send + 'static;
}

#[derive(Clone)]
pub struct LlmQwen {
    model: Qwen3,
    tokenizer: Tokenizer,
    device: Device,
}

impl LlmQwen {
    pub fn new(model_path: String, token_path: String) -> core::result::Result<Self, ModelError> {
        let mut file = std::fs::File::open(model_path.clone())
            .map_err(|_e| ModelError::ModelFileNotFound(model_path.clone()))?;
        let start = std::time::Instant::now();
        let device = device(false).unwrap();
        let model = {
            let model = gguf_file::Content::read(&mut file)
                .map_err(|_e| ModelError::ModelInitFailure(model_path.clone()))?;
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
                .map_err(|_e| ModelError::ModelInitFailure(model_path.clone()))?
        };
        tracing::info!("model built");
        let tokenizer = Tokenizer::from_file(token_path.clone())
            .map_err(|_e| ModelError::TokenInitFailure(token_path.clone()))?;
        Ok(Self {
            model,
            tokenizer,
            device: device.clone(),
        })
    }
}

async fn handle_chat(
    system_prompt: String,
    text: String,
    tokenizer: Tokenizer,
    mut model: Qwen3,
    device: Device,
    tx: tokio::sync::mpsc::Sender<core::result::Result<String, ModelError>>,
) -> core::result::Result<(), ModelError> {
    let mut tos = TokenOutputStream::new(tokenizer);
    let prompt_str = format!(
        "<|im_start|>system\n{system_prompt} /no_think<|im_end|>\n<|im_start|>user\n{text}<|im_end|>\n<|im_start|>assistant\n"
    );
    tracing::info!("formatted prompt: {}", &prompt_str);

    let tokens = tos
        .tokenizer()
        .encode(prompt_str, true)
        .map_err(|e| ModelError::Chat(format!("tokenizer encode error {}", e.to_string())))?;
    let tokens = tokens.get_ids();

    // TODO:setting
    let to_sample = 999;
    let temperature = 0.8;
    let seed = 299792458;
    let repeat_last_n = 64;
    let repeat_penalty = 1.1;

    let mut all_tokens = vec![];
    let mut text_result = Vec::new();
    let mut skip_think = false;

    let mut logits_processor = LogitsProcessor::from_sampling(seed, Sampling::All { temperature });

    let start_prompt_processing = std::time::Instant::now();

    let input = Tensor::new(tokens, &device)
        .map_err(|e| ModelError::Chat(format!("tensor create error {}", e.to_string())))?
        .unsqueeze(0)
        .map_err(|e| {
            ModelError::Chat(format!("tensor create unsqueeze error {}", e.to_string()))
        })?;
    let logits = model
        .forward(&input, 0)
        .map_err(|e| ModelError::Chat(format!("model forward error {}", e.to_string())))?;
    let logits = logits
        .squeeze(0)
        .map_err(|e| ModelError::Chat(format!("tensor squeeze error {}", e.to_string())))?;
    let mut next_token = logits_processor.sample(&logits).map_err(|e| {
        ModelError::Chat(format!("tensor processor sample error {}", e.to_string()))
    })?;

    let prompt_dt = start_prompt_processing.elapsed();

    all_tokens.push(next_token);
    if let Some(t) = tos
        .next_token(next_token)
        .map_err(|e| ModelError::Chat(format!("tensor encoding error {}", e.to_string())))?
    {
        text_result.push(t);
    }

    let eos_token = *tos
        .tokenizer()
        .get_vocab(true)
        .get("<|im_end|>")
        .ok_or_else(|| ModelError::Chat(format!("tensor can't get eos_token error ")))?;

    let start_post_prompt = std::time::Instant::now();

    let mut sampled = 0;
    let mut sentence_list: Vec<String> = Vec::new();
    let mut sentence: Vec<char> = Vec::new();
    for index in 0..to_sample {
        let input = Tensor::new(&[next_token], &device)
            .map_err(|e| ModelError::Chat(format!("tensor create error {}", e.to_string())))?
            .unsqueeze(0)
            .map_err(|e| {
                ModelError::Chat(format!("tensor create unsqueeze error {}", e.to_string()))
            })?;
        let logits = model
            .forward(&input, tokens.len() + index)
            .map_err(|e| ModelError::Chat(format!("model forward error {}", e.to_string())))?;
        let logits = logits
            .squeeze(0)
            .map_err(|e| ModelError::Chat(format!("tensor squeeze error {}", e.to_string())))?;
        let logits = {
            let start_at = all_tokens.len().saturating_sub(repeat_last_n);
            candle_transformers::utils::apply_repeat_penalty(
                &logits,
                repeat_penalty,
                &all_tokens[start_at..],
            )
            .map_err(|e| {
                ModelError::Chat(format!(
                    "tensor apply repeat penalty error {}",
                    e.to_string()
                ))
            })?
        };
        next_token = logits_processor.sample(&logits).map_err(|e| {
            ModelError::Chat(format!("tensor processor sample error {}", e.to_string()))
        })?;
        all_tokens.push(next_token);
        if let Some(t) = tos
            .next_token(next_token)
            .map_err(|e| ModelError::Chat(format!("tensor encoding error {}", e.to_string())))?
        {
            text_result.push(t);
            let text: String = text_result.clone().into_iter().collect();
            if !skip_think {
                if text.contains("</think>") {
                    if let Some(text) = filter_think(&text) {
                        text_result.clear();
                        for c in text.chars() {
                            sentence.push(c);
                        }
                    }
                    skip_think = true;
                }
            } else {
                for c in text.chars() {
                    sentence.push(c);
                    let regex = Regex::new(r"[。！？!?，、；,;]").unwrap();
                    // Break a sentence
                    if regex.is_match(&c.to_string()) {
                        let text: String = sentence.clone().into_iter().collect();
                        sentence.clear();
                        if let Some(text) = filter(&text) {
                            sentence_list.push(text);
                        }
                    }
                }
                text_result.clear();
            }
        }
        for text in sentence_list.clone() {
            if let Err(e) = tx.send(Ok(text.clone())).await {
                tracing::error!("chat send text error = {}", e);
                break;
            } else {
                tracing::info!("llm send text success, text = {}", text);
            }
        }
        sentence_list.clear();
        text_result.clear();
        sampled += 1;
        if next_token == eos_token {
            break;
        };
    }

    if let Some(rest) = tos
        .decode_rest()
        .map_err(|e| ModelError::Chat(format!("tensor decode rest error {}", e.to_string())))?
    {
        let text: String = sentence.clone().into_iter().collect();
        let text = format!("{text}{rest}");
        if let Some(text) = filter(&text) {
            if let Err(e) = tx.send(Ok(text.clone())).await {
                tracing::error!("chat send text error = {}", e);
            } else {
                tracing::info!("llm send text success, text = {}", text);
            }
        }
        text_result.clear();
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
    Ok(())
}

impl Llm for LlmQwen {
    fn chat(
        &self,
        system_prompt: String,
        text: String,
    ) -> impl Stream<Item = core::result::Result<String, ModelError>> + Unpin + Send + 'static {
        let tokenizer = self.tokenizer.clone();
        let model = self.model.clone();
        let device = self.device.clone();
        let (tx, rx) = channel::<core::result::Result<String, ModelError>>(10);
        thread::spawn(move || {
            block_on(async move {
                if let Err(e) =
                    handle_chat(system_prompt, text, tokenizer, model, device, tx.clone()).await
                {
                    if let Err(e) = tx.send(Err(e)).await {
                        tracing::error!("chat llmError send error = {}", e);
                    };
                };
                drop(tx);
            })
        });
        ReceiverStream::new(rx)
    }
}
