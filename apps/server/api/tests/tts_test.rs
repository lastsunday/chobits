use std::path::Path;
use std::{sync::Arc, thread};

use api::{
    common::ModelError,
    config::{TtsModel, audio::AudioConfig, tts::TtsConfig},
    tts::TtsFactory,
};
use futures::{Stream, executor::block_on};
use tokio::sync::mpsc::channel;
use tokio_stream::{StreamExt, wrappers::ReceiverStream};
use tracing::info;
use tracing_test::traced_test;
use wavers::write;

mod common;
use common::tts::*;

#[tokio::test]
#[traced_test]
#[ignore]
/// cargo test --test tts_test -- test_tts_default --ignored --nocapture
async fn test_tts_default() -> anyhow::Result<()> {
    const ENCODE_SAMPLE_RATE: u32 = 16000;
    // 16000Hz * 1 channel * 20 ms / 1000 = 320
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
    let text_stream = tts_stream(String::from("我不知道将去何方，但我已经在路上。"));
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

    // 4. decode opus packet to pcm data
    let mut decoder = opus_rs::OpusDecoder::new(ENCODE_SAMPLE_RATE as i32, 1_usize).unwrap();
    let mut decode_data: Vec<f32> = Vec::new();
    for n in 0..audio_len {
        let mut samples = vec![0f32; size];
        let data = audio.get(n).unwrap();
        let len = decoder.decode(data, size, &mut samples).unwrap();
        decode_data.append(&mut samples[..len].to_vec());
    }

    // the follow code is output wav file to test
    info!("decode_data len = {}", decode_data.len());
    std::fs::create_dir_all("./test_data")?;
    let fp = "./test_data/test_tts_default.wav";
    let sr: i32 = 16000;
    let _ = write(fp, &decode_data, sr, 1);
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
    let text_stream = tts_stream(String::from("我不知道将去何方，但我已经在路上。"));
    let mut tts_stream = tts.stream(Box::pin(text_stream)).await;
    let mut audio: Vec<Vec<u8>> = Vec::new();
    while let Some(data) = tts_stream.next().await {
        match data {
            Ok(data) => {
                assert_eq!(data.text, "我不知道将去何方，但我已经在路上。");
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
/// 先下载模型和参考音频：
///   cargo run --bin chobits-server -- downloader install tts pocket_tts default --all
///   cargo run --bin chobits-server -- downloader install reference audio xiyangyang --all
///   cargo run --bin chobits-server -- downloader install reference audio bria --all
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

    let ref_wav = ws_root.join("data/tts/reference/test_wavs/bria.wav");

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

    let text = "Today as always, men fall into two groups: slaves and free men. Whoever \
        does not have two-thirds of his day for himself, is a slave, whatever \
        he may be: a statesman, a businessman, an official, or a scholar. \
        Friends fell out often because life was changing so fast. The easiest \
        thing in the world was to lose touch with someone.";
    let text_stream = tts_stream(String::from(text));
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

    assert!(
        !all_packets.is_empty(),
        "Expected audio packets from PocketTTS"
    );

    // Save raw PCM (skip Opus)
    if let Some((samples, sr)) = &raw_pcm {
        std::fs::create_dir_all("./test_data")?;
        let _ = wavers::write("./test_data/test_tts_pocket_raw.wav", samples, *sr, 1);
        info!("raw PCM: {} samples at {}Hz", samples.len(), sr);
    }

    let decode_fs = 320; // 16000Hz * 1ch * 20ms / 1000
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

    std::fs::create_dir_all("./test_data")?;
    let _ = wavers::write("./test_data/test_tts_pocket.wav", &decoded, 16000, 1);
    Ok(())
}

/// Monorepo root path (3 levels up from `CARGO_MANIFEST_DIR`).
fn ws_root() -> &'static std::path::PathBuf {
    static ROOT: LazyLock<std::path::PathBuf> = LazyLock::new(|| {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf()
    });
    &ROOT
}

/// Standard AudioConfig for VITS tests: 16kHz / mono / 20ms frame duration.
fn vits_audio_config() -> AudioConfig {
    AudioConfig {
        output_sample_rate: Some(16000),
        output_channel: Some(1),
        output_frame_duration: Some(20),
        ..Default::default()
    }
}

/// Shared VITS test helper: create model → stream inference → Opus decode → write WAV.
async fn run_vits_test(
    tts_config: &TtsConfig,
    audio_config: &AudioConfig,
    wav: &str,
) -> anyhow::Result<()> {
    let tts = TtsFactory::create_model(tts_config, audio_config).await?;
    let text_stream = tts_stream(String::from(TEST_TTS_TEXT));
    let mut tts_stream = tts.stream(Box::pin(text_stream)).await;

    let gen_start = std::time::Instant::now();
    let mut all_packets: Vec<Vec<u8>> = Vec::new();
    while let Some(data) = tts_stream.next().await {
        match data {
            Ok(data) => {
                info!("text: {}", data.text);
                if let Some(packets) = data.audio {
                    all_packets.extend(packets);
                }
            }
            Err(e) => panic!("{:?}", e),
        }
    }
    let gen_elapsed = gen_start.elapsed();
    anyhow::ensure!(
        !all_packets.is_empty(),
        "Expected audio packets from VitsTTS"
    );

    let decode_fs = 320;
    let mut decoder = opus_rs::OpusDecoder::new(16000, 1_usize).unwrap();
    let mut decoded = Vec::new();
    for packet in &all_packets {
        let mut samples = vec![0f32; decode_fs];
        if let Ok(len) = decoder.decode(packet, decode_fs, &mut samples) {
            decoded.extend_from_slice(&samples[..len]);
        }
    }
    anyhow::ensure!(decoded.len() > 1000, "Decoded audio too short");
    let std_dur = estimate_std_duration(TEST_TTS_TEXT);
    info!("{}", analyze_audio(&decoded, 16000, gen_elapsed, std_dur));

    std::fs::create_dir_all("./test_data")?;
    let _ = wavers::write(wav, &decoded, 16000, 1);
    Ok(())
}

/// Scan length_scale values to find optimal timing (duration closest to standard).
async fn run_length_scale_scan(
    model: TtsModel,
    dir: &str,
    audio_cfg: &AudioConfig,
    wav_prefix: &str,
    ls_values: &[f32],
    sid: Option<i32>,
) -> anyhow::Result<()> {
    let path = ws_root()
        .join(dir)
        .to_string_lossy()
        .into_owned();
    let mut rows: Vec<(f32, f64, f64, f64)> = Vec::new();

    for &ls in ls_values {
        let wav = format!("./test_data/{wav_prefix}_ls{ls}.wav");
        let mut opts = serde_json::json!({
            "num_threads": 2,
            "noise_scale": 0.667,
            "length_scale": ls,
            "speed": 1.0,
            "debug": false,
        });
        if matches!(model, TtsModel::Vits) {
            let obj = opts.as_object_mut().unwrap();
            obj.insert("noise_scale_w".into(), serde_json::json!(0.8));
            if let Some(s) = sid {
                obj.insert("sid".into(), serde_json::json!(s));
            }
        }
        run_vits_test(
            &TtsConfig {
                model: Some(model.clone()),
                path: Some(path.clone()),
                options: Some(opts),
                ..Default::default()
            },
            audio_cfg,
            &wav,
        )
        .await?;

        let (samples, sr): (wavers::Samples<f32>, i32) = wavers::read(&wav)?;
        let diag = analyze_audio(
            &samples,
            sr as u32,
            std::time::Duration::ZERO,
            *TEST_TTS_TEXT_WEIGHT / 12.0,
        );
        info!("ls={ls}: {}", diag);
        let dev_pct = diag.std_diff_secs / diag.std_duration_secs * 100.0;
        rows.push((ls, diag.duration_secs, diag.std_diff_secs.abs(), dev_pct));
    }

    rows.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap());
    info!("=== length_scale scan summary (sorted by abs deviation) ===");
    info!("{:<8} {:<10} {:<10} {:<10}", "ls", "duration", "dev_abs", "dev_pct");
    for &(ls, dur, dev, dev_pct) in &rows {
        info!("{ls:<8.3} {dur:<6.2}s   {dev:<6.2}s   {dev_pct:<6.1}%");
    }
    if let Some(best) = rows.first() {
        info!("Best length_scale: {:.3} (dev={:.2}s, {:.1}%)", best.0, best.2, best.3);
    }
    Ok(())
}

#[tokio::test]
#[traced_test]
#[ignore]
/// cargo test --test tts_test -- test_tts_matcha_zh_baker --ignored --nocapture
/// 先下载模型：
///   cargo run --bin chobits-server -- downloader install tts matcha matcha-icefall-zh-baker --all
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
/// cargo test --test tts_test -- test_tts_vits_melo_tts_zh_en --ignored --nocapture
/// 先下载模型：
///   cargo run --bin chobits-server -- downloader install tts vits melo-tts-zh_en --all
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
/// 先下载模型：
///   cargo run --bin chobits-server -- downloader install tts vits zh-hf-theresa --all
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
/// 先下载模型：
///   cargo run --bin chobits-server -- downloader install tts vits aishell3 --all
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

#[tokio::test]
#[traced_test]
#[ignore]
/// cargo test --test tts_test -- test_tts_vits_melo_tts_zh_en_noise_scale --ignored --nocapture
async fn test_tts_vits_melo_tts_zh_en_noise_scale() -> anyhow::Result<()> {
    let path = ws_root()
        .join("data/tts/model/vits/melo-tts-zh_en/")
        .to_string_lossy()
        .into_owned();
    let audio_cfg = vits_audio_config();

    for (ns, ns_label) in [(0.667f32, "default"), (0.5, "mid"), (0.3, "low")] {
        let wav = format!("./test_data/test_tts_vits_melo_tts_zh_en_ns{ns}.wav");
        run_vits_test(
            &TtsConfig {
                model: Some(TtsModel::Vits),
                path: Some(path.clone()),
                options: Some(serde_json::json!({
                    "num_threads": 2, "noise_scale": ns, "noise_scale_w": 0.8,
                    "length_scale": 1.0, "speed": 1.0, "sid": 0, "debug": false,
                })),
                ..Default::default()
            },
            &audio_cfg,
            &wav,
        )
        .await?;
        let (samples, _sr): (wavers::Samples<f32>, i32) = wavers::read(&wav)?;
        info!(
            "ns={ns} ({ns_label}): {}",
            analyze_audio(
                &samples,
                _sr as u32,
                std::time::Duration::ZERO,
                *TEST_TTS_TEXT_WEIGHT / 12.0
            )
        );
    }

    for (nsw, nsw_label) in [(0.8f32, "default"), (0.5, "mid"), (0.2, "low")] {
        let wav = format!("./test_data/test_tts_vits_melo_tts_zh_en_nsw{nsw}.wav");
        run_vits_test(
            &TtsConfig {
                model: Some(TtsModel::Vits),
                path: Some(path.clone()),
                options: Some(serde_json::json!({
                    "num_threads": 2, "noise_scale": 0.667, "noise_scale_w": nsw,
                    "length_scale": 1.0, "speed": 1.0, "sid": 0, "debug": false,
                })),
                ..Default::default()
            },
            &audio_cfg,
            &wav,
        )
        .await?;
        let (samples, _sr): (wavers::Samples<f32>, i32) = wavers::read(&wav)?;
        info!(
            "nsw={nsw} ({nsw_label}): {}",
            analyze_audio(
                &samples,
                _sr as u32,
                std::time::Duration::ZERO,
                *TEST_TTS_TEXT_WEIGHT / 12.0
            )
        );
    }
    Ok(())
}

#[tokio::test]
#[traced_test]
#[ignore]
/// cargo test --test tts_test -- test_tts_vits_aishell3_scan_sid --ignored --nocapture
/// Scan speaker IDs 0–173 (step 10) to find well-performing SIDs.
async fn test_tts_vits_aishell3_scan_sid() -> anyhow::Result<()> {
    let path = ws_root()
        .join("data/tts/model/vits/aishell3/")
        .to_string_lossy()
        .into_owned();
    let audio_cfg = vits_audio_config();

    let mut rows: Vec<(i32, f64, f64, f64, usize, f64)> = Vec::new();

    for sid in (0..174).step_by(10) {
        let wav = format!("./test_data/aishell3_sid{sid}.wav");
        run_vits_test(
            &TtsConfig {
                model: Some(TtsModel::Vits),
                path: Some(path.clone()),
                options: Some(serde_json::json!({
                    "num_threads": 2, "noise_scale": 0.667, "noise_scale_w": 0.8,
                    "length_scale": 1.0, "speed": 1.0, "sid": sid, "debug": false,
                })),
                ..Default::default()
            },
            &audio_cfg,
            &wav,
        )
        .await?;
        let (samples, sr): (wavers::Samples<f32>, i32) = wavers::read(&wav)?;
        let diag = analyze_audio(
            &samples,
            sr as u32,
            std::time::Duration::ZERO,
            *TEST_TTS_TEXT_WEIGHT / 12.0,
        );
        let rms_db: f64 = (20.0f32
            * (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32)
                .sqrt()
                .log10()) as f64;
        info!("sid={sid}: {}", diag);
        info!("sid={sid}: rms_db={rms_db:.1}");
        rows.push((
            sid,
            diag.shimmer_pct,
            diag.dynamic_range_db,
            rms_db,
            diag.num_samples,
            diag.duration_secs,
        ));
    }

    rows.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
    info!("=== aishell3 SID scan summary (sorted by shimmer) ===");
    info!(
        "{:<6} {:<10} {:<12} {:<8} {:<10} {:<8}",
        "sid", "shimmer", "dr_db", "rms_db", "samples", "duration"
    );
    for (sid, shimmer, dr, rms, samples, dur) in &rows {
        info!("{sid:<6} {shimmer:<6.2}% ({dr:<6.1}dB) {rms:<6.1}dB  {samples:<8} {dur:<6.2}s");
    }
    Ok(())
}

#[tokio::test]
#[traced_test]
#[ignore]
/// cargo test --test tts_test -- test_tts_vits_zh_hf_theresa_scan_sid --ignored --nocapture
/// Scan speaker IDs 0–803 (step 20) to find well-performing SIDs.
async fn test_tts_vits_zh_hf_theresa_scan_sid() -> anyhow::Result<()> {
    let path = ws_root()
        .join("data/tts/model/vits/zh-hf-theresa/")
        .to_string_lossy()
        .into_owned();
    let audio_cfg = vits_audio_config();

    // collect rows for summary table
    let mut rows: Vec<(i32, f64, f64, f64, usize, f64)> = Vec::new(); // (sid, shimmer, dr, rms_db, samples, dur)

    for sid in (0..804).step_by(20) {
        let wav = format!("./test_data/zh-hf-theresa_sid{sid}.wav");
        run_vits_test(
            &TtsConfig {
                model: Some(TtsModel::Vits),
                path: Some(path.clone()),
                options: Some(serde_json::json!({
                    "num_threads": 2, "noise_scale": 0.667, "noise_scale_w": 0.8,
                    "length_scale": 1.0, "speed": 1.0, "sid": sid, "debug": false,
                })),
                ..Default::default()
            },
            &audio_cfg,
            &wav,
        )
        .await?;
        let (samples, sr): (wavers::Samples<f32>, i32) = wavers::read(&wav)?;
        let diag = analyze_audio(
            &samples,
            sr as u32,
            std::time::Duration::ZERO,
            *TEST_TTS_TEXT_WEIGHT / 12.0,
        );
        let rms_db: f64 = (20.0f32
            * (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32)
                .sqrt()
                .log10()) as f64;
        info!("sid={sid}: {}", diag);
        info!("sid={sid}: rms_db={rms_db:.1}");
        rows.push((
            sid,
            diag.shimmer_pct,
            diag.dynamic_range_db,
            rms_db,
            diag.num_samples,
            diag.duration_secs,
        ));
    }

    // print summary table sorted by shimmer ascending
    rows.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
    info!("=== SID scan summary (sorted by shimmer) ===");
    info!(
        "{:<6} {:<10} {:<12} {:<8} {:<10} {:<8}",
        "sid", "shimmer", "dr_db", "rms_db", "samples", "duration"
    );
    for (sid, shimmer, dr, rms, samples, dur) in &rows {
        info!("{sid:<6} {shimmer:<6.2}% ({dr:<6.1}dB) {rms:<6.1}dB  {samples:<8} {dur:<6.2}s");
    }
    Ok(())
}

#[tokio::test]
#[traced_test]
#[ignore]
/// cargo test --test tts_test -- test_tts_matcha_zh_baker_scan_ls --ignored --nocapture
async fn test_tts_matcha_zh_baker_scan_ls() -> anyhow::Result<()> {
    run_length_scale_scan(
        TtsModel::MatchaTts,
        "data/tts/model/matcha/matcha-icefall-zh-baker/",
        &vits_audio_config(),
        "matcha_zh_baker",
        &[1.2, 1.3, 1.4, 1.5],
        None,
    )
    .await
}

#[tokio::test]
#[traced_test]
#[ignore]
/// cargo test --test tts_test -- test_tts_vits_melo_tts_zh_en_scan_ls --ignored --nocapture
async fn test_tts_vits_melo_tts_zh_en_scan_ls() -> anyhow::Result<()> {
    run_length_scale_scan(
        TtsModel::Vits,
        "data/tts/model/vits/melo-tts-zh_en/",
        &vits_audio_config(),
        "melo_tts_zh_en",
        &[1.1, 1.2, 1.3, 1.4, 1.5],
        Some(0),
    )
    .await
}

#[tokio::test]
#[traced_test]
#[ignore]
/// cargo test --test tts_test -- test_tts_vits_zh_hf_theresa_scan_ls --ignored --nocapture
async fn test_tts_vits_zh_hf_theresa_scan_ls() -> anyhow::Result<()> {
    run_length_scale_scan(
        TtsModel::Vits,
        "data/tts/model/vits/zh-hf-theresa/",
        &vits_audio_config(),
        "zh_hf_theresa",
        &[1.4, 1.5, 1.6, 1.7, 1.8, 2.0],
        Some(0),
    )
    .await
}

#[tokio::test]
#[traced_test]
#[ignore]
/// cargo test --test tts_test -- test_tts_vits_aishell3_scan_ls --ignored --nocapture
async fn test_tts_vits_aishell3_scan_ls() -> anyhow::Result<()> {
    run_length_scale_scan(
        TtsModel::Vits,
        "data/tts/model/vits/aishell3/",
        &vits_audio_config(),
        "aishell3",
        &[0.5, 0.6, 0.7, 0.8, 0.9, 1.0],
        Some(0),
    )
    .await
}

fn tts_stream(
    text: String,
) -> impl Stream<Item = core::result::Result<String, ModelError>> + Unpin + Send + 'static {
    let (tx, rx) = channel::<core::result::Result<String, ModelError>>(10);
    thread::spawn(move || {
        block_on(async move {
            let _ = tx.send(Ok(text)).await;
            drop(tx);
        })
    });
    ReceiverStream::new(rx)
}
