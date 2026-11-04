use std::sync::{Arc, Mutex};
use std::time::Duration;

use rusqlite::Connection;
use serde_json::{Value, json};

use crate::api_queue::{
    ApiQueue, PRIORITY_ANALYTICS, PRIORITY_INDEXING, PRIORITY_INTERACTIVE, PRIORITY_SYNC,
    PRIORITY_USER_SEND, QueueRateLimiter,
};
use crate::error::{Error, Result};

use crate::helpscout::{
    ConversationCreated, ConversationPatch, ConversationQuery, CreateConversationInput,
    CreateThreadInput, CustomerQuery, HelpScoutProvider, HsBeaconChat, HsConversation, HsCustomer,
    HsDocArticle, HsDocCategory, HsDocCollection, HsField, HsFieldOption, HsFolder, HsMailbox,
    HsOrganization, HsPropertyDef, HsRating, HsReportRow, HsSavedReply, HsTag, HsTeam, HsThread,
    HsUser, HsUserStatus, HsWebhookConfig, HsWorkflow, Page, ThreadCreated,
};

mod http;

pub(crate) const HS_AUTHORIZE_URL: &str =
    "https://secure.helpscout.net/authentication/authorizeClientApplication";

const DOC_ARTICLE_FETCH_CAP: usize = 500;

#[derive(Debug, Clone)]
pub(crate) struct HsApiError {
    pub status_code: u16,
    pub message: String,
    pub friendly: String,
    pub retryable: bool,
}

impl std::fmt::Display for HsApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.friendly)
    }
}

impl std::error::Error for HsApiError {}

impl From<HsApiError> for Error {
    fn from(e: HsApiError) -> Self {
        Error::Other(Box::new(e))
    }
}

pub(crate) fn friendly_error(status: u16, body: &str, method: &str) -> String {
    let detail = if body.is_empty() {
        String::new()
    } else {
        body.chars().take(300).collect::<String>()
    };
    let get = method == "GET";
    match status {
        400 => format!("Help Scout rejected the request as invalid{}{}. No changes were made. {}", if get { " (check the filters used)" } else { " (check the values you entered)" }, "", detail),
        401 => "The Help Scout connection is no longer authenticated. Re-connect Help Scout in Settings, then retry. No changes were made.".to_string(),
        403 => format!("Help Scout denied permission for this operation (your user role may not allow it). No changes were made. {detail}"),
        404 => "Help Scout reports this record no longer exists (it may have been deleted or merged into another conversation). Local data was preserved.".to_string(),
        409 => format!("Help Scout reported a conflict - the record changed on the server while we were working. Refresh the conversation and retry. {detail}"),
        412 => "Help Scout rejected this change because the conversation cannot currently accept another thread. No local changes were treated as successful.".to_string(),
        413 => "The request was too large (attachments or message size exceed Help Scout limits). Try smaller content.".to_string(),
        415 => "Help Scout did not accept the format of this request. No changes were made.".to_string(),
        423 => "Help Scout has this conversation locked (another process or user is acting on it right now). Try again in a moment.".to_string(),
        429 => "Help Scout rate limit reached. The operation will be retried automatically when the limit resets.".to_string(),
        500 => "Help Scout reported an internal error. The operation can be retried; no local data was changed.".to_string(),
        503 => "Help Scout is temporarily unavailable. The operation can be retried automatically.".to_string(),
        504 => "The request to Help Scout timed out. It may or may not have completed remotely - verify in Help Scout before retrying a send.".to_string(),
        _ => format!("Help Scout returned status {status}. {detail}"),
    }
}

fn is_retryable(status: u16) -> bool {
    matches!(status, 429 | 500 | 503 | 504)
}

pub(crate) fn apply_m043(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS hs_rate_limit (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            limit_per_minute INTEGER NOT NULL DEFAULT 150,
            remaining INTEGER,
            retry_after_sec INTEGER,
            updated_at TEXT NOT NULL DEFAULT (datetime('now'))
        );",
    )?;
    let have_row: i64 = conn
        .query_row("SELECT COUNT(*) FROM hs_rate_limit", [], |r| r.get(0))
        .unwrap_or(0);
    if have_row == 0 {
        let legacy: Option<String> = conn
            .query_row(
                "SELECT value FROM application_settings WHERE key = 'hs_rate_limit'",
                [],
                |r| r.get(0),
            )
            .ok();
        if let Some(raw) = legacy {
            if let Ok(v) = serde_json::from_str::<Value>(&raw) {
                let _ = conn.execute(
                    "INSERT OR IGNORE INTO hs_rate_limit
                        (id, limit_per_minute, remaining, retry_after_sec, updated_at)
                     VALUES (1, ?1, ?2, ?3, COALESCE(?4, datetime('now')))",
                    rusqlite::params![
                        v["limitPerMinute"].as_i64().unwrap_or(150),
                        v["remaining"].as_i64(),
                        v["retryAfterSec"].as_i64(),
                        v["updatedAt"].as_str(),
                    ],
                );
            }
            let _ = conn.execute(
                "DELETE FROM application_settings WHERE key = 'hs_rate_limit'",
                [],
            );
        }
    }
    Ok(())
}

#[derive(Debug, Clone)]
pub(crate) struct RateLimitState {
    pub limit_per_minute: i64,
    pub remaining: Option<i64>,
    pub retry_after_sec: Option<i64>,
    pub updated_at: String,
}

impl Default for RateLimitState {
    fn default() -> Self {
        Self {
            limit_per_minute: 150,
            remaining: None,
            retry_after_sec: None,
            updated_at: chrono::Utc::now().to_rfc3339(),
        }
    }
}

pub struct HsRateLimiter {
    state: Mutex<RateLimitState>,
    send_timestamps: Mutex<Vec<i64>>,
    db: Option<crate::db::DbHandle>,
}

impl HsRateLimiter {
    pub fn new(db: Option<crate::db::DbHandle>) -> Self {
        let limiter = Self {
            state: Mutex::new(RateLimitState::default()),
            send_timestamps: Mutex::new(Vec::new()),
            db,
        };
        limiter.load();
        limiter
    }

    #[must_use]
    pub(crate) fn rate_limit_state(&self) -> RateLimitState {
        self.state.lock().unwrap_or_else(|p| p.into_inner()).clone()
    }

    fn load(&self) {
        let Some(db) = &self.db else { return };
        let conn = db.write_guard();
        let _ = apply_m043(&conn);
        let Ok(row) = conn.query_row(
            "SELECT limit_per_minute, remaining, retry_after_sec, updated_at
               FROM hs_rate_limit WHERE id = 1",
            [],
            |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, Option<i64>>(1)?,
                    r.get::<_, Option<i64>>(2)?,
                    r.get::<_, String>(3)?,
                ))
            },
        ) else {
            return;
        };
        let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
        state.limit_per_minute = row.0;
        state.remaining = row.1;
        state.retry_after_sec = row.2;
        state.updated_at = row.3;
    }

    fn persist(&self, state: &RateLimitState) {
        let Some(db) = &self.db else { return };
        let conn = db.write_guard();
        let _ = conn.execute(
            "INSERT INTO hs_rate_limit (id, limit_per_minute, remaining, retry_after_sec, updated_at)
             VALUES (1, ?1, ?2, ?3, ?4)
             ON CONFLICT(id) DO UPDATE SET
                limit_per_minute = excluded.limit_per_minute,
                remaining = excluded.remaining,
                retry_after_sec = excluded.retry_after_sec,
                updated_at = excluded.updated_at",
            rusqlite::params![
                state.limit_per_minute,
                state.remaining,
                state.retry_after_sec,
                state.updated_at
            ],
        );
    }

    pub(crate) fn record_response(
        &self,
        limit: Option<i64>,
        remaining: Option<i64>,
        retry_after: Option<i64>,
        is_write: bool,
    ) {
        let now_ms = chrono::Utc::now().timestamp_millis();
        {
            let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
            if let Some(l) = limit {
                state.limit_per_minute = l;
            }
            if remaining.is_some() {
                state.remaining = remaining;
            }
            state.retry_after_sec = if retry_after.is_some() {
                retry_after
            } else {
                None
            };
            state.updated_at = chrono::Utc::now().to_rfc3339();
            self.persist(&state);
        }
        let mut sends = self
            .send_timestamps
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        sends.retain(|t| now_ms - t < 60_000);
        sends.push(now_ms);
        if is_write {
            sends.push(now_ms);
        }
    }

    pub(crate) fn record_error_429(&self, retry_after_sec: Option<i64>) {
        let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
        state.retry_after_sec = Some(retry_after_sec.unwrap_or(30));
        state.remaining = Some(0);
        state.updated_at = chrono::Utc::now().to_rfc3339();
        self.persist(&state);
    }

    pub fn snapshot(&self) -> Value {
        let now_ms = chrono::Utc::now().timestamp_millis();
        let state = self.state.lock().unwrap_or_else(|p| p.into_inner()).clone();
        let mut sends = self
            .send_timestamps
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        sends.retain(|t| now_ms - t < 60_000);
        json!({
            "limitPerMinute": state.limit_per_minute,
            "remaining": state.remaining,
            "retryAfterSec": state.retry_after_sec,
            "updatedAt": state.updated_at,
            "inFlightWindow": sends.len(),
        })
    }
}

/// GA-1: the limiter is consumed through the queue's `QueueRateLimiter`
/// trait, so the dependency points one way (helpscout_real -> api_queue)
/// and the former `api_queue.rs -> helpscout_real::HsRateLimiter` import
/// (the module cycle the graphify audit surfaced) is gone.
impl QueueRateLimiter for HsRateLimiter {
    fn wait_time_ms(&self, _is_write: bool) -> i64 {
        let now_ms = chrono::Utc::now().timestamp_millis();
        let state = self.state.lock().unwrap_or_else(|p| p.into_inner());
        if let (Some(retry), Some(0)) = (state.retry_after_sec, state.remaining)
            && let Ok(updated) = chrono::DateTime::parse_from_rfc3339(&state.updated_at)
        {
            let reset_at = updated.timestamp_millis() + retry * 1000;
            if reset_at > now_ms {
                return reset_at - now_ms + 250;
            }
        }
        let mut sends = self
            .send_timestamps
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        sends.retain(|t| now_ms - t < 60_000);
        let effective_limit = (state.limit_per_minute - 5).max(5);
        if sends.len() as i64 + 1 > effective_limit
            && let Some(oldest) = sends.first()
        {
            let wait = 60_000 - (now_ms - oldest) + 100;
            if wait > 0 {
                return wait;
            }
        }
        0
    }
}

const TOKEN_EXPIRY_MARGIN_SECS: i64 = 120;

struct TokenRow {
    access_token: Option<String>,
    refresh_token: Option<String>,
    expires_at: Option<String>,
    revoked: bool,
}

