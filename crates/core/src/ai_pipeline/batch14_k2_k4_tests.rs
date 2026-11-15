use super::*;
use rusqlite::Connection;

fn db() -> Connection {
    let mut conn = Connection::open_in_memory().expect("db");
    crate::bootstrap::apply_all(&mut conn).expect("migrations");
    conn
}

fn seed_candidate_with_resolution(conn: &Connection) -> i64 {
    conn.execute(
        "INSERT INTO conversations (remote_id, number, subject, preview, status,
                                    remote_created_at, closed_at)
         VALUES (701, 701, 'Export timeout on weekly report', 'p', 'closed',
                 '2026-10-18T08:00:00.000Z', '2026-10-19T08:00:00.000Z')",
        [],
    )
    .expect("conversation");
    let conv = conn.last_insert_rowid();
    conn.execute(
        "INSERT INTO conversation_threads (conversation_id, type, state, body_text, remote_created_at)
         VALUES (?1, 'reply', 'published',
                 'Clear the export cache and re-run with the nightly profile.', '2026-10-19T07:00:00.000Z')",
        params![conv],
    )
    .expect("thread");
    conn.execute(
        "INSERT INTO knowledge_gap_candidates (dedup_key, query_text, occurrence_count,
             kind, evidence_conversation_ids, detail, status, provenance)
         VALUES ('k2:test', 'how do I fix the export timeout', 3, 'repeated_question_uncovered',
             ?, '{}', 'open', 'deterministic_local')",
        params![serde_json::to_string(&[conv]).unwrap_or_default()],
    )
    .expect("candidate");
    conn.last_insert_rowid()
}

#[test]
fn draft_article_refuses_honestly() {
    let conn = db();
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");

    let err = rt
        .block_on(draft_article(&conn, &AiBackend::Disabled, 4242))
        .expect_err("unknown candidate");
    assert!(
        err.message.contains("does not exist"),
        "{err_message}",
        err_message = err.message
    );

    let candidate = seed_candidate_with_resolution(&conn);
    let err = rt
        .block_on(draft_article(&conn, &AiBackend::Disabled, candidate))
        .expect_err("no backend");
    assert!(
        err.message.contains("disabled"),
        "the honest disabled message, got: {err_message}",
        err_message = err.message
    );
    let runs: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM ai_runs WHERE type = 'article_draft'",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);
    assert_eq!(runs, 0, "no run row for a refused draft");

    conn.execute(
        "INSERT INTO conversations (remote_id, number, subject, preview, status, remote_created_at)
         VALUES (702, 702, 'Still open', 'p', 'active', '2026-10-20T08:00:00.000Z')",
        [],
    )
    .expect("open");
    let open = conn.last_insert_rowid();
    conn.execute(
        "INSERT INTO knowledge_gap_candidates (dedup_key, query_text, occurrence_count,
             kind, evidence_conversation_ids, detail, status, provenance)
         VALUES ('k2:open', 'unresolved question', 2, 'repeated_question_uncovered',
             ?, '{}', 'open', 'deterministic_local')",
        params![serde_json::to_string(&[open]).unwrap_or_default()],
    )
    .expect("candidate 2");
    let open_candidate = conn.last_insert_rowid();
    let err = rt
        .block_on(draft_article(&conn, &AiBackend::Disabled, open_candidate))
        .expect_err("no evidence");
    assert!(
        err.message.contains("nothing grounded"),
        "grounded-evidence refusal, got: {err_message}",
        err_message = err.message
    );
}

#[test]
fn weekly_digest_archives_facts_without_backend() {
    let conn = db();
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    let id = rt
        .block_on(weekly_digest(&conn, &AiBackend::Disabled, "manual", 7))
        .expect("digest stored");
    let (status, narrative, trigger_kind, facts) = digest_row(&conn, id);
    assert_eq!(status, "facts_only");
    assert!(narrative.is_none());
    assert_eq!(trigger_kind, "manual");
    assert!(facts["volume"]["new_conversations"].is_number());
    assert!(facts["what_changed"]["new_gap_candidates"].is_array());
    assert_eq!(
        crate::digest::list_digests(&conn, Some("deterministic digest facts"), 5).len(),
        1
    );
}
