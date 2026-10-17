use std::path::{Path, PathBuf};

use crate::error::{Error, Result};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AppConfig {
    pub data_dir: PathBuf,
    pub demo_mode: bool,
    pub helpscout: HelpScoutConfig,
    pub ai: AiConfig,
    pub sync_interval_minutes: u32,
    pub loopback_port: u16,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct HelpScoutConfig {
    pub client_id: String,
    #[serde(skip_serializing, default)]
    pub client_secret: String,
    pub redirect_uri: String,
    #[serde(skip_serializing, default)]
    pub webhook_secret: String,
    #[serde(skip_serializing, default)]
    pub docs_api_key: String,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct AiConfig {
    pub provider: String,
    pub base_url: String,
    pub chat_model: String,
    pub embedding_model: String,
    pub timeout_ms: u32,
    pub concurrency: u32,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            data_dir: default_data_dir(),
            demo_mode: false,
            helpscout: HelpScoutConfig::default(),
            ai: AiConfig::default(),
            sync_interval_minutes: 5,
            loopback_port: 0,
        }
    }
}

impl HelpScoutConfig {
    pub fn redacted(&self) -> HelpScoutConfig {
        HelpScoutConfig {
            client_id: redact_string(&self.client_id),
            client_secret: String::new(),
            redirect_uri: self.redirect_uri.clone(),
            webhook_secret: String::new(),
            docs_api_key: String::new(),
        }
    }
}

pub fn default_data_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("SPP_DATA_DIR") {
        return PathBuf::from(dir);
    }

    let base = std::env::var("XDG_DATA_HOME")
        .ok()
        .filter(|s| !s.is_empty());
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    let root = base
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(home).join(".local").join("share"));
    root.join("supportos-plusplus")
}

fn env_str(key: &str, default: &str) -> String {
    std::env::var(key)
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| default.to_string())
}

fn env_bool(key: &str, default: bool) -> bool {
    match std::env::var(key) {
        Ok(v) if !v.trim().is_empty() => matches!(
            v.trim().to_ascii_lowercase().as_str(),
            "true" | "1" | "yes" | "on"
        ),
        _ => default,
    }
}

fn env_int(key: &str, default: i64) -> i64 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(default)
}

#[derive(Debug, Clone, PartialEq)]
pub struct EnvConfig {
    pub demo_mode: bool,
    pub log_level: String,
    pub helpscout: EnvHelpScout,
    pub lmstudio: EnvLmStudio,
    pub qdrant: EnvQdrant,
    pub sync: EnvSync,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EnvHelpScout {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
    pub api_base: String,
    pub webhook_secret: String,
    pub docs_api_key: String,
    pub docs_api_base: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EnvLmStudio {
    pub base_url: String,
    pub chat_model: Option<String>,
    pub embedding_model: Option<String>,
    pub timeout_ms: i64,
    pub concurrency: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EnvQdrant {
    pub url: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EnvSync {
    pub interval_minutes: i64,
    pub api_concurrency: i64,
}

pub fn load_env_config() -> EnvConfig {
    EnvConfig {
        demo_mode: env_bool("LOCAL_DEMO_MODE", false),
        log_level: env_str("LOG_LEVEL", "info"),
        helpscout: EnvHelpScout {
            client_id: env_str("HELPSCOUT_CLIENT_ID", ""),
            client_secret: env_str("HELPSCOUT_CLIENT_SECRET", ""),
            redirect_uri: env_str(
                "HELPSCOUT_REDIRECT_URI",
                "http://localhost:3000/oauth/callback",
            ),
            api_base: env_str("HELPSCOUT_API_BASE", "https://api.helpscout.net"),
            webhook_secret: env_str("HELPSCOUT_WEBHOOK_SECRET", ""),
            docs_api_key: env_str("HELPSCOUT_DOCS_API_KEY", ""),
            docs_api_base: env_str("HELPSCOUT_DOCS_API_BASE", "https://docsapi.helpscout.net"),
        },
        lmstudio: EnvLmStudio {
            base_url: env_str("LMSTUDIO_BASE_URL", "http://127.0.0.1:1234"),
            chat_model: std::env::var("LMSTUDIO_CHAT_MODEL")
                .ok()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty()),
            embedding_model: std::env::var("LMSTUDIO_EMBEDDING_MODEL")
                .ok()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty()),
            timeout_ms: env_int("LMSTUDIO_TIMEOUT_MS", 120_000),
            concurrency: env_int("LMSTUDIO_CONCURRENCY", 2),
        },
        qdrant: EnvQdrant {
            url: env_str("QDRANT_URL", "http://127.0.0.1:6333"),
            enabled: env_bool("QDRANT_ENABLED", true),
        },
        sync: EnvSync {
            interval_minutes: env_int("SYNC_INTERVAL_MINUTES", 5),
            api_concurrency: env_int("SYNC_API_CONCURRENCY", 2),
        },
    }
}

pub fn ensure_dir(path: &Path) -> Result<()> {
    if !path.exists() {
        std::fs::create_dir_all(path).map_err(Error::Io)?;
    }
    Ok(())
}

fn redact_string(s: &str) -> String {
    if s.is_empty() {
        return String::new();
    }
    if s.len() <= 4 {
        return "••••".into();
    }
    let head: usize = 2.min(s.len() / 4);
    let tail: usize = 2.min(s.len() / 4);
    format!("{}••••{}", &s[..head], &s[s.len() - tail..])
}
