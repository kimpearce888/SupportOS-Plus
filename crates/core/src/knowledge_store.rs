use std::io::Read as _;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, params};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::error::{Error, Result};

pub const IMPORT_DIR_NAME: &str = "knowledge-import";

/// Hard cap on a single imported file, checked before the file is read into
/// memory (migration-map row 4a). 64 MiB is ~10x a large text-heavy document.
pub const MAX_IMPORT_FILE_BYTES: u64 = 64 * 1024 * 1024;
/// Hard cap on PDF input handed to the text extractor — the PDF parser is the
/// most CPU-expensive path in the importer and must not be fed arbitrary
/// multi-hundred-MiB payloads.
pub const MAX_PDF_INPUT_BYTES: u64 = 32 * 1024 * 1024;
/// Hard cap on a single decompressed DOCX zip entry (`word/document.xml`),
/// checked against both the declared and the actually-read size.
pub const MAX_DOCX_ENTRY_BYTES: u64 = 64 * 1024 * 1024;

#[must_use]
pub fn chunk_text(text: &str, max_len: usize, overlap: usize) -> Vec<String> {
    let clean: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if clean.len() <= max_len {
        return if clean.is_empty() {
            Vec::new()
        } else {
            vec![clean]
        };
    }
    let bytes = clean.as_bytes();
    let mut chunks: Vec<String> = Vec::new();
    let mut start = 0usize;
    while start < bytes.len() {
        let mut end = (start + max_len).min(bytes.len());
        if end < bytes.len() {
            let window = &clean[start..end];
            if let Some(dot) = window.rfind(". ")
                && dot > max_len / 2
            {
                end = start + dot + 1;
            }
        }
        while end < bytes.len() && !clean.is_char_boundary(end) {
            end += 1;
        }
        chunks.push(clean[start..end].trim().to_string());
        if end >= bytes.len() {
            break;
        }
        start = end.saturating_sub(overlap);
        // Backing up `overlap` bytes from a boundary-checked `end` can
        // land inside a multibyte UTF-8 sequence; walking forward to the
        // next boundary keeps every slice below panic-free.
        while start < bytes.len() && !clean.is_char_boundary(start) {
            start += 1;
        }
    }
    chunks
}

#[must_use]
pub fn checksum(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    crate::crypto::hex(&hasher.finalize())
}

pub fn get_or_create_source(
    conn: &Connection,
    name: &str,
    kind: &str,
    visibility: &str,
) -> Result<i64> {
    let existing: Option<i64> = conn
        .query_row(
            "SELECT id FROM knowledge_sources WHERE name = ?1",
            params![name],
            |r| r.get(0),
        )
        .map(Some)
        .or_else(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => Ok(None),
            other => Err(other),
        })?;
    if let Some(id) = existing {
        return Ok(id);
    }
    conn.execute(
        "INSERT INTO knowledge_sources (name, kind, visibility) VALUES (?1, ?2, ?3)",
        params![name, kind, visibility],
    )?;
    Ok(conn.last_insert_rowid())
}

