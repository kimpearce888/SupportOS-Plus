use rusqlite::Connection;

use crate::error::Result;

struct Rebuild<'a> {
    new_name: &'a str,
    new_ddl: &'a str,
    copy_sql: &'a str,
    replacement_indexes: &'a [&'a str],
}

fn with_if_not_exists(sql: &str) -> String {
    let s = sql.trim().trim_end_matches(';');
    if let Some(rest) = s.strip_prefix("CREATE UNIQUE INDEX ") {
        format!("CREATE UNIQUE INDEX IF NOT EXISTS {rest}")
    } else if let Some(rest) = s.strip_prefix("CREATE INDEX ") {
        format!("CREATE INDEX IF NOT EXISTS {rest}")
    } else {
        s.to_string()
    }
}

fn table_exists(conn: &Connection, table: &str) -> Result<bool> {
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
        [table],
        |r| r.get(0),
    )?;
    Ok(n > 0)
}

fn captured_indexes(conn: &Connection, table: &str) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT name, sql FROM sqlite_master
          WHERE type = 'index' AND tbl_name = ?1 AND sql IS NOT NULL",
    )?;
    let rows: Vec<(String, String)> = stmt
        .query_map([table], |r| Ok((r.get(0)?, r.get(1)?)))?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows.into_iter().map(|(_, sql)| sql).collect())
}

fn captured_triggers(conn: &Connection, table: &str) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT sql FROM sqlite_master WHERE type = 'trigger' AND tbl_name = ?1 AND sql IS NOT NULL",
    )?;
    let rows: Vec<String> = stmt
        .query_map([table], |r| r.get(0))?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}

struct ChildRows {
    name: String,
    fks: Vec<(String, String, String)>,
    columns: Vec<String>,
    rows: Vec<Vec<rusqlite::types::Value>>,
}

fn fk_children(conn: &Connection, table: &str) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT name FROM sqlite_master
          WHERE type = 'table' AND name NOT LIKE 'sqlite_%'",
    )?;
    let names: Vec<String> = stmt
        .query_map([], |r| r.get(0))?
        .filter_map(|r| r.ok())
        .collect();
    drop(stmt);
    let mut children = Vec::new();
    for name in names {
        let mut fk = conn.prepare(&format!("PRAGMA foreign_key_list({name})"))?;
        let hits: i64 = fk
            .query_map([], |r| r.get::<_, String>(2))?
            .filter_map(|r| r.ok())
            .filter(|t| t == table)
            .count() as i64;
        if hits > 0 {
            children.push(name);
        }
    }
    Ok(children)
}

fn capture_child(conn: &Connection, name: &str, parent: &str) -> Result<ChildRows> {
    let columns: Vec<String> = {
        let mut stmt = conn.prepare(&format!("PRAGMA table_info({name})"))?;
        let cols: Vec<String> = stmt
            .query_map([], |r| r.get(1))?
            .filter_map(|c| c.ok())
            .collect();
        cols
    };
    let fks: Vec<(String, String, String)> = {
        let mut stmt = conn.prepare(&format!("PRAGMA foreign_key_list({name})"))?;
        let rows: Vec<(String, String, String)> = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(3)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(4)?,
                ))
            })?
            .filter_map(|r| r.ok())
            .filter(|(from, to, _)| to == parent && from != to)
            .collect();
        rows
    };
    let rows: Vec<Vec<rusqlite::types::Value>> = {
        let mut stmt = conn.prepare(&format!("SELECT * FROM {name}"))?;
        let n = columns.len();

        stmt.query_map([], |r| {
            let mut row: Vec<rusqlite::types::Value> = Vec::with_capacity(n);
            for i in 0..n {
                row.push(r.get(i)?);
            }
            Ok(row)
        })?
        .filter_map(|r| r.ok())
        .collect()
    };
    Ok(ChildRows {
        name: name.to_string(),
        fks,
        columns,
        rows,
    })
}

fn restore_child(conn: &Connection, cap: &ChildRows) -> Result<()> {
    let placeholders = (1..=cap.columns.len())
        .map(|i| format!("?{i}"))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "INSERT INTO {} ({}) VALUES ({})",
        cap.name,
        cap.columns.join(", "),
        placeholders
    );
    'rows: for row in &cap.rows {
        for (from, parent, to) in &cap.fks {
            let idx = cap
                .columns
                .iter()
                .position(|c| c == from)
                .expect("foreign_key_list column exists in table_info");
            if let rusqlite::types::Value::Integer(id) = &row[idx] {
                let n: i64 = conn.query_row(
                    &format!("SELECT COUNT(*) FROM {parent} WHERE {to} = ?1"),
                    [id],
                    |r| r.get(0),
                )?;
                if n == 0 {
                    continue 'rows;
                }
            }
        }
        conn.execute(&sql, rusqlite::params_from_iter(row.iter().cloned()))?;
    }
    Ok(())
}

