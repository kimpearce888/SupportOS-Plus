use rusqlite::Connection;

use crate::error::Result;

pub fn apply_m039(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS knowledge_sources (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            name        TEXT NOT NULL,
            kind        TEXT DEFAULT 'local_file',
            visibility  TEXT NOT NULL DEFAULT 'internal_only',
            created_at  TEXT NOT NULL DEFAULT (datetime('now'))
        );

        CREATE TABLE IF NOT EXISTS knowledge_documents (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            source_id       INTEGER NOT NULL REFERENCES knowledge_sources (id) ON DELETE CASCADE,
            title           TEXT NOT NULL,
            visibility      TEXT NOT NULL DEFAULT 'internal_only',
            version         INTEGER DEFAULT 1,
            checksum        TEXT,
            content         TEXT,
            format          TEXT DEFAULT 'markdown',
            last_reviewed_at TEXT,
            last_verified_at TEXT,
            last_indexed_at TEXT,
            created_at      TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at      TEXT NOT NULL DEFAULT (datetime('now')),
            provenance      TEXT DEFAULT 'human_local'
        );
        CREATE INDEX IF NOT EXISTS idx_knowledge_documents_source
            ON knowledge_documents (source_id);

        CREATE TABLE IF NOT EXISTS knowledge_chunks (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            document_id     INTEGER NOT NULL REFERENCES knowledge_documents (id) ON DELETE CASCADE,
            chunk_index     INTEGER NOT NULL,
            content         TEXT NOT NULL,
            fts_indexed     INTEGER DEFAULT 0,
            embedding_state TEXT DEFAULT 'not_indexed',
            embedding_model TEXT,
            embedding       BLOB,
            chunk_version   INTEGER DEFAULT 2,
            created_at      TEXT NOT NULL DEFAULT (datetime('now')),
            UNIQUE (document_id, chunk_index)
        );
        CREATE INDEX IF NOT EXISTS idx_knowledge_chunks_doc ON knowledge_chunks (document_id);

        CREATE TABLE IF NOT EXISTS docs_chunks (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            article_id      INTEGER NOT NULL,
            chunk_index     INTEGER NOT NULL,
            content         TEXT NOT NULL,
            embedding       BLOB,
            embedding_model TEXT,
            embedding_state TEXT NOT NULL DEFAULT 'not_indexed',
            chunk_version   INTEGER NOT NULL DEFAULT 2,
            created_at      TEXT NOT NULL DEFAULT (datetime('now')),
            UNIQUE (article_id, chunk_index)
        );
        CREATE INDEX IF NOT EXISTS idx_docs_chunks_article ON docs_chunks (article_id);
        CREATE INDEX IF NOT EXISTS idx_docs_chunks_state ON docs_chunks (embedding_state);

        CREATE TABLE IF NOT EXISTS conversation_chunks (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            conversation_id INTEGER NOT NULL REFERENCES conversations (id) ON DELETE CASCADE,
            chunk_index     INTEGER NOT NULL,
            content         TEXT NOT NULL,
            embedding       BLOB,
            embedding_model TEXT,
            embedding_state TEXT NOT NULL DEFAULT 'not_indexed',
            chunk_version   INTEGER NOT NULL DEFAULT 2,
            created_at      TEXT NOT NULL DEFAULT (datetime('now')),
            UNIQUE (conversation_id, chunk_index)
        );
        CREATE INDEX IF NOT EXISTS idx_conversation_chunks_conversation
            ON conversation_chunks (conversation_id);
        CREATE INDEX IF NOT EXISTS idx_conversation_chunks_state
            ON conversation_chunks (embedding_state);

        CREATE TABLE IF NOT EXISTS customer_emails (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            remote_id   INTEGER UNIQUE,
            customer_id INTEGER NOT NULL REFERENCES customers (id) ON DELETE CASCADE,
            value       TEXT NOT NULL,
            type        TEXT,
            UNIQUE (customer_id, value)
        );
        CREATE INDEX IF NOT EXISTS idx_customer_emails_value ON customer_emails (value);

        CREATE TABLE IF NOT EXISTS customer_phones (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            remote_id   INTEGER UNIQUE,
            customer_id INTEGER NOT NULL REFERENCES customers (id) ON DELETE CASCADE,
            value       TEXT,
            type        TEXT
        );

        CREATE TABLE IF NOT EXISTS customer_addresses (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            remote_id   INTEGER UNIQUE,
            customer_id INTEGER NOT NULL REFERENCES customers (id) ON DELETE CASCADE,
            lines       TEXT,
            city        TEXT,
            state       TEXT,
            postal_code TEXT,
            country     TEXT
        );

        CREATE TABLE IF NOT EXISTS customer_websites (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            remote_id   INTEGER UNIQUE,
            customer_id INTEGER NOT NULL REFERENCES customers (id) ON DELETE CASCADE,
            value       TEXT
        );

        CREATE TABLE IF NOT EXISTS customer_social_profiles (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            remote_id   INTEGER UNIQUE,
            customer_id INTEGER NOT NULL REFERENCES customers (id) ON DELETE CASCADE,
            value       TEXT,
            type        TEXT
        );

        CREATE TABLE IF NOT EXISTS customer_properties (
            id            INTEGER PRIMARY KEY AUTOINCREMENT,
            customer_id   INTEGER NOT NULL REFERENCES customers (id) ON DELETE CASCADE,
            definition_id INTEGER NOT NULL REFERENCES customer_property_definitions (id),
            value         TEXT,
            UNIQUE (customer_id, definition_id)
        );

        CREATE TABLE IF NOT EXISTS support_cases (
            id                  INTEGER PRIMARY KEY AUTOINCREMENT,
            conversation_id     INTEGER UNIQUE REFERENCES conversations (id) ON DELETE CASCADE,
            customer_id         INTEGER REFERENCES customers (id),
            problem             TEXT,
            root_question       TEXT,
            resolution          TEXT,
            answer              TEXT,
            product             TEXT,
            feature             TEXT,
            tags                TEXT,
            fields              TEXT,
            agent_user_id       INTEGER REFERENCES users (id),
            resolution_time_min REAL,
            rating              TEXT,
            created_at          TEXT NOT NULL DEFAULT (datetime('now')),
            provenance          TEXT DEFAULT 'local_derived'
        );
        CREATE INDEX IF NOT EXISTS idx_support_cases_conversation
            ON support_cases (conversation_id);

        CREATE TABLE IF NOT EXISTS ai_extracted_facts (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            conversation_id INTEGER REFERENCES conversations (id) ON DELETE CASCADE,
            run_id          INTEGER REFERENCES ai_runs (id) ON DELETE CASCADE,
            key             TEXT NOT NULL,
            value           TEXT,
            confidence      TEXT,
            created_at      TEXT NOT NULL DEFAULT (datetime('now'))
        );
        CREATE INDEX IF NOT EXISTS idx_ai_facts_conversation
            ON ai_extracted_facts (conversation_id);

        CREATE TABLE IF NOT EXISTS ai_sources (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            run_id      INTEGER NOT NULL REFERENCES ai_runs (id) ON DELETE CASCADE,
            source_type TEXT NOT NULL,
            source_id   INTEGER NOT NULL,
            title       TEXT,
            relevance   REAL,
            visibility  TEXT,
            timestamp   TEXT
        );",
    )?;
    add_column_if_missing(conn, "known_issues", "title", "TEXT")?;
    add_column_if_missing(conn, "known_issues", "symptoms", "TEXT")?;
    add_column_if_missing(conn, "known_issues", "feature", "TEXT")?;
    add_column_if_missing(conn, "known_issues", "known_cause", "TEXT")?;
    add_column_if_missing(conn, "known_issues", "workaround", "TEXT")?;
    add_column_if_missing(conn, "known_issues", "customer_safe_explanation", "TEXT")?;
    add_column_if_missing(conn, "known_issues", "internal_explanation", "TEXT")?;
    add_column_if_missing(
        conn,
        "known_issues",
        "provenance",
        "TEXT DEFAULT 'human_local'",
    )?;
    add_column_if_missing(conn, "known_issues", "product", "TEXT")?;
    add_column_if_missing(conn, "known_issues", "first_seen_at", "TEXT")?;
    add_column_if_missing(conn, "known_issues", "last_seen_at", "TEXT")?;
    add_column_if_missing(
        conn,
        "known_issues",
        "conversation_count",
        "INTEGER DEFAULT 0",
    )?;
    let uq_exists: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master
              WHERE type = 'index' AND name = 'uq_known_issue_links'",
            [],
            |r| r.get::<_, i64>(0),
        )
        .map(|n| n > 0)
        .unwrap_or(false);
    if !uq_exists {
        conn.execute(
            "DELETE FROM known_issue_links WHERE id NOT IN (
                 SELECT MIN(id) FROM known_issue_links
                  GROUP BY known_issue_id, conversation_id)",
            [],
        )?;
        conn.execute_batch(
            "CREATE UNIQUE INDEX IF NOT EXISTS uq_known_issue_links
                 ON known_issue_links (known_issue_id, conversation_id);",
        )?;
    }
    add_column_if_missing(conn, "issue_clusters", "title", "TEXT")?;
    add_column_if_missing(conn, "issue_clusters", "summary", "TEXT")?;
    add_column_if_missing(conn, "issue_clusters", "category", "TEXT")?;
    add_column_if_missing(conn, "issue_clusters", "feature", "TEXT")?;
    add_column_if_missing(conn, "issue_clusters", "ai_generated", "INTEGER DEFAULT 0")?;
    let _ = conn.execute("UPDATE app_state SET schema_version = 39 WHERE id = 1", []);
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
