#![cfg(feature = "kittest-ui")]

use eframe::egui;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use spp_core::db::DbHandle;
use spp_shell::design;
use spp_shell::pages::outreach::OutreachPage;

struct OutreachPageState {
    installed: bool,
    page: OutreachPage,
    db: DbHandle,
}

const TICKET_CUSTOMERS_DEFINITION: &str = r#"{"combinator":"all","conditions":[{"kind":"history","metric":"ticket_count","op":"gte","value":1}],"exclude":[]}"#;
const CHURN_RISK_DEFINITION: &str = r#"{"combinator":"any","conditions":[{"kind":"rating","metric":"not_good_90d","op":"gte","value":1}],"exclude":[]}"#;

fn seeded_page() -> (OutreachPage, DbHandle) {
    let mut conn = rusqlite::Connection::open_in_memory().expect("in-memory db");
    spp_core::bootstrap::apply_all(&mut conn).expect("bootstrap");

    conn.execute(
        "INSERT INTO users (id, remote_id, first_name, last_name, email, mention)
         VALUES (5, 505, 'Ada', 'Lovelace', 'ada@example.com', 'ada')",
        [],
    )
    .expect("user");
    conn.execute(
        "INSERT INTO mailboxes (id, remote_id, name, email)
         VALUES (1, 101, 'Support', 'support@example.com')",
        [],
    )
    .expect("mailbox");
    conn.execute(
        "INSERT INTO customers (id, remote_id, first_name, last_name, email)
         VALUES (9, 909, 'Kim', 'Pearce', 'kim@example.com'),
                (10, 910, 'Ada', 'Customer', 'adac@example.com')",
        [],
    )
    .expect("customers");
    for conv in 1..=2i64 {
        conn.execute(
            "INSERT INTO conversations (id, remote_id, number, subject, preview, status, type,
                 mailbox_local_id, assignee_local_id, customer_local_id, priority,
                 created_at, updated_at, closed_at, first_customer_message_at, local_created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 'active', 'email', 1, 5, ?6, 'none',
                     '2026-01-01T09:00:00Z', '2026-01-01T09:00:00Z', NULL,
                     '2026-01-01T09:00:00Z', '2026-01-01T09:00:00Z')",
            rusqlite::params![
                conv,
                1000 + conv,
                conv,
                format!("Ticket {conv}"),
                "help please",
                8 + conv
            ],
        )
        .expect("conversation");
    }

    let ticket_segment = spp_core::outreach::save_segment_v2(
        &conn,
        None,
        "Ticket owners",
        None,
        TICKET_CUSTOMERS_DEFINITION,
    )
    .expect("ticket segment");
    let _churn_segment =
        spp_core::outreach::save_segment_v2(&conn, None, "Churn risk", None, CHURN_RISK_DEFINITION)
            .expect("churn segment");

    let tree = spp_core::segment::parse_segment_tree(
        &serde_json::from_str::<serde_json::Value>(TICKET_CUSTOMERS_DEFINITION)
            .expect("definition json"),
    )
    .expect("segment tree");
    let recipients = {
        let engine = spp_core::segment::SegmentEngine::new(&conn);
        let preview = engine.preview(&tree, 1, i64::MAX);
        preview
            .get("rows")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default()
    };
    assert!(
        !recipients.is_empty(),
        "the segment must match the seeded tickets"
    );
    let snapshot = serde_json::to_string(&tree.to_json()).expect("snapshot");
    let draft = spp_core::outreach::create_outreach_campaign(
        &conn,
        "Check-in",
        "How is it going?",
        "Hi {{first_name}}, just checking in.",
        1,
        &[],
        Some(ticket_segment),
        Some(&snapshot),
        &recipients,
    )
    .expect("draft campaign");

    let completed = spp_core::outreach::create_outreach_campaign(
        &conn,
        "Spring push",
        "Feature launch",
        "The feature is live — come see.",
        1,
        &[],
        Some(ticket_segment),
        Some(&snapshot),
        &recipients,
    )
    .expect("completed campaign");
    conn.execute(
        "UPDATE outreach_campaigns SET status = 'completed' WHERE id = ?1",
        rusqlite::params![completed],
    )
    .expect("complete the campaign");
    let _ = draft;

    spp_core::outreach::add_to_dnc(&conn, 9, "asked politely").expect("dnc 9");
    spp_core::outreach::add_to_dnc(&conn, 10, "manual").expect("dnc 10");

    (OutreachPage::new(), DbHandle::new(conn))
}

fn render_outreach_page(dark: bool, width: f32, name: &str) {
    let (page, db) = seeded_page();
    let mut h = Harness::builder()
        .with_size(egui::vec2(width, 900.0))
        .with_theme(if dark {
            egui::Theme::Dark
        } else {
            egui::Theme::Light
        })
        .build_ui_state(
            move |ui, s| {
                if !s.installed {
                    design::install(ui.ctx());
                    s.installed = true;
                }
                design::host_standard_page(ui, |ui| s.page.ui(ui, &s.db));
            },
            OutreachPageState {
                installed: false,
                page,
                db,
            },
        );
    h.run_ok();
    h.get_all_by_label("Check-in — How is it going?")
        .next()
        .expect("the Check-in campaign chip")
        .click();
    h.run_ok();
    h.snapshot(name);
}

