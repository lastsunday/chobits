use std::collections::HashMap;

use anyhow::Context;
use async_trait::async_trait;
use rig::{
    completion::ToolDefinition,
    message::{ToolCall, ToolResult},
};
use tokio::sync::Mutex;

use crate::mcp::client::{McpClient, device::DeviceMcpClient};
use std::sync::Arc;

#[async_trait]
pub trait McpHost: Send + Sync {
    async fn set_device_client(&mut self, mcp_client: Arc<Mutex<DeviceMcpClient>>);

    async fn get_device_client(&mut self) -> Option<Arc<Mutex<DeviceMcpClient>>>;

    async fn add_client(&mut self, mcp_client: Box<dyn McpClient>);

    async fn get_tool(&self) -> anyhow::Result<Vec<ToolDefinition>>;

    async fn call_tool(&self, param: ToolCall) -> anyhow::Result<ToolResult>;
}

pub struct UnionMcpHost {
    pub session_id: Option<String>,
    mcp_client_list: Vec<Arc<dyn McpClient>>,
    // function_name_and_client_map: HashMap<String, Box<dyn McpClient>>,
    // TODO: map for function name and mcp client ref
    device_mcp_client: Option<Arc<Mutex<DeviceMcpClient>>>,
}

#[async_trait]
impl McpHost for UnionMcpHost {
    async fn set_device_client(&mut self, mcp_client: Arc<Mutex<DeviceMcpClient>>) {
        self.device_mcp_client = Some(mcp_client);
    }

    async fn get_device_client(&mut self) -> Option<Arc<Mutex<DeviceMcpClient>>> {
        self.device_mcp_client.clone()
    }

    async fn add_client(&mut self, mcp_client: Box<dyn McpClient>) {
        self.mcp_client_list.push(mcp_client.into());
    }

    async fn get_tool(&self) -> anyhow::Result<Vec<ToolDefinition>> {
        // TODO: refactor to &Vec::<ToolDefinition> ?
        let mut tools = Vec::<ToolDefinition>::new();
        if let Some(item) = self.device_mcp_client.clone() {
            let item = item.lock().await;
            let mut sub_items = item.get_tool().await?;
            tools.append(&mut sub_items);
        }
        for item in &self.mcp_client_list {
            let mut sub_items = item.get_tool().await?;
            tools.append(&mut sub_items);
        }
        Ok(tools)
    }

    async fn call_tool(&self, param: ToolCall) -> anyhow::Result<ToolResult> {
        // TODO: need route(check map for function name) before tool call
        // TODO: cache by function_name_and_client_map?
        let function_name = param.function.name.as_str();
        let mut function_name_and_client_map = HashMap::<String, Arc<dyn McpClient>>::new();
        for mcp_client in &self.mcp_client_list {
            let tools = mcp_client.get_tool().await?;
            for tool in tools {
                function_name_and_client_map.insert(tool.name.clone(), mcp_client.clone());
            }
        }
        if function_name_and_client_map.contains_key(function_name) {
            // server tool call
            let client = function_name_and_client_map
                .get(function_name)
                .with_context(|| {
                    anyhow::anyhow!(format!(
                        "can't find function name = {}",
                        param.function.name
                    ))
                })?;
            client.call_tool(param).await
        } else {
            // device tool call
            let mut function_name_and_client_map = HashMap::<String, String>::new();
            if let Some(mcp_client) = self.device_mcp_client.clone() {
                let mcp_client = mcp_client.lock().await;
                let tools = mcp_client.get_tool().await?;
                for tool in tools {
                    function_name_and_client_map.insert(tool.name.clone(), tool.name.clone());
                }
                if function_name_and_client_map.contains_key(function_name) {
                    mcp_client.call_tool(param).await
                } else {
                    Err(anyhow::anyhow!(format!(
                        "can't find function name = {}",
                        function_name
                    )))
                }
            } else {
                Err(anyhow::anyhow!(format!(
                    "can't find function name = {}",
                    function_name
                )))
            }
        }
    }
}

impl UnionMcpHost {
    //design note:
    //why not export device_mcp_client outside see: https://rust-unofficial.github.io/patterns/anti_patterns/deref.html#disadvantages

    pub fn new(session_id: Option<String>) -> Self {
        Self {
            session_id: session_id.clone(),
            mcp_client_list: vec![],
            device_mcp_client: None,
            // function_name_and_client_map: HashMap::new(),
        }
    }
}
