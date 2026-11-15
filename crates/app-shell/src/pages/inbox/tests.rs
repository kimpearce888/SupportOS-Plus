use super::*;
use crate::testsupport::fixture;
use spp_core::event_bus::EventBus;

fn seeded() -> (crate::testsupport::ConvSeed, i64, i64, i64, i64) {
    (
        ("conv", 1, "Export broken", "active", "email", 1, 5, 9),
        1,
        5,
        9,
        1001,
    )
}

#[test]
fn filters_from_bar_maps_every_field() {
    let db = fixture(Vec::new());
    let bar = FilterBar {
        view: Some(ViewFilter::Active),
        mailbox: Some(2),
        channel: Some("email".into()),
        tag: Some("bug".into()),
        query: Some("export".into()),
        number: Some(42),
        ops: None,
    };
    let filters = db.with_read(|conn| filters_from_bar(&bar, 0, conn));
    assert_eq!(filters.status.as_deref(), Some("active"));
    assert_eq!(filters.mailbox_id, Some(2));
    assert_eq!(filters.channel.as_deref(), Some("email"));
    assert_eq!(filters.tag.as_deref(), Some("bug"));
    assert_eq!(filters.query.as_deref(), Some("export"));
    assert_eq!(filters.number, Some(42));
    assert_eq!(filters.limit, Some(INBOX_PAGE_SIZE));
    assert_eq!(filters.offset, Some(0));
    assert_eq!(ViewFilter::All.status(), None);
}

#[test]
fn ops_tile_filter_drills_into_unassigned_conversations() {
    let db = fixture(vec![seeded().0]);
    db.with_write(|conn| {
        crate::testsupport::seed_conversation(conn, 2, "Orphan", "active", "email", 1, 0, 10);
    });
    let bar = FilterBar {
        ops: Some("unassigned".into()),
        ..FilterBar::default()
    };
    let (items, total) = db
        .with_read(|conn| {
            let filters = filters_from_bar(&bar, 0, conn);
            spp_core::inbox::list_conversations(conn, &filters)
        })
        .expect("ops list");
    assert_eq!(total, 1, "only the unassigned conversation matches");
    assert_eq!(items[0].id, 2);

    let bad = FilterBar {
        ops: Some("' OR 1=1 --".into()),
        ..FilterBar::default()
    };
    let filters = db.with_read(|conn| filters_from_bar(&bad, 0, conn));
    assert!(filters.extra_where.is_none());
}

#[test]
fn reply_key_matches_op_send_reply_format() {
    let key = reply_key(1234, false, "hello");
    assert!(key.starts_with("reply:1234:send:"));
    assert_eq!(key.len(), "reply:1234:send:".len() + 24);
    let draft = reply_key(1234, true, "hello");
    assert!(draft.starts_with("reply:1234:draft:"));
    assert_ne!(key, draft);
}

#[test]
fn submit_reply_creates_durable_job_and_blocks_duplicates() {
    let db = fixture(vec![seeded().0]);
    let bus = EventBus::new(8);
    let detail = db
        .with_read(|conn| spp_core::inbox::get_conversation(conn, 1))
        .expect("detail")
        .expect("conversation 1 exists");

    let outcome = submit_reply(&db, &bus, &detail, "here is the fix", "", "");
    let message = match outcome {
        ActionOutcome::Success(m) => m,
        other => panic!("expected success, got {other:?}"),
    };
    assert!(
        message.contains("delivering to Help Scout"),
        "honest delivery wording expected, got: {message}"
    );

    let (kind, status, key, payload): (String, String, String, String) = db.with_read(|conn| {
        conn.query_row(
            "SELECT kind, status, idempotency_key, payload FROM outbound_jobs WHERE id = 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .expect("job row")
    });
    assert_eq!(kind, "create_reply");
    assert_eq!(status, "queued");
    let expected_key = reply_key(detail.remote_id, false, "here is the fix");
    assert_eq!(key, expected_key);
    assert!(payload.contains("here is the fix"));

    let second = submit_reply(&db, &bus, &detail, "here is the fix", "", "");
    assert!(matches!(second, ActionOutcome::Rejected(m) if m.contains("duplicate")));

    let third = submit_reply(&db, &bus, &detail, "different text", "", "");
    assert!(matches!(third, ActionOutcome::Success(_)));
}

#[test]
fn submit_note_adds_thread_row_jobs_and_mention_notifications() {
    let db = fixture(vec![seeded().0]);
    let bus = EventBus::new(8);
    let mut rx = bus.subscribe();
    let detail = db
        .with_read(|conn| spp_core::inbox::get_conversation(conn, 1))
        .expect("detail")
        .expect("conversation 1 exists");

    let outcome = submit_note(&db, &bus, &detail, "fyi @ada please check");
    assert!(matches!(outcome, ActionOutcome::Success(_)));

    db.with_read(|conn| {
        let detail = spp_core::inbox::get_conversation(conn, 1)
            .expect("detail")
            .expect("conversation");
        assert_eq!(detail.thread.len(), 1);
        assert_eq!(detail.thread[0].thread_type, "note");
        assert_eq!(detail.thread[0].body.as_deref(), Some("fyi @ada please check"));

        let (kind,): (String,) = conn
            .query_row(
                "SELECT kind FROM outbound_jobs ORDER BY id DESC LIMIT 1",
                [],
                |r| Ok((r.get(0)?,)),
            )
            .expect("job");
        assert_eq!(kind, "create_note");

        let mentions: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM notifications WHERE type = 'mentioned' AND target_user_id = 5",
                [],
                |r| r.get(0),
            )
            .expect("notifications");
        assert_eq!(mentions, 1);
    });

    let mut saw_update = false;
    while let Ok(event) = rx.try_recv() {
        if matches!(event, spp_core::events::ServerEvent::ConversationUpdated(_)) {
            saw_update = true;
        }
    }
    assert!(saw_update);
}

