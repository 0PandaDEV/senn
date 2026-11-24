use std::{path::PathBuf, sync::OnceLock};

#[derive(Debug, Clone)]
pub struct Config {
    pub data_dir: PathBuf,
    pub base_url: String,
    pub secure_cookies: bool,
    pub trusted_proxy_hops: usize,
    pub metrics_url: Option<String>,
}

static CONFIG: OnceLock<Config> = OnceLock::new();

fn var(name: &str) -> Option<String> {
    std::env::var(format!("SENN_{name}"))
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

pub fn init() {
    let base_url = var("BASE_URL").unwrap_or_default();
    let config = Config {
        data_dir: PathBuf::from("./data"),
        secure_cookies: var("SECURE_COOKIES")
            .map(|v| v == "true")
            .unwrap_or_else(|| base_url.starts_with("https://")),
        base_url: base_url.trim_end_matches('/').to_string(),
        metrics_url: match std::env::var("SENN_METRICS_URL") {
            Ok(v) => Some(v.trim().trim_end_matches('/').to_string()).filter(|v| !v.is_empty()),
            Err(_) => Some("http://victoriametrics:8428".to_string()),
        },
        trusted_proxy_hops: match var("TRUST_PROXY").as_deref() {
            Some("true") => 1,
            Some(other) => other.parse().unwrap_or(0),
            None => 0,
        },
    };
    std::fs::create_dir_all(&config.data_dir).expect("create data dir");
    CONFIG.set(config).expect("config already initialised");
}

pub fn config() -> &'static Config {
    CONFIG.get().expect("config not initialised")
}
