pub mod frame;
pub mod message_converter;
pub mod session;

use crate::{
    AppState,
    asr::AsrFactory,
    config::{audio::AudioConfig, mcp::McpConfig, session::SessionConfig, vad::VadConfig},
    llm::LlmFactory,
    mcp::{
        client::server::ServerMcpClient,
        mcp_host::{McpHost, UnionMcpHost},
    },
    tts::TtsFactory,
    vad::VadFactory,
    ws::{frame::FrameResult, session::Session},
};

use axum::{
    RequestPartsExt, debug_handler,
    extract::{ConnectInfo, FromRequestParts, Path, State, WebSocketUpgrade, ws::Message},
    http::{HeaderMap, StatusCode, request::Parts},
    response::{IntoResponse, Response},
};
use axum_extra::{TypedHeader, headers};
use framework::error::AppError;
use framework::id::gen_id;
use framework::prelude::error as error_code;
use futures_util::{Sink, SinkExt, Stream, StreamExt};
use message_converter::convert_to_frame;
use rmcp::transport::{
    StreamableHttpClientTransport, streamable_http_client::StreamableHttpClientTransportConfig,
};
use serde::Serialize;
use session::{SessionBuilder, listener::DefaultListener};
use std::{collections::HashMap, net::SocketAddr, sync::Arc};
use tokio::sync::Mutex;
use tracing::{Instrument, Level, debug, error, info, span, trace};
use utoipa::ToSchema;
use utoipa_axum::{router::OpenApiRouter, routes};

#[derive(Serialize)]
struct ErrorFrame {
    #[serde(rename = "type")]
    mtype: &'static str,
    code: u32,
    message: String,
}

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
    State(AppState {
        session_config,
        mcp_config,
        vad_config,
        audio_config,
        ..
    }): State<AppState>,
) -> impl IntoResponse {
    info!("user_agent = {:?}", user_agent);
    ws.on_upgrade(move |socket| {
        let session_id = gen_id();
        let (write, read) = socket.split();
        handle_socket(
            session_id,
            session_config,
            mcp_config,
            vad_config,
            audio_config,
            write,
            read,
        )
    })
}

pub async fn handle_socket<W, R>(
    session_id: String,
    session_config: Arc<SessionConfig>,
    mcp_config: Arc<McpConfig>,
    vad_config: Arc<VadConfig>,
    audio_config: Arc<AudioConfig>,
    mut write: W,
    read: R,
) where
    W: Sink<Message> + Unpin + Send + 'static,
    R: Stream<Item = Result<Message, axum::Error>> + Unpin + Send + 'static,
{
    let span = span!(Level::DEBUG, "socket", id=%session_id);
    let _guard = span.enter();
    let mut session = SessionBuilder::new()
        .with_id(session_id.clone())
        .with_listener(Box::new(DefaultListener::new(
            Arc::new(Mutex::new(VadFactory::create_model(&vad_config))),
            AsrFactory::global().default().clone(),
            audio_config.clone(),
        )))
        .with_model(LlmFactory::global().default())
        .with_tts(TtsFactory::global().default())
        .with_mcp_host(Arc::new(Mutex::new(
            create_mcp_host(session_id.clone(), mcp_config.clone()).await,
        )))
        .with_config(session_config.clone())
        .with_audio_config(audio_config.clone())
        .build();
    if let Err(e) = session.start().instrument(span.clone()).await {
        error!("{}", e);
        let result = write.close().await;
        if result.is_err() {
            info!("write close failure");
        }
        return;
    }
    let session_id_clone = session_id.clone();
    let output = session.output_frame().await;
    tokio::spawn(async move {
        let span = span!(parent:None,Level::DEBUG, "socket", id=%session_id_clone);
        on_send(output, write).instrument(span).await
    });
    tokio::spawn(async move {
        let span = span!(parent:None,Level::DEBUG, "socket", id=%session_id);
        on_recv(session, read).instrument(span).await
    });
}

async fn on_recv<R>(mut session: Session, mut read: R)
where
    R: Stream<Item = Result<Message, axum::Error>> + Unpin + Send + 'static,
{
    while let Some(Ok(msg)) = read.next().await {
        let result = convert_to_frame(&msg).await;
        if result.is_break() {
            if let Some(item) = result.break_value() {
                match item {
                    Some(frame) => session.accept_frame(&frame).await,
                    None => info!("break value none"),
                }
            }
            session.stop().await;
            return;
        }
        if result.is_continue()
            && let Some(item) = result.continue_value()
            && let Some(frame) = item
        {
            session.accept_frame(&frame).await
        } else {
            info!("unkonw continue message");
        }
    }
}

async fn on_send<W>(
    mut output: impl Stream<Item = Result<FrameResult, AppError>> + Unpin + Send + 'static,
    mut write: W,
) where
    W: Sink<Message> + Unpin + Send + 'static,
{
    while let Some(data) = output.next().await {
        match data {
            Ok(frame) => {
                match &frame {
                    frame::FrameResult::AudioResult(_audio_message) => {
                        trace!(target:"frame","[SEND] Audio");
                    }
                    _ => {
                        debug!(target:"frame","[SEND] {:?}", frame);
                    }
                }
                match frame {
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
                }
            }
            Err(api_err) => {
                api_err.log();
                let AppError::App { code, message, .. } = &api_err;
                send_text(
                    &mut write,
                    &ErrorFrame {
                        mtype: "error",
                        code: *code,
                        message: message.clone(),
                    },
                )
                .await;
            }
        }
    }
    let result = write.close().await;
    if result.is_err() {
        info!("write close failure");
    }
}

pub async fn send_text<W, T>(write: &mut W, value: &T) -> bool
where
    W: Sink<Message> + Unpin + Send + 'static,
    T: ?Sized + Serialize,
{
    let result: String = serde_json::to_string(value).expect("value to json failure");
    write.send(Message::Text(result.into())).await.is_err()
}

async fn create_server_mcp_client(uri: String) -> anyhow::Result<ServerMcpClient> {
    let config = StreamableHttpClientTransportConfig::with_uri(uri);
    let transport = StreamableHttpClientTransport::from_config(config);
    let mut server_mcp_client = ServerMcpClient::new(transport).await?;
    server_mcp_client.init().await?;
    Ok(server_mcp_client)
}

async fn create_mcp_host(session_id: String, mcp_config: Arc<McpConfig>) -> UnionMcpHost {
    let mut mcp_host = UnionMcpHost::new(Some(session_id));
    let uri_list = &mcp_config.uri_list;
    if let Some(uri_list) = uri_list {
        for uri in uri_list {
            let server_mcp_client = create_server_mcp_client(uri.to_string()).await;
            match server_mcp_client {
                Ok(server_mcp_client) => {
                    mcp_host.add_client(Box::new(server_mcp_client)).await;
                }
                Err(e) => {
                    error!("{:?}", e);
                }
            }
        }
    }
    mcp_host
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

#[error_code]
pub enum WsErrorCode {
    TtsEncode = 504001,
    TtsText = 504002,
    AsrFailure = 504003,
    LlmFailure = 504004,
    InternalError = 504005,
}
