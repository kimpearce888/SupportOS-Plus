use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::ai_provider::{ChatMessage, LocalAiProvider};
use crate::catalog::{AI_ATTRIBUTE_SCHEMA_VERSION, AiAttributeKey, AttributeValueType};
use crate::error::{Error, Result};

pub const M033_SQL: &str = r#"
    CREATE TABLE IF NOT EXISTS ai_attributes (
        id              INTEGER PRIMARY KEY AUTOINCREMENT,
        conversation_id INTEGER NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
        attribute       TEXT NOT NULL,
        value           TEXT NOT NULL,
        value_type      TEXT NOT NULL,
        confidence      TEXT NOT NULL,
        source          TEXT NOT NULL,
        evidence        TEXT,
        run_id          INTEGER REFERENCES ai_runs(id) ON DELETE SET NULL,
        schema_version  TEXT NOT NULL,
        computed_at     TEXT NOT NULL DEFAULT (datetime('now')),
        superseded_at   TEXT
    );
    CREATE INDEX IF NOT EXISTS idx_ai_attributes_current
        ON ai_attributes (conversation_id, attribute, superseded_at);
    CREATE INDEX IF NOT EXISTS idx_ai_attributes_value
        ON ai_attributes (attribute, value, superseded_at);
    CREATE INDEX IF NOT EXISTS idx_ai_attributes_run
        ON ai_attributes (run_id);
"#;

fn table_exists(conn: &Connection, table: &str) -> Result<bool> {
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
        params![table],
        |r| r.get(0),
    )?;
    Ok(n != 0)
}

fn column_exists(conn: &Connection, table: &str, column: &str) -> Result<bool> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let cols: Vec<String> = stmt
        .query_map([], |row| row.get::<_, String>(1))?
        .filter_map(|c| c.ok())
        .collect();
    Ok(cols.iter().any(|c| c == column))
}

type LegacyM010Row = (i64, i64, String, String, String, String, f64, String);

pub fn apply_m033(conn: &Connection) -> Result<()> {
    if !table_exists(conn, "ai_attributes")? {
        conn.execute_batch(M033_SQL)?;
        let _ = conn.execute("UPDATE app_state SET schema_version = 33 WHERE id = 1", []);
        return Ok(());
    }
    if column_exists(conn, "ai_attributes", "attribute")? {
        conn.execute_batch(
            "CREATE INDEX IF NOT EXISTS idx_ai_attributes_current
                ON ai_attributes (conversation_id, attribute, superseded_at);
             CREATE INDEX IF NOT EXISTS idx_ai_attributes_value
                ON ai_attributes (attribute, value, superseded_at);
             CREATE INDEX IF NOT EXISTS idx_ai_attributes_run
                ON ai_attributes (run_id);",
        )?;
        return Ok(());
    }

    if conn.is_autocommit() {
        let fk_was_on: bool = conn
            .query_row("PRAGMA foreign_keys", [], |r| {
                r.get::<_, i64>(0).map(|v| v != 0)
            })
            .unwrap_or(false);
        let _ = conn.execute_batch("PRAGMA foreign_keys = OFF");
        conn.execute_batch("BEGIN")?;
        match rebuild_m010_to_reference(conn) {
            Ok(()) => conn.execute_batch("COMMIT")?,
            Err(e) => {
                let _ = conn.execute_batch("ROLLBACK");
                if fk_was_on {
                    let _ = conn.execute_batch("PRAGMA foreign_keys = ON");
                }
                return Err(e);
            }
        }
        if fk_was_on {
            let _ = conn.execute_batch("PRAGMA foreign_keys = ON");
        }
    } else {
        let _ = conn.execute_batch("PRAGMA defer_foreign_keys = ON");
        rebuild_m010_to_reference(conn)?;
    }
    let _ = conn.execute("UPDATE app_state SET schema_version = 33 WHERE id = 1", []);
    Ok(())
}

fn rebuild_m010_to_reference(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "ALTER TABLE ai_attributes RENAME TO ai_attributes_m010;
             CREATE TABLE ai_attributes (
                id              INTEGER PRIMARY KEY AUTOINCREMENT,
                conversation_id INTEGER NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
                attribute       TEXT NOT NULL,
                value           TEXT NOT NULL,
                value_type      TEXT NOT NULL,
                confidence      TEXT NOT NULL,
                source          TEXT NOT NULL,
                evidence        TEXT,
                run_id          INTEGER REFERENCES ai_runs(id) ON DELETE SET NULL,
                schema_version  TEXT NOT NULL,
                computed_at     TEXT NOT NULL DEFAULT (datetime('now')),
                superseded_at   TEXT
            );
             CREATE INDEX IF NOT EXISTS idx_ai_attributes_current
                ON ai_attributes (conversation_id, attribute, superseded_at);
             CREATE INDEX IF NOT EXISTS idx_ai_attributes_value
                ON ai_attributes (attribute, value, superseded_at);
             CREATE INDEX IF NOT EXISTS idx_ai_attributes_run
                ON ai_attributes (run_id);",
    )?;

    let mut stmt = conn.prepare(
        "SELECT id, conversation_id, attribute_key, value, evidence_excerpt, thread_ref,
                confidence, created_at
         FROM ai_attributes_m010",
    )?;
    let legacy: Vec<LegacyM010Row> = stmt
        .query_map([], |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
                r.get(6)?,
                r.get(7)?,
            ))
        })?
        .filter_map(|row| row.ok())
        .collect();
    drop(stmt);

    for (id, conversation_id, key, value, excerpt, thread_ref, confidence, created_at) in legacy {
        let Some(def) = AiAttributeKey::parse(&key) else {
            continue;
        };
        let confidence = if confidence <= 0.0 {
            "unknown"
        } else {
            "medium"
        };
        let evidence = json!([{ "excerpt": excerpt, "thread_ref": thread_ref }]).to_string();
        conn.execute(
            "INSERT INTO ai_attributes
                    (id, conversation_id, attribute, value, value_type, confidence, source,
                     evidence, run_id, schema_version, computed_at, superseded_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'ai', ?7, NULL, ?8, ?9, NULL)",
            params![
                id,
                conversation_id,
                def.as_str(),
                value,
                def.value_type().as_str(),
                confidence,
                evidence,
                AI_ATTRIBUTE_SCHEMA_VERSION,
                created_at,
            ],
        )?;
    }

    conn.execute_batch("DROP TABLE ai_attributes_m010")?;
    Ok(())
}

