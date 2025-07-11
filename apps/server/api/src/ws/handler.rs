use std::sync::Arc;

use axum::{body::Bytes, extract::ws::Message};
use service::chobits::message::{
    AudioFormat, Transport,
    abort::AbortMessage,
    hello::{AudioParam, HelloMessage},
    listen::{ListenMessage, ListenState},
    stt::SttMessage,
    tts::{TtsMessage, TtsState},
};
use tokio::sync::Mutex;

use crate::ws::{sender::Sender, tts::Tts};
use futures_util::Sink;

pub struct Handler<W, T>
where
    W: Sink<Message> + Unpin + 'static,
    T: Tts + 'static,
{
    session_id: String,
    sender: Box<Arc<Mutex<Sender<W, T>>>>,
    state: Arc<Mutex<State>>,
}

impl<W, T> Handler<W, T>
where
    W: Sink<Message> + Unpin + Send,
    T: Tts + Send,
{
    pub fn new(session_id: String, sender: Box<Arc<Mutex<Sender<W, T>>>>) -> Self {
        Self {
            session_id,
            sender,
            state: Arc::new(Mutex::new(State::new())),
        }
    }

    pub fn handle_hello(&self, message: HelloMessage) {
        let session_id = self.session_id.clone();
        let sender = self.sender.as_ref().clone();
        tokio::spawn(async move {
            let data = HelloMessage {
                message: service::chobits::message::Message {
                    mtype: service::chobits::message::Type::Hello,
                },
                transport: Some(Transport::Websocket),
                audio_params: Some(AudioParam {
                    format: AudioFormat::Opus,
                    sample_rate: 24000,
                    channels: 1,
                    frame_duration: 60,
                }),
                version: None,
                features: None,
                session_id: Some(session_id),
            };
            let mut sender = sender.lock().await;
            match sender.send_json_text(&data).await {
                Ok(_) => {}
                Err(error) => {
                    tracing::info!("send hello message error {}", error);
                }
            }
        });
    }

    pub fn handle_listen(&self, message: ListenMessage) {
        let message = message.clone();
        let sender = self.sender.as_ref().clone();
        let state = self.state.clone();
        let session_id = self.session_id.clone();
        tokio::spawn(async move {
            match message.state {
                ListenState::Start => {
                    let mut state = state.lock().await;
                    state.listen_start = true;
                }
                ListenState::Stop => {
                    let mut state = state.lock().await;
                    state.listen_start = false;
                }
                ListenState::Detect => {
                    // TODO: send stt from text,
                    // TODO: chatStreamBySentence
                    match message.text {
                        Some(text) => {
                            tracing::info!("listen detect text = {}", text);
                            let data = SttMessage::new(Some(session_id), Some(text.clone()));
                            let mut sender = sender.lock().await;
                            match sender.send_json_text(&data).await {
                                Ok(_) => (),
                                Err(error) => {
                                    tracing::info!("send tts message error {}", error);
                                }
                            }
                            match sender.send_tts_with_text(String::from(text.clone())).await {
                                Ok(_) => {}
                                Err(error) => {
                                    tracing::info!("send stt error {}", error);
                                }
                            };
                        }
                        None => {
                            tracing::info!("listen detect text not exists");
                        }
                    }
                }
                ListenState::Text => {
                    // TODO: if audio playing, stop audio logic, send tts message stop
                    // TODO: else send stt from text,
                    match message.text {
                        Some(text) => {
                            tracing::info!("listen text text = {}", text);
                            let data = TtsMessage::new(None, Some(text));
                            let mut sender = sender.lock().await;
                            match sender.send_json_text(&data).await {
                                Ok(_) => (),
                                Err(error) => {
                                    tracing::info!("send tts message error {}", error);
                                }
                            }
                        }
                        None => {
                            tracing::info!("listen text text not exists");
                        }
                    }
                }
            }
        });
    }

    pub fn handle_abort(&self, message: AbortMessage) {
        let sender = self.sender.as_ref().clone();
        tokio::spawn(async move {
            let data = TtsMessage::new(Some(TtsState::Stop), None);
            let mut sender = sender.lock().await;
            match sender.send_json_text(&data).await {
                Ok(_) => (),
                Err(error) => {
                    tracing::info!("send abort message error {}", error);
                }
            }
        });
    }

    pub fn handle_voice(&self, data: Bytes) {
        // let mut output = vec![0i16; mono_60_ms * 2];
        // decoder.decode(&data, &mut output, false).unwrap();
        // let probability = vad.predict(output);
        // tracing::info!("silence_count = {}", silence_count);
        // if probability > 0.5 {
        //     state.listen_start = true;
        //     tracing::info!("probability >0.5 = {}", probability);
        //     state.data.push(data.clone());
        //     silence_count = 0;
        // } else {
        //     silence_count += 1;
        //     if silence_count >= silence_max {
        //         if (silence_count == silence_max) {
        //             state.listen_start = false;
        //         }
        //     } else {
        //         state.data.push(data.clone());
        //     }
        // }
        // //write.send(Message::Binary(data)).await;
        //state.data.push(data.clone());
        // let data = TtsMessage::new(None, Some(String::from("hello")));
        // let result: String = serde_json::to_string(&data).unwrap();
        // if write
        //     .send(Message::Text(result.clone().into()))
        //     .await
        //     .is_ok()
        // {
        //     tracing::info!("return text success = {}", result);
        // }
    }
}

#[derive(Debug, Default, Clone)]
pub struct State {
    pub listen_start: bool,
    pub data: Vec<Bytes>,
}

impl State {
    pub fn new() -> Self {
        Self {
            listen_start: false,
            data: vec![],
        }
    }
}
