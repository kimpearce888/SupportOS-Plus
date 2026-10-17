use rusqlite::Connection;

use crate::error::Result;

pub fn apply_m029(conn: &Connection) -> Result<()> {
    apply_inner(conn)?;
    let _ = conn.execute("UPDATE app_state SET schema_version = 29 WHERE id = 1", []);
    Ok(())
}

fn apply_inner(conn: &Connection) -> Result<()> {
    if !column_exists(conn, "sync_runs", "kind")? {
        conn.execute_batch(
            "ALTER TABLE sync_runs RENAME TO sync_runs_legacy_m029;
             CREATE TABLE sync_runs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                kind TEXT NOT NULL,
                state TEXT NOT NULL,
                started_at TEXT NOT NULL,
                finished_at TEXT,
                resources_done INTEGER DEFAULT 0,
                resources_total INTEGER DEFAULT 0,
                records_processed INTEGER DEFAULT 0,
                errors INTEGER DEFAULT 0,
                detail TEXT
            );
             CREATE INDEX IF NOT EXISTS idx_sync_runs_started ON sync_runs(started_at DESC);
             INSERT INTO sync_runs (id, kind, state, started_at, finished_at, records_processed, errors)
               SELECT id, 'manual', COALESCE(status, 'completed'), COALESCE(started_at, datetime('now')),
                      completed_at, resources_synced,
                      CASE WHEN error IS NOT NULL THEN 1 ELSE 0 END
               FROM sync_runs_legacy_m029;
             DROP TABLE sync_runs_legacy_m029;",
        )?;
    }

    if table_exists(conn, "application_settings")?
        && !column_exists(conn, "application_settings", "updated_at")?
    {
        conn.execute_batch(
            "ALTER TABLE application_settings ADD COLUMN updated_at TEXT;
             UPDATE application_settings SET updated_at = datetime('now') WHERE updated_at IS NULL;",
        )?;
    }

    if !column_exists(conn, "sync_checkpoints", "last_success_at")? {
        conn.execute_batch(
            "ALTER TABLE sync_checkpoints RENAME TO sync_checkpoints_legacy_m029;
             CREATE TABLE sync_checkpoints (
                resource TEXT PRIMARY KEY,
                last_success_at TEXT,
                remote_cursor TEXT,
                page_state TEXT,
                records_processed INTEGER DEFAULT 0,
                records_failed INTEGER DEFAULT 0,
                last_error TEXT,
                retry_count INTEGER DEFAULT 0,
                status TEXT DEFAULT 'idle'
            );
             INSERT INTO sync_checkpoints (resource, last_success_at, remote_cursor, records_processed, status)
               SELECT resource, recorded_at, cursor_token, 0, 'idle'
               FROM sync_checkpoints_legacy_m029;
             DROP TABLE sync_checkpoints_legacy_m029;",
        )?;
    }

    if !table_exists(conn, "jobs")? {
        conn.execute_batch(
            "CREATE TABLE jobs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                queue TEXT NOT NULL,
                type TEXT NOT NULL,
                priority INTEGER NOT NULL DEFAULT 2,
                status TEXT NOT NULL DEFAULT 'queued',
                payload TEXT,
                attempt INTEGER DEFAULT 0,
                max_attempts INTEGER DEFAULT 3,
                error TEXT,
                run_at TEXT,
                locked_by TEXT,
                locked_at TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                started_at TEXT,
                completed_at TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_jobs_status ON jobs(status, priority, run_at);
            CREATE INDEX IF NOT EXISTS idx_jobs_queue ON jobs(queue, status);",
        )?;
    } else if !column_exists(conn, "jobs", "queue")? {
        conn.execute_batch(
            "ALTER TABLE jobs RENAME TO jobs_legacy_m029;
             CREATE TABLE jobs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                queue TEXT NOT NULL,
                type TEXT NOT NULL,
                priority INTEGER NOT NULL DEFAULT 2,
                status TEXT NOT NULL DEFAULT 'queued',
                payload TEXT,
                attempt INTEGER DEFAULT 0,
                max_attempts INTEGER DEFAULT 3,
                error TEXT,
                run_at TEXT,
                locked_by TEXT,
                locked_at TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                started_at TEXT,
                completed_at TEXT
            );
             CREATE INDEX IF NOT EXISTS idx_jobs_status ON jobs(status, priority, run_at);
             CREATE INDEX IF NOT EXISTS idx_jobs_queue ON jobs(queue, status);
             INSERT INTO jobs (id, queue, type, priority, status, payload, attempt, max_attempts,
                               error, run_at, created_at, started_at, completed_at)
               SELECT id, 'sync', kind, 2,
                      CASE state
                        WHEN 'pending' THEN 'queued'
                        WHEN 'queued' THEN 'queued'
                        WHEN 'claimed' THEN 'running'
                        WHEN 'running' THEN 'running'
                        WHEN 'done' THEN 'completed'
                        WHEN 'completed' THEN 'completed'
                        WHEN 'dead' THEN 'failed'
                        WHEN 'failed' THEN 'failed'
                        WHEN 'cancelled' THEN 'cancelled'
                        ELSE 'queued' END,
                      payload, attempts, max_attempts, last_error, available_at,
                      available_at, claimed_at, completed_at
               FROM jobs_legacy_m029;
             DROP TABLE jobs_legacy_m029;",
        )?;
    }

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS outbound_jobs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            kind TEXT NOT NULL,
            conversation_id INTEGER REFERENCES conversations(id) ON DELETE SET NULL,
            thread_id INTEGER REFERENCES conversation_threads(id) ON DELETE SET NULL,
            payload TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'queued',
            attempts INTEGER DEFAULT 0,
            max_attempts INTEGER DEFAULT 1,
            error TEXT,
            remote_result TEXT,
            requires_confirmation INTEGER DEFAULT 0,
            confirmed_at TEXT,
            idempotency_key TEXT UNIQUE,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now'))
        );
        CREATE INDEX IF NOT EXISTS idx_outbound_status ON outbound_jobs(status);

        CREATE TABLE IF NOT EXISTS outbound_attempts (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            outbound_job_id INTEGER NOT NULL REFERENCES outbound_jobs(id) ON DELETE CASCADE,
            attempt INTEGER NOT NULL,
            request_summary TEXT,
            status_code INTEGER,
            response_body TEXT,
            latency_ms INTEGER,
            at TEXT NOT NULL DEFAULT (datetime('now'))
        );

        CREATE TABLE IF NOT EXISTS audit_log (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            timestamp TEXT NOT NULL DEFAULT (datetime('now')),
            actor TEXT NOT NULL,
            action TEXT NOT NULL,
            conversation_id INTEGER,
            before_state TEXT,
            after_state TEXT,
            remote_operation TEXT,
            remote_result TEXT,
            ai_involvement INTEGER DEFAULT 0,
            job_id INTEGER,
            correlation_id TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_audit_conversation ON audit_log(conversation_id);
        CREATE INDEX IF NOT EXISTS idx_audit_time ON audit_log(timestamp DESC);

        CREATE TABLE IF NOT EXISTS application_errors (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            timestamp TEXT NOT NULL DEFAULT (datetime('now')),
            service TEXT,
            message TEXT,
            stack TEXT,
            context TEXT
        );",
    )?;

    if table_exists(conn, "conversation_threads")?
        && !column_exists(conn, "conversation_threads", "remote_id")?
    {
        conn.execute_batch(
            "ALTER TABLE conversation_threads ADD COLUMN remote_id INTEGER;
             CREATE UNIQUE INDEX IF NOT EXISTS idx_conv_threads_remote
                 ON conversation_threads (remote_id);",
        )?;
    }

    if table_exists(conn, "issue_clusters")? && !column_exists(conn, "issue_clusters", "trend")? {
        conn.execute_batch(
            "ALTER TABLE issue_clusters ADD COLUMN trend TEXT NOT NULL DEFAULT 'stable';",
        )?;
    }

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS accounts (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            remote_id INTEGER UNIQUE,
            plan TEXT,
            company_name TEXT,
            raw_json TEXT,
            raw_json_hash TEXT,
            last_seen_at TEXT,
            local_created_at TEXT NOT NULL DEFAULT (datetime('now')),
            local_updated_at TEXT NOT NULL DEFAULT (datetime('now'))
        );

        CREATE TABLE IF NOT EXISTS folders (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            remote_id INTEGER UNIQUE NOT NULL,
            mailbox_id INTEGER NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
            name TEXT NOT NULL,
            type TEXT,
            user_id INTEGER,
            total_count INTEGER DEFAULT 0,
            active_count INTEGER DEFAULT 0,
            raw_json TEXT,
            last_synced_at TEXT,
            deleted_at TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_folders_mailbox ON folders(mailbox_id);

        CREATE TABLE IF NOT EXISTS inbox_fields (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            remote_id INTEGER UNIQUE NOT NULL,
            mailbox_id INTEGER NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
            name TEXT NOT NULL,
            type TEXT,
            system_type TEXT,
            required INTEGER DEFAULT 0,
            sort_order INTEGER DEFAULT 0,
            raw_json TEXT,
            last_synced_at TEXT,
            deleted_at TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_inbox_fields_mailbox ON inbox_fields(mailbox_id);

        CREATE TABLE IF NOT EXISTS inbox_field_options (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            remote_id INTEGER UNIQUE NOT NULL,
            field_id INTEGER NOT NULL REFERENCES inbox_fields(id) ON DELETE CASCADE,
            label TEXT,
            sort_order INTEGER DEFAULT 0
        );

        CREATE TABLE IF NOT EXISTS customer_property_definitions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            remote_id INTEGER UNIQUE NOT NULL,
            name TEXT NOT NULL,
            slug TEXT,
            type TEXT,
            sort_order INTEGER DEFAULT 0,
            raw_json TEXT,
            last_synced_at TEXT,
            deleted_at TEXT
        );

        CREATE TABLE IF NOT EXISTS organization_property_definitions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            remote_id INTEGER UNIQUE NOT NULL,
            name TEXT NOT NULL,
            slug TEXT,
            type TEXT,
            sort_order INTEGER DEFAULT 0,
            raw_json TEXT,
            last_synced_at TEXT,
            deleted_at TEXT
        );

        CREATE TABLE IF NOT EXISTS organizations (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            remote_id INTEGER UNIQUE NOT NULL,
            name TEXT NOT NULL,
            domains TEXT,
            raw_json TEXT,
            raw_json_hash TEXT,
            remote_created_at TEXT,
            remote_updated_at TEXT,
            last_seen_at TEXT,
            last_synced_at TEXT,
            local_created_at TEXT NOT NULL DEFAULT (datetime('now')),
            local_updated_at TEXT NOT NULL DEFAULT (datetime('now')),
            deleted_at TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_organizations_name ON organizations(name);

        CREATE TABLE IF NOT EXISTS saved_replies (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            remote_id INTEGER UNIQUE,
            mailbox_local_id INTEGER REFERENCES mailboxes(id),
            name TEXT NOT NULL,
            preview TEXT,
            text TEXT,
            chat_text TEXT,
            raw_json TEXT,
            last_synced_at TEXT,
            deleted_at TEXT
        );

        CREATE TABLE IF NOT EXISTS workflows (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            remote_id INTEGER UNIQUE,
            mailbox_local_id INTEGER REFERENCES mailboxes(id),
            name TEXT NOT NULL,
            type TEXT,
            status TEXT,
            sort_order INTEGER,
            raw_json TEXT,
            remote_updated_at TEXT,
            last_synced_at TEXT,
            deleted_at TEXT
        );

        CREATE TABLE IF NOT EXISTS user_statuses (
            user_local_id INTEGER PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
            email_status TEXT,
            email_updated_at TEXT,
            chat_status TEXT,
            mailbox_statuses TEXT,
            raw_json TEXT,
            last_synced_at TEXT
        );

        CREATE TABLE IF NOT EXISTS webhook_configs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            remote_id INTEGER UNIQUE,
            url TEXT,
            events TEXT,
            status TEXT,
            raw_json TEXT,
            last_synced_at TEXT
        );

        CREATE TABLE IF NOT EXISTS docs_collections (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            remote_id INTEGER UNIQUE,
            slug TEXT,
            name TEXT NOT NULL,
            description TEXT,
            raw_json TEXT,
            last_synced_at TEXT,
            deleted_at TEXT
        );

        CREATE TABLE IF NOT EXISTS docs_categories (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            remote_id INTEGER UNIQUE,
            collection_local_id INTEGER NOT NULL REFERENCES docs_collections(id) ON DELETE CASCADE,
            slug TEXT,
            name TEXT NOT NULL,
            raw_json TEXT,
            last_synced_at TEXT,
            deleted_at TEXT
        );

        CREATE TABLE IF NOT EXISTS docs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            remote_id INTEGER UNIQUE,
            collection_local_id INTEGER REFERENCES docs_collections(id) ON DELETE SET NULL,
            category_local_id INTEGER REFERENCES docs_categories(id) ON DELETE SET NULL,
            slug TEXT,
            number INTEGER,
            revision INTEGER,
            name TEXT NOT NULL,
            text TEXT,
            text_plain TEXT,
            status TEXT,
            visibility TEXT,
            views INTEGER DEFAULT 0,
            raw_json TEXT,
            remote_created_at TEXT,
            remote_updated_at TEXT,
            last_synced_at TEXT,
            deleted_at TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_docs_collection ON docs(collection_local_id);

        CREATE TABLE IF NOT EXISTS ratings (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            remote_id INTEGER UNIQUE,
            conversation_id INTEGER REFERENCES conversations(id) ON DELETE CASCADE,
            thread_local_id INTEGER REFERENCES conversation_threads(id),
            rating TEXT,
            comments TEXT,
            customer_local_id INTEGER REFERENCES customers(id),
            user_local_id INTEGER REFERENCES users(id),
            remote_created_at TEXT,
            raw_json TEXT,
            last_synced_at TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_ratings_conversation ON ratings(conversation_id);",
    )?;

    Ok(())
}

