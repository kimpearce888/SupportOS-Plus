use rusqlite::{Connection, params};
use serde_json::{Value, json};

use crate::business_hours::now_iso;
use crate::error::{Error, Result};
use crate::migrations::add_column_if_missing;
use crate::textutil::cap_chars;

mod qa_ai;
pub(crate) use qa_ai::compute_qa_ai_layer;
pub(crate) use qa_ai::derive_outcome;

fn rx(pattern: &'static str) -> Result<regex::Regex> {
    regex::Regex::new(pattern).map_err(|e| Error::Other(Box::new(e)))
}

const FRICTION_FINDINGS_SQL: &str = r#"
    CREATE TABLE IF NOT EXISTS friction_findings (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        conversation_id INTEGER NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
        customer_local_id INTEGER REFERENCES customers(id) ON DELETE SET NULL,
        kind TEXT NOT NULL,
        severity TEXT NOT NULL DEFAULT 'low',
        evidence TEXT NOT NULL DEFAULT '[]',
        detail TEXT,
        computed_at TEXT NOT NULL DEFAULT (datetime('now'))
    );
    CREATE UNIQUE INDEX IF NOT EXISTS uq_friction_findings_conv_kind
        ON friction_findings(conversation_id, kind);
    CREATE INDEX IF NOT EXISTS idx_friction_findings_kind_severity
        ON friction_findings(kind, severity);
    CREATE INDEX IF NOT EXISTS idx_friction_findings_customer_ref
        ON friction_findings(customer_local_id);
"#;

pub(crate) fn ensure_quality_tables(conn: &Connection) -> Result<()> {
    conn.execute_batch(FRICTION_FINDINGS_SQL)?;
    add_column_if_missing(conn, "friction_findings", "detail", "TEXT")?;
    add_column_if_missing(conn, "post_resolution_qa", "computed_at", "TEXT")?;
    Ok(())
}

pub(crate) struct ThreadLite {
    pub thread_id: i64,
    pub kind: String,
    pub text: String,
    pub at: Option<String>,
}

pub(crate) fn published_threads(conn: &Connection, conversation_local: i64) -> Vec<ThreadLite> {
    crate::db::query_rows(
        conn,
        "SELECT id, thread_type, html_stripped, at FROM (
             SELECT id, type AS thread_type, COALESCE(body_html, body_text, '') AS html_stripped,
                    COALESCE(remote_created_at, created_at) AS at
             FROM conversation_threads
             WHERE conversation_id = ?1 AND state = 'published' AND deleted_at IS NULL
         ) ORDER BY at ASC",
        params![conversation_local],
        |r| {
            Ok(ThreadLite {
                thread_id: r.get(0)?,
                kind: r.get(1)?,
                text: crate::textutil::html_to_text(&r.get::<_, String>(2)?),
                at: r.get(3)?,
            })
        },
    )
    .unwrap_or_default()
}

pub(crate) fn is_closing_acknowledgment(text: &str) -> Result<bool> {
    static RE: std::sync::OnceLock<std::result::Result<regex::Regex, regex::Error>> =
        std::sync::OnceLock::new();
    static START: std::sync::OnceLock<std::result::Result<regex::Regex, regex::Error>> =
        std::sync::OnceLock::new();
    let re = RE
        .get_or_init(|| {
            regex::Regex::new(
                r"(?i)still|again|but\b|however|issue|problem|not work|broken|error|fail|doesn'?t|didn'?t|can'?t|cannot",
            )
        })
        .as_ref()
        .map_err(|e| Error::Other(Box::new(e.clone())))?;
    let start = START
        .get_or_init(|| {
            regex::Regex::new(
                r"(?i)^(thanks|thank you|thankyou|thx|appreciate|that (is|was|'s|sounds|seems) (exactly |just |very )?(what i|great|perfect|helpful|awesome|amazing|clear)|perfect|great|works|working|resolved|closing|closed|all set|confirmed|done|sorted)",
            )
        })
        .as_ref()
        .map_err(|e| Error::Other(Box::new(e.clone())))?;
    let t = text.trim();
    if t.is_empty() || t.chars().count() > 400 || t.contains('?') {
        return Ok(false);
    }
    if re.is_match(t) {
        return Ok(false);
    }
    Ok(start.is_match(t))
}

fn excerpt_of(text: &str, max: usize) -> String {
    let t: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if t.chars().count() <= max {
        t
    } else {
        let cut: String = cap_chars(&t, max);
        format!("{cut}...")
    }
}

fn normalize_span(text: &str) -> Vec<String> {
    let lowered: String = text
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { ' ' })
        .collect();
    lowered
        .split_whitespace()
        .filter(|w| w.chars().count() > 1)
        .map(String::from)
        .collect()
}

struct RepeatedSpan {
    span: String,
    thread_ids: Vec<i64>,
}

fn repeated_spans(messages: &[&ThreadLite], span_words: usize) -> Vec<RepeatedSpan> {
    let mut out: Vec<RepeatedSpan> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let normalized: Vec<(i64, Vec<String>, String)> = messages
        .iter()
        .map(|m| {
            let words = normalize_span(&m.text);
            let joined = words.join(" ");
            (m.thread_id, words, joined)
        })
        .collect();
    for i in 0..normalized.len() {
        let (tid, words, _) = &normalized[i];
        if words.len() < span_words {
            continue;
        }
        for start in 0..=(words.len() - span_words) {
            let span = words[start..start + span_words].join(" ");
            if !seen.insert(span.clone()) {
                continue;
            }
            let mut thread_ids = vec![*tid];
            for (tid_k, _, joined_k) in normalized.iter().skip(i + 1) {
                if joined_k.contains(&span) {
                    thread_ids.push(*tid_k);
                }
            }
            if thread_ids.len() >= 2 {
                out.push(RepeatedSpan { span, thread_ids });
            }
        }
    }
    out
}

