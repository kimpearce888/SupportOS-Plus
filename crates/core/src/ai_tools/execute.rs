use super::*;

fn clamp_limit(args: &serde_json::Map<String, Value>) -> i64 {
    let n = arg_i64(args, "limit").unwrap_or(5);
    n.clamp(1, 10)
}

fn arg_i64(args: &serde_json::Map<String, Value>, key: &str) -> Option<i64> {
    let n = args.get(key)?.as_number()?;
    if let Some(i) = n.as_i64() {
        return Some(i);
    }
    let f = n.as_f64()?;
    if f.fract() == 0.0 && f >= -(2f64.powi(63)) && f < 2f64.powi(63) {
        return Some(f as i64);
    }
    None
}

fn redact(conn: &Connection, text: &str) -> String {
    let enabled = crate::settings::get_bool(conn, "redaction_enabled", true).unwrap_or(true);
    crate::security::redact_text(text, enabled).0
}

fn redact_chars(conn: &Connection, text: &str, n: usize) -> String {
    let taken: String = text.chars().take(n).collect();
    redact(conn, &taken)
}

fn conversation_by_number(conn: &Connection, number: i64) -> Option<(i64, Option<i64>)> {
    conn.query_row(
        "SELECT id, customer_local_id FROM conversations WHERE number = ?1 AND deleted_at IS NULL",
        params![number],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )
    .ok()
}

pub fn execute(
    conn: &Connection,
    index: Option<&crate::vectorstore_sqlitevec::VectorIndex>,
    name: &str,
    args_json: &str,
) -> Value {
    let args: serde_json::Map<String, Value> = match serde_json::from_str(if args_json.is_empty() {
        "{}"
    } else {
        args_json
    }) {
        Ok(Value::Object(map)) => map,
        _ => return json!({ "error": "Invalid tool arguments" }),
    };
    let limit = clamp_limit(&args);
    match name {
        "search_conversations" => tool_search_conversations(conn, &args, limit),
        "search_knowledge" => tool_search_knowledge(conn, &args, limit),
        "search_known_issues" => tool_search_known_issues(conn, &args, limit),
        "search_saved_replies" => tool_search_saved_replies(conn, &args, limit),
        "get_conversation" => tool_get_conversation(conn, &args),
        "get_support_metrics" => tool_get_support_metrics(conn, &args),
        "get_conversation_context" => tool_get_conversation_context(conn, &args),
        "get_customer_history" => tool_get_customer_history(conn, &args, limit),
        "get_similar_conversations" => tool_get_similar_conversations(conn, &args, index, limit),
        "get_issue_clusters" => tool_get_issue_clusters(conn, &args, limit),
        "get_ai_analysis" => tool_get_ai_analysis(conn, &args),
        "get_supportos_metadata" => tool_get_supportos_metadata(conn, &args),
        "get_ai_attributes" => tool_get_ai_attributes(conn, &args),
        "search_incidents" => tool_search_incidents(conn, &args, limit),
        "search_custom_objects" => tool_search_custom_objects(conn, &args, limit),
        "get_customer_timeline" => tool_get_customer_timeline(conn, &args, limit),
        "search_connector_data" => tool_search_connector_data(conn, &args, limit),
        "get_knowledge_gaps" => tool_get_knowledge_gaps(conn, &args, limit),
        "get_friction_report" => tool_get_friction_report(conn, &args),
        "get_graph_neighbors" => tool_get_graph_neighbors(conn, &args),
        "get_graph_stats" => tool_get_graph_stats(conn),
        "get_customer_memory" => tool_get_customer_memory(conn, &args),
        _ => {
            json!({ "error": format!("Unknown tool {name} - allowed tools are read-only search tools") })
        }
    }
}

fn tool_search_conversations(
    conn: &Connection,
    args: &serde_json::Map<String, Value>,
    limit: i64,
) -> Value {
    let q: String = args
        .get("query")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .chars()
        .take(200)
        .collect();
    let hits = crate::search::search_conversations(
        conn,
        &q,
        &crate::search::SearchFilters::default(),
        limit,
    )
    .unwrap_or_default();
    json!(
        hits.iter()
            .map(|h| {
                let number: i64 = h
                    .title
                    .split(' ')
                    .next()
                    .map(|w| w.trim_start_matches('#'))
                    .and_then(|w| w.parse().ok())
                    .unwrap_or(h.id);
                json!({
                    "number": number,
                    "title": h.title,
                    "snippet": redact_chars(conn, &h.snippet, 300),
                })
            })
            .collect::<Vec<_>>()
    )
}

fn tool_search_knowledge(
    conn: &Connection,
    args: &serde_json::Map<String, Value>,
    limit: i64,
) -> Value {
    let q: String = args
        .get("query")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .chars()
        .take(200)
        .collect();
    let hits = crate::search::search_knowledge(conn, &q, None, limit).unwrap_or_default();
    json!(
        hits.iter()
            .map(|h| json!({
                "title": h.title,
                "snippet": redact_chars(conn, &h.snippet, 300),
            }))
            .collect::<Vec<_>>()
    )
}

fn tool_search_known_issues(
    conn: &Connection,
    args: &serde_json::Map<String, Value>,
    limit: i64,
) -> Value {
    let q: String = args
        .get("query")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .chars()
        .take(200)
        .collect();
    let hits = crate::search::search_known_issues(conn, &q, limit).unwrap_or_default();
    json!(
        hits.iter()
            .map(|h| json!({
                "id": h.id,
                "title": h.title,
                "snippet": redact_chars(conn, &h.snippet, 300),
            }))
            .collect::<Vec<_>>()
    )
}

fn tool_search_saved_replies(
    conn: &Connection,
    args: &serde_json::Map<String, Value>,
    limit: i64,
) -> Value {
    let q: String = args
        .get("query")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .chars()
        .take(200)
        .collect();
    let hits = crate::search::search_saved_replies(conn, &q, limit).unwrap_or_default();
    json!(
        hits.iter()
            .map(|h| json!({
                "id": h.id,
                "title": h.title,
                "snippet": redact_chars(conn, &h.snippet, 300),
            }))
            .collect::<Vec<_>>()
    )
}

