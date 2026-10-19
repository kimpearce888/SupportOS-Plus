//! Integration tests (TODO 45): `SyncEngine` round-trips against a local
//! mock Help Scout API server.
//!
//! One realistic fixture dataset (two mailboxes, three customers, three
//! conversations with threads, paged tags, teams, workflows, user
//! statuses…) is served over TCP by [`crate::mockserver`]; the engine's
//! `initial_sync`, `incremental_sync` and `sync_single_conversation` run
//! exactly as in production (same `RealHelpScoutProvider`, same
//! `ApiQueue`, same DB write path through `DbHandle`), and the resulting
//! mirror tables are asserted row-by-row. No credentials and no network
//! access to Help Scout are involved.

use std::sync::Arc;

use rusqlite::Connection;
use serde_json::{Value, json};

use crate::db::DbHandle;
use crate::helpscout_real::{HsCredentials, RealHelpScoutProvider};
use crate::mockserver::{MockRequest, MockResponse, MockServer};
use crate::sync_engine::SyncEngine;

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("test runtime")
}

fn fixture_db() -> DbHandle {
    let mut conn = Connection::open_in_memory().expect("in-memory connection");
    crate::bootstrap::apply_all(&mut conn).expect("migrations");
    DbHandle::new(conn)
}

/// The mutable bits of the fixture: the conversation list the mock serves
/// (incremental sync tests change it mid-test).
struct Fixture {
    conversations: Vec<Value>,
    threads: std::collections::HashMap<i64, Vec<Value>>,
}

impl Fixture {
    fn new() -> Self {
        let conv = |id: i64, number: i64, subject: &str, customer: i64, tags: &[&str]| {
            json!({
                "id": id, "number": number, "type": "email",
                "source": {"type": "email", "via": "customer"},
                "subject": subject, "preview": "preview…",
                "status": "active", "state": "published", "mailboxId": 1,
                "assignee": {"id": 42, "type": "user"},
                "primaryCustomer": {"id": customer},
                "createdAt": "2026-10-01T10:00:00Z",
                "userUpdatedAt": "2026-10-02T10:00:00Z",
                "threads": 2,
                "tags": tags.iter().map(|t| json!({"tag": t})).collect::<Vec<_>>()
            })
        };
        let thread = |id: i64, conv: i64, kind: &str, body: &str| {
            json!({
                "id": id, "type": kind, "status": "active", "state": "published",
                "body": body, "conversationId": conv,
                "createdAt": "2026-10-01T11:00:00Z",
                "to": [{"id": 900 + conv, "email": format!("c{conv}@example.com")}]
            })
        };
        let mut threads = std::collections::HashMap::new();
        threads.insert(
            101,
            vec![
                thread(501, 101, "customer", "I cannot log in"),
                thread(502, 101, "message", "We are on it"),
            ],
        );
        threads.insert(102, vec![thread(503, 102, "customer", "Billing question")]);
        threads.insert(103, vec![thread(504, 103, "customer", "Another issue")]);
        Self {
            conversations: vec![
                conv(101, 778, "Login broken", 901, &["login", "urgent"]),
                conv(102, 779, "Invoice wrong", 902, &["billing"]),
                conv(103, 780, "Feature request", 903, &[]),
            ],
            threads,
        }
    }
}

fn user_status_json(remote: i64) -> Value {
    json!({
        "user": {"id": remote},
        "email": {"status": "active", "updatedAt": "2026-10-02T00:00:00Z"},
        "chat": {"status": "away", "mailboxStatuses": {"1": "active"}}
    })
}

