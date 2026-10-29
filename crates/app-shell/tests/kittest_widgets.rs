//! RS-13 — kittest harness: per-widget/state snapshots, light + dark.
//!
//! Run under a display (Xvfb on CI-less sandboxes):
//! `xvfb-run cargo test -p supportos-plus-app-shell --features kittest-ui`
//! Regenerate the baseline PNGs with `UPDATE_SNAPSHOTS=1`.
//!
//! The snapshots under `crates/app-shell/tests/snapshots/` are the R2a drift
//! lock: any widget repaint that changes pixels fails verification until
//! reviewed against the DC (docs/audit/design-contract-v1.md §3).
#![cfg(feature = "kittest-ui")]

use eframe::egui;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use spp_shell::design::{self, BadgeTone, ButtonVariant};
use spp_shell::icons;

/// Harness state shared by the composition + interaction tests.
struct Widgets {
    installed: bool,
    tab: usize,
    applied: u32,
    drills: u32,
}

fn harness(dark: bool) -> Harness<'static, Widgets> {
    Harness::builder()
        .with_size(egui::vec2(760.0, 620.0))
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
                draw(ui, s);
            },
            Widgets {
                installed: false,
                tab: 0,
                applied: 0,
                drills: 0,
            },
        )
}

/// The full DS-C structure composition under test.
fn draw(ui: &mut egui::Ui, s: &mut Widgets) {
    // DS-C-01 PageHeader + counter pill.
    design::page_header(
        ui,
        "Customers",
        Some("Search, filter and open accounts"),
        |ui| {
            design::page_counter(ui, "4 customers");
            let _ = design::button_with_icon(
                ui,
                icons::Icon::Plus,
                "New customer",
                ButtonVariant::Primary,
            );
        },
    );

    // DS-C-09 Tabs (underline) — clicking moves the active underline.
    if let Some(i) = design::tabs(ui, &["All", "Active", "Pending", "Closed"], s.tab) {
        s.tab = i;
    }

    // DS-C-10 FilterBar — one primary apply (A-2).
    let apply = design::filter_bar(
        ui,
        |ui| {
            let _ = design::chip_with_icon(ui, true, icons::Icon::Filter, "Mine");
            let _ = design::chip(ui, false, "Unassigned");
            let _ = ui.add(
                egui::TextEdit::singleline(&mut String::new())
                    .hint_text("search conversations…")
                    .desired_width(220.0),
            );
        },
        "apply",
    );
    if apply.clicked() {
        s.applied += 1;
    }

    // DS-C-06 Badge / Chip / StatusDot — one tone system.
    ui.horizontal(|ui| {
        design::badge(ui, "Open", BadgeTone::Info);
        design::badge(ui, "Pending", BadgeTone::Warning);
        design::badge(ui, "Resolved", BadgeTone::Success);
        design::badge(ui, "Critical", BadgeTone::Destructive);
        design::badge(ui, "AI", BadgeTone::Ai);
        design::badge(ui, "Draft", BadgeTone::Neutral);
    });
    ui.horizontal(|ui| {
        design::status_dot(ui, BadgeTone::Info, false);
        design::status_dot(ui, BadgeTone::Warning, false);
        design::status_dot(ui, BadgeTone::Success, false);
        design::status_dot(ui, BadgeTone::Destructive, false);
        design::status_dot(ui, BadgeTone::Ai, false);
        design::status_dot(ui, BadgeTone::Neutral, true);
        ui.label("dots — same tone map");
    });

    // DS-C-11 Buttons — the full variant set.
    ui.horizontal(|ui| {
        let _ = design::button(ui, "Primary", ButtonVariant::Primary);
        let _ = design::button(ui, "Outline", ButtonVariant::Outline);
        let _ = design::button(ui, "Text", ButtonVariant::Text);
        let _ = design::button(ui, "Danger", ButtonVariant::Destructive);
        let _ = design::button(ui, "AI assist", ButtonVariant::Ai);
        let _ = design::button_with_icon(ui, icons::Icon::Refresh, "Sync", ButtonVariant::Outline);
    });

    // DS-C-03 StatTile · DS-C-04 MetricTile (banded + drill).
    ui.horizontal(|ui| {
        design::metric_card(ui, "New today", "12", Some("+3 vs yesterday"));
        if design::metric_tile(
            ui,
            "7",
            "sev-1 incidents",
            Some(BadgeTone::Destructive),
            false,
            None,
        )
        .clicked()
        {
            s.drills += 1;
        }
        if design::metric_tile(ui, "23", "open conversations", None, true, None).clicked() {
            s.drills += 1;
        }
    });

    // DS-C-05 DataTable — virtualized (500 rows, ~15 visible).
    design::data_table(
        ui,
        "customers",
        &["name", "email", "conversations"],
        500,
        30.0,
        |ui, i| {
            ui.label(format!("Customer {i}"));
            ui.label(format!("c{i}@example.com"));
            design::td_num(ui, &format!("{}", i % 17));
            ui.end_row();
        },
    );
}

