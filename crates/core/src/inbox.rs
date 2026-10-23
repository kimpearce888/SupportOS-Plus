use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};

use crate::error::Result;

pub fn apply_m028(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS conversation_threads (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            conversation_id INTEGER NOT NULL,
            thread_type     TEXT NOT NULL,
            body            TEXT,
            actor_type      TEXT NOT NULL,
            actor_id        INTEGER,
            created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
        );
        CREATE INDEX IF NOT EXISTS idx_conv_threads_conv
            ON conversation_threads (conversation_id, created_at);
        CREATE INDEX IF NOT EXISTS idx_conv_threads_type
            ON conversation_threads (thread_type);",
    )?;
    let _ = conn.execute("UPDATE app_state SET schema_version = 28 WHERE id = 1", []);
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThreadType {
    CustomerMessage,
    Reply,
    Note,
    System,
}

impl ThreadType {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::CustomerMessage => "customer_message",
            Self::Reply => "reply",
            Self::Note => "note",
            Self::System => "system",
        }
    }

    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "customer_message" => Some(Self::CustomerMessage),
            "reply" => Some(Self::Reply),
            "note" => Some(Self::Note),
            "system" => Some(Self::System),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationListItem {
    pub id: i64,
    pub remote_id: i64,
    pub number: i64,
    pub subject: Option<String>,
    pub preview: Option<String>,
    pub status: String,
    pub mailbox_id: Option<i64>,
    pub mailbox_name: Option<String>,
    pub assignee_id: Option<i64>,
    pub assignee_name: Option<String>,
    /// Clean-audit item 33: team assignments — the team the platform
    /// holds when no individual user is assigned.
    pub assigned_team_id: Option<i64>,
    pub assigned_team_name: Option<String>,
    pub customer_id: Option<i64>,
    pub customer_name: Option<String>,
    pub priority: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub closed_at: Option<String>,
    pub response_state: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct InboxFilters {
    pub status: Option<String>,
    pub mailbox_id: Option<i64>,
    pub assignee_id: Option<i64>,
    pub customer_id: Option<i64>,
    pub priority: Option<String>,
    pub channel: Option<String>,
    pub tag: Option<String>,
    pub query: Option<String>,
    pub number: Option<i64>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
    #[serde(skip)]
    pub extra_where: Option<String>,
    #[serde(skip)]
    pub extra_params: Vec<rusqlite::types::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadEntry {
    pub id: i64,
    pub conversation_id: i64,
    pub thread_type: String,
    pub body: Option<String>,
    pub actor_type: String,
    pub actor_id: Option<i64>,
    pub actor_name: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationDetail {
    pub id: i64,
    pub remote_id: i64,
    pub number: i64,
    pub subject: Option<String>,
    pub preview: Option<String>,
    pub status: String,
    pub mailbox_id: Option<i64>,
    pub mailbox_name: Option<String>,
    pub assignee_id: Option<i64>,
    pub assignee_name: Option<String>,
    /// Clean-audit item 33: team assignments — the team the platform
    /// holds when no individual user is assigned.
    pub assigned_team_id: Option<i64>,
    pub assigned_team_name: Option<String>,
    pub customer_id: Option<i64>,
    pub customer_name: Option<String>,
    pub customer_email: Option<String>,
    pub priority: Option<String>,
    pub response_state: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub closed_at: Option<String>,
    pub thread: Vec<ThreadEntry>,
    pub tags: Vec<String>,
}

pub fn list_conversations(
    conn: &Connection,
    filters: &InboxFilters,
) -> Result<(Vec<ConversationListItem>, u32)> {
    let limit = filters.limit.unwrap_or(50).min(200);
    let offset = filters.offset.unwrap_or(0);

    let mut where_parts: Vec<String> = Vec::new();
    let mut params_vec: Vec<rusqlite::types::Value> = Vec::new();

    where_parts.push("c.deleted_at IS NULL".to_string());
    where_parts.push("c.merged_into_conversation_id IS NULL".to_string());

    if let Some(ref status) = filters.status {
        where_parts.push("c.status = ?".to_string());
        params_vec.push(status.clone().into());
    }
    if let Some(mailbox_id) = filters.mailbox_id {
        where_parts.push("c.mailbox_local_id = ?".to_string());
        params_vec.push(mailbox_id.into());
    }
    if let Some(assignee_id) = filters.assignee_id {
        where_parts.push("c.assignee_local_id = ?".to_string());
        params_vec.push(assignee_id.into());
    }
    if let Some(customer_id) = filters.customer_id {
        where_parts.push("c.customer_local_id = ?".to_string());
        params_vec.push(customer_id.into());
    }
    if let Some(ref priority) = filters.priority {
        where_parts.push("c.priority = ?".to_string());
        params_vec.push(priority.clone().into());
    }
    if let Some(ref channel) = filters.channel {
        where_parts.push("c.type = ?".to_string());
        params_vec.push(channel.clone().into());
    }
    if let Some(ref tag) = filters.tag {
        where_parts.push(
            "EXISTS (SELECT 1 FROM conversation_tags ct JOIN tags t \
             ON t.id = ct.tag_id WHERE ct.conversation_id = c.id \
             AND t.name = ? COLLATE NOCASE)"
                .to_string(),
        );
        params_vec.push(tag.clone().into());
    }
    if let Some(number) = filters.number {
        where_parts.push("c.number = ?".to_string());
        params_vec.push(number.into());
    }
    if let Some(ref query) = filters.query {
        let q = format!("%{query}%");
        where_parts
            .push("(c.subject LIKE ? ESCAPE '\\' OR c.preview LIKE ? ESCAPE '\\')".to_string());
        params_vec.push(q.clone().into());
        params_vec.push(q.into());
    }
    if let Some(ref extra) = filters.extra_where {
        where_parts.push(format!("({extra})"));
        params_vec.extend(filters.extra_params.iter().cloned());
    }

    let where_clause = if where_parts.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", where_parts.join(" AND "))
    };

    let count_sql = format!("SELECT COUNT(*) FROM conversations c {where_clause}");
    let total: i64 = conn
        .query_row(&count_sql, params_from_iter_ref(&params_vec), |r| r.get(0))
        .unwrap_or(0);
    let total = u32::try_from(total).unwrap_or(0);

    let list_sql = format!(
        "SELECT c.id, c.remote_id, c.number, c.subject, c.preview, c.status,
                c.mailbox_local_id, m.name,
                c.assignee_local_id, u.first_name || ' ' || u.last_name,
                c.assigned_team_local_id, tm.name,
                c.customer_local_id, cu.first_name || ' ' || cu.last_name,
                c.priority, c.created_at, c.updated_at, c.closed_at,
                c.response_state
         FROM conversations c
         LEFT JOIN mailboxes m ON m.id = c.mailbox_local_id
         LEFT JOIN users u ON u.id = c.assignee_local_id
         LEFT JOIN teams tm ON tm.id = c.assigned_team_local_id
         LEFT JOIN customers cu ON cu.id = c.customer_local_id
         {where_clause}
         ORDER BY COALESCE(c.updated_at, c.local_created_at) DESC
         LIMIT ? OFFSET ?"
    );

    let mut params_with_pagination = params_vec.clone();
    params_with_pagination.push(i64::from(limit).into());
    params_with_pagination.push(i64::from(offset).into());

    let mut stmt = conn.prepare(&list_sql)?;
    let items: Result<Vec<ConversationListItem>> = stmt
        .query_map(params_from_iter_ref(&params_with_pagination), |row| {
            Ok(ConversationListItem {
                id: row.get(0)?,
                remote_id: row.get(1)?,
                number: row.get(2)?,
                subject: row.get(3)?,
                preview: row.get(4)?,
                status: row.get(5)?,
                mailbox_id: row.get(6)?,
                mailbox_name: row.get(7)?,
                assignee_id: row.get(8)?,
                assignee_name: row.get(9)?,
                assigned_team_id: row.get(10)?,
                assigned_team_name: row.get(11)?,
                customer_id: row.get(12)?,
                customer_name: row.get(13)?,
                priority: row.get(14)?,
                created_at: row.get(15)?,
                updated_at: row.get(16)?,
                closed_at: row.get(17)?,
                response_state: row.get::<_, Option<String>>(18).ok().flatten(),
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| e.into());
    let items = items?;

    Ok((items, total))
}

pub fn get_conversation(
    conn: &Connection,
    conversation_id: i64,
) -> Result<Option<ConversationDetail>> {
    let row = conn.query_row(
        "SELECT c.id, c.remote_id, c.number, c.subject, c.preview, c.status,
                    c.mailbox_local_id, m.name,
                    c.assignee_local_id, u.first_name || ' ' || u.last_name,
                    c.assigned_team_local_id, tm.name,
                    c.customer_local_id, cu.first_name || ' ' || cu.last_name, cu.email,
                    c.priority, c.response_state,
                    c.created_at, c.updated_at, c.closed_at
             FROM conversations c
             LEFT JOIN mailboxes m ON m.id = c.mailbox_local_id
             LEFT JOIN users u ON u.id = c.assignee_local_id
             LEFT JOIN teams tm ON tm.id = c.assigned_team_local_id
             LEFT JOIN customers cu ON cu.id = c.customer_local_id
             WHERE c.id = ?",
        params![conversation_id],
        |row| {
            Ok(ConversationDetail {
                id: row.get(0)?,
                remote_id: row.get(1)?,
                number: row.get(2)?,
                subject: row.get(3)?,
                preview: row.get(4)?,
                status: row.get(5)?,
                mailbox_id: row.get(6)?,
                mailbox_name: row.get(7)?,
                assignee_id: row.get(8)?,
                assignee_name: row.get(9)?,
                assigned_team_id: row.get(10)?,
                assigned_team_name: row.get(11)?,
                customer_id: row.get(12)?,
                customer_name: row.get(13)?,
                customer_email: row.get(14)?,
                priority: row.get(15)?,
                response_state: row.get::<_, Option<String>>(16).ok().flatten(),
                created_at: row.get(17)?,
                updated_at: row.get(18)?,
                closed_at: row.get(19)?,
                thread: Vec::new(),
                tags: Vec::new(),
            })
        },
    );
    let mut detail = match row {
        Ok(d) => d,
        Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(None),
        Err(e) => return Err(e.into()),
    };

    let mut stmt = conn.prepare(
        "SELECT t.id, t.conversation_id, t.type, t.body_text,
                t.from_type,
                COALESCE(t.created_by_user_id, t.created_by_customer_id,
                         t.created_by_system_user_id),
                CASE WHEN t.created_by_user_id IS NOT NULL
                     THEN u.first_name || ' ' || u.last_name
                     WHEN t.created_by_customer_id IS NOT NULL
                     THEN cu.first_name || ' ' || cu.last_name
                     ELSE NULL END,
                t.created_at
         FROM conversation_threads t
         LEFT JOIN users u ON u.id = t.created_by_user_id
         LEFT JOIN customers cu ON cu.id = t.created_by_customer_id
         WHERE t.conversation_id = ?
         ORDER BY t.created_at ASC",
    )?;
    let thread: Result<Vec<ThreadEntry>> = stmt
        .query_map(params![conversation_id], |row| {
            Ok(ThreadEntry {
                id: row.get(0)?,
                conversation_id: row.get(1)?,
                thread_type: row.get(2)?,
                body: row.get(3)?,
                actor_type: row.get(4)?,
                actor_id: row.get(5)?,
                actor_name: row.get(6)?,
                created_at: row.get(7)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| e.into());
    detail.thread = thread?;

    Ok(Some(detail))
}

fn params_from_iter_ref(values: &[rusqlite::types::Value]) -> impl rusqlite::Params + use<'_> {
    rusqlite::params_from_iter(values.iter().cloned())
}