#[must_use]
pub fn list_sources(conn: &Connection) -> Vec<Value> {
    conn.prepare(
        "SELECT s.id, s.name, s.kind, s.visibility, s.created_at,
           (SELECT COUNT(*) FROM knowledge_documents d WHERE d.source_id = s.id) AS document_count
         FROM knowledge_sources s ORDER BY s.name",
    )
    .and_then(|mut stmt| {
        let rows = stmt.query_map([], |r| {
            Ok(json!({
                "id": r.get::<_, i64>(0)?,
                "name": r.get::<_, String>(1)?,
                "kind": r.get::<_, String>(2)?,
                "visibility": r.get::<_, String>(3)?,
                "created_at": r.get::<_, String>(4)?,
                "document_count": r.get::<_, i64>(5)?,
            }))
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
    })
    .unwrap_or_default()
}

#[must_use]
pub fn list_documents(conn: &Connection, source_id: Option<i64>) -> Vec<Value> {
    let sql = "SELECT d.id, d.source_id, d.title, d.visibility, d.version, d.content,
                d.format, d.created_at, d.updated_at,
                (SELECT COUNT(*) FROM knowledge_chunks c WHERE c.document_id = d.id) AS chunk_count
              FROM knowledge_documents d
              WHERE (?1 IS NULL OR d.source_id = ?1)
              ORDER BY d.title";
    conn.prepare(sql)
        .and_then(|mut stmt| {
            let rows = stmt.query_map(params![source_id], |r| {
                let content: Option<String> = r.get(5)?;
                let preview: String = content
                    .unwrap_or_default()
                    .chars()
                    .take(200)
                    .collect::<String>()
                    .trim()
                    .to_string();
                Ok(json!({
                    "id": r.get::<_, i64>(0)?,
                    "source_id": r.get::<_, i64>(1)?,
                    "title": r.get::<_, String>(2)?,
                    "visibility": r.get::<_, String>(3)?,
                    "version": r.get::<_, Option<i64>>(4)?.unwrap_or(1),
                    "content_preview": preview,
                    "format": r.get::<_, Option<String>>(6)?,
                    "created_at": r.get::<_, String>(7)?,
                    "updated_at": r.get::<_, String>(8)?,
                    "chunk_count": r.get::<_, i64>(9)?,
                }))
            })?;
            rows.collect::<rusqlite::Result<Vec<_>>>()
        })
        .unwrap_or_default()
}

#[must_use]
pub fn get_document(conn: &Connection, id: i64) -> Option<Value> {
    conn.query_row(
        "SELECT d.id, d.source_id, d.title, d.visibility, d.version, d.content, d.format,
                d.created_at, d.updated_at, d.last_reviewed_at, d.last_verified_at,
                s.name AS source_name,
                (SELECT COUNT(*) FROM knowledge_chunks c WHERE c.document_id = d.id) AS chunk_count
         FROM knowledge_documents d JOIN knowledge_sources s ON s.id = d.source_id
         WHERE d.id = ?1",
        params![id],
        |r| {
            let content: Option<String> = r.get(5)?;
            let preview: String = content
                .clone()
                .unwrap_or_default()
                .chars()
                .take(200)
                .collect::<String>()
                .trim()
                .to_string();
            Ok(json!({
                "id": r.get::<_, i64>(0)?,
                "source_id": r.get::<_, i64>(1)?,
                "title": r.get::<_, String>(2)?,
                "visibility": r.get::<_, String>(3)?,
                "version": r.get::<_, Option<i64>>(4)?.unwrap_or(1),
                "content": content.unwrap_or_default(),
                "content_preview": preview,
                "format": r.get::<_, Option<String>>(6)?,
                "created_at": r.get::<_, String>(7)?,
                "updated_at": r.get::<_, String>(8)?,
                "last_reviewed_at": r.get::<_, Option<String>>(9)?,
                "last_verified_at": r.get::<_, Option<String>>(10)?,
                "source_name": r.get::<_, String>(11)?,
                "chunk_count": r.get::<_, i64>(12)?,
            }))
        },
    )
    .ok()
}

pub fn upsert_document(
    conn: &Connection,
    source_id: i64,
    title: &str,
    content: &str,
    visibility: &str,
    format: &str,
) -> Result<(i64, bool)> {
    let checksum = checksum(content);
    let existing: Option<(i64, Option<String>)> = conn
        .query_row(
            "SELECT id, checksum FROM knowledge_documents WHERE source_id = ?1 AND title = ?2",
            params![source_id, title],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map(Some)
        .or_else(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => Ok(None),
            other => Err(other),
        })?;
    if let Some((id, existing_checksum)) = &existing
        && existing_checksum.as_deref() == Some(checksum.as_str())
    {
        return Ok((*id, false));
    }

    let chunks = chunk_text(content, 1200, 150);
    conn.execute_batch("BEGIN")?;
    let outcome = (|| -> Result<i64> {
        let doc_id = match existing {
            Some((id, _)) => {
                conn.execute(
                    "UPDATE knowledge_documents
                     SET version = version + 1, checksum = ?1, content = ?2, format = ?3,
                         visibility = ?4, updated_at = datetime('now')
                     WHERE id = ?5",
                    params![checksum, content, format, visibility, id],
                )?;
                id
            }
            None => {
                conn.execute(
                    "INSERT INTO knowledge_documents (source_id, title, visibility, version, checksum, content, format)
                     VALUES (?1, ?2, ?3, 1, ?4, ?5, ?6)",
                    params![source_id, title, visibility, checksum, content, format],
                )?;
                conn.last_insert_rowid()
            }
        };
        conn.execute(
            "DELETE FROM knowledge_chunks WHERE document_id = ?1",
            params![doc_id],
        )?;
        conn.execute(
            "DELETE FROM fts_knowledge WHERE document_id = ?1",
            params![doc_id],
        )?;
        for (index, chunk) in chunks.iter().enumerate() {
            conn.execute(
                "INSERT INTO knowledge_chunks (document_id, chunk_index, content, chunk_version)
                 VALUES (?1, ?2, ?3, 2)",
                params![doc_id, index as i64, chunk],
            )?;
            let chunk_id = conn.last_insert_rowid();
            conn.execute(
                "INSERT INTO fts_knowledge (title, content, chunk_id, document_id, visibility)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![title, chunk, chunk_id, doc_id, visibility],
            )?;
        }
        conn.execute(
            "UPDATE knowledge_documents SET last_indexed_at = datetime('now') WHERE id = ?1",
            params![doc_id],
        )?;
        Ok(doc_id)
    })();
    match outcome {
        Ok(id) => {
            conn.execute_batch("COMMIT")?;
            Ok((id, true))
        }
        Err(e) => {
            let _ = conn.execute_batch("ROLLBACK");
            Err(e)
        }
    }
}

pub fn delete_document(conn: &Connection, id: i64) -> Result<bool> {
    conn.execute_batch("BEGIN")?;
    let outcome = (|| -> Result<bool> {
        conn.execute(
            "DELETE FROM knowledge_chunks WHERE document_id = ?1",
            params![id],
        )?;
        conn.execute(
            "DELETE FROM fts_knowledge WHERE document_id = ?1",
            params![id],
        )?;
        let removed = conn.execute("DELETE FROM knowledge_documents WHERE id = ?1", params![id])?;
        Ok(removed > 0)
    })();
    match outcome {
        Ok(removed) => {
            conn.execute_batch("COMMIT")?;
            Ok(removed)
        }
        Err(e) => {
            let _ = conn.execute_batch("ROLLBACK");
            Err(e)
        }
    }
}

#[must_use]
pub fn search_knowledge(conn: &Connection, query: &str, visibility: Option<&str>) -> Vec<Value> {
    let fts_query: String = query
        .replace(['"', '*', '(', ')'], " ")
        .split_whitespace()
        .take(8)
        .map(|t| format!("\"{t}\"*"))
        .collect::<Vec<_>>()
        .join(" ");
    if fts_query.is_empty() {
        return Vec::new();
    }
    let sql = match visibility {
        Some(_) => {
            "SELECT f.chunk_id, f.document_id, f.title,
                    snippet(fts_knowledge, 1, '[', ']', '…', 12) AS snippet, f.visibility
             FROM fts_knowledge f WHERE fts_knowledge MATCH ?1 AND f.visibility = ?2
             ORDER BY rank LIMIT 25"
        }
        None => {
            "SELECT f.chunk_id, f.document_id, f.title,
                    snippet(fts_knowledge, 1, '[', ']', '…', 12) AS snippet, f.visibility
             FROM fts_knowledge f WHERE fts_knowledge MATCH ?1
             ORDER BY rank LIMIT 25"
        }
    };
    conn.prepare(sql)
        .and_then(|mut stmt| {
            let rows = if visibility.is_some() {
                stmt.query_map(params![fts_query, visibility], fts_row)?
            } else {
                stmt.query_map(params![fts_query], fts_row)?
            };
            rows.collect::<rusqlite::Result<Vec<_>>>()
        })
        .unwrap_or_default()
}

fn fts_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Value> {
    Ok(json!({
        "chunk_id": r.get::<_, i64>(0)?,
        "document_id": r.get::<_, i64>(1)?,
        "title": r.get::<_, String>(2)?,
        "snippet": r.get::<_, String>(3)?,
        "visibility": r.get::<_, String>(4)?,
    }))
}

pub fn mark_reviewed(conn: &Connection, id: i64) -> Result<bool> {
    let rows = conn.execute(
        "UPDATE knowledge_documents SET last_reviewed_at = datetime('now') WHERE id = ?1",
        params![id],
    )?;
    Ok(rows > 0)
}

pub fn mark_verified(conn: &Connection, id: i64) -> Result<bool> {
    let rows = conn.execute(
        "UPDATE knowledge_documents SET last_verified_at = datetime('now') WHERE id = ?1",
        params![id],
    )?;
    Ok(rows > 0)
}

#[must_use]
pub fn freshness_report(conn: &Connection) -> Value {
    let stale_days = crate::settings::get_int(conn, "freshness_stale_days", 180).unwrap_or(180);
    let unreviewed_days =
        crate::settings::get_int(conn, "freshness_unreviewed_days", 90).unwrap_or(90);
    let low_usage_days =
        crate::settings::get_int(conn, "freshness_low_usage_days", 60).unwrap_or(60);
    let has_ai_evidence: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='ai_evidence'",
            [],
            |r| r.get::<_, i64>(0),
        )
        .map(|n| n > 0)
        .unwrap_or(false);
    let usage_subquery = if has_ai_evidence {
        "(SELECT COUNT(DISTINCT r.conversation_id) FROM ai_evidence r WHERE r.document_id = d.id)"
    } else {
        "0"
    };
    let sql = format!(
        "SELECT d.id, d.title, d.visibility, d.version, d.updated_at,
                d.last_reviewed_at, d.last_verified_at, s.name AS source_name,
                d.created_at,
                (SELECT COUNT(*) FROM knowledge_chunks c WHERE c.document_id = d.id) AS chunk_count,
                {usage_subquery} AS usage_count
         FROM knowledge_documents d JOIN knowledge_sources s ON s.id = d.source_id
         ORDER BY d.updated_at DESC LIMIT 200"
    );
    let rows: Vec<Value> = conn
        .prepare(&sql)
        .and_then(|mut stmt| {
            let rows = stmt.query_map([], |r| {
                let updated_at: Option<String> = r.get(4)?;
                let reviewed_at: Option<String> = r.get(5)?;
                let verified_at: Option<String> = r.get(6)?;
                let created_at: Option<String> = r.get(7)?;
                let usage_count: i64 = r.get::<_, i64>(10).unwrap_or(0);
                let days_since_update = updated_at.as_deref().and_then(days_since);
                let days_since_review = reviewed_at.as_deref().and_then(days_since);
                let stale = days_since_update.is_some_and(|d| d > stale_days);
                let unreviewed_long = match days_since_review {
                    None => days_since_update.is_some_and(|d| d > unreviewed_days),
                    Some(d) => d > unreviewed_days,
                };
                let unverified = verified_at.is_none();
                // Audit item 123: `freshness_low_usage_days` was read and
                // dropped — the low_usage flag fired for every unused doc,
                // including one created five minutes ago. The setting is
                // the grace period now: a doc counts as low-usage once it
                // has existed at least `low_usage_days` (falling back to
                // the update stamp when created_at is missing) and still
                // has no recorded usage.
                let age_days = created_at
                    .as_deref()
                    .and_then(days_since)
                    .or(days_since_update);
                let low_usage = usage_count == 0 && age_days.is_some_and(|d| d >= low_usage_days);
                let conflicting = false;
                let recurring_questions = usage_count > 5;
                Ok(json!({
                    "id": r.get::<_, i64>(0)?,
                    "document_id": r.get::<_, i64>(0)?,
                    "title": r.get::<_, String>(1)?,
                    "visibility": r.get::<_, String>(2)?,
                    "version": r.get::<_, Option<i64>>(3)?.unwrap_or(1),
                    "updated_at": updated_at,
                    "last_reviewed_at": reviewed_at,
                    "last_verified_at": verified_at,
                    "source_name": r.get::<_, Option<String>>(7)?,
                    "chunk_count": r.get::<_, i64>(9)?,
                    "usage_count": usage_count,
                    "days_since_update": days_since_update,
                    "days_since_review": days_since_review,
                    "freshness_status": if stale { "stale" } else { "fresh" },
                    "flags": {
                        "stale": stale,
                        "unreviewed_long": unreviewed_long,
                        "unverified": unverified,
                        "low_usage": low_usage,
                        "conflicting": conflicting,
                        "recurring_questions": recurring_questions,
                    },
                }))
            })?;
            rows.collect::<rusqlite::Result<Vec<_>>>()
        })
        .unwrap_or_default();
    let stale = rows
        .iter()
        .filter(|r| r.get("freshness_status").and_then(|v| v.as_str()) == Some("stale"))
        .count() as i64;
    let fresh = rows.len() as i64 - stale;
    // K3: freshness scores every source — the help-center mirror
    // (`docs_articles`) is scored with the same staleness threshold so a
    // published-but-rotting article is as visible as a stale local doc.
    let help_center: Vec<Value> = conn
        .prepare(
            "SELECT a.id, a.name, a.status, a.remote_updated_at, c.name
               FROM docs_articles a
               LEFT JOIN docs_collections c ON c.id = a.collection_local_id
              ORDER BY a.remote_updated_at DESC LIMIT 200",
        )
        .and_then(|mut stmt| {
            stmt.query_map([], |r| {
                let updated: Option<String> = r.get(3)?;
                let days_old = updated.as_deref().and_then(days_since);
                let stale = days_old.is_some_and(|d| d > stale_days);
                Ok(json!({
                    "id": r.get::<_, i64>(0)?,
                    "title": r.get::<_, String>(1)?,
                    "status": r.get::<_, Option<String>>(2)?,
                    "updated_at": updated,
                    "collection": r.get::<_, Option<String>>(4)?,
                    "days_since_update": days_old,
                    "freshness_status": if stale { "stale" } else { "fresh" },
                }))
            })
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
        })
        .unwrap_or_default();
    let help_center_stale = help_center
        .iter()
        .filter(|r| r.get("freshness_status").and_then(|v| v.as_str()) == Some("stale"))
        .count() as i64;
    json!({
        "stale": stale,
        "fresh": fresh,
        "documents": rows,
        "help_center": {
            "stale": help_center_stale,
            "total": help_center.len() as i64,
            "articles": help_center,
        },
        "thresholds": {
            "stale_days": stale_days,
            "unreviewed_days": unreviewed_days,
            "low_usage_days": low_usage_days,
        }
    })
}

fn days_since(stamp: &str) -> Option<i64> {
    let naive =
        chrono::NaiveDateTime::parse_from_str(stamp.trim_end_matches('Z'), "%Y-%m-%d %H:%M:%S")
            .ok()
            .or_else(|| {
                chrono::NaiveDateTime::parse_from_str(
                    stamp.trim_end_matches('Z'),
                    "%Y-%m-%dT%H:%M:%S%.f",
                )
                .ok()
            })?;
    let delta = chrono::Utc::now().naive_utc() - naive;
    Some(delta.num_days())
}

#[must_use]
pub fn related_ticket_estimate(conn: &Connection, id: i64) -> i64 {
    conn.query_row(
        "SELECT COUNT(DISTINCT r.conversation_id) AS n
           FROM ai_sources s JOIN ai_runs r ON r.id = s.run_id
          WHERE s.source_type = 'knowledge_document' AND s.source_id = ?1
            AND r.conversation_id IS NOT NULL",
        params![id],
        |r| r.get(0),
    )
    .unwrap_or(0)
}

#[must_use]
pub fn search_known_issues_by_title(conn: &Connection, title: &str) -> Vec<Value> {
    let fts_query: String = title
        .replace(['"', '*', '(', ')'], " ")
        .split_whitespace()
        .take(8)
        .map(|t| format!("\"{t}\"*"))
        .collect::<Vec<_>>()
        .join(" ");
    if !fts_query.is_empty() {
        let rows: Vec<Value> = conn
            .prepare(
                "SELECT ki.id, COALESCE(ki.title, ki.name) AS title,
                        snippet(fts_known_issues, 0, '[', ']', '…', 12) AS snippet
                 FROM fts_known_issues f JOIN known_issues ki ON ki.id = f.known_issue_id
                 WHERE fts_known_issues MATCH ?1 ORDER BY rank LIMIT 5",
            )
            .and_then(|mut stmt| {
                let rows = stmt.query_map(params![fts_query], |r| {
                    Ok(json!({
                        "id": r.get::<_, i64>(0)?,
                        "title": r.get::<_, Option<String>>(1)?,
                        "snippet": r.get::<_, Option<String>>(2)?,
                    }))
                })?;
                rows.collect::<rusqlite::Result<Vec<_>>>()
            })
            .unwrap_or_default();
        if !rows.is_empty() {
            return rows;
        }
    }
    let like = format!(
        "%{}%",
        title
            .split_whitespace()
            .next()
            .unwrap_or("**** no such issue ****")
    );
    conn.prepare(
        "SELECT id, COALESCE(title, name) AS title, NULL AS snippet
         FROM known_issues WHERE COALESCE(title, name) LIKE ?1 LIMIT 5",
    )
    .and_then(|mut stmt| {
        let rows = stmt.query_map(params![like], |r| {
            Ok(json!({
                "id": r.get::<_, i64>(0)?,
                "title": r.get::<_, Option<String>>(1)?,
                "snippet": r.get::<_, Option<String>>(2)?,
            }))
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
    })
    .unwrap_or_default()
}

#[must_use]
pub fn import_result(document_id: i64, title: &str, changed: bool) -> Value {
    json!({ "documentId": document_id, "title": title, "changed": changed })
}

pub fn import_manual(
    conn: &Connection,
    source_name: &str,
    documents: &[Value],
    visibility: &str,
) -> Result<Vec<Value>> {
    let source_id = get_or_create_source(conn, source_name, "manual", visibility)?;
    let mut out = Vec::with_capacity(documents.len());
    for doc in documents {
        let title = doc
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        let content = doc
            .get("content")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        let format = doc
            .get("format")
            .and_then(|v| v.as_str())
            .unwrap_or("markdown");
        let (id, changed) = upsert_document(conn, source_id, title, content, visibility, format)?;
        out.push(import_result(id, title, changed));
    }
    Ok(out)
}

#[must_use]
pub fn importable_files(data_dir: &Path) -> Value {
    let dir = data_dir.join(IMPORT_DIR_NAME);
    let mut files: Vec<String> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if !is_importable_filename(&name) {
                continue;
            }
            if entry.metadata().map(|m| m.is_file()).unwrap_or(false) {
                files.push(name);
            }
        }
    }
    files.sort();
    json!({
        "dir": dir.to_string_lossy(),
        "files": files,
    })
}

fn is_importable_filename(name: &str) -> bool {
    let lower = name.to_lowercase();
    [
        ".md",
        ".markdown",
        ".txt",
        ".csv",
        ".json",
        ".html",
        ".htm",
        ".pdf",
        ".docx",
    ]
    .iter()
    .any(|ext| lower.ends_with(ext))
}

fn lexically_resolve(base: &Path, requested: &Path) -> PathBuf {
    let joined = if requested.is_absolute() {
        requested.to_path_buf()
    } else {
        base.join(requested)
    };
    let mut out = PathBuf::new();
    for component in joined.components() {
        match component {
            std::path::Component::ParentDir => {
                out.pop();
            }
            std::path::Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

pub fn import_file(
    conn: &Connection,
    data_dir: &Path,
    path: &str,
    source_name: Option<&str>,
    visibility: &str,
) -> Result<Vec<Value>> {
    let requested = PathBuf::from(path);
    let import_root = data_dir.join(IMPORT_DIR_NAME);
    let lexical = lexically_resolve(&import_root, &requested);
    let safety_message = "For safety, file imports must live inside the \"knowledge-import\" folder of the data directory. Create it and copy your documents there.";
    if !lexical.starts_with(&import_root) || lexical == import_root {
        return Err(Error::Validation(safety_message.to_string()));
    }
    let canonical = std::fs::canonicalize(&lexical)
        .map_err(|_| Error::Validation(format!("File not found: {path}")))?;
    let root_canonical =
        std::fs::canonicalize(&import_root).unwrap_or_else(|_| import_root.clone());
    if !canonical.starts_with(&root_canonical) || canonical == root_canonical {
        return Err(Error::Validation(safety_message.to_string()));
    }

    let metadata = std::fs::metadata(&canonical)
        .map_err(|_| Error::Validation(format!("File not found: {path}")))?;
    if metadata.is_dir() {
        let mut files: Vec<PathBuf> = std::fs::read_dir(&canonical)
            .map_err(|e| Error::Validation(format!("Import failed: {e}")))?
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                p.file_name()
                    .map(|n| is_importable_filename(&n.to_string_lossy()))
                    .unwrap_or(false)
            })
            .collect();
        files.sort();
        files.truncate(200);
        let mut out = Vec::new();
        for file in files {
            let name = file
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "imported".to_string());
            out.extend(import_one_file(
                conn,
                &file,
                source_name.unwrap_or(&name),
                visibility,
            )?);
        }
        return Ok(out);
    }
    let fallback_name = canonical
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "imported".to_string());
    import_one_file(
        conn,
        &canonical,
        source_name.unwrap_or(&fallback_name),
        visibility,
    )
}

