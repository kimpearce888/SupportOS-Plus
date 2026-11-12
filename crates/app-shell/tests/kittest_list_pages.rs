#![cfg(feature = "kittest-ui")]

use eframe::egui;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use spp_core::db::DbHandle;
use spp_shell::design;
use spp_shell::pages::NavRequest;
use spp_shell::pages::customers::CustomersPage;
use spp_shell::pages::organizations::OrganizationsPage;

struct OrganizationsPageState {
    installed: bool,
    page: OrganizationsPage,
    db: DbHandle,
}

fn seeded_page() -> (OrganizationsPage, DbHandle) {
    let mut conn = rusqlite::Connection::open_in_memory().expect("in-memory db");
    spp_core::bootstrap::apply_all(&mut conn).expect("bootstrap");
    seed_mailbox(&conn);
    let northwind = spp_core::people_store::create_organization(
        &conn,
        "Northwind Logistics",
        &["northwind-logistics.test".into()],
    )
    .expect("northwind");
    let acme = spp_core::people_store::create_organization(
        &conn,
        "Acme",
        &["acme.com".into(), "acme.io".into()],
    )
    .expect("acme");
    let _globex =
        spp_core::people_store::create_organization(&conn, "Globex", &["globex.com".into()])
            .expect("globex");

    seed_customer(
        &conn,
        9,
        909,
        "Kim",
        "Pearce",
        "kim@northwind-logistics.test",
        Some("Head of Support"),
        northwind,
    );
    seed_customer(
        &conn,
        10,
        910,
        "Marta",
        "Keller",
        "marta@acme.test",
        Some("Ops lead"),
        acme,
    );
    seed_customer(
        &conn,
        11,
        911,
        "John",
        "Smith",
        "john@acme.test",
        None,
        acme,
    );

    seed_conversation(
        &conn,
        1,
        101,
        "Export broken",
        "active",
        9,
        "2026-02-16T10:10:00.000Z",
    );
    seed_conversation(
        &conn,
        2,
        102,
        "Billing address update",
        "pending",
        9,
        "2026-02-16T09:52:00.000Z",
    );
    seed_conversation(
        &conn,
        3,
        103,
        "Login loop on Safari",
        "closed",
        9,
        "2026-02-15T18:44:00.000Z",
    );
    seed_conversation(
        &conn,
        4,
        104,
        "Duplicate charge",
        "active",
        10,
        "2026-02-16T09:05:00.000Z",
    );
    seed_conversation(
        &conn,
        5,
        105,
        "Export format question",
        "closed",
        10,
        "2026-02-14T08:12:00.000Z",
    );
    seed_conversation(
        &conn,
        6,
        106,
        "API key rotation",
        "active",
        11,
        "2026-02-16T08:30:00.000Z",
    );

    (OrganizationsPage::new(), DbHandle::new(conn))
}

fn seed_mailbox(conn: &rusqlite::Connection) {
    conn.execute(
        "INSERT INTO mailboxes (id, remote_id, name, email)
         VALUES (1, 101, 'Support', 'support@example.com')",
        [],
    )
    .expect("mailbox");
}

#[allow(clippy::too_many_arguments)]
fn seed_customer(
    conn: &rusqlite::Connection,
    id: i64,
    remote_id: i64,
    first: &str,
    last: &str,
    email: &str,
    job_title: Option<&str>,
    organization_id: i64,
) {
    conn.execute(
        "INSERT INTO customers (id, remote_id, first_name, last_name, email, job_title,
             organization_id, organization)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL)",
        rusqlite::params![
            id,
            remote_id,
            first,
            last,
            email,
            job_title,
            organization_id
        ],
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

fn render_organizations_page(dark: bool, width: f32, name: &str) {
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
                // AUD-V1: the production host for standard pages —
                // CentralPanel + scroll + content column + the SP5/14
                // inset, exactly as app/mod.rs hosts them.
                egui::CentralPanel::default()
                    .frame(egui::Frame::new().fill(design::tokens(ui).background))
                    .show(ui, |ui| {
                        egui::ScrollArea::vertical()
                            .id_salt("spp-page-scroll")
                            .auto_shrink([false; 2])
                            .show(ui, |ui| {
                                ui.add_space(2.0);
                                design::content_column(ui, |ui| {
                                    egui::Frame::new()
                                        .fill(egui::Color32::TRANSPARENT)
                                        .inner_margin(egui::Margin::symmetric(
                                            design::scale::space::SP5 as i8,
                                            14,
                                        ))
                                        .show(ui, |ui| {
                                            s.page.ui(ui, &s.db, None);
                                        });
                                });
                            });
                    });
            },
            OrganizationsPageState {
                installed: false,
                page,
                db,
            },
        );
    h.run_ok();
    h.snapshot(name);
}

