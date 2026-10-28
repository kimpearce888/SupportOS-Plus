//! RS-17 — DS-A-01 list-page goldens (organizations pilot + customers).
//!
//! The lab references live at `mockups/design-lab/goldens/v4/{light,dark}/{organizations,customers}-*.png`;
//! egui renders will not pixel-match that HTML lab — these goldens lock the
//! EGUI page state (the real pages: PageHeader with the counter pill,
//! FilterBar, DataTable, and the DS-C-17 DetailPanel with its DS-A-03
//! record composition over a seeded database); the side-by-side review
//! against the lab is the G3 evidence pack.
//!
//! The customers half drives the page's real search flow first: the page
//! opens on its empty state (HEAD behavior — the list populates when the
//! user applies the search), so the goldens click the FilterBar's primary
//! `search` apply through the a11y tree before locking the populated
//! table, exactly what a user's empty-box search ("show all") does.
//!
//! Determinism note: the seed helpers below write **fixed timestamps** —
//! the organizations page renders names, domains and counts only, but the
//! conversation seeds pin `created_at`/`updated_at` anyway so nothing can
//! drift with wall-clock time. The customers seeds pin every sort key the
//! page reads (`last_activity_at`, `local_created_at`, event and
//! milestone timestamps) for the same reason. (`spp_shell::testsupport`
//! is `#[cfg(test)]`-gated and not visible to integration tests anyway.)
//!
//! Run under a display (Xvfb on display :99 in this lab):
//! `cargo test -p supportos-plus-app-shell --features kittest-ui --test kittest_list_pages`
//! Regenerate the baselines with `UPDATE_SNAPSHOTS=1`; a clean re-run must
//! then pass byte-identical (the repo's diff-locked-twice rule).
#![cfg(feature = "kittest-ui")]

use eframe::egui;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use spp_core::db::DbHandle;
use spp_shell::design;
use spp_shell::pages::customers::CustomersPage;
use spp_shell::pages::organizations::OrganizationsPage;

/// Harness state: the REAL page + the seeded database it reads.
struct OrganizationsPageState {
    installed: bool,
    page: OrganizationsPage,
    db: DbHandle,
}

// ---------------------------------------------------------------------------
// Deterministic fixture (fixed timestamps; mirrors the lab's org fixture)
// ---------------------------------------------------------------------------

/// Bootstrap an in-memory database (all migrations) and seed the
/// organizations fixture into it. The three organizations mirror the
/// conformed lab fixture (`mockups/design-lab/data.js` → `PAGES.organizations`):
/// Northwind Logistics, Acme (two members), Globex (none).
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

    // Members: customers linked by `organization_id` (the same edge the
    // detail SQL reads). Fixed ids/remote_ids keep the render stable.
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

    // Conversations ride the customer→organization edge too: they feed the
    // org's conversation counts (Kim 3, Marta 2, John 1).
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

/// One conversation row with fixed timestamps (created == updated == `at`);
/// same columns the sync pipeline fills, so the page exercises its real
/// count queries.
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

// ---------------------------------------------------------------------------
// The six goldens + the DetailPanel interaction
// ---------------------------------------------------------------------------

/// Render the real page once at `width` × 900 in `dark`/light and lock it as
/// `{name}.png` under `tests/snapshots/`.
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
                s.page.ui(ui, &s.db, None);
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

// ---------------------------------------------------------------------------
// Customers (DS-A-01): fixture + the six goldens + the DetailPanel interaction
// ---------------------------------------------------------------------------

/// Harness state: the REAL page + the seeded database it reads.
struct CustomersPageState {
    installed: bool,
    page: CustomersPage,
    db: DbHandle,
}

