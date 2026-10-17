use rusqlite::{Connection, params};

use crate::error::Result;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OAuthToken {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_in: Option<u64>,
    pub token_type: String,
    pub scope: Option<String>,
}

pub fn store_token(conn: &Connection, token: &OAuthToken) -> Result<()> {
    let expires_at = token.expires_in.map(|secs| {
        let now = chrono::Utc::now();
        let exp = now + chrono::Duration::seconds(secs as i64);
        exp.format("%Y-%m-%dT%H:%M:%SZ").to_string()
    });
    conn.execute(
        "INSERT INTO oauth_tokens (id, access_token, refresh_token, expires_at, token_type, scope)
         VALUES (1, ?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(id) DO UPDATE SET
            access_token = excluded.access_token,
            refresh_token = excluded.refresh_token,
            expires_at = excluded.expires_at,
            token_type = excluded.token_type,
            scope = excluded.scope,
            obtained_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')",
        params![
            token.access_token,
            token.refresh_token,
            expires_at,
            token.token_type,
            token.scope,
        ],
    )?;
    Ok(())
}

pub fn get_access_token(conn: &Connection) -> Result<Option<String>> {
    let v: Option<String> = conn
        .prepare("SELECT access_token FROM oauth_tokens WHERE id = 1")?
        .query_row([], |r| r.get::<_, String>(0))
        .ok();
    Ok(v)
}

pub fn has_token(conn: &Connection) -> Result<bool> {
    let exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM oauth_tokens WHERE id = 1 AND access_token IS NOT NULL)",
        [],
        |r| r.get(0),
    )?;
    Ok(exists)
}

pub fn delete_token(conn: &Connection) -> Result<bool> {
    let rows = conn.execute("DELETE FROM oauth_tokens WHERE id = 1", [])?;
    Ok(rows > 0)
}
