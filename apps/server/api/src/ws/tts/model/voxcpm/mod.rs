use std::pin::Pin;

use async_trait::async_trait;
use futures::Stream;

use crate::ws::{
    common::ModelError,
    tts::{Tts, TtsData, TtsError},
};

pub struct TtsVoxCPM {}

#[async_trait]
impl Tts for TtsVoxCPM {
    async fn stream(
        &self,
        text_stream: Pin<
            Box<dyn Stream<Item = core::result::Result<String, ModelError>> + Send + Sync>,
        >,
    ) -> Pin<Box<dyn Stream<Item = core::result::Result<TtsData, TtsError>> + Send + Sync>> {
        todo!()
    }
}
