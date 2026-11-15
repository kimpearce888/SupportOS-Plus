use super::*;
use rusqlite::Connection;

fn db() -> Connection {
    let mut conn = Connection::open_in_memory().expect("db");
    crate::bootstrap::apply_all(&mut conn).expect("migrations");
    conn
}

#[test]
fn refresh_replies_ignores_the_campaigns_own_message_under_clock_skew() {
    let conn = db();
    conn.execute(
        "INSERT INTO outreach_campaigns (name, subject, body, status)
         VALUES ('skew', 'Hi', 'Body', 'sending')",
        [],
    )
    .expect("campaign");
    let campaign: i64 = conn.last_insert_rowid();
    conn.execute(
        "INSERT INTO customers (remote_id, first_name) VALUES (1, 'Casey')",
        [],
    )
    .expect("customer");
    conn.execute(
        "INSERT INTO conversations (remote_id, number, subject, preview, status)
         VALUES (500, 500, 'campaign conv', 'p', 'active')",
        [],
    )
    .expect("conversation");
    let conv: i64 = conn.last_insert_rowid();
    conn.execute(
        "INSERT INTO outreach_recipients
             (campaign_id, customer_local_id, customer_remote_id, email, state,
              hs_conversation_remote_id, sent_at)
         VALUES (?1, 1, 1, 'c@example.com', 'sent', 500, '2026-10-24 10:00:00')",
        params![campaign],
    )
    .expect("recipient");
    conn.execute(
        "INSERT INTO conversation_threads (conversation_id, type, state, body_text, created_at)
         VALUES (?1, 'customer', 'published', 'the campaign email itself',
                 '2026-10-24T10:00:01.000Z')",
        [conv],
    )
    .expect("campaign thread");
    conn.execute(
        "INSERT INTO conversation_threads (conversation_id, type, state, body_text, created_at)
         VALUES (?1, 'customer', 'published', 'thanks — where do I find it?',
                 '2026-10-24T11:00:00.000Z')",
        [conv],
    )
    .expect("reply thread");

    let replied = refresh_replies(&conn, campaign).expect("refresh");
    assert_eq!(replied, 1, "exactly one real reply");
    let replied_at: String = conn
        .query_row(
            "SELECT replied_at FROM outreach_recipients WHERE campaign_id = ?1",
            [campaign],
            |r| r.get(0),
        )
        .expect("replied_at");
    assert_eq!(
        replied_at, "2026-10-24T11:00:00.000Z",
        "the reply must be the LATER thread, not the campaign's own message"
    );
}

#[test]
fn refresh_replies_counts_nothing_when_only_the_campaign_message_exists() {
    let conn = db();
    conn.execute(
        "INSERT INTO outreach_campaigns (name, subject, body, status)
         VALUES ('skew2', 'Hi', 'Body', 'sending')",
        [],
    )
    .expect("campaign");
    let campaign: i64 = conn.last_insert_rowid();
    conn.execute(
        "INSERT INTO customers (remote_id, first_name) VALUES (1, 'Casey')",
        [],
    )
    .expect("customer");
    conn.execute(
        "INSERT INTO conversations (remote_id, number, subject, preview, status)
         VALUES (501, 501, 'campaign conv', 'p', 'active')",
        [],
    )
    .expect("conversation");
    let conv: i64 = conn.last_insert_rowid();
    conn.execute(
        "INSERT INTO outreach_recipients
             (campaign_id, customer_local_id, customer_remote_id, email, state,
              hs_conversation_remote_id, sent_at)
         VALUES (?1, 1, 1, 'c@example.com', 'sent', 501, '2026-10-24 10:00:00')",
        params![campaign],
    )
    .expect("recipient");
    conn.execute(
        "INSERT INTO conversation_threads (conversation_id, type, state, body_text, created_at)
         VALUES (?1, 'customer', 'published', 'the campaign email itself',
                 '2026-10-24T10:00:01.000Z')",
        [conv],
    )
    .expect("campaign thread");

    let replied = refresh_replies(&conn, campaign).expect("refresh");
    assert_eq!(replied, 0, "the campaign's own message is not a reply");
    let replied_at: Option<String> = conn
        .query_row(
            "SELECT replied_at FROM outreach_recipients WHERE campaign_id = ?1",
            [campaign],
            |r| r.get(0),
        )
        .expect("replied_at");
    assert!(replied_at.is_none());
}