fn import_one_file(
    conn: &Connection,
    abs: &Path,
    source_name: &str,
    visibility: &str,
) -> Result<Vec<Value>> {
    let ext = abs
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let base = abs
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "imported".to_string());
    // Size cap before the read (row 4a): the file is fully buffered in
    // memory, so an oversized import must fail before allocation, not after.
    let file_len = std::fs::metadata(abs)
        .map_err(|e| Error::Validation(format!("Import failed: {e}")))?
        .len();
    if file_len > MAX_IMPORT_FILE_BYTES {
        return Err(Error::Validation(format!(
            "File is too large to import: {file_len} bytes \
             (the limit is {MAX_IMPORT_FILE_BYTES} bytes)."
        )));
    }
    let raw = std::fs::read(abs).map_err(|e| Error::Validation(format!("Import failed: {e}")))?;
    let text = String::from_utf8_lossy(&raw).to_string();

    let docs: Vec<(String, String, String)> = match ext.as_str() {
        "md" | "markdown" => vec![(base, text, "markdown".to_string())],
        "txt" => vec![(base, text, "txt".to_string())],
        "csv" => split_csv_sections(&base, &text),
        "json" => parse_json_documents(&base, &text)?,
        "html" | "htm" => {
            let title = extract_html_title(&text).unwrap_or_else(|| base.clone());
            let body = html_to_text_keep_structure(&text);
            vec![(title, body, "html".to_string())]
        }
        "pdf" => {
            let text = sandbox_parse("pdf", raw, parse_pdf_text)?;
            if text.trim().is_empty() {
                return Err(Error::Validation(
                    "No text could be extracted from this PDF (it may be scanned images)."
                        .to_string(),
                ));
            }
            vec![(base, text, "txt".to_string())]
        }
        "docx" => {
            let text = sandbox_parse("docx", raw, parse_docx_text)?;
            vec![(base, text, "txt".to_string())]
        }
        other => {
            return Err(Error::Validation(format!(
                "Unsupported file type: .{other}"
            )));
        }
    };

    let source_id = get_or_create_source(conn, source_name, "local_file", visibility)?;
    let mut out = Vec::with_capacity(docs.len());
    for (title, content, format) in docs {
        let (id, changed) =
            upsert_document(conn, source_id, &title, &content, visibility, &format)?;
        out.push(import_result(id, &title, changed));
    }
    Ok(out)
}

