#![cfg(feature = "kittest-ui")]

use eframe::egui;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use spp_core::db::DbHandle;
use spp_shell::design;
use spp_shell::pages::ai_center::AiCenterPage;

struct AiCenterPageState {
    installed: bool,
    page: AiCenterPage,
    db: DbHandle,
}

fn seeded_page() -> (AiCenterPage, DbHandle) {
    let mut conn = rusqlite::Connection::open_in_memory().expect("in-memory db");
    spp_core::bootstrap::apply_all(&mut conn).expect("bootstrap");
    (AiCenterPage::new(), DbHandle::new(conn))
}

/// UXF-1-7: a page seeded like the owner's screenshot report — failed
/// golden_eval runs in the Jobs table and a 0/9 golden-set run whose
/// cases all died on the LM Studio HTTP 400.
fn seeded_failures_page() -> (AiCenterPage, DbHandle) {
    let (page, db) = seeded_page();
    db.with_write(|conn| {
        for i in 0..12i64 {
            conn.execute(
                "INSERT INTO ai_runs (input_hash, prompt_version, model, response_json, type, status, conversation_id, created_at)
                 VALUES (?1, 'v1', 'gemma-4-e4b', '{}', 'golden_eval', 'failed', NULL, ?2)",
                rusqlite::params![
                    format!("run-hash-{i}"),
                    format!("2026-11-05T23:46:{i:02}Z")
                ],
            )
            .expect("seed ai_run");
        }
        let error = "config error: LM Studio chat request failed (HTTP 400 Bad Request): \
                     {\"error\":\"response_format.type' must be 'json_schema' or 'text'\"}";
        let results: Vec<serde_json::Value> = [
            "simple question",
            "multi-question ticket",
            "ambiguous ticket",
            "known issue",
            "customer history",
            "timezone issue",
            "integration issue",
            "billing question",
            "internal escalation",
        ]
        .iter()
        .map(|name| {
            serde_json::json!({
                "name": name,
                "category": "classification",
                "passed": false,
                "latency_ms": 8,
                "error": error,
            })
        })
        .collect();
        // `started_at` is pinned explicitly: the column DEFAULTs to
        // wall-clock `now`, and the Usage tab renders the raw stamp —
        // an unset value drifts the golden text run-to-run (the exact
        // nondeterminism class UXB-T1 closed for the other suites).
        conn.execute(
            "INSERT INTO golden_test_runs (backend, passed, failed, results, started_at)
             VALUES (?1, 0, 9, ?2, '2026-11-05T23:52:07.412Z')",
            rusqlite::params![
                "gemma-4-e4b",
                serde_json::to_string(&results).expect("results json")
            ],
        )
        .expect("seed golden run");
        Ok::<(), rusqlite::Error>(())
    })
    .expect("seed");
    (page, db)
}

fn render_ai_center_page(dark: bool, width: f32, name: &str) {
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
            AiCenterPageState {
                installed: false,
                page,
                db,
            },
        );
    h.run_ok();
    h.snapshot(name);
}

#[test]
fn r3_ai_center_page_1440_light() {
    render_ai_center_page(false, 1440.0, "r3_ai_center_page_1440_light");
}

#[test]
fn r3_ai_center_page_1024_light() {
    render_ai_center_page(false, 1024.0, "r3_ai_center_page_1024_light");
}

#[test]
fn r3_ai_center_page_800_light() {
    render_ai_center_page(false, 800.0, "r3_ai_center_page_800_light");
}

#[test]
fn r3_ai_center_page_1440_dark() {
    render_ai_center_page(true, 1440.0, "r3_ai_center_page_1440_dark");
}

#[test]
fn r3_ai_center_page_1024_dark() {
    render_ai_center_page(true, 1024.0, "r3_ai_center_page_1024_dark");
}

#[test]
fn r3_ai_center_page_800_dark() {
    render_ai_center_page(true, 800.0, "r3_ai_center_page_800_dark");
}

#[test]
fn r3_ai_center_usage_tab_light() {
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
            AiCenterPageState {
                installed: false,
                page,
                db,
            },
        );
    h.run_ok();
    h.get_all_by_label("Usage")
        .next()
        .expect("the Usage tab")
        .click();
    h.run_ok();
    h.run_steps(2);
    assert!(
        h.get_all_by_label("Run golden set now").next().is_some(),
        "the Usage tab must show the golden-set act"
    );
    h.snapshot("r3_ai_center_usage_tab_light");
}

