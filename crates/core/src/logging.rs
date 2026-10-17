use tracing_subscriber::{EnvFilter, fmt, prelude::*};

pub fn init() {
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        let level = std::env::var("LOG_LEVEL")
            .ok()
            .map(|l| l.trim().to_ascii_lowercase())
            .filter(|l| matches!(l.as_str(), "debug" | "info" | "warn" | "error"));
        EnvFilter::new(match level.as_deref() {
            Some("debug") => "debug".to_string(),
            Some("warn") => "warn".to_string(),
            Some("error") => "error".to_string(),
            _ => "info,spp_core=debug".to_string(),
        })
    });

    let is_json =
        cfg!(not(debug_assertions)) || std::env::var("SPP_LOG_FORMAT").as_deref() == Ok("json");

    let registry = tracing_subscriber::registry().with(env_filter);

    if is_json {
        registry.with(fmt::layer().json()).try_init().ok();
    } else {
        registry
            .with(fmt::layer().with_target(true).pretty())
            .try_init()
            .ok();
    }
}
