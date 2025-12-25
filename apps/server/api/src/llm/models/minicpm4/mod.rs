use crate::{
    common::ModelError,
    llm::{Model, models::token_converter::TokenConverter},
};
use aha::models::{GenerateModel, minicpm4::generate::MiniCPMGenerateModel};
use aha_openai_dive::v1::resources::chat::{
    ChatCompletionParameters, ChatMessage, ChatMessageContent,
};
use async_trait::async_trait;
use futures::{SinkExt, StreamExt, executor::block_on};
use futures_channel::mpsc::unbounded;
use rig::{
    completion::{CompletionError, CompletionRequest},
    message::{Message, UserContent},
    streaming::{RawStreamingChoice, StreamingCompletionResponse},
};
use std::{sync::Arc, thread};
use tokio::sync::Mutex;
use tracing::error;

pub struct Minicpm4<'a> {
    model: Arc<Mutex<MiniCPMGenerateModel<'a>>>,
}

impl<'a> Minicpm4<'a> {
    pub fn new(path: &str) -> core::result::Result<Self, ModelError> {
        let model = MiniCPMGenerateModel::init(path, None, None);
        match model {
            Ok(model) => Ok(Self {
                model: Arc::new(Mutex::new(model)),
            }),
            Err(e) => Err(ModelError::ModelInitFailure(e.to_string())),
        }
    }
}

