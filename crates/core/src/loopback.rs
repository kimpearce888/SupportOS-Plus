//! Minimal loopback HTTP listener — webhook ingress and the OAuth callback
//! only (migration-map row: "minimal loopback listener").
//!
//! After the Phase 4 cutover the full axum router is gone; the shell still
//! needs two inbound HTTP endpoints that cannot exist inside the egui
//! process:
//!
//! * `POST /webhook` — Help Scout event ingress. Signature verification,
//!   persistence, deduplication and job enqueueing are the exact
//!   [`crate::webhook_handler::process_webhook`] pipeline the axum route
//!   used, with the same status-code mapping.
//! * `GET /oauth/callback` — the browser lands here after the Help Scout
//!   authorization screen. The `state` parameter is validated against the
//!   single-use [`crate::oauth_state`] table, the code is exchanged, and
//!   the result is a tiny human-readable HTML page (the browser cannot
//!   render egui).
//!
//! The listener binds `127.0.0.1` only, speaks just enough HTTP/1.1 for
//! those two routes, and rejects everything else. Requests are capped in
//! size so a stray local process cannot balloon memory.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use rusqlite::Connection;

use crate::db::DbHandle;
use crate::error::Result;
use crate::helpscout_real::{HsCredentials, RealHelpScoutProvider};
use crate::webhook_handler::{self, WebhookProcessResult};

const MAX_HEADER_BYTES: usize = 16 * 1024;
const MAX_BODY_BYTES: usize = 1024 * 1024;
const MAX_HEADERS: usize = 64;
const READ_TIMEOUT: Duration = Duration::from_secs(10);

/// Parsed inbound request (just enough HTTP/1.1 for the two routes).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    pub method: String,
    /// Path without the query string, e.g. `/webhook`.
    pub path: String,
    /// Raw query string (no leading `?`), possibly empty.
    pub query: String,
    /// Lower-cased header names with their values.
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Request {
    pub fn header(&self, name: &str) -> Option<&str> {
        let name = name.to_ascii_lowercase();
        self.headers
            .iter()
            .find(|(k, _)| *k == name)
            .map(|(_, v)| v.as_str())
    }
}

/// Outbound response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Response {
    pub status: u16,
    pub reason: &'static str,
    pub content_type: &'static str,
    pub body: Vec<u8>,
}

impl Response {
    fn json(status: u16, reason: &'static str, body: String) -> Self {
        Self {
            status,
            reason,
            content_type: "application/json",
            body: body.into_bytes(),
        }
    }

    fn html(status: u16, body: String) -> Self {
        Self {
            status,
            reason: "OK",
            content_type: "text/html; charset=utf-8",
            body: body.into_bytes(),
        }
    }

    fn text(status: u16, reason: &'static str, body: &str) -> Self {
        Self {
            status,
            reason,
            content_type: "text/plain; charset=utf-8",
            body: body.as_bytes().to_vec(),
        }
    }

    fn serialize(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.body.len() + 128);
        let reason = if self.reason.is_empty() {
            "OK"
        } else {
            self.reason
        };
        out.extend_from_slice(format!("HTTP/1.1 {} {}\r\n", self.status, reason).as_bytes());
        out.extend_from_slice(format!("Content-Type: {}\r\n", self.content_type).as_bytes());
        out.extend_from_slice(format!("Content-Length: {}\r\n", self.body.len()).as_bytes());
        out.extend_from_slice(b"Connection: close\r\n\r\n");
        out.extend_from_slice(&self.body);
        out
    }
}

/// Everything the request handler needs.
pub struct LoopbackCtx {
    pub db: DbHandle,
    /// OAuth completion path; `None` in demo mode.
    pub oauth: Option<Arc<dyn OAuthCompletion>>,
    pub demo_mode: bool,
}

/// Identity returned by a successful OAuth code exchange.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectedIdentity {
    pub name: String,
    pub email: Option<String>,
    pub remote_id: i64,
}

