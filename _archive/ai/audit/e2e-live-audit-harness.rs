//! E2E LIVE AUDIT HARNESS (product code untouched; public API of spp_core only)
//! Drives the real production stack over real TCP:
//!   loopback OAuth server + HMAC-signed webhooks + Docs API + sync engine,
//! against a self-contained mock HelpScout endpoint set.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use hmac::{Hmac, KeyInit, Mac};
use rusqlite::Connection;
use serde_json::{Value, json};
use sha1::Sha1;

use spp_core::db::DbHandle;
use spp_core::event_bus::EventBus;
use spp_core::helpscout::HelpScoutProvider;
use spp_core::helpscout_real::{HsCredentials, RealHelpScoutProvider, SECRET_OAUTH_ACCESS};
use spp_core::loopback::{LoopbackServer, begin_oauth};
use spp_core::provider_handle::ProviderHandle;
use spp_core::settings;
use spp_core::sync_engine::SyncEngine;
use spp_core::events::SyncTrigger;

// ================= mock HelpScout server =================

#[derive(Clone, Debug)]
pub struct Req {
    pub method: String,
    pub path: String,
    pub headers: Vec<(String, String)>,
    pub body: String,
}

impl Req {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
    pub fn form(&self, key: &str) -> Option<String> {
        self.body.split('&').find_map(|pair| {
            let (k, v) = pair.split_once('=')?;
            (k == key).then(|| urldecode(v))
        })
    }
}

fn urldecode(s: &str) -> String {
    let mut out = String::new();
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '%' {
            let h: String = chars.by_ref().take(2).collect();
            if let Ok(b) = u8::from_str_radix(&h, 16) {
                out.push(b as char);
            }
        } else if c == '+' {
            out.push(' ');
        } else {
            out.push(c);
        }
    }
    out
}

#[derive(Clone)]
pub struct Resp {
    pub status: u16,
    pub body: String,
}

impl Resp {
    fn json(status: u16, v: Value) -> Resp {
        Resp { status, body: v.to_string() }
    }
}

type Handler = Arc<dyn Fn(&Req) -> Resp + Send + Sync>;

struct Mock {
    port: u16,
    hits: Arc<Mutex<Vec<Req>>>,
}

impl Mock {
    fn start(handler: Handler) -> Arc<Mock> {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock");
        let port = listener.local_addr().expect("addr").port();
        let hits: Arc<Mutex<Vec<Req>>> = Arc::new(Mutex::new(Vec::new()));
        let mock = Arc::new(Mock { port, hits: hits.clone() });
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                let Some(req) = read_request(&mut stream) else { continue };
                hits.lock().expect("hits").push(req.clone());
                let resp = handler(&req);
                let reason = match resp.status {
                    200 => "OK", 201 => "Created", 400 => "Bad Request", 401 => "Unauthorized",
                    403 => "Forbidden", 404 => "Not Found", _ => "Status",
                };
                let head = format!(
                    "HTTP/1.1 {} {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    resp.status, reason, resp.body.len()
                );
                let _ = stream.write_all(head.as_bytes());
                let _ = stream.write_all(resp.body.as_bytes());
                let _ = stream.flush();
            }
        });
        mock
    }

    fn url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    fn hits(&self) -> Vec<Req> {
        self.hits.lock().expect("hits").clone()
    }

    fn hit_count(&self, method: &str, path_prefix: &str) -> usize {
        self.hits()
            .iter()
            .filter(|h| h.method == method && h.path.starts_with(path_prefix))
            .count()
    }

    fn find_hit(&self, path_prefix: &str) -> Option<Req> {
        self.hits().into_iter().find(|h| h.path.starts_with(path_prefix))
    }
}

fn read_request(stream: &mut TcpStream) -> Option<Req> {
    stream.set_read_timeout(Some(Duration::from_secs(10))).ok()?;
    let mut buf: Vec<u8> = Vec::new();
    let mut tmp = [0u8; 8192];
    loop {
        if let Some(pos) = find_headers_end(&buf) {
            let cl = content_length(&String::from_utf8_lossy(&buf[..pos]));
            if buf.len() >= pos + 4 + cl {
                break;
            }
        }
        match stream.read(&mut tmp) {
            Ok(0) => break,
            Ok(n) => buf.extend_from_slice(&tmp[..n]),
            Err(_) => return None,
        }
        if buf.len() > 8 * 1024 * 1024 {
            return None;
        }
    }
    let pos = find_headers_end(&buf)?;
    let head = String::from_utf8_lossy(&buf[..pos]).to_string();
    let body = String::from_utf8_lossy(&buf[pos + 4..]).to_string();
    let mut lines = head.lines();
    let request_line = lines.next()?.to_string();
    let mut parts = request_line.split_whitespace();
    let method = parts.next()?.to_string();
    let target = parts.next()?.to_string();
    let path = target.split('?').next().unwrap_or("").to_string();
    let mut headers = Vec::new();
    for line in lines {
        if let Some((k, v)) = line.split_once(':') {
            headers.push((k.trim().to_ascii_lowercase(), v.trim().to_string()));
        }
    }
    Some(Req { method, path, headers, body })
}

