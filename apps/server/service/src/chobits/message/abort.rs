use super::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AbortMessage<'a> {
    #[serde(flatten)]
    pub message: Message,
    pub session_id: Option<&'a str>,
    pub reason: Option<&'a str>,
}

impl<'a> AbortMessage<'a> {
    pub fn new(session_id: Option<&'a str>, reason: Option<&'a str>) -> Self {
        Self {
            message: Message { mtype: Type::Abort },
            session_id,
            reason,
        }
    }
}