/// Route an untrusted-document parse through the sandbox when compiled
/// with `--features sandbox` (Linux): Landlock deny-all + seccomp
/// allow-list on a dedicated thread (see [`crate::sandbox`]). The bytes
/// are moved in — the sandboxed parser must not touch the filesystem.
/// Without the feature the parse runs inline, exactly as before.
fn sandbox_parse(
    label: &'static str,
    raw: Vec<u8>,
    parse: fn(&[u8]) -> Result<String>,
) -> Result<String> {
    #[cfg(all(target_os = "linux", feature = "sandbox"))]
    {
        crate::sandbox::run_sandboxed(label, move || parse(&raw))?
    }
    #[cfg(not(all(target_os = "linux", feature = "sandbox")))]
    {
        let _ = label;
        parse(&raw)
    }
}

fn parse_pdf_text(raw: &[u8]) -> Result<String> {
    // Input cap before the extractor runs (row 4a).
    if u64::try_from(raw.len()).unwrap_or(u64::MAX) > MAX_PDF_INPUT_BYTES {
        return Err(Error::Validation(format!(
            "PDF input is too large to parse: {} bytes \
             (the limit is {MAX_PDF_INPUT_BYTES} bytes).",
            raw.len()
        )));
    }
    pdf_extract::extract_text_from_mem(raw).map_err(|_| {
        Error::Validation(
            "PDF parsing is unavailable or failed for this file. Supported best with text-based PDFs."
                .to_string(),
        )
    })
}

