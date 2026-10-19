//! Test-only mock HTTP/1.1 server (`#[cfg(test)]` module).
//!
//! Integration verification (TODO 45) runs the real client stacks —
//! `RealHelpScoutProvider`, `SyncEngine`, `LoopbackServer`,
//! `OpenAiCompatibleClient` — against a local in-process server instead of
//! the remote services, so no credentials, network access, or cloud
//! dependencies are ever required. The server deliberately mirrors the
//! shape of `crate::loopback`'s minimal listener (one thread per
//! connection, `Connection: close`, bounded reads) but is generic: tests
//! install a handler closure that maps each request to a canned response,
//! and every received request is recorded for assertions.
//!
//! Nothing here is compiled into release builds: the module is declared
//! `#[cfg(test)]` in `lib.rs`.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

const MAX_HEADER_BYTES: usize = 16 * 1024;
const MAX_BODY_BYTES: usize = 8 * 1024 * 1024;
const READ_TIMEOUT: Duration = Duration::from_secs(10);

/// One recorded inbound request (header names lower-cased).
#[derive(Debug, Clone)]
pub struct MockRequest {
    pub method: String,
    /// Path without the query string.
    pub path: String,
    /// Raw query string (no leading `?`).
    pub query: String,
    pub headers: Vec<(String, String)>,
    pub body: String,
}

impl MockRequest {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }

    pub fn query_param(&self, key: &str) -> Option<String> {
        self.query.split('&').find_map(|pair| {
            let (k, v) = pair.split_once('=')?;
            (k == key).then(|| urldecode(v))
        })
    }

    /// Decoded `application/x-www-form-urlencoded` body pairs.
    pub fn form_pairs(&self) -> Vec<(String, String)> {
        self.body
            .split('&')
            .filter(|s| !s.is_empty())
            .map(|pair| match pair.split_once('=') {
                Some((k, v)) => (urldecode(k), urldecode(v)),
                None => (urldecode(pair), String::new()),
            })
            .collect()
    }

    pub fn form_value(&self, key: &str) -> Option<String> {
        self.form_pairs()
            .into_iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v)
    }
}

fn urldecode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 3 <= bytes.len() => {
                if let Ok(b) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                    out.push(b);
                    i += 3;
                } else {
                    out.push(b'%');
                    i += 1;
                }
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Canned response body: accepts a `String`, `&str`, or `serde_json::Value`
/// (serialized to compact JSON) so test fixtures can be written directly
/// as `json!({...})` literals.
#[derive(Debug, Clone)]
pub struct MockBody(pub String);

impl From<String> for MockBody {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for MockBody {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl From<serde_json::Value> for MockBody {
    fn from(v: serde_json::Value) -> Self {
        Self(v.to_string())
    }
}

/// Canned response returned by a test handler.
#[derive(Debug, Clone)]
pub struct MockResponse {
    pub status: u16,
    pub reason: &'static str,
    /// Extra headers (e.g. `x-ratelimit-remaining-minute`).
    pub headers: Vec<(String, String)>,
    pub body: String,
}

impl MockResponse {
    pub fn json(status: u16, body: impl Into<MockBody>) -> Self {
        Self {
            status,
            reason: reason_for(status),
            headers: vec![("content-type".to_string(), "application/json".to_string())],
            body: body.into().0,
        }
    }

    pub fn empty(status: u16) -> Self {
        Self {
            status,
            reason: reason_for(status),
            headers: Vec::new(),
            body: String::new(),
        }
    }

