fn main() {
    spp_core::logging::init();
    tracing::info!("SupportOS-Plus native shell starting");

    spp_core::config::migrate_legacy_data_dir();

    let data_dir = spp_core::config::default_data_dir();
    tracing::info!(?data_dir, "data directory");

    spp_shell::fatal::install_panic_hook(data_dir.clone());

    let lock = match spp_shell::single_instance::acquire(&data_dir) {
        Ok(lock) => lock,
        Err(e) => {
            tracing::error!(error = %e, "cannot acquire the instance lock");
            spp_shell::fatal::fatal(&data_dir, "cannot start", &e);
            std::process::exit(1);
        }
    };
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

    if let Some(level) = db.with_read(|conn| {
        spp_core::settings::get_string(conn, "log_level")
            .ok()
            .flatten()
    }) {
        spp_core::logging::apply_log_level(&level);
    }

    let bus = spp_core::event_bus::EventBus::default();
    let boot = spp_shell::build_boot_services(&db, &bus);

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
    // AU2-13f: the log used to name a "webhook, embedding" timer set that
    // does not exist; name the real WorkerManager timer set instead.
    tracing::info!(
        "background workers started (job loop, adaptive auto-sync, maintenance, \
         stale-job sweep, notification/customer-event/intelligence sweeps, \
         connector refresh timers)"
    );
    let _workers = workers;

    let env = spp_core::config::load_env_config();
    let redirect_uri = env.helpscout.redirect_uri.clone();
    let configured_port = std::env::var("SPP_LOOPBACK_PORT")
        .ok()
        .and_then(|p| p.parse::<u16>().ok().filter(|p| *p > 0))
        .or_else(|| spp_core::loopback::port_from_redirect_uri(&redirect_uri))
        .unwrap_or(0);
    let loopback = match spp_core::loopback::LoopbackServer::start(
        db.clone(),
        boot.handle.clone(),
        configured_port,
    ) {
        Ok(server) => Some(server),
        Err(primary) => {
            match spp_core::loopback::LoopbackServer::start(db.clone(), boot.handle.clone(), 0) {
                Ok(server) => {
                    tracing::warn!(
                        error = %primary,
                        port = server.port(),
                        "loopback listener could not bind the configured port; \
                         bound an OS-assigned port instead"
                    );
                    Some(server)
                }
                Err(e) => {
                    tracing::warn!(error = %e, "loopback listener could not be started");
                    None
                }
            }
        }
    };
    if let Some(server) = &loopback {
        spp_core::loopback::record_bound_port(server.port());
        tracing::info!(
            port = server.port(),
            base_url = server.base_url(),
            "loopback listener ready (POST /webhook, GET /oauth/callback)"
        );
    }
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
        spp_shell::fatal::fatal(&data_dir, "shell failed", &e);
        std::process::exit(1);
    }
}