fn split_after_question(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut prev_question = false;
    for c in text.chars() {
        if c.is_whitespace() {
            if prev_question && !current.is_empty() {
                out.push(std::mem::take(&mut current));
            }
            prev_question = false;
            continue;
        }
        prev_question = c == '?';
        current.push(c);
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

pub(crate) const KNOWLEDGE_GAP_KINDS: [(&str, &str); 5] = [
    (
        "repeated_question_uncovered",
        "Repeated question, no covering document",
    ),
    (
        "repeated_question_unsolved",
        "Repeated question, existing docs did not solve it",
    ),
    ("conflicting_knowledge", "Conflicting knowledge documents"),
    (
        "missing_troubleshooting_steps",
        "Missing troubleshooting steps",
    ),
    ("new_issue_undocumented", "New issue with no documentation"),
];

struct RepeatedQuestion {
    question: String,
    conversation_ids: Vec<i64>,
    count: usize,
}

fn repeated_questions(conn: &Connection, days: i64) -> Vec<RepeatedQuestion> {
    let days = days.clamp(1, 3650);
    let rows: Vec<(String, i64)> = crate::db::query_rows(
        conn,
        "SELECT json_extract(response_json, '$.primary_question') AS question, conversation_id
             FROM ai_runs
             WHERE type = 'ticket_analysis' AND status = 'completed' AND conversation_id IS NOT NULL
               AND id IN (SELECT MAX(id) FROM ai_runs
                           WHERE type = 'ticket_analysis' AND status = 'completed'
                             AND conversation_id IS NOT NULL
                           GROUP BY conversation_id)
               AND json_extract(response_json, '$.primary_question') IS NOT NULL
               AND created_at >= strftime('%Y-%m-%dT%H:%M:%fZ','now','-' || ?1 || ' days')",
        params![days],
        |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)),
    )
    .unwrap_or_default();
    let mut by_q: Vec<(String, Vec<i64>)> = Vec::new();
    let mut index: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for (question, conversation) in rows {
        let q: String = question
            .to_lowercase()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        if q.is_empty() {
            continue;
        }
        match index.get(&q) {
            Some(&i) => by_q[i].1.push(conversation),
            None => {
                index.insert(q.clone(), by_q.len());
                by_q.push((q, vec![conversation]));
            }
        }
    }
    let mut out: Vec<RepeatedQuestion> = by_q
        .into_iter()
        .filter(|(_, ids)| ids.len() >= 2)
        .map(|(question, conversation_ids)| RepeatedQuestion {
            count: conversation_ids.len(),
            question,
            conversation_ids,
        })
        .collect();
    out.sort_by_key(|q| std::cmp::Reverse(q.count));
    out.truncate(40);
    out
}

fn fts_tokens(question: &str) -> Option<String> {
    let tokens: Vec<String> = question
        .replace(['"', '*', '(', ')'], " ")
        .split_whitespace()
        .filter(|t| t.chars().count() > 2)
        .take(6)
        .map(|t| format!("\"{t}\"*"))
        .collect();
    (!tokens.is_empty()).then(|| tokens.join(" "))
}

fn knowledge_hits(conn: &Connection, question: &str) -> i64 {
    let Some(tokens) = fts_tokens(question) else {
        return 0;
    };
    let knowledge: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM fts_knowledge WHERE fts_knowledge MATCH ?1",
            params![tokens],
            |r| r.get(0),
        )
        .unwrap_or(0);
    if knowledge > 0 {
        return knowledge;
    }
    conn.query_row(
        "SELECT COUNT(*) FROM docs_fts WHERE docs_fts MATCH ?1",
        params![tokens],
        |r| r.get(0),
    )
    .unwrap_or(0)
}

fn closest_document(conn: &Connection, question: &str) -> Option<(i64, String, String)> {
    let tokens = fts_tokens(question)?;
    conn.query_row(
        "SELECT f.document_id, d.title, d.content
         FROM fts_knowledge f JOIN knowledge_documents d ON d.id = f.document_id
         WHERE fts_knowledge MATCH ?1 ORDER BY rank LIMIT 1",
        params![tokens],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )
    .ok()
}

fn friction_among(conn: &Connection, conversation_ids: &[i64]) -> usize {
    if conversation_ids.is_empty() {
        return 0;
    }
    let placeholders = vec!["?"; conversation_ids.len()].join(",");
    let sql = format!(
        "SELECT COUNT(*) FROM client_support_outcomes
         WHERE conversation_id IN ({placeholders})
           AND (follow_up_count > 0 OR clarification_count > 0)"
    );
    let Ok(mut stmt) = conn.prepare(&sql) else {
        return 0;
    };
    let n: i64 = stmt
        .query_row(rusqlite::params_from_iter(conversation_ids.iter()), |r| {
            r.get(0)
        })
        .unwrap_or(0);
    usize::try_from(n).unwrap_or(0)
}

fn seen_bounds(conn: &Connection, conversation_ids: &[i64]) -> (Option<String>, Option<String>) {
    if conversation_ids.is_empty() {
        return (None, None);
    }
    let placeholders = vec!["?"; conversation_ids.len()].join(",");
    let sql = format!(
        "SELECT MIN(COALESCE(remote_created_at, created_at)),
                MAX(COALESCE(remote_created_at, created_at))
         FROM conversations WHERE id IN ({placeholders})"
    );
    conn.query_row(
        &sql,
        rusqlite::params_from_iter(conversation_ids.iter()),
        |r| Ok((r.get(0)?, r.get(1)?)),
    )
    .unwrap_or((None, None))
}

fn insert_gap_candidate(
    conn: &Connection,
    kind: &str,
    question: &str,
    count: usize,
    conversation_ids: &[i64],
    document_ids: &[i64],
    detail: Value,
) -> bool {
    let normalized: String = cap_chars(
        &question
            .to_lowercase()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" "),
        300,
    );
    let dedup_key = format!("{kind}:{normalized}");
    let evidence: Vec<i64> = conversation_ids.iter().take(50).copied().collect();
    let docs: Vec<i64> = document_ids.iter().take(20).copied().collect();
    let existing: Option<i64> = conn
        .query_row(
            "SELECT id FROM knowledge_gap_candidates WHERE dedup_key = ?1",
            params![dedup_key],
            |r| r.get(0),
        )
        .ok();
    match existing {
        None => {
            conn.execute(
                "INSERT INTO knowledge_gap_candidates (dedup_key, kind, query_text,
                     occurrence_count, evidence_conversation_ids, related_document_ids,
                     detail, status, provenance, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'open', 'deterministic_local',
                     datetime('now'), datetime('now'))",
                params![
                    dedup_key,
                    kind,
                    normalized,
                    count as i64,
                    serde_json::to_string(&evidence).unwrap_or_default(),
                    serde_json::to_string(&docs).unwrap_or_default(),
                    detail.to_string()
                ],
            )
            .ok();
            true
        }
        Some(id) => {
            conn.execute(
                "UPDATE knowledge_gap_candidates SET occurrence_count = ?1,
                     evidence_conversation_ids = ?2, related_document_ids = ?3,
                     detail = ?4, updated_at = datetime('now')
                 WHERE id = ?5",
                params![
                    count as i64,
                    serde_json::to_string(&evidence).unwrap_or_default(),
                    serde_json::to_string(&docs).unwrap_or_default(),
                    detail.to_string(),
                    id
                ],
            )
            .ok();
            false
        }
    }
}

