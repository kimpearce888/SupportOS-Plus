use std::collections::{HashMap, HashSet};

use regex::Regex;
use rusqlite::{params, Connection};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};

use crate::error::Result;

pub const COACHING_MAX_DRAFT_CHARS: usize = 20_000;
pub const COACHING_MAX_DRAFT_BYTES: usize = 60_000;
pub const COACHING_NOTE: &str = "Advisory only - coaching never blocks or modifies the send. Every finding cites its evidence; heuristics are labeled as heuristics.";
const PROMPT_VERSION: &str = "agent_coaching_v1";

pub const COACHING_CHECK_KINDS: &[&str] = &[
    "unanswered_customer_questions",
    "duplicated_questions",
    "unsupported_claims",
    "unsupported_timeframe",
    "missing_acknowledgment",
    "excessive_wording",
    "insufficient_detail",
    "internal_information_leakage",
    "wrong_customer_context",
    "preference_mismatch",
];

fn check_label(kind: &str) -> &'static str {
    match kind {
        "unanswered_customer_questions" => "Unanswered customer questions",
        "duplicated_questions" => "Duplicated questions",
        "unsupported_claims" => "Unsupported claims",
        "unsupported_timeframe" => "Unsupported timeframe",
        "missing_acknowledgment" => "Missing acknowledgment",
        "excessive_wording" => "Excessive wording",
        "insufficient_detail" => "Insufficient detail",
        "internal_information_leakage" => "Internal information leakage",
        "wrong_customer_context" => "Wrong customer context",
        "preference_mismatch" => "Communication preference mismatch",
        _ => "Check",
    }
}

fn check_layer(kind: &str) -> &'static str {
    match kind {
        "unsupported_claims" => "ai",
        _ => "deterministic",
    }
}

pub fn ensure_coaching_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS coaching_reviews (
            conversation_id INTEGER PRIMARY KEY REFERENCES conversations(id) ON DELETE CASCADE,
            draft_sha256    TEXT NOT NULL,
            draft_excerpt   TEXT NOT NULL,
            deterministic   TEXT NOT NULL DEFAULT '{}',
            ai              TEXT,
            ai_run_id       INTEGER,
            draft_chars     INTEGER NOT NULL DEFAULT 0,
            draft_words     INTEGER NOT NULL DEFAULT 0,
            created_at      TEXT NOT NULL DEFAULT (datetime('now')),
            provenance      TEXT NOT NULL DEFAULT 'deterministic_local'
        );",
    )?;
    Ok(())
}

fn stopwords() -> &'static HashSet<&'static str> {
    static WORDS: std::sync::OnceLock<HashSet<&'static str>> = std::sync::OnceLock::new();
    WORDS.get_or_init(|| {
        [
            "the", "a", "an", "and", "or", "but", "if", "then", "than", "that", "this", "these",
            "those", "is", "are", "was", "were", "be", "been", "being", "am", "do", "does", "did",
            "doing", "have", "has", "had", "will", "would", "shall", "should", "can", "could",
            "may", "might", "i", "you", "he", "she", "it", "we", "they", "me", "him", "her", "us",
            "them", "my", "your", "our", "their", "his", "its", "of", "to", "in", "on", "at",
            "for", "with", "from", "by", "as", "about", "into", "over", "after", "before", "again",
            "there", "here", "what", "when", "where", "who", "why", "how", "all", "any", "both",
            "each", "few", "more", "most", "other", "some", "such", "no", "not", "only", "own",
            "same", "so", "too", "very", "just", "now",
        ]
        .into_iter()
        .collect()
    })
}

fn normalize_words(text: &str) -> Vec<String> {
    let lowered = text.to_lowercase();
    let mut out = Vec::new();
    let mut current = String::new();
    for c in lowered.chars() {
        if c.is_ascii_lowercase() || c.is_ascii_digit() {
            current.push(c);
        } else if !current.is_empty() {
            if current.chars().count() > 1 {
                out.push(std::mem::take(&mut current));
            } else {
                current.clear();
            }
        }
    }
    if current.chars().count() > 1 {
        out.push(current);
    }
    out
}

fn stem(word: &str) -> String {
    let chars = word.chars().count();
    if chars > 6 && word.ends_with("ically") {
        return word[..word.len() - 6].to_string();
    }
    if chars > 5 && word.ends_with("ally") {
        return word[..word.len() - 4].to_string();
    }
    if chars > 4 && word.ends_with("ies") {
        return format!("{}y", &word[..word.len() - 3]);
    }
    if chars > 4 && word.ends_with("es") {
        return word[..word.len() - 2].to_string();
    }
    if chars > 3 && word.ends_with('s') {
        return word[..word.len() - 1].to_string();
    }
    word.to_string()
}

fn token_covered(token: &str, against: &HashSet<String>) -> bool {
    let st = stem(token);
    if against.contains(&st) {
        return true;
    }
    for other in against {
        if st.chars().count() >= 5
            && other.chars().count() >= 5
            && (st.starts_with(other.as_str()) || other.starts_with(st.as_str()))
        {
            return true;
        }
    }
    false
}

fn content_tokens(text: &str) -> HashSet<String> {
    normalize_words(text)
        .into_iter()
        .filter(|w| !stopwords().contains(w.as_str()) && w.chars().count() > 2)
        .map(|w| stem(&w))
        .collect()
}

fn word_count(text: &str) -> usize {
    text.split_whitespace().filter(|s| !s.is_empty()).count()
}

fn split_sentences(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut current = String::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '.' || c == '!' || c == '?' {
            current.push(c);
            let mut j = i + 1;
            while j < chars.len() && chars[j].is_whitespace() {
                j += 1;
            }
            if j > i + 1 || j >= chars.len() {
                let trimmed = current.trim().to_string();
                if !trimmed.is_empty() {
                    out.push(trimmed);
                }
                current.clear();
                i = j;
                continue;
            }
            i += 1;
            continue;
        }
        if c == '\n' {
            let trimmed = current.trim().to_string();
            if !trimmed.is_empty() {
                out.push(trimmed);
            }
            current.clear();
            i += 1;
            continue;
        }
        current.push(c);
        i += 1;
    }
    let trimmed = current.trim().to_string();
    if !trimmed.is_empty() {
        out.push(trimmed);
    }
    out.into_iter().take(60).collect()
}

