use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use rusqlite::Connection;

use crate::db::DbHandle;
use crate::error::Result;
use crate::helpscout_real::HsCredentials;
use crate::provider_handle::ProviderHandle;
use crate::webhook_handler::{self, WebhookProcessResult};

static BOUND_PORT: OnceLock<u16> = OnceLock::new();

#[must_use]
pub fn bound_port() -> Option<u16> {
    BOUND_PORT.get().copied()
}

pub fn record_bound_port(port: u16) {
    let _ = BOUND_PORT.set(port);
}

const MAX_HEADER_BYTES: usize = 16 * 1024;
const MAX_BODY_BYTES: usize = 1024 * 1024;
const MAX_HEADERS: usize = 64;
const READ_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Request {
    pub method: String,
    pub path: String,
    pub query: String,
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

    fn html(status: u16, reason: &'static str, body: String) -> Self {
        Self {
            status,
            reason,
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

pub(crate) struct LoopbackCtx {
    pub db: DbHandle,
    pub port: u16,
    pub oauth: Option<Arc<dyn OAuthCompletion>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ConnectedIdentity {
    pub name: String,
    pub email: Option<String>,
    pub remote_id: i64,
}

pub(crate) trait OAuthCompletion: Send + Sync {
    fn complete(&self, code: &str) -> std::result::Result<ConnectedIdentity, String>;
}

struct RealOAuthCompletion {
    handle: Arc<ProviderHandle>,
}

impl OAuthCompletion for RealOAuthCompletion {
    fn complete(&self, code: &str) -> std::result::Result<ConnectedIdentity, String> {
        let Some(provider) = self.handle.real() else {
            return Err(
                "the authorization code cannot be exchanged: no Help Scout connection is \
                 active in this session. Save your client credentials in Settings first \
                 (they activate immediately, no restart needed)."
                    .to_string(),
            );
        };
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

pub fn begin_oauth(
    db: &DbHandle,
    credentials: &HsCredentials,
    redirect_uri: &str,
) -> Result<String> {
    let _ = db.with_write(|conn| crate::oauth_state::sweep_states(conn));
    let state = db.with_write(|conn| crate::oauth_state::issue_state(conn, redirect_uri, None))?;
    Ok(credentials.authorize_url(&state))
}

#[must_use]
pub fn port_from_redirect_uri(redirect_uri: &str) -> Option<u16> {
    let url = url::Url::parse(redirect_uri).ok()?;
    match url.host_str()? {
        "localhost" | "127.0.0.1" => url.port(),
        _ => None,
    }
}

#[must_use]
pub fn listener_answers(port: u16) -> bool {
    let Ok(mut stream) = TcpStream::connect(("127.0.0.1", port)) else {
        return false;
    };
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));
    if stream
        .write_all(b"GET /healthz HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
        .is_err()
    {
        return false;
    }
    let mut buf = [0u8; 512];
    let Ok(n) = stream.read(&mut buf) else {
        return false;
    };
    String::from_utf8_lossy(&buf[..n]).contains("supportos-loopback")
}

pub(crate) fn handle_request(req: &Request, ctx: &LoopbackCtx) -> Response {
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

fn handle_webhook(req: &Request, ctx: &LoopbackCtx) -> Response {
    let secret = std::env::var("HELPSCOUT_WEBHOOK_SECRET")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| {
            ctx.db.with_read(|conn| {
                match crate::settings::get_secret(conn, "webhook_secret") {
                    Ok(bytes) => bytes
                        .map(|b| String::from_utf8_lossy(&b).into_owned())
                        .filter(|s| !s.is_empty())
                        .unwrap_or_default(),
                    Err(error) => {
                        tracing::warn!(
                            error = %error,
                            "webhook secret envelope unreadable; deliveries will be rejected until it is repaired"
                        );
                        String::new()
                    }
                }
            })
        });

    let signature = req.header("x-helpscout-signature");
    let event_type = req.header("x-helpscout-event").unwrap_or("unknown");

    let secret = crate::webhook::effective_secret(&secret);

    let result = ctx.db.with_write(|conn| {
        webhook_handler::process_webhook(conn, secret.as_bytes(), &req.body, signature, event_type)
    });

    if let WebhookProcessResult::SecretNotConfigured = result {
        let _ = ctx.db.with_write(|conn| {
            crate::audit::log_event(
                conn,
                "warn",
                "webhook",
                "A webhook delivery was rejected: no webhook secret is configured",
                None,
                None,
            )
        });
    }

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
             via HELPSCOUT_WEBHOOK_SECRET.\"}"
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
        400,
        "Bad Request",
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
        return fail_page(
            "No Help Scout connection is active in this session. Save your \
             credentials in Settings and (re)start SupportOS+, then connect \
             from the Settings page.",
        );
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

    let redirect_ok =
        ctx.db.with_read(
            |conn| match crate::oauth_state::redirect_uri_for(conn, state_param) {
                Ok(Some(recorded)) => {
                    let expected = port_from_redirect_uri(&recorded);
                    expected.is_none_or(|expected| expected == ctx.port)
                }
                _ => false,
            },
        );
    if !redirect_ok {
        return fail_page(
            "The authorization response arrived on a different listener than the one \
             the request was issued for (redirect URI mismatch). For safety the code \
             was not exchanged — restart the connect flow from Settings.",
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
                let _ = crate::audit::log_event(
                    conn,
                    "info",
                    "helpscout",
                    &format!(
                        "Connected to Help Scout as {} (OAuth authorization code)",
                        identity.name
                    ),
                    None,
                    None,
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
                "OK",
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
        Err(e) => {
            let _ = ctx.db.with_write(|conn| {
                crate::audit::log_event(
                    conn,
                    "error",
                    "helpscout",
                    "The OAuth connection failed at the code exchange",
                    None,
                    Some(&serde_json::json!({ "detail": e.clone() })),
                )
            });
            fail_page(&escape_html(&e))
        }
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

pub struct LoopbackServer {
    port: u16,
    shutdown: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl LoopbackServer {
    pub fn start(db: DbHandle, handle: Arc<ProviderHandle>, port: u16) -> std::io::Result<Self> {
        let listener = TcpListener::bind(("127.0.0.1", port))?;
        let bound_port = listener.local_addr()?.port();
        let shutdown = Arc::new(AtomicBool::new(false));

        let ctx = Arc::new(LoopbackCtx {
            db,
            port: bound_port,
            oauth: Some(Arc::new(RealOAuthCompletion { handle }) as Arc<dyn OAuthCompletion>),
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

    #[must_use]
    pub fn port(&self) -> u16 {
        self.port
    }

    #[must_use]
    pub fn base_url(&self) -> String {
        format!("http://127.0.0.1:{}/", self.port)
    }

    pub fn stop(&mut self) {
        self.shutdown.store(true, Ordering::SeqCst);
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
        None => return Ok(()),
    };
    let response = handle_request(&req, ctx);
    stream.write_all(&response.serialize())?;
    stream.flush()?;
    let _ = stream.shutdown(std::net::Shutdown::Write);
    Ok(())
}

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
            continue;
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
            port: 1,
            oauth: oauth.map(|f| Arc::new(f) as Arc<dyn OAuthCompletion>),
        }
    }

    fn body_str(r: &Response) -> String {
        String::from_utf8_lossy(&r.body).into_owned()
    }

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

    #[test]
    fn callback_without_oauth_explains_the_real_state() {
        let c = ctx(None);
        let r = handle_request(
            &request("GET", "/oauth/callback?code=1&state=2", &[], b""),
            &c,
        );
        assert_eq!(
            r.status, 400,
            "failure pages answer 400, not 200 (audit §4)"
        );
        let body = body_str(&r);
        assert!(
            body.contains("No Help Scout connection is active"),
            "the page must describe the actual state: {body}"
        );
        assert!(!body.contains("Demo mode is active"));
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
        assert_eq!(
            r.status, 400,
            "failure pages answer 400, not 200 (audit §4)"
        );
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
        let r = handle_request(
            &request("GET", "/oauth/callback?code=1&state=deadbeef", &[], b""),
            &c,
        );
        assert!(body_str(&r).contains("did not match"));

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

    #[test]
    fn listener_answers_only_for_the_real_loopback_server() {
        let db = fixture_db();
        let mut server = LoopbackServer::start(
            db,
            std::sync::Arc::new(crate::provider_handle::ProviderHandle::new_empty()),
            0,
        )
        .expect("start the real listener");
        assert!(
            listener_answers(server.port()),
            "the real listener must answer the probe"
        );
        server.stop();

        let squatter = std::net::TcpListener::bind(("127.0.0.1", 0)).expect("bind squatter");
        let squatter_port = squatter.local_addr().expect("addr").port();
        assert!(
            !listener_answers(squatter_port),
            "a foreign program on the port must fail the probe"
        );
        drop(squatter);

        let probe = std::net::TcpListener::bind(("127.0.0.1", 0)).expect("bind probe");
        let closed_port = probe.local_addr().expect("addr").port();
        drop(probe);
        assert!(!listener_answers(closed_port), "a closed port must fail");
    }

    mod batch28_oauth_tests {
        use super::*;

        fn ctx_on_port(oauth: Option<FakeOAuth>, port: u16) -> LoopbackCtx {
            LoopbackCtx {
                db: fixture_db(),
                port,
                oauth: oauth.map(|f| Arc::new(f) as Arc<dyn OAuthCompletion>),
            }
        }

        fn good_oauth() -> Option<FakeOAuth> {
            Some(FakeOAuth {
                result: Some(ConnectedIdentity {
                    name: "A B".into(),
                    email: Some("a@b.c".into()),
                    remote_id: 7,
                }),
            })
        }

        fn backdate_state(db: &DbHandle, state: &str, hours: i64) {
            db.with_write(|conn| {
                conn.execute(
                    "UPDATE oauth_states
                    SET created_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now', ?2),
                        consumed_at = CASE WHEN consumed_at IS NULL THEN NULL
                                           ELSE strftime('%Y-%m-%dT%H:%M:%fZ', 'now', ?2) END
                  WHERE state = ?1",
                    rusqlite::params![state, format!("-{hours} hours")],
                )
                .expect("backdate");
            });
        }

        #[test]
        fn audit93_callback_rejects_a_state_issued_for_another_port() {
            let c = ctx_on_port(good_oauth(), 1);
            let state =
                c.db.with_write(|conn| {
                    crate::oauth_state::issue_state(conn, "http://127.0.0.1:2/oauth/callback", None)
                })
                .expect("issue state");
            let target = format!("/oauth/callback?code=1&state={state}");
            let r = handle_request(&request("GET", &target, &[], b""), &c);
            let body = body_str(&r);
            assert!(
                body.contains("different listener"),
                "the redirect mismatch is explained: {body}"
            );
            assert!(
                !body.contains("Connected to Help Scout"),
                "the code must not be exchanged on a mismatch"
            );

            let again = handle_request(&request("GET", &target, &[], b""), &c);
            assert!(
                body_str(&again).contains("did not match"),
                "the burned state cannot be replayed on the right port either"
            );
        }

        #[test]
        fn audit93_stale_states_expire() {
            let c = ctx_on_port(good_oauth(), 1);
            let stale =
                c.db.with_write(|conn| {
                    crate::oauth_state::issue_state(conn, "http://127.0.0.1:1/oauth/callback", None)
                })
                .expect("issue state");
            backdate_state(&c.db, &stale, 1);
            let target = format!("/oauth/callback?code=1&state={stale}");
            let r = handle_request(&request("GET", &target, &[], b""), &c);
            assert!(
                body_str(&r).contains("did not match"),
                "an expired state is rejected like an unknown one"
            );

            let fresh =
                c.db.with_write(|conn| {
                    crate::oauth_state::issue_state(conn, "http://127.0.0.1:1/oauth/callback", None)
                })
                .expect("issue state");
            let target = format!("/oauth/callback?code=1&state={fresh}");
            let r = handle_request(&request("GET", &target, &[], b""), &c);
            assert!(
                body_str(&r).contains("Connected to Help Scout"),
                "a fresh state on the matching port completes the flow"
            );
        }

        #[test]
        fn audit93_sweep_states_runs_on_every_new_authorization() {
            let c = ctx_on_port(None, 1);
            let consumed =
                c.db.with_write(|conn| {
                    crate::oauth_state::issue_state(conn, "http://127.0.0.1:1/oauth/callback", None)
                })
                .expect("issue");
            c.db.with_write(|conn| crate::oauth_state::consume_state(conn, &consumed))
                .expect("consume");
            let abandoned =
                c.db.with_write(|conn| {
                    crate::oauth_state::issue_state(conn, "http://127.0.0.1:1/oauth/callback", None)
                })
                .expect("issue");
            backdate_state(&c.db, &consumed, 2);
            backdate_state(&c.db, &abandoned, 2);

            let creds = crate::helpscout_real::HsCredentials {
                client_id: "cid".into(),
                client_secret: "secret".into(),
                redirect_uri: "http://127.0.0.1:1/oauth/callback".into(),
                ..Default::default()
            };
            let url = begin_oauth(&c.db, &creds, "http://127.0.0.1:1/oauth/callback")
                .expect("begin_oauth");
            let fresh = url.rsplit("state=").next().unwrap_or_default().to_string();

            let remaining: Vec<String> = c.db.with_read(|conn| {
                let mut stmt = conn
                    .prepare("SELECT state FROM oauth_states ORDER BY state")
                    .expect("select");
                stmt.query_map([], |r| r.get::<_, String>(0))
                    .expect("rows")
                    .filter_map(|r| r.ok())
                    .collect()
            });
            assert!(
                !remaining.contains(&consumed),
                "the consumed-old state was swept"
            );
            assert!(
                !remaining.contains(&abandoned),
                "the abandoned-old state was swept"
            );
            assert_eq!(remaining, vec![fresh], "only the fresh state remains");
        }
    }
}

#[cfg(test)]
mod batch22_audit32_tests {
    use super::*;

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

    fn ctx() -> LoopbackCtx {
        let mut conn = Connection::open_in_memory().expect("in-memory connection");
        crate::bootstrap::apply_all(&mut conn).expect("migrations");
        LoopbackCtx {
            db: DbHandle::new(conn),
            port: 1,
            oauth: None,
        }
    }

    #[test]
    fn webhook_secret_over_the_cap_verifies_against_the_truncation() {
        let c = ctx();
        let key_path =
            std::env::temp_dir().join(format!("spp-loopback-b22-key-{}.key", std::process::id()));
        crate::settings::init_master_key(&key_path).expect("init master key");

        let long_secret: String = "s".repeat(60);
        c.db.with_write(|conn| {
            crate::settings::set_secret(conn, "webhook_secret", long_secret.as_bytes())
        })
        .expect("store webhook secret");

        let body = br#"{"objectID":777}"#;
        let effective = crate::webhook::effective_secret(&long_secret);
        assert_eq!(effective.chars().count(), 40, "the cap applied");

        let good_sig = crate::webhook::compute_signature(effective.as_bytes(), body);
        let r = handle_request(
            &request(
                "POST",
                "/webhook",
                &[
                    ("X-Helpscout-Signature", &good_sig),
                    ("X-Helpscout-Event", "convo.created"),
                ],
                body,
            ),
            &c,
        );
        assert_eq!(
            r.status, 200,
            "the pre-fix loopback rejected exactly this delivery (verify-full vs register-capped)"
        );

        let wrong_sig = crate::webhook::compute_signature(long_secret.as_bytes(), body);
        let r = handle_request(
            &request(
                "POST",
                "/webhook",
                &[
                    ("X-Helpscout-Signature", &wrong_sig),
                    ("X-Helpscout-Event", "convo.created"),
                ],
                body,
            ),
            &c,
        );
        assert_eq!(r.status, 401, "the full-secret signature never verified");

        let _ = std::fs::remove_file(&key_path);
    }
}
