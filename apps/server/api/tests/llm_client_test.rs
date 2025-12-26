use api::{
    llm::{
        LlmFactory,
        client::{self, ChatRequest, ClientBuilder},
    },
    mcp::{
        client::server::ServerMcpClient,
        mcp_host::{McpHost, UnionMcpHost},
    },
    setup_mcp,
};
use rig::{
    OneOrMany,
    message::{AssistantContent, Message, Text, UserContent},
};
use rmcp::transport::{
    StreamableHttpClientTransport, streamable_http_client::StreamableHttpClientTransportConfig,
};
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_stream::StreamExt;
use tracing::{error, info};
use tracing_test::traced_test;
use utoipa_axum::router::OpenApiRouter;
mod common;
use common::{setup_database, tear_down};

use crate::common::router_client::RouterClient;

#[tokio::test]
#[traced_test]
#[ignore]
/// cargo test --test llm_client_test -- test_chat_simple --ignored --nocapture
async fn test_chat_simple() {
    let model = LlmFactory::create_model();
    let system_prompt = "你是一个助手，所有回答必须使用纯文本自然语言，禁止使用任何Markdown符号如#、-、*等并且数字使用中文字代替。".to_string();
    let client = ClientBuilder::new()
        .with_model(Arc::new(model))
        .build()
        .with_preamble(Some(system_prompt));
    let request = ChatRequest {
        message: Message::User {
            content: OneOrMany::<UserContent>::one(UserContent::Text(Text {
                text: r#"静夜思的内容"#.to_string(),
            })),
        },
    };
    let mut output = client.chat(request);
    let mut result = Vec::new();
    while let Some(text) = output.next().await {
        match text {
            Ok(text) => {
                result.push(text);
            }
            Err(e) => {
                error!("{}", e.to_string());
            }
        }
    }
    let result: String = result.into_iter().collect();
    info!("{}", result);
    assert_ne!(0, result.len());
}

#[tokio::test]
#[traced_test]
#[ignore]
/// cargo test --test llm_client_test -- test_short_question --ignored --nocapture
async fn test_short_question() {
    let model = LlmFactory::create_model();
    let system_prompt = "你是一个助手。".to_string();
    let client = ClientBuilder::new()
        .with_model(Arc::new(model))
        .build()
        .with_preamble(Some(system_prompt));
    let request = ChatRequest {
        message: Message::User {
            content: OneOrMany::<UserContent>::one(UserContent::Text(Text {
                text: r#"1+1="#.to_string(),
            })),
        },
    };
    let mut output = client.chat(request);
    let mut result = Vec::new();
    while let Some(text) = output.next().await {
        match text {
            Ok(text) => {
                result.push(text);
            }
            Err(e) => {
                error!("{}", e.to_string());
            }
        }
    }
    let result: String = result.into_iter().collect();
    info!("{}", result);
    assert_ne!(0, result.len());
}

#[tokio::test]
#[traced_test]
#[ignore]
/// cargo test --test llm_client_test -- test_chat_history --ignored --nocapture
async fn test_chat_history() {
    let model = LlmFactory::create_model();
    let system_prompt = "你是一个助手，协助用户进行记录，查询和提供建议，所有回答必须使用纯文本自然语言，禁止使用任何Markdown符号如#、-、*等并且数字使用中文字代替。".to_string();
    let client = ClientBuilder::new()
        .with_model(Arc::new(model))
        .build()
        .with_preamble(Some(system_prompt))
        .with_chat_history(Some(vec![
            Message::User {
                content: OneOrMany::<UserContent>::one(UserContent::Text(Text {
                    text: r#"记录一下，小小的电话号码为12349876"#.to_string(),
                })),
            },
            Message::Assistant {
                id: None,
                content: OneOrMany::<AssistantContent>::one(AssistantContent::Text(Text {
                    text: r#"小小电话号码为12349876"#.to_string(),
                })),
            },
        ]));
    let request = ChatRequest {
        message: Message::User {
            content: OneOrMany::<UserContent>::one(UserContent::Text(Text {
                text: r#"告诉我小小的电话号码"#.to_string(),
            })),
        },
    };
    let mut output = client.chat(request);
    let mut result = Vec::new();
    while let Some(text) = output.next().await {
        match text {
            Ok(text) => {
                result.push(text);
            }
            Err(e) => {
                error!("{}", e.to_string());
            }
        }
    }
    let result: String = result.into_iter().collect();
    assert_ne!(0, result.len());
    assert!(result.contains("12349876"));
    info!("{}", result);
}

#[tokio::test]
#[traced_test]
#[ignore]
/// cargo test --test llm_client_test -- test_chat_mcp --ignored --nocapture
async fn test_chat_mcp() -> anyhow::Result<()> {
    let model = LlmFactory::create_model();
    let mut union_mcp_host = UnionMcpHost::new(None);
    // TODO: sever client
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

    let mut server_client = ServerMcpClient::new(transport).await?;
    server_client.init().await?;
    union_mcp_host.add_client(Box::new(server_client)).await;

    let system_prompt = "".to_string();
    let client = client::ClientBuilder::new()
        .with_model(Arc::new(model))
        .with_mcp_host(Arc::new(Mutex::new(union_mcp_host)))
        .build()
        .with_preamble(Some(system_prompt));
    // let request = ChatRequest {
    //     message: Message::User {
    //         content: OneOrMany::<UserContent>::one(UserContent::Text(Text {
    //             text: r#"Calculate the sum of 24.5 and 17.3 using the calculator service"#
    //                 .to_string(),
    //         })),
    //     },
    // };
    let request = ChatRequest {
        message: Message::User {
            content: OneOrMany::<UserContent>::one(UserContent::Text(Text {
                // text: r#"What time is it now"#.to_string(),
                text: r#"现在几点"#.to_string(),
            })),
        },
    };
    let mut output = client.chat(request);
    let mut result = Vec::new();
    while let Some(text) = output.next().await {
        match text {
            Ok(text) => {
                result.push(text);
            }
            Err(e) => {
                error!("{}", e.to_string());
            }
        }
    }
    let result: String = result.into_iter().collect();
    assert_ne!(0, result.len());
    info!("{}", result);

    let _ = &state.conn.close().await.unwrap();
    tear_down(&container).await;
    Ok(())
}
