use std::sync::{
    Arc,
    atomic::{AtomicI64, Ordering},
};

use crate::{
    mcp::client::McpClient,
    ws::frame::{FrameError, FrameResult},
};
use async_trait::async_trait;
use rig::{
    completion::ToolDefinition,
    message::{ToolCall, ToolResult},
};
use rmcp::model::{
    ClientCapabilities, ConstString, Implementation, InitializeRequest, InitializeRequestParam,
    InitializeResult, JsonObject, JsonRpcMessage, JsonRpcRequest, JsonRpcVersion2_0,
    ListToolsRequest, ListToolsResult, PaginatedRequestParam, ProtocolVersion, Request, RequestId,
    Tool, object,
};
use serde::Serialize;
use service::chobits::message::{
    hello::HelloMessage,
    mcp::{McpMessage, McpRequest},
};
use tokio::sync::Mutex;
use tokio::sync::mpsc::Sender;
use tracing::{error, info};

#[derive(Debug, Clone)]
pub enum DeviceMcpPhase {
    Initialize,
    GetToolList,
}

pub struct DeviceMcpClient {
    session_id: Option<String>,
    current_request_id: Option<RequestId>,
    request_id: AtomicI64,
    next_cursor: Option<String>,
    pub tools: Vec<Tool>,
    pub phase: DeviceMcpPhase,
    output_tx: Arc<Mutex<Sender<Result<FrameResult, FrameError>>>>,
}

#[async_trait]
impl McpClient for DeviceMcpClient {
    async fn get_tool(&self) -> anyhow::Result<Vec<ToolDefinition>> {
        let mut result = vec![];
        for tool in &self.tools {
            result.push(ToolDefinition {
                name: tool.name.to_string(),
                description: tool.description.clone().unwrap_or_default().to_string(),
                parameters: serde_json::to_value(tool.input_schema.clone())?,
            });
        }
        Ok(result)
    }

    // TODO:
    async fn call_tool(&self, param: ToolCall) -> anyhow::Result<ToolResult> {
        todo!()
    }
}

impl DeviceMcpClient {
    pub fn new(
        session_id: Option<String>,
        output_tx: Arc<Mutex<Sender<Result<FrameResult, FrameError>>>>,
    ) -> Self {
        Self {
            session_id,
            current_request_id: None,
            request_id: AtomicI64::new(0),
            next_cursor: None,
            tools: Vec::new(),
            phase: DeviceMcpPhase::Initialize,
            output_tx,
        }
    }

    pub async fn create_initialize_request(&mut self) -> McpRequest {
        let id = RequestId::Number(self.request_id.fetch_add(1, Ordering::Relaxed));
        self.current_request_id = Some(id.clone());
        let request = InitializeRequest::new(InitializeRequestParam {
            protocol_version: ProtocolVersion::V_2025_06_18,
            capabilities: ClientCapabilities {
                ..Default::default()
            },
            client_info: Implementation::from_build_env(),
        });
        let method = request.method.as_str().to_string();
        let params = object(serde_json::to_value(request.params).unwrap());
        McpRequest::new(
            self.session_id.clone(),
            JsonRpcRequest {
                jsonrpc: JsonRpcVersion2_0,
                id,
                request: Request {
                    method,
                    params,
                    ..Default::default()
                },
            },
        )
    }

    pub async fn handle_initialize_result(&mut self, message: &JsonRpcMessage) {
        // info!("message = {:?}", message.clone());
        let result = message.clone().into_response();
        match result {
            Some((response, id)) => {
                if let Some(current_request_id) = &self.current_request_id {
                    if current_request_id.clone().eq(&id) {
                        let response: InitializeResult =
                            serde_json::from_value(serde_json::Value::Object(response)).unwrap();
                        info!(
                            "name = {}, version = {}",
                            response.server_info.name, response.server_info.version
                        );
                        self.phase = DeviceMcpPhase::GetToolList;
                    } else {
                        error!(
                            "invalid id,current = {:?}, reponse = {:?}",
                            self.current_request_id, id
                        );
                    }
                } else {
                    error!("invalid id,current is None, reponse = {:?}", id);
                }
            }
            None => {
                error!("invalid mpc message = {:?}", message);
            }
        }
    }

    pub async fn create_tools_list_request(&mut self) -> McpRequest {
        let id = RequestId::Number(self.request_id.fetch_add(1, Ordering::Relaxed));
        self.current_request_id = Some(id.clone());
        let request = ListToolsRequest::with_param(PaginatedRequestParam {
            cursor: self.next_cursor.clone(),
        });
        McpRequest::new(
            self.session_id.clone(),
            JsonRpcRequest {
                jsonrpc: JsonRpcVersion2_0,
                id,
                request: Request {
                    method: request.method.as_str().to_string(),
                    params: to_json_object(request.params),
                    ..Default::default()
                },
            },
        )
    }

    pub async fn handle_tools_list_result(&mut self, message: &JsonRpcMessage) -> bool {
        // info!("message = {:?}", message.clone());
        let result = message.clone().into_response();
        match result {
            Some((response, id)) => {
                if let Some(current_request_id) = &self.current_request_id {
                    if current_request_id.clone().eq(&id) {
                        let response: ListToolsResult =
                            serde_json::from_value(serde_json::Value::Object(response)).unwrap();
                        let next_cursor = response.next_cursor;
                        let mut tools = response.tools;
                        self.tools.append(&mut tools);
                        self.next_cursor = next_cursor.clone();
                        return next_cursor.is_some();
                    } else {
                        error!(
                            "invalid id,current = {:?}, reponse = {:?}",
                            self.current_request_id, id
                        );
                    }
                } else {
                    error!("invalid id,current is None, reponse = {:?}", id);
                }
            }
            None => {
                error!("invalid mpc message = {:?}", message);
            }
        }
        false
    }

    pub async fn handle_mcp(&mut self, message: &McpMessage) {
        match self.phase {
            DeviceMcpPhase::Initialize => {
                self.handle_mcp_initialize_result(message).await;
                self.request_mcp_tools_list().await;
            }
            DeviceMcpPhase::GetToolList => {
                let has_next = self.handle_mcp_tools_list_result(message).await;
                if has_next {
                    self.request_mcp_tools_list().await;
                } else {
                    // TODO:end of get deivce mcp tools list
                    // let tools_list = mcp_host.get_all_tools().await;
                    // info!("{:?}", tools_list);
                }
            }
        }
    }

    pub async fn request_mcp_initialize(&mut self, _hello_message: &HelloMessage) {
        let tx = self.output_tx.clone();
        let tx = tx.lock().await;
        let request = self.create_initialize_request().await;
        // mcp request send
        let result = tx.send(Ok(FrameResult::McpResult(request))).await;
        if result.is_err() {
            info!("tx send mcp initialize reqeust failure");
        }
    }

    async fn handle_mcp_initialize_result(&mut self, message: &McpMessage) {
        self.handle_initialize_result(&message.payload).await;
    }

    async fn request_mcp_tools_list(&mut self) {
        let tx = self.output_tx.clone();
        let tx = tx.lock().await;
        let result = tx
            .send(Ok(FrameResult::McpResult(
                self.create_tools_list_request().await,
            )))
            .await;
        if result.is_err() {
            info!("tx send mcp tools list reqeust failure");
        }
    }

    async fn handle_mcp_tools_list_result(&mut self, message: &McpMessage) -> bool {
        return self.handle_tools_list_result(&message.payload).await;
    }
}

fn to_json_object<T>(value: T) -> JsonObject
where
    T: Serialize,
{
    object(serde_json::to_value(value).unwrap())
}
