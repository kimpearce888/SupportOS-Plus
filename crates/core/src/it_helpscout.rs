//! Integration tests (TODO 45): `RealHelpScoutProvider` behavior against a
//! local mock Help Scout API server.
//!
//! These exercise the real HTTP stack (reqwest + `ApiQueue` + rate
//! limiter + token persistence) end-to-end over TCP against
//! [`crate::mockserver`], with generated throwaway credentials. They do
//! **not** touch `api.helpscout.net` — real-account verification requires
//! external credentials and is explicitly out of scope (documented in
//! `completed.md`).
//!
//! Runtime note: the slimmed tokio feature set has no `macros` feature, so
//! tests build a current-thread runtime manually (the same pattern the
//! shell pages use).

use std::sync::Arc;

use rusqlite::Connection;
use serde_json::{Value, json};

use crate::db::DbHandle;
use crate::helpscout::HelpScoutProvider;
use crate::helpscout_real::{
    HsApiError, HsCredentials, RealHelpScoutProvider, hs_error, hs_status,
};
use crate::mockserver::{MockResponse, MockServer};

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("test runtime")
}

/// Serializes this file against the process-global master key (other test
/// modules clear/replace it under the same lock; the OAuth flow now stores
/// tokens in the encrypted envelope, R7/D13, so every test here needs a
/// stable key for its whole duration).
fn secrets_guard() -> std::sync::MutexGuard<'static, ()> {
    crate::settings::SECRETS_TEST_LOCK
        .lock()
        .unwrap_or_else(|p| p.into_inner())
}

const TEST_MASTER_KEY: [u8; 32] = [7u8; 32];

fn fixture_db() -> DbHandle {
    // In-memory databases have no key file; install the deterministic key
    // the whole file shares (callers hold `secrets_guard` while using it).
    crate::settings::set_master_key_for_tests(TEST_MASTER_KEY);
    let mut conn = Connection::open_in_memory().expect("in-memory connection");
    crate::bootstrap::apply_all(&mut conn).expect("migrations");
    DbHandle::new(conn)
}

/// The OAuth tokens as stored by the envelope-backed `save_tokens`.
fn envelope_tokens(db: &DbHandle) -> (Option<String>, Option<String>) {
    db.with_read(|conn| {
        let read = |key: &str| {
            crate::settings::get_secret(conn, key)
                .expect("envelope read")
                .map(|b| String::from_utf8(b).expect("utf8 token"))
        };
        (
            read(crate::helpscout_real::SECRET_OAUTH_ACCESS),
            read(crate::helpscout_real::SECRET_OAUTH_REFRESH),
        )
    })
}

/// Generated throwaway credentials pointed at the mock server.
fn creds(mock: &MockServer) -> HsCredentials {
    HsCredentials {
        client_id: "test-client-id".into(),
        client_secret: "test-client-secret".into(),
        redirect_uri: format!("{}/oauth/callback", mock.url()),
        api_base: mock.url(),
        webhook_secret: "test-webhook-secret".into(),
        docs_api_key: String::new(),
        docs_api_base: mock.url(),
    }
}

fn provider(db: &DbHandle, creds: &HsCredentials) -> RealHelpScoutProvider {
    RealHelpScoutProvider::new(db.clone(), creds.clone())
}

/// Provider with a valid, non-expiring bearer token already stored.
fn provider_with_token(db: &DbHandle, creds: &HsCredentials) -> RealHelpScoutProvider {
    let p = provider(db, creds);
    p.save_tokens("valid-token", None, Some(3_600), Some("read write"))
        .expect("seed token");
    p
}

fn me_json() -> Value {
    json!({
        "id": 42, "firstName": "Kim", "lastName": "Pearce",
        "email": "kim@example.com", "role": "owner", "type": "user",
        "timezone": "Asia/Dhaka", "mention": "kim"
    })
}

fn token_json(access: &str, refresh: Option<&str>, expires_in: i64) -> Value {
    json!({
        "access_token": access,
        "token_type": "bearer",
        "expires_in": expires_in,
        "scope": "read write",
        "refresh_token": refresh
    })
}

// ---------------------------------------------------------------------------
// API round-trips: the real provider parses what the wire format returns
// ---------------------------------------------------------------------------

#[test]
fn get_me_roundtrip_sends_bearer_token() {
    let _secrets = secrets_guard();
    let mock = MockServer::start(crate::mockserver::fixed(MockResponse::json(
        200,
        me_json().to_string(),
    )))
    .expect("mock server");
    let db = fixture_db();
    let p = provider_with_token(&db, &creds(&mock));

    let me = runtime()
        .block_on(async { p.get_me().await })
        .expect("get_me");
    assert_eq!(me.remote_id, 42);
    assert_eq!(me.first_name.as_deref(), Some("Kim"));
    assert_eq!(me.email.as_deref(), Some("kim@example.com"));
    assert_eq!(me.role.as_deref(), Some("owner"));

    let hits = mock.hits();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].method, "GET");
    assert_eq!(hits[0].path, "/v2/users/me");
    assert_eq!(
        hits[0].header("authorization"),
        Some("Bearer valid-token"),
        "requests must carry the stored bearer token"
    );
}