#[test]
fn r3_organizations_page_1440_light() {
    render_organizations_page(false, 1440.0, "r3_organizations_page_1440_light");
}

#[test]
fn r3_organizations_page_1024_light() {
    render_organizations_page(false, 1024.0, "r3_organizations_page_1024_light");
}

#[test]
fn r3_organizations_page_800_light() {
    render_organizations_page(false, 800.0, "r3_organizations_page_800_light");
}

#[test]
fn r3_organizations_page_1440_dark() {
    render_organizations_page(true, 1440.0, "r3_organizations_page_1440_dark");
}

#[test]
fn r3_organizations_page_1024_dark() {
    render_organizations_page(true, 1024.0, "r3_organizations_page_1024_dark");
}

#[test]
fn r3_organizations_page_800_dark() {
    render_organizations_page(true, 800.0, "r3_organizations_page_800_dark");
}

struct CustomersPageState {
    installed: bool,
    page: CustomersPage,
    db: DbHandle,
}

fn seeded_customers_page() -> (CustomersPage, DbHandle) {
    let mut conn = rusqlite::Connection::open_in_memory().expect("in-memory db");
    spp_core::bootstrap::apply_all(&mut conn).expect("bootstrap");
    // AIE-1-4: pin the shot clock so the fixed 2026-02 seeds render the
    // same "Nd ago" Last Contact column on every calendar day — the
    // UXB-T1 freeze covered sync_health, operations, notifications and
    // reports, but the customers fixtures were left on the wall clock and
    // their goldens ticked one day further at a UTC midnight (the drift
    // gate fired on 2026-11-09, a day after the UXG-1 regen).
    assert!(
        spp_core::business_hours::pin_now_for_kittest(
            spp_core::business_hours::KITTEST_PINNED_NOW_ISO
        ),
        "the shot clock must pin to the golden anchor"
    );
    seed_mailbox(&conn);
    let northwind = spp_core::people_store::create_organization(
        &conn,
        "Northwind Logistics",
        &["northwind-logistics.test".into()],
    )
    .expect("northwind");

    seed_customer_row(
        &conn,
        9,
        909,
        "Kim",
        "Pearce",
        "kim@northwind-logistics.test",
        Some("+880 17 555 0142"),
        Some("Head of Support"),
        Some(northwind),
        "2026-02-10T08:00:00.000Z",
    );
    seed_customer_row(
        &conn,
        10,
        910,
        "Ada",
        "Customer",
        "ada@brightside.test",
        None,
        None,
        None,
        "2026-02-09T08:00:00.000Z",
    );
    seed_customer_row(
        &conn,
        11,
        911,
        "Nabin",
        "Sayeed",
        "nabin@brightside.test",
        None,
        None,
        None,
        "2026-02-08T08:00:00.000Z",
    );
    seed_customer_row(
        &conn,
        12,
        912,
        "Marta",
        "Keller",
        "marta@acme.test",
        None,
        None,
        None,
        "2026-02-07T08:00:00.000Z",
    );

    seed_customer_conversation(
        &conn,
        1,
        501,
        "Export to CSV fails on large files",
        "active",
        9,
        "2026-02-16T10:10:00.000Z",
    );
    seed_customer_conversation(
        &conn,
        2,
        503,
        "Cannot sign in with SSO",
        "active",
        9,
        "2026-02-15T22:26:00.000Z",
    );
    seed_customer_conversation(
        &conn,
        3,
        505,
        "Refund for duplicate charge",
        "pending",
        9,
        "2026-02-14T23:26:00.000Z",
    );
    seed_customer_conversation(
        &conn,
        4,
        507,
        "Password reset link expired",
        "active",
        10,
        "2026-02-15T09:05:00.000Z",
    );
    seed_customer_conversation(
        &conn,
        5,
        508,
        "Invoice PDF arrives blank",
        "closed",
        10,
        "2026-02-13T08:12:00.000Z",
    );
    seed_customer_conversation(
        &conn,
        6,
        509,
        "API rate limit questions",
        "active",
        11,
        "2026-02-12T10:00:00.000Z",
    );

    seed_customer_event(
        &conn,
        9,
        "support_conversation",
        "2026-02-16T10:14:00.000Z",
        "ENG-2293 merged to staging; update promised",
        "seed-kim-ev-1",
    );
    seed_customer_event(
        &conn,
        9,
        "customer_message",
        "2026-02-15T16:24:00.000Z",
        "Asks for an ETA on the export fix",
        "seed-kim-ev-2",
    );
    seed_customer_event(
        &conn,
        9,
        "support_conversation",
        "2026-02-14T08:31:00.000Z",
        "Export to CSV fails on large files",
        "seed-kim-ev-3",
    );
    seed_customer_event(
        &conn,
        9,
        "support_conversation",
        "2026-02-02T09:12:00.000Z",
        "Report export timeout on Growth plan",
        "seed-kim-ev-4",
    );

    conn.execute(
        "INSERT INTO customer_timeline (customer_id, event_type, event_data_json, occurred_at)
         VALUES (9, 'sla_breach', '{\"breach_type\":\"first_response\"}', '2026-02-14T08:40:00.000Z'),
                (9, 'high_effort', '{\"effort_score\":7.2}', '2026-02-09T14:41:00.000Z')",
        [],
    )
    .expect("milestones");

    let recommendation = serde_json::json!({
        "recommendation": {
            "approach": "lead with the engineering ticket number, then the workaround",
            "tone": "candid, low ceremony",
        },
        "prompt_version": "v3",
        "model": "gpt-4o-mini",
    })
    .to_string();
    conn.execute(
        "INSERT INTO client_current_signals
             (conversation_id, customer_id, signals_json, sources, generated_at, recommendation_json)
         VALUES (1, 9, '{}', 'ai + heuristics', '2026-02-16T10:20:00.000Z', ?1)",
        rusqlite::params![recommendation],
    )
    .expect("recommendation");

    (CustomersPage::new(), DbHandle::new(conn))
}

