use std::fmt;
use std::path::Path;
use std::sync::LazyLock;
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
async fn run_vits_test(tts_config: &TtsConfig, audio_config: &AudioConfig, wav: &str) -> anyhow::Result<()> {
    let tts = TtsFactory::create_model(tts_config, audio_config).await?;
    let text_stream = tts_stream(String::from(TEST_TTS_TEXT));
    let mut tts_stream = tts.stream(Box::pin(text_stream)).await;

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
    anyhow::ensure!(!all_packets.is_empty(), "Expected audio packets from VitsTTS");

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
    info!("{}", analyze_audio(&decoded, 16000));

    std::fs::create_dir_all("./test_data")?;
    let _ = wavers::write(wav, &decoded, 16000, 1);
    Ok(())
}

#[tokio::test]
#[traced_test]
#[ignore]
/// cargo test --test tts_test -- test_tts_vits_melo_tts_zh_en --ignored --nocapture
/// 先下载模型：
///   cargo run --bin chobits-server -- downloader install tts vits melo-tts-zh_en --all
async fn test_tts_vits_melo_tts_zh_en() -> anyhow::Result<()> {
    let path = ws_root().join("data/tts/model/vits/melo-tts-zh_en/").to_string_lossy().into_owned();
    run_vits_test(
        &TtsConfig { model: Some(TtsModel::Vits), path: Some(path), options: Some(serde_json::json!({
    "num_threads": 2,
    "noise_scale": 0.667,
    "noise_scale_w": 0.8,
    "length_scale": 1.0,
    "speed": 1.0,
    "sid": 0,
    "debug": false,
})), ..Default::default() },
        &vits_audio_config(),
        "./test_data/test_tts_vits_melo_tts_zh_en.wav",
    ).await
}

#[tokio::test]
#[traced_test]
#[ignore]
/// cargo test --test tts_test -- test_tts_vits_zh_hf_theresa --ignored --nocapture
/// 先下载模型：
///   cargo run --bin chobits-server -- downloader install tts vits zh-hf-theresa --all
async fn test_tts_vits_zh_hf_theresa() -> anyhow::Result<()> {
    let path = ws_root().join("data/tts/model/vits/zh-hf-theresa/").to_string_lossy().into_owned();
    run_vits_test(
        &TtsConfig { model: Some(TtsModel::Vits), path: Some(path), options: Some(serde_json::json!({
    "num_threads": 2,
    "noise_scale": 0.667,
    "noise_scale_w": 0.8,
    "length_scale": 1.0,
    "speed": 1.0,
    "sid": 0,
    "debug": false,
})), ..Default::default() },
        &vits_audio_config(),
        "./test_data/test_tts_vits_zh_hf_theresa.wav",
    ).await
}

#[tokio::test]
#[traced_test]
#[ignore]
/// cargo test --test tts_test -- test_tts_vits_aishell3 --ignored --nocapture
/// 先下载模型：
///   cargo run --bin chobits-server -- downloader install tts vits aishell3 --all
async fn test_tts_vits_aishell3() -> anyhow::Result<()> {
    let path = ws_root().join("data/tts/model/vits/aishell3/").to_string_lossy().into_owned();
    run_vits_test(
        &TtsConfig { model: Some(TtsModel::Vits), path: Some(path), options: Some(serde_json::json!({
    "num_threads": 2,
    "noise_scale": 0.667,
    "noise_scale_w": 0.8,
    "length_scale": 1.0,
    "speed": 1.0,
    "sid": 0,
    "debug": false,
})), ..Default::default() },
        &vits_audio_config(),
        "./test_data/test_tts_vits_aishell3.wav",
    ).await
}

/// Test text covering Chinese, English, and numeric patterns (rule FST + OOV scenarios).
const TEST_TTS_TEXT: &str = "2024年5月11号，拨打110或者18920240511，花了99块钱。我在学习machine learning和artificial intelligence。";

/// TTS 音频诊断结果。
#[derive(Debug)]
struct TtsAudioDiagnostics {
    num_samples: usize,
    duration_secs: f64,
    shimmer_pct: f64,
    dynamic_range_db: f64,
}

