use std::{collections::HashMap, net::SocketAddr};

use axum::{
    RequestPartsExt, debug_handler,
    extract::{ConnectInfo, FromRequestParts, Path, WebSocketUpgrade, ws::Message},
    http::{StatusCode, request::Parts},
    response::{IntoResponse, Response},
};
use axum_extra::{TypedHeader, headers};
use framework::middleware::get_auth_layer;
use service::AppState;

use utoipa::ToSchema;
use utoipa_axum::{router::OpenApiRouter, routes};
//allows to split the websocket stream into separate TX and RX branches
use futures_util::{Sink, SinkExt, Stream, StreamExt};

const TAG: &str = "ws";

pub fn create_routes(state: AppState) -> OpenApiRouter {
    OpenApiRouter::new()
        .routes(routes!(ws_handler))
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
    )
)]
async fn ws_handler(
    _version: Version,
    ws: WebSocketUpgrade,
    user_agent: Option<TypedHeader<headers::UserAgent>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| {
        let (write, read) = socket.split();
        handle_socket(write, read)
    })
}

pub async fn handle_socket<W, R>(mut write: W, mut read: R)
where
    W: Sink<Message> + Unpin,
    R: Stream<Item = Result<Message, axum::Error>> + Unpin,
{
    while let Some(Ok(msg)) = read.next().await {
        if let Message::Text(msg) = msg {
            if write
                .send(Message::Text(format!("You said: {msg}").into()))
                .await
                .is_err()
            {
                break;
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
