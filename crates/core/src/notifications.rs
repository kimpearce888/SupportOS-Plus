use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};

use crate::catalog::NotificationType;
use crate::error::Result;

pub const M005_SQL: &str = r#"
    CREATE TABLE IF NOT EXISTS notifications (
        id              INTEGER PRIMARY KEY AUTOINCREMENT,
        type            TEXT NOT NULL,
        severity        TEXT NOT NULL DEFAULT 'info',
        title           TEXT NOT NULL DEFAULT '',
        body            TEXT,
        target_user_id  INTEGER,
        actor_user_local_id INTEGER,
        conversation_id INTEGER,
        conversation_number INTEGER,
        customer_local_id   INTEGER,
        issue_id        INTEGER,
        campaign_id     INTEGER,
        job_id          INTEGER,
        side_thread_id  INTEGER,
        dedup_key       TEXT NOT NULL DEFAULT '',
        payload         TEXT,
        read_at         TEXT,
        created_at      TEXT NOT NULL DEFAULT (datetime('now'))
    );
    CREATE UNIQUE INDEX IF NOT EXISTS idx_notifications_dedup
        ON notifications (dedup_key);
    CREATE INDEX IF NOT EXISTS idx_notifications_target_unread
        ON notifications (target_user_id, read_at);
    CREATE INDEX IF NOT EXISTS idx_notifications_type_created_at
        ON notifications (type, created_at);
    CREATE INDEX IF NOT EXISTS idx_notifications_created_at
        ON notifications (created_at);

    UPDATE app_state SET schema_version = 5 WHERE id = 1;
"#;

pub fn apply_m005(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS notifications (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            type            TEXT NOT NULL,
            severity        TEXT NOT NULL DEFAULT 'info',
            title           TEXT NOT NULL DEFAULT '',
            body            TEXT,
            target_user_id  INTEGER,
            actor_user_local_id INTEGER,
            conversation_id INTEGER,
            conversation_number INTEGER,
            customer_local_id   INTEGER,
            issue_id        INTEGER,
            campaign_id     INTEGER,
            job_id          INTEGER,
            side_thread_id  INTEGER,
            dedup_key       TEXT NOT NULL DEFAULT '',
            payload         TEXT,
            read_at         TEXT,
            created_at      TEXT NOT NULL DEFAULT (datetime('now'))
        );
        CREATE UNIQUE INDEX IF NOT EXISTS idx_notifications_dedup
            ON notifications (dedup_key);
        CREATE INDEX IF NOT EXISTS idx_notifications_target_unread
            ON notifications (target_user_id, read_at);
        CREATE INDEX IF NOT EXISTS idx_notifications_type_created_at
            ON notifications (type, created_at);
        CREATE INDEX IF NOT EXISTS idx_notifications_created_at
            ON notifications (created_at);",
    )?;
    for (column, decl) in [
        ("title", "TEXT NOT NULL DEFAULT ''"),
        ("body", "TEXT"),
        ("actor_user_local_id", "INTEGER"),
        ("conversation_number", "INTEGER"),
        ("customer_local_id", "INTEGER"),
        ("issue_id", "INTEGER"),
        ("campaign_id", "INTEGER"),
        ("job_id", "INTEGER"),
        ("side_thread_id", "INTEGER"),
        ("dedup_key", "TEXT NOT NULL DEFAULT ''"),
    ] {
        let sql = format!("ALTER TABLE notifications ADD COLUMN {column} {decl}");
        if let Err(e) = conn.execute(&sql, []) {
            let msg = e.to_string();
            if !msg.contains("duplicate column name") {
                return Err(crate::error::Error::Sqlite(e));
            }
        }
    }
    let _ = conn.execute("UPDATE app_state SET schema_version = 5 WHERE id = 1", []);
    Ok(())
}

#[derive(Debug, Clone)]
pub struct NotificationInput<'a> {
    pub notification_type: NotificationType,
    pub severity: Option<&'a str>,
    pub title: String,
    pub body: Option<String>,
    pub target_user_local_id: Option<i64>,
    pub actor_user_local_id: Option<i64>,
    pub conversation_id: Option<i64>,
    pub conversation_number: Option<i64>,
    pub customer_local_id: Option<i64>,
    pub issue_id: Option<i64>,
    pub campaign_id: Option<i64>,
    pub job_id: Option<i64>,
    pub side_thread_id: Option<i64>,
    pub dedup_key: String,
}

