use super::*;
use crate::ai_prompts::{ClusterConversation, TicketAnalysis};
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
         VALUES (9401, 94, 'Invite emails bouncing', 'preview', 'active')",
        [],
    )
    .expect("conversation");
    let conv = conn.last_insert_rowid();
    conn.execute(
        "INSERT INTO conversation_threads (conversation_id, type, body_text, created_at)
         VALUES (?1, 'customer', 'Our invite emails bounce with a 550.', '2026-10-20T10:00:00Z')",
        params![conv],
    )
    .expect("thread");
    conv
}

fn analysis_json(intent: &str, summary: &str) -> String {
    let a = TicketAnalysis {
        intent: Some(intent.into()),
        summary: Some(summary.into()),
        ..Default::default()
    };
    serde_json::to_string(&a).expect("analysis json")
}

fn seed_run(
    conn: &Connection,
    conv: i64,
    input_hash: &str,
    intent: &str,
    summary: &str,
    source_title: Option<&str>,
) -> i64 {
    conn.execute(
        "INSERT INTO ai_runs (type, conversation_id, status, model, prompt_version,
                              input_hash, response_json, started_at, created_at)
         VALUES ('ticket_analysis', ?1, 'completed', 'test-model', ?2, ?3, ?4,
                 datetime('now'), datetime('now'))",
        params![
            conv,
            PROMPT_VERSIONS_TICKET_ANALYSIS,
            input_hash,
            analysis_json(intent, summary)
        ],
    )
    .expect("run");
    let run_id = conn.last_insert_rowid();
    if let Some(title) = source_title {
        conn.execute(
            "INSERT INTO ai_sources (run_id, source_type, source_id, title, relevance, visibility, timestamp)
             VALUES (?1, 'conversation', 94, ?2, 0.9, 'internal', '2026-10-20T10:00:00Z')",
            params![run_id, title],
        )
        .expect("source");
    }
    run_id
}

#[test]
fn cache_hit_serves_the_hash_matched_runs_own_analysis() {
    let conn = db();
    let conv = seed_conversation(&conn);
    let hash = input_hash(conv, &get_analysis_signature(&conn, conv));

    let run_a = seed_run(
        &conn,
        conv,
        &hash,
        "question",
        "A: the invite bounces",
        Some("A-source"),
    );
    let _run_b = seed_run(
        &conn,
        conv,
        "stale-hash",
        "bug_report",
        "B: newer analysis",
        Some("B-source"),
    );

    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime")
        .block_on(analyze_ticket(
            &conn,
            &AiBackend::Disabled,
            None,
            conv,
            false,
        ))
        .expect("the cache hit needs no backend");

    let (run_id, output): (i64, String) = conn
        .query_row(
            "SELECT id, response_json FROM ai_runs
              WHERE type='ticket_analysis' AND status='completed' AND input_hash = ?1",
            [&hash],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .expect("the hash-matched analysis run");
    assert_eq!(run_id, run_a, "provenance points at the hash-matched run");
    let analysis: crate::ai_prompts::TicketAnalysis =
        serde_json::from_str(&output).expect("analysis json");
    assert_eq!(
        analysis.intent.as_deref(),
        Some("question"),
        "the analysis is the hash-matched run's, not the newest run's"
    );
    assert_eq!(analysis.summary.as_deref(), Some("A: the invite bounces"));
    let titles: Vec<String> = {
        let mut stmt = conn
            .prepare("SELECT title FROM ai_sources WHERE run_id = ?1")
            .expect("sources query");
        stmt.query_map([run_a], |r| r.get::<_, String>(0))
            .expect("sources rows")
            .filter_map(|r| r.ok())
            .collect::<Vec<_>>()
    };
    assert_eq!(titles.len(), 1, "the sources are the hash-matched run's");
    assert_eq!(titles[0], "A-source");
}

#[test]
fn draft_fallback_no_longer_stores_the_raw_json_envelope() {
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

    {
        let conn = db();
        let conv = seed_conversation(&conn);
        let mock =
            crate::mockserver::MockServer::start(crate::mockserver::scripted(vec![chat_body(
                "{\"reply\": \"Dear customer, the fix is on the way.\"}",
            )]))
            .expect("mock");
        let backend = AiBackend::LmStudio {
            client: crate::ai_lm_studio::OpenAiCompatibleClient::new_with_timeout(
                mock.url(),
                5_000,
            ),
            model: Some("test-model".into()),
            endpoint_id: None,
        };
        let analysis = TicketAnalysis::default();
        let err = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime")
            .block_on(generate_draft(
                &conn,
                &backend,
                None,
                conv,
                "reply",
                false,
                Some(&analysis),
            ))
            .expect_err("an envelope without a draft field must fail loudly");
        assert!(
            err.message.contains("without a draft field"),
            "the failure names the real problem: {}",
            err.message
        );
        let status: String = conn
            .query_row(
                "SELECT status FROM ai_runs WHERE type = 'customer_draft' ORDER BY id DESC LIMIT 1",
                [],
                |r| r.get(0),
            )
            .expect("the run row exists");
        let drafts: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM ai_drafts WHERE conversation_id = ?1",
                params![conv],
                |r| r.get(0),
            )
            .unwrap_or(0);
        assert_eq!(status, "failed", "the run is failed, not completed");
        assert_eq!(drafts, 0, "no draft row stores the envelope");
    }

    {
        let conn = db();
        let conv = seed_conversation(&conn);
        let mock = crate::mockserver::MockServer::start(crate::mockserver::scripted(vec![
            chat_body("Dear customer, we cleared the invite queue and retried the batch."),
            crate::mockserver::MockResponse::json(
                200,
                json!({
                    "model": "test-model",
                    "choices": [{
                        "message": {"role": "assistant",
                                    "content": "{\"verified\": true, \"warnings\": []}"}
                    }],
                    "usage": {"prompt_tokens": 4, "completion_tokens": 2, "total_tokens": 6}
                })
                .to_string(),
            ),
        ]))
        .expect("mock");
        let backend = AiBackend::LmStudio {
            client: crate::ai_lm_studio::OpenAiCompatibleClient::new_with_timeout(
                mock.url(),
                5_000,
            ),
            model: Some("test-model".into()),
            endpoint_id: None,
        };
        let analysis = TicketAnalysis::default();
        let (draft, _) = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime")
            .block_on(generate_draft(
                &conn,
                &backend,
                None,
                conv,
                "reply",
                false,
                Some(&analysis),
            ))
            .expect("the prose draft is accepted");
        assert_eq!(
            draft.content, "Dear customer, we cleared the invite queue and retried the batch.",
            "the prose answer is the draft, verbatim"
        );
    }
}

