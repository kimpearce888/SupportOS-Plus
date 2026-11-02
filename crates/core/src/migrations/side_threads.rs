use rusqlite::Connection;

use crate::error::Result;
use serde::Serialize;

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
