use super::*;
pub(crate) fn build_interaction_strategy_block(
    conn: &Connection,
    conversation_local_id: i64,
) -> Option<String> {
    use crate::interaction_engine as engine;

    let stored = engine::get_latest_current_interaction(conn, conversation_local_id)
        .ok()
        .flatten()?;
    let customer = engine::conversation_customer(conn, conversation_local_id);

    let approach = match stored.recommendation.as_ref() {
        Some(r) => r.recommendation.clone(),
        None => {
            let current = crate::interaction_signals::CurrentInteraction {
                conversation_local_id,
                customer_local_id: customer,
                is_returning_client: false,
                signals: stored.signals.clone(),
                customer_goal: stored.customer_goal.clone(),
                message_stats: stored.message_stats.unwrap_or_default(),
                sources: stored.sources.clone(),
                generated_at: Some(stored.generated_at.clone()),
            };
            let baseline = customer
                .and_then(|cid| engine::comparison_baseline(conn, cid, conversation_local_id).ok())
                .flatten();
            let changes = engine::compute_changes(&current, baseline.as_ref());
            let overrides = customer
                .map(|cid| engine::get_active_overrides(conn, cid).unwrap_or_default())
                .unwrap_or_default();
            engine::heuristic_recommendation(&current, &changes, baseline.as_ref(), &overrides)?
        }
    };

    let preferences: Vec<String> = customer
        .and_then(|cid| engine::get_preferences(conn, cid).ok())
        .map(|prefs| prefs.into_iter().map(|p| p.preference).collect())
        .unwrap_or_default();

    let already_provided: Vec<String> = conn
        .prepare(
            "SELECT COALESCE(body_text, '') FROM conversation_threads
              WHERE conversation_id = ?1 AND type = 'customer' AND deleted_at IS NULL
              ORDER BY created_at DESC, id DESC LIMIT 10",
        )
        .ok()
        .and_then(|mut stmt| {
            stmt.query_map(params![conversation_local_id], |r| r.get::<_, String>(0))
                .map(|rows| rows.flatten().collect::<Vec<_>>())
                .ok()
        })
        .map(|texts| {
            let mut facts: Vec<String> = Vec::new();
            for t in &texts {
                for f in extract_provided_facts(t) {
                    if !facts.contains(&f) {
                        facts.push(f);
                    }
                }
            }
            facts
        })
        .unwrap_or_default();

    Some(crate::ai_prompts::interaction_strategy_block(
        approach.tone.as_deref(),
        approach.length.as_deref(),
        &approach.response_strategy,
        &approach.avoid,
        &preferences,
        &already_provided,
    ))
}

pub(crate) fn extract_provided_facts(text: &str) -> Vec<String> {
    let lower = text.to_lowercase();
    let patterns: [(&str, &str); 8] = [
        ("screenshot", "screenshots/attachments"),
        ("browser", "browser details"),
        ("version", "version numbers"),
        ("error", "error messages"),
        ("api key", "account identifiers"),
        ("already", "troubleshooting already performed"),
        ("timezone", "timezone information"),
        ("invoice", "billing details"),
    ];
    let mut facts = Vec::new();
    if lower.contains("screenshot") {
        facts.push(patterns[0].1.to_string());
    }
    for b in ["chrome", "firefox", "safari", "edge"] {
        if lower.contains(&format!("browser {b}"))
            || lower.contains(&format!("browser is {b}"))
            || lower.contains(&format!("using {b}"))
        {
            facts.push(patterns[1].1.to_string());
            break;
        }
    }
    if has_version_number(&lower) {
        facts.push(patterns[2].1.to_string());
    }
    if lower.contains("error message") || lower.contains("error code") || lower.contains("error ") {
        facts.push(patterns[3].1.to_string());
    }
    for t in [
        "api key",
        "token",
        "workspace id",
        "account id",
        "email address",
    ] {
        if lower.contains(t) {
            facts.push(patterns[4].1.to_string());
            break;
        }
    }
    if lower.contains("already tried")
        || lower.contains("already did")
        || lower.contains("already reinstalled")
        || lower.contains("already re-installed")
        || lower.contains("already restarted")
        || lower.contains("already cleared")
        || lower.contains("we already")
        || lower.contains("we have already")
    {
        facts.push(patterns[5].1.to_string());
    }
    if lower.contains("timezone") || lower.contains("utc+") || lower.contains("utc-") {
        facts.push(patterns[6].1.to_string());
    }
    if lower.contains("invoice")
        || lower.contains("receipt")
        || lower.contains("payment")
        || lower.contains("card declined")
        || lower.contains("card was declined")
        || lower.contains("billing")
    {
        facts.push(patterns[7].1.to_string());
    }
    facts
}

