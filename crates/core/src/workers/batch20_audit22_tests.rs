use super::*;

fn temp_db() -> (crate::db::DbHandle, crate::testkit::TempDir) {
    let dir = crate::testkit::TempDir::new("spp-workers-b20");
    let mut conn = crate::db::open(&dir.join("workers.db")).expect("open db");
    crate::bootstrap::apply_all(&mut conn).expect("migrations");
    (crate::db::DbHandle::new(conn), dir)
}

fn manager(db: &crate::db::DbHandle) -> Arc<WorkerManager> {
    Arc::new(WorkerManager::new(
        db.clone(),
        None,
        Arc::new(crate::helpscout::EmptyHelpScoutProvider::new()),
        crate::event_bus::EventBus::default(),
        std::env::temp_dir(),
        None,
    ))
}

fn rebuild_activity_jobs(db: &crate::db::DbHandle) -> i64 {
    db.with_read(|conn| {
        conn.query_row(
            "SELECT COUNT(*) FROM jobs WHERE type = 'rebuild_activity'",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0)
    })
}

fn ai_job_count(db: &crate::db::DbHandle) -> i64 {
    db.with_read(|conn| {
        conn.query_row("SELECT COUNT(*) FROM jobs WHERE queue = 'ai'", [], |r| {
            r.get(0)
        })
        .unwrap_or(0)
    })
}

#[test]
fn audp17_no_backend_configured_means_no_ai_jobs() {
    let (db, _scratch) = temp_db();
    db.with_write(|conn| {
        conn.execute(
            "INSERT INTO conversations (remote_id, number, subject, preview, status)
             VALUES (9501, 95, 'Fresh install', 'preview', 'active')",
            [],
        )
        .expect("conversation");
    });
    assert!(
        !db.with_read(crate::ai_hub::backend_configured),
        "precondition: a fresh mirror has no AI backend configured"
    );

    let manager = manager(&db);
    manager.on_after_initial_sync();
    assert_eq!(
        ai_job_count(&db),
        0,
        "no backend configured — no doomed analyze/attribute jobs"
    );
    assert_eq!(rebuild_activity_jobs(&db), 1);

    db.with_write(|conn| {
        let _ = conn.execute("DELETE FROM jobs", []);
    });
    manager.on_conversation_changed(1);
    assert_eq!(
        ai_job_count(&db),
        0,
        "the per-conversation fan-out is gated as well"
    );
}

#[test]
fn audp17_a_configured_backend_restores_the_fan_out() {
    let (db, _scratch) = temp_db();
    db.with_write(|conn| {
        conn.execute(
            "INSERT INTO conversations (remote_id, number, subject, preview, status)
             VALUES (9601, 96, 'Configured', 'preview', 'active')",
            [],
        )
        .expect("conversation");
        crate::settings::set_string(conn, "lmstudio_base_url", "http://127.0.0.1:1234/v1")
            .expect("legacy url");
    });
    assert!(db.with_read(crate::ai_hub::backend_configured));

    let manager = manager(&db);
    manager.on_after_initial_sync();
    assert!(
        ai_job_count(&db) >= 1,
        "a configured backend restores the AI job fan-out"
    );
}

#[test]
fn audit22_rebuild_activity_is_enqueued_after_initial_sync() {
    let (db, _scratch) = temp_db();
    let manager = manager(&db);
    manager.on_after_initial_sync();
    assert_eq!(
        rebuild_activity_jobs(&db),
        1,
        "the activity rebuild must ride along with the post-sync jobs"
    );
}

#[test]
fn audit22_repair_runs_once_and_only_for_existing_mirrors() {
    let (db, _scratch) = temp_db();

    db.with_write(|conn| enqueue_activity_rebuild_repair_once(conn));
    assert_eq!(rebuild_activity_jobs(&db), 0, "nothing to rebuild yet");
    assert_eq!(
        db.with_read(|conn| {
            crate::settings::get_i64(conn, "activity_rebuild_repaired", 0).unwrap_or(0)
        }),
        0,
        "the marker must not be consumed by an empty database"
    );

    db.with_write(|conn| {
        conn.execute(
            "INSERT INTO conversations (remote_id, number, subject, preview, status)
             VALUES (9401, 94, 'Pre-fix mirror', 'preview', 'active')",
            [],
        )
        .expect("conversation")
    });
    db.with_write(|conn| enqueue_activity_rebuild_repair_once(conn));
    assert_eq!(rebuild_activity_jobs(&db), 1);

    db.with_write(|conn| enqueue_activity_rebuild_repair_once(conn));
    assert_eq!(
        rebuild_activity_jobs(&db),
        1,
        "the repair is strictly one-time"
    );
}