fn rebuild_table(conn: &Connection, table: &str, rb: &Rebuild<'_>) -> Result<()> {
    conn.execute_batch("PRAGMA defer_foreign_keys = ON;")?;

    let indexes = captured_indexes(conn, table)?;
    let triggers = captured_triggers(conn, table)?;
    let replaced: Vec<String> = rb
        .replacement_indexes
        .iter()
        .map(|s| s.to_string())
        .collect();
    let replaced_names: Vec<String> = rb
        .replacement_indexes
        .iter()
        .filter_map(|ddl| {
            ddl.split_whitespace()
                .position(|w| w.eq_ignore_ascii_case("index"))
                .and_then(|p| ddl.split_whitespace().nth(p + 1))
                .map(|n| n.trim_matches('"').to_string())
        })
        .collect();

    if rb.new_name == table {
        let children: Vec<ChildRows> = fk_children(conn, table)?
            .iter()
            .map(|name| capture_child(conn, name, table))
            .collect::<Result<Vec<_>>>()?;
        conn.execute_batch(&format!(
            "CREATE TABLE \"{}__rebuild\" {};",
            table, rb.new_ddl
        ))?;
        conn.execute_batch(rb.copy_sql)?;
        conn.execute_batch(&format!("DROP TABLE \"{}\";", table))?;
        conn.execute_batch(&format!(
            "ALTER TABLE \"{}__rebuild\" RENAME TO \"{table}\";",
            table
        ))?;
        for ddl in &indexes {
            let name = ddl
                .split_whitespace()
                .position(|w| w.eq_ignore_ascii_case("index"))
                .and_then(|p| ddl.split_whitespace().nth(p + 1))
                .map(|n| n.trim_matches('"').to_string())
                .unwrap_or_default();
            if !replaced_names.iter().any(|n| n == &name) {
                conn.execute_batch(&with_if_not_exists(ddl))?;
            }
        }
        for ddl in &replaced {
            conn.execute_batch(&with_if_not_exists(ddl))?;
        }
        for ddl in &triggers {
            conn.execute_batch(ddl)?;
        }
        for cap in &children {
            restore_child(conn, cap)?;
        }
    } else {
        conn.execute_batch(&format!("CREATE TABLE \"{}\" {};", rb.new_name, rb.new_ddl))?;
        conn.execute_batch(rb.copy_sql)?;
        conn.execute_batch(&format!("DROP TABLE \"{}\";", table))?;
        for ddl in &replaced {
            conn.execute_batch(&with_if_not_exists(ddl))?;
        }
    }

    Ok(())
}

pub fn apply_m044(conn: &Connection) -> Result<()> {
    if table_exists(conn, "docs")? {
        if table_exists(conn, "docs_articles")? {
            conn.execute_batch("DROP TABLE docs;")?;
        } else {
            conn.execute_batch(
                "ALTER TABLE docs RENAME TO docs_articles;
                 CREATE INDEX IF NOT EXISTS idx_docs_articles_collection
                     ON docs_articles (collection_local_id);
                 CREATE INDEX IF NOT EXISTS idx_docs_articles_status
                     ON docs_articles (status);",
            )?;
        }
    }

    let members_exists = table_exists(conn, "issue_cluster_members")?;
    if members_exists {
        if table_exists(conn, "issue_cluster_conversations")? {
            conn.execute_batch("DROP TABLE issue_cluster_members;")?;
        } else {
            rebuild_table(
                conn,
                "issue_cluster_members",
                &Rebuild {
                    new_name: "issue_cluster_conversations",
                    new_ddl: "(
                        cluster_id      INTEGER NOT NULL
                            REFERENCES issue_clusters (id) ON DELETE CASCADE,
                        conversation_id INTEGER NOT NULL
                            REFERENCES conversations (id) ON DELETE CASCADE,
                        assigned_at     TEXT NOT NULL DEFAULT (datetime('now')),
                        PRIMARY KEY (cluster_id, conversation_id)
                    )",
                    copy_sql: "INSERT INTO issue_cluster_conversations
                                    (cluster_id, conversation_id, assigned_at)
                                SELECT m.cluster_id, m.conversation_id, m.assigned_at
                                  FROM issue_cluster_members m
                                 WHERE m.cluster_id IN (SELECT id FROM issue_clusters)
                                   AND m.conversation_id IN (SELECT id FROM conversations)",
                    replacement_indexes: &[
                        "CREATE INDEX idx_issue_cluster_conversations_conversation
                             ON issue_cluster_conversations (conversation_id)",
                    ],
                },
            )?;
        }
    }

    crate::encrypted_sync::ensure_log_table(conn)?;

    let _ = conn.execute("UPDATE app_state SET schema_version = 44 WHERE id = 1", []);
    Ok(())
}