fn has_version_number(lower: &str) -> bool {
    if let Some(idx) = lower.find("version ") {
        let rest = &lower[idx + 8..];
        if rest.starts_with(|c: char| c.is_ascii_digit()) {
            return true;
        }
    }
    let bytes: Vec<char> = lower.chars().collect();
    for i in 0..bytes.len().saturating_sub(3) {
        if bytes[i] == 'v'
            && bytes[i + 1].is_ascii_digit()
            && bytes[i + 2] == '.'
            && bytes[i + 3].is_ascii_digit()
        {
            return true;
        }
    }
    false
}

#[derive(Debug, Clone)]
pub(crate) struct AnalyzeInteractionOutcome {
    pub ai_enriched: bool,
}

#[derive(Debug, Default)]
struct ObservationOutput {
    signals: Vec<crate::interaction_signals::InteractionSignal>,
    customer_goal: Option<String>,
    notes: Vec<String>,
}

fn parse_observation(
    json: Option<&serde_json::Value>,
    valid_thread_ids: &std::collections::HashSet<i64>,
) -> ObservationOutput {
    use crate::interaction_signals::InteractionSignal;
    let Some(obj) = json.and_then(|v| v.as_object()) else {
        return ObservationOutput::default();
    };
    let mut signals = Vec::new();
    if let Some(arr) = obj.get("signals").and_then(|v| v.as_array()) {
        for s in arr {
            let dimension = s.get("dimension").and_then(|v| v.as_str()).unwrap_or("");
            let value = s.get("value").and_then(|v| v.as_str()).unwrap_or("");
            if !crate::interaction_signals::is_valid_value(dimension, value) {
                continue;
            }
            let confidence = s
                .get("confidence")
                .and_then(|v| v.as_str())
                .filter(|c| ["high", "medium", "low", "unknown"].contains(c))
                .unwrap_or("unknown")
                .to_string();
            let excerpt = s
                .get("evidence_excerpt")
                .and_then(|v| v.as_str())
                .filter(|e| !e.is_empty());
            let excerpt_safe = excerpt.is_some_and(|e| {
                crate::interaction_engine::assert_interaction_text_safe(Some(e)).ok
            });
            let thread_id = s
                .get("evidence_thread_local_id")
                .and_then(|v| v.as_i64())
                .filter(|id| valid_thread_ids.contains(id));
            signals.push(InteractionSignal {
                dimension: dimension.to_string(),
                value: value.to_string(),
                confidence,
                evidence: excerpt_safe.then(|| {
                    let e = excerpt.unwrap_or_default();
                    crate::interaction_signals::Evidence {
                        excerpt: e.to_string(),
                        thread_local_id: thread_id,
                        conversation_local_id: None,
                    }
                }),
                source: "ai".to_string(),
            });
        }
    }
    let sanitized = crate::interaction_signals::sanitize_signals(signals).signals;
    let goal_check = obj.get("customer_goal").and_then(|v| v.as_str());
    let customer_goal = goal_check
        .filter(|g| {
            !g.is_empty() && crate::interaction_engine::assert_interaction_text_safe(Some(g)).ok
        })
        .map(String::from);
    let notes = obj
        .get("notes")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|n| n.as_str())
                .filter(|n| crate::interaction_engine::assert_interaction_text_safe(Some(n)).ok)
                .take(6)
                .map(String::from)
                .collect()
        })
        .unwrap_or_default();
    ObservationOutput {
        signals: sanitized,
        customer_goal,
        notes,
    }
}

