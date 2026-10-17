use rusqlite::{Connection, params};
use serde_json::{Value, json};

pub const INCIDENT_STATUSES: [&str; 5] = [
    "investigating",
    "identified",
    "fix_in_progress",
    "monitoring",
    "resolved",
];

pub const INCIDENT_SEVERITIES: [&str; 4] = ["sev1", "sev2", "sev3", "sev4"];

#[derive(Debug, Default)]
pub struct IncidentListFilters {
    pub status: Option<String>,
    pub severity: Option<String>,
    pub open: bool,
    pub query: Option<String>,
    pub limit: i64,
    pub offset: i64,
}

const INCIDENT_ROW_SQL: &str = "i.id, i.code, i.title, i.known_issue_id, i.status, i.severity,
       i.source, i.description, i.internal_explanation, i.customer_safe_explanation,
       i.known_cause, i.workaround, i.resolution, i.started_at, i.resolved_at,
       i.owner_user_local_id, i.product, i.feature, i.created_at, i.updated_at,
       (SELECT COUNT(*) FROM incident_conversations ic WHERE ic.incident_id = i.id) AS conversation_count,
       (SELECT COUNT(DISTINCT c.customer_local_id) FROM incident_conversations ic
          JOIN conversations c ON c.id = ic.conversation_id
          WHERE ic.incident_id = i.id AND c.customer_local_id IS NOT NULL AND c.deleted_at IS NULL) AS customer_count,
       (SELECT COUNT(DISTINCT cu.organization) FROM incident_conversations ic
          JOIN conversations c ON c.id = ic.conversation_id
          JOIN customers cu ON cu.id = c.customer_local_id
          WHERE ic.incident_id = i.id AND c.deleted_at IS NULL
            AND cu.organization IS NOT NULL AND TRIM(cu.organization) != '') AS organization_count,
       (SELECT NULLIF(TRIM(COALESCE(u.first_name, '') || ' ' || COALESCE(u.last_name, '')), '')
          FROM users u WHERE u.id = i.owner_user_local_id) AS owner_name";

pub fn list_incident_rows(
    conn: &Connection,
    filters: &IncidentListFilters,
) -> crate::error::Result<(Vec<Value>, i64)> {
    let mut where_parts: Vec<String> = Vec::new();
    let mut bound: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    if let Some(status) = &filters.status {
        bound.push(Box::new(status.clone()));
        where_parts.push(format!("i.status = ?{}", bound.len()));
    }
    if let Some(severity) = &filters.severity {
        bound.push(Box::new(severity.clone()));
        where_parts.push(format!("i.severity = ?{}", bound.len()));
    }
    if filters.open {
        where_parts.push("i.status != 'resolved'".to_string());
    }
    if let Some(q) = filters.query.as_deref().filter(|q| !q.is_empty()) {
        bound.push(Box::new(format!("%{q}%")));
        where_parts.push(format!(
            "(i.title LIKE ?{} OR i.code LIKE ?{})",
            bound.len(),
            bound.len()
        ));
    }
    let where_sql = if where_parts.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", where_parts.join(" AND "))
    };

    let sql = format!(
        "SELECT {INCIDENT_ROW_SQL}
         FROM incidents i {where_sql}
         ORDER BY CASE i.status WHEN 'resolved' THEN 1 ELSE 0 END, i.updated_at DESC
         LIMIT ?{} OFFSET ?{}",
        bound.len() + 1,
        bound.len() + 2
    );
    bound.push(Box::new(filters.limit.clamp(1, 200)));
    bound.push(Box::new(filters.offset.max(0)));
    let rows = query_incident_rows(conn, &sql, &bound)?;

    let total: i64 = if where_parts.is_empty() {
        conn.query_row("SELECT COUNT(*) FROM incidents i", [], |r| r.get(0))?
    } else {
        let refs: Vec<&dyn rusqlite::ToSql> = bound[..bound.len() - 2]
            .iter()
            .map(|b| b.as_ref())
            .collect();
        conn.query_row(
            &format!("SELECT COUNT(*) FROM incidents i {where_sql}"),
            refs.as_slice(),
            |r| r.get(0),
        )?
    };
    Ok((rows, total))
}

#[must_use]
pub fn incident_detail(conn: &Connection, id: i64) -> Option<Value> {
    let incident = conn
        .query_row(
            &format!("SELECT {INCIDENT_ROW_SQL} FROM incidents i WHERE i.id = ?1"),
            params![id],
            incident_row_from_db,
        )
        .ok()?;
    let impact = crate::issue_impact::for_incident(conn, id)?;
    Some(json!({
        "incident": incident,
        "impact": impact,
        "conversations": list_conversations(conn, id, 200),
        "affected_customers": affected_customers(conn, id, 100),
        "affected_organizations": affected_organizations(conn, id, 50),
        "related": list_related(conn, id),
        "refs": list_refs(conn, id),
        "releases": list_releases(conn, id),
        "notes": list_notes(conn, id, 100),
        "timeline": list_events(conn, id, 200),
    }))
}

