use std::{sync::Arc, thread};

use crate::{
    common::ModelError,
    llm::{DummyModel, Model, chat::Chat},
    mcp::mcp_host::McpHost,
};
use framework::id::gen_id;
use futures::StreamExt;
use futures::{Stream, executor::block_on};
use rig::{
    OneOrMany,
    completion::{CompletionError, CompletionRequest},
    message::{AssistantContent, Message, Reasoning, Text, ToolCall, UserContent},
    streaming::{StreamedAssistantContent, StreamingCompletionResponse},
};
use tokio::sync::{
    Mutex,
    mpsc::{Sender, channel},
};
use tokio_stream::wrappers::ReceiverStream;
use tracing::{debug, error};

#[derive(Clone)]
pub struct Client {
    model: Arc<Box<dyn Model>>,
    temperature: Option<f64>,
    max_tokens: Option<u64>,
    history: Arc<Mutex<History>>,
    mcp_host: Option<Arc<Mutex<dyn McpHost>>>,
}

pub struct ChatRequest {
    pub message: Message,
}

pub struct History {
    pub preamble: Option<String>,
    pub chat_history: Vec<Message>,
}

impl Client {
    pub fn builder() -> ClientBuilder {
        ClientBuilder::new()
    }

    pub fn with_history(mut self, history: Arc<Mutex<History>>) -> Self {
        self.history = history;
        self
    }

    pub fn with_temperature(mut self, temperature: Option<f64>) -> Self {
        self.temperature = temperature;
        self
    }

    pub fn with_max_tokens(mut self, max_tokens: Option<u64>) -> Self {
        self.max_tokens = max_tokens;
        self
    }

