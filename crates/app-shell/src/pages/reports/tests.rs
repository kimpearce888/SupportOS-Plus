use super::*;
use crate::testsupport::fixture;

#[test]
fn sla_report_loads_and_bar_points_map() {
    let db = fixture(Vec::new());
    let report = db.with_read(|conn| {
        spp_core::sla::sla_report(
            conn,
            &spp_core::business_hours::iso_days_ago(30),
            &spp_core::business_hours::now_iso_millis(),
            None,
        )
        .expect("sla report")
    });
    assert!(report.mailboxes.len() >= 2);
    assert!(report.range.from.starts_with("20"));
}

#[test]
fn overview_and_lists_load_from_seeded_db() {
    let db = fixture(vec![("conv", 1, "Help export", "active", "email", 1, 5, 9)]);
    let mut page = ReportsPage::new();
    page.reload(&db);

    let overview = page.overview.clone().expect("overview");
    assert_eq!(overview.new_conversations, 1);
    assert_eq!(page.why_contacting.len(), 0);
    assert_eq!(page.top_questions.len(), 0);
    assert_eq!(page.radar.len(), 0);
    assert!(page.sla.is_some());

    page.days = 7;
    page.reload(&db);
    assert_eq!(page.overview.expect("overview 2").new_conversations, 1);
}

#[test]
fn why_contacting_rows_have_category_shape() {
    let row = serde_json::json!({ "category": "billing", "count": 3 });
    assert_eq!(i64_field(&row, "count"), 3);
    assert_eq!(str_field(&row, "category"), "billing");
}

#[test]
fn builder_tab_run_save_load_delete_round_trip() {
    let db = fixture(Vec::new());
    let mut page = ReportsPage::new();
    page.reload(&db);
    page.b_save_name = "my report".into();
    page.save_report_config(&db);
    assert!(
        page.b_note.as_ref().expect("note").0,
        "save over a clean mirror succeeds"
    );
    assert_eq!(page.b_saved.len(), 1);

    let config_json = page.b_saved[0].config.clone();
    page.b_metric = 3;
    page.b_from = "2000-01-01".into();
    page.load_saved_report(&config_json);
    assert_eq!(page.b_metric, 0, "the saved metric is restored");
    assert!(page.b_note.as_ref().expect("note").0);

    let config = page.build_config().expect("config");
    let outcome = db
        .with_read(|conn| spp_core::reports::run_builder_report(conn, &config))
        .expect("run");
    assert!(!outcome.notes.is_empty());
    page.delete_saved_report(&db, page.b_saved[0].id).ok();
    assert!(page.b_saved.is_empty(), "deleted");
}

#[test]
fn builder_form_validates_inputs() {
    let mut page = ReportsPage::new();
    page.b_f_mailbox_ids = "abc".into();
    assert!(page.build_config().is_err());
    page.b_f_mailbox_ids.clear();
    page.b_limit = "many".into();
    assert!(page.build_config().is_err());
    page.b_limit.clear();
    page.b_from = "bad".into();
    assert!(page.build_config().is_err());
    page.b_from = "2026-01-01".into();
    assert!(page.build_config().is_ok());
}

#[test]
fn insights_tab_loads_surface_data() {
    let db = fixture(Vec::new());
    let mut page = ReportsPage::new();
    page.reload(&db);
    assert!(page.i_health.is_some(), "health facts loaded");
    assert!(page.i_effectiveness.is_some(), "effectiveness loaded");

    page.i_g_kind = 8;
    page.i_g_label = "Spring push".into();
    page.add_graph_node_action(&db);
    assert!(page.i_note.as_ref().expect("note").0, "node added");
    assert_eq!(page.i_graph_nodes.len(), 1);
    let node = &page.i_graph_nodes[0];
    assert_eq!(node.kind, "campaign");
    assert_eq!(node.label.as_deref(), Some("Spring push"));

    page.i_g_label = "bad".into();
    page.i_g_props = "{not json".into();
    page.add_graph_node_action(&db);
    assert!(!page.i_note.as_ref().expect("note").0, "rejected");
    assert_eq!(page.i_graph_nodes.len(), 1, "nothing was written");
}
