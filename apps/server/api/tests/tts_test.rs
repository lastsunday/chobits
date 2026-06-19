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

/// Test text covering Chinese, English, and numeric patterns (rule FST + OOV scenarios).
const TEST_TTS_TEXT: &str = "2024年5月11号，拨打110或者18920240511，花了99块钱。我在学习machine learning和artificial intelligence。";

/// Weight of `TEST_TTS_TEXT` in the OmniVoice RuleDurationEstimator weight system.
///
/// Uses Unicode-range-based phonetic weights (CJK=3.0, digit=3.5, latin=1.0,
/// space=0.2, punctuation=0.5). This is a model-independent measure of the
/// "speech content" in a text.
static TEST_TTS_TEXT_WEIGHT: LazyLock<f64> = LazyLock::new(|| {
    TEST_TTS_TEXT
        .chars()
        .map(|c| match c as u32 {
            0x30..=0x39 => 3.5,
            0x41..=0x5A | 0x61..=0x7A => 1.0,
            0xC0..=0x024F => 1.0,
            0x20 | 0x3000 => 0.2,
            0x21..=0x2F | 0x3A..=0x40 | 0x5B..=0x60 | 0x7B..=0x7E | 0x3001..=0x303F => 0.5,
            0x3040..=0x309F | 0x30A0..=0x30FF => 2.5,
            0xAC00..=0xD7AF | 0x1100..=0x11FF => 2.2,
            0x3400..=0x4DBF | 0x4E00..=0x9FFF => 3.0,
            _ => 1.0,
        })
        .sum::<f64>()
});

/// Estimate standard speech duration from text content alone.
///
/// Uses a fixed standard speed factor of 12.0 weight-units per second
/// (~4 CJK chars/sec or ~150 WPM English). The result is always the same
/// for the same text regardless of model or length_scale, providing a
/// consistent reference for cross-model comparison.
fn estimate_std_duration(text: &str) -> f64 {
    const STANDARD_SPEED_FACTOR: f64 = 12.0;
    let weight: f64 = text
        .chars()
        .map(|c| match c as u32 {
            0x30..=0x39 => 3.5,
            0x41..=0x5A | 0x61..=0x7A => 1.0,
            0xC0..=0x024F => 1.0,
            0x20 | 0x3000 => 0.2,
            0x21..=0x2F | 0x3A..=0x40 | 0x5B..=0x60 | 0x7B..=0x7E | 0x3001..=0x303F => 0.5,
            0x3040..=0x309F | 0x30A0..=0x30FF => 2.5,
            0xAC00..=0xD7AF | 0x1100..=0x11FF => 2.2,
            0x3400..=0x4DBF | 0x4E00..=0x9FFF => 3.0,
            _ => 1.0,
        })
        .sum();
    weight / STANDARD_SPEED_FACTOR
}

/// TTS 音频诊断结果。
#[derive(Debug)]
struct TtsAudioDiagnostics {
    num_samples: usize,
    duration_secs: f64,
    shimmer_pct: f64,
    dynamic_range_db: f64,
    gen_elapsed_secs: f64,
    rtf: f64,
    std_duration_secs: f64,
    std_diff_secs: f64,
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

    /// 音质评分 (0–100)。Shimmer 70% weight, dynamic range 30% weight。
    fn audio_score(&self) -> f64 {
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

    /// 性能评分 (0–100)。基于 RTF（实时因子）。
    fn performance_score(&self) -> f64 {
        match self.rtf {
            r if r < 0.1 => 100.0,
            r if r < 0.3 => lerp(100.0, 80.0, (r - 0.1) / 0.2),
            r if r < 0.5 => lerp(80.0, 60.0, (r - 0.3) / 0.2),
            r if r < 1.0 => lerp(60.0, 0.0, (r - 0.5) / 0.5),
            _ => 0.0,
        }
    }

    /// 语速评分 (0–100)。基于偏离标准时长的百分比。
    fn timing_score(&self) -> f64 {
        let deviation = (self.std_diff_secs / self.std_duration_secs).abs();
        match deviation {
            d if d < 0.05 => 100.0,
            d if d < 0.20 => lerp(100.0, 80.0, (d - 0.05) / 0.15),
            d if d < 0.50 => lerp(80.0, 40.0, (d - 0.20) / 0.30),
            d if d < 1.00 => lerp(40.0, 0.0, (d - 0.50) / 0.50),
            _ => 0.0,
        }
    }

    fn score_grade(score: f64) -> &'static str {
        if score >= 86.0 {
            "E"
        } else if score >= 66.0 {
            "G"
        } else if score >= 41.0 {
            "F"
        } else if score >= 21.0 {
            "P"
        } else {
            "B"
        }
    }

    fn audio_grade(&self) -> &'static str {
        Self::score_grade(self.audio_score())
    }

    fn performance_grade(&self) -> &'static str {
        Self::score_grade(self.performance_score())
    }

    fn timing_grade(&self) -> &'static str {
        Self::score_grade(self.timing_score())
    }

    /// Overall usability verdict based on audio quality.
    fn verdict(&self) -> &'static str {
        match self.shimmer_pct {
            s if s >= 10.0 => {
                "Unsuitable for daily use — shimmer exceeds algorithm reliability limit"
            }
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
        let dev_pct = self.std_diff_secs / self.std_duration_secs * 100.0;
        write!(
            f,
            "Audio:scr={:.0}({}) Perf:scr={:.0}({}) Timing:scr={:.0}({}) | \
             sh={:.2}%({}) dr={:.1}dB({}) rtf={:.2} gen={:.1}s dur={:.2}s(std={:.1}s{:+.0}%) {}",
            self.audio_score(),
            self.audio_grade(),
            self.performance_score(),
            self.performance_grade(),
            self.timing_score(),
            self.timing_grade(),
            self.shimmer_pct,
            self.shimmer_grade(),
            self.dynamic_range_db,
            self.dr_grade(),
            self.rtf,
            self.gen_elapsed_secs,
            self.duration_secs,
            self.std_duration_secs,
            dev_pct,
            self.verdict(),
        )
    }
}

/// 对解码后 PCM 做完整音频诊断。
fn analyze_audio(
    samples: &[f32],
    sample_rate: u32,
    gen_elapsed: std::time::Duration,
    std_duration_secs: f64,
) -> TtsAudioDiagnostics {
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
        gen_elapsed_secs: gen_elapsed.as_secs_f64(),
        rtf: gen_elapsed.as_secs_f64() / (samples.len() as f64 / sample_rate as f64),
        std_duration_secs,
        std_diff_secs: samples.len() as f64 / sample_rate as f64 - std_duration_secs,
    }
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