/// Serve the full fixture dataset. Conversation page 2 is keyed by the
/// `cursor=p2` query parameter; tags are two pages.
fn fixture_handler(fix: &Fixture) -> crate::mockserver::Handler {
    let conversations = fix.conversations.clone();
    let threads = fix.threads.clone();
    let me_json = json!({
        "id": 42, "firstName": "Kim", "lastName": "Pearce",
        "email": "kim@example.com", "role": "owner", "type": "user"
    });
    let users_json = json!({"_embedded": {"users": [
        me_json.clone(),
        {"id": 43, "firstName": "Ada", "lastName": "Byron", "type": "user"}
    ]}, "page": {"number": 0, "totalPages": 1}});
    let system_users = json!({
        "_embedded": {"system-users": [{"id": 44, "firstName": "Sys", "type": "user"}]}
    });
    let teams = json!({"_embedded": {"teams": [
        {"id": 61, "name": "Tier 1", "_embedded": {"users": [{"id": 42}, {"id": 43}]}}
    ]}});
    let mailboxes = json!({"_embedded": {"mailboxes": [
        {"id": 1, "name": "Support", "slug": "support", "email": "support@example.com"},
        {"id": 2, "name": "Billing", "slug": "billing", "email": "billing@example.com"}
    ]}});
    let tag = |id: i64, t: &str| json!({"id": id, "tag": t, "slug": t});
    let customer = |id: i64, first: &str| {
        json!({"id": id, "firstName": first, "lastName": "Doe",
               "emails": [{"value": format!("{first}@example.com")}],
               "organization": {"id": 5, "name": "Analytical Engines"}})
    };
    Arc::new(move |req: &MockRequest| {
        let query_page = req.query_param("page").unwrap_or_default();
        match (req.method.as_str(), req.path.as_str()) {
            ("GET", "/v2/users/me") => MockResponse::json(200, me_json.to_string()),
            ("GET", "/v2/users") => MockResponse::json(200, users_json.to_string()),
            ("GET", "/v3/system-users") => MockResponse::json(200, system_users.to_string()),
            ("GET", "/v2/teams") => MockResponse::json(200, teams.to_string()),
            ("GET", "/v2/mailboxes") => MockResponse::json(200, mailboxes.to_string()),
            ("GET", "/v2/mailboxes/1/folders") => MockResponse::json(
                200,
                json!({"_embedded": {"folders": [
                    {"id": 11, "name": "Unassigned", "type": "unassigned", "totalCount": 3}
                ]}}),
            ),
            ("GET", "/v2/mailboxes/2/folders") => {
                MockResponse::json(200, json!({"_embedded": {"folders": []}}))
            }
            ("GET", "/v2/tags") if query_page == "0" => MockResponse::json(
                200,
                json!({"_embedded": {"tags": [tag(1, "alpha"), tag(2, "beta")]},
                       "page": {"number": 0, "totalPages": 2}}),
            ),
            ("GET", "/v2/tags") if query_page == "1" => MockResponse::json(
                200,
                json!({"_embedded": {"tags": [tag(3, "gamma"), tag(4, "delta")]},
                       "page": {"number": 1, "totalPages": 2}}),
            ),
            ("GET", "/v2/mailboxes/1/fields") => MockResponse::json(
                200,
                json!({"_embedded": {"fields": [
                    {"id": 21, "name": "Severity", "type": "single-line-list", "order": 0,
                     "_embedded": {"fields": [{"id": 31, "label": "High", "order": 1}]}}
                ]}}),
            ),
            ("GET", "/v2/mailboxes/2/fields") => {
                MockResponse::json(200, json!({"_embedded": {"fields": []}}))
            }
            ("GET", "/v2/customer-properties") => MockResponse::json(
                200,
                json!({"_embedded": {"properties": [{"id": 71, "name": "Plan", "slug": "plan", "type": "text"}]}}),
            ),
            ("GET", "/v2/organization-properties") => MockResponse::json(
                200,
                json!({"_embedded": {"properties": [{"id": 81, "name": "Region", "slug": "region", "type": "text"}]}}),
            ),
            ("GET", "/v2/organizations") => MockResponse::json(
                200,
                json!({"_embedded": {"organizations": [
                    {"id": 5, "name": "Analytical Engines", "domains": ["example.com"]}
                ]}, "page": {"number": 0, "totalPages": 1}}),
            ),
            ("GET", "/v3/customers") if req.query_param("cursor").is_none() => MockResponse::json(
                200,
                json!({
                    "_embedded": {"customers": [customer(901, "Ann"), customer(902, "Bob")]},
                    "page": {"nextCursor": "c2"}
                }),
            ),
            ("GET", "/v3/customers") => MockResponse::json(
                200,
                json!({"_embedded": {"customers": [customer(903, "Cid")]}}),
            ),
            ("GET", "/v2/mailboxes/1/saved-replies") => MockResponse::json(
                200,
                json!({"_embedded": {"savedReplies": [
                    {"id": 51, "name": "Hello", "preview": "Hi", "text": "Hi there!"}
                ]}}),
            ),
            ("GET", "/v2/mailboxes/2/saved-replies") => {
                MockResponse::json(200, json!({"_embedded": {"savedReplies": []}}))
            }
            ("GET", "/v2/workflows") => MockResponse::json(
                200,
                json!({"_embedded": {"workflows": [
                    {"id": 71, "name": "Auto-tag", "type": "manual", "status": "active", "mailboxId": 1},
                    {"id": 72, "name": "Escalate", "type": "automatic", "status": "active", "mailboxId": 1}
                ]}, "page": {"number": 0, "totalPages": 1}}),
            ),
            ("GET", "/v3/conversations") if req.query_param("cursor").is_none() => {
                let first_two: Vec<Value> = conversations.iter().take(2).cloned().collect();
                MockResponse::json(
                    200,
                    json!({
                        "_embedded": {"conversations": first_two},
                        "_links": {"next": {"href": "/v3/conversations?cursor=p2"}}
                    }),
                )
            }
            ("GET", "/v3/conversations") => {
                let rest: Vec<Value> = conversations.iter().skip(2).cloned().collect();
                MockResponse::json(200, json!({"_embedded": {"conversations": rest}}))
            }
            ("GET", p) if p.starts_with("/v3/conversations/") && p.ends_with("/threads") => {
                let conv: i64 = p
                    .trim_start_matches("/v3/conversations/")
                    .trim_end_matches("/threads")
                    .parse()
                    .unwrap_or(0);
                let list: Vec<Value> = threads.get(&conv).cloned().unwrap_or_default();
                MockResponse::json(200, json!({"_embedded": {"threads": list}}))
            }
            ("GET", p) if p.starts_with("/v3/conversations/") => {
                // Single-conversation fetch (sync_single_conversation).
                let conv: i64 = p
                    .trim_start_matches("/v3/conversations/")
                    .parse()
                    .unwrap_or(0);
                match conversations
                    .iter()
                    .find(|c| c["id"].as_i64() == Some(conv))
                {
                    Some(c) => MockResponse::json(200, c.to_string()),
                    None => MockResponse::json(404, r#"{"error":"not found"}"#),
                }
            }
            ("GET", p) if p.starts_with("/v2/users/") && p.ends_with("/status") => {
                let remote: i64 = p
                    .trim_start_matches("/v2/users/")
                    .trim_end_matches("/status")
                    .parse()
                    .unwrap_or(0);
                MockResponse::json(200, user_status_json(remote).to_string())
            }
            _ => MockResponse::json(404, json!({"error": "unexpected path"}).to_string()),
        }
    })
}

fn mock_with_fixture(fix: &Fixture) -> MockServer {
    MockServer::start(fixture_handler(fix)).expect("mock server")
}

fn engine(db: &DbHandle, mock: &MockServer) -> SyncEngine {
    let creds = HsCredentials {
        client_id: "test-client-id".into(),
        client_secret: "test-client-secret".into(),
        redirect_uri: String::new(),
        api_base: mock.url(),
        webhook_secret: "test-webhook-secret".into(),
        docs_api_key: String::new(),
        docs_api_base: mock.url(),
    };
    let provider = RealHelpScoutProvider::new(db.clone(), creds);
    provider
        .save_tokens("sync-token", None, Some(3_600), None)
        .expect("seed token");
    SyncEngine::new(db.clone(), Arc::new(provider))
}

fn count(db: &DbHandle, sql: &str) -> i64 {
    db.with_read(|conn| conn.query_row(sql, [], |r| r.get(0)))
        .unwrap_or(-1)
}

// ---------------------------------------------------------------------------

#[test]
fn initial_sync_round_trips_the_full_fixture_into_the_mirror() {
    let fix = Fixture::new();
    let mock = mock_with_fixture(&fix);
    let db = fixture_db();
    let bus = crate::eventing::EventBus::default();
    let mut rx = bus.subscribe();
    let engine = engine(&db, &mock).with_bus(bus.clone());

    let results = runtime()
        .block_on(async { engine.initial_sync().await })
        .expect("initial_sync");

    // Every resource completed without an error.
    let failed: Vec<String> = results
        .iter()
        .filter(|r| r.error.is_some())
        .map(|r| format!("{}: {:?}", r.resource, r.error))
        .collect();
    assert!(failed.is_empty(), "failed resources: {failed:?}");
    assert_eq!(results.len(), crate::sync_engine::INITIAL_SYNC_ORDER.len());

    // Reference data.
    assert_eq!(count(&db, "SELECT COUNT(*) FROM accounts"), 1);
    assert_eq!(
        count(&db, "SELECT COUNT(*) FROM users"),
        3,
        "me (42) + agent (43) + system user (44)"
    );
    assert_eq!(count(&db, "SELECT COUNT(*) FROM teams"), 1);
    assert_eq!(count(&db, "SELECT COUNT(*) FROM mailboxes"), 2);
    assert_eq!(count(&db, "SELECT COUNT(*) FROM folders"), 1);
    assert_eq!(
        count(&db, "SELECT COUNT(*) FROM tags"),
        7,
        "4 fixture tags (two pages) + login/urgent/billing from conversation ingestion"
    );
    assert_eq!(count(&db, "SELECT COUNT(*) FROM inbox_fields"), 1);
    assert_eq!(count(&db, "SELECT COUNT(*) FROM organizations"), 1);
    assert_eq!(
        count(&db, "SELECT COUNT(*) FROM customer_property_definitions"),
        1
    );
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) FROM organization_property_definitions"
        ),
        1
    );
    assert_eq!(count(&db, "SELECT COUNT(*) FROM saved_replies"), 1);
    assert_eq!(count(&db, "SELECT COUNT(*) FROM workflows"), 2);

    // Customer cursor pagination walked both pages.
    assert_eq!(count(&db, "SELECT COUNT(*) FROM customers"), 3);
    let ann_email: String = db
        .with_read(|conn| {
            conn.query_row(
                "SELECT email FROM customers WHERE remote_id = 901",
                [],
                |r| r.get(0),
            )
        })
        .unwrap_or_default();
    assert_eq!(ann_email, "Ann@example.com");

    // Conversations + threads (both the "conversations" and "threads"
    // resources walk the same pages; upserts are idempotent).
    assert_eq!(count(&db, "SELECT COUNT(*) FROM conversations"), 3);
    assert_eq!(count(&db, "SELECT COUNT(*) FROM conversation_threads"), 4);
    let thread_body: String = db
        .with_read(|conn| {
            conn.query_row(
                "SELECT body_text FROM conversation_threads WHERE remote_id = 501",
                [],
                |r| r.get(0),
            )
        })
        .unwrap_or_default();
    assert_eq!(thread_body, "I cannot log in");

    // Conversation tags landed through the join table.
    let mut tag_names: Vec<String> = db.with_read(|conn| {
        let mut stmt = conn
            .prepare(
                "SELECT t.name FROM conversation_tags ct
                     JOIN tags t ON t.id = ct.tag_id
                     JOIN conversations c ON c.id = ct.conversation_id
                     WHERE c.remote_id = 101",
            )
            .expect("tag query");
        let rows: Vec<String> = stmt
            .query_map([], |r| r.get(0))
            .expect("tag rows")
            .filter_map(|r| r.ok())
            .collect();
        rows
    });
    tag_names.sort();
    assert_eq!(tag_names, vec!["login".to_string(), "urgent".to_string()]);

    // User statuses were fetched for every synced user.
    assert_eq!(count(&db, "SELECT COUNT(*) FROM user_statuses"), 3);

    // Sync state machine + run bookkeeping.
    assert_eq!(crate::sync_engine::get_state(&db.write_guard()), "LIVE");
    let (run_state, finished_at, errors): (String, Option<String>, i64) = db
        .with_read(|conn| {
            conn.query_row(
                "SELECT state, finished_at, errors FROM sync_runs ORDER BY id DESC LIMIT 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
        })
        .expect("sync run row");
    assert_eq!(run_state, "LIVE");
    assert!(finished_at.is_some(), "the run must be marked finished");
    assert_eq!(errors, 0);

    // Checkpoint bookkeeping recorded a success per resource.
    let checkpointed: i64 = count(
        &db,
        "SELECT COUNT(*) FROM sync_checkpoints WHERE last_success_at IS NOT NULL",
    );
    assert!(checkpointed >= 20, "checkpoints recorded: {checkpointed}");

    // The sync_completed event was broadcast.
    let mut saw_sync_completed = false;
    while let Ok(event) = rx.try_recv() {
        if matches!(event, crate::events::ServerEvent::SyncCompleted { .. }) {
            saw_sync_completed = true;
        }
    }
    assert!(saw_sync_completed, "sync_completed must be broadcast");

    // The mock only ever saw authenticated requests.
    for hit in mock.hits() {
        assert_eq!(
            hit.header("authorization"),
            Some("Bearer sync-token"),
            "unauthenticated request to {}",
            hit.path
        );
    }
}

