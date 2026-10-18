use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};

use crate::catalog::NotificationType;
use crate::error::Result;
use crate::mentions;
use crate::notifications::{NotificationInput, record_notification};

pub const M006_SQL: &str = r#"
    CREATE TABLE IF NOT EXISTS side_threads (
        id                  INTEGER PRIMARY KEY AUTOINCREMENT,
        conversation_id     INTEGER NOT NULL,
        created_at          TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
        created_by_user_id  INTEGER
    );
    CREATE INDEX IF NOT EXISTS idx_side_threads_conversation
        ON side_threads (conversation_id, created_at);

    CREATE TABLE IF NOT EXISTS side_thread_messages (
        id                  INTEGER PRIMARY KEY AUTOINCREMENT,
        thread_id           INTEGER NOT NULL REFERENCES side_threads (id) ON DELETE CASCADE,
        body                TEXT NOT NULL,
        author_user_id      INTEGER,
        created_at          TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
        mentions_json       TEXT
    );
    CREATE INDEX IF NOT EXISTS idx_side_thread_messages_thread
        ON side_thread_messages (thread_id, created_at);

    CREATE TABLE IF NOT EXISTS side_thread_mentions (
        id              INTEGER PRIMARY KEY AUTOINCREMENT,
        side_thread_id  INTEGER NOT NULL REFERENCES side_threads (id) ON DELETE CASCADE,
        message_id      INTEGER NOT NULL REFERENCES side_thread_messages (id) ON DELETE CASCADE,
        user_local_id   INTEGER,
        team_local_id   INTEGER,
        created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
    );
    CREATE INDEX IF NOT EXISTS idx_side_thread_mentions_user
        ON side_thread_mentions (user_local_id, created_at);

    UPDATE app_state SET schema_version = 6 WHERE id = 1;
"#;

