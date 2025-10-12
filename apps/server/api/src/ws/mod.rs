pub mod asr;
pub mod common;
pub mod frame;
pub mod llm;
pub mod mcp;
pub mod message_converter;
pub mod session;
pub mod tts;
pub mod util;
pub mod vad;

use crate::{
    AppState, config,
    ws::{
        asr::asr_cache::AsrCache,
        message_converter::convert_to_frame,
        session::{Session, listener::DefaultListener},
        vad::vad_cache::VadCache,
    },
};
use axum::{
    RequestPartsExt, debug_handler,
    extract::{ConnectInfo, FromRequestParts, Path, WebSocketUpgrade, ws::Message},
    http::{HeaderMap, StatusCode, request::Parts},
    response::{IntoResponse, Response},
};
use axum_extra::{TypedHeader, headers};
use futures_util::{Sink, SinkExt, Stream, StreamExt};
use serde::Serialize;
use std::{collections::HashMap, net::SocketAddr, sync::Arc};
use tokio::sync::Mutex;
use tracing::{error, info};
use utoipa::ToSchema;
use utoipa_axum::{router::OpenApiRouter, routes};

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
    _headers: HeaderMap,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
) -> impl IntoResponse {
    tracing::info!("user_agent = {:?}", user_agent);
    ws.on_upgrade(|socket| {
        let (write, read) = socket.split();
        handle_socket(write, read)
    })
}

pub async fn send_text<W, T>(write: &mut W, value: &T) -> bool
where
    W: Sink<Message> + Unpin + Send + 'static,
    T: ?Sized + Serialize,
{
    let result: String = serde_json::to_string(value).expect("value to json failure");
    write.send(Message::Text(result.into())).await.is_err()
}

pub async fn handle_socket<W, R>(mut write: W, mut read: R)
where
    W: Sink<Message> + Unpin + Send + 'static,
    R: Stream<Item = Result<Message, axum::Error>> + Unpin + Send + 'static,
{
    let vad = Arc::new(Mutex::new(VadCache::create_vad()));
    let asr = Arc::new(Mutex::new(AsrCache::global().instance.clone()));
    let close_connection_no_voice_time = config::get().logic().close_connection_no_voice_time();
    let mut session = Session::new(
        Box::new(DefaultListener::new(vad, asr.clone())),
        Some(close_connection_no_voice_time),
    );
    let mut output = session.output_frame().await;
    tokio::spawn(async move {
        while let Some(data) = output.next().await {
            info!("{:?}", data);
            match data {
                Ok(frame_result) => match frame_result {
                    frame::FrameResult::HelloResult(message) => {
                        if send_text(&mut write, &message).await {
                            info!("send hello data failure");
                            break;
                        }
                    }
                    frame::FrameResult::STTResult(message) => {
                        if send_text(&mut write, &message).await {
                            info!("send stt data failure");
                            break;
                        }
                    }
                    frame::FrameResult::LLMResult(message) => {
                        if send_text(&mut write, &message).await {
                            info!("send llm data failure");
                            break;
                        }
                    }
                    frame::FrameResult::TTSResult(message) => {
                        if send_text(&mut write, &message).await {
                            info!("send tts data failure");
                            break;
                        }
                    }
                    frame::FrameResult::McpResult(message) => {
                        if send_text(&mut write, &message).await {
                            info!("send mcp request data failure");
                            break;
                        }
                    }
                    frame::FrameResult::AudioResult(audio_message) => {
                        let data = audio_message.data;
                        if write.send(Message::Binary(data.into())).await.is_err() {
                            info!("send audio data failure");
                            break;
                        }
                    }
                    frame::FrameResult::CloseResult => {
                        let result = write.close().await;
                        if result.is_err() {
                            info!("write close failure");
                            break;
                        }
                    }
                },
                Err(e) => {
                    error!("{:?}", e);
                    return;
                }
            }
        }
        let result = write.close().await;
        if result.is_err() {
            info!("write close failure");
        }
    });
    tokio::spawn(async move {
        while let Some(Ok(msg)) = read.next().await {
            let result = convert_to_frame(&msg).await;
            if result.is_break() {
                if let Some(item) = result.break_value() {
                    match item {
                        Some(frame) => match frame {
                            frame::Frame::Close(close_message) => {
                                info!("break value close message = {:?}", close_message);
                                session.stop().await;
                                return;
                            }
                            _ => {
                                session.accept_frame(&frame).await;
                            }
                        },
                        None => {
                            info!("break value none");
                            session.stop().await;
                            return;
                        }
                    }
                }
                return;
            }
            if result.is_continue()
                && let Some(item) = result.continue_value()
            {
                match item {
                    Some(frame) => {
                        match frame {
                            frame::Frame::Abort(abort_message) => {
                                info!("abort message = {:?}", abort_message);
                                session.stop().await;
                            }
                            frame::Frame::Ping { data } => {
                                // TODO: log session id
                                info!("ping,len = {}", data.len());
                            }
                            frame::Frame::Pong { data } => {
                                // TODO: log session id
                                info!("pong,len = {}", data.len());
                            }
                            _ => {
                                session.accept_frame(&frame).await;
                            }
                        }
                    }
                    None => {
                        info!("unkonw continue message");
                    }
                }
            }
        }
    });
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
