//! SupportOS+ (SupportOS-Plus) native shell entry point.
//!
//! Boot order: logging → single-instance lock → database with all boot
//! migrations → core event bus → boot services (sync engine + provider —
//! real when Help Scout credentials are configured, empty otherwise,
//! hot-swappable in-session since 1.4.0) → background worker host (job
//! drain, webhook drain, embedding pass, maintenance timers on a dedicated
//! 2-thread runtime) → minimal loopback listener (webhook ingress + OAuth
//! callback) → eframe window.
//! Everything heavy lives in the library so it stays unit-testable
//! without a window.

fn main() {
    spp_core::logging::init();
    tracing::info!("SupportOS-Plus native shell starting");

    // One-time upgrade from the pre-1.0.0 data layout (whole-directory
    // move + database rename); no-op on fresh installs and repeat runs.
    spp_core::config::migrate_legacy_data_dir();

    // Clean-audit item 153: the AppConfig struct carried four fields
    // nothing consumed; only `data_dir` was ever read.
    let data_dir = spp_core::config::default_data_dir();
    tracing::info!(?data_dir, "data directory");

    // v1.1.1: from here on, panics also land in logs/fatal.log. Desktop
    // entries discard stderr, so without this a renderer or context
    // panic leaves nothing behind ("opened it, nothing appeared").
    spp_shell::fatal::install_panic_hook(data_dir.clone());

    let lock = match spp_shell::single_instance::acquire(&data_dir) {
        Ok(lock) => lock,
        Err(e) => {
            tracing::error!(error = %e, "cannot acquire the instance lock");
            spp_shell::fatal::fatal(&data_dir, "cannot start", &e);
            std::process::exit(1);
        }
    };
    // Held for the whole process lifetime; releasing is automatic on exit.
    let _lock = lock;

    let db_path = data_dir.join(spp_core::config::DB_FILE_NAME);
    let db = match spp_shell::open_db(&db_path) {
        Ok(db) => db,
        Err(e) => {
            tracing::error!(error = %e, path = %db_path.display(), "failed to open SQLite DB");
            spp_shell::fatal::fatal(&data_dir, "failed to open SQLite DB", &e);
            std::process::exit(1);
        }
    };
    tracing::info!(path = %db_path.display(), "database ready (all migrations applied)");

    // Clean-audit item 138: the settings page's `log_level` row drives
    // the real subscriber — applied at boot from the stored setting (the
    // env/LOG_LEVEL default stood in until the database was readable),
    // and live on every settings save through the same handle.
    if let Some(level) = db.with_read(|conn| {
        spp_core::settings::get_string(conn, "log_level")
            .ok()
            .flatten()
    }) {
        spp_core::logging::apply_log_level(&level);
    }

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
            data_dir.clone(),
            enabled,
        ))
    };
    let workers = match spp_shell::workers_host::WorkersHost::start(
        db.clone(),
        Some(boot.engine.clone()),
        boot.provider.clone(),
        bus.clone(),
        data_dir.clone(),
        Some(vector_index.clone()),
    ) {
        Ok(host) => host,
        Err(e) => {
            tracing::error!(error = %e, "failed to start background workers");
            spp_shell::fatal::fatal(&data_dir, "failed to start background workers", &e);
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
    let loopback = match spp_core::loopback::LoopbackServer::start(
        db.clone(),
        boot.handle.clone(),
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
        data_dir: data_dir.clone(),
        sync: Some(boot.engine),
        handle: Some(boot.handle),
        workers: Some(std::sync::Arc::new(_workers)),
        vector_index: Some(vector_index),
    };

    // CI smoke hook (AppImage/deb jobs): verifies the full boot chain —
    // migrations, boot services, background workers, loopback listener —
    // and, since clean-audit item 71, the SHELL itself: the real
    // ShellApp is constructed headlessly and renders one frame per page
    // (the same tessellation path eframe drives), so renderer
    // regressions fail the smoke instead of passing CI unexercised.
    // Never active in normal runs.
    if std::env::var("SPP_SMOKE").as_deref() == Ok("1") {
        if let Err(e) = spp_shell::smoke::run_shell_smoke(options) {
            tracing::error!(error = %e, "SPP_SMOKE: shell smoke failed");
            spp_shell::fatal::fatal(&data_dir, "smoke failed", e);
            std::process::exit(1);
        }
        tracing::info!(
            operation = "smoke",
            "SPP_SMOKE=1: boot chain + shell/renderer verified, exiting"
        );
        return;
    }

    if let Err(e) = spp_shell::run(options) {
        tracing::error!(error = %e, "shell failed");
        // No window exists at this point; make the failure findable
        // (fatal.log) and visible (dialog) instead of a silent exit.
        spp_shell::fatal::fatal(&data_dir, "shell failed", &e);
        std::process::exit(1);
    }
}
