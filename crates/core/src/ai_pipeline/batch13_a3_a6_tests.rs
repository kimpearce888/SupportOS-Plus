use super::*;
use rusqlite::Connection;

fn db() -> Connection {
    let mut conn = Connection::open_in_memory().expect("db");
    crate::bootstrap::apply_all(&mut conn).expect("migrations");
    conn
}

#[test]
fn rewrite_composer_text_refuses_without_backend() {
    let conn = db();
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    let err = rt
        .block_on(rewrite_composer_text(
            &conn,
            &AiBackend::Disabled,
            None,
            "Please check the export logs.",
            "shorten",
        ))
        .expect_err("must refuse");
    assert!(
        err.message.contains("disabled"),
        "the honest disabled message, got: {err_message}",
        err_message = err.message
    );
    let runs: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM ai_runs WHERE type = 'draft_rewrite'",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);
    assert_eq!(runs, 0, "no run row for a refused rewrite");
}

#[test]
fn record_feedback_computes_edit_distance() {
    let conn = db();
    conn.execute(
        "INSERT INTO conversations (remote_id, number, subject, preview, status)
         VALUES (9501, 95, 'Draft conversation', 'preview', 'active')",
        [],
    )
    .expect("conversation");
    let conv = conn.last_insert_rowid();
    conn.execute(
        "INSERT INTO ai_drafts (conversation_id, content, state, model)
         VALUES (?1, 'Hello, we fixed the export timeout for you.', 'generated', 'm')",
        params![conv],
    )
    .expect("draft");
    let draft_id = conn.last_insert_rowid();
    record_feedback(
        &conn,
        draft_id,
        "Hello, we fixed the export timeout for you.",
        "Hi — the export timeout is fixed. Reply here if it recurs.",
        true,
    )
    .expect("feedback");
    set_draft_state(&conn, draft_id, "accepted").expect("state");
    let (distance, was_sent, state): (i64, i64, String) = conn
        .query_row(
            "SELECT f.edit_distance, f.was_sent, d.state
               FROM ai_feedback f JOIN ai_drafts d ON d.id = f.draft_id
              WHERE f.draft_id = ?1",
            params![draft_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .expect("feedback row");
    assert!(distance > 0, "a real edit distance was computed");
    assert_eq!(was_sent, 1);
    assert_eq!(state, "accepted");
}
