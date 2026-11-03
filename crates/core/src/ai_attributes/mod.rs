use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::ai_provider::{ChatMessage, LocalAiProvider};
use crate::catalog::{AI_ATTRIBUTE_SCHEMA_VERSION, AiAttributeKey, AttributeValueType};
use crate::error::{Error, Result};
use crate::sync_schema::table_exists;

pub(crate) const M033_SQL: &str = r#"
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

fn column_exists(conn: &Connection, table: &str, column: &str) -> Result<bool> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let cols: Vec<String> = stmt
        .query_map([], |row| row.get::<_, String>(1))?
        .filter_map(|c| c.ok())
        .collect();
    Ok(cols.iter().any(|c| c == column))
}

type LegacyM010Row = (i64, i64, String, String, String, String, f64, String);

pub(crate) fn apply_m033(conn: &Connection) -> Result<()> {
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

pub(crate) fn save_snapshot(
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

fn parse_js_number(raw: &str) -> Option<f64> {
    let t = raw.trim();
    if t.is_empty() {
        return Some(0.0);
    }
    t.parse::<f64>().ok()
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

mod extraction;
pub(crate) use extraction::compute_with_backend;

#[cfg(test)]
mod a2_wiring_tests {
    use super::extraction::compute_with_backend;
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

#[cfg(test)]
mod batch22_audit_tests {
    use super::extraction::{
        ExtractionOutput, finish_computation, prepare_computation, run_extraction,
    };
    use super::*;
    use crate::ai_provider::{ChatResponse, LocalAiProvider};

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

        async fn is_available(&self) -> bool {
            true
        }
    }

    #[test]
    fn run_extraction_crosses_the_redaction_boundary() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        let prompt = "Customer wrote: my card 4111 1111 1111 1111 was charged twice \
                      and my password: hunter2secret, the export is broken.";

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
    use super::extraction::{
        ATTRIBUTE_EXTRACTION_PROMPT_VERSION, ExtractionOutput, customer_messages,
        finish_computation, prepare_computation,
    };
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

    #[test]
    fn customer_messages_takes_newest_30_and_skips_soft_deleted() {
        let conn = db();
        let conv = seed_conversation(&conn);
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
        add_customer_message(&conn, conv, "   \n\t ", "2026-10-41T10:00:00Z");

        let messages = customer_messages(&conn, conv).expect("messages");
        assert_eq!(messages.len(), 30, "exactly the newest 30 live messages");
        let ids: Vec<i64> = messages.iter().map(|m| m.thread_local_id).collect();
        let mut sorted = ids.clone();
        sorted.sort();
        assert_eq!(ids, sorted, "presented oldest-first");
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

        let ledger = crate::ai_hub::usage_ledger(&conn, 7).expect("ledger");
        let row = ledger
            .iter()
            .find(|r| r.feature == "attributes")
            .expect("the extraction run appears in the cost ledger");
        assert_eq!(row.prompt_tokens, 11);
        assert_eq!(row.completion_tokens, 7);
        assert_eq!(row.runs, 1);
    }

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

        let prepared = prepare_computation(&conn, conv, "test-model", true)
            .expect("prepared")
            .expect("exists");
        let hash = prepared.input_hash.clone();

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

        let next = prepare_computation(&conn, conv, "test-model", false)
            .expect("prepared")
            .expect("exists");
        assert!(
            next.cached.is_none(),
            "a stuck 'queued' row is a cache miss, not a hit"
        );

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
