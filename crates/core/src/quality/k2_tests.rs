use super::*;
use rusqlite::Connection;

fn db() -> Connection {
    let mut conn = Connection::open_in_memory().expect("db");
    crate::bootstrap::apply_all(&mut conn).expect("migrations");
    conn
}

fn seed_closed_with_reply(conn: &Connection, remote: i64, days_ago: i64) -> i64 {
    let at = format!(
        "{}",
        (chrono::Utc::now() - chrono::Duration::try_days(days_ago).unwrap_or_default())
            .format("%Y-%m-%dT%H:%M:%S%.3fZ")
    );
    conn.execute(
        "INSERT INTO conversations (remote_id, number, subject, preview, status,
                                    remote_created_at, closed_at)
         VALUES (?1, ?1, 'Export timeouts on weekly report', 'p', 'closed', ?2, ?2)",
        params![remote, at],
    )
    .expect("conversation");
    let id = conn.last_insert_rowid();
    conn.execute(
        "INSERT INTO conversation_threads (conversation_id, type, state, body_text, remote_created_at)
         VALUES (?1, 'reply', 'published', 'Clear the export cache, then re-run with the
           nightly profile; the timeout is the stale lock file.', ?2)",
        params![id, at],
    )
    .expect("thread");
    id
}

#[test]
fn best_resolutions_picks_closed_with_published_reply_only() {
    let conn = db();
    let first = seed_closed_with_reply(&conn, 101, 2);
    let second = seed_closed_with_reply(&conn, 102, 1);
    conn.execute(
        "INSERT INTO conversations (remote_id, number, subject, preview, status, remote_created_at)
         VALUES (103, 103, 'Still open', 'p', 'active', '2026-10-20T08:00:00.000Z')",
        [],
    )
    .expect("open");
    let open = conn.last_insert_rowid();
    conn.execute(
        "INSERT INTO conversation_threads (conversation_id, type, state, body_text, remote_created_at)
         VALUES (?1, 'reply', 'published', 'draft answer', '2026-10-20T08:05:00.000Z')",
        params![open],
    )
    .expect("open thread");

    let cites = best_resolutions(&conn, &[first, second, open, 999_999], 5);
    assert_eq!(cites.len(), 2, "only the two closed conversations");
    assert_eq!(cites[0].number, 102);
    assert_eq!(cites[1].number, 101);
    assert!(
        cites[0].excerpt.contains("nightly profile"),
        "the final published reply is the excerpt"
    );
    assert_eq!(best_resolutions(&conn, &[first], 1).len(), 1);
    assert!(best_resolutions(&conn, &[], 5).is_empty());
}

#[test]
fn best_resolutions_carries_the_customers_own_question() {
    let conn = db();
    let id = seed_closed_with_reply(&conn, 201, 2);
    conn.execute(
        "INSERT INTO conversation_threads (conversation_id, type, state, body_text, remote_created_at)
         VALUES (?1, 'customer', 'published',
                 'Every night our weekly export dies at 2am — how do we stop the timeouts?',
                 '2026-10-18T08:00:00.000Z')",
        params![id],
    )
    .expect("customer thread");
    let cites = best_resolutions(&conn, &[id], 5);
    assert_eq!(cites.len(), 1);
    assert!(
        cites[0].customer_question.contains("weekly export dies"),
        "the customer's own words ride the citation: {:?}",
        cites[0].customer_question
    );
    assert!(
        cites[0].excerpt.contains("nightly profile"),
        "the agent's published reply stays the resolution excerpt"
    );

    let bare = seed_closed_with_reply(&conn, 202, 1);
    let bare_cites = best_resolutions(&conn, &[bare], 5);
    assert_eq!(bare_cites.len(), 1);
    assert_eq!(bare_cites[0].customer_question, "");
}

#[test]
fn best_resolutions_skips_closed_conversation_without_reply_text() {
    let conn = db();
    conn.execute(
        "INSERT INTO conversations (remote_id, number, subject, preview, status,
                                    remote_created_at, closed_at)
         VALUES (201, 201, 'Closed silently', 'p', 'closed',
                 '2026-10-18T08:00:00.000Z', '2026-10-19T08:00:00.000Z')",
        [],
    )
    .expect("closed no reply");
    let id = conn.last_insert_rowid();
    assert!(best_resolutions(&conn, &[id], 5).is_empty());
}
