use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::migrations::add_column_if_missing;

pub const M015_TO_M019_SQL: &str = r#"
    -- M015: known_issues + known_issue_links
    CREATE TABLE IF NOT EXISTS known_issues (
        id          INTEGER PRIMARY KEY AUTOINCREMENT,
        name        TEXT NOT NULL,
        status      TEXT NOT NULL DEFAULT 'active',
        description TEXT,
        created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
        updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
    );
    CREATE TABLE IF NOT EXISTS known_issue_links (
        id              INTEGER PRIMARY KEY AUTOINCREMENT,
        known_issue_id  INTEGER NOT NULL REFERENCES known_issues (id) ON DELETE CASCADE,
        conversation_id INTEGER NOT NULL,
        link_type       TEXT NOT NULL DEFAULT 'related'
    );
    CREATE INDEX IF NOT EXISTS idx_known_issue_links
        ON known_issue_links (known_issue_id, conversation_id);

    -- M016: issue_clusters + issue_cluster_conversations
    CREATE TABLE IF NOT EXISTS issue_clusters (
        id              INTEGER PRIMARY KEY AUTOINCREMENT,
        name            TEXT NOT NULL,
        conversation_count INTEGER NOT NULL DEFAULT 0,
        first_seen_at   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
        last_seen_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
        status          TEXT NOT NULL DEFAULT 'active'
    );
    CREATE TABLE IF NOT EXISTS issue_cluster_conversations (
        cluster_id      INTEGER NOT NULL REFERENCES issue_clusters (id) ON DELETE CASCADE,
        conversation_id INTEGER NOT NULL,
        PRIMARY KEY (cluster_id, conversation_id)
    );

    -- M017: incidents
    CREATE TABLE IF NOT EXISTS incidents (
        id              INTEGER PRIMARY KEY AUTOINCREMENT,
        known_issue_id  INTEGER REFERENCES known_issues (id),
        status          TEXT NOT NULL DEFAULT 'investigating',
        severity        TEXT NOT NULL DEFAULT 'sev4',
        source          TEXT NOT NULL DEFAULT 'manual',
        description     TEXT,
        created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
        updated_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
        resolved_at     TEXT
    );

    -- M018: sla_configs + sla_breaches
    CREATE TABLE IF NOT EXISTS sla_configs (
        mailbox_id          INTEGER PRIMARY KEY,
        first_response_hours REAL NOT NULL DEFAULT 24.0,
        resolution_hours     REAL NOT NULL DEFAULT 72.0
    );
    CREATE TABLE IF NOT EXISTS sla_breaches (
        id              INTEGER PRIMARY KEY AUTOINCREMENT,
        conversation_id INTEGER NOT NULL,
        breach_type     TEXT NOT NULL,
        breached_at     TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
    );

    -- M019: knowledge_doc_freshness + knowledge_gaps
    CREATE TABLE IF NOT EXISTS knowledge_doc_freshness (
        doc_id          INTEGER PRIMARY KEY,
        last_synced_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
        content_hash    TEXT,
        stale_at        TEXT
    );
    CREATE TABLE IF NOT EXISTS knowledge_gaps (
        id              INTEGER PRIMARY KEY AUTOINCREMENT,
        query_text      TEXT NOT NULL,
        hit_count       INTEGER NOT NULL DEFAULT 1,
        last_seen_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
    );
    CREATE INDEX IF NOT EXISTS idx_knowledge_gaps_query
        ON knowledge_gaps (query_text);

    UPDATE app_state SET schema_version = 19 WHERE id = 1;
"#;

