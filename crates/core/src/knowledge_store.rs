use std::io::Read as _;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, params};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::error::{Error, Result};

pub const IMPORT_DIR_NAME: &str = "knowledge-import";

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
                let usage_count: i64 = r.get::<_, i64>(9).unwrap_or(0);
                let days_since_update = updated_at.as_deref().and_then(days_since);
                let days_since_review = reviewed_at.as_deref().and_then(days_since);
                let stale = days_since_update.is_some_and(|d| d > stale_days);
                let unreviewed_long = match days_since_review {
                    None => days_since_update.is_some_and(|d| d > unreviewed_days),
                    Some(d) => d > unreviewed_days,
                };
                let unverified = verified_at.is_none();
                let low_usage = usage_count == 0;
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
                    "chunk_count": r.get::<_, i64>(8)?,
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
    json!({
        "stale": stale,
        "fresh": fresh,
        "documents": rows,
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
            let text = parse_pdf_text(&raw)?;
            if text.trim().is_empty() {
                return Err(Error::Validation(
                    "No text could be extracted from this PDF (it may be scanned images)."
                        .to_string(),
                ));
            }
            vec![(base, text, "txt".to_string())]
        }
        "docx" => {
            let text = parse_docx_text(&raw)?;
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

fn parse_pdf_text(raw: &[u8]) -> Result<String> {
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
        let mut document = archive
            .by_name("word/document.xml")
            .map_err(|_| docx_unavailable())?;
        document
            .read_to_string(&mut xml)
            .map_err(|_| docx_unavailable())?;
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
    for (entity, decoded) in [
        ("&nbsp;", " "),
        ("&amp;", "&"),
        ("&lt;", "<"),
        ("&gt;", ">"),
        ("&quot;", "\""),
        ("&#39;", "'"),
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
    let lower = input.to_lowercase();
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
    let lower = input.to_lowercase();
    let from_lower = from.to_lowercase();
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
    let lower = html.to_lowercase();
    let start = lower.find("<title>")? + "<title>".len();
    let end = lower[start..].find("</title>")? + start;
    let title = html[start..end].trim();
    if title.is_empty() {
        None
    } else {
        Some(title.to_string())
    }
}