#[test]
fn r3_outreach_page_1440_light() {
    render_outreach_page(false, 1440.0, "r3_outreach_page_1440_light");
}

#[test]
fn r3_outreach_page_1024_light() {
    render_outreach_page(false, 1024.0, "r3_outreach_page_1024_light");
}

#[test]
fn r3_outreach_page_800_light() {
    render_outreach_page(false, 800.0, "r3_outreach_page_800_light");
}

#[test]
fn r3_outreach_page_1440_dark() {
    render_outreach_page(true, 1440.0, "r3_outreach_page_1440_dark");
}

#[test]
fn r3_outreach_page_1024_dark() {
    render_outreach_page(true, 1024.0, "r3_outreach_page_1024_dark");
}

#[test]
fn r3_outreach_page_800_dark() {
    render_outreach_page(true, 800.0, "r3_outreach_page_800_dark");
}

#[test]
fn r3_outreach_segments_tab_light() {
    let (page, db) = seeded_page();
    let mut h = Harness::builder()
        .with_size(egui::vec2(1440.0, 900.0))
        .with_theme(egui::Theme::Light)
        .build_ui_state(
            move |ui, s| {
                if !s.installed {
                    design::install(ui.ctx());
                    s.installed = true;
                }
                design::host_standard_page(ui, |ui| s.page.ui(ui, &s.db));
            },
            OutreachPageState {
                installed: false,
                page,
                db,
            },
        );
    h.run_ok();
    h.get_all_by_label("Segments")
        .next()
        .expect("the tab")
        .click();
    h.run_ok();
    h.run_steps(2);
    assert!(
        h.get_all_by_label("Delete").next().is_some(),
        "the segment cards must carry their delete acts"
    );
    h.snapshot("r3_outreach_segments_tab_light");
}

#[test]
fn r3_outreach_new_campaign_light() {
    let (page, db) = seeded_page();
    let mut h = Harness::builder()
        .with_size(egui::vec2(1440.0, 900.0))
        .with_theme(egui::Theme::Light)
        .build_ui_state(
            move |ui, s| {
                if !s.installed {
                    design::install(ui.ctx());
                    s.installed = true;
                }
                design::host_standard_page(ui, |ui| s.page.ui(ui, &s.db));
            },
            OutreachPageState {
                installed: false,
                page,
                db,
            },
        );
    h.run_ok();
    h.get_all_by_label("New Campaign")
        .next()
        .expect("the New Campaign act")
        .click();
    h.run_ok();
    h.run_steps(2);
    assert!(
        h.get_all_by_label("Create Campaign").next().is_some(),
        "the New Campaign form must show the create act"
    );
    assert!(
        h.query_all_by_label("Mailbox id").next().is_none(),
        "the raw mailbox id field is gone"
    );
    assert!(
        h.query_all_by_label("Segment id").next().is_none(),
        "the raw segment id field is gone"
    );
    assert!(
        h.query_all_by_label_contains("saved segments")
            .next()
            .is_none(),
        "the saved-segments hint line is gone — the picker is the hint"
    );
    let picker_count = h
        .query_all(egui_kittest::kittest::By::new().role(egui::Role::ComboBox))
        .count();
    assert_eq!(
        picker_count, 2,
        "the mailbox and segment pickers are the form's two combo boxes"
    );
    h.query_all(egui_kittest::kittest::By::new().role(egui::Role::ComboBox))
        .next()
        .expect("the mailbox picker")
        .click();
    h.run_ok();
    h.run_steps(2);
    assert!(
        h.query_all(
            egui_kittest::kittest::By::new()
                .role(egui::Role::Button)
                .label("Support")
        )
        .next()
        .is_some(),
        "the seeded mailbox 'Support' is the picker's option"
    );
    h.get_all_by_label("Support")
        .next()
        .expect("the Support option")
        .click();
    h.run_ok();
    h.run_steps(2);
    h.snapshot("r3_outreach_new_campaign_light");
}

#[test]
fn audp2_outreach_segment_preview_light() {
    let (page, db) = seeded_page();
    let mut h = Harness::builder()
        .with_size(egui::vec2(1440.0, 900.0))
        .with_theme(egui::Theme::Light)
        .build_ui_state(
            move |ui, s| {
                if !s.installed {
                    design::install(ui.ctx());
                    s.installed = true;
                }
                design::host_standard_page(ui, |ui| s.page.ui(ui, &s.db));
            },
            OutreachPageState {
                installed: false,
                page,
                db,
            },
        );
    h.run_ok();
    h.get_all_by_label("Segments")
        .next()
        .expect("the tab")
        .click();
    h.run_ok();
    h.run_steps(2);
    h.get_all_by_label("Preview")
        .next()
        .expect("the preview act on the segment rows")
        .click();
    h.run_ok();
    h.run_steps(2);
    assert!(
        h.query_all_by_label_contains("recipients").next().is_some(),
        "the preview must show a recipient count"
    );
    h.snapshot("audp2_outreach_segment_preview_light");
}
