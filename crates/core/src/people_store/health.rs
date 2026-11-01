use super::*;

const EVIDENCE_LIMIT: i64 = 10;

fn evidence_ids(
    conn: &Connection,
    conv_scope: &str,
    subject_id: i64,
    extra_sql: &str,
    extra_params: &[rusqlite::types::Value],
) -> Vec<i64> {
    let sql = format!(
        "SELECT c.id FROM conversations c
         WHERE {conv_scope} AND c.deleted_at IS NULL {extra_sql}
         ORDER BY c.created_at DESC LIMIT {EVIDENCE_LIMIT}"
    );
    let mut params: Vec<rusqlite::types::Value> = vec![rusqlite::types::Value::Integer(subject_id)];
    params.extend_from_slice(extra_params);
    conn.prepare(&sql)
        .and_then(|mut stmt| {
            stmt.query_map(rusqlite::params_from_iter(params.iter()), |r| {
                r.get::<_, i64>(0)
            })
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
        })
        .unwrap_or_default()
}

pub(super) fn health_metric(
    key: &str,
    label: &str,
    value: Value,
    display: String,
    definition: &str,
    evidence: Vec<i64>,
    completeness: &str,
) -> Value {
    json!({
        "key": key,
        "label": label,
        "value": value,
        "display": display,
        "definition": definition,
        "evidence_conversation_ids": evidence,
        "completeness": completeness,
    })
}

pub(super) fn health_flag(
    key: &str,
    label: &str,
    severity: &str,
    detail: String,
    evidence: Vec<i64>,
) -> Value {
    json!({
        "key": key,
        "label": label,
        "severity": severity,
        "detail": detail,
        "evidence_conversation_ids": evidence,
    })
}

