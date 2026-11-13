use super::*;
fn closed_conversation_ids(conn: &Connection, customer_id: i64) -> std::collections::HashSet<i64> {
    get_customer_conversations(conn, customer_id, None)
        .map(|rows| {
            rows.into_iter()
                .filter(|c| c.status == "closed")
                .map(|c| c.id)
                .collect()
        })
        .unwrap_or_default()
}

pub(crate) fn rebuild_baseline(
    conn: &Connection,
    customer_id: i64,
) -> Result<Option<BehaviorBaseline>> {
    let closed = closed_conversation_ids(conn, customer_id);
    let observations: Vec<ObservationRow> = get_observations_for_customer(conn, customer_id)?
        .into_iter()
        .filter(|o| o.conversation_id.is_some_and(|id| closed.contains(&id)))
        .collect();
    if observations.is_empty() {
        return Ok(None);
    }
    let conv_rows = get_customer_conversations(conn, customer_id, None).unwrap_or_default();
    let conversation_count = conv_rows.len() as i64;
    let mut last_observed_by_dim: std::collections::BTreeMap<String, String> =
        std::collections::BTreeMap::new();
    for o in &observations {
        let entry = last_observed_by_dim.entry(o.dimension.clone()).or_default();
        if o.observed_at > *entry {
            *entry = o.observed_at.clone();
        }
    }
    let profile_version = next_profile_version(conn, customer_id);
    let mut dimensions = Vec::new();
    let mut dims: Vec<&str> = observations
        .iter()
        .map(|o| o.dimension.as_str())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    dims.sort();
    for dim in dims {
        let dim_obs: Vec<&ObservationRow> =
            observations.iter().filter(|o| o.dimension == dim).collect();
        let Some(best) = dominant_value(&dim_obs) else {
            continue;
        };
        let count = dim_obs.len() as i64;
        let mut confidence = "low".to_string();
        for o in &dim_obs {
            if rank_confidence(&o.confidence) > rank_confidence(&confidence) {
                confidence = o.confidence.clone();
            }
        }
        let effective = if count >= crate::intelligence::MIN_OBSERVATIONS_FOR_PREFERENCE as i64 {
            confidence.clone()
        } else {
            "low".to_string()
        };
        upsert_baseline(
            conn,
            customer_id,
            dim,
            &best,
            &effective,
            count,
            last_observed_by_dim.get(dim).map(String::as_str),
            profile_version,
        )?;
        dimensions.push(BaselineDimension {
            dimension: dim.to_string(),
            typical_value: best,
            confidence: effective,
            observation_count: count,
            last_observed: last_observed_by_dim.get(dim).cloned(),
        });
    }
    Ok(Some(BehaviorBaseline {
        customer_local_id: customer_id,
        conversation_count,
        observation_count: observations.len() as i64,
        dimensions,
        last_updated: Some(chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string()),
        profile_version,
    }))
}

fn rank_confidence(c: &str) -> u8 {
    match c {
        "high" => 3,
        "medium" => 2,
        "low" => 1,
        _ => 0,
    }
}

pub(crate) fn comparison_baseline(
    conn: &Connection,
    customer_id: i64,
    exclude_conversation_id: i64,
) -> Result<Option<BehaviorBaseline>> {
    let closed = closed_conversation_ids(conn, customer_id);
    let observations: Vec<ObservationRow> = get_observations_for_customer(conn, customer_id)?
        .into_iter()
        .filter(|o| {
            o.conversation_id.is_some_and(|id| closed.contains(&id))
                && o.conversation_id != Some(exclude_conversation_id)
        })
        .collect();
    if observations.is_empty() {
        return Ok(None);
    }
    let conv_rows = get_customer_conversations(conn, customer_id, None).unwrap_or_default();
    let mut dims: Vec<&str> = observations
        .iter()
        .map(|o| o.dimension.as_str())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    dims.sort();
    let mut dimensions = Vec::new();
    for dim in dims {
        let dim_obs: Vec<&ObservationRow> =
            observations.iter().filter(|o| o.dimension == dim).collect();
        let Some(best) = dominant_value(&dim_obs) else {
            continue;
        };
        let count = dim_obs.len() as i64;
        let last_observed = dim_obs
            .iter()
            .map(|o| o.observed_at.as_str())
            .max()
            .map(String::from);
        dimensions.push(BaselineDimension {
            dimension: dim.to_string(),
            typical_value: best,
            confidence: if count >= crate::intelligence::MIN_OBSERVATIONS_FOR_PREFERENCE as i64 {
                "medium".into()
            } else {
                "low".into()
            },
            observation_count: count,
            last_observed,
        });
    }
    Ok(Some(BehaviorBaseline {
        customer_local_id: customer_id,
        conversation_count: (conv_rows.len() as i64).saturating_sub(1).max(0),
        observation_count: observations.len() as i64,
        dimensions,
        last_updated: Some(chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string()),
        profile_version: 1,
    }))
}

