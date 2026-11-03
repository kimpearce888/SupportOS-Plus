use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};

use crate::error::Result;

const CLIENT_CURRENT_SIGNALS_SQL: &str = "
        CREATE TABLE IF NOT EXISTS client_current_signals (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        conversation_id INTEGER NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
        customer_id INTEGER REFERENCES customers(id) ON DELETE CASCADE,
        signals_json TEXT NOT NULL,
        message_stats_json TEXT,
        customer_goal TEXT,
        sources TEXT NOT NULL DEFAULT 'heuristic',
        analysis_version TEXT,
        generated_at TEXT NOT NULL DEFAULT (datetime('now')),
        provenance TEXT NOT NULL DEFAULT 'heuristic',
        UNIQUE (conversation_id)
        );
        CREATE INDEX IF NOT EXISTS idx_client_current_signals_customer
            ON client_current_signals(customer_id);
        CREATE UNIQUE INDEX IF NOT EXISTS idx_client_current_signals_unique
            ON client_current_signals(conversation_id);
";

pub(crate) fn ensure_client_current_signals_table(conn: &Connection) -> Result<()> {
    conn.execute_batch(CLIENT_CURRENT_SIGNALS_SQL)?;
    Ok(())
}

const INTERACTION_EVIDENCE_SQL: &str = "
        CREATE TABLE IF NOT EXISTS interaction_evidence (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        conversation_id INTEGER NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
        evidence_type TEXT NOT NULL,
        evidence_data TEXT NOT NULL,
        created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
        );
        CREATE INDEX IF NOT EXISTS idx_interaction_evidence_conversation
            ON interaction_evidence(conversation_id);
";

pub(crate) fn ensure_interaction_evidence_table(conn: &Connection) -> Result<()> {
    conn.execute_batch(INTERACTION_EVIDENCE_SQL)?;
    Ok(())
}

fn replace_evidence_rows(
    conn: &Connection,
    conversation_id: i64,
    signals: &[InteractionSignal],
) -> Result<()> {
    ensure_interaction_evidence_table(conn)?;
    conn.execute(
        "DELETE FROM interaction_evidence WHERE conversation_id = ?1",
        params![conversation_id],
    )?;
    for s in signals {
        let Some(ev) = &s.evidence else { continue };
        conn.execute(
            "INSERT INTO interaction_evidence (conversation_id, evidence_type, evidence_data)
             VALUES (?1, ?2, ?3)",
            params![
                conversation_id,
                s.dimension,
                serde_json::json!({
                    "excerpt": ev.excerpt,
                    "thread_local_id": ev.thread_local_id,
                    "conversation_local_id": ev.conversation_local_id,
                    "value": s.value,
                    "confidence": s.confidence,
                    "source": s.source,
                })
                .to_string()
            ],
        )?;
    }
    Ok(())
}

const TONE_VALUES: [&str; 8] = [
    "neutral",
    "friendly",
    "frustrated",
    "appreciative",
    "disappointed",
    "confrontational",
    "urgent",
    "uncertain",
];
const DIRECTNESS_VALUES: [&str; 4] = ["indirect", "conversational", "direct", "highly_direct"];
const DETAIL_VALUES: [&str; 5] = ["very_low", "low", "moderate", "high", "very_high"];
const TECHNICAL_VALUES: [&str; 4] = ["non_technical", "mixed", "technical", "highly_technical"];
const QUESTION_STRUCTURE_VALUES: [&str; 5] = [
    "single_question",
    "multiple_questions",
    "troubleshooting_oriented",
    "confirmation_oriented",
    "explanation_oriented",
];
const URGENCY_VALUES: [&str; 4] = ["none", "low", "moderate", "high"];
const FRUSTRATION_VALUES: [&str; 4] = ["none", "possible", "moderate", "strong"];
const EXPECTATION_VALUES: [&str; 7] = [
    "information",
    "explanation",
    "troubleshooting",
    "action",
    "immediate_resolution",
    "escalation",
    "confirmation",
];
const RESPONSE_PREFERENCE_VALUES: [&str; 6] = [
    "concise",
    "detailed",
    "step_by_step",
    "technical",
    "conversational",
    "outcome_focused",
];