impl TtsAudioDiagnostics {
    fn shimmer_grade(&self) -> &'static str {
        match self.shimmer_pct {
            s if s < 3.81 => "Excellent",
            s if s < 5.0 => "Good",
            s if s < 6.0 => "Fair",
            s if s < 10.0 => "Poor",
            _ => "Bad",
        }
    }

    fn dr_grade(&self) -> &'static str {
        match self.dynamic_range_db {
            d if d > 20.0 => "Good",
            d if d > 15.0 => "Fair",
            _ => "Poor",
        }
    }

    /// Composite score (0–100). Shimmer 70% weight, dynamic range 30% weight, linear interpolation within each tier.
    fn score(&self) -> f64 {
        let s = if self.shimmer_pct < 3.81 {
            100.0
        } else if self.shimmer_pct < 5.0 {
            lerp(100.0, 75.0, (self.shimmer_pct - 3.81) / (5.0 - 3.81))
        } else if self.shimmer_pct < 6.0 {
            lerp(75.0, 50.0, (self.shimmer_pct - 5.0) / (6.0 - 5.0))
        } else if self.shimmer_pct < 10.0 {
            lerp(50.0, 25.0, (self.shimmer_pct - 6.0) / (10.0 - 6.0))
        } else {
            0.0
        };
        let d = if self.dynamic_range_db > 20.0 {
            100.0
        } else if self.dynamic_range_db > 15.0 {
            lerp(0.0, 100.0, (self.dynamic_range_db - 15.0) / (20.0 - 15.0))
        } else {
            0.0
        };
        s * 0.7 + d * 0.3
    }

    /// Overall usability verdict.
    fn verdict(&self) -> &'static str {
        match self.shimmer_pct {
            s if s >= 10.0 => "Unsuitable for daily use — shimmer exceeds algorithm reliability limit",
            s if s >= 6.0 => "Marginal — shimmer in pathological range (>6%), noticeable roughness",
            s if s >= 5.0 => "Marginal — shimmer in warning zone (5–6%), slight tremor",
            _ => match self.dynamic_range_db {
                d if d < 10.0 => "Marginal — dynamic range too low (<10dB), flat audio",
                d if d < 15.0 => "Marginal — dynamic range narrow (10–15dB), compressed sound",
                _ => "Suitable for daily use — all indicators within normal range",
            },
        }
    }
}

/// 线性插值：t ∈ [0, 1] 时返回 start 到 end 之间的值。
fn lerp(start: f64, end: f64, t: f64) -> f64 {
    start + (end - start) * t.clamp(0.0, 1.0)
}

impl fmt::Display for TtsAudioDiagnostics {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "shimmer={:.2}% ({}), dynamic_range={:.1}dB ({}), score={:.0}/100, samples={}, duration={:.2}s  {}",
            self.shimmer_pct,
            self.shimmer_grade(),
            self.dynamic_range_db,
            self.dr_grade(),
            self.score(),
            self.num_samples,
            self.duration_secs,
            self.verdict(),
        )
    }
}

/// 对解码后 PCM 做完整音频诊断。
fn analyze_audio(samples: &[f32], sample_rate: u32) -> TtsAudioDiagnostics {
    let window = 160; // 10ms @ 16kHz
    let mut rms: Vec<f32> = samples
        .chunks(window)
        .map(|chunk| {
            let sq_sum: f32 = chunk.iter().map(|s| s * s).sum();
            (sq_sum / chunk.len() as f32).sqrt()
        })
        .collect();

    // 去除静音帧
    let peak = rms.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    rms.retain(|r| *r > peak * 0.05);

    // shimmer
    let shimmer_pct = if rms.len() < 2 {
        0.0
    } else {
        let mean = rms.iter().sum::<f32>() / rms.len() as f32;
        if mean < 1e-10 {
            0.0
        } else {
            let sum_diff: f32 = rms.windows(2).map(|w| (w[1] - w[0]).abs()).sum();
            let mean_diff = sum_diff / (rms.len() - 1) as f32;
            (mean_diff / mean * 100.0) as f64
        }
    };

    // dynamic range
    let dynamic_range_db = if rms.len() < 2 {
        0.0
    } else {
        let max_rms = rms.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        let min_rms = rms.iter().cloned().fold(f32::INFINITY, f32::min);
        if min_rms < 1e-10 {
            0.0
        } else {
            (20.0 * (max_rms / min_rms).log10()) as f64
        }
    };

    TtsAudioDiagnostics {
        num_samples: samples.len(),
        duration_secs: samples.len() as f64 / sample_rate as f64,
        shimmer_pct,
        dynamic_range_db,
    }
}