const ORDINAL_DIMENSIONS: [&str; 5] = [
    "urgency",
    "frustration",
    "detail",
    "directness",
    "technical_language",
];

pub(crate) fn compute_changes(
    current: &CurrentInteraction,
    baseline: Option<&BehaviorBaseline>,
) -> Vec<InteractionChange> {
    let Some(baseline) = baseline else {
        return Vec::new();
    };
    let mut changes = Vec::new();
    let current_by_dim: std::collections::HashMap<&str, &str> = current
        .signals
        .iter()
        .map(|s| (s.dimension.as_str(), s.value.as_str()))
        .collect();
    for b in &baseline.dimensions {
        let Some(cur) = current_by_dim.get(b.dimension.as_str()) else {
            continue;
        };
        if !ORDINAL_DIMENSIONS.contains(&b.dimension.as_str()) {
            if *cur == b.typical_value {
                continue;
            }
            changes.push(InteractionChange {
                dimension: b.dimension.clone(),
                baseline_value: b.typical_value.clone(),
                current_value: (*cur).to_string(),
                direction: "changed".into(),
                magnitude: 0.0,
                significant: false,
            });
            continue;
        }
        let (Some(b_rank), Some(c_rank)) = (
            dimension_rank(&b.dimension, &b.typical_value),
            dimension_rank(&b.dimension, cur),
        ) else {
            continue;
        };
        if b_rank == c_rank {
            continue;
        }
        let span = rank_span(&b.dimension);
        let raw_delta = (c_rank as f64 - b_rank as f64) / span as f64;
        let magnitude = (raw_delta.abs()).min(1.0);
        changes.push(InteractionChange {
            dimension: b.dimension.clone(),
            baseline_value: b.typical_value.clone(),
            current_value: (*cur).to_string(),
            direction: if raw_delta > 0.08 {
                "increase".into()
            } else if raw_delta < -0.08 {
                "decrease".into()
            } else {
                "same".into()
            },
            magnitude: (magnitude * 100.0).round() / 100.0,
            significant: magnitude >= crate::intelligence::CHANGE_SIGNIFICANCE_THRESHOLD,
        });
    }
    changes.sort_by(|a, b| b.magnitude.total_cmp(&a.magnitude));
    changes
}

