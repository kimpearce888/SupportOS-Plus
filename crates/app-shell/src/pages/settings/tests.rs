use super::*;
use crate::testsupport::fixture;

#[test]
fn general_settings_round_trip_through_the_settings_table() {
    let db = fixture(Vec::new());
    let original = db.with_read(GeneralSettings::load);
    assert_eq!(original, GeneralSettings::default());

    let mut changed = original;
    changed.sync_interval_minutes = 15;
    changed.reconcile_interval_minutes = 720;
    changed.ai_enabled = false;
    changed.log_level = "debug".into();
    db.with_write(|conn| changed.save(conn)).expect("save");

    let reloaded = db.with_read(GeneralSettings::load);
    assert_eq!(reloaded, changed);
    assert_ne!(reloaded, GeneralSettings::default());
}

#[test]
fn audit25_legacy_waiting_setting_loads_as_open() {
    let db = fixture(Vec::new());
    for (stored, expected) in [
        ("waiting", "open"),
        ("open", "open"),
        ("pending", "pending"),
        ("keep", "keep"),
        ("garbage", "pending"),
    ] {
        db.with_write(|conn| spp_core::settings::set_string(conn, "reply_status_after", stored))
            .expect("set");
        let loaded = db.with_read(GeneralSettings::load);
        assert_eq!(
            loaded.reply_status_after, expected,
            "stored '{stored}' must load as '{expected}'"
        );
    }
}

#[test]
fn helpscout_status_reads_tokens() {
    let db = fixture(Vec::new());
    let status = db.with_read(helpscout_status);
    assert!(!status.configured);
    assert!(!status.credentials, "no credentials on a fresh database");

    db.with_write(|conn| {
        conn.execute("INSERT INTO oauth_tokens (access_token) VALUES ('tok')", [])
            .expect("token");
    });
    let status2 = db.with_read(helpscout_status);
    assert!(status2.configured);
}

#[test]
fn business_hours_upsert_per_mailbox() {
    let db = fixture(Vec::new());
    let rows = db.with_read(load_business_hours);
    assert_eq!(rows.len(), 2, "Support and Billing mailboxes");
    assert!(rows.iter().all(|r| !r.configured));

    let mut row = rows.into_iter().find(|r| r.name == "Support").unwrap();
    row.timezone = "Asia/Dhaka".into();
    row.start_minute = 600;
    row.end_minute = 1100;
    db.with_write(|conn| save_business_hours(conn, &row))
        .expect("save hours");

    let reloaded = db.with_read(load_business_hours);
    let saved = reloaded.iter().find(|r| r.name == "Support").expect("row");
    assert!(saved.configured);
    assert_eq!(saved.timezone, "Asia/Dhaka");
    assert_eq!(saved.start_minute, 600);
    assert_eq!(minutes_label(saved.start_minute), "10:00");
}

#[test]
fn business_hours_validation_rejects_bad_rows() {
    let db = fixture(Vec::new());
    let rows = db.with_read(load_business_hours);
    let mut row = rows.into_iter().next().unwrap();
    row.end_minute = row.start_minute;
    let result = db.with_write(|conn| save_business_hours(conn, &row));
    assert!(result.is_err());

    row.end_minute = row.start_minute + 60;
    row.timezone = "Mars/Olympus".into();
    let result2 = db.with_write(|conn| save_business_hours(conn, &row));
    assert!(result2.is_err());
}

#[test]
fn business_hours_days_are_validated_and_normalized() {
    let db = fixture(Vec::new());
    let rows = db.with_read(load_business_hours);
    let mut row = rows.into_iter().find(|r| r.name == "Support").unwrap();
    row.timezone = "UTC".into();
    row.end_minute = row.start_minute + 480;

    for bad in ["1-5", "mon-fri", "1; 2; 3", "9", "", "  "] {
        row.days = bad.into();
        let result = db.with_write(|conn| save_business_hours(conn, &row));
        assert!(result.is_err(), "{bad:?} must be rejected");
    }

    row.days = "5, 1, 3".into();
    db.with_write(|conn| save_business_hours(conn, &row))
        .expect("save");
    let saved = db
        .with_read(load_business_hours)
        .into_iter()
        .find(|r| r.name == "Support")
        .unwrap();
    assert_eq!(
        saved.days, "[1,3,5]",
        "the stored list is normalized (deduped, sorted, JSON)"
    );

    row.days = "[1,2,3,4,5]".into();
    db.with_write(|conn| save_business_hours(conn, &row))
        .expect("save");
    let saved = db
        .with_read(load_business_hours)
        .into_iter()
        .find(|r| r.name == "Support")
        .unwrap();
    assert_eq!(saved.days, "[1,2,3,4,5]");
}