fn tool_get_conversation(conn: &Connection, args: &serde_json::Map<String, Value>) -> Value {
    let Some(num) = arg_i64(args, "number") else {
        return json!({ "error": "Invalid number" });
    };
    let row = conn
        .query_row(
            "SELECT id, number, subject, preview, status, COALESCE(remote_created_at, created_at)
               FROM conversations WHERE number = ?1",
            params![num],
            |r| {
                Ok(json!({
                    "id": r.get::<_, i64>(0)?,
                    "number": r.get::<_, i64>(1)?,
                    "subject": r.get::<_, Option<String>>(2)?,
                    "preview": r.get::<_, Option<String>>(3)?,
                    "status": r.get::<_, String>(4)?,
                    "remote_created_at": r.get::<_, Option<String>>(5)?,
                }))
            },
        )
        .ok();
    let Some(mut row) = row else {
        return json!({ "error": "Conversation not found" });
    };
    let id = row["id"].as_i64().unwrap_or_default();
    let threads = conn
        .prepare(
            "SELECT type, COALESCE(body_text, ''), COALESCE(remote_created_at, created_at)
               FROM conversation_threads
              WHERE conversation_id = ?1
              ORDER BY COALESCE(remote_created_at, created_at) DESC LIMIT 3",
        )
        .and_then(|mut stmt| {
            stmt.query_map(params![id], |r| {
                Ok(json!({
                    "type": r.get::<_, String>(0)?,
                    "text": redact_chars(conn, &r.get::<_, String>(1)?, 400),
                    "date": r.get::<_, Option<String>>(2)?,
                }))
            })
            .map(|rows| rows.filter_map(|t| t.ok()).collect::<Vec<_>>())
        })
        .unwrap_or_default();
    row["recent_threads"] = json!(threads);
    row
}

fn tool_get_support_metrics(conn: &Connection, args: &serde_json::Map<String, Value>) -> Value {
    let days = arg_i64(args, "days").unwrap_or(30).clamp(1, 365);
    let (conversations, active, closed) = conn
        .query_row(
            "SELECT COUNT(*),
                    SUM(CASE WHEN status='active' THEN 1 ELSE 0 END),
                    SUM(CASE WHEN status='closed' THEN 1 ELSE 0 END)
               FROM conversations
              WHERE deleted_at IS NULL
                AND julianday(COALESCE(remote_created_at, created_at)) >= julianday('now', '-' || ?1 || ' days')",
            params![days],
            |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, Option<i64>>(1)?.unwrap_or(0),
                    r.get::<_, Option<i64>>(2)?.unwrap_or(0),
                ))
            },
        )
        .unwrap_or((0, 0, 0));
    json!({
        "window_days": days,
        "conversations": conversations,
        "active": active,
        "closed": closed,
    })
}

fn tool_get_conversation_context(
    conn: &Connection,
    args: &serde_json::Map<String, Value>,
) -> Value {
    let Some(num) = arg_i64(args, "number") else {
        return json!({ "error": "Invalid number" });
    };
    let conv = conn
        .query_row(
            "SELECT c.id, c.number, c.subject, c.status, c.type,
                    COALESCE(c.remote_created_at, c.created_at), c.remote_updated_at,
                    (SELECT m.name FROM mailboxes m WHERE m.id = c.mailbox_local_id) AS mailbox,
                    (SELECT GROUP_CONCAT(t.name) FROM conversation_tags ct
                       JOIN tags t ON t.id = ct.tag_id WHERE ct.conversation_id = c.id) AS tags,
                    (SELECT TRIM(COALESCE(cu.first_name, '') || ' ' || COALESCE(cu.last_name, ''))
                       FROM customers cu WHERE cu.id = c.customer_local_id) AS customer
               FROM conversations c WHERE c.number = ?1 AND c.deleted_at IS NULL",
            params![num],
            |r| {
                Ok(json!({
                    "id": r.get::<_, i64>(0)?,
                    "number": r.get::<_, i64>(1)?,
                    "subject": r.get::<_, Option<String>>(2)?,
                    "status": r.get::<_, String>(3)?,
                    "type": r.get::<_, Option<String>>(4)?,
                    "remote_created_at": r.get::<_, Option<String>>(5)?,
                    "remote_updated_at": r.get::<_, Option<String>>(6)?,
                    "mailbox": r.get::<_, Option<String>>(7)?,
                    "tags": r.get::<_, Option<String>>(8)?,
                    "customer": r.get::<_, Option<String>>(9)?,
                }))
            },
        )
        .ok();
    let Some(conv) = conv else {
        return json!({ "error": "Conversation not found" });
    };
    let id = conv["id"].as_i64().unwrap_or_default();
    let messages: Vec<(String, String, Option<String>, String)> = conn
        .prepare(
            "SELECT COALESCE(from_name, type, 'unknown'), type,
                    COALESCE(remote_created_at, created_at), COALESCE(body_html, body_text, '')
               FROM conversation_threads
              WHERE conversation_id = ?1 AND deleted_at IS NULL AND state = 'published'
              ORDER BY COALESCE(remote_created_at, created_at) ASC LIMIT 30",
        )
        .and_then(|mut stmt| {
            stmt.query_map(params![id], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, Option<String>>(2)?,
                    r.get::<_, String>(3)?,
                ))
            })
            .map(|rows| rows.filter_map(|t| t.ok()).collect::<Vec<_>>())
        })
        .unwrap_or_default();
    let msgs: Vec<Value> = messages
        .iter()
        .map(|(author, kind, date, body)| {
            let text: String = crate::textutil::html_to_text(body)
                .chars()
                .take(700)
                .collect();
            json!({
                "author": author,
                "type": kind,
                "date": date,
                "text": redact(conn, &text),
            })
        })
        .collect();
    let mut participants: Vec<String> = Vec::new();
    for m in &msgs {
        let label = format!(
            "{} ({})",
            m["author"].as_str().unwrap_or(""),
            m["type"].as_str().unwrap_or("")
        );
        if !participants.contains(&label) {
            participants.push(label);
        }
    }
    participants.truncate(8);
    let mut out = conv;
    out["participants"] = json!(participants);
    out["messages"] = json!(msgs);
    out
}

