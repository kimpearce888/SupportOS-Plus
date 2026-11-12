use super::*;

pub(super) fn check_unanswered_questions(
    conv: &ConvRow,
    threads: &[ThreadRow],
    draft: &str,
) -> Value {
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

pub(super) fn check_duplicated_questions(threads: &[ThreadRow], draft: &str) -> Value {
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

pub(super) fn check_timeframe(conn: &Connection, conv: &ConvRow, draft: &str) -> Value {
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

pub(super) fn check_acknowledgment(threads: &[ThreadRow], signals: &str, draft: &str) -> Value {
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

pub(super) fn check_wording(
    preference: Option<&(String, String, bool, i64)>,
    draft: &str,
) -> Value {
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

pub(super) fn check_detail(signals: &str, threads: &[ThreadRow], draft: &str) -> Value {
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
                if technical {
                    " and technical-language signals"
                } else {
                    ""
                }
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

pub(super) fn check_internal_leakage(
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
                        if let Some(v) = value
                            && !v.trim().is_empty()
                        {
                            out.push((describe(&name, field, &v), v));
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
            if let Some(content) = content
                && !content.trim().is_empty()
            {
                corpus.push((
                    format!("internal knowledge document \"{title}\""),
                    content.chars().take(8000).collect(),
                ));
            }
        }
    }
    {
        let articles: Vec<(String, String)> = match conn.prepare(
            "SELECT DISTINCT da.name, COALESCE(da.text, '') FROM ai_sources s
                JOIN ai_runs r ON r.id = s.run_id
                JOIN docs_articles da ON da.id = s.source_id
             WHERE r.conversation_id = ?1 AND s.source_type = 'help_center_article'
               AND COALESCE(da.status, 'draft') != 'published'",
        ) {
            Ok(mut stmt) => stmt
                .query_map(params![conv.id], |r| {
                    Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
                })
                .map(|rows| rows.filter_map(|r| r.ok()).collect())
                .unwrap_or_default(),
            Err(_) => Vec::new(),
        };
        for (title, content) in articles.into_iter().take(10) {
            if !content.trim().is_empty() {
                corpus.push((
                    format!("internal help center article \"{title}\""),
                    content.chars().take(8000).collect(),
                ));
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

pub(super) fn check_wrong_context(
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
                if let Some(conv_customer) = conv.customer_id
                    && row_customer != Some(conv_customer) {
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

    let greeting_re = Regex::new(r"(?im)^(?:hi|hello|hey|dear)\s+([a-z][a-z'-]{1,30})\b").unwrap();
    if let Some(greeting) = greeting_re.captures(draft)
        && let Some((first_name, _)) = customer
        && let Some(greeted) = greeting.get(1).map(|m| m.as_str().to_lowercase())
    {
        let first = first_name.to_lowercase();
        let generic = [
            "team", "support", "there", "all", "everyone", "sir", "madam", "folks",
        ];
        if !generic.contains(&greeted.as_str()) && greeted != first && !first.starts_with(&greeted)
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

pub(super) fn check_preference_mismatch(
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
            if overridden {
                " (human override active)"
            } else {
                ""
            }
        ),
        format!(
            "preference {pref}, confidence {confidence}, {evidence_count} supporting conversation(s)"
        ),
        None,
        None,
        None,
    )];
    if let Some(mismatch) = mismatch {
        return result(
            kind,
            "flagged",
            format!(
                "Draft shape does not match the observed preference ({mismatch}). This is an observed preference, not a rule."
            ),
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
