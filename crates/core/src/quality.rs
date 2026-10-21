use rusqlite::{Connection, params};
use serde_json::{Value, json};

use crate::error::Result;

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

pub fn ensure_quality_tables(conn: &Connection) -> Result<()> {
    conn.execute_batch(FRICTION_FINDINGS_SQL)?;
    add_column_if_missing(conn, "friction_findings", "detail", "TEXT")?;
    add_column_if_missing(conn, "post_resolution_qa", "computed_at", "TEXT")?;
    Ok(())
}

fn add_column_if_missing(conn: &Connection, table: &str, column: &str, decl: &str) -> Result<()> {
    let cols: Vec<String> = conn
        .prepare(&format!("PRAGMA table_info({table})"))?
        .query_map([], |r| r.get::<_, String>(1))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    if !cols.iter().any(|c| c == column) {
        let _ = conn.execute(
            &format!("ALTER TABLE {table} ADD COLUMN {column} {decl}"),
            [],
        );
    }
    Ok(())
}

pub(crate) struct ThreadLite {
    pub thread_id: i64,
    pub kind: String,
    pub text: String,
    pub at: Option<String>,
}

pub(crate) fn published_threads(conn: &Connection, conversation_local: i64) -> Vec<ThreadLite> {
    let Ok(mut stmt) = conn.prepare(
        "SELECT id, thread_type, html_stripped, at FROM (
             SELECT id, type AS thread_type, COALESCE(body_html, body_text, '') AS html_stripped,
                    COALESCE(remote_created_at, created_at) AS at
             FROM conversation_threads
             WHERE conversation_id = ?1 AND state = 'published' AND deleted_at IS NULL
         ) ORDER BY at ASC",
    ) else {
        return Vec::new();
    };
    stmt.query_map(params![conversation_local], |r| {
        Ok(ThreadLite {
            thread_id: r.get(0)?,
            kind: r.get(1)?,
            text: crate::textutil::html_to_text(&r.get::<_, String>(2)?),
            at: r.get(3)?,
        })
    })
    .map(|rows| rows.filter_map(|r| r.ok()).collect())
    .unwrap_or_default()
}

pub(crate) fn is_closing_acknowledgment(text: &str) -> bool {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    static START: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let re = RE.get_or_init(|| {
        regex::Regex::new(
            r"(?i)still|again|but\b|however|issue|problem|not work|broken|error|fail|doesn'?t|didn'?t|can'?t|cannot",
        )
        .expect("closing-ack exclusion regex")
    });
    let start = START.get_or_init(|| {
        regex::Regex::new(
            r"(?i)^(thanks|thank you|thankyou|thx|appreciate|that (is|was|'s|sounds|seems) (exactly |just |very )?(what i|great|perfect|helpful|awesome|amazing|clear)|perfect|great|works|working|resolved|closing|closed|all set|confirmed|done|sorted)",
        )
        .expect("closing-ack start regex")
    });
    let t = text.trim();
    if t.is_empty() || t.chars().count() > 400 || t.contains('?') {
        return false;
    }
    if re.is_match(t) {
        return false;
    }
    start.is_match(t)
}

fn now_iso() -> String {
    chrono::Utc::now()
        .format("%Y-%m-%dT%H:%M:%S%.3fZ")
        .to_string()
}

