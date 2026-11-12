#![cfg(feature = "kittest-ui")]

use eframe::egui;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use spp_core::db::DbHandle;
use spp_shell::design;
use spp_shell::pages::issue_radar::IssueRadarPage;

struct IssueRadarPageState {
    installed: bool,
    page: IssueRadarPage,
    db: DbHandle,
}

fn seeded_page() -> (IssueRadarPage, DbHandle) {
    let mut conn = rusqlite::Connection::open_in_memory().expect("in-memory db");
    spp_core::bootstrap::apply_all(&mut conn).expect("bootstrap");
    seed_mailbox(&conn);
    seed_customer(&conn, 9, 909, "Kim", "Pearce", "kim@example.test");
    seed_customer(&conn, 11, 911, "Nabin", "Sayeed", "nabin@example.test");

    seed_conversation(
        &conn,
        1,
        501,
        "Export to CSV fails on large files",
        "active",
        9,
        "datetime('now', '-7 days')",
    );
    seed_conversation(
        &conn,
        2,
        505,
        "Refund for duplicate charge",
        "active",
        11,
        "datetime('now', '-7 days')",
    );
    seed_conversation(
        &conn,
        3,
        5033,
        "How do I bulk rename workspaces",
        "active",
        9,
        "datetime('now', '-7 days')",
    );
    seed_conversation(
        &conn,
        4,
        5021,
        "Export broken for CSV files",
        "closed",
        11,
        "datetime('now', '-7 days')",
    );
    seed_conversation(
        &conn,
        5,
        5040,
        "SSO redirect loop",
        "active",
        9,
        "datetime('now', '-7 days')",
    );
    seed_conversation(
        &conn,
        6,
        5061,
        "Cannot sign in after reset",
        "closed",
        11,
        "datetime('now', '-21 days')",
    );
    seed_conversation(
        &conn,
        7,
        5062,
        "Login loop on mobile",
        "closed",
        9,
        "datetime('now', '-21 days')",
    );

    let _ = spp_core::sla::sla_alerts(&conn);
    conn.execute(
        "INSERT INTO mailbox_business_hours (mailbox_local_id, timezone, days, start_minute,
             end_minute, first_response_target_min, resolution_target_min, updated_at)
         VALUES (1, 'UTC', '[1,2,3,4,5,6,7]', 0, 1440, 1440, 4320,
                 '2026-01-12T00:00:00.000Z')",
        [],
    )
    .expect("business hours");
    for (conv, at) in [
        (1, "datetime('now', '-7 days')"),
        (2, "datetime('now', '-7 days')"),
        (3, "datetime('now', '-7 days')"),
        (5, "datetime('now', '-7 days')"),
    ] {
        conn.execute(
            &format!(
                "INSERT INTO conversation_threads (conversation_id, type, body_text, from_type,
                     created_by_customer_id, created_at, state, deleted_at)
                 VALUES (?1, 'customer', 'still broken', 'customer', NULL, {at}, 'published', NULL)"
            ),
            rusqlite::params![conv],
        )
        .expect("customer thread");
    }

    conn.execute(
        "INSERT INTO issue_clusters (name, title, summary, category, trend,
             conversation_count, customer_count, first_seen_at, last_seen_at)
         VALUES ('Login loop', 'Login loop',
             'Users stuck at SSO redirect after the 2.1 deploy', 'auth', 'rising',
             5, 2, datetime('now', '-21 days'), datetime('now', '-7 days')),
            ('Export failures', 'Export failures', NULL, 'export', 'new',
             2, 2, datetime('now', '-7 days'), datetime('now', '-7 days'))",
        [],
    )
    .expect("clusters");
    for (cluster, conv) in [(1, 5), (1, 3), (1, 2), (1, 6), (1, 7), (2, 4), (2, 1)] {
        conn.execute(
            "INSERT INTO issue_cluster_conversations (cluster_id, conversation_id) VALUES (?1, ?2)",
            rusqlite::params![cluster, conv],
        )
        .expect("cluster link");
    }

    conn.execute(
        "INSERT INTO known_issues (name, status, title, symptoms, workaround, provenance,
             created_at, last_seen_at)
         VALUES ('Login loop', 'identified', 'Login loop',
             'SSO redirect loop after password change', 'Clear cookies', 'human_local',
             datetime('now', '-21 days'), datetime('now', '-7 days')),
            ('Export failures', 'monitoring', 'Export failures',
             'CSV export fails on files over 10 MB', NULL, 'human_local',
             datetime('now', '-7 days'), datetime('now', '-7 days'))",
        [],
    )
    .expect("known issues");
    for (issue, conv) in [(1, 5), (1, 3), (1, 2)] {
        conn.execute(
            "INSERT INTO known_issue_links (known_issue_id, conversation_id, link_type)
             VALUES (?1, ?2, 'human')",
            rusqlite::params![issue, conv],
        )
        .expect("known link");
    }
    spp_core::intelligence_features::refresh_known_issue_counts(&conn, 1).expect("refresh counts");

    conn.execute(
        "INSERT INTO knowledge_gap_candidates (kind, query_text, occurrence_count,
             evidence_conversation_ids, related_document_ids, detail, status,
             created_at, updated_at)
         VALUES ('repeated_question_uncovered', 'how do I bulk rename workspaces', 7,
             '[1,3]', '[]',
             '{\"explanation\":\"Asked 7 times across reopened conversations; no document mentions bulk rename.\"}',
             'open', '2026-01-12T07:00:00.000Z', '2026-01-12T09:00:00.000Z'),
            ('repeated_question_unsolved', 'webhook relay returns 401', 4,
             '[2]', '[]',
             '{\"explanation\":\"Docs cover the relay setup but not the 401 signature case.\"}',
             'approved', '2026-01-12T06:00:00.000Z', '2026-01-12T08:00:00.000Z'),
            ('repeated_question_uncovered', 'delete a mailbox', 1,
             '[]', '[]', '{}',
             'rejected', '2026-01-12T05:00:00.000Z', '2026-01-12T07:00:00.000Z')",
        [],
    )
    .expect("gap candidates");

    (IssueRadarPage::new(), DbHandle::new(conn))
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
    at_sql: &str,
) {
    let preview = format!("{subject} preview");
    conn.execute(
        &format!(
            "INSERT INTO conversations (id, remote_id, number, subject, preview, status, type,
                 mailbox_local_id, customer_local_id, priority,
                 created_at, updated_at, closed_at, first_customer_message_at, local_created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'email', 1, ?7, 'none', {at_sql}, {at_sql},
                     CASE WHEN ?6 = 'closed' THEN {at_sql} ELSE NULL END, {at_sql}, {at_sql})"
        ),
        rusqlite::params![id, 1000 + id, number, subject, preview, status, customer_id,],
    )
    .expect("conversation");
}

