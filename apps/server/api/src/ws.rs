use std::net::SocketAddr;

use axum::{
    debug_handler,
    extract::{ConnectInfo, WebSocketUpgrade, ws::Message},
    response::IntoResponse,
};
use axum_extra::{TypedHeader, headers};
use framework::middleware::get_auth_layer;
use service::AppState;

use utoipa_axum::{router::OpenApiRouter, routes};
//allows to split the websocket stream into separate TX and RX branches
use futures_util::{Sink, SinkExt, Stream, StreamExt};

const TAG: &str = "ws";

pub fn create_routes(state: AppState) -> OpenApiRouter {
    OpenApiRouter::new()
        .routes(routes!(ws_handler))
        .route_layer(get_auth_layer())
        .with_state(state)
}

#[debug_handler]
#[tracing::instrument(name="ws",skip_all,fields(ip = %addr))]
#[utoipa::path(get,path = "/ws",tag=TAG,security(()))]
async fn ws_handler(
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
