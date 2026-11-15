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

fn candidate_kinds(conn: &Connection) -> Vec<String> {
    crate::db::query_rows(conn, "SELECT kind FROM knowledge_gap_candidates", [], |r| {
        r.get::<_, String>(0)
    })
    .unwrap_or_default()
}

#[test]
fn repeated_uncovered_question_becomes_gap() {
    let conn = db_with_question("how do I rotate the gateway keys");
    let (_, created) = rebuild_gaps(&conn, 90).expect("rebuild gaps");
    assert!(created >= 1, "one candidate expected, created {created}");
    let kinds = candidate_kinds(&conn);
    assert!(
        kinds.iter().any(|k| k == "repeated_question_uncovered"),
        "the uncovered kind landed: {kinds:?}"
    );
}

#[test]
fn docs_mirror_counts_as_documented() {
    let conn = db_with_question("how do I reach the control port");
    conn.execute(
        "INSERT INTO docs_collections (remote_id, slug, name)
         VALUES (50, 'help', 'Help center')",
        [],
    )
    .expect("collection");
    let collection = conn.last_insert_rowid();
    conn.execute(
        "INSERT INTO docs_articles (remote_id, collection_local_id, number, slug, name, status)
         VALUES (100, ?1, 1, 'control-port', 'Reaching the control port', 'published')",
        params![collection],
    )
    .expect("article");
    let article = conn.last_insert_rowid();
    conn.execute(
        "INSERT INTO docs_fts (name, text, article_id)
         VALUES ('Reaching the control port',
                 'How do you reach the control port? Open port 443 on the gateway console.',
                 ?1)",
        params![article],
    )
    .expect("docs fts");
    let (_, created) = rebuild_gaps(&conn, 90).expect("rebuild gaps");
    assert_eq!(created, 0, "a documented question is not a gap");
    let kinds = candidate_kinds(&conn);
    assert!(
        kinds.is_empty(),
        "no candidates when the help center covers it: {kinds:?}"
    );
}

#[test]
fn friction_rebuild_is_deterministic_and_deduped() {
    let mut conn = Connection::open_in_memory().expect("db");
    crate::bootstrap::apply_all(&mut conn).expect("migrations");
    conn.execute(
        "INSERT INTO conversations (remote_id, number, subject, preview, status)
         VALUES (9401, 94, 'Loop ticket', 'preview', 'active')",
        [],
    )
    .expect("conversation");
    let conv = conn.last_insert_rowid();
    for body in [
        "The export keeps failing every single night and nothing helps.",
        "Again today: the export keeps failing every single night.",
    ] {
        conn.execute(
            "INSERT INTO conversation_threads (conversation_id, type, state, body_text,
                                              remote_created_at, created_at)
             VALUES (?1, 'customer', 'published', ?2,
                     '2026-10-20T10:00:00Z', datetime('now'))",
            params![conv, body],
        )
        .expect("customer thread");
    }
    let (scanned_first, found_first) = rebuild_friction(&conn).expect("rebuild friction");
    assert!(scanned_first >= 1, "the seeded conversation is scanned");
    assert!(found_first >= 1, "repeated spans detected: {found_first}");
    let rows_first: i64 = conn
        .query_row("SELECT COUNT(*) FROM friction_findings", [], |r| r.get(0))
        .unwrap_or(0);
    assert!(
        rows_first >= 1,
        "findings persist for the agent tool to read"
    );
    let (_, found_second) = rebuild_friction(&conn).expect("rebuild friction");
    assert!(found_second >= 1, "the findings are still detected");
    let rows_second: i64 = conn
        .query_row("SELECT COUNT(*) FROM friction_findings", [], |r| r.get(0))
        .unwrap_or(0);
    assert_eq!(rows_second, rows_first, "no duplicate rows across rebuilds");
}
