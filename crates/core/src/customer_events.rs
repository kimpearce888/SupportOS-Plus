use rusqlite::{params, Connection};
use serde_json::{json, Value};

use crate::error::Result;

pub fn apply_m036(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS customer_events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            customer_local_id INTEGER NOT NULL REFERENCES customers (id) ON DELETE CASCADE,
            event_kind TEXT NOT NULL
              CHECK (event_kind IN (
                'signup','support_conversation','customer_message','campaign','campaign_reply',
                'rating','incident_exposure','custom_object_event',
                'subscription_event','account_event','product_event','integration_event'
              )),
            occurred_at TEXT,
            title TEXT NOT NULL,
            detail TEXT,
            source TEXT NOT NULL DEFAULT 'local_derived',
            source_ref TEXT,
            dedup_key TEXT NOT NULL UNIQUE,
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        );
        CREATE INDEX IF NOT EXISTS idx_customer_events_customer
            ON customer_events (customer_local_id, occurred_at DESC);
        CREATE INDEX IF NOT EXISTS idx_customer_events_kind
            ON customer_events (event_kind);

        -- Reference 014 custom-object records + links (the port's routes
        -- already query these shapes; the tables were never created).
        CREATE TABLE IF NOT EXISTS custom_objects (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            type_id     INTEGER NOT NULL REFERENCES custom_object_types (id) ON DELETE CASCADE,
            title       TEXT NOT NULL DEFAULT '',
            data_json   TEXT NOT NULL DEFAULT '{}',
            deleted_at  TEXT,
            created_at  TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
        );
        CREATE INDEX IF NOT EXISTS idx_custom_objects_type ON custom_objects (type_id);

        CREATE TABLE IF NOT EXISTS custom_object_links (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            object_id       INTEGER NOT NULL REFERENCES custom_objects (id) ON DELETE CASCADE,
            target_kind     TEXT NOT NULL,
            target_local_id INTEGER NOT NULL,
            linked_by       TEXT NOT NULL DEFAULT 'human',
            linked_at       TEXT NOT NULL DEFAULT (datetime('now')),
            UNIQUE (object_id, target_kind, target_local_id)
        );
        CREATE INDEX IF NOT EXISTS idx_custom_object_links_target
            ON custom_object_links (target_kind, target_local_id);",
    )?;
    add_column_if_missing(conn, "conversations", "deleted_at", "TEXT")?;
    add_column_if_missing(
        conn,
        "conversations",
        "merged_into_conversation_id",
        "INTEGER",
    )?;
    add_column_if_missing(conn, "conversation_threads", "deleted_at", "TEXT")?;
    add_column_if_missing(
        conn,
        "conversation_threads",
        "fts_indexed",
        "INTEGER NOT NULL DEFAULT 0",
    )?;
    add_column_if_missing(conn, "customers", "deleted_at", "TEXT")?;
    add_column_if_missing(conn, "customers", "organization_id", "INTEGER")?;
    conn.execute(
        "UPDATE conversations SET deleted_at = COALESCE(deleted_at, datetime('now'))
          WHERE status = 'deleted' AND deleted_at IS NULL",
        [],
    )?;
    conn.execute(
        "UPDATE customers SET organization_id = (
            SELECT o.id FROM organizations o
            WHERE o.name = customers.organization AND o.deleted_at IS NULL
            LIMIT 1
        ) WHERE organization_id IS NULL AND organization IS NOT NULL AND organization != ''",
        [],
    )?;
    let _ = conn.execute("UPDATE app_state SET schema_version = 36 WHERE id = 1", []);
    Ok(())
}

fn add_column_if_missing(conn: &Connection, table: &str, column: &str, decl: &str) -> Result<()> {
    let exists: bool = conn
        .prepare(&format!("PRAGMA table_info({table})"))?
        .query_map([], |r| r.get::<_, String>(1))?
        .filter_map(|r| r.ok())
        .any(|c| c == column);
    if !exists {
        conn.execute(
            &format!("ALTER TABLE {table} ADD COLUMN {column} {decl}"),
            [],
        )?;
    }
    Ok(())
}