fn tool_get_customer_history(
    conn: &Connection,
    args: &serde_json::Map<String, Value>,
    limit: i64,
) -> Value {
    let Some(num) = arg_i64(args, "number") else {
        return json!({ "error": "Invalid number" });
    };
    let Some(customer) = conn
        .query_row(
            "SELECT customer_local_id FROM conversations WHERE number = ?1 AND deleted_at IS NULL",
            params![num],
            |r| r.get::<_, Option<i64>>(0),
        )
        .ok()
        .flatten()
    else {
        return json!({ "error": "Conversation not found or has no customer" });
    };
    let tickets: Vec<HistoryTicket> = conn
        .prepare(
            "SELECT c.number, c.subject, c.status, COALESCE(c.remote_created_at, c.created_at), c.closed_at,
                    (SELECT COALESCE(t.body_text, '') FROM conversation_threads t
                      WHERE t.conversation_id = c.id AND t.deleted_at IS NULL AND t.state='published'
                      ORDER BY COALESCE(t.remote_created_at, t.created_at) ASC LIMIT 1),
                    (SELECT COALESCE(t.body_text, '') FROM conversation_threads t
                      WHERE t.conversation_id = c.id AND t.deleted_at IS NULL AND t.state='published'
                        AND t.type='reply'
                      ORDER BY COALESCE(t.remote_created_at, t.created_at) DESC LIMIT 1)
               FROM conversations c
              WHERE c.customer_local_id = ?1 AND c.deleted_at IS NULL
                AND c.id != (SELECT id FROM conversations WHERE number = ?2)
              ORDER BY COALESCE(c.remote_created_at, c.created_at) DESC LIMIT ?3",
        )
        .and_then(|mut stmt| {
            stmt.query_map(params![customer, num, limit], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, Option<String>>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, Option<String>>(3)?,
                    r.get::<_, Option<String>>(4)?,
                    r.get::<_, String>(5)?,
                    r.get::<_, String>(6)?,
                ))
            })
            .map(|rows| rows.filter_map(|t| t.ok()).collect::<Vec<_>>())
        })
        .unwrap_or_default();
    json!({
        "total_previous_tickets": tickets.len(),
        "tickets": tickets.iter().map(|(number, subject, status, created, closed, first_message, last_reply)| json!({
            "number": number,
            "subject": subject,
            "status": status,
            "created": created,
            "closed": closed,
            "first_message": redact_chars(conn, first_message, 300),
            "last_reply": redact_chars(conn, last_reply, 300),
        })).collect::<Vec<_>>(),
    })
}

fn tool_get_similar_conversations(
    conn: &Connection,
    args: &serde_json::Map<String, Value>,
    index: Option<&crate::vectorstore_sqlitevec::VectorIndex>,
    limit: i64,
) -> Value {
    let Some(num) = arg_i64(args, "number") else {
        return json!({ "error": "Invalid number" });
    };
    let Some((id, _)) = conversation_by_number(conn, num) else {
        return json!({ "error": "Conversation not found" });
    };
    let semantic: Vec<(i64, f64)> = match index {
        Some(ix) => crate::ai_evidence::semantic_hits(conn, ix, id, 8),
        None => Vec::new(),
    };
    let similar =
        crate::ai_evidence::find_similar(conn, id, limit.clamp(1, 10) as usize, &semantic)
            .unwrap_or_default();
    json!(
        similar
            .iter()
            .map(|s| json!({
                "conversation_id": s.conversation_id,
                "number": s.number,
                "subject": s.subject,
                "resolution": redact_chars(conn, &s.resolution, 400),
            }))
            .collect::<Vec<_>>()
    )
}

fn tool_get_issue_clusters(
    conn: &Connection,
    args: &serde_json::Map<String, Value>,
    limit: i64,
) -> Value {
    let q: String = args
        .get("query")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .chars()
        .take(200)
        .collect();
    let like = format!(
        "%{}%",
        q.replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_")
    );
    let sql = format!(
        "SELECT ic.id, ic.title, ic.summary, ic.category, ic.product, ic.feature, ic.ai_generated,
                (SELECT GROUP_CONCAT(c.number) FROM issue_cluster_conversations icc
                   JOIN conversations c ON c.id = icc.conversation_id
                  WHERE icc.cluster_id = ic.id ORDER BY c.number DESC) AS numbers
           FROM issue_clusters ic
          {}
          ORDER BY ic.id DESC LIMIT {}",
        if q.is_empty() {
            String::new()
        } else {
            "WHERE ic.title LIKE ?1 ESCAPE '\\' OR ic.summary LIKE ?1 ESCAPE '\\' OR ic.category LIKE ?1 ESCAPE '\\'".to_string()
        },
        limit,
    );
    let rows: Vec<Value> = if q.is_empty() {
        crate::db::query_rows(conn, &sql, [], |r| map_cluster_row(conn, r)).unwrap_or_default()
    } else {
        crate::db::query_rows(conn, &sql, params![like], |r| map_cluster_row(conn, r))
            .unwrap_or_default()
    };
    json!(rows)
}

