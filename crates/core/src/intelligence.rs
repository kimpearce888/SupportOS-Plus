use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};

use crate::error::Result;

pub const RECENCY_HALF_LIFE_DAYS: f64 = 90.0;

pub const MIN_OBSERVATIONS_FOR_PREFERENCE: u32 = 3;

pub const CHANGE_SIGNIFICANCE_THRESHOLD: f64 = 0.34;

pub const M014_SQL: &str = r#"
    CREATE TABLE IF NOT EXISTS interaction_signals (
        id              INTEGER PRIMARY KEY AUTOINCREMENT,
        customer_id     INTEGER NOT NULL,
        signal_key      TEXT NOT NULL,
        signal_value    TEXT NOT NULL,
        observed_at     TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
        source_conversation_id INTEGER
    );
    CREATE INDEX IF NOT EXISTS idx_interaction_signals_customer
        ON interaction_signals (customer_id, signal_key, observed_at);

    UPDATE app_state SET schema_version = 14 WHERE id = 1;
"#;

pub fn apply_m014(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS interaction_signals (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            customer_id     INTEGER NOT NULL,
            signal_key      TEXT NOT NULL,
            signal_value    TEXT NOT NULL,
            observed_at     TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
            source_conversation_id INTEGER
        );
        CREATE INDEX IF NOT EXISTS idx_interaction_signals_customer
            ON interaction_signals (customer_id, signal_key, observed_at);",
    )?;
    let _ = conn.execute("UPDATE app_state SET schema_version = 14 WHERE id = 1", []);
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignalKey {
    ResponseStyle,
    QuestionCount,
    TechnicalFamiliarity,
    FrustrationCues,
    EscalationIntent,
}

impl SignalKey {
    pub const ALL: [Self; 5] = [
        Self::ResponseStyle,
        Self::QuestionCount,
        Self::TechnicalFamiliarity,
        Self::FrustrationCues,
        Self::EscalationIntent,
    ];

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ResponseStyle => "response_style",
            Self::QuestionCount => "question_count",
            Self::TechnicalFamiliarity => "technical_familiarity",
            Self::FrustrationCues => "frustration_cues",
            Self::EscalationIntent => "escalation_intent",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InteractionSignal {
    pub id: Option<i64>,
    pub customer_id: i64,
    pub signal_key: String,
    pub signal_value: String,
    pub observed_at: String,
    pub source_conversation_id: Option<i64>,
}

pub fn record_signal(
    conn: &Connection,
    customer_id: i64,
    key: SignalKey,
    value: &str,
    source_conversation_id: Option<i64>,
) -> Result<i64> {
    conn.execute(
        "INSERT INTO interaction_signals (customer_id, signal_key, signal_value, source_conversation_id)
         VALUES (?1, ?2, ?3, ?4)",
        params![customer_id, key.as_str(), value, source_conversation_id],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn get_signals(conn: &Connection, customer_id: i64) -> Result<Vec<InteractionSignal>> {
    let mut stmt = conn.prepare(
        "SELECT id, customer_id, signal_key, signal_value, observed_at, source_conversation_id
         FROM interaction_signals
         WHERE customer_id = ?1
         ORDER BY observed_at ASC",
    )?;
    let rows = stmt
        .query_map(params![customer_id], |r| {
            Ok(InteractionSignal {
                id: r.get(0)?,
                customer_id: r.get(1)?,
                signal_key: r.get(2)?,
                signal_value: r.get(3)?,
                observed_at: r.get(4)?,
                source_conversation_id: r.get(5)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub fn get_signals_for_key(
    conn: &Connection,
    customer_id: i64,
    key: SignalKey,
) -> Result<Vec<InteractionSignal>> {
    let mut stmt = conn.prepare(
        "SELECT id, customer_id, signal_key, signal_value, observed_at, source_conversation_id
         FROM interaction_signals
         WHERE customer_id = ?1 AND signal_key = ?2
         ORDER BY observed_at ASC",
    )?;
    let rows = stmt
        .query_map(params![customer_id, key.as_str()], |r| {
            Ok(InteractionSignal {
                id: r.get(0)?,
                customer_id: r.get(1)?,
                signal_key: r.get(2)?,
                signal_value: r.get(3)?,
                observed_at: r.get(4)?,
                source_conversation_id: r.get(5)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub fn count_observations(conn: &Connection, customer_id: i64, key: SignalKey) -> Result<u32> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM interaction_signals
         WHERE customer_id = ?1 AND signal_key = ?2",
        params![customer_id, key.as_str()],
        |r| r.get(0),
    )?;
    Ok(u32::try_from(count).unwrap_or(0))
}

pub fn is_confirmed_pattern(conn: &Connection, customer_id: i64, key: SignalKey) -> Result<bool> {
    Ok(count_observations(conn, customer_id, key)? >= MIN_OBSERVATIONS_FOR_PREFERENCE)
}

#[must_use]
pub fn recency_weight(days_ago: f64) -> f64 {
    if days_ago < 0.0 {
        return 1.0;
    }
    0.5_f64.powf(days_ago / RECENCY_HALF_LIFE_DAYS)
}

#[must_use]
pub fn is_significant_change(new_value_ratio: f64) -> bool {
    new_value_ratio.abs() > CHANGE_SIGNIFICANCE_THRESHOLD
}

#[must_use]
pub fn derive_signal(key: SignalKey, message_text: &str) -> String {
    let text_lower = message_text.to_lowercase();
    match key {
        SignalKey::QuestionCount => {
            let count = message_text.matches('?').count();
            count.to_string()
        }
        SignalKey::FrustrationCues => {
            let frustration_words = [
                "angry",
                "frustrated",
                "unacceptable",
                "ridiculous",
                "worst",
                "terrible",
                "horrible",
                "disappointed",
                "annoyed",
                "furious",
            ];
            let found = frustration_words
                .iter()
                .filter(|w| text_lower.contains(*w))
                .count();
            if found >= 3 {
                "angry".to_string()
            } else if found >= 1 {
                "frustrated".to_string()
            } else {
                "calm".to_string()
            }
        }
        SignalKey::EscalationIntent => {
            let escalation_words = [
                "manager",
                "supervisor",
                "escalate",
                "lawsuit",
                "cancel",
                "refund now",
                "better business",
                "attorney",
            ];
            let found = escalation_words
                .iter()
                .filter(|w| text_lower.contains(*w))
                .count();
            if found >= 1 {
                "yes".to_string()
            } else {
                "no".to_string()
            }
        }
        SignalKey::TechnicalFamiliarity => {
            let tech_words = [
                "api",
                "endpoint",
                "webhook",
                "json",
                "stack trace",
                "log",
                "debug",
                "http",
                "ssl",
                "certificate",
                "dns",
                "firewall",
            ];
            let found = tech_words
                .iter()
                .filter(|w| text_lower.contains(*w))
                .count();
            if found >= 3 {
                "expert".to_string()
            } else if found >= 1 {
                "intermediate".to_string()
            } else {
                "beginner".to_string()
            }
        }
        SignalKey::ResponseStyle => {
            let word_count = message_text.split_whitespace().count();
            if word_count > 100 {
                "formal".to_string()
            } else if word_count > 30 {
                "detailed".to_string()
            } else {
                "casual".to_string()
            }
        }
    }
}
