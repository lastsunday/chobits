use crate::{
    config,
    ws::{
        asr::Asr,
        listener::Listener,
        llm::{Llm, LlmQwen},
        sender::Sender,
        state::State,
        tts::Tts,
        vad::Vad,
    },
};
use axum::{body::Bytes, extract::ws::Message};
use chrono::Local;
use futures_util::Sink;
use service::chobits::message::{
    AudioFormat, Transport,
    abort::AbortMessage,
    hello::{AudioParam, HelloMessage},
    listen::{ListenMessage, ListenMode, ListenState},
    stt::SttMessage,
    tts::{TtsMessage, TtsState},
};
use std::{rc::Rc, sync::Arc};
use tokio::sync::Mutex;

pub struct Handler<W, T, V, A>
where
    W: Sink<Message> + Unpin + 'static,
    T: Tts + 'static,
    V: Vad + 'static,
    A: Asr + 'static,
{
    session_id: String,
    sender: Arc<Mutex<Sender<W, T>>>,
    state: Arc<Mutex<State>>,
    listener: Arc<Mutex<Listener<V, A>>>,
    llm: Arc<Mutex<Box<LlmQwen>>>,
}

impl<W, T, V, A> Handler<W, T, V, A>
where
    W: Sink<Message> + Unpin + Send,
    T: Tts + Send,
    V: Vad + Send,
    A: Asr + Send,
{
    pub fn new(
        session_id: String,
        sender: Arc<Mutex<Sender<W, T>>>,
        listener: Arc<Mutex<Listener<V, A>>>,
        state: Arc<Mutex<State>>,
        llm: Arc<Mutex<Box<LlmQwen>>>,
    ) -> Self {
        Self {
            session_id,
            sender,
            state,
            listener,
            llm,
        }
    }

    pub fn handle_hello(&self, _message: HelloMessage) {
        let session_id = self.session_id.clone();
        let sender = self.sender.clone();
        tokio::spawn(async move {
            let audio_config = config::get().audio();
            let data = HelloMessage {
                message: service::chobits::message::Message {
                    mtype: service::chobits::message::Type::Hello,
                },
                transport: Some(Transport::Websocket),
                audio_params: Some(AudioParam {
                    format: AudioFormat::Opus,
                    sample_rate: audio_config.output_sample_rate(),
                    channels: audio_config.output_channel(),
                    frame_duration: audio_config.output_frame_duration(),
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
        let listener = self.listener.clone();
        let llm = self.llm.clone();

        tokio::spawn(async move {
            match message.state {
                ListenState::Start => {
                    let mut listener = listener.lock().await;
                    listener.reset(message.mmod).await;
                }
                ListenState::Stop => {
                    let mut listener = listener.lock().await;
                    let result = listener.get_result().await;
                    Self::handle_listener_result(result, session_id, sender, llm).await;
                    listener.clear().await;
                }
                ListenState::Detect => {
                    // TODO: send stt from text,
                    // TODO: chatStreamBySentence
                    match message.text {
                        Some(text) => {
                            tracing::info!("listen detect text = {}", text);
                            let mut state = state.lock().await;
                            state.client_speaking = false;
                            drop(state);
                            Self::handle_listener_result(
                                Some(text.clone()),
                                session_id,
                                sender,
                                llm,
                            )
                            .await;
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

    pub fn handle_voice(&self, data: Bytes) {
        let session_id = self.session_id.clone();
        let state = self.state.clone();
        let listener = self.listener.clone();
        let sender = self.sender.clone();
        let llm = self.llm.clone();
        tokio::spawn(async move {
            let mut listener = listener.lock().await;
            listener.listen(Rc::new(&data));
            let state = state.lock().await;
            let client_speaking = state.client_speaking;
            let last_activity_time = state.last_activity_time;
            let last_speaking_time = state.last_speaking_time;
            drop(state);
            if !client_speaking {
                if let Some(listen_mode) = listener.get_listen_mode() {
                    match listen_mode {
                        ListenMode::Auto | ListenMode::RealTime => match last_speaking_time {
                            Some(last_speaking_time) => {
                                let logic_config = config::get().logic();
                                let offset_time =
                                    Local::now().timestamp_millis() - last_speaking_time;
                                let silence_voice_timeout = logic_config.silence_voice_timeout();
                                if offset_time >= silence_voice_timeout {
                                    tracing::info!(
                                        "offset_time = {} >= silence voice timeout = {}",
                                        offset_time,
                                        silence_voice_timeout,
                                    );
                                    let result = listener.get_result().await;
                                    Self::handle_listener_result(
                                        result,
                                        session_id,
                                        sender.clone(),
                                        llm,
                                    )
                                    .await;
                                    listener.clear().await;
                                }
                            }
                            None => (),
                        },
                        ListenMode::Manual => {}
                    }
                }
            }
            match last_activity_time {
                Some(last_activity_time) => {
                    let logic_config = config::get().logic();
                    let close_connection_no_voice_time =
                        logic_config.close_connection_no_voice_time();
                    let offset_time = Local::now().timestamp_millis() - last_activity_time;
                    //tracing::info!("last_activity_time offset_time = {}", offset_time);
                    if offset_time >= close_connection_no_voice_time {
                        tracing::info!(
                            "close connection no voice time, offset_time = {}",
                            offset_time
                        );
                        let sender = sender.clone();
                        let mut sender = sender.lock().await;
                        sender.close().await;
                        return;
                    }
                }
                None => (),
            }
        });
    }

    async fn handle_listener_result(
        result: Option<String>,
        session_id: String,
        sender: Arc<Mutex<Sender<W, T>>>,
        llm: Arc<Mutex<Box<LlmQwen>>>,
    ) {
        match result {
            Some(text) => {
                tracing::info!("listen result = {}", text);
                let data = SttMessage::new(Some(session_id), Some(text.clone()));
                let mut sender = sender.lock().await;
                match sender.send_json_text(&data).await {
                    Ok(_) => (),
                    Err(error) => {
                        tracing::info!("send tts message error {}", error);
                    }
                }
                let logic_config = config::get().logic();
                let llm = llm.lock().await;
                let output = llm.chat(logic_config.system_prompt().to_string(), text);
                if let Err(e) = sender.send_tts_with_text_stream(output).await {
                    tracing::info!("send tts message error {}", e);
                }
            }
            None => {
                tracing::info!("listen result is none");
            }
        }
    }
}