#[allow(clippy::too_many_arguments)]
fn seed_customer_row(
    conn: &rusqlite::Connection,
    id: i64,
    remote_id: i64,
    first: &str,
    last: &str,
    email: &str,
    phone: Option<&str>,
    job_title: Option<&str>,
    organization_id: Option<i64>,
    local_created_at: &str,
) {
    conn.execute(
        "INSERT INTO customers (id, remote_id, first_name, last_name, email, phone, job_title,
             organization_id, organization, created_at, local_created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, NULL, ?9, ?9)",
        rusqlite::params![
            id,
            remote_id,
            first,
            last,
            email,
            phone,
            job_title,
            organization_id,
            local_created_at
        ],
    )
    .expect("customer");
}

fn seed_customer_conversation(
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
             created_at, updated_at, last_activity_at, closed_at, first_customer_message_at,
             local_created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'email', 1, ?7, 'none', ?8, ?8, ?8,
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

fn seed_customer_event(
    conn: &rusqlite::Connection,
    customer_id: i64,
    kind: &str,
    occurred_at: &str,
    title: &str,
    dedup_key: &str,
) {
    conn.execute(
        "INSERT INTO customer_events (customer_local_id, event_kind, occurred_at, title,
             dedup_key, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?3)",
        rusqlite::params![customer_id, kind, occurred_at, title, dedup_key],
    )
    .expect("customer event");
}

fn render_customers_page(dark: bool, width: f32, name: &str) {
    let (page, db) = seeded_customers_page();
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
                // AUD-V1: the production host for standard pages —
                // CentralPanel + scroll + content column + the SP5/14
                // inset, exactly as app/mod.rs hosts them.
                egui::CentralPanel::default()
                    .frame(egui::Frame::new().fill(design::tokens(ui).background))
                    .show(ui, |ui| {
                        egui::ScrollArea::vertical()
                            .id_salt("spp-page-scroll")
                            .auto_shrink([false; 2])
                            .show(ui, |ui| {
                                ui.add_space(2.0);
                                design::content_column(ui, |ui| {
                                    egui::Frame::new()
                                        .fill(egui::Color32::TRANSPARENT)
                                        .inner_margin(egui::Margin::symmetric(
                                            design::scale::space::SP5 as i8,
                                            14,
                                        ))
                                        .show(ui, |ui| {
                                            s.page.ui(ui, &s.db, None);
                                        });
                                });
                            });
                    });
            },
            CustomersPageState {
                installed: false,
                page,
                db,
            },
        );
    h.run_ok();
    h.get_all_by_label("Search")
        .next()
        .expect("the FilterBar's search apply")
        .click();
    h.run_ok();
    h.snapshot(name);
}