#[test]
fn change_status_writes_locally_and_queues_remote_job() {
    let db = fixture(vec![seeded().0]);
    let bus = EventBus::new(8);
    let detail = db
        .with_read(|conn| spp_core::inbox::get_conversation(conn, 1))
        .expect("detail")
        .expect("conversation 1 exists");

    let outcome = mutate(
        &db,
        &bus,
        &detail,
        TicketOperation::ChangeStatus {
            conversation_remote_id: detail.remote_id,
            new_status: "closed".into(),
            actor_type: "user".into(),
            actor_id: None,
        },
        "update_status",
        &json!({ "conversationId": detail.id, "status": "closed" }),
    );
    assert!(matches!(outcome, ActionOutcome::Success(_)));

    db.with_read(|conn| {
        let status: String = conn
            .query_row("SELECT status FROM conversations WHERE id = 1", [], |r| {
                r.get(0)
            })
            .expect("status");
        assert_eq!(status, "closed");
        let (kind,): (String,) = conn
            .query_row(
                "SELECT kind FROM outbound_jobs ORDER BY id DESC LIMIT 1",
                [],
                |r| Ok((r.get(0)?,)),
            )
            .expect("job");
        assert_eq!(kind, "update_status");
    });
}

#[test]
fn eval_mode_blocks_all_mutations() {
    let db = fixture(vec![seeded().0]);
    db.with_write(|conn| {
        spp_core::settings::set_string(conn, "ai_evaluation_mode", "true").expect("set eval mode");
    });
    let bus = EventBus::new(8);
    let detail = db
        .with_read(|conn| spp_core::inbox::get_conversation(conn, 1))
        .expect("detail")
        .expect("conversation 1 exists");

    assert!(matches!(
        submit_note(&db, &bus, &detail, "note"),
        ActionOutcome::Rejected(_)
    ));
    assert!(matches!(
        submit_reply(&db, &bus, &detail, "reply", "", ""),
        ActionOutcome::Rejected(_)
    ));
    let jobs: i64 = db.with_read(|conn| {
        conn.query_row("SELECT COUNT(*) FROM outbound_jobs", [], |r| r.get(0))
            .expect("jobs")
    });
    assert_eq!(jobs, 0, "eval mode must not create jobs");
}

#[test]
fn assignable_users_and_tags_list() {
    let db = fixture(vec![("conv", 1, "Tagged", "active", "email", 1, 5, 9)]);
    db.with_write(|conn| {
        crate::testsupport::seed_tag(conn, 1, "billing");
    });
    let users = db.with_read(assignable_users);
    assert!(users.contains(&(5, "Ada Lovelace".into())));
    let tags = db.with_read(tag_names);
    assert_eq!(tags, vec!["billing".to_string()]);
}

