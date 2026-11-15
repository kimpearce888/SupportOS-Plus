use super::*;
use crate::testsupport::fixture;

#[test]
fn settings_default_to_lmstudio_endpoint() {
    let db = fixture(Vec::new());
    let settings = db.with_read(load_ai_settings);
    assert!(settings.ai_enabled);
    assert_eq!(settings.base_url, "http://127.0.0.1:1234");
    assert_eq!(settings.timeout_ms, 120_000);
    assert_eq!(settings.concurrency, 2);
    assert!(settings.chat_model.is_none());
}

#[test]
fn analytics_counts_seeded_runs_and_drafts() {
    let db = fixture(vec![("conv", 1, "Analyzed", "active", "email", 1, 5, 9)]);
    db.with_write(|conn| {
        conn.execute(
            "INSERT INTO ai_runs (input_hash, prompt_version, model, response_json, type, status, conversation_id, created_at)
             VALUES ('r1', 'v1', 'm', '{}', 'ticket_analysis', 'completed', 1, '2026-01-01T00:00:00Z'),
                    ('r2', 'v1', 'm', '{}', 'ticket_analysis', 'failed', NULL, '2026-01-01T00:01:00Z')",
            [],
        )
        .expect("runs");
        conn.execute(
            "INSERT INTO ai_drafts (conversation_id, content, state, created_at)
             VALUES (1, 'draft one', 'accepted', '2026-01-01T00:02:00Z'),
                    (1, 'draft two', 'rejected', '2026-01-01T00:03:00Z')",
            [],
        )
        .expect("drafts");
    });
    let analytics = db.with_read(load_analytics);
    assert_eq!(analytics.tickets_analyzed, 1);
    assert_eq!(analytics.analysis_success_rate, 50.0);
    assert_eq!(analytics.draft_count, 2);
    assert_eq!(analytics.drafts_accepted, 1);
    assert_eq!(analytics.drafts_rejected, 1);
    assert!(analytics.failure_patterns.is_empty());
}

#[test]
fn jobs_rows_load_after_seeding() {
    let db = fixture(vec![("conv", 7, "Seven", "active", "email", 1, 5, 9)]);
    db.with_write(|conn| {
        conn.execute(
            "INSERT INTO ai_runs (input_hash, prompt_version, model, response_json, type, status, conversation_id, created_at)
             VALUES ('r3', 'v1', 'qwen-7b', '{}', 'ticket_analysis', 'completed', 7, '2026-01-01T00:00:00Z')",
            [],
        )
        .expect("run");
    });
    let jobs = db.with_read(|conn| load_jobs(conn, 100));
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].run_type, "ticket_analysis");
    assert_eq!(jobs[0].conversation_id, Some(7));
    assert_eq!(jobs[0].model.as_deref(), Some("qwen-7b"));
}

#[test]
fn distributions_empty_on_a_fresh_database() {
    let db = fixture(Vec::new());
    let distributions = db
        .with_read(spp_core::ai_attributes::distributions)
        .unwrap_or_default();
    assert!(distributions.iter().all(|d| d.total_conversations >= 0));
}

fn golden_eval_jobs(db: &DbHandle) -> i64 {
    db.with_read(|conn| {
        conn.query_row(
            "SELECT COUNT(*) FROM jobs WHERE queue = 'ai' AND type = 'golden_eval'",
            [],
            |r| r.get(0),
        )
    })
    .expect("count golden_eval jobs")
}

#[test]
fn audit74_commit_persists_once_and_enqueues_one_eval() {
    let db = fixture(Vec::new());
    let mut page = AiCenterPage::new();
    page.reload(&db);
    assert_eq!(golden_eval_jobs(&db), 0);

    let mut roles = page.hub_roles.clone();
    roles.draft = Some(spp_core::ai_hub::RoleAssignment {
        endpoint: "lmstudio".into(),
        model: Some("qwen2.5-7b".into()),
    });
    page.commit_role_map(&db, &roles);
    assert_eq!(golden_eval_jobs(&db), 1, "one eval per committed edit");
    assert_eq!(page.hub_roles_saved, roles, "the snapshot catches up");

    let stored = db.with_read(spp_core::ai_hub::load_roles).expect("roles");
    assert_eq!(
        stored.draft.and_then(|d| d.model),
        Some("qwen2.5-7b".into())
    );

    page.commit_role_map(&db, &roles);
    assert_eq!(
        golden_eval_jobs(&db),
        1,
        "no second eval for an unchanged map through the UI path (blur guard)"
    );
}

