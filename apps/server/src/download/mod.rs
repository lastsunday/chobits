use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use api::config::{AsrModel, Config as AppConfig, LlmModel, TtsModel, VadModel};
use include_dir::{Dir, include_dir};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use sha2::Digest;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

const MAX_CONCURRENT_DOWNLOADS: usize = 4;

static MANIFESTS: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/src/download/manifests");

const DEFAULT_MIRRORS: &[&str] = &["https://hf-mirror.com"];
const MANIFESTS_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/src/download/manifests");

#[derive(Deserialize)]
struct ModelEntry {
    default_variant: Option<String>,
    variants: HashMap<String, Variant>,
}

#[derive(Deserialize)]
struct Variant {
    files: Vec<FileEntry>,
}

#[derive(Deserialize)]
struct FileEntry {
    url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    sha256: Option<String>,
    path: String,
}

#[derive(Deserialize)]
struct OverrideEntry {
    url: String,
    #[serde(default)]
    sha256: Option<String>,
}

type OverrideMap = HashMap<String, OverrideEntry>;

#[derive(Serialize)]
struct Report {
    completed_at: String,
    base_dir: String,
    files: Vec<ReportFile>,
}

#[derive(Serialize)]
struct ReportFile {
    path: String,
    size: u64,
    sha256: String,
    status: String,
}

