use super::*;
use rusqlite::Connection;

fn db_with_shared_tag_pattern() -> (Connection, i64) {
    let mut conn = Connection::open_in_memory().expect("db");
    crate::bootstrap::apply_all(&mut conn).expect("migrations");
    conn.execute(
        "INSERT INTO customers (remote_id, first_name, last_name)
         VALUES (501, 'Grace', 'Hopper')",
        [],
    )
    .expect("customer");
    let customer = conn.last_insert_rowid();
    let mut ids = Vec::new();
    for number in [9001, 9002, 9003] {
        conn.execute(
            "INSERT INTO conversations
                 (remote_id, number, subject, preview, status, customer_local_id,
                  created_at, remote_created_at)
             VALUES (?1, ?2, 'Billing recurs', 'preview', 'closed', ?3,
                     datetime('now'), datetime('now'))",
            params![number, number, customer],
        )
        .expect("conversation");
        ids.push(conn.last_insert_rowid());
    }
    let (current, rel_a, rel_b) = (ids[0], ids[1], ids[2]);
    for name in ["aaa-current-only", "shared-billing"] {
        conn.execute(
            "INSERT INTO tags (remote_id, name) VALUES (?1, ?2)",
            params![name.len() as i64, name],
        )
        .expect("tag");
        let tag = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO conversation_tags (conversation_id, tag_id) VALUES (?1, ?2)",
            params![current, tag],
        )
        .expect("current tag");
    }
    conn.execute(
        "INSERT INTO tags (remote_id, name) VALUES (99, 'shared-billing-alt')",
        [],
    )
    .expect("tag alt");
    let shared = conn
        .query_row(
            "SELECT id FROM tags WHERE name = 'shared-billing'",
            [],
            |r| r.get::<_, i64>(0),
        )
        .expect("shared tag id");
    for rel in [rel_a, rel_b] {
        conn.execute(
            "INSERT INTO conversation_tags (conversation_id, tag_id) VALUES (?1, ?2)",
            params![rel, shared],
        )
        .expect("related tag");
    }
    (conn, current)
}

#[test]
fn audit52_shared_tag_is_shared_with_related_conversations() {
    let (conn, current) = db_with_shared_tag_pattern();
    let findings = analyze_friction(&conn, current).expect("analyze friction");
    let finding = findings
        .iter()
        .find(|f| f["kind"] == json!("repeated_unresolved_interactions"))
        .expect("the repeated-unresolved finding fires for 3 conversations");
    let detail = finding["detail"].as_str().expect("detail");
    assert!(
        detail.contains("\"shared-billing\""),
        "the genuinely shared tag is named: {detail}"
    );
    assert!(
        !detail.contains("aaa-current-only"),
        "a current-only tag must never be presented as shared: {detail}"
    );
}

#[test]
fn audit52_no_tag_is_named_when_nothing_is_shared() {
    let (conn, _current) = db_with_shared_tag_pattern();
    conn.execute(
        "DELETE FROM conversation_tags WHERE conversation_id = (
             SELECT id FROM conversations WHERE number = 9001)",
        [],
    )
    .expect("strip tags");
    let current: i64 = conn
        .query_row(
            "SELECT id FROM conversations WHERE number = 9001",
            [],
            |r| r.get(0),
        )
        .expect("current id");
    let findings = analyze_friction(&conn, current).expect("analyze friction");
    assert!(
        findings
            .iter()
            .all(|f| f["kind"] != json!("repeated_unresolved_interactions")),
        "no related conversations means no finding"
    );
}