fn excerpt_of(text: &str, max: usize) -> String {
    let t: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if t.chars().count() <= max {
        t
    } else {
        let cut: String = t.chars().take(max).collect();
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

pub const KNOWLEDGE_GAP_KINDS: [(&str, &str); 5] = [
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

pub const FRICTION_KINDS: [(&str, &str); 6] = [
    (
        "repeated_customer_explanations",
        "Customer explained the same thing repeatedly",
    ),
    (
        "repeated_agent_questions",
        "The agent asked the same question repeatedly",
    ),
    ("troubleshooting_loop", "Unnecessary troubleshooting loop"),
    ("repeated_handoffs", "Repeated handoffs between agents"),
    (
        "repeated_unresolved_interactions",
        "Repeated unresolved interactions",
    ),
    (
        "duplicated_information_requests",
        "Agent re-requested information the customer already provided",
    ),
];

struct RepeatedQuestion {
    question: String,
    conversation_ids: Vec<i64>,
    count: usize,
}

fn repeated_questions(conn: &Connection, days: i64) -> Vec<RepeatedQuestion> {
    let days = days.clamp(1, 3650);
    let rows: Vec<(String, i64)> = {
        let Ok(mut stmt) = conn.prepare(
            "SELECT json_extract(response_json, '$.primary_question') AS question, conversation_id
             FROM ai_runs
             WHERE type = 'ticket_analysis' AND status = 'completed' AND conversation_id IS NOT NULL
               AND id IN (SELECT MAX(id) FROM ai_runs
                           WHERE type = 'ticket_analysis' AND status = 'completed'
                             AND conversation_id IS NOT NULL
                           GROUP BY conversation_id)
               AND json_extract(response_json, '$.primary_question') IS NOT NULL
               AND created_at >= datetime('now', '-' || ?1 || ' days')",
        ) else {
            return Vec::new();
        };
        stmt.query_map(params![days], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
        })
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_default()
    };
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
    // K1: the Help Scout Docs mirror counts as documentation too — a
    // question the help center already answers is not an
    // "asked often, documented nowhere" gap.
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
        "SELECT COUNT(*) FROM friction_scores f
         JOIN conversations c ON c.remote_id = f.conversation_id
         WHERE c.id IN ({placeholders})
           AND (json_extract(f.factors_json, '$.clarification_count') > 0
                OR json_extract(f.factors_json, '$.follow_up_count') > 0)"
    );
    let mut stmt = match conn.prepare(&sql) {
        Ok(s) => s,
        Err(_) => return 0,
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
    let normalized: String = question
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(300)
        .collect();
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

pub fn rebuild_gaps(conn: &Connection, days: i64) -> (usize, usize) {
    static TROUBLESHOOTING: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    static STEP_STRUCTURE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let troubleshooting = TROUBLESHOOTING.get_or_init(|| {
        regex::Regex::new(r"(?i)\b(error|fail(ed|ing)?|broken|not working|crash|issue|bug|fix|doesn'?t work|stopped working)\b")
            .expect("troubleshooting regex")
    });
    let step_structure = STEP_STRUCTURE.get_or_init(|| {
        regex::Regex::new(r"(?is)(\n\s*\d+[.)]\s|\bstep\s*\d|\bfirst\b.*\bthen\b|\n\s*[-*]\s)")
            .expect("step-structure regex")
    });

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
                        q.question.chars().take(120).collect::<String>(), q.count),
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
                        q.question.chars().take(120).collect::<String>(), hits, friction_count, q.count),
                    "method": "Cross of repeated questions with the interaction engine clarification/follow-up counts over the same conversations; association, not causation.",
                    "first_seen": first_seen,
                    "last_seen": last_seen
                }),
            );
            created += usize::from(newly);
        }
    }

    let docs: Vec<(i64, String)> = conn
        .prepare("SELECT id, title FROM knowledge_documents")
        .and_then(|mut stmt| {
            stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))
                .map(|rows| rows.filter_map(|r| r.ok()).collect())
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
                        q.question.chars().take(120).collect::<String>(), title),
                    "closest_document_title": title,
                    "method": "Troubleshooting-shaped question detection + step-structure regex over the closest FTS-matching document. A heuristic; the document may simply use prose.",
                    "first_seen": first_seen,
                    "last_seen": last_seen
                }),
            );
            created += usize::from(newly);
        }
    }

    let cluster_rows: Vec<(i64, String, i64)> = conn
        .prepare(
            "SELECT c.id, c.title,
                    (SELECT COUNT(*) FROM issue_cluster_conversations m WHERE m.cluster_id = c.id) AS n
             FROM issue_clusters c
             WHERE (SELECT COUNT(*) FROM issue_cluster_conversations m WHERE m.cluster_id = c.id) >= 3
             ORDER BY n DESC LIMIT 30",
        )
        .and_then(|mut stmt| {
            stmt.query_map([], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, Option<String>>(1)?.unwrap_or_default(),
                    r.get::<_, i64>(2)?,
                ))
            })
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
        })
        .unwrap_or_default();
    for (id, title, count) in cluster_rows {
        if knowledge_hits(conn, &title) == 0 {
            let members: Vec<i64> = conn
                .prepare(
                    "SELECT conversation_id FROM issue_cluster_conversations
                     WHERE cluster_id = ?1 ORDER BY conversation_id DESC LIMIT 50",
                )
                .ok()
                .and_then(|mut s| {
                    s.query_map(params![id], |r| r.get(0))
                        .map(|rows| rows.filter_map(|r| r.ok()).collect())
                        .ok()
                })
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
                        title.chars().take(120).collect::<String>(), count),
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
    (total, created)
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
    let mut detail_obj = json!({ "explanation": "", "method": "" });
    if let Some(d) = detail.as_deref()
        && let Ok(parsed) = serde_json::from_str::<Value>(d)
        && let Some(map) = parsed.as_object()
    {
        for (k, v) in map {
            detail_obj
                .as_object_mut()
                .unwrap()
                .insert(k.clone(), v.clone());
        }
    }
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

pub fn gap_by_id(conn: &Connection, id: i64) -> Option<Value> {
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
    let rows: Vec<Value> = conn
        .prepare(
            "SELECT id, kind, query_text, occurrence_count, evidence_conversation_ids,
                    related_document_ids, detail, status, decided_at, decision_note,
                    created_at, updated_at
             FROM knowledge_gap_candidates
             ORDER BY CASE status WHEN 'open' THEN 0 WHEN 'approved' THEN 1 ELSE 2 END,
                      occurrence_count DESC, updated_at DESC
             LIMIT 500",
        )
        .and_then(|mut stmt| {
            stmt.query_map([], |r| {
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
            })
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
        })
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

fn title_case(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut at_word_start = true;
    for c in s.chars() {
        if at_word_start && c.is_ascii_lowercase() {
            out.push(c.to_ascii_uppercase());
        } else {
            out.push(c);
        }
        at_word_start = !c.is_alphanumeric() && c != '_';
    }
    out
}

fn strip_interrogative_prefix(question: &str) -> &str {
    const FIRST_WORDS: [&str; 5] = ["how", "what", "why", "when", "where"];
    const SECOND_WORDS: [&str; 6] = ["do", "does", "is", "are", "can", "to"];
    fn advance_word(s: &str, vocab: &[&str]) -> Option<usize> {
        for w in vocab {
            if s.len() >= w.len()
                && s.is_char_boundary(w.len())
                && s[..w.len()].eq_ignore_ascii_case(w)
            {
                let after = &s[w.len()..];
                let rest = after.trim_start();
                let ws = after.len() - rest.len();
                if ws > 0 {
                    return Some(w.len() + ws);
                }
            }
        }
        None
    }
    let Some(first) = advance_word(question, &FIRST_WORDS) else {
        return question;
    };
    let Some(second) = advance_word(&question[first..], &SECOND_WORDS) else {
        return question;
    };
    &question[first + second..]
}

pub fn draft_gap(conn: &Connection, candidate_id: i64) -> Option<Value> {
    let candidate = gap_by_id(conn, candidate_id)?;
    let question = candidate["question"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    let evidence_ids: Vec<i64> = candidate["evidence_conversation_ids"]
        .as_array()
        .map(|a| a.iter().filter_map(|v| v.as_i64()).collect::<Vec<i64>>())
        .unwrap_or_default();
    let evidence: Vec<Value> = if evidence_ids.is_empty() {
        Vec::new()
    } else {
        let placeholders = vec!["?"; evidence_ids.len()].join(",");
        conn.prepare(&format!(
            "SELECT c.id, c.number, c.subject FROM conversations c
             WHERE c.id IN ({placeholders}) AND c.deleted_at IS NULL LIMIT 10"
        ))
        .and_then(|mut stmt| {
            stmt.query_map(rusqlite::params_from_iter(evidence_ids.iter()), |r| {
                Ok(json!({
                    "conversation_local_id": r.get::<_, i64>(0)?,
                    "number": r.get::<_, i64>(1)?,
                    "subject": r.get::<_, Option<String>>(2)?,
                }))
            })
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
        })
        .unwrap_or_default()
    };
    let stripped = strip_interrogative_prefix(&question);
    let base: String = if stripped.is_empty() {
        question.chars().take(80).collect()
    } else {
        stripped.chars().take(80).collect()
    };
    let suggested_title = title_case(&base);
    let mut outline: Vec<String> = vec![
        "Problem - the question as customers actually ask it (see evidence conversations below)"
            .into(),
        "Answer - one clear paragraph answering the question directly".into(),
        "Steps - numbered steps where applicable".into(),
        "Related - link related documents and known issues".into(),
    ];
    match candidate["kind"].as_str() {
        Some("missing_troubleshooting_steps") => {
            outline[2] =
                "Steps - numbered troubleshooting steps (the current document lacks them)".into();
        }
        Some("conflicting_knowledge") => {
            outline[0] = "Problem - these documents overlap; decide the single source of truth and redirect the other".into();
        }
        _ => {}
    }
    Some(json!({
        "candidate_id": candidate["id"],
        "kind": candidate["kind"],
        "suggested_title": suggested_title,
        "suggested_outline": outline,
        "evidence_conversations": evidence,
        "note": "A starting point for a human author. SupportOS never writes or publishes knowledge documents automatically."
    }))
}

struct FindingRow {
    conversation_id: i64,
    conversation_number: i64,
    customer_local_id: Option<i64>,
    kind: String,
    severity: String,
    evidence: String,
    detail: Option<String>,
    computed_at: String,
}

impl FindingRow {
    fn to_json(&self) -> Value {
        json!({
            "conversation_id": self.conversation_id,
            "conversation_number": self.conversation_number,
            "customer_local_id": self.customer_local_id,
            "kind": self.kind,
            "severity": self.severity,
            "evidence": serde_json::from_str::<Value>(&self.evidence)
                .unwrap_or_else(|_| json!([])),
            "detail": self.detail,
            "computed_at": self.computed_at,
        })
    }
}

const MIN_REPEAT_WORDS: usize = 6;

type AgentQuestion = (i64, Option<String>, String);

fn loop_problem_re() -> &'static regex::Regex {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| {
        regex::Regex::new(
            r"(?i)still (not|doesn'?t|no)|same (issue|problem|error)|happens again|again (the|it)|persist|not fixed|didn'?t (work|help|fix)|problem (is|persists|continues)|error (persists|continues|still)",
        )
        .expect("loop-problem regex")
    })
}