pub(crate) fn compute_outcome(
    conn: &Connection,
    conversation_id: i64,
) -> Result<Option<OutcomeComputed>> {
    let Some(conv) = conversation_info(conn, conversation_id) else {
        return Ok(None);
    };
    let Some(customer_id) = conv.customer_local_id else {
        return Ok(None);
    };
    let mut stmt = conn.prepare(
        "SELECT type, body_html, body_text, from_type, created_at
           FROM conversation_threads
          WHERE conversation_id = ?1 AND deleted_at IS NULL AND state = 'published'
          ORDER BY COALESCE(remote_created_at, created_at) ASC",
    )?;
    type ThreadRow = (String, Option<String>, Option<String>, Option<String>);
    let rows: Vec<ThreadRow> = stmt
        .query_map(params![conversation_id], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, Option<String>>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, Option<String>>(3)?,
            ))
        })?
        .filter_map(|r| r.ok())
        .collect();
    let text = |html: &Option<String>, body: &Option<String>| -> String {
        let raw = html
            .as_deref()
            .filter(|h| !h.is_empty())
            .or(body.as_deref())
            .unwrap_or("");
        crate::textutil::html_to_text(raw).to_lowercase()
    };
    let is_agent_reply = |t: &str, author: Option<&str>| -> bool {
        t == "reply"
            || t == "message"
            || (matches!(t, "chat" | "phone") && matches!(author, Some("user") | Some("agent")))
    };
    let customer_msgs: Vec<String> = rows
        .iter()
        .filter(|(t, _, _, _)| t == "customer")
        .map(|(_t, h, b, _a)| text(h, b))
        .collect();
    let reply_msgs: Vec<String> = rows
        .iter()
        .filter(|(t, _, _, a)| is_agent_reply(t, a.as_deref()))
        .map(|(_t, h, b, _a)| text(h, b))
        .collect();
    let note_msgs: Vec<String> = rows
        .iter()
        .filter(|(t, _, _, _)| t == "note")
        .map(|(_t, h, b, _a)| text(h, b))
        .collect();

    let mut follow_up_count: i64 = 0;
    let mut saw_reply = false;
    for (t, h, b, a) in &rows {
        if is_agent_reply(t, a.as_deref()) {
            saw_reply = true;
        } else if t == "customer" && saw_reply {
            let plain = {
                let raw = h
                    .as_deref()
                    .filter(|x| !x.is_empty())
                    .or(b.as_deref())
                    .unwrap_or("");
                crate::textutil::html_to_text(raw)
            };
            if !is_closing_acknowledgment(&plain) {
                follow_up_count += 1;
            }
        }
    }
    let clarification_re = regex::Regex::new(
        r"still|again|re-?send|clarif|you didn'?t|that didn'?t|not what i|same issue|as i (said|mentioned|wrote)",
    )
    .expect("static");
    let clarification_count = customer_msgs
        .iter()
        .skip(1)
        .filter(|t| clarification_re.is_match(t))
        .count() as i64;
    let note_re = regex::Regex::new(r"(?i)escalat|urgent|priority|vip").expect("static");
    let reply_re = regex::Regex::new(r"(?i)escalat").expect("static");
    let escalated = note_msgs.iter().any(|t| note_re.is_match(t))
        || reply_msgs.iter().any(|t| reply_re.is_match(t));
    let resolved_after_first: Option<bool> =
        (!reply_msgs.is_empty()).then(|| follow_up_count == 0 && conv.status == "closed");
    let effort = (customer_msgs.len() as f64) * 1.2
        + (follow_up_count as f64) * 1.5
        + (clarification_count as f64) * 2.0
        + i64::from(escalated) as f64 * 2.0;
    let effort_score =
        (!customer_msgs.is_empty()).then(|| (effort.min(10.0) * 10.0).round() / 10.0);
    let friction = match effort_score {
        Some(s) if s >= 6.0 => "high",
        Some(s) if s >= 3.5 => "moderate",
        _ => "none",
    }
    .to_string();
    let joined = reply_msgs.concat();
    let response_style: Option<String> = (!reply_msgs.is_empty()).then(|| {
        let avg = joined.len() as f64 / reply_msgs.len() as f64;
        let step_re =
            regex::Regex::new(r"(?i)\b(step|first|then|next|finally)\b|1\.").expect("static");
        if avg > 700.0 {
            "detailed_explanation".to_string()
        } else if step_re.is_match(&joined) {
            "step_by_step".to_string()
        } else if avg < 200.0 {
            "short_answer".to_string()
        } else {
            "direct_answer_with_explanation".to_string()
        }
    });

    let computed = OutcomeComputed {
        effort_score,
        friction: friction.clone(),
        follow_up_count,
        clarification_count,
        escalated,
        resolved_after_first_response: resolved_after_first,
        response_style: response_style.clone(),
    };
    upsert_outcome(
        conn,
        customer_id,
        conversation_id,
        resolved_after_first,
        follow_up_count,
        clarification_count,
        escalated,
        effort_score,
        response_style.as_deref(),
        Some(&friction),
    )?;
    Ok(Some(computed))
}

