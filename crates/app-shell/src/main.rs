//! SupportOS++ native shell entry point.
//!
//! Boot order: logging → single-instance lock → database with all boot
//! migrations → core event bus → eframe window. Everything heavy lives
//! in the library so it stays unit-testable without a window.

fn main() {
    spp_core::logging::init();
    tracing::info!("SupportOs-Plus native shell starting");

    let app_config = spp_core::config::AppConfig::default();
    tracing::info!(?app_config.data_dir, "data directory");

    let lock = match spp_shell::single_instance::acquire(&app_config.data_dir) {
        Ok(lock) => lock,
        Err(e) => {
            eprintln!("SupportOs-Plus: {e}");
            std::process::exit(1);
        }
    };
    // Held for the whole process lifetime; releasing is automatic on exit.
    let _lock = lock;

    let db_path = app_config.data_dir.join("supportos-plusplus.db");
    let db = match spp_shell::open_db(&db_path) {
        Ok(db) => db,
        Err(e) => {
            tracing::error!(error = %e, path = %db_path.display(), "failed to open SQLite DB");
            eprintln!("SupportOs-Plus: failed to open SQLite DB: {e}");
            std::process::exit(1);
        }
    };
    tracing::info!(path = %db_path.display(), "database ready (all migrations applied)");

    let bus = spp_core::http::EventBus::default();
    let options = spp_shell::ShellOptions {
        db,
        bus,
        data_dir: app_config.data_dir.clone(),
    };

    if let Err(e) = spp_shell::run(options) {
        tracing::error!(error = %e, "shell failed");
        eprintln!("SupportOs-Plus: shell failed: {e}");
        std::process::exit(1);
    }
}
