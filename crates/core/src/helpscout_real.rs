use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use rusqlite::Connection;
use serde_json::{Value, json};

use crate::api_queue::{
    ApiQueue, DEFAULT_CONCURRENCY, PRIORITY_ANALYTICS, PRIORITY_INDEXING, PRIORITY_INTERACTIVE,
    PRIORITY_SYNC, PRIORITY_USER_SEND,
};
use crate::error::{Error, Result};

use crate::helpscout::{
    ConversationCreated, ConversationPatch, ConversationQuery, CreateConversationInput,
    CreateThreadInput, CustomerQuery, HelpScoutProvider, HsBeaconChat, HsConversation, HsCustomer,
    HsDocArticle, HsDocCategory, HsDocCollection, HsField, HsFieldOption, HsFolder, HsMailbox,
    HsOrganization, HsPropertyDef, HsRating, HsReportRow, HsSavedReply, HsTag, HsTeam, HsThread,
    HsUser, HsUserStatus, HsWebhookConfig, HsWorkflow, Page, ThreadCreated,
};

pub const HS_API_BASE: &str = "https://api.helpscout.net";
pub const HS_DOCS_API_BASE: &str = "https://docsapi.helpscout.net";
pub const HS_AUTHORIZE_URL: &str =
    "https://secure.helpscout.net/authentication/authorizeClientApplication";

