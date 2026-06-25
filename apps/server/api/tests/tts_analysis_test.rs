use std::path::Path;

use api::{
    config::{TtsModel, audio::AudioConfig},
    util::compressor::{adaptive_normalize, evaluate_compressed, grid_search_compressor},
};
use sherpa_onnx::{
    GenerationConfig, OfflineTts, OfflineTtsConfig, OfflineTtsModelConfig,
    OfflineTtsVitsModelConfig,
};
use tracing::info;
use tracing_test::traced_test;

mod common;
use common::tts::*;

/// Scan length_scale values to find optimal timing (duration closest to standard).
async fn run_length_scale_scan(
    model: TtsModel,
    dir: &str,
    audio_cfg: &AudioConfig,
    wav_prefix: &str,
    ls_values: &[f32],
    sid: Option<i32>,
) -> anyhow::Result<()> {
    let path = ws_root().join(dir).to_string_lossy().into_owned();
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
            &api::config::tts::TtsConfig {
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
    info!(
        "{:<8} {:<10} {:<10} {:<10}",
        "ls", "duration", "dev_abs", "dev_pct"
    );
    for &(ls, dur, dev, dev_pct) in &rows {
        info!("{ls:<8.3} {dur:<6.2}s   {dev:<6.2}s   {dev_pct:<6.1}%");
    }
    if let Some(best) = rows.first() {
        info!(
            "Best length_scale: {:.3} (dev={:.2}s, {:.1}%)",
            best.0, best.2, best.3
        );
    }
    Ok(())
}

// ─── Comparison ───

#[tokio::test]
#[traced_test]
async fn test_compare_raw_vs_processed() -> anyhow::Result<()> {
    let model_dir_buf = ws_root().join("data/tts/model/vits/melo-tts-zh_en/");
    let model_dir = Path::new(&model_dir_buf);

    let config = OfflineTtsConfig {
        model: OfflineTtsModelConfig {
            vits: OfflineTtsVitsModelConfig {
                model: Some(model_dir.join("model.onnx").to_string_lossy().into_owned()),
                tokens: Some(model_dir.join("tokens.txt").to_string_lossy().into_owned()),
                lexicon: Some(model_dir.join("lexicon.txt").to_string_lossy().into_owned()),
                noise_scale: 0.667,
                noise_scale_w: 0.8,
                length_scale: 1.0,
                ..Default::default()
            },
            num_threads: 2,
            ..Default::default()
        },
        rule_fsts: collect_rule_fsts(model_dir),
        ..Default::default()
    };
    let tts = OfflineTts::create(&config).expect("Failed to create OfflineTts");
    let sample_rate = tts.sample_rate() as u32;
    info!("Model sample rate: {} Hz", sample_rate);

    let audio = tts
        .generate_with_config(
            TEST_TTS_TEXT,
            &GenerationConfig {
                speed: 1.0,
                sid: 0,
                ..Default::default()
            },
            None::<fn(&[f32], f32) -> bool>,
        )
        .expect("Generation failed");
    let raw_samples = audio.samples().to_vec();
    let raw_sr = audio.sample_rate();
    info!("Raw audio: {} samples at {} Hz", raw_samples.len(), raw_sr);

    let encode_sr = 16000u32;
    std::fs::create_dir_all("./test_data")?;

    wavers::write("./test_data/compare_raw.wav", &raw_samples, raw_sr, 1)?;
    let processed = opus_pipeline(&raw_samples, raw_sr, encode_sr);
    wavers::write(
        "./test_data/compare_processed.wav",
        &processed,
        encode_sr as i32,
        1,
    )?;

    let adaptive = adaptive_normalize(&raw_samples, raw_sr as u32);
    let adp_decoded = opus_pipeline(&adaptive, raw_sr, encode_sr);
    wavers::write(
        "./test_data/compare_adaptive.wav",
        &adp_decoded,
        encode_sr as i32,
        1,
    )?;

    info!("--- EBU R128 Metrics ---");
    if let Ok(m) = evaluate_compressed(&raw_samples, raw_sr as u32) {
        info!(
            "  Raw:      LRA={:.2} LU, LUFS={:.2}, Crest={:.1} dB",
            m.lra, m.lufs, m.crest_factor_db
        );
    }
    if let Ok(m) = evaluate_compressed(&adaptive, raw_sr as u32) {
        info!(
            "  Adaptive: LRA={:.2} LU, LUFS={:.2}, Crest={:.1} dB",
            m.lra, m.lufs, m.crest_factor_db
        );
    }

    info!("=== Done: compare_raw, compare_processed, compare_adaptive ===");
    Ok(())
}

#[tokio::test]
#[traced_test]
async fn test_grid_search_compressor() -> anyhow::Result<()> {
    let model_dir = ws_root().join("data/tts/model/vits/melo-tts-zh_en/");
    let model_path = Path::new(&model_dir);

    let config = OfflineTtsConfig {
        model: OfflineTtsModelConfig {
            vits: OfflineTtsVitsModelConfig {
                model: Some(model_path.join("model.onnx").to_string_lossy().into_owned()),
                tokens: Some(model_path.join("tokens.txt").to_string_lossy().into_owned()),
                lexicon: Some(
                    model_path
                        .join("lexicon.txt")
                        .to_string_lossy()
                        .into_owned(),
                ),
                noise_scale: 0.667,
                noise_scale_w: 0.8,
                length_scale: 1.0,
                ..Default::default()
            },
            num_threads: 2,
            ..Default::default()
        },
        rule_fsts: collect_rule_fsts(model_path),
        ..Default::default()
    };
    let tts = OfflineTts::create(&config).expect("Failed to create OfflineTts");
    let sample_rate = tts.sample_rate() as u32;
    info!("Model sample rate: {} Hz", sample_rate);

    let audio = tts
        .generate_with_config(
            TEST_TTS_TEXT,
            &GenerationConfig {
                speed: 1.0,
                sid: 0,
                ..Default::default()
            },
            None::<fn(&[f32], f32) -> bool>,
        )
        .expect("Generation failed");
    let raw_samples = audio.samples().to_vec();
    let raw_sr = audio.sample_rate();
    info!("Raw audio: {} samples at {} Hz", raw_samples.len(), raw_sr);

    let raw_metrics = evaluate_compressed(&raw_samples, raw_sr as u32)?;
    info!(
        "Raw (uncompressed): LRA={:.2} LU, LUFS={:.2}, Crest={:.1} dB",
        raw_metrics.lra, raw_metrics.lufs, raw_metrics.crest_factor_db,
    );

    let results = grid_search_compressor(&raw_samples, raw_sr as u32)?;

    info!("=== Grid Search Results (top 10) ===");
    for (i, (cfg, metrics)) in results.iter().enumerate().take(10) {
        info!(
            "#{}: threshold={:.0} ratio={:.0} knee={:.0} attack={:.0} release={:.0} makeup={:.0} | LRA={:.2} LUFS={:.2} Crest={:.1}",
            i + 1,
            cfg.threshold_db,
            cfg.ratio,
            cfg.knee_db,
            cfg.attack_ms,
            cfg.release_ms,
            cfg.makeup_gain_db,
            metrics.lra,
            metrics.lufs,
            metrics.crest_factor_db,
        );
    }

    if let Some((best_cfg, best_metrics)) = results.first() {
        info!("=== Best Compressor Config ===");
        info!(
            r#"compressor = {{ threshold_db = {}, ratio = {}, attack_ms = {}, release_ms = {}, makeup_gain_db = {}, knee_db = {} }}"#,
            best_cfg.threshold_db,
            best_cfg.ratio,
            best_cfg.attack_ms,
            best_cfg.release_ms,
            best_cfg.makeup_gain_db,
            best_cfg.knee_db,
        );
        info!(
            "Improvement: LRA {:.2} -> {:.2} LU ({:.0}% reduction)",
            raw_metrics.lra,
            best_metrics.lra,
            (1.0 - best_metrics.lra / raw_metrics.lra) * 100.0,
        );
    }

    Ok(())
}

// ─── Noise / speaker parameter scans ───

#[tokio::test]
#[traced_test]
async fn test_tts_vits_melo_tts_zh_en_noise_scale() -> anyhow::Result<()> {
    let path = ws_root()
        .join("data/tts/model/vits/melo-tts-zh_en/")
        .to_string_lossy()
        .into_owned();
    let audio_cfg = vits_audio_config();

    for (ns, ns_label) in [(0.667f32, "default"), (0.5, "mid"), (0.3, "low")] {
        let wav = format!("./test_data/test_tts_vits_melo_tts_zh_en_ns{ns}.wav");
        run_vits_test(
            &api::config::tts::TtsConfig {
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
            &api::config::tts::TtsConfig {
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
            &api::config::tts::TtsConfig {
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
async fn test_tts_vits_zh_hf_theresa_scan_sid() -> anyhow::Result<()> {
    let path = ws_root()
        .join("data/tts/model/vits/zh-hf-theresa/")
        .to_string_lossy()
        .into_owned();
    let audio_cfg = vits_audio_config();

    let mut rows: Vec<(i32, f64, f64, f64, usize, f64)> = Vec::new();

    for sid in (0..804).step_by(20) {
        let wav = format!("./test_data/zh-hf-theresa_sid{sid}.wav");
        run_vits_test(
            &api::config::tts::TtsConfig {
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

// ─── Length_scale scans ───

#[tokio::test]
#[traced_test]
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
async fn test_tts_matcha_zh_en_scan_ls() -> anyhow::Result<()> {
    run_length_scale_scan(
        TtsModel::MatchaTts,
        "data/tts/model/matcha/matcha-icefall-zh-en/",
        &vits_audio_config(),
        "matcha_zh_en",
        &[1.2, 1.3, 1.4, 1.5],
        None,
    )
    .await
}

#[tokio::test]
#[traced_test]
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
