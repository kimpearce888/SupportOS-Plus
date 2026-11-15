use super::*;
use crate::testsupport::fixture;

#[test]
fn invalidation_keeps_unsaved_general_edits() {
    let db = fixture(Vec::new());
    let mut page = SettingsPage::new();

    page.ensure_loaded(&db);
    let loaded_interval = page.general.as_ref().expect("loaded").sync_interval_minutes;

    page.general.as_mut().expect("form").sync_interval_minutes = 720;

    page.invalidate();

    page.ensure_loaded(&db);
    assert_eq!(
        page.general
            .as_ref()
            .expect("form kept")
            .sync_interval_minutes,
        720,
        "the unsaved edit survives the invalidation + reload"
    );
    assert!(
        page.helpscout.is_some(),
        "the connection-status mirror still refreshed"
    );
    assert!(
        !page.stale,
        "the pending invalidation was consumed by the reload"
    );

    assert_eq!(
        db.with_read(|conn| { spp_core::settings::get_i64(conn, "sync_interval_minutes", 1) })
            .unwrap_or(1),
        loaded_interval,
        "the invalidation saved nothing behind the user's back"
    );
}

#[test]
fn invalidation_refreshes_a_clean_form_from_the_db() {
    let db = fixture(Vec::new());
    let mut page = SettingsPage::new();
    page.ensure_loaded(&db);

    db.with_write(|conn| spp_core::settings::set_i64(conn, "sync_interval_minutes", 45))
        .expect("write");

    page.invalidate();
    page.ensure_loaded(&db);

    assert_eq!(
        page.general.as_ref().expect("form").sync_interval_minutes,
        45,
        "a clean form still takes the fresh read"
    );
}

#[test]
fn saved_edits_clear_the_dirty_guard() {
    let db = fixture(Vec::new());
    let mut page = SettingsPage::new();
    page.ensure_loaded(&db);

    page.general.as_mut().expect("form").api_concurrency = 4;
    page.save_general(&db);
    assert!(page.error.is_none(), "save failed: {:?}", page.error);

    db.with_write(|conn| spp_core::settings::set_i64(conn, "sync_interval_minutes", 30))
        .expect("write");
    page.invalidate();
    page.ensure_loaded(&db);

    let general = page.general.as_ref().expect("form");
    assert_eq!(
        general.sync_interval_minutes, 30,
        "fresh read after save (form was clean)"
    );
    assert_eq!(
        general.api_concurrency, 4,
        "the SAVED edit is the persisted value now"
    );
}

#[test]
fn lmstudio_invalidation_refreshes_a_clean_form_from_the_db() {
    let db = fixture(Vec::new());
    let mut page = SettingsPage::new();
    page.ensure_loaded(&db);

    db.with_write(|conn| {
        spp_core::settings::set_string(conn, "lmstudio_base_url", "http://192.168.1.10:1234")
    })
    .expect("write");

    page.invalidate();
    page.ensure_loaded(&db);

    assert_eq!(
        page.lmstudio_base_url, "http://192.168.1.10:1234",
        "a clean form still takes the fresh read"
    );
    assert_eq!(
        page.lmstudio_persisted.0, "http://192.168.1.10:1234",
        "the snapshot re-bases on the fresh read"
    );
}

#[test]
fn lmstudio_dirty_form_survives_invalidation() {
    let db = fixture(Vec::new());
    let mut page = SettingsPage::new();
    page.ensure_loaded(&db);

    page.lmstudio_base_url = "http://localhost:9999".into();
    page.lmstudio_chat_model = "qwen2.5-7b-instruct".into();
    page.invalidate();
    page.ensure_loaded(&db);

    assert_eq!(
        page.lmstudio_base_url, "http://localhost:9999",
        "unsaved edits survive a reload (pre-fix the is_empty gate re-pulled stored values over them)"
    );
    assert_eq!(page.lmstudio_chat_model, "qwen2.5-7b-instruct");

    page.save_lmstudio(&db);
    assert!(page.error.is_none(), "save failed: {:?}", page.error);

    db.with_write(|conn| {
        spp_core::settings::set_string(conn, "lmstudio_base_url", "http://other:1234")
    })
    .expect("write");
    page.invalidate();
    page.ensure_loaded(&db);

    assert_eq!(
        page.lmstudio_base_url, "http://other:1234",
        "after save the form is clean, so the fresh read wins"
    );
    assert_eq!(
        page.lmstudio_chat_model, "qwen2.5-7b-instruct",
        "the SAVED edit is the persisted value now"
    );
}