fn direct_complaint_re() -> &'static regex::Regex {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| {
        regex::Regex::new(
            r"(?i)(as|per) i (already |previously )?(said|mentioned|sent|wrote|provided|explained)|i already (sent|gave|provided|mentioned|told)|re-?sending|again[:,] |already (shared|attached)",
        )
        .expect("direct-complaint regex")
    })
}

pub fn analyze_friction(conn: &Connection, conversation_id: i64) -> Vec<Value> {
    let conv: Option<(i64, Option<i64>)> = conn
        .query_row(
            "SELECT number, customer_local_id FROM conversations
             WHERE id = ?1 AND deleted_at IS NULL",
            params![conversation_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .ok();
    let Some((number, customer_local_id)) = conv else {
        return Vec::new();
    };
    let threads = published_threads(conn, conversation_id);
    let customer: Vec<&ThreadLite> = threads.iter().filter(|t| t.kind == "customer").collect();
    let agents: Vec<&ThreadLite> = threads.iter().filter(|t| t.kind == "reply").collect();

    let now = now_iso();
    let mut findings: Vec<(String, String, Vec<Value>, String)> = Vec::new();
    let mut push = |kind: &str, severity: &str, evidence: Vec<Value>, detail: String| {
        findings.push((kind.to_string(), severity.to_string(), evidence, detail));
    };

    let customer_repeats = repeated_spans(&customer, MIN_REPEAT_WORDS);
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
        push(
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
        );
    }

    let mut question_groups: Vec<(String, Vec<AgentQuestion>)> = Vec::new();
    let mut group_index: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    for m in &agents {
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
        push(
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
        );
    }

    let mut saw_agent_reply = false;
    let mut loop_restatements: Vec<(i64, String, Option<String>)> = Vec::new();
    for t in &threads {
        if t.kind == "reply" {
            saw_agent_reply = true;
        } else if t.kind == "customer"
            && saw_agent_reply
            && loop_problem_re().is_match(&t.text)
            && !is_closing_acknowledgment(&t.text)
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
        push(
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
        );
    }

    let handoffs: Vec<(Option<String>, Option<String>)> = conn
        .prepare(
            "SELECT occurred_at, metadata FROM activity_events
             WHERE conversation_id = ?1 AND event_type = 'assignment_changed'
             ORDER BY occurred_at ASC",
        )
        .and_then(|mut stmt| {
            stmt.query_map(params![conversation_id], |r| Ok((r.get(0)?, r.get(1)?)))
                .map(|rows| rows.filter_map(|r| r.ok()).collect())
        })
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
        push(
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
        );
    }

    if let Some(customer_local) = customer_local_id {
        let related: Vec<RelatedConversation> = conn
            .prepare(
                "SELECT c2.id, c2.number, c2.subject, c2.status,
                        COALESCE(c2.remote_created_at, c2.created_at)
                 FROM conversations c2
                 WHERE c2.customer_local_id = ?1 AND c2.deleted_at IS NULL AND c2.id <> ?2
                   AND COALESCE(c2.remote_created_at, c2.created_at) >= datetime('now', '-90 days')
                   AND EXISTS (SELECT 1 FROM conversation_tags ct1
                               JOIN conversation_tags ct2 ON ct2.conversation_id = c2.id
                               JOIN tags tg ON tg.id = ct1.tag_id AND tg.id = ct2.tag_id
                               WHERE ct1.conversation_id = ?2)
                 ORDER BY c2.created_at DESC LIMIT 10",
            )
            .and_then(|mut stmt| {
                stmt.query_map(params![customer_local, conversation_id], |r| {
                    Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
                })
                .map(|rows| rows.filter_map(|r| r.ok()).collect())
            })
            .unwrap_or_default();
        if related.len() >= 2 {
            let shared_tag: Option<String> = conn
                .query_row(
                    "SELECT tg.name FROM conversation_tags ct1
                     JOIN conversation_tags ct2 ON ct2.conversation_id = ?1
                     JOIN tags tg ON tg.id = ct1.tag_id AND tg.id = ct2.tag_id
                     WHERE ct1.conversation_id = ?1 LIMIT 1",
                    params![conversation_id],
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
                            subject.as_deref().unwrap_or("(no subject)")),
                        "at": at
                    })
                })
                .collect();
            push(
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
            );
        }
    }

    let mut dup_requests: Vec<(i64, String, Option<String>)> = Vec::new();
    let mut supplied = (false, false, false);
    let digits_re = regex::Regex::new(r"\b\d{4,}\b").expect("digits regex");
    let email_re = regex::Regex::new(r"[\w.+-]+@[\w-]+\.[\w.]+").expect("email regex");
    let error_re = regex::Regex::new(r"(?i)error|fail|exception|crash").expect("error regex");
    let ask_pattern = regex::Regex::new(
        r"(?i)\?|could you (send|share|provide|confirm)|please (send|share|provide|confirm)",
    )
    .expect("ask-pattern regex");
    let order_re = regex::Regex::new(
        r"(?i)\border\b|\binvoice\b|account\s*(id|number)?|reference\b|ticket number",
    )
    .expect("order regex");
    let email_ask_re = regex::Regex::new(r"(?i)email").expect("email-ask regex");
    let error_ask_re =
        regex::Regex::new(r"(?i)error|log|trace|screenshot").expect("error-ask regex");
    for t in &threads {
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
    let direct_complaints: Vec<&ThreadLite> = customer
        .iter()
        .copied()
        .filter(|m| direct_complaint_re().is_match(&m.text))
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
        push(
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
        );
    }

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

    findings
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
        .collect()
}

pub fn rebuild_friction(conn: &Connection) -> (usize, usize) {
    let ids: Vec<i64> = {
        let Ok(mut stmt) = conn.prepare(
            "SELECT DISTINCT c.id FROM conversations c
             JOIN conversation_threads t ON t.conversation_id = c.id AND t.type = 'customer'
             WHERE c.deleted_at IS NULL",
        ) else {
            return (0, 0);
        };
        stmt.query_map([], |r| r.get(0))
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
            .unwrap_or_default()
    };
    let mut findings = 0usize;
    for id in &ids {
        findings += analyze_friction(conn, *id).len();
    }
    (ids.len(), findings)
}

pub fn friction_findings_for(conn: &Connection, conversation_id: i64) -> Vec<Value> {
    conn.prepare(
        "SELECT f.conversation_id, c.number, f.customer_local_id, f.kind, f.severity,
                f.evidence, f.detail, f.computed_at
         FROM friction_findings f JOIN conversations c ON c.id = f.conversation_id
         WHERE f.conversation_id = ?1
         ORDER BY CASE f.severity WHEN 'high' THEN 0 WHEN 'moderate' THEN 1 ELSE 2 END",
    )
    .and_then(|mut stmt| {
        stmt.query_map(params![conversation_id], |r| {
            Ok(FindingRow {
                conversation_id: r.get(0)?,
                conversation_number: r.get(1)?,
                customer_local_id: r.get(2)?,
                kind: r.get(3)?,
                severity: r.get(4)?,
                evidence: r.get(5)?,
                detail: r.get(6)?,
                computed_at: r.get(7)?,
            })
        })
        .map(|rows| rows.filter_map(|r| r.ok()).map(|r| r.to_json()).collect())
    })
    .unwrap_or_default()
}