pub fn apply_m042(conn: &Connection) -> Result<()> {
    if table_exists(conn, "sync_cursors")? && !column_exists(conn, "sync_cursors", "cursor")? {
        conn.execute_batch(
            "ALTER TABLE sync_cursors RENAME TO sync_cursors_legacy_m042;
             CREATE TABLE sync_cursors (
                resource TEXT PRIMARY KEY,
                cursor TEXT,
                updated_at TEXT
             );
             INSERT INTO sync_cursors (resource, cursor, updated_at)
               SELECT resource, cursor_token, COALESCE(last_seen_at, datetime('now'))
                 FROM sync_cursors_legacy_m042
                WHERE cursor_token IS NOT NULL AND cursor_token != '';
             DROP TABLE sync_cursors_legacy_m042;",
        )?;
    } else if !table_exists(conn, "sync_cursors")? {
        conn.execute_batch(
            "CREATE TABLE sync_cursors (
                resource TEXT PRIMARY KEY,
                cursor TEXT,
                updated_at TEXT
             );",
        )?;
    }
    Ok(())
}

pub fn column_exists(conn: &Connection, table: &str, column: &str) -> Result<bool> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        let name: String = row.get(1)?;
        if name == column {
            return Ok(true);
        }
    }
    Ok(false)
}

pub fn table_exists(conn: &Connection, table: &str) -> Result<bool> {
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
        [table],
        |r| r.get(0),
    )?;
    Ok(n > 0)
}