#[async_trait]
impl<'a> Model for Minicpm4<'a> {
    async fn stream(
        &self,
        request: CompletionRequest,
    ) -> Result<
        StreamingCompletionResponse<rig::providers::openai::streaming::StreamingCompletionResponse>,
        CompletionError,
    > {
        // use futures_channel::mpsc::channel with buffer size can't correct running,it will suspend in async
        // can futures_channel::mpsc::unbounded to fix it
        let (mut tx, rx) = unbounded::<
            Result<
                RawStreamingChoice<rig::providers::openai::streaming::StreamingCompletionResponse>,
                CompletionError,
            >,
        >();
        let mes = convert_request(&request);
        let model = self.model.clone();
        let mut model = model.lock().await;
        //for has lifetime 'a field, thread need use thread::scope
        thread::scope(|scope| {
            let scoped_join_handle = scope.spawn(move || {
                block_on(async {
                    let mut token_converter = TokenConverter::new();
                    let stream = model.generate_stream(mes);
                    match stream {
                        Ok(mut stream) => {
                            while let Some(item) = stream.next().await {
                                let result = convert_response(item, &mut token_converter);
                                match result {
                                    Ok(items) => {
                                        if !items.is_empty() {
                                            for item in items {
                                                if let Err(e) = tx.send(Ok(item)).await {
                                                    error!("send text error = {}", e);
                                                }
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        if let Err(e) = tx.send(Err(e)).await {
                                            error!("send  error = {}", e);
                                        }
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            let e = CompletionError::ProviderError(e.to_string());
                            if let Err(e) = tx.send(Err(e)).await {
                                error!("send text error = {}", e);
                            }
                        }
                    }
                    drop(tx);
                });
            });
            match scoped_join_handle.join() {
                Ok(_) => {}
                Err(e) => {
                    error!("{:?}", e);
                }
            }
        });
        Ok(StreamingCompletionResponse::stream(Box::pin(rx)))
    }
}

fn convert_response(
    resposne: Result<
        aha_openai_dive::v1::resources::chat::ChatCompletionChunkResponse,
        anyhow::Error,
    >,
    token_converter: &mut TokenConverter,
) -> Result<
    Vec<rig::streaming::RawStreamingChoice<rig::providers::openai::StreamingCompletionResponse>>,
    rig::completion::CompletionError,
> {
    match resposne {
        Ok(response) => {
            let choices = response.choices.first();
            match choices {
                Some(choice) => {
                    let delta = &choice.delta;
                    match delta {
                        aha_openai_dive::v1::resources::chat::DeltaChatMessage::Developer {
                            content,
                            name,
                        } => todo!(),
                        aha_openai_dive::v1::resources::chat::DeltaChatMessage::System {
                            content,
                            name,
                        } => todo!(),
                        aha_openai_dive::v1::resources::chat::DeltaChatMessage::User {
                            content,
                            name,
                        } => todo!(),
                        aha_openai_dive::v1::resources::chat::DeltaChatMessage::Assistant {
                            content,
                            reasoning_content,
                            refusal,
                            name,
                            tool_calls,
                        } => {
                            let mut text_collection = String::from("");
                            if let Some(content) = content {
                                match content {
                                    ChatMessageContent::Text(text) => {
                                        text_collection.push_str(text);
                                    }
                                    ChatMessageContent::ContentPart(chat_message_content_parts) => {
                                        todo!()
                                    }
                                    ChatMessageContent::None => todo!(),
                                }
                                // TODO: mcp call handle, it return function call xml not tool call json
                                if text_collection.is_empty() {
                                    Ok(token_converter.accept_final_text(&text_collection)?)
                                } else {
                                    Ok(token_converter.accept_text(&text_collection)?)
                                }
                            } else {
                                todo!()
                            }
                        }
                        aha_openai_dive::v1::resources::chat::DeltaChatMessage::Tool {
                            content,
                            tool_call_id,
                        } => todo!(),
                        aha_openai_dive::v1::resources::chat::DeltaChatMessage::Untagged {
                            content,
                            reasoning_content,
                            refusal,
                            name,
                            tool_calls,
                            tool_call_id,
                        } => todo!(),
                    }
                }
                None => todo!(),
            }
        }
        Err(e) => Err(CompletionError::ResponseError(e.to_string())),
    }
}

fn convert_request(request: &CompletionRequest) -> ChatCompletionParameters {
    let mut prompt = String::new();
    let mut messages = vec![];
    if let Some(text) = &request.preamble {
        prompt.push_str(text.as_str());
    }
    if !request.tools.is_empty() {
        // llm tool call agent example see: https://github.com/QwenLM/Qwen-Agent/blob/main/docs/llm.md
        // llm tool call template see:
        // 1. https://qwen.readthedocs.io/en/latest/getting_started/concepts.html#tool-calling
        // 2. https://qwen.readthedocs.io/en/latest/run_locally/ollama.html
        // 3. https://github.com/NousResearch/Hermes-Function-Calling#prompt-format-for-function-calling
        // reqeust object see: https://github.com/0xPlaygrounds/rig/blob/main/rig-core/src/completion/request.rs
        let mut tools_str = String::new();
        let tools = &request.tools;
        for tool in tools.iter() {
            let tool = rig::providers::openai::ToolDefinition::from(tool.clone());
            let tool_json_str = serde_json::to_string(&tool).unwrap();
            tools_str.push_str(&format!("{}\n", tool_json_str));
        }
        let mut tools_prompt = String::new();
        tools_prompt.push_str(&format!(
            "# Tools\n
            You may call one or more functions to assist with the user query.\n
            You are provided with function signatures within <tools></tools> XML tags:\n
            <tools>\n
            {}
            </tools>\n
            For each function call, return a json object with function name and arguments within <tool_call></tool_call> XML tags:\n
            <tool_call>\n
            {{\"name\": <function-name>, \"arguments\": <args-json-object>}}\n
            </tool_call>\n
            ",
        tools_str
        ));
        prompt.push_str(&tools_prompt);
    }
    if !prompt.is_empty() {
        messages.push(ChatMessage::System {
            content: ChatMessageContent::Text(prompt),
            name: None,
        });
    }
    for message in request.chat_history.iter() {
        match message {
            Message::User { content } => {
                for content in content.iter() {
                    match content {
                        UserContent::Text(text) => {
                            let message = ChatMessage::User {
                                content: ChatMessageContent::Text(text.text.to_string()),
                                name: None,
                            };
                            messages.push(message);
                        }
                        UserContent::ToolResult(tool_result) => todo!(),
                        UserContent::Image(image) => todo!(),
                        UserContent::Audio(audio) => todo!(),
                        UserContent::Video(video) => todo!(),
                        UserContent::Document(document) => todo!(),
                    }
                }
            }
            Message::Assistant { id, content } => todo!(),
        }
    }
    ChatCompletionParameters {
        model: "minicpm4".to_string(),
        messages,
        ..Default::default()
    }
}
