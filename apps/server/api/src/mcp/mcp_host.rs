use std::collections::HashMap;

use anyhow::Context;
use async_trait::async_trait;
use rig::{
    completion::ToolDefinition,
    message::{ToolCall, ToolResult},
};
use rmcp::model::{JsonRpcMessage, Tool};
use service::chobits::message::mcp::McpRequest;

use crate::mcp::client::{
    McpClient,
    device::{DeviceMcpClient, DeviceMcpPhase},
};
use std::sync::Arc;

#[async_trait]
pub trait McpHost: Send + Sync {
    async fn add_client(&mut self, mcp_client: Box<dyn McpClient>);

    async fn get_tool(&self) -> anyhow::Result<Vec<ToolDefinition>>;

    async fn call_tool(&self, param: ToolCall) -> anyhow::Result<ToolResult>;
}

pub struct UnionMcpHost {
    pub session_id: Option<String>,
    pub device_mcp_client: DeviceMcpClient,
    mcp_client_list: Vec<Arc<dyn McpClient>>,
    // function_name_and_client_map: HashMap<String, Box<dyn McpClient>>,
    // TODO: map for function name and mcp client ref
}

#[async_trait]
impl McpHost for UnionMcpHost {
    async fn add_client(&mut self, mcp_client: Box<dyn McpClient>) {
        self.mcp_client_list.push(mcp_client.into());
    }

    async fn get_tool(&self) -> anyhow::Result<Vec<ToolDefinition>> {
        // TODO: refactor to &Vec::<ToolDefinition> ?
        let mut tools = Vec::<ToolDefinition>::new();
        tools.append(&mut self.device_mcp_client.get_tool().await?);
        for item in &self.mcp_client_list {
            let mut sub_items = item.get_tool().await?;
            tools.append(&mut sub_items);
        }
        Ok(tools)
    }

    async fn call_tool(&self, param: ToolCall) -> anyhow::Result<ToolResult> {
        // TODO: need route(check map for function name) before tool call
        // TODO: cache by function_name_and_client_map?
        let mut function_name_and_client_map = HashMap::<String, Arc<dyn McpClient>>::new();
        for mcp_client in &self.mcp_client_list {
            let tools = mcp_client.get_tool().await?;
            for tool in tools {
                function_name_and_client_map.insert(tool.name, mcp_client.clone());
            }
        }
        let client = function_name_and_client_map
            .get(&param.function.name)
            .with_context(|| {
                anyhow::anyhow!(format!(
                    "can't find function name = {}",
                    param.function.name
                ))
            })?;
        client.call_tool(param).await
    }
}

impl UnionMcpHost {
    //design note:
    //why not export device_mcp_client outside see: https://rust-unofficial.github.io/patterns/anti_patterns/deref.html#disadvantages

    pub fn new(session_id: Option<String>) -> Self {
        Self {
            session_id: session_id.clone(),
            device_mcp_client: DeviceMcpClient::new(session_id.clone()),
            mcp_client_list: vec![],
            // function_name_and_client_map: HashMap::new(),
        }
    }

    pub async fn get_all_tools(&self) -> Vec<Tool> {
        self.device_mcp_client.tools.clone()
    }

    // device mcp start
    pub async fn create_initialize_request(&mut self) -> McpRequest {
        self.device_mcp_client.create_initialize_request().await
    }

    pub async fn handle_initialize_result(&mut self, message: &JsonRpcMessage) {
        self.device_mcp_client
            .handle_initialize_result(message)
            .await
    }

    pub async fn create_tools_list_request(&mut self) -> McpRequest {
        self.device_mcp_client.create_tools_list_request().await
    }

    pub async fn handle_tools_list_result(&mut self, message: &JsonRpcMessage) -> bool {
        self.device_mcp_client
            .handle_tools_list_result(message)
            .await
    }

    pub async fn get_phase(&self) -> &DeviceMcpPhase {
        &self.device_mcp_client.phase
    }
    // device mcp end
}