#[test]
fn initial_sync_is_idempotent_on_rerun() {
    let fix = Fixture::new();
    let mock = mock_with_fixture(&fix);
    let db = fixture_db();
    let engine = engine(&db, &mock);

    let _ = runtime()
        .block_on(async { engine.initial_sync().await })
        .expect("first sync");
    let before = count(&db, "SELECT COUNT(*) FROM conversation_threads");
    assert_eq!(before, 4);

    let _ = runtime()
        .block_on(async { engine.initial_sync().await })
        .expect("second sync");
    assert_eq!(
        count(&db, "SELECT COUNT(*) FROM conversations"),
        3,
        "re-sync must not duplicate conversations"
    );
    assert_eq!(
        count(&db, "SELECT COUNT(*) FROM conversation_threads"),
        4,
        "re-sync must not duplicate threads"
    );
}

#[test]
fn sync_single_conversation_updates_and_soft_deletes() {
    let fix = Fixture::new();
    let mock = mock_with_fixture(&fix);
    let db = fixture_db();
    let engine = engine(&db, &mock);

    // Bring one conversation in.
    let ok = runtime()
        .block_on(async { engine.sync_single_conversation(101).await })
        .expect("sync_single_conversation");
    assert!(ok);
    assert_eq!(count(&db, "SELECT COUNT(*) FROM conversations"), 1);
    assert_eq!(count(&db, "SELECT COUNT(*) FROM conversation_threads"), 2);

    // 404 upstream: soft delete, not an error.
    mock.set_handler(Arc::new(|req: &MockRequest| {
        if req.path == "/v3/conversations/101" {
            return MockResponse::json(404, r#"{"error":"gone"}"#);
        }
        if req.path == "/v3/conversations/101/threads" {
            return MockResponse::json(200, json!({"_embedded": {"threads": []}}).to_string());
        }
        MockResponse::json(404, r#"{"error":"gone"}"#)
    }));
    let ok = runtime()
        .block_on(async { engine.sync_single_conversation(101).await })
        .expect("404 handled");
    assert!(ok);
    let deleted: i64 = db
        .with_read(|conn| {
            conn.query_row(
                "SELECT COUNT(*) FROM conversations WHERE remote_id = 101 AND deleted_at IS NOT NULL",
                [],
                |r| r.get(0),
            )
        })
        .unwrap_or(-1);
    assert_eq!(deleted, 1, "missing upstream conversation is soft-deleted");
}

#[test]
fn incremental_sync_picks_up_changed_conversations() {
    let mut fix = Fixture::new();
    let mock = mock_with_fixture(&fix);
    let db = fixture_db();
    let engine = engine(&db, &mock);

    runtime()
        .block_on(async { engine.initial_sync().await })
        .expect("initial sync");

    // The customer asks for an update upstream: subject + a new thread.
    fix.conversations[0]["subject"] = json!("Login broken — URGENT");
    fix.threads.get_mut(&101).unwrap().push(json!({
        "id": 505, "type": "customer", "status": "active", "state": "published",
        "body": "Still cannot log in", "createdAt": "2026-10-03T10:00:00Z"
    }));
    mock.set_handler(fixture_handler(&fix));

    let results = runtime()
        .block_on(async { engine.incremental_sync().await })
        .expect("incremental sync");
    let failed: Vec<String> = results
        .iter()
        .filter(|r| r.error.is_some())
        .map(|r| format!("{}: {:?}", r.resource, r.error))
        .collect();
    assert!(failed.is_empty(), "failed resources: {failed:?}");

    let subject: String = db
        .with_read(|conn| {
            conn.query_row(
                "SELECT subject FROM conversations WHERE remote_id = 101",
                [],
                |r| r.get(0),
            )
        })
        .unwrap_or_default();
    assert_eq!(subject, "Login broken — URGENT");
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) FROM conversation_threads WHERE conversation_id = \
                    (SELECT id FROM conversations WHERE remote_id = 101)"
        ),
        3,
        "the new thread must be ingested"
    );
}

#[test]
fn hard_upstream_failure_is_reported_not_swallowed() {
    let fix = Fixture::new();
    let mock = mock_with_fixture(&fix);
    let db = fixture_db();
    let engine = engine(&db, &mock);

    // Every endpoint now rejects the request with a hard, non-retryable
    // 400: the run must surface an error, not pretend success.
    mock.set_handler(crate::mockserver::fixed(MockResponse::json(
        400,
        r#"{"error":"bad request"}"#,
    )));

    let results = runtime()
        .block_on(async { engine.initial_sync().await })
        .expect("run completes structurally");
    let account = results
        .iter()
        .find(|r| r.resource == "account")
        .expect("account result");
    assert!(
        account.error.is_some(),
        "a hard upstream failure must be reported in the resource result"
    );

    // And the state machine degraded to ERROR, not LIVE.
    let state = crate::sync_engine::get_state(&db.write_guard());
    assert_eq!(state, "ERROR");
}
