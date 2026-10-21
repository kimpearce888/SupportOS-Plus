use rusqlite::{Connection, params};

use crate::ai_prompts::{
    EvidenceContext, HistoryEntry, KnowledgeEntry, KnownIssueEntry, SavedReplyEntry, SimilarCase,
    ThreadEntry,
};
use crate::error::Result;
use crate::search::{
    fts_query_or, search_knowledge_raw, search_known_issues, search_saved_replies,
};

#[derive(Debug, Clone, PartialEq)]
pub struct SimilarConversation {
    pub conversation_id: i64,
    pub number: i64,
    pub subject: String,
    pub resolution: String,
    pub date: Option<String>,
    pub status: String,
    pub score: f64,
    pub why: Vec<String>,
}

/// Top durable customer facts, quarantine-filtered (A2 briefing feed —
/// shared by the evidence pack and the inbox "know before you reply" panel).
pub fn customer_memory_facts(
    conn: &Connection,
    customer_id: i64,
    limit: usize,
) -> Vec<crate::ai_prompts::MemoryEntry> {
    let Ok(mut stmt) = conn.prepare(
        "SELECT memory_key, memory_value, confidence
           FROM customer_memory
          WHERE customer_id = ?1
          ORDER BY last_seen_at DESC
          LIMIT ?2",
    ) else {
        return Vec::new();
    };
    stmt.query_map(params![customer_id, limit as i64], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, Option<String>>(2)?
                .unwrap_or_else(|| "low".into()),
        ))
    })
    .map(|rows| {
        rows.filter_map(|r| r.ok())
            .filter(|(key, value, _)| !crate::ai_memory_guard::is_quarantined(key, Some(value)))
            .map(|(key, value, confidence)| crate::ai_prompts::MemoryEntry {
                key,
                value,
                confidence,
            })
            .collect::<Vec<_>>()
    })
    .unwrap_or_default()
}

