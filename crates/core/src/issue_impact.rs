use rusqlite::{params, Connection};
use serde_json::{json, Value};

const IMPACT_NOTE: &str = "Customer and organization counts are distinct entities, never ticket counts. Products come from the local AI attribute layer (empty until conversations are analyzed). Release entries describe temporal associations only.";

#[must_use]
pub fn for_incident(conn: &Connection, incident_id: i64) -> Option<Value> {
    let exists: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM incidents WHERE id = ?1",
            params![incident_id],
            |r| r.get::<_, i64>(0),
        )
        .map(|n| n > 0)
        .unwrap_or(false);
    if !exists {
        return None;
    }
    Some(compute(
        conn,
        incident_id,
        "incident",
        "incident_conversations",
        "incident_id",
    ))
}

#[must_use]
pub fn for_known_issue(conn: &Connection, known_issue_id: i64) -> Option<Value> {
    let exists: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM known_issues WHERE id = ?1",
            params![known_issue_id],
            |r| r.get::<_, i64>(0),
        )
        .map(|n| n > 0)
        .unwrap_or(false);
    if !exists {
        return None;
    }
    Some(compute(
        conn,
        known_issue_id,
        "known_issue",
        "known_issue_links",
        "known_issue_id",
    ))
}

fn compute(
    conn: &Connection,
    subject_id: i64,
    subject_kind: &str,
    link_table: &str,
    link_column: &str,
) -> Value {
    let conv_ids_sql = format!(
        "SELECT l.conversation_id AS id FROM {link_table} l
          JOIN conversations c ON c.id = l.conversation_id
          WHERE l.{link_column} = ?1 AND c.deleted_at IS NULL"
    );
    let base =
        format!("FROM conversations c WHERE c.id IN ({conv_ids_sql}) AND c.deleted_at IS NULL");

    let counts: (
        i64,
        i64,
        i64,
        Option<String>,
        Option<String>,
        i64,
        i64,
        i64,
    ) = conn
        .query_row(
            &format!(
                "SELECT COUNT(*) AS conversations,
                   COUNT(DISTINCT c.customer_local_id) AS customers,
                   (SELECT COUNT(DISTINCT cu.organization) FROM conversations c2
                      JOIN customers cu ON cu.id = c2.customer_local_id
                      WHERE c2.id IN ({conv_ids_sql}) AND c2.deleted_at IS NULL
                        AND cu.organization IS NOT NULL AND TRIM(cu.organization) != '') AS organizations,
                   MIN(c.created_at) AS first_seen,
                   MAX(c.created_at) AS last_seen,
                   SUM(CASE WHEN c.status = 'active' THEN 1 ELSE 0 END) AS open_count,
                   SUM(CASE WHEN c.status = 'closed' THEN 1 ELSE 0 END) AS closed_count,
                   SUM(CASE WHEN c.customer_waiting_since IS NOT NULL AND c.status = 'active' THEN 1 ELSE 0 END) AS waiting_count
                 {base}"
            ),
            params![subject_id],
            |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get::<_, Option<i64>>(5)?.unwrap_or(0),
                    r.get::<_, Option<i64>>(6)?.unwrap_or(0),
                    r.get::<_, Option<i64>>(7)?.unwrap_or(0),
                ))
            },
        )
        .unwrap_or((0, 0, 0, None, None, 0, 0, 0));
    let (
        conversations,
        customers,
        organizations,
        first_seen,
        last_seen,
        open_count,
        closed_count,
        waiting_count,
    ) = counts;

    let recent = linked_since(conn, subject_id, &base, "-7 days");
    let previous = linked_between(conn, subject_id, &base, "-14 days", "-7 days");
    let recent14 = linked_since(conn, subject_id, &base, "-14 days");
    let prior14 = linked_between(conn, subject_id, &base, "-28 days", "-14 days");
    let (ratio, direction) = if previous > 0 {
        let r = recent as f64 / previous as f64;
        let d = if r >= 1.3 {
            "rising"
        } else if r <= 0.7 {
            "falling"
        } else {
            "flat"
        };
        (json!(r), d)
    } else if recent > 0 {
        (Value::Null, "rising")
    } else {
        (json!(0.0), "unknown")
    };

    let trend = if conversations == 0 {
        "unknown"
    } else if first_seen
        .as_deref()
        .map(|fs| {
            conn.query_row(
                "SELECT julianday(?1) >= julianday('now', '-7 days')",
                params![fs],
                |r| r.get::<_, i64>(0),
            )
            .map(|v| v == 1)
            .unwrap_or(false)
        })
        .unwrap_or(false)
    {
        "new"
    } else if prior14 == 0 && recent14 >= 1 {
        "rising"
    } else if prior14 > 0 && (recent14 as f64) < (prior14 as f64) * 0.7 {
        "falling"
    } else {
        "stable"
    };

    let inboxes = collect_pairs(
        conn,
        &format!(
            "SELECT (SELECT m.name FROM mailboxes m WHERE m.id = c.mailbox_local_id) AS mailbox,
                    COUNT(*) AS conversations
             {base} GROUP BY c.mailbox_local_id ORDER BY conversations DESC LIMIT 10"
        ),
        subject_id,
        "mailbox",
        "conversations",
    );

    let tags = collect_pairs(
        conn,
        &format!(
            "SELECT t.name AS tag, COUNT(*) AS conversations
             FROM conversation_tags ct
               JOIN tags t ON t.id = ct.tag_id
             WHERE ct.conversation_id IN ({conv_ids_sql})
             GROUP BY t.name ORDER BY conversations DESC LIMIT 10"
        ),
        subject_id,
        "tag",
        "conversations",
    );

    let products = collect_pairs(
        conn,
        &format!(
            "SELECT aa.value AS product, COUNT(DISTINCT aa.conversation_id) AS conversations
             FROM ai_attributes aa
             WHERE aa.attribute = 'product' AND aa.superseded_at IS NULL AND aa.source = 'ai'
               AND aa.conversation_id IN ({conv_ids_sql}) AND aa.confidence IN ('high', 'medium')
             GROUP BY aa.value ORDER BY conversations DESC LIMIT 10"
        ),
        subject_id,
        "product",
        "conversations",
    );

    let mut release_candidates: Vec<Value> = Vec::new();
    if subject_kind == "incident" {
        let releases: Vec<(i64, String, String)> = conn
            .prepare(
                "SELECT id, version_label, released_at FROM incident_releases
                 WHERE incident_id = ?1 AND released_at IS NOT NULL",
            )
            .and_then(|mut stmt| {
                let rows = stmt.query_map(params![subject_id], |r| {
                    Ok((r.get(0)?, r.get(1)?, r.get(2)?))
                })?;
                rows.collect::<rusqlite::Result<Vec<_>>>()
            })
            .unwrap_or_default();
        for (_, label, released_at) in releases {
            let n: i64 = conn
                .query_row(
                    &format!(
                        "SELECT COUNT(*) {base}
                         AND julianday(c.created_at) >= julianday(?2)
                         AND julianday(c.created_at) < julianday(?2, '+7 days')"
                    ),
                    params![subject_id, released_at],
                    |r| r.get(0),
                )
                .unwrap_or(0);
            if n > 0 {
                release_candidates.push(json!({
                    "version_label": label,
                    "released_at": released_at,
                    "conversations_within_7d": n,
                    "note": format!("{n} linked conversations started within 7 days after this release date - a temporal association, not a causal claim."),
                }));
            }
        }
    }

    json!({
        "subject_kind": subject_kind,
        "subject_id": subject_id,
        "affected_conversations": conversations,
        "affected_customers": customers,
        "affected_organizations": organizations,
        "first_seen_at": first_seen,
        "last_seen_at": last_seen,
        "growth_rate_7d": {
            "recent": recent,
            "previous": previous,
            "ratio": ratio,
            "direction": direction,
        },
        "trend": trend,
        "affected_inboxes": inboxes,
        "top_tags": tags,
        "products": products,
        "open_closed_distribution": { "open": open_count, "closed": closed_count },
        "customer_waiting_count": waiting_count,
        "release_correlation_candidates": release_candidates,
        "note": IMPACT_NOTE,
    })
}

