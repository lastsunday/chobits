use std::path::Path;
use std::sync::Arc;

use api::{
    config::{TtsModel, audio::AudioConfig, tts::TtsConfig},
    tts::TtsFactory,
};
use tokio_stream::StreamExt;
use tracing::info;
use tracing_test::traced_test;

mod common;
use common::tts::*;

#[tokio::test]
#[traced_test]
#[ignore]
/// cargo test --test tts_test -- test_tts_default --ignored --nocapture
async fn test_tts_default() -> anyhow::Result<()> {
    const ENCODE_SAMPLE_RATE: u32 = 16000;
    const MONO_20MS: usize = ENCODE_SAMPLE_RATE as usize * 20 / 1000;
    let size = MONO_20MS;
    TtsFactory::init(
        Arc::new(TtsConfig {
            model: Some(TtsModel::Mute),
            ..Default::default()
        }),
        Arc::new(AudioConfig {
            ..Default::default()
        }),
    )
    .await?;
    let tts = TtsFactory::global().default();
    let text_stream = tts_stream(String::from(TEST_TTS_TEXT));
    let mut tts_stream = tts.stream(Box::pin(text_stream)).await;

    let mut audio: Vec<Vec<u8>> = Vec::new();
    while let Some(data) = tts_stream.next().await {
        match data {
            Ok(data) => {
                info!("{:?}", data.text);
                match data.audio {
                    Some(data) => {
                        audio.append(&mut data.clone());
                    }
                    None => {
                        audio.append(&mut vec![]);
                    }
                }
            }
            Err(e) => {
                panic!("{:?}", e);
            }
        }
    }
    let audio_len = audio.len();
    info!("audio len = {}", audio_len);

    let mut decoder = opus_rs::OpusDecoder::new(ENCODE_SAMPLE_RATE as i32, 1_usize).unwrap();
    let mut decode_data: Vec<f32> = Vec::new();
    for n in 0..audio_len {
        let mut samples = vec![0f32; size];
        let data = audio.get(n).unwrap();
        let len = decoder.decode(data, size, &mut samples).unwrap();
        decode_data.append(&mut samples[..len].to_vec());
    }

    info!("decode_data len = {}", decode_data.len());
    std::fs::create_dir_all("./test_data")?;
    let _ = wavers::write("./test_data/test_tts_default.wav", &decode_data, 16000, 1);
    Ok(())
}

#[tokio::test]
#[traced_test]
/// cargo test --test tts_test -- test_tts_mute --nocapture
async fn test_tts_mute() -> anyhow::Result<()> {
    TtsFactory::init(
        Arc::new(TtsConfig {
            model: Some(TtsModel::Mute),
            ..Default::default()
        }),
        Arc::new(AudioConfig {
            ..Default::default()
        }),
    )
    .await?;
    let tts = TtsFactory::global().default();
    let text_stream = tts_stream(String::from(TEST_TTS_TEXT));
    let mut tts_stream = tts.stream(Box::pin(text_stream)).await;
    let mut audio: Vec<Vec<u8>> = Vec::new();
    while let Some(data) = tts_stream.next().await {
        match data {
            Ok(data) => {
                assert_eq!(data.text, TEST_TTS_TEXT);
                match data.audio {
                    Some(data) => {
                        audio.append(&mut data.clone());
                    }
                    None => {
                        audio.append(&mut vec![]);
                    }
                }
            }
            Err(e) => {
                panic!("{:?}", e);
            }
        }
    }
    let audio_len = audio.len();
    assert_eq!(0, audio_len);
    Ok(())
}

#[tokio::test]
#[traced_test]
#[ignore]
/// cargo test --test tts_test -- test_tts_pocket --ignored --nocapture
async fn test_tts_pocket() -> anyhow::Result<()> {
    let ws_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap();

    let model_path = ws_root
        .join("data/tts/model/pocket/default/")
        .to_string_lossy()
        .into_owned();

    let ref_wav = ws_root.join("data/tts/reference/bria.wav");

    let tts = TtsFactory::create_model(
        &TtsConfig {
            model: Some(TtsModel::PocketTts),
            path: Some(model_path),
            reference_prompt_wav_path: Some(ref_wav.to_string_lossy().into()),
            ..Default::default()
        },
        &AudioConfig {
            output_sample_rate: Some(16000),
            output_channel: Some(1),
            output_frame_duration: Some(20),
            ..Default::default()
        },
    )
    .await?;

    let gen_start = std::time::Instant::now();
    let text_stream = tts_stream(String::from(TEST_TTS_TEXT));
    let mut tts_stream = tts.stream(Box::pin(text_stream)).await;

    let mut all_packets: Vec<Vec<u8>> = Vec::new();
    let mut raw_pcm: Option<(Vec<f32>, i32)> = None;
    while let Some(data) = tts_stream.next().await {
        match data {
            Ok(data) => {
                info!("text: {}", data.text);
                if let Some(packets) = data.audio {
                    all_packets.extend(packets);
                }
                if raw_pcm.is_none() {
                    raw_pcm = data.raw_pcm;
                }
            }
            Err(e) => panic!("{:?}", e),
        }
    }
    let gen_elapsed = gen_start.elapsed();

    assert!(
        !all_packets.is_empty(),
        "Expected audio packets from PocketTTS"
    );

    if let Some((samples, sr)) = &raw_pcm {
        std::fs::create_dir_all("./test_data")?;
        let _ = wavers::write("./test_data/test_tts_pocket_raw.wav", samples, *sr, 1);
        info!("raw PCM: {} samples at {}Hz", samples.len(), sr);
    }

    let decode_fs = 320;
    let mut decoder = opus_rs::OpusDecoder::new(16000, 1_usize).unwrap();
    let mut decoded = Vec::new();
    for packet in &all_packets {
        let mut samples = vec![0f32; decode_fs];
        if let Ok(len) = decoder.decode(packet, decode_fs, &mut samples) {
            decoded.extend_from_slice(&samples[..len]);
        }
    }
    assert!(decoded.len() > 1000, "Decoded audio too short");
    info!("decoded {} PCM samples", decoded.len());
    info!("{}", analyze_audio(&decoded, 16000, gen_elapsed, estimate_std_duration(TEST_TTS_TEXT)));

    std::fs::create_dir_all("./test_data")?;
    let _ = wavers::write("./test_data/test_tts_pocket.wav", &decoded, 16000, 1);
    Ok(())
}