    #[must_use]
    pub fn with_header(mut self, name: &str, value: impl Into<String>) -> Self {
        self.headers.push((name.to_ascii_lowercase(), value.into()));
        self
    }
}

fn reason_for(status: u16) -> &'static str {
    match status {
        200 => "OK",
        201 => "Created",
        204 => "No Content",
        301 => "Moved Permanently",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        409 => "Conflict",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        503 => "Service Unavailable",
        504 => "Gateway Timeout",
        _ => "Status",
    }
}

/// Handlers are shared `Arc` closures — NOT behind a mutex — so a slow or
/// sleeping handler (timeout tests) never blocks other connections, and
/// concurrent connections are truly concurrent.
pub type Handler = std::sync::Arc<dyn Fn(&MockRequest) -> MockResponse + Send + Sync>;

/// Running mock server. Drop stops the accept thread.
pub struct MockServer {
    port: u16,
    hits: Arc<Mutex<Vec<MockRequest>>>,
    handler: Arc<std::sync::RwLock<Handler>>,
    shutdown: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl MockServer {
    /// Bind an OS-assigned loopback port and serve `handler` until dropped.
    pub fn start(handler: Handler) -> std::io::Result<Self> {
        let listener = TcpListener::bind(("127.0.0.1", 0))?;
        let port = listener.local_addr()?.port();
        let hits = Arc::new(Mutex::new(Vec::new()));
        let handler = Arc::new(std::sync::RwLock::new(handler));
        let shutdown = Arc::new(AtomicBool::new(false));

        let thread_shutdown = Arc::clone(&shutdown);
        let thread_hits = Arc::clone(&hits);
        let thread_handler = Arc::clone(&handler);
        let thread = std::thread::Builder::new()
            .name("spp-mock-http".to_string())
            .spawn(move || {
                while !thread_shutdown.load(Ordering::SeqCst) {
                    match listener.accept() {
                        Ok((stream, _addr)) => {
                            if thread_shutdown.load(Ordering::SeqCst) {
                                break;
                            }
                            let hits = Arc::clone(&thread_hits);
                            let handler = Arc::clone(&thread_handler);
                            let spawned = std::thread::Builder::new()
                                .name("spp-mock-conn".to_string())
                                .spawn(move || handle_conn(stream, hits, handler));
                            if spawned.is_err() {
                                // Test failure, not production code: propagate
                                // loudly so a resource-exhausted run fails
                                // instead of hanging.
                                panic!("mock server: connection thread spawn failed");
                            }
                        }
                        Err(_) => {
                            if thread_shutdown.load(Ordering::SeqCst) {
                                break;
                            }
                            std::thread::sleep(Duration::from_millis(5));
                        }
                    }
                }
            })?;

        Ok(Self {
            port,
            hits,
            handler,
            shutdown,
            thread: Some(thread),
        })
    }

    /// Base URL, e.g. `http://127.0.0.1:39877` (no trailing slash).
    #[must_use]
    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    /// Every request received so far, in arrival order.
    pub fn hits(&self) -> Vec<MockRequest> {
        self.hits.lock().unwrap_or_else(|p| p.into_inner()).clone()
    }

    /// Number of recorded requests whose method and path match.
    pub fn hit_count(&self, method: &str, path: &str) -> usize {
        self.hits()
            .iter()
            .filter(|r| r.method == method && r.path == path)
            .count()
    }

    /// Replace the handler (for sequential scripting mid-test).
    pub fn set_handler(&self, handler: Handler) {
        *self.handler.write().unwrap_or_else(|p| p.into_inner()) = handler;
    }

    pub fn stop(&mut self) {
        self.shutdown.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect(("127.0.0.1", self.port));
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for MockServer {
    fn drop(&mut self) {
        self.stop();
    }
}

fn handle_conn(
    stream: TcpStream,
    hits: Arc<Mutex<Vec<MockRequest>>>,
    handler: Arc<std::sync::RwLock<Handler>>,
) {
    let mut stream = stream;
    let req = match read_request(&mut stream) {
        Ok(Some(req)) => req,
        _ => return,
    };
    {
        let mut log = hits.lock().unwrap_or_else(|p| p.into_inner());
        log.push(req.clone());
    }
    // Snapshot the handler under a short read lock, then invoke it with NO
    // lock held: a slow/sleeping handler must not serialize other
    // connections (see the timeout tests).
    let handler = {
        let guard = handler.read().unwrap_or_else(|p| p.into_inner());
        std::sync::Arc::clone(&guard)
    };
    let response = handler(&req);
    let _ = stream.write_all(&serialize(&response));
    let _ = stream.flush();
    let _ = stream.shutdown(std::net::Shutdown::Write);
}

fn serialize(res: &MockResponse) -> Vec<u8> {
    let mut out = Vec::with_capacity(res.body.len() + 128);
    out.extend_from_slice(format!("HTTP/1.1 {} {}\r\n", res.status, res.reason).as_bytes());
    let mut have_type = false;
    for (k, v) in &res.headers {
        if k == "content-type" {
            have_type = true;
        }
        out.extend_from_slice(format!("{k}: {v}\r\n").as_bytes());
    }
    if !have_type && !res.body.is_empty() {
        out.extend_from_slice(b"Content-Type: application/json\r\n");
    }
    out.extend_from_slice(format!("Content-Length: {}\r\n", res.body.len()).as_bytes());
    out.extend_from_slice(b"Connection: close\r\n\r\n");
    out.extend_from_slice(res.body.as_bytes());
    out
}

fn read_request(stream: &mut TcpStream) -> std::io::Result<Option<MockRequest>> {
    let _ = stream.set_read_timeout(Some(READ_TIMEOUT));
    let _ = stream.set_write_timeout(Some(READ_TIMEOUT));
    let mut buf: Vec<u8> = Vec::with_capacity(1024);
    let mut chunk = [0u8; 4096];
    let header_end = loop {
        if let Some(pos) = find_header_end(&buf) {
            break pos;
        }
        if buf.len() > MAX_HEADER_BYTES {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "header block too large",
            ));
        }
        let n = stream.read(&mut chunk)?;
        if n == 0 {
            if buf.is_empty() {
                return Ok(None);
            }
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "closed mid-header",
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

    let mut headers = Vec::new();
    for line in lines {
        if line.is_empty() {
            continue;
        }
        if let Some((name, value)) = line.split_once(':') {
            headers.push((name.trim().to_ascii_lowercase(), value.trim().to_string()));
        }
    }

    let content_length = headers
        .iter()
        .find(|(k, _)| k == "content-length")
        .and_then(|(_, v)| v.parse::<usize>().ok())
        .unwrap_or(0);
    if content_length > MAX_BODY_BYTES {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "body too large",
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
                "closed mid-body",
            ));
        }
        let need = content_length - body.len();
        body.extend_from_slice(&chunk[..n.min(need)]);
    }

