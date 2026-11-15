use super::*;

fn conn_with_tokens() -> Connection {
    let mut conn = Connection::open_in_memory().expect("in-memory db");
    crate::bootstrap::apply_all(&mut conn).expect("migrations");
    ensure_account_row(&conn);
    conn.execute(
        "INSERT INTO oauth_tokens (id, account, access_token, refresh_token, token_type, revoked)
         VALUES (1, 'default', 'legacy-plain', 'legacy-refresh', 'bearer', 0)",
        [],
    )
    .expect("seed legacy plaintext tokens");
    conn
}

#[test]
fn m053_seals_legacy_plaintext_tokens_and_scrubs_the_row() {
    let _g = crate::settings::SECRETS_TEST_LOCK
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    crate::settings::set_master_key_for_tests([7u8; 32]);
    let conn = conn_with_tokens();

    apply_m053_oauth_token_envelope(&conn).expect("m053");

    let access = crate::settings::get_secret(&conn, SECRET_OAUTH_ACCESS)
        .expect("envelope read")
        .map(|b| String::from_utf8(b).expect("utf8"));
    let refresh = crate::settings::get_secret(&conn, SECRET_OAUTH_REFRESH)
        .expect("envelope read")
        .map(|b| String::from_utf8(b).expect("utf8"));
    assert_eq!(access.as_deref(), Some("legacy-plain"));
    assert_eq!(refresh.as_deref(), Some("legacy-refresh"));
    let (column_access, column_refresh): (String, Option<String>) = conn
        .query_row(
            "SELECT access_token, refresh_token FROM oauth_tokens WHERE account = 'default'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .expect("row");
    assert_eq!(column_access, "", "m053 must scrub the plaintext column");
    assert_eq!(column_refresh, None, "m053 must scrub the plaintext column");
    let stored: i64 = conn
        .query_row("SELECT COUNT(*) FROM oauth_tokens", [], |r| r.get(0))
        .expect("token count");
    assert!(stored > 0, "probe");
}

#[test]
fn m053_without_a_key_leaves_the_legacy_row_intact() {
    let _g = crate::settings::SECRETS_TEST_LOCK
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    crate::settings::clear_master_key_for_tests();
    let conn = conn_with_tokens();

    apply_m053_oauth_token_envelope(&conn).expect("m053 (skip)");

    let (column_access, _): (String, Option<String>) = conn
        .query_row(
            "SELECT access_token, refresh_token FROM oauth_tokens WHERE account = 'default'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .expect("row");
    assert_eq!(
        column_access, "legacy-plain",
        "without a key the row must be left exactly as it was"
    );
    let stored: i64 = conn
        .query_row("SELECT COUNT(*) FROM oauth_tokens", [], |r| r.get(0))
        .expect("token count");
    assert!(stored > 0, "probe");
    crate::settings::set_master_key_for_tests([7u8; 32]);
}