/// UXF-1-7: the seeded Jobs table (full-width card + data_table) —
/// locked at a wide and a narrow viewport so the width discipline is
/// diff-locked forever.
#[test]
fn r3_ai_center_jobs_tab_seeded_1440_light() {
    render_seeded_tab("Jobs", 1440.0, "r3_ai_center_jobs_tab_seeded_1440_light");
}

#[test]
fn r3_ai_center_jobs_tab_seeded_800_light() {
    render_seeded_tab("Jobs", 800.0, "r3_ai_center_jobs_tab_seeded_800_light");
}

/// UXF-1-7: the Usage tab with a 0/9 failing golden run — the contained
/// run block with the grouped mono error line (the owner-reported
/// 27-line dump state, now collapsed) is diff-locked.
#[test]
fn r3_ai_center_usage_tab_failing_run_1440_light() {
    render_seeded_tab(
        "Usage",
        1440.0,
        "r3_ai_center_usage_tab_failing_run_1440_light",
    );
}

#[test]
fn r3_ai_center_usage_tab_failing_run_800_light() {
    render_seeded_tab(
        "Usage",
        800.0,
        "r3_ai_center_usage_tab_failing_run_800_light",
    );
}

fn render_seeded_tab(tab: &str, width: f32, name: &str) {
    let (page, db) = seeded_failures_page();
    let mut h = Harness::builder()
        .with_size(egui::vec2(width, 900.0))
        .with_theme(egui::Theme::Light)
        .build_ui_state(
            move |ui, s| {
                if !s.installed {
                    design::install(ui.ctx());
                    s.installed = true;
                }
                s.page.ui(ui, &s.db);
            },
            AiCenterPageState {
                installed: false,
                page,
                db,
            },
        );
    h.run_ok();
    h.get_all_by_label(tab)
        .next()
        .unwrap_or_else(|| panic!("the {tab} tab"))
        .click();
    h.run_ok();
    h.run_steps(2);
    h.snapshot(name);
}

/// AUD-P1-6 (C7): the price-editor state — a custom (non-builtin)
/// endpoint carries the $/mtok line with the "Edit Prices" act; opening
/// it shows the inline prompt/completion pair with Save/Cancel. The
/// builtin endpoints (the base goldens) have no price line — prices are
/// a remote-endpoint concept.
#[test]
fn audp1_ai_center_price_editor_light() {
    let (page, db) = seeded_page();
    db.with_write(|conn| {
        spp_core::ai_hub::save_endpoints(
            conn,
            &[spp_core::ai_hub::EndpointConfig {
                id: "groq-remote".into(),
                name: "Groq (remote)".into(),
                base_url: "https://api.groq.example.test/openai/v1".into(),
                enabled: true,
                price_prompt_per_mtok: 0.59,
                price_completion_per_mtok: 0.79,
                builtin: false,
            }],
        )
        .expect("custom endpoint");
        Ok::<(), spp_core::Error>(())
    })
    .expect("seed");
    // tall harness: the endpoints card sits below the Health tab's upper
    // cards — the Edit Prices act must be inside the screen rect for the
    // click to land (a found-but-below-the-fold widget ignores clicks).
    let mut h = Harness::builder()
        .with_size(egui::vec2(1440.0, 2400.0))
        .with_theme(egui::Theme::Light)
        .build_ui_state(
            move |ui, s| {
                if !s.installed {
                    design::install(ui.ctx());
                    s.installed = true;
                }
                s.page.ui(ui, &s.db);
            },
            AiCenterPageState {
                installed: false,
                page,
                db,
            },
        );
    h.run_ok();
    h.get_all_by_label("Edit Prices")
        .next()
        .expect("the custom endpoint's Edit Prices act")
        .click();
    h.run_ok();
    h.run_steps(2);
    assert!(
        h.get_all_by_label("Save Prices").next().is_some(),
        "the inline price editor must carry its save act"
    );
    h.snapshot("audp1_ai_center_price_editor_light");
}