fn parse_recommendation(
    json: Option<&serde_json::Value>,
) -> Option<crate::interaction_engine::SupportApproach> {
    let obj = json?.as_object()?;
    let clean = |v: Option<&serde_json::Value>| -> Option<String> {
        v.and_then(|x| x.as_str())
            .filter(|s| !s.is_empty())
            .filter(|s| crate::interaction_engine::sanitize_interaction_text(s).ok)
            .map(String::from)
    };
    let clean_list = |key: &str, cap: usize| -> Vec<String> {
        obj.get(key)
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|x| x.as_str())
                    .filter(|s| crate::interaction_engine::assert_interaction_text_safe(Some(s)).ok)
                    .take(cap)
                    .map(String::from)
                    .collect()
            })
            .unwrap_or_default()
    };
    let length = obj
        .get("length")
        .and_then(|v| v.as_str())
        .filter(|l| ["concise", "moderate", "detailed"].contains(l))
        .map(String::from);
    Some(crate::interaction_engine::SupportApproach {
        tone: clean(obj.get("tone")),
        length,
        start_with: clean(obj.get("start_with")),
        then: clean(obj.get("then")),
        avoid: clean_list("avoid", 8),
        response_strategy: clean_list("response_strategy", 8),
        de_escalation: obj
            .get("de_escalation")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        escalation_recommendation: clean(obj.get("escalation_recommendation")),
        why: clean_list("why", 6),
        source: "ai".to_string(),
        confidence: "medium".to_string(),
    })
}

