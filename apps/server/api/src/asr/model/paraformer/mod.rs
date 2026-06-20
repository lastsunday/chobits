use async_trait::async_trait;
use sherpa_onnx::{OfflineParaformerModelConfig, OfflineRecognizer, OfflineRecognizerConfig};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::{
    asr::{Asr, RecognizerResult},
    common::ModelError,
};

pub struct AsrParaformer {
    recognizer: Arc<Mutex<OfflineRecognizer>>,
}

impl AsrParaformer {
    pub fn new(path: &str) -> Result<Self, ModelError> {
        let model_path = auto_discover_onnx(path, "model")
            .ok_or_else(|| ModelError::ModelFileNotFound(format!("model.int8.onnx in {path}")))?;
        let tokens_path = format!("{path}tokens.txt");
        if !std::path::Path::new(&tokens_path).exists() {
            return Err(ModelError::ModelFileNotFound(format!(
                "tokens.txt in {path}"
            )));
        }

        let mut config = OfflineRecognizerConfig::default();
        config.model_config.paraformer = OfflineParaformerModelConfig {
            model: Some(model_path),
        };
        config.model_config.tokens = Some(tokens_path);
        config.model_config.num_threads = 2;
        config.model_config.model_type = Some("paraformer".into());

        let recognizer = OfflineRecognizer::create(&config)
            .ok_or_else(|| ModelError::Asr("failed to create Paraformer recognizer".into()))?;
        Ok(Self {
            recognizer: Arc::new(Mutex::new(recognizer)),
        })
    }
}

#[async_trait]
impl Asr for AsrParaformer {
    async fn transcribe(
        &mut self,
        sample_rate: u32,
        samples: &[f32],
    ) -> Result<RecognizerResult, ModelError> {
        let recognizer = self.recognizer.clone();
        let recognizer = &mut *recognizer.lock().await;
        let stream = recognizer.create_stream();
        stream.accept_waveform(sample_rate as i32, samples);
        recognizer.decode(&stream);
        let result = stream
            .get_result()
            .ok_or_else(|| ModelError::Asr("Paraformer returned no result".into()))?;

        Ok(RecognizerResult {
            text: result.text,
            prob: 1.0,
        })
    }
}

fn auto_discover_onnx(dir: &str, prefix: &str) -> Option<String> {
    let p = std::path::Path::new(dir);
    std::fs::read_dir(p).ok().and_then(|mut entries| {
        entries.find_map(|entry| {
            entry.ok().and_then(|e| {
                let path = e.path();
                if path.extension().is_some_and(|ext| ext == "onnx")
                    && path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .is_some_and(|stem| stem.contains(prefix))
                {
                    path.to_str().map(|s| s.to_string())
                } else {
                    None
                }
            })
        })
    })
}