fn parse_docx_text(raw: &[u8]) -> Result<String> {
    let docx_unavailable =
        || Error::Validation("DOCX parsing is unavailable or failed for this file.".to_string());
    let mut archive =
        zip::ZipArchive::new(std::io::Cursor::new(raw)).map_err(|_| docx_unavailable())?;
    let mut xml = String::new();
    {
        let document = archive
            .by_name("word/document.xml")
            .map_err(|_| docx_unavailable())?;
        // Declared uncompressed size is checked first (row 4a) ...
        if document.size() > MAX_DOCX_ENTRY_BYTES {
            return Err(Error::Validation(format!(
                "DOCX document entry is too large: {} bytes \
                 (the limit is {MAX_DOCX_ENTRY_BYTES} bytes).",
                document.size()
            )));
        }
        // ... but zip declarations can lie, so the read itself is bounded by
        // take() and the produced length re-checked afterwards.
        let mut limited = document.take(MAX_DOCX_ENTRY_BYTES + 1);
        limited
            .read_to_string(&mut xml)
            .map_err(|_| docx_unavailable())?;
        if xml.len() as u64 > MAX_DOCX_ENTRY_BYTES {
            return Err(Error::Validation(format!(
                "DOCX document entry expanded beyond the {MAX_DOCX_ENTRY_BYTES}-byte limit."
            )));
        }
    }
    let mut out = String::new();
    let mut in_text = false;
    let mut reader = quick_xml::Reader::from_str(&xml);
    reader.config_mut().trim_text(false);
    loop {
        use quick_xml::events::Event;
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                if e.name().0 == "w:t" {
                    in_text = true;
                }
            }
            Ok(Event::Text(t)) => {
                if in_text {
                    out.push_str(&t.xml10_content());
                }
            }
            Ok(Event::GeneralRef(r)) => {
                if in_text {
                    let decoded: Option<char> = match r.as_ref() {
                        "amp" => Some('&'),
                        "lt" => Some('<'),
                        "gt" => Some('>'),
                        "quot" => Some('"'),
                        "apos" => Some('\''),
                        numeric => {
                            let n = numeric
                                .strip_prefix("#x")
                                .and_then(|h| u32::from_str_radix(h, 16).ok())
                                .or_else(|| {
                                    numeric
                                        .strip_prefix('#')
                                        .and_then(|d| d.parse::<u32>().ok())
                                });
                            n.and_then(char::from_u32)
                        }
                    };
                    if let Some(c) = decoded {
                        out.push(c);
                    }
                }
            }
            Ok(Event::End(e)) => match e.name().0 {
                "w:t" => in_text = false,
                "w:p" => out.push('\n'),
                _ => {}
            },
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(_) => return Err(docx_unavailable()),
        }
    }
    Ok(out)
}

fn split_csv_sections(base: &str, text: &str) -> Vec<(String, String, String)> {
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    if lines.is_empty() {
        return vec![(base.to_string(), String::new(), "txt".to_string())];
    }
    let header = parse_csv_line(lines[0]);
    let title_idx = header.iter().position(|h| {
        matches!(
            h.trim().to_lowercase().as_str(),
            "title" | "name" | "question" | "topic"
        )
    });
    let mut docs: Vec<(String, String, String)> = Vec::new();
    for line in lines.iter().take(501).skip(1) {
        let cells = parse_csv_line(line);
        if cells.iter().all(|c| c.trim().is_empty()) {
            continue;
        }
        let title = match title_idx {
            Some(idx) => cells.get(idx).map(String::as_str).unwrap_or(""),
            None => cells.first().map(String::as_str).unwrap_or(""),
        };
        let title: String = if title.trim().is_empty() {
            format!("{base} entry")
        } else {
            title.trim().chars().take(200).collect()
        };
        let content = header
            .iter()
            .enumerate()
            .map(|(i, h)| format!("{h}: {}", cells.get(i).map(String::as_str).unwrap_or("")))
            .collect::<Vec<_>>()
            .join("\n");
        docs.push((title, content, "txt".to_string()));
    }
    if docs.is_empty() {
        docs.push((base.to_string(), text.to_string(), "txt".to_string()));
    }
    docs
}

fn parse_csv_line(line: &str) -> Vec<String> {
    let mut cells: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut in_quotes = false;
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let ch = chars[i];
        if in_quotes {
            if ch == '"' {
                if chars.get(i + 1) == Some(&'"') {
                    cur.push('"');
                    i += 1;
                } else {
                    in_quotes = false;
                }
            } else {
                cur.push(ch);
            }
        } else if ch == '"' {
            in_quotes = true;
        } else if ch == ',' {
            cells.push(cur.clone());
            cur.clear();
        } else {
            cur.push(ch);
        }
        i += 1;
    }
    cells.push(cur);
    cells
}