pub async fn analyze_interaction(
    conn: &Connection,
    backend: &AiBackend,
    conversation_local_id: i64,
) -> std::result::Result<AnalyzeInteractionOutcome, LmStudioError> {
    use crate::interaction_engine as engine;

    let exists: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM conversations WHERE id = ?1 AND deleted_at IS NULL",
            params![conversation_local_id],
            |r| r.get(0),
        )
        .unwrap_or(0);
    if exists == 0 {
        return Err(LmStudioError::new("Conversation not found", false));
    }
    let _ = crate::interaction_signals::record_current_interaction(conn, conversation_local_id);
    if backend.kind() == "disabled" {
        return Ok(AnalyzeInteractionOutcome { ai_enriched: false });
    }
    let customer_id: Option<i64> = conn
        .query_row(
            "SELECT customer_local_id FROM conversations WHERE id = ?1",
            params![conversation_local_id],
            |r| r.get(0),
        )
        .ok();
    let Some(customer_id) = customer_id else {
        return Ok(AnalyzeInteractionOutcome { ai_enriched: false });
    };
    let customer_name: String = conn
        .query_row(
            "SELECT TRIM(COALESCE(first_name, '') || ' ' || COALESCE(last_name, '')) AS name
               FROM customers WHERE id = ?1",
            params![customer_id],
            |r| r.get::<_, Option<String>>(0),
        )
        .ok()
        .flatten()
        .unwrap_or_else(|| "Customer".into());
    let customer_name = if customer_name.trim().is_empty() {
        "Customer".to_string()
    } else {
        customer_name
    };
    let messages: Vec<(String, Option<i64>)> = {
        let mut stmt = conn
            .prepare(
                "SELECT id, body_html, body_text FROM conversation_threads
                  WHERE conversation_id = ?1 AND deleted_at IS NULL
                    AND type = 'customer' AND state = 'published'
                  ORDER BY remote_created_at ASC",
            )
            .map_err(|e| LmStudioError::new(e.to_string(), true))?;
        let rows = stmt
            .query_map(params![conversation_local_id], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, Option<String>>(1)?,
                    r.get::<_, Option<String>>(2)?,
                ))
            })
            .map_err(|e| LmStudioError::new(e.to_string(), true))?;
        rows.filter_map(|r| r.ok())
            .filter_map(|(id, html, body)| {
                let raw = html
                    .as_deref()
                    .filter(|h| !h.is_empty())
                    .or(body.as_deref())
                    .unwrap_or("");
                let text = crate::textutil::html_to_text(raw);
                (!text.trim().is_empty()).then_some((text, Some(id)))
            })
            .collect()
    };
    if messages.is_empty() {
        return Ok(AnalyzeInteractionOutcome { ai_enriched: false });
    }
    let history =
        engine::get_customer_conversations(conn, customer_id, Some(conversation_local_id))
            .unwrap_or_default();
    let baseline = engine::get_baseline(conn, customer_id).ok().flatten();
    let baseline_summary = baseline.as_ref().map(|b| {
        b.dimensions
            .iter()
            .map(|d| {
                format!(
                    "{}: usually {} ({} observations)",
                    d.dimension,
                    d.typical_value.replace('_', " "),
                    d.observation_count
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    });
    let recent_history: Vec<(i64, Option<&str>, String)> = history
        .iter()
        .take(5)
        .map(|h| {
            let first_msg: Option<String> = conn
                .query_row(
                    "SELECT body_html, body_text FROM conversation_threads
                      WHERE conversation_id = ?1 AND deleted_at IS NULL AND type = 'customer'
                      ORDER BY remote_created_at ASC LIMIT 1",
                    params![h.id],
                    |r| {
                        let html: Option<String> = r.get(0)?;
                        let body: Option<String> = r.get(1)?;
                        Ok(html
                            .as_deref()
                            .filter(|h| !h.is_empty())
                            .or(body.as_deref())
                            .map(String::from))
                    },
                )
                .ok()
                .flatten();
            let excerpt = first_msg
                .as_deref()
                .map(crate::textutil::html_to_text)
                .unwrap_or_default();
            (
                h.number,
                h.subject.as_deref(),
                excerpt.chars().take(240).collect(),
            )
        })
        .collect();
    let recent_history: Vec<(i64, Option<&str>, &str)> = recent_history
        .iter()
        .map(|(n, s, e)| (*n, *s, e.as_str()))
        .collect();
    let client_kind = if history.is_empty() {
        "first_time"
    } else {
        "returning"
    };

    let valid_thread_ids: std::collections::HashSet<i64> =
        messages.iter().filter_map(|(_, id)| *id).collect();
    let run_id1 = start_run(
        conn,
        "interaction_observation",
        Some(conversation_local_id),
        None,
        PROMPT_VERSIONS_INTERACTION_OBSERVATION,
        None,
        &serde_json::json!([conversation_local_id]),
    )
    .map_err(|e| LmStudioError::new(e.to_string(), true))?;
    let res = backend
        .chat_json(
            conn,
            INTERACTION_OBSERVATION_SYSTEM,
            &build_interaction_observation_user(
                &customer_name,
                client_kind,
                &messages,
                baseline_summary.as_deref(),
                &recent_history,
            ),
            true,
            Some(1400),
        )
        .await;
    let (obs, stage1_latency, stage1_usage) = match res {
        Ok(r) => (
            parse_observation(r.json.as_ref(), &valid_thread_ids),
            r.latency_ms,
            r.usage,
        ),
        Err(e) => {
            let _ = fail_run(conn, run_id1, &e.message);
            return Ok(AnalyzeInteractionOutcome { ai_enriched: false });
        }
    };
    complete_run_with_usage(
        conn,
        run_id1,
        &serde_json::json!({
            "signals": obs.signals,
            "customer_goal": obs.customer_goal,
            "notes": obs.notes,
        }),
        stage1_latency,
        stage1_usage.as_ref(),
        backend.endpoint_id(),
    )
    .map_err(|e| LmStudioError::new(e.to_string(), true))?;

    if !obs.signals.is_empty() {
        let heuristic =
            crate::interaction_signals::compute_current_interaction(conn, conversation_local_id)
                .ok()
                .flatten();
        let merged = match &heuristic {
            Some(h) => engine::merge_signals(&h.signals, &obs.signals),
            None => obs.signals.clone(),
        };
        let merged = crate::interaction_signals::sanitize_signals(merged).signals;
        let _ = engine::save_current_interaction(
            conn,
            conversation_local_id,
            Some(customer_id),
            &merged,
            heuristic
                .as_ref()
                .map(|h| &h.message_stats)
                .unwrap_or(&crate::interaction_signals::MessageStats::default()),
            obs.customer_goal
                .as_deref()
                .or(heuristic.as_ref().and_then(|h| h.customer_goal.as_deref())),
            "heuristic+ai",
            Some(PROMPT_VERSIONS_INTERACTION_OBSERVATION),
        );
        let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let ai_observations: Vec<engine::ObservationRow> = obs
            .signals
            .iter()
            .map(|s| engine::ObservationRow {
                customer_id,
                conversation_id: Some(conversation_local_id),
                thread_local_id: s.evidence.as_ref().and_then(|e| e.thread_local_id),
                dimension: s.dimension.clone(),
                value: s.value.clone(),
                confidence: s.confidence.clone(),
                evidence_excerpt: s.evidence.as_ref().map(|e| e.excerpt.clone()),
                source: "ai".into(),
                observed_at: now.clone(),
            })
            .collect();
        let _ = engine::insert_observations(conn, &ai_observations);
        let _ = engine::rebuild_baseline(conn, customer_id);
    }

    let stored = engine::get_latest_current_interaction(conn, conversation_local_id)
        .ok()
        .flatten();
    let card_current =
        crate::interaction_signals::compute_current_interaction(conn, conversation_local_id)
            .ok()
            .flatten();
    let mut stage_current =
        card_current
            .clone()
            .unwrap_or_else(|| crate::interaction_signals::CurrentInteraction {
                conversation_local_id,
                customer_local_id: Some(customer_id),
                is_returning_client: !history.is_empty(),
                signals: Vec::new(),
                customer_goal: None,
                message_stats: crate::interaction_signals::MessageStats::default(),
                sources: "heuristic+ai".into(),
                generated_at: None,
            });
    if let Some(stored) = &stored
        && !stored.signals.is_empty()
    {
        stage_current.signals = stored.signals.clone();
        stage_current.customer_goal = stored.customer_goal.clone();
    }
    let fresh_baseline = engine::comparison_baseline(conn, customer_id, conversation_local_id)
        .ok()
        .flatten();
    let changes = engine::compute_changes(&stage_current, fresh_baseline.as_ref());
    let preferences: Vec<(String, String)> = engine::get_preferences(conn, customer_id)
        .unwrap_or_default()
        .into_iter()
        .map(|p| (p.preference, p.origin))
        .collect();
    let outcome = engine::compute_outcome(conn, conversation_local_id)
        .ok()
        .flatten();
    let repeat_issue = engine::detect_repeat_issue(conn, conversation_local_id)
        .ok()
        .flatten();
    let fresh_summary = fresh_baseline.as_ref().map(|b| {
        b.dimensions
            .iter()
            .map(|d| {
                format!(
                    "{}: usually {} ({} observations)",
                    d.dimension,
                    d.typical_value.replace('_', " "),
                    d.observation_count
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    });
    let change_inputs: Vec<InteractionChangeInput> = changes
        .iter()
        .map(|c| InteractionChangeInput {
            dimension: c.dimension.clone(),
            baseline_value: Some(c.baseline_value.clone()),
            current_value: Some(c.current_value.clone()),
            significant: c.significant,
        })
        .collect();
    let sig_inputs: Vec<(String, String, String)> = stage_current
        .signals
        .iter()
        .map(|s| (s.dimension.clone(), s.value.clone(), s.confidence.clone()))
        .collect();
    let run_id2 = start_run(
        conn,
        "interaction_recommendation",
        Some(conversation_local_id),
        None,
        PROMPT_VERSIONS_INTERACTION_RECOMMENDATION,
        None,
        &serde_json::json!([conversation_local_id]),
    )
    .map_err(|e| LmStudioError::new(e.to_string(), true))?;
    let res2 = backend
        .chat_json(
            conn,
            INTERACTION_RECOMMENDATION_SYSTEM,
            &build_interaction_recommendation_user(
                client_kind,
                &sig_inputs,
                &change_inputs,
                fresh_summary.as_deref().or(baseline_summary.as_deref()),
                &preferences,
                repeat_issue.as_ref().is_some_and(|r| r.detected),
                outcome.as_ref().and_then(|o| o.effort_score),
            ),
            true,
            Some(900),
        )
        .await;
    let (rec, stage2_latency, stage2_usage, stage2_model) = match res2 {
        Ok(r) => (
            parse_recommendation(r.json.as_ref()),
            r.latency_ms,
            r.usage,
            r.model,
        ),
        Err(e) => {
            let _ = fail_run(conn, run_id2, &e.message);
            return Ok(AnalyzeInteractionOutcome { ai_enriched: false });
        }
    };
    match rec {
        Some(rec) => {
            complete_run_with_usage(
                conn,
                run_id2,
                &serde_json::to_value(&rec).unwrap_or_default(),
                stage2_latency,
                stage2_usage.as_ref(),
                backend.endpoint_id(),
            )
            .map_err(|e| LmStudioError::new(e.to_string(), true))?;
            let model_arg = (!stage2_model.is_empty()).then_some(stage2_model.as_str());
            let _ = engine::save_recommendation(
                conn,
                conversation_local_id,
                &rec,
                Some(PROMPT_VERSIONS_INTERACTION_RECOMMENDATION),
                model_arg,
            );
            Ok(AnalyzeInteractionOutcome { ai_enriched: true })
        }
        None => {
            let _ = fail_run(
                conn,
                run_id2,
                "Interaction recommendation returned an unparseable structure",
            );
            Ok(AnalyzeInteractionOutcome { ai_enriched: false })
        }
    }
}