#[test]
fn r3_customers_page_1440_light() {
    render_customers_page(false, 1440.0, "r3_customers_page_1440_light");
}

#[test]
fn r3_customers_page_1024_light() {
    render_customers_page(false, 1024.0, "r3_customers_page_1024_light");
}

#[test]
fn r3_customers_page_800_light() {
    render_customers_page(false, 800.0, "r3_customers_page_800_light");
}

#[test]
fn r3_customers_page_1440_dark() {
    render_customers_page(true, 1440.0, "r3_customers_page_1440_dark");
}

#[test]
fn r3_customers_page_1024_dark() {
    render_customers_page(true, 1024.0, "r3_customers_page_1024_dark");
}

#[test]
fn r3_customers_page_800_dark() {
    render_customers_page(true, 800.0, "r3_customers_page_800_dark");
}

#[test]
fn r3_customers_open_detail_light() {
    let (page, db) = seeded_customers_page();
    let mut h = Harness::builder()
        .with_size(egui::vec2(1440.0, 900.0))
        .with_theme(egui::Theme::Light)
        .build_ui_state(
            move |ui, s| {
                if !s.installed {
                    design::install(ui.ctx());
                    s.installed = true;
                }
                s.page.ui(ui, &s.db, None);
            },
            CustomersPageState {
                installed: false,
                page,
                db,
            },
        );
    h.run_ok();
    h.get_all_by_label("Search")
        .next()
        .expect("the FilterBar's search apply")
        .click();
    h.run_ok();
    h.get_all_by_label("Open")
        .next()
        .expect("the first row's open act")
        .click();
    h.run_ok();
    h.run_steps(2);
    assert!(
        h.query_by_label("close panel").is_some(),
        "the DetailPanel must be open after the row's open act"
    );
    h.snapshot("r3_customers_detail_light");
}

#[test]
fn organizations_open_detail_light() {
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
                s.page.ui(ui, &s.db, None);
            },
            OrganizationsPageState {
                installed: false,
                page,
                db,
            },
        );
    h.run_ok();
    h.get_all_by_label("Open")
        .next()
        .expect("the first row's open act")
        .click();
    h.run_ok();
    h.run_steps(2);
    assert!(
        h.query_by_label("close panel").is_some(),
        "the DetailPanel must be open after the row's open act"
    );
    h.snapshot("r3_organizations_detail_light");
}

/// AUD-P2-16 (C12): the detail panel's member rows are deep links — the
/// first row (Acme) opens its panel, and clicking a member routes to that
/// customer's profile.
struct OrganizationsPageNavState {
    installed: bool,
    page: OrganizationsPage,
    db: DbHandle,
    nav: Option<NavRequest>,
}

#[test]
fn audp2_organizations_members_clickable_light() {
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
                // Sticky capture: a later no-nav frame must not erase a
                // nav the click frame produced.
                if let Some(nav) = s.page.ui(ui, &s.db, None) {
                    s.nav = Some(nav);
                }
            },
            OrganizationsPageNavState {
                installed: false,
                page,
                db,
                nav: None,
            },
        );
    h.run_ok();
    h.get_all_by_label("Open")
        .next()
        .expect("the first row's open act")
        .click();
    h.run_ok();
    h.run_steps(2);
    assert!(
        h.query_by_label("close panel").is_some(),
        "the DetailPanel must be open after the row's open act"
    );
    assert!(
        h.query_by_label("Marta Keller").is_some(),
        "a member row must exist after Open"
    );
    h.get_by_label("Marta Keller").click();
    h.run_ok();
    assert_eq!(
        h.state().nav,
        Some(NavRequest::OpenCustomer(10)),
        "the member click opens that customer's profile"
    );
    h.run_steps(2);
    h.snapshot("audp2_organizations_members_clickable_light");
}
