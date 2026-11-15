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
        "spp-workers-b14-{}-{}",
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

fn creds_for(
    mock: &crate::mockserver::MockServer,
    docs_key: &str,
) -> crate::helpscout_real::HsCredentials {
    crate::helpscout_real::HsCredentials {
        client_id: "b14-client".into(),
        client_secret: "b14-secret".into(),
        redirect_uri: format!("{}/oauth/callback", mock.url()),
        api_base: mock.url(),
        webhook_secret: "b14-hook".into(),
        docs_api_key: docs_key.into(),
        docs_api_base: mock.url(),
    }
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

fn docs_router() -> crate::mockserver::Handler {
    std::sync::Arc::new(|req: &MockRequest| -> MockResponse {
        match (req.method.as_str(), req.path.as_str()) {
            ("POST", "/v1/articles") => MockResponse::json(
                200,
                serde_json::json!({
                    "article": {
                        "id": "abc909abc909abc909abc909", "status": "notpublished",
                        "name": "Fix the export timeout",
                        "collectionId": 55
                    }
                }),
            ),
            ("PUT", p)
                if p == "/v1/articles/abc909abc909abc909abc909"
                    || p == "/v1/articles/abc909abc909abc909abc909/drafts" =>
            {
                MockResponse::json(
                    200,
                    serde_json::json!({"article": {"id": "abc909abc909abc909abc909", "status": "published"}}),
                )
            }
            ("GET", "/v1/articles/abc909abc909abc909abc909") => MockResponse::json(
                200,
                serde_json::json!({
                    "article": {
                        "id": "abc909abc909abc909abc909", "status": "published",
                        "name": "Fix the export timeout",
                        "text": "Clear the export cache and re-run.",
                        "collectionId": 55,
                        "slug": "fix-export-timeout",
                        "number": 4,
                        "viewCount": 0,
                        "createdDate": "2026-10-21T00:00:00Z",
                        "modifiedDate": "2026-10-21T00:00:00Z"
                    }
                }),
            ),
            _ => crate::mockserver::not_found()(req),
        }
    })
}

fn seed_approved_draft(db: &crate::db::DbHandle) -> (i64, i64) {
    db.with_write(|conn| {
        conn.execute(
            "INSERT INTO docs_collections (remote_id, name) VALUES ('col55abc55abc55abc55abc', 'Troubleshooting')",
            [],
        )
        .expect("collection");
        let collection_local = conn.last_insert_rowid();
        let draft_id = crate::knowledge_store::insert_article_draft(
            conn,
            None,
            "Fix the export timeout",
            "Clear the export cache and re-run.",
            &serde_json::json!([]),
            None,
        )
        .expect("draft");
        crate::knowledge_store::decide_article_draft(conn, draft_id, "approved", None)
            .expect("approve");
        (draft_id, collection_local)
    })
}

#[test]
fn publish_job_runs_the_full_chain_and_verifies() {
    let _secrets = secrets_guard();
    crate::settings::set_master_key_for_tests([11u8; 32]);
    let db = temp_db();
    let mock = crate::mockserver::MockServer::start(docs_router()).expect("mock");
    let provider = crate::helpscout_real::RealHelpScoutProvider::new(
        db.clone(),
        creds_for(&mock, "docs-key-1"),
    );
    provider
        .save_tokens("b14-token", None, Some(3_600), None)
        .expect("seed token");
    let (draft_id, collection_local) = seed_approved_draft(&db);
    db.with_write(|conn| {
        jobs::enqueue(
            conn,
            "publish_article_draft",
            &serde_json::json!({"draftId": draft_id, "collectionLocalId": collection_local})
                .to_string(),
        )
        .expect("enqueue");
    });

    let manager = manager_with(&db, Arc::new(provider));
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    rt.block_on(async { manager.tick().await });

    let (state, remote, mirrored): (String, Option<String>, Option<String>) =
        db.with_read(|conn| {
            conn.query_row(
                "SELECT publish_state, remote_article_id,
                    (SELECT text FROM docs_articles WHERE remote_id = 'abc909abc909abc909abc909')
             FROM article_drafts WHERE id = ?1",
                rusqlite::params![draft_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap_or(("missing".into(), None, None))
        });
    assert_eq!(state, "verified", "the chain must reach verified");
    assert_eq!(remote.as_deref(), Some("abc909abc909abc909abc909"));
    assert!(
        mirrored
            .as_deref()
            .unwrap_or_default()
            .contains("export cache"),
        "the verify re-sync mirrored the published text"
    );

    let hits = mock.hits();
    let saw = |m: &str, p: &str| {
        hits.iter().any(|h| {
            h.method == m
                && h.path == p
                && h.header("authorization")
                    .is_some_and(|a| a.starts_with("Basic "))
        })
    };
    assert!(saw("POST", "/v1/articles"), "create must run first");
    assert!(
        saw("PUT", "/v1/articles/abc909abc909abc909abc909"),
        "publish"
    );
    assert!(
        saw("GET", "/v1/articles/abc909abc909abc909abc909"),
        "verify re-sync"
    );

    db.with_write(|conn| {
        jobs::enqueue(
            conn,
            "publish_article_draft",
            &serde_json::json!({"draftId": draft_id, "collectionLocalId": collection_local})
                .to_string(),
        )
        .expect("re-enqueue");
    });
    rt.block_on(async { manager.tick().await });
    let hits = mock.hits();
    let saw = |m: &str, p: &str| {
        hits.iter().any(|h| {
            h.method == m
                && h.path == p
                && h.header("authorization")
                    .is_some_and(|a| a.starts_with("Basic "))
        })
    };
    assert!(
        saw("PUT", "/v1/articles/abc909abc909abc909abc909/drafts"),
        "the linked-article update upserts the draft copy"
    );
    let state: String = db.with_read(|conn| {
        conn.query_row(
            "SELECT publish_state FROM article_drafts WHERE id = ?1",
            rusqlite::params![draft_id],
            |r| r.get(0),
        )
        .unwrap_or("missing".into())
    });
    assert_eq!(state, "verified", "the update run re-verifies");
}

#[test]
fn publish_job_refuses_an_unapproved_draft() {
    let _secrets = secrets_guard();
    crate::settings::set_master_key_for_tests([12u8; 32]);
    let db = temp_db();
    let mock = crate::mockserver::MockServer::start(docs_router()).expect("mock");
    let provider = crate::helpscout_real::RealHelpScoutProvider::new(
        db.clone(),
        creds_for(&mock, "docs-key-2"),
    );
    let draft_id = db.with_write(|conn| {
        crate::knowledge_store::insert_article_draft(
            conn,
            None,
            "Draft still in review",
            "body",
            &serde_json::json!([]),
            None,
        )
        .expect("draft")
    });
    db.with_write(|conn| {
        jobs::enqueue(
            conn,
            "publish_article_draft",
            &serde_json::json!({"draftId": draft_id, "collectionLocalId": 1}).to_string(),
        )
        .expect("enqueue");
    });
    let manager = manager_with(&db, Arc::new(provider));
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    rt.block_on(async { manager.tick().await });

    let (state, status): (String, String) = db.with_read(|conn| {
        conn.query_row(
            "SELECT publish_state, status FROM article_drafts WHERE id = ?1",
            rusqlite::params![draft_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap_or(("missing".into(), "missing".into()))
    });
    assert_eq!(state, "unpublished");
    assert_eq!(status, "drafted");
    assert!(mock.hits().is_empty(), "nothing reached the Docs API");
    let failed: i64 = db.with_read(|conn| {
        conn.query_row(
            "SELECT COUNT(*) FROM jobs WHERE type = 'publish_article_draft' AND status = 'failed'",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0)
    });
    assert_eq!(failed, 1, "the refusal fails the job honestly");
}

#[test]
fn fetch_hs_report_job_classifies_capability() {
    let _secrets = secrets_guard();
    crate::settings::set_master_key_for_tests([13u8; 32]);
    let db = temp_db();
    let mock = crate::mockserver::MockServer::start(std::sync::Arc::new(
        |req: &MockRequest| -> MockResponse {
            match req.path.as_str() {
                "/v2/reports/happiness" => MockResponse::json(
                    200,
                    serde_json::json!({
                        "current": {"great": 20, "okay": 3, "notGood": 1},
                        "previous": {"great": 12, "okay": 4, "notGood": 2}
                    }),
                ),
                "/v2/reports/company" => {
                    MockResponse::json(403, serde_json::json!({"message": "forbidden"}))
                }
                _ => crate::mockserver::not_found()(req),
            }
        },
    ))
    .expect("mock");
    let provider =
        crate::helpscout_real::RealHelpScoutProvider::new(db.clone(), creds_for(&mock, ""));
    provider
        .save_tokens("b14-token", None, Some(3_600), None)
        .expect("seed token");
    let manager = manager_with(&db, Arc::new(provider));
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");

    db.with_write(|conn| {
        jobs::enqueue(
            conn,
            "fetch_hs_report",
            &serde_json::json!({
                "key": "happiness",
                "start": "2026-10-14T00:00:00Z",
                "end": "2026-10-21T00:00:00Z"
            })
            .to_string(),
        )
        .expect("enqueue happiness");
    });
    rt.block_on(async { manager.tick().await });
    assert_eq!(
        db.with_read(crate::reports_api::get_capability),
        crate::reports_api::ReportsCapability::Available
    );
    let (_, _, _, data) = db
        .with_read(|conn| crate::reports_api::cached_report(conn, "happiness"))
        .expect("cached");
    assert_eq!(data["current"]["great"], 20);

    db.with_write(|conn| {
        jobs::enqueue(
            conn,
            "fetch_hs_report",
            &serde_json::json!({
                "key": "company",
                "start": "2026-10-14T00:00:00Z",
                "end": "2026-10-21T00:00:00Z"
            })
            .to_string(),
        )
        .expect("enqueue company");
    });
    rt.block_on(async { manager.tick().await });
    assert_eq!(
        db.with_read(crate::reports_api::get_capability),
        crate::reports_api::ReportsCapability::PlanBlocked,
        "the 403 wall must set the honest plan-blocked verdict"
    );
}

#[test]
fn fetch_hs_report_job_records_not_connected() {
    let db = temp_db();
    let manager = manager_with(
        &db,
        Arc::new(crate::helpscout::EmptyHelpScoutProvider::new()),
    );
    db.with_write(|conn| {
        jobs::enqueue(
            conn,
            "fetch_hs_report",
            &serde_json::json!({"key": "happiness", "start": "s", "end": "e"}).to_string(),
        )
        .expect("enqueue");
    });
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    rt.block_on(async { manager.tick().await });
    assert_eq!(
        db.with_read(crate::reports_api::get_capability),
        crate::reports_api::ReportsCapability::NotConnected
    );
}

#[test]
fn draft_article_job_skips_without_a_routed_backend() {
    let db = temp_db();
    let manager = manager_with(
        &db,
        Arc::new(crate::helpscout::EmptyHelpScoutProvider::new()),
    );
    db.with_write(|conn| {
        jobs::enqueue_on(conn, "ai", "draft_article", r#"{"candidateId": 1}"#, 3, 2)
            .expect("enqueue");
    });
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    rt.block_on(async { manager.tick().await });
    let drafts: i64 = db.with_read(|conn| {
        conn.query_row("SELECT COUNT(*) FROM article_drafts", [], |r| r.get(0))
            .unwrap_or(0)
    });
    assert_eq!(drafts, 0);
}

#[test]
fn weekly_digest_job_writes_the_narrative_end_to_end() {
    let _secrets = secrets_guard();
    crate::settings::set_master_key_for_tests([15u8; 32]);
    let db = temp_db();
    let mock = crate::mockserver::MockServer::start(std::sync::Arc::new(
        |req: &MockRequest| -> MockResponse {
            if req.method == "POST" && req.path == "/v1/chat/completions" {
                return MockResponse::json(
                    200,
                    serde_json::json!({
                        "model": "test-model",
                        "choices": [{
                            "message": {
                                "role": "assistant",
                                "content": "{\"narrative\": \"Volume held steady; the export-timeout cluster dominated; one volume spike stands out and the backlog grew.\"}"
                            },
                            "finish_reason": "stop"
                        }],
                        "usage": {"prompt_tokens": 100, "completion_tokens": 40, "total_tokens": 140}
                    }),
                );
            }
            crate::mockserver::not_found()(req)
        },
    ))
    .expect("mock");
    db.with_write(|conn| {
        crate::settings::set_string(conn, "lmstudio_base_url", &mock.url()).expect("base url");
    });
    let manager = manager_with(
        &db,
        Arc::new(crate::helpscout::EmptyHelpScoutProvider::new()),
    );
    db.with_write(|conn| {
        jobs::enqueue_on(
            conn,
            "ai",
            "weekly_digest",
            r#"{"triggerKind":"manual","days":7}"#,
            3,
            2,
        )
        .expect("enqueue");
    });
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    rt.block_on(async { manager.tick().await });
    let rows = db.with_read(|conn| crate::digest::list_digests(conn, None, 5));
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].status, "complete", "facts + AI narrative archived");
    assert_eq!(rows[0].trigger_kind, "manual");
    assert!(
        rows[0]
            .narrative
            .as_deref()
            .unwrap_or_default()
            .contains("export-timeout"),
        "the narrative came from the model"
    );
    let runs: i64 = db.with_read(|conn| {
        conn.query_row(
            "SELECT COUNT(*) FROM ai_runs WHERE type = 'weekly_digest' AND status = 'completed'",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0)
    });
    assert_eq!(runs, 1, "the model call is in the ledger");
}

#[test]
fn satisfaction_ratings_job_fetches_and_stores_the_rating() {
    let _secrets = secrets_guard();
    crate::settings::set_master_key_for_tests([14u8; 32]);
    let db = temp_db();
    let mock = crate::mockserver::MockServer::start(std::sync::Arc::new(
        |req: &MockRequest| -> MockResponse {
            if req.method == "GET" && req.path == "/v2/ratings/501" {
                MockResponse::json(
                    200,
                    serde_json::json!({
                        "id": 501,
                        "conversationId": 900,
                        "rating": "great",
                        "comments": "Fast, clear help",
                        "customer": {"id": 77, "firstName": "Rina", "lastName": "Chowdhury"},
                        "createdAt": "2026-10-21T01:02:03Z"
                    }),
                )
            } else {
                crate::mockserver::not_found()(req)
            }
        },
    ))
    .expect("mock");
    db.with_write(|conn| {
        conn.execute(
            "INSERT INTO conversations (remote_id, number, subject, preview, status, remote_created_at)
             VALUES (900, 900, 'Rated conversation', 'p', 'closed', '2026-10-20T08:00:00.000Z')",
            [],
        )
        .expect("conversation");
    });
    let provider =
        crate::helpscout_real::RealHelpScoutProvider::new(db.clone(), creds_for(&mock, ""));
    provider
        .save_tokens("b14-token", None, Some(3_600), None)
        .expect("seed token");
    let manager = manager_with(&db, Arc::new(provider));
    db.with_write(|conn| {
        jobs::enqueue(
            conn,
            "sync_conversation_ratings",
            &serde_json::json!({"ratingId": 501}).to_string(),
        )
        .expect("enqueue");
    });
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    rt.block_on(async { manager.tick().await });

    let (rating, comment, conv_number): (String, Option<String>, Option<i64>) =
        db.with_read(|conn| {
            conn.query_row(
                "SELECT r.rating, r.comments, c.number
                   FROM ratings r
                   LEFT JOIN conversations c ON c.id = r.conversation_id
                  WHERE r.remote_id = 501",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap_or(("missing".into(), None, None))
        });
    assert_eq!(rating, "great");
    assert_eq!(comment.as_deref(), Some("Fast, clear help"));
    assert_eq!(conv_number, Some(900));
    assert!(
        mock.hits().iter().any(|h| h.path == "/v2/ratings/501"),
        "the rating is fetched by id from Help Scout"
    );
}
