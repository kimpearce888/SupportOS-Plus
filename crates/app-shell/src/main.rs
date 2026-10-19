//! SupportOS+ (SupportOS-Plus) native shell entry point.
//!
//! Boot order: logging → single-instance lock → database with all boot
//! migrations → core event bus → boot services (sync engine + provider,
//! demo unless Help Scout credentials are configured, mirroring the Tauri
//! boot) → background worker host (job drain, webhook drain, embedding
//! pass, maintenance timers on a dedicated 2-thread runtime) → minimal
//! loopback listener (webhook ingress + OAuth callback) → eframe window.
//! Everything heavy lives in the library so it stays unit-testable
//! without a window.

fn main() {
    spp_core::logging::init();
    tracing::info!("SupportOS-Plus native shell starting");

    // One-time upgrade from the pre-1.0.0 data layout (whole-directory
    // move + database rename); no-op on fresh installs and repeat runs.
    spp_core::config::migrate_legacy_data_dir();

    let app_config = spp_core::config::AppConfig::default();
    tracing::info!(?app_config.data_dir, "data directory");

    let lock = match spp_shell::single_instance::acquire(&app_config.data_dir) {
        Ok(lock) => lock,
        Err(e) => {
            eprintln!("SupportOS-Plus: {e}");
            std::process::exit(1);
        }
    };
    // Held for the whole process lifetime; releasing is automatic on exit.
    let _lock = lock;

    let db_path = app_config.data_dir.join(spp_core::config::DB_FILE_NAME);
    let db = match spp_shell::open_db(&db_path) {
        Ok(db) => db,
        Err(e) => {
            tracing::error!(error = %e, path = %db_path.display(), "failed to open SQLite DB");
            eprintln!("SupportOS-Plus: failed to open SQLite DB: {e}");
            std::process::exit(1);
        }
    };
    tracing::info!(path = %db_path.display(), "database ready (all migrations applied)");

    let bus = spp_core::eventing::EventBus::default();
    let boot = spp_shell::build_boot_services(&db, &bus);

    // Background workers (cutover gap fix): job drain, webhook drain,
    // embedding batches and maintenance timers must run for the whole
    // session. The vector index rides along (enabled by default, store
    // opened lazily) so the embedding pass has somewhere to write.
    let vector_index = {
        let enabled = db
            .with_read(|conn| spp_core::settings::get_bool(conn, "qdrant_enabled", true))
            .unwrap_or(true);
        std::sync::Arc::new(spp_core::vectorstore_sqlitevec::VectorIndex::new(
            app_config.data_dir.clone(),
            enabled,
        ))
    };
    let workers = match spp_shell::workers_host::WorkersHost::start(
        db.clone(),
        Some(boot.engine.clone()),
        boot.provider.clone(),
        bus.clone(),
        app_config.data_dir.clone(),
        Some(vector_index),
    ) {
        Ok(host) => host,
        Err(e) => {
            tracing::error!(error = %e, "failed to start background workers");
            eprintln!("SupportOS-Plus: failed to start background workers: {e}");
            std::process::exit(1);
        }
    };
    tracing::info!("background workers started (job, webhook, embedding, maintenance timers)");
    // Stopped cleanly when main returns and the host drops.
    let _workers = workers;

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
        workers: Some(std::sync::Arc::new(_workers)),
    };

    // CI smoke hook (AppImage job): verifies the full boot chain —
    // migrations, boot services, background workers, loopback listener —
    // without opening a window, then exits 0. Never active in normal runs.
    if std::env::var("SPP_SMOKE").as_deref() == Ok("1") {
        tracing::info!(
            operation = "smoke",
            "SPP_SMOKE=1: boot chain verified, exiting"
        );
        return;
    }

    if let Err(e) = spp_shell::run(options) {
        tracing::error!(error = %e, "shell failed");
        eprintln!("SupportOS-Plus: shell failed: {e}");
        std::process::exit(1);
    }
}