pub(crate) fn outcome_summary(
    conn: &Connection,
    customer_id: i64,
) -> Result<Option<SupportOutcomeSummary>> {
    let outcomes = get_outcomes_for_customer(conn, customer_id)?;
    if outcomes.is_empty() {
        return Ok(None);
    }
    let total = outcomes.len() as i64;
    let resolved = outcomes
        .iter()
        .filter(|o| o.resolved_after_first_response == Some(1))
        .count() as i64;
    let with_follow_ups = outcomes.iter().filter(|o| o.follow_up_count > 0).count() as i64;
    let with_clarifications = outcomes
        .iter()
        .filter(|o| o.clarification_count > 0)
        .count() as i64;
    let escalated = outcomes.iter().filter(|o| o.escalated == 1).count() as i64;
    let avg_effort = outcomes
        .iter()
        .map(|o| o.effort_score.unwrap_or(0.0))
        .sum::<f64>()
        / total as f64;

    let conv_rows = get_customer_conversations(conn, customer_id, None).unwrap_or_default();
    let conv_numbers: std::collections::HashMap<i64, i64> =
        conv_rows.iter().map(|c| (c.id, c.number)).collect();
    let conv_subjects: std::collections::HashMap<i64, Option<String>> = conv_rows
        .iter()
        .map(|c| (c.id, c.subject.clone()))
        .collect();

    let mut style_groups: std::collections::HashMap<String, (i64, i64, Option<i64>)> =
        std::collections::HashMap::new();
    for o in &outcomes {
        let Some(style) = o.response_style.as_deref() else {
            continue;
        };
        let g = style_groups
            .entry(style.to_string())
            .or_insert((0, 0, None));
        g.1 += 1;
        if o.resolved_after_first_response == Some(1) {
            g.0 += 1;
            if g.2.is_none() {
                g.2 = Some(o.conversation_id);
            }
        }
    }
    let mut effective_approaches: Vec<EffectiveApproach> = style_groups
        .into_iter()
        .map(|(approach, (worked, _, example))| EffectiveApproach {
            approach: humanize_style(&approach),
            worked_count: worked,
            example_conversation_local_id: example,
            example_number: example.and_then(|id| conv_numbers.get(&id).copied()),
        })
        .collect();
    effective_approaches.sort_by_key(|a| std::cmp::Reverse(a.worked_count));
    let friction_flags: Vec<FrictionFlag> = outcomes
        .iter()
        .filter(|o| {
            o.friction.as_deref() == Some("high") || o.friction.as_deref() == Some("moderate")
        })
        .map(|o| FrictionFlag {
            conversation_local_id: o.conversation_id,
            number: conv_numbers.get(&o.conversation_id).copied().unwrap_or(0),
            subject: conv_subjects.get(&o.conversation_id).cloned().flatten(),
            friction: o.friction.clone().unwrap_or_else(|| "moderate".into()),
        })
        .collect();

    Ok(Some(SupportOutcomeSummary {
        customer_local_id: customer_id,
        total_conversations: total,
        first_response_resolution_rate: ratio(resolved, total),
        follow_up_rate: ratio(with_follow_ups, total),
        clarification_rate: ratio(with_clarifications, total),
        escalation_rate: ratio(escalated, total),
        avg_effort_score: (avg_effort * 10.0).round() / 10.0,
        effective_approaches,
        friction_flags,
    }))
}

