use crate::{
    config,
    ws::{
        common::ModelError,
        state::State,
        tts::Tts,
        util::llm::{EMOJI_MAP, analyze_emotion},
    },
};
use axum::extract::ws::Message;
use futures::Stream;
use futures_util::{Sink, SinkExt};
use serde::Serialize;
use service::chobits::message::{
    llm::LlmMessage,
    tts::{TtsMessage, TtsState},
};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time::{Instant, sleep};
use tokio_stream::StreamExt;

pub struct Sender<W, T>
where
    W: Sink<Message> + Unpin,
    T: Tts,
{
    write: Box<W>,
    tts: Box<T>,
    state: Arc<Mutex<State>>,
}

impl<W, T> Sender<W, T>
where
    W: Sink<Message> + Unpin,
    T: Tts,
{
    pub fn new(write: Box<W>, tts: Box<T>, state: Arc<Mutex<State>>) -> Self {
        Self { write, tts, state }
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
        let audio_config = config::get().audio();
        let delay = audio_config.output_frame_duration();
        let mut output = self.tts.output(text);
        let mut latest_time = Instant::now() + Duration::from_millis(delay);
        // pre buffer count
        let pre_buffer_frame_count: u64 = 6;
        let mut send_frame_count: u64 = 0;
        let mut state = self.state.lock().await;
        state.client_speaking = true;
        drop(state);
        while let Some(packet) = output.next().await {
            let now = Instant::now();
            let offset = (now - latest_time).as_millis() as u64;
            let mut actual_delay: u64 = 0;
            if offset < delay {
                actual_delay = delay - offset;
            }
            if send_frame_count >= pre_buffer_frame_count && actual_delay > 0 {
                sleep(Duration::from_millis(actual_delay)).await;
            }
            latest_time = Instant::now();
            if self
                .write
                .send(Message::Binary(packet.into()))
                .await
                .is_err()
            {
                return Err(SenderError::SendError);
            }
            send_frame_count += 1;
            let mut state = self.state.lock().await;
            state.update_last_activity_time();
            if !state.client_speaking {
                break;
            }
            drop(state);
        }
        let mut state = self.state.lock().await;
        state.client_speaking = false;
        drop(state);
        Ok(())
    }

    pub async fn send_audio(&mut self, data: Vec<Vec<u8>>) -> Result<(), SenderError> {
        let audio_config = config::get().audio();
        let delay = audio_config.output_frame_duration();
        let mut latest_time = Instant::now() + Duration::from_millis(delay);
        // pre buffer count
        let pre_buffer_frame_count: u64 = 6;
        let mut send_frame_count: u64 = 0;
        let mut state = self.state.lock().await;
        state.client_speaking = true;
        drop(state);
        let mut data = data.into_iter();
        while let Some(packet) = data.next() {
            let now = Instant::now();
            let offset = (now - latest_time).as_millis() as u64;
            let mut actual_delay: u64 = 0;
            if offset < delay {
                actual_delay = delay - offset;
            }
            if send_frame_count >= pre_buffer_frame_count && actual_delay > 0 {
                sleep(Duration::from_millis(actual_delay)).await;
            }
            latest_time = Instant::now();
            if self
                .write
                .send(Message::Binary(packet.into()))
                .await
                .is_err()
            {
                return Err(SenderError::SendError);
            }
            send_frame_count += 1;
            let mut state = self.state.lock().await;
            state.update_last_activity_time();
            if !state.client_speaking {
                break;
            }
            drop(state);
        }
        let mut state = self.state.lock().await;
        state.client_speaking = false;
        drop(state);
        Ok(())
    }

    pub async fn send_tts_with_text(&mut self, text: String) -> Result<(), SenderError> {
        let emotion = analyze_emotion(&text);
        if self.send_llm(emotion.to_string()).await.is_err() {
            return Err(SenderError::SendError);
        }
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

    pub async fn send_tts_with_text_stream(
        &mut self,
        text_stream: impl Stream<Item = core::result::Result<String, ModelError>>
        + Unpin
        + Send
        + 'static,
    ) -> Result<(), SenderError> {
        let tts = &self.tts;
        let mut text_stream = tts.output_stream(text_stream);
        while let Some(data) = text_stream.next().await {
            match data {
                Ok(data) => {
                    let text = data.text;
                    let emotion = analyze_emotion(&text);
                    if self.send_llm(emotion.to_string()).await.is_err() {
                        return Err(SenderError::SendError);
                    }
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
                    if self.send_audio(data.audio).await.is_err() {
                        return Err(SenderError::SendError);
                    }
                    tracing::info!("send audio success, text = {}", text);
                    if self
                        .send_json_text(&TtsMessage::new(Some(TtsState::SentenceEnd), None))
                        .await
                        .is_err()
                    {
                        return Err(SenderError::SendError);
                    }
                }
                Err(e) => {
                    tracing::error!("send tts error {}", e.to_string());
                }
            }
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

    pub async fn send_llm(&mut self, emotion: String) -> Result<(), SenderError> {
        let emoji = EMOJI_MAP.get(emotion.as_str());
        let mut emoji_text = "🙂";
        match emoji {
            Some(item) => {
                emoji_text = item;
            }
            None => {}
        }
        if self
            .send_json_text(&LlmMessage::new(
                None,
                Some(emotion),
                Some(emoji_text.to_string()),
            ))
            .await
            .is_err()
        {
            return Err(SenderError::SendError);
        }
        Ok(())
    }

    pub async fn close(&mut self) {
        let result = self.write.close().await;
        match result {
            Ok(_) => (),
            Err(_) => {
                tracing::error!("sender close error");
            }
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SenderError {
    #[error("Json Error: {0}")]
    JsonError(#[from] serde_json::Error),
    #[error("Send Error ")]
    SendError,
}