#[test]
fn audit74_typing_divergence_never_persists_or_enqueues() {
    let db = fixture(Vec::new());
    let ctx = egui::Context::default();
    let mut page = AiCenterPage::new();
    page.reload(&db);
    page.hub_roles.draft = Some(spp_core::ai_hub::RoleAssignment {
        endpoint: "lmstudio".into(),
        model: Some("q".into()),
    });
    let saved_before = page.hub_roles_saved.clone();
    assert_ne!(
        page.hub_roles, page.hub_roles_saved,
        "the divergence is set up"
    );

    for _ in 0..10 {
        ctx.run_pass(egui::RawInput::default(), |ctx| {
            ctx.root_ui(|ui| {
                egui::CentralPanel::default().show(ui, |ui| page.ui(ui, &db));
            });
        })
        .drop_without_applying_deltas();
    }
    assert_eq!(
        golden_eval_jobs(&db),
        0,
        "no golden_eval may be enqueued without a committed (blurred) edit"
    );
    let stored = db.with_read(spp_core::ai_hub::load_roles).expect("roles");
    assert_eq!(
        stored.draft.and_then(|d| d.model),
        None,
        "the divergent live map must not be persisted"
    );
    assert_eq!(
        page.hub_roles_saved, saved_before,
        "the snapshot is untouched"
    );
    assert_eq!(page.hub_roles.draft.and_then(|d| d.model), Some("q".into()));
}

#[test]
fn audit75_global_enter_never_submits_the_copilot() {
    let db = fixture(Vec::new());
    let ctx = egui::Context::default();
    let mut page = AiCenterPage::new();
    page.tab = Tab::Copilot;
    page.copilot_input = "what is the refund policy?".into();

    ctx.run_pass(egui::RawInput::default(), |ctx| {
        ctx.root_ui(|ui| {
            egui::CentralPanel::default().show(ui, |ui| page.ui(ui, &db));
        });
    })
    .drop_without_applying_deltas();
    let mut enter = egui::RawInput::default();
    enter.events.push(egui::Event::Key {
        key: egui::Key::Enter,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    });
    ctx.run_pass(enter, |ctx| {
        ctx.root_ui(|ui| {
            egui::CentralPanel::default().show(ui, |ui| page.ui(ui, &db));
        });
    })
    .drop_without_applying_deltas();

    let jobs: i64 = db
        .with_read(|conn| {
            conn.query_row(
                "SELECT COUNT(*) FROM jobs WHERE queue = 'ai' AND type = 'copilot_chat'",
                [],
                |r| r.get(0),
            )
        })
        .expect("count copilot jobs");
    assert_eq!(
        jobs, 0,
        "a global Enter may not submit the copilot chat anymore"
    );
    assert!(
        page.copilot_pending.is_none(),
        "no in-flight job was started either"
    );
    assert_eq!(page.copilot_input, "what is the refund policy?");
}

#[test]
fn provider_record_save_round_trips() {
    let db = fixture(Vec::new());
    let mut page = AiCenterPage::new();
    page.reload(&db);
    assert_eq!(
        page.ai_record
            .as_ref()
            .expect("record loaded")
            .provider_kind,
        spp_core::ai_center::ProviderKind::None
    );
    page.ai_kind_pick = 1;
    page.ai_chat_model_edit = "test-chat".into();
    page.ai_base_url_edit = "http://127.0.0.1:9999".into();
    page.save_ai_record(&db);
    let record = db
        .with_read(spp_core::ai_center::get_ai_status)
        .expect("status");
    assert_eq!(
        record.provider_kind,
        spp_core::ai_center::ProviderKind::LmStudio
    );
    assert_eq!(record.chat_model.as_deref(), Some("test-chat"));
    assert_eq!(record.base_url.as_deref(), Some("http://127.0.0.1:9999"));
    assert!(spp_core::ai_center::ai_features_enabled(&record));
    assert!(page.ai_record_note.as_ref().expect("note").0);
}

#[test]
fn provider_record_requires_positive_dim_with_embed_model() {
    let db = fixture(Vec::new());
    let mut page = AiCenterPage::new();
    page.reload(&db);
    page.ai_kind_pick = 1;
    page.ai_embed_model_edit = "bge-m3".into();
    page.ai_embed_dim_edit = "0".into();
    page.save_ai_record(&db);
    let note = page.ai_record_note.clone().expect("note");
    assert!(!note.0, "dim=0 with a model must be an error note");
    let record = db
        .with_read(spp_core::ai_center::get_ai_status)
        .expect("status");
    assert_eq!(
        record.provider_kind,
        spp_core::ai_center::ProviderKind::None
    );
    assert!(record.embedding_model.is_none(), "nothing was written");
}

#[test]
fn provider_record_dirty_form_survives_reload() {
    let db = fixture(Vec::new());
    let mut page = AiCenterPage::new();
    page.reload(&db);
    page.ai_chat_model_edit = "typed-but-unsaved".into();
    page.ai_form_clean = false;
    page.invalidate();
    page.reload(&db);
    assert_eq!(
        page.ai_chat_model_edit, "typed-but-unsaved",
        "unsaved edits survive invalidation (batch-27 convention)"
    );
}
