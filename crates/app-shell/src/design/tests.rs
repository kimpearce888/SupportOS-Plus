use super::*;

fn luminance(c: egui::Color32) -> f64 {
    fn channel(v: u8) -> f64 {
        let s = f64::from(v) / 255.0;
        if s <= 0.03928 {
            s / 12.92
        } else {
            ((s + 0.055) / 1.055).powf(2.4)
        }
    }
    let [r, g, b, _] = c.to_array();
    0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b)
}

fn contrast(a: egui::Color32, b: egui::Color32) -> f64 {
    let (l1, l2) = (luminance(a), luminance(b));
    let (hi, lo) = if l1 >= l2 { (l1, l2) } else { (l2, l1) };
    (hi + 0.05) / (lo + 0.05)
}

#[test]
fn body_text_meets_aa_in_both_themes() {
    assert!(contrast(LIGHT.foreground, LIGHT.background) >= 7.0);
    assert!(contrast(DARK.foreground, DARK.background) >= 7.0);
    assert!(contrast(LIGHT.muted_foreground, LIGHT.background) >= 4.0);
    assert!(contrast(LIGHT.muted_foreground, LIGHT.card) >= 4.5);
    assert!(contrast(DARK.muted_foreground, DARK.background) >= 4.5);
    assert!(contrast(DARK.muted_foreground, DARK.card) >= 4.5);
}

#[test]
fn accent_text_and_badge_tones_meet_aa() {
    for (fg, bg) in [
        (LIGHT.primary_foreground, LIGHT.primary),
        (DARK.primary_foreground, DARK.primary),
        (LIGHT.success_foreground, LIGHT.success_soft),
        (DARK.success_foreground, DARK.success_soft),
        (LIGHT.warning_foreground, LIGHT.warning_soft),
        (DARK.warning_foreground, DARK.warning_soft),
        (LIGHT.info_foreground, LIGHT.info_soft),
        (DARK.info_foreground, DARK.info_soft),
        (LIGHT.destructive_text, LIGHT.destructive_soft),
        (DARK.destructive_text, DARK.destructive_soft),
    ] {
        assert!(
            contrast(fg, bg) >= 4.5,
            "contrast {:.2} for {:?} on {:?}",
            contrast(fg, bg),
            fg,
            bg
        );
    }
}

#[test]
fn selection_pairs_have_contrast() {
    for (fg, bg) in [
        (LIGHT.selection_foreground, LIGHT.selection),
        (DARK.selection_foreground, DARK.selection),
    ] {
        assert!(contrast(fg, bg) >= 4.5);
    }
}

#[test]
fn ai_and_neutral_badge_pairs_meet_aa() {
    for (fg, bg) in [
        (LIGHT.ai_foreground, LIGHT.ai_soft),
        (DARK.ai_foreground, DARK.ai_soft),
        (LIGHT.neutral_foreground, LIGHT.muted),
        (DARK.neutral_foreground, DARK.muted),
    ] {
        assert!(contrast(fg, bg) >= 4.5, "{fg:?} on {bg:?}");
    }
}

#[test]
fn dc_v1_token_values_are_frozen() {
    assert_eq!(LIGHT.background, rgb(244, 244, 245));
    assert_eq!(DARK.background, rgb(9, 9, 11));
    assert_eq!(LIGHT.card, rgb(255, 255, 255));
    assert_eq!(DARK.card, rgb(24, 24, 27));
    assert_eq!(LIGHT.sidebar, rgb(250, 250, 250));
    assert_eq!(DARK.sidebar, rgb(16, 16, 19));
    assert_eq!(DARK.faint, rgb(12, 12, 14));
    assert_eq!(LIGHT.ai, rgb(124, 58, 237));
    assert_eq!(DARK.ai, rgb(167, 139, 250));
    assert_eq!(LIGHT.on_ai, rgb(255, 255, 255));
    assert_eq!(DARK.on_ai, rgb(255, 255, 255));
    assert_eq!(LIGHT.ai_soft, rgb(245, 243, 255));
    assert_eq!(DARK.ai_soft, rgb(36, 29, 59));
    assert_eq!(LIGHT.ai_foreground, rgb(109, 40, 217));
    assert_eq!(DARK.ai_foreground, rgb(196, 181, 253));
    assert_eq!(LIGHT.ai_border, rgb(221, 214, 254));
    assert_eq!(DARK.ai_border, rgb(70, 58, 112));
    assert_eq!(LIGHT.agent_surface, rgb(250, 250, 250));
    assert_eq!(DARK.agent_surface, rgb(29, 29, 33));
    assert_eq!(LIGHT.agent_border, rgb(228, 228, 231));
    assert_eq!(DARK.agent_border, rgb(44, 44, 49));
    assert_eq!(LIGHT.warning_border, rgb(253, 230, 138));
    assert_eq!(DARK.warning_border, rgb(87, 67, 26));
    assert_eq!(LIGHT.neutral_foreground, rgb(63, 63, 70));
    assert_eq!(DARK.neutral_foreground, rgb(212, 212, 216));
    assert_eq!(LIGHT.hyperlink, LIGHT.ai_foreground);
    assert_eq!(DARK.hyperlink, DARK.ai_foreground);
    assert_eq!(LIGHT.chart_accent, LIGHT.ai);
    assert_eq!(DARK.chart_accent, DARK.ai);
    assert_eq!(
        LIGHT.shadow,
        egui::Color32::from_rgba_premultiplied(0, 0, 0, 36)
    );
    assert_eq!(
        LIGHT.scrim,
        egui::Color32::from_rgba_premultiplied(0, 0, 0, 71)
    );
}

