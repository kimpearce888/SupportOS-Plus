//! SupportOS++ native shell entry point.
//!
//! Boot order: logging → single-instance lock → database with all boot
//! migrations → core event bus → boot services (sync engine + provider,
//! demo unless Help Scout credentials are configured, mirroring the Tauri
//! boot) → minimal loopback listener (webhook ingress + OAuth callback) →
//! eframe window. Everything heavy lives in the library so it stays
//! unit-testable without a window.

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

    let bus = spp_core::eventing::EventBus::default();
    let boot = spp_shell::build_boot_services(&db, &bus);

    // Minimal loopback listener (webhook ingress + OAuth callback only).
    // Port preference: the localhost port baked into the redirect URI
    // (Help Scout requires the redirect to match exactly), then
    // SPP_LOOPBACK_PORT, then an OS-assigned port that gets logged.
    let env = spp_core::config::load_env_config();
    let redirect_uri = env.helpscout.redirect_uri.clone();
    let configured_port = spp_core::loopback::port_from_redirect_uri(&redirect_uri)
        .or_else(|| {
            std::env::var("SPP_LOOPBACK_PORT")
                .ok()
                .and_then(|p| p.parse::<u16>().ok())
        })
        .unwrap_or(0);
    let demo_mode = boot.real.is_none();
    let loopback = match spp_core::loopback::LoopbackServer::start(
        db.clone(),
        boot.real.clone(),
        demo_mode,
        configured_port,
    ) {
        Ok(server) => {
            tracing::info!(
                port = server.port(),
                base_url = server.base_url(),
                "loopback listener ready (POST /webhook, GET /oauth/callback)"
            );
            Some(server)
        }
        Err(e) => {
            // The shell stays fully usable without inbound HTTP; webhook
            // events are also drained from the database at worker start.
            tracing::warn!(error = %e, "loopback listener could not be started");
            None
        }
    };
    // Served for the whole process lifetime.
    let _loopback = loopback;

    let options = spp_shell::ShellOptions {
        db,
        bus,
        data_dir: app_config.data_dir.clone(),
        sync: Some(boot.engine),
    };

    if let Err(e) = spp_shell::run(options) {
        tracing::error!(error = %e, "shell failed");
        eprintln!("SupportOs-Plus: shell failed: {e}");
        std::process::exit(1);
    }
}
