#![forbid(unsafe_code)]
#![deny(rust_2018_idioms)]
#![warn(clippy::all)]
#![allow(clippy::module_name_repetitions, clippy::missing_errors_doc)]

use std::sync::{Arc, Mutex};

use spp_core::config::AppConfig;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    spp_core::logging::init();

    tracing::info!("SupportOS++ starting");

    let app_config = AppConfig::default();
    tracing::info!(?app_config.data_dir, "data directory");

    let db_path = app_config.data_dir.join("supportos-plusplus.db");
    let conn = match open_db_with_all_migrations(&db_path) {
        Ok(c) => c,
        Err(e) => {
            tracing::error!(error = %e, path = %db_path.display(), "failed to open SQLite DB");
            eprintln!("[supportos++] backend error: failed to open SQLite DB: {e}");
            std::process::exit(1);
        }
    };

    let http_port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3000);
    let demo_mode = spp_core::settings::get_bool(&conn, "demo_mode", false).unwrap_or(false)
        || std::env::var("LOCAL_DEMO_MODE").as_deref() == Ok("true");

    let http_conn = Arc::new(Mutex::new(conn));
    let http_bus = spp_core::http::EventBus::default();

    let credentials = spp_core::helpscout_real::HsCredentials::from_env();
    let use_real = !demo_mode && credentials.is_configured();
    let (provider, real, provider_kind): (
        Arc<dyn spp_core::helpscout::HelpScoutProvider>,
        Option<Arc<spp_core::helpscout_real::RealHelpScoutProvider>>,
        String,
    ) = if use_real {
        let real = Arc::new(spp_core::helpscout_real::RealHelpScoutProvider::new(
            http_conn.clone(),
            credentials,
        ));
        (
            real.clone() as Arc<dyn spp_core::helpscout::HelpScoutProvider>,
            Some(real),
            "real".to_string(),
        )
    } else {
        (
            Arc::new(spp_core::helpscout::FakeHelpScoutProvider::new_demo())
                as Arc<dyn spp_core::helpscout::HelpScoutProvider>,
            None,
            "fake".to_string(),
        )
    };
    let sync = Arc::new(
        spp_core::sync_engine::SyncEngine::new(http_conn.clone(), provider)
            .with_bus(http_bus.clone()),
    );

    let qdrant = spp_core::http::server::AppState::qdrant_from_settings(
        &http_conn.lock().unwrap_or_else(|p| p.into_inner()),
        &app_config.data_dir,
    );
    let http_state = spp_core::http::server::AppState {
        conn: http_conn,
        data_dir: app_config.data_dir.clone(),
        port: http_port,
        host: "127.0.0.1".to_string(),
        demo_mode,
        bus: http_bus,
        limiter: spp_core::http::RateLimiter::new(),
        sync: Some(sync),
        real,
        provider_kind,
        workers: None,
        qdrant,
    };

    let http_server = spp_core::http::HttpServer::new(http_state);
    let http_addr = http_server.addr();
    tracing::info!(%http_addr, "HTTP API server starting on the loopback interface");

    tauri::async_runtime::spawn(async move {
        if let Err(e) = http_server.serve().await {
            tracing::error!(error = %e, "HTTP API server failed");
        }
    });

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.set_focus();
            }
        }))
        .run(tauri::generate_context!())
        .expect("error while running SupportOS++");
}

fn open_db_with_all_migrations(
    path: &std::path::Path,
) -> spp_core::error::Result<rusqlite::Connection> {
    let mut conn = spp_core::db::open(path)?;
    spp_core::bootstrap::apply_all(&mut conn)?;
    tracing::info!("All migrations applied successfully");
    Ok(conn)
}