fn render_issue_radar_page(dark: bool, width: f32, name: &str) {
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
            IssueRadarPageState {
                installed: false,
                page,
                db,
            },
        );
    h.run_ok();
    h.snapshot(name);
}

#[test]
fn r3_issue_radar_page_1440_light() {
    render_issue_radar_page(false, 1440.0, "r3_issue_radar_page_1440_light");
}

#[test]
fn r3_issue_radar_page_1024_light() {
    render_issue_radar_page(false, 1024.0, "r3_issue_radar_page_1024_light");
}

#[test]
fn r3_issue_radar_page_800_light() {
    render_issue_radar_page(false, 800.0, "r3_issue_radar_page_800_light");
}

#[test]
fn r3_issue_radar_page_1440_dark() {
    render_issue_radar_page(true, 1440.0, "r3_issue_radar_page_1440_dark");
}

#[test]
fn r3_issue_radar_page_1024_dark() {
    render_issue_radar_page(true, 1024.0, "r3_issue_radar_page_1024_dark");
}

#[test]
fn r3_issue_radar_page_800_dark() {
    render_issue_radar_page(true, 800.0, "r3_issue_radar_page_800_dark");
}

#[test]
fn r3_issue_radar_gaps_tab_light() {
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
            IssueRadarPageState {
                installed: false,
                page,
                db,
            },
        );
    h.run_ok();
    h.get_all_by_label("Gaps")
        .next()
        .expect("the Gaps tab")
        .click();
    h.run_ok();
    h.run_steps(2);
    assert!(
        h.query_by_label("Approve").is_some(),
        "the Gaps tab must show the candidates' approve act"
    );
    h.snapshot("r3_issue_radar_gaps_tab_light");
}
