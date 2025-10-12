use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CloseMessage<'a> {
    /// The reason as a code.
    pub code: u16,
    /// The reason as text string.
    pub reason: &'a str,
}

impl<'a> CloseMessage<'a> {
    pub fn new(code: u16, reason: &'a str) -> Self {
        Self { code, reason }
    }
}