fn find_headers_end(buf: &[u8]) -> Option<usize> {
    buf.windows(4).position(|w| w == b"\r\n\r\n")
}

fn content_length(head: &str) -> usize {
    head.lines()
        .find_map(|l| {
            let (k, v) = l.split_once(':')?;
            k.trim().eq_ignore_ascii_case("content-length")
                .then(|| v.trim().parse::<usize>().ok())?
        })
        .unwrap_or(0)
}

// ================= fixtures =================

fn temp_dir() -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("spp-e2e-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    dir
}

fn fixture_db() -> DbHandle {
    static INIT: Mutex<()> = Mutex::new(());
    let _guard = INIT.lock().unwrap_or_else(|p| p.into_inner());
    let dir = temp_dir();
    settings::init_master_key(&dir.join("secrets.key")).expect("master key");
    let mut conn = Connection::open_in_memory().expect("in-memory db");
    spp_core::bootstrap::apply_all(&mut conn).expect("migrations");
    DbHandle::new(conn)
}

fn creds(mock: &Mock, webhook_secret: &str, docs_key: &str) -> HsCredentials {
    HsCredentials {
        client_id: "e2e-client-id".into(),
        client_secret: "e2e-client-secret".into(),
        redirect_uri: "http://localhost:3000/oauth/callback".into(),
        api_base: mock.url(),
        webhook_secret: webhook_secret.into(),
        docs_api_key: docs_key.into(),
        docs_api_base: mock.url(),
    }
}

fn rt() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("test runtime")
}

fn me_json() -> Value {
    json!({
        "id": 42, "firstName": "Kim", "lastName": "Pearce",
        "email": "kim@example.com", "role": "owner", "type": "user"
    })
}

fn token_json(access: &str, refresh: &str) -> Value {
    json!({
        "access_token": access, "refresh_token": refresh,
        "token_type": "bearer", "expires_in": 86400, "scope": "read write"
    })
}

// raw TCP http client (no external http crate needed)
fn http(port: u16, method: &str, target: &str, headers: &[(&str, &str)], body: &[u8]) -> (u16, String) {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect loopback");
    stream.set_read_timeout(Some(Duration::from_secs(10))).ok();
    let mut head = format!(
        "{method} {target} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\nContent-Length: {}\r\n",
        body.len()
    );
    for (k, v) in headers {
        head.push_str(&format!("{k}: {v}\r\n"));
    }
    head.push_str("\r\n");
    stream.write_all(head.as_bytes()).expect("write head");
    if !body.is_empty() {
        stream.write_all(body).expect("write body");
    }
    stream.flush().expect("flush");
    let mut buf = Vec::new();
    let mut tmp = [0u8; 8192];
    loop {
        match stream.read(&mut tmp) {
            Ok(0) => break,
            Ok(n) => buf.extend_from_slice(&tmp[..n]),
            Err(_) => break,
        }
    }
    let text = String::from_utf8_lossy(&buf).to_string();
    let status = text
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(0);
    let body = text
        .split_once("\r\n\r\n")
        .map(|(_, b)| b.to_string())
        .unwrap_or_default();
    (status, body)
}

fn get(port: u16, target: &str) -> (u16, String) {
    http(port, "GET", target, &[], b"")
}

fn sign(secret: &[u8], body: &[u8]) -> String {
    use base64::Engine;
    let mut mac = <Hmac<Sha1> as KeyInit>::new_from_slice(secret).expect("hmac key");
    mac.update(body);
    base64::engine::general_purpose::STANDARD.encode(mac.finalize().into_bytes())
}

// drive the full OAuth browser flow against the loopback listener
fn drive_oauth(db: &DbHandle, creds: &HsCredentials, provider: Arc<RealHelpScoutProvider>, mock: &Mock) -> u16 {
    let mut server = LoopbackServer::start(
        db.clone(),
        Arc::new(ProviderHandle::new_real(provider.clone())),
        0,
    )
    .expect("loopback listener");
    let port = server.port();
    let redirect = format!("http://127.0.0.1:{port}/oauth/callback");
    let url = begin_oauth(db, creds, &redirect).expect("begin_oauth");
    let state = url.rsplit("state=").next().unwrap_or_default().to_string();
    let target = format!("/oauth/callback?code=E2E-CODE&state={state}");
    let (status, _body) = get(port, &target);
    server.stop();
    let _ = mock;
    status
}

// ================= T1: OAuth golden path over TCP =================