#[tokio::test]
#[traced_test]
#[ignore]
/// cargo test --test tts_test -- test_tts_matcha_zh_baker --ignored --nocapture
async fn test_tts_matcha_zh_baker() -> anyhow::Result<()> {
    let path = ws_root()
        .join("data/tts/model/matcha/matcha-icefall-zh-baker/")
        .to_string_lossy()
        .into_owned();
    run_vits_test(
        &TtsConfig {
            model: Some(TtsModel::MatchaTts),
            path: Some(path),
            options: Some(serde_json::json!({
                "num_threads": 2,
                "noise_scale": 0.667,
                "length_scale": 1.0,
                "speed": 1.0,
                "debug": false,
            })),
            ..Default::default()
        },
        &vits_audio_config(),
        "./test_data/test_tts_matcha_zh_baker.wav",
    )
    .await
}

#[tokio::test]
#[traced_test]
#[ignore]
/// cargo test --test tts_test -- test_tts_matcha_zh_en --ignored --nocapture
async fn test_tts_matcha_zh_en() -> anyhow::Result<()> {
    let path = ws_root()
        .join("data/tts/model/matcha/matcha-icefall-zh-en/")
        .to_string_lossy()
        .into_owned();
    run_vits_test(
        &TtsConfig {
            model: Some(TtsModel::MatchaTts),
            path: Some(path),
            options: Some(serde_json::json!({
                "num_threads": 2,
                "noise_scale": 0.667,
                "length_scale": 1.0,
                "speed": 1.0,
                "debug": false,
            })),
            ..Default::default()
        },
        &vits_audio_config(),
        "./test_data/test_tts_matcha_zh_en.wav",
    )
    .await
}

#[tokio::test]
#[traced_test]
#[ignore]
/// cargo test --test tts_test -- test_tts_vits_melo_tts_zh_en --ignored --nocapture
async fn test_tts_vits_melo_tts_zh_en() -> anyhow::Result<()> {
    let path = ws_root()
        .join("data/tts/model/vits/melo-tts-zh_en/")
        .to_string_lossy()
        .into_owned();
    run_vits_test(
        &TtsConfig {
            model: Some(TtsModel::Vits),
            path: Some(path),
            options: Some(serde_json::json!({
                "num_threads": 2,
                "noise_scale": 0.667,
                "noise_scale_w": 0.8,
                "length_scale": 1.0,
                "speed": 1.0,
                "sid": 0,
                "debug": false,
            })),
            ..Default::default()
        },
        &vits_audio_config(),
        "./test_data/test_tts_vits_melo_tts_zh_en.wav",
    )
    .await
}

#[tokio::test]
#[traced_test]
#[ignore]
/// cargo test --test tts_test -- test_tts_vits_zh_hf_theresa --ignored --nocapture
async fn test_tts_vits_zh_hf_theresa() -> anyhow::Result<()> {
    let path = ws_root()
        .join("data/tts/model/vits/zh-hf-theresa/")
        .to_string_lossy()
        .into_owned();
    run_vits_test(
        &TtsConfig {
            model: Some(TtsModel::Vits),
            path: Some(path),
            options: Some(serde_json::json!({
                "num_threads": 2,
                "noise_scale": 0.667,
                "noise_scale_w": 0.8,
                "length_scale": 1.0,
                "speed": 1.0,
                "sid": 0,
                "debug": false,
            })),
            ..Default::default()
        },
        &vits_audio_config(),
        "./test_data/test_tts_vits_zh_hf_theresa.wav",
    )
    .await
}

#[tokio::test]
#[traced_test]
#[ignore]
/// cargo test --test tts_test -- test_tts_vits_aishell3 --ignored --nocapture
async fn test_tts_vits_aishell3() -> anyhow::Result<()> {
    let path = ws_root()
        .join("data/tts/model/vits/aishell3/")
        .to_string_lossy()
        .into_owned();
    run_vits_test(
        &TtsConfig {
            model: Some(TtsModel::Vits),
            path: Some(path),
            options: Some(serde_json::json!({
                "num_threads": 2,
                "noise_scale": 0.667,
                "noise_scale_w": 0.8,
                "length_scale": 1.0,
                "speed": 1.0,
                "sid": 0,
                "debug": false,
            })),
            ..Default::default()
        },
        &vits_audio_config(),
        "./test_data/test_tts_vits_aishell3.wav",
    )
    .await
}