    pub fn chat(
        &self,
        request: ChatRequest,
    ) -> impl Stream<Item = core::result::Result<String, ModelError>> + Unpin + Send + 'static {
        let (tx, rx) = channel::<core::result::Result<String, ModelError>>(10);
        let model = self.model.clone();
        let mcp_host = self.mcp_host.clone();
        let tx_main = tx.clone();
        let clone_history = self.history.clone();
        let temperature = self.temperature;
        let max_tokens = self.max_tokens;
        thread::spawn(move || {
            let output = block_on(async move {
                let tools = {
                    if let Some(mcp_host) = &mcp_host {
                        let mcp_host = mcp_host.lock().await;
                        mcp_host.get_tool().await?
                    } else {
                        vec![]
                    }
                };
                let mut has_next_step = true;
                while has_next_step {
                    let history = clone_history.clone();
                    let mut history = history.lock().await;
                    let chat_history = {
                        if !history.chat_history.is_empty() {
                            let mut result = OneOrMany::many(history.chat_history.clone()).unwrap();
                            result.push(request.message.clone());
                            result
                        } else {
                            OneOrMany::one(request.message.clone())
                        }
                    };
                    history.chat_history.push(request.message.clone());
                    let preamble = history.preamble.clone();
                    drop(history);
                    let request = CompletionRequest {
                        preamble: preamble.clone(),
                        chat_history: chat_history.clone(),
                        documents: vec![],
                        tools: tools.clone(),
                        temperature,
                        max_tokens,
                        tool_choice: None,
                        additional_params: None,
                    };
                    let response = model.stream(request).await;
                    let messages = handle_response(response, Some(tx.clone())).await;
                    match messages {
                        Ok(messages) => {
                            has_next_step = false;
                            for message in &messages {
                                let history = clone_history.clone();
                                let mut history = history.lock().await;
                                history.chat_history.push(message.clone());
                                drop(history);
                                match message {
                                    Message::User { content: _content } => {
                                        //skip
                                    }
                                    Message::Assistant { id: _id, content } => {
                                        for item in content.iter() {
                                            match item {
                                                AssistantContent::ToolCall(ToolCall {
                                                    id,
                                                    call_id,
                                                    function,
                                                }) => {
                                                    if let Some(mcp_host) = mcp_host.clone() {
                                                        let mcp_host = mcp_host.lock().await;
                                                        let result = mcp_host
                                                            .call_tool(ToolCall {
                                                                id: id.clone(),
                                                                call_id: call_id.clone(),
                                                                function: function.clone(),
                                                            })
                                                            .await?;
                                                        let history = clone_history.clone();
                                                        let mut history = history.lock().await;
                                                        history.chat_history.push(Message::User {
                                                            content: OneOrMany::<UserContent>::one(
                                                                UserContent::ToolResult(result),
                                                            ),
                                                        });
                                                        drop(history);
                                                        has_next_step = true;
                                                    }
                                                }
                                                _ => {
                                                    //skip
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        Err(_) => todo!(),
                    }
                }
                drop(tx);
                anyhow::Ok(())
            });
            match output {
                Ok(_) => {
                    drop(tx_main);
                }
                Err(e) => {
                    block_on(async move {
                        if let Err(e) = tx_main.send(Err(ModelError::Chat(e.to_string()))).await {
                            error!("{:?}", e);
                        }
                        drop(tx_main);
                    });
                }
            }
        });
        ReceiverStream::new(rx)
    }
}

pub async fn handle_response(
    response: Result<
        StreamingCompletionResponse<rig::providers::openai::streaming::StreamingCompletionResponse>,
        CompletionError,
    >,
    tx: Option<Sender<Result<String, ModelError>>>,
) -> anyhow::Result<Vec<Message>> {
    let mut messages: Vec<Message> = vec![];
    let mut text_collector = String::new();
    let mut chat = Chat::new();
    match response {
        Ok(mut stream) => {
            while let Some(value) = stream.next().await {
                match value {
                    Ok(StreamedAssistantContent::Text(text)) => {
                        text_collector.push_str(&text.text);
                        if let Some(tx) = &tx {
                            let sentence_list = chat.accept_text(&text.text);
                            let sentence_iter = sentence_list.iter();
                            for sentence in sentence_iter {
                                tx.send(Ok(sentence.to_string())).await?;
                            }
                        }
                    }
                    Ok(StreamedAssistantContent::Final(
                        rig::providers::openai::StreamingCompletionResponse { usage },
                    )) => {
                        // TODO:
                        debug!("{:?}", usage);
                    }
                    Ok(StreamedAssistantContent::ToolCall(ToolCall {
                        id,
                        call_id,
                        function,
                    })) => {
                        messages.push(Message::Assistant {
                            id: Some(id.clone()),
                            content: OneOrMany::<AssistantContent>::one(
                                AssistantContent::ToolCall(ToolCall {
                                    id: id.clone(),
                                    call_id: call_id.clone(),
                                    function,
                                }),
                            ),
                        });
                    }
                    Ok(StreamedAssistantContent::ToolCallDelta { id: _id, delta }) => {
                        // TODO:
                        debug!("{:?}", delta);
                    }
                    Ok(StreamedAssistantContent::Reasoning(Reasoning {
                        id: _id,
                        reasoning,
                        ..
                    })) => {
                        // TODO:
                        debug!("{:?}", reasoning);
                    }
                    Err(e) => {
                        if let Some(tx) = &tx {
                            tx.send(Err(ModelError::ModelCompletionError(e.to_string())))
                                .await?;
                        }
                    }
                }
            }
            if let Some(tx) = &tx {
                let sentence_list = chat.accept_final();
                let sentence_iter = sentence_list.iter();
                for sentence in sentence_iter {
                    tx.send(Ok(sentence.to_string())).await?;
                }
            }
            if !text_collector.is_empty() {
                messages.push(Message::Assistant {
                    id: Some(gen_id()),
                    content: OneOrMany::<AssistantContent>::one(AssistantContent::Text(Text {
                        text: text_collector,
                    })),
                });
            }
            Ok(messages)
        }
        Err(e) => Err(anyhow::anyhow!(e.to_string())),
    }
}

pub struct ClientBuilder {
    model: Arc<Box<dyn Model>>,
    mcp_host: Option<Arc<Mutex<dyn McpHost>>>,
}

impl ClientBuilder {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn with_model(mut self, model: Arc<Box<dyn Model>>) -> ClientBuilder {
        self.model = model;
        self
    }

    pub fn with_mcp_host(mut self, mcp_host: Arc<Mutex<dyn McpHost>>) -> ClientBuilder {
        self.mcp_host = Some(mcp_host);
        self
    }

    pub fn build(self) -> Client {
        Client {
            model: self.model,
            temperature: None,
            max_tokens: None,
            history: Arc::new(Mutex::new(History {
                preamble: None,
                chat_history: vec![],
            })),
            mcp_host: self.mcp_host,
        }
    }
}

impl Default for ClientBuilder {
    fn default() -> Self {
        Self {
            model: Arc::new(Box::new(DummyModel::default())),
            mcp_host: None,
        }
    }
}