pub(crate) fn heuristic_recommendation(
    current: &CurrentInteraction,
    changes: &[InteractionChange],
    baseline: Option<&BehaviorBaseline>,
    overrides: &[OverrideRow],
) -> Option<SupportApproach> {
    let sig: std::collections::HashMap<&str, &str> = current
        .signals
        .iter()
        .map(|s| (s.dimension.as_str(), s.value.as_str()))
        .collect();
    let pref_override = overrides.iter().find(|o| {
        o.field == "response_preference"
            && crate::interaction_signals::dimension_vocabulary("response_preference")
                .is_some_and(|vocab| vocab.contains(&o.human_value.as_str()))
    });
    let explicit_pref = current.signals.iter().find(|s| {
        s.dimension == "response_preference" && s.source == "heuristic" && s.confidence == "high"
    });
    let urgency = sig.get("urgency").copied().unwrap_or("none");
    let frustration = sig.get("frustration").copied().unwrap_or("none");
    let detail = sig.get("detail").copied().unwrap_or("moderate");

    let mut avoid: Vec<String> = Vec::new();
    if frustration == "strong" || frustration == "moderate" {
        avoid.push("repeating troubleshooting steps the customer already described".into());
    }
    if urgency == "high" {
        avoid.push("generic responses that do not address the reported impact".into());
    }
    if detail == "low" || detail == "very_low" {
        avoid.push("long background explanations before the answer".into());
    }
    if current.message_stats.question_count > 1 {
        avoid.push("answering only one of several questions".into());
    }

    let mut strategy: Vec<String> = Vec::new();
    strategy.push("Acknowledge the specific issue the customer reported".into());
    if frustration != "none" {
        strategy.push("Recognize the reported impact before troubleshooting".into());
    }
    strategy.push("Answer the primary question directly".into());
    if crate::interaction_signals::needs_de_escalation(&current.signals) {
        strategy.push("Explain what is being checked and what happens next".into());
    }
    strategy.push("State the concrete next action".into());
    if urgency == "high" {
        strategy.push("Set expectations using only supported timeframes".into());
    }

    let length = if pref_override.is_some_and(|o| o.human_value == "concise")
        || explicit_pref.is_some_and(|s| s.value == "concise")
        || detail == "low"
        || detail == "very_low"
    {
        "concise"
    } else if detail == "very_high"
        || explicit_pref.is_some_and(|s| s.value == "detailed")
        || pref_override.is_some_and(|o| o.human_value == "detailed")
    {
        "detailed"
    } else {
        "moderate"
    };

    let mut why: Vec<String> = Vec::new();
    if let Some(b) = baseline {
        why.push(format!(
            "{} observations across {} conversations inform this approach",
            b.observation_count, b.conversation_count
        ));
    }
    let significant: Vec<&InteractionChange> = changes.iter().filter(|c| c.significant).collect();
    if !significant.is_empty() {
        why.push(format!(
            "today's interaction differs from the customer's norm: {}",
            significant
                .iter()
                .map(|c| format!("{} {}", c.dimension.replace('_', " "), c.direction))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    if explicit_pref.is_some() {
        why.push(
            "the customer explicitly requested this response style in the current message".into(),
        );
    }
    if pref_override.is_some() {
        why.push(
            "a support rep manually set a preference that takes precedence over AI inference"
                .into(),
        );
    }
    if why.is_empty() {
        why.push("based on observable signals in the current message".into());
    }

    Some(SupportApproach {
        tone: Some(
            if frustration != "none" || urgency == "high" {
                "Calm and direct"
            } else {
                "Direct and friendly"
            }
            .into(),
        ),
        length: Some(length.into()),
        start_with: Some(
            if frustration != "none" {
                "Acknowledge the specific problem and its impact, then answer the primary question."
            } else {
                "Answer the primary question directly."
            }
            .into(),
        ),
        then: Some("Explain the action being taken and what happens next.".into()),
        avoid,
        response_strategy: strategy,
        de_escalation: crate::interaction_signals::needs_de_escalation(&current.signals),
        escalation_recommendation: (sig.get("expectation") == Some(&"escalation")).then(|| {
            "The customer is asking for escalation — review the linked previous cases before replying.".to_string()
        }),
        why,
        source: if pref_override.is_some() {
            "ai+human-override".into()
        } else {
            "heuristic".into()
        },
        confidence: baseline
            .filter(|b| {
                b.observation_count
                    >= crate::intelligence::MIN_OBSERVATIONS_FOR_PREFERENCE as i64
            })
            .map(|_| "medium".into())
            .unwrap_or_else(|| "low".into()),
    })
}

const STOPWORDS: [&str; 16] = [
    "this", "that", "with", "from", "have", "been", "after", "before", "about", "would", "could",
    "their", "there", "issue", "problem", "support",
];

fn subject_tokens(subject: &str) -> Vec<String> {
    subject
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| t.chars().count() > 3 && !STOPWORDS.contains(t))
        .map(String::from)
        .collect()
}

fn conversation_tags(conn: &Connection, conversation_id: i64) -> Vec<String> {
    crate::db::query_rows(
        conn,
        "SELECT t.name FROM conversation_tags ct
           JOIN tags t ON t.id = ct.tag_id
          WHERE ct.conversation_id = ?1",
        params![conversation_id],
        |r| r.get::<_, String>(0),
    )
    .unwrap_or_default()
}

pub(crate) fn detect_repeat_issue(
    conn: &Connection,
    conversation_id: i64,
) -> Result<Option<RepeatIssue>> {
    let Some(conv) = conversation_info(conn, conversation_id) else {
        return Ok(None);
    };
    let Some(customer_id) = conv.customer_local_id else {
        return Ok(None);
    };
    let current_subject_tokens = subject_tokens(conv.subject.as_deref().unwrap_or(""));
    let current_tags: Vec<String> = conversation_tags(conn, conversation_id)
        .into_iter()
        .map(|t| t.to_lowercase())
        .collect();
    let history = get_customer_conversations(conn, customer_id, Some(conversation_id))?;
    let mut related: Vec<(&CustomerConversation, f64)> = history
        .iter()
        .map(|h| {
            let tokens = subject_tokens(h.subject.as_deref().unwrap_or(""));
            let token_overlap = tokens
                .iter()
                .filter(|t| current_subject_tokens.contains(t))
                .count() as f64
                / 1f64.max(current_subject_tokens.len().min(tokens.len().max(1)) as f64);
            let history_tags = conversation_tags(conn, h.id)
                .into_iter()
                .map(|t| t.to_lowercase())
                .collect::<Vec<_>>();
            let tag_overlap = current_tags
                .iter()
                .filter(|t| history_tags.contains(t))
                .count() as f64
                / 1f64.max(current_tags.len().min(history_tags.len()) as f64);
            let score = token_overlap.max(tag_overlap * 0.6);
            (h, score)
        })
        .filter(|(_, score)| *score >= 0.4)
        .collect::<Vec<_>>();
    related.sort_by(|a, b| b.1.total_cmp(&a.1));
    let related: Vec<RelatedConversation> = related
        .into_iter()
        .take(5)
        .map(|(h, _)| RelatedConversation {
            local_id: h.id,
            number: h.number,
            subject: h.subject.clone(),
        })
        .collect();
    Ok(Some(RepeatIssue {
        detected: related.len() >= 2,
        related_conversations: related,
    }))
}

fn ensure_history_backfill(conn: &Connection, customer_id: Option<i64>) -> Result<()> {
    let Some(customer_id) = customer_id else {
        return Ok(());
    };
    let conv_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM conversations WHERE customer_local_id = ?1 AND deleted_at IS NULL",
        params![customer_id],
        |r| r.get(0),
    )?;
    if conv_count == 0 {
        return Ok(());
    }
    let covered_count: i64 = conn.query_row(
        "SELECT COUNT(DISTINCT conversation_id) FROM client_behavior_observations
          WHERE customer_id = ?1 AND conversation_id IS NOT NULL",
        params![customer_id],
        |r| r.get(0),
    )?;
    if covered_count >= conv_count {
        return Ok(());
    }
    let convs = get_customer_conversations(conn, customer_id, None)?;
    if convs.is_empty() {
        return Ok(());
    }
    let covered: std::collections::HashSet<i64> = {
        let mut stmt = conn.prepare(
            "SELECT DISTINCT conversation_id FROM client_behavior_observations
              WHERE customer_id = ?1 AND conversation_id IS NOT NULL",
        )?;
        let rows: std::collections::HashSet<i64> = stmt
            .query_map(params![customer_id], |r| r.get::<_, i64>(0))?
            .filter_map(|r| r.ok())
            .collect();
        rows
    };
    let mut added = false;
    for c in &convs {
        if covered.contains(&c.id) {
            continue;
        }
        if crate::interaction_signals::record_current_interaction_opts(conn, c.id, false).is_err() {
            continue;
        }
        let _ = compute_outcome(conn, c.id);
        added = true;
    }
    if added {
        rebuild_baseline(conn, customer_id)?;
    }
    Ok(())
}

fn build_timeline(conv_rows: &[CustomerConversation]) -> Vec<TimelineEntry> {
    let mut sorted: Vec<&CustomerConversation> = conv_rows.iter().collect();
    sorted.sort_by(|a, b| a.remote_created_at.cmp(&b.remote_created_at));
    let mut by_month: std::collections::BTreeMap<String, (i64, Vec<i64>, Vec<String>)> =
        std::collections::BTreeMap::new();
    for c in sorted {
        let month = c
            .remote_created_at
            .as_deref()
            .unwrap_or("")
            .chars()
            .take(7)
            .collect::<String>();
        let g = by_month.entry(month).or_insert((0, Vec::new(), Vec::new()));
        g.0 += 1;
        g.1.push(c.id);
        g.2.push(c.subject.clone().unwrap_or_else(|| "(no subject)".into()));
    }
    by_month
        .into_iter()
        .rev()
        .take(12)
        .map(|(month, (count, ids, subjects))| TimelineEntry {
            month,
            conversation_count: count,
            summary: Some(if count == 1 {
                subjects.first().cloned().unwrap_or_default()
            } else {
                let head: Vec<&str> = subjects.iter().take(2).map(String::as_str).collect();
                format!(
                    "{} tickets: {}{}",
                    count,
                    head.join(" · "),
                    if count > 2 { " …" } else { "" }
                )
            }),
            conversation_local_ids: ids,
        })
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect()
}

pub fn build_profile(
    conn: &Connection,
    customer_id: i64,
) -> Result<Option<ClientInteractionProfile>> {
    let exists: i64 = conn.query_row(
        "SELECT COUNT(*) FROM customers WHERE id = ?1",
        params![customer_id],
        |r| r.get(0),
    )?;
    if exists == 0 {
        return Ok(None);
    }
    ensure_history_backfill(conn, Some(customer_id))?;
    let conv_rows = get_customer_conversations(conn, customer_id, None)?;
    let baseline = get_baseline(conn, customer_id)?;
    let observations = get_observations_for_customer(conn, customer_id)?;
    let pref_observations: Vec<&ObservationRow> = observations
        .iter()
        .filter(|o| o.dimension == "response_preference")
        .collect();
    let mut pref_conv_sets: std::collections::HashMap<String, std::collections::HashSet<i64>> =
        std::collections::HashMap::new();
    for o in &pref_observations {
        if let Some(conv_id) = o.conversation_id {
            pref_conv_sets
                .entry(o.value.clone())
                .or_default()
                .insert(conv_id);
        }
    }
    let mut preferences = get_preferences(conn, customer_id)?;
    let min_pref: i64 = crate::intelligence::MIN_OBSERVATIONS_FOR_PREFERENCE as i64;
    for (value, convs) in &pref_conv_sets {
        if convs.len() as i64 >= min_pref
            && !preferences
                .iter()
                .any(|p| p.preference == *value && p.origin == "human_entered")
        {
            let rows: Vec<&ObservationRow> = pref_observations
                .iter()
                .filter(|o| o.value == *value && !o.observed_at.is_empty())
                .copied()
                .collect();
            let mut dates: Vec<&str> = rows.iter().map(|o| o.observed_at.as_str()).collect();
            dates.sort();
            let confidence = if convs.len() >= 5 { "high" } else { "medium" };
            let last = dates.last().map(|d| d.to_string());
            upsert_preference(
                conn,
                customer_id,
                value,
                convs.len() as i64,
                dates.first().copied(),
                last.as_deref(),
                confidence,
                "ai_inferred",
            )?;
        }
    }
    preferences = get_preferences(conn, customer_id)?;
    let preferences: Vec<CommunicationPreference> = preferences
        .into_iter()
        .filter(|p| {
            p.origin == "human_entered"
                || p.evidence_count >= crate::intelligence::MIN_OBSERVATIONS_FOR_PREFERENCE as i64
        })
        .collect();

    let timeline = build_timeline(&conv_rows);
    let outcomes = outcome_summary(conn, customer_id)?;
    let playbook = build_playbook(baseline.as_ref(), outcomes.as_ref());
    let overrides = get_active_overrides(conn, customer_id)?;
    Ok(Some(ClientInteractionProfile {
        customer_local_id: customer_id,
        client_kind: if conv_rows.is_empty() {
            "first_time".into()
        } else {
            "returning".into()
        },
        baseline,
        preferences,
        timeline,
        outcomes,
        playbook,
        overrides,
    }))
}

fn build_playbook(
    baseline: Option<&BehaviorBaseline>,
    outcomes: Option<&SupportOutcomeSummary>,
) -> Option<ClientPlaybook> {
    if baseline.is_none() && outcomes.is_none() {
        return None;
    }
    let dims: std::collections::HashMap<&str, &str> = baseline
        .map(|b| {
            b.dimensions
                .iter()
                .map(|d| (d.dimension.as_str(), d.typical_value.as_str()))
                .collect()
        })
        .unwrap_or_default();
    let effective: Vec<&EffectiveApproach> = outcomes
        .map(|o| {
            o.effective_approaches
                .iter()
                .filter(|a| a.worked_count > 0)
                .collect()
        })
        .unwrap_or_default();
    let mut avoid: Vec<String> = Vec::new();
    let friction_high = outcomes
        .map(|o| o.friction_flags.iter().any(|f| f.friction == "high"))
        .unwrap_or(false);
    if dims.get("detail") == Some(&"very_high") || dims.get("detail") == Some(&"high") {
        avoid.push("one-line answers with no context".into());
    }
    if dims.get("detail") == Some(&"low") || dims.get("detail") == Some(&"very_low") {
        avoid.push("long background explanations".into());
    }
    if friction_high {
        avoid.push("asking for information the customer already provided".into());
    }
    Some(ClientPlaybook {
        best_opening: if dims.get("directness") == Some(&"direct")
            || dims.get("directness") == Some(&"highly_direct")
        {
            "Acknowledge the issue directly and answer the primary question first.".into()
        } else {
            "Friendly greeting, then the answer with brief context.".into()
        },
        best_explanation_style: if dims.get("technical_language") == Some(&"technical")
            || dims.get("technical_language") == Some(&"highly_technical")
        {
            "Technical but concise.".into()
        } else if dims.get("detail") == Some(&"low") || dims.get("detail") == Some(&"very_low") {
            "Short paragraphs, plain language.".into()
        } else {
            "Moderate detail with concrete examples.".into()
        },
        best_troubleshooting_style: if effective.iter().any(|e| e.approach.contains("step")) {
            "Numbered steps.".into()
        } else {
            "Direct fix with a short explanation.".into()
        },
        likely_follow_up: outcomes
            .and_then(|o| o.follow_up_rate)
            .filter(|r| *r > 0.4)
            .map(|_| {
                "Often asks follow-up questions — consider covering likely next questions proactively."
                    .to_string()
            }),
        historically_successful: (!effective.is_empty()).then(|| {
            effective
                .iter()
                .take(3)
                .map(|e| format!("{} (worked in {} cases)", e.approach, e.worked_count))
                .collect::<Vec<_>>()
                .join("; ")
        }),
        avoid,
    })
}
