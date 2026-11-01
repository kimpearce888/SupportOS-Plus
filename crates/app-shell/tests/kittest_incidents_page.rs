#![cfg(feature = "kittest-ui")]

use eframe::egui;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use spp_core::db::DbHandle;
use spp_core::intelligence_features::ManualIncident;
use spp_shell::design;
use spp_shell::pages::incidents::IncidentsPage;

struct IncidentsPageState {
    installed: bool,
    page: IncidentsPage,
    db: DbHandle,
}

fn seeded_page() -> (IncidentsPage, DbHandle) {
    let mut conn = rusqlite::Connection::open_in_memory().expect("in-memory db");
    spp_core::bootstrap::apply_all(&mut conn).expect("bootstrap");
    seed_mailbox(&conn);
    seed_user(&conn, 5, 500, "Nabin", "Sayeed");
    seed_customer(&conn, 9, 909, "Kim", "Pearce", "kim@example.test");
    seed_customer(&conn, 11, 911, "Nabin", "Sayeed", "nabin@example.test");

    seed_conversation(
        &conn,
        1,
        5021,
        "Export broken for CSV files",
        "active",
        9,
        "2026-01-12T07:02:00.000Z",
    );
    seed_conversation(
        &conn,
        2,
        5019,
        "Card declined for valid card",
        "active",
        11,
        "2026-01-12T06:40:00.000Z",
    );

    let incident = |title: &str, status: &str, severity: &str| ManualIncident {
        title: title.into(),
        status: status.into(),
        severity: severity.into(),
        owner_user_local_id: None,
        product: None,
        feature: None,
        description: None,
        internal_explanation: None,
        customer_safe_explanation: None,
        known_cause: None,
        workaround: None,
        resolution: None,
        started_at: None,
        conversation_ids: Vec::new(),
    };
    let first = spp_core::intelligence_features::create_manual_incident(
        &conn,
        &ManualIncident {
            description: Some(
                "Payment provider EU region returning 502s; checkout conversions \
                 down 60% in the affected region."
                    .into(),
            ),
            customer_safe_explanation: Some(
                "Some customers in Europe may see an error at checkout. Our team \
                 is on it."
                    .into(),
            ),
            conversation_ids: vec![1, 2],
            ..incident("Checkout outages for EU customers", "investigating", "sev1")
        },
    )
    .expect("first incident");
    spp_core::intelligence_features::create_manual_incident(
        &conn,
        &incident("SSO redirect loop", "identified", "sev2"),
    )
    .expect("second incident");
    spp_core::intelligence_features::create_manual_incident(
        &conn,
        &incident("CSV export failures", "monitoring", "sev3"),
    )
    .expect("third incident");

    conn.execute(
        "UPDATE incidents SET started_at = '2026-01-12 08:41:03',
             owner_user_local_id = 5,
             updated_at = CASE id WHEN 1 THEN '2026-01-12T11:00:00.000Z'
                                  WHEN 2 THEN '2026-01-12T09:00:00.000Z'
                                  ELSE '2026-01-12T08:00:00.000Z' END",
        [],
    )
    .expect("pin started/owner/updated");

    conn.execute(
        "INSERT INTO incident_notes (incident_id, author_user_local_id, body, created_at)
         VALUES (?1, 5, 'Mitigated by rolling back the payments deploy',
                 '2026-01-12T09:15:44.000Z')",
        rusqlite::params![first],
    )
    .expect("note");
    conn.execute(
        "INSERT INTO incident_releases (incident_id, version_label, notes, released_at,
             correlation, created_at)
         VALUES (?1, '2.1.4', NULL, '2026-01-12T09:20:00.000Z', NULL,
                 '2026-01-12T09:20:00.000Z'),
                (?1, '2.1.5', NULL, NULL, NULL, '2026-01-12T10:30:00.000Z')",
        rusqlite::params![first],
    )
    .expect("releases");

    (IncidentsPage::new(), DbHandle::new(conn))
}

fn seed_mailbox(conn: &rusqlite::Connection) {
    conn.execute(
        "INSERT INTO mailboxes (id, remote_id, name, email)
         VALUES (1, 101, 'Support', 'support@example.com')",
        [],
    )
    .expect("mailbox");
}

fn seed_user(conn: &rusqlite::Connection, id: i64, remote_id: i64, first: &str, last: &str) {
    conn.execute(
        "INSERT INTO users (id, remote_id, first_name, last_name, email, user_type)
         VALUES (?1, ?2, ?3, ?4, ?5, 'user')",
        rusqlite::params![
            id,
            remote_id,
            first,
            last,
            format!("{first}@example.test").to_lowercase()
        ],
    )
    .expect("user");
}

fn seed_customer(
    conn: &rusqlite::Connection,
    id: i64,
    remote_id: i64,
    first: &str,
    last: &str,
    email: &str,
) {
    conn.execute(
        "INSERT INTO customers (id, remote_id, first_name, last_name, email, organization)
         VALUES (?1, ?2, ?3, ?4, ?5, NULL)",
        rusqlite::params![id, remote_id, first, last, email],
    )
    .expect("customer");
}

fn seed_conversation(
    conn: &rusqlite::Connection,
    id: i64,
    number: i64,
    subject: &str,
    status: &str,
    customer_id: i64,
    at: &str,
) {
    conn.execute(
        "INSERT INTO conversations (id, remote_id, number, subject, preview, status, type,
             mailbox_local_id, customer_local_id, priority,
             created_at, updated_at, closed_at, first_customer_message_at, local_created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'email', 1, ?7, 'none', ?8, ?8,
                 CASE WHEN ?6 = 'closed' THEN ?8 ELSE NULL END, ?8, ?8)",
        rusqlite::params![
            id,
            1000 + id,
            number,
            subject,
            format!("{subject} preview"),
            status,
            customer_id,
            at,
        ],
    )
    .expect("conversation");
}

fn render_incidents_page(dark: bool, width: f32, name: &str) {
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
            IncidentsPageState {
                installed: false,
                page,
                db,
            },
        );
    h.run_ok();
    h.snapshot(name);
}

#[test]
fn r3_incidents_page_1440_light() {
    render_incidents_page(false, 1440.0, "r3_incidents_page_1440_light");
}

#[test]
fn r3_incidents_page_1024_light() {
    render_incidents_page(false, 1024.0, "r3_incidents_page_1024_light");
}

#[test]
fn r3_incidents_page_800_light() {
    render_incidents_page(false, 800.0, "r3_incidents_page_800_light");
}

#[test]
fn r3_incidents_page_1440_dark() {
    render_incidents_page(true, 1440.0, "r3_incidents_page_1440_dark");
}

#[test]
fn r3_incidents_page_1024_dark() {
    render_incidents_page(true, 1024.0, "r3_incidents_page_1024_dark");
}

#[test]
fn r3_incidents_page_800_dark() {
    render_incidents_page(true, 800.0, "r3_incidents_page_800_dark");
}

#[test]
fn r3_incidents_open_detail_light() {
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
            IncidentsPageState {
                installed: false,
                page,
                db,
            },
        );
    h.run_ok();
    h.get_all_by_label("open")
        .next()
        .expect("the first row's open act")
        .click();
    h.run_ok();
    h.run_steps(2);
    assert!(
        h.query_by_label("close panel").is_some(),
        "the DetailPanel must be open after the row's open act"
    );
    h.snapshot("r3_incidents_detail_light");
}
