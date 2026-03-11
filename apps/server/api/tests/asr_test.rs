use api::{
    asr::AsrFactory,
    config::{AsrModel, asr::AsrConfig},
    util::audio::pcm_decode,
};
use std::{path::PathBuf, sync::Arc};
use tracing::debug;
use tracing_test::traced_test;

#[tokio::test]
#[traced_test]
#[ignore]
/// cargo test --test asr_test -- test_asr --ignored --nocapture
/// asr speed up by release mode
/// cargo test --test asr_test --release -- test_asr --ignored --nocapture
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
        model: Some(AsrModel::Qwen3),
        path: Some(String::from("data/asr/model/Qwen/Qwen3-ASR-0.6B/")),
    }))
    .await;
    let asr = AsrFactory::global().default();
    let asr = asr.clone();
    let mut asr = asr.lock().await;
    let result = asr.transcribe(sample_rate, &pcm_data).await;
    debug!("{:?}", result);
}

#[tokio::test]
#[traced_test]
/// cargo test --test asr_test -- test_asr_model_void  --nocapture
async fn test_asr_model_void() {
    let mut model = AsrFactory::create_model(&AsrConfig {
        model: Some(AsrModel::Void),
        ..Default::default()
    });
    let result = model.transcribe(16000, &[]).await.unwrap();
    assert_eq!(String::new(), result.text);
    assert_eq!(1.0, result.prob);
}
