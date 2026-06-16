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
    let mut show_existing = !existing.is_empty();

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
    loop {
        if show_existing {
            show_existing = false;
            show_selections(&selections, &existing, &catalog);
        }

        let input = prompt_category(&cat_names)?;
        if input == "done" {
            break;
        }
        if input == "show" {
            show_selections(&selections, &existing, &catalog);
            continue;
        }

        let cat = input;
        let models = &catalog.iter().find(|(c, _)| c == &cat).unwrap().1;
        let model_names: Vec<String> = models.iter().map(|m| m.display.clone()).collect();
        let display = prompt_choice("  Select model", &model_names)?;
        let entry = models.iter().find(|m| m.display == display).unwrap();

        let var = if entry.variants.len() > 1 {
            let old_var = existing.get(&format!("{cat}_variant")).map(|s| s.as_str());
            let default = old_var.unwrap_or(&entry.default_variant);
            prompt_choice_default("  Select variant", default, &entry.variants)?
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

fn show_selections(
    selections: &HashMap<String, (String, String, String)>,
    existing: &HashMap<String, String>,
    catalog: &[(String, Vec<ModelInfo>)],
) {
    println!("\nCurrent selections:");
    for (cat_name, models) in catalog {
        let label = format!("  {cat_name}:");
        if let Some((display, toml_model, var)) = selections.get(cat_name) {
            let old_variant = existing.get(&format!("{cat_name}_variant"));
            let old_model = existing.get(&format!("{cat_name}_model"));
            let status = if old_model.map_or(true, |m| m != toml_model) {
                " (new)".to_string()
            } else if old_variant.map_or(true, |v| v != var) {
                format!(" (changed, was: {})", old_variant.unwrap_or(&String::new()))
            } else if *var == *models.iter().find(|m| m.display == *display).map(|m| &m.default_variant).unwrap_or(&String::new()) {
                " (default, unchanged)".to_string()
            } else {
                " (unchanged)".to_string()
            };
            println!("{label} {display} → {var}{status}");
        } else {
            println!("{label} (not selected)");
        }
    }
}

fn confirm(question: &str) -> Result<bool, Box<dyn std::error::Error>> {
    loop {
        eprint!("{question} [y/N]: ");
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        match input.trim().to_lowercase().as_str() {
            "y" | "yes" => return Ok(true),
            "n" | "no" | "" => return Ok(false),
            _ => eprintln!("Please answer y or n."),
        }
    }
}

fn prompt_category(valid: &[String]) -> Result<String, Box<dyn std::error::Error>> {
    loop {
        eprint!("Select category (or 'show'/'done') [{}]: ", valid.join("/"));
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        let input = input.trim().to_lowercase();
        if input == "done" || input == "show" || valid.iter().any(|v| v == &input) {
            return Ok(input);
        }
        eprintln!("Invalid. Choose from: {}, or 'show'/'done'", valid.join(", "));
    }
}

fn prompt_choice(question: &str, valid: &[String]) -> Result<String, Box<dyn std::error::Error>> {
    loop {
        eprint!("{question} [{}]: ", valid.join("/"));
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        let input = input.trim().to_lowercase();
        if valid.iter().any(|v| v == &input) {
            return Ok(input);
        }
        eprintln!("Invalid choice. Choose from: {}", valid.join(", "));
    }
}

fn prompt_choice_default(question: &str, default: &str, valid: &[String]) -> Result<String, Box<dyn std::error::Error>> {
    loop {
        eprint!("{question} [{}] (default: {default}): ", valid.join("/"));
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        let input = input.trim().to_lowercase();
        if input.is_empty() {
            return Ok(default.to_string());
        }
        if valid.iter().any(|v| v == &input) {
            return Ok(input);
        }
        eprintln!("Invalid choice. Choose from: {}", valid.join(", "));
    }
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
mod tests {
    use super::*;
    use std::fs;

    // ── helpers ──

    fn test_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join("chobits-test").join(name);
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn sha256_of(data: &[u8]) -> String {
        let mut hasher = sha2::Sha256::new();
        hasher.update(data);
        hex::encode(hasher.finalize())
    }

    fn make_file(path: &str, url: &str) -> FileEntry {
        FileEntry { path: path.into(), url: url.into(), sha256: None }
    }

    fn make_entry(
        files: Vec<(&str, Vec<FileEntry>)>,
        default_variant: Option<&str>,
    ) -> ModelEntry {
        ModelEntry {
            config: None,
            default_variant: default_variant.map(|s| s.into()),
            variants: files
                .into_iter()
                .map(|(k, v)| (k.into(), Variant { files: v }))
                .collect(),
        }
    }

    // ── generate_urls ──

    #[test]
    fn test_generate_urls_hf_adds_default_mirror() {
        let urls = generate_urls("https://huggingface.co/model/file.bin", &[]);
        assert_eq!(urls.len(), 2);
        assert_eq!(urls[0], "https://huggingface.co/model/file.bin");
        assert_eq!(urls[1], "https://hf-mirror.com/model/file.bin");
    }

    #[test]
    fn test_generate_urls_non_hf_no_mirror() {
        let urls = generate_urls("https://example.com/model.bin", &[]);
        assert_eq!(urls.len(), 1);
        assert_eq!(urls[0], "https://example.com/model.bin");
    }

    #[test]
    fn test_generate_urls_custom_mirrors() {
        let urls = generate_urls(
            "https://huggingface.co/model.bin",
            &["https://my-mirror.com".into()],
        );
        assert_eq!(urls.len(), 2);
        assert_eq!(urls[1], "https://my-mirror.com/model.bin");
    }

    #[test]
    fn test_generate_urls_empty_mirror_vec() {
        let urls = generate_urls("https://huggingface.co/model.bin", &Vec::new());
        assert_eq!(urls.len(), 2);
        assert_eq!(urls[1], "https://hf-mirror.com/model.bin");
    }

    // ── resolve_variants ──

    #[test]
    fn test_resolve_variants_specific() {
        let entry = make_entry(
            vec![
                ("a", vec![make_file("f1", "u1")]),
                ("b", vec![make_file("f2", "u2")]),
            ],
            Some("a"),
        );
        let r = resolve_variants(&entry, Some("b"));
        assert_eq!(r.len(), 1);
        assert!(r.contains_key("b"));
    }

    #[test]
    fn test_resolve_variants_default() {
        let entry = make_entry(
            vec![
                ("a", vec![make_file("f1", "u1")]),
                ("b", vec![make_file("f2", "u2")]),
            ],
            Some("a"),
        );
        let r = resolve_variants(&entry, None);
        assert_eq!(r.len(), 1);
        assert!(r.contains_key("a"));
    }

    #[test]
    fn test_resolve_variants_no_default_gives_all() {
        let entry = make_entry(
            vec![
                ("a", vec![make_file("f1", "u1")]),
                ("b", vec![make_file("f2", "u2")]),
            ],
            None,
        );
        let r = resolve_variants(&entry, None);
        assert_eq!(r.len(), 2);
    }

    #[test]
    fn test_resolve_variants_unknown_empty() {
        let entry =
            make_entry(vec![("a", vec![make_file("f1", "u1")])], Some("a"));
        let r = resolve_variants(&entry, Some("unknown"));
        assert!(r.is_empty());
    }

    // ── config_to_targets ──

    fn make_cfg(v: serde_json::Value) -> api::config::Config {
        serde_json::from_value(v).unwrap()
    }

    #[test]
    fn test_config_to_targets_defaults() {
        let t = config_to_targets(&make_cfg(serde_json::json!({})));
        // defaults: tts=PocketTts, asr=Qwen3, llm=Qwen3, vad=Earshot
        assert_eq!(t.len(), 3);
        assert!(t.contains(&("tts".into(), "pocket-tts".into(), None)));
        assert!(t.contains(&("asr".into(), "qwen3".into(), None)));
        assert!(t.contains(&("llm".into(), "qwen3".into(), None)));
    }

    #[test]
    fn test_config_to_targets_all_quiet() {
        let t = config_to_targets(&make_cfg(serde_json::json!({
            "tts_model": "mute",
            "asr_model": "void",
            "llm_model": "echo",
            "vad_model": "void",
        })));
        assert!(t.is_empty());
    }

    #[test]
    fn test_config_to_targets_variant() {
        let t = config_to_targets(&make_cfg(serde_json::json!({
            "tts_model": "voxcpm",
            "tts_variant": "1.5b",
            "asr_model": "void",
            "llm_model": "echo",
            "vad_model": "void",
        })));
        assert_eq!(t.len(), 1);
        assert_eq!(t[0], ("tts".into(), "voxcpm".into(), Some("1.5b".into())));
    }

    #[test]
    fn test_config_to_targets_silero() {
        let t = config_to_targets(&make_cfg(serde_json::json!({
            "vad_model": "silero",
        })));
        assert!(t.contains(&("vad".into(), "silero".into(), None)));
    }

    // ── sha256_file ──

    #[test]
    fn test_sha256_file_known() {
        let dir = test_dir("sha256_file");
        let path = dir.join("data.bin");
        let data = b"hello world";
        fs::write(&path, data).unwrap();
        let (size, sha) = sha256_file(&path).unwrap();
        assert_eq!(size, data.len() as u64);
        assert_eq!(sha, sha256_of(data));
    }

    // ── set_sha256 ──

    #[test]
    fn test_set_sha256_flat() {
        let mut v = serde_json::json!({"path": "m.bin", "url": "http://x"});
        set_sha256(&mut v, "m.bin", "abc");
        assert_eq!(v["sha256"], "abc");
    }

    #[test]
    fn test_set_sha256_nested() {
        let mut v = serde_json::json!({
            "variants": {
                "d": {
                    "files": [
                        {"path": "a", "url": "u1"},
                        {"path": "b", "url": "u2"},
                    ]
                }
            }
        });
        set_sha256(&mut v, "b", "s1");
        assert_eq!(v["variants"]["d"]["files"][1]["sha256"], "s1");
        assert!(v["variants"]["d"]["files"][0].get("sha256").is_none());
    }

    #[test]
    fn test_set_sha256_no_match() {
        let mut v = serde_json::json!({"files": [{"path": "a", "url": "u"}]});
        set_sha256(&mut v, "unknown", "x");
        assert!(v["files"][0].get("sha256").is_none());
    }

    // ── load_selections ──

    #[test]
    fn test_load_selections_valid() {
        let dir = test_dir("load_sel");
        let p = dir.join("c.toml");
        fs::write(&p, "[global]\ntts_model = \"voxcpm\"\ntts_variant = \"1.5b\"\n").unwrap();
        let m = load_selections(&p);
        assert_eq!(m.get("tts_model").unwrap(), "voxcpm");
        assert_eq!(m.get("tts_variant").unwrap(), "1.5b");
    }

    #[test]
    fn test_load_selections_no_global() {
        let dir = test_dir("load_sel_nog");
        let p = dir.join("c.toml");
        fs::write(&p, "[other]\nk = \"v\"\n").unwrap();
        assert!(load_selections(&p).is_empty());
    }

    #[test]
    fn test_load_selections_empty() {
        let dir = test_dir("load_sel_emp");
        let p = dir.join("c.toml");
        fs::write(&p, "").unwrap();
        assert!(load_selections(&p).is_empty());
    }

    // ── upsert_config ──

    #[test]
    fn test_upsert_config_new() {
        let dir = test_dir("upsert_new");
        let p = dir.join("c.toml");
        upsert_config(&p, &[("tts_model", "voxcpm")]).unwrap();
        let c = fs::read_to_string(&p).unwrap();
        assert!(c.contains("[global]"));
        assert!(c.contains("tts_model = \"voxcpm\""));
    }

    #[test]
    fn test_upsert_config_update() {
        let dir = test_dir("upsert_upd");
        let p = dir.join("c.toml");
        fs::write(&p, "[global]\ntts_model = \"old\"\n").unwrap();
        upsert_config(&p, &[("tts_model", "new"), ("tts_variant", "1.5b")]).unwrap();
        let c = fs::read_to_string(&p).unwrap();
        assert!(c.contains("tts_model = \"new\""));
        assert!(c.contains("tts_variant = \"1.5b\""));
    }

    #[test]
    fn test_upsert_config_preserve_sections() {
        let dir = test_dir("upsert_pres");
        let p = dir.join("c.toml");
        fs::write(&p, "[global]\ntts_model = \"a\"\n[other]\nk = \"v\"\n").unwrap();
        upsert_config(&p, &[("tts_model", "b")]).unwrap();
        let c = fs::read_to_string(&p).unwrap();
        assert!(c.contains("[other]"));
        assert!(c.contains("k = \"v\""));
        assert!(c.contains("tts_model = \"b\""));
    }

    #[test]
    fn test_upsert_config_add_global() {
        let dir = test_dir("upsert_add_g");
        let p = dir.join("c.toml");
        fs::write(&p, "[other]\nk = \"v\"\n").unwrap();
        upsert_config(&p, &[("tts_model", "m")]).unwrap();
        let c = fs::read_to_string(&p).unwrap();
        assert!(c.contains("[global]"));
        assert!(c.contains("tts_model = \"m\""));
    }

    // ── find_config ──

    fn with_cwd<F>(dir: &Path, f: F)
    where
        F: FnOnce(),
    {
        let orig = std::env::current_dir().unwrap();
        std::env::set_current_dir(dir).unwrap();
        f();
        std::env::set_current_dir(&orig).unwrap();
    }

    #[test]
    fn test_find_config_env_var() {
        let dir = test_dir("fc_env");
        let p = dir.join("my.toml");
        fs::write(&p, "").unwrap();
        let r = find_config_inner(Some(p.to_str().unwrap().into()));
        assert_eq!(r, Some(p));
    }

    #[test]
    fn test_find_config_finds_application_toml() {
        let dir = test_dir("fc_app");
        with_cwd(&dir, || {
            fs::write("application.toml", "").unwrap();
            assert_eq!(find_config_inner(None), Some(PathBuf::from("application.toml")));
        });
    }

    #[test]
    fn test_find_config_fallback() {
        let dir = test_dir("fc_fb");
        with_cwd(&dir, || {
            assert_eq!(find_config_inner(None), Some(PathBuf::from("application.toml")));
        });
    }

    // ── load_overrides ──

    #[tokio::test]
    async fn test_load_overrides_local() {
        let dir = test_dir("lo");
        let p = dir.join("ov.json");
        fs::write(
            &p,
            r#"{"data/m.bin": {"url": "http://localhost/m.bin", "sha256": "ab"}}"#,
        )
        .unwrap();
        let m = load_overrides(p.to_str().unwrap()).await.unwrap();
        assert_eq!(m.len(), 1);
        assert_eq!(m["data/m.bin"].url, "http://localhost/m.bin");
        assert_eq!(m["data/m.bin"].sha256.as_deref(), Some("ab"));
    }

    // ── try_download_url (mockito) ──

    #[tokio::test]
    async fn test_try_download_ok() {
        let mut srv = mockito::Server::new_async().await;
        let body = b"hello";
        let m = srv
            .mock("GET", "/f.bin")
            .with_status(200)
            .with_body(body)
            .expect(1)
            .create();

        let dir = test_dir("tdl_ok");
        let dest = dir.join("f.bin");
        let url = format!("{}/f.bin", srv.url());

        let r = try_download_url(&Client::new(), &url, &dest, None).await;
        assert!(r.is_ok());
        let (sz, sha) = r.unwrap();
        assert_eq!(sz, body.len() as u64);
        assert_eq!(sha, sha256_of(body));
        assert!(dest.exists());
        m.assert();
    }

    #[tokio::test]
    async fn test_try_download_sha_mismatch() {
        let mut srv = mockito::Server::new_async().await;
        let m = srv
            .mock("GET", "/f.bin")
            .with_status(200)
            .with_body(b"hello")
            .expect(1)
            .create();

        let dir = test_dir("tdl_mismatch");
        let dest = dir.join("f.bin");
        let url = format!("{}/f.bin", srv.url());

        let r = try_download_url(
            &Client::new(),
            &url,
            &dest,
            Some("0000000000000000000000000000000000000000000000000000000000000000"),
        )
        .await;
        assert!(r.is_err());
        assert!(!dest.exists());
        assert!(!dest.with_extension("tmp").exists());
        m.assert();
    }

    #[tokio::test]
    async fn test_try_download_conn_refused() {
        let client = Client::builder().no_proxy().build().unwrap();
        let dir = test_dir("tdl_conn");
        let r = try_download_url(&client, "http://127.0.0.1:18634/f.bin", &dir.join("f.bin"), None).await;
        assert!(r.is_err(), "expected error, got {:?}", r);
    }

    // ── download_file (mockito) ──

    #[tokio::test]
    async fn test_download_cached() {
        let mut srv = mockito::Server::new_async().await;
        let body = b"cached";
        let m = srv
            .mock("GET", "/f.bin")
            .with_status(200)
            .with_body(body)
            .expect(0)
            .create();

        let dir = test_dir("dl_cached");
        let dest = dir.join("f.bin");
        let sha = sha256_of(body);
        fs::write(&dest, body).unwrap();

        let r =
            download_file(&Client::new(), &format!("{}/f.bin", srv.url()), &dest, Some(&sha), &[])
                .await;
        assert!(r.is_ok());
        m.assert();
    }

    #[tokio::test]
    async fn test_download_re_download_on_sha_mismatch() {
        let mut srv = mockito::Server::new_async().await;
        let correct = b"correct";
        let m = srv
            .mock("GET", "/f.bin")
            .with_status(200)
            .with_body(correct)
            .expect(1)
            .create();

        let dir = test_dir("dl_redl");
        let dest = dir.join("f.bin");
        let sha = sha256_of(correct);
        fs::write(&dest, b"wrong").unwrap();

        let r = download_file(
            &Client::new(),
            &format!("{}/f.bin", srv.url()),
            &dest,
            Some(&sha),
            &[],
        )
        .await;
        assert!(r.is_ok());
        assert_eq!(fs::read(&dest).unwrap(), correct);
        m.assert();
    }

    #[tokio::test]
    async fn test_download_fresh() {
        let mut srv = mockito::Server::new_async().await;
        let body = b"fresh";
        let m = srv
            .mock("GET", "/f.bin")
            .with_status(200)
            .with_body(body)
            .expect(1)
            .create();

        let dir = test_dir("dl_fresh");
        let dest = dir.join("f.bin");

        let r = download_file(
            &Client::new(),
            &format!("{}/f.bin", srv.url()),
            &dest,
            None,
            &[],
        )
        .await;
        assert!(r.is_ok());
        assert_eq!(fs::read(&dest).unwrap(), body);
        m.assert();
    }
}
