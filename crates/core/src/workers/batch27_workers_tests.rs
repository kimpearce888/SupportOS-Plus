use super::*;
use crate::helpscout::HelpScoutProvider;
use crate::mockserver::{MockRequest, MockResponse};

fn secrets_guard() -> std::sync::MutexGuard<'static, ()> {
    crate::settings::SECRETS_TEST_LOCK
        .lock()
        .unwrap_or_else(|p| p.into_inner())
}

fn temp_db() -> crate::db::DbHandle {
    let dir = std::env::temp_dir().join(format!(
        "spp-workers-b27-{}-{}",
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
fn draft_article_missing_candidate_fails_the_real_job_row() {
    let db = temp_db();
    let manager = manager_with(
        &db,
        Arc::new(crate::helpscout::EmptyHelpScoutProvider::new()),
    );
    let id = db.with_write(|conn| jobs::enqueue(conn, "draft_article", "{}").expect("enqueue"));
    run_one_tick(&manager);

    let (status, error, _) = job_state(&db, id);
    assert_eq!(status, "failed", "the real job row takes the failure");
    assert!(
        error
            .as_deref()
            .unwrap_or_default()
            .contains("no candidateId"),
        "the honest error is on the row: {error:?}"
    );
}

#[test]
fn fetch_hs_report_unknown_key_fails_the_real_job_row() {
    let db = temp_db();
    let manager = manager_with(
        &db,
        Arc::new(crate::helpscout::EmptyHelpScoutProvider::new()),
    );
    let id = db.with_write(|conn| {
        jobs::enqueue(
            conn,
            "fetch_hs_report",
            &serde_json::json!({"key": "not_a_real_key"}).to_string(),
        )
        .expect("enqueue")
    });
    run_one_tick(&manager);

    let (status, error, _) = job_state(&db, id);
    assert_eq!(status, "failed");
    assert!(
        error
            .as_deref()
            .unwrap_or_default()
            .contains("unknown report key"),
        "the honest error is on the row: {error:?}"
    );
}

#[test]
fn rating_fetch_failure_requeues_the_job_instead_of_swallowing() {
    let _secrets = secrets_guard();
    crate::settings::set_master_key_for_tests([27u8; 32]);
    let db = temp_db();
    let mock = crate::mockserver::MockServer::start(std::sync::Arc::new(
        |req: &MockRequest| -> MockResponse {
            if req.method == "GET" && req.path == "/v2/ratings/501" {
                MockResponse::json(500, serde_json::json!({"error": "boom"}))
            } else {
                crate::mockserver::not_found()(req)
            }
        },
    ))
    .expect("mock");
    let provider = crate::helpscout_real::RealHelpScoutProvider::new(
        db.clone(),
        crate::helpscout_real::HsCredentials {
            client_id: "b27-client".into(),
            client_secret: "b27-secret".into(),
            redirect_uri: format!("{}/oauth/callback", mock.url()),
            api_base: mock.url(),
            webhook_secret: "b27-hook".into(),
            docs_api_key: String::new(),
            docs_api_base: mock.url(),
        },
    );
    provider
        .save_tokens("b27-token", None, Some(3_600), None)
        .expect("seed token");
    let manager = manager_with(&db, Arc::new(provider));
    let id = db.with_write(|conn| {
        jobs::enqueue(
            conn,
            "sync_conversation_ratings",
            &serde_json::json!({"ratingId": 501}).to_string(),
        )
        .expect("enqueue")
    });
    run_one_tick(&manager);

    let (status, error, attempt) = job_state(&db, id);
    assert_eq!(
        status, "queued",
        "the fetch failure requeues the job for a retry"
    );
    assert_eq!(attempt, 1, "one attempt spent");
    assert!(
        error.is_some(),
        "the failure is recorded on the row, not swallowed: {error:?}"
    );
    let ratings: i64 = db.with_read(|conn| {
        conn.query_row("SELECT COUNT(*) FROM ratings", [], |r| r.get(0))
            .unwrap_or(0)
    });
    assert_eq!(ratings, 0, "nothing stored yet — the retry will fetch it");
}

#[test]
fn panicking_job_fails_terminally_instead_of_requeueing() {
    let db = temp_db();
    let manager = manager_with(
        &db,
        Arc::new(crate::helpscout::EmptyHelpScoutProvider::new()),
    );
    let id = db.with_write(|conn| jobs::enqueue(conn, "__test_panic", "{}").expect("enqueue"));
    run_one_tick(&manager);

    let (status, error, attempt) = job_state(&db, id);
    assert_eq!(
        status, "failed",
        "a contained panic is terminal, not retryable"
    );
    assert_eq!(attempt, 1, "exactly one attempt — no re-runs");
    assert!(
        error.as_deref().unwrap_or_default().contains("panicked"),
        "the honest panic error sits on the row: {error:?}"
    );

    run_one_tick(&manager);
    let (status, _, attempt) = job_state(&db, id);
    assert_eq!(status, "failed", "the failed job stays failed");
    assert_eq!(attempt, 1, "no second execution of the panicking job");
}
