use super::*;
use crate::testsupport::fixture;

#[test]
fn set_priority_writes_the_local_priority_without_an_outbound_job() {
    let db = fixture(vec![(
        "conv",
        1,
        "Export broken",
        "active",
        "email",
        1,
        5,
        9,
    )]);
    let bus = EventBus::new(8);
    let mut page = InboxPage::new();
    page.selected = Some(1);
    page.detail = Some(
        db.with_read(|conn| spp_core::inbox::get_conversation(conn, 1))
            .expect("detail")
            .expect("conversation 1 exists"),
    );

    let outcome = page.set_priority(&db, &bus, "urgent");
    assert!(
        matches!(outcome, ActionOutcome::Success(ref m) if m.contains("urgent")),
        "the write path succeeds: {outcome:?}"
    );

    db.with_read(|conn| {
        let (priority, transitions): (String, i64) = conn
            .query_row(
                "SELECT supportos_priority,
                        (SELECT COUNT(*) FROM ticket_state_transitions
                          WHERE conversation_remote_id = 1001)
                   FROM conversations WHERE id = 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .expect("conversation");
        assert_eq!(priority, "urgent");
        assert_eq!(transitions, 1, "the transition is recorded");
        let outbound: i64 = conn
            .query_row("SELECT COUNT(*) FROM outbound_jobs", [], |r| r.get(0))
            .expect("outbound count");
        assert_eq!(outbound, 0, "a local priority is not pushed to Help Scout");
    });

    assert_eq!(
        page.detail.as_ref().and_then(|d| d.priority.clone()),
        Some("urgent".to_string())
    );

    let rejected = page.set_priority(&db, &bus, "mega-critical");
    assert!(
        matches!(rejected, ActionOutcome::Rejected(_)),
        "unknown priorities are rejected, not invented: {rejected:?}"
    );
}

#[test]
fn audp219_set_workflow_state_writes_state_id_and_transition_without_an_outbound_job() {
    let db = fixture(vec![(
        "conv",
        1,
        "Export broken",
        "active",
        "email",
        1,
        5,
        9,
    )]);
    let bus = EventBus::new(8);
    let mut page = InboxPage::new();
    page.selected = Some(1);
    page.detail = Some(
        db.with_read(|conn| spp_core::inbox::get_conversation(conn, 1))
            .expect("detail")
            .expect("conversation 1 exists"),
    );

    let outcome = page.set_workflow_state(&db, &bus, "investigating");
    assert!(
        matches!(outcome, ActionOutcome::Success(ref m) if m.contains("investigating")),
        "the write path succeeds: {outcome:?}"
    );

    db.with_read(|conn| {
        let (state, state_id, transitions): (String, Option<i64>, i64) = conn
            .query_row(
                "SELECT supportos_state, supportos_state_id,
                        (SELECT COUNT(*) FROM ticket_state_transitions
                          WHERE conversation_remote_id = 1001 AND to_state IS NOT NULL)
                   FROM conversations WHERE id = 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .expect("conversation");
        assert_eq!(state, "investigating");
        assert_eq!(transitions, 1, "the state transition is recorded");
        let resolved: i64 = conn
            .query_row(
                "SELECT id FROM ticket_states WHERE key = 'investigating'",
                [],
                |r| r.get(0),
            )
            .expect("lookup row");
        assert_eq!(state_id, Some(resolved));
        let outbound: i64 = conn
            .query_row("SELECT COUNT(*) FROM outbound_jobs", [], |r| r.get(0))
            .expect("outbound count");
        assert_eq!(
            outbound, 0,
            "a local workflow state is not pushed to Help Scout"
        );
    });

    assert_eq!(
        page.detail.as_ref().and_then(|d| d.workflow_state.clone()),
        Some("investigating".to_string())
    );

    let rejected = page.set_workflow_state(&db, &bus, "investigating-typo");
    assert!(
        matches!(rejected, ActionOutcome::Rejected(_)),
        "unknown workflow states are rejected, not invented: {rejected:?}"
    );
}

#[test]
fn audp14_set_tags_writes_the_mirror_and_queues_the_update_tags_job() {
    let db = fixture(vec![(
        "conv",
        1,
        "Export broken",
        "active",
        "email",
        1,
        5,
        9,
    )]);
    let bus = EventBus::new(8);
    let mut page = InboxPage::new();
    page.selected = Some(1);
    page.detail = Some(
        db.with_read(|conn| spp_core::inbox::get_conversation(conn, 1))
            .expect("detail")
            .expect("conversation 1 exists"),
    );

    let outcome = page.set_tags(&db, &bus, vec!["vip".into(), "billing".into()]);
    assert!(
        matches!(outcome, ActionOutcome::Success(_)),
        "the write path succeeds: {outcome:?}"
    );
    db.with_read(|conn| {
        let mut tags: Vec<String> = spp_core::conversation_ops::read_conversation_tags(conn, 1);
        tags.sort();
        assert_eq!(tags, vec!["billing".to_string(), "vip".to_string()]);
        let (kind, payload): (String, String) = conn
            .query_row(
                "SELECT kind, payload FROM outbound_jobs ORDER BY id DESC LIMIT 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .expect("job");
        assert_eq!(kind, "update_tags");
        let value: serde_json::Value = serde_json::from_str(&payload).expect("payload");
        assert_eq!(
            value["tags"],
            serde_json::json!(["vip", "billing"]),
            "the full replace list rides the job verbatim"
        );
    });

    let mut reloaded = page.detail.clone().expect("detail").tags;
    reloaded.sort();
    assert_eq!(reloaded, vec!["billing".to_string(), "vip".to_string()]);

    let outcome = page.set_tags(&db, &bus, vec!["billing".into()]);
    assert!(matches!(outcome, ActionOutcome::Success(_)));
    db.with_read(|conn| {
        let tags: Vec<String> = spp_core::conversation_ops::read_conversation_tags(conn, 1);
        assert_eq!(tags, vec!["billing".to_string()]);
    });

    let outcome = page.set_tags(&db, &bus, Vec::new());
    assert!(matches!(outcome, ActionOutcome::Success(_)));
    db.with_read(|conn| {
        let tags: Vec<String> = spp_core::conversation_ops::read_conversation_tags(conn, 1);
        assert!(tags.is_empty());
    });
}

#[test]
fn audp14_snooze_writes_the_local_column_and_queues_the_job() {
    let db = fixture(vec![(
        "conv",
        1,
        "Export broken",
        "active",
        "email",
        1,
        5,
        9,
    )]);
    let bus = EventBus::new(8);
    let mut page = InboxPage::new();
    page.selected = Some(1);
    page.detail = Some(
        db.with_read(|conn| spp_core::inbox::get_conversation(conn, 1))
            .expect("detail")
            .expect("conversation 1 exists"),
    );
    assert!(
        page.detail
            .as_ref()
            .expect("detail")
            .snoozed_until
            .is_none()
    );

    let until = "2026-12-01T09:00:00Z".to_string();
    let outcome = page.snooze(&db, &bus, until.clone());
    assert!(
        matches!(outcome, ActionOutcome::Success(ref m) if m.contains(&until)),
        "the write path succeeds: {outcome:?}"
    );
    db.with_read(|conn| {
        let stored: Option<String> = conn
            .query_row(
                "SELECT snoozed_until FROM conversations WHERE id = 1",
                [],
                |r| r.get(0),
            )
            .expect("row");
        assert_eq!(stored.as_deref(), Some(until.as_str()));
        let (kind, payload): (String, String) = conn
            .query_row(
                "SELECT kind, payload FROM outbound_jobs ORDER BY id DESC LIMIT 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .expect("job");
        assert_eq!(kind, "snooze");
        let value: serde_json::Value = serde_json::from_str(&payload).expect("payload");
        assert_eq!(value["snoozedUntil"], until);
    });
    assert_eq!(
        page.detail
            .clone()
            .expect("detail")
            .snoozed_until
            .as_deref(),
        Some(until.as_str()),
        "the reloaded detail shows the snooze state"
    );

    let outcome = page.unsnooze(&db, &bus);
    assert!(matches!(outcome, ActionOutcome::Success(_)));
    db.with_read(|conn| {
        let stored: Option<String> = conn
            .query_row(
                "SELECT snoozed_until FROM conversations WHERE id = 1",
                [],
                |r| r.get(0),
            )
            .expect("row");
        assert!(stored.is_none(), "unsnooze clears the local column");
        let (kind, _): (String, String) = conn
            .query_row(
                "SELECT kind, payload FROM outbound_jobs ORDER BY id DESC LIMIT 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .expect("job");
        assert_eq!(kind, "unsnooze");
    });
}