#[test]
fn structure_widgets_snapshot_light() {
    let mut h = harness(false);
    h.run_ok();
    h.snapshot("r2a_structure_widgets_light");
}

#[test]
fn structure_widgets_snapshot_dark() {
    let mut h = harness(true);
    h.run_ok();
    h.snapshot("r2a_structure_widgets_dark");
}

#[test]
fn structure_tabs_click_moves_underline_light() {
    let mut h = harness(false);
    h.run_ok();
    h.get_by_label("Pending").click();
    h.run_ok();
    h.snapshot("r2a_tabs_pending_light");
    assert_eq!(h.state().tab, 2, "clicking the Pending tab must move state");
}

#[test]
fn structure_tabs_click_moves_underline_dark() {
    let mut h = harness(true);
    h.run_ok();
    h.get_by_label("Pending").click();
    h.run_ok();
    h.snapshot("r2a_tabs_pending_dark");
    assert_eq!(h.state().tab, 2, "clicking the Pending tab must move state");
}

// ---------------------------------------------------------------------------
// RS-14 — R2b feedback set (DS-C-14/15/16/19)
// ---------------------------------------------------------------------------

struct Feedback {
    installed: bool,
    confirmed: bool,
}

fn harness_feedback(dark: bool) -> Harness<'static, Feedback> {
    Harness::builder()
        .with_size(egui::vec2(760.0, 880.0))
        .with_theme(if dark {
            egui::Theme::Dark
        } else {
            egui::Theme::Light
        })
        .build_ui_state(
            move |ui, s| {
                if !s.installed {
                    spp_shell::design::install(ui.ctx());
                    s.installed = true;
                }
                draw_feedback(ui, s);
            },
            Feedback {
                installed: false,
                confirmed: false,
            },
        )
}

fn draw_feedback(ui: &mut egui::Ui, s: &mut Feedback) {
    use spp_shell::design::{self, AvatarSize, BadgeTone, ButtonVariant, CalloutRole};

    // DS-C-14 EmptyState: full + CTA, and the slim variant.
    if design::empty_state_cta(
        ui,
        icons::Icon::Inbox,
        "Nothing here yet",
        "Connect a channel to start receiving conversations.",
        "Connect channel",
    ) {
        s.confirmed = true;
    }
    design::empty_state_slim(
        ui,
        icons::Icon::Clock,
        "No milestones",
        "Milestones appear after the first sync.",
    );

    // DS-C-15 Callout set — the five fixed B-1 roles.
    design::callout(ui, CalloutRole::Neutral, "In-card context note.");
    design::callout(
        ui,
        CalloutRole::Warn,
        "This report uses the legacy builder.",
    );
    design::callout(ui, CalloutRole::Err, "The last sync failed 3 minutes ago.");
    design::status_line(ui, true, "Up to date · synced 12:04");
    design::status_line(ui, false, "Sync failed — retry");
    design::quirk(
        ui,
        "Data-layer quirk: freshness is stale until U2 lands (receipt-cited).",
    );

    // DS-C-19 Avatar / Assignee.
    ui.horizontal(|ui| {
        design::avatar(ui, AvatarSize::Lg, "KP", BadgeTone::Neutral);
        design::avatar(ui, AvatarSize::Md, "AI", BadgeTone::Ai);
        design::avatar(ui, AvatarSize::Md, "NT", BadgeTone::Warning);
        design::assignee(ui, Some("Kim Pearce"));
        design::assignee(ui, None);
    });

    // DS-C-16 armed danger button — two-step confirm (A-5).
    ui.horizontal(|ui| {
        if design::armed_danger_button(ui, "row-delete", "delete", "confirm delete?") {
            s.confirmed = true;
        }
        let _ = design::button(ui, "Safe action", ButtonVariant::Outline);
    });

    // DC: DS-C-13 form rows.
    design::form_row(ui, "Team name", |ui| {
        let _ = ui.add(
            egui::TextEdit::singleline(&mut String::from("Support"))
                .hint_text("team name")
                .desired_width(160.0),
        );
    });
}