fn parse_json_documents(base: &str, text: &str) -> Result<Vec<(String, String, String)>> {
    let data: Value = serde_json::from_str(text)
        .map_err(|e| Error::Validation(format!("Import failed: invalid JSON: {e}")))?;
    let mut docs: Vec<(String, String, String)> = Vec::new();
    match &data {
        Value::Array(items) => {
            for (i, item) in items.iter().take(500).enumerate() {
                let title = item
                    .get("title")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
                    .or_else(|| {
                        item.get("name")
                            .and_then(|v| v.as_str())
                            .filter(|s| !s.is_empty())
                    })
                    .map(str::to_string)
                    .unwrap_or_else(|| format!("{base} [{}]", i + 1));
                docs.push((title, flatten_record(item), "txt".to_string()));
            }
        }
        Value::Object(obj) => {
            if let Some(Value::Array(items)) = obj.get("documents") {
                for item in items.iter().take(500) {
                    let title = item
                        .get("title")
                        .or_else(|| item.get("name"))
                        .and_then(|v| v.as_str())
                        .map(str::to_string)
                        .unwrap_or_else(|| base.to_string());
                    let content = item
                        .get("content")
                        .or_else(|| item.get("text"))
                        .and_then(|v| v.as_str())
                        .map(str::to_string)
                        .unwrap_or_else(|| flatten_record(item));
                    let format = item
                        .get("format")
                        .and_then(|v| v.as_str())
                        .unwrap_or("txt")
                        .to_string();
                    docs.push((title, content, format));
                }
            } else {
                let title = obj
                    .get("title")
                    .and_then(|v| v.as_str())
                    .map(str::to_string)
                    .unwrap_or_else(|| base.to_string());
                docs.push((title, flatten_record(&data), "txt".to_string()));
            }
        }
        _ => {
            return Err(Error::Validation(
                "Import failed: JSON must be an array or object.".to_string(),
            ));
        }
    }
    Ok(docs)
}

fn flatten_record(obj: &Value) -> String {
    let mut lines: Vec<String> = Vec::new();
    if let Some(map) = obj.as_object() {
        for (k, v) in map {
            match v {
                Value::Null => {}
                Value::String(s) => lines.push(format!("{k}: {s}")),
                Value::Number(n) => lines.push(format!("{k}: {n}")),
                Value::Bool(b) => lines.push(format!("{k}: {b}")),
                other => lines.push(format!("{k}: {other}")),
            }
        }
    }
    lines.join("\n")
}

#[must_use]
pub fn html_to_text_keep_structure(html: &str) -> String {
    let mut out = html.to_string();
    out = strip_tag_block(&out, "style");
    out = strip_tag_block(&out, "script");
    for tag in ["p", "div", "li", "h1", "h2", "h3", "h4", "h5", "h6", "tr"] {
        out = replace_ci(&out, &format!("</{tag}>"), "\n");
    }
    out = replace_ci(&out, "<br>", "\n");
    out = replace_ci(&out, "<br/>", "\n");
    out = replace_ci(&out, "<br />", "\n");
    let mut text = String::with_capacity(out.len());
    let mut inside = false;
    for ch in out.chars() {
        match ch {
            '<' => {
                inside = true;
                text.push(' ');
            }
            '>' => inside = false,
            _ if !inside => text.push(ch),
            _ => {}
        }
    }
    // Audit item 120: the decode order double-decoded — `&amp;` ran
    // FIRST, so an author's literal `&amp;lt;` became `&lt;` and then
    // decoded again into `<`; `&apos;` never decoded at all. Single-decode
    // semantics decode every other entity first and `&amp;` LAST, so
    // `&amp;lt;` lands as the literal text `&lt;`.
    for (entity, decoded) in [
        ("&nbsp;", " "),
        ("&lt;", "<"),
        ("&gt;", ">"),
        ("&quot;", "\""),
        ("&#39;", "'"),
        ("&apos;", "'"),
        ("&amp;", "&"),
    ] {
        text = text.replace(entity, decoded);
    }
    let mut collapsed = String::with_capacity(text.len());
    let mut last_space = false;
    for ch in text.chars() {
        let is_ws = ch == ' ' || ch == '\t';
        if is_ws {
            if !last_space {
                collapsed.push(' ');
            }
            last_space = true;
        } else {
            collapsed.push(ch);
            last_space = false;
        }
    }
    while collapsed.contains("\n\n\n") {
        collapsed = collapsed.replace("\n\n\n", "\n\n");
    }
    collapsed.trim().to_string()
}

fn strip_tag_block(input: &str, tag: &str) -> String {
    // Audit item 121: the haystack was lowercased with Unicode rules and
    // then used as an OFFSET MAP over the original — but some characters
    // (İ U+0130) expand to two chars when lowercased, so the offsets in
    // `lower` stop matching `input` and the slicing panics or mis-slices.
    // ASCII lowercasing is byte-for-byte 1:1, so the offsets stay valid;
    // the tags searched here are ASCII anyway.
    let lower = input.to_ascii_lowercase();
    let open = format!("<{tag}");
    let close = format!("</{tag}>");
    let mut out = String::with_capacity(input.len());
    let mut pos = 0usize;
    while let Some(start) = lower[pos..].find(&open) {
        let abs_start = pos + start;
        out.push_str(&input[pos..abs_start]);
        match lower[abs_start..].find(&close) {
            Some(rel_end) => {
                pos = abs_start + rel_end + close.len();
            }
            None => {
                return out;
            }
        }
    }
    out.push_str(&input[pos..]);
    out
}

fn replace_ci(input: &str, from: &str, to: &str) -> String {
    // Audit item 121: same offset-map hazard as strip_tag_block — the
    // Unicode-lowercased haystack's byte offsets were applied to the
    // original string (an İ anywhere before a match mis-sliced or
    // panicked). ASCII lowercasing keeps the offsets 1:1.
    let lower = input.to_ascii_lowercase();
    let from_lower = from.to_ascii_lowercase();
    let mut out = String::with_capacity(input.len());
    let mut pos = 0usize;
    while let Some(idx) = lower[pos..].find(&from_lower) {
        let abs = pos + idx;
        out.push_str(&input[pos..abs]);
        out.push_str(to);
        pos = abs + from.len();
    }
    out.push_str(&input[pos..]);
    out
}