#[allow(clippy::too_many_arguments)]
pub async fn run(
    category: Option<&str>,
    model: Option<&str>,
    variant: Option<&str>,
    data_dir: &Path,
    quiet: bool,
    mirrors: &[String],
    overrides: Option<&str>,
    write_checksums: bool,
    config_path: Option<&PathBuf>,
) -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::new();

    let override_map = match overrides {
        Some(src) => Some(load_overrides(src).await?),
        None => None,
    };

    let config_targets: Option<Vec<(String, String, Option<String>)>> =
        match config_path {
            Some(cfg_path) => {
                let figment = AppConfig::load(std::slice::from_ref(cfg_path))?;
                let cfg = AppConfig::new(&figment)?;
                let targets = config_to_targets(&cfg);
                if !quiet {
                    eprintln!(
                        "Config selects {} model(s)",
                        targets.len()
                    );
                    for (cat, m, var) in &targets {
                        if let Some(v) = var {
                            eprintln!("  {cat}/{m} (variant: {v})");
                        } else {
                            eprintln!("  {cat}/{m} (default variant)");
                        }
                    }
                }
                if targets.is_empty() {
                    if !quiet {
                        eprintln!("Nothing to download");
                    }
                    return Ok(());
                }
                Some(targets)
            }
            None => None,
        };

    let mut report_files = Vec::new();

    // (manifest_rel_path, file.path, sha256) for write_checksums
    let mut sha_updates: Vec<(PathBuf, String, String)> = Vec::new();

    for cat_dir in MANIFESTS.dirs() {
        let cat_name = dir_name(cat_dir);
        if let Some(c) = category
            && c != cat_name
        {
            continue;
        }

        for file_entry in cat_dir.files() {
            if file_entry.path().extension().is_none_or(|e| e != "json") {
                continue;
            }

            let model_name = file_entry.path().file_stem().unwrap().to_str().unwrap();
            if let Some(m) = model
                && m != model_name
            {
                continue;
            }

            if let Some(ref targets) = config_targets
                && !targets
                    .iter()
                    .any(|(c, m, _)| c == cat_name && m == model_name)
            {
                continue;
            }

            let entry: ModelEntry = serde_json::from_slice(file_entry.contents())?;

            let effective_variant = variant.or_else(|| {
                config_targets.as_ref().and_then(|targets| {
                    targets
                        .iter()
                        .find(|(c, m, _)| c == cat_name && m == model_name)
                        .and_then(|(_, _, v)| v.as_deref())
                })
            });
            let variants = resolve_variants(&entry, effective_variant);
            let manifest_rel = file_entry.path().to_path_buf();
            for (v_name, v) in &variants {
                if !quiet {
                    eprintln!("[{cat_name}/{model_name}/{v_name}]");
                }

                let sem = Arc::new(Semaphore::new(MAX_CONCURRENT_DOWNLOADS));
                let mut set = JoinSet::new();

                for file in &v.files {
                    let dest = data_dir.join(&file.path);

                    let (file_url, file_sha256) = match override_map
                        .as_ref()
                        .and_then(|m| m.get(&file.path))
                    {
                        Some(ov) => (ov.url.clone(), ov.sha256.clone().or_else(|| file.sha256.clone())),
                        None => (file.url.clone(), file.sha256.clone()),
                    };

                    let cl = client.clone();
                    let mir = mirrors.to_vec();
                    let fpath = file.path.clone();
                    let man = manifest_rel.clone();
                    let sem = sem.clone();

                    set.spawn(async move {
                        let _permit = sem.acquire_owned().await.unwrap();
                        let result = download_file(&cl, &file_url, &dest, file_sha256.as_deref(), &mir)
                            .await
                            .map_err(|e| format!("{e}"));
                        (fpath, man, result)
                    });
                }

                while let Some(res) = set.join_next().await {
                    let (fpath, man, result) = res?;
                    match result {
                        Ok((size, sha256)) => {
                            report_files.push(ReportFile {
                                path: fpath.clone(),
                                size,
                                sha256: sha256.clone(),
                                status: "ok".into(),
                            });
                            sha_updates.push((man, fpath, sha256));
                        }
                        Err(msg) => {
                            if !quiet {
                                eprintln!("  FAIL: {} ({msg})", fpath);
                            }
                            report_files.push(ReportFile {
                                path: fpath,
                                size: 0,
                                sha256: String::new(),
                                status: format!("failed: {msg}"),
                            });
                        }
                    }
                }
            }
        }
    }

    let total = report_files.len();
    let ok = report_files.iter().filter(|f| f.status == "ok").count();
    let failed = total - ok;

    if write_checksums && !sha_updates.is_empty() {
        write_checksums_to_manifests(&sha_updates)?;
    }

    if !quiet {
        eprintln!(
            "\nDownload complete: {ok} OK, {failed} failed. Report written to {}/download-report.json",
            data_dir.display()
        );
    }

    let report = Report {
        completed_at: jiff::Zoned::now().to_string(),
        base_dir: data_dir.to_string_lossy().into(),
        files: report_files,
    };

    std::fs::create_dir_all(data_dir)?;
    std::fs::write(
        data_dir.join("download-report.json"),
        serde_json::to_string_pretty(&report)?,
    )?;

    if failed > 0 {
        return Err(format!("{failed} file(s) failed to download").into());
    }

    Ok(())
}

pub fn list(category: Option<&str>, json: bool) {
    #[derive(Serialize)]
    struct ModelInfo {
        model: String,
        default_variant: Option<String>,
        variants: Vec<String>,
    }

    let mut output: HashMap<String, Vec<ModelInfo>> = HashMap::new();

    for cat_dir in MANIFESTS.dirs() {
        let cat_name = dir_name(cat_dir).to_string();
        if let Some(c) = category
            && c != cat_name
        {
            continue;
        }

        let models = output.entry(cat_name).or_default();

        for file_entry in cat_dir.files() {
            if file_entry.path().extension().is_none_or(|e| e != "json") {
                continue;
            }

            let model_name = file_entry
                .path()
                .file_stem()
                .unwrap()
                .to_str()
                .unwrap()
                .to_string();
            let Ok(entry) = serde_json::from_slice::<ModelEntry>(file_entry.contents()) else {
                continue;
            };

            models.push(ModelInfo {
                model: model_name,
                default_variant: entry.default_variant,
                variants: entry.variants.into_keys().collect(),
            });
        }
    }

    if json {
        println!("{}", serde_json::to_string_pretty(&output).unwrap());
        return;
    }

    for (cat, models) in &output {
        println!("{cat}");
        let count = models.len();
        for (i, m) in models.iter().enumerate() {
            let is_last = i == count - 1;
            let prefix = if is_last {
                "  └── "
            } else {
                "  ├── "
            };
            let child_prefix = if is_last { "      " } else { "  │   " };

            let has_variants = m.variants.len() > 1 || !m.variants.contains(&"default".into());
            println!("{prefix}{}", m.model);

            if has_variants {
                let vcount = m.variants.len();
                for (j, v) in m.variants.iter().enumerate() {
                    let v_is_last = j == vcount - 1;
                    let v_prefix = if v_is_last {
                        "└── "
                    } else {
                        "├── "
                    };
                    let suffix = if m.default_variant.as_deref() == Some(v) {
                        " (default)"
                    } else {
                        ""
                    };
                    println!("{child_prefix}{v_prefix}{v}{suffix}");
                }
            }
        }
    }
}

