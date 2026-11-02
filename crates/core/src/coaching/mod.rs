use std::collections::{HashMap, HashSet};

use regex::Regex;
use rusqlite::{Connection, params};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

use crate::error::Result;

pub const COACHING_NOTE: &str = "Advisory only - coaching never blocks or modifies the send. Every finding cites its evidence; heuristics are labeled as heuristics.";
const PROMPT_VERSION: &str = "agent_coaching_v1";

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
                t.text = crate::textutil::html_to_text(&t.text);
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

mod checks;

use checks::{
    check_acknowledgment, check_detail, check_duplicated_questions, check_internal_leakage,
    check_preference_mismatch, check_timeframe, check_unanswered_questions, check_wording,
    check_wrong_context,
};

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
    if let Some(c) = layer.get("unsupported_claims")
        && !c.is_null()
    {
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
    if let Some(c) = layer.get("wrong_context")
        && !c.is_null()
    {
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
    let input_hash = crate::crypto::hex(&hasher.finalize());
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
    let draft_sha256 = crate::crypto::hex(&hasher.finalize());
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