/// The OAuth half of the callback flow — exchange the code and fetch the
/// connected user — behind a trait so the HTTP layer is unit-testable
/// without Help Scout on the other end.
pub trait OAuthCompletion: Send + Sync {
    fn complete(&self, code: &str) -> std::result::Result<ConnectedIdentity, String>;
}

/// Production [`OAuthCompletion`] over the real provider: runs the two
/// async calls on a throwaway current-thread runtime (the listener thread
/// owns no reactor).
struct RealOAuthCompletion {
    provider: Arc<RealHelpScoutProvider>,
}

impl OAuthCompletion for RealOAuthCompletion {
    fn complete(&self, code: &str) -> std::result::Result<ConnectedIdentity, String> {
        let provider = Arc::clone(&self.provider);
        let code = code.to_string();
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| format!("failed to build a runtime for the code exchange: {e}"))?
            .block_on(async {
                use crate::helpscout::HelpScoutProvider;
                provider
                    .exchange_code(&code)
                    .await
                    .map_err(|e| format!("exchanging the authorization code failed: {e}"))?;
                let me = provider
                    .get_me()
                    .await
                    .map_err(|e| format!("fetching the connected user failed: {e}"))?;
                Ok(ConnectedIdentity {
                    name: format!(
                        "{} {}",
                        me.first_name.unwrap_or_default(),
                        me.last_name.unwrap_or_default()
                    )
                    .trim()
                    .to_string(),
                    email: me.email,
                    remote_id: me.remote_id,
                })
            })
    }
}

/// Issue a fresh single-use OAuth state and return the full authorization
/// URL the user should open in a browser.
///
/// The state is recorded in the [`crate::oauth_state`] table and consumed
/// exactly once by [`handle_callback`].
pub fn begin_oauth(
    db: &DbHandle,
    credentials: &HsCredentials,
    redirect_uri: &str,
) -> Result<String> {
    let state = db.with_write(|conn| crate::oauth_state::issue_state(conn, redirect_uri, None))?;
    Ok(credentials.authorize_url(&state))
}

/// Derive the listener port from a `http://localhost:PORT/...` or
/// `http://127.0.0.1:PORT/...` redirect URI. Returns `None` for anything
/// else (remote hosts, https, missing port).
#[must_use]
pub fn port_from_redirect_uri(redirect_uri: &str) -> Option<u16> {
    let url = url::Url::parse(redirect_uri).ok()?;
    match url.host_str()? {
        "localhost" | "127.0.0.1" => url.port(),
        _ => None,
    }
}

/// Dispatch one parsed request. Pure apart from the database and the
/// optional OAuth completion — fully unit-testable.
pub fn handle_request(req: &Request, ctx: &LoopbackCtx) -> Response {
    match (req.method.as_str(), req.path.as_str()) {
        ("POST", "/webhook") => handle_webhook(req, ctx),
        ("GET", "/oauth/callback") => handle_callback(req, ctx),
        ("GET", "/healthz") => {
            Response::json(200, "OK", "{\"ok\":true,\"service\":\"supportos-loopback\"}".into())
        }
        ("GET", "/") => Response::json(
            200,
            "OK",
            "{\"service\":\"supportos-loopback\",\"routes\":[\"POST /webhook\",\"GET /oauth/callback\"]}"
                .into(),
        ),
        (method, path) => {
            if matches!(path, "/webhook" | "/oauth/callback") {
                Response::text(405, "Method Not Allowed", "method not allowed for this route\n")
            } else {
                let _ = method;
                Response::text(404, "Not Found", "not found\n")
            }
        }
    }
}