#[test]
fn r4_d9_reply_carries_the_configured_status_after() {
    let db = fixture(vec![seeded().0]);
    let bus = EventBus::new(8);
    let detail = db
        .with_read(|conn| spp_core::inbox::get_conversation(conn, 1))
        .expect("detail")
        .expect("conversation 1 exists");

    let outcome = submit_reply(&db, &bus, &detail, "status after test", "", "");
    assert!(matches!(outcome, ActionOutcome::Success(_)));
    let payload: String = db.with_read(|conn| {
        conn.query_row(
            "SELECT payload FROM outbound_jobs ORDER BY id DESC LIMIT 1",
            [],
            |r| r.get(0),
        )
        .expect("job row")
    });
    let parsed: serde_json::Value = serde_json::from_str(&payload).expect("payload json");
    assert_eq!(
        parsed["statusAfter"], "pending",
        "D9: the default reply statusAfter is pending"
    );

    db.with_write(|conn| spp_core::settings::set_string(conn, "reply_status_after", "keep"))
        .expect("set keep");
    let outcome = submit_reply(&db, &bus, &detail, "keep setting text", "", "");
    assert!(matches!(outcome, ActionOutcome::Success(_)));
    let payload: String = db.with_read(|conn| {
        conn.query_row(
            "SELECT payload FROM outbound_jobs ORDER BY id DESC LIMIT 1",
            [],
            |r| r.get(0),
        )
        .expect("job row")
    });
    let parsed: serde_json::Value = serde_json::from_str(&payload).expect("payload json");
    assert!(
        parsed["statusAfter"].is_null(),
        "D9: keep means no statusAfter on the wire"
    );
}

#[test]
fn r4_d8_a_failed_enqueue_is_rejected_instead_of_faking_success() {
    let db = fixture(vec![seeded().0]);
    let bus = EventBus::new(8);
    let detail = db
        .with_read(|conn| spp_core::inbox::get_conversation(conn, 1))
        .expect("detail")
        .expect("conversation 1 exists");

    db.with_write(|conn| conn.execute_batch("DROP TABLE outbound_jobs"))
        .expect("drop the queue");

    let outcome = submit_reply(&db, &bus, &detail, "must be rejected", "", "");
    match outcome {
        ActionOutcome::Rejected(reason) => {
            assert!(
                !reason.is_empty(),
                "the failure must carry the database's reason"
            );
        }
        other => panic!("expected rejection, got {other:?}"),
    }
}

#[test]
fn audit24_poll_fills_the_composer_from_the_jobs_own_result() {
    let db = fixture(vec![seeded().0]);
    db.with_write(|conn| {
        conn.execute(
            "INSERT INTO ai_runs (type, conversation_id, status, model, prompt_version,
                                   input_hash, input_refs, created_at, started_at,
                                   response_json)
             VALUES ('draft_rewrite', NULL, 'completed', 'some-model', 'draft_rewrite_v1',
                     'bait', '[]', datetime('now'), datetime('now'),
                     '{\"output\":\"FOREIGN HISTORICAL TEXT\",\"instruction\":\"warmer\"}')",
            [],
        )
        .expect("foreign run");
    });
    let job_id = db.with_write(|conn| {
        spp_core::jobs::enqueue_on(conn, "ai", "rewrite_composer", "{}", 3, 2).expect("enqueue")
    });
    db.with_write(|conn| {
        conn.execute(
            "UPDATE jobs SET status = 'completed', completed_at = datetime('now'),
                             result = 'THE JOB RESULT' WHERE id = ?1",
            rusqlite::params![job_id],
        )
        .expect("complete with result")
    });

    let mut page = InboxPage {
        pending_ai: Some(PendingComposerAi {
            job_id,
            kind: "rewrite_composer",
            conversation_id: 1,
            origin_body: String::new(),
        }),
        ..Default::default()
    };
    page.poll_pending_ai(&db);
    assert_eq!(page.composer_body, "THE JOB RESULT");
    assert!(page.pending_ai.is_none(), "the poll resolves the job");
    assert!(!page.status_is_error, "a real result is a success");

    let job_id2 = db.with_write(|conn| {
        spp_core::jobs::enqueue_on(conn, "ai", "translate_composer", "{}", 3, 2).expect("enqueue")
    });
    db.with_write(|conn| {
        conn.execute(
            "UPDATE jobs SET status = 'completed', completed_at = datetime('now') WHERE id = ?1",
            rusqlite::params![job_id2],
        )
        .expect("complete without result")
    });
    page.composer_body.clear();
    page.pending_ai = Some(PendingComposerAi {
        job_id: job_id2,
        kind: "translate_composer",
        conversation_id: 1,
        origin_body: String::new(),
    });
    page.poll_pending_ai(&db);
    assert!(
        page.composer_body.is_empty(),
        "no foreign text may leak into the composer"
    );
    assert!(page.status_is_error, "the honest no-result message shows");
}