fn extract_html_title(html: &str) -> Option<String> {
    // Audit item 121: the same offset-map hazard as the sibling helpers —
    // ASCII lowercasing keeps the offsets 1:1 with the original.
    let lower = html.to_ascii_lowercase();
    let start = lower.find("<title>")? + "<title>".len();
    let end = lower[start..].find("</title>")? + start;
    let title = html[start..end].trim();
    if title.is_empty() {
        None
    } else {
        Some(title.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as _;

    fn fresh_conn() -> Connection {
        let mut conn = Connection::open_in_memory().expect("in-memory connection");
        crate::bootstrap::apply_all(&mut conn).expect("migrations");
        conn
    }

    fn import_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "spp-import-test-{tag}-{}-{}",
            std::process::id(),
            chrono::Utc::now().timestamp_millis()
        ));
        std::fs::create_dir_all(dir.join(IMPORT_DIR_NAME)).expect("import root");
        dir
    }

    #[test]
    fn import_rejects_oversized_files_before_reading() {
        let conn = fresh_conn();
        let data_dir = import_dir("file-cap");
        let file = data_dir.join(IMPORT_DIR_NAME).join("huge.txt");
        // One byte over the cap; written as a single sparse-ish write.
        let payload = vec![b'a'; MAX_IMPORT_FILE_BYTES as usize + 1];
        std::fs::write(&file, &payload).expect("write huge file");

        let err = import_file(&conn, &data_dir, "huge.txt", None, "internal")
            .expect_err("oversized import must be rejected");
        assert!(err.to_string().contains("too large to import"), "{err}");

        // Under the cap (tiny file) the same pipeline succeeds.
        let ok_file = data_dir.join(IMPORT_DIR_NAME).join("small.txt");
        std::fs::write(&ok_file, b"hello world").expect("write small file");
        let out = import_file(&conn, &data_dir, "small.txt", None, "internal")
            .expect("small import works");
        assert_eq!(out.len(), 1);
    }

    #[test]
    fn pdf_input_over_the_cap_is_rejected_before_parsing() {
        let junk = vec![b'%'; MAX_PDF_INPUT_BYTES as usize + 1];
        let err = parse_pdf_text(&junk).expect_err("oversized PDF input must be rejected");
        // The size guard fires before the extractor: the message names the
        // input cap, not a generic parse failure.
        assert!(err.to_string().contains("too large to parse"), "{err}");

        // Small junk still reaches the parser and fails there instead.
        let err = parse_pdf_text(b"%not-a-real-pdf").expect_err("junk pdf fails");
        assert!(!err.to_string().contains("too large"), "{err}");
    }

    fn build_docx(xml: &str) -> Vec<u8> {
        let mut buf = std::io::Cursor::new(Vec::new());
        let mut writer = zip::ZipWriter::new(&mut buf);
        let options: zip::write::SimpleFileOptions = Default::default();
        writer
            .start_file("word/document.xml", options)
            .expect("start entry");
        writer.write_all(xml.as_bytes()).expect("write entry");
        writer.finish().expect("finish zip");
        buf.into_inner()
    }

    #[test]
    fn docx_text_extraction_still_works_small_entries() {
        // Regression guard for the take() refactor: a normal document parses.
        let xml =
            "<w:document><w:body><w:p><w:r><w:t>hello docx</w:t></w:r></w:p></w:body></w:document>";
        let text = parse_docx_text(&build_docx(xml)).expect("parse small docx");
        assert!(text.contains("hello docx"), "{text}");
    }

    #[test]
    fn docx_declared_entry_size_is_capped() {
        // The declared uncompressed size of word/document.xml exceeds the
        // cap, so the import is refused before the entry is decompressed.
        // (Deflate squeezes 64 MiB of spaces down to a tiny zip.)
        let xml = " ".repeat(MAX_DOCX_ENTRY_BYTES as usize + 1);
        let zip_bytes = build_docx(&xml);
        let err = parse_docx_text(&zip_bytes).expect_err("oversized docx entry must be rejected");
        assert!(err.to_string().contains("too large"), "{err}");
    }

    #[cfg(all(target_os = "linux", feature = "sandbox"))]
    #[test]
    fn docx_and_pdf_parse_inside_the_sandbox() {
        // End-to-end through the same routing the import path uses: the
        // zip container + deflate + quick-xml (docx) and the pdf error
        // path must all work under Landlock deny-all + the seccomp
        // allow-list.
        let xml = "<w:document><w:body><w:p><w:r><w:t>hello sandbox</w:t></w:r></w:p></w:body></w:document>";
        let text =
            sandbox_parse("docx", build_docx(xml), parse_docx_text).expect("sandboxed docx parse");
        assert!(text.contains("hello sandbox"), "{text}");

        // Junk PDF: the parse fails the same way it does unsandboxed —
        // a parser error, not a sandbox/restriction error.
        let err = sandbox_parse("pdf", vec![b'%'; 64], parse_pdf_text)
            .expect_err("junk pdf fails inside the sandbox too");
        assert!(
            err.to_string()
                .contains("PDF parsing is unavailable or failed"),
            "{err}"
        );
    }
}

// ---------------------------------------------------------------------------
// K2: the AI article-draft review queue
// ---------------------------------------------------------------------------

/// Migration m056 (boot step 47): `article_drafts` — the K2 review queue.
/// Every AI-drafted knowledge article lands here as `drafted` and moves to
/// `approved` or `rejected` only by a human decision; the K3 publish path
/// accepts approved drafts exclusively. Nothing in this table is ever
/// visible to customers without that human click.
pub fn apply_m056_article_drafts(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS article_drafts (
            id                       INTEGER PRIMARY KEY AUTOINCREMENT,
            candidate_id             INTEGER REFERENCES knowledge_gap_candidates(id) ON DELETE SET NULL,
            title                    TEXT NOT NULL,
            body                     TEXT NOT NULL,
            citations                TEXT NOT NULL DEFAULT '[]',
            ai_run_id                INTEGER REFERENCES ai_runs(id),
            status                   TEXT NOT NULL DEFAULT 'drafted'
                     CHECK (status IN ('drafted','approved','rejected')),
            created_at               TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
            decided_at               TEXT,
            decided_by_user_local_id INTEGER,
            -- K3 publish state machine: unpublished -> draft_upserted ->
            -- published -> verified; publish_failed keeps the last error.
            publish_state            TEXT NOT NULL DEFAULT 'unpublished'
                     CHECK (publish_state IN ('unpublished','draft_upserted','published','verified','publish_failed')),
            remote_article_id        INTEGER,
            collection_local_id      INTEGER,
            published_at             TEXT,
            publish_error            TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_article_drafts_status
            ON article_drafts (status, created_at DESC);
        CREATE INDEX IF NOT EXISTS idx_article_drafts_candidate
            ON article_drafts (candidate_id);",
    )?;
    Ok(())
}

/// A review-queue row as the UI renders it.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ArticleDraftRow {
    pub id: i64,
    pub candidate_id: Option<i64>,
    pub title: String,
    pub body: String,
    pub citations: Value,
    pub ai_run_id: Option<i64>,
    pub status: String,
    pub created_at: String,
    pub decided_at: Option<String>,
    pub publish_state: String,
    pub remote_article_id: Option<i64>,
    pub collection_local_id: Option<i64>,
    pub published_at: Option<String>,
    pub publish_error: Option<String>,
}

fn article_draft_from_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<ArticleDraftRow> {
    let citations_raw: String = r.get(4)?;
    Ok(ArticleDraftRow {
        id: r.get(0)?,
        candidate_id: r.get(1)?,
        title: r.get(2)?,
        body: r.get(3)?,
        citations: serde_json::from_str(&citations_raw).unwrap_or_else(|_| json!([])),
        ai_run_id: r.get(5)?,
        status: r.get(6)?,
        created_at: r.get(7)?,
        decided_at: r.get(8)?,
        publish_state: r.get(9)?,
        remote_article_id: r.get(10)?,
        collection_local_id: r.get(11)?,
        published_at: r.get(12)?,
        publish_error: r.get(13)?,
    })
}