fn tool_get_ai_analysis(conn: &Connection, args: &serde_json::Map<String, Value>) -> Value {
    let Some(num) = arg_i64(args, "number") else {
        return json!({ "error": "Invalid number" });
    };
    let Some((id, _)) = conversation_by_number(conn, num) else {
        return json!({ "error": "Conversation not found" });
    };
    let run = conn
        .query_row(
            "SELECT response_json, model, latency_ms, created_at FROM ai_runs
              WHERE conversation_id = ?1 AND type = 'ticket_analysis' AND status = 'completed'
              ORDER BY id DESC LIMIT 1",
            params![id],
            |r| {
                Ok((
                    r.get::<_, Option<String>>(0)?,
                    r.get::<_, Option<String>>(1)?,
                    r.get::<_, Option<i64>>(2)?,
                    r.get::<_, Option<String>>(3)?,
                ))
            },
        )
        .ok();
    let Some((output, model, latency_ms, created_at)) = run else {
        return json!({
            "analysis": Value::Null,
            "note": "No AI analysis has been run for this conversation yet."
        });
    };
    let analysis = output
        .as_deref()
        .and_then(|o| serde_json::from_str::<Value>(o).ok())
        .unwrap_or(Value::Null);
    let verification = conn
        .query_row(
            "SELECT verification FROM ai_drafts
              WHERE conversation_id = ?1 AND verification IS NOT NULL
              ORDER BY id DESC LIMIT 1",
            params![id],
            |r| r.get::<_, Option<String>>(0),
        )
        .ok()
        .flatten()
        .and_then(|v| serde_json::from_str::<Value>(&v).ok())
        .map(|v| {
            json!({
                "verified": v.get("verified").and_then(Value::as_bool) == Some(true),
                "warnings": v.get("warnings").and_then(Value::as_array)
                    .map(|a| a.iter().take(5).cloned().collect::<Vec<_>>())
                    .unwrap_or_default(),
            })
        })
        .unwrap_or(Value::Null);
    json!({
        "analysis": analysis,
        "model": model,
        "analyzed_at": created_at,
        "latency_ms": latency_ms,
        "latest_draft_verification": verification,
    })
}

fn tool_get_supportos_metadata(conn: &Connection, args: &serde_json::Map<String, Value>) -> Value {
    let Some(num) = arg_i64(args, "number") else {
        return json!({ "error": "Invalid number" });
    };
    let row = conn
        .query_row(
            "SELECT c.number, c.supportos_priority, c.supportos_state_id,
                    (SELECT ts.name FROM ticket_states ts WHERE ts.id = c.supportos_state_id),
                    (SELECT TRIM(COALESCE(u2.first_name, '') || ' ' || COALESCE(u2.last_name, ''))
                       FROM users u2 WHERE u2.id = c.assignee_local_id),
                    (SELECT m.name FROM mailboxes m WHERE m.id = c.mailbox_local_id),
                    c.closed_at, c.first_customer_message_at, c.first_response_at,
                    c.last_customer_reply_at, c.last_human_agent_response_at, c.customer_waiting_since,
                    (SELECT kic.known_issue_id FROM known_issue_conversations kic
                      WHERE kic.conversation_id = c.id LIMIT 1)
               FROM conversations c WHERE c.number = ?1 AND c.deleted_at IS NULL",
            params![num],
            |r| {
                Ok(json!({
                    "number": r.get::<_, i64>(0)?,
                    "supportos_priority": r.get::<_, Option<String>>(1)?,
                    "ticket_state": r.get::<_, Option<String>>(3)?,
                    "assignee": r.get::<_, Option<String>>(4)?,
                    "mailbox": r.get::<_, Option<String>>(5)?,
                    "closed_at": r.get::<_, Option<String>>(6)?,
                    "first_customer_message_at": r.get::<_, Option<String>>(7)?,
                    "first_response_at": r.get::<_, Option<String>>(8)?,
                    "last_customer_reply_at": r.get::<_, Option<String>>(9)?,
                    "last_human_agent_response_at": r.get::<_, Option<String>>(10)?,
                    "customer_waiting_since": r.get::<_, Option<String>>(11)?,
                    "known_issue_linked": r.get::<_, Option<i64>>(12)?.is_some(),
                }))
            },
        )
        .ok();
    let Some(mut row) = row else {
        return json!({ "error": "Conversation not found" });
    };
    row["note"] = json!(
        "SLA state is computed live against mailbox business hours; customer_waiting_since is the local waiting marker."
    );
    row
}

fn tool_get_ai_attributes(conn: &Connection, args: &serde_json::Map<String, Value>) -> Value {
    let Some(num) = arg_i64(args, "number") else {
        return json!({ "error": "Invalid number" });
    };
    let Some((id, _)) = conversation_by_number(conn, num) else {
        return json!({ "error": "Conversation not found" });
    };
    let attrs: Vec<Value> = conn
        .prepare(
            "SELECT attribute, value, confidence, source, evidence, computed_at
               FROM ai_attributes WHERE conversation_id = ?1 AND superseded_at IS NULL",
        )
        .and_then(|mut stmt| {
            stmt.query_map(params![id], |r| {
                let evidence = r
                    .get::<_, Option<String>>(4)?
                    .and_then(|e| serde_json::from_str::<Value>(&e).ok())
                    .and_then(|e| e.as_array().cloned())
                    .unwrap_or_default();
                let excerpts: Vec<Value> = evidence
                    .iter()
                    .take(2)
                    .map(|e| {
                        json!({ "excerpt": redact_chars(
                            conn,
                            e.get("excerpt").and_then(Value::as_str).unwrap_or(""),
                            200,
                        )})
                    })
                    .collect();
                Ok(json!({
                    "attribute": r.get::<_, String>(0)?,
                    "value": r.get::<_, Option<String>>(1)?,
                    "confidence": r.get::<_, Option<String>>(2)?,
                    "source": r.get::<_, Option<String>>(3)?,
                    "computed_at": r.get::<_, Option<String>>(5)?,
                    "evidence": excerpts,
                }))
            })
            .map(|rows| rows.filter_map(|t| t.ok()).collect::<Vec<_>>())
        })
        .unwrap_or_default();
    json!({
        "attributes": attrs,
        "note": "Attributes absent from this list are unknown (not yet computed or no evidence)."
    })
}