/// Webhook ingress: resolve the secret (env override first, then the
/// encrypted envelope), verify, persist, and map the outcome onto the same
/// status codes the axum route used.
fn handle_webhook(req: &Request, ctx: &LoopbackCtx) -> Response {
    // Secret resolution order mirrors the old HTTP route: env override
    // first (CI / tests), then the encrypted envelope. An empty result is
    // handled inside process_webhook: outside demo mode it is a hard
    // reject (migration-map row 1).
    let secret = std::env::var("HELPSCOUT_WEBHOOK_SECRET")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| {
            ctx.db.with_read(|conn| {
                crate::settings::get_secret(conn, "webhook_secret")
                    .ok()
                    .flatten()
                    .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
                    .filter(|s| !s.is_empty())
                    .unwrap_or_default()
            })
        });

    let signature = req.header("x-helpscout-signature");
    let event_type = req.header("x-helpscout-event").unwrap_or("unknown");

    let result = ctx.db.with_write(|conn| {
        webhook_handler::process_webhook(
            conn,
            secret.as_bytes(),
            &req.body,
            signature,
            event_type,
            ctx.demo_mode,
        )
    });

    match result {
        WebhookProcessResult::Accepted { .. } => {
            Response::json(200, "OK", "{\"received\":true}".into())
        }
        WebhookProcessResult::Duplicate { .. } => {
            Response::json(200, "OK", "{\"received\":true,\"duplicate\":true}".into())
        }
        WebhookProcessResult::SignatureInvalid => Response::json(
            401,
            "Unauthorized",
            "{\"error\":\"Invalid webhook signature\"}".into(),
        ),
        WebhookProcessResult::SecretNotConfigured => Response::json(
            403,
            "Forbidden",
            "{\"error\":\"Webhook secret not configured\",\"message\":\"Unsigned webhook events \
             are rejected because no webhook secret is configured. Set a secret in settings or \
             via HELPSCOUT_WEBHOOK_SECRET, or enable demo mode.\"}"
                .into(),
        ),
        WebhookProcessResult::BadRequest => Response::json(
            400,
            "Bad Request",
            "{\"statusCode\":400,\"error\":\"Bad Request\",\"message\":\"Request body is not \
             valid JSON.\"}"
                .into(),
        ),
    }
}

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

const PAGE_STYLE: &str = "body{font-family:system-ui,sans-serif;max-width:36rem;margin:4rem \
                          auto;padding:0 1rem;color:#1f2933}h1{font-size:1.2rem}p{line-height:1.5;\
                          color:#52606d}";

fn page(title: &str, body_inner: &str) -> String {
    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>{title}</title>\
         <style>{PAGE_STYLE}</style></head><body>{body_inner}</body></html>"
    )
}

fn fail_page(message: &str) -> Response {
    Response::html(
        200,
        page(
            "SupportOS - connection not completed",
            &format!(
                "<h1>Help Scout connection not completed</h1><p>{message}</p>\
                 <p>Return to SupportOS Settings and try again, or use \"Connect with Client \
                 Credentials\".</p>"
            ),
        ),
    )
}

