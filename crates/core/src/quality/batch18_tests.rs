use super::*;
use rusqlite::Connection;

fn db_with_question(question: &str) -> Connection {
    let mut conn = Connection::open_in_memory().expect("db");
    crate::bootstrap::apply_all(&mut conn).expect("migrations");
    for i in 0..2 {
        conn.execute(
            "INSERT INTO conversations (remote_id, number, subject, preview, status)
             VALUES (?1, ?1, 'Recurring issue', 'preview', 'closed')",
            params![9300 + i],
        )
        .expect("conversation");
        let conv = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO ai_runs (type, conversation_id, status, model, prompt_version,
                                  input_hash, response_json, created_at, started_at)
             VALUES ('ticket_analysis', ?1, 'completed', 'm', 'ticket_analysis_v1',
                     'seeded', ?2, datetime('now'), datetime('now'))",
            params![conv, format!("{{\"primary_question\": \"{question}\"}}")],
        )
        .expect("analysis run");
    }
    conn
}

fn conversation_ids(conn: &Connection) -> Vec<i64> {
    crate::db::query_rows(conn, "SELECT id FROM conversations ORDER BY id", [], |r| {
        r.get::<_, i64>(0)
    })
    .unwrap_or_default()
}

fn candidate_kinds(conn: &Connection) -> Vec<String> {
    crate::db::query_rows(conn, "SELECT kind FROM knowledge_gap_candidates", [], |r| {
        r.get::<_, String>(0)
    })
    .unwrap_or_default()
}

fn seed_knowledge_hit(conn: &Connection) {
    conn.execute(
        "INSERT INTO knowledge_sources (name) VALUES ('test source')",
        [],
    )
    .expect("source");
    let source = conn.last_insert_rowid();
    conn.execute(
        "INSERT INTO knowledge_documents (source_id, title, content)
         VALUES (?1, 'Key rotation', 'Rotate the gateway keys from the console.')",
        params![source],
    )
    .expect("document");
    let document = conn.last_insert_rowid();
    conn.execute(
        "INSERT INTO fts_knowledge (title, content, chunk_id, document_id, visibility)
         VALUES ('Key rotation',
                 'How to rotate the gateway keys from the console when they expire.',
                 NULL, ?1, 'internal_only')",
        params![document],
    )
    .expect("fts row");
}

fn seed_outcome(conn: &Connection, conversation: i64, follow_ups: i64, clarifications: i64) {
    conn.execute(
        "INSERT INTO client_support_outcomes
             (customer_id, conversation_id, follow_up_count, clarification_count)
         VALUES (1, ?1, ?2, ?3)",
        params![conversation, follow_ups, clarifications],
    )
    .expect("outcome");
}

#[test]
fn repeated_unsolved_question_becomes_gap_on_real_interaction_data() {
    let conn = db_with_question("how do I rotate the gateway keys");
    seed_knowledge_hit(&conn);
    for id in conversation_ids(&conn) {
        seed_outcome(&conn, id, 1, 0);
    }
    let (_, created) = rebuild_gaps(&conn, 90);
    let kinds = candidate_kinds(&conn);
    assert!(
        created >= 1 && kinds.iter().any(|k| k == "repeated_question_unsolved"),
        "the unsolved kind must land on friction outcomes: created={created} kinds={kinds:?}"
    );
}

#[test]
fn cleanly_resolved_repeated_questions_stay_out_of_the_unsolved_gap() {
    let conn = db_with_question("how do I rotate the gateway keys");
    seed_knowledge_hit(&conn);
    for id in conversation_ids(&conn) {
        seed_outcome(&conn, id, 0, 0);
    }
    rebuild_gaps(&conn, 90);
    let kinds = candidate_kinds(&conn);
    assert!(
        !kinds.iter().any(|k| k == "repeated_question_unsolved"),
        "no friction outcomes means no unsolved gap: {kinds:?}"
    );
}

#[test]
fn qa_recompute_preserves_the_stored_ai_layer() {
    let mut conn = Connection::open_in_memory().expect("db");
    crate::bootstrap::apply_all(&mut conn).expect("migrations");
    conn.execute(
        "INSERT INTO conversations (remote_id, number, subject, preview, status)
         VALUES (9501, 95, 'QA ticket', 'preview', 'closed')",
        [],
    )
    .expect("conversation");
    let conv = conn.last_insert_rowid();
    for (kind, body) in [
        ("customer", "the export keeps failing every single night"),
        ("reply", "Nightly exports are repaired from Settings."),
    ] {
        conn.execute(
            "INSERT INTO conversation_threads (conversation_id, type, state, body_text)
             VALUES (?1, ?2, 'published', ?3)",
            params![conv, kind, body],
        )
        .expect("thread");
    }

    compute_qa_deterministic(&conn, conv).expect("first compute");
    conn.execute(
        "UPDATE post_resolution_qa
            SET ai = '{\"answered\":\"yes\"}', ai_run_id = 42,
                checked_at = '2026-01-02T00:00:00Z', recomputed_at = '2026-01-02T00:00:00Z'
          WHERE conversation_id = ?1",
        params![conv],
    )
    .expect("seed ai layer");

    compute_qa_deterministic(&conn, conv).expect("second compute");

    let (rows, ai, ai_run_id, recomputed): (i64, Option<String>, Option<i64>, Option<String>) =
        conn.query_row(
            "SELECT COUNT(*), ai, ai_run_id, recomputed_at FROM post_resolution_qa
              WHERE conversation_id = ?1",
            params![conv],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .expect("qa row");
    assert_eq!(rows, 1, "exactly one row survives a recompute");
    assert_eq!(
        ai.as_deref(),
        Some("{\"answered\":\"yes\"}"),
        "the stored AI layer must survive the maintenance rebuild"
    );
    assert_eq!(ai_run_id, Some(42));
    assert_eq!(recomputed.as_deref(), Some("2026-01-02T00:00:00Z"));

    let (det, computed_at): (String, Option<String>) = conn
        .query_row(
            "SELECT deterministic, computed_at FROM post_resolution_qa
              WHERE conversation_id = ?1",
            params![conv],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .expect("deterministic row");
    assert!(det.contains("\"conversation_local_id\""), "{det}");
    assert!(computed_at.is_some());
}

#[test]
fn qa_compute_still_inserts_when_the_row_is_missing() {
    let mut conn = Connection::open_in_memory().expect("db");
    crate::bootstrap::apply_all(&mut conn).expect("migrations");
    conn.execute(
        "INSERT INTO conversations (remote_id, number, subject, preview, status)
         VALUES (9502, 96, 'QA ticket two', 'preview', 'closed')",
        [],
    )
    .expect("conversation");
    let conv = conn.last_insert_rowid();
    conn.execute(
        "INSERT INTO conversation_threads (conversation_id, type, state, body_text)
         VALUES (?1, 'customer', 'published', 'where are my invoices stored')",
        params![conv],
    )
    .expect("thread");

    assert!(compute_qa_deterministic(&conn, conv).is_some());
    let rows: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM post_resolution_qa WHERE conversation_id = ?1",
            params![conv],
            |r| r.get(0),
        )
        .expect("count");
    assert_eq!(rows, 1);
}