#[test]
fn backup_create_lists_and_restores() {
    let db = fixture(vec![("conv", 1, "Keep me", "active", "email", 1, 5, 9)]);
    let dir = std::env::temp_dir().join(format!("spp-backup-{}", std::process::id()));
    let result = db.with_read(|conn| spp_core::backup_service::backup(conn, &dir));
    assert!(result.ok, "{}", result.message);
    assert!(result.path.is_some());

    let list = spp_core::backup_service::list_backups(&dir);
    assert_eq!(list.len(), 1);
    let file = str_field(&list[0], "file");
    assert!(file.contains("supportos-backup-"), "unexpected name {file}");
    assert!(i64_field(&list[0], "size_bytes") > 0);
    let _ = std::fs::remove_dir_all(&dir);
}

fn install_master_key() {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    let path = std::env::temp_dir().join(format!(
        "spp-settings-page-key-{}-{nanos}",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&path);
    spp_core::settings::init_master_key(&path).expect("master key");
    let _ = std::fs::remove_file(&path);
}

#[test]
fn helpscout_credentials_save_and_clear() {
    install_master_key();
    let db = fixture(Vec::new());

    let mut page = SettingsPage::new();
    page.hs_form_loaded = true;
    page.hs_client_id = "cid-app".into();
    page.hs_client_secret = "secret-app".into();
    page.hs_webhook_secret = "wh-app".into();
    page.save_helpscout_credentials(&db);

    assert!(
        page.message
            .as_deref()
            .is_some_and(|m| m.contains("stored")),
        "save must confirm: {:?}",
        page.message
    );
    assert!(page.error.is_none(), "{:?}", page.error);
    let status = db.with_read(helpscout_status);
    assert!(status.credentials, "credentials resolve from the envelope");
    assert_eq!(
        status.sources.client_id,
        spp_core::helpscout_real::CredentialSource::AppSettings
    );
    assert_eq!(
        status.sources.webhook_secret,
        spp_core::helpscout_real::CredentialSource::AppSettings
    );
    assert!(
        page.hs_client_secret.is_empty(),
        "secret fields are cleared after a successful save"
    );

    let stored = db.with_read(spp_core::helpscout_real::HsCredentials::from_envelope);
    assert_eq!(stored.client_id, "cid-app");
    assert_eq!(stored.client_secret, "secret-app");

    db.with_write(|conn| {
        conn.execute("INSERT INTO oauth_tokens (access_token) VALUES ('tok')", [])
            .expect("token");
    });
    page.clear_helpscout_credentials(&db);
    let status2 = db.with_read(helpscout_status);
    assert!(!status2.credentials);
    assert_eq!(
        status2.sources.client_id,
        spp_core::helpscout_real::CredentialSource::NotSet
    );
    assert!(status2.configured, "connection tokens survive the clear");
}

#[test]
fn reset_workspace_empties_content_and_keeps_config() {
    let db = fixture(vec![(
        "conv",
        1,
        "Seeded ticket",
        "active",
        "email",
        1,
        5,
        9,
    )]);
    db.with_write(|conn| {
        spp_core::settings::set_string(conn, "agent_language", "English").expect("lang");
    });

    let mut page = SettingsPage::new();
    page.purge_armed = true;
    page.reset_workspace(&db);

    assert!(
        page.message
            .as_deref()
            .is_some_and(|m| m.contains("workspace data deleted")),
        "reset must report: {:?}",
        page.message
    );
    assert!(page.error.is_none(), "{:?}", page.error);
    assert!(
        !page.purge_armed,
        "the confirmation disarms after the reset"
    );

    let (convs, users, mailboxes, lang, sync_state) = db.with_read(|conn| {
        let count = |table: &str| -> i64 {
            conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| {
                r.get::<_, i64>(0)
            })
            .unwrap_or_else(|e| panic!("count {table}: {e}"))
        };
        (
            count("conversations"),
            count("users"),
            count("mailboxes"),
            spp_core::settings::get_string(conn, "agent_language")
                .expect("read")
                .expect("present"),
            spp_core::sync_engine::get_state(conn),
        )
    });
    assert_eq!(convs, 0, "seeded conversations must be purged");
    assert_eq!(users, 0, "mirror users must be purged");
    assert_eq!(mailboxes, 0, "mirror mailboxes must be purged");
    assert_eq!(lang, "English", "application settings must survive");
    assert_eq!(sync_state, "NEW", "the reset arms a fresh first sync");
}