    let (path, query) = match target.split_once('?') {
        Some((p, q)) => (p.to_string(), q.to_string()),
        None => (target, String::new()),
    };

    Ok(Some(MockRequest {
        method,
        path,
        query,
        headers,
        body: String::from_utf8_lossy(&body).into_owned(),
    }))
}

fn find_header_end(buf: &[u8]) -> Option<usize> {
    buf.windows(4).position(|w| w == b"\r\n\r\n")
}

/// Handler that serves fixed responses in order and repeats the last one
/// afterwards. Used to script sequences such as `429` then `200`.
pub fn scripted(responses: Vec<MockResponse>) -> Handler {
    let queue = Mutex::new(std::collections::VecDeque::from(responses));
    std::sync::Arc::new(move |_req: &MockRequest| {
        let mut q = queue.lock().unwrap_or_else(|p| p.into_inner());
        match q.len() {
            0 => MockResponse::empty(500),
            1 => q
                .front()
                .cloned()
                .unwrap_or_else(|| MockResponse::empty(500)),
            _ => q.pop_front().unwrap_or_else(|| MockResponse::empty(500)),
        }
    })
}

/// Handler that always answers with one fixed response.
pub fn fixed(response: MockResponse) -> Handler {
    std::sync::Arc::new(move |_req: &MockRequest| response.clone())
}

/// 404 for everything — the "server is up but has no data" baseline.
pub fn not_found() -> Handler {
    fixed(MockResponse::json(404, r#"{"error":"not found"}"#))
}
