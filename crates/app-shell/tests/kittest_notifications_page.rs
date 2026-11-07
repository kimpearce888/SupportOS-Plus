#![cfg(feature = "kittest-ui")]

use eframe::egui;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use spp_core::db::DbHandle;
use spp_shell::design;
use spp_shell::pages::notifications::NotificationsPage;

struct NotificationsPageState {
    installed: bool,
    page: NotificationsPage,
    db: DbHandle,
}

fn seeded_page() -> (NotificationsPage, DbHandle) {
    let mut conn = rusqlite::Connection::open_in_memory().expect("in-memory db");
    spp_core::bootstrap::apply_all(&mut conn).expect("bootstrap");
    conn.execute(
        "INSERT INTO users (id, remote_id, first_name, last_name, email, mention)
         VALUES (5, 505, 'Ada', 'Lovelace', 'ada@example.com', 'ada')",
        [],
    )
    .expect("user");
    spp_core::settings::set_string(&conn, "me_remote_id", "505").expect("me");
    let row = |id: i64,
               ntype: &str,
               sev: &str,
               title: &str,
               body: &str,
               payload: Option<&str>,
               read: bool,
               ago: i64| {
        conn.execute(
            "INSERT INTO notifications (id, type, severity, title, body, target_user_id, dedup_key, created_at, read_at, payload)
             VALUES (?1, ?2, ?3, ?4, ?5, 5, ?6, ?7,
                     CASE WHEN ?8 = 1 THEN ?7 ELSE NULL END, ?9)",
            rusqlite::params![
                id,
                ntype,
                sev,
                title,
                body,
                format!("seed-{id}"),
                spp_core::business_hours::iso_days_ago(ago),
                read,
                payload
            ],
        )
        .expect("notification");
    };
    row(
        1,
        "customer_replied",
        "info",
        "Customer replied on #501 Export to CSV fails on large files",
        "Is there an ETA on the fix?",
        None,
        false,
        0,
    );
    row(
        2,
        "sla_breach",
        "error",
        "SLA breached on #505 Refund for duplicate charge",
        "",
        None,
        false,
        0,
    );
    row(
        3,
        "mentioned",
        "info",
        "Ada mentioned you on #490 Billing address will not save",
        "",
        None,
        true,
        1,
    );
    row(
        4,
        "campaign_reply",
        "info",
        "Campaign reply on Spring push",
        "",
        None,
        true,
        2,
    );
    row(
        5,
        "sync_failure",
        "error",
        "Sync failed on the inbox mirror",
        "",
        None,
        true,
        2,
    );
    row(
        6,
        "mentioned",
        "info",
        "Grace mentioned you on #477 Refund window",
        "",
        None,
        true,
        3,
    );
    // UX-35: a job-failure notification in the new shape — friendly title
    // and body, verbatim error preserved in the structured payload (the
    // card reveals it on hover).
    row(
        7,
        "job_failure",
        "warning",
        "Background job failed: AI task",
        "A background task could not reach the service it needs at localhost:1234 — the connection was refused, timed out, or the address does not resolve. If this is the AI assistant, check that LM Studio (or your OpenAI-compatible server) is running and that its address in Settings is correct. (Hover for the technical detail.)",
        Some(
            "{\"code\":\"unreachable\",\"raw\":\"LM Studio chat failed: error sending request for url (http://localhost:1234/v1/chat/completions)\",\"queue\":\"ai\",\"job_type\":\"ai\"}",
        ),
        false,
        0,
    );
    (NotificationsPage::new(), DbHandle::new(conn))
}

fn render_notifications_page(dark: bool, width: f32, name: &str) {
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
                s.page.ui(ui, &s.db);
            },
            NotificationsPageState {
                installed: false,
                page,
                db,
            },
        );
    h.run_ok();
    h.snapshot(name);
}

#[test]
fn r3_notifications_page_1440_light() {
    render_notifications_page(false, 1440.0, "r3_notifications_page_1440_light");
}

#[test]
fn r3_notifications_page_1024_light() {
    render_notifications_page(false, 1024.0, "r3_notifications_page_1024_light");
}

#[test]
fn r3_notifications_page_800_light() {
    render_notifications_page(false, 800.0, "r3_notifications_page_800_light");
}

#[test]
fn r3_notifications_page_1440_dark() {
    render_notifications_page(true, 1440.0, "r3_notifications_page_1440_dark");
}

#[test]
fn r3_notifications_page_1024_dark() {
    render_notifications_page(true, 1024.0, "r3_notifications_page_1024_dark");
}

#[test]
fn r3_notifications_page_800_dark() {
    render_notifications_page(true, 800.0, "r3_notifications_page_800_dark");
}

#[test]
fn r3_notifications_mentions_tab_light() {
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
                s.page.ui(ui, &s.db);
            },
            NotificationsPageState {
                installed: false,
                page,
                db,
            },
        );
    h.run_ok();
    h.get_all_by_label("Mentions")
        .next()
        .expect("the tab")
        .click();
    h.run_ok();
    h.run_steps(2);
    assert!(
        h.get_all_by_label("Mark Unread").next().is_some(),
        "the read mention rows must carry their mark-unread act"
    );
    h.snapshot("r3_notifications_mentions_tab_light");
}