pub fn friction_overview(conn: &Connection, days: i64) -> Value {
    let days = days.max(1);
    let window = format!("-{days} days");
    let kinds: Vec<Value> = FRICTION_KINDS
        .iter()
        .map(|(kind, label)| {
            let rows: Vec<Value> = conn
                .prepare(
                    "SELECT f.conversation_id, c.number, f.customer_local_id, f.kind, f.severity,
                            f.evidence, f.detail, f.computed_at
                     FROM friction_findings f JOIN conversations c ON c.id = f.conversation_id
                     WHERE f.kind = ?1
                       AND COALESCE(julianday(c.remote_created_at), julianday(f.computed_at))
                           >= julianday('now', ?2)
                     ORDER BY CASE f.severity WHEN 'high' THEN 0 WHEN 'moderate' THEN 1 ELSE 2 END,
                              f.computed_at DESC LIMIT 500",
                )
                .and_then(|mut stmt| {
                    stmt.query_map(params![kind, window], |r| {
                        Ok(FindingRow {
                            conversation_id: r.get(0)?,
                            conversation_number: r.get(1)?,
                            customer_local_id: r.get(2)?,
                            kind: r.get(3)?,
                            severity: r.get(4)?,
                            evidence: r.get(5)?,
                            detail: r.get(6)?,
                            computed_at: r.get(7)?,
                        })
                    })
                    .map(|rows| {
                        rows.filter_map(|r| r.ok()).map(|r| r.to_json()).collect()
                    })
                })
                .unwrap_or_default();
            json!({
                "kind": kind,
                "label": label,
                "conversations": rows.len(),
                "high_severity": rows.iter().filter(|r| r["severity"].as_str() == Some("high")).count(),
                "sample": rows.iter().take(10).cloned().collect::<Vec<_>>(),
            })
        })
        .collect();
    let customers: Vec<Value> = conn
        .prepare(
            "SELECT f.customer_local_id, COUNT(*) AS findings,
                    SUM(CASE WHEN f.severity = 'high' THEN 1 ELSE 0 END) AS high,
                    c2.first_name, c2.last_name
             FROM friction_findings f JOIN customers c2 ON c2.id = f.customer_local_id
             WHERE f.customer_local_id IS NOT NULL
               AND julianday(f.computed_at) >= julianday('now', ?1)
             GROUP BY f.customer_local_id ORDER BY findings DESC, high DESC LIMIT 10",
        )
        .and_then(|mut stmt| {
            stmt.query_map(params![window], |r| {
                Ok(json!({
                    "customer_local_id": r.get::<_, i64>(0)?,
                    "first_name": r.get::<_, Option<String>>(3)?,
                    "last_name": r.get::<_, Option<String>>(4)?,
                    "findings": r.get::<_, i64>(1)?,
                    "high": r.get::<_, Option<i64>>(2)?.unwrap_or(0),
                }))
            })
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
        })
        .unwrap_or_default();
    json!({
        "generated_at": now_iso(),
        "days": days,
        "kinds": kinds,
        "customers_most_affected": customers,
        "notes": [
            "Findings are deterministic text-shape heuristics over the local mirror - patterns, not judgments about people.",
            "Findings only exist for conversations analyzed after v2.1.0; run a rebuild to cover older history."
        ]
    })
}

const QA_PROMPT_VERSION: &str = "post_resolution_qa_v1";

