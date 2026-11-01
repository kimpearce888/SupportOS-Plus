use super::*;

pub const ATTRIBUTE_EXTRACTION_PROMPT_VERSION: &str = "attribute_extraction_v1";

const ATTRIBUTE_EXTRACTION_SYSTEM: &str = "\
You extract structured attributes from a customer support conversation. You are part of a LOCAL-first support tool: everything you output is stored as versioned, evidence-backed local metadata - never sent to the customer.\n\
Rules:\n\
- Extract ONLY what the messages support. A missing attribute is simply omitted - never invent values.\n\
- \"intent\" MUST be exactly one of: question | bug_report | feature_request | billing | how_to | account_management | feedback | other.\n\
- \"response_style\" MUST be exactly one of: concise | detailed | step_by_step | technical | conversational | outcome_focused - pick the style the CUSTOMER's own messages ask for, not what you would write.\n\
- \"product\", \"feature\", \"issue\", \"customer_goal\" are short free text (max ~12 words each).\n\
- For every attribute with confidence \"medium\" or \"high\" you MUST include an evidence_excerpt copied from the messages provided, plus the thread id it came from.\n\
- Use confidence \"low\" when the signal is weak or ambiguous.\n\
- Respond with a single JSON object, no prose.\n\
\n\
JSON shape:\n\
{\"attributes\": [{\"attribute\": \"intent|product|feature|issue|customer_goal|response_style\", \"value\": \"...\", \"confidence\": \"high|medium|low|unknown\", \"evidence_excerpt\": \"...\", \"evidence_thread_local_id\": 123}]}";

#[derive(Debug, Clone, PartialEq)]
pub struct CustomerMessage {
    pub thread_local_id: i64,
    pub text: String,
}

pub fn customer_messages(conn: &Connection, conversation_id: i64) -> Result<Vec<CustomerMessage>> {
    // Clean-audit item 36: this window used to be the OLDEST 30 customer
    // messages INCLUDING soft-deleted threads — a long ticket's recent
    // context (the half that says what the customer wants now) was cut
    // off, and deleted threads kept feeding the prompt. The window is
    // now the NEWEST 30 live threads, presented oldest-first (the
    // prompt's declared order).
    let mut stmt = conn.prepare(
        "SELECT id, COALESCE(body_text, '') FROM conversation_threads
         WHERE conversation_id = ?1 AND type = 'customer'
           AND deleted_at IS NULL
           AND TRIM(COALESCE(body_text, ''), char(32) || char(9) || char(10) || char(13)) <> ''
         ORDER BY created_at DESC, id DESC LIMIT 30",
    )?;
    let mut rows: Vec<CustomerMessage> = stmt
        .query_map(params![conversation_id], |r| {
            Ok(CustomerMessage {
                thread_local_id: r.get(0)?,
                text: r.get(1)?,
            })
        })?
        .filter_map(|row| row.ok())
        .map(|m| {
            let text: String = m.text.chars().take(1200).collect();
            CustomerMessage {
                thread_local_id: m.thread_local_id,
                text,
            }
        })
        .filter(|m| !m.text.trim().is_empty())
        .collect();
    // Newest-first from SQL; the prompt (and the deterministic layer's
    // `messages.first()` customer_goal) expect chronological order.
    rows.reverse();
    Ok(rows)
}

#[derive(Debug, Clone)]
pub struct PreparedComputation {
    pub conversation_id: i64,
    pub records: Vec<AttributeRecord>,
    pub prompt: Option<String>,
    pub input_hash: String,
    pub cached: Option<(i64, Value)>,
}

pub fn prepare_computation(
    conn: &Connection,
    conversation_id: i64,
    model: &str,
    force: bool,
) -> Result<Option<PreparedComputation>> {
    let exists: Option<i64> = conn
        .query_row(
            &format!("SELECT id FROM conversations WHERE id = ?1 AND {NOT_DELETED}"),
            params![conversation_id],
            |r| r.get(0),
        )
        .ok()
        .flatten();
    if exists.is_none() {
        return Ok(None);
    }

    let records = deterministic_records(conn, conversation_id)?;

    let messages = customer_messages(conn, conversation_id)?;
    let (prompt, input_hash) = if messages.is_empty() || model.is_empty() {
        (None, String::new())
    } else {
        let prompt = build_extraction_user(conn, conversation_id, &messages);
        let hash = crate::embeddings::content_hash(&prompt);
        (Some(prompt), hash)
    };

    let cached = if force || prompt.is_none() {
        None
    } else {
        lookup_cached_extraction(conn, &input_hash, model)?
            .filter(|(_, v)| v.get("attributes").is_some_and(|a| a.as_array().is_some()))
    };

    Ok(Some(PreparedComputation {
        conversation_id,
        records,
        prompt,
        input_hash,
        cached,
    }))
}

