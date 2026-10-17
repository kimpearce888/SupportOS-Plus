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
pub struct CustomerConversation {
    pub id: i64,
    pub remote_id: i64,
    pub number: i64,
    pub subject: Option<String>,
    pub status: String,
    pub mailbox_id: Option<i64>,
    pub mailbox_name: Option<String>,
    pub priority: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub closed_at: Option<String>,
    pub response_state: Option<String>,
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

pub fn get_customer(conn: &Connection, customer_id: i64) -> Result<Option<Customer>> {
    let row = conn.query_row(
        "SELECT id, remote_id, first_name, last_name, email, organization,
                job_title, phone, created_at, updated_at
         FROM customers WHERE id = ?",
        params![customer_id],
        |row| {
            Ok(Customer {
                id: row.get(0)?,
                remote_id: row.get(1)?,
                first_name: row.get(2)?,
                last_name: row.get(3)?,
                email: row.get(4)?,
                organization: row.get(5)?,
                job_title: row.get(6)?,
                phone: row.get(7)?,
                created_at: row.get(8)?,
                updated_at: row.get(9)?,
            })
        },
    );
    match row {
        Ok(c) => Ok(Some(c)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

pub fn list_customer_conversations(
    conn: &Connection,
    customer_id: i64,
    limit: Option<u32>,
) -> Result<Vec<CustomerConversation>> {
    let limit = limit.unwrap_or(50).min(200);
    let mut stmt = conn.prepare(
        "SELECT c.id, c.remote_id, c.number, c.subject, c.status,
                c.mailbox_local_id, m.name, c.priority,
                c.created_at, c.updated_at, c.closed_at, c.response_state
         FROM conversations c
         LEFT JOIN mailboxes m ON m.id = c.mailbox_local_id
         WHERE c.customer_local_id = ?
         ORDER BY COALESCE(c.updated_at, c.local_created_at) DESC
         LIMIT ?",
    )?;
    let items: Result<Vec<CustomerConversation>> = stmt
        .query_map(params![customer_id, i64::from(limit)], |row| {
            Ok(CustomerConversation {
                id: row.get(0)?,
                remote_id: row.get(1)?,
                number: row.get(2)?,
                subject: row.get(3)?,
                status: row.get(4)?,
                mailbox_id: row.get(5)?,
                mailbox_name: row.get(6)?,
                priority: row.get(7)?,
                created_at: row.get(8)?,
                updated_at: row.get(9)?,
                closed_at: row.get(10)?,
                response_state: row.get::<_, Option<String>>(11).ok().flatten(),
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| e.into());
    items
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

pub fn search_customers(
    conn: &Connection,
    query: &str,
    limit: Option<u32>,
) -> Result<Vec<Customer>> {
    let limit = limit.unwrap_or(20).min(100);
    let q = format!("%{query}%");
    let mut stmt = conn.prepare(
        "SELECT id, remote_id, first_name, last_name, email, organization,
                job_title, phone, created_at, updated_at
         FROM customers
         WHERE first_name LIKE ? ESCAPE '\\'
            OR last_name LIKE ? ESCAPE '\\'
            OR email LIKE ? ESCAPE '\\'
            OR organization LIKE ? ESCAPE '\\'
         ORDER BY COALESCE(updated_at, local_created_at) DESC
         LIMIT ?",
    )?;
    let items: Result<Vec<Customer>> = stmt
        .query_map(rusqlite::params![&q, &q, &q, &q, i64::from(limit)], |row| {
            Ok(Customer {
                id: row.get(0)?,
                remote_id: row.get(1)?,
                first_name: row.get(2)?,
                last_name: row.get(3)?,
                email: row.get(4)?,
                organization: row.get(5)?,
                job_title: row.get(6)?,
                phone: row.get(7)?,
                created_at: row.get(8)?,
                updated_at: row.get(9)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| e.into());
    items
}