pub struct QaAiOutcome {
    pub qa: Value,
    pub ai: Option<Value>,
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

pub fn compute_qa_deterministic(conn: &Connection, conversation_id: i64) -> Option<Value> {
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
    let (status, closed_at, created_at, first_response_at, history_complete) = conv?;
    let threads = published_threads(conn, conversation_id);
    let customer: Vec<&ThreadLite> = threads.iter().filter(|t| t.kind == "customer").collect();
    let replies: Vec<&ThreadLite> = threads.iter().filter(|t| t.kind == "reply").collect();

    let mut saw_reply = false;
    let mut back_and_forth: u32 = 0;
    for t in &threads {
        if t.kind == "reply" {
            saw_reply = true;
        } else if t.kind == "customer" && saw_reply && !is_closing_acknowledgment(&t.text) {
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
                        "excerpt": span.chars().take(120).collect::<String>()
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
    let _ = conn.execute(
        "DELETE FROM post_resolution_qa WHERE conversation_id = ?1",
        params![conversation_id],
    );
    let _ = conn.execute(
        "INSERT INTO post_resolution_qa (conversation_id, deterministic, computed_at)
         VALUES (?1, ?2, datetime('now'))",
        params![conversation_id, deterministic.to_string()],
    );
    Some(deterministic)
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
            compute_qa_deterministic(conn, conversation_id)?;
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

pub fn rebuild_qa(conn: &Connection) -> usize {
    let ids: Vec<i64> = conn
        .prepare("SELECT id FROM conversations WHERE deleted_at IS NULL AND status = 'closed'")
        .and_then(|mut stmt| {
            stmt.query_map([], |r| r.get(0))
                .map(|rows| rows.filter_map(|r| r.ok()).collect())
        })
        .unwrap_or_default();
    let count = ids.len();
    for id in ids {
        compute_qa_deterministic(conn, id);
    }
    count
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

fn coerce_ai_json(raw: &Value) -> Value {
    let val3 = |v: &Value| -> &'static str {
        match v.as_str() {
            Some("yes") => "yes",
            Some("no") => "no",
            _ => "unclear",
        }
    };
    let val4 = |v: &Value| -> &'static str {
        match v.as_str() {
            Some("yes") => "yes",
            Some("no") => "no",
            Some("partially") => "partially",
            _ => "unclear",
        }
    };
    let str_cap = |v: &Value, fallback: &str| -> String {
        v.as_str()
            .map(|s| s.chars().take(600).collect::<String>())
            .unwrap_or_else(|| fallback.to_string())
    };
    let ids = |v: &Value| -> Vec<i64> {
        v.as_array()
            .map(|a| {
                a.iter()
                    .filter(|x| x.as_i64().is_some_and(|n| n > 0))
                    .filter_map(|x| x.as_i64())
                    .take(10)
                    .collect::<Vec<i64>>()
            })
            .unwrap_or_default()
    };
    let answered = raw.get("answered");
    let evidence_supported = raw.get("evidence_supported");
    let correct_issue = raw.get("correct_issue");
    let suggestions = raw.get("suggestions");
    json!({
        "model": Value::Null,
        "answered": answered.map(|a| json!({
            "value": val3(&a["value"]),
            "reasoning": str_cap(&a["reasoning"], ""),
            "evidence_thread_ids": ids(&a["evidence_thread_ids"]),
        })),
        "evidence_supported": evidence_supported.map(|e| json!({
            "value": val4(&e["value"]),
            "reasoning": str_cap(&e["reasoning"], ""),
        })),
        "correct_issue": correct_issue.map(|c| json!({
            "value": val3(&c["value"]),
            "reasoning": str_cap(&c["reasoning"], ""),
        })),
        "suggestions": suggestions.map(|s| json!({
            "kb_improve": s["kb_improve"].as_bool().unwrap_or(false),
            "kb_reason": str_cap(&s["kb_reason"], ""),
            "saved_reply_suggested": s["saved_reply_suggested"].as_bool().unwrap_or(false),
            "saved_reply_title": s["saved_reply_title"].as_str()
                .map(|t| t.chars().take(200).collect::<String>()),
            "issue_association": s["issue_association"].as_str()
                .map(|t| t.chars().take(200).collect::<String>()),
        })),
    })
}

pub async fn compute_qa_ai_layer(
    conn: &Connection,
    backend: &crate::ai_pipeline::AiBackend,
    conversation_id: i64,
) -> QaAiOutcome {
    let deterministic = compute_qa_deterministic(conn, conversation_id);
    if deterministic.is_none() {
        return QaAiOutcome {
            qa: Value::Null,
            ai: None,
            error: Some("conversation not found".into()),
        };
    }
    if matches!(backend, crate::ai_pipeline::AiBackend::Disabled) {
        let qa = get_qa(conn, conversation_id, false).unwrap_or(Value::Null);
        return QaAiOutcome {
            qa,
            ai: None,
            error: Some(
                "AI is not available (disabled or offline). The deterministic QA layer was computed; the AI layer stays honestly absent."
                    .into(),
            ),
        };
    }
    let threads: Vec<(i64, String, String)> = conn
        .prepare(
            "SELECT id, type, COALESCE(body_html, body_text, '') FROM conversation_threads
             WHERE conversation_id = ?1 AND deleted_at IS NULL AND state = 'published'
             ORDER BY COALESCE(remote_created_at, created_at) ASC LIMIT 40",
        )
        .and_then(|mut stmt| {
            stmt.query_map(params![conversation_id], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            })
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
        })
        .unwrap_or_default();
    let transcript: String = {
        let lines: Vec<String> = threads
            .iter()
            .map(|(id, ttype, body)| {
                let author = match ttype.as_str() {
                    "customer" => "customer",
                    "reply" => "agent",
                    _ => "note",
                };
                let text = crate::textutil::html_to_text(body);
                format!(
                    "[{author} #{id}] {}",
                    text.chars().take(600).collect::<String>()
                )
            })
            .collect();
        lines.join("\n").chars().take(8000).collect()
    };
    let prompt = [
        "You are reviewing a CLOSED support conversation for quality insight. Answer strictly as JSON:".to_string(),
        "{\"answered\":{\"value\":\"yes|no|unclear\",\"reasoning\":\"...\",\"evidence_thread_ids\":[1]},\"evidence_supported\":{\"value\":\"yes|partially|no|unclear\",\"reasoning\":\"...\"},\"correct_issue\":{\"value\":\"yes|no|unclear\",\"reasoning\":\"...\"},\"suggestions\":{\"kb_improve\":false,\"kb_reason\":\"...\",\"saved_reply_suggested\":false,\"saved_reply_title\":null,\"issue_association\":null}}".to_string(),
        "Rules: base every claim on the transcript; cite thread ids from the transcript; when uncertain answer \"unclear\"; never invent ids; suggestions are recommendations for humans, nothing auto-applies.".to_string(),
        "Transcript:".to_string(),
        transcript,
    ]
    .join("\n");

    use sha2::{Digest, Sha256};
    let input_hash = {
        let mut hasher = Sha256::new();
        hasher.update(prompt.as_bytes());
        crate::crypto::hex(&hasher.finalize())
    };
    let run_id = crate::ai_pipeline::start_run(
        conn,
        "post_resolution_qa",
        Some(conversation_id),
        None,
        QA_PROMPT_VERSION,
        Some(&input_hash),
        &json!({
            "conversationId": conversation_id,
            "promptVersion": QA_PROMPT_VERSION,
            "inputHash": input_hash
        }),
    )
    .unwrap_or(0);

    let chat_result = backend
        .chat_qa(
            vec![
                crate::ai_provider::ChatMessage {
                    role: "system".into(),
                    content: "You are a careful support-quality reviewer. You output ONLY valid JSON matching the requested schema. You never fabricate evidence.".into(),
                },
                crate::ai_provider::ChatMessage {
                    role: "user".into(),
                    content: prompt,
                },
            ],
            0.1,
            800,
            true,
        )
        .await;

    match chat_result {
        Err(e) => {
            crate::ai_pipeline::fail_run(conn, run_id, &e.message).ok();
            let qa = get_qa(conn, conversation_id, true).unwrap_or(Value::Null);
            QaAiOutcome {
                qa,
                ai: None,
                error: Some(e.message),
            }
        }
        Ok(result) => finish_qa_ai_layer(
            conn,
            run_id,
            result.content.as_deref(),
            &result.model,
            result.latency_ms,
            conversation_id,
        ),
    }
}

fn finish_qa_ai_layer(
    conn: &Connection,
    run_id: i64,
    content: Option<&str>,
    model: &str,
    latency_ms: u64,
    conversation_id: i64,
) -> QaAiOutcome {
    let parsed = content.and_then(|content| {
        let raw = content.trim();
        let raw = raw
            .strip_prefix("```json")
            .or_else(|| raw.strip_prefix("```"))
            .unwrap_or(raw)
            .trim_start();
        let raw = raw.strip_suffix("```").unwrap_or(raw).trim_end();
        serde_json::from_str::<Value>(raw).ok()
    });
    match parsed {
        None => {
            crate::ai_pipeline::fail_run(conn, run_id, "unparseable model output").ok();
            let qa = get_qa(conn, conversation_id, true).unwrap_or(Value::Null);
            QaAiOutcome {
                qa,
                ai: None,
                error: Some(
                    "The model returned unparseable output; the AI layer stays honestly absent."
                        .into(),
                ),
            }
        }
        Some(raw) => {
            let mut with_model = coerce_ai_json(&raw);
            with_model["model"] = json!(model);
            crate::ai_pipeline::complete_run(conn, run_id, &with_model, latency_ms).ok();
            let _ = conn.execute(
                "UPDATE post_resolution_qa SET ai = ?2, ai_run_id = ?3,
                    recomputed_at = datetime('now')
                 WHERE conversation_id = ?1",
                params![conversation_id, with_model.to_string(), run_id],
            );
            let qa = get_qa(conn, conversation_id, true).unwrap_or(Value::Null);
            QaAiOutcome {
                qa,
                ai: Some(with_model),
                error: None,
            }
        }
    }
}

fn effectiveness_style_label(style: &str) -> String {
    match style {
        "detailed_explanation" => "Detailed explanation".to_string(),
        "step_by_step" => "Numbered steps".to_string(),
        "short_answer" => "Concise answer".to_string(),
        "direct_answer_with_explanation" => "Direct answer with explanation".to_string(),
        other => other.replace('_', " "),
    }
}

fn round2(x: f64) -> f64 {
    (x * 100.0).round() / 100.0
}

fn round1(x: f64) -> f64 {
    (x * 10.0).round() / 10.0
}

pub(crate) struct DerivedOutcome {
    pub(crate) conversation_id: i64,
    pub(crate) number: i64,
    pub(crate) subject: Option<String>,
    pub(crate) response_style: String,
    pub(crate) follow_up_count: i64,
    pub(crate) clarification_count: i64,
    pub(crate) resolved_after_first: Option<i64>,
    pub(crate) effort_score: Option<f64>,
    pub(crate) friction: String,
}

pub(crate) fn derive_outcome(
    threads: &[ThreadLite],
    status: &str,
    conv: (i64, i64, Option<String>),
) -> DerivedOutcome {
    let customer: Vec<&ThreadLite> = threads.iter().filter(|t| t.kind == "customer").collect();
    let replies: Vec<&ThreadLite> = threads.iter().filter(|t| t.kind == "reply").collect();
    let notes: Vec<&ThreadLite> = threads.iter().filter(|t| t.kind == "note").collect();
    let customer_texts: Vec<String> = customer
        .iter()
        .map(|t| t.text.trim().to_lowercase())
        .collect();
    let reply_texts: Vec<String> = replies
        .iter()
        .map(|t| t.text.trim().to_lowercase())
        .collect();

    let mut follow_up_count: i64 = 0;
    let mut saw_reply = false;
    for t in threads {
        if t.kind == "reply" {
            saw_reply = true;
        } else if t.kind == "customer" && saw_reply && !is_closing_acknowledgment(&t.text) {
            follow_up_count += 1;
        }
    }

    let clarification_re = regex::Regex::new(
        r"still|again|re-?send|clarif|you didn'?t|that didn'?t|not what i|same issue|as i (said|mentioned|wrote)",
    )
    .expect("clarification regex");
    let clarification_count: i64 = customer_texts
        .iter()
        .skip(1)
        .filter(|t| clarification_re.is_match(t))
        .count() as i64;

    let escalated = notes.iter().any(|n| {
        regex::Regex::new(r"(?i)escalat|urgent|priority|vip")
            .unwrap()
            .is_match(&n.text)
    }) || reply_texts.iter().any(|t| t.contains("escalat"));

    let resolved_after_first: Option<i64> = if replies.is_empty() {
        None
    } else {
        Some(i64::from(follow_up_count == 0 && status == "closed"))
    };

    let effort_raw = customer.len() as f64 * 1.2
        + follow_up_count as f64 * 1.5
        + clarification_count as f64 * 2.0
        + if escalated { 2.0 } else { 0.0 };
    let effort_score = if customer.is_empty() {
        None
    } else {
        Some(round1(effort_raw.min(10.0)))
    };
    let friction = match effort_score {
        None => "none".to_string(),
        Some(e) if e >= 6.0 => "high".to_string(),
        Some(e) if e >= 3.5 => "moderate".to_string(),
        Some(_) => "none".to_string(),
    };

    let total_reply_chars: usize = reply_texts.iter().map(|t| t.len()).sum();
    let avg_reply_len = if replies.is_empty() {
        0.0
    } else {
        total_reply_chars as f64 / replies.len() as f64
    };
    let joined = reply_texts.join(" ");
    let step_re = regex::Regex::new(r"\b(step|first|then|next|finally)\b|1\.").expect("step regex");
    let response_style = if replies.is_empty() {
        String::new()
    } else if avg_reply_len > 700.0 {
        "detailed_explanation".to_string()
    } else if step_re.is_match(&joined) {
        "step_by_step".to_string()
    } else if avg_reply_len < 200.0 {
        "short_answer".to_string()
    } else {
        "direct_answer_with_explanation".to_string()
    };

    DerivedOutcome {
        conversation_id: conv.0,
        number: conv.1,
        subject: conv.2,
        response_style,
        follow_up_count,
        clarification_count,
        resolved_after_first,
        effort_score,
        friction,
    }
}

pub fn effectiveness_report(conn: &Connection, days: i64) -> Value {
    let days = days.clamp(1, 3650);
    let window = format!("-{days} days");

    let window_clause = "c.deleted_at IS NULL
               AND COALESCE(julianday(c.remote_created_at), julianday(c.created_at))
                   >= julianday('now', ?1)
               AND EXISTS (SELECT 1 FROM conversation_threads t
                           WHERE t.conversation_id = c.id AND t.type = 'reply'
                             AND t.state = 'published' AND t.deleted_at IS NULL)";
    let total_in_window: i64 = conn
        .query_row(
            &format!("SELECT COUNT(*) FROM conversations c WHERE {window_clause}"),
            params![window],
            |r| r.get(0),
        )
        .unwrap_or(0);
    let convs: Vec<(i64, i64, Option<String>, String)> = conn
        .prepare(&format!(
            "SELECT c.id, c.number, c.subject, c.status
             FROM conversations c
             WHERE {window_clause}
             ORDER BY COALESCE(c.remote_created_at, c.created_at) DESC
             LIMIT 1000"
        ))
        .and_then(|mut stmt| {
            stmt.query_map(params![window], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
            })
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
        })
        .unwrap_or_default();

    let mut ratings_by_conv: std::collections::HashMap<i64, Vec<String>> =
        std::collections::HashMap::new();
    if !convs.is_empty() {
        let ids: Vec<String> = convs.iter().map(|c| c.0.to_string()).collect();
        let sql = format!(
            "SELECT conversation_id, rating FROM ratings WHERE conversation_id IN ({})",
            ids.join(",")
        );
        if let Ok(mut stmt) = conn.prepare(&sql)
            && let Ok(rows) = stmt.query_map([], |r| {
                Ok((r.get::<_, i64>(0)?, r.get::<_, Option<String>>(1)?))
            })
        {
            for row in rows.flatten() {
                if let Some(rating) = row.1 {
                    ratings_by_conv.entry(row.0).or_default().push(rating);
                }
            }
        }
    }

    let outcomes: Vec<DerivedOutcome> = convs
        .iter()
        .map(|(id, number, subject, status)| {
            let threads = published_threads(conn, *id);
            derive_outcome(&threads, status, (*id, *number, subject.clone()))
        })
        .collect();

    let doc_link_re = regex::Regex::new(
        r"(?i)https?://|docs?\.[a-z]|/help/|knowledge|documentation|\barticle\b|\bguide\b",
    )
    .expect("doc-link regex");
    let technical_re = regex::Regex::new(
        r"(?i)\b(api|endpoint|json|sdk|webhook|token|oauth|curl|console|stack trace|logs?|http|ssl|css|html|sql|cache)\b",
    )
    .expect("technical regex");
    let mut doc_link: std::collections::HashSet<i64> = std::collections::HashSet::new();
    let mut technical: std::collections::HashSet<i64> = std::collections::HashSet::new();
    for o in &outcomes {
        let threads = published_threads(conn, o.conversation_id);
        for t in threads.iter().filter(|t| t.kind == "reply") {
            if doc_link_re.is_match(&t.text) {
                doc_link.insert(o.conversation_id);
            }
            if technical_re.is_match(&t.text) {
                technical.insert(o.conversation_id);
            }
        }
    }

    let build_bucket =
        |key: &str, label: &str, kind: &str, matches: Vec<&DerivedOutcome>| -> Value {
            let n = matches.len();
            let rate = |num: usize| -> Option<f64> {
                if n > 0 {
                    Some(round2(num as f64 / n as f64))
                } else {
                    None
                }
            };
            let with_follow_ups = matches.iter().filter(|o| o.follow_up_count > 0).count();
            let with_clarifications = matches.iter().filter(|o| o.clarification_count > 0).count();
            let resolved_first = matches
                .iter()
                .filter(|o| o.resolved_after_first == Some(1))
                .count();
            let high_friction = matches.iter().filter(|o| o.friction == "high").count();
            let efforts: Vec<f64> = matches.iter().filter_map(|o| o.effort_score).collect();
            let rating_distribution: Value = if n > 0 {
                json!(
                    ["great", "okay", "not-good"]
                        .iter()
                        .map(|rating| {
                            let count = matches
                                .iter()
                                .filter(|o| {
                                    ratings_by_conv
                                        .get(&o.conversation_id)
                                        .is_some_and(|list| list.contains(&rating.to_string()))
                                })
                                .count();
                            json!({ "rating": rating, "count": count })
                        })
                        .collect::<Vec<_>>()
                )
            } else {
                Value::Null
            };
            let sample: Vec<Value> = matches
                .iter()
                .take(5)
                .map(|o| {
                    let ratings = ratings_by_conv.get(&o.conversation_id);
                    let mut summary = format!(
                        "{} follow-up(s), {} clarification(s)",
                        o.follow_up_count, o.clarification_count
                    );
                    if o.resolved_after_first == Some(1) {
                        summary.push_str(", resolved after first response");
                    }
                    if let Some(e) = o.effort_score {
                        summary.push_str(&format!(", effort {e}/10"));
                    }
                    if let Some(first) = ratings.and_then(|list| list.first()) {
                        summary.push_str(&format!(", rated {first}"));
                    }
                    json!({
                        "conversation_local_id": o.conversation_id,
                        "number": o.number,
                        "subject": o.subject,
                        "outcome_summary": summary,
                    })
                })
                .collect();
            json!({
                "style_key": key,
                "style_label": label,
                "kind": kind,
                "conversations": n,
                "follow_up_rate": rate(with_follow_ups),
                "clarification_rate": rate(with_clarifications),
                "resolved_after_first_rate": rate(resolved_first),
                "avg_effort_score": if efforts.is_empty() {
                    Value::Null
                } else {
                    json!(round1(efforts.iter().sum::<f64>() / efforts.len() as f64))
                },
                "high_friction_rate": rate(high_friction),
                "rating_distribution": rating_distribution,
                "sample_conversations": sample,
            })
        };

    let mut buckets: Vec<Value> = Vec::new();
    let mut style_order: Vec<String> = Vec::new();
    for o in &outcomes {
        if !o.response_style.is_empty() && !style_order.contains(&o.response_style) {
            style_order.push(o.response_style.clone());
        }
    }
    for style in &style_order {
        let matches: Vec<&DerivedOutcome> = outcomes
            .iter()
            .filter(|o| &o.response_style == style)
            .collect();
        buckets.push(build_bucket(
            style,
            &effectiveness_style_label(style),
            "response_style",
            matches,
        ));
    }
    let doc_matches: Vec<&DerivedOutcome> = outcomes
        .iter()
        .filter(|o| doc_link.contains(&o.conversation_id))
        .collect();
    if !doc_matches.is_empty() {
        buckets.push(build_bucket(
            "documentation_link",
            "Mentions documentation / links docs",
            "characteristic",
            doc_matches,
        ));
    }
    let tech_matches: Vec<&DerivedOutcome> = outcomes
        .iter()
        .filter(|o| technical.contains(&o.conversation_id))
        .collect();
    if !tech_matches.is_empty() {
        buckets.push(build_bucket(
            "technical_explanation",
            "Contains technical explanation",
            "characteristic",
            tech_matches,
        ));
    }
    buckets.sort_by(|a, b| {
        let av = a["conversations"].as_i64().unwrap_or(0);
        let bv = b["conversations"].as_i64().unwrap_or(0);
        bv.cmp(&av)
    });

    let mut notes = vec![
        "These are OBSERVED ASSOCIATIONS between how replies were written and what happened next. They do not show causation: agents may choose detailed replies for harder tickets, so outcomes reflect the mix of situations, not the style alone.".to_string(),
        "Styles come from the deterministic classifier the interaction engine already stores; characteristics (documentation link, technical explanation) are text-shape detections, not mutually exclusive categories.".to_string(),
        "Small buckets carry little information: treat any row under 5 conversations as anecdotal.".to_string(),
    ];
    if total_in_window > outcomes.len() as i64 {
        notes.push(format!(
            "Analysis bounded to the {} most recent of {} in-window conversations (v2.2.0 performance bound, plan Phase 41).",
            outcomes.len(),
            total_in_window
        ));
    }

    json!({
        "generated_at": now_iso(),
        "days": days,
        "total_analyzed": outcomes.len(),
        "buckets": buckets,
        "notes": notes,
    })
}

#[cfg(test)]
mod batch13_k1_tests {
    use super::*;
    use rusqlite::Connection;