#[test]
fn audit73_transform_completion_never_overwrites_in_flight_typing() {
    let db = fixture(vec![seeded().0]);
    let job_id = db.with_write(|conn| {
        spp_core::jobs::enqueue_on(conn, "ai", "rewrite_composer", "{}", 3, 2).expect("enqueue")
    });
    db.with_write(|conn| {
        conn.execute(
            "UPDATE jobs SET status = 'completed', completed_at = datetime('now'),
                             result = 'REWRITTEN RESULT' WHERE id = ?1",
            rusqlite::params![job_id],
        )
        .expect("complete with result")
    });

    let mut page = InboxPage {
        pending_ai: Some(PendingComposerAi {
            job_id,
            kind: "rewrite_composer",
            conversation_id: 1,
            origin_body: "original draft".into(),
        }),
        composer_body: "the user typed this in the meantime".into(),
        ..Default::default()
    };
    page.poll_pending_ai(&db);
    assert_eq!(
        page.composer_body, "the user typed this in the meantime",
        "in-flight typing wins over the finishing transform"
    );
    assert!(page.pending_ai.is_none(), "the poll still resolves the job");
    assert!(
        page.status_message
            .as_deref()
            .is_some_and(|m| m.contains("your text was kept")),
        "the kept-edit decision is said out loud: {:?}",
        page.status_message
    );
    assert!(!page.status_is_error, "keeping the edit is not an error");

    let job_id2 = db.with_write(|conn| {
        spp_core::jobs::enqueue_on(conn, "ai", "rewrite_composer", "{}", 3, 2).expect("enqueue")
    });
    db.with_write(|conn| {
        conn.execute(
            "UPDATE jobs SET status = 'completed', completed_at = datetime('now'),
                             result = 'REWRITTEN RESULT' WHERE id = ?1",
            rusqlite::params![job_id2],
        )
        .expect("complete with result")
    });
    page.pending_ai = Some(PendingComposerAi {
        job_id: job_id2,
        kind: "rewrite_composer",
        conversation_id: 1,
        origin_body: "original draft".into(),
    });
    page.composer_body = "original draft".into();
    page.poll_pending_ai(&db);
    assert_eq!(
        page.composer_body, "REWRITTEN RESULT",
        "an unedited composer still receives the transform"
    );
}

#[test]
fn audit25_open_is_the_await_agent_status_and_legacy_waiting_maps_to_it() {
    let db = fixture(vec![seeded().0]);
    let bus = EventBus::new(8);
    let detail = db
        .with_read(|conn| spp_core::inbox::get_conversation(conn, 1))
        .expect("detail")
        .expect("conversation 1 exists");

    let status_after_for = |text: &str| -> serde_json::Value {
        let outcome = submit_reply(&db, &bus, &detail, text, "", "");
        assert!(matches!(outcome, ActionOutcome::Success(_)));
        let payload: String = db.with_read(|conn| {
            conn.query_row(
                "SELECT payload FROM outbound_jobs ORDER BY id DESC LIMIT 1",
                [],
                |r| r.get(0),
            )
            .expect("job row")
        });
        serde_json::from_str(&payload).expect("payload json")
    };

    db.with_write(|conn| spp_core::settings::set_string(conn, "reply_status_after", "open"))
        .expect("set open");
    let parsed = status_after_for("open setting text");
    assert_eq!(
        parsed["statusAfter"], "open",
        "the await-agent status must be the platform's own `open`"
    );

    db.with_write(|conn| spp_core::settings::set_string(conn, "reply_status_after", "waiting"))
        .expect("set legacy waiting");
    let parsed = status_after_for("legacy waiting text");
    assert_eq!(
        parsed["statusAfter"], "open",
        "the legacy phantom status maps onto the real one"
    );
}

#[test]
fn ensure_loaded_latches_and_invalidate_refetches() {
    let db = crate::testsupport::empty_db();
    let mut page = InboxPage::new();
    assert!(!page.refs_loaded);
    assert!(!page.list_loaded);
    page.ensure_loaded(&db);

    assert!(page.refs_loaded);
    assert!(page.list_loaded);
    assert!(page.mailboxes.is_empty(), "no mailboxes seeded yet");
    assert!(page.list.is_empty(), "a legitimately empty mirror");

    page.status = Some("sentinel".into());
    page.ensure_loaded(&db);
    assert_eq!(
        page.status.as_deref(),
        Some("sentinel"),
        "both gates latch: no reload ran (pre-fix reload_refs + reload_list re-ran every frame on an empty mirror)"
    );

    db.with_write(|conn| {
        conn.execute(
            "INSERT INTO mailboxes (remote_id, name, email) VALUES (778, 'Latched', 'latched@example.test')",
            [],
        )
    })
    .expect("mailbox");
    page.invalidate();
    assert!(!page.refs_loaded);
    assert!(!page.list_loaded);
    page.ensure_loaded(&db);
    assert!(page.refs_loaded);
    assert!(page.list_loaded);
    assert_eq!(page.mailboxes.len(), 1, "invalidate refetches the refs");
}