fn token_row(conn: &Connection) -> Option<TokenRow> {
    let (access_column, refresh_column, expires_at, revoked): (
        Option<String>,
        Option<String>,
        Option<String>,
        i64,
    ) = conn
        .query_row(
            "SELECT access_token, refresh_token, expires_at, revoked
               FROM oauth_tokens WHERE account = 'default'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get::<_, i64>(3)?)),
        )
        .ok()?;
    let access_token = crate::settings::get_secret(conn, SECRET_OAUTH_ACCESS)
        .ok()
        .flatten()
        .and_then(|b| String::from_utf8(b).ok())
        .filter(|t| !t.is_empty())
        .or(access_column.filter(|t| !t.is_empty()));
    let refresh_token = crate::settings::get_secret(conn, SECRET_OAUTH_REFRESH)
        .ok()
        .flatten()
        .and_then(|b| String::from_utf8(b).ok())
        .filter(|t| !t.is_empty())
        .or(refresh_column.filter(|t| !t.is_empty()));
    Some(TokenRow {
        access_token,
        refresh_token,
        expires_at,
        revoked: revoked != 0,
    })
}

#[derive(Clone, Default)]
pub struct HsCredentials {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
    pub api_base: String,
    pub webhook_secret: String,
    pub docs_api_key: String,
    pub docs_api_base: String,
}

impl core::fmt::Debug for HsCredentials {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("HsCredentials")
            .field("client_id", &self.client_id)
            .field(
                "client_secret",
                &crate::config::debug_redact(&self.client_secret),
            )
            .field("redirect_uri", &self.redirect_uri)
            .field("api_base", &self.api_base)
            .field(
                "webhook_secret",
                &crate::config::debug_redact(&self.webhook_secret),
            )
            .field(
                "docs_api_key",
                &crate::config::debug_redact(&self.docs_api_key),
            )
            .field("docs_api_base", &self.docs_api_base)
            .finish()
    }
}

pub(crate) const SECRET_CLIENT_ID: &str = "helpscout_client_id";
pub(crate) const SECRET_CLIENT_SECRET: &str = "helpscout_client_secret";
pub const SECRET_DOCS_API_KEY: &str = "helpscout_docs_api_key";
pub(crate) const SECRET_WEBHOOK: &str = "webhook_secret";
pub const SECRET_OAUTH_ACCESS: &str = "helpscout_oauth_access_token";
pub const SECRET_OAUTH_REFRESH: &str = "helpscout_oauth_refresh_token";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CredentialSource {
    #[default]
    NotSet,
    Environment,
    AppSettings,
}

impl CredentialSource {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::NotSet => "not set",
            Self::Environment => "environment",
            Self::AppSettings => "app settings",
        }
    }

    #[must_use]
    pub fn is_set(self) -> bool {
        !matches!(self, Self::NotSet)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CredentialSources {
    pub client_id: CredentialSource,
    pub client_secret: CredentialSource,
    pub webhook_secret: CredentialSource,
    pub docs_api_key: CredentialSource,
}

fn merge_field(env_value: String, stored: Option<Vec<u8>>) -> (String, CredentialSource) {
    if !env_value.trim().is_empty() {
        return (env_value, CredentialSource::Environment);
    }
    match stored {
        Some(bytes) if !bytes.is_empty() => (
            String::from_utf8_lossy(&bytes).into_owned(),
            CredentialSource::AppSettings,
        ),
        _ => (String::new(), CredentialSource::NotSet),
    }
}

impl HsCredentials {
    pub(crate) fn from_env() -> Self {
        let env = crate::config::load_env_config();
        Self {
            client_id: env.helpscout.client_id,
            client_secret: env.helpscout.client_secret,
            redirect_uri: env.helpscout.redirect_uri,
            api_base: env.helpscout.api_base,
            webhook_secret: env.helpscout.webhook_secret,
            docs_api_key: env.helpscout.docs_api_key,
            docs_api_base: env.helpscout.docs_api_base,
        }
    }

    #[must_use]
    pub fn from_envelope(conn: &Connection) -> Self {
        let read = |key: &str| match crate::settings::get_secret(conn, key) {
            Ok(Some(bytes)) => String::from_utf8_lossy(&bytes).into_owned(),
            Ok(None) => String::new(),
            Err(error) => {
                tracing::warn!(key, error = %error, "secrets envelope unreadable; treating as unset");
                String::new()
            }
        };
        Self {
            client_id: read(SECRET_CLIENT_ID),
            client_secret: read(SECRET_CLIENT_SECRET),
            webhook_secret: read(SECRET_WEBHOOK),
            docs_api_key: read(SECRET_DOCS_API_KEY),
            ..Self::default()
        }
    }

    #[must_use]
    pub fn resolve(conn: &Connection) -> (Self, CredentialSources) {
        Self::resolve_with(&Self::from_env(), conn)
    }

    #[must_use]
    pub(crate) fn resolve_with(env: &Self, conn: &Connection) -> (Self, CredentialSources) {
        let read = |key: &str| crate::settings::get_secret(conn, key).ok().flatten();
        let (client_id, client_id_src) = merge_field(env.client_id.clone(), read(SECRET_CLIENT_ID));
        let (client_secret, client_secret_src) =
            merge_field(env.client_secret.clone(), read(SECRET_CLIENT_SECRET));
        let (webhook_secret, webhook_secret_src) =
            merge_field(env.webhook_secret.clone(), read(SECRET_WEBHOOK));
        let (docs_api_key, docs_api_key_src) =
            merge_field(env.docs_api_key.clone(), read(SECRET_DOCS_API_KEY));
        (
            Self {
                client_id,
                client_secret,
                webhook_secret,
                docs_api_key,
                redirect_uri: env.redirect_uri.clone(),
                api_base: env.api_base.clone(),
                docs_api_base: env.docs_api_base.clone(),
            },
            CredentialSources {
                client_id: client_id_src,
                client_secret: client_secret_src,
                webhook_secret: webhook_secret_src,
                docs_api_key: docs_api_key_src,
            },
        )
    }

    pub fn store_in_envelope(
        conn: &Connection,
        client_id: &str,
        client_secret: &str,
        webhook_secret: &str,
        docs_api_key: &str,
    ) -> crate::error::Result<()> {
        if !client_id.trim().is_empty() {
            crate::settings::set_secret(conn, SECRET_CLIENT_ID, client_id.trim().as_bytes())?;
        }
        if !client_secret.trim().is_empty() {
            crate::settings::set_secret(
                conn,
                SECRET_CLIENT_SECRET,
                client_secret.trim().as_bytes(),
            )?;
        }
        if !webhook_secret.trim().is_empty() {
            crate::settings::set_secret(conn, SECRET_WEBHOOK, webhook_secret.trim().as_bytes())?;
        }
        if !docs_api_key.trim().is_empty() {
            crate::settings::set_secret(conn, SECRET_DOCS_API_KEY, docs_api_key.trim().as_bytes())?;
        }
        crate::reports_api::reset_capability(conn);
        Ok(())
    }

    pub fn clear_envelope(conn: &Connection) -> crate::error::Result<usize> {
        let mut removed = 0;
        for key in [
            SECRET_CLIENT_ID,
            SECRET_CLIENT_SECRET,
            SECRET_WEBHOOK,
            SECRET_DOCS_API_KEY,
        ] {
            removed += usize::from(crate::settings::delete_secret(conn, key)?);
        }
        crate::reports_api::reset_capability(conn);
        Ok(removed)
    }

    pub fn is_configured(&self) -> bool {
        !self.client_id.is_empty() && !self.client_secret.is_empty()
    }

    pub fn authorize_url(&self, state: &str) -> String {
        format!(
            "{HS_AUTHORIZE_URL}?client_id={}&state={}",
            urlencode(&self.client_id),
            urlencode(state)
        )
    }
}

fn base64_of(s: &str) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(s)
}

fn stored_api_concurrency(db: &crate::db::DbHandle) -> usize {
    let env_default = crate::config::load_env_config().sync.api_concurrency.max(1) as usize;
    let stored = db.with_read(|conn| {
        crate::settings::get_i64(
            conn,
            "api_concurrency",
            i64::try_from(env_default).unwrap_or(2),
        )
    });
    match stored {
        Ok(n) if n >= 1 => n as usize,
        Ok(_) => 1,
        Err(_) => env_default,
    }
}

fn urlencode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => {
                out.push('%');
                out.push_str(&format!("{b:02X}"));
            }
        }
    }
    out
}

pub struct RealHelpScoutProvider {
    client: reqwest::Client,
    api_base: String,
    db: crate::db::DbHandle,
    pub credentials: HsCredentials,
    pub limiter: Arc<HsRateLimiter>,
    pub queue: Arc<ApiQueue>,
    docs_queue: Arc<ApiQueue>,
    missing_docs_key_warned: std::sync::atomic::AtomicBool,
}

impl RealHelpScoutProvider {
    #[must_use]
    pub fn new(db: crate::db::DbHandle, credentials: HsCredentials) -> Self {
        let limiter = Arc::new(HsRateLimiter::new(Some(db.clone())));
        let concurrency = stored_api_concurrency(&db);
        let queue = ApiQueue::new(
            Arc::clone(&limiter) as Arc<dyn QueueRateLimiter>,
            concurrency,
        );
        let docs_queue = ApiQueue::new(Arc::new(HsRateLimiter::new(None)), concurrency);
        Self {
            client: reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .timeout(Duration::from_secs(60))
                .build()
                .unwrap_or_default(),
            api_base: credentials.api_base.trim_end_matches('/').to_string(),
            db,
            limiter,
            queue,
            docs_queue,
            credentials,
            missing_docs_key_warned: std::sync::atomic::AtomicBool::new(false),
        }
    }

    pub fn set_api_concurrency(&self, n: usize) {
        self.queue.set_concurrency(n);
        self.docs_queue.set_concurrency(n);
    }

    #[must_use]
    pub fn api_concurrency(&self) -> usize {
        self.queue.concurrency()
    }

    #[must_use]
    pub(crate) fn http_client(&self) -> reqwest::Client {
        self.client.clone()
    }

    #[must_use]
    pub(crate) fn rate_limit_snapshot(&self) -> RateLimitState {
        self.limiter.rate_limit_state()
    }

