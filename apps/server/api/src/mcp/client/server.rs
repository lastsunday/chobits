use async_trait::async_trait;
use rig::{
    OneOrMany,
    agent::Text,
    completion::ToolDefinition,
    message::{ToolCall, ToolResult, ToolResultContent},
};
use rmcp::{
    RoleClient, ServiceExt,
    model::{
        CallToolRequestParams, ClientCapabilities, ClientInfo, Implementation,
        InitializeRequestParams, PaginatedRequestParams,
    },
    service::RunningService,
    transport::IntoTransport,
};

use crate::mcp::client::McpClient;

pub struct ServerMcpClient {
    client: RunningService<RoleClient, InitializeRequestParams>,
    tools: Vec<ToolDefinition>,
}

impl ServerMcpClient {
    pub async fn new<T, E, A>(transport: T) -> anyhow::Result<Self>
    where
        T: IntoTransport<RoleClient, E, A>,
        E: std::error::Error + Send + Sync + 'static,
    {
        let client_info = ClientInfo {
            meta: None,
            protocol_version: Default::default(),
            capabilities: ClientCapabilities::default(),
            client_info: Implementation {
                name: "Server mcp client".to_string(),
                title: None,
                version: "0.0.1".to_string(),
                website_url: None,
                icons: None,
            },
        };
        let client = client_info.serve(transport).await?;
        Ok(Self {
            client,
            tools: vec![],
        })
    }

    pub async fn init(&mut self) -> anyhow::Result<()> {
        let mut cursor = None;
        loop {
            // List tools
            let tools_result = self
                .client
                .list_tools(Some(PaginatedRequestParams { meta: None, cursor }))
                .await?;
            for tool in tools_result.tools {
                self.tools.push(ToolDefinition {
                    name: tool.name.to_string(),
                    description: tool.description.unwrap_or_default().to_string(),
                    parameters: serde_json::to_value(tool.input_schema)?,
                });
            }
            if let Some(next_cursor) = tools_result.next_cursor {
                cursor = Some(next_cursor);
            } else {
                break;
            }
        }
        Ok(())
    }
}

#[async_trait]
impl McpClient for ServerMcpClient {
    async fn get_tool(&self) -> anyhow::Result<Vec<ToolDefinition>> {
        Ok(self.tools.clone())
    }

    async fn call_tool(&self, param: ToolCall) -> anyhow::Result<ToolResult> {
        let id = param.id.clone();
        let call_id = param.call_id.clone();
        let function = &param.function;
        let function_json_text = serde_json::to_string(function)?;
        let request: CallToolRequestParams = serde_json::from_str(function_json_text.as_str())?;
        let response = self.client.call_tool(request).await?;

        let content = &response.content;
        match &content.len() {
            0 => Err(anyhow::anyhow!("call tool result must be not empty")),
            _ => {
                // TODO: multiple result handle?
                let item = content.first().unwrap();
                match &item.raw {
                    rmcp::model::RawContent::Text(raw_text_content) => Ok(ToolResult {
                        id,
                        call_id,
                        content: OneOrMany::one(ToolResultContent::Text(Text {
                            text: raw_text_content.text.clone(),
                        })),
                    }),
                    rmcp::model::RawContent::Image(..) => {
                        // TODO:
                        Err(anyhow::anyhow!("tool call image result not supported yet"))
                    }
                    rmcp::model::RawContent::Resource(..) => {
                        // TODO:
                        Err(anyhow::anyhow!(
                            "tool call resource result not supported yet"
                        ))
                    }
                    rmcp::model::RawContent::Audio(..) => {
                        // TODO:
                        Err(anyhow::anyhow!("tool call audio result not supported yet"))
                    }
                    rmcp::model::RawContent::ResourceLink(..) => {
                        // TODO:
                        Err(anyhow::anyhow!(
                            "tool call resource link result not supported yet"
                        ))
                    }
                }
            }
        }
    }
}