pub fn sweep(conn: &Connection) -> Result<usize> {
    let state: String = conn
        .query_row(
            "SELECT value FROM application_settings WHERE key = 'sync_state'",
            [],
            |r| r.get(0),
        )
        .unwrap_or_else(|_| "\"NEW\"".to_string());
    let state = state.trim_matches('"');
    if state == "NEW" || state == "INITIALIZING" || state == "BACKFILLING" {
        return Ok(0);
    }
    let mut created = 0;
    created += derive_recent_conversations(conn)?;
    created += derive_recent_campaigns(conn)?;
    created += derive_recent_ratings(conn)?;
    created += derive_incident_exposure(conn)?;
    created += derive_custom_object_events(conn)?;
    Ok(created)
}

fn counted(conn: &Connection, sql: &str, params: &[&dyn rusqlite::ToSql]) -> Result<usize> {
    Ok(conn.execute(sql, params)?)
}

fn derive_recent_conversations(conn: &Connection) -> Result<usize> {
    let mut created = 0;
    created += counted(
        conn,
        "INSERT OR IGNORE INTO customer_events
           (customer_local_id, event_kind, occurred_at, title, detail, source, source_ref, dedup_key)
         SELECT c.customer_local_id, 'support_conversation', c.created_at,
                'Support conversation #' || c.number || ' started',
                json_object('conversation_id', c.id, 'number', c.number, 'subject', c.subject, 'status', c.status),
                'hs_sync', 'conversations:' || c.remote_id,
                'conv_created:' || c.id
         FROM conversations c
         WHERE c.customer_local_id IS NOT NULL AND c.created_at IS NOT NULL AND c.deleted_at IS NULL
           AND julianday(c.updated_at) >= julianday('now', '-7 days')",
        &[],
    )?;
    created += counted(
        conn,
        "INSERT OR IGNORE INTO customer_events
           (customer_local_id, event_kind, occurred_at, title, detail, source, source_ref, dedup_key)
         SELECT c.customer_local_id, 'support_conversation', c.closed_at,
                'Support conversation #' || c.number || ' closed',
                json_object('conversation_id', c.id, 'number', c.number, 'subject', c.subject, 'status', 'closed'),
                'hs_sync', 'conversations:' || c.remote_id,
                'conv_closed:' || c.id
         FROM conversations c
         WHERE c.customer_local_id IS NOT NULL AND c.closed_at IS NOT NULL AND c.deleted_at IS NULL
           AND julianday(c.updated_at) >= julianday('now', '-7 days')",
        &[],
    )?;
    created += counted(
        conn,
        "INSERT OR IGNORE INTO customer_events
           (customer_local_id, event_kind, occurred_at, title, detail, source, source_ref, dedup_key)
         SELECT c.customer_local_id, 'customer_message', t.created_at,
                'Customer wrote in: ' || substr(COALESCE(c.subject, ''), 1, 80),
                json_object('conversation_id', c.id, 'number', c.number, 'subject', c.subject,
                            'excerpt', substr(COALESCE(t.body_text, ''), 1, 300)),
                'hs_sync', 'threads:' || t.remote_id,
                'conv_first_message:' || c.id
         FROM conversations c
         JOIN conversation_threads t ON t.conversation_id = c.id AND t.type = 'customer'
           AND t.deleted_at IS NULL AND t.state = 'published'
           AND t.created_at = (
             SELECT MIN(t2.created_at) FROM conversation_threads t2
             WHERE t2.conversation_id = c.id AND t2.type = 'customer'
               AND t2.deleted_at IS NULL AND t2.state = 'published'
           )
         WHERE c.customer_local_id IS NOT NULL AND c.deleted_at IS NULL
           AND julianday(c.created_at) >= julianday('now', '-7 days')",
        &[],
    )?;
    Ok(created)
}

