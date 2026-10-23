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

    // Audit item 111: the LIMIT 1000 silently truncated the sample the
    // alert's claim is built on — 2,500 escalated tickets reported as
    // "1000 escalated tickets", and older not-good ratings dropped out
    // of the rating overlap. Both caps are gone so the counts match the
    // claims; the tables are local and the queries are bounded by the
    // 30-day window.
    let escalated: Vec<i64> = conn
        .prepare(
            "SELECT c.id FROM conversations c
               JOIN conversation_tags ct ON ct.conversation_id = c.id
               JOIN tags t ON t.id = ct.tag_id
              WHERE t.name = 'escalated'
                AND julianday(c.created_at) >= julianday('now', '-30 days')",
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

    // Audit item 111: no silent LIMIT 2000 — the rating_correlated
    // overlap must see every not-good rating, not just the newest 2000.
    let bad_ids: std::collections::HashSet<i64> = conn
        .prepare(
            "SELECT r.conversation_id FROM ratings r
              WHERE r.rating = 'not-good' AND r.conversation_id IS NOT NULL",
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
        // Clean-audit item 54: "new" must be checked before "rising". The
        // pre-fix precedence tested the rising band first, and a brand-new
        // cluster (previous = 0, recent >= 3) always satisfied
        // `recent > 0 * 1.3` — so every genuinely new cluster with three
        // or more conversations was reported as "rising" and "new" was
        // unreachable exactly where it matters most.
        let trend = if previous == 0 && recent > 0 {
            "new"
        } else if (recent as f64) > previous as f64 * 1.3 && recent >= 3 {
            "rising"
        } else if previous > 0 && (recent as f64) < previous as f64 * 0.7 {
            "falling"
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
        // Audit item 112: conversations whose grouping key is NULL can
        // never concentrate, but they were counted in the denominator —
        // inflating the customer ratio (unattributed imports made the
        // alert fire on imaginary concentration) and diluting the inbox
        // ratio (silently suppressing real single-inbox concentration).
        // Both checks now run over the attributed subset only, and the
        // claim texts report the attributed totals.
        let attributed: Vec<&ConvFacts> = convs
            .iter()
            .copied()
            .filter(|f| f.customer_id.is_some())
            .collect();
        let attributed_total = attributed.len();
        if attributed_total >= 4 && !customers.is_empty() && customers.len() <= attributed_total / 2
        {
            alerts.push(radar_alert(
                "customer_concentration",
                format!("Customer concentration: {}", c.title),
                format!(
                    "{} conversations from only {} distinct customers - a small group hitting the same problem repeatedly (an association, not a causal claim). {}",
                    attributed_total,
                    customers.len(),
                    c.summary
                ),
                conv_ids.clone(),
                Some(c.id),
                "info",
            ));
        }

        let with_mailbox: Vec<&ConvFacts> = convs
            .iter()
            .copied()
            .filter(|f| f.mailbox_id.is_some())
            .collect();
        let mailbox_total = with_mailbox.len();
        let mut mailbox_counts: std::collections::HashMap<i64, usize> =
            std::collections::HashMap::new();
        for f in &with_mailbox {
            if let Some(m) = f.mailbox_id {
                *mailbox_counts.entry(m).or_insert(0) += 1;
            }
        }
        let dominant = mailbox_counts
            .iter()
            .max_by_key(|(_, n)| **n)
            .map(|(m, n)| (*m, *n));
        if let Some((mailbox_id, n)) = dominant
            && mailbox_total >= 4
            && n as f64 / mailbox_total as f64 >= 0.7
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
                    mailbox_total,
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

#[cfg(test)]
mod batch24_audit54_tests {
    use super::*;
    use rusqlite::Connection;
    use rusqlite::params;

    /// Four clusters covering every trend band:
    /// - "brand-new": 4 conversations in the last 14 days, none in the
    ///   14-28 day window — the shape the pre-fix precedence reported as
    ///   "rising" (recent > 0 * 1.3 always holds), leaving "new"
    ///   unreachable for any cluster with recent >= 3.
    /// - "growing": 5 previous, 8 recent — the rising band.
    /// - "shrinking": 10 previous, 3 recent — the falling band.
    /// - "flat": 5 previous, 5 recent — stable.
    fn db_with_trend_clusters() -> Connection {
        let mut conn = Connection::open_in_memory().expect("db");
        crate::bootstrap::apply_all(&mut conn).expect("migrations");
        let cases: &[(&str, usize, usize)] = &[
            ("brand-new", 0, 4),
            ("growing", 5, 8),
            ("shrinking", 10, 3),
            ("flat", 5, 5),
        ];
        // Conversation numbers are globally unique — one running counter
        // across the whole fixture.
        let mut number = 9000_i64;
        for (name, previous, recent) in cases {
            conn.execute(
                "INSERT INTO issue_clusters (name, conversation_count) VALUES (?1, ?2)",
                params![name, *previous as i64 + *recent as i64],
            )
            .expect("cluster");
            let cluster = conn.last_insert_rowid();
            for _ in 0..*previous {
                number += 1;
                conn.execute(
                    "INSERT INTO conversations (remote_id, number, subject, created_at)
                     VALUES (?1, ?1, 'old', datetime('now', ?2))",
                    params![number, "-20 days"],
                )
                .expect("old conversation");
                conn.execute(
                    "INSERT INTO issue_cluster_conversations (cluster_id, conversation_id)
                     VALUES (?1, ?2)",
                    params![cluster, conn.last_insert_rowid()],
                )
                .expect("member");
            }
            for _ in 0..*recent {
                number += 1;
                conn.execute(
                    "INSERT INTO conversations (remote_id, number, subject, created_at)
                     VALUES (?1, ?1, 'new', datetime('now', ?2))",
                    params![number, "-2 days"],
                )
                .expect("recent conversation");
                conn.execute(
                    "INSERT INTO issue_cluster_conversations (cluster_id, conversation_id)
                     VALUES (?1, ?2)",
                    params![cluster, conn.last_insert_rowid()],
                )
                .expect("member");
            }
        }
        conn
    }

    fn trend_of(conn: &Connection, name: &str) -> &'static str {
        let clusters = radar_clusters(conn);
        clusters
            .iter()
            .find(|c| c.title == name)
            .unwrap_or_else(|| panic!("cluster {name} missing"))
            .trend
    }

    /// Clean-audit item 54: a cluster whose conversations all fall inside
    /// the recent window (previous = 0, recent >= 3) is "new" — not
    /// "rising", which the pre-fix precedence reported because the rising
    /// band was tested first and `recent > 0 * 1.3` always holds.
    #[test]
    fn audit54_new_cluster_with_three_recent_is_new() {
        let conn = db_with_trend_clusters();
        assert_eq!(trend_of(&conn, "brand-new"), "new");
    }

    /// Clean-audit item 54: the other bands keep their meanings —
    /// growth on an established cluster is still "rising", decline
    /// still "falling", and no significant change still "stable".
    #[test]
    fn audit54_established_clusters_keep_their_bands() {
        let conn = db_with_trend_clusters();
        assert_eq!(trend_of(&conn, "growing"), "rising");
        assert_eq!(trend_of(&conn, "shrinking"), "falling");
        assert_eq!(trend_of(&conn, "flat"), "stable");
    }

    /// Clean-audit item 54: a small new cluster (previous = 0,
    /// recent < 3) was "new" even pre-fix — pinned so the reorder does
    /// not disturb it.
    #[test]
    fn audit54_small_new_cluster_stays_new() {
        let mut conn = Connection::open_in_memory().expect("db");
        crate::bootstrap::apply_all(&mut conn).expect("migrations");
        conn.execute(
            "INSERT INTO issue_clusters (name, conversation_count) VALUES ('small-new', 2)",
            [],
        )
        .expect("cluster");
        let cluster = conn.last_insert_rowid();
        for i in 0..2 {
            conn.execute(
                "INSERT INTO conversations (remote_id, number, subject, created_at)
                 VALUES (?1, ?1, 's', datetime('now', '-2 days'))",
                params![31_000_i64 + i],
            )
            .expect("conversation");
            conn.execute(
                "INSERT INTO issue_cluster_conversations (cluster_id, conversation_id)
                 VALUES (?1, ?2)",
                params![cluster, conn.last_insert_rowid()],
            )
            .expect("member");
        }
        assert_eq!(trend_of(&conn, "small-new"), "new");
    }
}

#[cfg(test)]
mod batch30_tests {
    use super::*;
    use rusqlite::{Connection, params};

    fn db() -> Connection {
        let mut conn = Connection::open_in_memory().expect("db");
        crate::bootstrap::apply_all(&mut conn).expect("migrations");
        conn
    }

    fn seed_cluster_conv(
        conn: &Connection,
        cluster: i64,
        number: i64,
        customer_local_id: Option<i64>,
        mailbox_local_id: Option<i64>,
        created_offset: &str,
    ) {
        conn.execute(
            "INSERT INTO conversations (remote_id, number, subject, customer_local_id, mailbox_local_id, created_at)
             VALUES (?1, ?1, 'concentration seed', ?2, ?3, datetime('now', ?4))",
            params![number, customer_local_id, mailbox_local_id, created_offset],
        )
        .expect("conversation");
        conn.execute(
            "INSERT INTO issue_cluster_conversations (cluster_id, conversation_id) VALUES (?1, ?2)",
            params![cluster, conn.last_insert_rowid()],
        )
        .expect("member");
    }

    /// Audit item 112: NULL-mailbox conversations diluted the inbox
    /// concentration denominator — 4 of 7 (0.57) suppressed an alert that
    /// is really 4 of 4 attributed conversations in one inbox (1.0).
    #[test]
    fn inbox_concentration_ignores_unattributed_conversations() {
        let conn = db();
        // Real parent rows: customer_local_id / mailbox_local_id are FKs.
        conn.execute(
            "INSERT INTO mailboxes (remote_id, name) VALUES (1, 'Support')",
            [],
        )
        .expect("mailbox");
        let mailbox: i64 = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO customers (remote_id, first_name) VALUES (101, 'Casey')",
            [],
        )
        .expect("customer");
        let customer: i64 = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO issue_clusters (name, conversation_count) VALUES ('inbox-spike', 7)",
            [],
        )
        .expect("cluster");
        let cluster = conn.last_insert_rowid();
        for number in 9001..=9004 {
            seed_cluster_conv(
                &conn,
                cluster,
                number,
                Some(customer),
                Some(mailbox),
                "-2 days",
            );
        }
        for number in 9005..=9007 {
            seed_cluster_conv(&conn, cluster, number, None, None, "-2 days");
        }

        let clusters = radar_clusters(&conn);
        let alerts = radar_extensions(&conn, &clusters);
        let inbox = alerts
            .iter()
            .find(|a| a["kind"] == "inbox_concentration")
            .unwrap_or_else(|| panic!("inbox_concentration alert missing: {alerts:?}"));
        assert!(
            inbox["detail"]
                .as_str()
                .unwrap()
                .starts_with("4 of 4 conversations"),
            "denominator must be the attributed subset: {inbox}"
        );
    }

    /// Audit item 112, the other direction: unattributed conversations
    /// inflated the customer concentration denominator — 2 attributed
    /// conversations (2 customers) plus 4 unattributed fired a false
    /// "6 conversations from 2 customers" alert pre-fix.
    #[test]
    fn customer_concentration_ignores_unattributed_conversations() {
        let conn = db();
        conn.execute(
            "INSERT INTO customers (remote_id, first_name) VALUES (7, 'Ada')",
            [],
        )
        .expect("customer 7");
        let customer7: i64 = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO customers (remote_id, first_name) VALUES (8, 'Ben')",
            [],
        )
        .expect("customer 8");
        let customer8: i64 = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO issue_clusters (name, conversation_count) VALUES ('mixed-attribution', 6)",
            [],
        )
        .expect("cluster");
        let cluster = conn.last_insert_rowid();
        seed_cluster_conv(&conn, cluster, 9101, Some(customer7), None, "-2 days");
        seed_cluster_conv(&conn, cluster, 9102, Some(customer8), None, "-2 days");
        for number in 9103..=9106 {
            seed_cluster_conv(&conn, cluster, number, None, None, "-2 days");
        }

        let clusters = radar_clusters(&conn);
        let alerts = radar_extensions(&conn, &clusters);
        assert!(
            !alerts.iter().any(|a| a["kind"] == "customer_concentration"),
            "two attributed conversations cannot concentrate: {alerts:?}"
        );
    }

    /// Audit item 111: the escalation alert's claim counted a silently
    /// LIMIT-capped sample — 1001 escalated tickets reported as "1000".
    #[test]
    fn escalation_alert_claims_the_real_count() {
        let conn = db();
        conn.execute(
            "INSERT INTO tags (remote_id, name) VALUES (1, 'escalated')",
            [],
        )
        .expect("tag");
        let tag: i64 = conn.last_insert_rowid();
        for i in 1..=1001_i64 {
            conn.execute(
                "INSERT INTO conversations (remote_id, number, subject, created_at)
                 VALUES (?1, ?1, 'escalated', datetime('now', '-1 days'))",
                params![i],
            )
            .expect("conversation");
            conn.execute(
                "INSERT INTO conversation_tags (conversation_id, tag_id) VALUES (?1, ?2)",
                params![conn.last_insert_rowid(), tag],
            )
            .expect("tag link");
        }

        let alerts = issue_radar_impl(&conn);
        let escalation = alerts
            .iter()
            .find(|a| a["kind"] == "escalation_heavy")
            .expect("escalation alert");
        assert!(
            escalation["title"]
                .as_str()
                .unwrap()
                .contains("1001 escalated tickets"),
            "the claim must match the real sample: {escalation}"
        );
    }
}