pub(crate) fn dimension_vocabulary(dimension: &str) -> Option<&'static [&'static str]> {
    match dimension {
        "tone" => Some(&TONE_VALUES),
        "directness" => Some(&DIRECTNESS_VALUES),
        "detail" => Some(&DETAIL_VALUES),
        "technical_language" => Some(&TECHNICAL_VALUES),
        "question_structure" => Some(&QUESTION_STRUCTURE_VALUES),
        "urgency" => Some(&URGENCY_VALUES),
        "frustration" => Some(&FRUSTRATION_VALUES),
        "expectation" => Some(&EXPECTATION_VALUES),
        "response_preference" => Some(&RESPONSE_PREFERENCE_VALUES),
        _ => None,
    }
}

pub(crate) fn needs_de_escalation(signals: &[InteractionSignal]) -> bool {
    let frustration = |level: &str| {
        signals
            .iter()
            .any(|s| s.dimension == "frustration" && s.value == level)
    };
    frustration("strong")
        || (signals
            .iter()
            .any(|s| s.dimension == "urgency" && s.value == "high")
            && (frustration("moderate") || frustration("strong")))
}

pub(crate) fn is_valid_value(dimension: &str, value: &str) -> bool {
    let vocab: &[&str] = match dimension {
        "tone" => &TONE_VALUES,
        "directness" => &DIRECTNESS_VALUES,
        "detail" => &DETAIL_VALUES,
        "technical_language" => &TECHNICAL_VALUES,
        "question_structure" => &QUESTION_STRUCTURE_VALUES,
        "urgency" => &URGENCY_VALUES,
        "frustration" => &FRUSTRATION_VALUES,
        "expectation" => &EXPECTATION_VALUES,
        "response_preference" => &RESPONSE_PREFERENCE_VALUES,
        _ => return false,
    };
    vocab.contains(&value)
}

const TECHNICAL_VOCAB: &[&str] = &[
    "api",
    "endpoint",
    "webhook",
    "payload",
    "json",
    "oauth",
    "token",
    "http",
    "https",
    "ssl",
    "tls",
    "dns",
    "timezone",
    "utc",
    "cron",
    "queue",
    "cache",
    "latency",
    "http status",
    "401",
    "403",
    "404",
    "500",
    "console",
    "log",
    "stack trace",
    "exception",
    "database",
    "sql",
    "index",
    "migration",
    "deploy",
    "build",
    "header",
    "request",
    "response",
    "callback",
    "integration",
    "sdk",
    "environment variable",
    "rate limit",
];
const URGENCY_MARKERS: &[&str] = &[
    "urgent",
    "asap",
    "immediately",
    "right now",
    "today",
    "as soon as possible",
    "deadline",
    "before our",
    "blocking",
    "production is down",
    "outage",
    "critical",
];
const FRUSTRATION_MARKERS: &[&str] = &[
    "again",
    "still",
    "already",
    "third time",
    "multiple times",
    "repeatedly",
    "frustrat",
    "unacceptable",
    "ridiculous",
    "seriously",
    "not working",
    "never works",
    "every time",
    "tired of",
    "disappointed",
    "kept",
    "nobody",
    "no one",
    "still not",
];
const DIRECT_MARKERS: &[&str] = &[
    "need you to",
    "fix",
    "tell me",
    "send me",
    "do not",
    "stop",
    "require",
    "must",
    "want",
    "instead of",
];
const INDIRECT_MARKERS: &[&str] = &[
    "i was wondering",
    "if possible",
    "could you perhaps",
    "when you have a moment",
    "sorry to bother",
    "not sure if",
    "might be able",
    "hopefully",
    "any chance",
];
const ACTION_EXPECTATION_MARKERS: &[&str] = &[
    "fix",
    "resolve",
    "restore",
    "refund",
    "escalate",
    "asap",
    "immediately",
    "compensation",
];
const ESCALATION_MARKERS: &[&str] = &[
    "escalate",
    "manager",
    "supervisor",
    "complaint",
    "legal",
    "cancel our",
    "terminate",
    "switching to",
];

