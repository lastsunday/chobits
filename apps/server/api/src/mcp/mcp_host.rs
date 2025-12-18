use rmcp::model::{JsonRpcMessage, Tool};
use service::chobits::message::mcp::McpRequest;

use crate::mcp::device::{DeviceMcpClient, DeviceMcpPhase};

pub struct McpHost {
    pub session_id: Option<String>,
    device_mcp_client: DeviceMcpClient,
}

impl McpHost {
    //design note:
    //why not export device_mcp_client outside see: https://rust-unofficial.github.io/patterns/anti_patterns/deref.html#disadvantages

    pub fn new(session_id: Option<String>) -> Self {
        Self {
            session_id: session_id.clone(),
            device_mcp_client: DeviceMcpClient::new(session_id.clone()),
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
