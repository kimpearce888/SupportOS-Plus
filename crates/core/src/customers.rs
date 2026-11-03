use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};

use crate::error::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Customer {
    pub id: i64,
    pub remote_id: i64,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub email: Option<String>,
    pub organization: Option<String>,
    pub job_title: Option<String>,
    pub phone: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomerTimelineEntry {
    pub id: i64,
    pub conversation_id: i64,
    pub conversation_number: i64,
    pub event_type: String,
    pub body: Option<String>,
    pub actor_type: String,
    pub actor_name: Option<String>,
    pub occurred_at: String,
}

pub fn customer_timeline(
    conn: &Connection,
    customer_id: i64,
    limit: Option<u32>,
) -> Result<Vec<CustomerTimelineEntry>> {
    let limit = limit.unwrap_or(100).min(500);

    let mut stmt = conn.prepare(
        "SELECT t.id, t.conversation_id, c.number, t.type,
                t.body_text, t.from_type,
                CASE WHEN t.created_by_user_id IS NOT NULL
                     THEN u.first_name || ' ' || u.last_name
                     WHEN t.created_by_customer_id IS NOT NULL
                     THEN cu.first_name || ' ' || cu.last_name
                     ELSE NULL END,
                t.created_at
         FROM conversation_threads t
         JOIN conversations c ON c.id = t.conversation_id
         LEFT JOIN users u ON u.id = t.created_by_user_id
         LEFT JOIN customers cu ON cu.id = t.created_by_customer_id
         WHERE c.customer_local_id = ?
         ORDER BY t.created_at DESC
         LIMIT ?",
    )?;
    let entries: Result<Vec<CustomerTimelineEntry>> = stmt
        .query_map(params![customer_id, i64::from(limit)], |row| {
            Ok(CustomerTimelineEntry {
                id: row.get(0)?,
                conversation_id: row.get(1)?,
                conversation_number: row.get(2)?,
                event_type: row.get(3)?,
                body: row.get(4)?,
                actor_type: row.get(5)?,
                actor_name: row.get(6)?,
                occurred_at: row.get(7)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| e.into());
    entries
}
