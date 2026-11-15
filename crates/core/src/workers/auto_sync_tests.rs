use super::*;

fn temp_db() -> crate::db::DbHandle {
    let dir = std::env::temp_dir().join(format!(
        "spp-workers-autosync-{}-{}",
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

#[test]
fn auto_sync_skips_the_empty_provider_and_stamps_nothing() {
    let db = temp_db();
    db.with_write(|conn| crate::sync_engine::set_state(conn, "LIVE"));
    let engine = Arc::new(crate::sync_engine::SyncEngine::new(
        db.clone(),
        Arc::new(crate::helpscout::EmptyHelpScoutProvider::new()),
    ));
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    let _enter = rt.enter();
    let manager = start_workers(
        db.clone(),
        Some(engine),
        Arc::new(crate::helpscout::EmptyHelpScoutProvider::new()),
        crate::event_bus::EventBus::default(),
        std::env::temp_dir(),
        None,
    );
    rt.block_on(manager.auto_sync());

    let (state, runs, checkpoints) = db.with_read(|conn| {
        (
            crate::sync_engine::get_state(conn),
            conn.query_row("SELECT COUNT(*) FROM sync_runs", [], |r| r.get::<_, i64>(0))
                .unwrap_or(0),
            conn.query_row(
                "SELECT COUNT(*) FROM sync_checkpoints WHERE status = 'ok'",
                [],
                |r| r.get::<_, i64>(0),
            )
            .unwrap_or(0),
        )
    });
    assert_eq!(state, "LIVE", "the timer must not touch the state");
    assert_eq!(runs, 0, "the timer must not start a run");
    assert_eq!(checkpoints, 0, "the timer must not stamp watermarks");
    manager.stop();
}
