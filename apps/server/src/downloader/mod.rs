use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use api::config::{AsrModel, Config as AppConfig, LlmModel, TtsModel, VadModel};
use dialoguer::Select;
use indicatif::{ProgressBar, ProgressStyle};
use include_dir::{Dir, include_dir};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use sha2::Digest;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

const MAX_CONCURRENT_DOWNLOADS: usize = 4;

static MANIFESTS: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/src/downloader/manifests");

const DEFAULT_MIRRORS: &[&str] = &["https://hf-mirror.com"];
const MANIFESTS_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/src/downloader/manifests");

#[derive(Deserialize)]
#[allow(dead_code)]
struct ConfigMeta {
    category: String,
    #[serde(rename = "model")]
    model_name: String,
}

#[derive(Deserialize)]
struct ModelEntry {
    config: Option<ConfigMeta>,
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

    let cfg_path = config_path
        .map(|p| p.clone())
        .or_else(|| find_config().filter(|p| p.exists()));

    let figment = match &cfg_path {
        Some(p) => AppConfig::load(std::slice::from_ref(p))?,
        None => AppConfig::load(&[] as &[std::path::PathBuf])?,
    };
    let cfg = AppConfig::new(&figment)?;
    let targets = config_to_targets(&cfg);

    if targets.is_empty() {
        if !quiet {
            eprintln!("No enabled models in configuration. Nothing to download.");
        }
        return Ok(());
    }

    if !quiet {
        eprintln!("Config selects {} model(s)", targets.len());
        for (cat, m, var) in &targets {
            if let Some(v) = var {
                eprintln!("  {cat}/{m} (variant: {v})");
            } else {
                eprintln!("  {cat}/{m} (default variant)");
            }
        }
        eprintln!();
    }

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

            if !targets
                .iter()
                .any(|(c, m, _)| c == cat_name && m == model_name)
            {
                continue;
            }

            let entry: ModelEntry = serde_json::from_slice(file_entry.contents())?;