pub fn apply_m006(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS side_threads (
            id                  INTEGER PRIMARY KEY AUTOINCREMENT,
            conversation_id     INTEGER NOT NULL,
            created_at          TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
            created_by_user_id  INTEGER
        );
        CREATE INDEX IF NOT EXISTS idx_side_threads_conversation
            ON side_threads (conversation_id, created_at);

        CREATE TABLE IF NOT EXISTS side_thread_messages (
            id                  INTEGER PRIMARY KEY AUTOINCREMENT,
            thread_id           INTEGER NOT NULL REFERENCES side_threads (id) ON DELETE CASCADE,
            body                TEXT NOT NULL,
            author_user_id      INTEGER,
            created_at          TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
            mentions_json       TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_side_thread_messages_thread
            ON side_thread_messages (thread_id, created_at);

        CREATE TABLE IF NOT EXISTS side_thread_mentions (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            side_thread_id  INTEGER NOT NULL REFERENCES side_threads (id) ON DELETE CASCADE,
            message_id      INTEGER NOT NULL REFERENCES side_thread_messages (id) ON DELETE CASCADE,
            user_local_id   INTEGER,
            team_local_id   INTEGER,
            created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
        );
        CREATE INDEX IF NOT EXISTS idx_side_thread_mentions_user
            ON side_thread_mentions (user_local_id, created_at);",
    )?;
    let _ = conn.execute("UPDATE app_state SET schema_version = 6 WHERE id = 1", []);
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SideThread {
    pub id: Option<i64>,
    pub conversation_id: i64,
    pub created_by_user_id: Option<i64>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SideThreadMessage {
    pub id: Option<i64>,
    pub thread_id: i64,
    pub body: String,
    pub author_user_id: Option<i64>,
    pub created_at: String,
    pub mentions_json: Option<String>,
}

pub fn create_side_thread(
    conn: &Connection,
    conversation_id: i64,
    created_by_user_id: Option<i64>,
) -> Result<i64> {
    conn.execute(
        "INSERT INTO side_threads (conversation_id, created_by_user_id)
         VALUES (?1, ?2)",
        params![conversation_id, created_by_user_id],
    )?;
    Ok(conn.last_insert_rowid())
}

fn audit_side_thread_event(
    conn: &Connection,
    action: &str,
    conversation_id: Option<i64>,
    after_state: serde_json::Value,
) -> Result<()> {
    let mut entry = crate::audit::AuditEntry::user(action);
    if let Some(id) = conversation_id {
        entry = entry.with_conversation_id(id);
    }
    crate::audit::audit(conn, &entry.with_after_state(after_state))
}

fn resolve_local_conversation_id(conn: &Connection, conversation_ref: i64) -> Option<i64> {
    conn.query_row(
        "SELECT id FROM conversations
         WHERE id = ?1 OR remote_id = ?1
         ORDER BY CASE WHEN id = ?1 THEN 0 ELSE 1 END LIMIT 1",
        params![conversation_ref],
        |r| r.get(0),
    )
    .ok()
}

fn thread_local_conversation_id(conn: &Connection, thread_id: i64) -> Option<i64> {
    let conversation_ref: Option<i64> = conn
        .query_row(
            "SELECT conversation_id FROM side_threads WHERE id = ?1",
            params![thread_id],
            |r| r.get(0),
        )
        .ok();
    conversation_ref.and_then(|c| resolve_local_conversation_id(conn, c))
}

pub fn resolve_side_thread(conn: &Connection, thread_id: i64) -> Result<bool> {
    let tx = conn.unchecked_transaction()?;
    let rows = tx.execute(
        "UPDATE side_threads SET resolved_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
         WHERE id = ?1",
        params![thread_id],
    )?;
    if rows == 0 {
        return Ok(false);
    }
    let resolved_at: Option<String> = tx
        .query_row(
            "SELECT resolved_at FROM side_threads WHERE id = ?1",
            params![thread_id],
            |r| r.get(0),
        )
        .ok()
        .flatten();
    audit_side_thread_event(
        &tx,
        "side_thread_resolved",
        thread_local_conversation_id(&tx, thread_id),
        serde_json::json!({ "sideThreadId": thread_id, "resolvedAt": resolved_at }),
    )?;
    tx.commit()?;
    Ok(true)
}

pub fn reopen_side_thread(conn: &Connection, thread_id: i64) -> Result<bool> {
    let tx = conn.unchecked_transaction()?;
    let rows = tx.execute(
        "UPDATE side_threads SET resolved_at = NULL WHERE id = ?1",
        params![thread_id],
    )?;
    if rows == 0 {
        return Ok(false);
    }
    audit_side_thread_event(
        &tx,
        "side_thread_reopened",
        thread_local_conversation_id(&tx, thread_id),
        serde_json::json!({ "sideThreadId": thread_id }),
    )?;
    tx.commit()?;
    Ok(true)
}

#[derive(Debug, Clone, PartialEq)]
pub struct CreateSideThreadInput {
    pub title: String,
    pub team_local_id: Option<i64>,
    pub participant_user_ids: Vec<i64>,
    pub created_by_user_id: Option<i64>,
    pub first_message: Option<String>,
}

pub type ValidationIssues = Vec<(String, String)>;

fn body_i64(body: &serde_json::Value, keys: &[&str]) -> Option<i64> {
    keys.iter()
        .find_map(|k| body.get(*k).and_then(|v| v.as_i64()))
}

fn body_str<'a>(body: &'a serde_json::Value, keys: &[&str]) -> Option<&'a str> {
    keys.iter()
        .find_map(|k| body.get(*k).and_then(|v| v.as_str()))
}

fn body_i64_array(body: &serde_json::Value, keys: &[&str]) -> Option<Vec<i64>> {
    for k in keys {
        if let Some(v) = body.get(*k) {
            if v.is_null() {
                return Some(Vec::new());
            }
            if let Some(arr) = v.as_array() {
                return Some(arr.iter().filter_map(|x| x.as_i64()).collect::<Vec<i64>>());
            }
        }
    }
    None
}

