use super::*;

fn conn() -> Connection {
    let mut c = Connection::open_in_memory().expect("db");
    crate::bootstrap::apply_all(&mut c).expect("migrations");
    c
}

fn seed(c: &Connection) -> i64 {
    c.execute(
        "INSERT INTO mailboxes (remote_id, name) VALUES (1, 'Support')",
        [],
    )
    .expect("mailbox");
    let mailbox = c.last_insert_rowid();
    create_outreach_campaign(c, "Check-in", "Hi", "Body", mailbox, &[], None, None, &[])
        .expect("campaign")
}

#[test]
fn audp2_campaign_edit_round_trips_on_drafts_and_refuses_after_queue() {
    let c = conn();
    let campaign = seed(&c);

    update_outreach_campaign(
        &c,
        campaign,
        "Check-in v2",
        "Hello",
        "New body",
        1,
        &["tag".to_string()],
    )
    .expect("draft edit");
    let (name, subject, tags): (String, String, String) = c
        .query_row(
            "SELECT name, subject, tags FROM outreach_campaigns WHERE id = ?1",
            params![campaign],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .expect("row");
    assert_eq!(name, "Check-in v2");
    assert_eq!(subject, "Hello");
    assert!(tags.contains("tag"), "{tags}");

    c.execute(
        "UPDATE outreach_campaigns SET status = 'queued' WHERE id = ?1",
        params![campaign],
    )
    .expect("queue");
    let err = update_outreach_campaign(&c, campaign, "X", "Y", "Z", 1, &[]).expect_err("guard");
    assert!(
        err.to_string().contains("only draft campaigns"),
        "the refusal explains the rule: {err}"
    );

    c.execute(
        "UPDATE outreach_campaigns SET status = 'draft' WHERE id = ?1",
        params![campaign],
    )
    .expect("redraft");
    let err = update_outreach_campaign(&c, campaign, "", "Y", "Z", 1, &[]).expect_err("blank");
    assert!(err.to_string().contains("required"), "{err}");

    let err = update_outreach_campaign(&c, campaign, "N", "S", "B", 999, &[]).expect_err("mailbox");
    assert!(err.to_string().contains("does not exist"), "{err}");

    let err = update_outreach_campaign(&c, 999, "N", "S", "B", 1, &[]).expect_err("missing");
    assert!(err.to_string().contains("not found"), "{err}");
}