#[tokio::test]
#[traced_test]
#[ignore]
/// cargo test --test tts_test -- test_tts_vits_melo_tts_zh_en_noise_scale --ignored --nocapture
async fn test_tts_vits_melo_tts_zh_en_noise_scale() -> anyhow::Result<()> {
    let path = ws_root().join("data/tts/model/vits/melo-tts-zh_en/").to_string_lossy().into_owned();
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
        ).await?;
        let (samples, _sr): (wavers::Samples<f32>, i32) = wavers::read(&wav)?;
        info!("ns={ns} ({ns_label}): {}", analyze_audio(&samples, _sr as u32));
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
        ).await?;
        let (samples, _sr): (wavers::Samples<f32>, i32) = wavers::read(&wav)?;
        info!("nsw={nsw} ({nsw_label}): {}", analyze_audio(&samples, _sr as u32));
    }
    Ok(())
}

#[tokio::test]
#[traced_test]
#[ignore]
/// cargo test --test tts_test -- test_tts_vits_aishell3_scan_sid --ignored --nocapture
/// Scan speaker IDs 0–173 (step 10) to find well-performing SIDs.
async fn test_tts_vits_aishell3_scan_sid() -> anyhow::Result<()> {
    let path = ws_root().join("data/tts/model/vits/aishell3/").to_string_lossy().into_owned();
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
        ).await?;
        let (samples, sr): (wavers::Samples<f32>, i32) = wavers::read(&wav)?;
        let diag = analyze_audio(&samples, sr as u32);
        let rms_db: f64 = (20.0f32 * (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32).sqrt().log10()) as f64;
        info!("sid={sid}: {}", diag);
        info!("sid={sid}: rms_db={rms_db:.1}");
        rows.push((sid, diag.shimmer_pct, diag.dynamic_range_db, rms_db, diag.num_samples, diag.duration_secs));
    }

    rows.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
    info!("=== aishell3 SID scan summary (sorted by shimmer) ===");
    info!("{:<6} {:<10} {:<12} {:<8} {:<10} {:<8}", "sid", "shimmer", "dr_db", "rms_db", "samples", "duration");
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
    let path = ws_root().join("data/tts/model/vits/zh-hf-theresa/").to_string_lossy().into_owned();
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
        ).await?;
        let (samples, sr): (wavers::Samples<f32>, i32) = wavers::read(&wav)?;
        let diag = analyze_audio(&samples, sr as u32);
        let rms_db: f64 = (20.0f32 * (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32).sqrt().log10()) as f64;
        info!("sid={sid}: {}", diag);
        info!("sid={sid}: rms_db={rms_db:.1}");
        rows.push((sid, diag.shimmer_pct, diag.dynamic_range_db, rms_db, diag.num_samples, diag.duration_secs));
    }

    // print summary table sorted by shimmer ascending
    rows.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
    info!("=== SID scan summary (sorted by shimmer) ===");
    info!("{:<6} {:<10} {:<12} {:<8} {:<10} {:<8}", "sid", "shimmer", "dr_db", "rms_db", "samples", "duration");
    for (sid, shimmer, dr, rms, samples, dur) in &rows {
        info!("{sid:<6} {shimmer:<6.2}% ({dr:<6.1}dB) {rms:<6.1}dB  {samples:<8} {dur:<6.2}s");
    }
    Ok(())
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