pub fn parse_create_side_thread_input(
    body: &serde_json::Value,
) -> std::result::Result<CreateSideThreadInput, ValidationIssues> {
    let mut issues: ValidationIssues = Vec::new();
    let title = body_str(body, &["title"]).unwrap_or("").trim().to_string();
    if title.is_empty() {
        issues.push((
            "title".into(),
            "Required and must be a non-empty string.".into(),
        ));
    }
    if let Some(v) = body.get("title")
        && !v.is_string()
    {
        issues.push((
            "title".into(),
            "Expected string, received other type.".into(),
        ));
    }
    let mut team_local_id = body_i64(body, &["teamId", "team_local_id"]);
    if let Some(v) = body.get("teamId").or_else(|| body.get("team_local_id"))
        && !v.is_null()
        && v.as_i64().is_none()
    {
        issues.push((
            "teamId".into(),
            "Expected number, received other type.".into(),
        ));
        team_local_id = None;
    }
    let participant_user_ids =
        body_i64_array(body, &["participantUserIds", "participant_user_ids"]).unwrap_or_default();
    if let Some(v) = body
        .get("participantUserIds")
        .or_else(|| body.get("participant_user_ids"))
        && !v.is_null()
        && v.as_array().is_none()
    {
        issues.push((
            "participantUserIds".into(),
            "Expected array of numbers.".into(),
        ));
    }
    let mut created_by_user_id = body_i64(body, &["createdByUserId", "created_by_user_id"]);
    if let Some(v) = body
        .get("createdByUserId")
        .or_else(|| body.get("created_by_user_id"))
        && !v.is_null()
        && v.as_i64().is_none()
    {
        issues.push((
            "createdByUserId".into(),
            "Expected number, received other type.".into(),
        ));
        created_by_user_id = None;
    }
    let mut first_message = body_str(body, &["firstMessage", "first_message"])
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    if let Some(v) = body
        .get("firstMessage")
        .or_else(|| body.get("first_message"))
        && !v.is_null()
        && !v.is_string()
    {
        issues.push((
            "firstMessage".into(),
            "Expected string, received other type.".into(),
        ));
        first_message = None;
    }
    if issues.is_empty() {
        Ok(CreateSideThreadInput {
            title,
            team_local_id,
            participant_user_ids,
            created_by_user_id,
            first_message,
        })
    } else {
        Err(issues)
    }
}