fn linked_since(conn: &Connection, subject_id: i64, base: &str, since: &str) -> i64 {
    conn.query_row(
        &format!(
            "SELECT COUNT(*) AS n {base} AND julianday(c.created_at) >= julianday('now', '{since}')"
        ),
        params![subject_id],
        |r| r.get(0),
    )
    .unwrap_or(0)
}

fn linked_between(conn: &Connection, subject_id: i64, base: &str, from: &str, to: &str) -> i64 {
    conn.query_row(
        &format!(
            "SELECT COUNT(*) AS n {base}
             AND julianday(c.created_at) >= julianday('now', '{from}')
             AND julianday(c.created_at) < julianday('now', '{to}')"
        ),
        params![subject_id],
        |r| r.get(0),
    )
    .unwrap_or(0)
}

fn collect_pairs(
    conn: &Connection,
    sql: &str,
    subject_id: i64,
    key: &str,
    count_key: &str,
) -> Vec<Value> {
    conn.prepare(sql)
        .and_then(|mut stmt| {
            let rows = stmt.query_map(params![subject_id], |r| {
                Ok((r.get::<_, Option<String>>(0)?, r.get::<_, i64>(1)?))
            })?;
            rows.collect::<rusqlite::Result<Vec<_>>>()
        })
        .map(|rows| {
            rows.into_iter()
                .map(|(k, n)| json!({ key: k, count_key: n }))
                .collect()
        })
        .unwrap_or_default()
}
