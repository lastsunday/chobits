use std::{collections::HashMap, net::SocketAddr, ops::ControlFlow, rc::Rc};

use axum::{
    RequestPartsExt,
    body::Bytes,
    debug_handler,
    extract::{
        ConnectInfo, FromRequestParts, Path, WebSocketUpgrade,
        ws::{Message, Utf8Bytes},
    },
    http::{HeaderMap, StatusCode, request::Parts},
    response::{IntoResponse, Response},
};
use axum_extra::{TypedHeader, headers};
use framework::{id::gen_id, middleware::get_auth_layer};
use serde_json::Value;
use service::{
    AppState,
    chobits::message::{
        AudioFormat, Transport,
        hello::{self, AudioParam, HelloMessage},
        listen::{ListenMessage, ListenState},
        tts::TtsMessage,
    },
};

use utoipa::ToSchema;
use utoipa_axum::{router::OpenApiRouter, routes};
//allows to split the websocket stream into separate TX and RX branches
use futures_util::{Sink, SinkExt, Stream, StreamExt};

const TAG: &str = "ws";

pub fn create_routes(state: AppState) -> OpenApiRouter {
    OpenApiRouter::new()
        .routes(routes!(ws_handler))
        //.layer(get_auth_layer())
        .with_state(state)
}

#[debug_handler]
#[tracing::instrument(name="ws",skip_all,fields(ip = %addr))]
#[utoipa::path(get,
    path = "/chobits/{version}",
    tag=TAG,
    security(()),
    params(
        ("version" = Version, Path,example="v1", description = "Version"),
        ("Protocol-Version" = String,Header,description="",example="1"),
        ("Device-Id" = String,Header,description="设备的唯一标识符（使用MAC地址或由硬件ID生成的伪MAC地址）",example="11:22:33:44:55:66"),
        ("Client-Id" = String,Header,description="客户端的唯一标识符，由软件自动生成的UUID v4（擦除FLASH或重装后会变化）",example="7b94d69a-9808-4c59-9c9b-704333b38aff"),
    )
)]
async fn ws_handler(
    _version: Version,
    ws: WebSocketUpgrade,
    user_agent: Option<TypedHeader<headers::UserAgent>>,
    headers: HeaderMap,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
) -> impl IntoResponse {
    tracing::info!("user_agent = {:?}", user_agent);
    let state = WebSocketState::new();
    ws.on_upgrade(|socket| {
        let (write, read) = socket.split();
        handle_socket(write, read, state)
    })
}

#[derive(Debug, Default, Clone)]
pub struct WebSocketState {
    pub listen_start: bool,
    pub data: Vec<Bytes>,
}

impl WebSocketState {
    pub fn new() -> Self {
        Self {
            listen_start: false,
            data: vec![],
        }
    }
}

pub async fn handle_socket<W, R>(mut write: W, mut read: R, state: WebSocketState)
where
    W: Sink<Message> + Unpin,
    R: Stream<Item = Result<Message, axum::Error>> + Unpin,
{
    let mut state = state;
    while let Some(Ok(msg)) = read.next().await {
        let result = process_message(msg, &mut write).await;
        if result.is_break() {
            return;
        }
        if result.is_continue() {
            if let Some(item) = result.continue_value() {
                match item {
                    Some(frame) => match frame {
                        Frame::Hello(hello_message) => {
                            if let Some(result) = convert_hello_message_to_command(hello_message) {
                                match result {
                                    HelloCommand::Hello(data) => {
                                        let result: String = serde_json::to_string(&data).unwrap();
                                        if write
                                            .send(Message::Text(result.clone().into()))
                                            .await
                                            .is_ok()
                                        {
                                            tracing::info!("return hello success = {}", result);
                                        }
                                    }
                                }
                            }
                        }
                        Frame::Listen(listen_message) => {
                            if let Some(result) = convert_listen_message_to_command(listen_message)
                            {
                                match result {
                                    ListenCommand::Start => {
                                        state.listen_start = true;
                                    }
                                    ListenCommand::Stop => {
                                        state.listen_start = false;
                                        for item in state.data.clone() {
                                            write.send(Message::Binary(item)).await;
                                        }
                                    }
                                    ListenCommand::Detect(text) => {
                                        // TODO: send stt from text,
                                        // TODO: chatStreamBySentence
                                        let data = TtsMessage::new(None, Some(text));
                                        let result: String = serde_json::to_string(&data).unwrap();
                                        if write
                                            .send(Message::Text(result.clone().into()))
                                            .await
                                            .is_ok()
                                        {
                                            tracing::info!("return detect success = {}", result);
                                        }
                                    }
                                    ListenCommand::Text(text) => {
                                        // TODO: if audio playing, stop audio logic, send tts message stop
                                        // TODO: else send stt from text,
                                        let data = TtsMessage::new(None, Some(text));
                                        let result: String = serde_json::to_string(&data).unwrap();
                                        if write
                                            .send(Message::Text(result.clone().into()))
                                            .await
                                            .is_ok()
                                        {
                                            tracing::info!("return text success = {}", result);
                                        }
                                    }
                                }
                            }
                        }
                        Frame::UnknowText(utf8_bytes) => {
                            tracing::warn!("unknow text = {}", utf8_bytes.to_string())
                        }
                        Frame::Voice(data) => {
                            state.data.push(data.clone());
                        }
                    },
                    None => {
                        //skip
                    }
                }
            }
        }
        tracing::info!(
            "websocket state: listen_start = {}, data length = {}",
            state.listen_start,
            state.data.len()
        );
        if !state.listen_start {
            state.data.clear();
        }
    }
}