impl Default for NotificationInput<'_> {
    fn default() -> Self {
        Self {
            notification_type: NotificationType::CustomerReplied,
            severity: None,
            title: String::new(),
            body: None,
            target_user_local_id: None,
            actor_user_local_id: None,
            conversation_id: None,
            conversation_number: None,
            customer_local_id: None,
            issue_id: None,
            campaign_id: None,
            job_id: None,
            side_thread_id: None,
            dedup_key: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NotificationRecord {
    pub id: i64,
    #[serde(rename = "type")]
    pub notification_type: NotificationType,
    pub severity: String,
    pub title: String,
    pub body: Option<String>,
    pub target_user_local_id: Option<i64>,
    pub actor_user_local_id: Option<i64>,
    pub conversation_id: Option<i64>,
    pub conversation_number: Option<i64>,
    pub customer_local_id: Option<i64>,
    pub issue_id: Option<i64>,
    pub campaign_id: Option<i64>,
    pub job_id: Option<i64>,
    pub side_thread_id: Option<i64>,
    pub created_at: String,
    pub read_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NotificationListResult {
    pub notifications: Vec<NotificationRecord>,
    pub total: i64,
    pub unread: i64,
}

fn parse_type(type_str: &str) -> Option<NotificationType> {
    NotificationType::ALL
        .iter()
        .copied()
        .find(|t| t.as_str() == type_str)
}

pub fn me_user_local_id(conn: &Connection) -> Result<Option<i64>> {
    let raw = crate::settings::get_string(conn, "me_remote_id")?;
    if let Some(raw) = raw {
        let trimmed = raw.trim_matches('"');
        if let Ok(remote_id) = trimmed.parse::<i64>() {
            let row: Option<i64> = conn
                .query_row(
                    "SELECT id FROM users WHERE remote_id = ?1",
                    params![remote_id],
                    |r| r.get(0),
                )
                .ok();
            if let Some(id) = row {
                return Ok(Some(id));
            }
        }
    }
    let fallback: Option<i64> = conn
        .query_row(
            "SELECT id FROM users WHERE deleted_at IS NULL ORDER BY id LIMIT 1",
            [],
            |r| r.get(0),
        )
        .ok();
    if fallback.is_none() {
        return Ok(conn
            .query_row("SELECT id FROM users ORDER BY id LIMIT 1", [], |r| r.get(0))
            .ok());
    }
    Ok(fallback)
}

pub fn pref_for(conn: &Connection, notif_type: NotificationType) -> Result<bool> {
    let key = format!("notifications.{}.enabled", notif_type.as_str());
    crate::settings::get_bool(conn, &key, notif_type.default_enabled())
}

pub fn insert(
    conn: &Connection,
    input: &NotificationInput<'_>,
) -> Result<Option<NotificationRecord>> {
    let severity = input
        .severity
        .unwrap_or_else(|| input.notification_type.severity());
    let inserted = conn.execute(
        "INSERT INTO notifications
            (type, severity, title, body, target_user_id, actor_user_local_id,
             conversation_id, conversation_number, customer_local_id, issue_id,
             campaign_id, job_id, side_thread_id, dedup_key)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
         ON CONFLICT(dedup_key) DO NOTHING",
        params![
            input.notification_type.as_str(),
            severity,
            truncate_chars(&input.title, 300),
            input.body.as_deref().map(|b| truncate_chars(b, 2000)),
            input.target_user_local_id,
            input.actor_user_local_id,
            input.conversation_id,
            input.conversation_number,
            input.customer_local_id,
            input.issue_id,
            input.campaign_id,
            input.job_id,
            input.side_thread_id,
            input.dedup_key,
        ],
    )?;
    if inserted == 0 {
        return Ok(None);
    }
    get(conn, conn.last_insert_rowid())
}

pub fn get(conn: &Connection, id: i64) -> Result<Option<NotificationRecord>> {
    let row = conn
        .query_row(
            "SELECT id, type, severity, title, body, target_user_id, actor_user_local_id,
                    conversation_id, conversation_number, customer_local_id, issue_id,
                    campaign_id, job_id, side_thread_id, created_at, read_at
             FROM notifications WHERE id = ?1",
            params![id],
            row_to_notification,
        )
        .ok();
    Ok(row)
}

pub fn record_notification(
    conn: &Connection,
    bus: Option<&crate::http::EventBus>,
    input: &NotificationInput<'_>,
) -> Result<Option<NotificationRecord>> {
    if !pref_for(conn, input.notification_type)? {
        return Ok(None);
    }
    let Some(created) = insert(conn, input)? else {
        return Ok(None);
    };
    if let Some(bus) = bus {
        let unread = unread_count(conn, me_user_local_id(conn)?)?;
        crate::http::event_bus::notify_notification_received(
            bus,
            created.id,
            created.notification_type.as_str(),
            &created.severity,
            &created.title,
            created.conversation_id,
            created.conversation_number,
            created.customer_local_id,
            created.target_user_local_id,
            unread.max(0) as u32,
        );
    }
    Ok(Some(created))
}

#[derive(Debug, Clone, Default)]
pub struct ListOptions<'a> {
    pub me_user_local_id: Option<i64>,
    pub unread_only: bool,
    pub notif_type: Option<&'a str>,
    pub limit: i64,
    pub offset: i64,
}

pub fn list(conn: &Connection, opts: &ListOptions<'_>) -> Result<NotificationListResult> {
    let limit = opts.limit.clamp(1, 200);
    let offset = opts.offset.max(0);
    let me = opts.me_user_local_id.unwrap_or(-1);
    let mut where_sql = String::from("(n.target_user_id IS NULL OR n.target_user_id = ?1)");
    if opts.unread_only {
        where_sql.push_str(" AND n.read_at IS NULL");
    }
    if let Some(t) = opts.notif_type {
        where_sql.push_str(&format!(" AND n.type = '{}'", t.replace('\'', "''")));
    }
    let rows: Vec<NotificationRecord> = {
        let mut stmt = conn.prepare(&format!(
            "SELECT n.id, n.type, n.severity, n.title, n.body, n.target_user_id,
                    n.actor_user_local_id, n.conversation_id, n.conversation_number,
                    n.customer_local_id, n.issue_id, n.campaign_id, n.job_id,
                    n.side_thread_id, n.created_at, n.read_at
             FROM notifications n WHERE {where_sql}
             ORDER BY n.created_at DESC, n.id DESC LIMIT ?2 OFFSET ?3"
        ))?;

        stmt.query_map(params![me, limit, offset], row_to_notification)?
            .collect::<rusqlite::Result<Vec<_>>>()?
    };
    let total: i64 = conn.query_row(
        &format!("SELECT COUNT(*) FROM notifications n WHERE {where_sql}"),
        params![me],
        |r| r.get(0),
    )?;
    let unread: i64 = conn.query_row(
        "SELECT COUNT(*) FROM notifications
          WHERE (target_user_id IS NULL OR target_user_id = ?1) AND read_at IS NULL",
        params![me],
        |r| r.get(0),
    )?;
    Ok(NotificationListResult {
        notifications: rows,
        total,
        unread,
    })
}

pub fn mentions_for_me(conn: &Connection, me: i64, limit: i64) -> Result<Vec<NotificationRecord>> {
    let limit = limit.clamp(1, 200);
    let mut stmt = conn.prepare(
        "SELECT id, type, severity, title, body, target_user_id, actor_user_local_id,
                conversation_id, conversation_number, customer_local_id, issue_id,
                campaign_id, job_id, side_thread_id, created_at, read_at
         FROM notifications
         WHERE type IN ('mentioned', 'team_mentioned') AND target_user_id = ?1
         ORDER BY created_at DESC, id DESC LIMIT ?2",
    )?;
    let rows = stmt
        .query_map(params![me, limit], row_to_notification)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub fn unread_count(conn: &Connection, me: Option<i64>) -> Result<i64> {
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM notifications
         WHERE (target_user_id IS NULL OR target_user_id = ?1) AND read_at IS NULL",
        params![me.unwrap_or(-1)],
        |r| r.get(0),
    )?;
    Ok(n)
}

pub fn mark_read(conn: &Connection, id: i64, me: Option<i64>, read: bool) -> Result<bool> {
    let stamp = if read { "datetime('now')" } else { "NULL" };
    let rows = conn.execute(
        &format!(
            "UPDATE notifications SET read_at = {stamp}
             WHERE id = ?1 AND (target_user_id IS NULL OR target_user_id = ?2)"
        ),
        params![id, me.unwrap_or(-1)],
    )?;
    Ok(rows > 0)
}

pub fn mark_all_read(conn: &Connection, me: Option<i64>) -> Result<usize> {
    let rows = conn.execute(
        "UPDATE notifications SET read_at = datetime('now')
         WHERE read_at IS NULL AND (target_user_id IS NULL OR target_user_id = ?1)",
        params![me.unwrap_or(-1)],
    )?;
    Ok(rows)
}

pub fn prune_older_than(conn: &Connection, cutoff: &str) -> Result<usize> {
    let rows = conn.execute(
        "DELETE FROM notifications
         WHERE julianday(created_at) < julianday(?1)
            OR (read_at IS NOT NULL AND julianday(read_at) < julianday(?1))",
        params![cutoff],
    )?;
    Ok(rows)
}

fn row_to_notification(r: &rusqlite::Row<'_>) -> rusqlite::Result<NotificationRecord> {
    let type_str: String = r.get(1)?;
    let notification_type = parse_type(&type_str).ok_or_else(|| {
        rusqlite::Error::FromSqlConversionFailure(
            1,
            rusqlite::types::Type::Text,
            format!("unknown notification type: {type_str}").into(),
        )
    })?;
    Ok(NotificationRecord {
        id: r.get(0)?,
        notification_type,
        severity: r.get(2)?,
        title: r.get(3)?,
        body: r.get(4)?,
        target_user_local_id: r.get(5)?,
        actor_user_local_id: r.get(6)?,
        conversation_id: r.get(7)?,
        conversation_number: r.get(8)?,
        customer_local_id: r.get(9)?,
        issue_id: r.get(10)?,
        campaign_id: r.get(11)?,
        job_id: r.get(12)?,
        side_thread_id: r.get(13)?,
        created_at: r.get(14)?,
        read_at: r.get(15)?,
    })
}

fn truncate_chars(s: &str, max: usize) -> String {
    let t = s.trim();
    if t.chars().count() > max {
        let cut: String = t.chars().take(max.saturating_sub(1)).collect();
        format!("{cut}…")
    } else {
        t.to_string()
    }
}
