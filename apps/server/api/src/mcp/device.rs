use std::sync::atomic::{AtomicI64, Ordering};

use rmcp::model::{
    ClientCapabilities, ConstString, Implementation, InitializeRequest, InitializeRequestParam,
    InitializeResult, JsonObject, JsonRpcMessage, JsonRpcRequest, JsonRpcVersion2_0,
    ListToolsRequest, ListToolsResult, PaginatedRequestParam, ProtocolVersion, Request, RequestId,
    Tool, object,
};
use serde::Serialize;
use service::chobits::message::mcp::McpRequest;
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
}

impl DeviceMcpClient {
    pub fn new(session_id: Option<String>) -> Self {
        Self {
            session_id,
            current_request_id: None,
            request_id: AtomicI64::new(0),
            next_cursor: None,
            tools: Vec::new(),
            phase: DeviceMcpPhase::Initialize,
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
}

fn to_json_object<T>(value: T) -> JsonObject
where
    T: Serialize,
{
    object(serde_json::to_value(value).unwrap())
}