    /// Seed a conversation with a completed ticket_analysis run whose
    /// `primary_question` is what the gap pipeline groups on.
    fn db_with_question(question: &str) -> Connection {
        let mut conn = Connection::open_in_memory().expect("db");
        crate::bootstrap::apply_all(&mut conn).expect("migrations");
        for i in 0..2 {
            conn.execute(
                "INSERT INTO conversations (remote_id, number, subject, preview, status)
                 VALUES (?1, ?1, 'Recurring issue', 'preview', 'closed')",
                params![9300 + i],
            )
            .expect("conversation");
            let conv = conn.last_insert_rowid();
            conn.execute(
                "INSERT INTO ai_runs (type, conversation_id, status, model, prompt_version,
                                      input_hash, response_json, created_at, started_at)
                 VALUES ('ticket_analysis', ?1, 'completed', 'm', 'ticket_analysis_v1',
                         'seeded', ?2, datetime('now'), datetime('now'))",
                params![conv, format!("{{\"primary_question\": \"{question}\"}}")],
            )
            .expect("analysis run");
        }
        conn
    }

    fn candidate_kinds(conn: &Connection) -> Vec<String> {
        conn.prepare("SELECT kind FROM knowledge_gap_candidates")
            .and_then(|mut stmt| {
                stmt.query_map([], |r| r.get::<_, String>(0))
                    .map(|rows| rows.filter_map(|r| r.ok()).collect())
            })
            .unwrap_or_default()
    }

