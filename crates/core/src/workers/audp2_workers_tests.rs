use super::*;

fn temp_db() -> (crate::db::DbHandle, crate::testkit::TempDir) {
    let dir = crate::testkit::TempDir::new("spp-workers-audp2");
    let mut conn = crate::db::open(&dir.join("workers.db")).expect("open db");
    crate::bootstrap::apply_all(&mut conn).expect("migrations");
    (crate::db::DbHandle::new(conn), dir)
}

fn manager_with(
    db: &crate::db::DbHandle,
    provider: Arc<dyn HelpScoutProvider>,
) -> Arc<WorkerManager> {
    Arc::new(WorkerManager::new(
        db.clone(),
        None,
        provider,
        crate::event_bus::EventBus::default(),
        std::env::temp_dir(),
        None,
    ))
}

fn job_state(db: &crate::db::DbHandle, id: i64) -> (String, Option<String>, i64) {
    db.with_read(|conn| {
        conn.query_row(
            "SELECT status, error, attempt FROM jobs WHERE id = ?1",
            rusqlite::params![id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .expect("job row")
    })
}

fn run_one_tick(manager: &Arc<WorkerManager>) {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    rt.block_on(async { manager.tick().await });
}

#[test]
fn audp21_lm_error_recovers_the_typed_retryable_flag() {
    use crate::ai_pipeline::{LmStudioError, lm_error};
    use crate::error::Error;
    let typed = Error::Other(Box::new(LmStudioError::new(
        "model output unparseable",
        false,
    )));
    let lm = lm_error(&typed).expect("typed error downcasts");
    assert!(!lm.retryable, "deterministic AI failures are terminal");
    let transport = Error::Other(Box::new(LmStudioError::new("connection refused", true)));
    assert!(
        lm_error(&transport).expect("transport downcasts").retryable,
        "transport failures retry"
    );
    let plain = Error::Other("a plain boxed string".into());
    assert!(
        lm_error(&plain).is_none(),
        "untyped errors carry no flag — the caller keeps its default"
    );
    assert!(
        lm_error(&Error::Config("not a box".into())).is_none(),
        "non-Other variants are not LmStudioErrors"
    );
}

#[test]
fn audp21_deterministic_ai_failure_fails_the_job_terminally() {
    let (db, _scratch) = temp_db();
    let manager = manager_with(
        &db,
        Arc::new(crate::helpscout::EmptyHelpScoutProvider::new()),
    );
    let id = db.with_write(|conn| {
        jobs::enqueue(
            conn,
            "analyze_ticket",
            &serde_json::json!({"conversationId": 424242}).to_string(),
        )
        .expect("enqueue")
    });
    run_one_tick(&manager);

    let (status, error, attempt) = job_state(&db, id);
    assert_eq!(
        status, "failed",
        "a deterministic 'conversation not found' must not burn all attempts (A7)"
    );
    assert_eq!(attempt, 1, "exactly one attempt spent");
    assert!(
        error.as_deref().unwrap_or_default().contains("not found"),
        "the honest error sits on the row: {error:?}"
    );
}

#[test]
fn audp21_transport_ai_failure_still_requeues_for_retry() {
    let (db, _scratch) = temp_db();
    db.with_write(|conn| {
        crate::settings::set_string(conn, "lmstudio_base_url", "http://127.0.0.1:9/v1")
            .expect("base url");
        crate::settings::set_i64(conn, "lmstudio_timeout_ms", 2_000).expect("timeout");
        conn.execute(
            "INSERT INTO conversations (remote_id, number, subject, preview, status, remote_created_at)
             VALUES (7001, 7001, 'Retry semantics', 'p', 'active', '2026-10-20T08:00:00.000Z')",
            [],
        )
        .expect("conversation");
    });
    let manager = manager_with(
        &db,
        Arc::new(crate::helpscout::EmptyHelpScoutProvider::new()),
    );
    let id = db.with_write(|conn| {
        let conv: i64 = conn
            .query_row(
                "SELECT id FROM conversations WHERE remote_id = 7001",
                [],
                |r| r.get(0),
            )
            .expect("seeded");
        jobs::enqueue(
            conn,
            "analyze_ticket",
            &serde_json::json!({"conversationId": conv}).to_string(),
        )
        .expect("enqueue")
    });
    run_one_tick(&manager);

    let (status, error, attempt) = job_state(&db, id);
    assert_eq!(
        status, "queued",
        "a transport failure is retryable — the job requeues (A7 keeps this side)"
    );
    assert_eq!(attempt, 1, "one attempt spent so far");
    assert!(
        error.is_some(),
        "the failure is recorded on the row: {error:?}"
    );
}
