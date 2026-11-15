use super::*;
use rusqlite::Connection;
use serde_json::json;

fn db() -> Connection {
    let mut conn = Connection::open_in_memory().expect("db");
    crate::bootstrap::apply_all(&mut conn).expect("migrations");
    conn
}

fn seed_conversation(conn: &Connection) -> i64 {
    conn.execute(
        "INSERT INTO conversations (remote_id, number, subject, preview, status)
         VALUES (9801, 98, 'Password reset loop', 'preview', 'active')",
        [],
    )
    .expect("conversation");
    let conv = conn.last_insert_rowid();
    conn.execute(
        "INSERT INTO conversation_threads (conversation_id, type, body_text, created_at)
         VALUES (?1, 'customer', 'Every reset email sends me back to the login.', '2026-10-22T10:00:00Z')",
        params![conv],
    )
    .expect("thread");
    conv
}

fn chat_body(content: &str) -> crate::mockserver::MockResponse {
    crate::mockserver::MockResponse::json(
        200,
        json!({
            "model": "test-model",
            "choices": [{
                "message": {"role": "assistant", "content": content}
            }],
            "usage": {"prompt_tokens": 5, "completion_tokens": 3, "total_tokens": 8}
        })
        .to_string(),
    )
}

fn backend(mock: &crate::mockserver::MockServer) -> AiBackend {
    AiBackend::LmStudio {
        client: crate::ai_lm_studio::OpenAiCompatibleClient::new_with_timeout(mock.url(), 5_000),
        model: Some("test-model".into()),
        endpoint_id: None,
    }
}

fn rt() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime")
}

fn latest_run(conn: &Connection, kind: &str) -> (String, String, String) {
    conn.query_row(
        "SELECT status, response_json, COALESCE(error, '') FROM ai_runs WHERE type = ?1 ORDER BY id DESC LIMIT 1",
        params![kind],
        |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        },
    )
    .expect("run row")
}

#[test]
fn audit96_empty_analysis_object_is_rejected_not_cached() {
    let conn = db();
    let conv = seed_conversation(&conn);
    let mock = crate::mockserver::MockServer::start(crate::mockserver::scripted(vec![
        chat_body("{}"),
        chat_body(
            "{\"summary\": \"Customer is stuck in a reset loop\", \
             \"intent\": \"get_account_access\", \"urgency\": \"high\"}",
        ),
    ]))
    .expect("mock");

    let err = rt()
        .block_on(analyze_ticket(&conn, &backend(&mock), None, conv, false))
        .expect_err("an all-empty analysis is not a usable answer");
    assert!(
        err.message.contains("empty analysis"),
        "the failure names the real problem: {}",
        err.message
    );
    let (status, _response, error) = latest_run(&conn, "ticket_analysis");
    assert_eq!(status, "failed", "the ledger row records the failure");
    assert!(
        error.contains("no usable core field"),
        "the failure reason is honest: {error}"
    );

    let outcome = rt()
        .block_on(analyze_ticket(&conn, &backend(&mock), None, conv, false))
        .expect("a content-bearing analysis is accepted");
    assert_eq!(
        outcome.analysis.summary.as_deref(),
        Some("Customer is stuck in a reset loop")
    );
    let (status, _, _) = latest_run(&conn, "ticket_analysis");
    assert_eq!(status, "completed", "the good answer completes its run");
}

#[test]
fn audit96_cached_empty_analysis_degrades_to_fresh_computation() {
    let conn = db();
    let conv = seed_conversation(&conn);
    let signature = get_analysis_signature(&conn, conv);
    let hash = input_hash(conv, &signature);
    conn.execute(
        "INSERT INTO ai_runs (type, conversation_id, status, model, prompt_version,
                              input_hash, response_json, started_at, created_at)
         VALUES ('ticket_analysis', ?1, 'completed', 'test-model', ?2, ?3, '{}',
                 datetime('now'), datetime('now'))",
        params![conv, PROMPT_VERSIONS_TICKET_ANALYSIS, hash],
    )
    .expect("pre-fix run");

    let mock = crate::mockserver::MockServer::start(crate::mockserver::scripted(vec![chat_body(
        "{\"intent\": \"get_account_access\", \"summary\": \"Reset loop\"}",
    )]))
    .expect("mock");

    let outcome = rt()
        .block_on(analyze_ticket(&conn, &backend(&mock), None, conv, false))
        .expect("the stale empty cache degrades to a fresh computation");
    assert_eq!(
        outcome.analysis.intent.as_deref(),
        Some("get_account_access"),
        "the fresh answer is served"
    );
}

#[test]
fn audit95_weekly_digest_ledger_matches_the_narrative_verdict() {
    {
        let conn = db();
        let mock =
            crate::mockserver::MockServer::start(crate::mockserver::scripted(vec![chat_body("")]))
                .expect("mock");
        let id = rt()
            .block_on(weekly_digest(&conn, &backend(&mock), "manual", 7))
            .expect("the digest still archives its facts");
        let (status, narrative, _trigger, _facts) = digest_row(&conn, id);
        assert_eq!(status, "failed");
        assert!(narrative.is_none());
        let (status, _, _) = latest_run(&conn, "weekly_digest");
        assert_eq!(
            status, "failed",
            "the ledger row tells the same story as the digest row"
        );
    }

    {
        let conn = db();
        let mock =
            crate::mockserver::MockServer::start(crate::mockserver::scripted(vec![chat_body(
                "{\"narrative\": \"A quiet week: volume down, SLA met.\"}",
            )]))
            .expect("mock");
        let id = rt()
            .block_on(weekly_digest(&conn, &backend(&mock), "manual", 7))
            .expect("digest stored");
        let (status, narrative, _trigger, _facts) = digest_row(&conn, id);
        assert_eq!(status, "complete");
        assert_eq!(
            narrative.as_deref(),
            Some("A quiet week: volume down, SLA met.")
        );
        let (status, _, _) = latest_run(&conn, "weekly_digest");
        assert_eq!(
            status, "completed",
            "the run row completes with the narrative"
        );
    }
}
