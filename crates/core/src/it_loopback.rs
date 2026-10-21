//! Integration tests (TODO 45): the loopback listener over real TCP.
//!
//! The unit tests in `loopback.rs` call `handle_request` in-process; these
//! tests start the actual `LoopbackServer` accept loop and drive it with a
//! raw HTTP client over a socket, which is the exact path a Help Scout
//! webhook POST or a browser OAuth redirect takes in production:
//!
//! * webhook ingress: HMAC-SHA1 signature verification, persistence,
//!   deduplication, job enqueue, status-code mapping, a concurrent burst;
//! * the OAuth authorization-code flow end-to-end: `begin_oauth` issues a
//!   single-use state, the browser-style `GET /oauth/callback?code=…`
//!   lands on the listener, the **real** `RealHelpScoutProvider` exchanges
//!   the code against the mock Help Scout token endpoint, fetches the
//!   connected user, and the tokens/audit rows land in the database.
//!
//! Secrets use the encrypted envelope (`settings::set_secret`) exactly as
//! production does — the master key comes from a per-process key file.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::Arc;

use rusqlite::Connection;

use crate::db::DbHandle;
use crate::helpscout_real::{HsCredentials, RealHelpScoutProvider};
use crate::loopback::LoopbackServer;

fn fixture_db() -> DbHandle {
    let mut conn = Connection::open_in_memory().expect("in-memory connection");
    crate::bootstrap::apply_all(&mut conn).expect("migrations");
    DbHandle::new(conn)
}

/// Hold the process-global secrets lock for the test's duration and
/// install the deterministic master key the whole test binary shares
/// (R7/D13: the OAuth flow and webhook-secret resolution now both touch
/// the encrypted envelope, so the key must stay stable while a test runs;
/// other modules clear/replace the key under this same lock).
fn install_master_key() -> std::sync::MutexGuard<'static, ()> {
    let guard = crate::settings::SECRETS_TEST_LOCK
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    crate::settings::set_master_key_for_tests([7u8; 32]);
    guard
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

/// Minimal HTTP/1.1 client over a raw socket: send, read to EOF (the
/// loopback server closes after responding), return (status, body).
fn http(port: u16, request: &str) -> (u16, String) {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect to loopback");
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(10)))
        .expect("read timeout");
    stream
        .set_write_timeout(Some(std::time::Duration::from_secs(10)))
        .expect("write timeout");
    stream.write_all(request.as_bytes()).expect("write request");
    stream.flush().expect("flush");
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).expect("read response");
    let text = String::from_utf8_lossy(&buf).into_owned();
    let status = text
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse::<u16>().ok())
        .expect("status line");
    let body = text
        .split_once("\r\n\r\n")
        .map(|(_, b)| b.to_string())
        .unwrap_or_default();
    (status, body)
}

