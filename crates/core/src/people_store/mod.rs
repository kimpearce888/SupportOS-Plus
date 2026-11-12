use rusqlite::{Connection, params};
use serde_json::{Value, json};

use crate::error::Result;

pub fn list_organizations(
    conn: &Connection,
    query: Option<&str>,
    limit: u32,
) -> Result<Vec<Value>> {
    let limit = limit.clamp(1, 200) as i64;
    let like = query
        .map(|q| {
            format!(
                "%{}%",
                q.trim()
                    .replace('\\', "\\\\")
                    .replace('%', "\\%")
                    .replace('_', "\\_")
            )
        })
        .unwrap_or_else(|| "%".to_string());
    let mut stmt = conn.prepare(
        "SELECT o.id, o.remote_id, o.name, o.domains, o.remote_created_at,
                o.remote_updated_at, o.local_created_at, o.local_updated_at,
                (SELECT COUNT(*) FROM customers cu
                  WHERE (cu.organization_id = o.id
                         OR (cu.organization_id IS NULL AND cu.organization = o.name COLLATE NOCASE))
                    AND cu.deleted_at IS NULL) AS customer_count,
                (SELECT COUNT(*) FROM conversations c
                  JOIN customers cu ON cu.id = c.customer_local_id
                  WHERE (cu.organization_id = o.id
                         OR (cu.organization_id IS NULL AND cu.organization = o.name COLLATE NOCASE))
                    AND cu.deleted_at IS NULL) AS conversation_count
         FROM organizations o
         WHERE o.deleted_at IS NULL
           AND o.name LIKE ?1 ESCAPE '\\'
         ORDER BY o.name COLLATE NOCASE
         LIMIT ?2",
    )?;
    let orgs: Vec<Value> = stmt
        .query_map(params![like, limit], org_row_to_json)?
        .filter_map(|r| r.ok())
        .collect();
    Ok(orgs)
}

fn org_row_to_json(row: &rusqlite::Row<'_>) -> rusqlite::Result<Value> {
    let domains_text: Option<String> = row.get("domains")?;
    let domains: Vec<String> = domains_text
        .and_then(|t| serde_json::from_str::<Vec<String>>(&t).ok())
        .unwrap_or_default();
    Ok(json!({
        "id": row.get::<_, i64>("id")?,
        "remote_id": row.get::<_, i64>("remote_id")?,
        "name": row.get::<_, String>("name")?,
        "domains": domains,
        "customer_count": row.get::<_, i64>("customer_count")?,
        "conversation_count": row.get::<_, i64>("conversation_count")?,
        "created_at": row
            .get::<_, Option<String>>("remote_created_at")?
            .or_else(|| row.get::<_, Option<String>>("local_created_at").ok().flatten()),
        "updated_at": row
            .get::<_, Option<String>>("remote_updated_at")?
            .or_else(|| row.get::<_, Option<String>>("local_updated_at").ok().flatten()),
    }))
}

const ORG_MEMBERS_SQL: &str = "(cu.organization_id = ?1
    OR (cu.organization_id IS NULL AND cu.organization = (SELECT name FROM organizations WHERE id = ?1) COLLATE NOCASE))";

