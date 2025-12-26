use serde::Deserialize;

#[derive(Debug, Default, Deserialize, Clone)]
pub struct VadConfig {
    path: Option<String>,
    num_threads: Option<i32>,
}

impl VadConfig {
    pub fn new() -> Self {
        Self {
            path: Some(String::from("data/vad/model/silero/")),
            num_threads: Some(4),
        }
    }

    pub fn path(&self) -> &str {
        self.path.as_deref().unwrap_or_default()
    }

    pub fn num_threads(&self) -> i32 {
        self.num_threads.unwrap_or_default()
    }
}