/// OAuth callback: validate the single-use state, exchange the code, show
/// a result page. Semantics ported from the old axum route, with the state
/// moving from the single-slot `application_settings` hack to the
/// dedicated single-use `oauth_states` table.
fn handle_callback(req: &Request, ctx: &LoopbackCtx) -> Response {
    let query: Vec<(String, String)> = url::form_urlencoded::parse(req.query.as_bytes())
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();
    let q = |key: &str| {
        query
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    };

    if let Some(err) = q("error") {
        let detail = q("error_description")
            .map(|d| format!(" - {d}"))
            .unwrap_or_default();
        return fail_page(&format!(
            "Help Scout returned an error: {}{}",
            escape_html(err),
            escape_html(&detail)
        ));
    }

    let Some(oauth) = &ctx.oauth else {
        return fail_page("Demo mode is active - OAuth is not needed.");
    };
    let (Some(code), Some(state_param)) = (q("code"), q("state")) else {
        return fail_page("The callback is missing its authorization code or state parameter.");
    };

    let state_ok = ctx
        .db
        .with_write(|conn| crate::oauth_state::consume_state(conn, state_param))
        .is_ok();
    if !state_ok {
        return fail_page(
            "The state parameter did not match the authorization request (it may have expired \
             or been reused). For safety the code was not exchanged.",
        );
    }

    match oauth.complete(code) {
        Ok(identity) => {
            ctx.db.with_write(|conn| {
                store_me_remote_id(conn, identity.remote_id);
                let _ = crate::jobs::audit(
                    conn,
                    "user",
                    "helpscout_connected",
                    None,
                    None,
                    None,
                    Some("GET /oauth/callback (code exchange)"),
                    None,
                    false,
                );
            });
            let email_part = identity
                .email
                .as_deref()
                .filter(|e| !e.is_empty())
                .map(|e| format!(" ({})", escape_html(e)))
                .unwrap_or_default();
            Response::html(
                200,
                page(
                    "SupportOS - connected",
                    &format!(
                        "<h1>Connected to Help Scout</h1><p>Connected as {}{}.</p>\
                         <p>You can close this tab and return to SupportOS. Reload the Settings \
                         page to see the connection status.</p>",
                        escape_html(&identity.name),
                        email_part
                    ),
                ),
            )
        }
        Err(e) => fail_page(&escape_html(&e)),
    }
}

fn store_me_remote_id(conn: &Connection, remote_id: i64) {
    let _ = conn.execute(
        "INSERT INTO application_settings (key, value, updated_at)
         VALUES ('me_remote_id', ?1, datetime('now'))
         ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
        rusqlite::params![serde_json::to_string(&remote_id).unwrap_or_default()],
    );
}