pub fn get_organization_detail(conn: &Connection, org_id: i64) -> Result<Option<Value>> {
    let org: Option<Value> = {
        let mut stmt = conn.prepare(
            "SELECT o.id, o.remote_id, o.name, o.domains, o.remote_created_at,
                    o.remote_updated_at, o.local_created_at, o.local_updated_at,
                    (SELECT COUNT(*) FROM customers cu
                      WHERE (cu.organization_id = o.id
                             OR (cu.organization_id IS NULL AND cu.organization = o.name COLLATE NOCASE))
                        AND cu.deleted_at IS NULL) AS customer_count,
                    (SELECT COUNT(*) FROM conversations c
                      JOIN customers cu ON cu.id = c.customer_local_id
                      WHERE (cu.organization_id = o.id
                             OR (cu.organization_id IS NULL AND cu.organization = o.name COLLATE NOCASE))
                        AND cu.deleted_at IS NULL) AS conversation_count
             FROM organizations o
             WHERE o.id = ?1 AND o.deleted_at IS NULL",
        )?;
        let mut rows: Vec<Value> = stmt
            .query_map(params![org_id], org_row_to_json)?
            .filter_map(|r| r.ok())
            .collect();
        rows.pop()
    };
    let Some(mut org) = org else {
        return Ok(None);
    };

    let mut stmt = conn.prepare(&format!(
        "SELECT cu.id, cu.first_name, cu.last_name, cu.email, cu.job_title,
                (SELECT COUNT(*) FROM conversations c
                  WHERE c.customer_local_id = cu.id AND c.deleted_at IS NULL) AS conversation_count,
                (SELECT COUNT(*) FROM conversations c
                  WHERE c.customer_local_id = cu.id AND c.status = 'active' AND c.deleted_at IS NULL) AS open_count,
                (SELECT MAX(COALESCE(c.updated_at, c.local_created_at)) FROM conversations c
                  WHERE c.customer_local_id = cu.id AND c.deleted_at IS NULL) AS last_activity_at
         FROM customers cu
         WHERE {ORG_MEMBERS_SQL} AND cu.deleted_at IS NULL
         ORDER BY COALESCE(cu.last_name, cu.last_name, cu.first_name, cu.email, cu.id)
         LIMIT 100"
    ))?;
    let members: Vec<Value> = stmt
        .query_map(params![org_id], |row| {
            let first: Option<String> = row.get(1)?;
            let last: Option<String> = row.get(2)?;
            let name = match (first.clone(), last.clone()) {
                (Some(f), Some(l)) if !f.trim().is_empty() && !l.trim().is_empty() => {
                    format!("{} {}", f.trim(), l.trim())
                }
                (Some(f), _) if !f.trim().is_empty() => f.trim().to_string(),
                (_, Some(l)) if !l.trim().is_empty() => l.trim().to_string(),
                _ => "(unnamed)".to_string(),
            };
            Ok(json!({
                "id": row.get::<_, i64>(0)?,
                "name": name,
                "email": row.get::<_, Option<String>>(3)?,
                "job_title": row.get::<_, Option<String>>(4)?,
                "conversation_count": row.get::<_, i64>(5)?,
                "open_count": row.get::<_, i64>(6)?,
                "last_activity_at": row.get::<_, Option<String>>(7)?,
            }))
        })?
        .filter_map(|r| r.ok())
        .collect();

    let (member_total, open_total, conversation_total): (i64, i64, i64) = conn
        .query_row(
            &format!(
                "SELECT COUNT(*),
                        COALESCE(SUM((SELECT COUNT(*) FROM conversations c
                           WHERE c.customer_local_id = cu.id AND c.deleted_at IS NULL)), 0),
                        COALESCE(SUM((SELECT COUNT(*) FROM conversations c
                           WHERE c.customer_local_id = cu.id AND c.status = 'active' AND c.deleted_at IS NULL)), 0)
                   FROM customers cu
                  WHERE {ORG_MEMBERS_SQL} AND cu.deleted_at IS NULL"
            ),
            params![org_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap_or((0, 0, 0));
    let member_count = member_total;

    if let Some(obj) = org.as_object_mut() {
        obj.insert("members".into(), Value::Array(members));
        obj.insert("member_count".into(), json!(member_count));
        obj.insert("open_conversation_count".into(), json!(open_total));
        if obj.get("customer_count").and_then(|v| v.as_i64()) == Some(0) && member_count > 0 {
            obj.insert("customer_count".into(), json!(member_count));
        }
        if obj.get("conversation_count").and_then(|v| v.as_i64()) == Some(0)
            && conversation_total > 0
        {
            obj.insert("conversation_count".into(), json!(conversation_total));
        }
    }
    Ok(Some(org))
}

mod health;

use health::build_support_health_report;

pub fn list_customers_summary(
    conn: &Connection,
    query: Option<&str>,
    page: i64,
    page_size: i64,
) -> Result<(Vec<Value>, i64)> {
    let page = page.clamp(1, 100_000);
    let page_size = page_size.clamp(1, 200);
    let offset = (page - 1) * page_size;

    let (where_sql, like): (String, Option<String>) = match query {
        Some(q) if !q.trim().is_empty() => {
            let like = format!(
                "%{}%",
                q.trim()
                    .replace('\\', "\\\\")
                    .replace('%', "\\%")
                    .replace('_', "\\_")
            );
            (
                "WHERE (c.first_name LIKE ?1 ESCAPE '\\'
                    OR c.last_name LIKE ?1 ESCAPE '\\'
                    OR c.email LIKE ?1 ESCAPE '\\'
                    OR c.organization LIKE ?1 ESCAPE '\\'
                    OR EXISTS (SELECT 1 FROM customer_emails ce
                                WHERE ce.customer_id = c.id
                                  AND ce.value LIKE ?1 ESCAPE '\\'))
                  AND c.deleted_at IS NULL"
                    .to_string(),
                Some(like),
            )
        }
        _ => ("WHERE c.deleted_at IS NULL".to_string(), None),
    };

    let total: i64 = if let Some(ref like) = like {
        conn.query_row(
            &format!("SELECT COUNT(*) FROM customers c {where_sql}"),
            params![like],
            |r| r.get(0),
        )
        .unwrap_or(0)
    } else {
        conn.query_row(
            &format!("SELECT COUNT(*) FROM customers c {where_sql}"),
            [],
            |r| r.get(0),
        )
        .unwrap_or(0)
    };

    let mut stmt = conn.prepare(&format!(
        "SELECT c.id, c.remote_id, c.first_name, c.last_name, c.job_title,
                c.email, c.phone, c.organization, c.organization_id,
                (SELECT o.name FROM organizations o WHERE o.id = c.organization_id) AS organization_name,
                (SELECT GROUP_CONCAT(ce.value) FROM customer_emails ce WHERE ce.customer_id = c.id) AS emails_csv,
                (SELECT GROUP_CONCAT(cp.value) FROM customer_phones cp WHERE cp.customer_id = c.id) AS phones_csv,
                (SELECT COUNT(*) FROM conversations cv
                  WHERE cv.customer_local_id = c.id AND cv.deleted_at IS NULL) AS conversation_count,
                (SELECT COUNT(*) FROM conversations cv
                  WHERE cv.customer_local_id = c.id AND cv.status IN ('active','pending')
                    AND cv.deleted_at IS NULL) AS open_conversation_count,
                (SELECT MAX(cv.last_activity_at) FROM conversations cv
                  WHERE cv.customer_local_id = c.id AND cv.deleted_at IS NULL) AS last_activity_at,
                (SELECT AVG(CASE r.rating WHEN 'great' THEN 5 WHEN 'okay' THEN 3 WHEN 'not-good' THEN 1 END)
                   FROM ratings r WHERE r.customer_local_id = c.id) AS average_rating
         FROM customers c
         {where_sql}
         ORDER BY COALESCE(
             (SELECT MAX(cv.last_activity_at) FROM conversations cv
              WHERE cv.customer_local_id = c.id AND cv.deleted_at IS NULL),
             c.local_created_at,
             c.created_at) DESC
         LIMIT ?2 OFFSET ?3"
    ))?;

    let like_param = like.clone().unwrap_or_else(|| "%".to_string());
    let rows: Vec<Value> = stmt
        .query_map(params![like_param, page_size, offset], customer_summary_row)?
        .filter_map(|r| r.ok())
        .collect();
    Ok((rows, total))
}

fn customer_summary_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Value> {
    let flat_email: Option<String> = row.get("email")?;
    let flat_phone: Option<String> = row.get("phone")?;
    let org_name: Option<String> = row.get("organization_name")?;
    let legacy_org: Option<String> = row.get("organization")?;
    let emails_csv: Option<String> = row.get("emails_csv")?;
    let phones_csv: Option<String> = row.get("phones_csv")?;
    let average_rating: Option<f64> = row.get("average_rating")?;
    Ok(json!({
        "id": row.get::<_, i64>("id")?,
        "remote_id": row.get::<_, i64>("remote_id")?,
        "first_name": row.get::<_, Option<String>>("first_name")?,
        "last_name": row.get::<_, Option<String>>("last_name")?,
        "job_title": row.get::<_, Option<String>>("job_title")?,
        "emails": csv_or_legacy(emails_csv, flat_email),
        "phones": csv_or_legacy(phones_csv, flat_phone),
        "organization_id": row.get::<_, Option<i64>>("organization_id")?,
        "organization_name": org_name.or(legacy_org),
        "conversation_count": row.get::<_, i64>("conversation_count")?,
        "open_conversation_count": row.get::<_, i64>("open_conversation_count")?,
        "last_activity_at": row.get::<_, Option<String>>("last_activity_at")?,
        "average_rating": average_rating,
    }))
}

fn csv_or_legacy(csv: Option<String>, legacy: Option<String>) -> Vec<String> {
    let from_csv = csv
        .as_deref()
        .map(|c| {
            c.split(',')
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if !from_csv.is_empty() {
        return from_csv;
    }
    legacy.map(|e| vec![e]).unwrap_or_default()
}

pub fn get_customer_detail(conn: &Connection, customer_id: i64) -> Result<Option<Value>> {
    let exists: bool = conn
        .query_row(
            "SELECT 1 FROM customers WHERE id = ?1 AND deleted_at IS NULL",
            params![customer_id],
            |_| Ok(()),
        )
        .is_ok();
    if !exists {
        return Ok(None);
    }

    let mut stmt = conn.prepare(
        "SELECT c.id, c.remote_id, c.first_name, c.last_name, c.job_title,
                c.email, c.phone, c.organization, c.organization_id,
                (SELECT o.name FROM organizations o WHERE o.id = c.organization_id) AS organization_name,
                (SELECT GROUP_CONCAT(ce.value) FROM customer_emails ce WHERE ce.customer_id = c.id) AS emails_csv,
                (SELECT GROUP_CONCAT(cp.value) FROM customer_phones cp WHERE cp.customer_id = c.id) AS phones_csv,
                (SELECT COUNT(*) FROM conversations cv
                  WHERE cv.customer_local_id = c.id AND cv.deleted_at IS NULL) AS conversation_count,
                (SELECT COUNT(*) FROM conversations cv
                  WHERE cv.customer_local_id = c.id AND cv.status IN ('active','pending')
                    AND cv.deleted_at IS NULL) AS open_conversation_count,
                (SELECT MAX(cv.last_activity_at) FROM conversations cv
                  WHERE cv.customer_local_id = c.id) AS last_activity_at,
                (SELECT AVG(CASE r.rating WHEN 'great' THEN 5 WHEN 'okay' THEN 3 WHEN 'not-good' THEN 1 END)
                   FROM ratings r WHERE r.customer_local_id = c.id) AS average_rating
         FROM customers c WHERE c.id = ?1",
    )?;
    let customer = stmt
        .query_row(params![customer_id], customer_summary_row)
        .map_err(crate::error::Error::from)?;

    let mut stmt = conn.prepare(
        "SELECT cv.id, cv.number, cv.subject, cv.status, cv.preview,
                cv.remote_created_at, cv.closed_at, cv.assignee_local_id,
                TRIM(COALESCE(u.first_name, '') || ' ' || COALESCE(u.last_name, '')) AS assignee
         FROM conversations cv
         LEFT JOIN users u ON u.id = cv.assignee_local_id
         WHERE cv.customer_local_id = ?1 AND cv.deleted_at IS NULL
         ORDER BY COALESCE(cv.remote_created_at, cv.created_at) DESC LIMIT 50",
    )?;
    let conversations: Vec<Value> = stmt
        .query_map(params![customer_id], |r| {
            Ok(json!({
                "id": r.get::<_, i64>(0)?,
                "number": r.get::<_, i64>(1)?,
                "subject": r.get::<_, Option<String>>(2)?,
                "status": r.get::<_, String>(3)?,
                "preview": r.get::<_, Option<String>>(4)?,
                "remote_created_at": r.get::<_, Option<String>>(5)?,
                "closed_at": r.get::<_, Option<String>>(6)?,
                "assignee": r.get::<_, Option<String>>(8)?.filter(|s| !s.trim().is_empty()),
            }))
        })?
        .filter_map(|r| r.ok())
        .collect();

    let mut stmt = conn.prepare(
        "SELECT rating, comments, remote_created_at, conversation_id
         FROM ratings WHERE customer_local_id = ?1
         ORDER BY remote_created_at DESC LIMIT 50",
    )?;
    let ratings: Vec<Value> = stmt
        .query_map(params![customer_id], |r| {
            Ok(json!({
                "rating": r.get::<_, Option<String>>(0)?,
                "comments": r.get::<_, Option<String>>(1)?,
                "created_at": r.get::<_, Option<String>>(2)?,
                "conversation_id": r.get::<_, Option<i64>>(3)?,
            }))
        })?
        .filter_map(|r| r.ok())
        .collect();

    let mut stmt = conn.prepare(
        "SELECT id, memory_key, memory_value, source, confidence, last_seen_at
         FROM customer_memory WHERE customer_id = ?1
         ORDER BY COALESCE(last_seen_at, created_at) DESC LIMIT 50",
    )?;
    let memories: Vec<Value> = stmt
        .query_map(params![customer_id], |r| {
            Ok(json!({
                "id": r.get::<_, i64>(0)?,
                "key": r.get::<_, String>(1)?,
                "value": r.get::<_, String>(2)?,
                "source": r.get::<_, String>(3)?,
                "confidence": r.get::<_, Option<String>>(4)?,
                "last_seen_at": r.get::<_, Option<String>>(5)?,
            }))
        })?
        .filter_map(|r| r.ok())
        .collect();

    let mut stmt = conn.prepare(
        "SELECT d.name, cp.value
         FROM customer_properties cp
         LEFT JOIN customer_property_definitions d ON d.id = cp.definition_id
         WHERE cp.customer_id = ?1
         ORDER BY 1 LIMIT 50",
    )?;
    let properties: Vec<Value> = stmt
        .query_map(params![customer_id], |r| {
            Ok(json!({
                "name": r.get::<_, String>(0)?,
                "value": r.get::<_, Option<String>>(1)?,
            }))
        })?
        .filter_map(|r| r.ok())
        .collect();

    let websites: Vec<Value> = string_column(
        conn,
        "SELECT value FROM customer_websites WHERE customer_id = ?1 ORDER BY id LIMIT 20",
        customer_id,
    )?;
    let social_profiles: Vec<Value> = conn
        .prepare(
            "SELECT type, value FROM customer_social_profiles
             WHERE customer_id = ?1 ORDER BY id LIMIT 20",
        )?
        .query_map(params![customer_id], |r| {
            Ok(json!({
                "type": r.get::<_, Option<String>>(0)?,
                "value": r.get::<_, Option<String>>(1)?,
            }))
        })?
        .filter_map(|r| r.ok())
        .collect();
    let address: Option<String> = conn
        .prepare(
            "SELECT lines, city, state, postal_code, country
             FROM customer_addresses WHERE customer_id = ?1 ORDER BY id LIMIT 1",
        )?
        .query_row(params![customer_id], |r| {
            let parts = [
                r.get::<_, Option<String>>(0)?,
                r.get::<_, Option<String>>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, Option<String>>(3)?,
                r.get::<_, Option<String>>(4)?,
            ];
            let joined = parts
                .iter()
                .flatten()
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join(", ");
            Ok(if joined.is_empty() {
                None
            } else {
                Some(joined)
            })
        })
        .unwrap_or(None);

    let topics: Vec<Value> = conversations
        .iter()
        .take(10)
        .map(|c| {
            json!({
                "number": c.get("number").cloned().unwrap_or(Value::Null),
                "topic": c.get("subject").cloned().unwrap_or(Value::Null),
            })
        })
        .collect();

    let mut stmt = conn.prepare(
        "SELECT cv.number, cv.subject,
                (SELECT t.body_text FROM conversation_threads t
                  WHERE t.conversation_id = cv.id
                    AND (t.type = 'reply'
                         OR (t.type IN ('message','chat','phone')
                             AND t.from_type IN ('user','agent')))
                    AND t.state = 'published' AND t.deleted_at IS NULL
                  ORDER BY COALESCE(t.remote_created_at, t.created_at) DESC LIMIT 1) AS resolution,
                cv.closed_at
         FROM conversations cv
         WHERE cv.customer_local_id = ?1 AND cv.status = 'closed' AND cv.deleted_at IS NULL
         ORDER BY cv.closed_at DESC LIMIT 5",
    )?;
    let resolutions: Vec<Value> = stmt
        .query_map(params![customer_id], |r| {
            let resolution: Option<String> = r.get(2)?;
            Ok(json!({
                "number": r.get::<_, i64>(0)?,
                "subject": r.get::<_, Option<String>>(1)?,
                "resolution": resolution.map(|res| res.chars().take(400).collect::<String>()),
                "closed_at": r.get::<_, Option<String>>(3)?,
            }))
        })?
        .filter_map(|r| r.ok())
        .collect();

    Ok(Some(json!({
        "customer": customer,
        "conversations": conversations,
        "ratings": ratings,
        "memories": memories,
        "properties": properties,
        "websites": websites,
        "social_profiles": social_profiles,
        "address": address,
        "topics": topics,
        "resolutions": resolutions,
    })))
}

fn string_column(conn: &Connection, sql: &str, id: i64) -> Result<Vec<Value>> {
    let mut stmt = conn.prepare(sql)?;
    let rows: Vec<Value> = stmt
        .query_map(params![id], |r| Ok(json!(r.get::<_, Option<String>>(0)?)))?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}

pub fn customer_support_health(conn: &Connection, customer_id: i64) -> Result<Option<Value>> {
    let row: Option<(i64, String)> = conn
        .query_row(
            "SELECT id, TRIM(COALESCE(first_name, '') || ' ' || COALESCE(last_name, ''))
             FROM customers WHERE id = ?1 AND deleted_at IS NULL",
            params![customer_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .ok();
    let Some((id, raw_label)) = row else {
        return Ok(None);
    };
    let label = raw_label.trim();
    let label = if label.is_empty() {
        format!("customer #{customer_id}")
    } else {
        label.to_string()
    };
    build_support_health_report(
        conn,
        "customer",
        id,
        &label,
        "c.customer_local_id = ?1",
        "r.customer_local_id = ?1",
        "c.customer_local_id = ?1",
    )
    .map(Some)
}

fn next_local_remote_id(conn: &Connection, table: &str) -> Result<i64> {
    let next: i64 = conn.query_row(
        &format!("SELECT COALESCE(MIN(remote_id), 0) - 1 FROM {table} WHERE remote_id < 0"),
        [],
        |r| r.get(0),
    )?;
    debug_assert!(next < 0, "local remote_id allocation must stay negative");
    Ok(next)
}

pub fn create_organization(conn: &Connection, name: &str, domains: &[String]) -> Result<i64> {
    let remote_id = next_local_remote_id(conn, "organizations")?;
    let domains_json = serde_json::to_string(domains).unwrap_or_else(|_| "[]".into());
    conn.execute(
        "INSERT INTO organizations (remote_id, name, domains, last_synced_at)
         VALUES (?1, ?2, ?3, datetime('now'))",
        params![remote_id, name, domains_json],
    )?;
    let id = conn.last_insert_rowid();
    let _ = crate::audit::audit(
        conn,
        &crate::audit::AuditEntry::user("organization_created").with_after_state(json!({
            "organization_id": id,
            "remote_id": remote_id,
            "name": name,
            "domains": domains,
        })),
    );
    Ok(id)
}

pub fn update_organization(
    conn: &Connection,
    org_id: i64,
    name: &str,
    domains: &[String],
) -> Result<bool> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(crate::error::Error::Validation(
            "Organization name is required".to_string(),
        ));
    }
    let domains_json = serde_json::to_string(domains).unwrap_or_else(|_| "[]".into());
    let changed = conn.execute(
        "UPDATE organizations
            SET name = ?1, domains = ?2, local_updated_at = datetime('now')
          WHERE id = ?3 AND deleted_at IS NULL",
        params![trimmed, domains_json, org_id],
    )?;
    if changed > 0 {
        let _ = crate::audit::audit(
            conn,
            &crate::audit::AuditEntry::user("organization_updated").with_after_state(json!({
                "organization_id": org_id,
                "name": trimmed,
                "domains": domains,
            })),
        );
    }
    Ok(changed > 0)
}

pub fn delete_organization(conn: &Connection, org_id: i64) -> Result<bool> {
    let changed = conn.execute(
        "UPDATE organizations SET deleted_at = datetime('now')
          WHERE id = ?1 AND deleted_at IS NULL",
        params![org_id],
    )?;
    if changed > 0 {
        let _ = crate::audit::audit(
            conn,
            &crate::audit::AuditEntry::user("organization_deleted")
                .with_after_state(json!({ "organization_id": org_id })),
        );
    }
    Ok(changed > 0)
}

#[cfg(test)]
mod batch29_audit103_104_105_tests {
    use super::*;
    use rusqlite::Connection;

    fn test_db() -> Connection {
        let mut conn = Connection::open_in_memory().expect("db");
        crate::bootstrap::apply_all(&mut conn).expect("migrations");
        conn
    }

    #[test]
    fn audit103_member_counts_are_exact_past_the_display_cap() {
        let conn = test_db();
        conn.execute(
            "INSERT INTO organizations (remote_id, name) VALUES (700, 'BigCorp')",
            [],
        )
        .expect("org");
        let org = conn.last_insert_rowid();
        for i in 0..120 {
            conn.execute(
                "INSERT INTO customers (remote_id, first_name, organization_id)
                 VALUES (?1, ?2, ?3)",
                params![900 + i, format!("m{i}"), org],
            )
            .expect("customer");
            let customer = conn.last_insert_rowid();
            conn.execute(
                "INSERT INTO conversations (remote_id, number, subject, status, customer_local_id)
                 VALUES (?1, ?2, 'open issue', 'active', ?3)",
                params![9100 + i, 1000 + i, customer],
            )
            .expect("conversation");
        }
        let detail = get_organization_detail(&conn, org)
            .expect("detail")
            .expect("org exists");
        assert_eq!(
            detail.get("member_count").and_then(Value::as_i64),
            Some(120),
            "member_count must be the exact member total, not the 100-row display cap"
        );
        assert_eq!(
            detail
                .get("members")
                .and_then(Value::as_array)
                .map(Vec::len),
            Some(100),
            "the members list stays a display list"
        );
        assert_eq!(
            detail
                .get("open_conversation_count")
                .and_then(Value::as_i64),
            Some(120),
            "the open total must sum every member, not the first page"
        );
        assert_eq!(
            detail.get("conversation_count").and_then(Value::as_i64),
            Some(120),
            "the conversation total must sum every member, not the first page"
        );
    }

    #[test]
    fn audit104_deleted_conversations_do_not_drive_last_activity() {
        let conn = test_db();
        conn.execute(
            "INSERT INTO customers (remote_id, first_name, last_name)
             VALUES (80, 'Ada', 'Lovelace')",
            [],
        )
        .expect("customer");
        let customer = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO conversations (remote_id, number, subject, status, customer_local_id,
                                        last_activity_at)
             VALUES (9501, 95, 'Live ticket', 'active', ?1, '2026-10-23T10:00:00Z')",
            params![customer],
        )
        .expect("live conversation");
        conn.execute(
            "INSERT INTO conversations (remote_id, number, subject, status, customer_local_id,
                                        last_activity_at, deleted_at)
             VALUES (9502, 96, 'Deleted ticket', 'active', ?1, '2026-10-24T09:00:00Z',
                     datetime('now'))",
            params![customer],
        )
        .expect("deleted conversation");
        let (rows, total) = list_customers_summary(&conn, None, 1, 50).expect("summary");
        assert_eq!(total, 1);
        let last = rows[0]
            .get("last_activity_at")
            .cloned()
            .and_then(|v| v.as_str().map(str::to_string));
        assert_eq!(
            last.as_deref(),
            Some("2026-10-23T10:00:00Z"),
            "the deleted conversation's newer stamp must not shadow the live one: {last:?}"
        );
    }

    #[test]
    fn audit104_deleted_threads_do_not_serve_as_resolutions() {
        let conn = test_db();
        conn.execute(
            "INSERT INTO customers (remote_id, first_name, last_name)
             VALUES (81, 'Alan', 'Turing')",
            [],
        )
        .expect("customer");
        let customer = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO conversations (remote_id, number, subject, status, customer_local_id)
             VALUES (9601, 96, 'Refund please', 'closed', ?1)",
            params![customer],
        )
        .expect("closed conversation");
        let conversation = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO conversation_threads (conversation_id, type, state, body_text,
                                               created_at)
             VALUES (?1, 'reply', 'published', 'Refunded in full.', '2026-10-20T10:00:00Z')",
            params![conversation],
        )
        .expect("live reply");
        conn.execute(
            "INSERT INTO conversation_threads (conversation_id, type, state, body_text,
                                               created_at, deleted_at)
             VALUES (?1, 'reply', 'published', 'WRONG ANSWER (deleted)', '2026-10-21T10:00:00Z',
                     datetime('now'))",
            params![conversation],
        )
        .expect("deleted reply");
        let detail = get_customer_detail(&conn, customer)
            .expect("detail")
            .expect("customer exists");
        let resolutions = detail
            .get("resolutions")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        assert_eq!(resolutions.len(), 1, "one closed conversation");
        let resolution = resolutions[0]
            .get("resolution")
            .cloned()
            .and_then(|v| v.as_str().map(str::to_string));
        assert_eq!(
            resolution.as_deref(),
            Some("Refunded in full."),
            "the deleted thread must not speak for the resolution: {resolution:?}"
        );
    }

    #[test]
    fn audit105_org_name_linking_is_case_insensitive() {
        let conn = test_db();
        conn.execute(
            "INSERT INTO organizations (remote_id, name) VALUES (700, 'Acme Corp')",
            [],
        )
        .expect("org");
        let org = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO customers (remote_id, first_name, last_name, organization)
             VALUES (-900, 'Ada', 'Lovelace', 'acme corp')",
            [],
        )
        .expect("customer");
        let id = conn.last_insert_rowid();
        let linked: Option<i64> = conn
            .query_row(
                "SELECT organization_id FROM customers WHERE id = ?1",
                params![id],
                |r| r.get(0),
            )
            .expect("row");
        assert_eq!(
            linked, None,
            "a raw insert does not id-link; the read-time name match is what counts"
        );

        conn.execute(
            "INSERT INTO customers (remote_id, first_name, last_name, organization)
             VALUES (90, 'Grace', 'Hopper', 'ACME CORP')",
            [],
        )
        .expect("fallback customer");
        let detail = get_organization_detail(&conn, org)
            .expect("detail")
            .expect("org exists");
        assert_eq!(
            detail.get("member_count").and_then(Value::as_i64),
            Some(2),
            "both customers count through the case-insensitive name match"
        );

        let orgs = list_organizations(&conn, None, 200).expect("orgs");
        assert_eq!(orgs.len(), 1);
        assert_eq!(
            orgs[0].get("customer_count").and_then(Value::as_i64),
            Some(2)
        );
    }
}

