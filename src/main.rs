use std::{
    collections::HashMap,
    fs,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
    thread::sleep,
};

use dirs::config_dir;
use serde::{Deserialize, Serialize};
use strip_ansi_escapes::strip;
use reqwest::blocking::Client;
use serde_json::Value;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "cargo-i18n")]
#[command(bin_name = "cargo")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Internationalization commands for Cargo
    I18n {
        #[command(subcommand)]
        command: I18nSubCommands,
    },
}

#[derive(Parser)]
struct I18nCommands {
    #[command(subcommand)]
    command: I18nSubCommands,
}

#[derive(Subcommand)]
enum I18nSubCommands {
    /// Run `cargo check` with i18n output in the current project
    Check {
        /// Path to the cargo project (default: ".")
        path: Option<PathBuf>,
    },
}

/// Configuration struct
#[derive(Deserialize, Serialize, Debug, Clone)]
struct Config {
    version: Option<String>,
    language: Option<String>,
    api_url: Option<String>,
    api_key: Option<String>,
    rate_limit: Option<u32>,
    model: Option<String>,
    temperature: Option<f32>,
    request_body_template: Option<String>,
    response_path: Option<String>,
}

struct RateLimiter {
    interval: Duration,
    last: Mutex<Instant>,
}

impl RateLimiter {
    fn new(max_per_sec: u32) -> Self {
        let safe_max = if max_per_sec < 1 { 1 } else { max_per_sec };
        let interval = Duration::from_secs_f64(1.0 / safe_max as f64);
        Self {
            interval,
            last: Mutex::new(Instant::now() - interval),
        }
    }
    fn wait(&self) {
        let mut last = self.last.lock().unwrap();
        let now = Instant::now();
        let elapsed = now.duration_since(*last);
        if elapsed < self.interval {
            sleep(self.interval - elapsed);
        }
        *last = Instant::now();
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    // 默认子命令为空时相当于 `check .`
    let project_path = match cli.command {
        Commands::I18n { command } => match command {
            I18nSubCommands::Check { path } => path.unwrap_or_else(|| ".".into()),
        },
    };

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
rate_limit = 8
model = "gpt-4o-mini"
temperature = 0.2

# Request body template. Supports {{model}}, {{prompt}}, and {{temperature}} variables.
request_body_template = """
{
    \"model\": \"{{model}}\",
    \"messages\": [{\"role\": \"user\", \"content\": \"{{prompt}}\"}],
    \"temperature\": {{temperature}}
}
"""
# JSON path to the response, using dot notation, e.g., choices.0.message.content
response_path = "choices.0.message.content"
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
    let cache_path = project_path.join(".cargo-check-i18n-cache.json");
    let cache = Arc::new(Mutex::new(load_cache(&cache_path)));

    // 读取 rate_limit，默认8，最小1
    let rate_limit = cfg.rate_limit.unwrap_or(8).max(1);
    let limiter = Arc::new(RateLimiter::new(rate_limit));

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
        spawn_reader(stdout, Arc::clone(&cache), cache_path.clone(), Arc::clone(&limiter)),
        spawn_reader(stderr, Arc::clone(&cache), cache_path.clone(), Arc::clone(&limiter)),
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

// 修改 spawn_reader 增加 limiter 参数
fn spawn_reader(
    stream: impl std::io::Read + Send + 'static,
    cache: Arc<Mutex<HashMap<String, String>>>,
    cache_path: PathBuf,
    limiter: Arc<RateLimiter>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let reader = BufReader::new(stream).lines();
        for line in reader.flatten() {
            process_line(&line, &cache, &cache_path, &limiter);
        }
    })
}

// 修改 process_line 增加 limiter 参数
fn process_line(
    raw: &str,
    cache: &Arc<Mutex<HashMap<String, String>>>,
    cache_path: &Path,
    limiter: &RateLimiter,
) {
    let clean = String::from_utf8_lossy(&strip(raw.as_bytes())).to_string();
    if should_translate(&clean) {
        let key = clean.trim().to_string();
        let zh = {
            let mut store = cache.lock().unwrap();
            if let Some(val) = store.get(&key) {
                val.clone()
            } else {
                let cfg = get_config();
                let language = cfg.language.clone().unwrap_or_else(|| "zh-CN".into());
                // 优化 prompt，去除多余空格和特殊字符
                let prompt = format!(
                    "Translate the following English compiler diagnostic message into {} as plain text: {}",
                    language,
                    key.replace('\n', " ").replace("```", "").trim()
                );
                limiter.wait();
                let res = query_llm(&prompt, &cfg);
                match res {
                    Some(ref v) if v != "Translation failed." => {
                        let trimmed = v.trim_end().to_string(); // 去除末尾的换行符
                        store.insert(key.clone(), trimmed.clone());
                        let _ = save_cache(cache_path, &*store);
                        trimmed
                    }
                    Some(v) => v.trim_end().to_string(),
                    None => "Translation failed.".to_string(),
                }
            }
        };
        println!("{} ({})", raw, zh); // 原文后直接加括号译文
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

fn query_llm(prompt: &str, cfg: &Config) -> Option<String> {
    let url = cfg.api_url.clone()?;
    let api_key = cfg.api_key.clone();
    let model = cfg.model.clone().unwrap_or_else(|| "".into());
    let temp = cfg.temperature.unwrap_or(0.2);

    // 构造请求体
    let body = if let Some(tpl) = &cfg.request_body_template {
        tpl.replace("{{model}}", &model)
            .replace("{{prompt}}", prompt)
            .replace("{{temperature}}", &temp.to_string())
    } else {
        serde_json::json!({
            "model": model,
            "messages": [{ "role": "user", "content": prompt }],
            "temperature": temp
        }).to_string()
    };

    let client = Client::new();
    let mut req = client.post(&url)
        .header("Content-Type", "application/json")
        .body(body);

    if let Some(key) = api_key {
        req = req.header("Authorization", format!("Bearer {}", key));
    }

    let resp = req.send().ok()?;
    if !resp.status().is_success() {
        eprintln!("API request failed: {}", resp.status());
        return None;
    }
    let v: Value = serde_json::from_str(&resp.text().ok()?).ok()?;

    // 解析响应
    let path = cfg.response_path.as_deref().unwrap_or("choices.0.message.content");
    extract_json_path(&v, path)
}

fn extract_json_path(v: &Value, path: &str) -> Option<String> {
    let mut cur = v;
    for seg in path.split('.') {
        if let Some(idx) = seg.parse::<usize>().ok() {
            cur = cur.get(idx)?;
        } else {
            cur = cur.get(seg)?;
        }
    }
    cur.as_str().map(str::to_string)
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