#[derive(Debug, Clone)]
pub struct HsApiError {
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

pub fn friendly_error(status: u16, body: &str, method: &str) -> String {
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

pub fn apply_m043(conn: &Connection) -> Result<()> {
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
pub struct RateLimitState {
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
    conn: Option<Arc<Mutex<Connection>>>,
}

impl HsRateLimiter {
    pub fn new(conn: Option<Arc<Mutex<Connection>>>) -> Self {
        let limiter = Self {
            state: Mutex::new(RateLimitState::default()),
            send_timestamps: Mutex::new(Vec::new()),
            conn,
        };
        limiter.load();
        limiter
    }

    fn load(&self) {
        let Some(conn) = &self.conn else { return };
        let Ok(conn) = conn.lock() else { return };
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
        let Some(conn) = &self.conn else { return };
        let Ok(conn) = conn.lock() else { return };
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

    pub fn record_response(
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

    pub fn record_error_429(&self, retry_after_sec: Option<i64>) {
        let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
        state.retry_after_sec = Some(retry_after_sec.unwrap_or(30));
        state.remaining = Some(0);
        state.updated_at = chrono::Utc::now().to_rfc3339();
        self.persist(&state);
    }

    pub fn wait_time_ms(&self, _is_write: bool) -> i64 {
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

const TOKEN_EXPIRY_MARGIN_SECS: i64 = 120;

struct TokenRow {
    access_token: Option<String>,
    refresh_token: Option<String>,
    expires_at: Option<String>,
    revoked: bool,
}

fn token_row(conn: &Connection) -> Option<TokenRow> {
    conn.query_row(
        "SELECT access_token, refresh_token, expires_at, revoked FROM oauth_tokens WHERE account = 'default'",
        [],
        |r| {
            Ok(TokenRow {
                access_token: r.get(0)?,
                refresh_token: r.get(1)?,
                expires_at: r.get(2)?,
                revoked: r.get::<_, i64>(3)? != 0,
            })
        },
    )
    .ok()
}

#[derive(Debug, Clone, Default)]
pub struct HsCredentials {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
    pub api_base: String,
    pub webhook_secret: String,
    pub docs_api_key: String,
    pub docs_api_base: String,
}

impl HsCredentials {
    pub fn from_env() -> Self {
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
    conn: Arc<Mutex<Connection>>,
    pub credentials: HsCredentials,
    pub limiter: Arc<HsRateLimiter>,
    pub queue: Arc<ApiQueue>,
    docs_queue: Arc<ApiQueue>,
}

impl RealHelpScoutProvider {
    #[must_use]
    pub fn new(conn: Arc<Mutex<Connection>>, credentials: HsCredentials) -> Self {
        let limiter = Arc::new(HsRateLimiter::new(Some(conn.clone())));
        let queue = ApiQueue::new(Arc::clone(&limiter), DEFAULT_CONCURRENCY);
        let docs_queue = ApiQueue::new(
            Arc::new(HsRateLimiter::new(Some(conn.clone()))),
            DEFAULT_CONCURRENCY,
        );
        Self {
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(60))
                .build()
                .unwrap_or_default(),
            api_base: credentials.api_base.trim_end_matches('/').to_string(),
            conn,
            limiter,
            queue,
            docs_queue,
            credentials,
        }
    }

    pub fn set_api_concurrency(&self, n: usize) {
        self.queue.set_concurrency(n);
    }

    fn conn_lock(&self) -> MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(|p| p.into_inner())
    }

    async fn docs_request(&self, path: &str) -> Result<Value> {
        if self.credentials.docs_api_key.is_empty() {
            return Ok(Value::Null);
        }
        let path = path.to_string();
        let fut = self.docs_request_inner(&path);
        self.docs_queue.enqueue(PRIORITY_SYNC, false, fut).await
    }

    async fn docs_request_inner(&self, path: &str) -> Result<Value> {
        let basic = base64_of(&format!("{}:X", self.credentials.docs_api_key));
        let url = format!(
            "{}{path}",
            self.credentials.docs_api_base.trim_end_matches('/')
        );
        let res = self
            .client
            .get(url)
            .header("Authorization", format!("Basic {basic}"))
            .send()
            .await
            .map_err(|e| -> crate::error::Error {
                HsApiError {
                    status_code: 0,
                    message: format!("Network error contacting the Docs API: {e}"),
                    friendly: "The Help Scout Docs API is unreachable. The docs mirror stays empty; everything else keeps working.".into(),
                    retryable: true,
                }
                .into()
            })?;
        let status = res.status().as_u16();
        let text = res.text().await.unwrap_or_default();
        if (200..300).contains(&status) {
            if text.is_empty() {
                return Ok(Value::Null);
            }
            return serde_json::from_str(&text).map_err(|_| {
                HsApiError {
                    status_code: status,
                    message: "Invalid JSON from the Docs API".into(),
                    friendly: "The Docs API returned an unexpected response format.".into(),
                    retryable: false,
                }
                .into()
            });
        }
        Err(HsApiError {
            status_code: status,
            message: format!("Docs API GET {path} -> {status}"),
            friendly: friendly_error(status, &text, "GET"),
            retryable: false,
        }
        .into())
    }

    async fn access_token(&self) -> Result<String> {
        let row = {
            let conn = self.conn_lock();
            ensure_account_row(&conn);
            token_row(&conn)
        };
        let Some(row) = row else {
            return Err(HsApiError {
                status_code: 401,
                message: "No Help Scout token available".into(),
                friendly: friendly_error(401, "", "GET"),
                retryable: false,
            }
            .into());
        };
        if row.revoked || row.access_token.as_deref().unwrap_or("").is_empty() {
            return Err(HsApiError {
                status_code: 401,
                message: "No Help Scout token available".into(),
                friendly: friendly_error(401, "", "GET"),
                retryable: false,
            }
            .into());
        }
        let expires_soon = match row.expires_at.as_deref() {
            Some(exp) => match chrono::DateTime::parse_from_rfc3339(exp) {
                Ok(t) => t.timestamp() < chrono::Utc::now().timestamp() + TOKEN_EXPIRY_MARGIN_SECS,
                Err(_) => true,
            },
            None => true,
        };
        if !expires_soon {
            return Ok(row.access_token.unwrap_or_default());
        }
        if let Some(refresh) = row.refresh_token
            && let Ok(token) = self.refresh_tokens(&refresh).await
        {
            return Ok(token);
        }
        Ok(row.access_token.unwrap_or_default())
    }

    pub async fn token_request(
        &self,
        form: &[(&str, &str)],
    ) -> std::result::Result<Value, HsApiError> {
        let mut form_data: Vec<(&str, &str)> = form.to_vec();
        form_data.push(("client_id", &self.credentials.client_id));
        form_data.push(("client_secret", &self.credentials.client_secret));
        let res = self
            .client
            .post(format!("{}/v2/oauth2/token", self.api_base))
            .timeout(Duration::from_secs(20))
            .form(&form_data)
            .send()
            .await
            .map_err(|e| HsApiError {
                status_code: 0,
                message: format!("OAuth token request failed: {e}"),
                friendly: "Help Scout rejected the login credentials. Check your Client ID/Secret in Settings, then try connecting again.".into(),
                retryable: false,
            })?;
        let status = res.status().as_u16();
        if !(200..300).contains(&status) {
            return Err(HsApiError {
                status_code: status,
                message: format!("OAuth token request failed ({status})"),
                friendly: "Help Scout rejected the login credentials. Check your Client ID/Secret in Settings, then try connecting again.".into(),
                retryable: false,
            });
        }
        res.json::<Value>()
            .await
            .map_err(|e| HsApiError {
                status_code: 0,
                message: format!("invalid token response: {e}"),
                friendly: "Help Scout returned an unexpected response format. The raw response was preserved for diagnostics.".into(),
                retryable: false,
            })
    }

    pub fn save_tokens(
        &self,
        access_token: &str,
        refresh_token: Option<&str>,
        expires_in: Option<i64>,
        scope: Option<&str>,
    ) -> Result<()> {
        let expires_at =
            chrono::Utc::now() + chrono::Duration::seconds(expires_in.unwrap_or(172_800));
        let expires_iso = expires_at.to_rfc3339();
        let keep_refresh: Option<String> = refresh_token.map(|s| s.to_string());
        let conn = self.conn_lock();
        ensure_account_row(&conn);
        conn.execute(
            "INSERT INTO oauth_tokens (account, access_token, refresh_token, token_type, expires_at, obtained_at, scope, revoked)
             VALUES ('default', ?1, ?2, 'bearer', ?3, datetime('now'), ?4, 0)
             ON CONFLICT(account) DO UPDATE SET access_token = excluded.access_token,
               refresh_token = COALESCE(excluded.refresh_token, refresh_token),
               expires_at = excluded.expires_at, obtained_at = datetime('now'),
               scope = excluded.scope, revoked = 0",
            rusqlite::params![access_token, keep_refresh, expires_iso, scope],
        )?;
        let _ = conn.execute(
            "UPDATE oauth_tokens SET access_token = ?1, refresh_token = ?2, expires_at = ?3, revoked = 0
              WHERE account = 'default'",
            rusqlite::params![access_token, keep_refresh, expires_iso],
        );
        Ok(())
    }

    pub async fn refresh_tokens(&self, refresh_token: &str) -> Result<String> {
        let v = self
            .token_request(&[
                ("grant_type", "refresh_token"),
                ("refresh_token", refresh_token),
            ])
            .await
            .map_err(Error::from)?;
        let access = v["access_token"].as_str().unwrap_or_default().to_string();
        let refresh = v["refresh_token"].as_str().map(|s| s.to_string());
        let expires = v["expires_in"].as_i64();
        let scope = v["scope"].as_str().map(|s| s.to_string());
        self.save_tokens(&access, refresh.as_deref(), expires, scope.as_deref())?;
        Ok(access)
    }

    pub async fn client_credentials_login(&self) -> Result<String> {
        let v = self
            .token_request(&[("grant_type", "client_credentials")])
            .await
            .map_err(Error::from)?;
        let access = v["access_token"].as_str().unwrap_or_default().to_string();
        let expires = v["expires_in"].as_i64();
        let scope = v["scope"].as_str().map(|s| s.to_string());
        self.save_tokens(&access, None, expires, scope.as_deref())?;
        Ok(access)
    }

    pub async fn exchange_code(&self, code: &str) -> Result<String> {
        let v = self
            .token_request(&[("grant_type", "authorization_code"), ("code", code)])
            .await
            .map_err(Error::from)?;
        let access = v["access_token"].as_str().unwrap_or_default().to_string();
        let refresh = v["refresh_token"].as_str().map(|s| s.to_string());
        let expires = v["expires_in"].as_i64();
        let scope = v["scope"].as_str().map(|s| s.to_string());
        self.save_tokens(&access, refresh.as_deref(), expires, scope.as_deref())?;
        Ok(access)
    }

    pub fn revoke(&self) -> Result<()> {
        let conn = self.conn_lock();
        ensure_account_row(&conn);
        conn.execute(
            "UPDATE oauth_tokens SET revoked = 1, access_token = '', refresh_token = NULL
              WHERE account = 'default'",
            [],
        )?;
        Ok(())
    }

    pub fn oauth_status(&self, demo_mode: bool) -> Value {
        let conn = self.conn_lock();
        ensure_account_row(&conn);
        let row = token_row(&conn);
        let authenticated = row.as_ref().is_some_and(|r| {
            !r.revoked && r.access_token.as_deref().is_some_and(|t| !t.is_empty())
        });
        json!({
            "configured": self.credentials.is_configured(),
            "authenticated": authenticated,
            "demoMode": demo_mode,
            "expiresAt": row.as_ref().and_then(|r| r.expires_at.clone()),
        })
    }

    pub async fn request(&self, path: &str, method: &str, body: Option<Value>) -> Result<Value> {
        let priority = if method != "GET" {
            PRIORITY_INTERACTIVE
        } else {
            PRIORITY_SYNC
        };
        self.request_pri(path, method, body, priority).await
    }

    pub async fn request_pri(
        &self,
        path: &str,
        method: &str,
        body: Option<Value>,
        priority: u8,
    ) -> Result<Value> {
        let is_write = method != "GET";
        let path = path.to_string();
        let method = method.to_string();
        let fut = self.request_inner(&path, &method, body, 3);
        self.queue.enqueue(priority, is_write, fut).await
    }

    async fn hs_range_report(
        &self,
        key: &str,
        name: &str,
        path: &str,
        start: &str,
        end: &str,
    ) -> Result<Option<HsReportRow>> {
        match self
            .request_pri(
                &format!("{path}?start={start}&end={end}"),
                "GET",
                None,
                PRIORITY_ANALYTICS,
            )
            .await
        {
            Ok(data) => Ok(Some(HsReportRow {
                key: key.to_string(),
                name: name.to_string(),
                source: "helpscout".to_string(),
                data,
            })),
            Err(_) => Ok(None),
        }
    }

    async fn request_inner(
        &self,
        path: &str,
        method: &str,
        body: Option<Value>,
        retries_left: u32,
    ) -> Result<Value> {
        let token = self.access_token().await?;
        let url = format!("{}{path}", self.api_base);
        let m: reqwest::Method = method
            .parse()
            .map_err(|_| Error::Other(format!("invalid HTTP method {method}").into()))?;
        let mut req = self
            .client
            .request(m, &url)
            .header("Authorization", format!("Bearer {token}"))
            .header("Accept", "application/json");
        if let Some(b) = &body {
            req = req.json(b);
        }

        let res = req.send().await;

        let res = match res {
            Ok(r) => r,
            Err(e) => {
                let msg = e.to_string();
                if retries_left > 0
                    && (msg.contains("timeout")
                        || msg.contains("Connection refused")
                        || msg.contains("error sending request"))
                {
                    tokio::time::sleep(Duration::from_millis(1_500)).await;
                    return Box::pin(self.request_inner(path, method, body, retries_left - 1))
                        .await;
                }
                return Err(HsApiError {
                    status_code: 0,
                    message: format!("Network error contacting Help Scout: {msg}"),
                    friendly: "Help Scout is unreachable. Local data remains fully browsable - you are in offline mode for remote actions.".into(),
                    retryable: true,
                }
                .into());
            }
        };

        let status = res.status().as_u16();
        let limit = res
            .headers()
            .get("x-ratelimit-limit-minute")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<i64>().ok());
        let remaining = res
            .headers()
            .get("x-ratelimit-remaining-minute")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<i64>().ok());
        let retry_after_hdr = res
            .headers()
            .get("x-ratelimit-retry-after")
            .or_else(|| res.headers().get("retry-after"))
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<i64>().ok());
        self.limiter
            .record_response(limit, remaining, retry_after_hdr, method != "GET");

        if status == 301 {
            let location = res
                .headers()
                .get("location")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("")
                .to_string();
            let new_id = location
                .split("conversations/")
                .nth(1)
                .and_then(|s| s.split(&['/', '?'][..]).next())
                .map(|s| s.to_string())
                .unwrap_or_else(|| "another conversation".into());
            return Err(HsApiError {
                status_code: 301,
                message: format!("Conversation merged into {new_id}"),
                friendly: "This conversation was merged into another conversation in Help Scout. Open the target conversation instead.".into(),
                retryable: false,
            }
            .into());
        }

        if status == 401 && retries_left > 0 {
            let refreshed = {
                let conn = self.conn_lock();
                token_row(&conn).and_then(|r| r.refresh_token)
            };
            if let Some(refresh) = refreshed
                && self.refresh_tokens(&refresh).await.is_ok()
            {
                return Box::pin(self.request_inner(path, method, body, retries_left - 1)).await;
            }
        }

        if status == 429 {
            let retry_after = retry_after_hdr.filter(|s| *s > 0).unwrap_or(30).min(60);
            self.limiter.record_error_429(Some(retry_after));
            if retries_left > 0 {
                tokio::time::sleep(
                    Duration::from_secs(retry_after as u64) + Duration::from_millis(250),
                )
                .await;
                return Box::pin(self.request_inner(path, method, body, retries_left - 1)).await;
            }
        }

        if is_retryable(status) && retries_left > 0 && status != 429 {
            tokio::time::sleep(Duration::from_secs(2)).await;
            return Box::pin(self.request_inner(path, method, body, retries_left - 1)).await;
        }

        if (200..300).contains(&status) {
            let text = res.text().await.unwrap_or_default();
            if text.is_empty() {
                return Ok(Value::Null);
            }
            return serde_json::from_str::<Value>(&text).map_err(|_| {
                HsApiError {
                    status_code: status,
                    message: "Invalid JSON from Help Scout".into(),
                    friendly: "Help Scout returned an unexpected response format. The raw response was preserved for diagnostics.".into(),
                    retryable: false,
                }
                .into()
            });
        }

        let body_text = res.text().await.unwrap_or_default();
        let detail: String = body_text.chars().take(400).collect();
        Err(HsApiError {
            status_code: status,
            message: format!("Help Scout {method} {path} -> {status}: {detail}"),
            friendly: friendly_error(status, &body_text, method),
            retryable: is_retryable(status)
                && !matches!(status, 400 | 401 | 403 | 404 | 409 | 412 | 413 | 415 | 423),
        }
        .into())
    }

    fn map_user(v: &Value) -> HsUser {
        HsUser {
            remote_id: v["id"].as_i64().unwrap_or(0),
            first_name: v["firstName"].as_str().map(|s| s.to_string()),
            last_name: v["lastName"].as_str().map(|s| s.to_string()),
            email: v["email"].as_str().map(|s| s.to_string()),
            role: v["role"].as_str().map(|s| s.to_string()),
            user_type: v["type"].as_str().unwrap_or("user").to_string(),
            timezone: v["timezone"].as_str().map(|s| s.to_string()),
            photo_url: v["photoUrl"].as_str().map(|s| s.to_string()),
            initials: v["initials"].as_str().map(|s| s.to_string()),
            mention: v["mention"].as_str().map(|s| s.to_string()),
            job_title: v["jobTitle"].as_str().map(|s| s.to_string()),
            phone: v["phone"].as_str().map(|s| s.to_string()),
            alternate_emails: Vec::new(),
            created_at: v["createdAt"].as_str().map(|s| s.to_string()),
            updated_at: v["updatedAt"].as_str().map(|s| s.to_string()),
        }
    }

    fn map_mailbox(v: &Value) -> HsMailbox {
        HsMailbox {
            remote_id: v["id"].as_i64().unwrap_or(0),
            name: v["name"].as_str().unwrap_or_default().to_string(),
            slug: v["slug"].as_str().map(|s| s.to_string()),
            email: v["email"].as_str().map(|s| s.to_string()),
            created_at: v["createdAt"].as_str().map(|s| s.to_string()),
            updated_at: v["updatedAt"].as_str().map(|s| s.to_string()),
        }
    }

    fn map_folder(v: &Value, mailbox_id: i64) -> HsFolder {
        HsFolder {
            remote_id: v["id"].as_i64().unwrap_or(0),
            mailbox_id,
            name: v["name"].as_str().unwrap_or_default().to_string(),
            kind: v["type"].as_str().unwrap_or_default().to_string(),
            user_id: v["userId"].as_i64(),
            total_count: v["totalCount"].as_i64().unwrap_or(0),
            active_count: v["activeCount"].as_i64().unwrap_or(0),
        }
    }

    fn map_field(v: &Value, mailbox_id: i64) -> HsField {
        HsField {
            remote_id: v["id"].as_i64().unwrap_or(0),
            mailbox_id,
            name: v["name"].as_str().unwrap_or_default().to_string(),
            kind: v["type"].as_str().unwrap_or_default().to_string(),
            system_type: v["systemType"].as_str().map(|s| s.to_string()),
            required: v["required"].as_bool().unwrap_or(false),
            sort_order: v["order"].as_i64().unwrap_or(0),
            options: v["_embedded"]["fields"]
                .as_array()
                .map(|opts| {
                    opts.iter()
                        .map(|o| HsFieldOption {
                            id: o["id"].as_i64().unwrap_or(0),
                            order: o["order"].as_i64().unwrap_or(0),
                            label: o["label"].as_str().unwrap_or_default().to_string(),
                        })
                        .collect()
                })
                .unwrap_or_default(),
        }
    }

    fn map_tag(v: &Value) -> HsTag {
        HsTag {
            remote_id: v["id"].as_i64().unwrap_or(0),
            name: v["tag"].as_str().unwrap_or_default().to_string(),
            slug: v["slug"].as_str().map(|s| s.to_string()),
            color: v["color"].as_str().map(|s| s.to_string()),
            ticket_count: v["ticketCount"].as_i64(),
            created_at: v["createdAt"].as_str().map(|s| s.to_string()),
            updated_at: v["updatedAt"].as_str().map(|s| s.to_string()),
        }
    }

    fn map_conversation(v: &Value) -> HsConversation {
        let assignee_type = v["assignee"]["type"].as_str().map(|s| s.to_string());
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
            assignee_id: v["assignee"]["id"].as_i64(),
            assignee_type: assignee_type.clone(),
            assigned_team_id: v["assignedTeam"]["id"].as_i64().or(
                if assignee_type.as_deref() == Some("team") {
                    v["assignee"]["id"].as_i64()
                } else {
                    None
                },
            ),
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
                        .map(|r| crate::helpscout::HsThreadRecipient {
                            id: r["id"].as_i64(),
                            email: r["email"].as_str().map(|s| s.to_string()),
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
            created_by_customer_id: v["createdByCustomer"]["id"].as_i64(),
            created_by_user_id: v["createdByUser"]["id"].as_i64(),
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

    async fn get_me(&self) -> Result<HsUser> {
        let v = self
            .request_pri("/v2/users/me", "GET", None, PRIORITY_INTERACTIVE)
            .await?;
        Ok(Self::map_user(&v))
    }

    async fn list_mailboxes(&self) -> Result<Vec<HsMailbox>> {
        let v = self.request("/v2/mailboxes", "GET", None).await?;
        Ok(v["_embedded"]["mailboxes"]
            .as_array()
            .map(|a| a.iter().map(Self::map_mailbox).collect())
            .unwrap_or_default())
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
        Ok(Vec::new())
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
        Ok(Vec::new())
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
                let msg = e.to_string();
                if msg.contains("-> 404") {
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

    async fn get_company_overall_report(
        &self,
        start: &str,
        end: &str,
    ) -> Result<Option<HsReportRow>> {
        self.hs_range_report(
            "hs_company_overall",
            "Help Scout Company Overall",
            "/v2/reports/company",
            start,
            end,
        )
        .await
    }

    async fn get_conversations_overall_report(
        &self,
        start: &str,
        end: &str,
    ) -> Result<Option<HsReportRow>> {
        self.hs_range_report(
            "hs_conversations_overall",
            "Help Scout Conversations Overall",
            "/v2/reports/conversations",
            start,
            end,
        )
        .await
    }

    async fn get_happiness_ratings_report(
        &self,
        start: &str,
        end: &str,
    ) -> Result<Option<HsReportRow>> {
        self.hs_range_report(
            "hs_happiness_ratings",
            "Help Scout Happiness Ratings",
            "/v2/reports/happiness",
            start,
            end,
        )
        .await
    }

    async fn get_productivity_overall_report(
        &self,
        start: &str,
        end: &str,
    ) -> Result<Option<HsReportRow>> {
        self.hs_range_report(
            "hs_productivity_overall",
            "Help Scout Productivity Overall",
            "/v2/reports/productivity",
            start,
            end,
        )
        .await
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
        let v = self
            .request(
                &format!("/v2/mailboxes/{mailbox_id}/saved-replies"),
                "GET",
                None,
            )
            .await?;
        Ok(v["_embedded"]["savedReplies"]
            .as_array()
            .map(|a| {
                a.iter()
                    .map(|r| HsSavedReply {
                        remote_id: r["id"].as_i64().unwrap_or(0),
                        name: r["name"].as_str().unwrap_or_default().to_string(),
                        preview: r["preview"].as_str().map(|s| s.to_string()),
                        text: r["text"].as_str().map(|s| s.to_string()),
                    })
                    .collect()
            })
            .unwrap_or_default())
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
        let v = self.request("/v2/webhooks", "GET", None).await?;
        Ok(v["_embedded"]["webhooks"]
            .as_array()
            .map(|a| {
                a.iter()
                    .map(|w| HsWebhookConfig {
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
                    })
                    .collect()
            })
            .unwrap_or_default())
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
        Ok(v["id"]
            .as_i64()
            .or_else(|| v["webhook"]["id"].as_i64())
            .unwrap_or(0))
    }

    async fn delete_webhook(&self, remote_id: i64) -> Result<bool> {
        self.request(&format!("/v2/webhooks/{remote_id}"), "DELETE", None)
            .await?;
        Ok(true)
    }

    async fn list_customer_property_definitions(&self) -> Result<Vec<HsPropertyDef>> {
        let v = self.request("/v2/customer-properties", "GET", None).await?;
        Ok(v["_embedded"]["customerProperties"]
            .as_array()
            .or_else(|| v["_embedded"]["properties"].as_array())
            .map(|a| a.iter().map(Self::map_property_def).collect())
            .unwrap_or_default())
    }

    async fn list_organization_property_definitions(&self) -> Result<Vec<HsPropertyDef>> {
        let v = self
            .request("/v2/organization-properties", "GET", None)
            .await?;
        Ok(v["_embedded"]["organizationProperties"]
            .as_array()
            .or_else(|| v["_embedded"]["properties"].as_array())
            .map(|a| a.iter().map(Self::map_property_def).collect())
            .unwrap_or_default())
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
        let v = self
            .request(
                &format!("/v3/conversations/{conversation_id}/threads"),
                "GET",
                None,
            )
            .await?;
        Ok(v["_embedded"]["threads"]
            .as_array()
            .map(|a| {
                a.iter()
                    .map(|t| Self::map_thread(t, conversation_id))
                    .collect()
            })
            .unwrap_or_default())
    }

    async fn get_customer(&self, customer_id: i64) -> Result<Option<HsCustomer>> {
        let res = self
            .request(&format!("/v2/customers/{customer_id}"), "GET", None)
            .await;
        match res {
            Ok(v) => Ok(Some(Self::map_customer(&v))),
            Err(e) => {
                if is_not_found(&e) {
                    return Ok(None);
                }
                Err(e)
            }
        }
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
        loop {
            let v = self.request(&path, "GET", None).await?;
            let embedded = v["_embedded"]["system-users"]
                .as_array()
                .or_else(|| v["_embedded"]["users"].as_array())
                .cloned()
                .unwrap_or_default();
            let n = embedded.len();
            out.extend(embedded.iter().map(Self::map_user));
            match hal_next_path(&v) {
                Some(next) if n > 0 => path = next,
                _ => break,
            }
        }
        Ok(out)
    }

    async fn list_doc_collections(&self) -> Result<Vec<HsDocCollection>> {
        let v = self.docs_request("/v2/collections").await?;
        Ok(v["collections"]
            .as_array()
            .map(|a| {
                a.iter()
                    .map(|c| HsDocCollection {
                        remote_id: c["id"].as_i64().unwrap_or(0),
                        slug: c["slug"].as_str().map(|s| s.to_string()),
                        name: c["name"].as_str().unwrap_or_default().to_string(),
                        description: c["description"].as_str().map(|s| s.to_string()),
                        visibility: c["visibility"].as_str().map(|s| s.to_string()),
                        article_count: c["articleCount"].as_i64(),
                    })
                    .collect()
            })
            .unwrap_or_default())
    }

    async fn list_doc_categories(&self, collection_id: i64) -> Result<Vec<HsDocCategory>> {
        let v = self
            .docs_request(&format!("/v2/collections/{collection_id}/categories"))
            .await?;
        Ok(v["categories"]
            .as_array()
            .map(|a| {
                a.iter()
                    .map(|c| HsDocCategory {
                        remote_id: c["id"].as_i64().unwrap_or(0),
                        collection_id,
                        slug: c["slug"].as_str().map(|s| s.to_string()),
                        name: c["name"].as_str().unwrap_or_default().to_string(),
                        sort_order: c["order"].as_i64(),
                    })
                    .collect()
            })
            .unwrap_or_default())
    }

    async fn list_doc_articles(&self, collection_id: i64) -> Result<Vec<HsDocArticle>> {
        let v = self
            .docs_request(&format!("/v2/collections/{collection_id}/articles"))
            .await?;
        Ok(v["articles"]
            .as_array()
            .map(|a| {
                a.iter()
                    .map(|art| HsDocArticle {
                        remote_id: art["id"].as_i64().unwrap_or(0),
                        collection_id,
                        category_id: art["categoryId"].as_i64(),
                        number: art["number"].as_i64(),
                        slug: art["slug"].as_str().map(|s| s.to_string()),
                        name: art["name"].as_str().unwrap_or_default().to_string(),
                        status: art["status"].as_str().map(|s| s.to_string()),
                        text: art["text"].as_str().map(|s| s.to_string()),
                        views: art["views"].as_i64(),
                        created_at: art["createdAt"].as_str().map(|s| s.to_string()),
                        updated_at: art["updatedAt"].as_str().map(|s| s.to_string()),
                    })
                    .collect()
            })
            .unwrap_or_default())
    }

    async fn create_reply_thread(&self, input: CreateThreadInput) -> Result<ThreadCreated> {
        let mut body = json!({ "text": input.text, "draft": input.draft });
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
            thread_id: res["id"].as_i64().unwrap_or_default(),
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
            thread_id: res["id"].as_i64().unwrap_or_default(),
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
            "subject": input.subject,
            "mailboxId": input.mailbox_id,
            "customer": { "id": input.customer_id },
            "threads": threads,
        });
        if let Some(status) = &input.status {
            body["status"] = json!(status);
        }
        if !input.tags.is_empty() {
            body["tags"] = json!(input.tags);
        }
        let res = self
            .request_pri("/v3/conversations", "POST", Some(body), PRIORITY_USER_SEND)
            .await?;
        let conversation_id = res["id"].as_i64().unwrap_or(0);
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
        let mut page_no: u32 = 0;
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
            let number = v["page"]["number"].as_u64().unwrap_or(0);
            if n == 0 || number + 1 >= total_pages {
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

pub fn hs_status(e: &Error) -> Option<u16> {
    if let Error::Other(b) = e
        && let Some(hs) = b.downcast_ref::<HsApiError>()
    {
        return Some(hs.status_code);
    }
    None
}

pub fn hs_error(e: &Error) -> Option<&HsApiError> {
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