#[cfg(test)]
mod audp2_tests {
    use super::*;
    use rusqlite::Connection;

    fn conn() -> Connection {
        let mut c = Connection::open_in_memory().expect("db");
        crate::bootstrap::apply_all(&mut c).expect("migrations");
        c
    }

    #[test]
    fn audp2_update_and_delete_organization_round_trip() {
        let c = conn();
        let org = create_organization(&c, "Acme", &["acme.com".into()]).expect("org");

        assert!(
            update_organization(
                &c,
                org,
                "Acme Industries",
                &["acme.com".into(), "acme.io".into()]
            )
            .expect("update")
        );
        let (name, domains): (String, String) = c
            .query_row(
                "SELECT name, domains FROM organizations WHERE id = ?1",
                rusqlite::params![org],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .expect("row");
        assert_eq!(name, "Acme Industries");
        assert!(domains.contains("acme.io"), "{domains}");

        let err = update_organization(&c, org, "   ", &[]).expect_err("name guard");
        assert!(err.to_string().contains("required"), "{err}");

        assert!(delete_organization(&c, org).expect("delete"));
        let gone: i64 = c
            .query_row(
                "SELECT COUNT(*) FROM organizations WHERE id = ?1 AND deleted_at IS NULL",
                rusqlite::params![org],
                |r| r.get(0),
            )
            .expect("count");
        assert_eq!(gone, 0);
        assert!(!delete_organization(&c, org).expect("delete again"));
        assert!(!update_organization(&c, org, "X", &[]).expect("update gone"));
    }
}
