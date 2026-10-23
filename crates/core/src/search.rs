use rusqlite::{Connection, OptionalExtension, ToSql, params};
use serde::{Deserialize, Serialize};

use crate::error::Result;

pub const MAX_SEARCH_QUERY_LENGTH: usize = 500;

pub const MAX_RESULTS_PER_TYPE: u32 = 50;

pub const SEARCH_SCOPES: &[&str] = &[
    "all",
    "tickets",
    "customers",
    "knowledge",
    "issues",
    "saved_replies",
    "ai",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchHit {
    pub scope: String,
    pub id: i64,
    pub title: String,
    pub subtitle: String,
    pub snippet: String,
    #[serde(serialize_with = "serialize_score")]
    pub score: f64,
    pub href: String,
    pub why: Vec<String>,
}

fn serialize_score<S>(value: &f64, serializer: S) -> std::result::Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    if value.is_finite() && value.fract() == 0.0 && value.abs() < 9.0e15 {
        serializer.serialize_i64(*value as i64)
    } else {
        serializer.serialize_f64(*value)
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SearchFilters {
    pub status: Option<String>,
    pub mailbox_id: Option<i64>,
    pub tag: Option<String>,
    pub since_days: Option<i64>,
    pub assignee_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResponse {
    pub query: String,
    pub hits: Vec<SearchHit>,
    pub total: usize,
    pub used_semantic: bool,
    pub semantic_available: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode_note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeRawHit {
    pub document_id: i64,
    pub title: String,
    pub snippet: String,
    pub visibility: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub resource_type: String,
    pub remote_id: i64,
    pub title: String,
    pub snippet: String,
}

pub fn apply_fts_migration(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE VIRTUAL TABLE IF NOT EXISTS fts_conversations USING fts5(
            subject, preview, customer, tags, numbers,
            conversation_id UNINDEXED
        );

        CREATE VIRTUAL TABLE IF NOT EXISTS fts_threads USING fts5(
            body,
            thread_id UNINDEXED, conversation_id UNINDEXED
        );

        CREATE VIRTUAL TABLE IF NOT EXISTS fts_knowledge USING fts5(
            title, content,
            chunk_id UNINDEXED, document_id UNINDEXED, visibility UNINDEXED
        );

        CREATE VIRTUAL TABLE IF NOT EXISTS fts_known_issues USING fts5(
            title, symptoms, workaround, customer_safe_explanation,
            known_issue_id UNINDEXED
        );

        CREATE VIRTUAL TABLE IF NOT EXISTS fts_saved_replies USING fts5(
            name, preview, text,
            saved_reply_id UNINDEXED
        );

        CREATE VIRTUAL TABLE IF NOT EXISTS fts_ai_analyses USING fts5(
            summary, primary_question, intent,
            conversation_id UNINDEXED, run_id UNINDEXED
        );

        CREATE VIRTUAL TABLE IF NOT EXISTS docs_fts USING fts5(
            name, text,
            article_id UNINDEXED
        );

        CREATE VIRTUAL TABLE IF NOT EXISTS fts_custom_objects USING fts5(
            title,
            search_text,
            object_id UNINDEXED
        );

        -- Legacy pre-parity tables (index_conversation / index_customer).
        CREATE VIRTUAL TABLE IF NOT EXISTS conversations_fts
         USING fts5(remote_id UNINDEXED, subject, preview);
        CREATE VIRTUAL TABLE IF NOT EXISTS customers_fts
         USING fts5(remote_id UNINDEXED, first_name, last_name, email, organization);",
    )?;

    if table_exists(conn, "conversations")? {
        add_column_if_missing(conn, "conversations", "deleted_at", "TEXT")?;
        add_column_if_missing(
            conn,
            "conversations",
            "merged_into_conversation_id",
            "INTEGER",
        )?;
    }
    if table_exists(conn, "customers")? {
        add_column_if_missing(conn, "customers", "deleted_at", "TEXT")?;
        add_column_if_missing(conn, "customers", "organization_id", "INTEGER")?;
    }
    if table_exists(conn, "conversation_threads")? {
        add_column_if_missing(conn, "conversation_threads", "deleted_at", "TEXT")?;
        add_column_if_missing(
            conn,
            "conversation_threads",
            "fts_indexed",
            "INTEGER NOT NULL DEFAULT 0",
        )?;
    }
    if table_exists(conn, "known_issues")? {
        add_column_if_missing(conn, "known_issues", "title", "TEXT")?;
        add_column_if_missing(conn, "known_issues", "symptoms", "TEXT")?;
        add_column_if_missing(conn, "known_issues", "workaround", "TEXT")?;
        add_column_if_missing(conn, "known_issues", "customer_safe_explanation", "TEXT")?;
    }

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

        CREATE TABLE IF NOT EXISTS customer_emails (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            remote_id   INTEGER UNIQUE,
            customer_id INTEGER NOT NULL REFERENCES customers (id) ON DELETE CASCADE,
            value       TEXT NOT NULL,
            type        TEXT,
            UNIQUE (customer_id, value)
        );
        CREATE INDEX IF NOT EXISTS idx_customer_emails_value ON customer_emails (value);

        CREATE TABLE IF NOT EXISTS ai_runs (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            input_hash      TEXT NOT NULL,
            prompt_version  TEXT NOT NULL,
            model           TEXT NOT NULL,
            response_json   TEXT NOT NULL,
            created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
        );
        CREATE INDEX IF NOT EXISTS idx_ai_runs_lookup
            ON ai_runs (input_hash, prompt_version, model);

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

        CREATE TABLE IF NOT EXISTS conversation_tags (
            conversation_id INTEGER NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
            tag_id         INTEGER NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
            PRIMARY KEY (conversation_id, tag_id)
        );
        CREATE INDEX IF NOT EXISTS idx_conversation_tags_tag
            ON conversation_tags(tag_id);",
    )?;
    Ok(())
}

fn table_exists(conn: &Connection, name: &str) -> Result<bool> {
    let exists = conn
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type IN ('table', 'view') AND name = ?1",
            params![name],
            |_| Ok(()),
        )
        .optional()?;
    Ok(exists.is_some())
}

fn column_exists(conn: &Connection, table: &str, column: &str) -> Result<bool> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let present = stmt
        .query_map([], |row| row.get::<_, String>(1))?
        .filter_map(|c| c.ok())
        .any(|c| c == column);
    Ok(present)
}

fn add_column_if_missing(conn: &Connection, table: &str, column: &str, decl: &str) -> Result<()> {
    if !column_exists(conn, table, column)? {
        conn.execute(
            &format!("ALTER TABLE {table} ADD COLUMN {column} {decl}"),
            [],
        )?;
    }
    Ok(())
}

#[must_use]
pub fn fts_query(q: &str) -> String {
    let tokens = tokenize(q, 1, 8);
    if tokens.is_empty() {
        return "\"\"".to_string();
    }
    tokens
        .iter()
        .map(|t| format!("\"{t}\"*"))
        .collect::<Vec<_>>()
        .join(" ")
}

#[must_use]
pub fn fts_query_or(q: &str, max_tokens: usize) -> String {
    let tokens = tokenize(q, 3, max_tokens);
    if tokens.is_empty() {
        return "\"\"".to_string();
    }
    tokens
        .iter()
        .map(|t| format!("\"{t}\"*"))
        .collect::<Vec<_>>()
        .join(" OR ")
}

/// Tokenize a search query. Public so the native shell's docs search builds
/// the exact same FTS patterns as the HTTP route.
pub fn tokenize(q: &str, min_chars: usize, max_tokens: usize) -> Vec<String> {
    let cleaned: String = q
        .chars()
        .map(|c| {
            if matches!(c, '"' | '*' | '(' | ')') {
                ' '
            } else {
                c
            }
        })
        .collect();
    cleaned
        .split_whitespace()
        .filter(|t| t.chars().count() >= min_chars)
        .take(max_tokens)
        .map(str::to_string)
        .collect()
}