pub(crate) fn rebuild_gaps(conn: &Connection, days: i64) -> Result<(usize, usize)> {
    static TROUBLESHOOTING: std::sync::OnceLock<std::result::Result<regex::Regex, regex::Error>> =
        std::sync::OnceLock::new();
    static STEP_STRUCTURE: std::sync::OnceLock<std::result::Result<regex::Regex, regex::Error>> =
        std::sync::OnceLock::new();
    let troubleshooting = TROUBLESHOOTING
        .get_or_init(|| {
            regex::Regex::new(r"(?i)\b(error|fail(ed|ing)?|broken|not working|crash|issue|bug|fix|doesn'?t work|stopped working)\b")
        })
        .as_ref()
        .map_err(|e| Error::Other(Box::new(e.clone())))?;
    let step_structure = STEP_STRUCTURE
        .get_or_init(|| {
            regex::Regex::new(r"(?is)(\n\s*\d+[.)]\s|\bstep\s*\d|\bfirst\b.*\bthen\b|\n\s*[-*]\s)")
        })
        .as_ref()
        .map_err(|e| Error::Other(Box::new(e.clone())))?;

    let mut created = 0;
    let questions = repeated_questions(conn, days);

    for q in &questions {
        let hits = knowledge_hits(conn, &q.question);
        if hits == 0 {
            let (first_seen, last_seen) = seen_bounds(conn, &q.conversation_ids);
            let newly = insert_gap_candidate(
                conn,
                "repeated_question_uncovered",
                &q.question,
                q.count,
                &q.conversation_ids,
                &[],
                json!({
                    "explanation": format!("\"{}\" was asked in {} conversations and produced zero knowledge-base hits.",
                        cap_chars(&q.question, 120), q.count),
                    "method": "Repeated primary questions (latest ticket_analysis runs) matched against the local FTS knowledge index; 0 hits.",
                    "first_seen": first_seen,
                    "last_seen": last_seen
                }),
            );
            created += usize::from(newly);
            continue;
        }
        let friction_count = friction_among(conn, &q.conversation_ids);
        let threshold = ((q.count as f64) / 2.0).ceil().max(1.0) as usize;
        if friction_count >= threshold {
            let (first_seen, last_seen) = seen_bounds(conn, &q.conversation_ids);
            let newly = insert_gap_candidate(
                conn,
                "repeated_question_unsolved",
                &q.question,
                q.count,
                &q.conversation_ids,
                &[],
                json!({
                    "explanation": format!("\"{}\" has {} knowledge hit(s), yet {} of {} asking conversations still showed clarification or follow-up friction - the existing answer did not resolve it.",
                        cap_chars(&q.question, 120), hits, friction_count, q.count),
                    "method": "Cross of repeated questions with the interaction engine clarification/follow-up counts over the same conversations; association, not causation.",
                    "first_seen": first_seen,
                    "last_seen": last_seen
                }),
            );
            created += usize::from(newly);
        }
    }

    let docs: Vec<(i64, String)> =
        crate::db::query_rows(conn, "SELECT id, title FROM knowledge_documents", [], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))
        })
        .unwrap_or_default();
    let tokens = |title: &str| -> std::collections::HashSet<String> {
        title
            .to_lowercase()
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { ' ' })
            .collect::<String>()
            .split_whitespace()
            .filter(|t| t.chars().count() > 2)
            .map(String::from)
            .collect()
    };
    let mut reported_pairs = std::collections::HashSet::new();
    for (i, (a_id, a_title)) in docs.iter().take(500).enumerate() {
        for (b_id, b_title) in docs.iter().take(500).skip(i + 1) {
            let ta = tokens(a_title);
            let tb = tokens(b_title);
            let shared: Vec<&String> = tb.iter().filter(|t| ta.contains(*t)).collect();
            if shared.len() >= 3 {
                let pair_key = format!("{}:{}", (*a_id).min(*b_id), (*a_id).max(*b_id));
                if !reported_pairs.insert(pair_key) {
                    continue;
                }
                let newly = insert_gap_candidate(
                    conn,
                    "conflicting_knowledge",
                    &format!("{a_title} / {b_title}"),
                    shared.len(),
                    &[],
                    &[*a_id, *b_id],
                    json!({
                        "explanation": format!("Two documents share {} title tokens (\"{}\") and may cover overlapping or conflicting guidance.",
                            shared.len(), shared.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(" ")),
                        "conflicting_titles": [a_title, b_title],
                        "method": "Deterministic title-token overlap (>= 3 shared tokens) over local documents - the same shape as the freshness report. Overlap does not prove contradiction; a human review decides."
                    }),
                );
                created += usize::from(newly);
            }
        }
    }

    for q in &questions {
        if !troubleshooting.is_match(&q.question) {
            continue;
        }
        if let Some((doc_id, title, content)) = closest_document(conn, &q.question)
            && !step_structure.is_match(&content)
        {
            let (first_seen, last_seen) = seen_bounds(conn, &q.conversation_ids);
            let newly = insert_gap_candidate(
                conn,
                "missing_troubleshooting_steps",
                &q.question,
                q.count,
                &q.conversation_ids,
                &[doc_id],
                json!({
                    "explanation": format!("\"{}\" is troubleshooting-shaped, and the closest document (\"{}\") contains no step structure (no numbered steps, no step markers).",
                        cap_chars(&q.question, 120), title),
                    "closest_document_title": title,
                    "method": "Troubleshooting-shaped question detection + step-structure regex over the closest FTS-matching document. A heuristic; the document may simply use prose.",
                    "first_seen": first_seen,
                    "last_seen": last_seen
                }),
            );
            created += usize::from(newly);
        }
    }

    let cluster_rows: Vec<(i64, String, i64)> = crate::db::query_rows(
        conn,
        "SELECT c.id, c.title,
                    (SELECT COUNT(*) FROM issue_cluster_conversations m WHERE m.cluster_id = c.id) AS n
             FROM issue_clusters c
             WHERE (SELECT COUNT(*) FROM issue_cluster_conversations m WHERE m.cluster_id = c.id) >= 3
             ORDER BY n DESC LIMIT 30",
        [],
        |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, Option<String>>(1)?.unwrap_or_default(),
                r.get::<_, i64>(2)?,
            ))
        },
    )
    .unwrap_or_default();
    for (id, title, count) in cluster_rows {
        if knowledge_hits(conn, &title) == 0 {
            let members: Vec<i64> = crate::db::query_rows(
                conn,
                "SELECT conversation_id FROM issue_cluster_conversations
                     WHERE cluster_id = ?1 ORDER BY conversation_id DESC LIMIT 50",
                params![id],
                |r| r.get(0),
            )
            .unwrap_or_default();
            let (first_seen, last_seen) = seen_bounds(conn, &members);
            let newly = insert_gap_candidate(
                conn,
                "new_issue_undocumented",
                &title,
                usize::try_from(count).unwrap_or(0),
                &members,
                &[],
                json!({
                    "explanation": format!("Issue \"{}\" groups {} conversations and has no covering documentation.",
                        cap_chars(&title, 120), count),
                    "issue_label": title,
                    "method": "Issue clusters (>= 3 conversations) matched against the local FTS knowledge index; 0 hits.",
                    "first_seen": first_seen,
                    "last_seen": last_seen
                }),
            );
            created += usize::from(newly);
        }
    }

    let total: usize = conn
        .query_row("SELECT COUNT(*) FROM knowledge_gap_candidates", [], |r| {
            r.get::<_, i64>(0)
        })
        .map(|n| usize::try_from(n).unwrap_or(0))
        .unwrap_or(0);
    Ok((total, created))
}