            let effective_variant = variant.or_else(|| {
                targets
                    .iter()
                    .find(|(c, m, _)| c == cat_name && m == model_name)
                    .and_then(|(_, _, v)| v.as_deref())
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
                        let result = download_file(&cl, &file_url, &dest, file_sha256.as_deref(), &mir, quiet)
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
    quiet: bool,
) -> Result<(u64, String), Box<dyn std::error::Error>> {
    if dest.exists() {
        let (size, actual) = sha256_file(dest)?;
        if let Some(expected) = expected_sha256 {
            if actual != expected {
                std::fs::remove_file(dest)?;
                if !quiet {
                    eprintln!("  SHA256 mismatch, re-downloading {}", dest.display());
                }
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
        if i > 0 && !quiet {
            eprintln!("  RETRY from {candidate}");
        }

        let result = try_download_url(client, candidate, dest, expected_sha256, quiet).await;
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
    quiet: bool,
) -> Result<(u64, String), Box<dyn std::error::Error>> {
    let tmp = dest.with_extension("tmp");
    let mut hasher = sha2::Sha256::new();
    let mut downloaded = 0u64;

    let mut resp = client.get(url).send().await?;
    let total_size = resp.content_length().unwrap_or(0);

    let pb = if !quiet && total_size > 0 {
        let pb = ProgressBar::new(total_size);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("{msg:.bold.dim} {bar:40.cyan/blue} {bytes}/{total_bytes} ({bytes_per_sec}, eta {eta})")
                .unwrap()
                .progress_chars("=>-"),
        );
        pb.set_message(
            dest.file_name()
                .unwrap_or_default()
                .to_str()
                .unwrap_or("")
                .to_string(),
        );
        Some(pb)
    } else {
        None
    };

    let mut file = std::fs::File::create(&tmp)?;
    while let Some(chunk) = resp.chunk().await? {
        hasher.update(&chunk);
        file.write_all(&chunk)?;
        downloaded += chunk.len() as u64;
        if let Some(ref pb) = pb {
            pb.set_position(downloaded);
        }
    }

    if let Some(pb) = pb {
        pb.finish_and_clear();
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

struct ModelInfo {
    display: String,
    toml_model: String,
    default_variant: String,
    variants: Vec<String>,
}

fn find_config_inner(chobits_config: Option<String>) -> Option<PathBuf> {
    if let Some(path) = chobits_config {
        let p = PathBuf::from(&path);
        if p.exists() {
            return Some(p);
        }
    }
    let p = PathBuf::from("application.toml");
    Some(if p.exists() { p } else { PathBuf::from("application.toml") })
}

fn find_config() -> Option<PathBuf> {
    find_config_inner(std::env::var("CHOBITS_CONFIG").ok())
}

fn load_selections(path: &Path) -> HashMap<String, String> {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return HashMap::new(),
    };
    let mut map = HashMap::new();
    let mut in_global = false;
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_global = line.trim_end() == "[global]";
            continue;
        }
        if !in_global || !line.contains('=') || line.starts_with('#') {
            continue;
        }
        if let Some(eq_pos) = line.find('=') {
            let key = line[..eq_pos].trim();
            let val = line[eq_pos + 1..].trim().trim_matches('"');
            if key.ends_with("_model") || key.ends_with("_variant") || key.ends_with("_path") {
                map.insert(key.to_string(), val.to_string());
            }
        }
    }
    map
}

fn upsert_config(path: &Path, updates: &[(&str, &str)]) -> Result<(), Box<dyn std::error::Error>> {
    let content = std::fs::read_to_string(path).unwrap_or_default();
    let mut lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();
    let mut updated = std::collections::HashSet::new();
    let mut in_global = false;
    let mut i = 0;

    while i < lines.len() {
        let trimmed = lines[i].trim().to_string();
        if trimmed.starts_with('[') {
            in_global = trimmed.trim_end() == "[global]";
            i += 1;
            continue;
        }
        if in_global && trimmed.contains('=') && !trimmed.starts_with('#') {
            if let Some(eq_pos) = trimmed.find('=') {
                let key = trimmed[..eq_pos].trim().to_string();
                if let Some(idx) = updates.iter().position(|(k, _)| *k == key.as_str()) {
                    lines[i] = format!("{key} = \"{}\"", updates[idx].1);
                    updated.insert(key);
                }
            }
        }
        i += 1;
    }

    let has_global = lines.iter().any(|l| l.trim().trim_end() == "[global]");
    if !updates.is_empty() && updated.len() < updates.len() {
        let insert_pos = if has_global {
            lines.iter().position(|l| l.trim().trim_end() == "[global]").unwrap() + 1
        } else {
            lines.len()
        };
        if !has_global {
            lines.insert(insert_pos, "[global]".into());
        }
        let mut offset = if has_global { 0 } else { 1 };
        for (key, val) in updates {
            if !updated.contains(*key) {
                lines.insert(insert_pos + offset, format!("{key} = \"{val}\""));
                offset += 1;
            }
        }
    } else if updated.is_empty() && !updates.is_empty() {
        lines.push(String::new());
        lines.push("[global]".into());
        for (key, val) in updates {
            lines.push(format!("{key} = \"{val}\""));
        }
    }

    std::fs::write(path, lines.join("\n") + "\n")?;
    Ok(())
}

pub async fn run_wizard(
    data_dir: &Path,
    quiet: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut catalog: Vec<(String, Vec<ModelInfo>)> = Vec::new();
    for cat_dir in MANIFESTS.dirs() {
        let cat_name = dir_name(cat_dir).to_string();
        let mut models = Vec::new();
        for file_entry in cat_dir.files() {
            if file_entry.path().extension().is_none_or(|e| e != "json") {
                continue;
            }
            let Ok(entry) = serde_json::from_slice::<ModelEntry>(file_entry.contents()) else {
                continue;
            };
            let display = file_entry.path().file_stem().unwrap().to_str().unwrap().to_string();
            let toml_model = entry.config.as_ref().map(|c| c.model_name.clone()).unwrap_or_else(|| display.clone());
            let default_variant = entry.default_variant.unwrap_or_else(|| "default".into());
            let variants: Vec<String> = entry.variants.keys().cloned().collect();
            models.push(ModelInfo { display, toml_model, default_variant, variants });
        }
        if !models.is_empty() {
            catalog.push((cat_name, models));
        }
    }

    // Find config & load existing selections
    let config_path = find_config();
    if let Some(ref p) = config_path {
        if p.exists() {
            println!("\nFound config: {}", p.display());
        } else {
            println!("\nConfig file: {} (will create)", p.display());
        }
    }
    let existing = config_path.as_ref().map(|p| load_selections(p)).unwrap_or_default();

    // Selection state: category → (manifest_name, toml_model, variant)
    let mut selections: HashMap<String, (String, String, String)> = HashMap::new();
    let cat_names: Vec<String> = catalog.iter().map(|(c, _)| c.clone()).collect();

    // Pre-populate from existing config
    for (key, val) in &existing {
        if let Some(cat) = key.strip_suffix("_model") {
            if let Some(model_info) = catalog.iter().find(|(c, _)| c == cat).and_then(|(_, models)| {
                models.iter().find(|m| m.toml_model == *val)
            }) {
                let variant = existing.get(&format!("{cat}_variant")).cloned().unwrap_or_else(|| model_info.default_variant.clone());
                selections.insert(cat.to_string(), (model_info.display.clone(), model_info.toml_model.clone(), variant));
            }
        }
    }

    // Display catalog
    println!("\nAvailable models:\n");
    for (cat_name, models) in &catalog {
        println!("  {cat_name}:");
        for m in models {
            let selected = selections.contains_key(cat_name);
            let sel = if selected { " [SELECTED]" } else { "" };
            println!("    {} (config model: {}){}", m.display, m.toml_model, sel);
            for v in &m.variants {
                let def = if *v == m.default_variant { " (default)" } else { "" };
                println!("      - {v}{def}");
            }
        }
    }

    // Selection loop
    let mut cat_items = cat_names.clone();
    cat_items.push("done".into());
    loop {
        let idx = Select::new()
            .with_prompt("Select category")
            .items(&cat_items)
            .interact()?;
        let input = &cat_items[idx];
        if input == "done" {
            break;
        }

        let cat = input.clone();
        let models = &catalog.iter().find(|(c, _)| c == &cat).unwrap().1;
        let model_names: Vec<String> = models.iter().map(|m| m.display.clone()).collect();
        let model_idx = Select::new()
            .with_prompt("Select model")
            .items(&model_names)
            .interact()?;
        let display = &model_names[model_idx];
        let entry = models.iter().find(|m| m.display == *display).unwrap();

        let var = if entry.variants.len() > 1 {
            let old_var = existing.get(&format!("{cat}_variant")).map(|s| s.as_str());
            let default = old_var.unwrap_or(&entry.default_variant);
            let default_idx = entry.variants.iter().position(|v| v == default).unwrap_or(0);
            let var_idx = Select::new()
                .with_prompt("Select variant")
                .items(&entry.variants)
                .default(default_idx)
                .interact()?;
            entry.variants[var_idx].clone()
        } else {
            entry.default_variant.clone()
        };

        selections.insert(cat.clone(), (entry.display.clone(), entry.toml_model.clone(), var.clone()));
        println!("  ✓ Added {}/{} ({})", cat, entry.display, var);
    }

    // Final summary
    println!("\n── Selections ──");
    for (cat_name, models) in &catalog {
        if let Some((_, toml_model, var)) = selections.get(cat_name) {
            let path = format!("data/{cat_name}/model/{}/{}", 
                models.iter().find(|m| m.toml_model == *toml_model).map(|m| m.display.as_str()).unwrap_or(toml_model),
                var);
            println!("  {cat_name}:  model={toml_model}  variant={var}  path={path}");
        } else {
            println!("  {cat_name}:  (not selected)");
        }
    }

    // Collect update lines
    let mut updates: Vec<(&str, &str)> = Vec::new();
    // Maintain ordering: tts, asr, llm, vad
    for cat in &["tts", "asr", "llm", "vad"] {
        if let Some((_, toml_model, var)) = selections.get(*cat) {
            let model_entry = catalog.iter().find(|(c, _)| c == cat).and_then(|(_, ms)| ms.iter().find(|m| m.toml_model == *toml_model));
            let has_variants = model_entry.map(|m| m.variants.len() > 1).unwrap_or(false);
            let model_key = format!("{cat}_model");
            updates.push((Box::leak(model_key.into_boxed_str()), Box::leak(toml_model.clone().into_boxed_str())));
            if has_variants {
                let var_key = format!("{cat}_variant");
                updates.push((Box::leak(var_key.into_boxed_str()), Box::leak(var.clone().into_boxed_str())));
            }
            let path = format!("data/{cat}/model/{}/{}",
                model_entry.map(|m| m.display.as_str()).unwrap_or(toml_model),
                var);
            let path_key = format!("{cat}_path");
            updates.push((Box::leak(path_key.into_boxed_str()), Box::leak(path.into_boxed_str())));
        }
    }

    if confirm("\nWrite to config file?")? {
        let path = config_path.as_ref().map(|p| p.as_path()).unwrap_or(Path::new("application.toml"));
        upsert_config(path, &updates)?;
        println!("✓ Written to {}", path.display());
    }

    if !selections.is_empty() && confirm("Download all selected models?")? {
        let mir: Vec<String> = Vec::new();
        for (cat, (display, _, var)) in &selections {
            println!("\n--- Downloading {cat}/{display}/{var} ---");
            if let Err(e) = run(
                Some(cat), Some(display.as_str()), Some(var.as_str()),
                data_dir, quiet, &mir, None, false, None,
            ).await {
                eprintln!("  FAIL: {e}");
            }
        }
    }

    Ok(())
}

fn confirm(question: &str) -> Result<bool, Box<dyn std::error::Error>> {
    Ok(dialoguer::Confirm::new()
        .with_prompt(question)
        .default(false)
        .interact()?)
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

#[cfg(test)]
mod tests;