#[test]
fn dc_v1_scales_and_type_roles_are_frozen() {
    assert_eq!(scale::space::SP1, 4.0);
    assert_eq!(scale::space::CLUSTER, 6.0);
    assert_eq!(scale::text::BODY, 13.5);
    assert_eq!(scale::text::TITLE, 18.0);
    assert_eq!(scale::text::SECTION_BIG, 16.0);
    assert_eq!(scale::radius::CARD, 10);
    assert_eq!(scale::radius::CONTROL, 8);
    assert_eq!(scale::radius::BADGE, 6);
    let ctx = egui::Context::default();
    install(&ctx);
    for theme in [egui::Theme::Light, egui::Theme::Dark] {
        let ts = &ctx.style_of(theme).text_styles;
        assert_eq!(ts[&egui::TextStyle::Body].size, scale::text::BODY);
        assert_eq!(ts[&egui::TextStyle::Button].size, scale::text::CONTROL);
        assert_eq!(
            ts[&egui::TextStyle::Monospace].size,
            scale::text::SMALL_HALF
        );
        assert_eq!(ts[&egui::TextStyle::Heading].size, scale::text::TITLE);
        assert_eq!(ts[&egui::TextStyle::Small].size, scale::text::SMALL);
    }
}

#[test]
fn dc_v1_tone_table_is_complete() {
    for status in [
        "open",
        "pending",
        "waiting",
        "resolved",
        "closed",
        "spam",
        "active",
        "published",
        "draft",
        "internal",
        "critical",
        "high",
        "medium",
        "low",
        "ok",
        "healthy",
        "live",
        "stale",
        "error",
        "syncing",
        "catching_up",
        "ai",
        "note",
        "customer",
        "agent",
    ] {
        let (tone, label) = status_tone(status);
        assert!(!label.is_empty());
        let _ = tone_colors_of(&LIGHT, tone);
        let _ = tone_colors_of(&DARK, tone);
    }
    assert_eq!(status_tone("catching_up").1, "Catching up");
    assert_eq!(status_tone("note").0, BadgeTone::Warning);
    assert_eq!(status_tone("critical").0, BadgeTone::Destructive);
    assert_eq!(status_tone("ai").0, BadgeTone::Ai);
    assert_eq!(
        status_tone("totally-unknown"),
        (BadgeTone::Neutral, "Other")
    );
}

#[test]
fn visuals_install_and_render_smoke() {
    let ctx = egui::Context::default();
    install(&ctx);
    let light_panel = ctx.style_of(egui::Theme::Light).visuals.panel_fill;
    let dark_panel = ctx.style_of(egui::Theme::Dark).visuals.panel_fill;
    assert_eq!(light_panel, LIGHT.background);
    assert_eq!(dark_panel, DARK.background);

    ctx.set_theme(egui::Theme::Dark);
    ctx.run_pass(egui::RawInput::default(), |ctx| {
        ctx.root_ui(|ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                page_header(ui, "Inbox", Some("42 open conversations"), |ui| {
                    let _ = button(ui, "New conversation", ButtonVariant::Primary);
                });
                let mut clicked = false;
                if chip(ui, true, "Active").clicked() {
                    clicked = true;
                }
                assert!(!clicked);
                badge(ui, "Active", BadgeTone::Info);
                metric_card(ui, "New", "12", Some("last 30 days"));
                data_table(ui, "smoke", &["name", "count"], 1, 24.0, |ui, i| {
                    if i == 0 {
                        ui.label("row");
                        td_num(ui, "3");
                    }
                });
                empty_state(ui, Icon::Inbox, "Nothing here", "No conversations match.");
                card_titled(ui, "A card", Some("with a hint"), |ui| {
                    hint_text(ui, "content");
                });
                let _ = icon_button(ui, Icon::Sun, ButtonVariant::Ghost, "theme");
            });
        });
    })
    .drop_without_applying_deltas();
}
