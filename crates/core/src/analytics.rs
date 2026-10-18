//! Analytics view data (promoted from the retired axum
//! `http/routes/analytics.rs` at the Phase 4 cutover).
//!
//! Pure `&Connection` computations the native shell renders directly,
//! without an HTTP round-trip: why-contacting categories, top questions,
//! and the issue-radar alert engine (cluster trends, high-volume
//! questions, rating correlations, and the derived extensions —
//! reappearing issues, customer concentration, spiking clusters).

use serde_json::{Value, json};

pub fn why_contacting_impl(conn: &rusqlite::Connection, days: i64) -> Vec<Value> {
    let Ok(mut stmt) = conn.prepare(
        "SELECT json_extract(a.response_json, '$.issue_cluster_candidate') AS category,
                a.conversation_id
           FROM ai_runs a
          WHERE a.type = 'ticket_analysis' AND a.status = 'completed'
            AND a.conversation_id IS NOT NULL
            AND a.id IN (SELECT MAX(id) FROM ai_runs
                          WHERE type = 'ticket_analysis' AND status = 'completed'
                         GROUP BY conversation_id)
            AND json_extract(a.response_json, '$.issue_cluster_candidate') IS NOT NULL
            AND a.created_at >= datetime('now', '-' || ?1 || ' days')",
    ) else {
        return Vec::new();
    };
    let rows: Vec<(String, i64)> = stmt
        .query_map([days], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
        })
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_default();
    let mut by_category: std::collections::HashMap<String, Vec<i64>> =
        std::collections::HashMap::new();
    for (category, conversation_id) in rows {
        let cat = category.trim().to_lowercase();
        by_category.entry(cat).or_default().push(conversation_id);
    }
    let mut out: Vec<Value> = by_category
        .into_iter()
        .map(|(category, ids)| {
            json!({ "category": category, "count": ids.len(), "conversation_ids": ids })
        })
        .collect();
    out.sort_by(|a, b| {
        b["count"]
            .as_i64()
            .unwrap_or(0)
            .cmp(&a["count"].as_i64().unwrap_or(0))
    });
    out
}

/// HTTP round-trip (pure `&Connection` computation, no axum types).
pub fn top_questions_impl(conn: &rusqlite::Connection, days: i64) -> Vec<Value> {
    let mut out: Vec<Value> = grouped_questions(conn, days)
        .into_iter()
        .map(|(question, ids)| {
            json!({ "question": question, "count": ids.len(), "conversation_ids": ids })
        })
        .collect();
    out.truncate(20);
    out
}

fn grouped_questions(conn: &rusqlite::Connection, days: i64) -> Vec<(String, Vec<i64>)> {
    let Ok(mut stmt) = conn.prepare(
        "SELECT LOWER(TRIM(json_extract(a.response_json, '$.primary_question'))) AS question,
                a.conversation_id
           FROM ai_runs a
          WHERE a.type = 'ticket_analysis' AND a.status = 'completed'
            AND a.conversation_id IS NOT NULL
            AND a.id IN (SELECT MAX(id) FROM ai_runs
                          WHERE type = 'ticket_analysis' AND status = 'completed'
                         GROUP BY conversation_id)
            AND json_extract(a.response_json, '$.primary_question') IS NOT NULL
            AND a.created_at >= datetime('now', '-' || ?1 || ' days')",
    ) else {
        return Vec::new();
    };
    let rows: Vec<(String, i64)> = stmt
        .query_map([days], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
        })
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_default();
    let mut by_question: std::collections::HashMap<String, Vec<i64>> =
        std::collections::HashMap::new();
    for (question, conversation_id) in rows {
        by_question
            .entry(question)
            .or_default()
            .push(conversation_id);
    }
    let mut out: Vec<(String, Vec<i64>)> = by_question.into_iter().collect();
    out.sort_by_key(|(_, ids)| std::cmp::Reverse(ids.len()));
    out
}

struct RadarCluster {
    id: i64,
    title: String,
    conversation_ids: Vec<i64>,
    customer_count: usize,
    summary: String,
    trend: &'static str,
}

/// Per-customer repeat facts for the repeated-unresolved extension:
/// `(contact count, first julian day, last julian day, conversation ids)`.
type CustomerRepeat = (usize, f64, f64, Vec<i64>);

struct ConvFacts {
    id: i64,
    customer_id: Option<i64>,
    mailbox_id: Option<i64>,
    created_at: Option<String>,
    jd: Option<f64>,
}