fn derive_recent_campaigns(conn: &Connection) -> Result<usize> {
    let mut created = 0;
    created += counted(
        conn,
        "INSERT OR IGNORE INTO customer_events
           (customer_local_id, event_kind, occurred_at, title, detail, source, source_ref, dedup_key)
         SELECT r.customer_local_id, 'campaign', r.sent_at,
                'Outreach campaign sent: ' || COALESCE(oc.name, 'campaign'),
                json_object('campaign_id', oc.id, 'campaign_name', oc.name, 'conversation_number', r.hs_conversation_number),
                'local_outreach', 'campaign:' || oc.id,
                'campaign_sent:' || oc.id || ':' || r.customer_local_id
         FROM outreach_recipients r
         JOIN outreach_campaigns oc ON oc.id = r.campaign_id
         WHERE r.sent_at IS NOT NULL AND julianday(r.sent_at) >= julianday('now', '-30 days')",
        &[],
    )?;
    created += counted(
        conn,
        "INSERT OR IGNORE INTO customer_events
           (customer_local_id, event_kind, occurred_at, title, detail, source, source_ref, dedup_key)
         SELECT r.customer_local_id, 'campaign_reply', r.replied_at,
                'Customer replied to outreach: ' || COALESCE(oc.name, 'campaign'),
                json_object('campaign_id', oc.id, 'campaign_name', oc.name),
                'local_outreach', 'campaign:' || oc.id,
                'campaign_replied:' || oc.id || ':' || r.customer_local_id
         FROM outreach_recipients r
         JOIN outreach_campaigns oc ON oc.id = r.campaign_id
         WHERE r.replied_at IS NOT NULL AND julianday(r.replied_at) >= julianday('now', '-30 days')",
        &[],
    )?;
    Ok(created)
}

fn derive_recent_ratings(conn: &Connection) -> Result<usize> {
    counted(
        conn,
        "INSERT OR IGNORE INTO customer_events
           (customer_local_id, event_kind, occurred_at, title, detail, source, source_ref, dedup_key)
         SELECT r.customer_local_id, 'rating', r.remote_created_at,
                'Customer rated the support ' || r.rating,
                json_object('rating', r.rating, 'comments', r.comments, 'conversation_id', r.conversation_id),
                'hs_sync', 'ratings:' || r.remote_id,
                'rating:' || r.id
         FROM ratings r
         WHERE r.customer_local_id IS NOT NULL AND r.remote_created_at IS NOT NULL
           AND julianday(r.remote_created_at) >= julianday('now', '-30 days')",
        &[],
    )
}

fn derive_incident_exposure(conn: &Connection) -> Result<usize> {
    counted(
        conn,
        "INSERT OR IGNORE INTO customer_events
           (customer_local_id, event_kind, occurred_at, title, detail, source, source_ref, dedup_key)
         SELECT DISTINCT c.customer_local_id, 'incident_exposure', ic.linked_at,
                'Affected by incident ' || i.code || ': ' || substr(i.title, 1, 80),
                json_object('incident_id', i.id, 'code', i.code, 'title', i.title,
                            'severity', i.severity, 'status', i.status, 'conversation_id', c.id),
                'local_derived', 'incidents:' || i.id,
                'incident_exposure:' || i.id || ':' || c.customer_local_id
         FROM incidents i
         JOIN incident_conversations ic ON ic.incident_id = i.id
         JOIN conversations c ON c.id = ic.conversation_id
         WHERE i.status != 'resolved' AND c.customer_local_id IS NOT NULL AND c.deleted_at IS NULL",
        &[],
    )
}

