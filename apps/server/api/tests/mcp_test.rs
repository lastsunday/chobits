use api::setup_mcp;
use rmcp::{
    ServiceExt as _rmcp_ServiceExt,
    model::{CallToolRequestParam, ClientCapabilities, ClientInfo, Implementation},
    transport::{
        StreamableHttpClientTransport, streamable_http_client::StreamableHttpClientTransportConfig,
    },
};
use tracing_test::traced_test;
use utoipa_axum::router::OpenApiRouter;

mod common;
use common::{setup_database, tear_down};

use crate::common::router_client::RouterClient;

#[tokio::test]
#[traced_test]
/// cargo test --test mcp_test -- test_administrator_mcp --nocapture
async fn test_administrator_mcp() -> anyhow::Result<()> {
    let (container, state) = setup_database().await;
    let router = OpenApiRouter::new();
    let ct = tokio_util::sync::CancellationToken::new();
    let router = setup_mcp(router, state.clone(), ct.child_token())
        .split_for_parts()
        .0;
    let config = StreamableHttpClientTransportConfig {
        uri: "/mcp".into(),
        ..Default::default()
    };
    let client = RouterClient { router };
    let transport = StreamableHttpClientTransport::with_client(client, config);
    let client_info = ClientInfo {
        protocol_version: Default::default(),
        capabilities: ClientCapabilities::default(),
        client_info: Implementation {
            name: "test sse client".to_string(),
            title: None,
            version: "0.0.1".to_string(),
            website_url: None,
            icons: None,
        },
    };
    let client = client_info.serve(transport).await.inspect_err(|e| {
        tracing::error!("client error: {:?}", e);
    })?;
    // Initialize
    let server_info = client.peer_info();
    tracing::info!("Connected to server: {server_info:#?}");

    // List tools
    let tools = client.list_tools(Default::default()).await?;
    tracing::info!("Available tools: {tools:#?}");

    let tool_name = "sum";
    let tool_result = client
        .call_tool(CallToolRequestParam {
            name: tool_name.into(),
            arguments: serde_json::json!({
                "a":1,
                "b":2
            })
            .as_object()
            .cloned(),
        })
        .await?;
    tracing::info!("Tool({tool_name}) result: {tool_result:#?}");

    let tool_name = "datetime";
    let tool_result = client
        .call_tool(CallToolRequestParam {
            name: tool_name.into(),
            arguments: None,
        })
        .await?;
    tracing::info!("Tool({tool_name}) result: {tool_result:#?}");

    client.cancel().await?;

    let _ = &state.conn.close().await.unwrap();
    tear_down(&container).await;

    Ok(())
}