pub(crate) fn apply_m015_to_m019(conn: &Connection) -> Result<()> {
    conn.execute_batch(M015_TO_M019_SQL)?;
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KnownIssue {
    pub id: Option<i64>,
    pub name: String,
    pub status: String,
    pub description: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

pub fn create_known_issue(conn: &Connection, name: &str, description: Option<&str>) -> Result<i64> {
    conn.execute(
        "INSERT INTO known_issues (name, description) VALUES (?1, ?2)",
        params![name, description],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn refresh_known_issue_counts(conn: &Connection, known_issue_id: i64) -> Result<()> {
    conn.execute(
        "UPDATE known_issues SET
            conversation_count = (SELECT COUNT(*) FROM known_issue_links kil
                                  JOIN conversations c ON c.id = kil.conversation_id
                                  WHERE kil.known_issue_id = ?1),
            first_seen_at = (SELECT MIN(COALESCE(c.remote_created_at, c.created_at)) FROM known_issue_links kil
                               JOIN conversations c ON c.id = kil.conversation_id WHERE kil.known_issue_id = ?1),
            last_seen_at = (SELECT MAX(COALESCE(c.remote_created_at, c.created_at)) FROM known_issue_links kil
                              JOIN conversations c ON c.id = kil.conversation_id WHERE kil.known_issue_id = ?1)
         WHERE id = ?1",
        params![known_issue_id],
    )?;
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IssueCluster {
    pub id: Option<i64>,
    pub name: String,
    pub conversation_count: i64,
    pub first_seen_at: String,
    pub last_seen_at: String,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Incident {
    pub id: Option<i64>,
    pub known_issue_id: Option<i64>,
    pub status: String,
    pub severity: String,
    pub source: String,
    pub description: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub resolved_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SlaConfig {
    pub mailbox_id: i64,
    pub first_response_hours: f64,
    pub resolution_hours: f64,
}

pub fn set_sla_config(
    conn: &Connection,
    mailbox_id: i64,
    first_response_hours: f64,
    resolution_hours: f64,
) -> Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO sla_configs (mailbox_id, first_response_hours, resolution_hours)
         VALUES (?1, ?2, ?3)",
        params![mailbox_id, first_response_hours, resolution_hours],
    )?;
    Ok(())
}

pub fn get_sla_config(conn: &Connection, mailbox_id: i64) -> Result<Option<SlaConfig>> {
    let row: Option<(i64, f64, f64)> = conn
        .query_row(
            "SELECT mailbox_id, first_response_hours, resolution_hours FROM sla_configs WHERE mailbox_id = ?1",
            params![mailbox_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .ok();
    match row {
        None => Ok(None),
        Some((id, fr, res)) => Ok(Some(SlaConfig {
            mailbox_id: id,
            first_response_hours: fr,
            resolution_hours: res,
        })),
    }
}

pub fn record_sla_breach(
    conn: &Connection,
    conversation_id: i64,
    breach_type: &str,
) -> Result<i64> {
    conn.execute(
        "INSERT INTO sla_breaches (conversation_id, breach_type) VALUES (?1, ?2)",
        params![conversation_id, breach_type],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn count_sla_breaches(conn: &Connection) -> Result<u32> {
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM sla_breaches", [], |r| r.get(0))?;
    Ok(u32::try_from(count).unwrap_or(0))
}

pub fn record_knowledge_gap(conn: &Connection, query_text: &str) -> Result<()> {
    let existing: Option<i64> = conn
        .query_row(
            "SELECT hit_count FROM knowledge_gaps WHERE query_text = ?1",
            params![query_text],
            |r| r.get(0),
        )
        .ok();
    if existing.is_some() {
        conn.execute(
            "UPDATE knowledge_gaps SET hit_count = hit_count + 1, last_seen_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
             WHERE query_text = ?1",
            params![query_text],
        )?;
    } else {
        conn.execute(
            "INSERT INTO knowledge_gaps (query_text, hit_count, last_seen_at)
             VALUES (?1, 1, strftime('%Y-%m-%dT%H:%M:%fZ','now'))",
            params![query_text],
        )?;
    }
    Ok(())
}

pub fn list_knowledge_gaps(conn: &Connection, limit: u32) -> Result<Vec<(String, i64)>> {
    let limit = limit.clamp(1, 200);
    let mut stmt = conn.prepare(
        "SELECT query_text, hit_count FROM knowledge_gaps ORDER BY hit_count DESC, last_seen_at DESC LIMIT ?1",
    )?;
    let rows = stmt
        .query_map(params![limit], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub(crate) fn update_doc_freshness(
    conn: &Connection,
    doc_id: i64,
    content_hash: Option<&str>,
    stale: bool,
) -> Result<()> {
    let stale_at = if stale {
        "strftime('%Y-%m-%dT%H:%M:%fZ','now')"
    } else {
        "NULL"
    };
    let sql = format!(
        "INSERT OR REPLACE INTO knowledge_doc_freshness (doc_id, last_synced_at, content_hash, stale_at)
         VALUES (?1, strftime('%Y-%m-%dT%H:%M:%fZ','now'), ?2, {stale_at})"
    );
    conn.execute(&sql, params![doc_id, content_hash])?;
    Ok(())
}

pub fn count_stale_docs(conn: &Connection) -> Result<u32> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM knowledge_doc_freshness WHERE stale_at IS NOT NULL",
        [],
        |r| r.get(0),
    )?;
    Ok(u32::try_from(count).unwrap_or(0))
}

pub(crate) fn apply_m035(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS incident_conversations (
            incident_id     INTEGER NOT NULL REFERENCES incidents (id) ON DELETE CASCADE,
            conversation_id INTEGER NOT NULL,
            linked_by       TEXT NOT NULL DEFAULT 'human',
            linked_at       TEXT NOT NULL DEFAULT (datetime('now')),
            PRIMARY KEY (incident_id, conversation_id)
        );
        CREATE INDEX IF NOT EXISTS idx_incident_conversations_conversation
            ON incident_conversations (conversation_id);

        CREATE TABLE IF NOT EXISTS incident_related (
            incident_id     INTEGER NOT NULL REFERENCES incidents (id) ON DELETE CASCADE,
            target_kind     TEXT NOT NULL,
            target_local_id INTEGER NOT NULL,
            note            TEXT,
            linked_at       TEXT NOT NULL DEFAULT (datetime('now')),
            PRIMARY KEY (incident_id, target_kind, target_local_id)
        );

        CREATE TABLE IF NOT EXISTS incident_refs (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            incident_id INTEGER NOT NULL REFERENCES incidents (id) ON DELETE CASCADE,
            system      TEXT NOT NULL,
            reference   TEXT NOT NULL,
            url         TEXT,
            title       TEXT,
            status      TEXT,
            notes       TEXT,
            created_at  TEXT NOT NULL DEFAULT (datetime('now'))
        );
        CREATE INDEX IF NOT EXISTS idx_incident_refs_incident
            ON incident_refs (incident_id);

        -- v2.1.0 (M5, plan Phase 26) knowledge-gap candidate pipeline.
        -- `status` is the task-specified port vocabulary: an undecided
        -- candidate is 'open' (the reference calls it 'candidate').
        CREATE TABLE IF NOT EXISTS knowledge_gap_candidates (
            id               INTEGER PRIMARY KEY AUTOINCREMENT,
            query_text       TEXT NOT NULL,
            occurrence_count INTEGER NOT NULL DEFAULT 1,
            kind             TEXT,
            status           TEXT NOT NULL DEFAULT 'open'
                CHECK (status IN ('open','approved','rejected')),
            decision_note    TEXT,
            decided_at       TEXT,
            created_at       TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
        );

        -- v1.x interaction overrides (spec #22, #45, #56): a human
        -- response-preference override takes precedence over AI inference.
        CREATE TABLE IF NOT EXISTS interaction_overrides (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            customer_id INTEGER NOT NULL,
            field       TEXT NOT NULL,
            value       TEXT NOT NULL,
            reason      TEXT,
            created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
            updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
            UNIQUE (customer_id, field)
        );

        -- Automation run log (reference engine.record → automation_runs).
        CREATE TABLE IF NOT EXISTS automation_runs (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            rule_id         INTEGER NOT NULL REFERENCES automation_rules (id) ON DELETE CASCADE,
            conversation_id INTEGER,
            triggered_by    TEXT NOT NULL DEFAULT 'manual',
            outcome         TEXT NOT NULL,
            created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
        );
        CREATE INDEX IF NOT EXISTS idx_automation_runs_rule
            ON automation_runs (rule_id);",
    )?;
    add_column_if_missing(conn, "incidents", "title", "TEXT")?;
    add_column_if_missing(conn, "incidents", "code", "TEXT")?;
    add_column_if_missing(conn, "incidents", "owner_user_local_id", "INTEGER")?;
    add_column_if_missing(conn, "incidents", "product", "TEXT")?;
    add_column_if_missing(conn, "incidents", "feature", "TEXT")?;
    add_column_if_missing(conn, "incidents", "internal_explanation", "TEXT")?;
    add_column_if_missing(conn, "incidents", "customer_safe_explanation", "TEXT")?;
    add_column_if_missing(conn, "incidents", "known_cause", "TEXT")?;
    add_column_if_missing(conn, "incidents", "workaround", "TEXT")?;
    add_column_if_missing(conn, "incidents", "resolution", "TEXT")?;
    add_column_if_missing(conn, "incidents", "started_at", "TEXT")?;
    add_column_if_missing(conn, "issue_clusters", "known_issue_id", "INTEGER")?;
    add_column_if_missing(
        conn,
        "customer_memory",
        "source",
        "TEXT NOT NULL DEFAULT 'ai'",
    )?;
    let _ = conn.execute("UPDATE app_state SET schema_version = 35 WHERE id = 1", []);
    Ok(())
}

pub(crate) fn apply_m041(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS incident_releases (
            id            INTEGER PRIMARY KEY AUTOINCREMENT,
            incident_id   INTEGER NOT NULL REFERENCES incidents (id) ON DELETE CASCADE,
            version_label TEXT NOT NULL,
            notes         TEXT,
            released_at   TEXT,
            correlation   TEXT,
            created_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
        );
        CREATE INDEX IF NOT EXISTS idx_incident_releases_incident
            ON incident_releases (incident_id);

        CREATE TABLE IF NOT EXISTS incident_notes (
            id                 INTEGER PRIMARY KEY AUTOINCREMENT,
            incident_id        INTEGER NOT NULL REFERENCES incidents (id) ON DELETE CASCADE,
            author_user_local_id INTEGER REFERENCES users (id) ON DELETE SET NULL,
            body               TEXT NOT NULL,
            created_at         TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
        );
        CREATE INDEX IF NOT EXISTS idx_incident_notes_incident
            ON incident_notes (incident_id, created_at);

        -- Reference 014 incident_events: the append-only workspace
        -- timeline. `dedup_key` is UNIQUE so re-recording the same
        -- transition is a no-op (reference repo.record with INSERT OR
        -- IGNORE on the dedup key).
        CREATE TABLE IF NOT EXISTS incident_events (
            id                 INTEGER PRIMARY KEY AUTOINCREMENT,
            incident_id        INTEGER NOT NULL REFERENCES incidents (id) ON DELETE CASCADE,
            event_type         TEXT NOT NULL,
            actor_user_local_id INTEGER REFERENCES users (id) ON DELETE SET NULL,
            occurred_at        TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
            detail             TEXT,
            source             TEXT NOT NULL DEFAULT 'local',
            dedup_key          TEXT NOT NULL UNIQUE
        );
        CREATE INDEX IF NOT EXISTS idx_incident_events_incident
            ON incident_events (incident_id, occurred_at);",
    )?;
    let _ = conn.execute("UPDATE app_state SET schema_version = 41 WHERE id = 1", []);
    Ok(())
}

fn next_incident_code(conn: &Connection) -> String {
    let maxn: Option<i64> = conn
        .query_row(
            "SELECT MAX(CAST(substr(code, 5) AS INTEGER)) AS maxn FROM incidents
             WHERE code LIKE 'INC-%' AND length(code) >= 5
               AND substr(code, 5) GLOB '[0-9]*'",
            [],
            |r| r.get(0),
        )
        .unwrap_or(None);
    format!("INC-{:03}", maxn.unwrap_or(0) + 1)
}

#[derive(Debug, Clone)]
pub struct ManualIncident {
    pub title: String,
    pub status: String,
    pub severity: String,
    pub owner_user_local_id: Option<i64>,
    pub product: Option<String>,
    pub feature: Option<String>,
    pub description: Option<String>,
    pub internal_explanation: Option<String>,
    pub customer_safe_explanation: Option<String>,
    pub known_cause: Option<String>,
    pub workaround: Option<String>,
    pub resolution: Option<String>,
    pub started_at: Option<String>,
    pub conversation_ids: Vec<i64>,
}

pub fn create_manual_incident(conn: &Connection, inc: &ManualIncident) -> Result<i64> {
    let tx = conn.unchecked_transaction()?;
    let code = next_incident_code(&tx);
    let resolved_at = if inc.status == "resolved" {
        "strftime('%Y-%m-%dT%H:%M:%fZ','now')"
    } else {
        "NULL"
    };
    let sql = format!(
        "INSERT INTO incidents (known_issue_id, status, severity, source, title, code, resolved_at,
             owner_user_local_id, product, feature, description, internal_explanation,
             customer_safe_explanation, known_cause, workaround, resolution, started_at)
         VALUES (NULL, ?1, ?2, 'manual', ?3, ?4, {resolved_at}, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)"
    );
    tx.execute(
        &sql,
        params![
            inc.status,
            inc.severity,
            inc.title,
            code,
            inc.owner_user_local_id,
            inc.product,
            inc.feature,
            inc.description,
            inc.internal_explanation,
            inc.customer_safe_explanation,
            inc.known_cause,
            inc.workaround,
            inc.resolution,
            inc.started_at,
        ],
    )?;
    let id = tx.last_insert_rowid();
    let _ = record_incident_event(
        &tx,
        id,
        "created",
        None,
        Some(
            &serde_json::json!({ "code": code, "title": inc.title, "status": inc.status, "severity": inc.severity }),
        ),
        &format!("incident:{id}:created:0"),
    )?;
    for conv_id in &inc.conversation_ids {
        tx.execute(
            "INSERT OR IGNORE INTO incident_conversations (incident_id, conversation_id, linked_by)
             VALUES (?1, ?2, 'human')",
            params![id, conv_id],
        )?;
        let _ = record_incident_event(
            &tx,
            id,
            "conversation_linked",
            None,
            Some(&serde_json::json!({ "conversation_id": conv_id })),
            &format!("incident:{id}:conversation_linked:conv:{conv_id}"),
        )?;
    }
    tx.commit()?;
    Ok(id)
}

#[derive(Default)]
pub struct IncidentPatch {
    pub title: Option<String>,
    pub status: Option<String>,
    pub severity: Option<String>,
    pub owner_user_local_id: Option<Option<i64>>,
    pub product: Option<Option<String>>,
    pub feature: Option<Option<String>>,
    pub description: Option<Option<String>>,
    pub internal_explanation: Option<Option<String>>,
    pub customer_safe_explanation: Option<Option<String>>,
    pub known_cause: Option<Option<String>>,
    pub workaround: Option<Option<String>>,
    pub resolution: Option<Option<String>>,
    pub started_at: Option<Option<String>>,
}

pub fn link_incident_conversation(
    conn: &Connection,
    incident_id: i64,
    conversation_id: i64,
    linked_by: &str,
) -> Result<bool> {
    let created = conn.execute(
        "INSERT OR IGNORE INTO incident_conversations (incident_id, conversation_id, linked_by)
         VALUES (?1, ?2, ?3)",
        params![incident_id, conversation_id, linked_by],
    )? > 0;
    if created {
        conn.execute(
            "UPDATE incidents SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ?1",
            params![incident_id],
        )?;
        let _ = record_incident_event(
            conn,
            incident_id,
            "conversation_linked",
            None,
            Some(&serde_json::json!({ "conversation_id": conversation_id })),
            &format!("incident:{incident_id}:conversation_linked:conv:{conversation_id}"),
        );
    }
    Ok(created)
}

pub fn record_incident_event(
    conn: &Connection,
    incident_id: i64,
    event_type: &str,
    actor_user_local_id: Option<i64>,
    detail: Option<&serde_json::Value>,
    dedup_key: &str,
) -> Result<bool> {
    Ok(conn.execute(
        "INSERT OR IGNORE INTO incident_events
             (incident_id, event_type, actor_user_local_id, detail, dedup_key)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            incident_id,
            event_type,
            actor_user_local_id,
            detail.map(|d| d.to_string()),
            dedup_key
        ],
    )? > 0)
}

pub fn add_incident_note(
    conn: &Connection,
    incident_id: i64,
    author_user_local_id: Option<i64>,
    body: &str,
) -> Result<i64> {
    conn.execute(
        "INSERT INTO incident_notes (incident_id, author_user_local_id, body)
         VALUES (?1, ?2, ?3)",
        params![incident_id, author_user_local_id, body],
    )?;
    let note_id = conn.last_insert_rowid();
    let _ = conn.execute(
        "UPDATE incidents SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ?1",
        params![incident_id],
    );
    let _ = record_incident_event(
        conn,
        incident_id,
        "note_added",
        author_user_local_id,
        Some(&serde_json::json!({ "note_id": note_id })),
        &format!("incident:{incident_id}:note_added:{note_id}"),
    );
    Ok(note_id)
}

fn conversation_customer_id(conn: &Connection, conversation_id: i64) -> Option<i64> {
    conn.query_row(
        "SELECT customer_local_id FROM conversations WHERE id = ?1",
        params![conversation_id],
        |r| r.get(0),
    )
    .ok()
}

fn persist_breach(conn: &Connection, conversation_id: i64, breach_type: &str) -> Result<bool> {
    let already: i64 = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sla_breaches
          WHERE conversation_id = ?1 AND breach_type = ?2)",
        params![conversation_id, breach_type],
        |r| r.get(0),
    )?;
    if already != 0 {
        return Ok(false);
    }
    record_sla_breach(conn, conversation_id, breach_type)?;
    if let Some(customer_id) = conversation_customer_id(conn, conversation_id) {
        let data = format!("{{\"breach_type\":\"{breach_type}\"}}");
        let _ = crate::reports::record_timeline_event(conn, customer_id, "sla_breach", Some(&data));
    }
    Ok(true)
}

pub fn sweep_sla_breaches(conn: &Connection) -> Result<usize> {
    let alerts = crate::sla::sla_alerts(conn)?;
    let mut recorded = 0usize;
    for row in &alerts.alerts {
        if row.state != crate::sla::SlaAlertState::Breached {
            continue;
        }
        if persist_breach(conn, row.conversation_id, row.target_kind.as_str())? {
            recorded += 1;
        }
    }
    Ok(recorded)
}

pub fn sweep_unmonitored_sla(conn: &Connection) -> Result<usize> {
    let mailboxes: Vec<i64> = {
        let mut stmt = conn.prepare(
            "SELECT m.id FROM mailboxes m
              WHERE NOT EXISTS (SELECT 1 FROM mailbox_business_hours b
                                 WHERE b.mailbox_local_id = m.id)
                AND EXISTS (SELECT 1 FROM conversations c
                             WHERE c.mailbox_local_id = m.id
                               AND c.status IN ('active','pending')
                               AND c.deleted_at IS NULL)",
        )?;
        let rows = stmt.query_map([], |r| r.get(0))?;
        rows.flatten().collect()
    };
    let mut recorded = 0usize;
    for mailbox_id in mailboxes {
        let (fr_hours, res_hours) = match get_sla_config(conn, mailbox_id)? {
            Some(cfg) => (cfg.first_response_hours, cfg.resolution_hours),
            None => (24.0, 72.0),
        };
        let mut stmt = conn.prepare(
            "SELECT c.id,
                    EXISTS (SELECT 1 FROM conversation_threads t
                             WHERE t.conversation_id = c.id
                               AND t.type IN ('message','chat','phone')
                               AND t.from_type IN ('user','agent')
                               AND t.state = 'published' AND t.deleted_at IS NULL) AS replied,
                    (julianday('now') - julianday(COALESCE(
                        (SELECT t.created_at FROM conversation_threads t
                          WHERE t.conversation_id = c.id AND t.type = 'customer'
                            AND t.deleted_at IS NULL
                          ORDER BY t.created_at DESC LIMIT 1),
                        c.created_at))) * 24.0,
                    (julianday('now') - julianday(c.created_at)) * 24.0
               FROM conversations c
              WHERE c.mailbox_local_id = ?1
                AND c.status IN ('active','pending')
                AND c.deleted_at IS NULL",
        )?;
        let rows = stmt.query_map(params![mailbox_id], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, i64>(1)? != 0,
                r.get::<_, f64>(2)?,
                r.get::<_, f64>(3)?,
            ))
        })?;
        for row in rows.flatten() {
            let (id, replied, fr_elapsed, res_elapsed) = row;
            let breach_type = if !replied && fr_elapsed > fr_hours {
                "first_response"
            } else if replied && res_elapsed > res_hours {
                "resolution"
            } else {
                continue;
            };
            if persist_breach(conn, id, breach_type)? {
                recorded += 1;
            }
        }
    }
    Ok(recorded)
}

pub fn sweep_doc_freshness(conn: &Connection) -> Result<usize> {
    let stale_days = crate::settings::get_int(conn, "freshness_stale_days", 180)?.max(1);
    let pruned = conn.execute(
        "DELETE FROM knowledge_doc_freshness
          WHERE doc_id NOT IN (SELECT id FROM knowledge_documents)",
        [],
    )?;
    let marked = conn.execute(
        "UPDATE knowledge_doc_freshness
            SET stale_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
          WHERE stale_at IS NULL
            AND EXISTS (SELECT 1 FROM knowledge_documents d
                         WHERE d.id = knowledge_doc_freshness.doc_id
                           AND d.updated_at IS NOT NULL
                           AND julianday('now') - julianday(d.updated_at) > ?1)",
        params![stale_days],
    )?;
    let cleared = conn.execute(
        "UPDATE knowledge_doc_freshness
            SET stale_at = NULL
          WHERE stale_at IS NOT NULL
            AND NOT EXISTS (SELECT 1 FROM knowledge_documents d
                             WHERE d.id = knowledge_doc_freshness.doc_id
                               AND d.updated_at IS NOT NULL
                               AND julianday('now') - julianday(d.updated_at) > ?1)",
        params![stale_days],
    )?;
    Ok(pruned + marked + cleared)
}

pub fn sweep(conn: &Connection) -> Result<()> {
    let mut first_err: Option<crate::error::Error> = None;
    if let Err(e) = sweep_sla_breaches(conn) {
        first_err = first_err.or(Some(e));
    }
    if let Err(e) = sweep_unmonitored_sla(conn) {
        first_err = first_err.or(Some(e));
    }
    if let Err(e) = sweep_doc_freshness(conn) {
        first_err = first_err.or(Some(e));
    }
    match first_err {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

#[cfg(test)]
mod item150_tests {
    use super::*;
    use rusqlite::Connection;

    fn db() -> Connection {
        let mut conn = Connection::open_in_memory().expect("db");
        crate::bootstrap::apply_all(&mut conn).expect("migrations");
        conn
    }

    fn seed_mailbox(conn: &Connection, id: i64) {
        conn.execute(
            "INSERT INTO mailboxes (id, remote_id, name) VALUES (?1, ?1, ?2)",
            params![id, format!("Mailbox {id}")],
        )
        .expect("mailbox");
    }

    fn seed_customer(conn: &Connection, id: i64) {
        conn.execute(
            "INSERT INTO customers (id, remote_id, first_name, last_name) VALUES (?1, ?1, 'Test', 'Customer')",
            params![id],
        )
        .expect("customer");
    }

    fn seed_conversation(conn: &Connection, id: i64, mailbox: i64, created_ago_days: i64) {
        conn.execute(
            "INSERT INTO conversations (id, remote_id, number, subject, preview, status,
                                        mailbox_local_id, customer_local_id, created_at)
             VALUES (?1, ?1, ?1, 'tracked', 'p', 'active', ?2, 1,
                     datetime('now', ?3))",
            params![id, mailbox, format!("-{created_ago_days} days")],
        )
        .expect("conversation");
    }

    #[test]
    fn sweep_sla_breaches_persists_live_verdicts_deduped() {
        let conn = db();
        seed_customer(&conn, 1);
        seed_mailbox(&conn, 1);
        seed_conversation(&conn, 10, 1, 3);
        conn.execute(
            "INSERT INTO mailbox_business_hours
                (mailbox_local_id, timezone, days, start_minute, end_minute,
                 first_response_target_min, resolution_target_min, updated_at)
             VALUES (1, 'UTC', '[0,1,2,3,4,5,6]', 0, 1440, 1, 10080, datetime('now'))",
            [],
        )
        .expect("business hours");
        conn.execute(
            "INSERT INTO conversation_threads (conversation_id, type, state, created_at)
             VALUES (10, 'customer', 'published', datetime('now', '-2 days'))",
            [],
        )
        .expect("customer thread");

        let recorded = sweep_sla_breaches(&conn).expect("sweep");
        assert_eq!(recorded, 1, "the live Breached verdict is persisted once");
        assert_eq!(count_sla_breaches(&conn).expect("count"), 1);
        let (ctype, ckind): (String, String) = conn
            .query_row(
                "SELECT breach_type, (SELECT breach_type FROM sla_breaches LIMIT 1)
                 FROM sla_breaches LIMIT 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap_or(("none".into(), "none".into()));
        assert_eq!(ctype, "first_response");
        assert_eq!(ckind, "first_response");
        let milestones: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM customer_timeline WHERE event_type = 'sla_breach'",
                [],
                |r| r.get(0),
            )
            .expect("timeline count");
        assert_eq!(
            milestones, 1,
            "the support-milestone timeline gained the fact"
        );

        let again = sweep_sla_breaches(&conn).expect("second sweep");
        assert_eq!(again, 0, "deduped per conversation + breach kind");
        assert_eq!(count_sla_breaches(&conn).expect("count"), 1);
    }

    #[test]
    fn sweep_unmonitored_honors_sla_configs() {
        let conn = db();
        seed_customer(&conn, 1);
        seed_mailbox(&conn, 2);
        seed_conversation(&conn, 20, 2, 3);

        let default_sweep = sweep_unmonitored_sla(&conn).expect("default sweep");
        assert_eq!(default_sweep, 1, "72h unanswered breaches the 24h default");

        set_sla_config(&conn, 2, 100.0, 200.0).expect("config");
        let relaxed = sweep_unmonitored_sla(&conn).expect("relaxed sweep");
        assert_eq!(relaxed, 0, "the configured 100h threshold is honored");
        assert_eq!(count_sla_breaches(&conn).expect("count"), 1);

        let cfg = get_sla_config(&conn, 2).expect("read").expect("row");
        assert_eq!(cfg.first_response_hours, 100.0);
        assert_eq!(cfg.resolution_hours, 200.0);
    }

    #[test]
    fn knowledge_gaps_gain_rows_and_list_back() {
        let conn = db();
        record_knowledge_gap(&conn, "refund policy").expect("first");
        record_knowledge_gap(&conn, "refund policy").expect("second");
        record_knowledge_gap(&conn, "api keys").expect("other");
        let gaps = list_knowledge_gaps(&conn, 10).expect("list");
        assert_eq!(gaps.len(), 2);
        assert_eq!(gaps[0], ("refund policy".into(), 2));
        assert_eq!(gaps[1], ("api keys".into(), 1));
    }

    #[test]
    fn doc_freshness_stamps_marks_clears_and_prunes() {
        let conn = db();
        let source = crate::knowledge_store::get_or_create_source(
            &conn,
            "test-source",
            "local_file",
            "internal_only",
        )
        .expect("source");
        let (doc_id, _changed) = crate::knowledge_store::upsert_document(
            &conn,
            source,
            "Doc",
            "body",
            "internal_only",
            "markdown",
        )
        .expect("doc");
        let stamped: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM knowledge_doc_freshness WHERE doc_id = ?1 AND stale_at IS NULL",
                params![doc_id],
                |r| r.get(0),
            )
            .expect("freshness row");
        assert_eq!(stamped, 1, "ingest stamps the tracked freshness row");

        conn.execute(
            "UPDATE knowledge_documents SET updated_at = datetime('now', '-200 days')
             WHERE id = ?1",
            params![doc_id],
        )
        .expect("backdate");
        sweep_doc_freshness(&conn).expect("stale sweep");
        assert_eq!(count_stale_docs(&conn).expect("stale count"), 1);

        crate::knowledge_store::upsert_document(
            &conn,
            source,
            "Doc",
            "body 2",
            "internal_only",
            "markdown",
        )
        .expect("re-ingest");
        sweep_doc_freshness(&conn).expect("clear sweep");
        assert_eq!(count_stale_docs(&conn).expect("stale count"), 0);

        crate::knowledge_store::delete_document(&conn, doc_id).expect("delete");
        sweep_doc_freshness(&conn).expect("prune sweep");
        let left: i64 = conn
            .query_row("SELECT COUNT(*) FROM knowledge_doc_freshness", [], |r| {
                r.get(0)
            })
            .expect("rows left");
        assert_eq!(left, 0, "orphaned freshness rows are pruned");
    }

    #[test]
    fn combined_sweep_succeeds_on_clean_database() {
        let conn = db();
        crate::intelligence_features::sweep(&conn).expect("combined sweep");
    }
}
