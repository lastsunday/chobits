use api::{asr::AsrFactory, config::asr::AsrConfig, util::audio::pcm_decode};
use std::{path::PathBuf, sync::Arc};
use tracing::debug;
use tracing_test::traced_test;

#[tokio::test]
#[traced_test]
#[ignore]
/// cargo test --test asr_test -- test_asr --ignored --nocapture
async fn test_asr() {
    let wav_file: PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "resources",
        "test",
        "samples_jfk.wav",
    ]
    .iter()
    .collect();
    debug!("{}", wav_file.display());
    let (pcm_data, sample_rate) = pcm_decode(wav_file).unwrap();
    debug!(
        "pcm_data len = {},sample_rate = {}",
        pcm_data.len(),
        sample_rate
    );

    AsrFactory::init(Arc::new(AsrConfig {
        path: Some(String::from("data/asr/model/openai/whisper-small/")),
    }))
    .await;
    let asr = AsrFactory::global().default();
    let asr = asr.clone();
    let mut asr = asr.lock().await;
    let result = asr.transcribe(sample_rate, &pcm_data).await;
    debug!("{:?}", result);
}