fn is_question(sentence: &str) -> bool {
    if sentence.contains('?') {
        return true;
    }
    let lower = sentence.trim().to_lowercase();
    for starter in [
        "what", "why", "how", "when", "where", "who", "which", "can", "could", "should", "would",
        "is", "are", "does", "do", "did", "will", "has", "have",
    ] {
        if lower == starter || lower.starts_with(&format!("{starter} ")) {
            return true;
        }
    }
    false
}

fn is_closing_ack(sentence: &str) -> bool {
    let t = sentence.trim().trim_end_matches(['.', '!']).to_lowercase();
    matches!(
        t.as_str(),
        "ok" | "okay"
            | "got it"
            | "thanks"
            | "thank you"
            | "great"
            | "perfect"
            | "sounds good"
            | "appreciate it"
    )
}

fn ack_markers() -> &'static Regex {
    static RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)\b(sorry|apolog\w*|understand\w*|appreciate\w*|thank you|thanks for|patience|hear you|frustrat\w*)\b")
            .unwrap()
    })
}

fn frustration_markers() -> &'static Regex {
    static RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)\b(frustrat\w*|unacceptable|ridiculous|again\?|still not working|third time|last time i|fed up|not happy|disappointed|worst)\b")
            .unwrap()
    })
}

fn timeframe_patterns() -> &'static Vec<(Regex, &'static str)> {
    static PATTERNS: std::sync::OnceLock<Vec<(Regex, &'static str)>> = std::sync::OnceLock::new();
    PATTERNS.get_or_init(|| {
        vec![
            (
                Regex::new(r"(?i)\b(within|in|before|after)\s+(?:the\s+)?(?:next\s+)?(\d{1,4})\s*(minutes?|mins?|hours?|hrs?|days?|weeks?|business\s+days?)\b").unwrap(),
                "relative timeframe",
            ),
            (
                Regex::new(r"(?i)\bby\s+(tomorrow|today|tonight|eod|end\s+of\s+(?:the\s+)?(?:day|week|month|quarter)|next\s+(?:monday|tuesday|wednesday|thursday|friday)|monday|tuesday|wednesday|thursday|friday|saturday|sunday)\b").unwrap(),
                "deadline",
            ),
            (
                Regex::new(r"(?i)\b(as soon as possible|asap|right away|immediately)\b").unwrap(),
                "urgency commitment",
            ),
        ]
    })
}

struct ThreadRow {
    id: i64,
    kind: String,
    text: String,
}

struct ConvRow {
    id: i64,
    number: i64,
    subject: Option<String>,
    customer_id: Option<i64>,
}

fn load_thread_rows(conn: &Connection, conversation_id: i64) -> Vec<ThreadRow> {
    let Ok(mut stmt) = conn.prepare(
        "SELECT id, thread_type, html_stripped FROM (
             SELECT id, type AS thread_type,
                    COALESCE(body_html, body_text, '') AS html_stripped,
                    COALESCE(remote_created_at, created_at) AS at
             FROM conversation_threads
             WHERE conversation_id = ?1 AND deleted_at IS NULL AND state = 'published'
         ) ORDER BY at ASC",
    ) else {
        return Vec::new();
    };
    stmt.query_map(params![conversation_id], |r| {
        Ok(ThreadRow {
            id: r.get(0)?,
            kind: r.get::<_, Option<String>>(1)?.unwrap_or_default(),
            text: r.get(2)?,
        })
    })
    .map(|rows| {
        rows.filter_map(|r| r.ok())
            .map(|mut t| {
                t.text = crate::demo::html_to_text(&t.text);
                t
            })
            .collect()
    })
    .unwrap_or_default()
}

fn load_conv(conn: &Connection, conversation_id: i64) -> Option<ConvRow> {
    conn.query_row(
        "SELECT id, number, subject, customer_local_id FROM conversations
          WHERE id = ?1 AND deleted_at IS NULL",
        params![conversation_id],
        |r| {
            Ok(ConvRow {
                id: r.get(0)?,
                number: r.get(1)?,
                subject: r.get(2)?,
                customer_id: r.get(3)?,
            })
        },
    )
    .ok()
}

fn latest_signals(conn: &Connection, conversation_id: i64) -> String {
    conn.query_row(
        "SELECT signals_json FROM client_current_signals WHERE conversation_id = ?1",
        params![conversation_id],
        |r| r.get(0),
    )
    .unwrap_or_default()
}