pub fn apply_m045(conn: &Connection) -> Result<()> {
    if crate::sync_schema::column_exists(conn, "conversation_threads", "actor_id")? {
        rebuild_table(
            conn,
            "conversation_threads",
            &Rebuild {
                new_name: "conversation_threads",
                new_ddl: "(
                    id                        INTEGER PRIMARY KEY AUTOINCREMENT,
                    remote_id                 INTEGER,
                    conversation_id           INTEGER NOT NULL
                                                  REFERENCES conversations (id) ON DELETE CASCADE,
                    type                      TEXT,
                    state                     TEXT DEFAULT 'published',
                    body_text                 TEXT,
                    body_html                 TEXT,
                    from_name                 TEXT,
                    from_email                TEXT,
                    from_type                 TEXT,
                    created_by_user_id        INTEGER REFERENCES users (id),
                    created_by_customer_id    INTEGER REFERENCES customers (id),
                    created_by_system_user_id INTEGER REFERENCES system_users (id),
                    assigned_to_type         TEXT,
                    assigned_to_id            INTEGER,
                    saved_reply_local_id      INTEGER REFERENCES saved_replies (id),
                    action_type               TEXT,
                    action_text               TEXT,
                    to_list                   TEXT,
                    cc_list                   TEXT,
                    bcc_list                  TEXT,
                    scheduled_for             TEXT,
                    remote_created_at         TEXT,
                    remote_updated_at         TEXT,
                    local_created_at           TEXT NOT NULL DEFAULT (datetime('now')),
                    local_updated_at           TEXT NOT NULL DEFAULT (datetime('now')),
                    last_synced_at            TEXT,
                    raw_json                  TEXT,
                    raw_json_hash             TEXT,
                    deleted_at                TEXT,
                    fts_indexed               INTEGER DEFAULT 0,
                    embedding_state           TEXT DEFAULT 'not_indexed',
                    created_at                TEXT NOT NULL
                                                  DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
                )",
                copy_sql: "INSERT INTO \"conversation_threads__rebuild\"
                               (id, remote_id, conversation_id, type, state,
                                body_text, body_html, from_name, from_email, from_type,
                                created_by_user_id, created_by_customer_id,
                                created_by_system_user_id,
                                assigned_to_type, assigned_to_id, saved_reply_local_id,
                                action_type, action_text, to_list, cc_list, bcc_list,
                                scheduled_for, remote_created_at, remote_updated_at,
                                local_created_at, local_updated_at, last_synced_at,
                                raw_json, raw_json_hash, deleted_at, fts_indexed,
                                embedding_state, created_at)
                           SELECT t.id, t.remote_id, t.conversation_id,
                                  t.thread_type, t.state, t.body, t.body_html,
                                  t.from_name, t.from_email,
                                  CASE WHEN t.actor_type = 'system'
                                       THEN 'system_user' ELSE t.actor_type END,
                                  CASE WHEN t.actor_type = 'user'
                                            AND t.actor_id IN (SELECT id FROM users)
                                       THEN t.actor_id END,
                                  CASE WHEN t.actor_type = 'customer'
                                            AND t.actor_id IN (SELECT id FROM customers)
                                       THEN t.actor_id END,
                                  CASE WHEN t.actor_type IN ('system', 'system_user')
                                            AND t.actor_id IN (SELECT id FROM system_users)
                                       THEN t.actor_id END,
                                  t.assigned_to_type, t.assigned_to_id,
                                  CASE WHEN t.saved_reply_local_id
                                             IN (SELECT id FROM saved_replies)
                                       THEN t.saved_reply_local_id END,
                                  t.action_type, t.action_text,
                                  t.to_list, t.cc_list, t.bcc_list,
                                  t.scheduled_for,
                                  t.remote_created_at, t.remote_updated_at,
                                  t.local_created_at, t.local_updated_at,
                                  t.last_synced_at, t.raw_json, t.raw_json_hash,
                                  t.deleted_at, t.fts_indexed, t.embedding_state,
                                  t.created_at
                             FROM conversation_threads t
                            WHERE t.conversation_id IN (SELECT id FROM conversations)",
                replacement_indexes: &["CREATE INDEX idx_conv_threads_type
                         ON conversation_threads (type)"],
            },
        )?;
    }
    let _ = conn.execute("UPDATE app_state SET schema_version = 45 WHERE id = 1", []);
    Ok(())
}