fn tool_search_incidents(
    conn: &Connection,
    args: &serde_json::Map<String, Value>,
    limit: i64,
) -> Value {
    let q: String = args
        .get("query")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .chars()
        .take(200)
        .collect::<String>()
        .to_lowercase();
    let rows: Vec<Value> = conn
        .prepare(
            "SELECT i.id, i.code, i.title, i.status, i.severity, i.product, i.feature,
                    (SELECT COUNT(*) FROM incident_conversations ic WHERE ic.incident_id = i.id),
                    (SELECT COUNT(DISTINCT c.customer_local_id) FROM incident_conversations ic
                       JOIN conversations c ON c.id = ic.conversation_id
                      WHERE ic.incident_id = i.id AND c.customer_local_id IS NOT NULL AND c.deleted_at IS NULL)
               FROM incidents i
              ORDER BY CASE i.status WHEN 'resolved' THEN 1 ELSE 0 END, i.updated_at DESC LIMIT 20",
        )
        .and_then(|mut stmt| {
            stmt.query_map([], |r| {
                Ok(json!({
                    "code": r.get::<_, Option<String>>(1)?,
                    "title": r.get::<_, Option<String>>(2)?,
                    "status": r.get::<_, String>(3)?,
                    "severity": r.get::<_, String>(4)?,
                    "product": r.get::<_, Option<String>>(5)?,
                    "feature": r.get::<_, Option<String>>(6)?,
                    "conversation_count": r.get::<_, i64>(7)?,
                    "customer_count": r.get::<_, i64>(8)?,
                }))
            })
            .map(|rows| rows.filter_map(|t| t.ok()).collect::<Vec<_>>())
        })
        .unwrap_or_default();
    json!(
        rows.into_iter()
            .filter(|r| {
                q.is_empty()
                    || format!(
                        "{} {} {}",
                        r["code"].as_str().unwrap_or(""),
                        r["title"].as_str().unwrap_or(""),
                        r["product"].as_str().unwrap_or("")
                    )
                    .to_lowercase()
                    .contains(&q)
            })
            .take(limit as usize)
            .map(|mut r| {
                let conversations = r["conversation_count"].as_i64().unwrap_or(0);
                let customers = r["customer_count"].as_i64().unwrap_or(0);
                r["affected_conversations"] = json!(conversations);
                r["affected_customers"] = json!(customers);
                r["note"] =
                    json!("Affected customers are distinct customers, never ticket counts.");
                r
            })
            .collect::<Vec<_>>()
    )
}

fn tool_search_custom_objects(
    conn: &Connection,
    args: &serde_json::Map<String, Value>,
    limit: i64,
) -> Value {
    let q: String = args
        .get("query")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .chars()
        .take(200)
        .collect();
    let type_slug = args
        .get("type")
        .and_then(Value::as_str)
        .map(|s| s.chars().take(60).collect::<String>());
    let type_id: Option<i64> = type_slug.as_deref().and_then(|slug| {
        conn.query_row(
            "SELECT id FROM custom_object_types WHERE slug = ?1 AND deleted_at IS NULL",
            params![slug],
            |r| r.get(0),
        )
        .ok()
    });
    let tokens = crate::search::fts_query(&q.replace(['"', '*', '(', ')'], " "));
    if tokens == "\"\"" {
        return json!([]);
    }
    let sql = format!(
        "SELECT o.id, o.title, o.data_json, t.name AS type_name, t.slug AS type_slug
           FROM fts_custom_objects f
           JOIN custom_objects o ON o.id = f.object_id AND o.deleted_at IS NULL
           JOIN custom_object_types t ON t.id = o.type_id
          WHERE fts_custom_objects MATCH ?1 {}
          ORDER BY rank LIMIT ?2",
        if type_id.is_some() {
            "AND o.type_id = ?3"
        } else {
            ""
        },
    );
    let rows: Vec<(i64, String, String, String)> = if let Some(tid) = type_id {
        conn.prepare(&sql)
            .and_then(|mut stmt| {
                stmt.query_map(params![tokens, limit, tid], |r| {
                    Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
                })
                .map(|rows| rows.filter_map(|t| t.ok()).collect::<Vec<_>>())
            })
            .unwrap_or_default()
    } else {
        conn.prepare(&sql)
            .and_then(|mut stmt| {
                stmt.query_map(params![tokens, limit], |r| {
                    Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
                })
                .map(|rows| rows.filter_map(|t| t.ok()).collect::<Vec<_>>())
            })
            .unwrap_or_default()
    };
    json!(rows
        .iter()
        .map(|(id, title, data_json, type_name)| {
            let properties: Value = serde_json::from_str(data_json).unwrap_or(json!({}));
            let mut clean = serde_json::Map::new();
            if let Some(obj) = properties.as_object() {
                for (k, v) in obj.iter().take(12) {
                    let v = match v.as_str() {
                        Some(s) => Value::String(redact_chars(conn, s, 200)),
                        None => v.clone(),
                    };
                    clean.insert(k.clone(), v);
                }
            }
            let _ = id;
            json!({ "type": type_name, "title": redact_chars(conn, title, 200), "properties": clean })
        })
        .collect::<Vec<_>>())
}