pub fn check_create_references(
    conn: &Connection,
    input: &CreateSideThreadInput,
) -> ValidationIssues {
    let mut issues = ValidationIssues::new();
    if let Some(team_id) = input.team_local_id {
        let known: bool = conn
            .query_row(
                "SELECT 1 FROM teams WHERE id = ?1",
                params![team_id],
                |_| Ok(()),
            )
            .is_ok();
        if !known {
            issues.push(("teamId".into(), format!("Unknown team {team_id}.")));
        }
    }
    let unknown = unknown_user_ids(conn, &input.participant_user_ids);
    if !unknown.is_empty() {
        issues.push((
            "participantUserIds".into(),
            format!(
                "Unknown user(s): {}.",
                unknown
                    .iter()
                    .map(|id| id.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        ));
    }
    if let Some(creator) = input.created_by_user_id {
        let known: bool = conn
            .query_row(
                "SELECT 1 FROM users WHERE id = ?1",
                params![creator],
                |_| Ok(()),
            )
            .is_ok();
        if !known {
            issues.push(("createdByUserId".into(), format!("Unknown user {creator}.")));
        }
    }
    issues
}

fn unknown_user_ids(conn: &Connection, ids: &[i64]) -> Vec<i64> {
    let mut unknown = Vec::new();
    for id in ids {
        if unknown.contains(id) {
            continue;
        }
        let known: bool = conn
            .query_row("SELECT 1 FROM users WHERE id = ?1", params![id], |_| Ok(()))
            .is_ok();
        if !known {
            unknown.push(*id);
        }
    }
    unknown
}

pub fn create_side_thread_full(
    conn: &Connection,
    bus: Option<&crate::eventing::EventBus>,
    conversation_id: i64,
    input: &CreateSideThreadInput,
) -> Result<i64> {
    let tx = conn.unchecked_transaction()?;
    tx.execute(
        "INSERT INTO side_threads (conversation_id, title, team_local_id, created_by_user_id)
         VALUES (?1, ?2, ?3, ?4)",
        params![
            conversation_id,
            input.title,
            input.team_local_id,
            input.created_by_user_id
        ],
    )?;
    let thread_id = tx.last_insert_rowid();
    insert_participants_unchecked(
        &tx,
        thread_id,
        &input.participant_user_ids,
        input.created_by_user_id,
    )?;
    if let Some(first) = &input.first_message {
        insert_side_thread_message_unchecked(&tx, bus, thread_id, first, input.created_by_user_id)?;
    }
    audit_side_thread_event(
        &tx,
        "side_thread_created",
        resolve_local_conversation_id(&tx, conversation_id),
        serde_json::json!({
            "sideThreadId": thread_id,
            "title": input.title,
            "teamLocalId": input.team_local_id,
            "createdByUserId": input.created_by_user_id,
            "participantUserIds": input.participant_user_ids,
            "firstMessage": input.first_message.is_some(),
        }),
    )?;
    tx.commit()?;
    Ok(thread_id)
}

pub fn add_participants_checked(
    conn: &Connection,
    thread_id: i64,
    user_ids: &[i64],
    added_by_user_local_id: Option<i64>,
) -> Result<Vec<i64>> {
    let unknown = unknown_user_ids(conn, user_ids);
    if !unknown.is_empty() {
        return Err(crate::error::Error::Other(
            format!(
                "Unknown user(s): {}",
                unknown
                    .iter()
                    .map(|id| id.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
            .into(),
        ));
    }
    let tx = conn.unchecked_transaction()?;
    let inserted = insert_participants_unchecked(&tx, thread_id, user_ids, added_by_user_local_id)?;
    if !inserted.is_empty() {
        audit_side_thread_event(
            &tx,
            "side_thread_participants_added",
            thread_local_conversation_id(&tx, thread_id),
            serde_json::json!({
                "sideThreadId": thread_id,
                "addedUserIds": inserted,
                "addedByUserId": added_by_user_local_id,
            }),
        )?;
    }
    tx.commit()?;
    Ok(inserted)
}

fn insert_participants_unchecked(
    conn: &Connection,
    thread_id: i64,
    user_ids: &[i64],
    added_by_user_local_id: Option<i64>,
) -> Result<Vec<i64>> {
    let mut inserted = Vec::new();
    for uid in user_ids {
        let rows = conn.execute(
            "INSERT OR IGNORE INTO side_thread_participants
                (side_thread_id, user_local_id, added_by_user_local_id)
             VALUES (?1, ?2, ?3)",
            params![thread_id, uid, added_by_user_local_id],
        )?;
        if rows > 0 {
            inserted.push(*uid);
        }
    }
    Ok(inserted)
}

pub fn add_side_thread_message(
    conn: &Connection,
    bus: Option<&crate::eventing::EventBus>,
    thread_id: i64,
    body: &str,
    author_user_id: Option<i64>,
) -> Result<i64> {
    let tx = conn.unchecked_transaction()?;
    let message_id =
        insert_side_thread_message_unchecked(&tx, bus, thread_id, body, author_user_id)?;
    tx.commit()?;
    Ok(message_id)
}

fn insert_side_thread_message_unchecked(
    conn: &Connection,
    bus: Option<&crate::eventing::EventBus>,
    thread_id: i64,
    body: &str,
    author_user_id: Option<i64>,
) -> Result<i64> {
    let raw_mentions = mentions::scan_for_mentions(body)?;
    let mentions_json = if raw_mentions.is_empty() {
        None
    } else {
        let display_strings: Vec<String> = raw_mentions
            .iter()
            .map(mentions::Mention::display)
            .collect();
        Some(serde_json::to_string(&display_strings).map_err(|e| {
            crate::error::Error::Config(format!("mentions_json serialization failed: {e}"))
        })?)
    };

    conn.execute(
        "INSERT INTO side_thread_messages (thread_id, body, author_user_id, mentions_json)
         VALUES (?1, ?2, ?3, ?4)",
        params![thread_id, body, author_user_id, mentions_json],
    )?;
    let message_id = conn.last_insert_rowid();

    let mut mention_count: usize = 0;
    let thread: Option<(i64, Option<String>)> = conn
        .query_row(
            "SELECT conversation_id, title FROM side_threads WHERE id = ?1",
            params![thread_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .ok();
    if let Some((conversation_ref, thread_title)) = thread {
        let conv: Option<(i64, Option<i64>)> = conn
            .query_row(
                "SELECT id, number FROM conversations
                 WHERE id = ?1 OR remote_id = ?1
                 ORDER BY CASE WHEN id = ?1 THEN 0 ELSE 1 END LIMIT 1",
                params![conversation_ref],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .ok();
        let (conversation_id, conversation_number) = conv
            .map(|(id, number)| (Some(id), number))
            .unwrap_or((None, None));
        let thread_title = thread_title.unwrap_or_else(|| "side thread".into());
        let number_for_title = conversation_number.unwrap_or(conversation_ref);
        let author_name = author_user_id
            .and_then(|id| {
                mentions::build_mention_directory(conn)
                    .ok()
                    .and_then(|d| d.display_by_user.get(&id).cloned())
            })
            .unwrap_or_else(|| "Someone".into());
        let trimmed_body = trim_200(body);

        let directory = mentions::build_mention_directory(conn)?;
        let resolved = mentions::parse_mentions(body, &directory)?;
        mention_count = resolved.len();
        for m in resolved {
            conn.execute(
                "INSERT INTO side_thread_mentions
                    (side_thread_id, message_id, user_local_id, team_local_id)
                 VALUES (?1, ?2, ?3, ?4)",
                params![thread_id, message_id, m.user_local_id, m.team_local_id],
            )?;
            if let Some(user) = m.user_local_id {
                if Some(user) == author_user_id {
                    continue;
                }
                let _ = record_notification(
                    conn,
                    bus,
                    &NotificationInput {
                        notification_type: NotificationType::Mentioned,
                        title: format!(
                            "{author_name} mentioned you in \"{thread_title}\" (#{number_for_title})"
                        ),
                        body: Some(trimmed_body.clone()),
                        target_user_local_id: Some(user),
                        actor_user_local_id: author_user_id,
                        conversation_id,
                        conversation_number,
                        side_thread_id: Some(thread_id),
                        dedup_key: format!("n:stm:{message_id}:{user}"),
                        ..Default::default()
                    },
                );
            }
            if let Some(team) = m.team_local_id {
                for member in mentions::team_members(conn, team)?.into_iter() {
                    if Some(member) == author_user_id {
                        continue;
                    }
                    let _ = record_notification(
                        conn,
                        bus,
                        &NotificationInput {
                            notification_type: NotificationType::TeamMentioned,
                            title: format!(
                                "{author_name} mentioned @{} (you are a member) in \"{thread_title}\" (#{number_for_title})",
                                m.display
                            ),
                            body: Some(trimmed_body.clone()),
                            target_user_local_id: Some(member),
                            actor_user_local_id: author_user_id,
                            conversation_id,
                            conversation_number,
                            side_thread_id: Some(thread_id),
                            dedup_key: format!("n:stm:{message_id}:team{team}:{member}"),
                            ..Default::default()
                        },
                    );
                }
            }
        }
    }
    audit_side_thread_event(
        conn,
        "side_thread_message_added",
        thread_local_conversation_id(conn, thread_id),
        serde_json::json!({
            "sideThreadId": thread_id,
            "messageId": message_id,
            "authorUserId": author_user_id,
            "mentionCount": mention_count,
        }),
    )?;
    Ok(message_id)
}

fn trim_200(body: &str) -> String {
    if body.chars().count() > 200 {
        let cut: String = body.chars().take(199).collect();
        format!("{cut}…")
    } else {
        body.to_string()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SideThreadMentionForUser {
    pub message_id: i64,
    pub thread_id: i64,
    pub thread_title: Option<String>,
    pub conversation_id: i64,
    pub conversation_number: Option<i64>,
    pub author: Option<String>,
    pub body: String,
    pub created_at: String,
}

pub fn mentions_for_user(conn: &Connection, me: i64) -> Result<Vec<SideThreadMentionForUser>> {
    let mut stmt = conn.prepare(
        "SELECT m.id, m.thread_id, t.title, t.conversation_id, m.body, m.created_at,
                (SELECT TRIM(COALESCE(u.first_name, '') || ' ' || COALESCE(u.last_name, ''))
                   FROM users u WHERE u.id = m.author_user_id) AS author
         FROM side_thread_mentions stm
         JOIN side_thread_messages m ON m.id = stm.message_id
         JOIN side_threads t ON t.id = stm.side_thread_id
         WHERE stm.user_local_id = ?1
         ORDER BY m.created_at DESC LIMIT 100",
    )?;
    let rows = stmt
        .query_map(params![me], |r| {
            Ok(SideThreadMentionForUser {
                message_id: r.get(0)?,
                thread_id: r.get(1)?,
                thread_title: r.get(2)?,
                conversation_id: r.get(3)?,
                conversation_number: None,
                author: r.get(6)?,
                body: r.get(4)?,
                created_at: r.get(5)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut resolved = rows;
    for m in &mut resolved {
        m.conversation_number = conn
            .query_row(
                "SELECT number FROM conversations
                 WHERE id = ?1 OR remote_id = ?1
                 ORDER BY CASE WHEN id = ?1 THEN 0 ELSE 1 END LIMIT 1",
                params![m.conversation_id],
                |r| r.get(0),
            )
            .ok();
    }
    Ok(resolved)
}

pub fn list_side_threads_for_conversation(
    conn: &Connection,
    conversation_id: i64,
) -> Result<Vec<SideThread>> {
    let mut stmt = conn.prepare(
        "SELECT id, conversation_id, created_by_user_id, created_at
         FROM side_threads
         WHERE conversation_id = ?1
         ORDER BY julianday(created_at) ASC",
    )?;
    let rows = stmt
        .query_map(params![conversation_id], |r| {
            Ok(SideThread {
                id: r.get(0)?,
                conversation_id: r.get(1)?,
                created_by_user_id: r.get(2)?,
                created_at: r.get(3)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub fn list_side_thread_messages(
    conn: &Connection,
    thread_id: i64,
) -> Result<Vec<SideThreadMessage>> {
    let mut stmt = conn.prepare(
        "SELECT id, thread_id, body, author_user_id, created_at, mentions_json
         FROM side_thread_messages
         WHERE thread_id = ?1
         ORDER BY julianday(created_at) ASC",
    )?;
    let rows = stmt
        .query_map(params![thread_id], |r| {
            Ok(SideThreadMessage {
                id: r.get(0)?,
                thread_id: r.get(1)?,
                body: r.get(2)?,
                author_user_id: r.get(3)?,
                created_at: r.get(4)?,
                mentions_json: r.get(5)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub fn count_side_thread_messages(conn: &Connection, thread_id: i64) -> Result<u32> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM side_thread_messages WHERE thread_id = ?1",
        params![thread_id],
        |r| r.get(0),
    )?;
    Ok(u32::try_from(count).unwrap_or(0))
}

#[derive(Debug, Clone, Serialize)]
pub struct SideThreadSummary {
    pub id: i64,
    pub conversation_id: i64,
    pub conversation_number: Option<i64>,
    pub title: String,
    pub team_local_id: Option<i64>,
    pub team_name: Option<String>,
    pub status: String,
    pub created_by_user_local_id: Option<i64>,
    pub created_at: String,
    pub updated_at: String,
    pub resolved_at: Option<String>,
    pub message_count: i64,
    pub last_message_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SideThreadParticipant {
    pub user_local_id: i64,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub mention: Option<String>,
    pub added_at: String,
    pub added_by_user_local_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SideThreadMentionRef {
    pub user_local_id: Option<i64>,
    pub team_local_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SideThreadMessageItem {
    pub id: i64,
    pub side_thread_id: i64,
    pub author_user_local_id: Option<i64>,
    pub author_first_name: Option<String>,
    pub author_last_name: Option<String>,
    pub body: String,
    pub created_at: String,
    pub mentions: Vec<SideThreadMentionRef>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SideThreadDetail {
    pub id: i64,
    pub conversation_id: i64,
    pub conversation_number: Option<i64>,
    pub title: String,
    pub team_local_id: Option<i64>,
    pub team_name: Option<String>,
    pub status: String,
    pub created_by_user_local_id: Option<i64>,
    pub created_at: String,
    pub updated_at: String,
    pub resolved_at: Option<String>,
    pub message_count: i64,
    pub last_message_at: Option<String>,
    pub participants: Vec<SideThreadParticipant>,
    pub messages: Vec<SideThreadMessageItem>,
}

struct ThreadSummaryRow {
    id: i64,
    conversation_id: i64,
    title: String,
    team_local_id: Option<i64>,
    team_name: Option<String>,
    status: String,
    created_by_user_local_id: Option<i64>,
    created_at: String,
    updated_at: String,
    resolved_at: Option<String>,
    conversation_number: Option<i64>,
    message_count: i64,
    last_message_at: Option<String>,
}

impl ThreadSummaryRow {
    fn read(r: &rusqlite::Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: r.get(0)?,
            conversation_id: r.get(1)?,
            title: r.get(2)?,
            team_local_id: r.get(3)?,
            status: r.get(4)?,
            created_by_user_local_id: r.get(5)?,
            created_at: r.get(6)?,
            updated_at: r.get(7)?,
            resolved_at: r.get(8)?,
            conversation_number: r.get(9)?,
            team_name: r.get(10)?,
            message_count: r.get(11)?,
            last_message_at: r.get(12)?,
        })
    }
}

const THREAD_SUMMARY_SELECT: &str = "SELECT st.id, st.conversation_id, st.title, st.team_local_id,
       st.status, st.created_by_user_id, st.created_at, st.updated_at, st.resolved_at,
       c.number AS conversation_number, t.name AS team_name,
       (SELECT COUNT(*) FROM side_thread_messages m WHERE m.thread_id = st.id) AS message_count,
       (SELECT MAX(m.created_at) FROM side_thread_messages m WHERE m.thread_id = st.id) AS last_message_at
FROM side_threads st
JOIN conversations c ON c.id = st.conversation_id
LEFT JOIN teams t ON t.id = st.team_local_id";

pub fn list_side_thread_summaries(
    conn: &Connection,
    conversation_id: i64,
) -> Result<Vec<SideThreadSummary>> {
    let mut stmt = conn.prepare(&format!(
        "{THREAD_SUMMARY_SELECT}
         WHERE st.conversation_id = ?1
         ORDER BY (st.status = 'resolved') ASC, julianday(st.updated_at) DESC"
    ))?;
    let rows = stmt
        .query_map(params![conversation_id], ThreadSummaryRow::read)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows
        .into_iter()
        .map(|r| SideThreadSummary {
            id: r.id,
            conversation_id: r.conversation_id,
            conversation_number: r.conversation_number,
            title: r.title,
            team_local_id: r.team_local_id,
            team_name: r.team_name,
            status: r.status,
            created_by_user_local_id: r.created_by_user_local_id,
            created_at: r.created_at,
            updated_at: r.updated_at,
            resolved_at: r.resolved_at,
            message_count: r.message_count,
            last_message_at: r.last_message_at,
        })
        .collect())
}

pub fn get_side_thread_detail(
    conn: &Connection,
    thread_id: i64,
) -> Result<Option<SideThreadDetail>> {
    use rusqlite::OptionalExtension;

    let mut stmt = conn.prepare(&format!("{THREAD_SUMMARY_SELECT} WHERE st.id = ?1"))?;
    let row = stmt
        .query_row(params![thread_id], ThreadSummaryRow::read)
        .optional()?;
    let Some(row) = row else {
        return Ok(None);
    };

    let mut stmt = conn.prepare(
        "SELECT p.user_local_id, u.first_name, u.last_name, u.mention,
                p.added_at, p.added_by_user_local_id
         FROM side_thread_participants p JOIN users u ON u.id = p.user_local_id
         WHERE p.side_thread_id = ?1
         ORDER BY julianday(p.added_at) ASC, p.user_local_id ASC",
    )?;
    let participants = stmt
        .query_map(params![thread_id], |r| {
            Ok(SideThreadParticipant {
                user_local_id: r.get(0)?,
                first_name: r.get(1)?,
                last_name: r.get(2)?,
                mention: r.get(3)?,
                added_at: r.get(4)?,
                added_by_user_local_id: r.get(5)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let mut stmt = conn.prepare(
        "SELECT m.id, m.thread_id, m.author_user_id, m.body, m.created_at,
                u.first_name AS author_first_name, u.last_name AS author_last_name
         FROM side_thread_messages m LEFT JOIN users u ON u.id = m.author_user_id
         WHERE m.thread_id = ?1
         ORDER BY julianday(m.created_at) ASC, m.id ASC",
    )?;
    let messages = stmt
        .query_map(params![thread_id], |r| {
            Ok(SideThreadMessageItem {
                id: r.get(0)?,
                side_thread_id: r.get(1)?,
                author_user_local_id: r.get(2)?,
                author_first_name: r.get(5)?,
                author_last_name: r.get(6)?,
                body: r.get(3)?,
                created_at: r.get(4)?,
                mentions: Vec::new(),
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let mut by_message: std::collections::HashMap<i64, Vec<SideThreadMentionRef>> =
        std::collections::HashMap::new();
    let mut stmt = conn.prepare(
        "SELECT message_id, user_local_id, team_local_id
         FROM side_thread_mentions WHERE side_thread_id = ?1",
    )?;
    let mention_rows = stmt
        .query_map(params![thread_id], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                SideThreadMentionRef {
                    user_local_id: r.get(1)?,
                    team_local_id: r.get(2)?,
                },
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    for (message_id, m) in mention_rows {
        by_message.entry(message_id).or_default().push(m);
    }
    let mut messages = messages;
    for m in &mut messages {
        if let Some(mentions) = by_message.get(&m.id) {
            m.mentions = mentions.clone();
        }
    }

    Ok(Some(SideThreadDetail {
        id: row.id,
        conversation_id: row.conversation_id,
        conversation_number: row.conversation_number,
        title: row.title,
        team_local_id: row.team_local_id,
        team_name: row.team_name,
        status: row.status,
        created_by_user_local_id: row.created_by_user_local_id,
        created_at: row.created_at,
        updated_at: row.updated_at,
        resolved_at: row.resolved_at,
        message_count: row.message_count,
        last_message_at: row.last_message_at,
        participants,
        messages,
    }))
}