pub fn apply_m046(conn: &Connection) -> Result<()> {
    if !fk_declared(conn, "customers", "organization_id")? {
        rebuild_table(
            conn,
            "customers",
            &Rebuild {
                new_name: "customers",
                new_ddl: "(
                    id               INTEGER PRIMARY KEY,
                    remote_id        INTEGER NOT NULL UNIQUE,
                    first_name       TEXT,
                    last_name        TEXT,
                    email            TEXT,
                    organization     TEXT,
                    job_title        TEXT,
                    phone            TEXT,
                    created_at       TEXT,
                    updated_at       TEXT,
                    local_created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
                    deleted_at       TEXT,
                    organization_id  INTEGER REFERENCES organizations (id) ON DELETE SET NULL,
                    photo_url        TEXT,
                    raw_json         TEXT,
                    raw_json_hash    TEXT,
                    remote_created_at TEXT,
                    remote_updated_at TEXT,
                    last_seen_at     TEXT,
                    last_synced_at   TEXT,
                    local_updated_at TEXT NOT NULL DEFAULT (datetime('now')),
                    background       TEXT,
                    age              TEXT,
                    gender           TEXT,
                    location         TEXT
                )",
                copy_sql: "INSERT INTO \"customers__rebuild\"
                               (id, remote_id, first_name, last_name, email, organization,
                                job_title, phone, created_at, updated_at, local_created_at,
                                deleted_at, organization_id, photo_url, raw_json,
                                raw_json_hash, remote_created_at, remote_updated_at,
                                last_seen_at, last_synced_at, local_updated_at,
                                background, age, gender, location)
                           SELECT id, remote_id, first_name, last_name, email, organization,
                                  job_title, phone, created_at, updated_at, local_created_at,
                                  deleted_at,
                                  CASE WHEN organization_id IN (SELECT id FROM organizations)
                                       THEN organization_id END,
                                  photo_url, raw_json, raw_json_hash, remote_created_at,
                                  remote_updated_at, last_seen_at, last_synced_at,
                                  local_updated_at, background, age, gender, location
                             FROM customers",
                replacement_indexes: &[],
            },
        )?;
    }

    if !fk_declared(conn, "attachments", "thread_id")? {
        rebuild_table(
            conn,
            "attachments",
            &Rebuild {
                new_name: "attachments",
                new_ddl: "(
                    id              INTEGER PRIMARY KEY AUTOINCREMENT,
                    remote_id       INTEGER UNIQUE,
                    thread_id       INTEGER NOT NULL
                                        REFERENCES conversation_threads (id) ON DELETE CASCADE,
                    conversation_id INTEGER NOT NULL
                                        REFERENCES conversations (id) ON DELETE CASCADE,
                    filename        TEXT,
                    mime_type       TEXT,
                    size            INTEGER,
                    local_path       TEXT,
                    hash            TEXT,
                    downloaded_at   TEXT,
                    state           TEXT DEFAULT 'metadata',
                    raw_json        TEXT
                )",
                copy_sql: "INSERT INTO \"attachments__rebuild\"
                               (id, remote_id, thread_id, conversation_id, filename,
                                mime_type, size, local_path, hash, downloaded_at,
                                state, raw_json)
                           SELECT a.id, a.remote_id, a.thread_id, a.conversation_id,
                                  a.filename, a.mime_type, a.size, a.local_path, a.hash,
                                  a.downloaded_at, a.state, a.raw_json
                             FROM attachments a
                            WHERE a.conversation_id IN (SELECT id FROM conversations)
                              AND a.thread_id IN (SELECT id FROM conversation_threads)",
                replacement_indexes: &[],
            },
        )?;
    }

    if !fk_declared(conn, "activity_events", "thread_local_id")? {
        rebuild_table(
            conn,
            "activity_events",
            &Rebuild {
                new_name: "activity_events",
                new_ddl: "(
                    id              INTEGER PRIMARY KEY AUTOINCREMENT,
                    conversation_id INTEGER NOT NULL
                                        REFERENCES conversations (id) ON DELETE CASCADE,
                    event_type      TEXT NOT NULL,
                    actor_type      TEXT NOT NULL,
                    actor_id        INTEGER,
                    occurred_at     TEXT NOT NULL,
                    dedup_key        TEXT NOT NULL UNIQUE,
                    created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
                    thread_local_id INTEGER REFERENCES conversation_threads (id) ON DELETE SET NULL,
                    source          TEXT NOT NULL DEFAULT 'sync',
                    metadata        TEXT
                )",
                copy_sql: "INSERT INTO \"activity_events__rebuild\"
                               (id, conversation_id, event_type, actor_type, actor_id,
                                occurred_at, dedup_key, created_at, thread_local_id,
                                source, metadata)
                           SELECT e.id, e.conversation_id, e.event_type, e.actor_type,
                                  e.actor_id, e.occurred_at, e.dedup_key, e.created_at,
                                  CASE WHEN e.thread_local_id IN (SELECT id FROM conversation_threads)
                                       THEN e.thread_local_id END,
                                  e.source, e.metadata
                             FROM activity_events e
                            WHERE e.conversation_id IN (SELECT id FROM conversations)",
                replacement_indexes: &[],
            },
        )?;
    }

    if !fk_declared(conn, "notifications", "conversation_id")? {
        rebuild_table(
            conn,
            "notifications",
            &Rebuild {
                new_name: "notifications",
                new_ddl: "(
                    id              INTEGER PRIMARY KEY AUTOINCREMENT,
                    type            TEXT NOT NULL,
                    severity        TEXT NOT NULL DEFAULT 'info',
                    title           TEXT NOT NULL DEFAULT '',
                    body            TEXT,
                    target_user_id  INTEGER REFERENCES users (id) ON DELETE CASCADE,
                    actor_user_local_id INTEGER REFERENCES users (id) ON DELETE SET NULL,
                    conversation_id INTEGER REFERENCES conversations (id) ON DELETE CASCADE,
                    conversation_number INTEGER,
                    customer_local_id   INTEGER REFERENCES customers (id) ON DELETE SET NULL,
                    issue_id        INTEGER,
                    campaign_id     INTEGER,
                    job_id          INTEGER,
                    side_thread_id  INTEGER,
                    dedup_key        TEXT NOT NULL DEFAULT '',
                    payload         TEXT,
                    read_at         TEXT,
                    created_at      TEXT NOT NULL DEFAULT (datetime('now'))
                )",
                copy_sql: "INSERT INTO \"notifications__rebuild\"
                               (id, type, severity, title, body, target_user_id,
                                actor_user_local_id, conversation_id, conversation_number,
                                customer_local_id, issue_id, campaign_id, job_id,
                                side_thread_id, dedup_key, payload, read_at, created_at)
                           SELECT n.id, n.type, n.severity, n.title, n.body,
                                  n.target_user_id,
                                  CASE WHEN n.actor_user_local_id IN (SELECT id FROM users)
                                       THEN n.actor_user_local_id END,
                                  n.conversation_id, n.conversation_number,
                                  CASE WHEN n.customer_local_id IN (SELECT id FROM customers)
                                       THEN n.customer_local_id END,
                                  n.issue_id, n.campaign_id, n.job_id, n.side_thread_id,
                                  n.dedup_key, n.payload, n.read_at, n.created_at
                             FROM notifications n
                            WHERE (n.conversation_id IS NULL
                                   OR n.conversation_id IN (SELECT id FROM conversations))
                              AND (n.target_user_id IS NULL
                                   OR n.target_user_id IN (SELECT id FROM users))",
                replacement_indexes: &[],
            },
        )?;
    }

    if !fk_declared(conn, "side_threads", "conversation_id")? {
        rebuild_table(
            conn,
            "side_threads",
            &Rebuild {
                new_name: "side_threads",
                new_ddl: "(
                    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
                    conversation_id     INTEGER NOT NULL
                                            REFERENCES conversations (id) ON DELETE CASCADE,
                    created_at          TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
                    created_by_user_id   INTEGER REFERENCES users (id) ON DELETE SET NULL,
                    title               TEXT NOT NULL DEFAULT '',
                    team_local_id       INTEGER REFERENCES teams (id) ON DELETE SET NULL,
                    status              TEXT NOT NULL DEFAULT 'open',
                    updated_at          TEXT NOT NULL DEFAULT (datetime('now')),
                    resolved_at         TEXT
                )",
                copy_sql: "INSERT INTO \"side_threads__rebuild\"
                               (id, conversation_id, created_at, created_by_user_id,
                                title, team_local_id, status, updated_at, resolved_at)
                           SELECT s.id, s.conversation_id, s.created_at,
                                  CASE WHEN s.created_by_user_id IN (SELECT id FROM users)
                                       THEN s.created_by_user_id END,
                                  s.title,
                                  CASE WHEN s.team_local_id IN (SELECT id FROM teams)
                                       THEN s.team_local_id END,
                                  s.status, s.updated_at, s.resolved_at
                             FROM side_threads s
                            WHERE s.conversation_id IN (SELECT id FROM conversations)",
                replacement_indexes: &[],
            },
        )?;
    }

    if !fk_declared(conn, "knowledge_gap_candidates", "decided_by_user_local_id")? {
        rebuild_table(
            conn,
            "knowledge_gap_candidates",
            &Rebuild {
                new_name: "knowledge_gap_candidates",
                new_ddl: "(
                    id               INTEGER PRIMARY KEY AUTOINCREMENT,
                    query_text       TEXT NOT NULL,
                    occurrence_count INTEGER NOT NULL DEFAULT 1,
                    kind             TEXT,
                    status           TEXT NOT NULL DEFAULT 'open'
                                         CHECK (status IN ('open','approved','rejected')),
                    decision_note    TEXT,
                    decided_at       TEXT,
                    created_at       TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
                    dedup_key        TEXT,
                    evidence_conversation_ids TEXT NOT NULL DEFAULT '[]',
                    related_document_ids TEXT NOT NULL DEFAULT '[]',
                    detail           TEXT,
                    decided_by_user_local_id INTEGER REFERENCES users (id) ON DELETE SET NULL,
                    updated_at       TEXT NOT NULL DEFAULT (datetime('now')),
                    provenance       TEXT NOT NULL DEFAULT 'deterministic_local'
                )",
                copy_sql: "INSERT INTO \"knowledge_gap_candidates__rebuild\"
                               (id, query_text, occurrence_count, kind, status,
                                decision_note, decided_at, created_at, dedup_key,
                                evidence_conversation_ids, related_document_ids,
                                detail, decided_by_user_local_id, updated_at, provenance)
                           SELECT k.id, k.query_text, k.occurrence_count, k.kind, k.status,
                                  k.decision_note, k.decided_at, k.created_at, k.dedup_key,
                                  k.evidence_conversation_ids, k.related_document_ids,
                                  k.detail,
                                  CASE WHEN k.decided_by_user_local_id IN (SELECT id FROM users)
                                       THEN k.decided_by_user_local_id END,
                                  k.updated_at, k.provenance
                             FROM knowledge_gap_candidates k",
                replacement_indexes: &[],
            },
        )?;
    }

    if !fk_declared(conn, "customer_memory", "customer_id")? {
        rebuild_table(
            conn,
            "customer_memory",
            &Rebuild {
                new_name: "customer_memory",
                new_ddl: "(
                    id              INTEGER PRIMARY KEY AUTOINCREMENT,
                    customer_id     INTEGER NOT NULL
                                        REFERENCES customers (id) ON DELETE CASCADE,
                    memory_key      TEXT NOT NULL,
                    memory_value    TEXT NOT NULL,
                    evidence_excerpt TEXT NOT NULL,
                    source_conversation_id INTEGER
                                        REFERENCES conversations (id) ON DELETE SET NULL,
                    created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
                    source          TEXT NOT NULL DEFAULT 'ai',
                    origin          TEXT DEFAULT 'conversation',
                    first_seen_at   TEXT,
                    last_seen_at    TEXT,
                    confidence      TEXT DEFAULT 'unknown',
                    provenance      TEXT DEFAULT 'ai_generated',
                    kind            TEXT NOT NULL DEFAULT 'fact'
                )",
                copy_sql: "INSERT INTO \"customer_memory__rebuild\"
                               (id, customer_id, memory_key, memory_value, evidence_excerpt,
                                source_conversation_id, created_at, source, origin,
                                first_seen_at, last_seen_at, confidence, provenance, kind)
                           SELECT m.id, m.customer_id, m.memory_key, m.memory_value,
                                  m.evidence_excerpt,
                                  CASE WHEN m.source_conversation_id
                                            IN (SELECT id FROM conversations)
                                       THEN m.source_conversation_id END,
                                  m.created_at, m.source, m.origin, m.first_seen_at,
                                  m.last_seen_at, m.confidence, m.provenance, m.kind
                             FROM customer_memory m
                            WHERE m.customer_id IN (SELECT id FROM customers)",
                replacement_indexes: &[],
            },
        )?;
    }

    if !fk_declared(conn, "issue_clusters", "known_issue_id")? {
        rebuild_table(
            conn,
            "issue_clusters",
            &Rebuild {
                new_name: "issue_clusters",
                new_ddl: "(
                    id              INTEGER PRIMARY KEY AUTOINCREMENT,
                    name            TEXT NOT NULL,
                    conversation_count INTEGER NOT NULL DEFAULT 0,
                    first_seen_at   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
                    last_seen_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
                    status          TEXT NOT NULL DEFAULT 'active',
                    trend           TEXT NOT NULL DEFAULT 'stable',
                    known_issue_id  INTEGER REFERENCES known_issues (id) ON DELETE SET NULL,
                    product         TEXT,
                    title           TEXT,
                    summary         TEXT,
                    category        TEXT,
                    feature         TEXT,
                    ai_generated    INTEGER DEFAULT 0,
                    customer_count  INTEGER DEFAULT 0,
                    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
                    updated_at      TEXT NOT NULL DEFAULT (datetime('now')),
                    provenance      TEXT DEFAULT 'ai_generated'
                )",
                copy_sql: "INSERT INTO \"issue_clusters__rebuild\"
                               (id, name, conversation_count, first_seen_at, last_seen_at,
                                status, trend, known_issue_id, product, title, summary,
                                category, feature, ai_generated, customer_count,
                                created_at, updated_at, provenance)
                           SELECT i.id, i.name, i.conversation_count, i.first_seen_at,
                                  i.last_seen_at, i.status, i.trend,
                                  CASE WHEN i.known_issue_id IN (SELECT id FROM known_issues)
                                       THEN i.known_issue_id END,
                                  i.product, i.title, i.summary, i.category, i.feature,
                                  i.ai_generated, i.customer_count, i.created_at,
                                  i.updated_at, i.provenance
                             FROM issue_clusters i",
                replacement_indexes: &[],
            },
        )?;
    }

    if !fk_declared(conn, "ai_runs", "conversation_id")? {
        rebuild_table(
            conn,
            "ai_runs",
            &Rebuild {
                new_name: "ai_runs",
                new_ddl: "(
                    id              INTEGER PRIMARY KEY AUTOINCREMENT,
                    input_hash      TEXT NOT NULL,
                    prompt_version  TEXT NOT NULL,
                    model           TEXT NOT NULL,
                    response_json   TEXT NOT NULL,
                    created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
                    type            TEXT NOT NULL DEFAULT 'analysis',
                    conversation_id INTEGER REFERENCES conversations (id) ON DELETE SET NULL,
                    status          TEXT NOT NULL DEFAULT 'queued',
                    input_refs      TEXT,
                    error           TEXT,
                    latency_ms      INTEGER,
                    token_usage     TEXT,
                    started_at      TEXT,
                    completed_at    TEXT,
                    provenance      TEXT NOT NULL DEFAULT 'ai_generated'
                )",
                copy_sql: "INSERT INTO \"ai_runs__rebuild\"
                               (id, input_hash, prompt_version, model, response_json,
                                created_at, type, conversation_id, status, input_refs,
                                error, latency_ms, token_usage, started_at, completed_at,
                                provenance)
                           SELECT r.id, r.input_hash, r.prompt_version, r.model,
                                  r.response_json, r.created_at, r.type,
                                  CASE WHEN r.conversation_id IN (SELECT id FROM conversations)
                                       THEN r.conversation_id END,
                                  r.status, r.input_refs, r.error, r.latency_ms,
                                  r.token_usage, r.started_at, r.completed_at,
                                  r.provenance
                             FROM ai_runs r",
                replacement_indexes: &[],
            },
        )?;
    }

    if !fk_declared(conn, "outreach_campaigns", "mailbox_local_id")? {
        rebuild_table(
            conn,
            "outreach_campaigns",
            &Rebuild {
                new_name: "outreach_campaigns",
                new_ddl: "(
                    id              INTEGER PRIMARY KEY AUTOINCREMENT,
                    name            TEXT NOT NULL,
                    subject         TEXT NOT NULL,
                    body            TEXT NOT NULL,
                    mailbox_local_id INTEGER REFERENCES mailboxes (id),
                    tags            TEXT NOT NULL DEFAULT '[]',
                    status          TEXT NOT NULL DEFAULT 'draft',
                    segment_id      INTEGER REFERENCES saved_segments (id),
                    segment_snapshot TEXT,
                    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
                    queued_at       TEXT,
                    completed_at    TEXT,
                    updated_at      TEXT NOT NULL DEFAULT (datetime('now'))
                )",
                copy_sql: "INSERT INTO \"outreach_campaigns__rebuild\"
                               (id, name, subject, body, mailbox_local_id, tags, status,
                                segment_id, segment_snapshot, created_at, queued_at,
                                completed_at, updated_at)
                           SELECT o.id, o.name, o.subject, o.body,
                                  CASE WHEN o.mailbox_local_id IN (SELECT id FROM mailboxes)
                                       THEN o.mailbox_local_id END,
                                  o.tags, o.status,
                                  CASE WHEN o.segment_id IN (SELECT id FROM saved_segments)
                                       THEN o.segment_id END,
                                  o.segment_snapshot, o.created_at, o.queued_at,
                                  o.completed_at, o.updated_at
                             FROM outreach_campaigns o",
                replacement_indexes: &[],
            },
        )?;
    }

    if !fk_declared(conn, "ticket_state_transitions", "conversation_remote_id")? {
        rebuild_table(
            conn,
            "ticket_state_transitions",
            &Rebuild {
                new_name: "ticket_state_transitions",
                new_ddl: "(
                    id                     INTEGER PRIMARY KEY AUTOINCREMENT,
                    conversation_remote_id INTEGER NOT NULL
                                               REFERENCES conversations (remote_id) ON DELETE CASCADE,
                    from_state             TEXT,
                    to_state               TEXT,
                    from_priority          TEXT,
                    to_priority            TEXT,
                    actor_type             TEXT NOT NULL,
                    actor_id               INTEGER,
                    transitioned_at        TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
                )",
                copy_sql: "INSERT INTO \"ticket_state_transitions__rebuild\"
                               (id, conversation_remote_id, from_state, to_state,
                                from_priority, to_priority, actor_type, actor_id,
                                transitioned_at)
                           SELECT t.id, t.conversation_remote_id, t.from_state, t.to_state,
                                  t.from_priority, t.to_priority, t.actor_type, t.actor_id,
                                  t.transitioned_at
                             FROM ticket_state_transitions t
                            WHERE t.conversation_remote_id IN (SELECT remote_id FROM conversations)",
                replacement_indexes: &[],
            },
        )?;
    }

    let _ = conn.execute("UPDATE app_state SET schema_version = 46 WHERE id = 1", []);
    Ok(())
}