#[test]
fn feedback_widgets_snapshot_light() {
    let mut h = harness_feedback(false);
    h.run_ok();
    h.snapshot("r2b_feedback_widgets_light");
}

#[test]
fn feedback_widgets_snapshot_dark() {
    let mut h = harness_feedback(true);
    h.run_ok();
    h.snapshot("r2b_feedback_widgets_dark");
}

#[test]
fn armed_confirm_requires_two_clicks_light() {
    let mut h = harness_feedback(false);
    h.run_ok();
    // First click arms — no confirm.
    h.get_by_label("delete").click();
    h.run_ok();
    h.run_steps(3); // settle past the arm frame; the danger label renders
    h.snapshot("r2b_armed_confirm_light");
    assert!(!h.state().confirmed, "first click must only arm");
    // Second click on the armed label confirms.
    h.get_by_label("confirm delete?").click();
    h.run_ok();
    h.run_steps(2);
    assert!(h.state().confirmed, "second click must confirm");
}

#[test]
fn armed_confirm_requires_two_clicks_dark() {
    let mut h = harness_feedback(true);
    h.run_ok();
    h.get_by_label("delete").click();
    h.run_ok();
    h.run_steps(3);
    h.snapshot("r2b_armed_confirm_dark");
    assert!(!h.state().confirmed);
    h.get_by_label("confirm delete?").click();
    h.run_ok();
    h.run_steps(2);
    assert!(h.state().confirmed);
}

// ---------------------------------------------------------------------------
// RS-15 — R2c inbox identity set (DS-C-20/21/22)
// ---------------------------------------------------------------------------

struct InboxIdentity {
    installed: bool,
    quoted_open: bool,
    sent: Vec<String>,
    inserted: usize,
}

fn harness_identity(dark: bool) -> Harness<'static, InboxIdentity> {
    Harness::builder()
        .with_size(egui::vec2(720.0, 900.0))
        .with_theme(if dark {
            egui::Theme::Dark
        } else {
            egui::Theme::Light
        })
        .build_ui_state(
            move |ui, s| {
                if !s.installed {
                    spp_shell::design::install(ui.ctx());
                    s.installed = true;
                }
                draw_identity(ui, s);
            },
            InboxIdentity {
                installed: false,
                quoted_open: false,
                sent: Vec::new(),
                inserted: 0,
            },
        )
}

fn draw_identity(ui: &mut egui::Ui, s: &mut InboxIdentity) {
    use spp_shell::design::{self, ComposerMode, ComposerState, MessageRole};

    design::day_divider(ui, "Today");

    design::message_card(
        ui,
        MessageRole::Customer,
        "Marta Silva",
        "10:02",
        "The export button does nothing on the dashboard.",
        |ui| {
            design::file_chip(ui, icons::Icon::Download, "export-log.txt", "24 KB");
        },
    );
    design::message_card(
        ui,
        MessageRole::Agent,
        "Kim Pearce",
        "10:05",
        "Thanks — reproducing now. Can you share the report id?",
        |_| {},
    );
    design::message_card(
        ui,
        MessageRole::Note,
        "Kim Pearce",
        "10:06",
        "Likely the v2 export path — flagged for the data track.",
        |_| {},
    );
    // DS-C-22: the AI card must identify as AI in 5 seconds — violet + AI badge.
    design::message_card(
        ui,
        MessageRole::Ai,
        "Support AI",
        "10:07",
        "Draft: this matches the known export regression fixed in v1.8.4.",
        |ui| {
            let _ = design::button(ui, "insert as reply", ButtonVariant::Outline);
        },
    );

    design::quoted_history(ui, "quoted", &mut s.quoted_open, 2, |ui| {
        design::message_card(
            ui,
            MessageRole::Customer,
            "Marta Silva",
            "Yesterday",
            "Older thread message.",
            |_| {},
        );
    });

    // DS-C-22 copilot cards.
    if design::cop_suggest(
        ui,
        "Suggested reply",
        &[
            String::from("Acknowledge the regression"),
            String::from("Point to the fix in v1.8.4"),
        ],
        &["insert", "edit"],
    )
    .is_some()
    {
        s.inserted += 1;
    }
    if design::cop_similar(
        ui,
        "Export stuck at 0%",
        "Fixed by retrying after the sync completes.",
    )
    .clicked()
    {
        s.inserted += 1;
    }
    design::cop_chat(ui, true, "I found 2 similar resolved conversations.");
    design::cop_chat(ui, false, "Show me the fix for the first one.");

    // DS-C-21 composer.
    let mut st = ComposerState::new();
    st.mode = ComposerMode::Reply;
    st.draft = String::new();
    st.draft_saved = true;
    let replies = vec![String::from("Thanks for the report — we're on it.")];
    if let Some(act) = design::composer(ui, "test", &mut st, &replies) {
        match act {
            design::ComposerAct::Send(_, text) => s.sent.push(text),
            design::ComposerAct::SavedReply(_) | design::ComposerAct::AiInsert => s.inserted += 1,
        }
    }
    ui.label(format!("sent={}", s.sent.len()));
    ui.monospace(format!("sent_text={}", s.sent.join("|")));
    ui.label(format!("inserted={}", s.inserted));
}