fn tool_get_customer_timeline(
    conn: &Connection,
    args: &serde_json::Map<String, Value>,
    limit: i64,
) -> Value {
    let Some(num) = arg_i64(args, "number") else {
        return json!({ "error": "Invalid number" });
    };
    let Some(customer) = conn
        .query_row(
            "SELECT customer_local_id FROM conversations WHERE number = ?1 AND deleted_at IS NULL",
            params![num],
            |r| r.get::<_, Option<i64>>(0),
        )
        .ok()
        .flatten()
    else {
        return json!({ "error": "Conversation not found or has no customer" });
    };
    let events: Vec<Value> = conn
        .prepare(
            "SELECT event_kind, occurred_at, title, source FROM customer_events
              WHERE customer_local_id = ?1
              ORDER BY COALESCE(occurred_at, created_at) DESC, id DESC LIMIT ?2",
        )
        .and_then(|mut stmt| {
            stmt.query_map(params![customer, limit.clamp(1, 20)], |r| {
                Ok(json!({
                    "kind": r.get::<_, String>(0)?,
                    "at": r.get::<_, Option<String>>(1)?,
                    "title": redact_chars(conn, &r.get::<_, String>(2)?, 160),
                    "source": r.get::<_, String>(3)?,
                }))
            })
            .map(|rows| rows.filter_map(|t| t.ok()).collect::<Vec<_>>())
        })
        .unwrap_or_default();
    json!({ "events": events })
}

fn tool_search_connector_data(
    conn: &Connection,
    args: &serde_json::Map<String, Value>,
    limit: i64,
) -> Value {
    let connector_name: String = args
        .get("connector")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .chars()
        .take(80)
        .collect();
    if connector_name.is_empty() {
        return json!({ "error": "Invalid connector name" });
    }
    let q: String = args
        .get("query")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .chars()
        .take(200)
        .collect::<String>()
        .to_lowercase();
    let row = conn
        .query_row(
            "SELECT id, name, allowed_ai, enabled FROM connectors WHERE name = ?1",
            params![connector_name],
            |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, i64>(2)?,
                    r.get::<_, i64>(3)?,
                ))
            },
        )
        .ok();
    let Some((id, name, allowed_ai, enabled)) = row else {
        let available: Vec<String> = conn
            .prepare("SELECT name FROM connectors WHERE allowed_ai = 1 AND enabled = 1 LIMIT 10")
            .and_then(|mut stmt| {
                stmt.query_map([], |r| r.get::<_, String>(0))
                    .map(|rows| rows.filter_map(|t| t.ok()).collect::<Vec<_>>())
            })
            .unwrap_or_default();
        return json!({ "error": "Connector not found", "ai_visible_connectors": available });
    };
    if allowed_ai != 1 {
        return json!({ "error": format!("Connector \"{name}\" is not marked as AI-visible. Its data stays private to the UI.") });
    }
    if enabled != 1 {
        return json!({ "error": format!("Connector \"{name}\" is disabled.") });
    }
    let data_rows: Vec<String> = conn
        .prepare(
            "SELECT data FROM connector_rows WHERE connector_id = ?1 ORDER BY id DESC LIMIT 100",
        )
        .and_then(|mut stmt| {
            stmt.query_map(params![id], |r| r.get::<_, String>(0))
                .map(|rows| rows.filter_map(|t| t.ok()).collect::<Vec<_>>())
        })
        .unwrap_or_default();
    let results: Vec<Value> = data_rows
        .into_iter()
        .map(|d| serde_json::from_str::<Value>(&d).unwrap_or(json!({})))
        .filter(|d| q.is_empty() || d.to_string().to_lowercase().contains(&q))
        .take(limit as usize)
        .map(|d| {
            let mut out = serde_json::Map::new();
            if let Some(obj) = d.as_object() {
                for (k, v) in obj.iter().take(15) {
                    let v = match v.as_str() {
                        Some(s) => Value::String(redact_chars(conn, s, 300)),
                        None => v.clone(),
                    };
                    out.insert(k.clone(), v);
                }
            }
            Value::Object(out)
        })
        .collect();
    json!({ "connector": name, "results": results })
}

fn tool_get_knowledge_gaps(
    conn: &Connection,
    args: &serde_json::Map<String, Value>,
    limit: i64,
) -> Value {
    let kind_filter = args
        .get("kind")
        .and_then(Value::as_str)
        .map(|s| s.chars().take(60).collect::<String>());
    let rows: Vec<GapRow> = conn
        .prepare(
            "SELECT id, kind, query_text, occurrence_count, status, detail
               FROM knowledge_gap_candidates
              ORDER BY CASE status WHEN 'open' THEN 0 ELSE 1 END, occurrence_count DESC LIMIT 50",
        )
        .and_then(|mut stmt| {
            stmt.query_map([], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, Option<String>>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, i64>(3)?,
                    r.get::<_, String>(4)?,
                    r.get::<_, Option<String>>(5)?,
                ))
            })
            .map(|rows| rows.filter_map(|t| t.ok()).collect::<Vec<_>>())
        })
        .unwrap_or_default();
    json!(rows
        .into_iter()
        .filter(|r| {
            kind_filter
                .as_deref()
                .map(|k| r.1.as_deref() == Some(k))
                .unwrap_or(true)
        })
        .take(limit as usize)
        .map(|(id, kind, question, occurrences, status, detail)| {
            let explanation: String = detail
                .as_deref()
                .and_then(|d| serde_json::from_str::<Value>(d).ok())
                .and_then(|d| {
                    d.get("explanation").and_then(Value::as_str).map(str::to_string)
                })
                .unwrap_or_default();
            json!({
                "id": id,
                "kind": kind,
                "question": redact_chars(conn, &question, 200),
                "occurrences": occurrences,
                "status": status,
                "explanation": redact_chars(conn, &explanation, 300),
                "note": "Candidates are deterministic detections awaiting human approval; approving never publishes automatically."
            })
        })
        .collect::<Vec<_>>())
}

