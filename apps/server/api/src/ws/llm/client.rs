use std::{sync::Arc, thread};

use crate::ws::{
    common::ModelError,
    llm::{DummyModel, Model, chat::Chat},
};
use futures::StreamExt;
use futures::{Stream, executor::block_on};
use rig::{
    completion::CompletionRequest,
    message::{Reasoning, ToolCall},
    providers::openai::StreamingCompletionResponse,
    streaming::StreamedAssistantContent,
};
use tokio::sync::mpsc::channel;
use tokio_stream::wrappers::ReceiverStream;
use tracing::error;

#[derive(Clone)]
pub struct Client {
    model: Arc<Box<dyn Model>>,
    // TODO: mcp call logic?
}

impl Client {
    pub fn builder() -> ClientBuilder {
        ClientBuilder::new()
    }

    pub fn chat(
        &self,
        request: CompletionRequest,
    ) -> impl Stream<Item = core::result::Result<String, ModelError>> + Unpin + Send + 'static // TODO:return text or mcp result?
    {
        let (tx, rx) = channel::<core::result::Result<String, ModelError>>(10);
        let model = self.model.clone();
        thread::spawn(move || {
            block_on(async move {
                let mut chat = Chat::new();
                let response = model.stream(request).await;
                match response {
                    Ok(mut stream) => {
                        // TODO:
                        while let Some(value) = stream.next().await {
                            match value {
                                Ok(StreamedAssistantContent::Text(text)) => {
                                    let sentence_list = chat.accept_text(&text.text);
                                    let sentence_iter = sentence_list.iter();
                                    for sentence in sentence_iter {
                                        let e = tx.send(Ok(sentence.to_string())).await;
                                        if let Err(e) = e {
                                            error!("{:?}", e);
                                        }
                                    }
                                }
                                Ok(StreamedAssistantContent::Final(
                                    StreamingCompletionResponse { .. },
                                )) => {
                                    // TODO:
                                }
                                Ok(StreamedAssistantContent::ToolCall(ToolCall { .. })) => {
                                    // TODO:
                                }
                                Ok(StreamedAssistantContent::Reasoning(Reasoning {
                                    reasoning: _,
                                    ..
                                })) => {
                                    // TODO:
                                }
                                Err(e) => {
                                    let e = tx.send(Err(e.into())).await;
                                    if let Err(e) = e {
                                        error!("{:?}", e);
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        let e = tx.send(Err(e.into())).await;
                        if let Err(e) = e {
                            error!("{:?}", e);
                        }
                    }
                }
                let sentence_list = chat.accept_final();
                let sentence_iter = sentence_list.iter();
                for sentence in sentence_iter {
                    let e = tx.send(Ok(sentence.to_string())).await;
                    if let Err(e) = e {
                        error!("{:?}", e);
                    }
                }
                drop(tx);
            })
        });
        ReceiverStream::new(rx)
    }
}

pub struct ClientBuilder {
    model: Arc<Box<dyn Model>>,
}

impl ClientBuilder {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn with_client(self, model: Arc<Box<dyn Model>>) -> ClientBuilder {
        ClientBuilder { model }
    }

    pub fn build(self) -> Client {
        Client { model: self.model }
    }
}

impl Default for ClientBuilder {
    fn default() -> Self {
        Self {
            model: Arc::new(Box::new(DummyModel::default())),
        }
    }
}