/// Running loopback listener. Dropping it stops the accept thread.
pub struct LoopbackServer {
    port: u16,
    shutdown: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl LoopbackServer {
    /// Bind `127.0.0.1:port` (port 0 = OS-assigned; see [`Self::port`])
    /// and start serving. Only the two documented routes exist; everything
    /// else is rejected.
    pub fn start(
        db: DbHandle,
        real: Option<Arc<RealHelpScoutProvider>>,
        demo_mode: bool,
        port: u16,
    ) -> std::io::Result<Self> {
        let listener = TcpListener::bind(("127.0.0.1", port))?;
        let bound_port = listener.local_addr()?.port();
        let shutdown = Arc::new(AtomicBool::new(false));

        let ctx = Arc::new(LoopbackCtx {
            db,
            oauth: real.map(|provider| {
                Arc::new(RealOAuthCompletion { provider }) as Arc<dyn OAuthCompletion>
            }),
            demo_mode,
        });

        let thread_shutdown = Arc::clone(&shutdown);
        let thread = std::thread::Builder::new()
            .name("spp-loopback".to_string())
            .spawn(move || serve(listener, ctx, thread_shutdown))?;

        Ok(Self {
            port: bound_port,
            shutdown,
            thread: Some(thread),
        })
    }

    /// The bound port (useful when started on an OS-assigned port).
    #[must_use]
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Base URL of the listener, e.g. `http://127.0.0.1:39877/`.
    #[must_use]
    pub fn base_url(&self) -> String {
        format!("http://127.0.0.1:{}/", self.port)
    }

    /// Stop the accept thread. Idempotent; also runs on drop.
    pub fn stop(&mut self) {
        self.shutdown.store(true, Ordering::SeqCst);
        // Unblocks the accept() call: a connect-and-drop is the portable
        // way without platform-specific socket APIs.
        if self.port > 0 {
            let _ = std::net::TcpStream::connect(("127.0.0.1", self.port));
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for LoopbackServer {
    fn drop(&mut self) {
        self.stop();
    }
}

fn serve(listener: TcpListener, ctx: Arc<LoopbackCtx>, shutdown: Arc<AtomicBool>) {
    while !shutdown.load(Ordering::SeqCst) {
        match listener.accept() {
            Ok((stream, _addr)) => {
                if shutdown.load(Ordering::SeqCst) {
                    break;
                }
                // One thread per connection: the OAuth code exchange can
                // take seconds, and webhook bursts must not queue behind
                // it. On thread-spawn failure (resource exhaustion) the
                // connection is dropped — the webhook sender retries, and
                // the browser shows its own error.
                let spawned = std::thread::Builder::new()
                    .name("spp-loopback-conn".to_string())
                    .spawn({
                        let ctx = Arc::clone(&ctx);
                        move || handle_conn(stream, ctx)
                    });
                if spawned.is_err() {
                    tracing::warn!(
                        "loopback: per-connection thread spawn failed; connection dropped"
                    );
                }
            }
            Err(e) => {
                if shutdown.load(Ordering::SeqCst) {
                    break;
                }
                tracing::warn!(error = %e, "loopback: accept failed");
                std::thread::sleep(Duration::from_millis(50));
            }
        }
    }
}

fn handle_conn(stream: TcpStream, ctx: Arc<LoopbackCtx>) {
    let mut stream = stream;
    if let Err(e) = handle_connection(&mut stream, &ctx) {
        tracing::debug!(error = %e, "loopback: connection ended with an IO error");
    }
}

fn handle_connection(stream: &mut TcpStream, ctx: &LoopbackCtx) -> std::io::Result<()> {
    let _ = stream.set_read_timeout(Some(READ_TIMEOUT));
    let _ = stream.set_write_timeout(Some(READ_TIMEOUT));
    let req = match read_request(stream)? {
        Some(req) => req,
        None => return Ok(()), // closed before sending anything
    };
    let response = handle_request(&req, ctx);
    stream.write_all(&response.serialize())?;
    stream.flush()?;
    let _ = stream.shutdown(std::net::Shutdown::Write);
    Ok(())
}

/// Read and parse one HTTP/1.1 request. `Ok(None)` = peer closed cleanly
/// before the header block. Oversized or malformed input is an error; the
/// caller drops the connection.
fn read_request(stream: &mut TcpStream) -> std::io::Result<Option<Request>> {
    let mut buf: Vec<u8> = Vec::with_capacity(1024);
    let mut chunk = [0u8; 2048];
    let header_end = loop {
        if let Some(pos) = find_header_end(&buf) {
            break pos;
        }
        if buf.len() > MAX_HEADER_BYTES {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "header block exceeds 16 KiB",
            ));
        }
        let n = stream.read(&mut chunk)?;
        if n == 0 {
            if buf.is_empty() {
                return Ok(None);
            }
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "connection closed mid-header",
            ));
        }
        buf.extend_from_slice(&chunk[..n]);
    };

    let header_block = String::from_utf8_lossy(&buf[..header_end]).into_owned();
    let mut lines = header_block.split("\r\n");
    let request_line = lines.next().unwrap_or_default().to_string();
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or_default().to_string();
    let target = parts.next().unwrap_or_default().to_string();
    if method.is_empty() || target.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "malformed request line",
        ));
    }

    let mut headers = Vec::new();
    for line in lines {
        if line.is_empty() {
            continue;
        }
        if headers.len() >= MAX_HEADERS {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "too many headers",
            ));
        }
        let Some((name, value)) = line.split_once(':') else {
            continue; // tolerate junk lines, like most minimal servers
        };
        headers.push((name.trim().to_ascii_lowercase(), value.trim().to_string()));
    }

    let content_length = headers
        .iter()
        .find(|(k, _)| k == "content-length")
        .and_then(|(_, v)| v.parse::<usize>().ok())
        .unwrap_or(0);
    if content_length > MAX_BODY_BYTES {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "body exceeds 1 MiB",
        ));
    }

    let mut body = buf[header_end + 4..].to_vec();
    if body.len() > content_length {
        body.truncate(content_length);
    }
    while body.len() < content_length {
        let n = stream.read(&mut chunk)?;
        if n == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "connection closed mid-body",
            ));
        }
        let need = content_length - body.len();
        body.extend_from_slice(&chunk[..n.min(need)]);
    }

    let (path, query) = match target.split_once('?') {
        Some((p, q)) => (p.to_string(), q.to_string()),
        None => (target, String::new()),
    };

    Ok(Some(Request {
        method,
        path,
        query,
        headers,
        body,
    }))
}