#[test]
fn list_mailboxes_and_dependent_resources_roundtrip() {
    let _secrets = secrets_guard();
    let mock = MockServer::start(Arc::new(|req: &crate::mockserver::MockRequest| {
        match (req.method.as_str(), req.path.as_str()) {
            ("GET", "/v2/mailboxes") => MockResponse::json(
                200,
                json!({"_embedded": {"mailboxes": [
                    {"id": 1, "name": "Support", "slug": "support", "email": "support@example.com"},
                    {"id": 2, "name": "Billing", "slug": "billing", "email": "billing@example.com"}
                ]}}),
            ),
            ("GET", "/v2/mailboxes/1/folders") => MockResponse::json(
                200,
                json!({"_embedded": {"folders": [
                    {"id": 11, "name": "Unassigned", "type": "unassigned", "totalCount": 3, "activeCount": 2}
                ]}}),
            ),
            ("GET", "/v2/mailboxes/1/fields") => MockResponse::json(
                200,
                json!({"_embedded": {"fields": [
                    {"id": 21, "name": "Severity", "type": "single-line-list", "required": true, "order": 0,
                     "_embedded": {"fields": [{"id": 31, "label": "High", "order": 1}]}}
                ]}}),
            ),
            ("GET", "/v2/mailboxes/1/saved-replies") => MockResponse::json(
                200,
                json!({"_embedded": {"savedReplies": [
                    {"id": 51, "name": "Hello", "preview": "Hi there", "text": "Hi there!"}
                ]}}),
            ),
            ("GET", "/v2/teams") => MockResponse::json(
                200,
                json!({"_embedded": {"teams": [
                    {"id": 61, "name": "Tier 1", "_embedded": {"users": [{"id": 42}, {"id": 43}]}}
                ]}}),
            ),
            _ => MockResponse::json(404, r#"{"error":"unexpected path"}"#),
        }
    }))
    .expect("mock server");
    let db = fixture_db();
    let p = provider_with_token(&db, &creds(&mock));
    let rt = runtime();

    let mailboxes = rt
        .block_on(async { p.list_mailboxes().await })
        .expect("mailboxes");
    assert_eq!(mailboxes.len(), 2);
    assert_eq!(mailboxes[0].name, "Support");

    let folders = rt
        .block_on(async { p.list_folders(1).await })
        .expect("folders");
    assert_eq!(folders.len(), 1);
    assert_eq!(folders[0].name, "Unassigned");
    assert_eq!(folders[0].total_count, 3);

    let fields = rt
        .block_on(async { p.list_inbox_fields(1).await })
        .expect("inbox fields");
    assert_eq!(fields.len(), 1);
    assert_eq!(fields[0].options.len(), 1);
    assert_eq!(fields[0].options[0].label, "High");

    let replies = rt
        .block_on(async { p.list_saved_replies(1).await })
        .expect("saved replies");
    assert_eq!(replies.len(), 1);
    assert_eq!(replies[0].text, Some("Hi there!".into()));

    let teams = rt.block_on(async { p.list_teams().await }).expect("teams");
    assert_eq!(teams.len(), 1);
    assert_eq!(teams[0].member_user_ids, vec![42, 43]);

    assert!(mock.hit_count("GET", "/v2/mailboxes") >= 1);
}

#[test]
fn conversations_threads_and_customers_roundtrip() {
    let _secrets = secrets_guard();
    let conv = json!({
        "id": 101, "number": 778, "type": "email",
        "source": {"type": "email", "via": "customer"},
        "subject": "Login broken", "preview": "I cannot log in…",
        "status": "active", "state": "published", "mailboxId": 1,
        "assignee": {"id": 42, "type": "user"},
        "primaryCustomer": {"id": 900},
        "createdAt": "2026-10-01T10:00:00Z", "userUpdatedAt": "2026-10-02T10:00:00Z",
        "threads": 2, "tags": [{"tag": "login"}, {"tag": "urgent"}],
        "fields": [{"id": 21, "value": "high", "label": "High", "systemType": "severity"}]
    });
    let mock = MockServer::start(Arc::new(move |req: &crate::mockserver::MockRequest| {
        match (req.method.as_str(), req.path.as_str()) {
            ("GET", "/v3/conversations/101") => MockResponse::json(200, conv.to_string()),
            ("GET", "/v3/conversations/101/threads") => MockResponse::json(
                200,
                json!({"_embedded": {"threads": [
                    {"id": 501, "type": "customer", "status": "active", "state": "published",
                     "body": "<p>I cannot log in</p>", "createdByCustomer": {"id": 900},
                     "createdAt": "2026-10-01T10:00:00Z",
                     "to": [{"id": 900, "email": "cust@example.com"}],
                     "attachments": [{"id": 601, "filename": "log.txt", "mimeType": "text/plain", "size": 512}]}
                ]}}),
            ),
            ("GET", "/v2/customers/900") => MockResponse::json(
                200,
                json!({
                    "id": 900, "firstName": "Ada", "lastName": "Lovelace",
                    "email": "cust@example.com", "createdAt": "2026-09-01T00:00:00Z",
                    "emails": [{"value": "cust@example.com", "type": "home"}],
                    "phones": [{"value": "+15550100", "type": "mobile"}],
                    "organization": {"id": 5, "name": "Analytical Engines"},
                    "address": {"city": "London", "country": "GB"}
                }),
            ),
            _ => MockResponse::json(404, r#"{"error":"unexpected path"}"#),
        }
    }))
    .expect("mock server");
    let db = fixture_db();
    let p = provider_with_token(&db, &creds(&mock));
    let rt = runtime();

    let conv = rt
        .block_on(async { p.get_conversation(101).await })
        .expect("get_conversation")
        .expect("some conversation");
    assert_eq!(conv.remote_id, 101);
    assert_eq!(conv.number, 778);
    assert_eq!(conv.subject.as_deref(), Some("Login broken"));
    assert_eq!(conv.status, "active");
    assert_eq!(conv.mailbox_id, 1);
    assert_eq!(conv.customer_id, 900);
    assert_eq!(conv.assignee_id, Some(42));
    assert_eq!(conv.tags, vec!["login".to_string(), "urgent".to_string()]);
    assert_eq!(conv.thread_count, 2);

    let threads = rt
        .block_on(async { p.list_threads(101).await })
        .expect("list_threads");
    assert_eq!(threads.len(), 1);
    assert_eq!(threads[0].remote_id, 501);
    assert_eq!(threads[0].conversation_id, 101);
    assert_eq!(threads[0].body.as_deref(), Some("<p>I cannot log in</p>"));
    assert_eq!(threads[0].to.len(), 1);
    assert_eq!(threads[0].to[0].email.as_deref(), Some("cust@example.com"));
    assert_eq!(threads[0].attachments.len(), 1);
    assert_eq!(threads[0].attachments[0].size, Some(512));

    let customer = rt
        .block_on(async { p.get_customer(900).await })
        .expect("get_customer")
        .expect("some customer");
    assert_eq!(customer.remote_id, 900);
    assert_eq!(customer.first_name.as_deref(), Some("Ada"));
    assert_eq!(customer.emails.len(), 1);
    assert_eq!(customer.organization.as_deref(), Some("Analytical Engines"));
    assert_eq!(
        customer.address.as_ref().and_then(|a| a.city.as_deref()),
        Some("London")
    );

    // 404 maps to Ok(None), not an error (deleted/merged upstream).
    let missing = rt
        .block_on(async { p.get_conversation(999).await })
        .expect("404 is not a hard error");
    assert!(missing.is_none());
}

#[test]
fn create_conversation_and_thread_roundtrips() {
    let _secrets = secrets_guard();
    let mock = MockServer::start(Arc::new(|req: &crate::mockserver::MockRequest| {
        match (req.method.as_str(), req.path.as_str()) {
            // Audit fix (C1): re-pinned to the REAL create contract —
            // POST /v2/conversations (not /v3) answers 201 with an EMPTY
            // body and the new id in the Resource-Id header. The old mock
            // fed a JSON body with "id"/"number"/"threads", which encoded
            // a contract that does not exist and kept live creates broken.
            ("POST", "/v2/conversations") => {
                MockResponse::empty(201).with_header("Resource-Id", "101")
            }
            ("POST", "/v2/conversations/101/reply") => MockResponse::json(201, json!({"id": 503})),
            ("POST", "/v2/conversations/101/notes") => MockResponse::json(201, json!({"id": 504})),
            ("PATCH", "/v2/conversations/101") => MockResponse::empty(204),
            ("PUT", "/v2/conversations/101/tags") => MockResponse::empty(204),
            _ => MockResponse::json(404, r#"{"error":"unexpected path"}"#),
        }
    }))
    .expect("mock server");
    let db = fixture_db();
    let p = provider_with_token(&db, &creds(&mock));
    let rt = runtime();

    use crate::helpscout::{CreateConversationInput, CreateThreadInput};

    let created = rt
        .block_on(async {
            p.create_conversation(CreateConversationInput {
                mailbox_id: 1,
                customer_id: 900,
                subject: "New issue".into(),
                body: "Something broke".into(),
                tags: vec!["bug".into()],
                // Audit fix (C1): None must default the status to "open"
                // and the channel to "email" (the live API 400s without
                // both top-level fields).
                status: None,
                channel: None,
            })
            .await
        })
        .expect("create_conversation");
    // The real contract: id arrives via the Resource-Id header; number and
    // thread id are unknown at create time (empty 201 body).
    assert_eq!(created.conversation_id, 101);
    assert_eq!(created.number, 0);
    assert_eq!(created.thread_id, 0);
    let body: Value = serde_json::from_str(&mock.hits()[0].body).expect("request body json");
    assert_eq!(body["subject"], "New issue");
    assert_eq!(body["mailboxId"], 1);
    assert_eq!(body["threads"][0]["customer"]["id"], 900);
    // Audit fix (C1): required top-level fields must be present.
    assert_eq!(body["type"], "email");
    assert_eq!(body["status"], "open");

    let reply = rt
        .block_on(async {
            p.create_reply_thread(CreateThreadInput {
                conversation_id: 101,
                text: "We are on it".into(),
                cc: vec!["boss@example.com".into()],
                bcc: Vec::new(),
                draft: false,
                status_after: Some("active".into()),
                assign_to: None,
                customer_id: Some(900),
            })
            .await
        })
        .expect("create_reply_thread");
    assert_eq!(reply.thread_id, 503);
    let body: Value = serde_json::from_str(&mock.hits()[1].body).expect("reply body json");
    assert_eq!(body["cc"][0], "boss@example.com");
    assert_eq!(body["status"], "active");

    let note = rt
        .block_on(async {
            p.create_note_thread(CreateThreadInput {
                conversation_id: 101,
                text: "internal note".into(),
                draft: false,
                cc: Vec::new(),
                bcc: Vec::new(),
                status_after: None,
                assign_to: None,
                customer_id: None,
            })
            .await
        })
        .expect("create_note_thread");
    assert_eq!(note.thread_id, 504);

    let updated = rt
        .block_on(async {
            use crate::helpscout::ConversationPatch;
            p.update_conversation(
                101,
                ConversationPatch {
                    subject: Some("Updated subject".into()),
                    status: Some("pending".into()),
                    mailbox_id: None,
                    assign_to: Some(Some(42)),
                },
            )
            .await
        })
        .expect("update_conversation");
    assert!(updated);

    rt.block_on(async { p.update_tags(101, vec!["new-tag".into()]).await })
        .expect("update_tags");

    let patch_hits: Vec<_> = mock
        .hits()
        .iter()
        .filter(|h| h.method == "PATCH")
        .cloned()
        .collect();
    assert_eq!(patch_hits.len(), 3, "subject + status + assignTo patches");
    let bodies: Vec<Value> = patch_hits
        .iter()
        .map(|h| serde_json::from_str(&h.body).expect("patch body"))
        .collect();
    assert_eq!(bodies[0]["path"], "/subject");
    assert_eq!(bodies[1]["path"], "/status");
    assert_eq!(bodies[2]["path"], "/assignTo");
    assert_eq!(bodies[2]["value"], 42);
}

// ---------------------------------------------------------------------------
// Pagination: v2 page numbers and v3 cursors
// ---------------------------------------------------------------------------

#[test]
fn v2_pagination_follows_page_numbers_until_total_pages() {
    let _secrets = secrets_guard();
    // 3 pages of tags, 2 per page, totalPages = 3.
    let mock = MockServer::start(Arc::new(|req: &crate::mockserver::MockRequest| {
        let page: u32 = req
            .query_param("page")
            .and_then(|p| p.parse().ok())
            .unwrap_or(0);
        let tag = |id: i64, slug: &str| json!({"id": id, "tag": slug, "slug": slug});
        match page {
            1 => MockResponse::json(
                200,
                json!({"_embedded": {"tags": [tag(1, "alpha"), tag(2, "beta")]},
                       "page": {"number": 1, "totalPages": 3}}),
            ),
            2 => MockResponse::json(
                200,
                json!({"_embedded": {"tags": [tag(3, "gamma"), tag(4, "delta")]},
                       "page": {"number": 2, "totalPages": 3}}),
            ),
            3 => MockResponse::json(
                200,
                json!({"_embedded": {"tags": [tag(5, "epsilon")]},
                       "page": {"number": 3, "totalPages": 3}}),
            ),
            _ => MockResponse::json(200, json!({"_embedded": {"tags": []}})),
        }
    }))
    .expect("mock server");
    let db = fixture_db();
    let p = provider_with_token(&db, &creds(&mock));

    let tags = runtime()
        .block_on(async { p.list_tags().await })
        .expect("list_tags");
    let names: Vec<String> = tags.iter().map(|t| t.name.clone()).collect();
    assert_eq!(names, vec!["alpha", "beta", "gamma", "delta", "epsilon"]);

    let tag_pages: Vec<String> = mock
        .hits()
        .iter()
        .filter(|h| h.path == "/v2/tags")
        .filter_map(|h| h.query_param("page"))
        .collect();
    assert_eq!(tag_pages, vec!["1", "2", "3"], "must walk every page once");
}

#[test]
fn v2_pagination_stops_on_empty_page_safely() {
    let _secrets = secrets_guard();
    // Server claims more pages but returns an empty embedded list: the
    // walker must terminate (empty page = stop), not loop forever.
    let mock = MockServer::start(Arc::new(|_req: &crate::mockserver::MockRequest| {
        MockResponse::json(
            200,
            json!({"_embedded": {"users": []}, "page": {"number": 1, "totalPages": 5}}),
        )
    }))
    .expect("mock server");
    let db = fixture_db();
    let p = provider_with_token(&db, &creds(&mock));

    let users = runtime()
        .block_on(async { p.list_users().await })
        .expect("list_users");
    assert!(users.is_empty());
    assert_eq!(mock.hit_count("GET", "/v2/users"), 1);
}

#[test]
fn v3_conversation_pagination_follows_hal_next_links() {
    let _secrets = secrets_guard();
    let mock = MockServer::start(Arc::new(|req: &crate::mockserver::MockRequest| {
        let conv = |id: i64| {
            json!({"id": id, "number": id + 100, "type": "email", "status": "active",
                   "mailboxId": 1, "primaryCustomer": {"id": 900}, "threads": 0})
        };
        if req.path == "/v3/conversations" && req.query_param("cursor").is_none() {
            return MockResponse::json(
                200,
                json!({
                    "_embedded": {"conversations": [conv(101), conv(102)]},
                    "_links": {"next": {"href": "/v3/conversations?cursor=page2token"}}
                }),
            );
        }
        if req.path == "/v3/conversations"
            && req.query_param("cursor").as_deref() == Some("page2token")
        {
            return MockResponse::json(200, json!({"_embedded": {"conversations": [conv(103)]}}));
        }
        MockResponse::json(404, r#"{"error":"unexpected"}"#)
    }))
    .expect("mock server");
    let db = fixture_db();
    let p = provider_with_token(&db, &creds(&mock));

    use crate::helpscout::ConversationQuery;
    let first = runtime()
        .block_on(async {
            p.list_conversations(&ConversationQuery {
                status: Some("all".into()),
                ..Default::default()
            })
            .await
        })
        .expect("first page");
    assert_eq!(first.items.len(), 2);
    assert_eq!(
        first.next_cursor.as_deref(),
        Some("/v3/conversations?cursor=page2token")
    );

    // Follow the cursor the way sync_conversations does.
    let second = runtime()
        .block_on(async {
            p.list_conversations(&ConversationQuery {
                status: Some("all".into()),
                cursor: first.next_cursor.clone(),
                ..Default::default()
            })
            .await
        })
        .expect("second page");
    assert_eq!(second.items.len(), 1);
    assert!(second.next_cursor.is_none());
    assert_eq!(second.items[0].remote_id, 103);

    // The HAL href was converted to a bare path + forwarded as the cursor.
    let followed = mock
        .hits()
        .iter()
        .any(|h| h.query_param("cursor").as_deref() == Some("page2token"));
    assert!(followed, "cursor must be forwarded to the next request");
}

#[test]
fn v3_customer_pagination_follows_next_cursor_field() {
    let _secrets = secrets_guard();
    let mock = MockServer::start(Arc::new(|req: &crate::mockserver::MockRequest| {
        let cust = |id: i64, name: &str| {
            json!({"id": id, "firstName": name, "lastName": "Doe", "emails": []})
        };
        if req.path == "/v3/customers" && req.query_param("cursor").is_none() {
            return MockResponse::json(
                200,
                json!({
                    "_embedded": {"customers": [cust(901, "Ann")]},
                    "page": {"nextCursor": "c2"}
                }),
            );
        }
        if req.path == "/v3/customers" && req.query_param("cursor").as_deref() == Some("c2") {
            return MockResponse::json(
                200,
                json!({"_embedded": {"customers": [cust(902, "Bob")]}}),
            );
        }
        MockResponse::json(404, r#"{"error":"unexpected"}"#)
    }))
    .expect("mock server");
    let db = fixture_db();
    let p = provider_with_token(&db, &creds(&mock));

    use crate::helpscout::CustomerQuery;
    let page = runtime()
        .block_on(async {
            p.list_customers(&CustomerQuery {
                modified_since: None,
                cursor: None,
                ..Default::default()
            })
            .await
        })
        .expect("first customers page");
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].first_name.as_deref(), Some("Ann"));
    assert_eq!(page.next_cursor.as_deref(), Some("c2"));

    let page2 = runtime()
        .block_on(async {
            p.list_customers(&CustomerQuery {
                cursor: page.next_cursor.clone(),
                ..Default::default()
            })
            .await
        })
        .expect("second customers page");
    assert_eq!(page2.items.len(), 1);
    assert!(page2.next_cursor.is_none());
}

#[test]
fn system_users_follows_next_links() {
    let _secrets = secrets_guard();
    let mock = MockServer::start(Arc::new(|req: &crate::mockserver::MockRequest| {
        if req.path == "/v3/system-users" && req.query.is_empty() {
            return MockResponse::json(
                200,
                json!({
                    "_embedded": {"system-users": [{"id": 7, "firstName": "Sys", "type": "user"}]},
                    "_links": {"next": {"href": "/v3/system-users?page=2"}}
                }),
            );
        }
        if req.path == "/v3/system-users" && req.query.contains("page=2") {
            return MockResponse::json(
                200,
                json!({"_embedded": {"system-users": [{"id": 8, "firstName": "Ops", "type": "user"}]}}),
            );
        }
        MockResponse::json(404, r#"{"error":"unexpected"}"#)
    }))
    .expect("mock server");
    let db = fixture_db();
    let p = provider_with_token(&db, &creds(&mock));

    let users = runtime()
        .block_on(async { p.list_system_users().await })
        .expect("list_system_users");
    assert_eq!(users.len(), 2);
    assert_eq!(users[1].remote_id, 8);
}

// ---------------------------------------------------------------------------
// Error mapping and recovery
// ---------------------------------------------------------------------------

#[test]
fn error_statuses_map_to_friendly_non_retryable_errors() {
    let _secrets = secrets_guard();
    for status in [400u16, 403, 404, 409, 423] {
        let mock = MockServer::start(crate::mockserver::fixed(MockResponse::json(
            status,
            r#"{"error":"nope"}"#,
        )))
        .expect("mock server");
        let db = fixture_db();
        let p = provider_with_token(&db, &creds(&mock));

        let err = runtime()
            .block_on(async { p.get_me().await })
            .expect_err("must fail");
        let hs = hs_error(&err).expect("HsApiError").clone();
        assert_eq!(hs.status_code, status);
        assert!(!hs.retryable, "status {status} must not be retryable");
        assert!(!hs.friendly.is_empty(), "friendly text required");
        // The raw error keeps the request shape for diagnostics.
        assert!(
            hs.message.contains("/v2/users/me"),
            "message: {}",
            hs.message
        );
    }
}

#[test]
fn server_error_5xx_is_retried_and_then_recovers() {
    let _secrets = secrets_guard();
    // First response 503, second 200: the provider must retry once and
    // succeed, having hit the endpoint exactly twice.
    let mock = MockServer::start(crate::mockserver::scripted(vec![
        MockResponse::json(503, r#"{"error":"maintenance"}"#),
        MockResponse::json(200, me_json().to_string()),
    ]))
    .expect("mock server");
    let db = fixture_db();
    let p = provider_with_token(&db, &creds(&mock));

    let me = runtime()
        .block_on(async { p.get_me().await })
        .expect("recovers after 503");
    assert_eq!(me.remote_id, 42);
    assert_eq!(mock.hit_count("GET", "/v2/users/me"), 2);
}

#[test]
fn rate_limit_429_is_honored_retried_and_recorded() {
    let _secrets = secrets_guard();
    // 429 with a 1-second retry window, then success. Also sends the
    // x-ratelimit headers so the limiter state can be cross-checked.
    let mock = MockServer::start(crate::mockserver::scripted(vec![
        MockResponse::json(429, r#"{"error":"rate limited"}"#)
            .with_header("x-ratelimit-limit-minute", "100")
            .with_header("x-ratelimit-remaining-minute", "0")
            .with_header("x-ratelimit-retry-after", "1"),
        MockResponse::json(200, me_json().to_string())
            .with_header("x-ratelimit-limit-minute", "100")
            .with_header("x-ratelimit-remaining-minute", "99"),
    ]))
    .expect("mock server");
    let db = fixture_db();
    let p = provider_with_token(&db, &creds(&mock));

    let started = std::time::Instant::now();
    let me = runtime()
        .block_on(async { p.get_me().await })
        .expect("recovers after 429");
    assert_eq!(me.remote_id, 42);
    assert_eq!(mock.hit_count("GET", "/v2/users/me"), 2);
    assert!(
        started.elapsed() >= std::time::Duration::from_millis(1000),
        "the retry must actually wait out the 1 s retry-after window"
    );

    // The limiter recorded the rate-limit state it saw on the wire.
    let snapshot = p.limiter.snapshot();
    assert_eq!(snapshot["limitPerMinute"], json!(100));
    assert_eq!(snapshot["remaining"], json!(99));
}

#[test]
fn rate_limit_headers_on_success_are_recorded() {
    let _secrets = secrets_guard();
    let mock = MockServer::start(crate::mockserver::fixed(MockResponse::json(
        200,
        me_json().to_string(),
    )))
    .expect("mock server");
    let db = fixture_db();
    let p = provider_with_token(&db, &creds(&mock));

    runtime()
        .block_on(async { p.get_me().await })
        .expect("get_me");
    // No headers sent: defaults stay intact and nothing panics.
    let snapshot = p.limiter.snapshot();
    assert!(snapshot["limitPerMinute"].as_i64().unwrap_or(0) > 0);
}

#[test]
fn unreachable_server_reports_offline_friendly_error() {
    let _secrets = secrets_guard();
    // Bind then immediately stop a mock server to get a free port with
    // nothing listening: the provider must fail with the offline error,
    // not a raw transport panic.
    let dead = MockServer::start(crate::mockserver::not_found()).expect("mock server");
    let dead_url = dead.url();
    let dead_port = dead_url.rsplit(':').next().unwrap_or_default().to_string();
    drop(dead);

    let mut creds = HsCredentials::default();
    creds.client_id = "test-client-id".into();
    creds.client_secret = "test-client-secret".into();
    creds.api_base = format!("http://127.0.0.1:{dead_port}");
    creds.docs_api_base = creds.api_base.clone();
    let db = fixture_db();
    let p = RealHelpScoutProvider::new(db, creds);
    // A valid stored token: the request must actually be attempted (and
    // fail at the transport layer) rather than being rejected up front
    // with a 401 no-token error.
    p.save_tokens("offline-token", None, Some(3_600), None)
        .expect("seed token");

    let err = runtime()
        .block_on(async { p.get_me().await })
        .expect_err("offline");
    let hs = hs_error(&err).cloned().expect("HsApiError");
    assert_eq!(hs.status_code, 0, "transport failure has no HTTP status");
    assert!(hs.retryable, "offline must be retryable");
    assert!(
        hs.friendly.contains("unreachable") || hs.friendly.contains("offline"),
        "friendly text should explain offline mode: {}",
        hs.friendly
    );
}

#[test]
fn invalid_json_from_success_response_is_reported() {
    let _secrets = secrets_guard();
    let mock = MockServer::start(crate::mockserver::fixed(MockResponse::json(
        200,
        "this is not json",
    )))
    .expect("mock server");
    let db = fixture_db();
    let p = provider_with_token(&db, &creds(&mock));

    let err = runtime()
        .block_on(async { p.get_me().await })
        .expect_err("bad json");
    let hs = hs_error(&err).cloned().expect("HsApiError");
    assert!(!hs.retryable);
    assert!(hs.friendly.contains("unexpected response format"));
}

// ---------------------------------------------------------------------------
// OAuth: token endpoint, authorization-code exchange, refresh, revoke
// ---------------------------------------------------------------------------

#[test]
fn client_credentials_login_exchanges_and_persists() {
    let _secrets = secrets_guard();
    // Handler decisions are recorded via hits() and asserted afterwards —
    // a panic inside a connection thread would only sever the socket and
    // surface as a confusing transport error.
    let mock = MockServer::start(Arc::new(|req: &crate::mockserver::MockRequest| {
        if req.path == "/v2/oauth2/token" {
            return MockResponse::json(200, token_json("cc-token", None, 7200).to_string());
        }
        MockResponse::json(200, me_json().to_string())
    }))
    .expect("mock server");
    let db = fixture_db();
    let p = provider(&db, &creds(&mock));

    let token = runtime()
        .block_on(async { p.client_credentials_login().await })
        .expect("client_credentials_login");
    assert_eq!(token, "cc-token");

    let token_req = {
        let hits = mock.hits();
        hits.iter()
            .find(|h| h.path == "/v2/oauth2/token")
            .expect("token request recorded")
            .clone()
    };
    assert_eq!(token_req.method, "POST");
    assert_eq!(
        token_req.form_value("grant_type").as_deref(),
        Some("client_credentials")
    );
    assert_eq!(
        token_req.form_value("client_id").as_deref(),
        Some("test-client-id")
    );
    assert_eq!(
        token_req.form_value("client_secret").as_deref(),
        Some("test-client-secret")
    );
    assert_eq!(
        token_req.header("content-type"),
        Some("application/x-www-form-urlencoded"),
        "token request is form-encoded"
    );

    // Persisted in the envelope and visible through the status endpoint;
    // the row itself keeps metadata only (R7/D13: no plaintext in sqlite).
    let status = p.oauth_status();
    assert_eq!(status["configured"], json!(true));
    assert_eq!(status["authenticated"], json!(true));
    let (envelope_access, envelope_refresh) = envelope_tokens(&db);
    assert_eq!(envelope_access.as_deref(), Some("cc-token"));
    assert_eq!(envelope_refresh, None);
    let (access_column, refresh_column, revoked): (String, Option<String>, i64) = db
        .with_read(|conn| {
            conn.query_row(
                "SELECT access_token, refresh_token, revoked FROM oauth_tokens WHERE account = 'default'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
        })
        .expect("token metadata row");
    assert_eq!(access_column, "", "the column must stay empty");
    assert_eq!(refresh_column, None, "the column must stay empty");
    assert_eq!(revoked, 0);

    // The stored token is actually used for the next API call.
    let me = runtime()
        .block_on(async { p.get_me().await })
        .expect("get_me with new token");
    assert_eq!(me.remote_id, 42);
    assert_eq!(
        mock.hits()
            .iter()
            .find(|h| h.path == "/v2/users/me")
            .and_then(|h| h.header("authorization")),
        Some("Bearer cc-token")
    );
}

#[test]
fn authorization_code_exchange_persists_refresh_token() {
    let _secrets = secrets_guard();
    let mock = MockServer::start(crate::mockserver::fixed(MockResponse::json(
        200,
        token_json("auth-code-token", Some("auth-refresh-token"), 86_400).to_string(),
    )))
    .expect("mock server");
    let db = fixture_db();
    let p = provider(&db, &creds(&mock));

    let token = runtime()
        .block_on(async { p.exchange_code("the-auth-code").await })
        .expect("exchange_code");
    assert_eq!(token, "auth-code-token");

    let token_req = mock.hits().first().cloned().expect("token request");
    assert_eq!(
        token_req.form_value("grant_type").as_deref(),
        Some("authorization_code")
    );
    assert_eq!(
        token_req.form_value("code").as_deref(),
        Some("the-auth-code")
    );
    assert_eq!(
        token_req.form_value("client_id").as_deref(),
        Some("test-client-id")
    );

    let (envelope_access, envelope_refresh) = envelope_tokens(&db);
    assert_eq!(envelope_access.as_deref(), Some("auth-code-token"));
    assert_eq!(
        envelope_refresh.as_deref(),
        Some("auth-refresh-token"),
        "the refresh token must persist through the authorization-code grant"
    );
    let (access_column, refresh_column): (String, Option<String>) = db
        .with_read(|conn| {
            conn.query_row(
                "SELECT access_token, refresh_token FROM oauth_tokens WHERE account = 'default'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
        })
        .expect("token metadata row");
    assert_eq!(access_column, "", "the column must stay empty (R7/D13)");
    assert_eq!(refresh_column, None, "the column must stay empty (R7/D13)");
}

#[test]
fn expired_token_refreshes_via_token_endpoint_and_retries() {
    let _secrets = secrets_guard();
    // Production refresh path: stored token is expired; the data request
    // gets a 401; the provider refreshes via the token endpoint and the
    // original request is retried with the fresh token.
    let token_requests = Arc::new(std::sync::Mutex::new(0usize));
    let counter = Arc::clone(&token_requests);
    let mock = MockServer::start(Arc::new(move |req: &crate::mockserver::MockRequest| {
        if req.path == "/v2/oauth2/token" {
            *counter.lock().unwrap() += 1;
            return MockResponse::json(
                200,
                token_json("fresh-token", Some("fresh-refresh"), 3_600).to_string(),
            );
        }
        if req.path == "/v2/users/me" {
            let n = *counter.lock().unwrap();
            if n == 0 {
                // No refresh yet: reject the stale token.
                return MockResponse::json(401, r#"{"error":"invalid token"}"#);
            }
            return MockResponse::json(200, me_json().to_string());
        }
        MockResponse::json(404, r#"{"error":"unexpected"}"#)
    }))
    .expect("mock server");
    let db = fixture_db();
    let p = provider(&db, &creds(&mock));
    // Expired 2 minutes in the past: past the 120 s expiry margin.
    p.save_tokens("stale-token", Some("stale-refresh"), Some(-300), None)
        .expect("seed stale token");

    let me = runtime()
        .block_on(async { p.get_me().await })
        .expect("refresh + retry");
    assert_eq!(me.remote_id, 42);
    assert_eq!(*token_requests.lock().unwrap(), 1, "exactly one refresh");

    let fresh_call = mock
        .hits()
        .iter()
        .find(|h| h.path == "/v2/users/me")
        .cloned()
        .expect("me call");
    assert_eq!(
        fresh_call.header("authorization"),
        Some("Bearer fresh-token"),
        "the retried request must use the refreshed token"
    );
}

#[test]
fn refresh_token_grant_roundtrip() {
    let _secrets = secrets_guard();
    let mock = MockServer::start(crate::mockserver::fixed(MockResponse::json(
        200,
        token_json("new-access", Some("new-refresh"), 600).to_string(),
    )))
    .expect("mock server");
    let db = fixture_db();
    let p = provider(&db, &creds(&mock));

    let token = runtime()
        .block_on(async { p.refresh_tokens("r-tok").await })
        .expect("refresh_tokens");
    assert_eq!(token, "new-access");
    let token_req = mock.hits().first().cloned().expect("token request");
    assert_eq!(
        token_req.form_value("grant_type").as_deref(),
        Some("refresh_token")
    );
    assert_eq!(
        token_req.form_value("refresh_token").as_deref(),
        Some("r-tok")
    );
    let (envelope_access, _) = envelope_tokens(&db);
    assert_eq!(envelope_access.as_deref(), Some("new-access"));
    let access_column: String = db
        .with_read(|conn| {
            conn.query_row(
                "SELECT access_token FROM oauth_tokens WHERE account = 'default'",
                [],
                |r| r.get(0),
            )
        })
        .expect("row");
    assert_eq!(access_column, "", "the column must stay empty (R7/D13)");
}

#[test]
fn token_endpoint_rejection_is_friendly() {
    let _secrets = secrets_guard();
    let mock = MockServer::start(crate::mockserver::fixed(MockResponse::json(
        400,
        r#"{"error":"invalid_client"}"#,
    )))
    .expect("mock server");
    let db = fixture_db();
    let p = provider(&db, &creds(&mock));

    let err = runtime()
        .block_on(async { p.exchange_code("bad-code").await })
        .expect_err("exchange must fail");
    let hs = hs_error(&err).cloned().expect("HsApiError");
    assert_eq!(hs.status_code, 400);
    assert!(
        hs.friendly.contains("Check your Client ID/Secret"),
        "friendly: {}",
        hs.friendly
    );
}

#[test]
fn revoke_clears_the_token_and_blocks_requests() {
    let _secrets = secrets_guard();
    let mock = MockServer::start(crate::mockserver::fixed(MockResponse::json(
        200,
        me_json().to_string(),
    )))
    .expect("mock server");
    let db = fixture_db();
    let p = provider_with_token(&db, &creds(&mock));

    p.revoke().expect("revoke");
    let status = p.oauth_status();
    assert_eq!(status["authenticated"], json!(false));
    let (envelope_access, envelope_refresh) = envelope_tokens(&db);
    assert_eq!(envelope_access, None, "revocation must purge the envelope");
    assert_eq!(envelope_refresh, None, "revocation must purge the envelope");

    let err = runtime()
        .block_on(async { p.get_me().await })
        .expect_err("blocked");
    assert_eq!(
        hs_status(&err),
        Some(401),
        "revoked connection must surface as unauthenticated"
    );
}

// ---------------------------------------------------------------------------
// Docs API
// ---------------------------------------------------------------------------

#[test]
fn docs_api_skipped_entirely_without_key() {
    let _secrets = secrets_guard();
    let mock = MockServer::start(crate::mockserver::not_found()).expect("mock server");
    let db = fixture_db();
    let p = provider_with_token(&db, &creds(&mock)); // empty docs key

    let cols = runtime()
        .block_on(async { p.list_doc_collections().await })
        .expect("collections");
    assert!(cols.is_empty());
    assert!(
        mock.hits().is_empty(),
        "no docs request may be made without a docs key"
    );
}

#[test]
fn docs_api_uses_basic_auth_on_its_own_base() {
    let _secrets = secrets_guard();
    // Audit fix (C2): re-pinned to the REAL Docs API contract — every v1
    // list response wraps page/pages/items in a keyed envelope
    // ({"collections":{...}}). The old mock fed a top-level {"items":...},
    // encoding a shape the real service never returns and keeping the
    // Help-Center sync at 0 records against production.
    let mock = MockServer::start(crate::mockserver::fixed(MockResponse::json(
        200,
        json!({"collections": {"page": 1, "pages": 1, "count": 1,
               "items": [{"id": "6adf725b922c92c2dd6ce7d7", "name": "Docs", "slug": "docs", "articleCount": 2}]}}),
    )))
    .expect("mock server");
    let db = fixture_db();
    let mut c = creds(&mock);
    c.docs_api_key = "docs-key".into();
    let p = provider_with_token(&db, &c);

    let cols = runtime()
        .block_on(async { p.list_doc_collections().await })
        .expect("collections");
    assert_eq!(cols.len(), 1);
    assert_eq!(cols[0].name, "Docs");
    // Audit fix (C2, live): the Docs API's ids are 24-char hex ObjectIds —
    // the mirror must carry them verbatim (the old i64 parse zeroed them).
    assert_eq!(cols[0].remote_id, "6adf725b922c92c2dd6ce7d7");

    let token_req = {
        let hits = mock.hits();
        hits.first().cloned().expect("docs request")
    };
    assert!(
        token_req.path.starts_with("/v1/collections"),
        "docs must use the v1 API, got {}",
        token_req.path
    );
    let expected = {
        use base64::Engine;
        base64::engine::general_purpose::STANDARD.encode("docs-key:X")
    };
    assert_eq!(
        token_req.header("authorization"),
        Some(format!("Basic {expected}").as_str())
    );
}

// ---------------------------------------------------------------------------
// HsApiError plumbing sanity (public helpers used by the shell)
// ---------------------------------------------------------------------------

#[test]
fn hs_status_and_error_helpers_downcast() {
    let _secrets = secrets_guard();
    let e: crate::error::Error = HsApiError {
        status_code: 429,
        message: "m".into(),
        friendly: "f".into(),
        retryable: true,
    }
    .into();
    assert_eq!(hs_status(&e), Some(429));
    assert!(hs_error(&e).is_some());
    assert_eq!(e.to_string(), "f", "Display renders the friendly text");
}

// ---------------------------------------------------------------------------
// v1.4.0 R4 (D7): the reply dispatch resolves the customer from the live
// conversation when the mirror row lacks one, and repairs the mirror.
// ---------------------------------------------------------------------------

#[test]
fn reply_dispatch_resolves_the_customer_from_the_api_and_repairs_the_mirror() {
    let _secrets = secrets_guard();
    let mock = MockServer::start(Arc::new(|req: &crate::mockserver::MockRequest| {
        match (req.method.as_str(), req.path.as_str()) {
            // The dispatch's customer fallback fetches the live
            // conversation; primaryCustomer.id is the answer.
            ("GET", "/v3/conversations/77") => MockResponse::json(
                200,
                json!({
                    "id": 77, "number": 9001, "status": "active",
                    "mailboxId": 1,
                    "primaryCustomer": {"id": 900, "email": "kim@example.com"},
                    "threads": 2,
                    "createdBy": {"type": "customer"},
                    "source": {"type": "email", "via": "customer"}
                }),
            ),
            ("POST", "/v2/conversations/77/reply") => {
                MockResponse::json(201, json!({"id": 950})).with_header("Resource-Id", "950")
            }
            _ => MockResponse::json(404, r#"{"error":"unexpected path"}"#),
        }
    }))
    .expect("mock server");
    let db = fixture_db();
    let creds = creds(&mock);
    let provider = provider_with_token(&db, &creds);

    // A mirror conversation with NO customer link — the pre-1.4.0
    // guaranteed-400 case (D7). A customer row exists but is unlinked.
    db.with_write(|conn| {
        conn.execute(
            "INSERT INTO conversations (remote_id, number, status, mailbox_local_id,
                 subject, created_at, updated_at)
             VALUES (77, 9001, 'active', NULL, 'Customer missing?', 'now', 'now')",
            [],
        )
        .expect("seed conversation");
        let conv_local = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO customers (remote_id, email) VALUES (900, 'kim@example.com')",
            [],
        )
        .expect("seed customer");
        let cust_local = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO outbound_jobs (kind, conversation_id, payload, status, attempts,
                 created_at, updated_at)
             VALUES ('create_reply', ?1, ?2, 'queued', 0, datetime('now'), datetime('now'))",
            rusqlite::params![
                conv_local,
                serde_json::to_string(&json!({
                    "conversationId": conv_local,
                    "text": "We fixed it",
                    "draft": false,
                    "cc": [],
                    "bcc": [],
                    "statusAfter": "pending",
                    "assignTo": null
                }))
                .expect("payload"),
            ],
        )
        .expect("seed outbound job");
        Ok::<(i64, i64), rusqlite::Error>((conv_local, cust_local))
    })
    .expect("seed mirror");
    let (_, seeded_customer_local) = db
        .with_read(|conn| {
            conn.query_row("SELECT id FROM customers WHERE remote_id = 900", [], |r| {
                r.get::<_, i64>(0)
            })
        })
        .map(|id| ((), id))
        .expect("seeded customer read");

    let engine = Arc::new(crate::sync_engine::SyncEngine::new(
        db.clone(),
        Arc::new(provider),
    ));
    // The runtime must exist (and be entered) before start_workers:
    // start() spawns its timer tasks with tokio::spawn, which needs an
    // ambient runtime on this thread. A current-thread runtime keeps the
    // test in control — the spawned timers only advance while a block_on
    // below drives the reactor.
    let rt = runtime();
    let _enter = rt.enter();
    let manager = crate::workers::start_workers(
        db.clone(),
        Some(engine),
        Arc::new(crate::helpscout_real::RealHelpScoutProvider::new(
            db.clone(),
            creds,
        )),
        crate::eventing::EventBus::default(),
        std::env::temp_dir(),
        None,
    );

    rt.block_on(manager.tick());
    // The dispatcher may need a second tick to claim after the fallback
    // GET; tick again to be safe.
    rt.block_on(manager.tick());
    manager.stop();

    let (status, error, mirror_customer) = db.with_read(|conn| {
        let (status, error): (String, Option<String>) = conn
            .query_row(
                "SELECT status, error FROM outbound_jobs ORDER BY id DESC LIMIT 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .expect("job row");
        let mirror_customer: Option<i64> = conn
            .query_row(
                "SELECT c.customer_local_id FROM conversations c WHERE c.remote_id = 77",
                [],
                |r| r.get(0),
            )
            .expect("conversation row");
        (status, error, mirror_customer)
    });
    let reply_hits: Vec<_> = mock
        .hits()
        .iter()
        .filter(|h| h.path.contains("/reply"))
        .cloned()
        .collect();
    assert_eq!(
        reply_hits.len(),
        1,
        "exactly one reply POST (no retries): {:?}",
        mock.hits()
            .iter()
            .map(|h| h.path.clone())
            .collect::<Vec<_>>()
    );
    let body: Value = serde_json::from_str(&reply_hits[0].body).expect("reply body json");
    assert_eq!(
        body["customer"]["id"], 900,
        "D7: the fallback resolved the customer onto the POST body"
    );
    assert_eq!(
        body["status"], "pending",
        "D9: the configured statusAfter rides along"
    );
    assert_eq!(status, "confirmed", "the reply landed; error: {error:?}");
    assert_eq!(
        mirror_customer,
        Some(seeded_customer_local),
        "D7: the mirror row is repaired — the customer link points at the \
         seeded customer"
    );
}

// ---------------------------------------------------------------------------
// Batch 15 (P2/P3): live chats, saved replies, workflows, the Resource-Id fix
// ---------------------------------------------------------------------------

#[test]
fn chat_get_and_send_roundtrip_the_documented_wire_contract() {
    let _secrets = secrets_guard();
    let mock = MockServer::start(Arc::new(|req: &crate::mockserver::MockRequest| {
        match (req.method.as_str(), req.path.as_str()) {
            ("GET", "/v2/chats/chat-9") => MockResponse::json(
                200,
                json!({
                    "id": "chat-9", "mailboxId": 3, "state": "open",
                    "customer": {"id": 7, "first": "Kim", "last": "One"},
                    "events": [{"id": "m-1", "type": "customer.message.created",
                                 "text": "hi", "createdAt": "2026-10-21T10:00:00Z"}]
                }),
            ),
            ("PUT", p) if p.starts_with("/v2/chats/chat-9/messages/") => {
                MockResponse::json(201, json!({"ok": true}))
            }
            _ => MockResponse::json(404, json!({"message": "no route"})),
        }
    }))
    .expect("mock server");
    let db = fixture_db();
    let p = provider_with_token(&db, &creds(&mock));

    let chat = runtime()
        .block_on(async { p.get_chat("chat-9").await })
        .expect("get_chat")
        .expect("chat object");
    assert_eq!(chat.id, "chat-9");
    assert_eq!(chat.mailbox_id, 3);
    assert_eq!(chat.state, "open");
    assert_eq!(chat.customer.expect("customer").remote_id, 7);

    runtime()
        .block_on(async {
            p.send_chat_message("chat-9", "m-uuid-1", "we are on it")
                .await
        })
        .expect("send");

    let hits = mock.hits();
    let send = hits
        .iter()
        .find(|h| h.method == "PUT")
        .expect("the PUT happened");
    assert!(
        send.path.starts_with("/v2/chats/chat-9/messages/m-uuid-1"),
        "the caller-generated message id rides in the path: {}",
        send.path
    );
    let body: Value = serde_json::from_str(&send.body).expect("send body json");
    assert_eq!(body["text"], "we are on it");
}

#[test]
fn chat_send_422_already_in_use_is_treated_as_delivered() {
    let _secrets = secrets_guard();
    let mock = MockServer::start(crate::mockserver::fixed(MockResponse::json(
        422,
        json!({"message": "id value is already in use", "code": 422}),
    )))
    .expect("mock server");
    let db = fixture_db();
    let p = provider_with_token(&db, &creds(&mock));

    // The message id is ours, so the platform's 422 means a retry after a
    // timeout already delivered — never a double-send.
    let outcome =
        runtime().block_on(async { p.send_chat_message("chat-9", "m-uuid-2", "retry me").await });
    assert!(
        outcome.is_ok(),
        "422 must be Ok (already delivered): {outcome:?}"
    );
}

#[test]
fn saved_reply_crud_roundtrips_the_platforms_wire_contract() {
    let _secrets = secrets_guard();
    let mock = MockServer::start(Arc::new(|req: &crate::mockserver::MockRequest| {
        let path = req.path.as_str();
        match req.method.as_str() {
            "POST" if path.ends_with("/saved-replies") => MockResponse::json(
                201,
                json!({"id": 91, "name": "Refund path", "preview": "…"}),
            )
            .with_header("Resource-Id", "91"),
            "PUT" if path.ends_with("/saved-replies/91") => {
                MockResponse::json(200, json!({"id": 91}))
            }
            "DELETE" if path.ends_with("/saved-replies/91") => MockResponse::empty(204),
            _ => MockResponse::json(404, json!({"message": "no route"})),
        }
    }))
    .expect("mock server");
    let db = fixture_db();
    let p = provider_with_token(&db, &creds(&mock));

    let created = runtime()
        .block_on(async { p.create_saved_reply(3, "Refund path", "<p>how</p>").await })
        .expect("create saved reply");
    assert_eq!(
        created, 91,
        "the new remote id comes from the Resource-Id header"
    );

    runtime()
        .block_on(async {
            p.update_saved_reply(3, 91, "Refund path v2", "<p>how v2</p>")
                .await
        })
        .expect("update saved reply");
    runtime()
        .block_on(async { p.delete_saved_reply(3, 91).await })
        .expect("delete saved reply");

    let hits = mock.hits();
    let post = hits
        .iter()
        .find(|h| h.method == "POST")
        .expect("POST happened");
    let body: Value = serde_json::from_str(&post.body).expect("create body json");
    assert_eq!(body["name"], "Refund path", "the platform's required name");
    assert_eq!(body["text"], "<p>how</p>");
    let put = hits
        .iter()
        .find(|h| h.method == "PUT")
        .expect("PUT happened");
    assert!(
        put.path.ends_with("/v2/mailboxes/3/saved-replies/91"),
        "{}",
        put.path
    );
    let delete = hits
        .iter()
        .find(|h| h.method == "DELETE")
        .expect("DELETE happened");
    assert!(
        delete.path.ends_with("/saved-replies/91"),
        "{}",
        delete.path
    );
}

#[test]
fn workflow_run_roundtrips_the_documented_contract() {
    let _secrets = secrets_guard();
    let mock = MockServer::start(Arc::new(|req: &crate::mockserver::MockRequest| {
        match (req.method.as_str(), req.path.as_str()) {
            ("POST", "/v2/workflows/7/run") => MockResponse::json(200, json!({})),
            _ => MockResponse::json(404, json!({"message": "no route"})),
        }
    }))
    .expect("mock server");
    let db = fixture_db();
    let p = provider_with_token(&db, &creds(&mock));

    let ran = runtime()
        .block_on(async { p.run_workflow(7, 900).await })
        .expect("run workflow");
    assert!(ran);

    let hits = mock.hits();
    assert_eq!(hits.len(), 1);
    let body: Value = serde_json::from_str(&hits[0].body).expect("run body json");
    assert_eq!(
        body["conversationIds"],
        json!([900]),
        "the resolved conversation remote id rides in conversationIds"
    );
}

#[test]
fn create_webhook_reads_the_resource_id_header_on_the_201_answer() {
    let _secrets = secrets_guard();
    let mock = MockServer::start(Arc::new(|req: &crate::mockserver::MockRequest| {
        match (req.method.as_str(), req.path.as_str()) {
            // The documented 201: empty body, new hook id in Resource-Id.
            ("POST", "/v2/webhooks") => MockResponse::empty(201).with_header("Resource-Id", "55"),
            _ => MockResponse::json(404, json!({"message": "no route"})),
        }
    }))
    .expect("mock server");
    let db = fixture_db();
    let p = provider_with_token(&db, &creds(&mock));

    let events = vec![
        "convo.created".to_string(),
        "beacon.chat.created".to_string(),
    ];
    let hook_id = runtime()
        .block_on(async {
            p.create_webhook("https://relay/hook", &events, "s", "lbl")
                .await
        })
        .expect("create webhook");

    // Before the fix this read the (empty) body and returned 0 for every
    // created hook, making a successful registration look like a failure.
    assert_eq!(hook_id, 55, "the Resource-Id header must be the answer");

    let hits = mock.hits();
    let body: Value = serde_json::from_str(&hits[0].body).expect("create body json");
    assert_eq!(body["url"], "https://relay/hook");
    assert_eq!(body["label"], "lbl");
    assert_eq!(
        body["events"],
        json!(["convo.created", "beacon.chat.created"])
    );
}

// ---------------------------------------------------------------------------
// Clean-audit batch 21 (items 28 and 30)
// ---------------------------------------------------------------------------

/// Audit item 28: `list_threads` used to read only page 1 of the v3
/// threads collection and hand it to the sync's stale-thread sweep as the
/// conversation's full thread set — every thread past page 1 was deleted
/// from the mirror as "gone from the platform". The read now follows the
/// HAL next link until the pages run out.
#[test]
fn list_threads_follows_hal_pagination_to_the_last_page() {
    let _secrets = secrets_guard();
    let mock = MockServer::start(Arc::new(|req: &crate::mockserver::MockRequest| {
        match (req.method.as_str(), req.path.as_str()) {
            ("GET", "/v3/conversations/101/threads") if req.query_param("page").is_none() => {
                MockResponse::json(
                    200,
                    json!({
                        "_embedded": {"threads": [
                            {"id": 501, "type": "customer", "status": "active",
                             "state": "published", "body": "<p>one</p>",
                             "createdByCustomer": {"id": 900},
                             "createdAt": "2026-10-01T10:00:00Z"}
                        ]},
                        "_links": {"next": {"href": "/v3/conversations/101/threads?page=2"}}
                    }),
                )
            }
            ("GET", "/v3/conversations/101/threads")
                if req.query_param("page").as_deref() == Some("2") =>
            {
                MockResponse::json(
                    200,
                    json!({
                        "_embedded": {"threads": [
                            {"id": 502, "type": "customer", "status": "active",
                             "state": "published", "body": "<p>two</p>",
                             "createdByCustomer": {"id": 900},
                             "createdAt": "2026-10-01T11:00:00Z"},
                            {"id": 503, "type": "message", "status": "active",
                             "state": "published", "body": "<p>three</p>",
                             "createdBy": {"id": 42, "type": "user"},
                             "createdAt": "2026-10-01T12:00:00Z"}
                        ]}
                    }),
                )
            }
            _ => MockResponse::json(404, json!({"error": "unexpected path"})),
        }
    }))
    .expect("mock server");
    let db = fixture_db();
    let p = provider_with_token(&db, &creds(&mock));

    let threads = runtime()
        .block_on(async { p.list_threads(101).await })
        .expect("list_threads");
    assert_eq!(
        threads.iter().map(|t| t.remote_id).collect::<Vec<_>>(),
        vec![501, 502, 503],
        "both pages must arrive — page 1 alone is not the thread set"
    );

    assert_eq!(mock.hit_count("GET", "/v3/conversations/101/threads"), 2);
    let page2 = mock
        .hits()
        .iter()
        .any(|h| h.query_param("page").as_deref() == Some("2"));
    assert!(page2, "the HAL next link must be followed exactly once");
}

/// Audit item 30: a retried saved-reply create re-POSTed the payload and
/// duplicated the reply on the platform whenever the first attempt's POST
/// had landed but the response or the post-write re-read failed. The
/// create is now idempotent by its natural key (mailbox + name + text):
/// the retry finds its own earlier write and skips the re-POST.
#[test]
fn retried_saved_reply_create_never_reposts() {
    let _secrets = secrets_guard();
    // GET sequence on /saved-replies:
    //   1 — attempt 1 pre-check: the mailbox holds nothing yet
    //   2 — attempt 1 post-create re-read: 404 (a non-retryable answer,
    //       so it surfaces through the in-client GET retry layer — the
    //       POST landed, the job still fails and is requeued)
    //   3 — attempt 2 pre-check: the platform now holds the twin
    //   4 — attempt 2 post-create re-read: success, job completes
    let gets = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counter = Arc::clone(&gets);
    let mock = MockServer::start(Arc::new(move |req: &crate::mockserver::MockRequest| {
        let path = req.path.as_str();
        match req.method.as_str() {
            "GET" if path.ends_with("/saved-replies") => {
                let n = counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
                match n {
                    1 => MockResponse::json(200, json!({"_embedded": {"savedReplies": []}})),
                    2 => MockResponse::json(404, json!({"message": "re-read 404"})),
                    _ => MockResponse::json(
                        200,
                        json!({"_embedded": {"savedReplies": [
                            {"id": 91, "name": "Refund path", "text": "<p>how</p>"}
                        ]}}),
                    ),
                }
            }
            "POST" if path.ends_with("/saved-replies") => {
                MockResponse::json(201, json!({"id": 91, "name": "Refund path"}))
                    .with_header("Resource-Id", "91")
            }
            _ => MockResponse::json(404, json!({"message": "no route"})),
        }
    }))
    .expect("mock server");
    let db = fixture_db();
    let p = provider_with_token(&db, &creds(&mock));
    let manager = crate::workers::WorkerManager::new(
        db.clone(),
        None,
        Arc::new(p) as Arc<dyn HelpScoutProvider>,
        crate::eventing::EventBus::default(),
        std::env::temp_dir(),
        None,
    );

    let payload = json!({
        "action": "create",
        "mailboxRemoteId": 3,
        "name": "Refund path",
        "text": "<p>how</p>"
    })
    .to_string();
    db.with_write(|conn| {
        crate::jobs::enqueue_on(conn, "sync", "saved_reply_write", &payload, 1, 3)
    })
    .expect("enqueue");

    // Attempt 1: pre-check (empty) → POST lands → re-read 500s → the job
    // is requeued as retryable.
    runtime().block_on(async { manager.tick().await });
    let posts_after_first = mock.hits().iter().filter(|h| h.method == "POST").count();
    assert_eq!(posts_after_first, 1, "the first attempt POSTs once");
    let status: String = db
        .with_read(|conn| {
            conn.query_row(
                "SELECT status FROM jobs WHERE type = 'saved_reply_write'",
                [],
                |r| r.get(0),
            )
        })
        .expect("job row");
    assert_eq!(status, "queued", "the failed re-read must requeue the job");

    // Fast-forward past the retry backoff and run the retry.
    db.with_write(|conn| {
        conn.execute(
            "UPDATE jobs SET run_at = NULL WHERE type = 'saved_reply_write'",
            [],
        )
        .expect("clear backoff")
    });
    runtime().block_on(async { manager.tick().await });

    let posts_after_retry = mock.hits().iter().filter(|h| h.method == "POST").count();
    assert_eq!(
        posts_after_retry, 1,
        "the retried create must NOT re-POST — the platform already holds the twin"
    );
    let status: String = db
        .with_read(|conn| {
            conn.query_row(
                "SELECT status FROM jobs WHERE type = 'saved_reply_write'",
                [],
                |r| r.get(0),
            )
        })
        .expect("job row");
    assert_eq!(status, "completed", "the retry completes via the re-read");
    let mirrored: i64 = db
        .with_read(|conn| {
            conn.query_row(
                "SELECT COUNT(*) FROM saved_replies WHERE remote_id = 91",
                [],
                |r| r.get(0),
            )
        })
        .expect("mirror row");
    assert_eq!(
        mirrored, 1,
        "the re-read still lands the reply in the mirror"
    );
}

/// Clean-audit batch 28, item 92: `get_rating`'s 404 mapping. The pre-fix
/// branch matched the literal "-> 404" substring of the error's Display —
/// but HsApiError's Display prints the FRIENDLY text, so the mapping
/// never fired and a genuinely-absent rating surfaced as an Err (with
/// batch 27's item 85, retried and then failed) instead of the honest
/// Ok(None) every other 404-tolerant fetch returns.
#[test]
fn audit92_get_rating_maps_404_to_none_not_an_error() {
    let _secrets = secrets_guard();
    let db = fixture_db();
    // 404 on the rating id, 400 on everything else (so a wrong-status
    // answer cannot accidentally pass as "not found").
    let mock = MockServer::start(Arc::new(|req: &crate::mockserver::MockRequest| {
        if req.method == "GET" && req.path == "/v2/ratings/404" {
            MockResponse::json(404, r#"{"error":"not found"}"#)
        } else {
            MockResponse::json(400, r#"{"error":"bad request"}"#)
        }
    }))
    .expect("mock server");
    let creds = creds(&mock);
    let provider = provider_with_token(&db, &creds);

    let gone = runtime()
        .block_on(async { provider.get_rating(404).await })
        .expect("a 404 rating is Ok(None), not an error");
    assert!(gone.is_none(), "the absent rating maps to None");

    // And the error path proves the mapping is not "any error is None":
    // a 400 on the same endpoint stays an Err carrying its status.
    let err = runtime()
        .block_on(async { provider.get_rating(400).await })
        .expect_err("a hard 400 stays an error");
    assert_eq!(hs_status(&err), Some(400), "the error keeps its status");
}