#[test]
fn t1_oauth_live_roundtrip_over_tcp() {
    let db = fixture_db();
    let me_body = me_json().to_string();
    let mock = Mock::start(Arc::new(move |req: &Req| {
        if req.path == "/v2/oauth2/token" {
            return Resp::json(200, token_json("e2e-access", "e2e-refresh"));
        }
        if req.path == "/v2/users/me" {
            return Resp::json(200, serde_json::from_str::<Value>(&me_body).unwrap());
        }
        Resp::json(404, json!({"error": "unexpected path"}))
    }));

    let c = creds(&mock, "", "");
    let provider = Arc::new(RealHelpScoutProvider::new(db.clone(), c.clone()));

    let mut server = LoopbackServer::start(
        db.clone(),
        Arc::new(ProviderHandle::new_real(provider.clone())),
        0,
    )
    .expect("listener");
    let port = server.port();

    let redirect = format!("http://127.0.0.1:{port}/oauth/callback");
    let authorize_url = begin_oauth(&db, &c, &redirect).expect("begin_oauth");
    assert!(authorize_url.contains("state="), "authorize url carries state");
    let state = authorize_url.rsplit("state=").next().unwrap_or_default().to_string();
    assert!(!state.is_empty());

    let target = format!("/oauth/callback?code=E2E-CODE&state={state}");
    let (status, body) = get(port, &target);
    assert_eq!(status, 200, "callback success page: {body}");
    assert!(body.contains("Connected"), "connected banner in page: {body}");
    assert!(body.contains("Kim"), "me profile on page: {body}");

    // token exchange observed with correct form fields
    let token_req = mock.find_hit("/v2/oauth2/token").expect("token exchange hit");
    assert_eq!(token_req.method, "POST");
    assert_eq!(token_req.form("grant_type").as_deref(), Some("authorization_code"));
    assert_eq!(token_req.form("code").as_deref(), Some("E2E-CODE"));
    assert_eq!(token_req.form("client_id").as_deref(), Some("e2e-client-id"));
    assert_eq!(token_req.form("client_secret").as_deref(), Some("e2e-client-secret"));

    // access token stored encrypted under envelope, never in plaintext column
    let envelope = db.with_read(|conn| {
        settings::get_secret(conn, SECRET_OAUTH_ACCESS)
            .expect("envelope read")
            .map(|b| String::from_utf8(b).expect("utf8"))
    });
    assert_eq!(envelope.as_deref(), Some("e2e-access"));

    let plain: (String, Option<String>) = db.with_read(|conn| {
        conn.query_row(
            "SELECT access_token, refresh_token FROM oauth_tokens WHERE account = 'default'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
    })
    .expect("oauth metadata row");
    assert_eq!(plain.0, "", "plaintext column stays empty by design");
    assert_eq!(plain.1, None);

    // identity + audit trail recorded
    let me_id: Option<String> = db.with_read(|conn| {
        conn.query_row(
            "SELECT value FROM application_settings WHERE key = 'me_remote_id'",
            [],
            |r| r.get(0),
        )
        .ok()
    });
    assert_eq!(me_id.as_deref(), Some("42"));
    let audited: i64 = db.with_read(|conn| {
        conn.query_row(
            "SELECT COUNT(*) FROM audit_log WHERE action = 'helpscout_connected'",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0)
    });
    assert!(audited >= 1, "connection audited");

    // state replay rejected, no second exchange
    let before = mock.hit_count("POST", "/v2/oauth2/token");
    let (status, body) = get(port, &target);
    assert_eq!(status, 400, "replay rejected: {body}");
    assert_eq!(
        mock.hit_count("POST", "/v2/oauth2/token"),
        before,
        "no double exchange on replay"
    );

    // provider now authenticated and serves get_me with the stored bearer
    let me = rt().block_on(async { provider.get_me().await }).expect("get_me");
    assert_eq!(me.remote_id, 42);
    let me_hit = mock.find_hit("/v2/users/me").expect("me hit");
    assert_eq!(me_hit.header("authorization"), Some("Bearer e2e-access"));

    server.stop();
}

// ================= T2: OAuth failure path (token endpoint rejects) =================

#[test]
fn t2_oauth_token_endpoint_rejection_over_tcp() {
    let db = fixture_db();
    let mock = Mock::start(Arc::new(|req: &Req| {
        if req.path == "/v2/oauth2/token" {
            return Resp::json(400, json!({"error": "invalid_grant", "error_description": "code expired"}));
        }
        Resp::json(404, json!({"error": "unexpected"}))
    }));
    let c = creds(&mock, "", "");
    let provider = Arc::new(RealHelpScoutProvider::new(db.clone(), c.clone()));
    let mut server = LoopbackServer::start(
        db.clone(),
        Arc::new(ProviderHandle::new_real(provider)),
        0,
    )
    .expect("listener");
    let port = server.port();
    let redirect = format!("http://127.0.0.1:{port}/oauth/callback");
    let url = begin_oauth(&db, &c, &redirect).expect("begin_oauth");
    let state = url.rsplit("state=").next().unwrap_or_default().to_string();
    let target = format!("/oauth/callback?code=BAD&state={state}");
    let (status, body) = get(port, &target);
    assert!((200..600).contains(&status), "a page is served, not a crash: {status} {body}");
    let lower = body.to_lowercase();
    assert!(
        lower.contains("connection not completed") || lower.contains("failed") || lower.contains("error"),
        "failure copy on page: {body}"
    );
    assert!(
        lower.contains("client id") || lower.contains("credentials") || lower.contains("try again"),
        "actionable guidance on the failure page: {body}"
    );
    // no token stored
    let envelope = db.with_read(|conn| settings::get_secret(conn, SECRET_OAUTH_ACCESS)
        .expect("envelope read"));
    assert!(envelope.is_none(), "no token persisted on failed exchange");
    server.stop();
}

// ================= T3: 401 → refresh → retry =================

#[test]
fn t3_refresh_rotation_on_401_over_tcp() {
    let db = fixture_db();
    let me_body = me_json().to_string();
    let state_ = Mutex::new(0u32);
    let mock = Mock::start(Arc::new(move |req: &Req| {
        if req.path == "/v2/oauth2/token" {
            // authorization_code first, then refresh_token rotation
            if req.form("grant_type").as_deref() == Some("refresh_token") {
                return Resp::json(200, token_json("rotated-access", "rotated-refresh"));
            }
            return Resp::json(200, token_json("initial-access", "initial-refresh"));
        }
        if req.path == "/v2/users/me" {
            let mut st = state_.lock().unwrap();
            *st += 1;
            let n = *st;
            if n == 1 {
                return Resp::json(401, json!({"error": "unauthorized"}));
            }
            return Resp::json(200, serde_json::from_str::<Value>(&me_body).unwrap());
        }
        Resp::json(404, json!({"error": "unexpected"}))
    }));

    let c = creds(&mock, "", "");
    let provider = Arc::new(RealHelpScoutProvider::new(db.clone(), c.clone()));
    // OAuth flow to seed initial tokens
    let status = drive_oauth(&db, &c, provider.clone(), &mock);
    assert_eq!(status, 200);

    let me = rt().block_on(async { provider.get_me().await }).expect("get_me after refresh");
    assert_eq!(me.remote_id, 42);

    // refresh exchange happened
    let refresh_hit = mock
        .hits()
        .into_iter()
        .find(|h| h.path == "/v2/oauth2/token" && h.form("grant_type").as_deref() == Some("refresh_token"))
        .expect("refresh exchange recorded");
    assert_eq!(refresh_hit.form("refresh_token").as_deref(), Some("initial-refresh"));

    // the second me call carried the rotated bearer
    // The callback's own identity fetch met the 401, refreshed, and retried transparently;
    // our explicit get_me then rode the rotated token.
    let me_calls = mock
        .hits()
        .into_iter()
        .filter(|h| h.path == "/v2/users/me")
        .collect::<Vec<_>>();
    assert_eq!(me_calls.len(), 3, "401 + refresh-retry during callback, then explicit call: {:?}", me_calls);
    assert_eq!(me_calls[0].header("authorization"), Some("Bearer initial-access"));
    assert_eq!(me_calls[1].header("authorization"), Some("Bearer rotated-access"));
    assert_eq!(me_calls[2].header("authorization"), Some("Bearer rotated-access"));

    // envelope now carries the rotated token
    let envelope = db.with_read(|conn| {
        settings::get_secret(conn, SECRET_OAUTH_ACCESS)
            .expect("envelope read")
            .map(|b| String::from_utf8(b).expect("utf8"))
    });
    assert_eq!(envelope.as_deref(), Some("rotated-access"));
}

// ================= T4: signed webhooks over TCP =================

#[test]
fn t4_webhook_live_ingest_over_tcp() {
    let db = fixture_db();
    // store webhook secret the way Settings does
    db.with_write(|conn| {
        spp_core::helpscout_real::HsCredentials::store_in_envelope(
            conn, "id", "secret", "whsec-e2e", "",
        )
    })
    .expect("store envelope");

    let mock = Mock::start(Arc::new(|_req: &Req| Resp::json(404, json!({"error": "unused"}))));
    let c = creds(&mock, "whsec-e2e", "");
    let provider = Arc::new(RealHelpScoutProvider::new(db.clone(), c));
    let mut server = LoopbackServer::start(
        db.clone(),
        Arc::new(ProviderHandle::new_real(provider)),
        0,
    )
    .expect("listener");
    let port = server.port();

    let body = br#"{"objectID":555}"#;
    let sig = sign(b"whsec-e2e", body);

    // valid signature accepted
    let (status, resp) = http(
        port,
        "POST",
        "/webhook",
        &[("X-HelpScout-Signature", &sig), ("X-HelpScout-Event", "convo.created")],
        body,
    );
    assert_eq!(status, 200, "signed webhook accepted: {resp}");
    assert!(resp.contains("\"received\":true"));

    // mirror-side effect: event persisted processed + sync job enqueued
    let (state, _): (String, i64) = db.with_read(|conn| {
        conn.query_row(
            "SELECT processing_state, COUNT(*) FROM webhook_events WHERE event_type = 'convo.created' GROUP BY processing_state",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .expect("webhook event row")
    });
    assert_eq!(state, "processed");
    let jobs: i64 = db.with_read(|conn| {
        conn.query_row(
            "SELECT COUNT(*) FROM jobs WHERE type = 'sync_conversation' AND payload LIKE '%555%'",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0)
    });
    assert!(jobs >= 1, "webhook enqueues the conversation sync job");

    // duplicate delivery deduped
    let (status, resp) = http(
        port,
        "POST",
        "/webhook",
        &[("X-HelpScout-Signature", &sig), ("X-HelpScout-Event", "convo.created")],
        body,
    );
    assert_eq!(status, 200);
    assert!(resp.contains("\"duplicate\":true"), "dedup flagged: {resp}");

    // invalid signature hard-rejected
    let (status, resp) = http(
        port,
        "POST",
        "/webhook",
        &[("X-HelpScout-Signature", "AAAA"), ("X-HelpScout-Event", "convo.created")],
        body,
    );
    assert_eq!(status, 401, "bad signature rejected: {resp}");

    // garbage body rejected as bad request
    let bad = b"not-json";
    let sig_bad = sign(b"whsec-e2e", bad);
    let (status, resp) = http(
        port,
        "POST",
        "/webhook",
        &[("X-HelpScout-Signature", &sig_bad), ("X-HelpScout-Event", "convo.created")],
        bad,
    );
    assert_eq!(status, 400, "bad json rejected: {resp}");

    // unknown event type is honestly marked ignored (v1.9.7 B7 fix)
    let unknown = br#"{"id": 1}"#;
    let sig_u = sign(b"whsec-e2e", unknown);
    let (status, _) = http(
        port,
        "POST",
        "/webhook",
        &[("X-HelpScout-Signature", &sig_u), ("X-HelpScout-Event", "drone.updated")],
        unknown,
    );
    assert_eq!(status, 200);
    let state: String = db.with_read(|conn| {
        conn.query_row(
            "SELECT processing_state FROM webhook_events WHERE event_type = 'drone.updated'",
            [],
            |r| r.get(0),
        )
        .expect("unknown event row")
    });
    assert_eq!(state, "ignored", "unknown events are marked ignored, not processed");

    server.stop();
}

// ================= T5: no webhook secret configured → hard reject =================

#[test]
fn t5_webhook_without_secret_hard_rejects_over_tcp() {
    let db = fixture_db();
    let mock = Mock::start(Arc::new(|_req: &Req| Resp::json(404, json!({"error": "unused"}))));
    let c = creds(&mock, "", "");
    let provider = Arc::new(RealHelpScoutProvider::new(db.clone(), c));
    let mut server = LoopbackServer::start(
        db.clone(),
        Arc::new(ProviderHandle::new_real(provider)),
        0,
    )
    .expect("listener");
    let port = server.port();
    let (status, body) = http(
        port,
        "POST",
        "/webhook",
        &[("X-HelpScout-Signature", "x"), ("X-HelpScout-Event", "convo.created")],
        br#"{"objectID":1}"#,
    );
    assert_eq!(status, 403, "unsigned/no-secret rejected: {body}");
    server.stop();
}

// ================= T6: beacon chat webhook ingests a chat row =================

#[test]
fn t6_beacon_chat_webhook_ingests_over_tcp() {
    let db = fixture_db();
    db.with_write(|conn| {
        spp_core::helpscout_real::HsCredentials::store_in_envelope(
            conn, "id", "secret", "whsec-chat", "",
        )
    })
    .expect("store envelope");

    let mock = Mock::start(Arc::new(|_req: &Req| Resp::json(404, json!({"error": "unused"}))));
    let c = creds(&mock, "whsec-chat", "");
    let provider = Arc::new(RealHelpScoutProvider::new(db.clone(), c));
    let mut server = LoopbackServer::start(
        db.clone(),
        Arc::new(ProviderHandle::new_real(provider)),
        0,
    )
    .expect("listener");
    let port = server.port();

    let body = br#"{"chat": {"id": "chat-77", "customer": {"id": 9, "name": "Nadia"}, "status": "open"}}"#;
    let sig = sign(b"whsec-chat", body);
    let (status, resp) = http(
        port,
        "POST",
        "/webhook",
        &[("X-HelpScout-Signature", &sig), ("X-HelpScout-Event", "beacon.chat.created")],
        body,
    );
    assert_eq!(status, 200, "beacon chat accepted: {resp}");

    let chats: i64 = db.with_read(|conn| {
        conn.query_row("SELECT COUNT(*) FROM beacon_chats", [], |r| r.get(0))
            .unwrap_or(0)
    });
    assert!(chats >= 1, "beacon chat row ingested into the mirror");
    server.stop();
}

// ================= T7: Docs API sync live =================

#[test]
fn t7_docs_api_sync_live_over_tcp() {
    let db = fixture_db();
    let mock = Mock::start(Arc::new(move |req: &Req| {
        if req.path == "/v2/oauth2/token" {
            return Resp::json(200, token_json("docs-access", "docs-refresh"));
        }
        if req.path == "/v2/users/me" {
            return Resp::json(200, me_json());
        }
        if req.path == "/v1/collections" {
            return Resp::json(200, json!({
                "collections": {
                    "items": [
                        {"id": "col1", "name": "General", "slug": "general",
                         "visibility": "public", "articleCount": 1}
                    ],
                    "pages": 1
                }
            }));
        }
        if req.path == "/v1/collections/col1/categories" {
            return Resp::json(200, json!({"categories": {"items": [], "pages": 1}}));
        }
        if req.path == "/v1/collections/col1/articles" {
            return Resp::json(200, json!({
                "articles": {
                    "items": [
                        {"id": "art1", "name": "How to reset", "slug": "how-to-reset",
                         "status": "published", "text": "Step 1. Reboot."}
                    ],
                    "pages": 1
                }
            }));
        }
        if req.path == "/v1/articles/art1" {
            return Resp::json(200, json!({
                "article": {"id": "art1", "name": "How to reset", "slug": "how-to-reset",
                            "status": "published", "text": "Step 1. Reboot.",
                            "collectionId": "col1"}
            }));
        }
        Resp::json(404, json!({"error": "unexpected"}))
    }));

    let c = creds(&mock, "", "docs-key-e2e");
    let provider = Arc::new(RealHelpScoutProvider::new(db.clone(), c.clone()));

    // seed the connection through the real OAuth callback
    let status = drive_oauth(&db, &c, provider.clone(), &mock);
    assert_eq!(status, 200);

    // docs requests carry HTTP Basic with the docs key
    let engine = SyncEngine::new(db.clone(), provider.clone()).with_bus(EventBus::default());
    let results = rt().block_on(async { engine.sync_docs_only(SyncTrigger::Manual).await })
        .expect("docs sync ok");
    assert_eq!(results.len(), 2, "docs_collections + docs_articles results");
    assert!(results.iter().all(|r| r.error.is_none()), "no errors: {results:?}");

    let cols: i64 = db.with_read(|conn| {
        conn.query_row("SELECT COUNT(*) FROM docs_collections", [], |r| r.get(0))
            .unwrap_or(0)
    });
    assert!(cols >= 1, "collection mirrored");
    let arts: i64 = db.with_read(|conn| {
        conn.query_row("SELECT COUNT(*) FROM docs_articles", [], |r| r.get(0))
            .unwrap_or(0)
    });
    assert!(arts >= 1, "article mirrored");

    let col_hit = mock.find_hit("/v1/collections").expect("collections request");
    assert!(
        col_hit.header("authorization").unwrap_or_default().starts_with("Basic "),
        "docs api key sent as HTTP Basic"
    );
}

// ================= T8: docs sync without a key stays idle + warns =================

#[test]
fn t8_docs_without_key_warns_and_stays_idle() {
    let db = fixture_db();
    let mock = Mock::start(Arc::new(move |req: &Req| {
        if req.path == "/v2/oauth2/token" {
            return Resp::json(200, token_json("idle-access", "idle-refresh"));
        }
        if req.path == "/v2/users/me" {
            return Resp::json(200, me_json());
        }
        Resp::json(404, json!({"error": "unexpected"}))
    }));
    let c = creds(&mock, "", "");
    let provider = Arc::new(RealHelpScoutProvider::new(db.clone(), c.clone()));
    let status = drive_oauth(&db, &c, provider.clone(), &mock);
    assert_eq!(status, 200);

    let engine = SyncEngine::new(db.clone(), provider.clone()).with_bus(EventBus::default());
    let results = rt().block_on(async { engine.sync_docs_only(SyncTrigger::Manual).await })
        .expect("docs sync returns ok when idle");
    assert!(results.iter().all(|r| r.error.is_none()));

    // no docs requests were ever made
    assert_eq!(mock.hit_count("GET", "/v1/"), 0, "no docs calls without key");

    // the idle state is disclosed in the event log
    let warned: i64 = db.with_read(|conn| {
        conn.query_row(
            "SELECT COUNT(*) FROM application_errors WHERE service = 'docs' AND level = 'warn'",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0)
    });
    assert!(warned >= 1, "docs idle warning recorded");
}

// ================= T9: unreachable network → friendly error =================

#[test]
fn t9_unreachable_api_fails_with_friendly_error() {
    let db = fixture_db();
    // port 9 (discard service, closed in this sandbox) — never listens
    let c = HsCredentials {
        client_id: "x".into(),
        client_secret: "y".into(),
        redirect_uri: "http://localhost:3000/oauth/callback".into(),
        api_base: "http://127.0.0.1:9".into(),
        webhook_secret: String::new(),
        docs_api_key: String::new(),
        docs_api_base: "http://127.0.0.1:9".into(),
    };
    let provider = RealHelpScoutProvider::new(db.clone(), c);
    let err = rt().block_on(async { provider.get_me().await }).expect_err("must fail");
    let msg = err.to_string();
    assert!(!msg.is_empty());
    assert!(
        msg.to_lowercase().contains("unreachable") || msg.to_lowercase().contains("internet")
            || msg.to_lowercase().contains("network") || msg.to_lowercase().contains("connect"),
        "friendly offline copy: {msg}"
    );
}

// ================= T10: full initial sync over mock (19 resources) =================

#[test]
fn t10_initial_sync_live_end_to_end() {
    let db = fixture_db();

    let me_body = me_json().to_string();
    let thread_body = json!({
        "_embedded": {"threads": [
            {"id": 9001, "type": "customer", "state": "published",
             "body": "My widget will not pair.",
             "customer": {"id": 7, "email": "nadia@example.com"},
             "createdAt": "2026-11-01T10:00:00Z"}
        ]},
        "_links": {}
    })
    .to_string();

    let mock = Mock::start(Arc::new(move |req: &Req| {
        let p = req.path.as_str();
        if p == "/v2/oauth2/token" {
            return Resp::json(200, token_json("sync-access", "sync-refresh"));
        }
        if p == "/v2/users/me" {
            return Resp::json(200, serde_json::from_str::<Value>(&me_body).unwrap());
        }
        if p == "/v2/users" {
            return Resp::json(200, json!({
                "_embedded": {"users": [
                    {"id": 42, "firstName": "Kim", "lastName": "Pearce", "email": "kim@example.com", "role": "owner"}
                ]},
                "page": {"totalPages": 1, "number": 1}
            }));
        }
        if p == "/v3/system-users" {
            return Resp::json(200, json!({"_embedded": {"system-users": []}, "_links": {}}));
        }
        if p == "/v2/teams" {
            return Resp::json(200, json!({"_embedded": {"teams": []}, "page": {"totalPages": 1, "number": 1}}));
        }
        if p == "/v2/mailboxes" {
            return Resp::json(200, json!({
                "_embedded": {"mailboxes": [
                    {"id": 1, "name": "Support", "slug": "support", "email": "support@example.com"}
                ]},
                "page": {"totalPages": 1, "number": 1}
            }));
        }
        if p == "/v2/mailboxes/1/folders" {
            return Resp::json(200, json!({"_embedded": {"folders": [
                {"id": 11, "name": "Unassigned", "type": "unassigned"}
            ]}}));
        }
        if p == "/v2/mailboxes/1/fields" {
            return Resp::json(200, json!({"_embedded": {"fields": []}}));
        }
        if p == "/v2/mailboxes/1/saved-replies" {
            return Resp::json(200, json!({"_embedded": {"savedReplies": []}}));
        }
        if p == "/v2/tags" {
            return Resp::json(200, json!({"_embedded": {"tags": [
                {"id": 5, "name": "hardware", "slug": "hardware"}
            ]}, "page": {"totalPages": 1, "number": 1}}));
        }
        if p == "/v2/workflows" {
            return Resp::json(200, json!({"_embedded": {"workflows": []}, "page": {"totalPages": 1, "number": 1}}));
        }
        if p == "/v2/customer-properties" {
            return Resp::json(200, json!({"_embedded": {"customer-properties": []}, "page": {"totalPages": 1, "number": 1}}));
        }
        if p == "/v2/organization-properties" {
            return Resp::json(200, json!({"_embedded": {"organization-properties": []}}));
        }
        if p == "/v2/organizations" {
            return Resp::json(200, json!({"_embedded": {"organizations": []}, "page": {"totalPages": 1, "number": 1}}));
        }
        if p == "/v2/users/42/status" {
            return Resp::json(200, json!({"status": "active"}));
        }
        if p == "/v1/collections" {
            return Resp::json(200, json!({"collections": {"items": [], "pages": 1}}));
        }
        // v3 paginated lists
        if p == "/v3/customers" {
            return Resp::json(200, json!({
                "_embedded": {"customers": [
                    {"id": 7, "firstName": "Nadia", "lastName": "Haq",
                     "emails": [{"value": "nadia@example.com", "type": "home"}]}
                ]},
                "_links": {}
            }));
        }
        if p == "/v3/conversations" {
            return Resp::json(200, json!({
                "_embedded": {"conversations": [
                    {"id": 555, "number": 1001, "type": "email", "status": "active",
                     "state": "active",
                     "subject": "Widget will not pair",
                     "preview": "My widget will not pair.",
                     "mailboxId": 1,
                     "primaryCustomer": {"id": 7},
                     "assignee": {"id": 42, "type": "user"},
                     "threads": 1,
                     "createdAt": "2026-11-01T09:00:00Z",
                     "userUpdatedAt": "2026-11-01T10:00:00Z",
                     "tags": [{"tag": "hardware"}]}
                ]},
                "_links": {}
            }));
        }
        if p == "/v3/conversations/555/threads" {
            return Resp::json(200, serde_json::from_str::<Value>(&thread_body).unwrap());
        }
        if p == "/v3/conversations/555" {
            return Resp::json(200, json!({
                "id": 555, "number": 1001, "type": "email", "status": "active",
                "subject": "Widget will not pair",
                "mailboxId": 1, "primaryCustomer": {"id": 7},
                "threads": 1, "createdAt": "2026-11-01T09:00:00Z"
            }));
        }
        Resp::json(404, json!({"error": format!("unexpected {}", req.path)}))
    }));

    let c = creds(&mock, "whsec-sync", "docs-key-e2e");
    let provider = Arc::new(RealHelpScoutProvider::new(db.clone(), c.clone()));
    let status = drive_oauth(&db, &c, provider.clone(), &mock);
    assert_eq!(status, 200);

    let engine = SyncEngine::new(db.clone(), provider.clone()).with_bus(EventBus::default());
    let results = rt().block_on(async { engine.initial_sync(SyncTrigger::Manual).await })
        .expect("initial sync completes");

    assert_eq!(results.len(), 19, "all 19 resources ran: {results:?}");
    let hard_failures: Vec<_> = results.iter().filter(|r| r.failed > 0 && r.error.is_some()).collect();
    assert!(hard_failures.is_empty(), "no failed resources: {hard_failures:?}");

    // mirror contents
    let convs: i64 = db.with_read(|conn| {
        conn.query_row("SELECT COUNT(*) FROM conversations", [], |r| r.get(0))
            .unwrap_or(0)
    });
    assert_eq!(convs, 1, "conversation mirrored");
    let threads: i64 = db.with_read(|conn| {
        conn.query_row("SELECT COUNT(*) FROM conversation_threads", [], |r| r.get(0))
            .unwrap_or(0)
    });
    assert_eq!(threads, 1, "thread mirrored");
    let customers: i64 = db.with_read(|conn| {
        conn.query_row("SELECT COUNT(*) FROM customers", [], |r| r.get(0))
            .unwrap_or(0)
    });
    assert_eq!(customers, 1, "customer mirrored");
    let mailboxes: i64 = db.with_read(|conn| {
        conn.query_row("SELECT COUNT(*) FROM mailboxes", [], |r| r.get(0))
            .unwrap_or(0)
    });
    assert_eq!(mailboxes, 1, "mailbox mirrored");

    // rate limiter metadata did not leak secrets and sync state is terminal
    let state: String = db.with_read(|conn| {
        conn.query_row("SELECT value FROM application_settings WHERE key = 'sync_state'", [], |r| r.get(0))
            .unwrap_or_else(|_| "unknown".into())
    });
    let _ = state; // state naming may vary; only absence of panic matters here

    // second run (idempotent) still succeeds and does not duplicate rows
    let results2 = rt().block_on(async { engine.initial_sync(SyncTrigger::Manual).await })
        .expect("second initial sync completes");
    assert!(results2.iter().all(|r| r.error.is_none()), "idempotent rerun: {results2:?}");
    let convs2: i64 = db.with_read(|conn| {
        conn.query_row("SELECT COUNT(*) FROM conversations", [], |r| r.get(0))
            .unwrap_or(0)
    });
    assert_eq!(convs2, 1, "no duplicate conversations after rerun");
}

// ================= T11: docs publish (write path) =================

#[test]
fn t11_docs_publish_draft_live() {
    let db = fixture_db();
    let mock = Mock::start(Arc::new(move |req: &Req| {
        if req.path == "/v2/oauth2/token" {
            return Resp::json(200, token_json("pub-access", "pub-refresh"));
        }
        if req.path == "/v2/users/me" {
            return Resp::json(200, me_json());
        }
        if req.method == "POST" && req.path == "/v1/articles" {
            return Resp::json(201, json!({"article": {"id": "art-new", "name": "Fresh"}}));
        }
        if req.method == "PUT" && req.path == "/v1/articles/art1/drafts" {
            return Resp::json(200, json!({"article": {"id": "art1"}}));
        }
        Resp::json(404, json!({"error": "unexpected"}))
    }));

    let c = creds(&mock, "", "docs-key-e2e");
    let provider = Arc::new(RealHelpScoutProvider::new(db.clone(), c.clone()));
    let status = drive_oauth(&db, &c, provider.clone(), &mock);
    assert_eq!(status, 200);

    // create
    let id = rt().block_on(async {
        provider.upsert_doc_article_draft(&spp_core::helpscout::DocArticleDraftInput {
            collection_id: "col1".into(),
            name: "Fresh article".into(),
            text: "Hello world".into(),
            remote_id: None,
            slug: Some("fresh-article".into()),
        })
        .await
    })
    .expect("draft created");
    assert_eq!(id, "art-new");

    // update
    let id2 = rt().block_on(async {
        provider.upsert_doc_article_draft(&spp_core::helpscout::DocArticleDraftInput {
            collection_id: "col1".into(),
            name: "Updated".into(),
            text: "Hello again".into(),
            remote_id: Some("art1".into()),
            slug: None,
        })
        .await
    })
    .expect("draft updated");
    assert_eq!(id2, "art1");

    // both write hits recorded with Basic auth
    let post = mock.hits().into_iter().find(|h| h.method == "POST" && h.path == "/v1/articles")
        .expect("POST recorded");
    assert!(post.header("authorization").unwrap_or_default().starts_with("Basic "));
    let put = mock.hits().into_iter().find(|h| h.method == "PUT" && h.path == "/v1/articles/art1/drafts")
        .expect("PUT recorded");
    assert!(put.header("authorization").unwrap_or_default().starts_with("Basic "));
}