const ARTICLE_DRAFT_COLUMNS: &str = "id, candidate_id, title, body, citations, ai_run_id,
    status, created_at, decided_at, publish_state, remote_article_id,
    collection_local_id, published_at, publish_error";

/// Insert a freshly AI-drafted article. Called only by the AI pipeline
/// after a successful model run; the row starts `drafted`.
pub fn insert_article_draft(
    conn: &Connection,
    candidate_id: Option<i64>,
    title: &str,
    body: &str,
    citations: &Value,
    ai_run_id: Option<i64>,
) -> Result<i64> {
    conn.execute(
        "INSERT INTO article_drafts
             (candidate_id, title, body, citations, ai_run_id, status)
         VALUES (?1, ?2, ?3, ?4, ?5, 'drafted')",
        params![
            candidate_id,
            title,
            body,
            serde_json::to_string(citations).unwrap_or_else(|_| "[]".into()),
            ai_run_id
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

/// The review queue: newest first, optionally narrowed to one status.
pub fn list_article_drafts(conn: &Connection, status: Option<&str>) -> Vec<ArticleDraftRow> {
    let sql = format!(
        "SELECT {ARTICLE_DRAFT_COLUMNS} FROM article_drafts
         WHERE (?1 IS NULL OR status = ?1)
         ORDER BY id DESC LIMIT 200"
    );
    conn.prepare(&sql)
        .and_then(|mut stmt| {
            stmt.query_map(params![status], article_draft_from_row)
                .map(|rows| rows.filter_map(|r| r.ok()).collect())
        })
        .unwrap_or_default()
}

#[must_use]
pub fn article_draft_by_id(conn: &Connection, id: i64) -> Option<ArticleDraftRow> {
    let sql = format!("SELECT {ARTICLE_DRAFT_COLUMNS} FROM article_drafts WHERE id = ?1");
    conn.query_row(&sql, params![id], |r| article_draft_from_row(r).map(Some))
        .unwrap_or(None)
}

/// The human decision — the only way out of the queue. A draft that was
/// already decided cannot be re-decided; a publish in flight
/// (`draft_upserted`/`published`/`verified`) cannot be un-approved from
/// under the publisher.
pub fn decide_article_draft(
    conn: &Connection,
    id: i64,
    decision: &str,
    user_local_id: Option<i64>,
) -> Option<ArticleDraftRow> {
    let ok_decision = matches!(decision, "approved" | "rejected");
    if !ok_decision {
        return None;
    }
    let changed = conn
        .execute(
            "UPDATE article_drafts
             SET status = ?2, decided_at = strftime('%Y-%m-%dT%H:%M:%fZ','now'),
                 decided_by_user_local_id = ?3
             WHERE id = ?1 AND status = 'drafted'
               AND publish_state = 'unpublished'",
            params![id, decision, user_local_id],
        )
        .unwrap_or(0);
    if changed == 0 {
        return None;
    }
    article_draft_by_id(conn, id)
}

/// K3: record the publish state machine transitions. Only the worker
/// driving the Docs API calls this.
pub fn set_article_draft_publish_state(
    conn: &Connection,
    id: i64,
    publish_state: &str,
    remote_article_id: Option<i64>,
    collection_local_id: Option<i64>,
    publish_error: Option<&str>,
) -> Result<()> {
    conn.execute(
        "UPDATE article_drafts
         SET publish_state = ?2,
             remote_article_id = COALESCE(?3, remote_article_id),
             collection_local_id = COALESCE(?4, collection_local_id),
             published_at = CASE WHEN ?2 IN ('published','verified')
                                THEN strftime('%Y-%m-%dT%H:%M:%fZ','now')
                                ELSE published_at END,
             publish_error = ?5
         WHERE id = ?1",
        params![
            id,
            publish_state,
            remote_article_id,
            collection_local_id,
            publish_error
        ],
    )?;
    Ok(())
}

/// True when an approved draft exists for the candidate (the queue refuses
/// duplicates so a repeated "draft article" click does not stack rows).
#[must_use]
pub fn article_draft_exists_for_candidate(conn: &Connection, candidate_id: i64) -> bool {
    conn.query_row(
        "SELECT COUNT(*) FROM article_drafts
          WHERE candidate_id = ?1 AND status IN ('drafted','approved')",
        params![candidate_id],
        |r| r.get::<_, i64>(0),
    )
    .map(|n| n > 0)
    .unwrap_or(false)
}

/// Queue counts for the tab badge.
#[must_use]
pub fn article_draft_queue_counts(conn: &Connection) -> (i64, i64, i64) {
    conn.query_row(
        "SELECT (SELECT COUNT(*) FROM article_drafts WHERE status = 'drafted'),
                (SELECT COUNT(*) FROM article_drafts WHERE status = 'approved'),
                (SELECT COUNT(*) FROM article_drafts WHERE publish_state IN ('published','verified'))",
        [],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )
    .unwrap_or((0, 0, 0))
}

#[cfg(test)]
mod batch31_tests {
    use super::*;

    /// Audit item 120: `&amp;` decoded FIRST double-decoded authored text
    /// — `&amp;lt;` became `&lt;` and then `<` — and `&apos;` never
    /// decoded at all. Single-decode semantics land `&amp;lt;` as the
    /// literal text `&lt;` and decode `&apos;`.
    #[test]
    fn entity_decode_is_single_pass_and_decodes_apos() {
        let out = html_to_text_keep_structure("a &amp;lt;script&amp;gt; b");
        assert_eq!(out, "a &lt;script&gt; b", "no double decode: {out:?}");
        assert_eq!(
            html_to_text_keep_structure("it&#39;s Dave&apos;s"),
            "it's Dave's",
            "both apostrophe entities decode"
        );
        assert_eq!(html_to_text_keep_structure("a &amp; b"), "a & b");
    }

    /// Audit item 121: İ (U+0130) expands to two chars when Unicode-
    /// lowercased, so the lowercased haystack's byte offsets stopped
    /// matching the original and the tag stripper sliced garbage. The
    /// helpers fold ASCII-only now — offsets stay 1:1 and the strip still
    /// works.
    #[test]
    fn html_helpers_survive_italian_i_before_tag_blocks() {
        let html = "İstruzioni <style>body { color: red }</style> visibili";
        let out = html_to_text_keep_structure(html);
        assert!(
            !out.contains("color"),
            "the style block is stripped: {out:?}"
        );
        assert!(
            out.contains("visibili"),
            "content after the block survives: {out:?}"
        );
        let out2 = html_to_text_keep_structure("İ<br>next");
        assert!(out2.contains("next"), "br handling still works: {out2:?}");
    }
}
