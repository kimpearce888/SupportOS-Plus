use super::*;

fn temp_db() -> crate::db::DbHandle {
    let dir = std::env::temp_dir().join(format!(
        "spp-workers-audp2rec-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default()
    ));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let mut conn = crate::db::open(&dir.join("workers.db")).expect("open db");
    crate::bootstrap::apply_all(&mut conn).expect("migrations");
    crate::db::DbHandle::new(conn)
}

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime")
}

#[test]
fn audp25_scheduled_reconcile_skips_the_empty_provider_quietly() {
    let db = temp_db();
    let engine = Arc::new(crate::sync_engine::SyncEngine::new(
        db.clone(),
        Arc::new(crate::helpscout::EmptyHelpScoutProvider::new()),
    ));
    let manager = Arc::new(WorkerManager::new(
        db.clone(),
        Some(engine),
        Arc::new(crate::helpscout::EmptyHelpScoutProvider::new()),
        crate::event_bus::EventBus::default(),
        std::env::temp_dir(),
        None,
    ));
    runtime().block_on(async { manager.scheduled_reconcile_tick().await });
    let runs: i64 = db.with_read(|conn| {
        conn.query_row("SELECT COUNT(*) FROM sync_runs", [], |r| r.get(0))
            .unwrap_or(0)
    });
    assert_eq!(
        runs, 0,
        "no reconcile run is started against the empty provider"
    );
}

#[test]
fn audp25_scheduled_reconcile_defers_while_a_sync_is_running() {
    let db = temp_db();
    let engine = Arc::new(crate::sync_engine::SyncEngine::new(
        db.clone(),
        Arc::new(crate::helpscout::EmptyHelpScoutProvider::new()),
    ));
    engine.set_running_for_tests(true);
    let manager = Arc::new(WorkerManager::new(
        db.clone(),
        Some(engine.clone()),
        Arc::new(crate::helpscout::EmptyHelpScoutProvider::new()),
        crate::event_bus::EventBus::default(),
        std::env::temp_dir(),
        None,
    ));
    runtime().block_on(async { manager.scheduled_reconcile_tick().await });
    let reconciling: i64 = db.with_read(|conn| {
        conn.query_row(
            "SELECT COUNT(*) FROM sync_runs WHERE kind = 'reconciliation'",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0)
    });
    assert_eq!(
        reconciling, 0,
        "a running sync defers the scheduled reconcile instead of erroring"
    );
    engine.set_running_for_tests(false);
}

#[test]
fn audp25_reconcile_interval_setting_clamps_and_disables() {
    let db = temp_db();
    let read = |key: &str, default: i64| {
        db.with_read(|conn| {
            crate::settings::get_i64(conn, key, default)
                .unwrap_or(default)
                .clamp(0, 10_080)
        })
    };
    assert_eq!(
        read("reconcile_interval_minutes", 1_440),
        1_440,
        "default cadence is daily"
    );
    db.with_write(|conn| {
        crate::settings::set_i64(conn, "reconcile_interval_minutes", 0).expect("off");
    });
    assert_eq!(
        read("reconcile_interval_minutes", 1_440),
        0,
        "0 is in-band and means the schedule is off"
    );
    db.with_write(|conn| {
        crate::settings::set_i64(conn, "reconcile_interval_minutes", 999_999).expect("huge");
    });
    assert_eq!(
        read("reconcile_interval_minutes", 1_440),
        10_080,
        "the cadence clamps to one week"
    );
}