/// What one extraction chat call produced — the run row's raw material
/// for the cost ledger (clean-audit item 37: content + latency + the
/// endpoint's token usage).
#[derive(Debug, Clone)]
pub struct ExtractionOutput {
    pub content: String,
    pub latency_ms: u64,
    pub usage: Option<crate::ai_provider::TokenUsage>,
}

pub async fn run_extraction(
    conn: &Connection,
    provider: &dyn LocalAiProvider,
    model: &str,
    prompt: &str,
) -> Option<ExtractionOutput> {
    if !provider.is_available().await {
        return None;
    }
    // Clean-audit item 35: the extraction prompt carries raw customer
    // messages — it crosses the same prompt-redaction boundary the
    // pipeline's chat_json uses (`redaction_enabled` setting, default
    // on) instead of bypassing it. Emails, card numbers, phone numbers
    // and secrets never reach the model from this path.
    let redaction_enabled =
        crate::settings::get_bool(conn, "redaction_enabled", true).unwrap_or(true);
    let (safe_prompt, findings) = crate::security::redact_text(prompt, redaction_enabled);
    if !findings.is_empty() {
        tracing::info!(
            operation = "attribute_extraction",
            patterns = ?findings.iter().filter_map(|f| f.get("pattern")).collect::<Vec<_>>(),
            "redaction boundary applied to the extraction prompt"
        );
    }
    let started = std::time::Instant::now();
    match provider
        .chat_structured(
            model,
            &[
                ChatMessage {
                    role: "system".into(),
                    content: ATTRIBUTE_EXTRACTION_SYSTEM.into(),
                },
                ChatMessage {
                    role: "user".into(),
                    content: safe_prompt,
                },
            ],
        )
        .await
    {
        Ok(response) => Some(ExtractionOutput {
            latency_ms: started.elapsed().as_millis() as u64,
            usage: response.usage,
            content: response.content,
        }),
        Err(_) => None,
    }
}

pub fn finish_computation(
    conn: &mut Connection,
    prepared: PreparedComputation,
    fresh_output: Option<ExtractionOutput>,
    model: &str,
    endpoint_id: Option<&str>,
) -> Result<Option<AttributeSnapshot>> {
    let PreparedComputation {
        conversation_id,
        mut records,
        prompt,
        input_hash,
        cached,
    } = prepared;

    let mut ai_run_id: Option<i64> = None;
    let mut extracted: Option<Value> = None;

    if let Some((run_id, cached_value)) = cached {
        extracted = Some(cached_value);
        ai_run_id = Some(run_id);
    } else if let (Some(_), Some(fresh)) = (&prompt, fresh_output)
        // Clean-audit item 34: local models happily wrap their JSON in
        // markdown fences or surround it with prose — a strict
        // from_str on the raw answer lost every such extraction. Parse
        // through the pipeline's canonical fence-stripping extractor
        // (the same one chat_json uses) before the shape validation.
        && let Some(parsed) = crate::ai_pipeline::extract_json(&fresh.content)
        && parsed
            .get("attributes")
            .is_some_and(|a| a.as_array().is_some())
    {
        // Clean-audit item 37: the run row now lands as a completed
        // 'attributes' run carrying the cost ledger's raw material
        // (latency, token usage, endpoint) instead of a bare row stuck
        // at the status column's 'queued' default, invisible to
        // `usage_ledger`'s `WHERE status='completed'`.
        let run_id = store_extraction_run(
            conn,
            ExtractionRunRow {
                conversation_id,
                input_hash: &input_hash,
                model,
                output: &fresh.content,
                latency_ms: fresh.latency_ms,
                usage: fresh.usage.as_ref(),
                endpoint_id,
            },
        )?;
        extracted = Some(parsed);
        ai_run_id = Some(run_id);
    }

    if let Some(Value::Object(map)) = extracted
        && let Some(Value::Array(attrs)) = map.get("attributes")
    {
        merge_extraction(&mut records, attrs, ai_run_id);
    }

    save_snapshot(conn, conversation_id, &records, ai_run_id)?;
    snapshot(conn, conversation_id)
}

