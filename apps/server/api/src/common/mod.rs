use candle_core::{
    Device, Error, Result,
    utils::{cuda_is_available, metal_is_available},
};
use rig::completion::CompletionError;

pub fn device(cpu: bool) -> Result<Device> {
    if cpu {
        Ok(Device::Cpu)
    } else if cuda_is_available() {
        Ok(Device::new_cuda(0)?)
    } else if metal_is_available() {
        Ok(Device::new_metal(0)?)
    } else {
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        {
            println!(
                "Running on CPU, to run on GPU(metal), build this example with `--features metal`"
            );
        }
        #[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
        {
            tracing::info!(
                "Running on CPU, to run on GPU, build this example with `--features cuda`"
            );
        }
        Ok(Device::Cpu)
    }
}

pub fn format_size(size_in_bytes: usize) -> String {
    if size_in_bytes < 1_000 {
        format!("{size_in_bytes}B")
    } else if size_in_bytes < 1_000_000 {
        format!("{:.2}KB", size_in_bytes as f64 / 1e3)
    } else if size_in_bytes < 1_000_000_000 {
        format!("{:.2}MB", size_in_bytes as f64 / 1e6)
    } else {
        format!("{:.2}GB", size_in_bytes as f64 / 1e9)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ModelError {
    #[error("model file not found path = {0}")]
    ModelFileNotFound(String),
    #[error("token file not found path = {0}")]
    TokenFileNotFound(String),
    #[error("model init failure path = {0}")]
    ModelInitFailure(String),
    #[error("token init failure path = {0}")]
    TokenInitFailure(String),
    #[error("token convert failure = {0}")]
    TokenConvertFailure(String),
    #[error("model completion failure = {0}")]
    ModelCompletionError(String),
    #[error("chat failure msg = {0}")]
    Chat(String),
    #[error("tensor error msg = {0}")]
    Tensor(String),
    #[error("decoder error msg = {0}")]
    Decoder(String),
    #[error("tts error msg = {0}")]
    Tts(String),
}

impl From<Error> for ModelError {
    fn from(value: Error) -> Self {
        ModelError::Tensor(value.to_string())
    }
}

impl From<CompletionError> for ModelError {
    fn from(value: CompletionError) -> Self {
        ModelError::ModelCompletionError(value.to_string())
    }
}

impl From<regex::Error> for ModelError {
    fn from(value: regex::Error) -> Self {
        ModelError::TokenConvertFailure(value.to_string())
    }
}

impl From<ModelError> for CompletionError {
    fn from(value: ModelError) -> Self {
        CompletionError::ResponseError(value.to_string())
    }
}
