#[cfg(feature = "mkl")]
extern crate intel_mkl_src;

#[cfg(feature = "accelerate")]
extern crate accelerate_src;

pub mod quantized;

use super::token_output_stream::TokenOutputStream;
use crate::ws::{
    common::{ModelError, device, format_size},
    llm::Model,
    util::llm::{filter, filter_think},
};
use async_trait::async_trait;
use candle_core::{Device, Tensor, quantized::gguf_file};
use candle_transformers::generation::{LogitsProcessor, Sampling};
use futures::{SinkExt, executor::block_on};
use futures_channel::mpsc::{Sender, channel};
use quantized::ModelWeights as Qwen3;
use regex::Regex;
use rig::{
    completion::{CompletionError, CompletionRequest, CompletionResponse},
    message::{AssistantContent, Message, UserContent},
    streaming::{RawStreamingChoice, StreamingCompletionResponse},
};
use std::thread;
use tokenizers::Tokenizer;

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
        let device = device(false)?;
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

fn convert_request_to_prompt(request: &CompletionRequest) -> String {
    let mut prompt = String::new();
    if let Some(text) = &request.preamble {
        prompt.push_str(&format!(
            "<|im_start|>system\n{} /no_think<|im_end|>\n",
            text
        ));
    }
    let chat_history = &request.chat_history;
    for message in chat_history.iter() {
        match message {
            Message::User { content } => {
                let items = content.iter();
                for item in items {
                    match item {
                        UserContent::Text(text) => {
                            prompt.push_str(&format!("|im_start|>user\n{}<|im_end|>\n", text.text));
                        }
                        _ => {
                            // TODO: fix other
                        }
                    }
                }
            }
            Message::Assistant { id: _, content } => {
                let items = content.iter();
                for item in items {
                    match item {
                        AssistantContent::Text(text) => {
                            prompt.push_str(&format!(
                                "|im_start|>assistant\n{}<|im_end|>\n",
                                text.text
                            ));
                        }
                        _ => {
                            // TODO: fix other
                        }
                    }
                }
            }
        }
    }
    prompt.push_str(r#"|im_start|>assistant\n"#);
    prompt
}

async fn handle(
    request: &CompletionRequest,
    tokenizer: Tokenizer,
    mut model: Qwen3,
    device: Device,
    mut tx: Sender<
        Result<
            RawStreamingChoice<rig::providers::openai::streaming::StreamingCompletionResponse>,
            CompletionError,
        >,
    >,
) -> Result<(), CompletionError> {
    let mut tos = TokenOutputStream::new(tokenizer);
    let prompt_str = convert_request_to_prompt(request);
    tracing::info!("formatted prompt: {}", &prompt_str);

    let tokens = tos
        .tokenizer()
        .encode(prompt_str, true)
        .map_err(|e| ModelError::Chat(format!("tokenizer encode error {}", e)))?;
    let tokens = tokens.get_ids();

    // TODO:setting
    let to_sample = request.max_tokens.unwrap_or(999) as usize;
    let temperature = request.temperature.unwrap_or(0.8);
    let seed = 299792458;
    let repeat_last_n = 64;
    let repeat_penalty = 1.1;

    let mut all_tokens = vec![];
    let mut text_result = Vec::new();
    let mut skip_think = false;

    let mut logits_processor = LogitsProcessor::from_sampling(seed, Sampling::All { temperature });

    let start_prompt_processing = std::time::Instant::now();

    let input = Tensor::new(tokens, &device)
        .map_err(|e| ModelError::Chat(format!("tensor create error {}", e)))?
        .unsqueeze(0)
        .map_err(|e| ModelError::Chat(format!("tensor create unsqueeze error {}", e)))?;
    let logits = model
        .forward(&input, 0)
        .map_err(|e| ModelError::Chat(format!("model forward error {}", e)))?;
    let logits = logits
        .squeeze(0)
        .map_err(|e| ModelError::Chat(format!("tensor squeeze error {}", e)))?;
    let mut next_token = logits_processor
        .sample(&logits)
        .map_err(|e| ModelError::Chat(format!("tensor processor sample error {}", e)))?;

    let prompt_dt = start_prompt_processing.elapsed();

    all_tokens.push(next_token);
    if let Some(t) = tos
        .next_token(next_token)
        .map_err(|e| ModelError::Chat(format!("tensor encoding error {}", e)))?
    {
        text_result.push(t);
    }

    let eos_token = *tos
        .tokenizer()
        .get_vocab(true)
        .get("<|im_end|>")
        .ok_or_else(|| ModelError::Chat("tensor can't get eos_token error ".to_string()))?;

    let start_post_prompt = std::time::Instant::now();

    let mut sampled = 0;
    let mut sentence_list: Vec<String> = Vec::new();
    let mut sentence: Vec<char> = Vec::new();
    let regex = Regex::new(r"[。！？!?；;]").unwrap();
    for index in 0..to_sample {
        let input = Tensor::new(&[next_token], &device)
            .map_err(|e| ModelError::Chat(format!("tensor create error {}", e)))?
            .unsqueeze(0)
            .map_err(|e| ModelError::Chat(format!("tensor create unsqueeze error {}", e)))?;
        let logits = model
            .forward(&input, tokens.len() + index)
            .map_err(|e| ModelError::Chat(format!("model forward error {}", e)))?;
        let logits = logits
            .squeeze(0)
            .map_err(|e| ModelError::Chat(format!("tensor squeeze error {}", e)))?;
        let logits = {
            let start_at = all_tokens.len().saturating_sub(repeat_last_n);
            candle_transformers::utils::apply_repeat_penalty(
                &logits,
                repeat_penalty,
                &all_tokens[start_at..],
            )
            .map_err(|e| ModelError::Chat(format!("tensor apply repeat penalty error {}", e)))?
        };
        next_token = logits_processor
            .sample(&logits)
            .map_err(|e| ModelError::Chat(format!("tensor processor sample error {}", e)))?;
        all_tokens.push(next_token);
        if let Some(t) = tos
            .next_token(next_token)
            .map_err(|e| ModelError::Chat(format!("tensor encoding error {}", e)))?
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
                text.chars().for_each(|c| {
                    sentence.push(c);
                    // Break a sentence
                    if regex.is_match(&c.to_string()) {
                        let text: String = sentence.clone().into_iter().collect();
                        sentence.clear();
                        if let Some(text) = filter(&text) {
                            sentence_list.push(text);
                        }
                    }
                });
                text_result.clear();
            }
        }
        for text in sentence_list.clone() {
            let message = RawStreamingChoice::Message(text.clone());
            if let Err(e) = tx.send(Ok(message)).await {
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

    let mut last_content = None;
    if let Some(rest) = tos
        .decode_rest()
        .map_err(|e| ModelError::Chat(format!("tensor decode rest error {}", e)))?
    {
        let text: String = sentence.clone().into_iter().collect();
        let text = format!("{text}{rest}");
        last_content = Some(text);
    } else if !sentence.is_empty() {
        let result: String = sentence.clone().into_iter().collect();
        last_content = Some(result);
    }
    if let Some(text) = last_content
        && let Some(text) = filter(&text)
    {
        let message = RawStreamingChoice::Message(text.clone());
        if let Err(e) = tx.send(Ok(message)).await {
            tracing::error!("chat send text error = {}", e);
        } else {
            tracing::info!("llm send text success, text = {}", text);
        }
    }
    text_result.clear();

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

#[async_trait]
impl Model for LlmQwen {
    async fn completion(
        &self,
        _request: CompletionRequest,
    ) -> Result<CompletionResponse<rig::providers::openai::CompletionResponse>, CompletionError>
    {
        todo!()
    }

    async fn stream(
        &self,
        request: CompletionRequest,
    ) -> Result<
        StreamingCompletionResponse<rig::providers::openai::streaming::StreamingCompletionResponse>,
        CompletionError,
    > {
        let tokenizer = self.tokenizer.clone();
        let model = self.model.clone();
        let device = self.device.clone();
        let (mut tx, rx) = channel::<
            Result<
                RawStreamingChoice<rig::providers::openai::streaming::StreamingCompletionResponse>,
                CompletionError,
            >,
        >(10);
        thread::spawn(move || {
            block_on(async move {
                if let Err(e) = handle(&request, tokenizer, model, device, tx.clone()).await {
                    if let Err(e) = tx.send(Err(e)).await {
                        tracing::error!("chat llmError send error = {}", e);
                    };
                };
                drop(tx);
            })
        });
        Ok(StreamingCompletionResponse::stream(Box::pin(rx)))
    }
}