fn derive_custom_object_events(conn: &Connection) -> Result<usize> {
    counted(
        conn,
        "INSERT OR IGNORE INTO customer_events
           (customer_local_id, event_kind, occurred_at, title, detail, source, source_ref, dedup_key)
         SELECT l.target_local_id, 'custom_object_event', l.linked_at,
                COALESCE(t.name, 'Record') || ': ' || substr(o.title, 1, 80),
                json_object('object_id', o.id, 'object_title', o.title, 'type_name', t.name),
                'custom_object', 'custom_objects:' || o.id,
                'cobj:' || o.id || ':customer:' || l.target_local_id
         FROM custom_object_links l
         JOIN custom_objects o ON o.id = l.object_id AND o.deleted_at IS NULL
         JOIN custom_object_types t ON t.id = o.type_id
         WHERE l.target_kind = 'customer'",
        &[],
    )
}

pub fn rebuild(conn: &Connection) -> Result<usize> {
    let mut created = 0;
    created += counted(
        conn,
        "INSERT OR IGNORE INTO customer_events
           (customer_local_id, event_kind, occurred_at, title, detail, source, source_ref, dedup_key)
         SELECT c.id, 'signup', c.created_at, 'Customer created in Help Scout',
                json_object('remote_id', c.remote_id), 'hs_sync', 'customers:' || c.remote_id,
                'signup:' || c.id
         FROM customers c WHERE c.created_at IS NOT NULL AND c.deleted_at IS NULL",
        &[],
    )?;
    created += counted(
        conn,
        "INSERT OR IGNORE INTO customer_events
           (customer_local_id, event_kind, occurred_at, title, detail, source, source_ref, dedup_key)
         SELECT c.customer_local_id, 'support_conversation', c.created_at,
                'Support conversation #' || c.number || ' started',
                json_object('conversation_id', c.id, 'number', c.number, 'subject', c.subject, 'status', c.status),
                'hs_sync', 'conversations:' || c.remote_id,
                'conv_created:' || c.id
         FROM conversations c
         WHERE c.customer_local_id IS NOT NULL AND c.created_at IS NOT NULL AND c.deleted_at IS NULL",
        &[],
    )?;
    created += counted(
        conn,
        "INSERT OR IGNORE INTO customer_events
           (customer_local_id, event_kind, occurred_at, title, detail, source, source_ref, dedup_key)
         SELECT c.customer_local_id, 'support_conversation', c.closed_at,
                'Support conversation #' || c.number || ' closed',
                json_object('conversation_id', c.id, 'number', c.number, 'subject', c.subject, 'status', 'closed'),
                'hs_sync', 'conversations:' || c.remote_id,
                'conv_closed:' || c.id
         FROM conversations c
         WHERE c.customer_local_id IS NOT NULL AND c.closed_at IS NOT NULL AND c.deleted_at IS NULL",
        &[],
    )?;
    created += counted(
        conn,
        "INSERT OR IGNORE INTO customer_events
           (customer_local_id, event_kind, occurred_at, title, detail, source, source_ref, dedup_key)
         SELECT c.customer_local_id, 'customer_message', t.created_at,
                'Customer wrote in: ' || substr(COALESCE(c.subject, ''), 1, 80),
                json_object('conversation_id', c.id, 'number', c.number, 'subject', c.subject,
                            'excerpt', substr(COALESCE(t.body_text, ''), 1, 300)),
                'hs_sync', 'threads:' || t.remote_id,
                'conv_first_message:' || c.id
         FROM conversations c
         JOIN conversation_threads t ON t.conversation_id = c.id AND t.type = 'customer'
           AND t.deleted_at IS NULL AND t.state = 'published'
           AND t.created_at = (
             SELECT MIN(t2.created_at) FROM conversation_threads t2
             WHERE t2.conversation_id = c.id AND t2.type = 'customer'
               AND t2.deleted_at IS NULL AND t2.state = 'published'
           )
         WHERE c.customer_local_id IS NOT NULL AND c.deleted_at IS NULL",
        &[],
    )?;
    created += counted(
        conn,
        "INSERT OR IGNORE INTO customer_events
           (customer_local_id, event_kind, occurred_at, title, detail, source, source_ref, dedup_key)
         SELECT r.customer_local_id, 'campaign', r.sent_at,
                'Outreach campaign sent: ' || COALESCE(oc.name, 'campaign'),
                json_object('campaign_id', oc.id, 'campaign_name', oc.name, 'conversation_number', r.hs_conversation_number),
                'local_outreach', 'campaign:' || oc.id,
                'campaign_sent:' || oc.id || ':' || r.customer_local_id
         FROM outreach_recipients r JOIN outreach_campaigns oc ON oc.id = r.campaign_id
         WHERE r.sent_at IS NOT NULL",
        &[],
    )?;
    created += counted(
        conn,
        "INSERT OR IGNORE INTO customer_events
           (customer_local_id, event_kind, occurred_at, title, detail, source, source_ref, dedup_key)
         SELECT r.customer_local_id, 'campaign_reply', r.replied_at,
                'Customer replied to outreach: ' || COALESCE(oc.name, 'campaign'),
                json_object('campaign_id', oc.id, 'campaign_name', oc.name),
                'local_outreach', 'campaign:' || oc.id,
                'campaign_replied:' || oc.id || ':' || r.customer_local_id
         FROM outreach_recipients r JOIN outreach_campaigns oc ON oc.id = r.campaign_id
         WHERE r.replied_at IS NOT NULL",
        &[],
    )?;
    created += counted(
        conn,
        "INSERT OR IGNORE INTO customer_events
           (customer_local_id, event_kind, occurred_at, title, detail, source, source_ref, dedup_key)
         SELECT r.customer_local_id, 'rating', r.remote_created_at,
                'Customer rated the support ' || r.rating,
                json_object('rating', r.rating, 'comments', r.comments, 'conversation_id', r.conversation_id),
                'hs_sync', 'ratings:' || r.remote_id,
                'rating:' || r.id
         FROM ratings r
         WHERE r.customer_local_id IS NOT NULL AND r.remote_created_at IS NOT NULL",
        &[],
    )?;
    created += derive_incident_exposure(conn)?;
    created += derive_custom_object_events(conn)?;
    Ok(created)
}