pub fn apply_m047(conn: &Connection) -> Result<()> {
    if crate::sync_schema::column_exists(conn, "conversations", "mailbox_id")? {
        rebuild_table(
            conn,
            "conversations",
            &Rebuild {
                new_name: "conversations",
                new_ddl: "(
                    id                          INTEGER PRIMARY KEY AUTOINCREMENT,
                    remote_id                   INTEGER UNIQUE NOT NULL,
                    number                      INTEGER UNIQUE,
                    subject                     TEXT,
                    preview                     TEXT,
                    status                      TEXT NOT NULL DEFAULT 'active',
                    state                       TEXT DEFAULT 'published',
                    type                        TEXT,
                    mailbox_local_id            INTEGER REFERENCES mailboxes (id),
                    folder_local_id             INTEGER REFERENCES folders (id),
                    customer_local_id           INTEGER REFERENCES customers (id),
                    assignee_local_id           INTEGER REFERENCES users (id),
                    assigned_team_local_id      INTEGER REFERENCES teams (id),
                    closed_by                   INTEGER,
                    closed_at                   TEXT,
                    snoozed_until               TEXT,
                    thread_count                INTEGER DEFAULT 0,
                    is_unread                   INTEGER DEFAULT 0,
                    hs_url                      TEXT,
                    merged_into_conversation_id INTEGER REFERENCES conversations (id),
                    remote_created_at           TEXT,
                    remote_updated_at           TEXT,
                    local_created_at            TEXT NOT NULL DEFAULT (datetime('now')),
                    local_updated_at            TEXT NOT NULL DEFAULT (datetime('now')),
                    last_seen_at                TEXT,
                    last_synced_at              TEXT,
                    first_activity_at            TEXT,
                    last_activity_at            TEXT,
                    raw_json                    TEXT,
                    raw_json_hash               TEXT,
                    deleted_at                 TEXT,
                    source_type                 TEXT,
                    source_via                  TEXT,
                    first_customer_message_at   TEXT,
                    first_response_at           TEXT,
                    last_customer_reply_at      TEXT,
                    last_human_agent_response_at TEXT,
                    last_system_response_at     TEXT,
                    last_note_at                TEXT,
                    customer_waiting_since      TEXT,
                    last_status_change_at       TEXT,
                    last_assignment_change_at   TEXT,
                    last_tag_change_at          TEXT,
                    last_custom_field_change_at TEXT,
                    activity_history_complete   INTEGER NOT NULL DEFAULT 0,
                    supportos_priority          TEXT NOT NULL DEFAULT 'none',
                    supportos_state_id          INTEGER REFERENCES ticket_states (id),
                    priority                    TEXT,
                    created_at                  TEXT,
                    updated_at                  TEXT,
                    response_state              TEXT NOT NULL DEFAULT 'needs_first_response',
                    supportos_state             TEXT
                )",
                copy_sql: "INSERT INTO \"conversations__rebuild\"
                               (id, remote_id, number, subject, preview, status,
                                state, type, mailbox_local_id, folder_local_id,
                                customer_local_id, assignee_local_id,
                                assigned_team_local_id, closed_by, closed_at,
                                snoozed_until, thread_count, is_unread, hs_url,
                                merged_into_conversation_id, remote_created_at,
                                remote_updated_at, local_created_at, local_updated_at,
                                last_seen_at, last_synced_at, first_activity_at,
                                last_activity_at, raw_json, raw_json_hash, deleted_at,
                                source_type, source_via, first_customer_message_at,
                                first_response_at, last_customer_reply_at,
                                last_human_agent_response_at, last_system_response_at,
                                last_note_at, customer_waiting_since,
                                last_status_change_at, last_assignment_change_at,
                                last_tag_change_at, last_custom_field_change_at,
                                activity_history_complete, supportos_priority,
                                supportos_state_id, priority, created_at, updated_at,
                                response_state, supportos_state)
                           SELECT c.id, c.remote_id,
                                  CASE WHEN c.number IS NULL THEN NULL
                                       WHEN EXISTS (SELECT 1 FROM conversations n
                                                    WHERE n.number = c.number
                                                      AND n.id < c.id)
                                       THEN NULL ELSE c.number END,
                                  c.subject, c.preview, c.status, c.state, c.type,
                                  CASE WHEN c.mailbox_id IN (SELECT id FROM mailboxes)
                                       THEN c.mailbox_id END,
                                  CASE WHEN c.folder_local_id IN (SELECT id FROM folders)
                                       THEN c.folder_local_id END,
                                  CASE WHEN c.customer_id IN (SELECT id FROM customers)
                                       THEN c.customer_id END,
                                  CASE WHEN c.assignee_id IN (SELECT id FROM users)
                                       THEN c.assignee_id END,
                                  CASE WHEN c.assigned_team_local_id IN (SELECT id FROM teams)
                                       THEN c.assigned_team_local_id END,
                                  c.closed_by, c.closed_at, c.snoozed_until,
                                  c.thread_count, c.is_unread, c.hs_url,
                                  CASE WHEN c.merged_into_conversation_id
                                            IN (SELECT id FROM conversations)
                                       THEN c.merged_into_conversation_id END,
                                  c.remote_created_at, c.remote_updated_at,
                                  c.local_created_at, c.local_updated_at,
                                  c.last_seen_at, c.last_synced_at,
                                  c.first_activity_at, c.last_activity_at,
                                  c.raw_json, c.raw_json_hash, c.deleted_at,
                                  c.source_type, c.source_via,
                                  c.first_customer_message_at, c.first_response_at,
                                  c.last_customer_reply_at,
                                  c.last_human_agent_response_at,
                                  c.last_system_response_at, c.last_note_at,
                                  c.customer_waiting_since,
                                  c.last_status_change_at,
                                  c.last_assignment_change_at,
                                  c.last_tag_change_at,
                                  c.last_custom_field_change_at,
                                  COALESCE(c.activity_history_complete, 0),
                                  COALESCE(c.supportos_priority, 'none'),
                                  CASE WHEN c.supportos_state_id
                                            IN (SELECT id FROM ticket_states)
                                       THEN c.supportos_state_id END,
                                  c.priority, c.created_at, c.updated_at,
                                  c.response_state, c.supportos_state
                             FROM conversations c",
                replacement_indexes: &[
                    "CREATE INDEX idx_conversations_mailbox
                         ON conversations (mailbox_local_id, status)",
                    "CREATE INDEX idx_conversations_assignee
                         ON conversations (assignee_local_id, status)",
                    "CREATE INDEX idx_conversations_customer
                         ON conversations (customer_local_id)",
                    "CREATE INDEX idx_conversations_customer_status
                         ON conversations (customer_local_id, status)",
                    "CREATE INDEX idx_conversations_customer_created
                         ON conversations (customer_local_id, remote_created_at DESC)",
                ],
            },
        )?;
    }
    let _ = conn.execute("UPDATE app_state SET schema_version = 47 WHERE id = 1", []);
    Ok(())
}

fn fk_declared(conn: &Connection, table: &str, column: &str) -> Result<bool> {
    let mut stmt = conn.prepare(&format!("PRAGMA foreign_key_list({table})"))?;
    let hits = stmt
        .query_map([], |r| r.get::<_, String>(3))?
        .filter_map(|r| r.ok())
        .filter(|from| from == column)
        .count();
    Ok(hits > 0)
}
