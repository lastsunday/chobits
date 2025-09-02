use super::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AudioMessage {
    pub data: Vec<Vec<u8>>,
}

impl AudioMessage {
    pub fn new(data: Vec<Vec<u8>>) -> Self {
        Self { data }
    }
}
