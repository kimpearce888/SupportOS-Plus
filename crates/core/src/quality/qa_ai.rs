use super::*;
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

pub(crate) async fn compute_qa_ai_layer(
    conn: &Connection,
    backend: &crate::ai_pipeline::AiBackend,
    conversation_id: i64,
) -> QaAiOutcome {
    let deterministic = compute_qa_deterministic(conn, conversation_id);
    if deterministic.is_none() {
        return QaAiOutcome {
            error: Some("conversation not found".into()),
        };
    }
    if matches!(backend, crate::ai_pipeline::AiBackend::Disabled) {
        let _ = get_qa(conn, conversation_id, false);
        return QaAiOutcome {
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
            let _ = get_qa(conn, conversation_id, true);
            QaAiOutcome {
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
            let _ = get_qa(conn, conversation_id, true);
            QaAiOutcome {
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
            let _ = get_qa(conn, conversation_id, true);
            QaAiOutcome { error: None }
        }
    }
}

fn round1(x: f64) -> f64 {
    (x * 10.0).round() / 10.0
}

pub(crate) struct DerivedOutcome {
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
            .expect("static regex")
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

    let _ = (conv, &response_style, follow_up_count, clarification_count);
    DerivedOutcome {
        resolved_after_first,
        effort_score,
        friction,
    }
}
