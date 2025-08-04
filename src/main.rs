use std::{
    collections::HashMap,
    env,
    fs,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{Arc, Mutex},
    thread,
};

use dirs::config_dir;
use serde::{Deserialize, Serialize};
use strip_ansi_escapes::strip;
use reqwest::blocking::Client;
use serde_json::Value;

/// Configuration struct
#[derive(Deserialize, Serialize, Debug, Clone)]
struct Config {
    version: Option<String>,
    language: Option<String>,
    api_url: Option<String>,
    api_key: Option<String>,
    model: Option<String>,
    temperature: Option<f32>,
    default_prompt: Option<String>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Configuration path
    let config_path = config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("cargo-check-i18n")
        .join("config.toml");
    fs::create_dir_all(config_path.parent().unwrap())?;

    // Load or initialize configuration
    let cfg: Config = if config_path.exists() {
        let s = fs::read_to_string(&config_path)?;
        toml::from_str(&s)?
    } else {
        let example = r#"version = "1.0"
language = "zh-CN"
api_url = "https://api.openai.com/v1/chat/completions"
api_key = ""
model = "gpt-4o-mini"
temperature = 0.2
"#;
        fs::write(&config_path, example)?;
        eprintln!(
            "Example configuration {} has been created. Please fill in the api_key and try again.",
            config_path.display()
        );
        return Ok(());
    };

    // Validate API key
    let _api_key = cfg.api_key.clone().filter(|k| !k.is_empty()).unwrap_or_else(|| {
        eprintln!("Please provide the api_key in config.toml");
        std::process::exit(1);
    });

    // Cache
    let project_path = env::args().nth(1).unwrap_or_else(|| ".".into());
    let cache_path = PathBuf::from(&project_path).join(".cargo-check-i18n-cache.json");
    let cache = Arc::new(Mutex::new(load_cache(&cache_path)));

    // Run cargo check
    let mut child = Command::new("cargo")
        .args(&["check", "--color=always"])
        .current_dir(&project_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();

    let handles = vec![
        spawn_reader(stdout, Arc::clone(&cache), cache_path.clone()),
        spawn_reader(stderr, Arc::clone(&cache), cache_path.clone()),
    ];
    for h in handles {
        h.join().unwrap();
    }

    let status = child.wait()?;
    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }
    Ok(())
}

fn spawn_reader(
    stream: impl std::io::Read + Send + 'static,
    cache: Arc<Mutex<HashMap<String, String>>>,
    cache_path: PathBuf,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let reader = BufReader::new(stream).lines();
        for line in reader.flatten() {
            process_line(&line, &cache, &cache_path);
        }
    })
}

fn process_line(
    raw: &str,
    cache: &Arc<Mutex<HashMap<String, String>>>,
    cache_path: &Path,
) {
    let clean = String::from_utf8_lossy(&strip(raw.as_bytes())).to_string();
    if should_translate(&clean) {
        let key = clean.trim().to_string();
        // Insert or retrieve translation, enforcing the use of 'language'
        let zh = {
            let mut store = cache.lock().unwrap();
            let result = store.entry(key.clone()).or_insert_with(|| {
                let cfg = get_config();
                let language = cfg.language.clone().unwrap_or_else(|| "zh-CN".into());
                let prompt = format!("As a plain text translator, translate the following English into{}：{}", language, key);
                let res = query_openai(&prompt, &cfg).unwrap_or_else(|| "Translation failed.".into());
                res.lines().map(str::trim_end).collect::<Vec<_>>().join(" ")
            }).clone();
            let _ = save_cache(cache_path, &*store);
            result
        };
        println!("{} ({})", raw, zh);
    } else {
        println!("{}", raw);
    }
}

fn should_translate(line: &str) -> bool {
    let t = line.trim();
    let lower = t.to_lowercase();
    if lower.starts_with("compiling")
        || lower.starts_with("checking")
        || lower.starts_with("finished")
        || lower.starts_with("-->")
    {
        return false;
    }
    if t.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) && t.contains('|') {
        return false;
    }
    if t == "|" {
        return false;
    }
    if let Some(i) = line.find('|') {
        let after = &line[i + 1..];
        if !after.trim_start().starts_with('-') && !after.trim_start().starts_with('^') {
            return false;
        }
    }
    lower.contains("error")
        || lower.contains("warning")
        || lower.contains("note")
        || lower.contains("help")
        || (line.len() > 15 && line.len() < 120 && line.chars().any(|c| c.is_alphabetic()))
}

fn get_config() -> Config {
    let path = config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("cargo-check-i18n")
        .join("config.toml");
    let s = fs::read_to_string(&path).unwrap();
    toml::from_str(&s).unwrap()
}

fn query_openai(prompt: &str, cfg: &Config) -> Option<String> {
    let url = cfg.api_url.clone()?;
    let model = cfg.model.clone().unwrap_or_else(|| "gpt-4o-mini".into());
    let temp = cfg.temperature.unwrap_or(0.2);
    let api_key = cfg.api_key.clone()?;
    let client = Client::new();
    let body = serde_json::json!({
        "model": model,
        "messages": [{ "role": "user", "content": prompt }],
        "temperature": temp
    });
    let resp = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .body(body.to_string())
        .send()
        .ok()?;
    if !resp.status().is_success() {
        eprintln!("API request failed: {}", resp.status());
        return None;
    }
    let v: Value = serde_json::from_str(&resp.text().ok()?).ok()?;
    v["choices"][0]["message"]["content"].as_str().map(str::to_string)
}

fn load_cache(path: &Path) -> HashMap<String, String> {
    fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_cache(path: &Path, cache: &HashMap<String, String>) -> Result<(), std::io::Error> {
    fs::write(path, serde_json::to_string_pretty(cache)?)
}