fn tool_get_friction_report(conn: &Connection, args: &serde_json::Map<String, Value>) -> Value {
    let days = arg_i64(args, "days").unwrap_or(30).clamp(1, 365);
    let rows: Vec<Value> = conn
        .prepare(
            "SELECT f.kind, COUNT(*), SUM(CASE WHEN f.severity = 'high' THEN 1 ELSE 0 END)
               FROM friction_findings f JOIN conversations c ON c.id = f.conversation_id
              WHERE COALESCE(julianday(c.remote_created_at), julianday(f.computed_at)) >= julianday('now', ?1)
              GROUP BY f.kind ORDER BY 2 DESC",
        )
        .and_then(|mut stmt| {
            stmt.query_map(params![format!("-{days} days")], |r| {
                Ok(json!({
                    "kind": r.get::<_, String>(0)?,
                    "conversations": r.get::<_, i64>(1)?,
                    "high_severity": r.get::<_, Option<i64>>(2)?.unwrap_or(0),
                }))
            })
            .map(|rows| rows.filter_map(|t| t.ok()).collect::<Vec<_>>())
        })
        .unwrap_or_default();
    json!({
        "window_days": days,
        "kinds": rows,
        "note": "Deterministic text-shape heuristics over the local mirror - patterns, not judgments about people. Findings exist only for analyzed conversations."
    })
}

fn tool_get_graph_neighbors(conn: &Connection, args: &serde_json::Map<String, Value>) -> Value {
    let kind = args
        .get("kind")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let Some(local_id) = arg_i64(args, "local_id").filter(|v| *v > 0) else {
        return json!({ "error": "Invalid local_id" });
    };
    let Some(kind_enum) = GraphNodeKind::parse(&kind) else {
        return json!({ "error": format!("Unknown node kind: {kind}") });
    };
    let Some(node_ref) = crate::support_graph::node_ref(conn, kind_enum, local_id)
        .ok()
        .flatten()
    else {
        return json!({ "error": "Node not found" });
    };
    let edges =
        crate::support_graph::human_edges_touching(conn, kind_enum, local_id).unwrap_or_default();
    let center_key = json!({"kind": kind, "local_id": local_id});
    let items: Vec<Value> = edges
        .iter()
        .map(|e| {
            let source_is_center = e["source"]["kind"] == center_key["kind"]
                && e["source"]["local_id"] == center_key["local_id"];
            let other = if source_is_center {
                &e["target"]
            } else {
                &e["source"]
            };
            json!({
                "relation": e["relation"],
                "connected_kind": other["kind"],
                "connected": redact_chars(conn, other["label"].as_str().unwrap_or(""), 120),
                "note": e["note"],
            })
        })
        .collect::<Vec<_>>();
    json!({
        "node": { "kind": kind, "label": redact_chars(conn, &node_ref.label, 160) },
        "total_edges": edges.len(),
        "edges": items,
        "note": "Edges are read from the local support graph store; only human-asserted edges are stored."
    })
}

fn tool_get_graph_stats(conn: &Connection) -> Value {
    let nodes: Vec<Value> = conn
        .prepare("SELECT kind, COUNT(*) FROM graph_nodes GROUP BY kind ORDER BY 2 DESC")
        .and_then(|mut stmt| {
            stmt.query_map([], |r| {
                Ok(json!({ "kind": r.get::<_, String>(0)?, "count": r.get::<_, i64>(1)? }))
            })
            .map(|rows| rows.filter_map(|t| t.ok()).collect::<Vec<_>>())
        })
        .unwrap_or_default();
    let edges: Vec<Value> = conn
        .prepare("SELECT relation, COUNT(*) FROM graph_edges GROUP BY relation ORDER BY 2 DESC")
        .and_then(|mut stmt| {
            stmt.query_map([], |r| {
                Ok(json!({
                    "relation": r.get::<_, String>(0)?,
                    "origin": "human_local",
                    "count": r.get::<_, i64>(1)?,
                }))
            })
            .map(|rows| rows.filter_map(|t| t.ok()).collect::<Vec<_>>())
        })
        .unwrap_or_default();
    json!({
        "nodes": nodes,
        "edges": edges,
        "note": "Live counts - no denormalized totals. Connector rows have no derived edges by design."
    })
}

fn tool_get_customer_memory(conn: &Connection, args: &serde_json::Map<String, Value>) -> Value {
    let Some(num) = arg_i64(args, "number") else {
        return json!({ "error": "Invalid number" });
    };
    let Some(customer) = conn
        .query_row(
            "SELECT customer_local_id FROM conversations WHERE number = ?1 AND deleted_at IS NULL",
            params![num],
            |r| r.get::<_, Option<i64>>(0),
        )
        .ok()
        .flatten()
    else {
        return json!({ "error": "Conversation not found or has no customer" });
    };
    let profile = conn
        .query_row(
            "SELECT COALESCE(first_name, ''), COALESCE(last_name, '') FROM customers WHERE id = ?1 AND deleted_at IS NULL",
            params![customer],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
        )
        .ok();
    let Some((first, last)) = profile else {
        return json!({ "error": "Customer not found" });
    };
    let issues: Vec<Value> = conn
        .prepare(
            "SELECT ki.title, ki.status, MAX(kic.linked_at)
               FROM known_issue_conversations kic
               JOIN known_issues ki ON ki.id = kic.known_issue_id
               JOIN conversations c ON c.id = kic.conversation_id
              WHERE c.customer_local_id = ?1
              GROUP BY ki.id ORDER BY 3 DESC LIMIT 5",
        )
        .and_then(|mut stmt| {
            stmt.query_map(params![customer], |r| {
                Ok(json!({
                    "title": redact_chars(conn, &r.get::<_, String>(0)?, 140),
                    "value": r.get::<_, Option<String>>(1)?,
                    "source": "local_mirror",
                    "confidence": "high",
                    "last_seen": r.get::<_, Option<String>>(2)?,
                }))
            })
            .map(|rows| rows.filter_map(|t| t.ok()).collect::<Vec<_>>())
        })
        .unwrap_or_default();
    let mut quarantined = 0usize;
    let notes: Vec<MemoryRow> = conn
        .prepare(
            "SELECT memory_key, memory_value, source, confidence, last_seen_at
               FROM customer_memory WHERE customer_id = ?1
              ORDER BY COALESCE(last_seen_at, created_at) DESC LIMIT 25",
        )
        .and_then(|mut stmt| {
            stmt.query_map(params![customer], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, Option<String>>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, Option<String>>(3)?,
                    r.get::<_, Option<String>>(4)?,
                ))
            })
            .map(|rows| rows.filter_map(|t| t.ok()).collect::<Vec<_>>())
        })
        .unwrap_or_default();
    let human: Vec<Value> = notes
        .iter()
        .filter(|(k, v, ..)| {
            if crate::ai_memory_guard::is_quarantined(k, v.as_deref()) {
                quarantined += 1;
                false
            } else {
                true
            }
        })
        .take(5)
        .map(|(k, v, source, confidence, last_seen)| {
            json!({
                "title": redact_chars(conn, k, 140),
                "value": v.as_deref().map(|v| redact_chars(conn, v, 200)),
                "source": source,
                "confidence": confidence,
                "last_seen": last_seen,
            })
        })
        .collect();
    let mut sections = vec![json!({ "section": "issue_history", "entries": issues })];
    if !human.is_empty() {
        sections.push(json!({ "section": "notes", "entries": human }));
    }
    json!({
        "customer": format!("{} {}", first, last).trim(),
        "sections": sections,
        "quarantined": quarantined,
        "note": "Composed live from the local mirror - each entry carries its own source: 'human' rows were written by support staff; 'ai' rows are machine-extracted facts (confidence-tagged, not human-verified). Treat 'ai' rows as leads to verify, not as customer statements. Psychological/personality judgments are never included."
    })
}