fn post_webhook(port: u16, signature: Option<&str>, event: &str, body: &str) -> (u16, String) {
    let sig_header = signature
        .map(|s| format!("X-HelpScout-Signature: {s}\r\n"))
        .unwrap_or_default();
    let req = format!(
        "POST /webhook HTTP/1.1\r\nHost: 127.0.0.1\r\n{sig_header}\
         X-HelpScout-Event: {event}\r\nContent-Type: application/json\r\n\
         Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    http(port, &req)
}

fn get(port: u16, target: &str) -> (u16, String) {
    let req = format!("GET {target} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n");
    http(port, &req)
}

// ---------------------------------------------------------------------------
// Webhook ingress over real TCP
// ---------------------------------------------------------------------------

#[test]
fn signed_webhook_roundtrip_over_tcp() {
    let _secrets = install_master_key();
    let db = fixture_db();
    db.with_write(|conn| crate::settings::set_secret(conn, "webhook_secret", b"top-secret"))
        .expect("store webhook secret");
    let mut server = LoopbackServer::start(
        db.clone(),
        std::sync::Arc::new(crate::provider_handle::ProviderHandle::new_empty()),
        0,
    )
    .expect("start listener");
    let port = server.port();

    let (status, body) = get(port, "/healthz");
    assert_eq!(status, 200, "listener must answer healthz");
    assert!(body.contains("supportos-loopback"));

    let secret = b"top-secret";
    let event_body = r#"{"objectID":123,"number":778}"#;
    let sig = crate::webhook::compute_signature(secret, event_body.as_bytes());

    let (status, body) = post_webhook(port, Some(&sig), "convo.created", event_body);
    assert_eq!(status, 200, "signed webhook accepted: {body}");
    assert_eq!(body, "{\"received\":true}");

    // Persisted, marked processed, and the sync job enqueued.
    let (state, jobs): (String, i64) = db
        .with_read(|conn| {
            conn.query_row(
                "SELECT (SELECT processing_state FROM webhook_events WHERE event_type = 'convo.created'),
                        (SELECT COUNT(*) FROM jobs WHERE type = 'sync_conversation')",
                [],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)),
            )
            .unwrap_or(("missing".to_string(), 0))
        });
    assert_eq!(state, "processed");
    assert_eq!(jobs, 1);

    // A full duplicate replays the same row: 200, flagged, no new job.
    let (status, body) = post_webhook(port, Some(&sig), "convo.created", event_body);
    assert_eq!(status, 200);
    assert!(body.contains("\"duplicate\":true"), "body: {body}");
    let jobs: i64 = db
        .with_read(|conn| {
            conn.query_row(
                "SELECT COUNT(*) FROM jobs WHERE type = 'sync_conversation'",
                [],
                |r| r.get(0),
            )
        })
        .unwrap_or(-1);
    assert_eq!(jobs, 1, "duplicates must not enqueue twice");

    // Tampered body (signature no longer matches): 401, nothing stored.
    let (status, _) = post_webhook(port, Some(&sig), "convo.created", r#"{"objectID":124}"#);
    assert_eq!(status, 401);
    let events: i64 = db
        .with_read(|conn| conn.query_row("SELECT COUNT(*) FROM webhook_events", [], |r| r.get(0)))
        .unwrap_or(-1);
    assert_eq!(events, 1, "rejected webhook must not be stored");

    // Unsigned: 401.
    let (status, _) = post_webhook(port, None, "convo.created", r#"{"objectID":125}"#);
    assert_eq!(status, 401);

    server.stop();
}

#[test]
fn webhook_without_configured_secret_hard_rejects() {
    let _secrets = install_master_key();
    let db = fixture_db(); // no webhook_secret stored
    let mut server = LoopbackServer::start(
        db.clone(),
        std::sync::Arc::new(crate::provider_handle::ProviderHandle::new_empty()),
        0,
    )
    .expect("start listener");

    let (status, body) = post_webhook(server.port(), None, "convo.created", r#"{"id":9}"#);
    assert_eq!(
        status, 403,
        "unsigned + unconfigured secret must 403: {body}"
    );
    let events: i64 = db
        .with_read(|conn| conn.query_row("SELECT COUNT(*) FROM webhook_events", [], |r| r.get(0)))
        .unwrap_or(-1);
    assert_eq!(events, 0);
    server.stop();
}

#[test]
fn webhook_burst_is_served_concurrently() {
    let _secrets = install_master_key();
    let db = fixture_db();
    db.with_write(|conn| crate::settings::set_secret(conn, "webhook_secret", b"burst-secret"))
        .expect("store webhook secret");
    let mut server = LoopbackServer::start(
        db.clone(),
        std::sync::Arc::new(crate::provider_handle::ProviderHandle::new_empty()),
        0,
    )
    .expect("start listener");
    let port = server.port();

    // 12 concurrent POSTs of distinct events: the per-connection threads
    // must all answer, all rows must land.
    let mut handles = Vec::new();
    for id in 1..=12i64 {
        let handle = std::thread::spawn(move || {
            let body = format!(r#"{{"objectID":{id}}}"#);
            let sig = crate::webhook::compute_signature(b"burst-secret", body.as_bytes());
            post_webhook(port, Some(&sig), "convo.customer.reply.created", &body)
        });
        handles.push(handle);
    }
    let mut statuses = Vec::new();
    for handle in handles {
        let (status, _) = handle.join().expect("worker thread");
        statuses.push(status);
    }
    assert!(
        statuses.iter().all(|s| *s == 200),
        "all burst webhooks must be accepted: {statuses:?}"
    );
    let mut ok_events = 0i64;
    let mut jobs = 0i64;
    db.with_read(|conn| {
        ok_events = conn
            .query_row(
                "SELECT COUNT(*) FROM webhook_events WHERE processing_state = 'processed'",
                [],
                |r| r.get::<_, i64>(0),
            )
            .unwrap_or(0);
        jobs = conn
            .query_row(
                "SELECT COUNT(*) FROM jobs WHERE type = 'sync_conversation'",
                [],
                |r| r.get::<_, i64>(0),
            )
            .unwrap_or(0);
    });
    assert_eq!((ok_events, jobs), (12, 12));
    server.stop();
}

// ---------------------------------------------------------------------------
// OAuth authorization-code flow, end-to-end over TCP
// ---------------------------------------------------------------------------

#[test]
fn oauth_authorization_code_flow_over_tcp() {
    let _secrets = install_master_key();
    let db = fixture_db();

    // Mock Help Scout: token endpoint + the connected user.
    let me = serde_json::json!({
        "id": 42, "firstName": "Kim", "lastName": "Pearce",
        "email": "kim@example.com", "role": "owner", "type": "user"
    });
    let me_body = me.to_string();
    let mock = crate::mockserver::MockServer::start(Arc::new(
        move |req: &crate::mockserver::MockRequest| {
            if req.path == "/v2/oauth2/token" {
                return crate::mockserver::MockResponse::json(
                    200,
                    serde_json::json!({
                        "access_token": "tcp-flow-token",
                        "refresh_token": "tcp-flow-refresh",
                        "token_type": "bearer",
                        "expires_in": 86400,
                        "scope": "read write"
                    })
                    .to_string(),
                );
            }
            if req.path == "/v2/users/me" {
                return crate::mockserver::MockResponse::json(200, me_body.clone());
            }
            crate::mockserver::MockResponse::json(404, r#"{"error":"unexpected"}"#)
        },
    ))
    .expect("mock server");

    let creds = HsCredentials {
        client_id: "test-client-id".into(),
        client_secret: "test-client-secret".into(),
        redirect_uri: "http://127.0.0.1:1/oauth/callback".to_string(),
        api_base: mock.url(),
        webhook_secret: String::new(),
        docs_api_key: String::new(),
        docs_api_base: mock.url(),
    };
    let provider = Arc::new(RealHelpScoutProvider::new(db.clone(), creds.clone()));

    let mut server = LoopbackServer::start(
        db.clone(),
        std::sync::Arc::new(crate::provider_handle::ProviderHandle::new_real(
            provider.clone(),
        )),
        0,
    )
    .expect("listener");
    let port = server.port();

    // begin_oauth issues the single-use state and builds the real
    // authorize URL.
    let redirect = format!("http://127.0.0.1:{port}/oauth/callback");
    let authorize_url = crate::loopback::begin_oauth(&db, &creds, &redirect).expect("begin_oauth");
    assert!(
        authorize_url.starts_with(crate::helpscout_real::HS_AUTHORIZE_URL),
        "authorize URL must target Help Scout's authorize screen"
    );
    let state = authorize_url
        .rsplit("state=")
        .next()
        .unwrap_or_default()
        .to_string();
    assert_eq!(state.len(), 32, "state is 16 random bytes hex-encoded");

    // The browser lands on the callback.
    let target = format!("/oauth/callback?code=THE-CODE&state={state}");
    let (status, body) = get(port, &target);
    assert_eq!(status, 200);
    assert!(
        body.contains("Connected to Help Scout"),
        "success page: {body}"
    );
    assert!(body.contains("Kim Pearce"));
    assert!(body.contains("kim@example.com"));

    // The code was exchanged against the mock token endpoint with the
    // generated test credentials.
    let token_req = {
        let hits = mock.hits();
        hits.iter()
            .find(|h| h.path == "/v2/oauth2/token")
            .expect("token exchange recorded")
            .clone()
    };
    assert_eq!(token_req.method, "POST");
    assert_eq!(
        token_req.form_value("grant_type").as_deref(),
        Some("authorization_code")
    );
    assert_eq!(token_req.form_value("code").as_deref(), Some("THE-CODE"));
    assert_eq!(
        token_req.form_value("client_id").as_deref(),
        Some("test-client-id")
    );
    assert_eq!(
        token_req.form_value("client_secret").as_deref(),
        Some("test-client-secret")
    );

    // Tokens persisted in the envelope; the row keeps metadata only
    // (R7/D13: no plaintext tokens in sqlite).
    let (envelope_access, envelope_refresh) = envelope_tokens(&db);
    assert_eq!(envelope_access.as_deref(), Some("tcp-flow-token"));
    assert_eq!(envelope_refresh.as_deref(), Some("tcp-flow-refresh"));
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

    let me_id: Option<String> = db
        .with_read(|conn| {
            conn.query_row(
                "SELECT value FROM application_settings WHERE key = 'me_remote_id'",
                [],
                |r| r.get(0),
            )
            .ok()
        })
        .expect("settings readable");
    assert_eq!(me_id.as_deref(), Some("42"));

    let audits: i64 = db.with_read(|conn| {
        conn.query_row(
            "SELECT COUNT(*) FROM audit_log WHERE action = 'helpscout_connected'",
            [],
            |r| r.get::<_, i64>(0),
        )
        .unwrap_or(0)
    });
    assert!(audits >= 1, "connection must be audited");

    // The state is single-use: replaying it fails safely without another
    // exchange.
    let exchanges_before = mock.hit_count("POST", "/v2/oauth2/token");
    let (status, body) = get(port, &target);
    assert_eq!(status, 200);
    assert!(
        body.contains("did not match"),
        "replayed state must be rejected: {body}"
    );
    assert_eq!(
        mock.hit_count("POST", "/v2/oauth2/token"),
        exchanges_before,
        "a replayed state must never trigger a second exchange"
    );

    // The provider is now authenticated with the exchanged token.
    let status = provider.oauth_status();
    assert_eq!(status["authenticated"], serde_json::json!(true));

    server.stop();
}

#[test]
fn oauth_callback_reports_token_endpoint_rejection() {
    let _secrets = install_master_key();
    let db = fixture_db();
    let mock = crate::mockserver::MockServer::start(crate::mockserver::fixed(
        crate::mockserver::MockResponse::json(400, r#"{"error":"invalid_client"}"#),
    ))
    .expect("mock server");

    let creds = HsCredentials {
        client_id: "test-client-id".into(),
        client_secret: "wrong-secret".into(),
        redirect_uri: String::new(),
        api_base: mock.url(),
        webhook_secret: String::new(),
        docs_api_key: String::new(),
        docs_api_base: mock.url(),
    };
    let provider = Arc::new(RealHelpScoutProvider::new(db.clone(), creds.clone()));

    let mut server = LoopbackServer::start(
        db.clone(),
        std::sync::Arc::new(crate::provider_handle::ProviderHandle::new_real(provider)),
        0,
    )
    .expect("listener");
    let port = server.port();

    let redirect = format!("http://127.0.0.1:{port}/oauth/callback");
    let authorize_url = crate::loopback::begin_oauth(&db, &creds, &redirect).expect("begin_oauth");
    let state = authorize_url
        .rsplit("state=")
        .next()
        .unwrap_or_default()
        .to_string();

    let target = format!("/oauth/callback?code=BAD&state={state}");
    let (status, body) = get(port, &target);
    assert_eq!(status, 200);
    assert!(
        body.contains("not completed"),
        "failure page must explain: {body}"
    );

    // No token may have been stored for a failed exchange.
    let tokens: i64 = db
        .with_read(|conn| {
            conn.query_row(
                "SELECT COUNT(*) FROM oauth_tokens WHERE access_token != ''",
                [],
                |r| r.get(0),
            )
        })
        .unwrap_or(-1);
    assert_eq!(tokens, 0);
    server.stop();
}

// ---------------------------------------------------------------------------
// P1: satisfaction-webhook ingress over real TCP (relay-ready ingestion)
// ---------------------------------------------------------------------------

/// A relay (P4's user-run tunnel) forwards Help Scout's
/// `satisfaction.ratings` webhook to the loopback listener; the signed
/// event must verify, persist, and enqueue the fetch-by-id job — the
/// only ratings path the platform offers (there is no list-ratings API).
#[test]
fn satisfaction_webhook_ingests_over_tcp() {
    let _secrets = install_master_key();
    let db = fixture_db();
    db.with_write(|conn| crate::settings::set_secret(conn, "webhook_secret", b"top-secret"))
        .expect("store webhook secret");
    let mut server = LoopbackServer::start(
        db.clone(),
        std::sync::Arc::new(crate::provider_handle::ProviderHandle::new_empty()),
        0,
    )
    .expect("start listener");
    let port = server.port();

    let secret = b"top-secret";
    // The documented payload shape: a Rating object with ratingId plus the
    // conversation reference.
    let event_body =
        r#"{"ratingId": 501, "conversationId": 900, "rating": "great", "comments": "Fast help"}"#;
    let sig = crate::webhook::compute_signature(secret, event_body.as_bytes());
    let (status, body) = post_webhook(port, Some(&sig), "satisfaction.ratings", event_body);
    assert_eq!(status, 200, "signed satisfaction webhook accepted: {body}");

    let (state, jobs): (String, i64) = db.with_read(|conn| {
        conn.query_row(
            "SELECT (SELECT processing_state FROM webhook_events
                         WHERE event_type = 'satisfaction.ratings'),
                        (SELECT COUNT(*) FROM jobs
                         WHERE type = 'sync_conversation_ratings')",
            [],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)),
        )
        .unwrap_or(("missing".to_string(), 0))
    });
    assert_eq!(state, "processed");
    assert_eq!(jobs, 1, "the fetch-by-id rating job is enqueued");

    // Unsigned satisfaction events are rejected at the door, exactly like
    // every other event kind.
    let (status, body) = post_webhook(port, None, "satisfaction.ratings", event_body);
    assert_ne!(status, 200, "unsigned events must not pass: {body}");
    server.stop();
}

// P2: the live-chat mirror ingests over real TCP — the five
// `beacon.chat.*` events are the platform's only push path for chats
// (Help Scout has no list endpoint), so the relay → listener → mirror
// chain is the whole story, verified end to end.
#[test]
fn beacon_chat_created_ingests_into_the_mirror_over_tcp() {
    let _secrets = install_master_key();
    let db = fixture_db();
    db.with_write(|conn| crate::settings::set_secret(conn, "webhook_secret", b"chat-secret"))
        .expect("store webhook secret");
    let mut server = LoopbackServer::start(
        db.clone(),
        std::sync::Arc::new(crate::provider_handle::ProviderHandle::new_empty()),
        0,
    )
    .expect("start listener");
    let port = server.port();

    // The documented shape: the five beacon.chat.* events carry the Chat
    // object; chat ids are UUID strings, unlike every numeric Inbox id.
    let event_body = r#"{"chat": {"id": "b7e5-uuid-1", "mailboxId": 3, "state": "open",
        "customer": {"id": 71, "first": "Kim", "last": "One"},
        "events": [{"id": "m-1", "type": "customer.message.created",
                     "text": "hello from beacon", "createdAt": "2026-10-21T11:00:00Z"}]}}"#;
    let sig = crate::webhook::compute_signature(b"chat-secret", event_body.as_bytes());
    let (status, body) = post_webhook(port, Some(&sig), "beacon.chat.created", event_body);
    assert_eq!(status, 200, "signed beacon chat webhook accepted: {body}");

    let (state, mirrored, messages): (String, i64, i64) = db.with_read(|conn| {
        conn.query_row(
            "SELECT (SELECT processing_state FROM webhook_events
                         WHERE event_type = 'beacon.chat.created'),
                    (SELECT COUNT(*) FROM beacon_chats WHERE chat_id = 'b7e5-uuid-1'),
                    (SELECT COUNT(*) FROM beacon_chat_messages
                      WHERE chat_id = 'b7e5-uuid-1' AND text = 'hello from beacon')",
            [],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, i64>(2)?,
                ))
            },
        )
        .unwrap_or(("missing".to_string(), 0, 0))
    });
    assert_eq!(state, "processed");
    assert_eq!(mirrored, 1, "the chat object must land in the m059 mirror");
    assert_eq!(messages, 1, "the embedded customer message is stored");

    // Unsigned chat events are rejected at the door like everything else.
    let (status, body) = post_webhook(port, None, "beacon.chat.created", event_body);
    assert_ne!(status, 200, "unsigned events must not pass: {body}");
    server.stop();
}