async fn load_overrides(src: &str) -> Result<OverrideMap, Box<dyn std::error::Error>> {
    let bytes: Vec<u8> = if src.starts_with("http://") || src.starts_with("https://") {
        Client::new().get(src).send().await?.bytes().await?.to_vec()
    } else {
        std::fs::read(src)?
    };
    Ok(serde_json::from_slice(&bytes)?)
}

fn generate_urls(primary: &str, mirrors: &[String]) -> Vec<String> {
    let mut urls = vec![primary.to_string()];

    if let Some(path) = primary.strip_prefix("https://huggingface.co/") {
        let list: Vec<&str> = if mirrors.is_empty() {
            DEFAULT_MIRRORS.to_vec()
        } else {
            mirrors.iter().map(|s| s.as_str()).collect()
        };
        for m in list {
            urls.push(format!("{}/{}", m.trim_end_matches('/'), path));
        }
    }

    urls
}

async fn download_file(
    client: &Client,
    url: &str,
    dest: &Path,
    expected_sha256: Option<&str>,
    mirrors: &[String],
) -> Result<(u64, String), Box<dyn std::error::Error>> {
    if dest.exists() {
        let (size, actual) = sha256_file(dest)?;
        if let Some(expected) = expected_sha256 {
            if actual != expected {
                std::fs::remove_file(dest)?;
                eprintln!("  SHA256 mismatch, re-downloading {}", dest.display());
            } else {
                return Ok((size, actual));
            }
        } else {
            return Ok((size, actual));
        }
    }

    std::fs::create_dir_all(dest.parent().unwrap())?;
    let candidates = generate_urls(url, mirrors);

    for (i, candidate) in candidates.iter().enumerate() {
        if i > 0 {
            eprintln!("  RETRY from {candidate}");
        }

        let result = try_download_url(client, candidate, dest, expected_sha256).await;
        if result.is_ok() {
            return result;
        }
    }

    Err("All download attempts failed".into())
}

async fn try_download_url(
    client: &Client,
    url: &str,
    dest: &Path,
    expected_sha256: Option<&str>,
) -> Result<(u64, String), Box<dyn std::error::Error>> {
    let tmp = dest.with_extension("tmp");
    let mut hasher = sha2::Sha256::new();
    let mut downloaded = 0u64;

    let mut resp = client.get(url).send().await?;

    let mut file = std::fs::File::create(&tmp)?;
    while let Some(chunk) = resp.chunk().await? {
        hasher.update(&chunk);
        file.write_all(&chunk)?;
        downloaded += chunk.len() as u64;
    }

    let actual = hex::encode(hasher.finalize());

    if let Some(expected) = expected_sha256
        && actual != *expected
    {
        let _ = std::fs::remove_file(&tmp);
        return Err(format!("SHA256 mismatch: expected {expected}, got {actual}").into());
    }

    std::fs::rename(&tmp, dest)?;

    Ok((downloaded, actual))
}