#[test]
fn inbox_identity_snapshot_light() {
    let mut h = harness_identity(false);
    h.run_ok();
    h.snapshot("r2c_inbox_identity_light");
}

#[test]
fn inbox_identity_snapshot_dark() {
    let mut h = harness_identity(true);
    h.run_ok();
    h.snapshot("r2c_inbox_identity_dark");
}

struct ComposerOnly {
    installed: bool,
    sent: Vec<String>,
    inserted: usize,
}

fn harness_composer(dark: bool) -> Harness<'static, ComposerOnly> {
    Harness::builder()
        .with_size(egui::vec2(720.0, 520.0))
        .with_theme(if dark {
            egui::Theme::Dark
        } else {
            egui::Theme::Light
        })
        .build_ui_state(
            move |ui, s| {
                if !s.installed {
                    spp_shell::design::install(ui.ctx());
                    s.installed = true;
                }
                use spp_shell::design::{ComposerMode, ComposerState};
                let mut st = ComposerState::new();
                st.mode = ComposerMode::Reply;
                if s.sent.is_empty() {
                    st.draft = String::from("draft text");
                }
                st.draft_saved = true;
                let replies = vec![String::from("Thanks for the report — we're on it.")];
                if let Some(act) = spp_shell::design::composer(ui, "test", &mut st, &replies) {
                    match act {
                        spp_shell::design::ComposerAct::Send(_, text) => s.sent.push(text),
                        _ => s.inserted += 1,
                    }
                }
                ui.monospace(format!("sent_text={}", s.sent.join("|")));
            },
            ComposerOnly {
                installed: false,
                sent: Vec::new(),
                inserted: 0,
            },
        )
}

#[test]
fn composer_snapshot_light() {
    let mut h = harness_composer(false);
    h.run_ok();
    h.snapshot("r2c_composer_light");
}

#[test]
fn composer_snapshot_dark() {
    let mut h = harness_composer(true);
    h.run_ok();
    h.snapshot("r2c_composer_dark");
}

#[test]
fn composer_send_button_and_shortcut_light() {
    let mut h = harness_composer(false);
    h.run_ok();
    // The send button sends the draft.
    h.get_by_label("send").click();
    h.run_ok();
    h.run_steps(2);
    assert_eq!(
        h.state().sent.len(),
        1,
        "send button must emit ComposerAct::Send"
    );
    assert_eq!(h.state().sent[0], "draft text");
    // Ctrl+Enter sends again (shortcut path).
    h.key_combination_modifiers(egui::Modifiers::CTRL, &[egui::Key::Enter]);
    h.run_ok();
    h.run_steps(2);
    assert_eq!(h.state().sent.len(), 2, "Ctrl+Enter must send");
}

#[test]
fn ai_message_card_is_identifiable() {
    // The 5-second pre-check: the AI card exposes an "AI" badge node and the
    // role label, isolated from any other context.
    let mut h = harness_identity(false);
    h.run_ok();
    let ai_nodes = h.get_all_by_label_contains("AI").count();
    assert!(ai_nodes >= 2, "AI badge + role label must both exist");
}