#[derive(Debug, Clone)]
pub(crate) struct MessageForAnalysis {
    pub text: String,
    pub thread_local_id: Option<i64>,
    pub conversation_local_id: Option<i64>,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct MessageStats {
    pub customer_messages: usize,
    pub avg_message_length: usize,
    pub question_count: usize,
    pub exclamation_ratio: f64,
    pub caps_ratio: f64,
}

#[must_use]
pub(crate) fn compute_message_stats(messages: &[MessageForAnalysis]) -> MessageStats {
    let texts: Vec<&str> = messages.iter().map(|m| m.text.as_str()).collect();
    let total_len: usize = texts.iter().map(|t| t.chars().count()).sum();
    let all = texts.join("\n");
    let question_count = all.matches('?').count();
    let exclamations = all.matches('!').count();
    let letters: String = all.chars().filter(|c| c.is_ascii_alphabetic()).collect();
    let caps_letters: String = all.chars().filter(|c| c.is_ascii_uppercase()).collect();
    let (final_cur, final_len, repeated) =
        all.chars()
            .fold((None, 0usize, 0usize), |(cur, len, count), c| {
                if c == '!' || c == '?' {
                    match cur {
                        Some(prev) if prev == c => (Some(c), len + 1, count),
                        Some(_) => (Some(c), 1, count + usize::from(len >= 2)),
                        None => (Some(c), 1, count),
                    }
                } else {
                    (None, 0, count + usize::from(cur.is_some() && len >= 2))
                }
            });
    let repeated = repeated + usize::from(final_cur.is_some() && final_len >= 2);
    MessageStats {
        customer_messages: messages.len(),
        avg_message_length: if messages.is_empty() {
            0
        } else {
            total_len / messages.len()
        },
        question_count,
        exclamation_ratio: if texts.is_empty() {
            0.0
        } else {
            round2((exclamations + repeated * 2) as f64 / texts.len() as f64)
        },
        caps_ratio: if letters.is_empty() {
            0.0
        } else {
            round3(caps_letters.chars().count() as f64 / letters.chars().count() as f64)
        },
    }
}

pub(crate) fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

fn round3(v: f64) -> f64 {
    (v * 1000.0).round() / 1000.0
}

fn is_word_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

fn has_marker(text: &str, marker: &str) -> bool {
    let hay = text.to_lowercase();
    let needle = marker.to_lowercase();
    let hay_bytes = hay.as_bytes();
    let needle_bytes = needle.as_bytes();
    if needle_bytes.is_empty() || hay_bytes.len() < needle_bytes.len() {
        return false;
    }
    let mut start = 0;
    while let Some(pos) = hay[start..].find(&needle) {
        let at = start + pos;
        let end = at + needle_bytes.len();
        let before_ok = at == 0 || !is_word_char(hay_bytes[at - 1] as char);
        let after_ok = end == hay_bytes.len() || !is_word_char(hay_bytes[end] as char);
        if before_ok && after_ok {
            return true;
        }
        start = at + 1;
        if start >= hay_bytes.len() {
            break;
        }
    }
    false
}

fn count_markers(text: &str, markers: &[&str]) -> usize {
    markers.iter().filter(|m| has_marker(text, m)).count()
}

fn matches_any(text: &str, markers: &[&str]) -> bool {
    markers.iter().any(|m| has_marker(text, m))
}

fn excerpt(text: &str, max_len: usize) -> String {
    let clean: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if clean.chars().count() <= max_len {
        clean
    } else {
        let cut: String = clean.chars().take(max_len - 3).collect();
        format!("{cut}…")
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Evidence {
    pub excerpt: String,
    pub thread_local_id: Option<i64>,
    pub conversation_local_id: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InteractionSignal {
    pub dimension: String,
    pub value: String,
    pub confidence: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evidence: Option<Evidence>,
    pub source: String,
}

macro_rules! cached_regex {
    ($name:ident, $pattern:expr) => {
        fn $name() -> &'static regex::Regex {
            static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
            RE.get_or_init(|| regex::Regex::new($pattern).expect("valid classification regex"))
        }
    };
}
cached_regex!(
    tone_appreciative_re,
    r"thank(s| you)? for|appreciate|great (work|job|support|help)|awesome|excellent"
);
cached_regex!(tone_evidence_re, r"thank(s| you)? for|appreciate");
cached_regex!(
    question_troubleshooting_re,
    r"error|broken|failing|not work|issue|problem"
);
cached_regex!(urgency_severe_re, r"production is down|outage|emergency");
cached_regex!(
    expectation_info_re,
    r"how do i|how can i|what is|when will|where is|which|why|explain|reason|cause"
);
cached_regex!(
    expectation_info_strict_re,
    r"how do i|how can i|what is|when will|where is|which"
);
cached_regex!(expectation_explain_re, r"why|explain|reason|cause");
cached_regex!(
    expectation_evidence_re,
    r"why|explain|how do|how can|when will|where is|which|what is"
);
cached_regex!(
    pref_concise_re,
    r"keep (it |this )?(short|brief)|concise|short answer|no lengthy|brief answer|be brief"
);
cached_regex!(
    pref_step_by_step_re,
    r"step by step|step-by-step|walk me through|instructions"
);
cached_regex!(
    pref_detailed_re,
    r"detailed|thorough|in depth|in-depth|full explanation|comprehensive"
);

fn find_evidence(
    messages: &[MessageForAnalysis],
    predicate: impl Fn(&str) -> bool,
) -> Option<Evidence> {
    for m in messages {
        if predicate(&m.text.to_lowercase()) {
            return Some(Evidence {
                excerpt: excerpt(&m.text, 220),
                thread_local_id: m.thread_local_id,
                conversation_local_id: m.conversation_local_id,
            });
        }
    }
    None
}

fn tone_evidence_predicate(t: &str) -> bool {
    matches_any(t, FRUSTRATION_MARKERS)
        || matches_any(t, URGENCY_MARKERS)
        || tone_evidence_re().is_match(t)
}

#[must_use]
pub(crate) fn heuristic_signals(
    messages: &[MessageForAnalysis],
    stats: &MessageStats,
) -> Vec<InteractionSignal> {
    if messages.is_empty() {
        return Vec::new();
    }
    let all: String = messages
        .iter()
        .map(|m| m.text.as_str())
        .collect::<Vec<_>>()
        .join("\n")
        .to_lowercase();
    let mut signals: Vec<InteractionSignal> = Vec::new();
    let mut push = |dimension: &str, value: &str, confidence: &str, evidence: Option<Evidence>| {
        if is_valid_value(dimension, value) {
            signals.push(InteractionSignal {
                dimension: dimension.to_string(),
                value: value.to_string(),
                confidence: confidence.to_string(),
                evidence,
                source: "heuristic".to_string(),
            });
        }
    };

    let tone = if matches_any(&all, FRUSTRATION_MARKERS) {
        "frustrated"
    } else if matches_any(&all, URGENCY_MARKERS) {
        "urgent"
    } else if tone_appreciative_re().is_match(&all) {
        "appreciative"
    } else {
        "neutral"
    };
    push(
        "tone",
        tone,
        if tone == "neutral" { "low" } else { "medium" },
        find_evidence(messages, tone_evidence_predicate),
    );

    let direct_hits = count_markers(&all, DIRECT_MARKERS);
    let indirect_hits = count_markers(&all, INDIRECT_MARKERS);
    let directness = if direct_hits >= 2 {
        "highly_direct"
    } else if direct_hits == 1 && indirect_hits == 0 {
        "direct"
    } else {
        "conversational"
    };
    push(
        "directness",
        directness,
        if direct_hits + indirect_hits >= 1 {
            "medium"
        } else {
            "low"
        },
        find_evidence(messages, |t| {
            matches_any(t, DIRECT_MARKERS) || matches_any(t, INDIRECT_MARKERS)
        }),
    );

    let avg_len = stats.avg_message_length;
    let detail = if avg_len > 900 {
        "very_high"
    } else if avg_len > 450 {
        "high"
    } else if avg_len > 150 {
        "moderate"
    } else if avg_len > 60 {
        "low"
    } else {
        "very_low"
    };
    let first_msg = messages.first();
    push(
        "detail",
        detail,
        if avg_len > 150 { "medium" } else { "low" },
        first_msg.map(|m| Evidence {
            excerpt: excerpt(&m.text, 220),
            thread_local_id: m.thread_local_id,
            conversation_local_id: m.conversation_local_id,
        }),
    );

    let tech_hits = count_markers(&all, TECHNICAL_VOCAB);
    let technical = if tech_hits >= 4 {
        "highly_technical"
    } else if tech_hits >= 2 {
        "technical"
    } else if tech_hits == 1 {
        "mixed"
    } else {
        "non_technical"
    };
    push(
        "technical_language",
        technical,
        if tech_hits >= 1 { "medium" } else { "low" },
        find_evidence(messages, |t| matches_any(t, TECHNICAL_VOCAB)),
    );

    let question_structure = if stats.question_count >= 3 || stats.question_count == 2 {
        "multiple_questions"
    } else if stats.question_count == 1 {
        "single_question"
    } else if question_troubleshooting_re().is_match(&all) {
        "troubleshooting_oriented"
    } else {
        "explanation_oriented"
    };
    push(
        "question_structure",
        question_structure,
        if stats.question_count >= 1 {
            "medium"
        } else {
            "low"
        },
        find_evidence(messages, |t| t.contains('?')),
    );

    let urgency_hits = count_markers(&all, URGENCY_MARKERS);
    let urgency = if urgency_hits >= 3 || urgency_severe_re().is_match(&all) {
        "high"
    } else if urgency_hits >= 1 {
        "moderate"
    } else {
        "none"
    };
    push(
        "urgency",
        urgency,
        if urgency_hits >= 1 { "medium" } else { "low" },
        find_evidence(messages, |t| matches_any(t, URGENCY_MARKERS)),
    );

    let frustration_hits = count_markers(&all, FRUSTRATION_MARKERS);
    let frustration = if frustration_hits >= 3 {
        "strong"
    } else if frustration_hits >= 1 {
        "moderate"
    } else {
        "none"
    };
    push(
        "frustration",
        frustration,
        if frustration_hits >= 1 {
            "medium"
        } else {
            "low"
        },
        find_evidence(messages, |t| matches_any(t, FRUSTRATION_MARKERS)),
    );

    let expectation_marker_hit = matches_any(&all, ESCALATION_MARKERS)
        || count_markers(&all, ACTION_EXPECTATION_MARKERS) >= 1
        || expectation_info_re().is_match(&all);
    let expectation = if matches_any(&all, ESCALATION_MARKERS) {
        "escalation"
    } else if count_markers(&all, ACTION_EXPECTATION_MARKERS) >= 2
        || (urgency == "high" && matches_any(&all, ACTION_EXPECTATION_MARKERS))
    {
        "immediate_resolution"
    } else if matches_any(&all, ACTION_EXPECTATION_MARKERS) {
        "action"
    } else if expectation_info_strict_re().is_match(&all) {
        "information"
    } else if expectation_explain_re().is_match(&all) {
        "explanation"
    } else {
        "information"
    };
    push(
        "expectation",
        expectation,
        if expectation_marker_hit {
            "medium"
        } else {
            "low"
        },
        find_evidence(messages, |t| {
            matches_any(t, ACTION_EXPECTATION_MARKERS)
                || matches_any(t, ESCALATION_MARKERS)
                || expectation_evidence_re().is_match(t)
        }),
    );

    signals
}

#[derive(Debug, Clone)]
pub(crate) struct SanitizedSignals {
    pub signals: Vec<InteractionSignal>,
}

#[must_use]
pub(crate) fn sanitize_signals(signals: Vec<InteractionSignal>) -> SanitizedSignals {
    let mut kept: Vec<InteractionSignal> = Vec::new();
    let mut seen: Vec<(String, String)> = Vec::new();
    for s in signals {
        let key = (s.dimension.clone(), s.value.clone());
        if seen.contains(&key) {
            continue;
        }
        if !is_valid_value(&s.dimension, &s.value) {
            continue;
        }
        let has_evidence = s.evidence.as_ref().is_some_and(|e| !e.excerpt.is_empty());
        if (s.confidence == "high" || s.confidence == "medium") && !has_evidence {
            continue;
        }
        seen.push(key);
        kept.push(s);
    }
    SanitizedSignals { signals: kept }
}

#[must_use]
pub(crate) fn explicit_current_preference(
    messages: &[MessageForAnalysis],
) -> Option<(String, Option<Evidence>)> {
    let all: String = messages
        .iter()
        .map(|m| m.text.as_str())
        .collect::<Vec<_>>()
        .join("\n")
        .to_lowercase();
    let prefs: [(&regex::Regex, &str); 3] = [
        (pref_concise_re(), "concise"),
        (pref_step_by_step_re(), "step_by_step"),
        (pref_detailed_re(), "detailed"),
    ];
    for (re, value) in prefs {
        if re.is_match(&all) {
            let evidence = find_evidence(messages, |t| re.is_match(t));
            return Some((value.to_string(), evidence));
        }
    }
    None
}

#[must_use]
pub(crate) fn infer_customer_goal(
    signals: &[InteractionSignal],
    subject: Option<&str>,
) -> Option<String> {
    let expectation = signals
        .iter()
        .find(|s| s.dimension == "expectation")
        .map(|s| s.value.as_str());
    match expectation {
        Some(exp) => match exp {
            "immediate_resolution" => Some("Get the issue resolved immediately"),
            "action" => Some("Get a concrete action taken"),
            "escalation" => Some("Get the issue escalated"),
            "explanation" => Some("Understand why this is happening"),
            "information" => Some("Get specific information"),
            "troubleshooting" => Some("Get help troubleshooting"),
            "confirmation" => Some("Confirm a suspected behavior"),
            _ => None,
        }
        .map(String::from),
        None => subject.map(|s| format!("Address: {s}")),
    }
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct CurrentInteraction {
    pub conversation_local_id: i64,
    pub customer_local_id: Option<i64>,
    pub is_returning_client: bool,
    pub signals: Vec<InteractionSignal>,
    pub customer_goal: Option<String>,
    pub message_stats: MessageStats,
    #[serde(skip_serializing_if = "String::is_empty", default)]
    pub sources: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub generated_at: Option<String>,
}

type CustomerThreadRow = (i64, Option<String>, Option<String>, Option<String>, String);

pub(crate) fn customer_messages(
    conn: &Connection,
    conversation_id: i64,
) -> Result<Vec<MessageForAnalysis>> {
    let mut stmt = conn.prepare(
        "SELECT id, body_html, body_text, remote_created_at, created_at
           FROM conversation_threads
          WHERE conversation_id = ?1
            AND type = 'customer'
            AND state = 'published'
          ORDER BY COALESCE(remote_created_at, created_at) ASC",
    )?;
    let rows: Vec<CustomerThreadRow> = stmt
        .query_map(params![conversation_id], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get::<_, Option<String>>(4)?.unwrap_or_default(),
            ))
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows
        .into_iter()
        .filter_map(|(id, body_html, body, _remote_created_at, _created_at)| {
            let raw = body_html
                .as_deref()
                .filter(|h| !h.is_empty())
                .or(body.as_deref())
                .unwrap_or("");
            let text = crate::textutil::html_to_text(raw);
            if text.trim().is_empty() {
                None
            } else {
                Some(MessageForAnalysis {
                    text,
                    thread_local_id: Some(id),
                    conversation_local_id: Some(conversation_id),
                })
            }
        })
        .collect())
}

pub(crate) fn compute_current_interaction(
    conn: &Connection,
    conversation_id: i64,
) -> Result<Option<CurrentInteraction>> {
    let conv = conn
        .query_row(
            "SELECT customer_local_id, subject FROM conversations WHERE id = ?1 AND deleted_at IS NULL",
            params![conversation_id],
            |row| {
                Ok((
                    row.get::<_, Option<i64>>(0)?,
                    row.get::<_, Option<String>>(1)?,
                ))
            },
        )
        .ok();
    let Some((customer_local_id, subject)) = conv else {
        return Ok(None);
    };
    let messages = customer_messages(conn, conversation_id)?;
    let stats = compute_message_stats(&messages);
    let signals = sanitize_signals(heuristic_signals(&messages, &stats)).signals;
    let mut signals = signals;
    if let Some((pref, evidence)) = explicit_current_preference(&messages) {
        signals.push(InteractionSignal {
            dimension: "response_preference".to_string(),
            value: pref,
            confidence: "high".to_string(),
            evidence,
            source: "heuristic".to_string(),
        });
    }
    let is_returning_client = customer_local_id
        .map(|cid| {
            conn.query_row(
                "SELECT COUNT(*) FROM conversations WHERE customer_local_id = ?1 AND id != ?2",
                params![cid, conversation_id],
                |r| r.get::<_, i64>(0),
            )
            .map(|n| n > 0)
            .unwrap_or(false)
        })
        .unwrap_or(false);
    let goal = infer_customer_goal(&signals, subject.as_deref());
    Ok(Some(CurrentInteraction {
        conversation_local_id: conversation_id,
        customer_local_id,
        is_returning_client,
        signals,
        customer_goal: goal,
        message_stats: stats,
        sources: "heuristic".to_string(),
        generated_at: Some(chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)),
    }))
}

pub(crate) fn record_current_interaction(
    conn: &Connection,
    conversation_id: i64,
) -> Result<Option<CurrentInteraction>> {
    record_current_interaction_opts(conn, conversation_id, true)
}

pub(crate) fn record_current_interaction_opts(
    conn: &Connection,
    conversation_id: i64,
    rebuild_baseline: bool,
) -> Result<Option<CurrentInteraction>> {
    let Some(current) = compute_current_interaction(conn, conversation_id)? else {
        return Ok(None);
    };
    ensure_client_current_signals_table(conn)?;
    let signals_json = serde_json::to_string(&current.signals)?;
    let stats_json = serde_json::to_string(&current.message_stats)?;
    conn.execute(
        "INSERT INTO client_current_signals
             (conversation_id, customer_id, signals_json, message_stats_json, customer_goal,
              sources, analysis_version, generated_at, provenance)
         VALUES (?1, ?2, ?3, ?4, ?5, 'heuristic', 'heuristic_v1', datetime('now'), 'heuristic')
         ON CONFLICT (conversation_id) DO UPDATE SET
           customer_id = excluded.customer_id,
           signals_json = excluded.signals_json,
           message_stats_json = excluded.message_stats_json,
           customer_goal = excluded.customer_goal,
           sources = excluded.sources,
           analysis_version = excluded.analysis_version,
           generated_at = datetime('now'),
           provenance = excluded.provenance",
        params![
            conversation_id,
            current.customer_local_id,
            signals_json,
            stats_json,
            current.customer_goal,
        ],
    )?;
    let _ = replace_evidence_rows(conn, conversation_id, &current.signals);
    let _ = crate::interaction_engine::ensure_schema(conn);
    let _ = crate::interaction_engine::record_observations_for(conn, &current);
    if rebuild_baseline && let Some(cid) = current.customer_local_id {
        let _ = crate::interaction_engine::rebuild_baseline(conn, cid);
    }
    Ok(Some(current))
}

pub(crate) fn backfill_all(conn: &Connection) -> Result<usize> {
    let ids: Vec<i64> = {
        let mut stmt =
            conn.prepare("SELECT id FROM conversations WHERE deleted_at IS NULL ORDER BY id")?;

        stmt.query_map([], |r| r.get(0))?
            .filter_map(|r| r.ok())
            .collect()
    };
    let mut written = 0;
    for id in ids {
        if record_current_interaction(conn, id).is_ok_and(|c| c.is_some()) {
            written += 1;
        }
    }
    Ok(written)
}

#[cfg(test)]
mod batch30_tests {
    use super::*;
    use rusqlite::{Connection, params};

    #[test]
    fn customer_messages_order_chronologically_without_remote_stamps() {
        let mut conn = Connection::open_in_memory().expect("db");
        crate::bootstrap::apply_all(&mut conn).expect("migrations");
        conn.execute(
            "INSERT INTO conversations (remote_id, number, subject, preview, status)
             VALUES (900, 900, 'sequence', 'p', 'active')",
            [],
        )
        .expect("conversation");
        let conv: i64 = conn.last_insert_rowid();
        for (offset, body) in [
            ("-1 minutes", "newest"),
            ("-2 hours", "middle"),
            ("-2 days", "oldest"),
        ] {
            conn.execute(
                "INSERT INTO conversation_threads (conversation_id, type, state, body_text, created_at)
                 VALUES (?1, 'customer', 'published', ?2, datetime('now', ?3))",
                params![conv, body, offset],
            )
            .expect("thread");
        }

        let msgs = customer_messages(&conn, conv).expect("messages");
        assert_eq!(msgs.len(), 3);
        assert_eq!(msgs[0].text, "oldest");
        assert_eq!(msgs[1].text, "middle");
        assert_eq!(msgs[2].text, "newest");
    }
}