fn merge_extraction(records: &mut Vec<AttributeRecord>, attrs: &[Value], ai_run_id: Option<i64>) {
    for a in attrs {
        let Some(obj) = a.as_object() else { continue };
        let Some(attr_name) = obj.get("attribute").and_then(Value::as_str) else {
            continue;
        };
        let Some(key) = AiAttributeKey::parse(attr_name) else {
            continue;
        };
        if !is_ai_slot(key) {
            continue;
        }
        let Some(raw_value) = obj.get("value").and_then(Value::as_str) else {
            continue;
        };
        let Some(value) = normalize_ai_value(key, raw_value) else {
            continue;
        };
        let mut confidence = obj
            .get("confidence")
            .and_then(Value::as_str)
            .and_then(parse_confidence)
            .unwrap_or("unknown");
        let excerpt = obj.get("evidence_excerpt").and_then(Value::as_str);
        if (confidence == "high" || confidence == "medium") && excerpt.is_none() {
            confidence = "low";
        }
        let evidence = match excerpt {
            Some(e) => vec![json!({
                "excerpt": e.chars().take(500).collect::<String>(),
                "thread_local_id": obj
                    .get("evidence_thread_local_id")
                    .and_then(Value::as_i64),
            })],
            None => Vec::new(),
        };
        records.push(AttributeRecord {
            key,
            value,
            confidence,
            source: "ai",
            evidence,
            run_id: ai_run_id,
        });
    }
}

pub async fn compute(
    conn: &mut Connection,
    provider: &dyn LocalAiProvider,
    model: &str,
    endpoint_id: Option<&str>,
    conversation_id: i64,
    force: bool,
) -> Result<Option<AttributeSnapshot>> {
    let Some(prepared) = prepare_computation(conn, conversation_id, model, force)? else {
        return Ok(None);
    };
    let fresh = match &prepared.prompt {
        Some(prompt) if prepared.cached.is_none() => {
            run_extraction(conn, provider, model, prompt).await
        }
        _ => None,
    };
    finish_computation(conn, prepared, fresh, model, endpoint_id)
}

fn is_ai_slot(key: AiAttributeKey) -> bool {
    matches!(
        key,
        AiAttributeKey::Intent
            | AiAttributeKey::Product
            | AiAttributeKey::Feature
            | AiAttributeKey::Issue
            | AiAttributeKey::CustomerGoal
            | AiAttributeKey::ResponseStyle
    )
}

fn parse_confidence(s: &str) -> Option<&'static str> {
    match s {
        "high" => Some("high"),
        "medium" => Some("medium"),
        "low" => Some("low"),
        "unknown" => Some("unknown"),
        _ => None,
    }
}

fn normalize_ai_value(key: AiAttributeKey, value: &str) -> Option<String> {
    let v = value.trim();
    if v.is_empty() {
        return None;
    }
    if key.value_type() == AttributeValueType::Enum && !key.values().contains(&v) {
        return None;
    }
    Some(v.chars().take(300).collect())
}

fn build_extraction_user(
    conn: &Connection,
    conversation_id: i64,
    messages: &[CustomerMessage],
) -> String {
    let subject: String = conn
        .query_row(
            "SELECT COALESCE(subject, '') FROM conversations WHERE id = ?1",
            params![conversation_id],
            |r| r.get(0),
        )
        .unwrap_or_default();
    let subject: String = subject.chars().take(300).collect();
    let mut parts = vec![
        format!("SUBJECT: {subject}"),
        "CUSTOMER MESSAGES (oldest first):".into(),
    ];
    for m in messages {
        parts.push(format!("  [thread {}] {}", m.thread_local_id, m.text));
    }
    parts.push(String::new());
    parts.push("Extract the attributes that are actually supported by these messages.".into());
    parts.join("\n")
}

/// What one completed extraction run must persist (clean-audit item 37's
/// ledger row: the hash-keyed cache columns plus the cost ledger's raw
/// material).
struct ExtractionRunRow<'a> {
    conversation_id: i64,
    input_hash: &'a str,
    model: &'a str,
    output: &'a str,
    latency_ms: u64,
    usage: Option<&'a crate::ai_provider::TokenUsage>,
    endpoint_id: Option<&'a str>,
}

