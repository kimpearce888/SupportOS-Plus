use super::*;

fn migrated_conn() -> Connection {
    let mut conn = Connection::open_in_memory().expect("in-memory db");
    crate::bootstrap::apply_all(&mut conn).expect("migrations");
    conn
}

fn install_key() {
    crate::settings::clear_master_key_for_tests();
    let path = std::env::temp_dir().join(format!(
        "spp-hscreds-key-{}-{}",
        std::process::id(),
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
    ));
    let _ = std::fs::remove_file(&path);
    crate::settings::init_master_key(&path).expect("master key");
    let _ = std::fs::remove_file(&path);
}

#[test]
fn credentials_envelope_lifecycle() {
    let _key_guard = crate::settings::SECRETS_TEST_LOCK
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    install_key();
    let conn = migrated_conn();

    let env_none = HsCredentials::default();
    let (resolved, sources) = HsCredentials::resolve_with(&env_none, &conn);
    assert!(!resolved.is_configured());
    assert_eq!(sources.client_id, CredentialSource::NotSet);
    assert_eq!(sources.client_secret, CredentialSource::NotSet);
    assert_eq!(sources.webhook_secret, CredentialSource::NotSet);
    assert_eq!(sources.docs_api_key, CredentialSource::NotSet);

    HsCredentials::store_in_envelope(&conn, "cid-1", "csecret-1", "wh-1", "").expect("store");
    let (r2, s2) = HsCredentials::resolve_with(&env_none, &conn);
    assert!(r2.is_configured());
    assert_eq!(r2.client_id, "cid-1");
    assert_eq!(r2.client_secret, "csecret-1");
    assert_eq!(r2.webhook_secret, "wh-1");
    assert_eq!(r2.docs_api_key, "");
    assert_eq!(s2.client_id, CredentialSource::AppSettings);
    assert_eq!(s2.client_secret, CredentialSource::AppSettings);
    assert_eq!(s2.webhook_secret, CredentialSource::AppSettings);
    assert_eq!(s2.docs_api_key, CredentialSource::NotSet);

    let blob: Vec<u8> = conn
        .query_row(
            "SELECT value FROM secrets WHERE key = 'helpscout_client_secret'",
            [],
            |r| r.get(0),
        )
        .expect("blob");
    assert!(!blob.windows(8).any(|w| w == b"csecret-1"));

    let env_some = HsCredentials {
        client_id: "env-cid".into(),
        webhook_secret: "env-wh".into(),
        ..HsCredentials::default()
    };
    let (r3, s3) = HsCredentials::resolve_with(&env_some, &conn);
    assert_eq!(r3.client_id, "env-cid");
    assert_eq!(s3.client_id, CredentialSource::Environment);
    assert_eq!(s3.webhook_secret, CredentialSource::Environment);
    assert_eq!(r3.webhook_secret, "env-wh");
    assert_eq!(s3.client_secret, CredentialSource::AppSettings);
    assert_eq!(r3.client_secret, "csecret-1");

    let removed = HsCredentials::clear_envelope(&conn).expect("clear");
    assert_eq!(
        removed, 3,
        "client id + secret + webhook (docs never stored)"
    );
    let (r4, s4) = HsCredentials::resolve_with(&env_none, &conn);
    assert!(!r4.is_configured());
    assert_eq!(s4.client_id, CredentialSource::NotSet);
    assert_eq!(s4.client_secret, CredentialSource::NotSet);
    assert_eq!(s4.webhook_secret, CredentialSource::NotSet);
}

#[test]
fn debug_output_never_contains_credential_values() {
    let creds = HsCredentials {
        client_id: "client-id-abc".into(),
        client_secret: "TOPSECRETVALUE1".into(),
        redirect_uri: "http://localhost:3000/oauth/callback".into(),
        api_base: "https://api.helpscout.net".into(),
        webhook_secret: "WHSECVALUE1".into(),
        docs_api_key: "DOCSKEYVALUE1".into(),
        docs_api_base: "https://docsapi.helpscout.net".into(),
    };
    let rendered = format!("{creds:?}");
    assert!(!rendered.contains("TOPSECRETVALUE1"), "{rendered}");
    assert!(!rendered.contains("WHSECVALUE1"), "{rendered}");
    assert!(!rendered.contains("DOCSKEYVALUE1"), "{rendered}");
    assert!(rendered.contains("<redacted 15 chars>"), "{rendered}");
    assert!(rendered.contains("client-id-abc"), "{rendered}");
}