#[test]
fn begin_oauth_without_an_active_connection_explains_itself() {
    install_master_key();
    let db = fixture(Vec::new());
    db.with_write(|conn| {
        spp_core::helpscout_real::HsCredentials::store_in_envelope(conn, "cid", "secret", "", "")
            .expect("store");
    });

    let mut page = SettingsPage::new();
    page.begin_oauth(&db);
    assert!(page.authorize_url.is_none(), "no URL without a connection");
    assert!(
        page.error
            .as_deref()
            .is_some_and(|e| e.contains("no restart")),
        "the error must point at the activation path: {:?}",
        page.error
    );
}

#[test]
fn begin_oauth_succeeds_against_a_live_listener() {
    let db = fixture(Vec::new());
    let mut server = spp_core::loopback::LoopbackServer::start(
        db.clone(),
        std::sync::Arc::new(spp_core::provider_handle::ProviderHandle::new_empty()),
        0,
    )
    .expect("listener");
    let redirect_uri = format!("http://localhost:{}/oauth/callback", server.port());
    let creds = spp_core::helpscout_real::HsCredentials {
        client_id: "cid-live".into(),
        client_secret: "secret-live".into(),
        redirect_uri: redirect_uri.clone(),
        ..Default::default()
    };

    let mut page = SettingsPage::new();
    page.set_provider_handle(std::sync::Arc::new(
        spp_core::provider_handle::ProviderHandle::new_real(std::sync::Arc::new(
            spp_core::helpscout_real::RealHelpScoutProvider::new(db.clone(), creds.clone()),
        )),
    ));
    page.begin_oauth_at(&db, &creds, &redirect_uri);

    let url = page.authorize_url.clone().expect("authorization URL");
    assert!(
        url.contains("client_id=cid-live"),
        "the URL carries the client id: {url}"
    );
    assert!(url.contains("state="));
    assert!(page.error.is_none(), "{:?}", page.error);
    let states: i64 = db.with_read(|conn| {
        conn.query_row(
            "SELECT COUNT(*) FROM oauth_states WHERE consumed_at IS NULL",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0)
    });
    assert_eq!(states, 1, "one single-use state backs the URL");
    server.stop();
}

#[test]
fn begin_oauth_rewrites_the_recorded_uri_to_the_bound_port() {
    let db = fixture(Vec::new());
    let mut server = spp_core::loopback::LoopbackServer::start(
        db.clone(),
        std::sync::Arc::new(spp_core::provider_handle::ProviderHandle::new_empty()),
        0,
    )
    .expect("listener");
    let bound = server.port();
    let redirect_uri = "http://localhost:3000/oauth/callback";
    assert_ne!(
        spp_core::loopback::port_from_redirect_uri(redirect_uri),
        Some(bound),
        "the fixture must exercise the mismatch"
    );
    let creds = spp_core::helpscout_real::HsCredentials {
        client_id: "cid-live".into(),
        client_secret: "secret-live".into(),
        redirect_uri: redirect_uri.to_string(),
        ..Default::default()
    };

    let mut page = SettingsPage::new();
    page.begin_oauth_on(&db, &creds, redirect_uri, Some(bound));

    let url = page.authorize_url.clone().expect("authorization URL");
    assert!(page.error.is_none(), "{:?}", page.error);
    let state = url.split("state=").nth(1).unwrap_or_default().to_string();
    let recorded = db.with_read(|conn| {
        spp_core::oauth_state::redirect_uri_for(conn, &state).expect("redirect lookup")
    });
    assert_eq!(
        recorded.as_deref(),
        Some(format!("http://localhost:{bound}/oauth/callback").as_str()),
        "the recorded URI must be rewritten to the bound port"
    );
    server.stop();
}