#[derive(Debug, Clone, PartialEq)]
pub struct AttributeRecord {
    pub key: AiAttributeKey,
    pub value: String,
    pub confidence: &'static str,
    pub source: &'static str,
    pub evidence: Vec<Value>,
    pub run_id: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AttributeRow {
    pub id: i64,
    pub conversation_id: i64,
    pub conversation_number: Option<i64>,
    pub attribute: String,
    pub value: String,
    pub value_type: String,
    pub confidence: String,
    pub source: String,
    pub evidence: Value,
    pub run_id: Option<i64>,
    pub schema_version: String,
    pub computed_at: String,
    #[serde(skip_serializing)]
    pub superseded_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AttributeSnapshot {
    pub conversation_id: i64,
    pub conversation_number: Option<i64>,
    pub attributes: Vec<Value>,
    pub unknown: Vec<&'static str>,
    pub computed_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AttributeMatch {
    pub conversation_id: i64,
    pub number: i64,
    pub subject: Option<String>,
    pub value: String,
    pub confidence: String,
    pub source: String,
    pub computed_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AttributeDistribution {
    pub attribute: &'static str,
    pub label: &'static str,
    pub value_type: &'static str,
    pub total_conversations: i64,
    pub known: i64,
    pub unknown: i64,
    pub values: Vec<AttributeValueCount>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttributeValueCount {
    pub value: String,
    pub count: i64,
}

const NOT_DELETED: &str = "(status IS NULL OR status != 'deleted')";

fn insert_record(
    tx: &rusqlite::Transaction<'_>,
    conversation_id: i64,
    r: &AttributeRecord,
    run_id: Option<i64>,
) -> Result<bool> {
    let def = r.key;
    if def.value_type() == AttributeValueType::Enum && !def.values().contains(&r.value.as_str()) {
        return Ok(false);
    }
    if def.value_type() == AttributeValueType::Number
        && parse_js_number(&r.value).is_none_or(|f| !f.is_finite())
    {
        return Ok(false);
    }
    if def.value_type() == AttributeValueType::Boolean && r.value != "true" && r.value != "false" {
        return Ok(false);
    }
    let evidence = {
        let s = serde_json::to_string(&r.evidence)
            .map_err(|e| Error::Config(format!("evidence serialization failed: {e}")))?;
        s.chars().take(4000).collect::<String>()
    };
    tx.execute(
        "INSERT INTO ai_attributes
            (conversation_id, attribute, value, value_type, confidence, source, evidence,
             run_id, schema_version)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            conversation_id,
            def.as_str(),
            r.value,
            def.value_type().as_str(),
            r.confidence,
            r.source,
            evidence,
            r.run_id.or(run_id),
            AI_ATTRIBUTE_SCHEMA_VERSION,
        ],
    )?;
    Ok(true)
}

pub fn save_snapshot(
    conn: &mut Connection,
    conversation_id: i64,
    records: &[AttributeRecord],
    run_id: Option<i64>,
) -> Result<()> {
    let tx = conn.transaction()?;
    tx.execute(
        "UPDATE ai_attributes SET superseded_at = datetime('now')
         WHERE conversation_id = ?1 AND superseded_at IS NULL",
        params![conversation_id],
    )?;
    for r in records {
        insert_record(&tx, conversation_id, r, run_id)?;
    }
    tx.commit()?;
    Ok(())
}

pub fn upsert_single(
    conn: &mut Connection,
    conversation_id: i64,
    record: AttributeRecord,
) -> Result<i64> {
    let tx = conn.transaction()?;
    tx.execute(
        "UPDATE ai_attributes SET superseded_at = datetime('now')
         WHERE conversation_id = ?1 AND attribute = ?2 AND superseded_at IS NULL",
        params![conversation_id, record.key.as_str()],
    )?;
    let inserted = insert_record(&tx, conversation_id, &record, None)?;
    let id = if inserted { tx.last_insert_rowid() } else { 0 };
    tx.commit()?;
    Ok(id)
}

pub fn current_for_conversation(
    conn: &Connection,
    conversation_id: i64,
) -> Result<Vec<AttributeRow>> {
    let mut stmt = conn.prepare(
        "SELECT a.id, a.conversation_id, a.attribute, a.value, a.value_type, a.confidence,
                a.source, a.evidence, a.run_id, a.schema_version, a.computed_at,
                c.number AS conversation_number
         FROM ai_attributes a JOIN conversations c ON c.id = a.conversation_id
         WHERE a.conversation_id = ?1 AND a.superseded_at IS NULL",
    )?;
    let rows = stmt
        .query_map(params![conversation_id], map_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub fn history(
    conn: &Connection,
    conversation_id: i64,
    attribute: AiAttributeKey,
    limit: i64,
) -> Result<Vec<AttributeRow>> {
    let mut stmt = conn.prepare(
        "SELECT a.id, a.conversation_id, a.attribute, a.value, a.value_type, a.confidence,
                a.source, a.evidence, a.run_id, a.schema_version, a.computed_at,
                c.number AS conversation_number, a.superseded_at
         FROM ai_attributes a JOIN conversations c ON c.id = a.conversation_id
         WHERE a.conversation_id = ?1 AND a.attribute = ?2
         ORDER BY a.id DESC LIMIT ?3",
    )?;
    let rows = stmt
        .query_map(
            params![conversation_id, attribute.as_str(), limit],
            map_history_row,
        )?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchOp {
    Equals,
    NotEquals,
    Contains,
    Gt,
    Gte,
    Lt,
    Lte,
    Unknown,
}

impl MatchOp {
    #[must_use]
    pub fn allowed(value_type: AttributeValueType) -> &'static [Self] {
        match value_type {
            AttributeValueType::Number => &[
                Self::Equals,
                Self::NotEquals,
                Self::Gt,
                Self::Gte,
                Self::Lt,
                Self::Lte,
                Self::Unknown,
            ],
            _ => &[Self::Equals, Self::NotEquals, Self::Contains, Self::Unknown],
        }
    }

    #[must_use]
    pub fn parse(raw: &str, value_type: AttributeValueType) -> Self {
        let op = match raw {
            "equals" => Self::Equals,
            "not_equals" => Self::NotEquals,
            "contains" => Self::Contains,
            "gt" => Self::Gt,
            "gte" => Self::Gte,
            "lt" => Self::Lt,
            "lte" => Self::Lte,
            "unknown" => Self::Unknown,
            _ => return Self::Equals,
        };
        if Self::allowed(value_type).contains(&op) {
            op
        } else {
            Self::Equals
        }
    }

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Equals => "equals",
            Self::NotEquals => "not_equals",
            Self::Contains => "contains",
            Self::Gt => "gt",
            Self::Gte => "gte",
            Self::Lt => "lt",
            Self::Lte => "lte",
            Self::Unknown => "unknown",
        }
    }
}

fn escape_like(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        if ch == '\\' || ch == '%' || ch == '_' {
            out.push('\\');
        }
        out.push(ch);
    }
    out
}

fn parse_js_number(raw: &str) -> Option<f64> {
    let t = raw.trim();
    if t.is_empty() {
        return Some(0.0);
    }
    t.parse::<f64>().ok()
}

pub fn conversations_matching(
    conn: &Connection,
    attribute: AiAttributeKey,
    op: MatchOp,
    value: &str,
    limit: i64,
) -> Result<Vec<AttributeMatch>> {
    let cap = limit.clamp(1, 200);
    let map_match = |r: &rusqlite::Row<'_>| -> rusqlite::Result<AttributeMatch> {
        Ok(AttributeMatch {
            conversation_id: r.get(0)?,
            number: r.get(1)?,
            subject: r.get(2)?,
            value: r.get::<_, Option<String>>(3)?.unwrap_or_default(),
            confidence: r.get::<_, Option<String>>(4)?.unwrap_or_default(),
            source: r.get::<_, Option<String>>(5)?.unwrap_or_default(),
            computed_at: r.get::<_, Option<String>>(6)?.unwrap_or_default(),
        })
    };

    if op == MatchOp::Unknown {
        let mut stmt = conn.prepare(&format!(
            "SELECT c.id, c.number, c.subject, '' AS value, 'unknown' AS confidence,
                    'none' AS source, '' AS computed_at
             FROM conversations c
             WHERE {NOT_DELETED} AND NOT EXISTS (
               SELECT 1 FROM ai_attributes a
               WHERE a.conversation_id = c.id AND a.attribute = ?1 AND a.superseded_at IS NULL
             )
             ORDER BY c.number LIMIT ?2"
        ))?;
        let rows = stmt
            .query_map(params![attribute.as_str(), cap], map_match)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        return Ok(rows);
    }

    let is_number = attribute.value_type() == AttributeValueType::Number;
    let (table, cmp, param): (&str, &str, String) = if is_number {
        let Some(target) = parse_js_number(value) else {
            return Ok(Vec::new());
        };
        if !target.is_finite() {
            return Ok(Vec::new());
        }
        let cmp = match op {
            MatchOp::Equals => "= ?",
            MatchOp::NotEquals => "!= ?",
            MatchOp::Gt => "> ?",
            MatchOp::Gte => ">= ?",
            MatchOp::Lt => "< ?",
            MatchOp::Lte => "<= ?",
            _ => return Ok(Vec::new()),
        };
        ("CAST(a.value AS REAL)", cmp, format!("{target}"))
    } else {
        match op {
            MatchOp::Contains => (
                "LOWER(a.value)",
                "LIKE ? ESCAPE '\\'",
                format!("%{}%", escape_like(value)),
            ),
            MatchOp::Equals => ("LOWER(a.value)", "= ?", value.to_lowercase()),
            MatchOp::NotEquals => ("LOWER(a.value)", "!= ?", value.to_lowercase()),
            _ => return Ok(Vec::new()),
        }
    };

    let sql = format!(
        "SELECT a.conversation_id, c.number, c.subject, a.value, a.confidence, a.source,
                a.computed_at
         FROM ai_attributes a JOIN conversations c ON c.id = a.conversation_id
         WHERE a.attribute = ? AND a.superseded_at IS NULL AND {NOT_DELETED} AND {table} {cmp}
         ORDER BY c.number LIMIT ?"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map(params![attribute.as_str(), param, cap], map_match)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub fn distributions(conn: &Connection) -> Result<Vec<AttributeDistribution>> {
    let total: i64 = conn.query_row(
        &format!("SELECT COUNT(*) FROM conversations WHERE {NOT_DELETED}"),
        [],
        |r| r.get(0),
    )?;
    let mut out = Vec::with_capacity(AiAttributeKey::ALL.len());
    for def in AiAttributeKey::ALL {
        let mut stmt = conn.prepare(
            "SELECT value, COUNT(DISTINCT conversation_id) AS count FROM ai_attributes
             WHERE attribute = ?1 AND superseded_at IS NULL
             GROUP BY value ORDER BY count DESC, value",
        )?;
        let rows: Vec<(String, i64)> = stmt
            .query_map(params![def.as_str()], |r| Ok((r.get(0)?, r.get(1)?)))?
            .filter_map(|row| row.ok())
            .collect();
        let known: i64 = rows.iter().map(|(_, c)| c).sum();
        out.push(AttributeDistribution {
            attribute: def.as_str(),
            label: def.label(),
            value_type: def.value_type().as_str(),
            total_conversations: total,
            known,
            unknown: (total - known).max(0),
            values: rows
                .into_iter()
                .take(25)
                .map(|(value, count)| AttributeValueCount { value, count })
                .collect(),
        });
    }
    Ok(out)
}

pub fn distinct_values(
    conn: &Connection,
    attribute: AiAttributeKey,
    limit: i64,
) -> Result<Vec<String>> {
    let cap = limit.clamp(1, 200);
    let mut stmt = conn.prepare(
        "SELECT DISTINCT value FROM ai_attributes
         WHERE attribute = ?1 AND superseded_at IS NULL
         ORDER BY value LIMIT ?2",
    )?;
    let rows = stmt
        .query_map(params![attribute.as_str(), cap], |r| r.get::<_, String>(0))?
        .filter_map(|row| row.ok())
        .collect();
    Ok(rows)
}

pub fn conversations_with_attributes(conn: &Connection, limit: i64) -> Result<Vec<i64>> {
    let mut stmt = conn.prepare(
        "SELECT DISTINCT conversation_id FROM ai_attributes
         WHERE superseded_at IS NULL LIMIT ?1",
    )?;
    let rows = stmt
        .query_map(params![limit], |r| r.get::<_, i64>(0))?
        .filter_map(|row| row.ok())
        .collect();
    Ok(rows)
}

fn map_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<AttributeRow> {
    let evidence: Option<String> = r.get(7)?;
    let evidence = evidence
        .as_deref()
        .and_then(|s| serde_json::from_str::<Value>(s).ok())
        .unwrap_or_else(|| json!([]));
    Ok(AttributeRow {
        id: r.get(0)?,
        conversation_id: r.get(1)?,
        conversation_number: r.get::<_, Option<i64>>(11)?,
        attribute: r.get(2)?,
        value: r.get(3)?,
        value_type: r.get(4)?,
        confidence: r.get(5)?,
        source: r.get(6)?,
        evidence,
        run_id: r.get(8)?,
        schema_version: r.get(9)?,
        computed_at: r.get(10)?,
        superseded_at: None,
    })
}

fn map_history_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<AttributeRow> {
    let mut row = map_row(r)?;
    row.superseded_at = r.get(12)?;
    Ok(row)
}

pub fn snapshot(conn: &Connection, conversation_id: i64) -> Result<Option<AttributeSnapshot>> {
    let conv: Option<(i64, Option<i64>)> = conn
        .query_row(
            &format!("SELECT id, number FROM conversations WHERE id = ?1 AND {NOT_DELETED}"),
            params![conversation_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .ok();
    let Some((id, number)) = conv else {
        return Ok(None);
    };
    let rows = current_for_conversation(conn, id)?;
    let known: Vec<&str> = rows.iter().map(|r| r.attribute.as_str()).collect();
    let unknown: Vec<&'static str> = AiAttributeKey::ALL
        .iter()
        .map(|k| k.as_str())
        .filter(|k| !known.contains(k))
        .collect();
    let computed_at = rows
        .iter()
        .map(|r| r.computed_at.as_str())
        .max()
        .map(str::to_string);
    let attributes = rows
        .into_iter()
        .map(|r| {
            let mut v = serde_json::to_value(&r).unwrap_or_else(|_| json!({}));
            if let Value::Object(map) = &mut v {
                map.insert("status".into(), json!("known"));
            }
            v
        })
        .collect();
    Ok(Some(AttributeSnapshot {
        conversation_id: id,
        conversation_number: number,
        attributes,
        unknown,
        computed_at,
    }))
}

pub const ATTRIBUTE_EXTRACTION_PROMPT_VERSION: &str = "attribute_extraction_v1";

const ATTRIBUTE_EXTRACTION_SYSTEM: &str = "\
You extract structured attributes from a customer support conversation. You are part of a LOCAL-first support tool: everything you output is stored as versioned, evidence-backed local metadata - never sent to the customer.\n\
Rules:\n\
- Extract ONLY what the messages support. A missing attribute is simply omitted - never invent values.\n\
- \"intent\" MUST be exactly one of: question | bug_report | feature_request | billing | how_to | account_management | feedback | other.\n\
- \"response_style\" MUST be exactly one of: concise | detailed | step_by_step | technical | conversational | outcome_focused - pick the style the CUSTOMER's own messages ask for, not what you would write.\n\
- \"product\", \"feature\", \"issue\", \"customer_goal\" are short free text (max ~12 words each).\n\
- For every attribute with confidence \"medium\" or \"high\" you MUST include an evidence_excerpt copied from the messages provided, plus the thread id it came from.\n\
- Use confidence \"low\" when the signal is weak or ambiguous.\n\
- Respond with a single JSON object, no prose.\n\
\n\
JSON shape:\n\
{\"attributes\": [{\"attribute\": \"intent|product|feature|issue|customer_goal|response_style\", \"value\": \"...\", \"confidence\": \"high|medium|low|unknown\", \"evidence_excerpt\": \"...\", \"evidence_thread_local_id\": 123}]}";

#[derive(Debug, Clone, PartialEq)]
pub struct CustomerMessage {
    pub thread_local_id: i64,
    pub text: String,
}

pub fn customer_messages(conn: &Connection, conversation_id: i64) -> Result<Vec<CustomerMessage>> {
    // Clean-audit item 36: this window used to be the OLDEST 30 customer
    // messages INCLUDING soft-deleted threads — a long ticket's recent
    // context (the half that says what the customer wants now) was cut
    // off, and deleted threads kept feeding the prompt. The window is
    // now the NEWEST 30 live threads, presented oldest-first (the
    // prompt's declared order).
    let mut stmt = conn.prepare(
        "SELECT id, COALESCE(body_text, '') FROM conversation_threads
         WHERE conversation_id = ?1 AND type = 'customer'
           AND deleted_at IS NULL
           AND TRIM(COALESCE(body_text, ''), char(32) || char(9) || char(10) || char(13)) <> ''
         ORDER BY created_at DESC, id DESC LIMIT 30",
    )?;
    let mut rows: Vec<CustomerMessage> = stmt
        .query_map(params![conversation_id], |r| {
            Ok(CustomerMessage {
                thread_local_id: r.get(0)?,
                text: r.get(1)?,
            })
        })?
        .filter_map(|row| row.ok())
        .map(|m| {
            let text: String = m.text.chars().take(1200).collect();
            CustomerMessage {
                thread_local_id: m.thread_local_id,
                text,
            }
        })
        .filter(|m| !m.text.trim().is_empty())
        .collect();
    // Newest-first from SQL; the prompt (and the deterministic layer's
    // `messages.first()` customer_goal) expect chronological order.
    rows.reverse();
    Ok(rows)
}

#[derive(Debug, Clone)]
pub struct PreparedComputation {
    pub conversation_id: i64,
    pub records: Vec<AttributeRecord>,
    pub prompt: Option<String>,
    pub input_hash: String,
    pub cached: Option<(i64, Value)>,
}

pub fn prepare_computation(
    conn: &Connection,
    conversation_id: i64,
    model: &str,
    force: bool,
) -> Result<Option<PreparedComputation>> {
    let exists: Option<i64> = conn
        .query_row(
            &format!("SELECT id FROM conversations WHERE id = ?1 AND {NOT_DELETED}"),
            params![conversation_id],
            |r| r.get(0),
        )
        .ok()
        .flatten();
    if exists.is_none() {
        return Ok(None);
    }

    let records = deterministic_records(conn, conversation_id)?;

    let messages = customer_messages(conn, conversation_id)?;
    let (prompt, input_hash) = if messages.is_empty() || model.is_empty() {
        (None, String::new())
    } else {
        let prompt = build_extraction_user(conn, conversation_id, &messages);
        let hash = crate::embeddings::content_hash(&prompt);
        (Some(prompt), hash)
    };

    let cached = if force || prompt.is_none() {
        None
    } else {
        lookup_cached_extraction(conn, &input_hash, model)?
            .filter(|(_, v)| v.get("attributes").is_some_and(|a| a.as_array().is_some()))
    };

    Ok(Some(PreparedComputation {
        conversation_id,
        records,
        prompt,
        input_hash,
        cached,
    }))
}

/// What one extraction chat call produced — the run row's raw material
/// for the cost ledger (clean-audit item 37: content + latency + the
/// endpoint's token usage).
#[derive(Debug, Clone)]
pub struct ExtractionOutput {
    pub content: String,
    pub latency_ms: u64,
    pub usage: Option<crate::ai_provider::TokenUsage>,
}

pub async fn run_extraction(
    conn: &Connection,
    provider: &dyn LocalAiProvider,
    model: &str,
    prompt: &str,
) -> Option<ExtractionOutput> {
    if !provider.is_available().await {
        return None;
    }
    // Clean-audit item 35: the extraction prompt carries raw customer
    // messages — it crosses the same prompt-redaction boundary the
    // pipeline's chat_json uses (`redaction_enabled` setting, default
    // on) instead of bypassing it. Emails, card numbers, phone numbers
    // and secrets never reach the model from this path.
    let redaction_enabled =
        crate::settings::get_bool(conn, "redaction_enabled", true).unwrap_or(true);
    let (safe_prompt, findings) = crate::security::redact_text(prompt, redaction_enabled);
    if !findings.is_empty() {
        tracing::info!(
            operation = "attribute_extraction",
            patterns = ?findings.iter().filter_map(|f| f.get("pattern")).collect::<Vec<_>>(),
            "redaction boundary applied to the extraction prompt"
        );
    }
    let started = std::time::Instant::now();
    match provider
        .chat_structured(
            model,
            &[
                ChatMessage {
                    role: "system".into(),
                    content: ATTRIBUTE_EXTRACTION_SYSTEM.into(),
                },
                ChatMessage {
                    role: "user".into(),
                    content: safe_prompt,
                },
            ],
        )
        .await
    {
        Ok(response) => Some(ExtractionOutput {
            latency_ms: started.elapsed().as_millis() as u64,
            usage: response.usage,
            content: response.content,
        }),
        Err(_) => None,
    }
}

pub fn finish_computation(
    conn: &mut Connection,
    prepared: PreparedComputation,
    fresh_output: Option<ExtractionOutput>,
    model: &str,
    endpoint_id: Option<&str>,
) -> Result<Option<AttributeSnapshot>> {
    let PreparedComputation {
        conversation_id,
        mut records,
        prompt,
        input_hash,
        cached,
    } = prepared;

    let mut ai_run_id: Option<i64> = None;
    let mut extracted: Option<Value> = None;

    if let Some((run_id, cached_value)) = cached {
        extracted = Some(cached_value);
        ai_run_id = Some(run_id);
    } else if let (Some(_), Some(fresh)) = (&prompt, fresh_output)
        // Clean-audit item 34: local models happily wrap their JSON in
        // markdown fences or surround it with prose — a strict
        // from_str on the raw answer lost every such extraction. Parse
        // through the pipeline's canonical fence-stripping extractor
        // (the same one chat_json uses) before the shape validation.
        && let Some(parsed) = crate::ai_pipeline::extract_json(&fresh.content)
        && parsed
            .get("attributes")
            .is_some_and(|a| a.as_array().is_some())
    {
        // Clean-audit item 37: the run row now lands as a completed
        // 'attributes' run carrying the cost ledger's raw material
        // (latency, token usage, endpoint) instead of a bare row stuck
        // at the status column's 'queued' default, invisible to
        // `usage_ledger`'s `WHERE status='completed'`.
        let run_id = store_extraction_run(
            conn,
            ExtractionRunRow {
                conversation_id,
                input_hash: &input_hash,
                model,
                output: &fresh.content,
                latency_ms: fresh.latency_ms,
                usage: fresh.usage.as_ref(),
                endpoint_id,
            },
        )?;
        extracted = Some(parsed);
        ai_run_id = Some(run_id);
    }

    if let Some(Value::Object(map)) = extracted
        && let Some(Value::Array(attrs)) = map.get("attributes")
    {
        merge_extraction(&mut records, attrs, ai_run_id);
    }

    save_snapshot(conn, conversation_id, &records, ai_run_id)?;
    snapshot(conn, conversation_id)
}

fn merge_extraction(records: &mut Vec<AttributeRecord>, attrs: &[Value], ai_run_id: Option<i64>) {
    for a in attrs {
        let Some(obj) = a.as_object() else { continue };
        let Some(attr_name) = obj.get("attribute").and_then(Value::as_str) else {
            continue;
        };
        let Some(key) = AiAttributeKey::parse(attr_name) else {
            continue;
        };
        if !is_ai_slot(key) {
            continue;
        }
        let Some(raw_value) = obj.get("value").and_then(Value::as_str) else {
            continue;
        };
        let Some(value) = normalize_ai_value(key, raw_value) else {
            continue;
        };
        let mut confidence = obj
            .get("confidence")
            .and_then(Value::as_str)
            .and_then(parse_confidence)
            .unwrap_or("unknown");
        let excerpt = obj.get("evidence_excerpt").and_then(Value::as_str);
        if (confidence == "high" || confidence == "medium") && excerpt.is_none() {
            confidence = "low";
        }
        let evidence = match excerpt {
            Some(e) => vec![json!({
                "excerpt": e.chars().take(500).collect::<String>(),
                "thread_local_id": obj
                    .get("evidence_thread_local_id")
                    .and_then(Value::as_i64),
            })],
            None => Vec::new(),
        };
        records.push(AttributeRecord {
            key,
            value,
            confidence,
            source: "ai",
            evidence,
            run_id: ai_run_id,
        });
    }
}

pub async fn compute(
    conn: &mut Connection,
    provider: &dyn LocalAiProvider,
    model: &str,
    endpoint_id: Option<&str>,
    conversation_id: i64,
    force: bool,
) -> Result<Option<AttributeSnapshot>> {
    let Some(prepared) = prepare_computation(conn, conversation_id, model, force)? else {
        return Ok(None);
    };
    let fresh = match &prepared.prompt {
        Some(prompt) if prepared.cached.is_none() => {
            run_extraction(conn, provider, model, prompt).await
        }
        _ => None,
    };
    finish_computation(conn, prepared, fresh, model, endpoint_id)
}

fn is_ai_slot(key: AiAttributeKey) -> bool {
    matches!(
        key,
        AiAttributeKey::Intent
            | AiAttributeKey::Product
            | AiAttributeKey::Feature
            | AiAttributeKey::Issue
            | AiAttributeKey::CustomerGoal
            | AiAttributeKey::ResponseStyle
    )
}

fn parse_confidence(s: &str) -> Option<&'static str> {
    match s {
        "high" => Some("high"),
        "medium" => Some("medium"),
        "low" => Some("low"),
        "unknown" => Some("unknown"),
        _ => None,
    }
}

fn normalize_ai_value(key: AiAttributeKey, value: &str) -> Option<String> {
    let v = value.trim();
    if v.is_empty() {
        return None;
    }
    if key.value_type() == AttributeValueType::Enum && !key.values().contains(&v) {
        return None;
    }
    Some(v.chars().take(300).collect())
}

fn build_extraction_user(
    conn: &Connection,
    conversation_id: i64,
    messages: &[CustomerMessage],
) -> String {
    let subject: String = conn
        .query_row(
            "SELECT COALESCE(subject, '') FROM conversations WHERE id = ?1",
            params![conversation_id],
            |r| r.get(0),
        )
        .unwrap_or_default();
    let subject: String = subject.chars().take(300).collect();
    let mut parts = vec![
        format!("SUBJECT: {subject}"),
        "CUSTOMER MESSAGES (oldest first):".into(),
    ];
    for m in messages {
        parts.push(format!("  [thread {}] {}", m.thread_local_id, m.text));
    }
    parts.push(String::new());
    parts.push("Extract the attributes that are actually supported by these messages.".into());
    parts.join("\n")
}

/// What one completed extraction run must persist (clean-audit item 37's
/// ledger row: the hash-keyed cache columns plus the cost ledger's raw
/// material).
struct ExtractionRunRow<'a> {
    conversation_id: i64,
    input_hash: &'a str,
    model: &'a str,
    output: &'a str,
    latency_ms: u64,
    usage: Option<&'a crate::ai_provider::TokenUsage>,
    endpoint_id: Option<&'a str>,
}

fn store_extraction_run(conn: &Connection, row: ExtractionRunRow<'_>) -> Result<i64> {
    let usage_json = match row.usage {
        Some(u) => serde_json::to_string(u)?,
        None => "null".to_string(),
    };
    conn.execute(
        "INSERT INTO ai_runs (input_hash, prompt_version, model, response_json,
                              type, conversation_id, status, latency_ms,
                              started_at, completed_at, token_usage, endpoint_id)
         VALUES (?1, ?2, ?3, ?4, 'attributes', ?5, 'completed', ?6,
                 datetime('now'), datetime('now'), ?7, ?8)",
        params![
            row.input_hash,
            ATTRIBUTE_EXTRACTION_PROMPT_VERSION,
            row.model,
            row.output,
            row.conversation_id,
            row.latency_ms as i64,
            usage_json,
            row.endpoint_id
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

fn lookup_cached_extraction(
    conn: &Connection,
    input_hash: &str,
    model: &str,
) -> Result<Option<(i64, Value)>> {
    // Clean-audit item 37: only completed runs are cache hits — a
    // pre-fix row stuck at the 'queued' default is treated as a miss so
    // the next computation re-runs and stores a proper completed row
    // (self-healing, newest-first below finds it thereafter).
    let row: Option<(i64, String)> = conn
        .query_row(
            "SELECT id, response_json FROM ai_runs
             WHERE input_hash = ?1 AND prompt_version = ?2 AND model = ?3
               AND status = 'completed'
             ORDER BY created_at DESC, id DESC LIMIT 1",
            params![input_hash, ATTRIBUTE_EXTRACTION_PROMPT_VERSION, model],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .ok();
    Ok(row.and_then(|(id, json)| serde_json::from_str::<Value>(&json).ok().map(|v| (id, v))))
}

const TECHNICAL_VOCAB: &[&str] = &[
    "api",
    "endpoint",
    "webhook",
    "payload",
    "json",
    "oauth",
    "token",
    "http",
    "https",
    "ssl",
    "tls",
    "dns",
    "timezone",
    "utc",
    "cron",
    "queue",
    "cache",
    "latency",
    "http status",
    "401",
    "403",
    "404",
    "500",
    "console",
    "log",
    "stack trace",
    "exception",
    "database",
    "sql",
    "index",
    "migration",
    "deploy",
    "build",
    "header",
    "request",
    "response",
    "callback",
    "integration",
    "sdk",
    "environment variable",
    "rate limit",
];
const URGENCY_MARKERS: &[&str] = &[
    "urgent",
    "asap",
    "immediately",
    "right now",
    "today",
    "as soon as possible",
    "deadline",
    "before our",
    "blocking",
    "production is down",
    "outage",
    "critical",
];
const FRUSTRATION_MARKERS: &[&str] = &[
    "again",
    "still",
    "already",
    "third time",
    "multiple times",
    "repeatedly",
    "frustrat",
    "unacceptable",
    "ridiculous",
    "seriously",
    "not working",
    "never works",
    "every time",
    "tired of",
    "disappointed",
    "kept",
    "nobody",
    "no one",
    "still not",
];
const ESCALATION_MARKERS: &[&str] = &[
    "escalate",
    "manager",
    "supervisor",
    "complaint",
    "legal",
    "cancel our",
    "terminate",
    "switching to",
];
const ACTION_EXPECTATION_MARKERS: &[&str] = &[
    "fix",
    "resolve",
    "restore",
    "refund",
    "escalate",
    "asap",
    "immediately",
    "compensation",
];

fn find_evidence(messages: &[CustomerMessage], hit: impl Fn(&str) -> bool) -> Option<Value> {
    for m in messages {
        if hit(&m.text.to_lowercase()) {
            return Some(json!({
                "excerpt": excerpt(&m.text, 220),
                "thread_local_id": m.thread_local_id,
            }));
        }
    }
    None
}

fn excerpt(text: &str, max_len: usize) -> String {
    let clean: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if clean.chars().count() <= max_len {
        clean
    } else {
        let head: String = clean.chars().take(max_len - 3).collect();
        format!("{head}\u{2026}")
    }
}

fn is_word_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

fn has_marker(haystack_lower: &str, marker: &str) -> bool {
    let bytes = haystack_lower.as_bytes();
    let mut start = 0usize;
    while let Some(pos) = haystack_lower[start..].find(marker) {
        let abs = start + pos;
        let end = abs + marker.len();
        let before_ok = abs == 0 || !is_word_byte(bytes[abs - 1]);
        let after_ok = end >= bytes.len() || !is_word_byte(bytes[end]);
        if before_ok && after_ok {
            return true;
        }
        start = abs + 1;
    }
    false
}

fn count_markers(haystack_lower: &str, markers: &[&str]) -> usize {
    markers
        .iter()
        .filter(|m| has_marker(haystack_lower, m))
        .count()
}

struct DeterministicSignals {
    urgency: &'static str,
    urgency_confidence: &'static str,
    urgency_evidence: Option<Value>,
    frustration: &'static str,
    frustration_confidence: &'static str,
    frustration_evidence: Option<Value>,
    technical: &'static str,
    technical_confidence: &'static str,
    technical_evidence: Option<Value>,
    escalation: bool,
    escalation_evidence: Option<Value>,
    question_count: usize,
    response_style: Option<&'static str>,
    response_style_evidence: Option<Value>,
    customer_goal: Option<String>,
}

fn derive_signals(messages: &[CustomerMessage]) -> Option<DeterministicSignals> {
    if messages.is_empty() {
        return None;
    }
    let all: String = messages
        .iter()
        .map(|m| m.text.to_lowercase())
        .collect::<Vec<_>>()
        .join("\n");

    let urgency_hits = count_markers(&all, URGENCY_MARKERS);
    let urgency = if urgency_hits >= 3
        || ["production is down", "outage", "emergency"]
            .iter()
            .any(|p| all.contains(p))
    {
        "high"
    } else if urgency_hits >= 1 {
        "moderate"
    } else {
        "none"
    };
    let urgency_confidence = if urgency_hits >= 1 { "medium" } else { "low" };
    let urgency_evidence = find_evidence(messages, |t| {
        URGENCY_MARKERS.iter().any(|m| has_marker(t, m))
    });

    let frustration_hits = count_markers(&all, FRUSTRATION_MARKERS);
    let frustration = if frustration_hits >= 3 {
        "strong"
    } else if frustration_hits >= 1 {
        "moderate"
    } else {
        "none"
    };
    let frustration_confidence = if frustration_hits >= 1 {
        "medium"
    } else {
        "low"
    };
    let frustration_evidence = find_evidence(messages, |t| {
        FRUSTRATION_MARKERS.iter().any(|m| has_marker(t, m))
    });

    let tech_hits = count_markers(&all, TECHNICAL_VOCAB);
    let technical = if tech_hits >= 4 {
        "highly_technical"
    } else if tech_hits >= 2 {
        "technical"
    } else if tech_hits == 1 {
        "mixed"
    } else {
        "non_technical"
    };
    let technical_confidence = if tech_hits >= 1 { "medium" } else { "low" };
    let technical_evidence = find_evidence(messages, |t| {
        TECHNICAL_VOCAB.iter().any(|v| has_marker(t, v))
    });

    let escalation = ESCALATION_MARKERS.iter().any(|m| has_marker(&all, m));
    let escalation_evidence = find_evidence(messages, |t| {
        ESCALATION_MARKERS.iter().any(|m| has_marker(t, m))
    });

    let question_count = messages.iter().map(|m| m.text.matches('?').count()).sum();

    let (response_style, response_style_evidence) = if all.contains("concise")
        || has_marker(&all, "short answer")
        || has_marker(&all, "no lengthy")
        || has_marker(&all, "brief answer")
        || has_marker(&all, "be brief")
        || all.contains("keep it short")
        || all.contains("keep this short")
        || all.contains("keep it brief")
        || all.contains("keep this brief")
    {
        (
            Some("concise"),
            find_evidence(messages, |t| {
                t.contains("concise")
                    || t.contains("short answer")
                    || t.contains("no lengthy")
                    || t.contains("brief answer")
                    || t.contains("be brief")
                    || t.contains("keep it short")
                    || t.contains("keep this short")
                    || t.contains("keep it brief")
                    || t.contains("keep this brief")
            }),
        )
    } else if all.contains("step by step")
        || all.contains("step-by-step")
        || all.contains("walk me through")
        || has_marker(&all, "instructions")
    {
        (
            Some("step_by_step"),
            find_evidence(messages, |t| {
                t.contains("step by step")
                    || t.contains("step-by-step")
                    || t.contains("walk me through")
                    || t.contains("instructions")
            }),
        )
    } else if has_marker(&all, "detailed")
        || has_marker(&all, "thorough")
        || all.contains("in depth")
        || all.contains("in-depth")
        || all.contains("full explanation")
        || has_marker(&all, "comprehensive")
    {
        (
            Some("detailed"),
            find_evidence(messages, |t| {
                t.contains("detailed")
                    || t.contains("thorough")
                    || t.contains("in depth")
                    || t.contains("in-depth")
                    || t.contains("full explanation")
                    || t.contains("comprehensive")
            }),
        )
    } else {
        (None, None)
    };

    let customer_goal = messages
        .first()
        .map(|m| excerpt(&m.text, 300))
        .filter(|g| !g.trim().is_empty());

    Some(DeterministicSignals {
        urgency,
        urgency_confidence,
        urgency_evidence,
        frustration,
        frustration_confidence,
        frustration_evidence,
        technical,
        technical_confidence,
        technical_evidence,
        escalation,
        escalation_evidence,
        question_count,
        response_style,
        response_style_evidence,
        customer_goal,
    })
}

pub fn deterministic_records(
    conn: &Connection,
    conversation_id: i64,
) -> Result<Vec<AttributeRecord>> {
    let mut out: Vec<AttributeRecord> = Vec::new();
    let messages = customer_messages(conn, conversation_id)?;
    let signals = derive_signals(&messages);

    if let Some(s) = &signals {
        let mk = |key: AiAttributeKey,
                  value: String,
                  confidence: &'static str,
                  evidence: Vec<Value>| AttributeRecord {
            key,
            value,
            confidence,
            source: "deterministic",
            evidence,
            run_id: None,
        };
        let evidence_vec =
            |e: &Option<Value>| e.as_ref().map(|v| vec![v.clone()]).unwrap_or_default();

        out.push(mk(
            AiAttributeKey::Urgency,
            s.urgency.to_string(),
            s.urgency_confidence,
            evidence_vec(&s.urgency_evidence),
        ));
        out.push(mk(
            AiAttributeKey::FrustrationCues,
            s.frustration.to_string(),
            s.frustration_confidence,
            evidence_vec(&s.frustration_evidence),
        ));
        out.push(mk(
            AiAttributeKey::TechnicalFamiliarity,
            s.technical.to_string(),
            s.technical_confidence,
            evidence_vec(&s.technical_evidence),
        ));
        out.push(mk(
            AiAttributeKey::EscalationSignal,
            if s.escalation { "true" } else { "false" }.to_string(),
            if s.escalation { "medium" } else { "low" },
            if s.escalation {
                evidence_vec(&s.escalation_evidence)
            } else {
                Vec::new()
            },
        ));
        out.push(mk(
            AiAttributeKey::QuestionCount,
            s.question_count.to_string(),
            "high",
            Vec::new(),
        ));
        if let Some(style) = s.response_style {
            out.push(mk(
                AiAttributeKey::ResponseStyle,
                style.to_string(),
                "high",
                evidence_vec(&s.response_style_evidence),
            ));
        }

        let risk = if s.escalation
            || (s.urgency == "high" && (s.frustration == "moderate" || s.frustration == "strong"))
        {
            ("high", "medium")
        } else if s.urgency != "none"
            || s.frustration != "none"
            || count_markers(
                &messages
                    .iter()
                    .map(|m| m.text.to_lowercase())
                    .collect::<Vec<_>>()
                    .join("\n"),
                ACTION_EXPECTATION_MARKERS,
            ) >= 1
        {
            ("medium", "low")
        } else {
            ("low", "low")
        };
        let risk_evidence: Vec<Value> = [
            &s.urgency_evidence,
            &s.frustration_evidence,
            &s.escalation_evidence,
        ]
        .into_iter()
        .flatten()
        .take(3)
        .cloned()
        .collect();
        out.push(mk(
            AiAttributeKey::Risk,
            risk.0.to_string(),
            risk.1,
            risk_evidence,
        ));

        if let Some(goal) = &s.customer_goal {
            out.push(mk(
                AiAttributeKey::CustomerGoal,
                goal.clone(),
                "low",
                Vec::new(),
            ));
        }
    }

    let linked: Option<i64> = conn
        .query_row(
            "SELECT 1 FROM known_issue_links WHERE conversation_id = ?1 LIMIT 1",
            params![conversation_id],
            |r| r.get(0),
        )
        .ok()
        .flatten();
    out.push(AttributeRecord {
        key: AiAttributeKey::KnownIssue,
        value: if linked.is_some() { "true" } else { "false" }.to_string(),
        confidence: "high",
        source: "deterministic",
        evidence: Vec::new(),
        run_id: None,
    });

    let cluster: Option<String> = conn
        .query_row(
            "SELECT ic.name FROM issue_cluster_conversations icm
             JOIN issue_clusters ic ON ic.id = icm.cluster_id
             WHERE icm.conversation_id = ?1
             ORDER BY ic.id DESC LIMIT 1",
            params![conversation_id],
            |r| r.get(0),
        )
        .ok()
        .flatten();
    if let Some(name) = cluster {
        out.push(AttributeRecord {
            key: AiAttributeKey::IssueCluster,
            value: name.chars().take(300).collect(),
            confidence: "medium",
            source: "deterministic",
            evidence: Vec::new(),
            run_id: None,
        });
    }

    Ok(out)
}

/// A2 entry point: attribute extraction routed through the provider hub.
/// With a live backend the AI layer fills intent/product/feature/issue/
/// customer_goal/response_style; with AI disabled the deterministic
/// layer still lands so the Attributes tab and automation conditions
/// stay alive without any model.
pub async fn compute_with_backend(
    conn: &mut Connection,
    backend: &crate::ai_pipeline::AiBackend,
    conversation_id: i64,
    force: bool,
) -> Result<Option<AttributeSnapshot>> {
    match backend {
        crate::ai_pipeline::AiBackend::LmStudio {
            client,
            model,
            endpoint_id,
        } => {
            let provider = crate::ai_lm_studio::LmStudioProvider::with_client(client.clone());
            compute(
                conn,
                &provider,
                model.as_deref().unwrap_or(""),
                endpoint_id.as_deref(),
                conversation_id,
                force,
            )
            .await
        }
        crate::ai_pipeline::AiBackend::Disabled => {
            // No model configured: deterministic-only, still a real snapshot.
            if let Some(prepared) = prepare_computation(conn, conversation_id, "", force)? {
                finish_computation(conn, prepared, None, "", None)
            } else {
                Ok(None)
            }
        }
    }
}

#[cfg(test)]
mod a2_wiring_tests {
    use super::*;

    fn db_with_conversation() -> Connection {
        let mut conn = Connection::open_in_memory().expect("db");
        crate::bootstrap::apply_all(&mut conn).expect("migrations");
        conn.execute(
            "INSERT INTO conversations (remote_id, number, subject, preview, status)
             VALUES (9101, 91, 'Export failing again', 'preview', 'active')",
            [],
        )
        .expect("conversation");
        let conv = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO conversation_threads (conversation_id, type, body_text, created_at)
             VALUES (?1, 'customer', ?2, datetime('now'))",
            rusqlite::params![
                conv,
                "This is urgent — the export throws an error every time \
                 and we cannot work. I want a manager. How do I fix the export timeout?"
            ],
        )
        .expect("thread");
        conn
    }

    /// A2: with no model at all, the deterministic layer still lands the
    /// snapshot — the Attributes tab and automation conditions go live
    /// without any endpoint configured.
    #[test]
    fn compute_with_disabled_backend_lands_deterministic_attributes() {
        let mut conn = db_with_conversation();
        let conv: i64 = conn
            .query_row("SELECT id FROM conversations", [], |r| r.get(0))
            .expect("conv id");
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        let snapshot = rt
            .block_on(compute_with_backend(
                &mut conn,
                &crate::ai_pipeline::AiBackend::Disabled,
                conv,
                false,
            ))
            .expect("compute")
            .expect("snapshot");
        let has = |key: &str| {
            snapshot
                .attributes
                .iter()
                .any(|a| a.get("attribute").and_then(serde_json::Value::as_str) == Some(key))
        };
        assert!(has("urgency"), "urgency must be deterministic");
        assert!(has("question_count"));
        assert!(has("escalation_signal"));
    }

    /// A2: a human correction supersedes the machine row — the chip editor
    /// and automation conditions read the corrected value.
    #[test]
    fn human_override_supersedes_the_machine_row() {
        let mut conn = db_with_conversation();
        let conv: i64 = conn
            .query_row("SELECT id FROM conversations", [], |r| r.get(0))
            .expect("conv id");
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        rt.block_on(compute_with_backend(
            &mut conn,
            &crate::ai_pipeline::AiBackend::Disabled,
            conv,
            false,
        ))
        .expect("compute");
        upsert_single(
            &mut conn,
            conv,
            AttributeRecord {
                key: AiAttributeKey::Intent,
                value: "question".into(),
                confidence: "high",
                source: "human",
                evidence: Vec::new(),
                run_id: None,
            },
        )
        .expect("override");
        let current = current_for_conversation(&conn, conv).expect("rows");
        let intents: Vec<_> = current.iter().filter(|r| r.attribute == "intent").collect();
        assert_eq!(intents.len(), 1, "the old row must be superseded");
        assert_eq!(intents[0].value, "question");
        assert_eq!(intents[0].source, "human");
    }
}

/// Clean-audit batch 22 (items 34-35): the attribute extraction parses
/// raw model output without the pipeline's fence-stripping extractor, and
/// its prompt — raw customer messages — bypassed the prompt-redaction
/// boundary every other AI path crosses.
#[cfg(test)]
mod batch22_audit_tests {
    use super::*;
    use crate::ai_provider::{ChatResponse, LocalAiProvider, ModelInfo};

    fn db_with_conversation() -> Connection {
        let mut conn = Connection::open_in_memory().expect("db");
        crate::bootstrap::apply_all(&mut conn).expect("migrations");
        conn.execute(
            "INSERT INTO conversations (remote_id, number, subject, preview, status)
             VALUES (9301, 93, 'Export failing again', 'preview', 'active')",
            [],
        )
        .expect("conversation");
        let conv = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO conversation_threads (conversation_id, type, body_text, created_at)
             VALUES (?1, 'customer', ?2, datetime('now'))",
            rusqlite::params![
                conv,
                "This is urgent — the export throws an error every time \
                 and we cannot work. How do I fix the export timeout?"
            ],
        )
        .expect("thread");
        conn
    }

    fn conv_id(conn: &Connection) -> i64 {
        conn.query_row("SELECT id FROM conversations", [], |r| r.get(0))
            .expect("conv id")
    }

    /// Item 34: a fenced answer and a prose-wrapped answer both parse —
    /// the strict `serde_json::from_str` on the raw string used to drop
    /// every such extraction (and never even stored the run).
    #[test]
    fn fenced_and_prose_wrapped_extraction_outputs_are_parsed() {
        for (label, raw) in [
            (
                "fenced",
                "```json\n{\"attributes\":[{\"attribute\":\"intent\",\"value\":\"bug_report\",\"confidence\":\"high\",\"evidence_excerpt\":\"export throws an error\"}]}\n```",
            ),
            (
                "prose-wrapped",
                "Sure! Here is the JSON you asked for:\n{\"attributes\":[{\"attribute\":\"intent\",\"value\":\"bug_report\",\"confidence\":\"high\",\"evidence_excerpt\":\"export throws an error\"}]}\nHope that helps.",
            ),
        ] {
            let mut conn = db_with_conversation();
            let conv = conv_id(&conn);
            let prepared = prepare_computation(&conn, conv, "test-model", true)
                .expect("prepared")
                .expect("a prepared computation exists");
            assert!(prepared.cached.is_none(), "{label}: no cache under force");
            assert!(prepared.prompt.is_some(), "{label}: prompt built");

            let snapshot = finish_computation(
                &mut conn,
                prepared,
                Some(ExtractionOutput {
                    content: raw.to_string(),
                    latency_ms: 42,
                    usage: Some(crate::ai_provider::TokenUsage {
                        prompt_tokens: 11,
                        completion_tokens: 7,
                        total_tokens: 18,
                    }),
                }),
                "test-model",
                Some("lmstudio-local"),
            )
            .expect("finish")
            .expect("snapshot");

            let intent = snapshot
                .attributes
                .iter()
                .find(|a| a.get("attribute").and_then(Value::as_str) == Some("intent"));
            let Some(intent) = intent else {
                panic!(
                    "{label}: the AI intent attribute must land, got {:?}",
                    snapshot.attributes
                );
            };
            assert_eq!(
                intent.get("value").and_then(Value::as_str),
                Some("bug_report"),
                "{label}: value survives the fenced parse"
            );
            assert_eq!(
                intent.get("source").and_then(Value::as_str),
                Some("ai"),
                "{label}: the row is the machine's, not deterministic"
            );
            assert!(
                intent.get("run_id").and_then(Value::as_i64).is_some(),
                "{label}: the run is stored even for a wrapped answer"
            );
        }
    }

    /// A recording provider for the redaction-boundary test: counts which
    /// chat lane was used and captures exactly what the model received.
    struct RecordingProvider {
        seen: std::sync::Mutex<Vec<crate::ai_provider::ChatMessage>>,
        structured_calls: std::sync::atomic::AtomicUsize,
        plain_calls: std::sync::atomic::AtomicUsize,
    }

    impl RecordingProvider {
        fn new() -> Self {
            Self {
                seen: std::sync::Mutex::new(Vec::new()),
                structured_calls: std::sync::atomic::AtomicUsize::new(0),
                plain_calls: std::sync::atomic::AtomicUsize::new(0),
            }
        }

        fn captured_user_content(&self) -> String {
            self.seen
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .iter()
                .find(|m| m.role == "user")
                .map(|m| m.content.clone())
                .expect("the model must receive a user message")
        }
    }

    #[async_trait::async_trait]
    impl LocalAiProvider for RecordingProvider {
        async fn list_models(&self) -> crate::error::Result<Vec<ModelInfo>> {
            Ok(Vec::new())
        }

        async fn chat(
            &self,
            _model: &str,
            _messages: &[crate::ai_provider::ChatMessage],
        ) -> crate::error::Result<ChatResponse> {
            self.plain_calls
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            Err(crate::error::Error::Config(
                "plain chat must not be used".into(),
            ))
        }

        async fn chat_structured(
            &self,
            _model: &str,
            messages: &[crate::ai_provider::ChatMessage],
        ) -> crate::error::Result<ChatResponse> {
            self.structured_calls
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            *self.seen.lock().unwrap_or_else(|p| p.into_inner()) = messages.to_vec();
            Ok(ChatResponse {
                content: "{\"attributes\":[]}".into(),
                model: "recording".into(),
                usage: None,
                finish_reason: None,
            })
        }

        async fn embed(
            &self,
            _model: &str,
            _text: &str,
        ) -> crate::error::Result<crate::ai_provider::EmbedResponse> {
            Err(crate::error::Error::Config("not needed".into()))
        }

        async fn is_available(&self) -> bool {
            true
        }
    }

    /// Item 35: the extraction prompt carries raw customer messages —
    /// boundary-covered PII (card numbers, credentials) must never reach
    /// the model, and the boundary honors the `redaction_enabled`
    /// setting like every other AI path.
    #[test]
    fn run_extraction_crosses_the_redaction_boundary() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        let prompt = "Customer wrote: my card 4111 1111 1111 1111 was charged twice \
                      and my password: hunter2secret, the export is broken.";

        // Default: redaction on — the model sees placeholders, not PII.
        {
            let conn = db_with_conversation();
            let provider = RecordingProvider::new();
            let out = rt
                .block_on(run_extraction(&conn, &provider, "test-model", prompt))
                .expect("an answer");
            assert_eq!(out.content, "{\"attributes\":[]}");
            let captured = provider.captured_user_content();
            assert!(
                !captured.contains("4111 1111 1111 1111"),
                "the card number must be redacted before the model: {captured}"
            );
            assert!(
                !captured.contains("hunter2secret"),
                "the credential must be redacted before the model: {captured}"
            );
            assert!(
                captured.contains("[REDACTED-CARD]"),
                "the boundary's placeholder is what the model sees: {captured}"
            );
            assert!(
                captured.contains("the export is broken"),
                "the non-PII text survives: {captured}"
            );
            assert_eq!(
                provider
                    .structured_calls
                    .load(std::sync::atomic::Ordering::Relaxed),
                1,
                "item 34: the structured (json_mode) lane is the one used"
            );
            assert_eq!(
                provider
                    .plain_calls
                    .load(std::sync::atomic::Ordering::Relaxed),
                0,
                "item 34: the plain chat lane must not be used"
            );
        }

        // The setting is the switch: redaction off passes the text through.
        {
            let conn = db_with_conversation();
            crate::settings::set_bool(&conn, "redaction_enabled", false).expect("setting");
            let provider = RecordingProvider::new();
            rt.block_on(run_extraction(&conn, &provider, "test-model", prompt))
                .expect("an answer");
            let captured = provider.captured_user_content();
            assert_eq!(
                captured, prompt,
                "redaction_enabled=false is honored verbatim"
            );
        }
    }
}

#[cfg(test)]
mod batch23_audit_tests {
    use super::*;
    use rusqlite::Connection;

    fn db() -> Connection {
        let mut conn = Connection::open_in_memory().expect("db");
        crate::bootstrap::apply_all(&mut conn).expect("migrations");
        conn
    }

    fn seed_conversation(conn: &Connection) -> i64 {
        conn.execute(
            "INSERT INTO conversations (remote_id, number, subject, preview, status)
             VALUES (9301, 93, 'Export failing', 'preview', 'active')",
            [],
        )
        .expect("conversation");
        conn.last_insert_rowid()
    }

    fn add_customer_message(conn: &Connection, conv: i64, text: &str, stamp: &str) -> i64 {
        conn.execute(
            "INSERT INTO conversation_threads (conversation_id, type, body_text, created_at)
             VALUES (?1, 'customer', ?2, ?3)",
            rusqlite::params![conv, text, stamp],
        )
        .expect("thread");
        conn.last_insert_rowid()
    }

    /// Item 36: the extraction window is the NEWEST 30 live customer
    /// messages, oldest-first — soft-deleted threads and whitespace-only
    /// bodies are excluded, and the recent context is no longer cut off
    /// in favor of the OLDEST 30.
    #[test]
    fn customer_messages_takes_newest_30_and_skips_soft_deleted() {
        let conn = db();
        let conv = seed_conversation(&conn);
        // 32 live messages, numbered by timestamp.
        let mut oldest_id = 0;
        for i in 1..=32 {
            let id = add_customer_message(
                &conn,
                conv,
                &format!("live message {i}"),
                &format!("2026-10-{:02}T10:00:00Z", i),
            );
            if i == 1 {
                oldest_id = id;
            }
        }
        // A soft-deleted thread newer than everything.
        let deleted = add_customer_message(
            &conn,
            conv,
            "DELETED-TEXT must never reach the prompt",
            "2026-10-40T10:00:00Z",
        );
        conn.execute(
            "UPDATE conversation_threads SET deleted_at = datetime('now') WHERE id = ?1",
            rusqlite::params![deleted],
        )
        .expect("soft delete");
        // A whitespace-only body, also newest.
        add_customer_message(&conn, conv, "   \n\t ", "2026-10-41T10:00:00Z");

        let messages = customer_messages(&conn, conv).expect("messages");
        assert_eq!(messages.len(), 30, "exactly the newest 30 live messages");
        // Chronological order (the prompt's declared order).
        let ids: Vec<i64> = messages.iter().map(|m| m.thread_local_id).collect();
        let mut sorted = ids.clone();
        sorted.sort();
        assert_eq!(ids, sorted, "presented oldest-first");
        // The window is messages 3..=32 (newest 30), not 1..=30.
        assert!(
            messages.iter().any(|m| m.text.contains("live message 32")),
            "the newest message is included: {:?}",
            messages.last().map(|m| m.text.clone())
        );
        assert!(
            !messages.iter().any(|m| m.thread_local_id == oldest_id),
            "the OLDEST message must fall out of the window"
        );
        assert!(
            !messages.iter().any(|m| m.text.contains("DELETED-TEXT")),
            "soft-deleted threads never reach the prompt"
        );
        assert!(
            messages
                .last()
                .is_some_and(|m| m.text.contains("live message 32")),
            "newest live message is the last (chronological) entry"
        );
    }

    /// Item 37: a fresh extraction lands as a COMPLETED 'attributes' run
    /// carrying the cost ledger's raw material (latency, token usage,
    /// endpoint) — `usage_ledger`'s `WHERE status='completed'` sees it.
    #[test]
    fn extraction_run_lands_completed_in_the_cost_ledger() {
        let mut conn = db();
        let conv = seed_conversation(&conn);
        add_customer_message(
            &conn,
            conv,
            "The export throws an error every time.",
            "2026-10-01T10:00:00Z",
        );

        let prepared = prepare_computation(&conn, conv, "test-model", true)
            .expect("prepared")
            .expect("exists");
        let snapshot = finish_computation(
            &mut conn,
            prepared,
            Some(ExtractionOutput {
                content: "{\"attributes\":[{\"attribute\":\"intent\",\"value\":\"bug_report\",\"confidence\":\"high\",\"evidence_excerpt\":\"export throws an error\"}]}".into(),
                latency_ms: 42,
                usage: Some(crate::ai_provider::TokenUsage {
                    prompt_tokens: 11,
                    completion_tokens: 7,
                    total_tokens: 18,
                }),
            }),
            "test-model",
            Some("lmstudio-local"),
        )
        .expect("finish")
        .expect("snapshot");

        let run_id = snapshot
            .attributes
            .iter()
            .find_map(|a| a.get("run_id").and_then(Value::as_i64))
            .expect("the AI attribute carries its run id");

        let row = conn
            .query_row(
                "SELECT type, status, latency_ms, token_usage, endpoint_id, conversation_id
                   FROM ai_runs WHERE id = ?1",
                rusqlite::params![run_id],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, i64>(2)?,
                        r.get::<_, String>(3)?,
                        r.get::<_, Option<String>>(4)?,
                        r.get::<_, Option<i64>>(5)?,
                    ))
                },
            )
            .expect("the run row exists");
        assert_eq!(
            row.0, "attributes",
            "the run is typed for the ledger's per-feature rows"
        );
        assert_eq!(row.1, "completed", "not stuck at the 'queued' default");
        assert_eq!(row.2, 42, "latency recorded");
        assert!(
            row.3.contains("\"prompt_tokens\":11") && row.3.contains("\"completion_tokens\":7"),
            "token usage recorded: {}",
            row.3
        );
        assert_eq!(
            row.4.as_deref(),
            Some("lmstudio-local"),
            "endpoint recorded"
        );
        assert_eq!(row.5, Some(conv), "the run is pinned to its conversation");

        // The ledger itself sees the run (tokens attributed, feature row).
        let ledger = crate::ai_hub::usage_ledger(&conn, 7).expect("ledger");
        let row = ledger
            .iter()
            .find(|r| r.feature == "attributes")
            .expect("the extraction run appears in the cost ledger");
        assert_eq!(row.prompt_tokens, 11);
        assert_eq!(row.completion_tokens, 7);
        assert_eq!(row.runs, 1);
    }