pub fn build(
    conn: &Connection,
    index: Option<&crate::vectorstore_sqlitevec::VectorIndex>,
    conversation_local_id: i64,
    include_internal: bool,
) -> Result<Option<EvidenceContext>> {
    let conv = match conn.query_row(
        "SELECT number, subject, preview, customer_local_id
           FROM conversations WHERE id = ?1",
        params![conversation_local_id],
        |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, Option<String>>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, Option<i64>>(3)?,
            ))
        },
    ) {
        Ok(v) => v,
        Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    let (number, subject, preview, customer_id) = conv;

    let customer_name = customer_id
        .and_then(|cid| {
            conn.query_row(
                "SELECT TRIM(COALESCE(first_name, '') || ' ' || COALESCE(last_name, '')) AS name
                   FROM customers WHERE id = ?1",
                params![cid],
                |r| r.get::<_, Option<String>>(0),
            )
            .ok()
            .flatten()
        })
        .unwrap_or_else(|| "Unknown customer".to_string());

    let mut stmt = conn.prepare(
        "SELECT type, from_name, body_html, body_text,
                COALESCE(remote_created_at, created_at), from_type,
                COALESCE(created_by_user_id, created_by_customer_id,
                         created_by_system_user_id)
           FROM conversation_threads
          WHERE conversation_id = ?1 AND deleted_at IS NULL AND state != 'draft'
          ORDER BY COALESCE(remote_created_at, created_at) ASC, id ASC",
    )?;
    let threads: Vec<ThreadEntry> = stmt
        .query_map(params![conversation_local_id], |r| {
            let kind: String = r.get(0)?;
            let from_name: Option<String> = r.get(1)?;
            let body_html: Option<String> = r.get(2)?;
            let body: Option<String> = r.get(3)?;
            let at: Option<String> = r.get(4)?;
            let actor_type: Option<String> = r.get(5)?;
            let actor_id: Option<i64> = r.get(6)?;
            let author = from_name.unwrap_or_else(|| {
                if actor_type.as_deref() == Some("user") && actor_id.unwrap_or(0) != 0 {
                    "Agent".to_string()
                } else {
                    "Customer".to_string()
                }
            });
            let raw_text = body_html.or(body).unwrap_or_default();
            Ok(ThreadEntry {
                author,
                kind,
                date: at.unwrap_or_default().chars().take(10).collect(),
                text: crate::textutil::html_to_text(&raw_text)
                    .chars()
                    .take(1500)
                    .collect(),
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let history: Vec<HistoryEntry> = match customer_id {
        Some(cid) => {
            let mut stmt = conn.prepare(
                "SELECT cv.number, cv.subject, cv.preview, COALESCE(cv.remote_created_at, cv.created_at),
                        COALESCE((SELECT t2.body_text FROM conversation_threads t2
                                   WHERE t2.conversation_id = cv.id AND t2.type='reply'
                                   ORDER BY COALESCE(t2.remote_created_at, t2.created_at) DESC LIMIT 1), '') AS last_reply
                   FROM conversations cv
                  WHERE cv.customer_local_id = ?1 AND cv.id != ?2 AND cv.deleted_at IS NULL
                  ORDER BY COALESCE(cv.remote_created_at, cv.created_at) DESC LIMIT 5",
            )?;
            let rows: Vec<HistoryEntry> = stmt
                .query_map(params![cid, conversation_local_id], |r| {
                    let number: i64 = r.get(0)?;
                    let subject: Option<String> = r.get(1)?;
                    let preview: Option<String> = r.get(2)?;
                    let at: Option<String> = r.get(3)?;
                    let last_reply: String = r.get(4)?;
                    let summary_source = if last_reply.is_empty() {
                        preview.unwrap_or_default()
                    } else {
                        last_reply
                    };
                    Ok(HistoryEntry {
                        number,
                        subject: subject.unwrap_or_else(|| "(no subject)".into()),
                        summary: summary_source.chars().take(200).collect(),
                        days_ago: at.as_deref().map(days_ago).unwrap_or(0),
                    })
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            rows
        }
        None => Vec::new(),
    };

    // A4 retrieval spine: the semantic lane feeds real sqlite-vec hits into
    // the similar-issues merge, so evidence chips can say "semantic match"
    // with an actual vector behind them. Absent index / unrouted Embed role /
    // disabled feature = the honest empty lane (FTS-only).
    let semantic: Vec<(i64, f64)> = match index {
        Some(ix) => semantic_hits(conn, ix, conversation_local_id, 8),
        None => Vec::new(),
    };
    let similar: Vec<SimilarCase> = find_similar(conn, conversation_local_id, 5, &semantic)
        .unwrap_or_default()
        .into_iter()
        .map(|s| SimilarCase {
            number: s.number,
            subject: s.subject,
            resolution: s.resolution.chars().take(300).collect(),
            date: s.date.unwrap_or_default().chars().take(10).collect(),
            visibility: "internal_only".to_string(),
        })
        .collect();

    let query = format!(
        "{} {}",
        subject.as_deref().unwrap_or(""),
        preview.as_deref().unwrap_or("")
    );
    let known_issues: Vec<KnownIssueEntry> = search_known_issues(conn, &query, 3)
        .unwrap_or_default()
        .iter()
        .filter_map(|k| {
            conn.query_row(
                "SELECT title, symptoms, customer_safe_explanation, workaround
                   FROM known_issues WHERE id = ?1",
                params![k.id],
                |r| {
                    Ok(KnownIssueEntry {
                        title: r.get(0)?,
                        symptoms: r.get::<_, Option<String>>(1)?.unwrap_or_default(),
                        customer_safe_explanation: r.get(2)?,
                        workaround: r.get(3)?,
                    })
                },
            )
            .ok()
        })
        .take(3)
        .collect();

    let knowledge: Vec<KnowledgeEntry> = search_knowledge_raw(
        conn,
        &query,
        if include_internal {
            None
        } else {
            Some("customer_safe")
        },
        4,
        "or",
    )
    .unwrap_or_default()
    .iter()
    .take(4)
    .filter_map(|k| {
        conn.query_row(
            "SELECT title, content FROM knowledge_documents WHERE id = ?1",
            params![k.document_id],
            |r| {
                Ok(KnowledgeEntry {
                    title: r.get(0)?,
                    text: r
                        .get::<_, Option<String>>(1)?
                        .unwrap_or_else(|| k.snippet.clone())
                        .chars()
                        .take(1200)
                        .collect(),
                    visibility: if k.visibility == "customer_safe" {
                        "customer_safe".to_string()
                    } else {
                        "internal_only".to_string()
                    },
                })
            },
        )
        .ok()
    })
    .collect();

    let saved_replies: Vec<SavedReplyEntry> = search_saved_replies(conn, &query, 3)
        .unwrap_or_default()
        .iter()
        .take(3)
        .filter_map(|r| {
            conn.query_row(
                "SELECT name, text, preview FROM saved_replies WHERE id = ?1",
                params![r.id],
                |row| {
                    let name: String = row.get(0)?;
                    let text: Option<String> = row.get(1)?;
                    let preview: Option<String> = row.get(2)?;
                    let body = text.or(preview).unwrap_or_else(|| r.snippet.clone());
                    Ok(SavedReplyEntry {
                        name,
                        text: body.chars().take(800).collect(),
                    })
                },
            )
            .ok()
        })
        .collect();

    Ok(Some(EvidenceContext {
        conversation_number: number,
        subject: subject.unwrap_or_else(|| "(no subject)".into()),
        customer_name,
        customer_history: history,
        threads,
        similar_cases: similar,
        known_issues,
        knowledge,
        saved_replies,
        interaction_strategy: None,
        // A2: the evidence pack carries the customer's durable facts —
        // filtered through the memory guard so quarantined material
        // (payment/credential/health) never reaches a prompt.
        customer_memory: customer_id
            .map(|cid| customer_memory_facts(conn, cid, 5))
            .unwrap_or_default(),
    }))
}

/// A4 retrieval spine: real sqlite-vec hits for a conversation's own text.
///
/// Embeds the conversation's subject+preview through the hub Embed role
/// (local by default, cloud only on explicit user routing), searches the
/// vector index, and maps `conversation_chunk` payload hits back to their
/// conversations — one `(conversation_local_id, score)` pair per
/// conversation, keeping the best score. Best-effort by design: the
/// retrieval feature off, an unrouted Embed role, an unreachable endpoint
/// or an empty index all yield an empty vec and callers stay FTS-only.
#[must_use]
pub fn semantic_hits(
    conn: &Connection,
    index: &crate::vectorstore_sqlitevec::VectorIndex,
    conversation_local_id: i64,
    limit: usize,
) -> Vec<(i64, f64)> {
    if !crate::ai_hub::ai_feature_enabled(conn, "retrieval") {
        return Vec::new();
    }
    let query: String = match conn.query_row(
        "SELECT COALESCE(subject, '') || ' ' || COALESCE(preview, '')
           FROM conversations WHERE id = ?1",
        params![conversation_local_id],
        |r| r.get(0),
    ) {
        Ok(q) => q,
        Err(_) => return Vec::new(),
    };
    if query.trim().is_empty() {
        return Vec::new();
    }
    let vector = match crate::embeddings::embed_query_text_blocking(conn, &query) {
        Ok(Some(v)) => v,
        Ok(None) => return Vec::new(),
        Err(e) => {
            tracing::debug!(
                operation = "semantic_hits",
                "query embedding unavailable: {e}"
            );
            return Vec::new();
        }
    };
    let search_limit = limit.saturating_mul(4).max(12);
    let hits = index.search(&vector, search_limit);
    let mut best: Vec<(i64, f64)> = Vec::new();
    for hit in hits {
        let payload = match hit.payload.as_object() {
            Some(p) => p,
            None => continue,
        };
        if payload.get("entity_type").and_then(|v| v.as_str()) != Some("conversation_chunk") {
            continue;
        }
        let Some(entity_id) = payload.get("entity_id").and_then(|v| v.as_i64()) else {
            continue;
        };
        if entity_id == conversation_local_id {
            continue;
        }
        let score = f64::from(hit.score);
        match best.iter_mut().find(|(cid, _)| *cid == entity_id) {
            Some(entry) => {
                if score > entry.1 {
                    entry.1 = score;
                }
            }
            None => best.push((entity_id, score)),
        }
        if best.len() >= limit {
            break;
        }
    }
    best.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    best
}

pub fn find_similar(
    conn: &Connection,
    conversation_local_id: i64,
    limit: usize,
    semantic_hits: &[(i64, f64)],
) -> Result<Vec<SimilarConversation>> {
    let me = match conn.query_row(
        "SELECT c.id, c.number, c.subject, c.preview, c.status, c.customer_local_id,
                COALESCE(c.remote_created_at, c.created_at),
                (SELECT GROUP_CONCAT(t.name) FROM conversation_tags ct JOIN tags t ON t.id = ct.tag_id
                  WHERE ct.conversation_id = c.id) AS tags
           FROM conversations c WHERE c.id = ?1",
        params![conversation_local_id],
        |r| {
            Ok(MeRow {
                subject: r.get(2)?,
                preview: r.get(3)?,
                customer_id: r.get(5)?,
                tags: r.get(7)?,
            })
        },
    ) {
        Ok(v) => v,
        Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(Vec::new()),
        Err(e) => return Err(e.into()),
    };

    let my_tags: Vec<String> = me
        .tags
        .unwrap_or_default()
        .split(',')
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect();

    let keywords = fts_query_or(
        &format!(
            "{} {}",
            me.subject.clone().unwrap_or_default(),
            me.preview.clone().unwrap_or_default()
        ),
        10,
    );
    let fts_rows: Vec<CandidateRow> = if keywords != "\"\"" {
        let mut stmt = conn.prepare(
        "SELECT c.id, c.number, c.subject, c.preview, c.status, c.customer_local_id,
                COALESCE(c.remote_created_at, c.created_at),
                (SELECT GROUP_CONCAT(t.name) FROM conversation_tags ct JOIN tags t ON t.id = ct.tag_id
                  WHERE ct.conversation_id = c.id) AS tags, MIN(rank) AS rank
               FROM (SELECT conversation_id, rank FROM fts_conversations WHERE fts_conversations MATCH ?1
                     UNION ALL
                     SELECT conversation_id, rank FROM fts_threads WHERE fts_threads MATCH ?1) m
               JOIN conversations c ON c.id = m.conversation_id
              WHERE c.id != ?2 AND c.deleted_at IS NULL
              GROUP BY c.id ORDER BY rank LIMIT 25",
        )?;
        let rows: Vec<CandidateRow> = stmt
            .query_map(params![keywords, conversation_local_id], map_candidate_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        rows
    } else {
        Vec::new()
    };

    struct Scored {
        row: CandidateRow,
        score: f64,
        why: Vec<String>,
    }
    let mut candidates: Vec<Scored> = fts_rows
        .into_iter()
        .map(|row| Scored {
            row,
            score: 0.4,
            why: vec!["keyword match".to_string()],
        })
        .collect();

    for (cid, score) in semantic_hits {
        if *cid == conversation_local_id {
            continue;
        }
        let idx = candidates.iter().position(|c| c.row.id == *cid);
        match idx {
            Some(idx) => {
                candidates[idx].score += score * 0.5;
                candidates[idx].why.push("semantic match".to_string());
            }
            None => {
                if let Ok(row) = conn.query_row(
                    "SELECT c.id, c.number, c.subject, c.preview, c.status, c.customer_local_id,
                            COALESCE(c.remote_created_at, c.created_at),
                            (SELECT GROUP_CONCAT(t.name) FROM conversation_tags ct JOIN tags t ON t.id = ct.tag_id
                              WHERE ct.conversation_id = c.id) AS tags, 0 AS rank
                       FROM conversations c WHERE c.id = ?1 AND c.deleted_at IS NULL",
                    params![cid],
                    map_candidate_row,
                ) {
                    candidates.push(Scored {
                        row,
                        score: score * 0.5,
                        why: vec!["semantic match".to_string()],
                    });
                }
            }
        }
    }

    let now_ms = now_millis();
    let mut scored: Vec<SimilarConversation> = Vec::new();
    for c in &candidates {
        let mut score = c.score;
        let mut why = c.why.clone();
        if let (Some(their), Some(mine)) = (c.row.customer_id, me.customer_id)
            && their == mine
        {
            score += 0.15;
            why.push("same customer".to_string());
        }
        let their_tags: Vec<String> = c
            .row
            .tags
            .clone()
            .unwrap_or_default()
            .split(',')
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect();
        let shared: Vec<&String> = my_tags.iter().filter(|t| their_tags.contains(t)).collect();
        if !shared.is_empty() {
            score += (shared.len() as f64 * 0.08).min(0.25);
            let names: Vec<&str> = shared.iter().map(|s| s.as_str()).collect();
            why.push(format!("shared tags: {}", names.join(", ")));
        }
        let age_days = c
            .row
            .remote_created_at
            .as_deref()
            .map(|s| (now_ms - parse_epoch_millis(s)) as f64 / 86_400_000.0)
            .unwrap_or(999.0);
        if age_days < 90.0 {
            score += 0.1;
            why.push("recent".to_string());
        }
        let last_reply: String = conn
            .query_row(
                "SELECT body_text FROM conversation_threads
                  WHERE conversation_id = ?1 AND type='reply'
                  ORDER BY COALESCE(remote_created_at, created_at) DESC LIMIT 1",
                params![c.row.id],
                |r| r.get(0),
            )
            .unwrap_or_default();
        let resolution = if last_reply.is_empty() {
            c.row.preview.clone().unwrap_or_default()
        } else {
            last_reply.chars().take(400).collect()
        };
        scored.push(SimilarConversation {
            conversation_id: c.row.id,
            number: c.row.number,
            subject: c
                .row
                .subject
                .clone()
                .unwrap_or_else(|| "(no subject)".into()),
            resolution,
            date: c.row.remote_created_at.clone(),
            status: c.row.status.clone(),
            score: (score * 100.0).round() / 100.0,
            why,
        });
    }
    scored.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    scored.truncate(limit);
    Ok(scored)
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AiSourceRef {
    pub source_type: String,
    pub source_id: i64,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relevance: Option<f64>,
    pub visibility: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<String>,
}

pub fn sources_for(conn: &Connection, ctx: &EvidenceContext) -> Vec<AiSourceRef> {
    let mut sources: Vec<AiSourceRef> = Vec::new();
    let conv_id_by_number = |number: i64| -> Option<i64> {
        conn.query_row(
            "SELECT id FROM conversations WHERE number = ?1",
            params![number],
            |r| r.get(0),
        )
        .ok()
    };
    for h in &ctx.customer_history {
        if let Some(id) = conv_id_by_number(h.number) {
            sources.push(AiSourceRef {
                source_type: "conversation".into(),
                source_id: id,
                title: format!("#{} {}", h.number, h.subject),
                relevance: Some(0.5),
                visibility: "internal_only".into(),
                timestamp: None,
            });
        }
    }
    for s in &ctx.similar_cases {
        if let Some(id) = conv_id_by_number(s.number) {
            sources.push(AiSourceRef {
                source_type: "conversation".into(),
                source_id: id,
                title: format!("#{} {}", s.number, s.subject),
                relevance: Some(0.7),
                visibility: s.visibility.clone(),
                timestamp: None,
            });
        }
    }
    for k in &ctx.knowledge {
        if let Ok(id) = conn.query_row(
            "SELECT id FROM knowledge_documents WHERE title = ?1",
            params![k.title],
            |r| r.get(0),
        ) {
            sources.push(AiSourceRef {
                source_type: "knowledge_document".into(),
                source_id: id,
                title: k.title.clone(),
                relevance: Some(0.8),
                visibility: k.visibility.clone(),
                timestamp: None,
            });
        }
    }
    for k in &ctx.known_issues {
        if let Ok(id) = conn.query_row(
            "SELECT id FROM known_issues WHERE title = ?1",
            params![k.title],
            |r| r.get(0),
        ) {
            sources.push(AiSourceRef {
                source_type: "known_issue".into(),
                source_id: id,
                title: k.title.clone(),
                relevance: Some(0.8),
                visibility: "uncertain".into(),
                timestamp: None,
            });
        }
    }
    for s in &ctx.saved_replies {
        if let Ok(id) = conn.query_row(
            "SELECT id FROM saved_replies WHERE name = ?1",
            params![s.name],
            |r| r.get(0),
        ) {
            sources.push(AiSourceRef {
                source_type: "saved_reply".into(),
                source_id: id,
                title: s.name.clone(),
                relevance: Some(0.6),
                visibility: "customer_safe".into(),
                timestamp: None,
            });
        }
    }
    sources
}

#[derive(Debug, Clone)]
struct MeRow {
    subject: Option<String>,
    preview: Option<String>,
    customer_id: Option<i64>,
    tags: Option<String>,
}

#[derive(Debug, Clone)]
struct CandidateRow {
    id: i64,
    number: i64,
    subject: Option<String>,
    preview: Option<String>,
    status: String,
    customer_id: Option<i64>,
    remote_created_at: Option<String>,
    tags: Option<String>,
}

fn map_candidate_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<CandidateRow> {
    Ok(CandidateRow {
        id: r.get(0)?,
        number: r.get(1)?,
        subject: r.get(2)?,
        preview: r.get(3)?,
        status: r.get(4)?,
        customer_id: r.get(5)?,
        remote_created_at: r.get(6)?,
        tags: r.get(7)?,
    })
}

fn days_ago(iso: &str) -> i64 {
    let delta = now_millis() - parse_epoch_millis(iso);
    ((delta as f64) / 86_400_000.0).round() as i64
}

fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn parse_epoch_millis(s: &str) -> i64 {
    let s = s.trim();
    let try_formats = [
        "%Y-%m-%dT%H:%M:%S%.fZ",
        "%Y-%m-%dT%H:%M:%S%.f",
        "%Y-%m-%d %H:%M:%S%.f",
        "%Y-%m-%d %H:%M:%S",
        "%Y-%m-%d",
    ];
    for f in try_formats {
        if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(s, f) {
            return dt.and_utc().timestamp_millis();
        }
        if f == "%Y-%m-%d"
            && let Ok(d) = chrono::NaiveDate::parse_from_str(s, f)
        {
            return d.and_hms_opt(0, 0, 0).unwrap().and_utc().timestamp_millis();
        }
    }
    0
}

#[cfg(test)]
mod a2_memory_feed_tests {
    use super::*;

    fn db_with_customer_memory() -> Connection {
        let mut conn = Connection::open_in_memory().expect("db");
        crate::bootstrap::apply_all(&mut conn).expect("migrations");
        conn.execute(
            "INSERT INTO customers (remote_id, first_name, last_name)
             VALUES (70, 'Ann', 'Lee')",
            [],
        )
        .expect("customer");
        let customer = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO conversations (remote_id, number, subject, preview, status, customer_local_id)
             VALUES (9201, 92, 'Billing question', 'preview', 'active', ?1)",
            rusqlite::params![customer],
        )
        .expect("conversation");
        let conv = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO customer_memory (customer_id, memory_key, memory_value, evidence_excerpt,
                     source_conversation_id, confidence, provenance, source, last_seen_at)
             VALUES (?1, 'prefers short replies', 'yes — keep replies under five lines', 'from threads',
                     NULL, 'high', 'ai_generated', 'ai', datetime('now'))",
            rusqlite::params![customer],
        )
        .expect("memory");
        conn.execute(
            "INSERT INTO customer_memory (customer_id, memory_key, memory_value, evidence_excerpt,
                     source_conversation_id, confidence, provenance, source, last_seen_at)
             VALUES (?1, 'diagnosed with bipolar', 'mentioned in a thread', 'from threads',
                     NULL, 'high', 'ai_generated', 'ai', datetime('now'))",
            rusqlite::params![customer],
        )
        .expect("quarantined memory");
        conn.execute(
            "INSERT INTO conversation_threads (conversation_id, type, body_text, created_at)
             VALUES (?1, 'customer', 'Hello, a quick question about my invoice.', datetime('now'))",
            rusqlite::params![conv],
        )
        .expect("thread");
        conn
    }

    /// A2: the evidence pack carries the customer's durable facts — and the
    /// memory guard keeps quarantined material (payment details here) out
    /// of every prompt.
    #[test]
    fn evidence_pack_carries_memory_and_quarantines_secrets() {
        let conn = db_with_customer_memory();
        let conv: i64 = conn
            .query_row("SELECT id FROM conversations", [], |r| r.get(0))
            .expect("conv id");
        let ctx = build(&conn, None, conv, false)
            .expect("build")
            .expect("context");
        assert!(
            ctx.customer_memory
                .iter()
                .any(|m| m.key == "prefers short replies"),
            "the durable fact must reach the evidence pack"
        );
        assert!(
            ctx.customer_memory
                .iter()
                .all(|m| !m.key.contains("diagnosed")),
            "quarantined material must never reach a prompt: {:?}",
            ctx.customer_memory
        );
        // The rendered evidence carries the briefing block for the model.
        let rendered = crate::ai_prompts::build_ticket_analysis_user(&ctx);
        assert!(rendered.contains("DURABLE CUSTOMER FACTS"));
        assert!(rendered.contains("prefers short replies"));
        assert!(!rendered.contains("bipolar"));
    }
}

#[cfg(test)]
mod batch13_a4_evidence_tests {
    use super::*;
    use rusqlite::Connection;

    fn db_with_conversation() -> (Connection, i64) {
        let mut conn = Connection::open_in_memory().expect("db");
        crate::bootstrap::apply_all(&mut conn).expect("migrations");
        conn.execute(
            "INSERT INTO conversations (remote_id, number, subject, preview, status)
             VALUES (9701, 97, 'Export timeouts nightly', 'preview', 'active')",
            [],
        )
        .expect("conversation");
        let conv = conn.last_insert_rowid();
        (conn, conv)
    }

    /// A4: with no Embed role routed (and no legacy embedding model), the
    /// semantic lane yields nothing — find_similar stays FTS-only and its
    /// results keep their honest "keyword match" reasons.
    #[test]
    fn semantic_hits_empty_without_routed_embed_role() {
        let (conn, conv) = db_with_conversation();
        let dir = std::env::temp_dir().join(format!("spp-evidence-test-{}", std::process::id()));
        let index = crate::vectorstore_sqlitevec::VectorIndex::new(&dir, true);
        let hits = semantic_hits(&conn, &index, conv, 5);
        assert!(hits.is_empty(), "no embedding model means no semantic lane");
        let similar = find_similar(&conn, conv, 5, &hits).expect("similar");
        assert!(
            similar
                .iter()
                .all(|s| !s.why.iter().any(|w| w == "semantic match")),
            "no semantic claims without vectors"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A4: the retrieval feature gate closes the semantic lane even when a
    /// model would be reachable.
    #[test]
    fn semantic_hits_respect_the_feature_gate() {
        let (conn, conv) = db_with_conversation();
        crate::settings::set_bool(&conn, "ai_feature_retrieval_enabled", false).expect("gate");
        let dir = std::env::temp_dir().join(format!("spp-evidence-gate-{}", std::process::id()));
        let index = crate::vectorstore_sqlitevec::VectorIndex::new(&dir, true);
        let hits = semantic_hits(&conn, &index, conv, 5);
        assert!(hits.is_empty(), "the kill switch dominates the lane");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
