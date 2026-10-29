//! RS-18 — DS-A-01 incidents goldens (wave B pilot).
//!
//! The lab reference lives at `mockups/design-lab/goldens/v4/{light,dark}/incidents-*.png`;
//! egui renders will not pixel-match that HTML lab — these goldens lock the
//! EGUI page state (the real page: PageHeader with the counter pill, the
//! DS-C-10 FilterBar, the DS-C-05 DataTable, and the DS-C-17 DetailPanel
//! with its DS-A-03 record composition over a seeded database); the
//! side-by-side review against the lab is the G3 evidence pack.
//!
//! Determinism note: the seed helpers below write **fixed timestamps** —
//! the incidents themselves are created through the real
//! `create_manual_incident` write, then the store-level fields the create
//! route does not fill (started_at, owner, updated_at ordering, notes and
//! releases rows) are pinned with UPDATEs/INSERTs so nothing can drift
//! with wall-clock time. The list sorts non-resolved first by updated_at
//! DESC, so the pinned updated_at values fix the render order (INC-001,
//! INC-002, INC-003).
//!
//! Run under a display (Xvfb on display :99 in this lab):
//! `cargo test -p supportos-plus-app-shell --features kittest-ui --test kittest_incidents_page`
//! Regenerate the baselines with `UPDATE_SNAPSHOTS=1`; a clean re-run must
//! then pass byte-identical (the repo's diff-locked-twice rule).
#![cfg(feature = "kittest-ui")]

use eframe::egui;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use spp_core::db::DbHandle;
use spp_core::intelligence_features::ManualIncident;
use spp_shell::design;
use spp_shell::pages::incidents::IncidentsPage;

/// Harness state: the REAL page + the seeded database it reads.
struct IncidentsPageState {
    installed: bool,
    page: IncidentsPage,
    db: DbHandle,
}

// ---------------------------------------------------------------------------
// Deterministic fixture (fixed timestamps; mirrors the lab's incident fixture)
// ---------------------------------------------------------------------------

/// Bootstrap an in-memory database (all migrations) and seed the incidents
/// fixture into it. The three incidents mirror the conformed lab fixture
/// (`mockups/design-lab/data.js` → `PAGES.incidents.list`): INC checkout
/// outage (sev1, investigating, two linked conversations), SSO redirect
/// loop (sev2, identified) and CSV export failures (sev3, monitoring). The
/// detail row pins the lab's started/owner/description/customer-safe/
/// notes/releases values.
fn seeded_page() -> (IncidentsPage, DbHandle) {
    let mut conn = rusqlite::Connection::open_in_memory().expect("in-memory db");
    spp_core::bootstrap::apply_all(&mut conn).expect("bootstrap");
    seed_mailbox(&conn);
    seed_user(&conn, 5, 500, "Nabin", "Sayeed");
    seed_customer(&conn, 9, 909, "Kim", "Pearce", "kim@example.test");
    seed_customer(&conn, 11, 911, "Nabin", "Sayeed", "nabin@example.test");

    // The two linked conversations (the lab's linked card: #5021 export,
    // #5019 card decline) ride customers 9 / 11 so the affected-customers
    // card lists Kim Pearce and Nabin Sayeed.
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

    // Pin the store-level fields the create route does not fill: the
    // detail's started/owner line and the list ordering (non-resolved
    // first, updated_at DESC — so INC-001 must be the most recently
    // updated of the three).
    conn.execute(
        "UPDATE incidents SET started_at = '2026-01-12 08:41:03',
             owner_user_local_id = 5,
             updated_at = CASE id WHEN 1 THEN '2026-01-12T11:00:00.000Z'
                                  WHEN 2 THEN '2026-01-12T09:00:00.000Z'
                                  ELSE '2026-01-12T08:00:00.000Z' END",
        [],
    )
    .expect("pin started/owner/updated");

    // The lab's note and release rows, with fixed created_at stamps.
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

/// One conversation row with fixed timestamps (created == updated == `at`);
/// the same columns the sync pipeline fills.
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

/// The A-1 flow: a row's `open` act opens the DS-C-17 DetailPanel with the
/// DS-A-03 record composition (rows sort non-resolved first by updated_at
/// DESC, so the first `open` is INC-001 — the checkout outage with the two
/// linked conversations, the note and the releases). Snapshot the open
/// panel.
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
    h.snapshot("r3_incidents_detail_light");
}
