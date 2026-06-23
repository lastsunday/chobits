use crate::common::ModelError;
use crate::tts::{Tts, TtsData, TtsError};
use async_trait::async_trait;
use futures::Stream;
use futures::executor::block_on;
use std::pin::Pin;
use std::thread;
use tokio::sync::mpsc::channel;
use tokio_stream::StreamExt;
use tokio_stream::wrappers::ReceiverStream;
use tokio_util::sync::CancellationToken;
use tracing::error;

pub struct TtsMute {}

impl TtsMute {
    pub async fn new() -> Result<Self, anyhow::Error> {
        Ok(Self {})
    }
}

#[async_trait]
impl Tts for TtsMute {
    async fn stream(
        &self,
        mut text_stream: Pin<
            Box<dyn Stream<Item = core::result::Result<String, ModelError>> + Send + Sync>,
        >,
        cancel: CancellationToken,
    ) -> Pin<Box<dyn Stream<Item = core::result::Result<TtsData, TtsError>> + Send + Sync>> {
        let (tx, rx) = channel(10);
        thread::spawn(move || {
            block_on(async move {
                while let Some(text) = text_stream.next().await {
                    if cancel.is_cancelled() {
                        break;
                    }
                    let tx = tx.clone();
                    match &text {
                        Ok(text) => {
                            let data = TtsData {
                                audio: None,
                                text: text.to_string(),
                                raw_pcm: None,
                            };
                            if let Err(e) = tx.send(Ok(data)).await {
                                error!("output packet error = {}", e);
                                break;
                            }
                        }
                        Err(e) => {
                            error!("tts text stream error = {}", e.to_string());
                            if let Err(e) = tx.send(Err(TtsError::Text(e.to_string()))).await {
                                error!("send error failure = {}", e);
                            }
                            break;
                        }
                    }
                }
                drop(tx);
            })
        });
        Box::pin(ReceiverStream::new(rx))
    }
}
