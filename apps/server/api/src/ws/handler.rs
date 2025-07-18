use std::{rc::Rc, sync::Arc};

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

use crate::ws::{listener::Listener, sender::Sender, tts::Tts, vad::Vad};
use futures_util::Sink;

pub struct Handler<W, T, V>
where
    W: Sink<Message> + Unpin + 'static,
    T: Tts + 'static,
    V: Vad + 'static,
{
    session_id: String,
    sender: Arc<Mutex<Sender<W, T>>>,
    state: Arc<Mutex<State>>,
    listener: Listener<W, T, V>,
}

impl<W, T, V> Handler<W, T, V>
where
    W: Sink<Message> + Unpin + Send,
    T: Tts + Send,
    V: Vad + Send,
{
    pub fn new(
        session_id: String,
        sender: Arc<Mutex<Sender<W, T>>>,
        listener: Listener<W, T, V>,
    ) -> Self {
        Self {
            session_id,
            sender,
            state: Arc::new(Mutex::new(State::new())),
            listener,
        }
    }

    pub fn handle_hello(&self, _message: HelloMessage) {
        let session_id = self.session_id.clone();
        let sender = self.sender.clone();
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
        let sender = self.sender.clone();
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
                            match sender.send_tts_with_text(text.clone()).await {
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

    pub fn handle_abort(&self, _message: AbortMessage) {
        let sender = self.sender.clone();
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

    pub fn handle_voice(&mut self, data: Bytes) {
        self.listener.listen(Rc::new(&data));
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