fn radar_alert(
    kind: &str,
    title: String,
    detail: String,
    conversation_ids: Vec<i64>,
    cluster_id: Option<i64>,
    severity: &str,
) -> Value {
    let capped: Vec<i64> = conversation_ids.into_iter().take(10).collect();
    json!({
        "kind": kind,
        "title": title,
        "detail": detail,
        "conversation_ids": capped,
        "cluster_id": cluster_id,
        "severity": severity,
    })
}

/// The issue-radar alert engine: cluster trends, high-volume questions,
/// rating correlations, and the derived extensions.
pub fn issue_radar_impl(conn: &rusqlite::Connection) -> Vec<Value> {
    let mut alerts: Vec<Value> = Vec::new();
    let clusters = radar_clusters(conn);

    for c in &clusters {
        if c.trend == "new" && c.conversation_ids.len() >= 2 {
            alerts.push(radar_alert(
                "new_cluster",
                format!("New issue cluster: {}", c.title),
                format!(
                    "{} conversations from {} customers first seen recently. {}",
                    c.conversation_ids.len(),
                    c.customer_count,
                    c.summary
                ),
                c.conversation_ids.clone(),
                Some(c.id),
                if c.conversation_ids.len() >= 5 {
                    "critical"
                } else {
                    "warning"
                },
            ));
        }
        if c.trend == "rising" && c.conversation_ids.len() >= 3 {
            alerts.push(radar_alert(
                "volume_spike",
                format!("Rising volume: {}", c.title),
                format!(
                    "Conversation volume in this cluster increased vs the previous 14 days. {}",
                    c.summary
                ),
                c.conversation_ids.clone(),
                Some(c.id),
                "warning",
            ));
        }
        if c.trend == "stable" && c.conversation_ids.len() >= 5 {
            alerts.push(radar_alert(
                "recurring_issue",
                format!("Recurring issue: {}", c.title),
                format!(
                    "{} conversations over time. Consider a knowledge article or known issue entry. {}",
                    c.conversation_ids.len(),
                    c.summary
                ),
                c.conversation_ids.clone(),
                Some(c.id),
                "info",
            ));
        }
    }

    for q in top_questions_impl(conn, 30).into_iter().take(3) {
        let count = q["count"].as_i64().unwrap_or(0);
        if count >= 3 {
            alerts.push(radar_alert(
                "high_volume_question",
                format!("High-volume question ({count} tickets)"),
                q["question"].as_str().unwrap_or_default().to_string(),
                q["conversation_ids"]
                    .as_array()
                    .map(|a| a.iter().filter_map(|v| v.as_i64()).collect::<Vec<i64>>())
                    .unwrap_or_default(),
                None,
                "info",
            ));
        }
    }

    let escalated: Vec<i64> = conn
        .prepare(
            "SELECT c.id FROM conversations c
               JOIN conversation_tags ct ON ct.conversation_id = c.id
               JOIN tags t ON t.id = ct.tag_id
              WHERE t.name = 'escalated'
                AND julianday(c.created_at) >= julianday('now', '-30 days')
              LIMIT 1000",
        )
        .map(|mut stmt| {
            stmt.query_map([], |r| r.get::<_, i64>(0))
                .map(|rows| rows.filter_map(|r| r.ok()).collect())
                .unwrap_or_default()
        })
        .unwrap_or_default();
    if escalated.len() >= 2 {
        alerts.push(radar_alert(
            "escalation_heavy",
            format!(
                "{} escalated tickets in the last 30 days",
                escalated.len()
            ),
            "Multiple tickets required escalation. Review the underlying causes - this is a correlation, not a causal claim.".to_string(),
            escalated,
            None,
            "warning",
        ));
    }

    let bad_ids: std::collections::HashSet<i64> = conn
        .prepare(
            "SELECT r.conversation_id FROM ratings r
              WHERE r.rating = 'not-good' AND r.conversation_id IS NOT NULL
              ORDER BY r.id DESC LIMIT 2000",
        )
        .map(|mut stmt| {
            stmt.query_map([], |r| r.get::<_, i64>(0))
                .map(|rows| rows.filter_map(|r| r.ok()).collect())
                .unwrap_or_default()
        })
        .unwrap_or_default();
    if !bad_ids.is_empty() {
        for c in &clusters {
            let overlap: Vec<i64> = c
                .conversation_ids
                .iter()
                .copied()
                .filter(|id| bad_ids.contains(id))
                .collect();
            if overlap.len() >= 2 {
                alerts.push(radar_alert(
                    "rating_correlated",
                    format!("Cluster \"{}\" is associated with poor ratings", c.title),
                    format!(
                        "{} conversations in this cluster received \"not-good\" ratings. This is an association, not proof of causation.",
                        overlap.len()
                    ),
                    overlap,
                    Some(c.id),
                    "warning",
                ));
            }
        }
    }

    alerts.extend(radar_extensions(conn, &clusters));
    alerts
}