    /// K1: "asked often, documented nowhere" — a repeated question with zero
    /// coverage in the knowledge base AND the help-center mirror becomes an
    /// uncovered-gap candidate.
    #[test]
    fn repeated_uncovered_question_becomes_gap() {
        let conn = db_with_question("how do I rotate the gateway keys");
        let (_, created) = rebuild_gaps(&conn, 90);
        assert!(created >= 1, "one candidate expected, created {created}");
        let kinds = candidate_kinds(&conn);
        assert!(
            kinds.iter().any(|k| k == "repeated_question_uncovered"),
            "the uncovered kind landed: {kinds:?}"
        );
    }

    /// K1: a question the help center already answers is NOT a gap — the
    /// docs_fts mirror counts as documentation (the coverage extension).
    #[test]
    fn docs_mirror_counts_as_documented() {
        let conn = db_with_question("how do I reach the control port");
        conn.execute(
            "INSERT INTO docs_collections (remote_id, slug, name)
             VALUES (50, 'help', 'Help center')",
            [],
        )
        .expect("collection");
        let collection = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO docs_articles (remote_id, collection_local_id, number, slug, name, status)
             VALUES (100, ?1, 1, 'control-port', 'Reaching the control port', 'published')",
            params![collection],
        )
        .expect("article");
        let article = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO docs_fts (name, text, article_id)
             VALUES ('Reaching the control port',
                     'How do you reach the control port? Open port 443 on the gateway console.',
                     ?1)",
            params![article],
        )
        .expect("docs fts");
        let (_, created) = rebuild_gaps(&conn, 90);
        assert_eq!(created, 0, "a documented question is not a gap");
        let kinds = candidate_kinds(&conn);
        assert!(
            kinds.is_empty(),
            "no candidates when the help center covers it: {kinds:?}"
        );
    }

    /// K1: friction findings are deterministic and idempotent per
    /// conversation+kind (UNIQUE dedupe).
    #[test]
    fn friction_rebuild_is_deterministic_and_deduped() {
        let mut conn = Connection::open_in_memory().expect("db");
        crate::bootstrap::apply_all(&mut conn).expect("migrations");
        conn.execute(
            "INSERT INTO conversations (remote_id, number, subject, preview, status)
             VALUES (9401, 94, 'Loop ticket', 'preview', 'active')",
            [],
        )
        .expect("conversation");
        let conv = conn.last_insert_rowid();
        // Two customer threads repeating the same 7-word span — the
        // repeated_customer_explanations detector fires across threads.
        for body in [
            "The export keeps failing every single night and nothing helps.",
            "Again today: the export keeps failing every single night.",
        ] {
            conn.execute(
                "INSERT INTO conversation_threads (conversation_id, type, state, body_text,
                                                  remote_created_at, created_at)
                 VALUES (?1, 'customer', 'published', ?2,
                         '2026-10-20T10:00:00Z', datetime('now'))",
                params![conv, body],
            )
            .expect("customer thread");
        }
        let (scanned_first, found_first) = rebuild_friction(&conn);
        assert!(scanned_first >= 1, "the seeded conversation is scanned");
        assert!(found_first >= 1, "repeated spans detected: {found_first}");
        let rows_first: i64 = conn
            .query_row("SELECT COUNT(*) FROM friction_findings", [], |r| r.get(0))
            .unwrap_or(0);
        assert!(
            rows_first >= 1,
            "findings persist for the agent tool to read"
        );
        // UNIQUE(conversation, kind) makes the rebuild idempotent: the row
        // count must not grow on a second pass.
        let (_, found_second) = rebuild_friction(&conn);
        assert!(found_second >= 1, "the findings are still detected");
        let rows_second: i64 = conn
            .query_row("SELECT COUNT(*) FROM friction_findings", [], |r| r.get(0))
            .unwrap_or(0);
        assert_eq!(rows_second, rows_first, "no duplicate rows across rebuilds");
    }
}