#[test]
fn interaction_strategy_block_renders_real_strategy_data() {
    let conn = db();
    let bare = seed_conversation(&conn);
    assert!(
        build_interaction_strategy_block(&conn, bare).is_none(),
        "no stored signals - no block, honestly"
    );

    conn.execute(
        "INSERT INTO customers (remote_id, first_name, last_name) VALUES (81, 'Ada', 'Lovelace')",
        [],
    )
    .expect("customer");
    let customer = conn.last_insert_rowid();
    conn.execute(
        "UPDATE conversations SET customer_local_id = ?1 WHERE id = ?2",
        params![customer, bare],
    )
    .expect("link customer");
    conn.execute(
        "INSERT INTO conversation_threads (conversation_id, type, body_text, created_at)
         VALUES (?1, 'customer', 'I attached a screenshot of the bounce; already tried a resend.', '2026-10-21T10:00:00Z')",
        params![bare],
    )
    .expect("thread");

    let signals = vec![
        crate::interaction_signals::InteractionSignal {
            dimension: "frustration".into(),
            value: "moderate".into(),
            confidence: "high".into(),
            evidence: None,
            source: "heuristic".into(),
        },
        crate::interaction_signals::InteractionSignal {
            dimension: "detail".into(),
            value: "high".into(),
            confidence: "medium".into(),
            evidence: None,
            source: "heuristic".into(),
        },
    ];
    let stats = crate::interaction_signals::MessageStats {
        customer_messages: 2,
        avg_message_length: 40,
        question_count: 1,
        exclamation_ratio: 0.0,
        caps_ratio: 0.0,
    };
    crate::interaction_engine::save_current_interaction(
        &conn,
        bare,
        Some(customer),
        &signals,
        &stats,
        Some("get the invites delivered"),
        "heuristic",
        None,
    )
    .expect("current interaction");
    let block =
        build_interaction_strategy_block(&conn, bare).expect("the heuristic carries the block");
    assert!(
        block.contains("COMMUNICATION APPROACH"),
        "the section renders: {block}"
    );
    assert!(
        block.contains("ALREADY PROVIDED BY THE CUSTOMER"),
        "the already-provided facts render: {block}"
    );
    assert!(
        block.contains("screenshots/attachments"),
        "the screenshot fact from the customer's message is listed: {block}"
    );
    assert!(
        block.contains("do NOT ask again"),
        "the instruction not to re-ask is present: {block}"
    );

    crate::interaction_engine::save_recommendation(
        &conn,
        bare,
        &crate::interaction_engine::SupportApproach {
            tone: Some("warm".into()),
            length: Some("concise".into()),
            start_with: None,
            then: None,
            avoid: vec!["long background explanations".into()],
            response_strategy: vec![
                "Acknowledge the bounce".into(),
                "State the retry plan".into(),
            ],
            de_escalation: false,
            escalation_recommendation: None,
            why: vec![],
            source: "ai".into(),
            confidence: "medium".into(),
        },
        None,
        None,
    )
    .expect("recommendation");
    let block = build_interaction_strategy_block(&conn, bare).expect("the block renders");
    assert!(block.contains("Tone: warm"), "{block}");
    assert!(
        block.contains("Acknowledge the bounce -> State the retry plan"),
        "{block}"
    );
    assert!(
        block.contains("Avoid: long background explanations"),
        "{block}"
    );
}

#[test]
fn issue_cluster_prompt_declares_the_sample_cap() {
    let sample = vec![
        ClusterConversation {
            number: 9,
            subject: "Invite emails bouncing".into(),
            preview: "550 error".into(),
            tags: vec!["email".into()],
        },
        ClusterConversation {
            number: 10,
            subject: "More invites bouncing".into(),
            preview: "550 again".into(),
            tags: vec![],
        },
    ];
    let capped = build_issue_cluster_user(&sample, 500);
    assert!(
        capped.contains("the window holds 500 conversations; only the newest 2 are listed"),
        "the cap is said out loud: {capped}"
    );
    assert!(capped.contains("Cluster only the listed ones."), "{capped}");
    let uncapped = build_issue_cluster_user(&sample, 2);
    assert!(
        !uncapped.contains("NOTE: the window holds"),
        "no sampling note when everything is listed: {uncapped}"
    );
}
