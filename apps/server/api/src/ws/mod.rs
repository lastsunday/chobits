pub mod asr;
pub mod asr_cache;
pub mod frame;
pub mod handler;
pub mod listener;
pub mod message_converter;
pub mod sender;
pub mod tts;
pub mod tts_cache;
pub mod util;
pub mod vad;
pub mod vad_cache;

use super::ws::sender::Sender;
use crate::{
    AppState,
    ws::{
        asr_cache::AsrCache, frame::Frame, handler::Handler, listener::Listener,
        message_converter::convert_to_frame, tts_cache::TtsCache, vad_cache::VadCache,
    },
};
use axum::{
    RequestPartsExt, debug_handler,
    extract::{ConnectInfo, FromRequestParts, Path, WebSocketUpgrade, ws::Message},
    http::{HeaderMap, StatusCode, request::Parts},
    response::{IntoResponse, Response},
};
use axum_extra::{TypedHeader, headers};
use framework::id::gen_id;
use futures_util::{Sink, Stream, StreamExt};
use std::{collections::HashMap, net::SocketAddr, sync::Arc};
use tokio::sync::Mutex;
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

pub async fn handle_socket<W, R>(write: W, mut read: R)
where
    W: Sink<Message> + Unpin + Send + 'static,
    R: Stream<Item = Result<Message, axum::Error>> + Unpin,
{
    let session_id = gen_id();
    let tts = TtsCache::global().instance.clone();
    let sender = Sender::new(Box::new(write), tts);
    let sender = Arc::new(Mutex::new(sender));
    let vad = VadCache::create_vad();
    let vad = Arc::new(Mutex::new(vad));
    let asr = AsrCache::global().instance.clone();
    let asr = Arc::new(Mutex::new(asr));
    let listener = Listener::new(session_id.clone(), sender.clone(), vad, asr.clone());
    let mut handler = Handler::new(session_id, sender.clone(), listener);
    while let Some(Ok(msg)) = read.next().await {
        let result = convert_to_frame(msg).await;
        if result.is_break() {
            if let Some(item) = result.break_value() {
                if let Some(frame) = item {
                    match frame {
                        Frame::Close(message) => {
                            tracing::info!(
                                "close code = {},reason = {}",
                                message.code,
                                message.reason
                            );
                        }
                        _ => {}
                    }
                }
            }
            return;
        }
        if result.is_continue() {
            if let Some(item) = result.continue_value() {
                match item {
                    Some(frame) => match frame {
                        Frame::Hello(message) => {
                            handler.handle_hello(message);
                        }
                        Frame::Listen(message) => {
                            handler.handle_listen(message);
                        }
                        Frame::UnknowText(utf8_bytes) => {
                            tracing::warn!("unknow text = {}", utf8_bytes.to_string())
                        }
                        Frame::Voice(data) => {
                            handler.handle_voice(data);
                        }
                        Frame::Abort(message) => {
                            handler.handle_abort(message);
                        }
                        _ => {}
                    },
                    None => {
                        tracing::info!("unkonw message");
                    }
                }
            }
        }
    }
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