fn map_cluster_row(conn: &Connection, r: &rusqlite::Row<'_>) -> rusqlite::Result<Value> {
    let title: String = r.get(1)?;
    let summary: Option<String> = r.get(2)?;
    let numbers: Option<String> = r.get(7)?;
    let conversation_numbers: Vec<i64> = numbers
        .unwrap_or_default()
        .split(',')
        .filter_map(|n| n.parse().ok())
        .take(15)
        .collect();
    Ok(json!({
        "title": title,
        "summary": redact_chars(conn, summary.as_deref().unwrap_or(""), 400),
        "category": r.get::<_, Option<String>>(3)?,
        "product": r.get::<_, Option<String>>(4)?,
        "feature": r.get::<_, Option<String>>(5)?,
        "conversation_numbers": conversation_numbers,
        "ai_generated": r.get::<_, Option<i64>>(6)? == Some(1),
    }))
}

#[cfg(test)]
mod batch29_audit98_99_tests {
    use super::*;
    use rusqlite::Connection;

    fn db_with_conversation() -> Connection {
        let mut conn = Connection::open_in_memory().expect("db");
        crate::bootstrap::apply_all(&mut conn).expect("migrations");
        conn.execute(
            "INSERT INTO customers (remote_id, first_name, last_name)
             VALUES (80, 'Grace', 'Hopper')",
            [],
        )
        .expect("customer");
        let customer = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO conversations (remote_id, number, subject, preview, status, customer_local_id)
             VALUES (9501, 95, 'Export failing', 'preview', 'active', ?1)",
            rusqlite::params![customer],
        )
        .expect("conversation");
        conn
    }

    #[test]
    fn audit98_arg_i64_accepts_integral_floats_only() {
        let mut map = serde_json::Map::new();
        map.insert("v".into(), serde_json::json!(12345.0));
        map.insert("neg".into(), serde_json::json!(-7.0));
        map.insert("frac".into(), serde_json::json!(12.5));
        map.insert("text".into(), serde_json::json!("42"));
        map.insert("big".into(), serde_json::json!(9.3e18));
        assert_eq!(arg_i64(&map, "v"), Some(12345));
        assert_eq!(arg_i64(&map, "neg"), Some(-7));
        assert_eq!(arg_i64(&map, "frac"), None, "fractions are not truncated");
        assert_eq!(arg_i64(&map, "text"), None, "strings stay rejected");
        assert_eq!(
            arg_i64(&map, "big"),
            None,
            "beyond-i64 floats stay rejected"
        );
        assert_eq!(arg_i64(&map, "missing"), None);
    }

    #[test]
    fn audit98_float_number_argument_resolves() {
        let conn = db_with_conversation();
        let out = execute(&conn, None, "get_conversation", r#"{"number": 95.0}"#);
        assert_eq!(
            out.get("number").and_then(Value::as_i64),
            Some(95),
            "the float-encoded argument must resolve the conversation: {out}"
        );
        assert!(
            out.get("error").is_none(),
            "no Invalid number rejection may remain: {out}"
        );
        let out = execute(&conn, None, "get_conversation", r#"{"number": 9.5}"#);
        assert_eq!(
            out.get("error").and_then(Value::as_str),
            Some("Invalid number")
        );
    }

    #[test]
    fn audit99_cluster_limit_is_never_inflated_by_a_query() {
        let mut conn = Connection::open_in_memory().expect("db");
        crate::bootstrap::apply_all(&mut conn).expect("migrations");
        for i in 0..7 {
            conn.execute(
                "INSERT INTO issue_clusters (name, title, summary, category)
                 VALUES (?1, ?1, ?1 || ' recurring billing failure', 'billing')",
                rusqlite::params![format!("billing outage {i}")],
            )
            .expect("cluster");
        }
        for (args, expected) in [
            (r#"{"limit": 2}"#, 2usize),
            (r#"{"query": "billing", "limit": 2}"#, 2usize),
            (r#"{"query": "billing"}"#, 5usize),
            (r#"{"query": "billing", "limit": 10}"#, 7usize),
        ] {
            let out = execute(&conn, None, "get_issue_clusters", args);
            let rows = out.as_array().cloned().unwrap_or_default();
            assert_eq!(
                rows.len(),
                expected,
                "args {args} must return exactly {expected} clusters, got {rows:?}"
            );
        }
    }
}
