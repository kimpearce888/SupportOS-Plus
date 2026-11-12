#![cfg(feature = "kittest-ui")]

use eframe::egui;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use spp_core::db::DbHandle;
use spp_shell::design;
use spp_shell::pages::automation::AutomationPage;

struct AutomationPageState {
    installed: bool,
    page: AutomationPage,
    db: DbHandle,
}

fn seeded_page() -> (AutomationPage, DbHandle) {
    let mut conn = rusqlite::Connection::open_in_memory().expect("in-memory db");
    spp_core::bootstrap::apply_all(&mut conn).expect("bootstrap");
    conn.execute(
        "INSERT INTO users (id, remote_id, first_name, last_name, email, mention)
         VALUES (5, 505, 'Ada', 'Lovelace', 'ada@example.com', 'ada')",
        [],
    )
    .expect("users");
    conn.execute(
        "INSERT INTO automation_rules (name, trigger_json, action_json, enabled)
         VALUES ('Close stale solved', '{\"kind\":\"sla_risk\"}',
                 '{\"kind\":\"add_tag\",\"tag\":\"stale\"}', 1),
                ('Escalate urgent replies', '{\"kind\":\"tag_added\",\"tag\":\"urgent\"}',
                 '{\"kind\":\"set_priority\",\"new_priority\":\"urgent\"}', 1)",
        [],
    )
    .expect("rules");
    let rule_id: i64 = conn.last_insert_rowid();
    conn.execute(
        "INSERT INTO automation_approvals (rule_id, conversation_id, proposed_action_json)
         VALUES (?1, 2, '{\"kind\":\"set_priority\",\"priority\":\"urgent\"}')",
        rusqlite::params![rule_id],
    )
    .expect("approval");
    (AutomationPage::new(), DbHandle::new(conn))
}

fn render_automation_page(dark: bool, width: f32, name: &str) {
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
            AutomationPageState {
                installed: false,
                page,
                db,
            },
        );
    h.run_ok();
    h.snapshot(name);
}

#[test]
fn r3_automation_page_1440_light() {
    render_automation_page(false, 1440.0, "r3_automation_page_1440_light");
}

#[test]
fn r3_automation_page_1024_light() {
    render_automation_page(false, 1024.0, "r3_automation_page_1024_light");
}

#[test]
fn r3_automation_page_800_light() {
    render_automation_page(false, 800.0, "r3_automation_page_800_light");
}

#[test]
fn r3_automation_page_1440_dark() {
    render_automation_page(true, 1440.0, "r3_automation_page_1440_dark");
}

#[test]
fn r3_automation_page_1024_dark() {
    render_automation_page(true, 1024.0, "r3_automation_page_1024_dark");
}

#[test]
fn r3_automation_page_800_dark() {
    render_automation_page(true, 800.0, "r3_automation_page_800_dark");
}

#[test]
fn r3_automation_new_rule_light() {
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
            AutomationPageState {
                installed: false,
                page,
                db,
            },
        );
    h.run_ok();
    h.get_all_by_label("New Rule")
        .next()
        .expect("the New Rule act")
        .click();
    h.run_ok();
    h.run_steps(2);
    assert!(
        h.get_all_by_label("Create Rule").next().is_some(),
        "the New Rule form must show the Create Rule act"
    );
    h.snapshot("r3_automation_new_rule_light");
}

#[test]
fn audp1_automation_edit_assign_rule_light() {
    let (page, db) = seeded_page();
    db.with_write(|conn| {
        conn.execute(
            "INSERT INTO automation_rules (name, trigger_json, action_json, enabled)
             VALUES ('Assign escalations', '{\"kind\":\"customer_reply\"}',
                     '{\"kind\":\"assign\",\"assignee_remote_id\":505}', 1)",
            [],
        )
        .expect("assign rule");
        Ok::<(), rusqlite::Error>(())
    })
    .expect("seed");
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
            AutomationPageState {
                installed: false,
                page,
                db,
            },
        );
    h.run_ok();
    h.get_all_by_label("Edit")
        .nth(2)
        .expect("the third rule's Edit act")
        .click();
    h.run_ok();
    h.run_steps(2);
    assert!(
        h.get_all_by_label("Edit Rule").next().is_some(),
        "the form must open in edit mode for the seeded rule"
    );
    assert!(
        h.get_all_by_label("Ada Lovelace").next().is_some(),
        "the assignee picker must be prefilled with the rule's assignee"
    );
    h.snapshot("audp1_automation_edit_assign_rule_light");
}