    /// Item 37 (self-heal): a pre-fix row stuck at the 'queued' default
    /// with the same hash is NOT a cache hit — the next computation
    /// re-runs and stores a proper completed row.
    #[test]
    fn stuck_queued_extraction_rows_self_heal() {
        let mut conn = db();
        let conv = seed_conversation(&conn);
        add_customer_message(
            &conn,
            conv,
            "The export throws an error every time.",
            "2026-10-01T10:00:00Z",
        );

        // Discover the hash the old writer would have used.
        let prepared = prepare_computation(&conn, conv, "test-model", true)
            .expect("prepared")
            .expect("exists");
        let hash = prepared.input_hash.clone();

        // The pre-fix row: bare columns, status defaults to 'queued'.
        conn.execute(
            "INSERT INTO ai_runs (input_hash, prompt_version, model, response_json)
             VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![
                hash,
                ATTRIBUTE_EXTRACTION_PROMPT_VERSION,
                "test-model",
                "{\"attributes\":[]}"
            ],
        )
        .expect("stuck row");

        // force=false: the queued row must not serve as a cache hit.
        let next = prepare_computation(&conn, conv, "test-model", false)
            .expect("prepared")
            .expect("exists");
        assert!(
            next.cached.is_none(),
            "a stuck 'queued' row is a cache miss, not a hit"
        );

        // Completing the recomputation stores a proper completed row...
        finish_computation(
            &mut conn,
            next,
            Some(ExtractionOutput {
                content: "{\"attributes\":[]}".into(),
                latency_ms: 5,
                usage: None,
            }),
            "test-model",
            None,
        )
        .expect("finish")
        .expect("snapshot");

        // ...and only the completed row is a hit from now on.
        let after = prepare_computation(&conn, conv, "test-model", false)
            .expect("prepared")
            .expect("exists");
        let (cached_id, _) = after.cached.expect("the completed row is the cache hit");
        let status: String = conn
            .query_row(
                "SELECT status FROM ai_runs WHERE id = ?1",
                rusqlite::params![cached_id],
                |r| r.get(0),
            )
            .expect("status");
        assert_eq!(status, "completed");
    }
}