#[allow(clippy::too_many_arguments)]
fn gap_candidate_json(
    id: i64,
    kind: Option<String>,
    query_text: String,
    occurrence_count: i64,
    evidence_ids: String,
    doc_ids: String,
    detail: Option<String>,
    status: String,
    decided_at: Option<String>,
    decision_note: Option<String>,
    created_at: String,
    updated_at: String,
) -> Value {
    let mut detail_map = serde_json::Map::new();
    detail_map.insert("explanation".into(), json!(""));
    detail_map.insert("method".into(), json!(""));
    if let Some(d) = detail.as_deref()
        && let Ok(parsed) = serde_json::from_str::<Value>(d)
        && let Some(map) = parsed.as_object()
    {
        for (k, v) in map {
            detail_map.insert(k.clone(), v.clone());
        }
    }
    let detail_obj = Value::Object(detail_map);
    json!({
        "id": id,
        "kind": kind,
        "question": query_text,
        "occurrence_count": occurrence_count,
        "evidence_conversation_ids": serde_json::from_str::<Value>(&evidence_ids)
            .unwrap_or_else(|_| json!([])),
        "related_document_ids": serde_json::from_str::<Value>(&doc_ids)
            .unwrap_or_else(|_| json!([])),
        "detail": detail_obj,
        "status": status,
        "decided_at": decided_at,
        "decision_note": decision_note,
        "created_at": created_at,
        "updated_at": updated_at,
    })
}

const GAP_COLS: &str = "id, kind, query_text, occurrence_count, evidence_conversation_ids, \
                        related_document_ids, detail, status, decided_at, decision_note, \
                        created_at, updated_at";

pub(crate) fn gap_by_id(conn: &Connection, id: i64) -> Option<Value> {
    conn.query_row(
        &format!("SELECT {GAP_COLS} FROM knowledge_gap_candidates WHERE id = ?1"),
        params![id],
        |r| {
            Ok(gap_candidate_json(
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
                r.get(6)?,
                r.get(7)?,
                r.get(8)?,
                r.get(9)?,
                r.get(10)?,
                r.get(11)?,
            ))
        },
    )
    .ok()
}

pub fn gap_report(conn: &Connection) -> Value {
    let rows: Vec<Value> = crate::db::query_rows(
        conn,
        "SELECT id, kind, query_text, occurrence_count, evidence_conversation_ids,
                related_document_ids, detail, status, decided_at, decision_note,
                created_at, updated_at
         FROM knowledge_gap_candidates
         ORDER BY CASE status WHEN 'open' THEN 0 WHEN 'approved' THEN 1 ELSE 2 END,
                  occurrence_count DESC, updated_at DESC
         LIMIT 500",
        [],
        |r| {
            Ok(gap_candidate_json(
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
                r.get(6)?,
                r.get(7)?,
                r.get(8)?,
                r.get(9)?,
                r.get(10)?,
                r.get(11)?,
            ))
        },
    )
    .unwrap_or_default();
    let totals: (i64, i64, i64) = conn
        .query_row(
            "SELECT (SELECT COUNT(*) FROM knowledge_gap_candidates WHERE status = 'open'),
                    (SELECT COUNT(*) FROM knowledge_gap_candidates WHERE status = 'approved'),
                    (SELECT COUNT(*) FROM knowledge_gap_candidates WHERE status = 'rejected')",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap_or((0, 0, 0));
    let kinds: Vec<Value> = KNOWLEDGE_GAP_KINDS
        .iter()
        .map(|(kind, label)| {
            json!({
                "kind": kind,
                "label": label,
                "candidates": rows.iter().filter(|r| r["kind"].as_str() == Some(*kind)).cloned().collect::<Vec<_>>(),
            })
        })
        .collect();
    json!({
        "generated_at": now_iso(),
        "kinds": kinds,
        "totals": { "candidates": totals.0, "approved": totals.1, "rejected": totals.2 },
        "notes": [
            "Candidates are deterministic detections; a human decides. Approving marks the candidate only - nothing is published automatically.",
            "Run a rebuild after new syncs or analyses to refresh evidence; approve/reject decisions survive rebuilds."
        ]
    })
}

pub fn decide_gap(
    conn: &Connection,
    candidate_id: i64,
    decision: &str,
    note: Option<&str>,
    user_local_id: Option<i64>,
) -> Option<Value> {
    let changed = conn
        .execute(
            "UPDATE knowledge_gap_candidates
             SET status = ?2, decided_at = datetime('now'), decision_note = ?3,
                 decided_by_user_local_id = ?4, updated_at = datetime('now')
             WHERE id = ?1 AND status = 'open'",
            params![candidate_id, decision, note, user_local_id],
        )
        .unwrap_or(0);
    if changed == 0 {
        return None;
    }
    gap_by_id(conn, candidate_id)
}

const MIN_REPEAT_WORDS: usize = 6;

type AgentQuestion = (i64, Option<String>, String);

fn loop_problem_re() -> Result<&'static regex::Regex> {
    static RE: std::sync::OnceLock<std::result::Result<regex::Regex, regex::Error>> =
        std::sync::OnceLock::new();
    RE.get_or_init(|| {
        regex::Regex::new(
            r"(?i)still (not|doesn'?t|no)|same (issue|problem|error)|happens again|again (the|it)|persist|not fixed|didn'?t (work|help|fix)|problem (is|persists|continues)|error (persists|continues|still)",
        )
    })
    .as_ref()
    .map_err(|e| Error::Other(Box::new(e.clone())))
}

fn direct_complaint_re() -> Result<&'static regex::Regex> {
    static RE: std::sync::OnceLock<std::result::Result<regex::Regex, regex::Error>> =
        std::sync::OnceLock::new();
    RE.get_or_init(|| {
        regex::Regex::new(
            r"(?i)(as|per) i (already |previously )?(said|mentioned|sent|wrote|provided|explained)|i already (sent|gave|provided|mentioned|told)|re-?sending|again[:,] |already (shared|attached)",
        )
    })
    .as_ref()
    .map_err(|e| Error::Other(Box::new(e.clone())))
}

type Finding = (String, String, Vec<Value>, String);

fn finding(kind: &str, severity: &str, evidence: Vec<Value>, detail: String) -> Finding {
    (kind.to_string(), severity.to_string(), evidence, detail)
}