fn radar_clusters(conn: &rusqlite::Connection) -> Vec<RadarCluster> {
    let cluster_rows: Vec<(i64, String)> = conn
        .prepare("SELECT id, name FROM issue_clusters ORDER BY conversation_count DESC")
        .map(|mut stmt| {
            stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))
                .map(|rows| rows.filter_map(|r| r.ok()).collect())
                .unwrap_or_default()
        })
        .unwrap_or_default();
    if cluster_rows.is_empty() {
        return Vec::new();
    }

    let members: Vec<(i64, String, Vec<i64>)> = cluster_rows
        .into_iter()
        .map(|(id, title)| {
            let member_ids: Vec<i64> = conn
                .prepare(
                    "SELECT conversation_id FROM issue_cluster_conversations WHERE cluster_id = ?1",
                )
                .and_then(|mut stmt| {
                    Ok(stmt
                        .query_map(rusqlite::params![id], |r| r.get::<_, i64>(0))?
                        .filter_map(|r| r.ok())
                        .collect())
                })
                .unwrap_or_default();
            (id, title, member_ids)
        })
        .collect();
    let all_conv_ids: Vec<i64> = members
        .iter()
        .flat_map(|(_, _, ids)| ids.iter().copied())
        .collect::<std::collections::HashSet<i64>>()
        .into_iter()
        .collect();
    if all_conv_ids.is_empty() {
        return Vec::new();
    }

    let placeholders = all_conv_ids
        .iter()
        .map(|_| "?")
        .collect::<Vec<_>>()
        .join(",");
    let sql = format!(
        "SELECT c.id, c.customer_local_id, c.mailbox_local_id, c.created_at, julianday(c.created_at) AS jd
           FROM conversations c WHERE c.id IN ({placeholders})"
    );
    let facts: Vec<ConvFacts> = conn
        .prepare(&sql)
        .and_then(|mut stmt| {
            let params: Vec<&dyn rusqlite::ToSql> = all_conv_ids
                .iter()
                .map(|i| i as &dyn rusqlite::ToSql)
                .collect();
            Ok(stmt
                .query_map(params.as_slice(), |r| {
                    Ok(ConvFacts {
                        id: r.get(0)?,
                        customer_id: r.get(1)?,
                        mailbox_id: r.get(2)?,
                        created_at: r.get(3)?,
                        jd: r.get(4)?,
                    })
                })?
                .filter_map(|r| r.ok())
                .collect())
        })
        .unwrap_or_default();
    let now_jd: f64 = conn
        .query_row("SELECT julianday('now')", [], |r| r.get(0))
        .unwrap_or(0.0);

    let mut clusters = Vec::new();
    for (id, title, member_ids) in members {
        let convs: Vec<&ConvFacts> = facts
            .iter()
            .filter(|f| member_ids.contains(&f.id))
            .collect();
        if convs.is_empty() {
            continue;
        }

        let recent = convs
            .iter()
            .filter(|f| f.jd.is_some_and(|d| now_jd - d <= 14.0))
            .count();
        let previous = convs
            .iter()
            .filter(|f| {
                f.jd.is_some_and(|d| now_jd - d > 14.0 && now_jd - d <= 28.0)
            })
            .count();
        let trend = if (recent as f64) > previous as f64 * 1.3 && recent >= 3 {
            "rising"
        } else if previous > 0 && (recent as f64) < previous as f64 * 0.7 {
            "falling"
        } else if previous == 0 && recent > 0 {
            "new"
        } else {
            "stable"
        };

        let customer_count = convs
            .iter()
            .filter_map(|f| f.customer_id)
            .collect::<std::collections::HashSet<i64>>()
            .len();
        let summary = format!(
            "{} conversations from {} customers.",
            convs.len(),
            customer_count
        );
        clusters.push(RadarCluster {
            id,
            title,
            conversation_ids: convs.iter().map(|f| f.id).collect(),
            customer_count,
            summary,
            trend,
        });
    }
    clusters
}