fn customer_preference(conn: &Connection, customer_id: i64) -> Option<(String, String, bool, i64)> {
    let row: Option<(String, String, Option<String>, i64)> = conn
        .query_row(
            "SELECT preference, confidence, human_override_value, evidence_count
             FROM client_communication_preferences
             WHERE customer_id = ?1
             ORDER BY CASE WHEN human_override_value IS NOT NULL THEN 0 ELSE 1 END,
                      evidence_count DESC LIMIT 1",
            params![customer_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .ok();
    let (preference, confidence, override_value, evidence_count) = row?;
    Some(match override_value {
        Some(v) => (v, "human".to_string(), true, evidence_count),
        None => (preference, confidence, false, evidence_count),
    })
}

fn result(
    kind: &str,
    status: &str,
    detail: impl Into<String>,
    findings: Vec<Value>,
    ai_suffix: bool,
) -> Value {
    let label = check_label(kind);
    json!({
        "kind": kind,
        "label": if ai_suffix { format!("{label} (AI)") } else { label.to_string() },
        "layer": check_layer(kind),
        "status": status,
        "detail": detail.into(),
        "findings": findings,
    })
}

fn summarize(checks: &[Value]) -> Value {
    let flagged = checks
        .iter()
        .filter(|c| c.get("status").and_then(|s| s.as_str()) == Some("flagged"))
        .count();
    let checks_run = checks
        .iter()
        .filter(|c| {
            matches!(
                c.get("status").and_then(|s| s.as_str()),
                Some("pass") | Some("flagged")
            )
        })
        .count();
    let checks_unavailable = checks
        .iter()
        .filter(|c| c.get("status").and_then(|s| s.as_str()) == Some("unavailable"))
        .count();
    json!({
        "flagged": flagged,
        "checks_run": checks_run,
        "checks_unavailable": checks_unavailable,
    })
}

fn evidence(
    description: impl Into<String>,
    excerpt: impl Into<String>,
    conversation_id: Option<i64>,
    thread_id: Option<i64>,
    incident_code: Option<&str>,
) -> Value {
    let mut map = Map::new();
    map.insert("description".into(), json!(description.into()));
    map.insert("excerpt".into(), json!(excerpt.into()));
    if let Some(id) = conversation_id {
        map.insert("conversation_id".into(), json!(id));
    }
    if let Some(id) = thread_id {
        map.insert("thread_id".into(), json!(id));
    }
    if let Some(code) = incident_code {
        map.insert("incident_code".into(), json!(code));
    }
    Value::Object(map)
}

fn check_unanswered_questions(conv: &ConvRow, threads: &[ThreadRow], draft: &str) -> Value {
    let kind = "unanswered_customer_questions";
    let customer_msgs: Vec<&ThreadRow> = threads
        .iter()
        .filter(|t| t.kind == "customer")
        .rev()
        .take(30)
        .collect();
    let mut questions: Vec<(String, i64)> = Vec::new();
    for m in &customer_msgs {
        for sentence in split_sentences(&m.text) {
            if is_question(&sentence) && !is_closing_ack(&sentence) {
                questions.push((sentence, m.id));
            }
        }
    }
    if questions.is_empty() {
        return result(
            kind,
            "not_applicable",
            "No open customer questions detected in this conversation.",
            vec![],
            false,
        );
    }
    let draft_tokens = content_tokens(draft);
    let mut findings: Vec<Value> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    for (q, thread_id) in &questions {
        let key = normalize_words(q).join(" ");
        if !seen.insert(key) {
            continue;
        }
        let q_tokens: Vec<String> = content_tokens(q).into_iter().collect();
        if q_tokens.is_empty() {
            continue;
        }
        let covered = q_tokens
            .iter()
            .filter(|t| token_covered(t, &draft_tokens))
            .count();
        let ratio = covered as f64 / q_tokens.len() as f64;
        if ratio < 0.4 && q_tokens.len() >= 2 {
            findings.push(json!({
                "draft_excerpt": "(the draft does not appear to address this)",
                "evidence": [evidence(
                    "customer question",
                    q.chars().take(240).collect::<String>(),
                    Some(conv.id),
                    Some(*thread_id),
                    None
                )],
                "advice": "Answer it directly, or explicitly say the answer is unknown / needs more time."
            }));
        }
    }
    let detail = if !findings.is_empty() {
        format!(
            "{} customer question(s) show no matching coverage in the draft (token-overlap heuristic, < 40% key-token coverage).",
            findings.len()
        )
    } else {
        format!(
            "All {} detected customer question(s) appear addressed (token-overlap heuristic).",
            questions.len()
        )
    };
    result(
        kind,
        if findings.is_empty() {
            "pass"
        } else {
            "flagged"
        },
        detail,
        findings,
        false,
    )
}

fn check_duplicated_questions(threads: &[ThreadRow], draft: &str) -> Value {
    let kind = "duplicated_questions";
    let mut prior_agent_questions: HashSet<String> = HashSet::new();
    let mut prior_by_text: Vec<(String, i64)> = Vec::new();
    for m in threads.iter().filter(|t| t.kind == "reply") {
        for sentence in split_sentences(&m.text) {
            if is_question(&sentence) {
                let key = normalize_words(&sentence).join(" ");
                if prior_agent_questions.insert(key) {
                    prior_by_text.push((sentence, m.id));
                }
            }
        }
    }
    if prior_agent_questions.is_empty() {
        return result(
            kind,
            "not_applicable",
            "No earlier agent questions to duplicate.",
            vec![],
            false,
        );
    }
    let mut findings: Vec<Value> = Vec::new();
    for sentence in split_sentences(draft) {
        if !is_question(&sentence) {
            continue;
        }
        let draft_tokens: HashSet<String> = content_tokens(&sentence);
        if draft_tokens.is_empty() {
            continue;
        }
        for (prior, thread_id) in &prior_by_text {
            let prior_tokens = content_tokens(prior);
            if prior_tokens.is_empty() {
                continue;
            }
            let overlap = draft_tokens
                .iter()
                .filter(|t| prior_tokens.contains(*t))
                .count() as f64
                / draft_tokens.len() as f64;
            if overlap >= 0.7 {
                findings.push(json!({
                    "draft_excerpt": sentence.chars().take(240).collect::<String>(),
                    "evidence": [evidence(
                        "earlier agent question in this conversation",
                        prior.chars().take(240).collect::<String>(),
                        None,
                        Some(*thread_id),
                        None
                    )],
                    "advice": "The customer already received this question. Check their answer above instead of asking again."
                }));
                break;
            }
        }
    }
    let detail = if !findings.is_empty() {
        format!(
            "{} draft question(s) repeat a question already asked in this conversation.",
            findings.len()
        )
    } else {
        "No draft question repeats an earlier agent question.".to_string()
    };
    result(
        kind,
        if findings.is_empty() {
            "pass"
        } else {
            "flagged"
        },
        detail,
        findings,
        false,
    )
}

fn check_timeframe(conn: &Connection, conv: &ConvRow, draft: &str) -> Value {
    let kind = "unsupported_timeframe";
    let active_incidents: Vec<(String, String, String)> = match conn.prepare(
        "SELECT i.code, i.status, i.title FROM incident_conversations ic
            JOIN incidents i ON i.id = ic.incident_id
         WHERE ic.conversation_id = ?1 AND i.status != 'resolved'",
    ) {
        Ok(mut stmt) => stmt
            .query_map(params![conv.id], |r| {
                Ok((
                    r.get::<_, Option<String>>(0)?.unwrap_or_default(),
                    r.get::<_, Option<String>>(1)?.unwrap_or_default(),
                    r.get::<_, Option<String>>(2)?.unwrap_or_default(),
                ))
            })
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
            .unwrap_or_default(),
        Err(_) => Vec::new(),
    };
    let mut promises: Vec<(String, &str)> = Vec::new();
    for sentence in split_sentences(draft) {
        for (re, label) in timeframe_patterns() {
            if re.is_match(&sentence) {
                promises.push((sentence.chars().take(240).collect(), *label));
                break;
            }
        }
    }
    if promises.is_empty() {
        return result(
            kind,
            "pass",
            "No explicit timeframe commitments found in the draft.",
            vec![],
            false,
        );
    }
    let findings: Vec<Value> = promises
        .iter()
        .map(|(sentence, _label)| {
            let mut ev = vec![evidence(
                "draft sentence",
                sentence.clone(),
                Some(conv.id),
                None,
                active_incidents.first().map(|(code, ..)| code.as_str()),
            )];
            if !active_incidents.is_empty() {
                let listing = active_incidents
                    .iter()
                    .map(|(code, status, _)| format!("{code} [{status}]"))
                    .collect::<Vec<_>>()
                    .join(", ");
                let excerpt = active_incidents
                    .iter()
                    .map(|(code, status, title)| {
                        format!("{code}: {title} (status: {status})")
                    })
                    .collect::<Vec<_>>()
                    .join("; ");
                ev.push(evidence(
                    format!(
                        "linked ACTIVE incident{} ({listing})",
                        if active_incidents.len() > 1 { "s" } else { "" }
                    ),
                    excerpt.chars().take(240).collect::<String>(),
                    None,
                    None,
                    None,
                ));
                json!({
                    "draft_excerpt": sentence,
                    "evidence": ev,
                    "advice": "A timeframe is promised while an active incident is linked to this conversation. Confirm the incident owner expects resolution inside this window before committing."
                })
            } else {
                json!({
                    "draft_excerpt": sentence,
                    "evidence": ev,
                    "advice": "Explicit time commitment - verify it is supported by evidence (SLA, incident status, owner confirmation) before sending."
                })
            }
        })
        .collect();
    let detail = format!(
        "{} explicit timeframe commitment(s){}.",
        promises.len(),
        if active_incidents.is_empty() {
            String::new()
        } else {
            format!(" and {} linked active incident(s)", active_incidents.len())
        }
    );
    result(kind, "flagged", detail, findings, false)
}

fn check_acknowledgment(threads: &[ThreadRow], signals: &str, draft: &str) -> Value {
    let kind = "missing_acknowledgment";
    let signals_re = Regex::new(r#"(?i)frustration[":\s]*(moderate|strong)"#).unwrap();
    let mut frustration: Option<(String, String, Option<i64>)> = None;
    if signals_re.is_match(signals) {
        frustration = Some((
            "interaction signals (deterministic layer)".into(),
            signals.chars().take(240).collect(),
            None,
        ));
    } else {
        for m in threads
            .iter()
            .filter(|t| t.kind == "customer")
            .rev()
            .take(8)
        {
            if frustration_markers().is_match(&m.text) {
                frustration = Some((
                    "customer message (frustration markers)".into(),
                    m.text.chars().take(240).collect(),
                    Some(m.id),
                ));
                break;
            }
        }
    }
    let Some((description, excerpt, thread_id)) = frustration else {
        return result(
            kind,
            "not_applicable",
            "No frustration cues detected in recent customer messages or interaction signals.",
            vec![],
            false,
        );
    };
    if ack_markers().is_match(draft) {
        return result(
            kind,
            "pass",
            "Frustration cues present and the draft acknowledges them.",
            vec![],
            false,
        );
    }
    result(
        kind,
        "flagged",
        "Frustration cues detected but the draft contains no acknowledgment markers.",
        vec![json!({
            "draft_excerpt": "(no sorry / understand / appreciate / patience wording found)",
            "evidence": [evidence(description, excerpt, None, thread_id, None)],
            "advice": "Acknowledge the experience before diving into the technical answer (one sentence is enough)."
        })],
        false,
    )
}

fn check_wording(preference: Option<&(String, String, bool, i64)>, draft: &str) -> Value {
    let kind = "excessive_wording";
    let words = word_count(draft);
    let paragraphs = draft
        .split("\n\n")
        .map(|p| p.trim())
        .filter(|p| !p.trim().is_empty())
        .count();
    let concise = preference.map(|p| p.0.as_str()) == Some("concise");
    let threshold = if concise { 250 } else { 500 };
    if words > threshold {
        let evidence_desc = if concise {
            format!(
                "communication preference: concise (confidence {})",
                preference.map(|p| p.1.clone()).unwrap_or_default()
            )
        } else {
            "length threshold".to_string()
        };
        let evidence_excerpt = if concise {
            format!(
                "preference: concise (confidence {})",
                preference.map(|p| p.1.clone()).unwrap_or_default()
            )
        } else {
            format!("{words} words, {paragraphs} paragraphs")
        };
        let detail = format!(
            "Draft is {words} words across {paragraphs} paragraph(s){}.",
            if concise {
                format!(
                    "; this customer's observed preference is concise (confidence {})",
                    preference.map(|p| p.1.clone()).unwrap_or_default()
                )
            } else {
                String::new()
            }
        );
        let advice = if concise {
            "Trim to the essentials - this customer historically prefers concise answers."
        } else {
            "Consider trimming: very long replies correlate with lower resolution-after-first-response in the effectiveness report (an association, not causation)."
        };
        return result(
            kind,
            "flagged",
            detail,
            vec![json!({
                "draft_excerpt": draft.chars().take(240).collect::<String>(),
                "evidence": [evidence(evidence_desc, evidence_excerpt, None, None, None)],
                "advice": advice
            })],
            false,
        );
    }
    result(
        kind,
        "pass",
        format!("Draft length is reasonable ({words} words, {paragraphs} paragraph(s))."),
        vec![],
        false,
    )
}

fn check_detail(signals: &str, threads: &[ThreadRow], draft: &str) -> Value {
    let kind = "insufficient_detail";
    let words = word_count(draft);
    let question_count = threads
        .iter()
        .filter(|t| t.kind == "customer")
        .rev()
        .take(10)
        .map(|m| {
            split_sentences(&m.text)
                .into_iter()
                .filter(|s| is_question(s) && !is_closing_ack(s))
                .count()
        })
        .sum::<usize>();
    let technical_re =
        Regex::new(r#"(?i)technical_language[":\s]*(technical|highly_technical)"#).unwrap();
    let technical = technical_re.is_match(signals);
    if words < 15 && (question_count > 0 || technical) {
        return result(
            kind,
            "flagged",
            format!(
                "Draft is only {words} words while {question_count} open customer question(s){} exist.",
                if technical { " and technical-language signals" } else { "" }
            ),
            vec![json!({
                "draft_excerpt": draft.chars().take(240).collect::<String>(),
                "evidence": [evidence(
                    "conversation context",
                    format!("{question_count} open question(s){}", if technical { ", technical familiarity signals" } else { "" }),
                    None, None, None
                )],
                "advice": "Expand with the concrete steps or answer - one-line replies on substantive questions correlate with follow-up loops."
            })],
            false,
        );
    }
    result(
        kind,
        "pass",
        format!(
            "Draft detail is proportionate ({words} words vs {question_count} open question(s))."
        ),
        vec![],
        false,
    )
}

fn check_internal_leakage(
    conn: &Connection,
    conv: &ConvRow,
    threads: &[ThreadRow],
    draft: &str,
) -> Value {
    let kind = "internal_information_leakage";
    let mut corpus: Vec<(String, String)> = Vec::new();
    for note in threads.iter().filter(|t| t.kind == "note").rev().take(100) {
        if !note.text.trim().is_empty() {
            corpus.push((
                format!("internal note (thread #{})", note.id),
                note.text.clone(),
            ));
        }
    }
    let collect_internal =
        |sql: &str, describe: &dyn Fn(&str, &str, &str) -> String| -> Vec<(String, String)> {
            let mut out = Vec::new();
            if let Ok(mut stmt) = conn.prepare(sql) {
                let rows: Vec<(String, Option<String>, Option<String>)> = stmt
                    .query_map(params![conv.id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
                    .map(|rows| rows.filter_map(|r| r.ok()).collect())
                    .unwrap_or_default();
                for (name, internal_explanation, known_cause) in rows {
                    for (field, value) in [
                        ("internal_explanation", internal_explanation),
                        ("known_cause", known_cause),
                    ] {
                        if let Some(v) = value {
                            if !v.trim().is_empty() {
                                out.push((describe(&name, field, &v), v));
                            }
                        }
                    }
                }
            }
            out
        };
    corpus.extend(collect_internal(
        "SELECT i.code, i.internal_explanation, i.known_cause FROM incident_conversations ic
            JOIN incidents i ON i.id = ic.incident_id WHERE ic.conversation_id = ?1",
        &|code, field, _| format!("incident {code} {field} (internal)"),
    ));
    corpus.extend(collect_internal(
        "SELECT ki.title, ki.internal_explanation, ki.known_cause FROM known_issue_conversations kic
            JOIN known_issues ki ON ki.id = kic.known_issue_id WHERE kic.conversation_id = ?1",
        &|title, field, _| format!("known issue \"{title}\" {field} (internal)"),
    ));
    {
        let docs: Vec<(String, Option<String>)> = match conn.prepare(
            "SELECT DISTINCT kd.title, kd.content FROM ai_sources s
                JOIN ai_runs r ON r.id = s.run_id
                JOIN knowledge_documents kd ON kd.id = s.source_id
             WHERE r.conversation_id = ?1 AND s.source_type = 'knowledge_document' AND kd.visibility = 'internal_only'",
        ) {
            Ok(mut stmt) => stmt
                .query_map(params![conv.id], |r| Ok((r.get(0)?, r.get(1)?)))
                .map(|rows| rows.filter_map(|r| r.ok()).collect())
                .unwrap_or_default(),
            Err(_) => Vec::new(),
        };
        for (title, content) in docs.into_iter().take(10) {
            if let Some(content) = content {
                if !content.trim().is_empty() {
                    corpus.push((
                        format!("internal knowledge document \"{title}\""),
                        content.chars().take(8000).collect(),
                    ));
                }
            }
        }
    }
    if corpus.is_empty() {
        return result(
            kind,
            "not_applicable",
            "No internal-only material is linked to this conversation (notes, incident internals, internal docs).",
            vec![],
            false,
        );
    }
    let draft_words = normalize_words(draft);
    let mut draft_grams: HashSet<String> = HashSet::new();
    for i in 0..draft_words.len().saturating_sub(5) {
        let gram = draft_words[i..i + 6].join(" ");
        draft_grams.insert(gram);
    }
    let mut findings: Vec<Value> = Vec::new();
    for (description, text) in &corpus {
        let source_words = normalize_words(text);
        let mut hit: Option<String> = None;
        let mut i = 0;
        while i + 6 <= source_words.len() && hit.is_none() {
            let gram = source_words[i..i + 6].join(" ");
            if draft_grams.contains(&gram) {
                hit = Some(gram);
            }
            i += 1;
        }
        if hit.is_some() {
            findings.push(json!({
                "draft_excerpt": draft.chars().take(240).collect::<String>(),
                "evidence": [evidence(description.clone(), text.chars().take(240).collect::<String>(), None, None, None)],
                "advice": "This draft shares a verbatim span with internal-only material. Rewrite that passage in customer-safe language before sending."
            }));
        }
    }
    let detail = if !findings.is_empty() {
        format!(
            "{} verbatim span(s) shared with internal-only material (6+ consecutive words).",
            findings.len()
        )
    } else {
        format!(
            "No verbatim overlap with {} internal source(s) (6-gram shingle check).",
            corpus.len()
        )
    };
    result(
        kind,
        if findings.is_empty() {
            "pass"
        } else {
            "flagged"
        },
        detail,
        findings,
        false,
    )
}

fn check_wrong_context(
    conn: &Connection,
    conv: &ConvRow,
    customer: Option<&(String, Option<String>)>,
    draft: &str,
) -> Value {
    let kind = "wrong_customer_context";
    let mut findings: Vec<Value> = Vec::new();

    let number_refs_re = Regex::new(r"#(\d{3,8})\b").unwrap();
    let number_refs: Vec<i64> = number_refs_re
        .captures_iter(draft)
        .filter_map(|c| c.get(1).and_then(|m| m.as_str().parse::<i64>().ok()))
        .take(10)
        .collect();
    for r in number_refs {
        let row: Option<(i64, Option<i64>)> = conn
            .query_row(
                "SELECT c.id, c.customer_local_id FROM conversations c
                  WHERE c.number = ?1 AND c.deleted_at IS NULL",
                params![r],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .ok();
        match row {
            None => findings.push(json!({
                "draft_excerpt": format!("#{r}"),
                "evidence": [evidence(
                    "conversation lookup by number",
                    format!("No conversation #{r} exists in the local mirror."),
                    None, None, None
                )],
                "advice": format!("#{r} does not match any known conversation - verify the number before sending.")
            })),
            Some((id, row_customer)) => {
                if let Some(conv_customer) = conv.customer_id {
                    if row_customer != Some(conv_customer) {
                        findings.push(json!({
                            "draft_excerpt": format!("#{r}"),
                            "evidence": [evidence(
                                "conversation lookup by number",
                                format!("Conversation #{r} belongs to a different customer (local id {}).", row_customer.map(|c| c.to_string()).unwrap_or_else(|| "null".into())),
                                Some(id), None, None
                            )],
                            "advice": format!("#{r} belongs to another customer - referencing it here may leak cross-customer context.")
                        }));
                    }
                }
            }
        }
    }

    let greeting_re = Regex::new(r"(?im)^(?:hi|hello|hey|dear)\s+([a-z][a-z'-]{1,30})\b").unwrap();
    if let Some(greeting) = greeting_re.captures(draft) {
        if let Some((first_name, _)) = customer {
            if let Some(greeted) = greeting.get(1).map(|m| m.as_str().to_lowercase()) {
                let first = first_name.to_lowercase();
                let generic = [
                    "team", "support", "there", "all", "everyone", "sir", "madam", "folks",
                ];
                if !generic.contains(&greeted.as_str())
                    && greeted != first
                    && !first.starts_with(&greeted)
                {
                    findings.push(json!({
                        "draft_excerpt": greeting.get(0).map(|m| m.as_str()).unwrap_or("").chars().take(240).collect::<String>(),
                        "evidence": [evidence(
                            "customer record",
                            format!("This customer is {}.", first_name),
                            None, None, None
                        )],
                        "advice": "The greeting names someone else - confirm you are replying in the right conversation with the right customer."
                    }));
                }
            }
        }
    }

    let detail = if !findings.is_empty() {
        format!(
            "{} context mismatch(es) detected (conversation references / greeting name).",
            findings.len()
        )
    } else {
        "No cross-customer context mismatches detected.".to_string()
    };
    result(
        kind,
        if findings.is_empty() {
            "pass"
        } else {
            "flagged"
        },
        detail,
        findings,
        false,
    )
}

fn check_preference_mismatch(
    preference: Option<&(String, String, bool, i64)>,
    threads: &[ThreadRow],
    draft: &str,
) -> Value {
    let kind = "preference_mismatch";
    let Some(&(ref pref, ref confidence, overridden, evidence_count)) = preference else {
        return result(
            kind,
            "not_applicable",
            "No communication preference on record for this customer (honest unknown - at least 3 distinct conversations are required before one is inferred).",
            vec![],
            false,
        );
    };
    let words = normalize_words(draft).len();
    let steps_re = Regex::new(r"(?i)\b(step|first|then|next|finally)\b|\d\.").unwrap();
    let has_steps = steps_re.is_match(draft);
    let open_question = threads
        .iter()
        .filter(|t| t.kind == "customer")
        .rev()
        .take(10)
        .any(|m| {
            split_sentences(&m.text)
                .iter()
                .any(|s| is_question(s) && !is_closing_ack(s))
        });
    let mismatch = if pref == "concise" && words > 250 {
        Some(format!("concise preference, {words}-word draft"))
    } else if pref == "detailed" && words < 60 && open_question {
        Some(format!(
            "detailed preference, {words}-word draft on an open question"
        ))
    } else if pref == "step_by_step" && !has_steps && words > 80 && open_question {
        Some("step_by_step preference, no step/list structure detected".to_string())
    } else {
        None
    };
    let ev = vec![evidence(
        format!(
            "communication preference: {pref}{}",
            if overridden { " (human override active)" } else { "" }
        ),
        format!("preference {pref}, confidence {confidence}, {evidence_count} supporting conversation(s)"),
        None, None, None,
    )];
    if let Some(mismatch) = mismatch {
        return result(
            kind,
            "flagged",
            format!("Draft shape does not match the observed preference ({mismatch}). This is an observed preference, not a rule."),
            vec![json!({
                "draft_excerpt": draft.chars().take(240).collect::<String>(),
                "evidence": ev,
                "advice": format!("This customer's observed preference is \"{pref}\". Reshaping the draft may land better - your call.")
            })],
            false,
        );
    }
    result(
        kind,
        "pass",
        format!(
            "Draft shape matches the observed preference ({pref}, confidence {confidence}{}).",
            if overridden {
                ", human override active"
            } else {
                ""
            }
        ),
        vec![],
        false,
    )
}

pub type CoachingChatFn<'a> = &'a dyn Fn(
    Vec<crate::ai_provider::ChatMessage>,
    f64,
    u32,
    bool,
) -> std::pin::Pin<
    Box<
        dyn std::future::Future<Output = std::result::Result<(Option<String>, String, u64), String>>
            + 'a,
    >,
>;

fn coerce_ai_layer(content: &str) -> Option<Value> {
    let raw: Value = serde_json::from_str(content.trim()).ok()?;
    let verdict_of = |v: &Value, allowed: &[&str]| -> Option<String> {
        v.as_str()
            .filter(|s| allowed.contains(s))
            .map(str::to_string)
    };
    let claims = raw.get("unsupported_claims");
    let context = raw.get("wrong_context");
    let mut out = Map::new();
    out.insert(
        "unsupported_claims".to_string(),
        match claims {
            Some(c) if c.is_object() => json!({
                "verdict": verdict_of(&c["verdict"], &["none", "possible", "likely"]).unwrap_or_else(|| "possible".into()),
                "reasoning": c.get("reasoning").and_then(|v| v.as_str()).unwrap_or("").chars().take(500).collect::<String>(),
                "excerpt": c.get("excerpt").and_then(|v| v.as_str()).unwrap_or("").chars().take(240).collect::<String>(),
            }),
            _ => Value::Null,
        },
    );
    out.insert(
        "wrong_context".to_string(),
        match context {
            Some(c) if c.is_object() => json!({
                "verdict": verdict_of(&c["verdict"], &["no", "possible", "yes"]).unwrap_or_else(|| "possible".into()),
                "reasoning": c.get("reasoning").and_then(|v| v.as_str()).unwrap_or("").chars().take(500).collect::<String>(),
                "excerpt": c.get("excerpt").and_then(|v| v.as_str()).unwrap_or("").chars().take(240).collect::<String>(),
            }),
            _ => Value::Null,
        },
    );
    Some(Value::Object(out))
}

fn ai_checks(layer: &Value) -> Vec<Value> {
    let mut out = Vec::new();
    if let Some(c) = layer.get("unsupported_claims") {
        if !c.is_null() {
            let verdict = c["verdict"].as_str().unwrap_or("possible");
            out.push(result(
                "unsupported_claims",
                if verdict == "none" { "pass" } else { "flagged" },
                format!(
                    "AI review verdict: {}. {}",
                    verdict,
                    c["reasoning"].as_str().unwrap_or("")
                ),
                if verdict == "none" {
                    vec![]
                } else {
                    vec![json!({
                        "draft_excerpt": if c["excerpt"].as_str().unwrap_or("").is_empty() { "(see reasoning)".to_string() } else { c["excerpt"].as_str().unwrap().to_string() },
                        "evidence": [evidence("local model reasoning", c["reasoning"].as_str().unwrap_or(""), None, None, None)],
                        "advice": "Verify the claim against conversation or knowledge evidence, or soften it (\"typically\", \"in most cases\")."
                    })]
                },
                false,
            ));
        }
    }
    if let Some(c) = layer.get("wrong_context") {
        if !c.is_null() {
            let verdict = c["verdict"].as_str().unwrap_or("possible");
            out.push(result(
                "wrong_customer_context",
                if verdict == "no" { "pass" } else { "flagged" },
                format!(
                    "AI context review verdict: {}. {}",
                    verdict,
                    c["reasoning"].as_str().unwrap_or("")
                ),
                if verdict == "no" {
                    vec![]
                } else {
                    vec![json!({
                        "draft_excerpt": if c["excerpt"].as_str().unwrap_or("").is_empty() { "(see reasoning)".to_string() } else { c["excerpt"].as_str().unwrap().to_string() },
                        "evidence": [evidence("local model reasoning", c["reasoning"].as_str().unwrap_or(""), None, None, None)],
                        "advice": "Re-read the customer's last message - the draft may be answering a different question than the one asked."
                    })]
                },
                true,
            ));
        }
    }
    out
}

async fn compute_ai_layer(
    conn: &Connection,
    chat: Option<CoachingChatFn<'_>>,
    conv: &ConvRow,
    threads: &[ThreadRow],
    draft: &str,
) -> (Option<Value>, Option<String>, Option<i64>) {
    let Some(chat) = chat else {
        return (
            None,
            Some(
                "AI coaching is unavailable (no local model configured). The deterministic checks were computed."
                    .into(),
            ),
            None,
        );
    };
    let recent: Vec<String> = threads
        .iter()
        .rev()
        .take(8)
        .rev()
        .map(|t| {
            let text: String = t.text.chars().take(700).collect();
            let who = if t.kind == "customer" {
                "CUSTOMER"
            } else if t.kind == "reply" {
                "AGENT"
            } else {
                "NOTE"
            };
            format!("{who}: {text}")
        })
        .collect();
    let system = [
        "You are a pre-send review assistant for a support agent. You see one draft reply and the recent conversation.",
        "Check exactly two things:",
        "1. unsupported_claims: does the draft assert specific facts (version numbers, causes, guarantees, policy statements) that are NOT supported by the conversation evidence?",
        "2. wrong_context: does the draft answer the wrong question, an outdated question, or a different customer issue?",
        "Answer strictly as JSON: {\"unsupported_claims\":{\"verdict\":\"none|possible|likely\",\"reasoning\":\"...\",\"excerpt\":\"...\"},\"wrong_context\":{\"verdict\":\"no|possible|yes\",\"reasoning\":\"...\",\"excerpt\":\"...\"}}.",
        "Be conservative: \"none\"/\"no\" unless the mismatch is clear. Quote the draft verbatim in excerpt (max 200 chars).",
    ]
    .join("\n");
    let user = [
        format!(
            "CONVERSATION #{} SUBJECT: {}",
            conv.number,
            conv.subject.as_deref().unwrap_or("(none)")
        ),
        "RECENT MESSAGES:".to_string(),
        recent.join("\n"),
        "DRAFT TO REVIEW:".to_string(),
        draft.chars().take(6000).collect::<String>(),
        "Respond with the JSON object only.".to_string(),
    ]
    .join("\n");

    let mut hasher = Sha256::new();
    hasher.update(&user);
    let input_hash = format!("{:x}", hasher.finalize());
    let run_id = crate::ai_pipeline::start_run(
        conn,
        "agent_coaching",
        Some(conv.id),
        None,
        PROMPT_VERSION,
        Some(&input_hash),
        &json!({ "conversationId": conv.id, "promptVersion": PROMPT_VERSION, "inputHash": input_hash }),
    )
    .ok();
    let messages = vec![
        crate::ai_provider::ChatMessage {
            role: "system".into(),
            content: system,
        },
        crate::ai_provider::ChatMessage {
            role: "user".into(),
            content: user,
        },
    ];
    match chat(messages, 0.1, 700, true).await {
        Ok((content, model, latency)) => {
            let parsed = coerce_ai_layer(content.as_deref().unwrap_or(""));
            match parsed {
                None => {
                    if let Some(run_id) = run_id {
                        let _ = crate::ai_pipeline::fail_run(
                            conn,
                            run_id,
                            "unparseable coaching output",
                        );
                    }
                    (
                        None,
                        Some("The local model returned an unparseable coaching result; nothing was guessed. The deterministic checks were computed.".into()),
                        None,
                    )
                }
                Some(mut layer) => {
                    if let (Some(run_id), Some(obj)) = (run_id, layer.as_object_mut()) {
                        obj.insert("model".into(), json!(model));
                        let _ =
                            crate::ai_pipeline::complete_run(conn, run_id, &json!(model), latency);
                    }
                    (Some(layer), None, run_id)
                }
            }
        }
        Err(e) => {
            if let Some(run_id) = run_id {
                let _ = crate::ai_pipeline::fail_run(conn, run_id, &e);
            }
            (
                None,
                Some(format!(
                    "{e} The deterministic checks were computed and stored."
                )),
                None,
            )
        }
    }
}

pub fn get(conn: &Connection, conversation_id: i64) -> Result<Option<Value>> {
    let row: Option<(String, String, Option<String>, i64, i64, String)> = conn
        .query_row(
            "SELECT draft_sha256, deterministic, ai, draft_chars, draft_words, created_at
             FROM coaching_reviews WHERE conversation_id = ?1",
            params![conversation_id],
            |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                ))
            },
        )
        .ok();
    let Some((draft_sha256, deterministic, ai_stored, draft_chars, draft_words, created_at)) = row
    else {
        return Ok(None);
    };
    let mut checks: Vec<Value> = serde_json::from_str::<Value>(&deterministic)
        .ok()
        .and_then(|v| v.as_array().cloned())
        .unwrap_or_default();
    let mut ai = json!({ "available": false, "model": Value::Null, "error": Value::Null });
    if let Some(ai_stored) = ai_stored {
        if let Ok(parsed) = serde_json::from_str::<Value>(&ai_stored) {
            if let Some(error) = parsed.get("error").filter(|e| !e.is_null()) {
                ai["model"] = parsed.get("model").cloned().unwrap_or(Value::Null);
                ai["error"] = json!(error);
            } else {
                ai = json!({
                    "available": true,
                    "model": parsed.get("model").cloned().unwrap_or(Value::Null),
                    "error": Value::Null
                });
                checks.extend(ai_checks(&parsed));
            }
        } else {
            ai["error"] = json!("stored AI layer was unparseable");
        }
    }
    Ok(Some(json!({
        "conversation_id": conversation_id,
        "draft_sha256": draft_sha256,
        "draft_chars": draft_chars,
        "draft_words": draft_words,
        "checks": checks,
        "ai": ai,
        "summary": summarize(&checks),
        "note": COACHING_NOTE,
        "reviewed_at": created_at
    })))
}

pub async fn review_draft(
    conn: &Connection,
    chat: Option<CoachingChatFn<'_>>,
    conversation_id: i64,
    draft: &str,
    include_ai: bool,
) -> Result<std::result::Result<Value, &'static str>> {
    let Some(conv) = load_conv(conn, conversation_id) else {
        return Ok(Err("not_found"));
    };
    let trimmed = draft.trim();
    if trimmed.is_empty() {
        return Ok(Err("empty_draft"));
    }
    let threads = load_thread_rows(conn, conversation_id);
    let customer: Option<(String, Option<String>)> = conv
        .customer_id
        .and_then(|id| {
            conn.query_row(
                "SELECT first_name, last_name FROM customers WHERE id = ?1",
                params![id],
                |r| {
                    Ok((
                        r.get::<_, Option<String>>(0)?,
                        r.get::<_, Option<String>>(1)?,
                    ))
                },
            )
            .ok()
        })
        .map(|(first, last)| (first.unwrap_or_default(), last));
    let signals = latest_signals(conn, conversation_id);
    let preference = conv
        .customer_id
        .and_then(|id| customer_preference(conn, id));

    let mut checks: Vec<Value> = vec![
        check_unanswered_questions(&conv, &threads, trimmed),
        check_duplicated_questions(&threads, trimmed),
        check_timeframe(conn, &conv, trimmed),
        check_acknowledgment(&threads, &signals, trimmed),
        check_wording(preference.as_ref(), trimmed),
        check_detail(&signals, &threads, trimmed),
        check_internal_leakage(conn, &conv, &threads, trimmed),
        check_wrong_context(conn, &conv, customer.as_ref(), trimmed),
        check_preference_mismatch(preference.as_ref(), &threads, trimmed),
    ];

    let mut ai = json!({ "available": false, "model": Value::Null, "error": Value::Null });
    let mut ai_stored: Option<String> = None;
    let mut ai_run_id: Option<i64> = None;
    if include_ai {
        let (layer, error, run_id) = compute_ai_layer(conn, chat, &conv, &threads, trimmed).await;
        ai_run_id = run_id;
        match (layer, error) {
            (Some(layer), None) => {
                ai = json!({
                    "available": true,
                    "model": layer.get("model").cloned().unwrap_or(Value::Null),
                    "error": Value::Null
                });
                ai_stored = serde_json::to_string(&layer).ok();
                checks.extend(ai_checks(&layer));
            }
            (_, Some(error)) => {
                ai["error"] = json!(error);
                ai_stored = serde_json::to_string(&json!({ "error": error, "model": null })).ok();
            }
            (None, None) => {}
        }
    }

    let draft_words = word_count(trimmed);
    let mut hasher = Sha256::new();
    hasher.update(trimmed.as_bytes());
    let draft_sha256 = format!("{:x}", hasher.finalize());
    let deterministic_only: Vec<Value> = checks
        .iter()
        .filter(|c| c.get("layer").and_then(|l| l.as_str()) == Some("deterministic"))
        .cloned()
        .collect();
    conn.execute(
        "INSERT INTO coaching_reviews (conversation_id, draft_sha256, draft_excerpt, deterministic,
                                        ai, ai_run_id, draft_chars, draft_words, created_at, provenance)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, datetime('now'), 'deterministic_local')
         ON CONFLICT (conversation_id) DO UPDATE SET
           draft_sha256 = excluded.draft_sha256,
           draft_excerpt = excluded.draft_excerpt,
           deterministic = excluded.deterministic,
           ai = excluded.ai,
           ai_run_id = excluded.ai_run_id,
           draft_chars = excluded.draft_chars,
           draft_words = excluded.draft_words,
           created_at = datetime('now')",
        params![
            conversation_id,
            draft_sha256,
            trimmed.chars().take(300).collect::<String>(),
            serde_json::to_string(&deterministic_only).unwrap_or_else(|_| "[]".into()),
            ai_stored,
            ai_run_id,
            trimmed.chars().count() as i64,
            draft_words as i64,
        ],
    )?;
    let _ = HashMap::<String, String>::new();

    Ok(Ok(json!({
        "conversation_id": conversation_id,
        "draft_sha256": draft_sha256,
        "draft_chars": trimmed.chars().count(),
        "draft_words": draft_words,
        "checks": checks,
        "ai": ai,
        "summary": summarize(&checks),
        "note": COACHING_NOTE,
        "reviewed_at": sqlite_now_compact()
    })))
}

fn sqlite_now_compact() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs();
    let days = i64::try_from(secs / 86_400).unwrap_or(0);
    let rem = secs % 86_400;
    let (h, mi, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02} {h:02}:{mi:02}:{s:02}")
}
