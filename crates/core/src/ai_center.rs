use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::catalog::CopilotTool;
use crate::error::Result;

pub const M009_SQL: &str = r#"
    CREATE TABLE IF NOT EXISTS ai_settings (
        id              INTEGER PRIMARY KEY CHECK (id = 1),
        provider_kind   TEXT NOT NULL DEFAULT 'none',
        chat_model      TEXT,
        embedding_model TEXT,
        embedding_dim   INTEGER,
        base_url        TEXT,
        updated_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
    );
    INSERT OR IGNORE INTO ai_settings (id, provider_kind) VALUES (1, 'none');

    UPDATE app_state SET schema_version = 9 WHERE id = 1;
"#;

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiStatus {
    pub provider_kind: ProviderKind,
    pub chat_model: Option<String>,
    pub embedding_model: Option<String>,
    pub embedding_dim: Option<usize>,
    pub base_url: Option<String>,
    pub provider_available: Option<bool>,
}

impl Default for AiStatus {
    fn default() -> Self {
        Self {
            provider_kind: ProviderKind::None,
            chat_model: None,
            embedding_model: None,
            embedding_dim: None,
            base_url: None,
            provider_available: None,
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
            provider_available: None,
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
pub fn copilot_tool_allowlist() -> &'static [CopilotTool] {
    &CopilotTool::ALL
}

#[must_use]
pub fn ai_features_enabled(status: &AiStatus) -> bool {
    status.provider_kind != ProviderKind::None && status.chat_model.is_some()
}
