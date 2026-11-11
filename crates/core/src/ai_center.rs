use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};

use crate::error::Result;

pub fn apply_m009(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS ai_settings (
            id              INTEGER PRIMARY KEY CHECK (id = 1),
            provider_kind   TEXT NOT NULL DEFAULT 'none',
            chat_model      TEXT,
            embedding_model TEXT,
            embedding_dim   INTEGER,
            base_url        TEXT,
            updated_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
        );
        INSERT OR IGNORE INTO ai_settings (id, provider_kind) VALUES (1, 'none');",
    )?;
    let _ = conn.execute("UPDATE app_state SET schema_version = 9 WHERE id = 1", []);
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    None,
    LmStudio,
}

impl ProviderKind {
    pub const ALL: [Self; 2] = [Self::None, Self::LmStudio];

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::LmStudio => "lm_studio",
        }
    }

    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "none" => Some(Self::None),
            "lm_studio" => Some(Self::LmStudio),
            _ => None,
        }
    }

    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::None => "None (no AI provider)",
            Self::LmStudio => "LM Studio",
        }
    }
}

/// The settings record persisted in `ai_settings`.
///
/// Deliberately carries NO availability claim: a verdict like
/// "provider reachable" is live network state, not a stored setting, and
/// the honest source for it is the AI hub's `probe_endpoint` lane (the
/// AI Center page polls it and renders "reachable — N models · X ms" /
/// "offline — ..."). The audit (B6) found this struct previously carried a
/// `provider_available: Option<bool>` that no constructor ever populated
/// and no consumer ever read — an always-`None` dead field masquerading as
/// a health signal; it was removed rather than fake-populated.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiStatus {
    pub provider_kind: ProviderKind,
    pub chat_model: Option<String>,
    pub embedding_model: Option<String>,
    pub embedding_dim: Option<usize>,
    pub base_url: Option<String>,
}

impl Default for AiStatus {
    fn default() -> Self {
        Self {
            provider_kind: ProviderKind::None,
            chat_model: None,
            embedding_model: None,
            embedding_dim: None,
            base_url: None,
        }
    }
}

pub fn get_ai_status(conn: &Connection) -> Result<AiStatus> {
    struct AiSettingsRow {
        kind: String,
        chat: Option<String>,
        embed: Option<String>,
        dim: Option<i64>,
        url: Option<String>,
    }
    let row: Option<AiSettingsRow> = conn
        .query_row(
            "SELECT provider_kind, chat_model, embedding_model, embedding_dim, base_url
             FROM ai_settings WHERE id = 1",
            [],
            |r| {
                Ok(AiSettingsRow {
                    kind: r.get(0)?,
                    chat: r.get(1)?,
                    embed: r.get(2)?,
                    dim: r.get(3)?,
                    url: r.get(4)?,
                })
            },
        )
        .ok();
    match row {
        None => Ok(AiStatus::default()),
        Some(r) => Ok(AiStatus {
            provider_kind: ProviderKind::parse(&r.kind).unwrap_or(ProviderKind::None),
            chat_model: r.chat,
            embedding_model: r.embed,
            embedding_dim: r.dim.map(|d| d as usize),
            base_url: r.url,
        }),
    }
}

pub fn set_provider_kind(conn: &Connection, kind: ProviderKind) -> Result<()> {
    conn.execute(
        "UPDATE ai_settings SET provider_kind = ?1, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
         WHERE id = 1",
        params![kind.as_str()],
    )?;
    Ok(())
}

pub fn set_chat_model(conn: &Connection, model: &str) -> Result<()> {
    conn.execute(
        "UPDATE ai_settings SET chat_model = ?1, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
         WHERE id = 1",
        params![model],
    )?;
    Ok(())
}

pub fn set_embedding_model(conn: &Connection, model: &str, dim: usize) -> Result<()> {
    conn.execute(
        "UPDATE ai_settings SET embedding_model = ?1, embedding_dim = ?2,
         updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = 1",
        params![model, dim as i64],
    )?;
    Ok(())
}

pub fn set_base_url(conn: &Connection, url: &str) -> Result<()> {
    conn.execute(
        "UPDATE ai_settings SET base_url = ?1, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
         WHERE id = 1",
        params![url],
    )?;
    Ok(())
}

#[must_use]
pub fn ai_features_enabled(status: &AiStatus) -> bool {
    status.provider_kind != ProviderKind::None && status.chat_model.is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn db() -> Connection {
        let mut conn = Connection::open_in_memory().expect("db");
        crate::bootstrap::apply_all(&mut conn).expect("migrations");
        conn
    }

    #[test]
    fn setters_round_trip_through_get_ai_status() {
        let conn = db();
        let status = get_ai_status(&conn).expect("status");
        assert_eq!(status.provider_kind, ProviderKind::None);
        assert!(status.chat_model.is_none());

        set_provider_kind(&conn, ProviderKind::LmStudio).expect("kind");
        set_chat_model(&conn, "qwen2.5-7b-instruct").expect("chat");
        set_embedding_model(&conn, "bge-m3", 1024).expect("embed");
        set_base_url(&conn, "http://127.0.0.1:1234").expect("base");

        let status = get_ai_status(&conn).expect("status");
        assert_eq!(status.provider_kind, ProviderKind::LmStudio);
        assert_eq!(status.chat_model.as_deref(), Some("qwen2.5-7b-instruct"));
        assert_eq!(status.embedding_model.as_deref(), Some("bge-m3"));
        assert_eq!(status.embedding_dim, Some(1024));
        assert_eq!(status.base_url.as_deref(), Some("http://127.0.0.1:1234"));
        assert!(ai_features_enabled(&status));

        set_provider_kind(&conn, ProviderKind::None).expect("kind back");
        let status = get_ai_status(&conn).expect("status");
        assert!(!ai_features_enabled(&status));
    }

    #[test]
    fn ai_features_enabled_requires_provider_and_chat_model() {
        let mut status = AiStatus::default();
        assert!(!ai_features_enabled(&status));
        status.provider_kind = ProviderKind::LmStudio;
        assert!(!ai_features_enabled(&status));
        status.chat_model = Some("m".into());
        assert!(ai_features_enabled(&status));
    }

    /// AUD-P2-3-1 (B6): the record no longer carries an availability
    /// claim — `provider_available` was removed (always-`None`, zero
    /// readers; availability truth lives in the hub probe lane). A
    /// serialized legacy record that still contains the dead key must
    /// deserialize fine — serde ignores unknown fields — so removing the
    /// field breaks no persisted/fixture JSON.
    #[test]
    fn audp2_3_b6_legacy_json_with_removed_provider_available_still_loads() {
        let legacy = serde_json::json!({
            "provider_kind": "lm_studio",
            "chat_model": "qwen2.5-7b-instruct",
            "embedding_model": "bge-m3",
            "embedding_dim": 1024,
            "base_url": "http://127.0.0.1:1234",
            "provider_available": true
        });
        let status: AiStatus =
            serde_json::from_value(legacy).expect("legacy record must deserialize");
        assert_eq!(status.provider_kind, ProviderKind::LmStudio);
        assert_eq!(status.chat_model.as_deref(), Some("qwen2.5-7b-instruct"));
        assert_eq!(status.base_url.as_deref(), Some("http://127.0.0.1:1234"));
        // And the fresh serialization round-trips without the dead key.
        let back = serde_json::to_value(&status).expect("serialize");
        assert!(back.get("provider_available").is_none());
    }
}
