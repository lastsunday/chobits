use std::time::Duration;

use axum::extract::ws::Message;
use futures_util::{Sink, SinkExt};
use serde::Serialize;
use service::chobits::message::tts::{TtsMessage, TtsState};
use tokio::time::{Instant, sleep_until};
use tokio_stream::StreamExt;

use crate::ws::tts::Tts;

pub struct Sender<W, T>
where
    W: Sink<Message> + Unpin,
    T: Tts,
{
    write: Box<W>,
    tts: Box<T>,
}

impl<W, T> Sender<W, T>
where
    W: Sink<Message> + Unpin,
    T: Tts,
{
    pub fn new(write: Box<W>, tts: Box<T>) -> Self {
        Self { write, tts }
    }

    pub async fn send_json_text<V>(&mut self, value: &V) -> Result<(), SenderError>
    where
        V: Serialize,
    {
        let result: String = serde_json::to_string(value)?;
        if self
            .write
            .send(Message::Text(result.clone().into()))
            .await
            .is_err()
        {
            Err(SenderError::SendError)
        } else {
            tracing::info!("send json text success = {}", result);
            Ok(())
        }
    }

    pub async fn send_tts(&mut self, text: String) -> Result<(), SenderError> {
        let delay = 60;
        let mut output = self.tts.output(text);
        let mut latest_time = Instant::now() + Duration::from_millis(delay);
        while let Some(packet) = output.next().await {
            let now = Instant::now();
            let offset = now - latest_time + Duration::from_millis(delay);
            sleep_until(now + offset).await;
            if self
                .write
                .send(Message::Binary(packet.into()))
                .await
                .is_err()
            {
                return Err(SenderError::SendError);
            }
            latest_time = Instant::now();
        }
        Ok(())
    }

    pub async fn send_tts_with_text(&mut self, text: String) -> Result<(), SenderError> {
        if self
            .send_json_text(&TtsMessage::new(
                Some(TtsState::SentenceStart),
                Some(text.clone()),
            ))
            .await
            .is_err()
        {
            return Err(SenderError::SendError);
        }
        if self
            .send_json_text(&TtsMessage::new(Some(TtsState::Start), None))
            .await
            .is_err()
        {
            return Err(SenderError::SendError);
        }
        if self.send_tts(text.clone()).await.is_err() {
            return Err(SenderError::SendError);
        }
        if self
            .send_json_text(&TtsMessage::new(Some(TtsState::SentenceEnd), None))
            .await
            .is_err()
        {
            return Err(SenderError::SendError);
        }
        if self
            .send_json_text(&TtsMessage::new(Some(TtsState::Stop), None))
            .await
            .is_err()
        {
            return Err(SenderError::SendError);
        }
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SenderError {
    #[error("Json Error: {0}")]
    JsonError(#[from] serde_json::Error),
    #[error("Send Error ")]
    SendError,
}
