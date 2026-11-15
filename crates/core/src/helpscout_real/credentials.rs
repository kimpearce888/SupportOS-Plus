use super::*;

pub(super) const TOKEN_EXPIRY_MARGIN_SECS: i64 = 120;

pub(super) struct TokenRow {
    pub(super) access_token: Option<String>,
    pub(super) refresh_token: Option<String>,
    pub(super) expires_at: Option<String>,
    pub(super) revoked: bool,
}

pub(super) fn token_row(conn: &Connection) -> Option<TokenRow> {
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

pub(super) fn base64_of(s: &str) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(s)
}

pub(super) fn stored_api_concurrency(db: &crate::db::DbHandle) -> usize {
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

pub(super) fn urlencode(s: &str) -> String {
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