fn hydrate_event(row: &rusqlite::Row<'_>) -> rusqlite::Result<Value> {
    let detail: Option<String> = row.get("detail")?;
    let detail = detail
        .and_then(|d| serde_json::from_str::<Value>(&d).ok())
        .unwrap_or(Value::Null);
    Ok(json!({
        "id": row.get::<_, i64>("id")?,
        "customer_local_id": row.get::<_, i64>("customer_local_id")?,
        "event_kind": row.get::<_, String>("event_kind")?,
        "occurred_at": row.get::<_, Option<String>>("occurred_at")?,
        "title": row.get::<_, String>("title")?,
        "detail": detail,
        "source": row.get::<_, String>("source")?,
        "source_ref": row.get::<_, Option<String>>("source_ref")?,
        "created_at": row.get::<_, String>("created_at")?,
    }))
}

pub fn list_for_customer(
    conn: &Connection,
    customer_local_id: i64,
    kind: Option<&str>,
    limit: i64,
    offset: i64,
) -> Result<(Vec<Value>, i64)> {
    let limit = limit.clamp(1, 200);
    let offset = offset.max(0);
    let (events, total) = match kind {
        Some(k) => {
            let mut stmt = conn.prepare(
                "SELECT * FROM customer_events ce
                  WHERE ce.customer_local_id = ?1 AND ce.event_kind = ?2
                  ORDER BY COALESCE(ce.occurred_at, ce.created_at) DESC, ce.id DESC
                  LIMIT ?3 OFFSET ?4",
            )?;
            let events = stmt
                .query_map(params![customer_local_id, k, limit, offset], hydrate_event)?
                .filter_map(|r| r.ok())
                .collect();
            let total = conn.query_row(
                "SELECT COUNT(*) FROM customer_events ce
                  WHERE ce.customer_local_id = ?1 AND ce.event_kind = ?2",
                params![customer_local_id, k],
                |r| r.get(0),
            )?;
            (events, total)
        }
        None => {
            let mut stmt = conn.prepare(
                "SELECT * FROM customer_events ce
                  WHERE ce.customer_local_id = ?1
                  ORDER BY COALESCE(ce.occurred_at, ce.created_at) DESC, ce.id DESC
                  LIMIT ?2 OFFSET ?3",
            )?;
            let events = stmt
                .query_map(params![customer_local_id, limit, offset], hydrate_event)?
                .filter_map(|r| r.ok())
                .collect();
            let total = conn.query_row(
                "SELECT COUNT(*) FROM customer_events ce WHERE ce.customer_local_id = ?1",
                params![customer_local_id],
                |r| r.get(0),
            )?;
            (events, total)
        }
    };
    Ok((events, total))
}

