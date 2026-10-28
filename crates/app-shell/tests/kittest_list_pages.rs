//! RS-17 — DS-A-01 list-page goldens (organizations pilot; customers follows).
//!
//! The lab reference lives at `mockups/design-lab/goldens/v4/{light,dark}/organizations-*.png`;
//! egui renders will not pixel-match that HTML lab — these goldens lock the
//! EGUI page state (the real `OrganizationsPage`: PageHeader with the counter
//! pill, FilterBar, DataTable, the page-flow create card and the DS-C-17
//! DetailPanel with its DS-A-03 record composition over a seeded database);
//! the side-by-side review against the lab is the G3 evidence pack.
//!
//! Determinism note: the seed helpers below write **fixed timestamps** —
//! the organizations page renders names, domains and counts only, but the
//! conversation seeds pin `created_at`/`updated_at` anyway so nothing can
//! drift with wall-clock time. (`spp_shell::testsupport` is
//! `#[cfg(test)]`-gated and not visible to integration tests anyway.)
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