fn find_header_end(buf: &[u8]) -> Option<usize> {
    buf.windows(4).position(|w| w == b"\r\n\r\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_db() -> DbHandle {
        let mut conn = Connection::open_in_memory().expect("in-memory connection");
        crate::bootstrap::apply_all(&mut conn).expect("migrations");
        DbHandle::new(conn)
    }

    /// Install the secrets master key for tests that store/read a webhook
    /// secret (in-memory databases have no key file to anchor one next to).
    fn install_master_key() {
        let key_path =
            std::env::temp_dir().join(format!("spp-loopback-test-key-{}.key", std::process::id()));
        crate::settings::init_master_key(&key_path).expect("init master key");
    }

    fn request(method: &str, target: &str, headers: &[(&str, &str)], body: &[u8]) -> Request {
        Request {
            method: method.to_string(),
            path: target.split('?').next().unwrap_or_default().to_string(),
            query: target
                .split_once('?')
                .map(|(_, q)| q.to_string())
                .unwrap_or_default(),
            headers: headers
                .iter()
                .map(|(k, v)| (k.to_ascii_lowercase(), v.to_string()))
                .collect(),
            body: body.to_vec(),
        }
    }

    /// Canned OAuth completion for tests.
    struct FakeOAuth {
        result: Option<ConnectedIdentity>,
    }

    impl OAuthCompletion for FakeOAuth {
        fn complete(&self, _code: &str) -> std::result::Result<ConnectedIdentity, String> {
            self.result
                .clone()
                .ok_or_else(|| "exchange failed".to_string())
        }
    }

    fn ctx(oauth: Option<FakeOAuth>) -> LoopbackCtx {
        LoopbackCtx {
            db: fixture_db(),
            oauth: oauth.map(|f| Arc::new(f) as Arc<dyn OAuthCompletion>),
            demo_mode: false,
        }
    }

    fn body_str(r: &Response) -> String {
        String::from_utf8_lossy(&r.body).into_owned()
    }

    // --- routing ---

    #[test]
    fn unknown_paths_and_methods_are_rejected() {
        let c = ctx(None);
        let r = handle_request(&request("GET", "/inbox", &[], b""), &c);
        assert_eq!(r.status, 404);

        let r = handle_request(&request("GET", "/webhook", &[], b""), &c);
        assert_eq!(r.status, 405);

        let r = handle_request(&request("POST", "/oauth/callback", &[], b""), &c);
        assert_eq!(r.status, 405);
    }

    #[test]
    fn healthz_and_root_respond() {
        let c = ctx(None);
        let r = handle_request(&request("GET", "/healthz", &[], b""), &c);
        assert_eq!(r.status, 200);
        assert!(body_str(&r).contains("supportos-loopback"));

        let r = handle_request(&request("GET", "/", &[], b""), &c);
        assert_eq!(r.status, 200);
        assert!(body_str(&r).contains("/webhook"));
    }

    // --- webhook route ---

    #[test]
    fn webhook_maps_result_status_codes() {
        let c = ctx(None);
        install_master_key();
        c.db.with_write(|conn| crate::settings::set_secret(conn, "webhook_secret", b"top-secret"))
            .expect("store webhook secret");
        let secret = b"top-secret";
        let body = br#"{"objectID":123}"#;
        let sig = crate::webhook::compute_signature(secret, body);

        let r = handle_request(
            &request(
                "POST",
                "/webhook",
                &[
                    ("X-HelpScout-Signature", &sig),
                    ("X-HelpScout-Event", "convo.created"),
                ],
                body,
            ),
            &c,
        );
        assert_eq!(r.status, 200);
        assert_eq!(body_str(&r), "{\"received\":true}");

        // Duplicate of the same body: still 200, flagged.
        let r = handle_request(
            &request(
                "POST",
                "/webhook",
                &[
                    ("X-HelpScout-Signature", &sig),
                    ("X-HelpScout-Event", "convo.created"),
                ],
                body,
            ),
            &c,
        );
        assert_eq!(r.status, 200);
        assert!(body_str(&r).contains("\"duplicate\":true"));

        let count: i64 =
            c.db.with_read(|conn| {
                conn.query_row(
                    "SELECT COUNT(*) FROM jobs WHERE type = 'sync_conversation'",
                    [],
                    |row| row.get(0),
                )
            })
            .expect("jobs readable");
        assert_eq!(count, 1, "duplicate must not enqueue twice");
    }

    #[test]
    fn webhook_rejects_bad_signature_and_empty_secret() {
        let c = ctx(None);
        install_master_key();
        c.db.with_write(|conn| crate::settings::set_secret(conn, "webhook_secret", b"secret"))
            .expect("store webhook secret");
        let r = handle_request(
            &request(
                "POST",
                "/webhook",
                &[
                    (
                        "X-HelpScout-Signature",
                        "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=",
                    ),
                    ("X-HelpScout-Event", "convo.created"),
                ],
                br#"{"id":1}"#,
            ),
            &c,
        );
        assert_eq!(r.status, 401);

        // No secret configured and not demo mode: hard reject (a fresh
        // database without the secret, same request shape).
        let c = ctx(None);
        let r = handle_request(
            &request(
                "POST",
                "/webhook",
                &[("X-HelpScout-Event", "convo.created")],
                br#"{"id":1}"#,
            ),
            &c,
        );
        assert_eq!(r.status, 403);
    }

    #[test]
    fn webhook_rejects_non_json_bodies() {
        let c = ctx(None);
        install_master_key();
        c.db.with_write(|conn| crate::settings::set_secret(conn, "webhook_secret", b"secret"))
            .expect("store webhook secret");
        let body = b"not-json";
        let sig = crate::webhook::compute_signature(b"secret", body);
        let r = handle_request(
            &request("POST", "/webhook", &[("X-HelpScout-Signature", &sig)], body),
            &c,
        );
        assert_eq!(r.status, 400);
    }

    // --- OAuth callback route ---

    #[test]
    fn callback_without_oauth_shows_demo_page() {
        let c = ctx(None);
        let r = handle_request(
            &request("GET", "/oauth/callback?code=1&state=2", &[], b""),
            &c,
        );
        assert_eq!(r.status, 200);
        assert!(body_str(&r).contains("Demo mode"));
    }

    #[test]
    fn callback_rejects_provider_error_param() {
        let c = ctx(None);
        let r = handle_request(
            &request(
                "GET",
                "/oauth/callback?error=access_denied&error_description=nope",
                &[],
                b"",
            ),
            &c,
        );
        assert_eq!(r.status, 200);
        assert!(body_str(&r).contains("access_denied"));
        assert!(body_str(&r).contains("nope"));
    }

    #[test]
    fn callback_rejects_missing_code_or_state() {
        let c = ctx(Some(FakeOAuth { result: None }));
        let r = handle_request(&request("GET", "/oauth/callback?code=1", &[], b""), &c);
        assert!(body_str(&r).contains("missing its authorization code"));

        let r = handle_request(&request("GET", "/oauth/callback?state=2", &[], b""), &c);
        assert!(body_str(&r).contains("missing its authorization code"));
    }

    #[test]
    fn callback_rejects_unknown_or_reused_state() {
        let c = ctx(Some(FakeOAuth {
            result: Some(ConnectedIdentity {
                name: "A B".into(),
                email: Some("a@b.c".into()),
                remote_id: 7,
            }),
        }));
        // No state was ever issued.
        let r = handle_request(
            &request("GET", "/oauth/callback?code=1&state=deadbeef", &[], b""),
            &c,
        );
        assert!(body_str(&r).contains("did not match"));

        // Issue a state, consume it once successfully, then replay it.
        let state = c
            .db
            .with_write(|conn| crate::oauth_state::issue_state(conn, "http://127.0.0.1:1/", None))
            .expect("issue state");
        let target = format!("/oauth/callback?code=1&state={state}");
        let ok = handle_request(&request("GET", &target, &[], b""), &c);
        assert!(body_str(&ok).contains("Connected to Help Scout"));

        let replay = handle_request(&request("GET", &target, &[], b""), &c);
        assert!(
            body_str(&replay).contains("did not match"),
            "state must be single-use"
        );
    }

    #[test]
    fn callback_success_stores_me_remote_id_and_audits() {
        let c = ctx(Some(FakeOAuth {
            result: Some(ConnectedIdentity {
                name: "Kim Pearce".into(),
                email: Some("kim@example.com".into()),
                remote_id: 42,
            }),
        }));
        let state = c
            .db
            .with_write(|conn| crate::oauth_state::issue_state(conn, "http://127.0.0.1:1/", None))
            .expect("issue state");
        let target = format!("/oauth/callback?code=CODE&state={state}");
        let r = handle_request(&request("GET", &target, &[], b""), &c);
        assert!(body_str(&r).contains("Kim Pearce"));
        assert!(body_str(&r).contains("kim@example.com"));

        let stored: Option<String> =
            c.db.with_read(|conn| {
                conn.query_row(
                    "SELECT value FROM application_settings WHERE key = 'me_remote_id'",
                    [],
                    |row| row.get(0),
                )
                .ok()
            })
            .expect("application_settings readable");
        assert_eq!(stored.as_deref(), Some("42"));

        let audits: i64 =
            c.db.with_read(|conn| {
                conn.query_row(
                    "SELECT COUNT(*) FROM audit_log WHERE action = 'helpscout_connected'",
                    [],
                    |row| row.get(0),
                )
                .or::<i64>(Ok(0))
            })
            .unwrap_or(0);
        assert!(audits >= 1, "connection must be audited");
    }

    #[test]
    fn callback_exchange_failure_shows_fail_page() {
        let c = ctx(Some(FakeOAuth { result: None }));
        let state = c
            .db
            .with_write(|conn| crate::oauth_state::issue_state(conn, "http://127.0.0.1:1/", None))
            .expect("issue state");
        let target = format!("/oauth/callback?code=BAD&state={state}");
        let r = handle_request(&request("GET", &target, &[], b""), &c);
        assert!(body_str(&r).contains("exchange failed"));
    }

    // --- helpers ---

    #[test]
    fn port_from_redirect_uri_extracts_loopback_ports() {
        assert_eq!(
            port_from_redirect_uri("http://localhost:3000/oauth/callback"),
            Some(3000)
        );
        assert_eq!(
            port_from_redirect_uri("http://127.0.0.1:39877/x"),
            Some(39877)
        );
        assert_eq!(
            port_from_redirect_uri("http://localhost/oauth/callback"),
            None
        );
        assert_eq!(port_from_redirect_uri("https://api.helpscout.net/cb"), None);
        assert_eq!(port_from_redirect_uri("not a url"), None);
    }

    #[test]
    fn begin_oauth_issues_a_url_backed_by_a_single_use_state() {
        let db = fixture_db();
        let creds = HsCredentials::default();
        let url =
            begin_oauth(&db, &creds, "http://127.0.0.1:39999/oauth/callback").expect("begin_oauth");
        assert!(url.contains("client_id="));
        let state = url.rsplit("state=").next().unwrap_or_default().to_string();
        assert_eq!(state.len(), 32, "16 random bytes hex-encoded");
        let consumed = db.with_write(|conn| crate::oauth_state::consume_state(conn, &state));
        assert!(consumed.is_ok());
    }
}
