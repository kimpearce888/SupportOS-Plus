#![cfg(feature = "kittest-ui")]

use eframe::egui;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use spp_shell::design::{self, BadgeTone, ButtonVariant};
use spp_shell::icons;

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

fn draw(ui: &mut egui::Ui, s: &mut Widgets) {
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

    if let Some(i) = design::tabs(ui, &["All", "Active", "Pending", "Closed"], s.tab) {
        s.tab = i;
    }

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

    ui.horizontal(|ui| {
        let _ = design::button(ui, "Primary", ButtonVariant::Primary);
        let _ = design::button(ui, "Outline", ButtonVariant::Outline);
        let _ = design::button(ui, "Text", ButtonVariant::Text);
        let _ = design::button(ui, "Danger", ButtonVariant::Destructive);
        let _ = design::button(ui, "AI assist", ButtonVariant::Ai);
        let _ = design::button_with_icon(ui, icons::Icon::Refresh, "Sync", ButtonVariant::Outline);
    });

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

    ui.horizontal(|ui| {
        design::avatar(ui, AvatarSize::Lg, "KP", BadgeTone::Neutral);
        design::avatar(ui, AvatarSize::Md, "AI", BadgeTone::Ai);
        design::avatar(ui, AvatarSize::Md, "NT", BadgeTone::Warning);
        design::assignee(ui, Some("Kim Pearce"));
        design::assignee(ui, None);
    });

    ui.horizontal(|ui| {
        if design::armed_danger_button(ui, "row-delete", "delete", "confirm delete?") {
            s.confirmed = true;
        }
        let _ = design::button(ui, "Safe action", ButtonVariant::Outline);
    });

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
    h.get_by_label("delete").click();
    h.run_ok();
    h.run_steps(3);
    h.snapshot("r2b_armed_confirm_light");
    assert!(!h.state().confirmed, "first click must only arm");
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

struct InboxIdentity {
    installed: bool,
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
                sent: Vec::new(),
                inserted: 0,
            },
        )
}

fn draw_identity(ui: &mut egui::Ui, s: &mut InboxIdentity) {
    use spp_shell::design::{self, ComposerMode, ComposerState, MessageRole};

    // AU2-13a: the string-body `message_card` wrapper was deleted — the
    // fixture now drives the same `message_card_with` form the inbox and
    // live-chat pages use. The body closure replicates the old wrapper's
    // rendering (LEAD-size foreground label) so only the removals of the
    // dead quoted_history/file_chip/cop_similar surface change the frame.
    // scale::text::LEAD is a crate token, deliberately not pub; the value is
    // pinned here and the snapshot diff-lock flags any token drift.
    const LEAD: f32 = 14.0;
    fn body_label(ui: &mut egui::Ui, body: &str) {
        let t = design::tokens(ui);
        ui.label(egui::RichText::new(body).size(LEAD).color(t.foreground));
    }

    design::day_divider(ui, "Today");

    design::message_card_with(
        ui,
        MessageRole::Customer,
        "Marta Silva",
        None,
        "10:02",
        |ui| body_label(ui, "The export button does nothing on the dashboard."),
        |_| {},
    );
    design::message_card_with(
        ui,
        MessageRole::Agent,
        "Kim Pearce",
        None,
        "10:05",
        |ui| body_label(ui, "Thanks — reproducing now. Can you share the report id?"),
        |_| {},
    );
    design::message_card_with(
        ui,
        MessageRole::Note,
        "Kim Pearce",
        None,
        "10:06",
        |ui| {
            body_label(
                ui,
                "Likely the v2 export path — flagged for the data track.",
            )
        },
        |_| {},
    );
    design::message_card_with(
        ui,
        MessageRole::Ai,
        "Support AI",
        None,
        "10:07",
        |ui| {
            body_label(
                ui,
                "Draft: this matches the known export regression fixed in v1.8.4.",
            )
        },
        |ui| {
            let _ = design::button(ui, "insert as reply", ButtonVariant::Outline);
        },
    );

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
    design::cop_chat(ui, true, "I found 2 similar resolved conversations.");
    design::cop_chat(ui, false, "Show me the fix for the first one.");

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
    h.get_by_label("send").click();
    h.run_ok();
    h.run_steps(2);
    assert_eq!(
        h.state().sent.len(),
        1,
        "send button must emit ComposerAct::Send"
    );
    assert_eq!(h.state().sent[0], "draft text");
    h.key_combination_modifiers(egui::Modifiers::CTRL, &[egui::Key::Enter]);
    h.run_ok();
    h.run_steps(2);
    assert_eq!(h.state().sent.len(), 2, "Ctrl+Enter must send");
}

#[test]
fn ai_message_card_is_identifiable() {
    let mut h = harness_identity(false);
    h.run_ok();
    let ai_nodes = h.get_all_by_label_contains("AI").count();
    assert!(ai_nodes >= 2, "AI badge + role label must both exist");
}

struct HeroState {
    installed: bool,
}

fn harness_hero(dark: bool) -> Harness<'static, HeroState> {
    Harness::builder()
        .with_size(egui::vec2(640.0, 240.0))
        .with_theme(if dark {
            egui::Theme::Dark
        } else {
            egui::Theme::Light
        })
        .build_ui_state(
            |ui, s| {
                if !s.installed {
                    design::install(ui.ctx());
                    s.installed = true;
                }
                design::hero(
                    ui,
                    "LIVE",
                    "last success: 2026-10-27 04:31:12 · never shows relative time — forensic value",
                    |ui| {
                        design::badge(ui, "LIVE", design::BadgeTone::Success);
                    },
                );
            },
            HeroState { installed: false },
        )
}

#[test]
fn hero_snapshot_light() {
    let mut h = harness_hero(false);
    h.run_ok();
    h.snapshot("r20_hero_light");
}

#[test]
fn hero_snapshot_dark() {
    let mut h = harness_hero(true);
    h.run_ok();
    h.snapshot("r20_hero_dark");
}
