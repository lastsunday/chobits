use async_trait::async_trait;
use rig::{
    completion::ToolDefinition,
    message::{ToolCall, ToolResult},
};

pub mod device;
pub mod server;

#[async_trait]
pub trait McpClient: Send + Sync {
    async fn get_tool(&self) -> anyhow::Result<Vec<ToolDefinition>>;

    async fn call_tool(&self, param: ToolCall) -> anyhow::Result<ToolResult>;
}