pub(super) fn build_support_health_report(
    conn: &Connection,
    subject_kind: &str,
    subject_id: i64,
    subject_label: &str,
    conv_scope: &str,
    rating_scope: &str,
    incident_scope: &str,
) -> Result<Value> {
    let mut metrics: Vec<Value> = Vec::new();
    let mut flags: Vec<Value> = Vec::new();
    let ev = |extra_sql: &str, extra: &[rusqlite::types::Value]| -> Vec<i64> {
        evidence_ids(conn, conv_scope, subject_id, extra_sql, extra)
    };

    let open_convs: i64 = conn
        .query_row(
            &format!(
                "SELECT COUNT(*) FROM conversations c
                 WHERE {conv_scope} AND c.deleted_at IS NULL AND c.status = 'active'"
            ),
            params![subject_id],
            |r| r.get(0),
        )
        .unwrap_or(0);
    let total_convs: i64 = conn
        .query_row(
            &format!(
                "SELECT COUNT(*) FROM conversations c
                 WHERE {conv_scope} AND c.deleted_at IS NULL"
            ),
            params![subject_id],
            |r| r.get(0),
        )
        .unwrap_or(0);
    let volume_90: i64 = conn
        .query_row(
            &format!(
                "SELECT COUNT(*) FROM conversations c
                 WHERE {conv_scope} AND c.deleted_at IS NULL
                   AND julianday(c.created_at) >= julianday('now', '-90 days')"
            ),
            params![subject_id],
            |r| r.get(0),
        )
        .unwrap_or(0);

    metrics.push(health_metric(
        "open_conversations",
        "Open conversations",
        json!(open_convs),
        open_convs.to_string(),
        &format!("Conversations currently in status \"active\" for this {subject_kind}."),
        ev("AND c.status = 'active'", &[]),
        "known",
    ));
    metrics.push(health_metric(
        "support_volume_total",
        "Support volume (all time)",
        json!(total_convs),
        total_convs.to_string(),
        &format!("Total conversations ever synced for this {subject_kind}."),
        ev("", &[]),
        "known",
    ));
    metrics.push(health_metric(
        "support_volume_90d",
        "Support volume (90 days)",
        json!(volume_90),
        volume_90.to_string(),
        "Conversations created in the last 90 days.",
        ev(
            "AND julianday(c.created_at) >= julianday('now', '-90 days')",
            &[],
        ),
        "known",
    ));

    let (waiting_count, waiting_avg_days, waiting_max_days): (i64, Option<f64>, Option<f64>) = conn
        .query_row(
            &format!(
                "SELECT COUNT(*),
                        AVG(julianday('now') - julianday(c.customer_waiting_since)),
                        MAX(julianday('now') - julianday(c.customer_waiting_since))
                 FROM conversations c
                 WHERE {conv_scope} AND c.deleted_at IS NULL
                   AND c.status = 'active' AND c.customer_waiting_since IS NOT NULL"
            ),
            params![subject_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap_or((0, None, None));
    let waiting_evidence = ev(
        "AND c.status = 'active' AND c.customer_waiting_since IS NOT NULL",
        &[],
    );
    metrics.push(health_metric(
        "waiting_count",
        "Currently waiting",
        json!(waiting_count),
        waiting_count.to_string(),
        "Open conversations where the customer is waiting for a reply (local waiting marker).",
        waiting_evidence.clone(),
        if waiting_count > 0 {
            "known"
        } else {
            "unknown"
        },
    ));
    if waiting_count > 0 {
        let avg_days = waiting_avg_days.unwrap_or(0.0);
        metrics.push(health_metric(
            "waiting_avg_days",
            "Average wait",
            json!(avg_days),
            format!("{avg_days:.1} days"),
            "Average calendar days waiting across currently-open conversations (not business hours).",
            ev(
                "AND c.status = 'active' AND c.customer_waiting_since IS NOT NULL",
                &[],
            ),
            "known",
        ));
        let max_days = waiting_max_days.unwrap_or(0.0);
        if max_days >= 5.0 {
            flags.push(health_flag(
                "waiting_long",
                "Waiting a long time",
                "critical",
                format!("A conversation has been waiting {max_days:.1} calendar days for a reply."),
                waiting_evidence,
            ));
        } else if max_days >= 2.0 {
            flags.push(health_flag(
                "waiting_long",
                "Waiting",
                "warning",
                format!("A conversation has been waiting {max_days:.1} calendar days for a reply."),
                waiting_evidence,
            ));
        }
    }

    let (fr_avg_hours, fr_n): (Option<f64>, i64) = conn
        .query_row(
            &format!(
                "SELECT AVG((julianday(c.first_response_at) - julianday(c.first_customer_message_at)) * 24),
                        COUNT(*)
                 FROM conversations c
                 WHERE {conv_scope} AND c.deleted_at IS NULL
                   AND c.first_response_at IS NOT NULL AND c.first_customer_message_at IS NOT NULL
                   AND julianday(c.created_at) >= julianday('now', '-90 days')"
            ),
            params![subject_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap_or((None, 0));
    let fr_avg = fr_avg_hours.unwrap_or(0.0);
    metrics.push(health_metric(
        "first_response_avg_hours",
        "Avg first response (90d)",
        json!(fr_avg),
        if fr_n > 0 {
            format!("{fr_avg:.1} hours")
        } else {
            "unknown".to_string()
        },
        "Average calendar hours between the first customer message and the first agent reply (last 90 days). Business-hours SLA timing lives on each conversation.",
        ev(
            "AND c.first_response_at IS NOT NULL AND c.first_customer_message_at IS NOT NULL AND julianday(c.created_at) >= julianday('now', '-90 days')",
            &[],
        ),
        if fr_n > 0 { "known" } else { "unknown" },
    ));

    let bad_count: i64 = conn
        .query_row(
            &format!(
                "SELECT COUNT(*) FROM ratings r
                 WHERE r.rating = 'not-good' AND {rating_scope}
                   AND julianday(r.remote_created_at) >= julianday('now', '-90 days')"
            ),
            params![subject_id],
            |r| r.get(0),
        )
        .unwrap_or(0);
    let bad_evidence: Vec<i64> = conn
        .prepare(&format!(
            "SELECT r.conversation_id FROM ratings r
             WHERE r.rating = 'not-good' AND {rating_scope} AND r.conversation_id IS NOT NULL
               AND julianday(r.remote_created_at) >= julianday('now', '-90 days')
             LIMIT {EVIDENCE_LIMIT}"
        ))
        .and_then(|mut stmt| {
            stmt.query_map(params![subject_id], |r| r.get::<_, i64>(0))
                .map(|rows| rows.filter_map(|r| r.ok()).collect())
        })
        .unwrap_or_default();
    metrics.push(health_metric(
        "negative_outcomes_90d",
        "Negative ratings (90d)",
        json!(bad_count),
        bad_count.to_string(),
        "Ratings \"not-good\" received in the last 90 days (Help Scout mirror).",
        bad_evidence.clone(),
        "known",
    ));
    if bad_count >= 2 {
        flags.push(health_flag(
            "negative_outcomes",
            "Recent negative outcomes",
            "warning",
            format!("{bad_count} \"not-good\" ratings in the last 90 days."),
            bad_evidence,
        ));
    }

    let escalated_sql = "AND EXISTS (SELECT 1 FROM conversation_tags ct JOIN tags t ON t.id = ct.tag_id WHERE ct.conversation_id = c.id AND t.name = 'escalated')";
    let escalations: i64 = conn
        .query_row(
            &format!(
                "SELECT COUNT(*) FROM conversations c
                 WHERE {conv_scope} AND c.deleted_at IS NULL {escalated_sql}"
            ),
            params![subject_id],
            |r| r.get(0),
        )
        .unwrap_or(0);
    metrics.push(health_metric(
        "escalation_history",
        "Escalated conversations",
        json!(escalations),
        escalations.to_string(),
        "Conversations carrying the \"escalated\" tag (all time, Help Scout mirror).",
        ev(escalated_sql, &[]),
        "known",
    ));
    if escalations >= 2 {
        flags.push(health_flag(
            "escalation_history",
            "Escalation history",
            "info",
            format!("{escalations} conversations were escalated historically."),
            ev(escalated_sql, &[]),
        ));
    }

    let avg_msgs: Option<f64> = conn
        .query_row(
            &format!(
                "SELECT AVG(x.msgs) FROM (
                   SELECT c.id,
                          (SELECT COUNT(*) FROM conversation_threads t
                            WHERE t.conversation_id = c.id AND t.type = 'customer'
                              AND t.deleted_at IS NULL AND t.state = 'published') AS msgs
                   FROM conversations c
                   WHERE {conv_scope} AND c.deleted_at IS NULL
                     AND julianday(c.created_at) >= julianday('now', '-90 days')) x"
            ),
            params![subject_id],
            |r| r.get(0),
        )
        .unwrap_or(None);
    let avg_msgs = avg_msgs.unwrap_or(0.0);
    metrics.push(health_metric(
        "customer_effort_msgs",
        "Customer effort proxy (90d)",
        json!(avg_msgs),
        if avg_msgs > 0.0 {
            format!("{avg_msgs:.1} messages / conversation")
        } else {
            "unknown".to_string()
        },
        "Average number of customer messages per conversation in the last 90 days. A high value often means more back-and-forth to get resolved - an operational proxy, not a judgment about anyone.",
        ev("AND julianday(c.created_at) >= julianday('now', '-90 days')", &[]),
        if avg_msgs > 0.0 { "partial" } else { "unknown" },
    ));
    if avg_msgs >= 5.0 {
        flags.push(health_flag(
            "high_effort",
            "High customer effort",
            "info",
            format!(
                "An average of {avg_msgs:.1} customer messages per conversation in the last 90 days."
            ),
            ev("AND julianday(c.created_at) >= julianday('now', '-90 days')", &[]),
        ));
    }

    let repeated: Option<(String, i64)> = conn
        .query_row(
            &format!(
                "SELECT ic.name AS title, COUNT(*) AS n
                 FROM conversations c
                 JOIN issue_cluster_conversations icm ON icm.conversation_id = c.id
                 JOIN issue_clusters ic ON ic.id = icm.cluster_id
                 WHERE {conv_scope} AND c.deleted_at IS NULL
                 GROUP BY ic.id HAVING COUNT(*) >= 2 ORDER BY COUNT(*) DESC LIMIT 1"
            ),
            params![subject_id],
            |r| {
                Ok((
                    r.get::<_, Option<String>>(0)?.unwrap_or_default(),
                    r.get(1)?,
                ))
            },
        )
        .ok();
    let repeated_n = repeated.as_ref().map_or(0, |(_, n)| *n);
    let repeated_title = repeated.as_ref().map_or(String::new(), |(t, _)| t.clone());
    metrics.push(health_metric(
        "repeated_issues",
        "Repeated issue exposure",
        json!(repeated_n),
        if repeated_n > 0 {
            format!("{repeated_n} in \"{repeated_title}\"")
        } else {
            "none".to_string()
        },
        "Most-repeated issue cluster for this subject (conversations within one cluster). \"None\" is honest when no cluster repeats.",
        ev(
            "AND EXISTS (SELECT 1 FROM issue_cluster_conversations icm WHERE icm.conversation_id = c.id)",
            &[],
        ),
        "known",
    ));
    if repeated_n >= 3 {
        flags.push(health_flag(
            "repeated_issue",
            "Repeated issue",
            "warning",
            format!("{repeated_n} conversations in the \"{repeated_title}\" issue cluster."),
            ev(
                "AND EXISTS (SELECT 1 FROM issue_cluster_conversations icm JOIN issue_clusters ic ON ic.id = icm.cluster_id WHERE icm.conversation_id = c.id AND ic.name = ?)",
                &[rusqlite::types::Value::Text(repeated_title.clone())],
            ),
        ));
    }

    let unresolved_sql = "AND c.status = 'active' AND EXISTS (SELECT 1 FROM known_issue_links kil JOIN known_issues ki ON ki.id = kil.known_issue_id WHERE kil.conversation_id = c.id AND ki.status != 'resolved')";
    let unresolved_count: i64 = conn
        .query_row(
            &format!(
                "SELECT COUNT(*) FROM conversations c
                 WHERE {conv_scope} AND c.deleted_at IS NULL {unresolved_sql}"
            ),
            params![subject_id],
            |r| r.get(0),
        )
        .unwrap_or(0);
    metrics.push(health_metric(
        "unresolved_known_issues",
        "Open conversations on unresolved issues",
        json!(unresolved_count),
        unresolved_count.to_string(),
        "Open conversations linked to known issues that are not yet resolved.",
        ev(unresolved_sql, &[]),
        "known",
    ));
    if unresolved_count >= 1 {
        flags.push(health_flag(
            "unresolved_issues",
            "Unresolved issues",
            "warning",
            format!(
                "{unresolved_count} open conversation(s) tied to known issues that are not resolved."
            ),
            ev(unresolved_sql, &[]),
        ));
    }

    struct IncidentRow {
        incident_id: i64,
        code: Option<String>,
        title: Option<String>,
        severity: String,
        status: String,
        conversations: i64,
    }
    let incidents: Vec<IncidentRow> = conn
        .prepare(&format!(
            "SELECT DISTINCT i.id, i.code, i.title, i.severity, i.status, COUNT(DISTINCT c.id) AS conversations
             FROM incidents i
             JOIN incident_conversations ic ON ic.incident_id = i.id
             JOIN conversations c ON c.id = ic.conversation_id
             WHERE i.status != 'resolved' AND c.deleted_at IS NULL AND {incident_scope}
             GROUP BY i.id
             ORDER BY CASE i.severity WHEN 'sev1' THEN 1 WHEN 'sev2' THEN 2 WHEN 'sev3' THEN 3 ELSE 4 END"
        ))
        .and_then(|mut stmt| {
            stmt.query_map(params![subject_id], |r| {
                Ok(IncidentRow {
                    incident_id: r.get(0)?,
                    code: r.get(1)?,
                    title: r.get(2)?,
                    severity: r.get(3)?,
                    status: r.get(4)?,
                    conversations: r.get(5)?,
                })
            })
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
        })
        .unwrap_or_default();
    let incident_evidence = ev(
        "AND EXISTS (SELECT 1 FROM incident_conversations ic JOIN incidents i ON i.id = ic.incident_id WHERE ic.conversation_id = c.id AND i.status != 'resolved')",
        &[],
    );
    metrics.push(health_metric(
        "incident_exposure",
        "Current incident exposure",
        json!(incidents.len()),
        if incidents.is_empty() {
            "none".to_string()
        } else {
            format!("{} active incident(s)", incidents.len())
        },
        "Active (non-resolved) incidents this subject is exposed to via linked conversations.",
        incident_evidence,
        "known",
    ));
    let incident_exposure: Vec<Value> = incidents
        .iter()
        .map(|inc| {
            let severity = match inc.severity.as_str() {
                "sev1" => "critical",
                "sev2" => "warning",
                _ => "info",
            };
            flags.push(health_flag(
                &format!("incident_{}", inc.incident_id),
                &format!("Incident {}", inc.code.clone().unwrap_or_default()),
                severity,
                format!(
                    "{} linked conversation(s) in active incident \"{}\" (status {}).",
                    inc.conversations,
                    inc.title.clone().unwrap_or_default(),
                    inc.status
                ),
                ev(
                    "AND EXISTS (SELECT 1 FROM incident_conversations ic JOIN incidents i ON i.id = ic.incident_id WHERE ic.conversation_id = c.id AND i.status != 'resolved' AND i.id = ?)",
                    &[rusqlite::types::Value::Integer(inc.incident_id)],
                ),
            ));
            json!({
                "incident_id": inc.incident_id,
                "code": inc.code,
                "title": inc.title,
                "severity": inc.severity,
                "status": inc.status,
                "conversations": inc.conversations,
            })
        })
        .collect();

    Ok(json!({
        "subject_kind": subject_kind,
        "subject_id": subject_id,
        "subject_label": subject_label,
        "metrics": metrics,
        "flags": flags,
        "incident_exposure": incident_exposure,
        "generated_at": chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string(),
        "note": "Operational facts with evidence only. No psychological or personal judgments; no aggregate \"score\" by design (plan Phase 24).",
    }))
}