/// Escape a user-supplied LIKE pattern body so `%`, `_` and the escape
/// character `\` match literally (the pattern's wrapping `%…%` and the
/// query's `ESCAPE '\\'` clause stay in charge of the wildcarding).
///
/// Clean-audit item 83: the inbox query filter, the customers search
/// and the saved-replies mirror search interpolated raw input into LIKE
/// patterns — a `%`/`_` in the search text acted as a wildcard and a
/// trailing `\` escaped the wrapping percent, silently breaking the
/// search. Shared by every LIKE-with-user-input call site.
#[must_use]
pub fn escape_like(q: &str) -> String {
    let mut out = String::with_capacity(q.len());
    for c in q.chars() {
        if matches!(c, '\\' | '%' | '_') {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

pub fn search_conversations(
    conn: &Connection,
    query: &str,
    filters: &SearchFilters,
    limit: i64,
) -> Result<Vec<SearchHit>> {
    let fts = fts_query(query);
    if fts == "\"\"" && !has_filters(filters) {
        return recent_conversations(conn, limit);
    }

    let mut where_sql = vec![
        "c.deleted_at IS NULL".to_string(),
        "c.merged_into_conversation_id IS NULL".to_string(),
    ];
    let mut args: Vec<Box<dyn ToSql>> = Vec::new();

    if status_applies(filters) {
        let idx = args.len() + 1;
        where_sql.push(format!("c.status = ?{idx}"));
        args.push(Box::new(filters.status.clone().unwrap_or_default()));
    }
    if let Some(mailbox) = filters.mailbox_id.filter(|v| *v != 0) {
        let idx = args.len() + 1;
        where_sql.push(format!("c.mailbox_local_id = ?{idx}"));
        args.push(Box::new(mailbox));
    }
    if let Some(tag) = filters.tag.clone().filter(|t| !t.is_empty()) {
        let idx = args.len() + 1;
        where_sql.push(format!(
            "EXISTS (SELECT 1 FROM conversation_tags ct JOIN tags t ON t.id = ct.tag_id \
             WHERE ct.conversation_id = c.id AND t.name = ?{idx} COLLATE NOCASE)"
        ));
        args.push(Box::new(tag));
    }
    if let Some(days) = filters.since_days.filter(|v| *v != 0) {
        let idx = args.len() + 1;
        where_sql.push(format!(
            "julianday(c.created_at) >= julianday('now', '-' || ?{idx} || ' days')"
        ));
        args.push(Box::new(days));
    }
    if let Some(assignee) = filters.assignee_id.filter(|v| *v != 0) {
        let idx = args.len() + 1;
        where_sql.push(format!("c.assignee_local_id = ?{idx}"));
        args.push(Box::new(assignee));
    }
    let trimmed = query.trim();
    if !trimmed.is_empty()
        && trimmed.chars().all(|c| c.is_ascii_digit())
        && let Ok(num) = trimmed.parse::<i64>()
    {
        let idx = args.len() + 1;
        where_sql.push(format!("c.number = ?{idx}"));
        args.push(Box::new(num));
    }

    let mut join_fts = String::new();
    if fts != "\"\"" {
        let idx = args.len() + 1;
        join_fts = format!(
            "JOIN (
                SELECT conversation_id, MIN(rank) AS r FROM (
                    SELECT conversation_id, rank FROM fts_conversations WHERE fts_conversations MATCH ?{idx}
                    UNION ALL
                    SELECT f.conversation_id, rank FROM fts_threads f WHERE fts_threads MATCH ?{idx}
                    UNION ALL
                    SELECT c2.id, rank FROM fts_ai_analyses a JOIN conversations c2 ON c2.id = a.conversation_id WHERE fts_ai_analyses MATCH ?{idx}
                ) GROUP BY conversation_id
            ) m ON m.conversation_id = c.id"
        );
        args.push(Box::new(fts));
    }
    let rank_expr = if join_fts.is_empty() {
        "0"
    } else {
        "COALESCE(m.r, 999999)"
    };
    let limit_idx = args.len() + 1;
    args.push(Box::new(limit));

    let sql = format!(
        "SELECT c.id, c.number, c.subject, c.preview, c.status, c.created_at,
           TRIM(COALESCE(cu.first_name, '') || ' ' || COALESCE(cu.last_name, '')) AS customer,
           COALESCE((SELECT ce.value FROM customer_emails ce WHERE ce.customer_id = cu.id LIMIT 1), '') AS customer_email,
           {rank_expr} AS rank
         FROM conversations c
         LEFT JOIN customers cu ON cu.id = c.customer_local_id
         {join_fts}
         WHERE {}
         ORDER BY rank ASC, c.created_at DESC
         LIMIT ?{limit_idx}",
        where_sql.join(" AND ")
    );

    let mut stmt = conn.prepare(&sql)?;
    let arg_refs: Vec<&dyn ToSql> = args.iter().map(Box::as_ref).collect();
    let rows = stmt
        .query_map(arg_refs.as_slice(), |r| {
            let id: i64 = r.get(0)?;
            let number: i64 = r.get(1)?;
            let subject: Option<String> = r.get(2)?;
            Ok(SearchHit {
                scope: "tickets".into(),
                id,
                title: format!(
                    "#{number} {}",
                    subject.unwrap_or_else(|| "(no subject)".into())
                ),
                subtitle: [
                    r.get::<_, String>(6)?,
                    r.get::<_, String>(7)?,
                    r.get::<_, String>(4)?,
                ]
                .into_iter()
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join(" · "),
                snippet: r.get::<_, Option<String>>(3)?.unwrap_or_default(),
                score: 1.0,
                href: format!("/inbox/conversation/{id}"),
                why: vec!["keyword match".into()],
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

fn recent_conversations(conn: &Connection, limit: i64) -> Result<Vec<SearchHit>> {
    let mut stmt = conn.prepare(
        "SELECT id, number, subject, preview, status
           FROM conversations
          WHERE deleted_at IS NULL AND merged_into_conversation_id IS NULL
          ORDER BY updated_at DESC LIMIT ?1",
    )?;
    let rows = stmt
        .query_map(params![limit], |r| {
            let id: i64 = r.get(0)?;
            let number: i64 = r.get(1)?;
            Ok(SearchHit {
                scope: "tickets".into(),
                id,
                title: format!(
                    "#{number} {}",
                    r.get::<_, Option<String>>(2)?
                        .unwrap_or_else(|| "(no subject)".into())
                ),
                subtitle: r.get(4)?,
                snippet: r.get::<_, Option<String>>(3)?.unwrap_or_default(),
                score: 0.5,
                href: format!("/inbox/conversation/{id}"),
                why: vec!["recent".into()],
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

fn status_applies(f: &SearchFilters) -> bool {
    matches!(f.status.as_deref(), Some(s) if !s.is_empty() && s != "all")
}

fn has_filters(f: &SearchFilters) -> bool {
    status_applies(f)
        || f.mailbox_id.is_some_and(|v| v != 0)
        || f.tag.as_deref().is_some_and(|t| !t.is_empty())
        || f.since_days.is_some_and(|v| v != 0)
        || f.assignee_id.is_some_and(|v| v != 0)
}

pub fn search_customers(conn: &Connection, query: &str, limit: i64) -> Result<Vec<SearchHit>> {
    if query.trim().is_empty() {
        return Ok(Vec::new());
    }
    let like = format!("%{}%", escape_like(query));
    let mut stmt = conn.prepare(
        "SELECT c.id, c.first_name, c.last_name,
           (SELECT ce.value FROM customer_emails ce WHERE ce.customer_id = c.id LIMIT 1) AS email,
           o.name AS org,
           (SELECT COUNT(*) FROM conversations cv WHERE cv.customer_local_id = c.id) AS conv_count
         FROM customers c LEFT JOIN organizations o ON o.id = c.organization_id
         WHERE c.deleted_at IS NULL AND (
           c.first_name LIKE ?1 ESCAPE '\\' OR c.last_name LIKE ?1 ESCAPE '\\' OR
           EXISTS (SELECT 1 FROM customer_emails ce WHERE ce.customer_id = c.id AND ce.value LIKE ?1 ESCAPE '\\')
         )
         ORDER BY c.last_name LIMIT ?2",
    )?;
    let rows = stmt
        .query_map(params![like, limit], |r| {
            let id: i64 = r.get(0)?;
            let first: Option<String> = r.get(1)?;
            let last: Option<String> = r.get(2)?;
            let email: Option<String> = r.get(3)?;
            let org: Option<String> = r.get(4)?;
            let conv_count: i64 = r.get(5)?;
            let title = [first, last]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join(" ");
            Ok(SearchHit {
                scope: "customers".into(),
                id,
                title: if title.is_empty() {
                    "Unknown".into()
                } else {
                    title
                },
                subtitle: email.unwrap_or_default(),
                snippet: match org.filter(|o| !o.is_empty()) {
                    Some(org) => format!("{org} · {conv_count} conversations"),
                    None => format!("{conv_count} conversations"),
                },
                score: 1.0,
                href: format!("/customers/{id}"),
                why: vec!["name/email match".into()],
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

#[allow(clippy::ref_option)]
pub fn search_knowledge_raw(
    conn: &Connection,
    query: &str,
    visibility: Option<&str>,
    limit: i64,
    mode: &str,
) -> Result<Vec<KnowledgeRawHit>> {
    if query.trim().is_empty() {
        return Ok(Vec::new());
    }
    let fts = if mode == "or" {
        fts_query_or(query, 10)
    } else {
        fts_query(query)
    };
    let vis_sql = visibility.map_or("", |_| "AND f.visibility = ?2");
    // Clean-audit item 58: `fts_knowledge` holds one row PER CHUNK, so a
    // multi-chunk document matched on several chunks used to flood the
    // LIMIT with copies of itself (a 10-chunk document was "half" of a
    // LIMIT 20 result) while single-chunk documents were pushed out. The
    // rows are consumed rank-ordered and deduped per document — the best
    // (lowest-rank) chunk speaks for its document — until `limit` DISTINCT
    // documents have been collected.
    let sql = format!(
        "SELECT f.document_id, f.title, snippet(fts_knowledge, 1, '[', ']', '…', 14) AS snippet, f.visibility
           FROM fts_knowledge f WHERE fts_knowledge MATCH ?1 {vis_sql} ORDER BY rank"
    );
    let limit = limit.max(0) as usize;
    let mut stmt = conn.prepare(&sql)?;
    let map_row = |r: &rusqlite::Row<'_>| -> rusqlite::Result<KnowledgeRawHit> {
        Ok(KnowledgeRawHit {
            document_id: r.get(0)?,
            title: r.get(1)?,
            snippet: r.get(2)?,
            visibility: r.get(3)?,
        })
    };
    let rows = match visibility {
        Some(v) => stmt.query_map(params![fts, v], map_row)?,
        None => stmt.query_map(params![fts], map_row)?,
    };
    let mut hits: Vec<KnowledgeRawHit> = Vec::new();
    let mut seen_documents: std::collections::HashSet<i64> = std::collections::HashSet::new();
    for row in rows {
        let hit = row?;
        if seen_documents.insert(hit.document_id) {
            hits.push(hit);
            if hits.len() >= limit {
                break;
            }
        }
    }
    Ok(hits)
}

pub fn search_knowledge(
    conn: &Connection,
    query: &str,
    visibility: Option<&str>,
    limit: i64,
) -> Result<Vec<SearchHit>> {
    Ok(search_knowledge_raw(conn, query, visibility, limit, "and")?
        .into_iter()
        .map(|r| SearchHit {
            scope: "knowledge".into(),
            id: r.document_id,
            title: r.title,
            subtitle: format!(
                "Knowledge · {}",
                if r.visibility == "customer_safe" {
                    "customer-safe"
                } else {
                    "internal"
                }
            ),
            snippet: r.snippet,
            score: 1.0,
            href: format!("/knowledge?doc={}", r.document_id),
            why: vec!["knowledge match".into()],
        })
        .collect())
}

pub fn search_known_issues(conn: &Connection, query: &str, limit: i64) -> Result<Vec<SearchHit>> {
    if query.trim().is_empty() {
        return Ok(Vec::new());
    }
    if !table_exists(conn, "known_issues")? {
        return Ok(Vec::new());
    }
    let fts = fts_query(query);
    let mut stmt = conn.prepare(
        "SELECT ki.id, ki.title, snippet(fts_known_issues, 0, '[', ']', '…', 14) AS snippet
           FROM fts_known_issues f JOIN known_issues ki ON ki.id = f.known_issue_id
          WHERE fts_known_issues MATCH ?1 ORDER BY rank LIMIT ?2",
    )?;
    let rows = stmt
        .query_map(params![fts, limit], |r| {
            Ok(SearchHit {
                scope: "issues".into(),
                id: r.get(0)?,
                title: r.get(1)?,
                subtitle: "Known issue".into(),
                snippet: r.get(2)?,
                score: 1.0,
                href: "/issues?tab=known".into(),
                why: vec!["known issue match".into()],
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub fn search_saved_replies(conn: &Connection, query: &str, limit: i64) -> Result<Vec<SearchHit>> {
    if query.trim().is_empty() {
        return Ok(Vec::new());
    }
    if !table_exists(conn, "saved_replies")? {
        return Ok(Vec::new());
    }
    let fts = fts_query(query);
    let mut stmt = conn.prepare(
        "SELECT sr.id, sr.name, snippet(fts_saved_replies, 1, '[', ']', '…', 14) AS snippet
           FROM fts_saved_replies f JOIN saved_replies sr ON sr.id = f.saved_reply_id
          WHERE fts_saved_replies MATCH ?1 AND sr.deleted_at IS NULL ORDER BY rank LIMIT ?2",
    )?;
    let rows = stmt
        .query_map(params![fts, limit], |r| {
            Ok(SearchHit {
                scope: "saved_replies".into(),
                id: r.get(0)?,
                title: r.get(1)?,
                subtitle: "Saved reply".into(),
                snippet: r.get(2)?,
                score: 1.0,
                href: "/inbox".into(),
                why: vec!["saved reply match".into()],
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub fn search_ai_analyses(conn: &Connection, query: &str, limit: i64) -> Result<Vec<SearchHit>> {
    if query.trim().is_empty() {
        return Ok(Vec::new());
    }
    let fts = fts_query(query);
    let mut stmt = conn.prepare(
        "SELECT a.conversation_id, c.number, snippet(fts_ai_analyses, 0, '[', ']', '…', 14) AS snippet, c.subject
           FROM fts_ai_analyses a JOIN conversations c ON c.id = a.conversation_id
          WHERE fts_ai_analyses MATCH ?1 AND c.deleted_at IS NULL ORDER BY rank LIMIT ?2",
    )?;
    let rows = stmt
        .query_map(params![fts, limit], |r| {
            let conversation_id: i64 = r.get(0)?;
            let number: i64 = r.get(1)?;
            let subject: Option<String> = r.get(3)?;
            Ok(SearchHit {
                scope: "ai".into(),
                id: conversation_id,
                title: format!("AI analysis · #{number} {}", subject.unwrap_or_default()),
                subtitle: "AI-derived (marked as AI-generated)".into(),
                snippet: r.get(2)?,
                score: 1.0,
                href: format!("/inbox/conversation/{conversation_id}"),
                why: vec!["AI analysis match".into()],
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub fn search(
    conn: &Connection,
    query: &str,
    scope: &str,
    filters: &SearchFilters,
) -> Result<SearchResponse> {
    apply_fts_migration(conn)?;
    let scope = if scope.is_empty() { "all" } else { scope };
    let mut hits = Vec::new();
    if scope == "all" || scope == "tickets" {
        hits.extend(search_conversations(conn, query, filters, 40)?);
    }
    if scope == "all" || scope == "customers" {
        hits.extend(search_customers(conn, query, 15)?);
    }
    if scope == "all" || scope == "knowledge" {
        hits.extend(search_knowledge(conn, query, None, 10)?);
    }
    if scope == "all" || scope == "issues" {
        hits.extend(search_known_issues(conn, query, 10)?);
    }
    if scope == "all" || scope == "saved_replies" {
        hits.extend(search_saved_replies(conn, query, 10)?);
    }
    if scope == "all" || scope == "ai" {
        hits.extend(search_ai_analyses(conn, query, 10)?);
    }
    let total = hits.len();
    Ok(SearchResponse {
        query: query.to_string(),
        hits,
        total,
        used_semantic: false,
        semantic_available: false,
        mode_note: None,
    })
}

/// A4 retrieval spine: FTS search fused with real sqlite-vec hits.
///
/// The BM25 lanes run exactly as [`search`] does; when the retrieval feature
/// is enabled, an Embed role is routed, and a vector index is available, the
/// query is embedded and dense hits are fused per lane via the existing RRF
/// k=60 math (see `hybrid_search::RRF_K`). The reported flags are real:
/// `used_semantic` is set only when vector hits actually made the list, and
/// every fused hit carries its lane provenance ("keyword match" /
/// "semantic match") in `why`. Absent any part of the chain the response is
/// the honest FTS-only result with `semantic_available: false` (or a
/// one-line mode note explaining the fallback).
pub fn search_with_semantics(
    conn: &Connection,
    index: Option<&crate::vectorstore_sqlitevec::VectorIndex>,
    query: &str,
    scope: &str,
    filters: &SearchFilters,
) -> Result<SearchResponse> {
    let mut response = search(conn, query, scope, filters)?;
    let scope = if scope.is_empty() { "all" } else { scope };

    let index = match index {
        Some(ix) => ix,
        None => {
            response.semantic_available = false;
            response.mode_note =
                Some("FTS only — the vector index is off (Settings > Vector index).".into());
            return Ok(response);
        }
    };
    if !crate::ai_hub::ai_feature_enabled(conn, "retrieval") {
        response.semantic_available = false;
        response.mode_note =
            Some("FTS only — semantic retrieval is disabled (AI Center > Features).".into());
        return Ok(response);
    }
    let embed_settings = crate::embeddings::lm_studio_embedding_settings(conn)?;
    if embed_settings.embedding_model.is_none() {
        response.semantic_available = false;
        response.mode_note = Some(
            "FTS only — no embedding model routed (AI Center > Model-role routing, Embed role)."
                .into(),
        );
        return Ok(response);
    }
    response.semantic_available = true;

    let query_vec = match crate::embeddings::embed_query_text_blocking(conn, query) {
        Ok(Some(v)) => v,
        Ok(None) => {
            response.mode_note = Some(
                "FTS only — the embedding endpoint could not be reached for this query.".into(),
            );
            return Ok(response);
        }
        Err(e) => {
            response.mode_note = Some(format!(
                "FTS only — the embedding endpoint could not be reached for this query: {e}"
            ));
            return Ok(response);
        }
    };
    let dense = index.search(&query_vec, 60);

    // Map dense hits to per-lane semantic SearchHits.
    let want_tickets = scope == "all" || scope == "tickets";
    let want_knowledge = scope == "all" || scope == "knowledge";
    let mut semantic_hits: Vec<SearchHit> = Vec::new();
    for hit in dense {
        let payload = match hit.payload.as_object() {
            Some(p) => p,
            None => continue,
        };
        let entity_type = payload.get("entity_type").and_then(|v| v.as_str());
        let entity_id = payload.get("entity_id").and_then(|v| v.as_i64());
        let Some(entity_id) = entity_id else { continue };
        let text: String = payload
            .get("text")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .chars()
            .take(240)
            .collect();
        match entity_type {
            Some("conversation_chunk") if want_tickets => {
                let Ok((number, subject, status)) = conn.query_row(
                    "SELECT number, subject, status FROM conversations
                          WHERE id = ?1 AND deleted_at IS NULL",
                    params![entity_id],
                    |r| {
                        Ok((
                            r.get::<_, i64>(0)?,
                            r.get::<_, Option<String>>(1)?,
                            r.get::<_, Option<String>>(2)?,
                        ))
                    },
                ) else {
                    continue;
                };
                semantic_hits.push(SearchHit {
                    scope: "tickets".into(),
                    id: entity_id,
                    title: format!(
                        "#{number} {}",
                        subject.unwrap_or_else(|| "(no subject)".into())
                    ),
                    subtitle: status.unwrap_or_default(),
                    snippet: text,
                    score: (f64::from(hit.score) * 100.0).round() / 100.0,
                    href: format!("/inbox/conversation/{entity_id}"),
                    why: vec!["semantic match".into()],
                });
            }
            Some("knowledge_chunk") if want_knowledge => {
                let Ok((document_id, title)) = conn.query_row(
                    "SELECT d.id, d.title FROM knowledge_chunks c
                          JOIN knowledge_documents d ON d.id = c.document_id
                          WHERE c.id = ?1",
                    params![
                        payload
                            .get("chunk_id")
                            .and_then(|v| v.as_i64())
                            .unwrap_or(0)
                    ],
                    |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)),
                ) else {
                    continue;
                };
                semantic_hits.push(SearchHit {
                    scope: "knowledge".into(),
                    id: document_id,
                    title,
                    subtitle: "Knowledge base".into(),
                    snippet: text,
                    score: (f64::from(hit.score) * 100.0).round() / 100.0,
                    href: format!("/knowledge?doc={document_id}"),
                    why: vec!["semantic match".into()],
                });
            }
            Some("docs_chunk") if want_knowledge => {
                let Ok((article_id, name)) = conn.query_row(
                    "SELECT a.id, a.name FROM docs_articles a WHERE a.id = ?1",
                    params![entity_id],
                    |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)),
                ) else {
                    continue;
                };
                semantic_hits.push(SearchHit {
                    scope: "knowledge".into(),
                    id: article_id,
                    title: name,
                    subtitle: "Help center article".into(),
                    snippet: text,
                    score: (f64::from(hit.score) * 100.0).round() / 100.0,
                    href: format!("/knowledge/help-center/{article_id}"),
                    why: vec!["semantic match".into()],
                });
            }
            _ => {}
        }
    }
    if semantic_hits.is_empty() {
        response.mode_note =
            Some("FTS only — the vector index has no indexed content for this query yet.".into());
        return Ok(response);
    }

    // RRF fusion (k=60, same constant as hybrid_search): per lane, every hit
    // scores 1/(k + rank) on each list it appears on. FTS rank is position in
    // its lane; semantic rank is by cosine score.
    let k = crate::hybrid_search::RRF_K;
    let mut fused: Vec<SearchHit> = Vec::new();
    for lane in ["tickets", "knowledge"] {
        let fts_lane: Vec<&SearchHit> = response.hits.iter().filter(|h| h.scope == lane).collect();
        let sem_lane: Vec<&SearchHit> = semantic_hits.iter().filter(|h| h.scope == lane).collect();
        if sem_lane.is_empty() {
            continue;
        }
        struct Fused {
            hit: SearchHit,
            rrf: f64,
        }
        let mut merged: Vec<Fused> = Vec::new();
        let push_or_merge = |hit: &SearchHit, rrf: f64, merged: &mut Vec<Fused>| {
            if let Some(existing) = merged
                .iter_mut()
                .find(|f| f.hit.scope == hit.scope && f.hit.id == hit.id)
            {
                existing.rrf += rrf;
                for reason in &hit.why {
                    if !existing.hit.why.contains(reason) {
                        existing.hit.why.push(reason.clone());
                    }
                }
            } else {
                merged.push(Fused {
                    hit: hit.clone(),
                    rrf,
                });
            }
        };
        for (rank, hit) in fts_lane.iter().enumerate() {
            push_or_merge(hit, 1.0 / (f64::from(k) + rank as f64 + 1.0), &mut merged);
        }
        for (rank, hit) in sem_lane.iter().enumerate() {
            push_or_merge(hit, 1.0 / (f64::from(k) + rank as f64 + 1.0), &mut merged);
        }
        merged.sort_by(|a, b| {
            b.rrf
                .partial_cmp(&a.rrf)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let lane_limit = if lane == "tickets" { 40 } else { 10 };
        for (i, f) in merged.into_iter().enumerate() {
            if i >= lane_limit {
                break;
            }
            let mut hit = f.hit;
            hit.score = (f.rrf * 10_000.0).round() / 10_000.0;
            fused.push(hit);
        }
        // The FTS-only hits of this lane are superseded by the fused list.
        response.hits.retain(|h| h.scope != lane);
    }
    let fused_any = !fused.is_empty();
    // Clean-audit item 59: the final sort ranks the whole response by score,
    // but the fused lanes carry RRF-scale scores (≈ 0.016–0.033) while the
    // untouched FTS-only lanes (customers/issues/saved_replies/ai) still
    // carried their FTS-era constant 1.0 — every one of them outranked every
    // fused hit, so the response claimed "BM25 + sqlite-vec fused" while the
    // semantic results sat at the bottom of the list. When fusion ran, the
    // surviving FTS-only lanes are re-scored onto the same RRF scale (by
    // their rank within the lane, exactly how the fused lanes scored their
    // FTS halves), so the cross-lane ordering is meaningful.
    if fused_any {
        rescore_fts_only_lanes_to_rrf_scale(&mut response.hits, k);
    }
    response.hits.extend(fused);
    response.hits.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    response.total = response.hits.len();
    if fused_any {
        response.used_semantic = true;
        response.mode_note = Some(format!(
            "BM25 + sqlite-vec fused (RRF k={k}, embeddings local)"
        ));
    }
    Ok(response)
}

/// Clean-audit item 59: put the FTS-only lanes on the RRF scale the fused
/// lanes score on — `1/(k + rank + 1)` by rank within each lane, the same
/// formula the fusion loop applied to the FTS halves of the fused lanes.
/// Only the lanes that never fused are touched (the fused lanes were already
/// retained out of `hits` by the caller).
fn rescore_fts_only_lanes_to_rrf_scale(hits: &mut [SearchHit], k: u32) {
    const FTS_ONLY_LANES: [&str; 4] = ["customers", "issues", "saved_replies", "ai"];
    let mut rank_by_lane: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for hit in hits.iter_mut() {
        if FTS_ONLY_LANES.contains(&hit.scope.as_str()) {
            let rank = rank_by_lane.entry(hit.scope.as_str()).or_insert(0);
            hit.score = 1.0 / (f64::from(k) + *rank as f64 + 1.0);
            *rank += 1;
        }
    }
}

pub fn universal_search(conn: &Connection, query: &str) -> Result<Vec<SearchResult>> {
    let query = cap_query(query);
    if query.trim().is_empty() {
        return Ok(Vec::new());
    }
    let response = search(conn, query, "all", &SearchFilters::default())?;
    Ok(response
        .hits
        .into_iter()
        .map(|h| SearchResult {
            resource_type: h.scope,
            remote_id: h.id,
            title: h.title,
            snippet: h.snippet,
        })
        .collect())
}

fn cap_query(query: &str) -> &str {
    if query.len() <= MAX_SEARCH_QUERY_LENGTH {
        return query;
    }
    let mut end = MAX_SEARCH_QUERY_LENGTH;
    while !query.is_char_boundary(end) {
        end -= 1;
    }
    &query[..end]
}

pub fn index_conversation_fts(conn: &Connection, local_id: i64) -> Result<()> {
    conn.execute(
        "DELETE FROM fts_conversations WHERE conversation_id = ?1",
        params![local_id],
    )?;
    let row = conn
        .query_row(
            "SELECT c.id, c.number, c.subject, c.preview,
               TRIM(COALESCE(cu.first_name, '') || ' ' || COALESCE(cu.last_name, '')) AS customer,
               (SELECT GROUP_CONCAT(t.name) FROM conversation_tags ct
                  JOIN tags t ON t.id = ct.tag_id WHERE ct.conversation_id = c.id) AS tags
             FROM conversations c LEFT JOIN customers cu ON cu.id = c.customer_local_id
            WHERE c.id = ?1",
            params![local_id],
            |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, Option<String>>(2)?,
                    r.get::<_, Option<String>>(3)?,
                    r.get::<_, Option<String>>(4)?,
                    r.get::<_, Option<String>>(5)?,
                ))
            },
        )
        .optional()?;
    if let Some((id, number, subject, preview, customer, tags)) = row {
        conn.execute(
            "INSERT INTO fts_conversations (subject, preview, customer, tags, numbers, conversation_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                subject.unwrap_or_default(),
                preview.unwrap_or_default(),
                customer.unwrap_or_default(),
                tags.unwrap_or_default(),
                number.to_string(),
                id
            ],
        )?;
        conn.execute(
            "DELETE FROM fts_threads WHERE conversation_id = ?1",
            params![local_id],
        )?;
        if table_exists(conn, "conversation_threads")? {
            conn.execute(
                "INSERT INTO fts_threads (body, thread_id, conversation_id)
                 SELECT t.body_text, t.id, t.conversation_id FROM conversation_threads t
                  WHERE t.conversation_id = ?1 AND t.body_text IS NOT NULL AND LENGTH(t.body_text) > 0
                    AND t.deleted_at IS NULL",
                params![local_id],
            )?;
        }
    }
    Ok(())
}

pub fn index_thread_fts(conn: &Connection, thread_local_id: i64) -> Result<()> {
    conn.execute(
        "DELETE FROM fts_threads WHERE thread_id = ?1",
        params![thread_local_id],
    )?;
    let row = conn
        .query_row(
            "SELECT body_text, conversation_id FROM conversation_threads WHERE id = ?1",
            params![thread_local_id],
            |r| Ok((r.get::<_, Option<String>>(0)?, r.get::<_, i64>(1)?)),
        )
        .optional()?;
    let mut flag: Option<i64> = None;
    if let Some((Some(body), conversation_id)) = row {
        if !body.trim().is_empty() {
            conn.execute(
                "INSERT INTO fts_threads (body, thread_id, conversation_id) VALUES (?1, ?2, ?3)",
                params![body, thread_local_id, conversation_id],
            )?;
            flag = Some(1);
        } else {
            flag = Some(0);
        }
    }
    if let Some(flag) = flag
        && column_exists(conn, "conversation_threads", "fts_indexed")?
    {
        conn.execute(
            "UPDATE conversation_threads SET fts_indexed = ?1 WHERE id = ?2",
            params![flag, thread_local_id],
        )?;
    }
    Ok(())
}

pub fn index_knowledge_chunk_fts(conn: &Connection, chunk_id: i64) -> Result<()> {
    conn.execute(
        "DELETE FROM fts_knowledge WHERE chunk_id = ?1",
        params![chunk_id],
    )?;
    let row = conn
        .query_row(
            "SELECT c.content, c.document_id, d.title, d.visibility
               FROM knowledge_chunks c JOIN knowledge_documents d ON d.id = c.document_id
              WHERE c.id = ?1",
            params![chunk_id],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                ))
            },
        )
        .optional()?;
    if let Some((content, document_id, title, visibility)) = row {
        conn.execute(
            "INSERT INTO fts_knowledge (title, content, chunk_id, document_id, visibility)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![title, content, chunk_id, document_id, visibility],
        )?;
    }
    conn.execute(
        "UPDATE knowledge_chunks SET fts_indexed = 1 WHERE id = ?1",
        params![chunk_id],
    )?;
    Ok(())
}

pub fn index_known_issue_fts(conn: &Connection, known_issue_id: i64) -> Result<()> {
    conn.execute(
        "DELETE FROM fts_known_issues WHERE known_issue_id = ?1",
        params![known_issue_id],
    )?;
    let row = conn
        .query_row(
            "SELECT COALESCE(title, ''), COALESCE(symptoms, ''), COALESCE(workaround, ''),
                    COALESCE(customer_safe_explanation, '')
               FROM known_issues WHERE id = ?1",
            params![known_issue_id],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                ))
            },
        )
        .optional()?;
    if let Some((title, symptoms, workaround, explanation)) = row {
        conn.execute(
            "INSERT INTO fts_known_issues (title, symptoms, workaround, customer_safe_explanation, known_issue_id)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![title, symptoms, workaround, explanation, known_issue_id],
        )?;
    }
    Ok(())
}

pub fn index_saved_reply_fts(conn: &Connection, saved_reply_id: i64) -> Result<()> {
    conn.execute(
        "DELETE FROM fts_saved_replies WHERE saved_reply_id = ?1",
        params![saved_reply_id],
    )?;
    let row = conn
        .query_row(
            "SELECT name, COALESCE(preview, ''), COALESCE(text, '') FROM saved_replies WHERE id = ?1",
            params![saved_reply_id],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            },
        )
        .optional()?;
    if let Some((name, preview, text)) = row {
        conn.execute(
            "INSERT INTO fts_saved_replies (saved_reply_id, name, preview, text) VALUES (?1, ?2, ?3, ?4)",
            params![saved_reply_id, name, preview, text],
        )?;
    }
    Ok(())
}

pub fn index_ai_analysis_fts(
    conn: &Connection,
    conversation_id: i64,
    run_id: i64,
    summary: Option<&str>,
    primary_question: Option<&str>,
    intent: Option<&str>,
) -> Result<()> {
    conn.execute(
        "DELETE FROM fts_ai_analyses WHERE conversation_id = ?1 AND run_id = ?2",
        params![conversation_id, run_id],
    )?;
    conn.execute(
        "INSERT INTO fts_ai_analyses (summary, primary_question, intent, conversation_id, run_id)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            summary.unwrap_or(""),
            primary_question.unwrap_or(""),
            intent.unwrap_or(""),
            conversation_id,
            run_id
        ],
    )?;
    Ok(())
}

pub fn rebuild_indexes(conn: &Connection) -> Result<usize> {
    apply_fts_migration(conn)?;
    conn.execute_batch("BEGIN")?;
    let result = rebuild_indexes_tx(conn);
    match result {
        Ok(n) => {
            conn.execute_batch("COMMIT")?;
            Ok(n)
        }
        Err(e) => {
            let _ = conn.execute_batch("ROLLBACK");
            Err(e)
        }
    }
}

fn rebuild_indexes_tx(conn: &Connection) -> Result<usize> {
    for table in [
        "fts_conversations",
        "fts_threads",
        "fts_knowledge",
        "fts_known_issues",
        "fts_saved_replies",
        "fts_ai_analyses",
        "docs_fts",
        "fts_custom_objects",
        "conversations_fts",
        "customers_fts",
    ] {
        conn.execute(&format!("DELETE FROM {table}"), [])?;
    }

    let conv_ids: Vec<i64> = {
        let mut stmt = conn.prepare("SELECT id FROM conversations WHERE deleted_at IS NULL")?;

        stmt.query_map([], |r| r.get(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?
    };
    for id in &conv_ids {
        index_conversation_fts(conn, *id)?;
    }
    let conv_count = conv_ids.len();

    // Clean-audit item 64: this bulk insert used to run right after the
    // per-conversation loop above — but `index_conversation_fts` already
    // inserts every non-deleted conversation's threads into `fts_threads`,
    // so every live thread was inserted TWICE per rebuild (FTS5 corpus
    // statistics doubled; matched threads double-counted). The loop is the
    // single writer now; the flag bookkeeping below stays as it was.
    if table_exists(conn, "conversation_threads")? {
        conn.execute(
            "UPDATE conversation_threads SET fts_indexed = 1
              WHERE body_text IS NOT NULL AND LENGTH(body_text) > 0",
            [],
        )?;
    }

    conn.execute(
        "INSERT INTO fts_knowledge (title, content, chunk_id, document_id, visibility)
         SELECT d.title, c.content, c.id, c.document_id, d.visibility
           FROM knowledge_chunks c JOIN knowledge_documents d ON d.id = c.document_id",
        [],
    )?;
    conn.execute("UPDATE knowledge_chunks SET fts_indexed = 1", [])?;

    if table_exists(conn, "known_issues")? {
        conn.execute(
            "INSERT INTO fts_known_issues (title, symptoms, workaround, customer_safe_explanation, known_issue_id)
             SELECT COALESCE(title, ''), COALESCE(symptoms, ''), COALESCE(workaround, ''),
                    COALESCE(customer_safe_explanation, ''), id
               FROM known_issues",
            [],
        )?;
    }

    if table_exists(conn, "saved_replies")? {
        conn.execute(
            "INSERT INTO fts_saved_replies (saved_reply_id, name, preview, text)
             SELECT id, name, COALESCE(preview, ''), COALESCE(text, '')
               FROM saved_replies WHERE deleted_at IS NULL",
            [],
        )?;
    }

    conn.execute(
        "INSERT INTO fts_ai_analyses (summary, primary_question, intent, conversation_id, run_id)
         SELECT COALESCE(MAX(CASE WHEN key = 'summary' THEN value END), ''),
                COALESCE(MAX(CASE WHEN key = 'primary_question' THEN value END), ''),
                COALESCE(MAX(CASE WHEN key = 'intent' THEN value END), ''),
                conversation_id, run_id
           FROM ai_extracted_facts
          WHERE key IN ('summary', 'primary_question', 'intent') AND conversation_id IS NOT NULL
          GROUP BY conversation_id, run_id",
        [],
    )?;

    if table_exists(conn, "docs_articles")? {
        conn.execute(
            "INSERT INTO docs_fts (name, text, article_id)
             SELECT name, COALESCE(text, ''), id FROM docs_articles WHERE deleted_at IS NULL",
            [],
        )?;
    }

    if table_exists(conn, "custom_objects")? {
        rebuild_custom_objects_fts(conn)?;
    }

    conn.execute(
        "INSERT INTO conversations_fts (remote_id, subject, preview)
         SELECT remote_id, COALESCE(subject, ''), COALESCE(preview, '')
           FROM conversations WHERE deleted_at IS NULL",
        [],
    )?;
    conn.execute(
        "INSERT INTO customers_fts (remote_id, first_name, last_name, email, organization)
         SELECT remote_id, COALESCE(first_name, ''), COALESCE(last_name, ''), COALESCE(email, ''), COALESCE(organization, '')
           FROM customers WHERE deleted_at IS NULL",
        [],
    )?;

    if table_exists(conn, "application_settings")? {
        conn.execute(
            "INSERT OR REPLACE INTO application_settings (key, value) VALUES ('fts_version', '3')",
            [],
        )?;
    }
    Ok(conv_count)
}

fn rebuild_custom_objects_fts(conn: &Connection) -> Result<()> {
    let rows: Vec<(i64, String, Option<String>)> = {
        let mut stmt = conn.prepare(
            "SELECT o.id, o.title, o.data_json
               FROM custom_objects o
               JOIN custom_object_types t ON t.id = o.type_id
              WHERE o.deleted_at IS NULL",
        )?;

        stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .collect::<rusqlite::Result<Vec<_>>>()?
    };
    for (id, title, data_json) in rows {
        let search_text = build_custom_object_search_text(&title, data_json.as_deref());
        conn.execute(
            "INSERT INTO fts_custom_objects (title, search_text, object_id) VALUES (?1, ?2, ?3)",
            params![title, search_text, id],
        )?;
    }
    Ok(())
}

fn build_custom_object_search_text(title: &str, data_json: Option<&str>) -> String {
    let mut parts = vec![title.to_string()];
    if let Some(json) = data_json
        && let Ok(serde_json::Value::Object(map)) = serde_json::from_str(json)
    {
        for value in map.values() {
            match value {
                serde_json::Value::String(s) => parts.push(s.clone()),
                serde_json::Value::Number(n) => parts.push(n.to_string()),
                serde_json::Value::Bool(b) => parts.push(b.to_string()),
                _ => {}
            }
        }
    }
    let joined = parts.join(" ");
    if joined.chars().count() > 5000 {
        joined.chars().take(5000).collect()
    } else {
        joined
    }
}

pub fn index_conversation(
    conn: &Connection,
    remote_id: i64,
    subject: &str,
    preview: &str,
) -> Result<()> {
    conn.execute(
        "DELETE FROM conversations_fts WHERE remote_id = ?1",
        params![remote_id],
    )?;
    conn.execute(
        "INSERT INTO conversations_fts (remote_id, subject, preview) VALUES (?1, ?2, ?3)",
        params![remote_id, subject, preview],
    )?;
    if let Ok(local_id) = conn.query_row(
        "SELECT id FROM conversations WHERE remote_id = ?1",
        params![remote_id],
        |r| r.get::<_, i64>(0),
    ) {
        index_conversation_fts(conn, local_id)?;
    }
    Ok(())
}

pub fn index_customer(
    conn: &Connection,
    remote_id: i64,
    first_name: &str,
    last_name: &str,
    email: &str,
    organization: &str,
) -> Result<()> {
    conn.execute(
        "DELETE FROM customers_fts WHERE remote_id = ?1",
        params![remote_id],
    )?;
    conn.execute(
        "INSERT INTO customers_fts (remote_id, first_name, last_name, email, organization)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![remote_id, first_name, last_name, email, organization],
    )?;
    Ok(())
}

#[cfg(test)]
mod batch13_a4_search_tests {
    use super::*;
    use rusqlite::Connection;

    fn db_with_conversation() -> Connection {
        let mut conn = Connection::open_in_memory().expect("db");
        crate::bootstrap::apply_all(&mut conn).expect("migrations");
        conn.execute(
            "INSERT INTO conversations (remote_id, number, subject, preview, status)
             VALUES (9601, 96, 'Export timeout errors', 'The export throws errors.', 'active')",
            [],
        )
        .expect("conversation");
        apply_fts_migration(&conn).expect("fts");
        rebuild_indexes(&conn).expect("index");
        conn
    }

    /// A4: without an index the fused search degrades to the honest FTS-only
    /// response — used_semantic false, a mode note explaining why, and the
    /// FTS hits intact.
    #[test]
    fn fused_search_degrades_honestly_without_index() {
        let conn = db_with_conversation();
        let response =
            search_with_semantics(&conn, None, "export", "all", &SearchFilters::default())
                .expect("search");
        assert!(!response.used_semantic, "no vector lane ran");
        assert!(
            response.mode_note.is_some(),
            "the fallback is explained, not silent"
        );
        assert!(
            !response.hits.is_empty(),
            "the FTS lane still finds the conversation"
        );
        assert!(
            response
                .hits
                .iter()
                .all(|h| h.why.contains(&"keyword match".to_string())),
            "lane provenance is reported"
        );
    }

    /// A4: a disabled index is also an honest FTS-only answer (the
    /// qdrant_enabled setting off).
    #[test]
    fn fused_search_with_disabled_index_stays_fts_only() {
        let conn = db_with_conversation();
        let dir = std::env::temp_dir().join(format!("spp-search-test-{}", std::process::id()));
        let index = crate::vectorstore_sqlitevec::VectorIndex::new(&dir, false);
        let response = search_with_semantics(
            &conn,
            Some(&index),
            "export",
            "all",
            &SearchFilters::default(),
        )
        .expect("search");
        assert!(!response.used_semantic, "a disabled index never fuses");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// Clean-audit batch 25: item 58 (the knowledge FTS lane never deduped per
/// document — one multi-chunk document flooded the LIMIT), item 59 (fused
/// RRF scores were sorted against FTS-only constant-1.0 scores), item 64
/// (rebuild_indexes double-inserted every thread into fts_threads).
#[cfg(test)]
mod batch25_search_tests {
    use super::*;
    use rusqlite::Connection;

    fn db() -> Connection {
        let mut conn = Connection::open_in_memory().expect("db");
        crate::bootstrap::apply_all(&mut conn).expect("migrations");
        apply_fts_migration(&conn).expect("fts");
        conn
    }

    /// One knowledge source; documents get explicit chunk rows in both
    /// knowledge_chunks and fts_knowledge exactly the way the ingest lane
    /// writes them.
    fn seed_knowledge(conn: &Connection, docs: &[(&str, &[&str])]) {
        conn.execute(
            "INSERT INTO knowledge_sources (name, kind) VALUES ('src', 'local_file')",
            [],
        )
        .expect("source");
        let source_id = conn.last_insert_rowid();
        for (title, chunks) in docs {
            conn.execute(
                "INSERT INTO knowledge_documents (source_id, title, visibility, content)
                 VALUES (?1, ?2, 'internal_only', ?3)",
                params![source_id, title, chunks.join("\n\n")],
            )
            .expect("document");
            let document_id = conn.last_insert_rowid();
            for (index, content) in chunks.iter().enumerate() {
                conn.execute(
                    "INSERT INTO knowledge_chunks (document_id, chunk_index, content, chunk_version)
                     VALUES (?1, ?2, ?3, 2)",
                    params![document_id, index as i64, content],
                )
                .expect("chunk");
                let chunk_id = conn.last_insert_rowid();
                conn.execute(
                    "INSERT INTO fts_knowledge (title, content, chunk_id, document_id, visibility)
                     VALUES (?1, ?2, ?3, ?4, 'internal_only')",
                    params![title, content, chunk_id, document_id],
                )
                .expect("fts row");
            }
        }
    }

    /// Item 58: a document split into several matching chunks must occupy
    /// ONE result slot (its best chunk), not flood the limit with copies of
    /// itself while other matching documents are pushed out entirely.
    #[test]
    fn audit58_knowledge_hits_dedupe_per_document() {
        let conn = db();
        seed_knowledge(
            &conn,
            &[
                // A 6-chunk document, every chunk mentioning "invoice".
                (
                    "Invoice manual",
                    &[
                        "invoice one",
                        "invoice two",
                        "invoice three",
                        "invoice four",
                        "invoice five",
                        "invoice six",
                    ],
                ),
                // Two single-chunk documents that also match "invoice".
                ("Invoice FAQ", &["the invoice cycle"]),
                ("Invoice policy", &["pay the invoice in 30 days"]),
            ],
        );
        let hits =
            search_knowledge_raw(&conn, "invoice", None, 3, "and").expect("knowledge search");
        assert_eq!(
            hits.len(),
            3,
            "three DISTINCT documents fill the limit of three: {hits:?}"
        );
        let mut document_ids: Vec<i64> = hits.iter().map(|h| h.document_id).collect();
        document_ids.sort();
        document_ids.dedup();
        assert_eq!(
            document_ids.len(),
            3,
            "every hit is a different document: {hits:?}"
        );
        // Pre-fix: the 6-chunk manual would have consumed the whole LIMIT 3
        // by itself, and the FAQ/policy documents would be invisible.
        assert!(
            hits.iter().any(|h| h.title == "Invoice FAQ")
                && hits.iter().any(|h| h.title == "Invoice policy"),
            "single-chunk documents are not crowded out: {hits:?}"
        );
    }

    /// Item 58, visibility filter: the dedupe respects the filter.
    #[test]
    fn audit58_dedupe_respects_the_visibility_filter() {
        let conn = db();
        conn.execute(
            "INSERT INTO knowledge_sources (name, kind) VALUES ('src', 'local_file')",
            [],
        )
        .expect("source");
        let source_id = conn.last_insert_rowid();
        for (title, visibility) in [
            ("Safe doc", "customer_safe"),
            ("Internal doc", "internal_only"),
        ] {
            conn.execute(
                "INSERT INTO knowledge_documents (source_id, title, visibility, content)
                 VALUES (?1, ?2, ?3, 'billing content')",
                params![source_id, title, visibility],
            )
            .expect("document");
            let document_id = conn.last_insert_rowid();
            conn.execute(
                "INSERT INTO knowledge_chunks (document_id, chunk_index, content, chunk_version)
                 VALUES (?1, 0, 'billing content', 2)",
                params![document_id],
            )
            .expect("chunk");
            let chunk_id = conn.last_insert_rowid();
            conn.execute(
                "INSERT INTO fts_knowledge (title, content, chunk_id, document_id, visibility)
                 VALUES (?1, 'billing content', ?2, ?3, ?4)",
                params![title, chunk_id, document_id, visibility],
            )
            .expect("fts row");
        }
        let hits = search_knowledge_raw(&conn, "billing", Some("customer_safe"), 5, "and")
            .expect("search");
        assert_eq!(hits.len(), 1, "only the customer-safe document matches");
        assert_eq!(hits[0].title, "Safe doc");
        assert_eq!(hits[0].visibility, "customer_safe");
    }

    fn hit(scope: &str, id: i64, score: f64) -> SearchHit {
        SearchHit {
            scope: scope.into(),
            id,
            title: format!("{scope} {id}"),
            subtitle: String::new(),
            snippet: String::new(),
            score,
            href: String::new(),
            why: vec!["keyword match".into()],
        }
    }

    /// Item 59: after a fusion ran, the surviving FTS-only lanes are
    /// re-scored onto the RRF scale — the pre-fix constant 1.0 outranked
    /// every fused hit (RRF scores peak around 0.033) and pushed the
    /// "semantic" results to the bottom of a response that claimed fusion.
    #[test]
    fn audit59_fts_only_lanes_rescore_to_the_rrf_scale() {
        let k = crate::hybrid_search::RRF_K;
        let mut hits = vec![
            hit("customers", 1, 1.0),
            hit("customers", 2, 1.0),
            hit("issues", 7, 1.0),
            hit("saved_replies", 9, 1.0),
        ];
        rescore_fts_only_lanes_to_rrf_scale(&mut hits, k);
        let expected = |rank: usize| 1.0 / (f64::from(k) + rank as f64 + 1.0);
        assert!(
            (hits[0].score - expected(0)).abs() < 1e-12,
            "customers rank 0: {}",
            hits[0].score
        );
        assert!(
            (hits[1].score - expected(1)).abs() < 1e-12,
            "customers rank 1: {}",
            hits[1].score
        );
        assert!(
            (hits[2].score - expected(0)).abs() < 1e-12,
            "issues rank 0: {}",
            hits[2].score
        );
        assert!(
            (hits[3].score - expected(0)).abs() < 1e-12,
            "saved_replies rank 0: {}",
            hits[3].score
        );
        // A hit that appeared on BOTH fused lists (1/61 + 1/62 ≈ 0.0325)
        // now outranks any single-list FTS-only hit (1/61 ≈ 0.0164) —
        // before the fix the FTS-only 1.0 beat everything fused.
        let fused_both = 1.0 / (f64::from(k) + 1.0) + 1.0 / (f64::from(k) + 2.0);
        let mut all = hits.clone();
        all.push(hit("tickets", 42, fused_both));
        all.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        assert_eq!(
            all[0].id, 42,
            "the hit found by BOTH lanes ranks first on the shared scale"
        );
    }

    /// Item 59: lanes that never fuse are untouched when no fusion ran
    /// (the caller gates the rescore on `fused_any` — this pins the
    /// helper's own lane list).
    #[test]
    fn audit59_rescore_leaves_unknown_lanes_alone() {
        let k = crate::hybrid_search::RRF_K;
        let mut hits = vec![hit("tickets", 5, 0.5), hit("customers", 1, 1.0)];
        rescore_fts_only_lanes_to_rrf_scale(&mut hits, k);
        assert_eq!(
            hits[0].score, 0.5,
            "the tickets lane (fused lanes' scope) is not re-scored by the helper"
        );
        let expected = 1.0 / (f64::from(k) + 1.0);
        assert!(
            (hits[1].score - expected).abs() < 1e-12,
            "the customers lane IS re-scored onto the RRF scale: {}",
            hits[1].score
        );
    }

    /// Item 64: a rebuild inserts every live thread exactly once — the
    /// pre-fix rebuild ran the per-conversation indexer (which writes each
    /// conversation's threads) AND a bulk INSERT over all threads, so every
    /// thread landed in fts_threads twice.
    #[test]
    fn audit64_rebuild_inserts_each_thread_exactly_once() {
        let conn = db();
        conn.execute(
            "INSERT INTO conversations (remote_id, number, subject, preview, status)
             VALUES (7001, 70, 'Exports', 'preview', 'active')",
            [],
        )
        .expect("conversation");
        let conversation_id = conn.last_insert_rowid();
        for body in [
            "first thread body",
            "second thread body",
            "third thread body",
        ] {
            conn.execute(
                "INSERT INTO conversation_threads
                    (conversation_id, remote_id, type, body_text, created_at)
                 VALUES (?1, NULL, 'customer', ?2, '2026-10-01 10:00:00')",
                params![conversation_id, body],
            )
            .expect("thread");
        }
        rebuild_indexes(&conn).expect("rebuild");
        let fts_rows: i64 = conn
            .query_row("SELECT COUNT(*) FROM fts_threads", [], |r| r.get(0))
            .expect("count");
        assert_eq!(
            fts_rows, 3,
            "three live threads, three fts rows — pre-fix this was 6"
        );
        let distinct_thread_ids: i64 = conn
            .query_row(
                "SELECT COUNT(DISTINCT thread_id) FROM fts_threads",
                [],
                |r| r.get(0),
            )
            .expect("distinct");
        assert_eq!(distinct_thread_ids, 3, "no thread id appears twice");
        // The threads still match after the rebuild.
        let found = search_conversations(&conn, "second thread", &SearchFilters::default(), 10)
            .expect("search");
        assert!(
            found.iter().any(|h| h.id == conversation_id),
            "the rebuilt index still finds the thread text"
        );
    }
}