fn convert_hello_message_to_command(message: HelloMessage) -> Option<HelloCommand> {
    let result = HelloMessage {
        message: service::chobits::message::Message {
            mtype: String::from(r#"hello"#),
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
        session_id: Some(gen_id()),
    };
    Some(HelloCommand::Hello(result))
}

fn convert_listen_message_to_command(message: ListenMessage) -> Option<ListenCommand> {
    match message.state {
        ListenState::Start => {
            return Some(ListenCommand::Start);
        }
        ListenState::Stop => {
            return Some(ListenCommand::Stop);
        }
        ListenState::Detect => {
            if let Some(text) = message.text {
                return Some(ListenCommand::Detect(text));
            }
        }
        ListenState::Text => {
            if let Some(text) = message.text {
                return Some(ListenCommand::Text(text));
            }
        }
    }
    None
}

#[derive(Debug)]
pub enum HelloCommand {
    Hello(HelloMessage),
}

#[derive(Debug)]
pub enum ListenCommand {
    Start,
    Stop,
    Detect(String),
    Text(String),
}

/// helper to print contents of messages to stdout. Has special treatment for Close.
async fn process_message<W>(msg: Message, write: &mut W) -> ControlFlow<(), Option<Frame>>
where
    W: Sink<Message> + Unpin,
{
    match msg {
        Message::Text(t) => match serde_json::from_slice::<Value>(t.as_bytes()) {
            Ok(json) => {
                tracing::info!("is object = {},data = {}", json.is_object(), json);
                if json.is_object() {
                    match json.get("type") {
                        Some(mtype) => {
                            tracing::info!("mtype = {:?}", mtype);
                            if mtype == r#"hello"# {
                                match serde_json::from_slice::<HelloMessage>(t.as_bytes()) {
                                    Ok(message) => {
                                        return ControlFlow::Continue(Some(Frame::Hello(message)));
                                    }
                                    Err(_) => {
                                        return ControlFlow::Continue(Some(Frame::UnknowText(t)));
                                    }
                                }
                            } else if mtype == r#"listen"# {
                                match serde_json::from_slice::<ListenMessage>(t.as_bytes()) {
                                    Ok(message) => {
                                        return ControlFlow::Continue(Some(Frame::Listen(message)));
                                    }
                                    Err(_) => {
                                        return ControlFlow::Continue(Some(Frame::UnknowText(t)));
                                    }
                                }
                            }
                        }
                        None => {
                            tracing::info!("can't find type field");
                        }
                    }
                } else if json.is_number() {
                    if write
                        .send(Message::Text(String::from(t.as_str()).into()))
                        .await
                        .is_ok()
                    {
                        tracing::info!("return hello number success");
                    } else {
                        tracing::info!("return hello number failure");
                    }
                } else {
                    tracing::info!("unknow json message = {}", json)
                }
            }
            Err(_) => {
                return ControlFlow::Continue(Some(Frame::UnknowText(t)));
            }
        },
        Message::Binary(d) => {
            return ControlFlow::Continue(Some(Frame::Voice(d)));
        }
        Message::Close(c) => {
            if let Some(cf) = c {
                println!(
                    ">>>  sent close with code {} and reason `{}`",
                    cf.code, cf.reason
                );
            } else {
                println!(">>>  somehow sent close message without CloseFrame");
            }
            return ControlFlow::Break(());
        }

        Message::Pong(v) => {
            println!(">>>  sent pong with {v:?}");
        }
        // You should never need to manually handle Message::Ping, as axum's websocket library
        // will do so for you automagically by replying with Pong and copying the v according to
        // spec. But if you need the contents of the pings you can see them here.
        Message::Ping(v) => {
            println!(">>>  sent ping with {v:?}");
        }
    }
    ControlFlow::Continue(None)
}

#[derive(Debug)]
pub enum Frame {
    Hello(HelloMessage),
    Listen(ListenMessage),
    UnknowText(Utf8Bytes),
    Voice(Bytes),
}

#[derive(Debug, PartialEq, Eq, ToSchema)]
enum Version {
    V1,
}

impl<S> FromRequestParts<S> for Version
where
    S: Send + Sync,
{
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let params: Path<HashMap<String, String>> =
            parts.extract().await.map_err(IntoResponse::into_response)?;

        let version = params
            .get("version")
            .ok_or_else(|| (StatusCode::NOT_FOUND, "version param missing").into_response())?;

        match version.as_str() {
            "v1" => Ok(Version::V1),
            _ => Err((StatusCode::NOT_FOUND, "unknown version").into_response()),
        }
    }
}