// ---------------------------------------------------------------------------
// K2: best-resolution evidence for AI article drafting
// ---------------------------------------------------------------------------

/// One resolution citation: the closed conversation, its number/subject,
/// and the excerpt of the final published agent reply (the same excerpt
/// logic the A4 retrieval spine stores as a `resolution` chunk).
#[derive(Debug, Clone, serde::Serialize)]
pub struct ResolutionCitation {
    pub conversation_local_id: i64,
    pub number: i64,
    pub subject: Option<String>,
    pub closed_at: Option<String>,
    pub excerpt: String,
}

/// The "best resolutions" a K2 article draft is grounded in: among the
/// evidence conversations, the ones that are closed and carry a published
/// reply, newest closure first, capped at `limit`. Conversations without a
/// published reply are skipped — nothing is excerpted from open threads.
pub fn best_resolutions(
    conn: &Connection,
    conversation_ids: &[i64],
    limit: usize,
) -> Vec<ResolutionCitation> {
    if conversation_ids.is_empty() || limit == 0 {
        return Vec::new();
    }
    let placeholders = vec!["?"; conversation_ids.len()].join(",");
    let rows: Vec<(i64, i64, Option<String>, Option<String>)> = conn
        .prepare(&format!(
            "SELECT c.id, c.number, c.subject, c.closed_at
               FROM conversations c
              WHERE c.id IN ({placeholders}) AND c.deleted_at IS NULL
                AND c.status = 'closed' AND c.closed_at IS NOT NULL
              ORDER BY c.closed_at DESC"
        ))
        .and_then(|mut stmt| {
            stmt.query_map(rusqlite::params_from_iter(conversation_ids.iter()), |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, Option<String>>(2)?,
                    r.get::<_, Option<String>>(3)?,
                ))
            })
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
        })
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
        out.push(ResolutionCitation {
            conversation_local_id: id,
            number,
            subject,
            closed_at,
            excerpt,
        });
    }
    out
}

/// The final published reply of a closed conversation, text-stripped and
/// capped — the shared excerpt logic with `embeddings::resolution_excerpt`
/// (kept local here so quality has no dependency on the chunker).
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

#[cfg(test)]
mod k2_tests {
    use super::*;
    use rusqlite::Connection;

    fn db() -> Connection {
        let mut conn = Connection::open_in_memory().expect("db");
        crate::bootstrap::apply_all(&mut conn).expect("migrations");
        conn
    }

    fn seed_closed_with_reply(conn: &Connection, remote: i64, days_ago: i64) -> i64 {
        let at = format!(
            "{}",
            (chrono::Utc::now() - chrono::Duration::try_days(days_ago).unwrap_or_default())
                .format("%Y-%m-%dT%H:%M:%S%.3fZ")
        );
        conn.execute(
            "INSERT INTO conversations (remote_id, number, subject, preview, status,
                                        remote_created_at, closed_at)
             VALUES (?1, ?1, 'Export timeouts on weekly report', 'p', 'closed', ?2, ?2)",
            params![remote, at],
        )
        .expect("conversation");
        let id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO conversation_threads (conversation_id, type, state, body_text, remote_created_at)
             VALUES (?1, 'reply', 'published', 'Clear the export cache, then re-run with the
               nightly profile; the timeout is the stale lock file.', ?2)",
            params![id, at],
        )
        .expect("thread");
        id
    }

    #[test]
    fn best_resolutions_picks_closed_with_published_reply_only() {
        let conn = db();
        let first = seed_closed_with_reply(&conn, 101, 2);
        let second = seed_closed_with_reply(&conn, 102, 1);
        // An active conversation with a reply must be skipped entirely.
        conn.execute(
            "INSERT INTO conversations (remote_id, number, subject, preview, status, remote_created_at)
             VALUES (103, 103, 'Still open', 'p', 'active', '2026-10-20T08:00:00.000Z')",
            [],
        )
        .expect("open");
        let open = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO conversation_threads (conversation_id, type, state, body_text, remote_created_at)
             VALUES (?1, 'reply', 'published', 'draft answer', '2026-10-20T08:05:00.000Z')",
            params![open],
        )
        .expect("open thread");

        let cites = best_resolutions(&conn, &[first, second, open, 999_999], 5);
        assert_eq!(cites.len(), 2, "only the two closed conversations");
        // Newest closure first.
        assert_eq!(cites[0].number, 102);
        assert_eq!(cites[1].number, 101);
        assert!(
            cites[0].excerpt.contains("nightly profile"),
            "the final published reply is the excerpt"
        );
        // The limit caps the list.
        assert_eq!(best_resolutions(&conn, &[first], 1).len(), 1);
        assert!(best_resolutions(&conn, &[], 5).is_empty());
    }

    #[test]
    fn best_resolutions_skips_closed_conversation_without_reply_text() {
        let conn = db();
        conn.execute(
            "INSERT INTO conversations (remote_id, number, subject, preview, status,
                                        remote_created_at, closed_at)
             VALUES (201, 201, 'Closed silently', 'p', 'closed',
                     '2026-10-18T08:00:00.000Z', '2026-10-19T08:00:00.000Z')",
            [],
        )
        .expect("closed no reply");
        let id = conn.last_insert_rowid();
        assert!(best_resolutions(&conn, &[id], 5).is_empty());
    }
}