/// Bootstrap an in-memory database (all migrations) and seed the customers
/// fixture into it. The four customers mirror the conformed lab fixture
/// (`mockups/design-lab/data.js` → `PAGES.customers.results`): Kim Pearce
/// (three conversations, customer events, support milestones and the AI
/// recommendation the detail panel reads), Ada Customer (two), Nabin
/// Sayeed (one) and Marta Keller (none). The list sorts by most recent
/// activity, so the seeded timestamps fix the render order (Kim, Ada,
/// Nabin, Marta).
fn seeded_customers_page() -> (CustomersPage, DbHandle) {
    let mut conn = rusqlite::Connection::open_in_memory().expect("in-memory db");
    spp_core::bootstrap::apply_all(&mut conn).expect("bootstrap");
    seed_mailbox(&conn);
    // The organization edge feeds the record head's company meta (the
    // detail SQL resolves organization_id → organizations.name).
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

    // Kim's conversations are the lab's profile rows (501 active, 503
    // active, 505 pending); Ada two, Nabin one, Marta none. The rows pin
    // `last_activity_at` too — it is the list's sort key AND the
    // last-contact column.
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

    // Kim's customer events (the timeline card reads the latest 100).
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

    // Kim's support milestones (item 56: the intelligence sweep's rows —
    // the lab's breach: first_response and effort score 7.20). The detail
    // parser reads breach_type / effort_score out of the payload.
    conn.execute(
        "INSERT INTO customer_timeline (customer_id, event_type, event_data_json, occurred_at)
         VALUES (9, 'sla_breach', '{\"breach_type\":\"first_response\"}', '2026-02-14T08:40:00.000Z'),
                (9, 'high_effort', '{\"effort_score\":7.2}', '2026-02-09T14:41:00.000Z')",
        [],
    )
    .expect("milestones");

    // Kim's AI-stage recommendation (the M21 client_current_signals read:
    // the stage badge, the Support approach block and the provenance).
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

/// The customers variant of `seed_customer`: phone, job title, the org
/// edge and the `local_created_at` sort key all pinned.
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

/// One conversation row with fixed timestamps, including
/// `last_activity_at` (the customers list sort key and last-contact
/// column; created == updated == last activity == `at`).
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

/// One customer event row (the timeline card's source); `created_at`
/// rides `occurred_at` so nothing drifts with wall-clock time.
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

/// Render the real page at `width` × 900 in `dark`/light and lock it as
/// `{name}.png` under `tests/snapshots/`. The page opens on its empty
/// state (HEAD behavior — the list populates when the search is applied),
/// so the golden clicks the FilterBar's primary `search` apply through
/// the a11y tree first (the user's empty-box "show all" search).
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
                s.page.ui(ui, &s.db, None);
            },
            CustomersPageState {
                installed: false,
                page,
                db,
            },
        );
    h.run_ok();
    h.get_all_by_label("search")
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

/// The A-1 flow: the search populates the table, and a row's `open` act
/// opens the DS-C-17 DetailPanel with the DS-A-03 record composition
/// (rows sort by most recent activity, so the first `open` is Kim Pearce
/// — three conversations, events, milestones and the AI recommendation).
/// Snapshot the open panel.
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
    h.get_all_by_label("search")
        .next()
        .expect("the FilterBar's search apply")
        .click();
    h.run_ok();
    // Click the first row's open act through the accessibility tree.
    h.get_all_by_label("open")
        .next()
        .expect("the first row's open act")
        .click();
    h.run_ok();
    h.run_steps(2); // settle the panel frame
    // The panel is open: its close button is in the tree, and
    // the a11y check (still ON) validates the panel's controls.
    assert!(
        h.query_by_label("close panel").is_some(),
        "the DetailPanel must be open after the row's open act"
    );
    h.snapshot("r3_customers_detail_light");
}

/// The A-1 flow: a row's `open` act opens the DS-C-17 DetailPanel with the
/// DS-A-03 record composition (rows sort by name, so the first `open` is
/// Acme — two members, three conversations). Snapshot the open panel.
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
    // Click the first row's open act through the accessibility tree.
    h.get_all_by_label("open")
        .next()
        .expect("the first row's open act")
        .click();
    h.run_ok();
    h.run_steps(2); // settle the panel frame
    // The panel is open: its close button is in the tree, and
    // the a11y check (still ON) validates the panel's controls.
    assert!(
        h.query_by_label("close panel").is_some(),
        "the DetailPanel must be open after the row's open act"
    );
    h.snapshot("r3_organizations_detail_light");
}
