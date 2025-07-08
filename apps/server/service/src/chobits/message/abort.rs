use super::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AbortMessage {
    #[serde(flatten)]
    pub message: Message,
    pub session_id: Option<String>,
    pub reason: String,
}

impl AbortMessage {
    pub fn new(session_id: Option<String>, reason: String) -> Self {
        Self {
            message: Message { mtype: Type::Abort },
            session_id,
            reason,
        }
    }
}
