use super::*;

fn env_default_concurrency() -> usize {
    crate::config::load_env_config().sync.api_concurrency.max(1) as usize
}

fn migrated_conn() -> Connection {
    let mut conn = Connection::open_in_memory().expect("in-memory db");
    crate::bootstrap::apply_all(&mut conn).expect("migrations");
    conn
}

fn provider_with(conn: Connection, setting: Option<i64>) -> RealHelpScoutProvider {
    if let Some(n) = setting {
        crate::settings::set_i64(&conn, "api_concurrency", n).expect("store setting");
    }
    RealHelpScoutProvider::new(crate::db::DbHandle::new(conn), HsCredentials::default())
}

#[test]
fn api_concurrency_setting_reaches_both_provider_queues() {
    let p = provider_with(migrated_conn(), None);
    assert_eq!(p.api_concurrency(), env_default_concurrency());
    assert_eq!(
        p.docs_queue.concurrency(),
        env_default_concurrency(),
        "the docs queue follows the same default"
    );

    let p = provider_with(migrated_conn(), Some(5));
    assert_eq!(p.api_concurrency(), 5);
    assert_eq!(
        p.docs_queue.concurrency(),
        5,
        "the docs queue follows the setting"
    );

    p.set_api_concurrency(3);
    assert_eq!(p.api_concurrency(), 3);
    assert_eq!(
        p.docs_queue.concurrency(),
        3,
        "the live resize covers the docs queue"
    );

    let p = provider_with(migrated_conn(), Some(0));
    assert_eq!(p.api_concurrency(), 1, "stored 0 clamps to 1");

    let conn = migrated_conn();
    crate::settings::set_string(&conn, "api_concurrency", "not-a-number").expect("garbage");
    let p = RealHelpScoutProvider::new(crate::db::DbHandle::new(conn), HsCredentials::default());
    assert_eq!(
        p.api_concurrency(),
        env_default_concurrency(),
        "garbage falls back to the default"
    );
}

#[test]
fn map_conversation_keeps_team_ids_out_of_the_user_slot() {
    let team_assigned = serde_json::json!({
        "id": 101, "number": 7, "status": "active", "mailboxId": 1,
        "assignee": {"id": 42, "type": "team"},
        "primaryCustomer": {"id": 900}
    });
    let c = RealHelpScoutProvider::map_conversation(&team_assigned);
    assert_eq!(c.assignee_id, None, "the user slot stays empty for a team");
    assert_eq!(c.assignee_type.as_deref(), Some("team"));
    assert_eq!(
        c.assigned_team_id,
        Some(42),
        "the team id rides its own field"
    );

    let user_assigned = serde_json::json!({
        "id": 102, "number": 8, "status": "active", "mailboxId": 1,
        "assignee": {"id": 55, "type": "user"},
        "primaryCustomer": {"id": 900}
    });
    let c = RealHelpScoutProvider::map_conversation(&user_assigned);
    assert_eq!(c.assignee_id, Some(55));
    assert_eq!(c.assigned_team_id, None);

    let both = serde_json::json!({
        "id": 103, "number": 9, "status": "active", "mailboxId": 1,
        "assignee": {"id": 55, "type": "user"},
        "assignedTeam": {"id": 42},
        "primaryCustomer": {"id": 900}
    });
    let c = RealHelpScoutProvider::map_conversation(&both);
    assert_eq!(c.assignee_id, Some(55));
    assert_eq!(c.assigned_team_id, Some(42));

    let team_only = serde_json::json!({
        "id": 104, "number": 10, "status": "active", "mailboxId": 1,
        "assignedTeam": {"id": 77},
        "primaryCustomer": {"id": 900}
    });
    let c = RealHelpScoutProvider::map_conversation(&team_only);
    assert_eq!(c.assignee_id, None);
    assert_eq!(c.assigned_team_id, Some(77));
}