fn store_extraction_run(conn: &Connection, row: ExtractionRunRow<'_>) -> Result<i64> {
    let usage_json = match row.usage {
        Some(u) => serde_json::to_string(u)?,
        None => "null".to_string(),
    };
    conn.execute(
        "INSERT INTO ai_runs (input_hash, prompt_version, model, response_json,
                              type, conversation_id, status, latency_ms,
                              started_at, completed_at, token_usage, endpoint_id)
         VALUES (?1, ?2, ?3, ?4, 'attributes', ?5, 'completed', ?6,
                 datetime('now'), datetime('now'), ?7, ?8)",
        params![
            row.input_hash,
            ATTRIBUTE_EXTRACTION_PROMPT_VERSION,
            row.model,
            row.output,
            row.conversation_id,
            row.latency_ms as i64,
            usage_json,
            row.endpoint_id
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

fn lookup_cached_extraction(
    conn: &Connection,
    input_hash: &str,
    model: &str,
) -> Result<Option<(i64, Value)>> {
    // Clean-audit item 37: only completed runs are cache hits — a
    // pre-fix row stuck at the 'queued' default is treated as a miss so
    // the next computation re-runs and stores a proper completed row
    // (self-healing, newest-first below finds it thereafter).
    let row: Option<(i64, String)> = conn
        .query_row(
            "SELECT id, response_json FROM ai_runs
             WHERE input_hash = ?1 AND prompt_version = ?2 AND model = ?3
               AND status = 'completed'
             ORDER BY created_at DESC, id DESC LIMIT 1",
            params![input_hash, ATTRIBUTE_EXTRACTION_PROMPT_VERSION, model],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .ok();
    Ok(row.and_then(|(id, json)| serde_json::from_str::<Value>(&json).ok().map(|v| (id, v))))
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

fn find_evidence(messages: &[CustomerMessage], hit: impl Fn(&str) -> bool) -> Option<Value> {
    for m in messages {
        if hit(&m.text.to_lowercase()) {
            return Some(json!({
                "excerpt": excerpt(&m.text, 220),
                "thread_local_id": m.thread_local_id,
            }));
        }
    }
    None
}

fn excerpt(text: &str, max_len: usize) -> String {
    let clean: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if clean.chars().count() <= max_len {
        clean
    } else {
        let head: String = clean.chars().take(max_len - 3).collect();
        format!("{head}\u{2026}")
    }
}

fn is_word_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

fn has_marker(haystack_lower: &str, marker: &str) -> bool {
    let bytes = haystack_lower.as_bytes();
    let mut start = 0usize;
    while let Some(pos) = haystack_lower[start..].find(marker) {
        let abs = start + pos;
        let end = abs + marker.len();
        let before_ok = abs == 0 || !is_word_byte(bytes[abs - 1]);
        let after_ok = end >= bytes.len() || !is_word_byte(bytes[end]);
        if before_ok && after_ok {
            return true;
        }
        start = abs + 1;
    }
    false
}

fn count_markers(haystack_lower: &str, markers: &[&str]) -> usize {
    markers
        .iter()
        .filter(|m| has_marker(haystack_lower, m))
        .count()
}

struct DeterministicSignals {
    urgency: &'static str,
    urgency_confidence: &'static str,
    urgency_evidence: Option<Value>,
    frustration: &'static str,
    frustration_confidence: &'static str,
    frustration_evidence: Option<Value>,
    technical: &'static str,
    technical_confidence: &'static str,
    technical_evidence: Option<Value>,
    escalation: bool,
    escalation_evidence: Option<Value>,
    question_count: usize,
    response_style: Option<&'static str>,
    response_style_evidence: Option<Value>,
    customer_goal: Option<String>,
}

fn derive_signals(messages: &[CustomerMessage]) -> Option<DeterministicSignals> {
    if messages.is_empty() {
        return None;
    }
    let all: String = messages
        .iter()
        .map(|m| m.text.to_lowercase())
        .collect::<Vec<_>>()
        .join("\n");

    let urgency_hits = count_markers(&all, URGENCY_MARKERS);
    let urgency = if urgency_hits >= 3
        || ["production is down", "outage", "emergency"]
            .iter()
            .any(|p| all.contains(p))
    {
        "high"
    } else if urgency_hits >= 1 {
        "moderate"
    } else {
        "none"
    };
    let urgency_confidence = if urgency_hits >= 1 { "medium" } else { "low" };
    let urgency_evidence = find_evidence(messages, |t| {
        URGENCY_MARKERS.iter().any(|m| has_marker(t, m))
    });

    let frustration_hits = count_markers(&all, FRUSTRATION_MARKERS);
    let frustration = if frustration_hits >= 3 {
        "strong"
    } else if frustration_hits >= 1 {
        "moderate"
    } else {
        "none"
    };
    let frustration_confidence = if frustration_hits >= 1 {
        "medium"
    } else {
        "low"
    };
    let frustration_evidence = find_evidence(messages, |t| {
        FRUSTRATION_MARKERS.iter().any(|m| has_marker(t, m))
    });

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
    let technical_confidence = if tech_hits >= 1 { "medium" } else { "low" };
    let technical_evidence = find_evidence(messages, |t| {
        TECHNICAL_VOCAB.iter().any(|v| has_marker(t, v))
    });

    let escalation = ESCALATION_MARKERS.iter().any(|m| has_marker(&all, m));
    let escalation_evidence = find_evidence(messages, |t| {
        ESCALATION_MARKERS.iter().any(|m| has_marker(t, m))
    });

    let question_count = messages.iter().map(|m| m.text.matches('?').count()).sum();

    let (response_style, response_style_evidence) = if all.contains("concise")
        || has_marker(&all, "short answer")
        || has_marker(&all, "no lengthy")
        || has_marker(&all, "brief answer")
        || has_marker(&all, "be brief")
        || all.contains("keep it short")
        || all.contains("keep this short")
        || all.contains("keep it brief")
        || all.contains("keep this brief")
    {
        (
            Some("concise"),
            find_evidence(messages, |t| {
                t.contains("concise")
                    || t.contains("short answer")
                    || t.contains("no lengthy")
                    || t.contains("brief answer")
                    || t.contains("be brief")
                    || t.contains("keep it short")
                    || t.contains("keep this short")
                    || t.contains("keep it brief")
                    || t.contains("keep this brief")
            }),
        )
    } else if all.contains("step by step")
        || all.contains("step-by-step")
        || all.contains("walk me through")
        || has_marker(&all, "instructions")
    {
        (
            Some("step_by_step"),
            find_evidence(messages, |t| {
                t.contains("step by step")
                    || t.contains("step-by-step")
                    || t.contains("walk me through")
                    || t.contains("instructions")
            }),
        )
    } else if has_marker(&all, "detailed")
        || has_marker(&all, "thorough")
        || all.contains("in depth")
        || all.contains("in-depth")
        || all.contains("full explanation")
        || has_marker(&all, "comprehensive")
    {
        (
            Some("detailed"),
            find_evidence(messages, |t| {
                t.contains("detailed")
                    || t.contains("thorough")
                    || t.contains("in depth")
                    || t.contains("in-depth")
                    || t.contains("full explanation")
                    || t.contains("comprehensive")
            }),
        )
    } else {
        (None, None)
    };

    let customer_goal = messages
        .first()
        .map(|m| excerpt(&m.text, 300))
        .filter(|g| !g.trim().is_empty());

    Some(DeterministicSignals {
        urgency,
        urgency_confidence,
        urgency_evidence,
        frustration,
        frustration_confidence,
        frustration_evidence,
        technical,
        technical_confidence,
        technical_evidence,
        escalation,
        escalation_evidence,
        question_count,
        response_style,
        response_style_evidence,
        customer_goal,
    })
}

pub fn deterministic_records(
    conn: &Connection,
    conversation_id: i64,
) -> Result<Vec<AttributeRecord>> {
    let mut out: Vec<AttributeRecord> = Vec::new();
    let messages = customer_messages(conn, conversation_id)?;
    let signals = derive_signals(&messages);

    if let Some(s) = &signals {
        let mk = |key: AiAttributeKey,
                  value: String,
                  confidence: &'static str,
                  evidence: Vec<Value>| AttributeRecord {
            key,
            value,
            confidence,
            source: "deterministic",
            evidence,
            run_id: None,
        };
        let evidence_vec =
            |e: &Option<Value>| e.as_ref().map(|v| vec![v.clone()]).unwrap_or_default();

        out.push(mk(
            AiAttributeKey::Urgency,
            s.urgency.to_string(),
            s.urgency_confidence,
            evidence_vec(&s.urgency_evidence),
        ));
        out.push(mk(
            AiAttributeKey::FrustrationCues,
            s.frustration.to_string(),
            s.frustration_confidence,
            evidence_vec(&s.frustration_evidence),
        ));
        out.push(mk(
            AiAttributeKey::TechnicalFamiliarity,
            s.technical.to_string(),
            s.technical_confidence,
            evidence_vec(&s.technical_evidence),
        ));
        out.push(mk(
            AiAttributeKey::EscalationSignal,
            if s.escalation { "true" } else { "false" }.to_string(),
            if s.escalation { "medium" } else { "low" },
            if s.escalation {
                evidence_vec(&s.escalation_evidence)
            } else {
                Vec::new()
            },
        ));
        out.push(mk(
            AiAttributeKey::QuestionCount,
            s.question_count.to_string(),
            "high",
            Vec::new(),
        ));
        if let Some(style) = s.response_style {
            out.push(mk(
                AiAttributeKey::ResponseStyle,
                style.to_string(),
                "high",
                evidence_vec(&s.response_style_evidence),
            ));
        }

        let risk = if s.escalation
            || (s.urgency == "high" && (s.frustration == "moderate" || s.frustration == "strong"))
        {
            ("high", "medium")
        } else if s.urgency != "none"
            || s.frustration != "none"
            || count_markers(
                &messages
                    .iter()
                    .map(|m| m.text.to_lowercase())
                    .collect::<Vec<_>>()
                    .join("\n"),
                ACTION_EXPECTATION_MARKERS,
            ) >= 1
        {
            ("medium", "low")
        } else {
            ("low", "low")
        };
        let risk_evidence: Vec<Value> = [
            &s.urgency_evidence,
            &s.frustration_evidence,
            &s.escalation_evidence,
        ]
        .into_iter()
        .flatten()
        .take(3)
        .cloned()
        .collect();
        out.push(mk(
            AiAttributeKey::Risk,
            risk.0.to_string(),
            risk.1,
            risk_evidence,
        ));

        if let Some(goal) = &s.customer_goal {
            out.push(mk(
                AiAttributeKey::CustomerGoal,
                goal.clone(),
                "low",
                Vec::new(),
            ));
        }
    }

    let linked: Option<i64> = conn
        .query_row(
            "SELECT 1 FROM known_issue_links WHERE conversation_id = ?1 LIMIT 1",
            params![conversation_id],
            |r| r.get(0),
        )
        .ok()
        .flatten();
    out.push(AttributeRecord {
        key: AiAttributeKey::KnownIssue,
        value: if linked.is_some() { "true" } else { "false" }.to_string(),
        confidence: "high",
        source: "deterministic",
        evidence: Vec::new(),
        run_id: None,
    });

    let cluster: Option<String> = conn
        .query_row(
            "SELECT ic.name FROM issue_cluster_conversations icm
             JOIN issue_clusters ic ON ic.id = icm.cluster_id
             WHERE icm.conversation_id = ?1
             ORDER BY ic.id DESC LIMIT 1",
            params![conversation_id],
            |r| r.get(0),
        )
        .ok()
        .flatten();
    if let Some(name) = cluster {
        out.push(AttributeRecord {
            key: AiAttributeKey::IssueCluster,
            value: name.chars().take(300).collect(),
            confidence: "medium",
            source: "deterministic",
            evidence: Vec::new(),
            run_id: None,
        });
    }

    Ok(out)
}

/// A2 entry point: attribute extraction routed through the provider hub.
/// With a live backend the AI layer fills intent/product/feature/issue/
/// customer_goal/response_style; with AI disabled the deterministic
/// layer still lands so the Attributes tab and automation conditions
/// stay alive without any model.
pub async fn compute_with_backend(
    conn: &mut Connection,
    backend: &crate::ai_pipeline::AiBackend,
    conversation_id: i64,
    force: bool,
) -> Result<Option<AttributeSnapshot>> {
    match backend {
        crate::ai_pipeline::AiBackend::LmStudio {
            client,
            model,
            endpoint_id,
        } => {
            let provider = crate::ai_lm_studio::LmStudioProvider::with_client(client.clone());
            compute(
                conn,
                &provider,
                model.as_deref().unwrap_or(""),
                endpoint_id.as_deref(),
                conversation_id,
                force,
            )
            .await
        }
        crate::ai_pipeline::AiBackend::Disabled => {
            // No model configured: deterministic-only, still a real snapshot.
            if let Some(prepared) = prepare_computation(conn, conversation_id, "", force)? {
                finish_computation(conn, prepared, None, "", None)
            } else {
                Ok(None)
            }
        }
    }
}