fn query_incident_rows(
    conn: &Connection,
    sql: &str,
    bound: &[Box<dyn rusqlite::ToSql>],
) -> crate::error::Result<Vec<Value>> {
    let refs: Vec<&dyn rusqlite::ToSql> = bound.iter().map(|b| b.as_ref()).collect();
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt
        .query_map(refs.as_slice(), incident_row_from_db)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

fn incident_row_from_db(r: &rusqlite::Row<'_>) -> rusqlite::Result<Value> {
    Ok(json!({
        "id": r.get::<_, i64>(0)?,
        "code": r.get::<_, Option<String>>(1)?,
        "title": r.get::<_, Option<String>>(2)?,
        "known_issue_id": r.get::<_, Option<i64>>(3)?,
        "status": r.get::<_, String>(4)?,
        "severity": r.get::<_, String>(5)?,
        "source": r.get::<_, String>(6)?,
        "description": r.get::<_, Option<String>>(7)?,
        "internal_explanation": r.get::<_, Option<String>>(8)?,
        "customer_safe_explanation": r.get::<_, Option<String>>(9)?,
        "known_cause": r.get::<_, Option<String>>(10)?,
        "workaround": r.get::<_, Option<String>>(11)?,
        "resolution": r.get::<_, Option<String>>(12)?,
        "started_at": r.get::<_, Option<String>>(13)?,
        "resolved_at": r.get::<_, Option<String>>(14)?,
        "owner_user_local_id": r.get::<_, Option<i64>>(15)?,
        "product": r.get::<_, Option<String>>(16)?,
        "feature": r.get::<_, Option<String>>(17)?,
        "created_at": r.get::<_, String>(18)?,
        "updated_at": r.get::<_, String>(19)?,
        "conversation_count": r.get::<_, i64>(20)?,
        "customer_count": r.get::<_, i64>(21)?,
        "organization_count": r.get::<_, i64>(22)?,
        "owner_name": r.get::<_, Option<String>>(23)?,
    }))
}

#[must_use]
pub fn list_conversations(conn: &Connection, incident_id: i64, limit: i64) -> Vec<Value> {
    let sql = "SELECT ic.conversation_id, c.number, c.subject, c.status,
           (SELECT m.name FROM mailboxes m WHERE m.id = c.mailbox_local_id) AS mailbox,
           c.customer_local_id,
           (SELECT NULLIF(TRIM(COALESCE(cu.first_name, '') || ' ' || COALESCE(cu.last_name, '')), '')
              FROM customers cu WHERE cu.id = c.customer_local_id) AS customer_name,
           c.created_at, ic.linked_by, ic.linked_at
         FROM incident_conversations ic
           JOIN conversations c ON c.id = ic.conversation_id
         WHERE ic.incident_id = ?1 AND c.deleted_at IS NULL
         ORDER BY c.created_at DESC
         LIMIT ?2";
    conn.prepare(sql)
        .and_then(|mut stmt| {
            let rows = stmt.query_map(params![incident_id, limit.clamp(1, 500)], |r| {
                Ok(json!({
                    "conversation_id": r.get::<_, i64>(0)?,
                    "number": r.get::<_, i64>(1)?,
                    "subject": r.get::<_, Option<String>>(2)?,
                    "status": r.get::<_, String>(3)?,
                    "mailbox": r.get::<_, Option<String>>(4)?,
                    "customer_local_id": r.get::<_, i64>(5)?,
                    "customer_name": r.get::<_, Option<String>>(6)?,
                    "remote_created_at": r.get::<_, Option<String>>(7)?,
                    "linked_by": r.get::<_, String>(8)?,
                    "linked_at": r.get::<_, String>(9)?,
                }))
            })?;
            rows.collect::<rusqlite::Result<Vec<_>>>()
        })
        .unwrap_or_default()
}

#[must_use]
pub fn affected_customers(conn: &Connection, incident_id: i64, limit: i64) -> Vec<Value> {
    let sql = "SELECT c.customer_local_id,
           (SELECT NULLIF(TRIM(COALESCE(cu.first_name, '') || ' ' || COALESCE(cu.last_name, '')), '')
              FROM customers cu WHERE cu.id = c.customer_local_id) AS name,
           (SELECT cu.email FROM customers cu WHERE cu.id = c.customer_local_id) AS email,
           (SELECT NULLIF(cu.organization, '') FROM customers cu WHERE cu.id = c.customer_local_id) AS organization,
           COUNT(*) AS conversations,
           SUM(CASE WHEN c.status = 'active' THEN 1 ELSE 0 END) AS open_conversations
         FROM incident_conversations ic
           JOIN conversations c ON c.id = ic.conversation_id
         WHERE ic.incident_id = ?1 AND c.deleted_at IS NULL AND c.customer_local_id IS NOT NULL
         GROUP BY c.customer_local_id
         ORDER BY conversations DESC, c.customer_local_id
         LIMIT ?2";
    conn.prepare(sql)
        .and_then(|mut stmt| {
            let rows = stmt.query_map(params![incident_id, limit.clamp(1, 500)], |r| {
                Ok(json!({
                    "customer_local_id": r.get::<_, i64>(0)?,
                    "name": r.get::<_, Option<String>>(1)?,
                    "email": r.get::<_, Option<String>>(2)?,
                    "organization": r.get::<_, Option<String>>(3)?,
                    "conversations": r.get::<_, i64>(4)?,
                    "open_conversations": r.get::<_, Option<i64>>(5)?.unwrap_or(0),
                }))
            })?;
            rows.collect::<rusqlite::Result<Vec<_>>>()
        })
        .unwrap_or_default()
}

#[must_use]
pub fn affected_organizations(conn: &Connection, incident_id: i64, limit: i64) -> Vec<Value> {
    let sql = "SELECT (SELECT o.id FROM organizations o WHERE o.name = cu.organization LIMIT 1) AS organization_id,
           cu.organization AS name,
           COUNT(DISTINCT c.customer_local_id) AS customers,
           COUNT(*) AS conversations
         FROM incident_conversations ic
           JOIN conversations c ON c.id = ic.conversation_id
           JOIN customers cu ON cu.id = c.customer_local_id
         WHERE ic.incident_id = ?1 AND c.deleted_at IS NULL
           AND cu.organization IS NOT NULL AND TRIM(cu.organization) != ''
         GROUP BY cu.organization
         ORDER BY conversations DESC
         LIMIT ?2";
    conn.prepare(sql)
        .and_then(|mut stmt| {
            let rows = stmt.query_map(params![incident_id, limit.clamp(1, 200)], |r| {
                Ok(json!({
                    "organization_id": r.get::<_, Option<i64>>(0)?,
                    "name": r.get::<_, Option<String>>(1)?,
                    "customers": r.get::<_, i64>(2)?,
                    "conversations": r.get::<_, i64>(3)?,
                }))
            })?;
            rows.collect::<rusqlite::Result<Vec<_>>>()
        })
        .unwrap_or_default()
}

#[must_use]
pub fn list_related(conn: &Connection, incident_id: i64) -> Vec<Value> {
    let rows: Vec<(String, i64, Option<String>, String)> = conn
        .prepare(
            "SELECT target_kind, target_local_id, note, linked_at
             FROM incident_related WHERE incident_id = ?1 ORDER BY linked_at DESC",
        )
        .and_then(|mut stmt| {
            let rows = stmt.query_map(params![incident_id], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
            })?;
            rows.collect::<rusqlite::Result<Vec<_>>>()
        })
        .unwrap_or_default();
    rows.into_iter()
        .map(|(kind, target, note, linked_at)| {
            json!({
                "incident_id": incident_id,
                "target_kind": kind,
                "target_local_id": target,
                "target_label": label_for(conn, &kind, target),
                "note": note,
                "linked_at": linked_at,
            })
        })
        .collect()
}