fn sha256_file(path: &Path) -> Result<(u64, String), Box<dyn std::error::Error>> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = sha2::Sha256::new();
    let mut buf = [0u8; 8192];
    let mut total = 0u64;

    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        total += n as u64;
    }

    Ok((total, hex::encode(hasher.finalize())))
}

fn write_checksums_to_manifests(
    updates: &[(PathBuf, String, String)],
) -> Result<(), Box<dyn std::error::Error>> {
    let base = Path::new(MANIFESTS_DIR);

    let mut grouped: HashMap<&Path, Vec<(&str, &str)>> = HashMap::new();
    for (man, path, sha) in updates {
        grouped
            .entry(man.as_path())
            .or_default()
            .push((path.as_str(), sha.as_str()));
    }

    for (rel, entries) in &grouped {
        let full = base.join(rel);
        let mut val: serde_json::Value = serde_json::from_slice(&std::fs::read(&full)?)?;
        for (target_path, sha) in entries {
            set_sha256(&mut val, target_path, sha);
        }
        std::fs::write(&full, serde_json::to_string_pretty(&val)?)?;
    }

    Ok(())
}

fn set_sha256(val: &mut serde_json::Value, target: &str, sha: &str) {
    match val {
        serde_json::Value::Object(map) => {
            if let Some(serde_json::Value::String(p)) = map.get("path")
                && p == target
            {
                map.insert("sha256".into(), serde_json::Value::String(sha.into()));
                return;
            }
            for v in map.values_mut() {
                set_sha256(v, target, sha);
            }
        }
        serde_json::Value::Array(arr) => {
            for v in arr.iter_mut() {
                set_sha256(v, target, sha);
            }
        }
        _ => {}
    }
}

fn resolve_variants<'a>(
    entry: &'a ModelEntry,
    variant: Option<&'a str>,
) -> HashMap<&'a str, &'a Variant> {
    match variant {
        Some(v) => {
            let mut map = HashMap::new();
            if let Some(var) = entry.variants.get(v) {
                map.insert(v, var);
            }
            map
        }
        None => {
            if let Some(default) = entry.default_variant.as_deref() {
                let mut map = HashMap::new();
                if let Some(var) = entry.variants.get(default) {
                    map.insert(default, var);
                }
                map
            } else {
                entry
                    .variants
                    .iter()
                    .map(|(k, v)| (k.as_str(), v))
                    .collect()
            }
        }
    }
}

fn dir_name<'a>(dir: &'a Dir<'a>) -> &'a str {
    dir.path().file_name().unwrap().to_str().unwrap()
}

fn config_to_targets(
    config: &AppConfig,
) -> Vec<(String, String, Option<String>)> {
    let mut targets = Vec::new();

    match config.tts_model.clone().unwrap_or_default() {
        TtsModel::PocketTts => {
            targets.push(("tts".into(), "pocket-tts".into(), config.tts_variant.clone()));
        }
        TtsModel::Voxcpm => {
            targets.push(("tts".into(), "voxcpm".into(), config.tts_variant.clone()));
        }
        TtsModel::Mute => {}
    }

    match config.asr_model.clone().unwrap_or_default() {
        AsrModel::Qwen3 => {
            targets.push(("asr".into(), "qwen3".into(), config.asr_variant.clone()));
        }
        AsrModel::Whisper => {
            targets.push(("asr".into(), "whisper".into(), config.asr_variant.clone()));
        }
        AsrModel::Void => {}
    }

    match config.llm_model.clone().unwrap_or_default() {
        LlmModel::Qwen3 => {
            targets.push(("llm".into(), "qwen3".into(), config.llm_variant.clone()));
        }
        LlmModel::MiniCPM4 => {}
        LlmModel::Echo => {}
    }

    match config.vad_model.clone().unwrap_or_default() {
        VadModel::Silero => {
            targets.push(("vad".into(), "silero".into(), config.vad_variant.clone()));
        }
        VadModel::Earshot | VadModel::Void => {}
    }

    targets
}