    fn conn_lock(&self) -> crate::db::DbWriteGuard<'_> {
        self.db.write_guard()
    }

    fn log_missing_docs_key(&self) {
        if !self
            .missing_docs_key_warned
            .swap(true, std::sync::atomic::Ordering::Relaxed)
        {
            let conn = self.conn_lock();
            let _ = crate::audit::log_event(
                &conn,
                "warn",
                "docs",
                "Help Center sync is idle: no Docs API key configured. Add one in Settings (Help Scout Profile -> Security & Access -> Docs API).",
                None,
                None,
            );
            tracing::warn!(
                operation = "docs",
                "Docs API key missing - Help Center articles will not sync until one is configured"
            );
        }
    }

    async fn docs_request(&self, path: &str) -> Result<Value> {
        if self.credentials.docs_api_key.is_empty() {
            return Ok(Value::Null);
        }
        let path = path.to_string();
        let mut attempt = 0u32;
        loop {
            attempt += 1;
            let fut = self.docs_request_inner(&path);
            let res = self.docs_queue.enqueue(PRIORITY_SYNC, false, fut).await;
            match res {
                Err(e) if attempt < 3 => {
                    let status = hs_status(&e);
                    if matches!(status, Some(429) | Some(503)) {
                        let wait = (5u64 * u64::from(attempt)).min(60);
                        tracing::warn!(
                            operation = "docs",
                            path = %path,
                            status = ?status,
                            wait_secs = wait,
                            "Docs API throttled — retrying"
                        );
                        tokio::time::sleep(Duration::from_secs(wait)).await;
                        continue;
                    }
                    return Err(e);
                }
                other => return other,
            }
        }
    }

    fn map_conversation(v: &Value) -> HsConversation {
        let assignee_type = v["assignee"]["type"].as_str().map(|s| s.to_string());
        let assignee_is_team = assignee_type.as_deref() == Some("team");
        HsConversation {
            remote_id: v["id"].as_i64().unwrap_or(0),
            number: v["number"].as_i64().unwrap_or(0),
            kind: v["type"].as_str().map(|s| s.to_string()),
            source_type: v["source"]["type"].as_str().map(|s| s.to_string()),
            source_via: v["source"]["via"].as_str().map(|s| s.to_string()),
            subject: v["subject"].as_str().map(|s| s.to_string()),
            preview: v["preview"].as_str().map(|s| s.to_string()),
            status: v["status"].as_str().unwrap_or("active").to_string(),
            state: v["state"].as_str().map(|s| s.to_string()),
            mailbox_id: v["mailboxId"].as_i64().unwrap_or(0),
            assignee_id: if assignee_is_team {
                None
            } else {
                v["assignee"]["id"].as_i64()
            },
            assignee_type: assignee_type.clone(),
            assigned_team_id: v["assignedTeam"]["id"].as_i64().or(if assignee_is_team {
                v["assignee"]["id"].as_i64()
            } else {
                None
            }),
            customer_id: v["primaryCustomer"]["id"].as_i64().unwrap_or(0),
            priority: None,
            created_at: v["createdAt"].as_str().map(|s| s.to_string()),
            updated_at: v["userUpdatedAt"].as_str().map(|s| s.to_string()),
            closed_at: v["closedAt"].as_str().map(|s| s.to_string()),
            snoozed_until: v["snooze"]["snoozedUntil"].as_str().map(|s| s.to_string()),
            thread_count: v["threads"].as_i64().unwrap_or(0),
            merged_into: None,
            tags: v["tags"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|t| t["tag"].as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default(),
            custom_fields: v["fields"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .map(|f| crate::helpscout::HsCustomFieldValue {
                            field_id: f["id"].as_i64().unwrap_or(0),
                            value: f["value"].as_str().map(String::from),
                            text: f["label"].as_str().map(String::from),
                            system_type: f["systemType"].as_str().map(String::from),
                        })
                        .collect()
                })
                .unwrap_or_default(),
        }
    }

    fn map_customer(v: &Value) -> HsCustomer {
        let emails: Vec<crate::helpscout::HsCustomerEmail> = v["emails"]
            .as_array()
            .map(|a| {
                a.iter()
                    .map(|e| crate::helpscout::HsCustomerEmail {
                        value: e["value"].as_str().map(|s| s.to_string()),
                        kind: e["type"].as_str().map(|s| s.to_string()),
                    })
                    .collect()
            })
            .unwrap_or_default();
        let phones: Vec<crate::helpscout::HsCustomerPhone> = v["phones"]
            .as_array()
            .map(|a| {
                a.iter()
                    .map(|p| crate::helpscout::HsCustomerPhone {
                        value: p["value"].as_str().map(|s| s.to_string()),
                        kind: p["type"].as_str().map(|s| s.to_string()),
                    })
                    .collect()
            })
            .unwrap_or_default();
        HsCustomer {
            remote_id: v["id"].as_i64().unwrap_or(0),
            first_name: v["firstName"].as_str().map(|s| s.to_string()),
            last_name: v["lastName"].as_str().map(|s| s.to_string()),
            email: emails.first().and_then(|e| e.value.clone()),
            organization: v["organization"]["name"].as_str().map(|s| s.to_string()),
            job_title: v["jobTitle"].as_str().map(|s| s.to_string()),
            phone: phones.first().and_then(|p| p.value.clone()),
            created_at: v["createdAt"].as_str().map(|s| s.to_string()),
            updated_at: v["updatedAt"].as_str().map(|s| s.to_string()),
            photo_url: v["photoUrl"].as_str().map(|s| s.to_string()),
            organization_id: v["organization"]["id"].as_i64(),
            background: v["background"].as_str().map(|s| s.to_string()),
            age: v["age"].as_i64().map(|a| a.to_string()),
            gender: v["gender"].as_str().map(|s| s.to_string()),
            location: v["location"].as_str().map(|s| s.to_string()),
            emails,
            phones,
            chats: v["chatHandles"]
                .as_array()
                .or_else(|| v["chats"].as_array())
                .map(|a| {
                    a.iter()
                        .map(|ch| crate::helpscout::HsCustomerChat {
                            value: ch["value"].as_str().map(|s| s.to_string()),
                            kind: ch["type"].as_str().map(|s| s.to_string()),
                        })
                        .collect()
                })
                .unwrap_or_default(),
            websites: v["websites"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .map(|w| crate::helpscout::HsCustomerWebsite {
                            value: w["value"].as_str().map(|s| s.to_string()),
                        })
                        .collect()
                })
                .unwrap_or_default(),
            social_profiles: v["socialProfiles"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .map(|s| crate::helpscout::HsCustomerSocialProfile {
                            value: s["value"].as_str().map(|x| x.to_string()),
                            kind: s["type"].as_str().map(|x| x.to_string()),
                        })
                        .collect()
                })
                .unwrap_or_default(),
            address: v.get("address").filter(|a| a.is_object()).and_then(|a| {
                serde_json::from_value::<crate::helpscout::HsCustomerAddress>(a.clone()).ok()
            }),
            properties: v["properties"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .map(|p| crate::helpscout::HsCustomerPropertyValue {
                            definition_remote_id: p["definitionRemoteId"].as_i64(),
                            key: p["key"].as_str().map(|s| s.to_string()),
                            name: p["name"].as_str().map(|s| s.to_string()),
                            value: p["value"].as_str().map(|s| s.to_string()),
                        })
                        .collect()
                })
                .unwrap_or_default(),
        }
    }

    fn map_thread(v: &Value, conversation_id: i64) -> HsThread {
        let recipients = |key: &str| -> Vec<crate::helpscout::HsThreadRecipient> {
            v[key]
                .as_array()
                .map(|a| {
                    a.iter()
                        .map(|r| {
                            if r.is_string() {
                                crate::helpscout::HsThreadRecipient {
                                    id: None,
                                    email: r.as_str().map(|s| s.to_string()),
                                }
                            } else {
                                crate::helpscout::HsThreadRecipient {
                                    id: r["id"].as_i64(),
                                    email: r["email"].as_str().map(|s| s.to_string()),
                                }
                            }
                        })
                        .collect()
                })
                .unwrap_or_default()
        };
        let attachments = v["attachments"]
            .as_array()
            .map(|a| {
                a.iter()
                    .map(|at| crate::helpscout::HsThreadAttachment {
                        remote_id: at["id"].as_i64().unwrap_or(0),
                        filename: at["filename"].as_str().map(|s| s.to_string()),
                        mime_type: at["mimeType"].as_str().map(|s| s.to_string()),
                        size: at["size"].as_i64(),
                    })
                    .collect()
            })
            .unwrap_or_default();
        HsThread {
            remote_id: v["id"].as_i64().unwrap_or(0),
            conversation_id,
            kind: v["type"].as_str().unwrap_or("message").to_string(),
            status: v["status"].as_str().map(|s| s.to_string()),
            state: v["state"].as_str().map(|s| s.to_string()),
            body: v["body"].as_str().map(|s| s.to_string()),
            created_by_customer_id: if v["createdBy"]["type"].as_str() == Some("customer") {
                v["createdBy"]["id"].as_i64()
            } else {
                v["createdByCustomer"]["id"].as_i64()
            },
            created_by_user_id: if v["createdBy"]["type"].as_str() == Some("user") {
                v["createdBy"]["id"].as_i64()
            } else {
                v["createdByUser"]["id"].as_i64()
            },
            assigned_to_id: v["assignedTo"]["id"].as_i64(),
            created_at: v["createdAt"].as_str().map(|s| s.to_string()),
            to: recipients("to"),
            cc: recipients("cc"),
            attachments,
            scheduled_for: v["scheduledFor"].as_str().map(|s| s.to_string()),
        }
    }

    fn map_organization(v: &Value) -> HsOrganization {
        HsOrganization {
            remote_id: v["id"].as_i64().unwrap_or(0),
            name: v["name"].as_str().unwrap_or_default().to_string(),
            domains: v["domains"]
                .as_array()
                .map(|d| {
                    d.iter()
                        .filter_map(|x| x.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default(),
            created_at: v["createdAt"].as_str().map(|s| s.to_string()),
            updated_at: v["updatedAt"].as_str().map(|s| s.to_string()),
        }
    }
}

#[async_trait::async_trait]
impl HelpScoutProvider for RealHelpScoutProvider {
    fn kind(&self) -> &'static str {
        "real"
    }

    fn docs_configured(&self) -> bool {
        !self.credentials.docs_api_key.is_empty()
    }

    async fn get_me(&self) -> Result<HsUser> {
        let v = self
            .request_pri("/v2/users/me", "GET", None, PRIORITY_INTERACTIVE)
            .await?;
        Ok(Self::map_user(&v))
    }

    async fn list_mailboxes(&self) -> Result<Vec<HsMailbox>> {
        self.list_all_v2("/v2/mailboxes", "mailboxes", Self::map_mailbox)
            .await
    }

    async fn list_users(&self) -> Result<Vec<HsUser>> {
        self.list_all_v2("/v2/users", "users", Self::map_user).await
    }

    async fn list_teams(&self) -> Result<Vec<HsTeam>> {
        let v = self.request("/v2/teams", "GET", None).await?;
        Ok(v["_embedded"]["teams"]
            .as_array()
            .map(|a| {
                a.iter()
                    .map(|t| HsTeam {
                        remote_id: t["id"].as_i64().unwrap_or(0),
                        name: t["name"].as_str().unwrap_or_default().to_string(),
                        member_user_ids: t["_embedded"]["users"]
                            .as_array()
                            .map(|users| {
                                users
                                    .iter()
                                    .filter_map(|u| u["id"].as_i64())
                                    .collect::<Vec<i64>>()
                            })
                            .unwrap_or_default(),
                    })
                    .collect()
            })
            .unwrap_or_default())
    }

    async fn list_tags(&self) -> Result<Vec<HsTag>> {
        self.list_all_v2("/v2/tags", "tags", Self::map_tag).await
    }

    async fn list_conversations(&self, query: &ConversationQuery) -> Result<Page<HsConversation>> {
        let path = if let Some(cursor) = query.cursor.as_deref().filter(|c| c.starts_with('/')) {
            cursor.to_string()
        } else {
            let mut path = String::from("/v3/conversations?status=");
            path.push_str(query.status.as_deref().unwrap_or("all"));
            if let Some(mb) = query.mailbox_id {
                path.push_str(&format!("&inboxId={mb}"));
            }
            if let Some(since) = &query.modified_since {
                path.push_str(&format!("&modifiedSince={}", urlencode(since)));
            }
            if let Some(cursor) = &query.cursor {
                path.push_str(&format!("&cursor={}", urlencode(cursor)));
            }
            path
        };
        let v = self.request(&path, "GET", None).await?;
        Ok(Page {
            items: v["_embedded"]["conversations"]
                .as_array()
                .map(|a| a.iter().map(Self::map_conversation).collect())
                .unwrap_or_default(),
            next_cursor: hal_next_path(&v),
        })
    }

    async fn list_customers(&self, query: &CustomerQuery) -> Result<Page<HsCustomer>> {
        let path = if let Some(cursor) = query.cursor.as_deref().filter(|c| c.starts_with('/')) {
            cursor.to_string()
        } else {
            let mut path = String::from("/v3/customers");
            let mut sep = '?';
            if let Some(since) = &query.modified_since {
                path.push(sep);
                path.push_str(&format!("modifiedSince={}", urlencode(since)));
                sep = '&';
            }
            if let Some(cursor) = &query.cursor {
                path.push(sep);
                path.push_str(&format!("cursor={}", urlencode(cursor)));
            }
            path
        };
        let v = self.request(&path, "GET", None).await?;
        Ok(Page {
            items: v["_embedded"]["customers"]
                .as_array()
                .map(|a| a.iter().map(Self::map_customer).collect())
                .unwrap_or_default(),
            next_cursor: hal_next_path(&v),
        })
    }

    async fn list_beacon_chats(&self) -> Result<Vec<HsBeaconChat>> {
        Err(Error::Other(
            "Help Scout exposes no list endpoint for chats: they arrive via \
             beacon.chat webhooks and are fetched by id"
                .into(),
        ))
    }

    async fn list_docs(&self) -> Result<Vec<HsDocArticle>> {
        let mut out = Vec::new();
        for col in self.list_doc_collections().await? {
            for a in self.list_doc_articles(col.remote_id).await? {
                out.push(a);
            }
        }
        Ok(out)
    }

    async fn list_ratings(&self) -> Result<Vec<HsRating>> {
        Err(Error::Other(
            "Help Scout exposes no list endpoint for ratings: they arrive via \
             the satisfaction.ratings webhook and are fetched by id; the \
             happiness reports require a Plus/Pro plan"
                .into(),
        ))
    }

    async fn get_rating(&self, rating_id: i64) -> Result<Option<HsRating>> {
        let raw = match self
            .request_pri(
                &format!("/v2/ratings/{rating_id}"),
                "GET",
                None,
                PRIORITY_ANALYTICS,
            )
            .await
        {
            Ok(v) => v,
            Err(e) => {
                if is_not_found(&e) {
                    return Ok(None);
                }
                return Err(e);
            }
        };
        let customer = raw.get("customer").cloned().unwrap_or(Value::Null);
        let first = customer.get("firstName").and_then(|v| v.as_str());
        let last = customer.get("lastName").and_then(|v| v.as_str());
        let customer_name = match (first, last) {
            (Some(f), Some(l)) => Some(format!("{f} {l}")),
            (Some(f), None) => Some(f.to_string()),
            (None, Some(l)) => Some(l.to_string()),
            (None, None) => None,
        };
        Ok(Some(HsRating {
            remote_id: raw.get("id").and_then(|v| v.as_i64()).unwrap_or(0),
            conversation_id: raw.get("conversationId").and_then(|v| v.as_i64()),
            thread_id: raw.get("threadId").and_then(|v| v.as_i64()),
            rating: raw
                .get("rating")
                .and_then(|v| v.as_str())
                .map(str::to_string),
            comment: raw
                .get("comments")
                .and_then(|v| v.as_str())
                .map(str::to_string),
            customer_id: customer.get("id").and_then(|v| v.as_i64()),
            customer_name,
            user_id: raw.get("userId").and_then(|v| v.as_i64()).or_else(|| {
                raw.get("user")
                    .and_then(|u| u.get("id"))
                    .and_then(|v| v.as_i64())
            }),
            created_at: raw
                .get("createdAt")
                .and_then(|v| v.as_str())
                .map(str::to_string),
        }))
    }

    async fn get_report(&self, key: &str, start: &str, end: &str) -> Result<Option<HsReportRow>> {
        let Some(spec) = crate::reports_api::report_spec(key) else {
            return Err(Error::Config(format!(
                "Unknown Help Scout report key {key:?}"
            )));
        };
        let data = self
            .request_pri(
                &format!("{}?start={}&end={}", spec.path, start, end),
                "GET",
                None,
                PRIORITY_ANALYTICS,
            )
            .await?;
        Ok(Some(HsReportRow {
            key: spec.key.to_string(),
            name: spec.name.to_string(),
            source: "helpscout".to_string(),
            data,
        }))
    }

    async fn get_doc_article(&self, remote_id: String) -> Result<Option<HsDocArticle>> {
        if self.credentials.docs_api_key.is_empty() {
            return Ok(None);
        }
        let v = self
            .docs_request(&format!("/v1/articles/{remote_id}"))
            .await?;
        let article = v["article"].clone();
        if article.is_null() {
            return Ok(None);
        }
        Ok(Some(Self::doc_article_from_value(remote_id, &article)))
    }

    async fn upsert_doc_article_draft(
        &self,
        input: &crate::helpscout::DocArticleDraftInput,
    ) -> Result<String> {
        if self.credentials.docs_api_key.is_empty() {
            return Err(Error::Config(
                "Docs API publishing is unavailable on this connection - no Docs API key is \
                 configured."
                    .into(),
            ));
        }
        if let Some(remote_id) = &input.remote_id {
            self.docs_request_send(
                "PUT",
                &format!("/v1/articles/{remote_id}/drafts"),
                Some(serde_json::json!({ "text": input.text })),
            )
            .await?;
            return Ok(remote_id.clone());
        }
        let mut body = serde_json::json!({
            "collectionId": input.collection_id,
            "name": input.name,
            "text": input.text,
            "status": "notpublished",
        });
        if let Some(slug) = input.slug.as_deref().filter(|s| !s.trim().is_empty()) {
            body["slug"] = serde_json::json!(slug);
        }
        let v = self
            .docs_request_send("POST", "/v1/articles?reload=true", Some(body))
            .await?;
        let id = v["article"]["id"]
            .as_str()
            .or_else(|| v["id"].as_str())
            .map(|s| s.to_string())
            .filter(|s| !s.is_empty())
            .ok_or_else::<crate::error::Error, _>(|| {
                HsApiError {
                    status_code: 200,
                    message: "Docs API created the article but returned no id".into(),
                    friendly: "The article was created, but the response did not identify it. \
                               Re-sync the collection before publishing."
                        .into(),
                    retryable: false,
                }
                .into()
            })?;
        Ok(id)
    }

    async fn publish_doc_article(&self, remote_id: String, name: &str, text: &str) -> Result<()> {
        if self.credentials.docs_api_key.is_empty() {
            return Err(Error::Config(
                "Docs API publishing is unavailable on this connection - no Docs API key is \
                 configured."
                    .into(),
            ));
        }
        self.docs_request_send(
            "PUT",
            &format!("/v1/articles/{remote_id}"),
            Some(serde_json::json!({
                "name": name,
                "text": text,
                "status": "published",
            })),
        )
        .await?;
        Ok(())
    }

    async fn list_folders(&self, mailbox_id: i64) -> Result<Vec<HsFolder>> {
        let v = self
            .request(&format!("/v2/mailboxes/{mailbox_id}/folders"), "GET", None)
            .await?;
        Ok(v["_embedded"]["folders"]
            .as_array()
            .map(|a| a.iter().map(|f| Self::map_folder(f, mailbox_id)).collect())
            .unwrap_or_default())
    }

    async fn list_inbox_fields(&self, mailbox_id: i64) -> Result<Vec<HsField>> {
        let v = self
            .request(&format!("/v2/mailboxes/{mailbox_id}/fields"), "GET", None)
            .await?;
        Ok(v["_embedded"]["fields"]
            .as_array()
            .map(|a| a.iter().map(|f| Self::map_field(f, mailbox_id)).collect())
            .unwrap_or_default())
    }

    async fn list_saved_replies(&self, mailbox_id: i64) -> Result<Vec<HsSavedReply>> {
        self.list_all_v2(
            &format!("/v2/mailboxes/{mailbox_id}/saved-replies"),
            "savedReplies",
            |r| HsSavedReply {
                remote_id: r["id"].as_i64().unwrap_or(0),
                name: r["name"].as_str().unwrap_or_default().to_string(),
                preview: r["preview"].as_str().map(|s| s.to_string()),
                text: r["text"].as_str().map(|s| s.to_string()),
            },
        )
        .await
    }

    async fn create_saved_reply(&self, mailbox_id: i64, name: &str, text: &str) -> Result<i64> {
        let v = self
            .request_pri(
                &format!("/v2/mailboxes/{mailbox_id}/saved-replies"),
                "POST",
                Some(json!({ "name": name, "text": text })),
                PRIORITY_USER_SEND,
            )
            .await?;
        Ok(v["resourceId"]
            .as_i64()
            .or_else(|| v["id"].as_i64())
            .unwrap_or(0))
    }

    async fn update_saved_reply(
        &self,
        mailbox_id: i64,
        saved_reply_id: i64,
        name: &str,
        text: &str,
    ) -> Result<()> {
        self.request_pri(
            &format!("/v2/mailboxes/{mailbox_id}/saved-replies/{saved_reply_id}"),
            "PUT",
            Some(json!({ "name": name, "text": text })),
            PRIORITY_USER_SEND,
        )
        .await?;
        Ok(())
    }

    async fn delete_saved_reply(&self, mailbox_id: i64, saved_reply_id: i64) -> Result<()> {
        self.request_pri(
            &format!("/v2/mailboxes/{mailbox_id}/saved-replies/{saved_reply_id}"),
            "DELETE",
            None,
            PRIORITY_USER_SEND,
        )
        .await?;
        Ok(())
    }

    async fn list_workflows(&self) -> Result<Vec<HsWorkflow>> {
        self.list_all_v2("/v2/workflows", "workflows", |w| HsWorkflow {
            remote_id: w["id"].as_i64().unwrap_or(0),
            mailbox_id: w["mailboxId"].as_i64(),
            name: w["name"].as_str().unwrap_or_default().to_string(),
            kind: w["type"].as_str().unwrap_or("manual").to_string(),
            status: w["status"].as_str().unwrap_or("inactive").to_string(),
            sort_order: w["order"].as_i64().unwrap_or(0),
        })
        .await
    }

    async fn list_webhooks(&self) -> Result<Vec<HsWebhookConfig>> {
        let mut out: Vec<HsWebhookConfig> = Vec::new();
        let mut page_no: u32 = 1;
        loop {
            let v = self
                .request(&format!("/v2/webhooks?page={page_no}"), "GET", None)
                .await?;
            let embedded = v["_embedded"]["webhooks"]
                .as_array()
                .cloned()
                .unwrap_or_default();
            let n = embedded.len();
            for w in &embedded {
                out.push(HsWebhookConfig {
                    remote_id: w["id"].as_i64().unwrap_or(0),
                    url: w["url"].as_str().unwrap_or_default().to_string(),
                    events: w["events"]
                        .as_array()
                        .map(|e| {
                            e.iter()
                                .filter_map(|x| x.as_str().map(|s| s.to_string()))
                                .collect()
                        })
                        .unwrap_or_default(),
                    status: w["status"].as_str().unwrap_or("enabled").to_string(),
                });
            }
            let total_pages = v["page"]["totalPages"].as_u64().unwrap_or(1);
            let number = v["page"]["number"].as_u64().unwrap_or(1);
            if n == 0 || number >= total_pages {
                break;
            }
            page_no += 1;
            if page_no > 1_000 {
                break;
            }
        }
        Ok(out)
    }

    async fn create_webhook(
        &self,
        url: &str,
        events: &[String],
        secret: &str,
        label: &str,
    ) -> Result<i64> {
        let v = self
            .request(
                "/v2/webhooks",
                "POST",
                Some(json!({ "url": url, "events": events, "secret": secret, "label": label })),
            )
            .await?;
        Ok(v["resourceId"]
            .as_i64()
            .or_else(|| v["id"].as_i64())
            .or_else(|| v["webhook"]["id"].as_i64())
            .unwrap_or(0))
    }

    async fn delete_webhook(&self, remote_id: i64) -> Result<bool> {
        self.request(&format!("/v2/webhooks/{remote_id}"), "DELETE", None)
            .await?;
        Ok(true)
    }

    async fn get_chat(&self, chat_id: &str) -> Result<Option<crate::helpscout::HsChat>> {
        let res = self
            .request_pri(
                &format!("/v2/chats/{chat_id}"),
                "GET",
                None,
                PRIORITY_INTERACTIVE,
            )
            .await;
        match res {
            Ok(v) => Ok(crate::chats::parse_chat_object(&v)),
            Err(e) => {
                if is_not_found(&e) {
                    return Ok(None);
                }
                Err(e)
            }
        }
    }

    async fn send_chat_message(&self, chat_id: &str, message_id: &str, text: &str) -> Result<()> {
        let res = self
            .request_pri(
                &format!("/v2/chats/{chat_id}/messages/{message_id}"),
                "PUT",
                Some(json!({ "text": text })),
                PRIORITY_USER_SEND,
            )
            .await;
        match res {
            Ok(_) => Ok(()),
            Err(e) => {
                if let crate::error::Error::Other(boxed) = &e
                    && boxed
                        .downcast_ref::<HsApiError>()
                        .is_some_and(|hs| hs.status_code == 422)
                {
                    return Ok(());
                }
                Err(e)
            }
        }
    }

    async fn list_customer_property_definitions(&self) -> Result<Vec<HsPropertyDef>> {
        let mut out: Vec<HsPropertyDef> = Vec::new();
        let mut page_no: u32 = 1;
        loop {
            let v = self
                .request(
                    &format!("/v2/customer-properties?page={page_no}"),
                    "GET",
                    None,
                )
                .await?;
            let embedded = v["_embedded"]["customerProperties"]
                .as_array()
                .or_else(|| v["_embedded"]["properties"].as_array())
                .cloned()
                .unwrap_or_default();
            let n = embedded.len();
            out.extend(embedded.iter().map(Self::map_property_def));
            let total_pages = v["page"]["totalPages"].as_u64().unwrap_or(1);
            let number = v["page"]["number"].as_u64().unwrap_or(1);
            if n == 0 || number >= total_pages {
                break;
            }
            page_no += 1;
            if page_no > 1_000 {
                break;
            }
        }
        Ok(out)
    }

    async fn list_organization_property_definitions(&self) -> Result<Vec<HsPropertyDef>> {
        match self
            .request("/v2/organization-properties", "GET", None)
            .await
        {
            Ok(v) => Ok(v["_embedded"]["organizationProperties"]
                .as_array()
                .or_else(|| v["_embedded"]["properties"].as_array())
                .map(|a| a.iter().map(Self::map_property_def).collect())
                .unwrap_or_default()),
            Err(e) if is_not_found(&e) => Ok(Vec::new()),
            Err(e) => Err(e),
        }
    }

    async fn list_organizations(&self) -> Result<Vec<HsOrganization>> {
        self.list_all_v2("/v2/organizations", "organizations", Self::map_organization)
            .await
    }

    async fn get_conversation(&self, conversation_id: i64) -> Result<Option<HsConversation>> {
        let res = self
            .request_pri(
                &format!("/v3/conversations/{conversation_id}"),
                "GET",
                None,
                PRIORITY_INTERACTIVE,
            )
            .await;
        match res {
            Ok(v) => Ok(Some(Self::map_conversation(&v))),
            Err(e) => {
                if is_not_found(&e) {
                    return Ok(None);
                }
                Err(e)
            }
        }
    }

    async fn list_threads(&self, conversation_id: i64) -> Result<Vec<HsThread>> {
        let mut out: Vec<HsThread> = Vec::new();
        let mut path = format!("/v3/conversations/{conversation_id}/threads");
        let mut pages = 0u32;
        loop {
            pages += 1;
            let v = self.request(&path, "GET", None).await?;
            let embedded = v["_embedded"]["threads"]
                .as_array()
                .cloned()
                .unwrap_or_default();
            let n = embedded.len();
            out.extend(
                embedded
                    .iter()
                    .map(|t| Self::map_thread(t, conversation_id)),
            );
            match hal_next_path(&v) {
                Some(next) if n > 0 => {
                    if pages >= 200 {
                        tracing::warn!(
                            conversation_id,
                            pages,
                            "threads pagination cap reached — keeping the threads fetched so far"
                        );
                        break;
                    }
                    path = next;
                }
                _ => break,
            }
        }
        Ok(out)
    }

    async fn get_customer(&self, customer_id: i64) -> Result<Option<HsCustomer>> {
        let res = self
            .request(&format!("/v2/customers/{customer_id}"), "GET", None)
            .await;
        match res {
            Ok(v) => {
                let mut c = Self::map_customer(&v);
                if c.channels_all_empty()
                    && let Some(ch) = self.get_customer_channels(customer_id).await?
                {
                    c.emails = ch.emails;
                    c.phones = ch.phones;
                    c.chats = ch.chats;
                    c.websites = ch.websites;
                    c.social_profiles = ch.social_profiles;
                }
                Ok(Some(c))
            }
            Err(e) => {
                if is_not_found(&e) {
                    return Ok(None);
                }
                Err(e)
            }
        }
    }

    async fn get_customer_channels(&self, customer_id: i64) -> Result<Option<HsCustomer>> {
        let mut emails: Vec<Value> = Vec::new();
        let mut phones: Vec<Value> = Vec::new();
        let mut chats: Vec<Value> = Vec::new();
        let mut websites: Vec<Value> = Vec::new();
        let mut social: Vec<Value> = Vec::new();
        for (sub, slot) in [
            ("emails", 0usize),
            ("phones", 1),
            ("chats", 2),
            ("websites", 3),
            ("social-profiles", 4),
        ] {
            let res = self
                .request(&format!("/v2/customers/{customer_id}/{sub}"), "GET", None)
                .await;
            match res {
                Ok(v) => {
                    let arr = v["_embedded"][sub.replace('-', "_").as_str()]
                        .as_array()
                        .or_else(|| v["_embedded"][sub].as_array())
                        .cloned()
                        .unwrap_or_default();
                    match slot {
                        0 => emails = arr,
                        1 => phones = arr,
                        2 => chats = arr,
                        3 => websites = arr,
                        _ => social = arr,
                    }
                }
                Err(e) if is_not_found(&e) => {}
                Err(e) => return Err(e),
            }
        }
        Ok(Some(HsCustomer {
            remote_id: customer_id,
            emails: emails
                .iter()
                .map(|e| crate::helpscout::HsCustomerEmail {
                    value: e["value"].as_str().map(|s| s.to_string()),
                    kind: e["type"].as_str().map(|s| s.to_string()),
                })
                .collect(),
            phones: phones
                .iter()
                .map(|p| crate::helpscout::HsCustomerPhone {
                    value: p["value"].as_str().map(|s| s.to_string()),
                    kind: p["type"].as_str().map(|s| s.to_string()),
                })
                .collect(),
            chats: chats
                .iter()
                .map(|c| crate::helpscout::HsCustomerChat {
                    value: c["value"].as_str().map(|s| s.to_string()),
                    kind: c["type"].as_str().map(|s| s.to_string()),
                })
                .collect(),
            websites: websites
                .iter()
                .map(|w| crate::helpscout::HsCustomerWebsite {
                    value: w["value"].as_str().map(|s| s.to_string()),
                })
                .collect(),
            social_profiles: social
                .iter()
                .map(|s| crate::helpscout::HsCustomerSocialProfile {
                    value: s["value"].as_str().map(|x| x.to_string()),
                    kind: s["type"].as_str().map(|x| x.to_string()),
                })
                .collect(),
            ..HsCustomer::placeholder()
        }))
    }

    async fn get_user_status(&self, user_id: i64) -> Result<Option<HsUserStatus>> {
        let res = self
            .request(&format!("/v2/users/{user_id}/status"), "GET", None)
            .await;
        match res {
            Ok(v) => Ok(Some(HsUserStatus {
                user_id,
                email_status: v["email"]["status"].as_str().map(|s| s.to_string()),
                email_updated_at: v["email"]["updatedAt"].as_str().map(|s| s.to_string()),
                chat_status: v["chat"]["status"].as_str().map(|s| s.to_string()),
                mailbox_statuses: v["chat"]["mailboxStatuses"].clone(),
            })),
            Err(e) => {
                if is_not_found(&e) {
                    return Ok(None);
                }
                Err(e)
            }
        }
    }

    async fn list_system_users(&self) -> Result<Vec<HsUser>> {
        let mut out = Vec::new();
        let mut path = String::from("/v3/system-users");
        let mut pages = 0u32;
        loop {
            pages += 1;
            let v = self.request(&path, "GET", None).await?;
            let embedded = v["_embedded"]["system-users"]
                .as_array()
                .or_else(|| v["_embedded"]["users"].as_array())
                .cloned()
                .unwrap_or_default();
            let n = embedded.len();
            out.extend(embedded.iter().map(Self::map_user));
            match hal_next_path(&v) {
                Some(next) if n > 0 => {
                    if pages >= 50 {
                        tracing::warn!(pages, "system-users pagination cap reached");
                        break;
                    }
                    path = next;
                }
                _ => break,
            }
        }
        Ok(out)
    }

    async fn list_doc_collections(&self) -> Result<Vec<HsDocCollection>> {
        if self.credentials.docs_api_key.is_empty() {
            self.log_missing_docs_key();
            return Ok(Vec::new());
        }
        let mut out: Vec<HsDocCollection> = Vec::new();
        let mut page = 1u32;
        loop {
            let v = self
                .docs_request(&format!("/v1/collections?page={page}"))
                .await?;
            let (items, pages) = docs_list(&v, "collections");
            if items.is_empty() {
                break;
            }
            for c in &items {
                out.push(HsDocCollection {
                    remote_id: c["id"].as_str().unwrap_or_default().to_string(),
                    slug: c["slug"].as_str().map(|s| s.to_string()),
                    name: c["name"].as_str().unwrap_or_default().to_string(),
                    description: c["description"].as_str().map(|s| s.to_string()),
                    visibility: c["visibility"].as_str().map(|s| s.to_string()),
                    article_count: c["articleCount"].as_i64(),
                });
            }
            if page as u64 >= pages {
                break;
            }
            page += 1;
        }
        Ok(out)
    }

    async fn list_doc_categories(&self, collection_id: String) -> Result<Vec<HsDocCategory>> {
        if self.credentials.docs_api_key.is_empty() {
            self.log_missing_docs_key();
            return Ok(Vec::new());
        }
        let mut out: Vec<HsDocCategory> = Vec::new();
        let mut page = 1u32;
        loop {
            let v = self
                .docs_request(&format!(
                    "/v1/collections/{collection_id}/categories?page={page}"
                ))
                .await?;
            let (items, pages) = docs_list(&v, "categories");
            if items.is_empty() {
                break;
            }
            for c in &items {
                out.push(HsDocCategory {
                    remote_id: c["id"].as_str().unwrap_or_default().to_string(),
                    collection_id: collection_id.clone(),
                    slug: c["slug"].as_str().map(|s| s.to_string()),
                    name: c["name"].as_str().unwrap_or_default().to_string(),
                    sort_order: c["order"].as_i64(),
                });
            }
            if page as u64 >= pages {
                break;
            }
            page += 1;
        }
        Ok(out)
    }

    async fn list_doc_articles(&self, collection_id: String) -> Result<Vec<HsDocArticle>> {
        if self.credentials.docs_api_key.is_empty() {
            self.log_missing_docs_key();
            return Ok(Vec::new());
        }
        let mut refs: Vec<(String, Value)> = Vec::new();
        let mut page = 1u32;
        loop {
            let v = self
                .docs_request(&format!(
                    "/v1/collections/{collection_id}/articles?page={page}&pageSize=100&status=all"
                ))
                .await?;
            let (items, pages) = docs_list(&v, "articles");
            if items.is_empty() {
                break;
            }
            for art in &items {
                let Some(id) = art["id"]
                    .as_str()
                    .map(|s| s.to_string())
                    .filter(|s| !s.is_empty())
                else {
                    continue;
                };
                refs.push((id, art.clone()));
            }
            if page as u64 >= pages {
                break;
            }
            page += 1;
        }
        let mut changed: Vec<(String, Value)> = Vec::new();
        let mut unchanged_count = 0usize;
        {
            let conn = self.conn_lock();
            for (id, r) in refs {
                let remote_mod = r["modifiedDate"].as_str();
                let stored: Option<String> = conn
                    .query_row(
                        "SELECT remote_updated_at FROM docs_articles WHERE remote_id = ?1",
                        rusqlite::params![id],
                        |row| row.get(0),
                    )
                    .ok();
                let unchanged =
                    matches!((remote_mod, stored.as_deref()), (Some(a), Some(b)) if a == b);
                if unchanged {
                    unchanged_count += 1;
                } else {
                    changed.push((id, r));
                }
            }
        }
        if unchanged_count > 0 {
            tracing::debug!(
                operation = "docs_articles",
                unchanged = unchanged_count,
                to_fetch = changed.len(),
                "Docs articles: skipping already-synced unchanged articles"
            );
        }
        let mut refs = changed;
        if refs.len() > DOC_ARTICLE_FETCH_CAP {
            tracing::warn!(
                count = refs.len(),
                cap = DOC_ARTICLE_FETCH_CAP,
                operation = "docs_articles",
                "New/changed article count exceeds the per-pass fetch cap; the remainder syncs next pass (frontier advances each pass)"
            );
            refs.truncate(DOC_ARTICLE_FETCH_CAP);
        }
        let mut out: Vec<HsDocArticle> = Vec::new();
        for (id, r) in &refs {
            let full = match self.docs_request(&format!("/v1/articles/{id}")).await {
                Ok(v) => v["article"].clone(),
                Err(e) => {
                    tracing::warn!(article = id, error = %e, "Docs article fetch failed");
                    r.clone()
                }
            };
            out.push(HsDocArticle {
                remote_id: id.clone(),
                collection_id: collection_id.clone(),
                category_id: full["categories"]
                    .as_array()
                    .and_then(|a| a.first())
                    .and_then(|c| c["id"].as_str().map(|s| s.to_string())),
                number: full["number"].as_i64().or_else(|| r["number"].as_i64()),
                slug: full["slug"].as_str().map(|s| s.to_string()),
                name: full["name"]
                    .as_str()
                    .or_else(|| r["name"].as_str())
                    .unwrap_or_default()
                    .to_string(),
                status: full["status"].as_str().map(|s| s.to_string()),
                text: full["text"].as_str().map(|s| s.to_string()),
                views: full["viewCount"]
                    .as_i64()
                    .or_else(|| r["viewCount"].as_i64()),
                created_at: full["createdDate"].as_str().map(|s| s.to_string()),
                updated_at: full["modifiedDate"].as_str().map(|s| s.to_string()),
            });
        }
        Ok(out)
    }

    async fn create_reply_thread(&self, input: CreateThreadInput) -> Result<ThreadCreated> {
        let mut body = json!({ "text": input.text, "draft": input.draft });
        if let Some(customer_id) = input.customer_id {
            body["customer"] = json!({ "id": customer_id });
        }
        if !input.cc.is_empty() {
            body["cc"] = json!(input.cc);
        }
        if !input.bcc.is_empty() {
            body["bcc"] = json!(input.bcc);
        }
        if let Some(status_after) = &input.status_after {
            body["status"] = json!(status_after);
        }
        if let Some(assign_to) = input.assign_to {
            body["assignTo"] = json!(assign_to);
        }
        let res = self
            .request_pri(
                &format!("/v2/conversations/{}/reply", input.conversation_id),
                "POST",
                Some(body),
                PRIORITY_USER_SEND,
            )
            .await?;
        Ok(ThreadCreated {
            thread_id: res["resourceId"]
                .as_i64()
                .or_else(|| res["id"].as_i64())
                .unwrap_or_default(),
            conversation_id: input.conversation_id,
        })
    }

    async fn create_note_thread(&self, input: CreateThreadInput) -> Result<ThreadCreated> {
        let res = self
            .request_pri(
                &format!("/v2/conversations/{}/notes", input.conversation_id),
                "POST",
                Some(json!({ "text": input.text })),
                PRIORITY_USER_SEND,
            )
            .await?;
        Ok(ThreadCreated {
            thread_id: res["resourceId"]
                .as_i64()
                .or_else(|| res["id"].as_i64())
                .unwrap_or_default(),
            conversation_id: input.conversation_id,
        })
    }

    async fn update_conversation(
        &self,
        conversation_id: i64,
        patch: ConversationPatch,
    ) -> Result<bool> {
        let path = format!("/v2/conversations/{conversation_id}");
        if let Some(subject) = patch.subject {
            self.request(
                &path,
                "PATCH",
                Some(json!({ "op": "replace", "path": "/subject", "value": subject })),
            )
            .await?;
        }
        if let Some(status) = patch.status {
            self.request(
                &path,
                "PATCH",
                Some(json!({ "op": "replace", "path": "/status", "value": status })),
            )
            .await?;
        }
        if let Some(mailbox_id) = patch.mailbox_id {
            self.request(
                &path,
                "PATCH",
                Some(json!({ "op": "move", "path": "/mailboxId", "value": mailbox_id })),
            )
            .await?;
        }
        match patch.assign_to {
            Some(None) => {
                self.request(
                    &path,
                    "PATCH",
                    Some(json!({ "op": "remove", "path": "/assignTo" })),
                )
                .await?;
            }
            Some(Some(assign_to)) => {
                self.request(
                    &path,
                    "PATCH",
                    Some(json!({ "op": "replace", "path": "/assignTo", "value": assign_to })),
                )
                .await?;
            }
            None => {}
        }
        Ok(true)
    }

    async fn create_conversation(
        &self,
        input: CreateConversationInput,
    ) -> Result<ConversationCreated> {
        let mut threads = json!([{
            "type": "customer",
            "customer": { "id": input.customer_id },
            "text": input.body,
        }]);
        if !input.tags.is_empty() {
            threads[0]["tags"] = json!(input.tags);
        }
        let mut body = json!({
            "type": input.channel.as_deref().unwrap_or("email"),
            "status": input.status.as_deref().unwrap_or("open"),
            "subject": input.subject,
            "mailboxId": input.mailbox_id,
            "customer": { "id": input.customer_id },
            "threads": threads,
        });
        if !input.tags.is_empty() {
            body["tags"] = json!(input.tags);
        }
        let res = self
            .request_pri("/v2/conversations", "POST", Some(body), PRIORITY_USER_SEND)
            .await?;
        let conversation_id = res["resourceId"]
            .as_i64()
            .or_else(|| res["id"].as_i64())
            .unwrap_or(0);
        let number = res["number"].as_i64().unwrap_or(0);
        let thread_id = res["threads"]
            .as_array()
            .and_then(|a| a.first())
            .and_then(|t| t["id"].as_i64())
            .unwrap_or(0);
        if conversation_id == 0 {
            return Err(HsApiError {
                status_code: 200,
                message: "Help Scout created the conversation but returned no id".into(),
                friendly:
                    "Help Scout accepted the new conversation but did not return a conversation id. \
                     The recipient will be reconciled against the local mirror."
                        .into(),
                retryable: false,
            }
            .into());
        }
        Ok(ConversationCreated {
            conversation_id,
            number,
            thread_id,
        })
    }

    async fn update_tags(&self, conversation_id: i64, tags: Vec<String>) -> Result<bool> {
        self.request(
            &format!("/v2/conversations/{conversation_id}/tags"),
            "PUT",
            Some(json!({ "tags": tags })),
        )
        .await?;
        Ok(true)
    }

    async fn update_custom_fields(
        &self,
        conversation_id: i64,
        fields: Vec<(i64, Option<String>)>,
    ) -> Result<bool> {
        let body = json!({
            "fields": fields
                .iter()
                .map(|(id, value)| json!({ "id": id, "value": value.clone().unwrap_or_default() }))
                .collect::<Vec<_>>()
        });
        self.request(
            &format!("/v2/conversations/{conversation_id}/fields"),
            "PUT",
            Some(body),
        )
        .await?;
        Ok(true)
    }

    async fn snooze_conversation(
        &self,
        conversation_id: i64,
        snoozed_until: String,
        unsnooze_on_customer_reply: bool,
    ) -> Result<bool> {
        self.request(
            &format!("/v2/conversations/{conversation_id}/snooze"),
            "PUT",
            Some(json!({
                "snoozedUntil": snoozed_until,
                "unsnoozeOnCustomerReply": unsnooze_on_customer_reply,
            })),
        )
        .await?;
        Ok(true)
    }

    async fn unsnooze_conversation(&self, conversation_id: i64) -> Result<bool> {
        self.request(
            &format!("/v2/conversations/{conversation_id}/snooze"),
            "DELETE",
            None,
        )
        .await?;
        Ok(true)
    }

    async fn schedule_thread(
        &self,
        conversation_id: i64,
        thread_id: i64,
        scheduled_for: String,
        unschedule_on_customer_reply: bool,
    ) -> Result<bool> {
        self.request(
            &format!("/v2/conversations/{conversation_id}/threads/{thread_id}/schedule"),
            "PUT",
            Some(json!({
                "scheduledFor": scheduled_for,
                "unscheduleOnCustomerReply": unschedule_on_customer_reply,
                "sendAsCreator": false,
            })),
        )
        .await?;
        Ok(true)
    }

    async fn publish_scheduled_thread(&self, conversation_id: i64, thread_id: i64) -> Result<bool> {
        self.request(
            &format!("/v2/conversations/{conversation_id}/threads/{thread_id}/schedule"),
            "PATCH",
            Some(json!({ "op": "replace", "path": "/state", "value": "published" })),
        )
        .await?;
        Ok(true)
    }

    async fn delete_thread_schedule(&self, conversation_id: i64, thread_id: i64) -> Result<bool> {
        self.request(
            &format!("/v2/conversations/{conversation_id}/threads/{thread_id}/schedule"),
            "DELETE",
            None,
        )
        .await?;
        Ok(true)
    }

    async fn run_workflow(&self, workflow_id: i64, conversation_id: i64) -> Result<bool> {
        self.request(
            &format!("/v2/workflows/{workflow_id}/run"),
            "POST",
            Some(json!({ "conversationIds": [conversation_id] })),
        )
        .await?;
        Ok(true)
    }

    async fn get_attachment_data(
        &self,
        conversation_id: i64,
        _thread_id: i64,
        attachment_id: i64,
    ) -> Result<Option<crate::helpscout::AttachmentData>> {
        match self
            .request_pri(
                &format!("/v2/conversations/{conversation_id}/attachments/{attachment_id}/data"),
                "GET",
                None,
                PRIORITY_INDEXING,
            )
            .await
        {
            Ok(v) => {
                let Some(data) = v["data"].as_str() else {
                    return Ok(None);
                };
                use base64::Engine;
                let bytes = base64::engine::general_purpose::STANDARD
                    .decode(data)
                    .map_err(|e| {
                        crate::error::Error::Other(format!("attachment data: {e}").into())
                    })?;
                Ok(Some(crate::helpscout::AttachmentData {
                    data: bytes,
                    mime_type: None,
                    filename: None,
                }))
            }
            Err(e) => {
                if crate::helpscout_real::hs_status(&e) == Some(404) {
                    Ok(None)
                } else {
                    Err(e)
                }
            }
        }
    }

    async fn get_routing_configuration(
        &self,
        mailbox_id: i64,
    ) -> Result<Option<serde_json::Value>> {
        match self
            .request(&format!("/v2/mailboxes/{mailbox_id}/routing"), "GET", None)
            .await
        {
            Ok(v) => Ok(Some(v)),
            Err(e) => {
                if crate::helpscout_real::hs_status(&e) == Some(404) {
                    Ok(None)
                } else {
                    Err(e)
                }
            }
        }
    }

    async fn ping(&self) -> Result<bool> {
        self.get_me().await?;
        Ok(true)
    }
}

impl RealHelpScoutProvider {
    fn map_property_def(v: &Value) -> HsPropertyDef {
        HsPropertyDef {
            remote_id: v["id"].as_i64().unwrap_or(0),
            name: v["name"].as_str().unwrap_or_default().to_string(),
            slug: v["slug"].as_str().map(|s| s.to_string()),
            kind: v["type"].as_str().unwrap_or("text").to_string(),
            sort_order: v["order"].as_i64().unwrap_or(0),
        }
    }

    async fn list_all_v2<T>(
        &self,
        base: &str,
        embedded_key: &str,
        map: impl Fn(&Value) -> T,
    ) -> Result<Vec<T>> {
        let mut out: Vec<T> = Vec::new();
        let mut page_no: u32 = 1;
        loop {
            let sep = if base.contains('?') { '&' } else { '?' };
            let v = self
                .request(&format!("{base}{sep}page={page_no}"), "GET", None)
                .await?;
            let embedded = v["_embedded"][embedded_key]
                .as_array()
                .cloned()
                .unwrap_or_default();
            let n = embedded.len();
            out.extend(embedded.iter().map(&map));
            let total_pages = v["page"]["totalPages"].as_u64().unwrap_or(1);
            let number = v["page"]["number"].as_u64().unwrap_or(1);
            if n == 0 || number >= total_pages {
                break;
            }
            page_no += 1;
            if page_no > 1_000 {
                break;
            }
        }
        Ok(out)
    }
}

fn docs_list(v: &Value, key: &str) -> (Vec<Value>, u64) {
    let scope = v.get(key).filter(|s| s.is_object()).unwrap_or(v);
    let items = scope["items"].as_array().cloned().unwrap_or_default();
    let pages = scope["pages"].as_u64().unwrap_or(1);
    (items, pages)
}

fn hal_next_path(v: &Value) -> Option<String> {
    if let Some(href) = v["_links"]["next"]["href"]
        .as_str()
        .filter(|h| !h.is_empty())
    {
        let path = if let Some(pos) = href.find("://") {
            let rest = &href[pos + 3..];
            let slash = rest.find('/')?;
            &rest[slash..]
        } else {
            href
        };
        if path.starts_with('/') {
            return Some(path.to_string());
        }
        return None;
    }
    v["page"]["nextCursor"]
        .as_str()
        .filter(|c| !c.is_empty())
        .map(|c| c.to_string())
}

fn is_not_found(e: &Error) -> bool {
    hs_status(e) == Some(404) || e.to_string().contains("-> 404")
}

pub(crate) fn hs_status(e: &Error) -> Option<u16> {
    if let Error::Other(b) = e
        && let Some(hs) = b.downcast_ref::<HsApiError>()
    {
        return Some(hs.status_code);
    }
    None
}

pub(crate) fn hs_error(e: &Error) -> Option<&HsApiError> {
    if let Error::Other(b) = e {
        b.downcast_ref::<HsApiError>()
    } else {
        None
    }
}

fn ensure_account_row(conn: &Connection) {
    let has_account_col: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('oauth_tokens') WHERE name = 'account'",
            [],
            |r| r.get::<_, i64>(0),
        )
        .map(|n| n > 0)
        .unwrap_or(false);
    if !has_account_col {
        let _ = conn.execute_batch(
            "ALTER TABLE oauth_tokens ADD COLUMN account TEXT;
             ALTER TABLE oauth_tokens ADD COLUMN revoked INTEGER DEFAULT 0;
             UPDATE oauth_tokens SET account = 'default' WHERE account IS NULL;",
        );
    }
    let _ = conn.execute_batch(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_oauth_tokens_account ON oauth_tokens(account);",
    );
}

