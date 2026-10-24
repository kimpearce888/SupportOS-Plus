use std::sync::OnceLock;
use tracing_subscriber::{EnvFilter, Registry, fmt, layer::SubscriberExt, prelude::*, reload};

/// The live log-filter handle, set once by [`init`] when this process
/// owns the global subscriber. Test binaries that install their own
/// subscriber leave it `None`; [`apply_log_level`] then no-ops.
static FILTER: OnceLock<Option<reload::Handle<EnvFilter, Registry>>> = OnceLock::new();

/// The directive string for a level word — mirrors the boot default's
/// shape so the setting only moves the global cap (under `info` the
/// `spp_core` targets keep their extra headroom).
fn directive_for(level: &str) -> String {
    match level {
        "debug" => "debug".to_string(),
        "warn" => "warn".to_string(),
        "error" => "error".to_string(),
        _ => "info,spp_core=debug".to_string(),
    }
}

pub fn init() {
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        let level = std::env::var("LOG_LEVEL")
            .ok()
            .map(|l| l.trim().to_ascii_lowercase())
            .filter(|l| matches!(l.as_str(), "debug" | "info" | "warn" | "error"));
        let directive = level
            .as_deref()
            .map(directive_for)
            .unwrap_or_else(|| directive_for("info"));
        EnvFilter::new(directive)
    });

    // Clean-audit item 138: the filter sits behind a reload layer so the
    // settings page's `log_level` row can reach the live subscriber
    // (applied at boot from the stored setting, and on every save).
    let (filter_layer, handle) = reload::Layer::new(env_filter);
    let registry = tracing_subscriber::registry().with(filter_layer);

    let is_json =
        cfg!(not(debug_assertions)) || std::env::var("SPP_LOG_FORMAT").as_deref() == Ok("json");

    let installed = if is_json {
        registry.with(fmt::layer().json()).try_init().is_ok()
    } else {
        registry
            .with(fmt::layer().with_target(true).pretty())
            .try_init()
            .is_ok()
    };
    let _ = FILTER.set(installed.then_some(handle));
}

/// Apply a log level ("debug" | "info" | "warn" | "error") to the live
/// subscriber. Unknown levels are ignored (the UI only offers the four);
/// a process that does not own the global subscriber (test binaries) is
/// a no-op.
pub fn apply_log_level(level: &str) {
    let level = level.trim().to_ascii_lowercase();
    if !matches!(level.as_str(), "debug" | "info" | "warn" | "error") {
        return;
    }
    if let Some(Some(handle)) = FILTER.get() {
        let _ = handle.modify(|f| *f = EnvFilter::new(directive_for(&level)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `apply_log_level` must be safe in every process: unknown levels
    /// never touch the filter, and a process that does not own the
    /// global subscriber (every test binary) no-ops instead of
    /// panicking on the missing handle.
    #[test]
    fn apply_log_level_is_safe_without_a_global_subscriber() {
        apply_log_level("debug");
        apply_log_level("  WARN  ");
        apply_log_level("bogus");
        apply_log_level("");
        apply_log_level("info");
    }
}
