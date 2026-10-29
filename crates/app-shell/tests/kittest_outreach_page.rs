//! RS-19 — DS outreach goldens (wave C).
//!
//! The lab reference lives at `mockups/design-lab/goldens/v4/{light,dark}/outreach-*.png`;
//! egui renders will not pixel-match that HTML lab — these goldens lock the
//! EGUI page state (the real `OutreachPage`: the DS-C-09 tab rail, the
//! campaign cards with the lab's chip + raw-status badge head and the
//! split action rows, the selected campaign's report card with the lab's
//! warn-callout receipt, the Segments tab's card anatomy with the criteria
//! JSON lines, and the New Campaign form) over a seeded mirror; the
//! side-by-side review against the lab is the G3 evidence pack.
//!
//! The page opens on the Campaigns tab with nothing selected (HEAD
//! behavior — the report card appears when the user picks a campaign), so
//! the goldens click the first campaign chip through the a11y tree before
//! locking the populated page, exactly what a user's first visit does
//! (the customers precedent). The two interaction goldens switch to the
//! Segments tab (the criteria-line presentation fix) and open the New
//! Campaign form (the toolbar act, its labelled_by bindings under the
//! a11y check).
//!
//! Run under a display (Xvfb on display :99 in this lab):
//! `cargo test -p supportos-plus-app-shell --features kittest-ui --test kittest_outreach_page`
//! Regenerate the baselines with `UPDATE_SNAPSHOTS=1`; a clean re-run must
//! then pass byte-identical (the repo's diff-locked-twice rule).
#![cfg(feature = "kittest-ui")]

use eframe::egui;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use spp_core::db::DbHandle;
use spp_shell::design;
use spp_shell::pages::outreach::OutreachPage;

/// Harness state: the REAL page + the seeded database it reads.
struct OutreachPageState {
    installed: bool,
    page: OutreachPage,
    db: DbHandle,
}

const TICKET_CUSTOMERS_DEFINITION: &str = r#"{"combinator":"all","conditions":[{"kind":"history","metric":"ticket_count","op":"gte","value":1}],"exclude":[]}"#;
const CHURN_RISK_DEFINITION: &str = r#"{"combinator":"any","conditions":[{"kind":"rating","metric":"not_good_90d","op":"gte","value":1}],"exclude":[]}"#;

/// Bootstrap an in-memory database and seed the outreach fixture: a
/// Support mailbox, two customers with one ticket each (so the saved
/// "Ticket owners" segment matches both), the lab's two segments (Ticket
/// owners + Churn risk, the criteria JSON lines), two campaigns — the
/// lab's "Check-in — How is it going?" draft (recipients snapshotted from
/// the real segment engine preview) and a completed "Spring push —
/// Feature launch" — plus two DNC rows (the lab's asked-politely +
/// manual reasons).
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

    // The lab's two segments (the Segments tab's criteria lines).
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

    // The draft campaign, recipients snapshotted from the real segment
    // engine preview (the same path the page's create form runs).
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

    // The lab's second campaign rides the completed status (the success
    // badge in the card head).
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

    // The lab's DNC rows (the Do-not-contact tab's list).
    spp_core::outreach::add_to_dnc(&conn, 9, "asked politely").expect("dnc 9");
    spp_core::outreach::add_to_dnc(&conn, 10, "manual").expect("dnc 10");

    (OutreachPage::new(), DbHandle::new(conn))
}

// ---------------------------------------------------------------------------
// The six goldens + the Segments-tab and New-Campaign interactions
// ---------------------------------------------------------------------------

/// Render the real page once at `width` × 900 in `dark`/light, pick the
/// first campaign (the user's first act; the lab's `sel: 0` state — the
/// report card appears) and lock the populated page as `{name}.png` under
/// `tests/snapshots/`.
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
                s.page.ui(ui, &s.db);
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

/// The Segments tab: the lab's card anatomy — strong id — name with the
/// danger delete pushed right and the criteria JSON as the mono line
/// beneath. Snapshot the tab state.
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
                s.page.ui(ui, &s.db);
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
    h.run_steps(2); // settle the tab frame
    assert!(
        h.get_all_by_label("delete").next().is_some(),
        "the segment cards must carry their delete acts"
    );
    h.snapshot("r3_outreach_segments_tab_light");
}

/// The toolbar act: clicking `New Campaign` opens the create form (name /
/// subject / body / mailbox / segment inputs, each bound to its visible
/// label — the a11y check validates the bindings) with the saved-segments
/// hint and the create act. Snapshot the form state.
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
                s.page.ui(ui, &s.db);
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
    h.run_steps(2); // settle the form frame
    assert!(
        h.get_all_by_label("create campaign").next().is_some(),
        "the New Campaign form must show the create act"
    );
    h.snapshot("r3_outreach_new_campaign_light");
}