#[test]
fn client_credentials_connect_reports_failures_in_app() {
    let db = fixture(Vec::new());
    let creds = spp_core::helpscout_real::HsCredentials {
        client_id: "cid-x".into(),
        client_secret: "secret-x".into(),
        api_base: "http://127.0.0.1:1".into(),
        ..Default::default()
    };
    let mut page = SettingsPage::new();
    page.set_provider_handle(std::sync::Arc::new(
        spp_core::provider_handle::ProviderHandle::new_real(std::sync::Arc::new(
            spp_core::helpscout_real::RealHelpScoutProvider::new(db.clone(), creds),
        )),
    ));
    page.connect_client_credentials();
    assert!(page.hs_connecting, "the connect runs in the background");

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
    while page.hs_connecting && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(50));
        page.poll_connect_result(&db);
    }
    assert!(!page.hs_connecting, "the connect must finish");
    assert!(
        page.error
            .as_deref()
            .is_some_and(|e| e.contains("rejected the login credentials")),
        "the failure must be reported in-app with the friendly message: {:?}",
        page.error
    );
}

#[test]
fn r3_saving_credentials_activates_the_provider_in_session() {
    install_master_key();
    let db = fixture(Vec::new());
    let handle = std::sync::Arc::new(spp_core::provider_handle::ProviderHandle::new_empty());
    db.with_write(|conn| {
        spp_core::sync_engine::set_state(conn, "LIVE");
        let _ = spp_core::settings::set_string(conn, "provider_kind", "empty");
    });
    let mut page = SettingsPage::new();
    page.set_provider_handle(std::sync::Arc::clone(&handle));
    page.hs_client_id = "cid-activate".into();
    page.hs_client_secret = "secret-activate".into();
    page.save_helpscout_credentials(&db);
    assert!(
        handle.is_real(),
        "the provider must be live immediately after the save"
    );
    assert!(handle.real().is_some());
    let (state, kind) = db.with_read(|conn| {
        (
            spp_core::sync_engine::get_state(conn),
            spp_core::settings::get_string(conn, "provider_kind")
                .ok()
                .flatten(),
        )
    });
    assert_eq!(state, "NEW", "the poisoned LIVE watermark is reset (R2)");
    assert_eq!(kind.as_deref(), Some("real"));
    assert!(
        !page.doctor_running,
        "bare pages skip the network side effects"
    );
}

#[test]
fn restore_backup_lands_a_verified_file_and_refuses_junk() {
    let tmp = std::env::temp_dir().join(format!(
        "spp-restore-page-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&tmp).expect("tmp dir");
    let db_path = tmp.join("live.db");
    let mut file_conn = rusqlite::Connection::open(&db_path).expect("file db");
    spp_core::bootstrap::apply_all(&mut file_conn).expect("migrations");
    drop(file_conn);
    let db = DbHandle::new(rusqlite::Connection::open(&db_path).expect("reopen file db"));

    let mut page = SettingsPage::new();
    page.restore_armed = None;

    let backups_dir = tmp.join("backups");
    let result = db.with_read(|conn| spp_core::backup_service::backup(conn, &backups_dir));
    assert!(result.ok, "backup for the test: {}", result.message);
    let backup_path = result.path.expect("backup path");
    page.restore_backup(&db, &backup_path);
    assert!(
        page.message
            .as_deref()
            .is_some_and(|m| m.contains("restored") && m.contains("restart")),
        "success reports the restart requirement: {:?}",
        page.message
    );

    page.message = None;
    page.error = None;
    let junk = tmp.join("junk.db");
    std::fs::write(&junk, b"definitely not a database").expect("junk");
    page.restore_backup(&db, junk.to_str().expect("path"));
    assert!(
        page.error
            .as_deref()
            .is_some_and(|e| e.contains("integrity") || e.contains("refused")),
        "the refusal is reported in-app: {:?}",
        page.error
    );
    let still_live: bool = db.with_read(|conn| {
        conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='app_state'",
            [],
            |r| r.get::<_, i64>(0),
        )
        .map(|n| n > 0)
        .unwrap_or(false)
    });
    assert!(still_live, "the live database survived the refusal");

    let _ = std::fs::remove_dir_all(&tmp);
}
