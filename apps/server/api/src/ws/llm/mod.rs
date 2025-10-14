pub mod llm_cache;
pub mod model;
pub mod models;

use std::pin::Pin;

use futures::Stream;

use crate::ws::common::ModelError;

pub trait Llm: Send + Sync {
    fn chat(
        &self,
        system_prompt: String,
        text: String,
    ) -> Pin<Box<dyn Stream<Item = Result<String, ModelError>> + Send>>;
}