#[cfg(test)]
mod credentials_tests {
    use super::*;

    fn migrated_conn() -> Connection {
        let mut conn = Connection::open_in_memory().expect("in-memory db");
        crate::bootstrap::apply_all(&mut conn).expect("migrations");
        conn
    }

    fn install_key() {
        crate::settings::clear_master_key_for_tests();
        let path = std::env::temp_dir().join(format!(
            "spp-hscreds-key-{}-{}",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        let _ = std::fs::remove_file(&path);
        crate::settings::init_master_key(&path).expect("master key");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn credentials_envelope_lifecycle() {
        let _key_guard = crate::settings::SECRETS_TEST_LOCK
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        install_key();
        let conn = migrated_conn();

        let env_none = HsCredentials::default();
        let (resolved, sources) = HsCredentials::resolve_with(&env_none, &conn);
        assert!(!resolved.is_configured());
        assert_eq!(sources.client_id, CredentialSource::NotSet);
        assert_eq!(sources.client_secret, CredentialSource::NotSet);
        assert_eq!(sources.webhook_secret, CredentialSource::NotSet);
        assert_eq!(sources.docs_api_key, CredentialSource::NotSet);

        HsCredentials::store_in_envelope(&conn, "cid-1", "csecret-1", "wh-1", "").expect("store");
        let (r2, s2) = HsCredentials::resolve_with(&env_none, &conn);
        assert!(r2.is_configured());
        assert_eq!(r2.client_id, "cid-1");
        assert_eq!(r2.client_secret, "csecret-1");
        assert_eq!(r2.webhook_secret, "wh-1");
        assert_eq!(r2.docs_api_key, "");
        assert_eq!(s2.client_id, CredentialSource::AppSettings);
        assert_eq!(s2.client_secret, CredentialSource::AppSettings);
        assert_eq!(s2.webhook_secret, CredentialSource::AppSettings);
        assert_eq!(s2.docs_api_key, CredentialSource::NotSet);

        let blob: Vec<u8> = conn
            .query_row(
                "SELECT value FROM secrets WHERE key = 'helpscout_client_secret'",
                [],
                |r| r.get(0),
            )
            .expect("blob");
        assert!(!blob.windows(8).any(|w| w == b"csecret-1"));

        let env_some = HsCredentials {
            client_id: "env-cid".into(),
            webhook_secret: "env-wh".into(),
            ..HsCredentials::default()
        };
        let (r3, s3) = HsCredentials::resolve_with(&env_some, &conn);
        assert_eq!(r3.client_id, "env-cid");
        assert_eq!(s3.client_id, CredentialSource::Environment);
        assert_eq!(s3.webhook_secret, CredentialSource::Environment);
        assert_eq!(r3.webhook_secret, "env-wh");
        assert_eq!(s3.client_secret, CredentialSource::AppSettings);
        assert_eq!(r3.client_secret, "csecret-1");

        let removed = HsCredentials::clear_envelope(&conn).expect("clear");
        assert_eq!(
            removed, 3,
            "client id + secret + webhook (docs never stored)"
        );
        let (r4, s4) = HsCredentials::resolve_with(&env_none, &conn);
        assert!(!r4.is_configured());
        assert_eq!(s4.client_id, CredentialSource::NotSet);
        assert_eq!(s4.client_secret, CredentialSource::NotSet);
        assert_eq!(s4.webhook_secret, CredentialSource::NotSet);
    }

    #[test]
    fn debug_output_never_contains_credential_values() {
        let creds = HsCredentials {
            client_id: "client-id-abc".into(),
            client_secret: "TOPSECRETVALUE1".into(),
            redirect_uri: "http://localhost:3000/oauth/callback".into(),
            api_base: "https://api.helpscout.net".into(),
            webhook_secret: "WHSECVALUE1".into(),
            docs_api_key: "DOCSKEYVALUE1".into(),
            docs_api_base: "https://docsapi.helpscout.net".into(),
        };
        let rendered = format!("{creds:?}");
        assert!(!rendered.contains("TOPSECRETVALUE1"), "{rendered}");
        assert!(!rendered.contains("WHSECVALUE1"), "{rendered}");
        assert!(!rendered.contains("DOCSKEYVALUE1"), "{rendered}");
        assert!(rendered.contains("<redacted 15 chars>"), "{rendered}");
        assert!(rendered.contains("client-id-abc"), "{rendered}");
    }
}

#[cfg(test)]
mod r6_honest_stub_tests {
    use super::*;

    fn rt() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime")
    }

    #[test]
    fn list_ratings_and_chats_state_the_platform_reality() {
        let conn = Connection::open_in_memory().expect("db");
        let provider =
            RealHelpScoutProvider::new(crate::db::DbHandle::new(conn), HsCredentials::default());
        let runtime = rt();
        let ratings_err = runtime
            .block_on(provider.list_ratings())
            .expect_err("list_ratings must refuse to pretend the platform lists ratings");
        assert!(
            ratings_err
                .to_string()
                .contains("no list endpoint for ratings"),
            "unexpected message: {ratings_err}"
        );
        let chats_err = runtime
            .block_on(provider.list_beacon_chats())
            .expect_err("list_beacon_chats must refuse to pretend the platform lists chats");
        assert!(
            chats_err.to_string().contains("no list endpoint for chats"),
            "unexpected message: {chats_err}"
        );
    }
}

pub(crate) fn apply_m053_oauth_token_envelope(conn: &Connection) -> Result<()> {
    if !crate::sync_schema::table_exists(conn, "oauth_tokens")? {
        return Ok(());
    }
    if crate::settings::master_key_available() {
        let mut stmt = conn.prepare(
            "SELECT id, access_token, refresh_token FROM oauth_tokens
              WHERE COALESCE(access_token, '') != '' OR refresh_token IS NOT NULL",
        )?;
        let rows: Vec<(i64, Option<String>, Option<String>)> = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .filter_map(|r| r.ok())
            .collect();
        drop(stmt);
        for (id, access, refresh) in rows {
            if let Some(access) = access.filter(|t| !t.is_empty()) {
                crate::settings::set_secret(conn, SECRET_OAUTH_ACCESS, access.as_bytes())?;
            }
            if let Some(refresh) = refresh.filter(|t| !t.is_empty()) {
                crate::settings::set_secret(conn, SECRET_OAUTH_REFRESH, refresh.as_bytes())?;
            }
            conn.execute(
                "UPDATE oauth_tokens SET access_token = '', refresh_token = NULL WHERE id = ?1",
                rusqlite::params![id],
            )?;
            tracing::info!(
                row = id,
                "m053: sealed OAuth tokens into the encrypted envelope"
            );
        }
    } else {
        tracing::warn!(
            "secrets master key not initialized; OAuth tokens stay in the \
             legacy columns until the next boot with a key (m053 pending)"
        );
    }
    Ok(())
}

#[cfg(test)]
mod r7_token_envelope_tests {
    use super::*;

    fn conn_with_tokens() -> Connection {
        let mut conn = Connection::open_in_memory().expect("in-memory db");
        crate::bootstrap::apply_all(&mut conn).expect("migrations");
        ensure_account_row(&conn);
        conn.execute(
            "INSERT INTO oauth_tokens (id, account, access_token, refresh_token, token_type, revoked)
             VALUES (1, 'default', 'legacy-plain', 'legacy-refresh', 'bearer', 0)",
            [],
        )
        .expect("seed legacy plaintext tokens");
        conn
    }

    #[test]
    fn m053_seals_legacy_plaintext_tokens_and_scrubs_the_row() {
        let _g = crate::settings::SECRETS_TEST_LOCK
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        crate::settings::set_master_key_for_tests([7u8; 32]);
        let conn = conn_with_tokens();

        apply_m053_oauth_token_envelope(&conn).expect("m053");

        let access = crate::settings::get_secret(&conn, SECRET_OAUTH_ACCESS)
            .expect("envelope read")
            .map(|b| String::from_utf8(b).expect("utf8"));
        let refresh = crate::settings::get_secret(&conn, SECRET_OAUTH_REFRESH)
            .expect("envelope read")
            .map(|b| String::from_utf8(b).expect("utf8"));
        assert_eq!(access.as_deref(), Some("legacy-plain"));
        assert_eq!(refresh.as_deref(), Some("legacy-refresh"));
        let (column_access, column_refresh): (String, Option<String>) = conn
            .query_row(
                "SELECT access_token, refresh_token FROM oauth_tokens WHERE account = 'default'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .expect("row");
        assert_eq!(column_access, "", "m053 must scrub the plaintext column");
        assert_eq!(column_refresh, None, "m053 must scrub the plaintext column");
        let stored: i64 = conn
            .query_row("SELECT COUNT(*) FROM oauth_tokens", [], |r| r.get(0))
            .expect("token count");
        assert!(stored > 0, "probe");
    }

    #[test]
    fn m053_without_a_key_leaves_the_legacy_row_intact() {
        let _g = crate::settings::SECRETS_TEST_LOCK
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        crate::settings::clear_master_key_for_tests();
        let conn = conn_with_tokens();

        apply_m053_oauth_token_envelope(&conn).expect("m053 (skip)");

        let (column_access, _): (String, Option<String>) = conn
            .query_row(
                "SELECT access_token, refresh_token FROM oauth_tokens WHERE account = 'default'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .expect("row");
        assert_eq!(
            column_access, "legacy-plain",
            "without a key the row must be left exactly as it was"
        );
        let stored: i64 = conn
            .query_row("SELECT COUNT(*) FROM oauth_tokens", [], |r| r.get(0))
            .expect("token count");
        assert!(stored > 0, "probe");
        crate::settings::set_master_key_for_tests([7u8; 32]);
    }
}

#[cfg(test)]
mod batch22_audit_tests {
    use super::*;

    fn env_default_concurrency() -> usize {
        crate::config::load_env_config().sync.api_concurrency.max(1) as usize
    }

    fn migrated_conn() -> Connection {
        let mut conn = Connection::open_in_memory().expect("in-memory db");
        crate::bootstrap::apply_all(&mut conn).expect("migrations");
        conn
    }

    fn provider_with(conn: Connection, setting: Option<i64>) -> RealHelpScoutProvider {
        if let Some(n) = setting {
            crate::settings::set_i64(&conn, "api_concurrency", n).expect("store setting");
        }
        RealHelpScoutProvider::new(crate::db::DbHandle::new(conn), HsCredentials::default())
    }

    #[test]
    fn api_concurrency_setting_reaches_both_provider_queues() {
        let p = provider_with(migrated_conn(), None);
        assert_eq!(p.api_concurrency(), env_default_concurrency());
        assert_eq!(
            p.docs_queue.concurrency(),
            env_default_concurrency(),
            "the docs queue follows the same default"
        );

        let p = provider_with(migrated_conn(), Some(5));
        assert_eq!(p.api_concurrency(), 5);
        assert_eq!(
            p.docs_queue.concurrency(),
            5,
            "the docs queue follows the setting"
        );

        p.set_api_concurrency(3);
        assert_eq!(p.api_concurrency(), 3);
        assert_eq!(
            p.docs_queue.concurrency(),
            3,
            "the live resize covers the docs queue"
        );

        let p = provider_with(migrated_conn(), Some(0));
        assert_eq!(p.api_concurrency(), 1, "stored 0 clamps to 1");

        let conn = migrated_conn();
        crate::settings::set_string(&conn, "api_concurrency", "not-a-number").expect("garbage");
        let p =
            RealHelpScoutProvider::new(crate::db::DbHandle::new(conn), HsCredentials::default());
        assert_eq!(
            p.api_concurrency(),
            env_default_concurrency(),
            "garbage falls back to the default"
        );
    }

    #[test]
    fn map_conversation_keeps_team_ids_out_of_the_user_slot() {
        let team_assigned = serde_json::json!({
            "id": 101, "number": 7, "status": "active", "mailboxId": 1,
            "assignee": {"id": 42, "type": "team"},
            "primaryCustomer": {"id": 900}
        });
        let c = RealHelpScoutProvider::map_conversation(&team_assigned);
        assert_eq!(c.assignee_id, None, "the user slot stays empty for a team");
        assert_eq!(c.assignee_type.as_deref(), Some("team"));
        assert_eq!(
            c.assigned_team_id,
            Some(42),
            "the team id rides its own field"
        );

        let user_assigned = serde_json::json!({
            "id": 102, "number": 8, "status": "active", "mailboxId": 1,
            "assignee": {"id": 55, "type": "user"},
            "primaryCustomer": {"id": 900}
        });
        let c = RealHelpScoutProvider::map_conversation(&user_assigned);
        assert_eq!(c.assignee_id, Some(55));
        assert_eq!(c.assigned_team_id, None);

        let both = serde_json::json!({
            "id": 103, "number": 9, "status": "active", "mailboxId": 1,
            "assignee": {"id": 55, "type": "user"},
            "assignedTeam": {"id": 42},
            "primaryCustomer": {"id": 900}
        });
        let c = RealHelpScoutProvider::map_conversation(&both);
        assert_eq!(c.assignee_id, Some(55));
        assert_eq!(c.assigned_team_id, Some(42));

        let team_only = serde_json::json!({
            "id": 104, "number": 10, "status": "active", "mailboxId": 1,
            "assignedTeam": {"id": 77},
            "primaryCustomer": {"id": 900}
        });
        let c = RealHelpScoutProvider::map_conversation(&team_only);
        assert_eq!(c.assignee_id, None);
        assert_eq!(c.assigned_team_id, Some(77));
    }
}