fn label_for(conn: &Connection, kind: &str, id: i64) -> String {
    let sql = match kind {
        "known_issue" => "SELECT name FROM known_issues WHERE id = ?1",
        "knowledge_doc" => "SELECT title FROM knowledge_documents WHERE id = ?1",
        "campaign" => "SELECT name FROM outreach_campaigns WHERE id = ?1",
        "custom_object" => "SELECT title FROM custom_objects WHERE id = ?1",
        _ => return format!("#{id}"),
    };
    let label: Option<String> = conn.query_row(sql, params![id], |r| r.get(0)).ok();
    match (kind, label) {
        ("known_issue", Some(l)) => l,
        ("known_issue", None) => format!("known issue #{id}"),
        ("knowledge_doc", Some(l)) => l,
        ("knowledge_doc", None) => format!("document #{id}"),
        ("campaign", Some(l)) => l,
        ("campaign", None) => format!("campaign #{id}"),
        ("custom_object", Some(l)) => l,
        ("custom_object", None) => format!("object #{id}"),
        _ => format!("#{id}"),
    }
}

#[must_use]
pub fn list_refs(conn: &Connection, incident_id: i64) -> Vec<Value> {
    conn.prepare(
        "SELECT id, incident_id, system, reference, url, title, status, notes, created_at
         FROM incident_refs WHERE incident_id = ?1 ORDER BY id",
    )
    .and_then(|mut stmt| {
        let rows = stmt.query_map(params![incident_id], |r| {
            Ok(json!({
                "id": r.get::<_, i64>(0)?,
                "incident_id": r.get::<_, i64>(1)?,
                "system": r.get::<_, String>(2)?,
                "reference": r.get::<_, String>(3)?,
                "url": r.get::<_, Option<String>>(4)?,
                "title": r.get::<_, Option<String>>(5)?,
                "status": r.get::<_, Option<String>>(6)?,
                "notes": r.get::<_, Option<String>>(7)?,
                "created_at": r.get::<_, String>(8)?,
            }))
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
    })
    .unwrap_or_default()
}

#[must_use]
pub fn list_releases(conn: &Connection, incident_id: i64) -> Vec<Value> {
    conn.prepare(
        "SELECT id, incident_id, version_label, notes, released_at, correlation, created_at
         FROM incident_releases WHERE incident_id = ?1 ORDER BY id DESC",
    )
    .and_then(|mut stmt| {
        let rows = stmt.query_map(params![incident_id], |r| {
            Ok(json!({
                "id": r.get::<_, i64>(0)?,
                "incident_id": r.get::<_, i64>(1)?,
                "version_label": r.get::<_, String>(2)?,
                "notes": r.get::<_, Option<String>>(3)?,
                "released_at": r.get::<_, Option<String>>(4)?,
                "correlation": r.get::<_, Option<String>>(5)?,
                "created_at": r.get::<_, String>(6)?,
            }))
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
    })
    .unwrap_or_default()
}

#[must_use]
pub fn list_notes(conn: &Connection, incident_id: i64, limit: i64) -> Vec<Value> {
    conn.prepare(
        "SELECT n.id, n.incident_id, n.author_user_local_id,
           (SELECT NULLIF(TRIM(COALESCE(u.first_name, '') || ' ' || COALESCE(u.last_name, '')), '')
              FROM users u WHERE u.id = n.author_user_local_id) AS author_name,
           n.body, n.created_at
         FROM incident_notes n WHERE n.incident_id = ?1
         ORDER BY n.created_at DESC LIMIT ?2",
    )
    .and_then(|mut stmt| {
        let rows = stmt.query_map(params![incident_id, limit.clamp(1, 200)], |r| {
            Ok(json!({
                "id": r.get::<_, i64>(0)?,
                "incident_id": r.get::<_, i64>(1)?,
                "author_user_local_id": r.get::<_, Option<i64>>(2)?,
                "author_name": r.get::<_, Option<String>>(3)?,
                "body": r.get::<_, String>(4)?,
                "created_at": r.get::<_, String>(5)?,
            }))
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
    })
    .unwrap_or_default()
}

#[must_use]
pub fn incident_note_json(conn: &Connection, note_id: i64) -> Option<Value> {
    conn.query_row(
        "SELECT n.id, n.incident_id, n.author_user_local_id,
           (SELECT NULLIF(TRIM(COALESCE(u.first_name, '') || ' ' || COALESCE(u.last_name, '')), '')
              FROM users u WHERE u.id = n.author_user_local_id) AS author_name,
           n.body, n.created_at
         FROM incident_notes n WHERE n.id = ?1",
        params![note_id],
        |r| {
            Ok(json!({
                "id": r.get::<_, i64>(0)?,
                "incident_id": r.get::<_, i64>(1)?,
                "author_user_local_id": r.get::<_, Option<i64>>(2)?,
                "author_name": r.get::<_, Option<String>>(3)?,
                "body": r.get::<_, String>(4)?,
                "created_at": r.get::<_, String>(5)?,
            }))
        },
    )
    .ok()
}

#[must_use]
pub fn list_events(conn: &Connection, incident_id: i64, limit: i64) -> Vec<Value> {
    conn.prepare(
        "SELECT id, incident_id, event_type, actor_user_local_id, occurred_at, detail, source
         FROM incident_events WHERE incident_id = ?1
         ORDER BY occurred_at DESC, id DESC LIMIT ?2",
    )
    .and_then(|mut stmt| {
        let rows = stmt.query_map(params![incident_id, limit.clamp(1, 500)], |r| {
            Ok(json!({
                "id": r.get::<_, i64>(0)?,
                "incident_id": r.get::<_, i64>(1)?,
                "event_type": r.get::<_, String>(2)?,
                "actor_user_local_id": r.get::<_, Option<i64>>(3)?,
                "occurred_at": r.get::<_, String>(4)?,
                "detail": r.get::<_, Option<String>>(5)?,
                "source": r.get::<_, String>(6)?,
            }))
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
    })
    .unwrap_or_default()
}