fn radar_extensions(conn: &rusqlite::Connection, clusters: &[RadarCluster]) -> Vec<Value> {
    let mut alerts: Vec<Value> = Vec::new();
    if clusters.is_empty() {
        return alerts;
    }
    let ids: Vec<i64> = clusters
        .iter()
        .flat_map(|c| c.conversation_ids.clone())
        .collect();
    if ids.is_empty() {
        return alerts;
    }
    let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let sql = format!(
        "SELECT c.id, c.customer_local_id, c.mailbox_local_id, c.created_at, julianday(c.created_at) AS jd
           FROM conversations c WHERE c.id IN ({placeholders})"
    );
    let facts: Vec<ConvFacts> = conn
        .prepare(&sql)
        .and_then(|mut stmt| {
            let params: Vec<&dyn rusqlite::ToSql> =
                ids.iter().map(|i| i as &dyn rusqlite::ToSql).collect();
            Ok(stmt
                .query_map(params.as_slice(), |r| {
                    Ok(ConvFacts {
                        id: r.get(0)?,
                        customer_id: r.get(1)?,
                        mailbox_id: r.get(2)?,
                        created_at: r.get(3)?,
                        jd: r.get(4)?,
                    })
                })?
                .filter_map(|r| r.ok())
                .collect())
        })
        .unwrap_or_default();
    let now_jd: f64 = conn
        .query_row("SELECT julianday('now')", [], |r| r.get(0))
        .unwrap_or(0.0);

    for c in clusters {
        let convs: Vec<&ConvFacts> = facts
            .iter()
            .filter(|f| c.conversation_ids.contains(&f.id))
            .collect();
        if convs.len() < 2 {
            continue;
        }
        let conv_ids: Vec<i64> = convs.iter().map(|f| f.id).collect();

        let recent_count = convs
            .iter()
            .filter(|f| f.jd.is_some_and(|d| now_jd - d <= 30.0))
            .count();
        let older_count = convs
            .iter()
            .filter(|f| {
                f.jd.is_some_and(|d| now_jd - d > 30.0 && now_jd - d <= 60.0)
            })
            .count();
        if recent_count >= 2 && older_count >= 2 {
            alerts.push(radar_alert(
                "reappearing_issue",
                format!("Reappearing issue: {}", c.title),
                format!(
                    "{} conversations 30-60 days ago and {} in the last 30 days - the topic went quiet and came back. {}",
                    older_count, recent_count, c.summary
                ),
                conv_ids.clone(),
                Some(c.id),
                "warning",
            ));
        }

        let customers: std::collections::HashSet<i64> =
            convs.iter().filter_map(|f| f.customer_id).collect();
        if convs.len() >= 4 && !customers.is_empty() && customers.len() <= convs.len() / 2 {
            alerts.push(radar_alert(
                "customer_concentration",
                format!("Customer concentration: {}", c.title),
                format!(
                    "{} conversations from only {} distinct customers - a small group hitting the same problem repeatedly (an association, not a causal claim). {}",
                    convs.len(),
                    customers.len(),
                    c.summary
                ),
                conv_ids.clone(),
                Some(c.id),
                "info",
            ));
        }

        let mut mailbox_counts: std::collections::HashMap<i64, usize> =
            std::collections::HashMap::new();
        for f in &convs {
            if let Some(m) = f.mailbox_id {
                *mailbox_counts.entry(m).or_insert(0) += 1;
            }
        }
        let dominant = mailbox_counts
            .iter()
            .max_by_key(|(_, n)| **n)
            .map(|(m, n)| (*m, *n));
        if let Some((mailbox_id, n)) = dominant
            && convs.len() >= 4
            && n as f64 / convs.len() as f64 >= 0.7
        {
            let mailbox_name: String = conn
                .query_row(
                    "SELECT name FROM mailboxes WHERE id = ?1",
                    rusqlite::params![mailbox_id],
                    |r| r.get(0),
                )
                .unwrap_or_else(|_| format!("mailbox #{mailbox_id}"));
            alerts.push(radar_alert(
                "inbox_concentration",
                format!("Inbox concentration: {}", c.title),
                format!(
                    "{} of {} conversations arrived in \"{}\" - the issue is concentrated in one channel. {}",
                    n,
                    convs.len(),
                    mailbox_name,
                    c.summary
                ),
                conv_ids.clone(),
                Some(c.id),
                "info",
            ));
        }

        let mut jds: Vec<f64> = convs.iter().filter_map(|f| f.jd).collect();
        jds.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let date_by_jd: Vec<(f64, &str)> = convs
            .iter()
            .filter_map(|f| f.jd.map(|d| (d, f.created_at.as_deref().unwrap_or(""))))
            .collect();
        if jds.len() >= 4 {
            let (mut best_start, mut best_count) = (None, 0usize);
            for &start in &jds {
                let in_window = jds
                    .iter()
                    .filter(|&&d| d >= start && d < start + 7.0)
                    .count();
                if in_window > best_count {
                    best_count = in_window;
                    best_start = Some(start);
                }
            }
            if let Some(best) = best_start
                && best_count as f64 / jds.len() as f64 >= 0.6
            {
                let start_date = date_by_jd
                    .iter()
                    .find(|(d, _)| *d == best)
                    .map(|(_, s)| *s)
                    .unwrap_or("");
                let releases: Vec<String> = conn
                    .prepare(
                        "SELECT version_label FROM incident_releases
                          WHERE released_at IS NOT NULL
                            AND julianday(released_at) >= ?1
                            AND julianday(released_at) < ?2
                          LIMIT 3",
                    )
                    .and_then(|mut stmt| {
                        Ok(stmt
                            .query_map(rusqlite::params![best, best + 7.0], |r| {
                                r.get::<_, String>(0)
                            })?
                            .filter_map(|r| r.ok())
                            .collect())
                    })
                    .unwrap_or_default();
                let release_note = if releases.is_empty() {
                    String::new()
                } else {
                    format!(
                        " Releases recorded in this window: {} (association only).",
                        releases.join(", ")
                    )
                };
                alerts.push(radar_alert(
                    "release_correlation",
                    format!("Temporal burst: {}", c.title),
                    format!(
                        "{} of {} conversations started within one 7-day window starting {}. Temporal clustering suggests something changed - possibly a release - but this is an association, not a causal claim.{} {}",
                        best_count,
                        jds.len(),
                        truncate_chars(start_date, 10),
                        release_note,
                        c.summary
                    ),
                    conv_ids.clone(),
                    Some(c.id),
                    "warning",
                ));
            }
        }

        let mut by_customer: std::collections::HashMap<i64, CustomerRepeat> =
            std::collections::HashMap::new();
        for f in &convs {
            let (cid, jd) = match (f.customer_id, f.jd) {
                (Some(cid), Some(jd)) => (cid, jd),
                _ => continue,
            };
            let entry = by_customer.entry(cid).or_insert((0, jd, jd, Vec::new()));
            entry.0 += 1;
            entry.1 = entry.1.min(jd);
            entry.2 = entry.2.max(jd);
            entry.3.push(f.id);
        }
        let repeated: Vec<&CustomerRepeat> = by_customer
            .values()
            .filter(|e| e.0 >= 2 && e.2 - e.1 >= 14.0)
            .collect();
        if !repeated.is_empty() {
            alerts.push(radar_alert(
                "repeated_unresolved",
                format!("Repeated unresolved pattern: {}", c.title),
                format!(
                    "{} customer(s) wrote in {} times about this cluster across 14+ day spans - the underlying problem may not be resolved. {}",
                    repeated.len(),
                    repeated.iter().map(|e| e.0.to_string()).collect::<Vec<_>>().join(", "),
                    c.summary
                ),
                repeated.iter().flat_map(|e| e.3.clone()).collect(),
                Some(c.id),
                "warning",
            ));
        }
    }

    let recent7: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM conversations
              WHERE julianday(created_at) >= julianday('now', '-7 days')",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);
    let prev7: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM conversations
              WHERE julianday(created_at) >= julianday('now', '-14 days')
                AND julianday(created_at) < julianday('now', '-7 days')",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);
    if recent7 >= 10 && prev7 > 0 && recent7 as f64 / prev7 as f64 >= 1.4 {
        let sample: Vec<i64> = conn
            .prepare(
                "SELECT id FROM conversations
                  WHERE julianday(created_at) >= julianday('now', '-7 days')
                  ORDER BY created_at DESC LIMIT 10",
            )
            .map(|mut stmt| {
                stmt.query_map([], |r| r.get::<_, i64>(0))
                    .map(|rows| rows.filter_map(|r| r.ok()).collect())
                    .unwrap_or_default()
            })
            .unwrap_or_default();
        alerts.push(radar_alert(
            "volume_spike",
            format!("Unusual support volume: {recent7} conversations in the last 7 days"),
            format!(
                "{} conversations in the last 7 days vs {} in the previous 7 (a {}% increase). An association with a change somewhere - not a causal claim.",
                recent7,
                prev7,
                ((recent7 as f64 / prev7 as f64 - 1.0) * 100.0).round() as i64
            ),
            sample,
            None,
            if recent7 as f64 / prev7 as f64 >= 2.0 {
                "critical"
            } else {
                "warning"
            },
        ));
    }

    alerts
}

fn truncate_chars(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}
