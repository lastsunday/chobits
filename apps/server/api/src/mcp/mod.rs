use rmcp::transport::streamable_http_server::{
    StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
};
use tokio_util::sync::CancellationToken;
use utoipa_axum::router::OpenApiRouter;

use crate::{AppState, mcp::tool::calculator::Calculator};

pub mod device;
pub mod mcp_host;
pub mod tool;

pub fn create_routes(state: AppState, cancellation_token: CancellationToken) -> OpenApiRouter {
    let service = StreamableHttpService::new(
        || Ok(Calculator::new()),
        LocalSessionManager::default().into(),
        StreamableHttpServerConfig {
            cancellation_token,
            ..Default::default()
        },
    );

    OpenApiRouter::new()
        .nest_service("/mcp", service)
        //.layer(get_auth_layer())
        .with_state(state)
}