pub(crate) fn analyze_friction(conn: &Connection, conversation_id: i64) -> Result<Vec<Value>> {
    let conv: Option<(i64, Option<i64>)> = conn
        .query_row(
            "SELECT number, customer_local_id FROM conversations
             WHERE id = ?1 AND deleted_at IS NULL",
            params![conversation_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .ok();
    let Some((number, customer_local_id)) = conv else {
        return Ok(Vec::new());
    };
    let threads = published_threads(conn, conversation_id);
    let customer: Vec<&ThreadLite> = threads.iter().filter(|t| t.kind == "customer").collect();
    let agents: Vec<&ThreadLite> = threads.iter().filter(|t| t.kind == "reply").collect();

    let now = now_iso();
    let mut findings: Vec<Finding> = Vec::new();
    findings.extend(repeated_customer_explanations_findings(&customer));
    findings.extend(repeated_agent_questions_findings(&agents));
    findings.extend(troubleshooting_loop_findings(&threads)?);
    findings.extend(repeated_handoff_findings(conn, conversation_id));
    findings.extend(repeated_unresolved_findings(
        conn,
        customer_local_id,
        conversation_id,
    ));
    findings.extend(duplicated_request_findings(&threads, &customer)?);

    for (kind, severity, evidence, detail) in &findings {
        let _ = conn.execute(
            "INSERT INTO friction_findings (conversation_id, customer_local_id, kind, severity, evidence, detail, computed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, datetime('now'))
             ON CONFLICT (conversation_id, kind) DO UPDATE SET
                 customer_local_id = excluded.customer_local_id,
                 severity = excluded.severity,
                 evidence = excluded.evidence,
                 detail = excluded.detail,
                 computed_at = datetime('now')",
            params![
                conversation_id,
                customer_local_id,
                kind,
                severity,
                serde_json::to_string(evidence).unwrap_or_else(|_| "[]".into()),
                detail
            ],
        );
    }
    if findings.is_empty() {
        let _ = conn.execute(
            "DELETE FROM friction_findings WHERE conversation_id = ?1",
            params![conversation_id],
        );
    } else {
        let kinds: Vec<&str> = findings.iter().map(|(k, _, _, _)| k.as_str()).collect();
        let placeholders = vec!["?"; kinds.len()].join(",");
        let sql = format!(
            "DELETE FROM friction_findings WHERE conversation_id = ?1 AND kind NOT IN ({placeholders})"
        );
        let mut params_vec: Vec<&dyn rusqlite::ToSql> = vec![&conversation_id];
        for k in &kinds {
            params_vec.push(k);
        }
        let _ = conn.execute(&sql, params_vec.as_slice());
    }

    Ok(findings
        .into_iter()
        .map(|(kind, severity, evidence, detail)| {
            json!({
                "conversation_id": conversation_id,
                "conversation_number": number,
                "customer_local_id": customer_local_id,
                "kind": kind,
                "severity": severity,
                "evidence": evidence,
                "detail": detail,
                "computed_at": now,
            })
        })
        .collect())
}

fn repeated_customer_explanations_findings(customer: &[&ThreadLite]) -> Vec<Finding> {
    let mut out: Vec<Finding> = Vec::new();
    let customer_repeats = repeated_spans(customer, MIN_REPEAT_WORDS);
    if !customer_repeats.is_empty() {
        let mut sorted = customer_repeats;
        sorted.sort_by_key(|s| std::cmp::Reverse(s.thread_ids.len()));
        let best = &sorted[0];
        let evidence: Vec<Value> = best
            .thread_ids
            .iter()
            .filter_map(|tid| customer.iter().find(|m| m.thread_id == *tid))
            .map(|m| {
                json!({
                    "thread_id": m.thread_id, "author_type": "customer",
                    "excerpt": excerpt_of(&m.text, 160), "at": m.at
                })
            })
            .collect();
        out.push(finding(
            "repeated_customer_explanations",
            if best.thread_ids.len() >= 3 {
                "high"
            } else {
                "moderate"
            },
            evidence,
            format!(
                "The customer re-stated the same {MIN_REPEAT_WORDS}+ word span across {} messages (\"{}\"). Detected by repeated-span matching across customer messages; a heuristic, not a judgment.",
                best.thread_ids.len(),
                excerpt_of(&best.span, 80)
            ),
        ));
    }
    out
}

fn repeated_agent_questions_findings(agents: &[&ThreadLite]) -> Vec<Finding> {
    let mut out: Vec<Finding> = Vec::new();
    let mut question_groups: Vec<(String, Vec<AgentQuestion>)> = Vec::new();
    let mut group_index: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    for m in agents {
        for q in split_after_question(&m.text) {
            let q = q.trim();
            if !q.ends_with('?') {
                continue;
            }
            let norm = normalize_span(q);
            if norm.len() < 4 {
                continue;
            }
            let key = norm.join(" ");
            match group_index.get(&key) {
                Some(&i) => {
                    if !question_groups[i]
                        .1
                        .iter()
                        .any(|(tid, _, _)| *tid == m.thread_id)
                    {
                        question_groups[i]
                            .1
                            .push((m.thread_id, m.at.clone(), q.to_string()));
                    }
                }
                None => {
                    group_index.insert(key.clone(), question_groups.len());
                    question_groups.push((key, vec![(m.thread_id, m.at.clone(), q.to_string())]));
                }
            }
        }
    }
    if let Some((_, group)) = question_groups.into_iter().find(|(_, l)| l.len() >= 2) {
        let evidence: Vec<Value> = group
            .iter()
            .map(|(tid, at, q)| {
                json!({
                    "thread_id": tid, "author_type": "agent",
                    "excerpt": excerpt_of(q, 160), "at": at
                })
            })
            .collect();
        out.push(finding(
            "repeated_agent_questions",
            if group.len() >= 3 { "high" } else { "moderate" },
            evidence,
            format!(
                "The agent asked the same question in {} replies (\"{}\"). Detected by normalized question-sentence matching; a heuristic.",
                group.len(),
                excerpt_of(
                    group
                        .first()
                        .map(|(_, _, q)| q.as_str())
                        .unwrap_or_default(),
                    80
                )
            ),
        ));
    }
    out
}

fn troubleshooting_loop_findings(threads: &[ThreadLite]) -> Result<Vec<Finding>> {
    let mut out: Vec<Finding> = Vec::new();
    let mut saw_agent_reply = false;
    let mut loop_restatements: Vec<(i64, String, Option<String>)> = Vec::new();
    for t in threads {
        if t.kind == "reply" {
            saw_agent_reply = true;
        } else if t.kind == "customer"
            && saw_agent_reply
            && loop_problem_re()?.is_match(&t.text)
            && !is_closing_acknowledgment(&t.text)?
        {
            loop_restatements.push((t.thread_id, t.text.clone(), t.at.clone()));
        }
    }
    if loop_restatements.len() >= 2 {
        let evidence: Vec<Value> = loop_restatements
            .iter()
            .map(|(tid, text, at)| {
                json!({
                    "thread_id": tid, "author_type": "customer",
                    "excerpt": excerpt_of(text, 160), "at": at
                })
            })
            .collect();
        out.push(finding(
            "troubleshooting_loop",
            if loop_restatements.len() >= 3 {
                "high"
            } else {
                "moderate"
            },
            evidence,
            format!(
                "After agent replies, the customer restated the unresolved problem {} times (issue-shaped language after an agent reply). Detected by problem-restatement patterns; a heuristic.",
                loop_restatements.len()
            ),
        ));
    }
    Ok(out)
}

fn repeated_handoff_findings(conn: &Connection, conversation_id: i64) -> Vec<Finding> {
    let mut out: Vec<Finding> = Vec::new();
    let handoffs: Vec<(Option<String>, Option<String>)> = crate::db::query_rows(
        conn,
        "SELECT occurred_at, metadata FROM activity_events
             WHERE conversation_id = ?1 AND event_type = 'assignment_changed'
             ORDER BY occurred_at ASC",
        params![conversation_id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )
    .unwrap_or_default();
    let history_complete: bool = conn
        .query_row(
            "SELECT activity_history_complete FROM conversations WHERE id = ?1",
            params![conversation_id],
            |r| r.get::<_, i64>(0),
        )
        .map(|v| v == 1)
        .unwrap_or(false);
    if handoffs.len() >= 3 {
        let evidence: Vec<Value> = handoffs
            .iter()
            .take(6)
            .map(|(at, metadata)| {
                json!({
                    "thread_id": 0, "author_type": "agent",
                    "excerpt": match metadata.as_deref() {
                        Some(m) if !m.is_empty() =>
                            format!("Assignment change: {}", excerpt_of(m, 100)),
                        _ => "Assignment change".to_string(),
                    },
                    "at": at
                })
            })
            .collect();
        out.push(finding(
            "repeated_handoffs",
            if handoffs.len() >= 4 {
                "high"
            } else {
                "moderate"
            },
            evidence,
            format!(
                "{} assignment changes observed in the local event history{}. Detected deterministically from recorded conversation events.",
                handoffs.len(),
                if history_complete {
                    ""
                } else {
                    " (pre-sync history is unknown - Help Scout exposes no historical event log)"
                }
            ),
        ));
    }
    out
}

fn repeated_unresolved_findings(
    conn: &Connection,
    customer_local_id: Option<i64>,
    conversation_id: i64,
) -> Vec<Finding> {
    let mut out: Vec<Finding> = Vec::new();
    if let Some(customer_local) = customer_local_id {
        let related: Vec<RelatedConversation> = crate::db::query_rows(
            conn,
            "SELECT c2.id, c2.number, c2.subject, c2.status,
                    COALESCE(c2.remote_created_at, c2.created_at)
                 FROM conversations c2
                 WHERE c2.customer_local_id = ?1 AND c2.deleted_at IS NULL AND c2.id <> ?2
                   AND COALESCE(c2.remote_created_at, c2.created_at) >= strftime('%Y-%m-%dT%H:%M:%fZ','now','-90 days')
                   AND EXISTS (SELECT 1 FROM conversation_tags ct1
                               JOIN conversation_tags ct2 ON ct2.conversation_id = c2.id
                               JOIN tags tg ON tg.id = ct1.tag_id AND tg.id = ct2.tag_id
                               WHERE ct1.conversation_id = ?2)
                 ORDER BY c2.created_at DESC LIMIT 10",
            params![customer_local, conversation_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )
        .unwrap_or_default();
        if related.len() >= 2 {
            let shared_tag: Option<String> = conn
                .query_row(
                    "SELECT tg.name FROM tags tg
                     WHERE EXISTS (SELECT 1 FROM conversation_tags ct1
                                    WHERE ct1.conversation_id = ?1
                                      AND ct1.tag_id = tg.id)
                       AND EXISTS (SELECT 1 FROM conversation_tags ct2
                                     JOIN conversations c2 ON c2.id = ct2.conversation_id
                                    WHERE ct2.tag_id = tg.id
                                      AND c2.customer_local_id = ?2
                                      AND c2.deleted_at IS NULL
                                      AND c2.id <> ?1
                                      AND COALESCE(c2.remote_created_at, c2.created_at)
                                          >= strftime('%Y-%m-%dT%H:%M:%fZ','now','-90 days'))
                     ORDER BY tg.name LIMIT 1",
                    params![conversation_id, customer_local],
                    |r| r.get(0),
                )
                .ok();
            let evidence: Vec<Value> = related
                .iter()
                .take(4)
                .map(|(_, number, subject, status, at)| {
                    json!({
                        "thread_id": 0, "author_type": "customer",
                        "excerpt": format!("Conversation #{number}: {} ({status})",
                            subject.as_deref().unwrap_or(crate::catalog::NO_SUBJECT)),
                        "at": at
                    })
                })
                .collect();
            out.push(finding(
                "repeated_unresolved_interactions",
                if related.len() >= 3 {
                    "high"
                } else {
                    "moderate"
                },
                evidence,
                format!(
                    "The customer opened {} conversations in the last 90 days sharing the tag{} - the same problem keeps returning. Customer-level pattern detected from the local mirror; association, not causation.",
                    related.len() + 1,
                    shared_tag
                        .as_deref()
                        .map(|t| format!(" \"{t}\""))
                        .unwrap_or_default()
                ),
            ));
        }
    }
    out
}

fn duplicated_request_findings(
    threads: &[ThreadLite],
    customer: &[&ThreadLite],
) -> Result<Vec<Finding>> {
    let mut out: Vec<Finding> = Vec::new();
    let mut dup_requests: Vec<(i64, String, Option<String>)> = Vec::new();
    let mut supplied = (false, false, false);
    let digits_re = rx(r"\b\d{4,}\b")?;
    let email_re = rx(r"[\w.+-]+@[\w-]+\.[\w.]+")?;
    let error_re = rx(r"(?i)error|fail|exception|crash")?;
    let ask_pattern =
        rx(r"(?i)\?|could you (send|share|provide|confirm)|please (send|share|provide|confirm)")?;
    let order_re =
        rx(r"(?i)\border\b|\binvoice\b|account\s*(id|number)?|reference\b|ticket number")?;
    let email_ask_re = rx(r"(?i)email")?;
    let error_ask_re = rx(r"(?i)error|log|trace|screenshot")?;
    for t in threads {
        if t.kind == "customer" {
            if digits_re.is_match(&t.text) {
                supplied.0 = true;
            }
            if email_re.is_match(&t.text) {
                supplied.1 = true;
            }
            if error_re.is_match(&t.text) {
                supplied.2 = true;
            }
        } else if t.kind == "reply" {
            let asks_category = ask_pattern.is_match(&t.text)
                && (if order_re.is_match(&t.text) {
                    supplied.0
                } else if email_ask_re.is_match(&t.text) {
                    supplied.1
                } else if error_ask_re.is_match(&t.text) {
                    supplied.2
                } else {
                    false
                });
            if asks_category {
                dup_requests.push((t.thread_id, t.text.clone(), t.at.clone()));
            }
        }
    }
    let complaint_re = direct_complaint_re()?;
    let direct_complaints: Vec<&ThreadLite> = customer
        .iter()
        .copied()
        .filter(|m| complaint_re.is_match(&m.text))
        .collect();
    if !dup_requests.is_empty() || !direct_complaints.is_empty() {
        let mut evidence: Vec<Value> = Vec::new();
        for (tid, text, at) in dup_requests.iter().take(3) {
            evidence.push(json!({
                "thread_id": tid, "author_type": "agent",
                "excerpt": excerpt_of(text, 160), "at": at
            }));
        }
        for c in direct_complaints.iter().take(3) {
            evidence.push(json!({
                "thread_id": c.thread_id, "author_type": "customer",
                "excerpt": excerpt_of(&c.text, 160), "at": c.at
            }));
        }
        let total_evidence = dup_requests.len() + direct_complaints.len();
        out.push(finding(
            "duplicated_information_requests",
            if total_evidence >= 3 { "high" } else { "low" },
            evidence,
            format!(
                "The agent asked for information (order-style ids, emails, dates, error codes) the customer had already supplied,{}. Detected by entity-overlap and direct-phrase matching; a heuristic.",
                if !direct_complaints.is_empty() {
                    format!(
                        " and/or the customer said they were re-sending it ({} direct mentions)",
                        direct_complaints.len()
                    )
                } else {
                    String::new()
                }
            ),
        ));
    }
    Ok(out)
}

pub(crate) fn rebuild_friction(conn: &Connection) -> Result<(usize, usize)> {
    let ids: Vec<i64> = crate::db::query_rows(
        conn,
        "SELECT DISTINCT c.id FROM conversations c
             JOIN conversation_threads t ON t.conversation_id = c.id AND t.type = 'customer'
             WHERE c.deleted_at IS NULL",
        [],
        |r| r.get(0),
    )
    .unwrap_or_default();
    let mut findings = 0usize;
    for id in &ids {
        findings += analyze_friction(conn, *id)?.len();
    }
    Ok((ids.len(), findings))
}

const QA_PROMPT_VERSION: &str = "post_resolution_qa_v1";

pub(crate) struct QaAiOutcome {
    pub error: Option<String>,
}

fn parse_stamp(s: &str) -> Option<chrono::DateTime<chrono::Utc>> {
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(s) {
        return Some(dt.with_timezone(&chrono::Utc));
    }
    chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S")
        .ok()
        .map(|naive| naive.and_utc())
}

fn minutes_between(later: &str, earlier: &str) -> Option<i64> {
    let l = parse_stamp(later)?;
    let e = parse_stamp(earlier)?;
    let minutes = (l - e).num_milliseconds() as f64 / 60_000.0;
    let rounded = minutes.round() as i64;
    (rounded >= 0).then_some(rounded)
}

type RelatedConversation = (i64, i64, Option<String>, String, Option<String>);

pub(crate) fn compute_qa_deterministic(
    conn: &Connection,
    conversation_id: i64,
) -> Result<Option<Value>> {
    type QaConversationRow = (String, Option<String>, Option<String>, Option<String>, i64);
    let conv: Option<QaConversationRow> = conn
        .query_row(
            "SELECT status, closed_at, COALESCE(remote_created_at, created_at),
                    first_response_at, activity_history_complete
             FROM conversations WHERE id = ?1 AND deleted_at IS NULL",
            params![conversation_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )
        .ok();
    let Some((status, closed_at, created_at, first_response_at, history_complete)) = conv else {
        return Ok(None);
    };
    let threads = published_threads(conn, conversation_id);
    let customer: Vec<&ThreadLite> = threads.iter().filter(|t| t.kind == "customer").collect();
    let replies: Vec<&ThreadLite> = threads.iter().filter(|t| t.kind == "reply").collect();

    let mut saw_reply = false;
    let mut back_and_forth: u32 = 0;
    for t in &threads {
        if t.kind == "reply" {
            saw_reply = true;
        } else if t.kind == "customer" && saw_reply && !is_closing_acknowledgment(&t.text)? {
            back_and_forth += 1;
        }
    }

    let mut repeated: Vec<Value> = Vec::new();
    let mut seen_spans = std::collections::HashSet::new();
    let normalized_customer: Vec<(i64, Vec<String>, String)> = customer
        .iter()
        .map(|m| {
            let words = normalize_span(&m.text);
            let joined = words.join(" ");
            (m.thread_id, words, joined)
        })
        .collect();
    for i in 0..normalized_customer.len() {
        let (tid, words, _) = &normalized_customer[i];
        if words.len() < 6 {
            continue;
        }
        for start in 0..=(words.len() - 6) {
            let span = words[start..start + 6].join(" ");
            if !seen_spans.insert(span.clone()) {
                continue;
            }
            for (_, _, joined_k) in normalized_customer.iter().skip(i + 1) {
                if joined_k.contains(&span) {
                    repeated.push(json!({
                        "thread_id": tid,
                        "excerpt": cap_chars(&span, 120)
                    }));
                    break;
                }
            }
        }
    }
    let repeated_count = repeated.len();
    let repeated_evidence: Vec<Value> = repeated.into_iter().take(5).collect();

    let messages_after_close: u32 = closed_at.as_deref().map_or(0, |closed| {
        customer
            .iter()
            .filter(|m| {
                m.at.as_deref()
                    .is_some_and(|at| !at.is_empty() && string_gt(at, closed))
            })
            .count() as u32
    });

    let handoffs: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM activity_events
             WHERE conversation_id = ?1 AND event_type = 'assignment_changed'",
            params![conversation_id],
            |r| r.get(0),
        )
        .unwrap_or(0);

    let question_count: u32 = customer
        .iter()
        .map(|m| m.text.matches('?').count() as u32)
        .sum();

    let first_reply_at = replies.first().and_then(|r| r.at.clone());
    let first_response_stamp = first_response_at.or(first_reply_at);
    let first_response_minutes = first_response_stamp
        .as_deref()
        .zip(created_at.as_deref())
        .and_then(|(later, earlier)| minutes_between(later, earlier));
    let resolution_minutes = closed_at
        .as_deref()
        .zip(created_at.as_deref())
        .and_then(|(later, earlier)| minutes_between(later, earlier));

    let history_complete = history_complete != 0;
    let deterministic = json!({
        "conversation_local_id": conversation_id,
        "closed": status == "closed",
        "back_and_forth_count": back_and_forth,
        "repeated_information_count": repeated_count,
        "repeated_information_evidence": repeated_evidence,
        "messages_after_close": messages_after_close,
        "handoff_count": handoffs,
        "handoff_history_complete": history_complete,
        "customer_question_count": question_count,
        "agent_reply_count": replies.len(),
        "first_response_minutes": first_response_minutes,
        "resolution_minutes": resolution_minutes,
        "computed_honestly": [
            "Counts derive from the locally mirrored thread list; pre-sync edits are not reconstructable.",
            "Repeated information is a repeated 6-word-span heuristic, not semantic understanding.",
            format!("Handoff counts cover locally recorded events only{}",
                if history_complete { "" } else { " (pre-sync history unknown)" })
        ]
    });
    let updated = conn.execute(
        "UPDATE post_resolution_qa SET deterministic = ?2, computed_at = datetime('now')
         WHERE conversation_id = ?1",
        params![conversation_id, deterministic.to_string()],
    );
    if updated.unwrap_or(0) == 0 {
        let _ = conn.execute(
            "INSERT INTO post_resolution_qa (conversation_id, deterministic, computed_at)
             VALUES (?1, ?2, datetime('now'))",
            params![conversation_id, deterministic.to_string()],
        );
    }
    Ok(Some(deterministic))
}

fn string_gt(a: &str, b: &str) -> bool {
    a.as_bytes() > b.as_bytes()
}

struct QaRow {
    deterministic: String,
    ai: Option<String>,
    computed_at: Option<String>,
    checked_at: Option<String>,
    recomputed_at: Option<String>,
}

fn qa_row(conn: &Connection, conversation_id: i64) -> Option<QaRow> {
    conn.query_row(
        "SELECT deterministic, ai, computed_at, checked_at, recomputed_at
         FROM post_resolution_qa WHERE conversation_id = ?1",
        params![conversation_id],
        |r| {
            Ok(QaRow {
                deterministic: r.get(0)?,
                ai: r.get(1)?,
                computed_at: r.get(2)?,
                checked_at: r.get(3)?,
                recomputed_at: r.get(4)?,
            })
        },
    )
    .ok()
}

pub fn get_qa(conn: &Connection, conversation_id: i64, ai_available: bool) -> Option<Value> {
    let row = match qa_row(conn, conversation_id) {
        Some(row) => row,
        None => {
            compute_qa_deterministic(conn, conversation_id).ok()?;
            qa_row(conn, conversation_id)?
        }
    };
    Some(json!({
        "conversation_id": conversation_id,
        "deterministic": serde_json::from_str::<Value>(&row.deterministic)
            .unwrap_or_else(|_| json!({})),
        "ai": row.ai.as_deref()
            .and_then(|a| serde_json::from_str::<Value>(a).ok()),
        "ai_available": ai_available,
        "computed_at": row.computed_at.or(row.checked_at),
        "recomputed_at": row.recomputed_at,
    }))
}

pub(crate) fn rebuild_qa(conn: &Connection) -> Result<usize> {
    let ids: Vec<i64> = crate::db::query_rows(
        conn,
        "SELECT id FROM conversations WHERE deleted_at IS NULL AND status = 'closed'",
        [],
        |r| r.get(0),
    )
    .unwrap_or_default();
    let count = ids.len();
    for id in ids {
        compute_qa_deterministic(conn, id)?;
    }
    Ok(count)
}

pub fn qa_overview(conn: &Connection) -> Value {
    let closed: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM conversations WHERE deleted_at IS NULL AND status = 'closed'",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);
    let (qa_rows, with_ai): (i64, i64) = conn
        .query_row(
            "SELECT COUNT(*), COALESCE(SUM(CASE WHEN ai IS NOT NULL THEN 1 ELSE 0 END), 0)
             FROM post_resolution_qa",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap_or((0, 0));
    json!({
        "closed_conversations": closed,
        "qa_rows": qa_rows,
        "with_ai_layer": with_ai,
    })
}

#[cfg(test)]
mod batch13_k1_tests;
#[cfg(test)]
mod batch18_tests;
#[cfg(test)]
mod batch24_audit52_tests;
#[cfg(test)]
mod k2_tests;

#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct ResolutionCitation {
    pub conversation_local_id: i64,
    pub number: i64,
    pub subject: Option<String>,
    pub closed_at: Option<String>,
    pub customer_question: String,
    pub excerpt: String,
}

pub(crate) fn best_resolutions(
    conn: &Connection,
    conversation_ids: &[i64],
    limit: usize,
) -> Vec<ResolutionCitation> {
    if conversation_ids.is_empty() || limit == 0 {
        return Vec::new();
    }
    let placeholders = vec!["?"; conversation_ids.len()].join(",");
    let rows: Vec<(i64, i64, Option<String>, Option<String>)> = crate::db::query_rows(
        conn,
        &format!(
            "SELECT c.id, c.number, c.subject, c.closed_at
               FROM conversations c
              WHERE c.id IN ({placeholders}) AND c.deleted_at IS NULL
                AND c.status = 'closed' AND c.closed_at IS NOT NULL
              ORDER BY c.closed_at DESC"
        ),
        rusqlite::params_from_iter(conversation_ids.iter()),
        |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, Option<String>>(3)?,
            ))
        },
    )
    .unwrap_or_default();
    let mut out: Vec<ResolutionCitation> = Vec::new();
    for (id, number, subject, closed_at) in rows {
        if out.len() >= limit {
            break;
        }
        let excerpt = resolution_excerpt_of(conn, id);
        if excerpt.is_empty() {
            continue;
        }
        let customer_question = customer_question_of(conn, id);
        out.push(ResolutionCitation {
            conversation_local_id: id,
            number,
            subject,
            closed_at,
            customer_question,
            excerpt,
        });
    }
    out
}

fn resolution_excerpt_of(conn: &Connection, conversation_id: i64) -> String {
    let raw: Option<(Option<String>, Option<String>)> = conn
        .query_row(
            "SELECT body_text, body_html
               FROM conversation_threads
              WHERE conversation_id = ?1 AND type = 'reply' AND state = 'published'
                AND deleted_at IS NULL AND COALESCE(body_text, body_html, '') <> ''
              ORDER BY COALESCE(remote_created_at, created_at) DESC, id DESC
              LIMIT 1",
            params![conversation_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .ok();
    let Some((body_text, body_html)) = raw else {
        return String::new();
    };
    let body = body_text
        .filter(|b| !b.trim().is_empty())
        .or_else(|| {
            body_html
                .map(|h| crate::textutil::html_to_text(&h))
                .filter(|t| !t.trim().is_empty())
        })
        .unwrap_or_default();
    excerpt_of(body.trim(), 900)
}

fn customer_question_of(conn: &Connection, conversation_id: i64) -> String {
    let raw: Option<(Option<String>, Option<String>)> = conn
        .query_row(
            "SELECT body_text, body_html
               FROM conversation_threads
              WHERE conversation_id = ?1
                AND type IN ('customer', 'customer_message')
                AND state = 'published' AND deleted_at IS NULL
                AND COALESCE(body_text, body_html, '') <> ''
              ORDER BY COALESCE(remote_created_at, created_at) ASC, id ASC
              LIMIT 1",
            params![conversation_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .ok();
    let Some((body_text, body_html)) = raw else {
        return String::new();
    };
    let body = body_text
        .filter(|b| !b.trim().is_empty())
        .or_else(|| {
            body_html
                .map(|h| crate::textutil::html_to_text(&h))
                .filter(|t| !t.trim().is_empty())
        })
        .unwrap_or_default();
    excerpt_of(body.trim(), 400)
}