pub fn kind_counts(conn: &Connection, customer_local_id: i64) -> Result<Vec<Value>> {
    let mut stmt = conn.prepare(
        "SELECT event_kind AS kind, COUNT(*) AS n FROM customer_events
          WHERE customer_local_id = ?1 GROUP BY event_kind ORDER BY n DESC",
    )?;
    let rows = stmt
        .query_map(params![customer_local_id], |r| {
            Ok(json!({"kind": r.get::<_, String>(0)?, "n": r.get::<_, i64>(1)?}))
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}

pub fn list_for_organization(
    conn: &Connection,
    organization_id: i64,
    kind: Option<&str>,
    limit: i64,
    offset: i64,
) -> Result<(Vec<Value>, i64)> {
    let limit = limit.clamp(1, 200);
    let offset = offset.max(0);
    let select = |where_clause: &str, kind_filter: Option<&str>| -> Result<(Vec<Value>, i64)> {
        let sql = format!(
            "SELECT ce.*, (SELECT TRIM(COALESCE(cu2.first_name, '') || ' ' || COALESCE(cu2.last_name, ''))
                             FROM customers cu2 WHERE cu2.id = ce.customer_local_id) AS customer_name
             FROM customer_events ce
             JOIN customers cu ON cu.id = ce.customer_local_id
             WHERE {where_clause}
             ORDER BY COALESCE(ce.occurred_at, ce.created_at) DESC, ce.id DESC
             LIMIT {limit} OFFSET {offset}"
        );
        let mut stmt = conn.prepare(&sql)?;
        let map = |row: &rusqlite::Row<'_>| -> rusqlite::Result<Value> {
            let mut v = hydrate_event(row)?;
            if let Some(obj) = v.as_object_mut() {
                obj.insert(
                    "customer_name".into(),
                    row.get::<_, Option<String>>("customer_name")?.into(),
                );
            }
            Ok(v)
        };
        let events: Vec<Value> = match kind_filter {
            Some(k) => stmt
                .query_map(params![organization_id, k], map)?
                .filter_map(|r| r.ok())
                .collect(),
            None => stmt
                .query_map(params![organization_id], map)?
                .filter_map(|r| r.ok())
                .collect(),
        };
        let count_sql = format!(
            "SELECT COUNT(*) AS n FROM customer_events ce
             JOIN customers cu ON cu.id = ce.customer_local_id WHERE {where_clause}"
        );
        let total = match kind_filter {
            Some(k) => conn.query_row(&count_sql, params![organization_id, k], |r| r.get(0))?,
            None => conn.query_row(&count_sql, params![organization_id], |r| r.get(0))?,
        };
        Ok((events, total))
    };
    match kind {
        Some(k) => select("cu.organization_id = ?1 AND ce.event_kind = ?2", Some(k)),
        None => select("cu.organization_id = ?1", None),
    }
}
