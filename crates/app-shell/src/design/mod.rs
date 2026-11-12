use eframe::egui;

use crate::icons::Icon;

mod controls;
use controls::CARD_PAD_X;
use controls::chip_impl;
pub(crate) use controls::{
    card, card_titled, card_titled_exact, card_titled_wide, hint_text, pager, section_title,
};

#[derive(Debug, Clone, Copy)]
pub struct Tokens {
    pub background: egui::Color32,
    pub sidebar: egui::Color32,
    pub card: egui::Color32,
    pub border: egui::Color32,
    pub border_strong: egui::Color32,
    pub foreground: egui::Color32,
    pub muted: egui::Color32,
    pub muted_foreground: egui::Color32,
    pub accent: egui::Color32,
    pub primary: egui::Color32,
    pub primary_foreground: egui::Color32,
    pub on_ai: egui::Color32,
    pub secondary: egui::Color32,
    pub secondary_foreground: egui::Color32,
    pub destructive: egui::Color32,
    pub destructive_foreground: egui::Color32,
    pub success: egui::Color32,
    pub success_soft: egui::Color32,
    pub success_foreground: egui::Color32,
    pub warning: egui::Color32,
    pub warning_soft: egui::Color32,
    pub warning_foreground: egui::Color32,
    pub info: egui::Color32,
    pub info_soft: egui::Color32,
    pub info_foreground: egui::Color32,
    pub destructive_soft: egui::Color32,
    pub destructive_text: egui::Color32,
    pub ring: egui::Color32,
    pub selection: egui::Color32,
    pub selection_foreground: egui::Color32,
    pub input: egui::Color32,
    pub faint: egui::Color32,
    pub hyperlink: egui::Color32,
    pub chart_accent: egui::Color32,
    pub chart_accent_2: egui::Color32,
    pub hover_tint: egui::Color32,
    pub ai: egui::Color32,
    pub ai_soft: egui::Color32,
    pub ai_foreground: egui::Color32,
    pub ai_border: egui::Color32,
    pub agent_surface: egui::Color32,
    pub agent_border: egui::Color32,
    pub warning_border: egui::Color32,
    pub neutral_foreground: egui::Color32,
    pub shadow: egui::Color32,
    pub scrim: egui::Color32,
}

const fn rgb(r: u8, g: u8, b: u8) -> egui::Color32 {
    egui::Color32::from_rgb(r, g, b)
}

pub(crate) const LIGHT: Tokens = Tokens {
    background: rgb(244, 244, 245),
    sidebar: rgb(250, 250, 250),
    card: rgb(255, 255, 255),
    border: rgb(228, 228, 231),
    border_strong: rgb(212, 212, 216),
    foreground: rgb(9, 9, 11),
    muted: rgb(244, 244, 245),
    muted_foreground: rgb(113, 113, 122),
    accent: rgb(244, 244, 245),
    primary: rgb(24, 24, 27),
    primary_foreground: rgb(250, 250, 250),
    on_ai: rgb(255, 255, 255),
    secondary: rgb(244, 244, 245),
    secondary_foreground: rgb(24, 24, 27),
    destructive: rgb(220, 38, 38),
    destructive_foreground: rgb(255, 255, 255),
    success: rgb(22, 163, 74),
    success_soft: rgb(240, 253, 244),
    success_foreground: rgb(21, 128, 61),
    warning: rgb(217, 119, 6),
    warning_soft: rgb(255, 251, 235),
    warning_foreground: rgb(180, 83, 9),
    info: rgb(37, 99, 235),
    info_soft: rgb(239, 246, 255),
    info_foreground: rgb(29, 78, 216),
    destructive_soft: rgb(254, 226, 226),
    destructive_text: rgb(185, 28, 28),
    ring: rgb(24, 24, 27),
    selection: rgb(244, 244, 245),
    selection_foreground: rgb(24, 24, 27),
    input: rgb(255, 255, 255),
    faint: rgb(250, 250, 250),
    hyperlink: rgb(109, 40, 217),
    chart_accent: rgb(124, 58, 237),
    chart_accent_2: rgb(24, 24, 27),
    hover_tint: rgb(9, 9, 11),
    ai: rgb(124, 58, 237),
    ai_soft: rgb(245, 243, 255),
    ai_foreground: rgb(109, 40, 217),
    ai_border: rgb(221, 214, 254),
    agent_surface: rgb(250, 250, 250),
    agent_border: rgb(228, 228, 231),
    warning_border: rgb(253, 230, 138),
    neutral_foreground: rgb(63, 63, 70),
    shadow: egui::Color32::from_rgba_premultiplied(0, 0, 0, 36),
    scrim: egui::Color32::from_rgba_premultiplied(0, 0, 0, 71),
};

pub(crate) const DARK: Tokens = Tokens {
    background: rgb(9, 9, 11),
    sidebar: rgb(16, 16, 19),
    card: rgb(24, 24, 27),
    border: rgb(48, 48, 53),
    border_strong: rgb(66, 66, 72),
    foreground: rgb(250, 250, 250),
    muted: rgb(39, 39, 42),
    muted_foreground: rgb(186, 186, 196),
    accent: rgb(39, 39, 42),
    primary: rgb(250, 250, 250),
    primary_foreground: rgb(24, 24, 27),
    on_ai: rgb(255, 255, 255),
    secondary: rgb(39, 39, 42),
    secondary_foreground: rgb(250, 250, 250),
    destructive: rgb(185, 28, 28),
    destructive_foreground: rgb(250, 250, 250),
    success: rgb(34, 197, 94),
    success_soft: rgb(10, 44, 26),
    success_foreground: rgb(74, 222, 128),
    warning: rgb(245, 158, 11),
    warning_soft: rgb(43, 34, 5),
    warning_foreground: rgb(251, 191, 36),
    info: rgb(59, 130, 246),
    info_soft: rgb(23, 37, 84),
    info_foreground: rgb(96, 165, 250),
    destructive_soft: rgb(69, 26, 26),
    destructive_text: rgb(248, 113, 113),
    ring: rgb(212, 212, 216),
    selection: rgb(39, 39, 42),
    selection_foreground: rgb(250, 250, 250),
    input: rgb(12, 12, 14),
    faint: rgb(12, 12, 14),
    hyperlink: rgb(196, 181, 253),
    chart_accent: rgb(167, 139, 250),
    chart_accent_2: rgb(250, 250, 250),
    hover_tint: rgb(250, 250, 250),
    ai: rgb(167, 139, 250),
    ai_soft: rgb(36, 29, 59),
    ai_foreground: rgb(196, 181, 253),
    ai_border: rgb(70, 58, 112),
    agent_surface: rgb(29, 29, 33),
    agent_border: rgb(44, 44, 49),
    warning_border: rgb(87, 67, 26),
    neutral_foreground: rgb(212, 212, 216),
    shadow: egui::Color32::from_rgba_premultiplied(0, 0, 0, 36),
    scrim: egui::Color32::from_rgba_premultiplied(0, 0, 0, 71),
};

#[must_use]
pub fn tokens(ui: &egui::Ui) -> &'static Tokens {
    if ui.visuals().dark_mode {
        &DARK
    } else {
        &LIGHT
    }
}

#[must_use]
pub(crate) fn tokens_for(ctx: &egui::Context) -> &'static Tokens {
    if ctx.theme() == egui::Theme::Dark {
        &DARK
    } else {
        &LIGHT
    }
}

pub mod scale {
    pub mod space {
        pub const SP1: f32 = 4.0;
        pub const SP2: f32 = 8.0;
        pub const SP3: f32 = 12.0;
        pub const SP4: f32 = 16.0;
        pub const SP5: f32 = 24.0;
        pub const SP6: f32 = 32.0;
        pub(crate) const CLUSTER: f32 = 6.0;
        pub(crate) const PAD_CARD: f32 = 14.0;
        pub(crate) const PAD_TOOL: f32 = 10.0;
    }
    pub mod text {
        pub(crate) const META: f32 = 11.0;
        pub(crate) const META_HALF: f32 = 11.5;
        pub(crate) const SMALL: f32 = 12.0;
        pub(crate) const SMALL_HALF: f32 = 12.5;
        pub(crate) const CONTROL: f32 = 13.0;
        pub(crate) const BODY: f32 = 13.5;
        pub(crate) const LEAD: f32 = 14.0;
        pub(crate) const SECTION: f32 = 15.0;
        pub(crate) const SECTION_BIG: f32 = 16.0;
        pub(crate) const STAT: f32 = 20.0;
        pub(crate) const STAT_BIG: f32 = 26.0;
        pub(crate) const TITLE: f32 = 18.0;
    }
    pub(crate) mod radius {
        pub(crate) const CARD: u8 = 10;
        pub(crate) const CONTROL: u8 = 8;
        pub(crate) const BADGE: u8 = 6;
    }
    pub const CONTENT_MAX: f32 = 1800.0;
    pub const READER_MEASURE: f32 = 860.0;
}

pub fn install(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    use egui::FontFamily::{Monospace, Proportional};
    use egui::TextStyle::{Body, Button, Heading, Monospace as MonoStyle, Small};
    fonts
        .families
        .get_mut(&Proportional)
        .expect("proportional family")
        .push("Hack".into());
    let mut text_styles = egui::Style::default().text_styles;
    text_styles.insert(Heading, egui::FontId::new(scale::text::TITLE, Proportional));
    text_styles.insert(Body, egui::FontId::new(scale::text::BODY, Proportional));
    text_styles.insert(
        Button,
        egui::FontId::new(scale::text::CONTROL, Proportional),
    );
    text_styles.insert(
        MonoStyle,
        egui::FontId::new(scale::text::SMALL_HALF, Monospace),
    );
    text_styles.insert(Small, egui::FontId::new(scale::text::SMALL, Proportional));
    ctx.set_fonts(fonts);

    ctx.all_styles_mut(|style| {
        style.text_styles = text_styles.clone();
        style.spacing.item_spacing = egui::vec2(8.0, 8.0);
        style.spacing.button_padding = egui::vec2(12.0, 6.0);
        style.spacing.window_margin = egui::Margin::same(12);
        style.spacing.menu_margin = egui::Margin::same(8);
        style.spacing.interact_size = egui::vec2(0.0, 32.0);
        style.spacing.tooltip_width = 300.0;
        style.visuals.interact_cursor = Some(egui::CursorIcon::PointingHand);
    });

    ctx.set_visuals_of(egui::Theme::Light, light_visuals());
    ctx.set_visuals_of(egui::Theme::Dark, dark_visuals());
}

#[must_use]
pub(crate) fn light_visuals() -> egui::Visuals {
    let mut v = egui::Visuals::light();
    let t = &LIGHT;
    v.dark_mode = false;
    v.panel_fill = t.background;
    v.window_fill = t.card;
    v.window_stroke = egui::Stroke::new(1.0, t.border);
    v.window_corner_radius = egui::CornerRadius::same(12);
    v.window_shadow = shadow(14, 40);
    v.popup_shadow = shadow(16, 36);
    v.menu_corner_radius = egui::CornerRadius::same(10);
    v.extreme_bg_color = t.input;
    v.text_edit_bg_color = Some(t.input);
    v.faint_bg_color = t.faint;
    v.code_bg_color = t.muted;
    v.hyperlink_color = t.hyperlink;
    v.warn_fg_color = t.warning_foreground;
    v.error_fg_color = t.destructive;
    v.selection = egui::style::Selection {
        bg_fill: t.selection,
        stroke: egui::Stroke::new(1.0, t.selection_foreground),
    };
    v.widgets = egui::style::Widgets {
        noninteractive: widget(
            t.sidebar,
            egui::Color32::TRANSPARENT,
            t.border,
            t.foreground,
        ),
        inactive: widget(t.card, t.card, t.border, t.foreground),
        hovered: widget(t.card, t.muted, t.border_strong, t.foreground),
        active: widget(t.card, t.border, t.border_strong, t.foreground),
        open: widget(t.card, t.muted, t.border_strong, t.foreground),
    };
    v.button_frame = true;
    v
}

#[must_use]
pub(crate) fn dark_visuals() -> egui::Visuals {
    let mut v = egui::Visuals::dark();
    let t = &DARK;
    v.dark_mode = true;
    v.panel_fill = t.background;
    v.window_fill = t.card;
    v.window_stroke = egui::Stroke::new(1.0, t.border);
    v.window_corner_radius = egui::CornerRadius::same(12);
    v.window_shadow = shadow(24, 64);
    v.popup_shadow = shadow(20, 56);
    v.menu_corner_radius = egui::CornerRadius::same(10);
    v.extreme_bg_color = t.input;
    v.text_edit_bg_color = Some(t.input);
    v.faint_bg_color = t.faint;
    v.code_bg_color = t.muted;
    v.hyperlink_color = t.hyperlink;
    v.warn_fg_color = t.warning_foreground;
    v.error_fg_color = rgb(248, 113, 113);
    v.selection = egui::style::Selection {
        bg_fill: t.selection,
        stroke: egui::Stroke::new(1.0, t.selection_foreground),
    };
    v.widgets = egui::style::Widgets {
        noninteractive: widget(
            t.background,
            egui::Color32::TRANSPARENT,
            t.border,
            t.foreground,
        ),
        inactive: widget(t.card, t.card, t.border, t.foreground),
        hovered: widget(t.card, t.muted, t.border_strong, t.foreground),
        active: widget(t.card, t.border_strong, t.border_strong, t.foreground),
        open: widget(t.card, t.muted, t.border_strong, t.foreground),
    };
    v.button_frame = true;
    v
}

fn widget(
    bg_fill: egui::Color32,
    weak: egui::Color32,
    border: egui::Color32,
    fg: egui::Color32,
) -> egui::style::WidgetVisuals {
    egui::style::WidgetVisuals {
        bg_fill,
        weak_bg_fill: weak,
        bg_stroke: egui::Stroke::new(1.0, border),
        corner_radius: egui::CornerRadius::same(scale::radius::CONTROL),
        fg_stroke: egui::Stroke::new(1.0, fg),
        expansion: 0.0,
    }
}

fn shadow(blur: u8, alpha: u8) -> egui::Shadow {
    egui::Shadow {
        offset: [0, 4],
        blur,
        spread: 0,
        color: egui::Color32::from_black_alpha(alpha),
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ButtonVariant {
    #[default]
    Primary,
    Secondary,
    Outline,
    Ghost,
    Destructive,
    Text,
    Ai,
}

pub fn button(ui: &mut egui::Ui, text: &str, variant: ButtonVariant) -> egui::Response {
    button_impl(ui, text, None, variant)
}

pub fn button_with_icon(
    ui: &mut egui::Ui,
    icon: Icon,
    text: &str,
    variant: ButtonVariant,
) -> egui::Response {
    button_impl(ui, text, Some(icon), variant)
}

pub(crate) fn icon_button(
    ui: &mut egui::Ui,
    icon: Icon,
    variant: ButtonVariant,
    tooltip: &str,
) -> egui::Response {
    let t = tokens(ui);
    let (fill, fg, stroke) = variant_look(t, variant);
    let size = egui::vec2(30.0, 30.0);
    let (id, rect) = ui.allocate_space(size);
    let response = ui
        .interact(
            rect,
            id.with(("spp-icon-btn", tooltip)),
            egui::Sense::click(),
        )
        .on_hover_text(tooltip);
    let bg = if fill == egui::Color32::TRANSPARENT && (response.hovered() || response.has_focus()) {
        t.accent
    } else {
        fill
    };
    if bg != egui::Color32::TRANSPARENT {
        ui.painter()
            .rect_filled(rect, egui::CornerRadius::same(6), bg);
    }
    if stroke != egui::Stroke::NONE {
        ui.painter().rect_stroke(
            rect,
            egui::CornerRadius::same(6),
            stroke,
            egui::StrokeKind::Inside,
        );
    }
    let icon_rect = egui::Rect::from_center_size(
        rect.center(),
        egui::vec2(rect.height() * 0.6, rect.height() * 0.6),
    );
    icon.paint_at(ui.painter(), icon_rect, fg);
    hover_overlay(ui, &response, t, variant);
    focus_feedback(ui, &response);
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), tooltip.to_owned())
    });
    response
}

fn button_impl(
    ui: &mut egui::Ui,
    text: &str,
    icon: Option<Icon>,
    variant: ButtonVariant,
) -> egui::Response {
    let t = tokens(ui);
    let (fill, fg, stroke) = variant_look(t, variant);
    let galley = egui::WidgetText::from(
        egui::RichText::new(text)
            .color(fg)
            .size(scale::text::CONTROL),
    )
    .into_galley(
        ui,
        Some(egui::TextWrapMode::Extend),
        f32::INFINITY,
        egui::TextStyle::Button,
    );
    let pad = 14.0;
    let icon_zone = if icon.is_some() { 13.0 + 5.0 } else { 0.0 };
    let size = egui::vec2(icon_zone + galley.size().x + 2.0 * pad, 32.0);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    if fill != egui::Color32::TRANSPARENT {
        ui.painter()
            .rect_filled(rect, egui::CornerRadius::same(8), fill);
    }
    if stroke != egui::Stroke::NONE {
        ui.painter().rect_stroke(
            rect,
            egui::CornerRadius::same(8),
            stroke,
            egui::StrokeKind::Inside,
        );
    }
    let mut x = rect.min.x + pad;
    if let Some(icon) = icon {
        let icon_rect = egui::Rect::from_center_size(
            egui::pos2(x + 6.5, rect.center().y),
            egui::vec2(13.0, 13.0),
        );
        icon.paint_at(ui.painter(), icon_rect, fg);
        x += icon_zone;
    }
    ui.painter().galley(
        egui::pos2(x, rect.center().y - galley.size().y / 2.0),
        galley,
        fg,
    );
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), text.to_owned())
    });
    hover_overlay(ui, &response, t, variant);
    focus_feedback(ui, &response);
    response
}

fn variant_look(
    t: &Tokens,
    variant: ButtonVariant,
) -> (egui::Color32, egui::Color32, egui::Stroke) {
    match variant {
        ButtonVariant::Primary => (t.primary, t.primary_foreground, egui::Stroke::NONE),
        ButtonVariant::Secondary => (t.secondary, t.secondary_foreground, egui::Stroke::NONE),
        ButtonVariant::Outline => (t.card, t.foreground, egui::Stroke::new(1.0, t.border)),
        ButtonVariant::Ghost => (egui::Color32::TRANSPARENT, t.foreground, egui::Stroke::NONE),
        ButtonVariant::Destructive => (t.destructive, t.destructive_foreground, egui::Stroke::NONE),
        ButtonVariant::Text => (
            egui::Color32::TRANSPARENT,
            t.muted_foreground,
            egui::Stroke::NONE,
        ),
        ButtonVariant::Ai => (t.ai, t.on_ai, egui::Stroke::NONE),
    }
}

fn hover_overlay(ui: &mut egui::Ui, response: &egui::Response, t: &Tokens, variant: ButtonVariant) {
    let light_mode = !ui.visuals().dark_mode;
    let darken = match variant {
        ButtonVariant::Primary => true,
        ButtonVariant::Secondary | ButtonVariant::Ghost | ButtonVariant::Outline => light_mode,
        ButtonVariant::Destructive => true,
        ButtonVariant::Text => light_mode,
        ButtonVariant::Ai => true,
    };
    let tint = if darken && light_mode {
        LIGHT.hover_tint
    } else if darken {
        egui::Color32::BLACK
    } else {
        DARK.hover_tint
    };
    let _ = t;
    let [tr, tg, tb, _] = tint.to_array();
    let color = |alpha: u8| egui::Color32::from_rgba_unmultiplied(tr, tg, tb, alpha);
    let rect = response.rect;
    if response.is_pointer_button_down_on() {
        ui.painter()
            .rect_filled(rect, egui::CornerRadius::same(6), color(48));
    } else if response.hovered() {
        ui.painter()
            .rect_filled(rect, egui::CornerRadius::same(6), color(22));
    }
}

fn focus_feedback(ui: &mut egui::Ui, response: &egui::Response) {
    if response.has_focus() {
        let t = tokens(ui);
        ui.painter().rect_stroke(
            response.rect.expand(2.0),
            egui::CornerRadius::same(8),
            egui::Stroke::new(2.0, t.ring),
            egui::StrokeKind::Outside,
        );
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BadgeTone {
    Neutral,
    Success,
    Warning,
    Destructive,
    Info,
    Ai,
}

pub(crate) fn tone_colors(ui: &egui::Ui, tone: BadgeTone) -> (egui::Color32, egui::Color32) {
    tone_colors_of(tokens(ui), tone)
}

pub(crate) fn tone_colors_of(t: &Tokens, tone: BadgeTone) -> (egui::Color32, egui::Color32) {
    match tone {
        BadgeTone::Neutral => (t.muted, t.neutral_foreground),
        BadgeTone::Success => (t.success_soft, t.success_foreground),
        BadgeTone::Warning => (t.warning_soft, t.warning_foreground),
        BadgeTone::Destructive => (t.destructive_soft, t.destructive_text),
        BadgeTone::Info => (t.info_soft, t.info_foreground),
        BadgeTone::Ai => (t.ai_soft, t.ai_foreground),
    }
}

pub fn badge(ui: &mut egui::Ui, text: &str, tone: BadgeTone) -> egui::Response {
    let (bg, fg) = tone_colors(ui, tone);
    let ai_stroke = (tone == BadgeTone::Ai).then(|| egui::Stroke::new(1.0, tokens(ui).ai_border));
    let rich = egui::RichText::new(text).color(fg).size(12.0).strong();
    let galley = egui::WidgetText::from(rich).into_galley(
        ui,
        Some(egui::TextWrapMode::Extend),
        f32::INFINITY,
        egui::TextStyle::Small,
    );
    let pad = egui::vec2(9.0, 3.0);
    let size = galley.size() + 2.0 * pad;
    let (id, rect) = ui.allocate_space(size);
    ui.painter()
        .rect_filled(rect, egui::CornerRadius::same(6), bg);
    if let Some(stroke) = ai_stroke {
        ui.painter().rect_stroke(
            rect,
            egui::CornerRadius::same(6),
            stroke,
            egui::StrokeKind::Inside,
        );
    }
    ui.painter().galley(
        egui::pos2(rect.min.x + pad.x, rect.center().y - galley.size().y / 2.0),
        galley,
        fg,
    );
    ui.interact(rect, id, egui::Sense::hover())
}

pub(crate) fn status_tone(status: &str) -> (BadgeTone, &'static str) {
    match status {
        "open" => (BadgeTone::Info, "Open"),
        "pending" => (BadgeTone::Warning, "Pending"),
        "waiting" => (BadgeTone::Warning, "Waiting"),
        "resolved" => (BadgeTone::Success, "Resolved"),
        "closed" => (BadgeTone::Neutral, "Closed"),
        "spam" => (BadgeTone::Destructive, "Spam"),
        "active" => (BadgeTone::Info, "Active"),
        "published" => (BadgeTone::Success, "Published"),
        "draft" => (BadgeTone::Neutral, "Draft"),
        "internal" => (BadgeTone::Info, "Internal"),
        "critical" => (BadgeTone::Destructive, "Critical"),
        "high" => (BadgeTone::Warning, "High"),
        "medium" => (BadgeTone::Info, "Medium"),
        "low" => (BadgeTone::Neutral, "Low"),
        "ok" => (BadgeTone::Success, "OK"),
        "healthy" => (BadgeTone::Success, "Healthy"),
        "live" => (BadgeTone::Success, "Live"),
        "stale" => (BadgeTone::Warning, "Stale"),
        "error" => (BadgeTone::Destructive, "Error"),
        "syncing" => (BadgeTone::Info, "Syncing"),
        "catching_up" => (BadgeTone::Info, "Catching up"),
        "ai" => (BadgeTone::Ai, "AI"),
        "note" => (BadgeTone::Warning, "Internal note"),
        "customer" => (BadgeTone::Neutral, "Customer"),
        "agent" => (BadgeTone::Neutral, "Agent"),
        _ => (BadgeTone::Neutral, "Other"),
    }
}

pub fn chip(ui: &mut egui::Ui, selected: bool, text: &str) -> egui::Response {
    chip_impl(ui, selected, text, None)
}

pub fn chip_with_icon(ui: &mut egui::Ui, selected: bool, icon: Icon, text: &str) -> egui::Response {
    chip_impl(ui, selected, text, Some(icon))
}

pub fn segmented(
    ui: &mut egui::Ui,
    id_salt: &str,
    options: &[&str],
    active: usize,
) -> Option<usize> {
    let t = tokens(ui);
    let mut clicked = None;
    egui::Frame::new()
        .fill(t.muted)
        .corner_radius(egui::CornerRadius::same(scale::radius::CONTROL))
        .inner_margin(egui::Margin::same(2))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                for (i, opt) in options.iter().enumerate() {
                    let on = i == active;
                    let rich = egui::RichText::new(*opt)
                        .size(scale::text::CONTROL)
                        .color(if on { t.foreground } else { t.muted_foreground });
                    let rich = if on { rich.strong() } else { rich };
                    let galley = egui::WidgetText::from(rich).into_galley(
                        ui,
                        Some(egui::TextWrapMode::Extend),
                        f32::INFINITY,
                        egui::TextStyle::Button,
                    );
                    let size = egui::vec2(galley.size().x + 22.0, 28.0);
                    let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::click());
                    if on {
                        let r = egui::CornerRadius::same(scale::radius::CONTROL - 2);
                        ui.painter().rect_filled(rect, r, t.card);
                        if ui.visuals().dark_mode {
                            ui.painter().rect_stroke(
                                rect,
                                r,
                                egui::Stroke::new(1.0, t.border),
                                egui::StrokeKind::Inside,
                            );
                        }
                    }
                    ui.painter().galley(
                        egui::pos2(
                            rect.center().x - galley.size().x / 2.0,
                            rect.center().y - galley.size().y / 2.0,
                        ),
                        galley,
                        t.foreground,
                    );
                    focus_feedback(ui, &resp);
                    resp.widget_info(|| {
                        egui::WidgetInfo::selected(
                            egui::Role::RadioButton,
                            ui.is_enabled(),
                            on,
                            (*opt).to_owned(),
                        )
                    });
                    if resp.clicked() {
                        clicked = Some(i);
                    }
                }
            });
        });
    let _ = id_salt;
    clicked
}

pub fn switch(ui: &mut egui::Ui, on: bool) -> bool {
    let t = tokens(ui);
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(34.0, 20.0), egui::Sense::click());
    let track = if on { t.primary } else { t.border_strong };
    ui.painter()
        .rect_filled(rect, egui::CornerRadius::same(10), track);
    let knob_d = 14.0;
    let cx = if on {
        rect.max.x - 2.0 - knob_d / 2.0
    } else {
        rect.min.x + 2.0 + knob_d / 2.0
    };
    let knob = if on && ui.visuals().dark_mode {
        t.primary_foreground
    } else {
        egui::Color32::WHITE
    };
    ui.painter()
        .circle_filled(egui::pos2(cx, rect.center().y), knob_d / 2.0, knob);
    if let Some(cursor) = ui.visuals().interact_cursor
        && resp.hovered()
    {
        ui.ctx().set_cursor_icon(cursor);
    }
    focus_feedback(ui, &resp);
    resp.widget_info(|| {
        egui::WidgetInfo::selected(egui::Role::Switch, ui.is_enabled(), on, "switch")
    });
    if resp.clicked() { !on } else { on }
}

pub fn select<R>(
    ui: &mut egui::Ui,
    id_salt: &str,
    selected: &str,
    width: f32,
    menu: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::Response {
    let t = tokens(ui);
    let rich = egui::RichText::new(selected)
        .size(scale::text::CONTROL)
        .color(t.foreground);
    let galley = egui::WidgetText::from(rich).into_galley(
        ui,
        Some(egui::TextWrapMode::Truncate),
        (width - 46.0).max(20.0),
        egui::TextStyle::Button,
    );
    let size = egui::vec2(width.max(galley.size().x + 46.0), 32.0);
    let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::click());
    let radius = egui::CornerRadius::same(scale::radius::CONTROL);
    ui.painter().rect_filled(rect, radius, t.card);
    let stroke_col = if resp.hovered() || resp.has_focus() {
        t.border_strong
    } else {
        t.border
    };
    ui.painter().rect_stroke(
        rect,
        radius,
        egui::Stroke::new(1.0, stroke_col),
        egui::StrokeKind::Inside,
    );
    ui.painter().galley(
        egui::pos2(rect.min.x + 12.0, rect.center().y - galley.size().y / 2.0),
        galley,
        t.foreground,
    );
    let chev = 13.0;
    let chev_rect = egui::Rect::from_center_size(
        egui::pos2(rect.max.x - 17.0, rect.center().y),
        egui::vec2(chev, chev),
    );
    Icon::ChevronDown.paint_at(ui.painter(), chev_rect, t.muted_foreground);
    focus_feedback(ui, &resp);
    let selected_owned = selected.to_owned();
    resp.widget_info(|| {
        let mut info = egui::WidgetInfo::labeled(
            egui::Role::ComboBox,
            ui.is_enabled(),
            selected_owned.clone(),
        );
        info.current_text_value = Some(selected_owned.clone());
        info
    });
    let popup_id = ui.make_persistent_id(("spp-select", id_salt));
    egui::Popup::menu(&resp)
        .id(popup_id)
        .width(rect.width())
        .frame(
            egui::Frame::new()
                .fill(t.card)
                .stroke(egui::Stroke::new(1.0, t.border))
                .corner_radius(egui::CornerRadius::same(10))
                .inner_margin(egui::Margin::same(4))
                .shadow(egui::Shadow {
                    offset: [0, 8],
                    blur: 24,
                    spread: 0,
                    color: egui::Color32::from_black_alpha(60),
                }),
        )
        .show(|ui| {
            ui.set_min_width(rect.width() - 8.0);
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
            menu(ui)
        });
    resp
}

pub fn page_header(
    ui: &mut egui::Ui,
    title: &str,
    subtitle: Option<&str>,
    actions: impl FnOnce(&mut egui::Ui),
) {
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.label(
                egui::RichText::new(title)
                    .text_style(egui::TextStyle::Heading)
                    .strong(),
            );
            if let Some(subtitle) = subtitle {
                hint_text(ui, subtitle);
            }
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            actions(ui);
        });
    });
    ui.add_space(scale::space::SP3);
}

pub fn centered_column<R>(
    ui: &mut egui::Ui,
    max_w: f32,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<R> {
    let avail = ui.available_rect_before_wrap();
    let width = avail.width();
    let (col, gutter) = if width.is_finite() {
        let col = width.min(max_w).max(1.0);
        (col, ((width - col) * 0.5).floor().max(0.0))
    } else {
        (max_w, 0.0)
    };
    let left = avail.left() + gutter;
    let rect = egui::Rect::from_min_max(
        egui::pos2(left, avail.top()),
        egui::pos2(left + col, avail.bottom()),
    );
    ui.scope_builder(egui::UiBuilder::new().max_rect(rect), add_contents)
}

pub fn content_column<R>(
    ui: &mut egui::Ui,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<R> {
    centered_column(ui, scale::CONTENT_MAX, add_contents)
}

pub fn host_standard_page<R>(
    ui: &mut egui::Ui,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<R> {
    let bg = tokens(ui).background;
    egui::CentralPanel::default()
        .frame(egui::Frame::new().fill(bg))
        .show(ui, |ui| {
            egui::ScrollArea::vertical()
                .id_salt("spp-page-scroll")
                .auto_shrink([false; 2])
                .show(ui, |ui| {
                    ui.add_space(2.0);
                    content_column(ui, |ui| {
                        egui::Frame::new()
                            .fill(egui::Color32::TRANSPARENT)
                            .inner_margin(egui::Margin::symmetric(
                                scale::space::SP5 as i8,
                                scale::space::PAD_CARD as i8,
                            ))
                            .show(ui, add_contents)
                            .inner
                    })
                    .inner
                })
                .inner
        })
}

pub fn host_inbox_page<R>(
    ui: &mut egui::Ui,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<R> {
    let bg = tokens(ui).background;
    egui::CentralPanel::default()
        .frame(
            egui::Frame::new()
                .fill(bg)
                .inner_margin(egui::Margin::symmetric(
                    scale::space::SP5 as i8,
                    scale::space::PAD_CARD as i8,
                )),
        )
        .show(ui, add_contents)
}

pub fn metric_card(ui: &mut egui::Ui, label: &str, value: &str, hint: Option<&str>) {
    metric_card_w(ui, label, value, hint, 150.0);
}

pub fn metric_card_w(
    ui: &mut egui::Ui,
    label: &str,
    value: &str,
    hint: Option<&str>,
    outer_w: f32,
) {
    card(ui, |ui| {
        ui.set_min_width((outer_w - CARD_PAD_X).max(96.0));
        ui.vertical(|ui| {
            ui.label(
                egui::RichText::new(label)
                    .small()
                    .color(tokens(ui).muted_foreground),
            );
            ui.label(
                egui::RichText::new(value)
                    .size(scale::text::STAT_BIG)
                    .strong()
                    .color(tokens(ui).foreground),
            );
            if let Some(hint) = hint {
                ui.label(
                    egui::RichText::new(hint)
                        .small()
                        .color(tokens(ui).muted_foreground),
                );
            }
        });
    });
}

pub fn hero(ui: &mut egui::Ui, state: &str, sub: &str, right: impl FnOnce(&mut egui::Ui)) {
    card(ui, |ui| {
        ui.horizontal(|ui| {
            let t = tokens(ui);
            ui.label(
                egui::RichText::new(state)
                    .size(17.0)
                    .monospace()
                    .color(t.foreground),
            );
            ui.label(
                egui::RichText::new(sub)
                    .size(scale::text::SMALL)
                    .color(t.muted_foreground),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), right);
        });
    });
}

pub fn page_counter(ui: &mut egui::Ui, text: &str) {
    let t = tokens(ui);
    let galley = egui::WidgetText::from(
        egui::RichText::new(text)
            .size(scale::text::SMALL_HALF)
            .color(t.muted_foreground),
    )
    .into_galley(
        ui,
        Some(egui::TextWrapMode::Extend),
        f32::INFINITY,
        egui::TextStyle::Small,
    );
    let pad = egui::vec2(10.0, 3.0);
    let size = galley.size() + 2.0 * pad;
    let (id, rect) = ui.allocate_space(size);
    ui.painter()
        .rect_filled(rect, egui::CornerRadius::same(9), t.muted);
    ui.painter().galley(
        egui::pos2(rect.min.x + pad.x, rect.center().y - galley.size().y / 2.0),
        galley,
        t.muted_foreground,
    );
    ui.advance_cursor_after_rect(rect);
    let _ = id;
}

pub(crate) fn view_item(
    ui: &mut egui::Ui,
    label: &str,
    count: u32,
    selected: bool,
) -> egui::Response {
    let t = tokens(ui);
    let (rect, resp) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 30.0), egui::Sense::click());
    let fill = if selected {
        t.selection
    } else if resp.hovered() {
        t.accent
    } else {
        egui::Color32::TRANSPARENT
    };
    if fill != egui::Color32::TRANSPARENT {
        ui.painter()
            .rect_filled(rect, egui::CornerRadius::same(8), fill);
    }
    ui.painter().text(
        egui::pos2(rect.min.x + 10.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        label,
        egui::FontId::proportional(scale::text::CONTROL),
        if selected {
            t.foreground
        } else {
            t.muted_foreground
        },
    );
    ui.painter().text(
        egui::pos2(rect.max.x - 10.0, rect.center().y),
        egui::Align2::RIGHT_CENTER,
        count.to_string(),
        egui::FontId::proportional(scale::text::META),
        t.muted_foreground,
    );
    resp.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::Role::ListItem,
            true,
            selected,
            format!("{label} ({count})"),
        )
    });
    resp
}

pub(crate) fn tone_solid(t: &Tokens, tone: BadgeTone) -> egui::Color32 {
    match tone {
        BadgeTone::Neutral => t.border_strong,
        BadgeTone::Success => t.success,
        BadgeTone::Warning => t.warning,
        BadgeTone::Destructive => t.destructive,
        BadgeTone::Info => t.info,
        BadgeTone::Ai => t.ai,
    }
}

pub fn status_dot(ui: &mut egui::Ui, tone: BadgeTone, big: bool) {
    let t = tokens(ui);
    let d = if big { 11.0 } else { 8.0 };
    let (id, rect) = ui.allocate_space(egui::vec2(d, d));
    ui.painter()
        .circle_filled(rect.center(), d / 2.0, tone_solid(t, tone));
    ui.advance_cursor_after_rect(rect);
    let _ = id;
}

fn tabs_row(ui: &mut egui::Ui, items: &[&str], active: usize, base: usize) -> Option<usize> {
    let t = tokens(ui);
    let mut clicked = None;
    ui.horizontal_wrapped(|ui| {
        for (i, item) in items.iter().enumerate() {
            let global = base + i;
            let on = global == active;
            let text = egui::RichText::new(*item)
                .size(scale::text::BODY)
                .color(if on { t.foreground } else { t.muted_foreground });
            let text = if on { text.strong() } else { text };
            let galley = egui::WidgetText::from(text).into_galley(
                ui,
                Some(egui::TextWrapMode::Extend),
                f32::INFINITY,
                egui::TextStyle::Button,
            );
            let size = egui::vec2(galley.size().x + 26.0, 34.0);
            let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::click());
            ui.painter().galley(
                egui::pos2(
                    rect.center().x - galley.size().x / 2.0,
                    rect.center().y - galley.size().y / 2.0,
                ),
                galley,
                if on || resp.hovered() {
                    t.foreground
                } else {
                    t.muted_foreground
                },
            );
            if on {
                ui.painter().rect_filled(
                    egui::Rect::from_min_max(
                        egui::pos2(rect.min.x + 10.0, rect.max.y - 3.0),
                        egui::pos2(rect.max.x - 10.0, rect.max.y - 1.0),
                    ),
                    egui::CornerRadius::ZERO,
                    t.primary,
                );
            }
            resp.widget_info(|| {
                egui::WidgetInfo::labeled(egui::Role::Tab, true, (*item).to_owned())
            });
            if resp.clicked() {
                clicked = Some(global);
            }
        }
    });
    clicked
}

pub fn tabs(ui: &mut egui::Ui, items: &[&str], active: usize) -> Option<usize> {
    let clicked = tabs_row(ui, items, active, 0);
    divider(ui);
    ui.add_space(scale::space::SP2);
    clicked
}

pub fn filter_bar<R>(
    ui: &mut egui::Ui,
    filters: impl FnOnce(&mut egui::Ui) -> R,
    apply_label: &str,
) -> egui::Response {
    ui.horizontal(|ui| {
        filters(ui);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            button(ui, apply_label, ButtonVariant::Primary)
        })
        .inner
    })
    .inner
}

pub(crate) fn empty_state(ui: &mut egui::Ui, icon: Icon, title: &str, hint: &str) {
    let t = tokens(ui);
    ui.vertical_centered(|ui| {
        ui.add_space(18.0);
        let (id, rect) = ui.allocate_space(egui::vec2(40.0, 40.0));
        icon.paint_at(ui.painter(), rect, t.muted_foreground);
        ui.advance_cursor_after_rect(rect);
        let _ = id;
        ui.add_space(8.0);
        ui.label(
            egui::RichText::new(title)
                .size(15.0)
                .strong()
                .color(t.muted_foreground),
        );
        hint_text(ui, hint);
        ui.add_space(18.0);
    });
}

pub fn td_num(ui: &mut egui::Ui, text: &str) {
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        ui.monospace(egui::RichText::new(text).color(tokens(ui).foreground));
    });
}

pub fn data_table(
    ui: &mut egui::Ui,
    id: &str,
    headers: &[&str],
    row_count: usize,
    row_height: f32,
    row: impl FnMut(&mut egui::Ui, usize),
) {
    let t = tokens(ui);
    let n = headers.len().max(1);
    let avail = ui.available_width();
    let inner = (avail - 2.0).max(60.0);
    const CELL_PAD: f32 = 10.0;
    let cell_area = (inner - 2.0 * CELL_PAD).max(40.0);
    let exact = (cell_area - 16.0 * (n.saturating_sub(1)) as f32) / n as f32;
    let floor_fits = 90.0 * n as f32 + 16.0 * (n.saturating_sub(1)) as f32 <= cell_area;
    let col_w = if floor_fits {
        exact.max(90.0)
    } else {
        exact.max(40.0)
    }
    .floor()
    .max(40.0);
    egui::Frame::new()
        .stroke(egui::Stroke::new(1.0, t.border))
        .corner_radius(egui::CornerRadius::same(10))
        .show(ui, |ui| {
            ui.set_min_width(inner);
            egui::Frame::new()
                .fill(t.sidebar)
                .inner_margin(egui::Margin::symmetric(CELL_PAD as i8, 0))
                .corner_radius(egui::CornerRadius {
                    nw: 9,
                    ne: 9,
                    sw: 0,
                    se: 0,
                })
                .show(ui, |ui| {
                    egui::Grid::new(egui::IdSalt::new(format!("spp-dt-{id}-head")))
                        .num_columns(n)
                        .spacing(egui::vec2(16.0, 0.0))
                        .min_col_width(col_w)
                        .max_col_width(col_w)
                        .min_row_height(row_height)
                        .show(ui, |ui| {
                            for header in headers {
                                ui.label(
                                    egui::RichText::new(header.to_uppercase())
                                        .size(scale::text::META)
                                        .strong()
                                        .color(t.muted_foreground),
                                );
                            }
                            ui.end_row();
                        });
                });
            egui::Frame::new()
                .inner_margin(egui::Margin::symmetric(CELL_PAD as i8, 0))
                .show(ui, |ui| {
                    egui::ScrollArea::vertical()
                        .id_salt(format!("spp-dt-{id}"))
                        .show_rows(ui, row_height, row_count, |ui, range| {
                            egui::Grid::new(egui::IdSalt::new(format!("spp-dt-{id}-grid")))
                                .num_columns(headers.len())
                                .spacing(egui::vec2(16.0, 0.0))
                                .min_row_height(row_height)
                                .min_col_width(col_w)
                                .max_col_width(col_w)
                                .show(ui, |ui| {
                                    let mut row = row;
                                    for i in range {
                                        row(ui, i);
                                        ui.end_row();
                                    }
                                });
                        });
                });
        });
}

pub fn metric_tile(
    ui: &mut egui::Ui,
    count: &str,
    label: &str,
    band: Option<BadgeTone>,
    drill: bool,
    drill_line: Option<&str>,
) -> egui::Response {
    let t = tokens(ui);
    let count_color = band.map(|b| tone_colors_of(t, b).1).unwrap_or(t.foreground);
    let count_g = egui::WidgetText::from(
        egui::RichText::new(count)
            .size(scale::text::STAT)
            .strong()
            .color(count_color),
    )
    .into_galley(
        ui,
        Some(egui::TextWrapMode::Extend),
        f32::INFINITY,
        egui::TextStyle::Body,
    );
    let label_g = egui::WidgetText::from(
        egui::RichText::new(label)
            .size(scale::text::BODY)
            .color(t.foreground),
    )
    .into_galley(
        ui,
        Some(egui::TextWrapMode::Extend),
        f32::INFINITY,
        egui::TextStyle::Body,
    );
    let drill_h = drill_line.map(|l| {
        egui::WidgetText::from(egui::RichText::new(l).small().color(t.muted_foreground))
            .into_galley(
                ui,
                Some(egui::TextWrapMode::Extend),
                f32::INFINITY,
                egui::TextStyle::Small,
            )
            .size()
            .y
    });
    let height = 40.0 + drill_h.unwrap_or(0.0);
    let width = 196.0f32.max(count_g.size().x + label_g.size().x + 12.0 + 10.0 + 12.0 + 12.0);
    let size = egui::vec2(width, height);
    let (id, rect) = ui.allocate_space(size);
    let sense = if drill {
        egui::Sense::click()
    } else {
        egui::Sense::hover()
    };
    let resp = ui.interact(rect, id, sense);
    ui.painter()
        .rect_filled(rect, egui::CornerRadius::same(10), t.card);
    let edge = if drill && resp.hovered() {
        t.border_strong
    } else {
        t.border
    };
    ui.painter().rect_stroke(
        rect,
        egui::CornerRadius::same(10),
        egui::Stroke::new(1.0, edge),
        egui::StrokeKind::Inside,
    );
    if let Some(b) = band {
        ui.painter().rect_filled(
            egui::Rect::from_min_max(rect.min, egui::pos2(rect.min.x + 3.0, rect.max.y)),
            egui::CornerRadius::ZERO,
            tone_solid(t, b),
        );
    }
    let (cw, ch) = (count_g.size().x, count_g.size().y);
    let lh = label_g.size().y;
    let y = rect.center().y - drill_h.map(|h| h / 2.0).unwrap_or(0.0);
    ui.painter().galley(
        egui::pos2(rect.min.x + 12.0, y - ch / 2.0),
        count_g,
        count_color,
    );
    ui.painter().galley(
        egui::pos2(rect.min.x + 12.0 + cw + 10.0, y - lh / 2.0),
        label_g,
        t.foreground,
    );
    if let Some((line, line_h)) = drill_line.zip(drill_h) {
        let line_g =
            egui::WidgetText::from(egui::RichText::new(line).small().color(t.muted_foreground))
                .into_galley(
                    ui,
                    Some(egui::TextWrapMode::Extend),
                    f32::INFINITY,
                    egui::TextStyle::Small,
                );
        ui.painter().galley(
            egui::pos2(rect.min.x + 12.0, rect.max.y - line_h - 6.0),
            line_g,
            t.muted_foreground,
        );
    }
    let resp = if drill {
        resp.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        resp
    };
    resp.widget_info(|| {
        egui::WidgetInfo::labeled(egui::Role::Button, true, format!("{count} {label}"))
    });
    ui.advance_cursor_after_rect(rect);
    resp
}

pub(crate) fn bar_row(ui: &mut egui::Ui, label: &str, value: i64, max: i64, slim: bool) {
    let t = tokens(ui);
    let label_w = if slim { 120.0 } else { 140.0 };
    let value_w = 56.0;
    let gap = 10.0;
    let track_h = 14.0;
    ui.horizontal(|ui| {
        ui.allocate_ui_with_layout(
            egui::vec2(label_w, track_h),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                ui.set_min_width(label_w);
                ui.label(
                    egui::RichText::new(label)
                        .size(scale::text::SMALL_HALF)
                        .color(t.foreground),
                );
            },
        );
        let track_w = (ui.available_width() - value_w - gap).max(40.0);
        let (track, _) = ui.allocate_exact_size(egui::vec2(track_w, track_h), egui::Sense::hover());
        ui.painter()
            .rect_filled(track, egui::CornerRadius::same(7), t.muted);
        let frac = ((value as f64 / max.max(1) as f64) * 100.0).clamp(3.0, 100.0) / 100.0;
        let fill_w = (track.width() as f64 * frac) as f32;
        if fill_w > 0.5 {
            ui.painter().rect_filled(
                egui::Rect::from_min_max(track.min, egui::pos2(track.min.x + fill_w, track.max.y)),
                egui::CornerRadius::same(7),
                t.chart_accent.gamma_multiply(0.8),
            );
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(
                egui::RichText::new(value.to_string())
                    .size(scale::text::SMALL_HALF)
                    .color(t.foreground),
            );
        });
    });
}

pub(crate) fn form_grid<R>(
    ui: &mut egui::Ui,
    id: &str,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<R> {
    egui::Grid::new(egui::IdSalt::new(format!("spp-form-{id}")))
        .num_columns(2)
        .spacing(egui::vec2(16.0, 10.0))
        .show(ui, add_contents)
}

pub(crate) fn field_label(ui: &mut egui::Ui, text: &str) -> egui::Response {
    ui.label(
        egui::RichText::new(text)
            .size(13.0)
            .color(tokens(ui).muted_foreground),
    )
}

pub(crate) fn success_text(ui: &mut egui::Ui, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .small()
            .color(tokens(ui).success_foreground),
    );
}

pub(crate) fn error_text(ui: &mut egui::Ui, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .small()
            .color(tokens(ui).destructive_text),
    );
}

pub(crate) fn divider(ui: &mut egui::Ui) {
    let t = tokens(ui);
    let width = ui.available_width();
    let (id, rect) = ui.allocate_space(egui::vec2(width, 1.0));
    ui.painter()
        .rect_filled(rect, egui::CornerRadius::ZERO, t.border);
    let _ = id;
}

pub fn empty_state_slim(ui: &mut egui::Ui, icon: Icon, title: &str, hint: &str) {
    let t = tokens(ui);
    ui.vertical_centered(|ui| {
        ui.add_space(scale::space::SP3);
        icon.show(ui, 20.0, t.muted_foreground);
        ui.label(
            egui::RichText::new(title)
                .size(scale::text::SECTION)
                .strong()
                .color(t.foreground),
        );
        hint_text(ui, hint);
        ui.add_space(scale::space::SP2);
    });
}

pub fn empty_state_cta(
    ui: &mut egui::Ui,
    icon: Icon,
    title: &str,
    hint: &str,
    cta_label: &str,
) -> bool {
    empty_state_cta_with(ui, icon, title, hint, Icon::Plus, cta_label)
}

pub fn empty_state_cta_with(
    ui: &mut egui::Ui,
    icon: Icon,
    title: &str,
    hint: &str,
    cta_icon: Icon,
    cta_label: &str,
) -> bool {
    let mut clicked = false;
    ui.vertical_centered(|ui| {
        ui.add_space(scale::space::SP4);
        let t = tokens(ui);
        let (id, rect) = ui.allocate_space(egui::vec2(38.0, 38.0));
        icon.paint_at(ui.painter(), rect, t.muted_foreground);
        ui.advance_cursor_after_rect(rect);
        let _ = id;
        ui.add_space(scale::space::SP2);
        ui.label(
            egui::RichText::new(title)
                .size(scale::text::SECTION)
                .strong()
                .color(t.foreground),
        );
        hint_text(ui, hint);
        ui.add_space(scale::space::SP3);
        clicked = button_with_icon(ui, cta_icon, cta_label, ButtonVariant::Primary).clicked();
        ui.add_space(scale::space::SP4);
    });
    clicked
}

pub fn callout(ui: &mut egui::Ui, role: CalloutRole, text: &str) {
    let t = tokens(ui);
    let (bg, fg, border) = match role {
        CalloutRole::Neutral => (t.muted, t.muted_foreground, None),
        CalloutRole::Warn => (t.warning_soft, t.warning_foreground, Some(t.warning_border)),
        CalloutRole::Err => (t.destructive_soft, t.destructive_text, None),
    };
    egui::Frame::new()
        .fill(bg)
        .corner_radius(egui::CornerRadius::same(8))
        .inner_margin(egui::Margin::same(10))
        .show(ui, |ui| {
            if let Some(b) = border {
                ui.painter().rect_stroke(
                    ui.max_rect(),
                    egui::CornerRadius::same(8),
                    egui::Stroke::new(1.0, b),
                    egui::StrokeKind::Inside,
                );
            }
            ui.label(
                egui::RichText::new(text)
                    .size(scale::text::SMALL_HALF)
                    .color(fg),
            );
        });
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalloutRole {
    Neutral,
    Warn,
    Err,
}

pub fn status_line(ui: &mut egui::Ui, ok: bool, text: &str) {
    let t = tokens(ui);
    ui.label(
        egui::RichText::new(text)
            .size(scale::text::SMALL_HALF)
            .color(if ok {
                t.success_foreground
            } else {
                t.destructive_text
            }),
    );
}

pub fn quirk(ui: &mut egui::Ui, text: &str) {
    let t = tokens(ui);
    egui::Frame::new()
        .fill(t.muted.gamma_multiply(0.55))
        .corner_radius(egui::CornerRadius::same(8))
        .inner_margin(egui::Margin::same(10))
        .show(ui, |ui| {
            ui.painter().rect_stroke(
                ui.max_rect(),
                egui::CornerRadius::same(8),
                egui::Stroke::new(1.0, t.border_strong),
                egui::StrokeKind::Inside,
            );
            ui.label(
                egui::RichText::new(text)
                    .size(scale::text::SMALL)
                    .color(t.muted_foreground),
            );
        });
}

pub fn avatar(ui: &mut egui::Ui, size: AvatarSize, initials: &str, role: BadgeTone) {
    let t = tokens(ui);
    let d = match size {
        AvatarSize::Lg => 34.0,
        AvatarSize::Md => 28.0,
        AvatarSize::Sm => 16.0,
    };
    let (bg, fg) = match (size, role) {
        (AvatarSize::Lg, _) | (AvatarSize::Md, BadgeTone::Neutral) => (t.muted, t.muted_foreground),
        (AvatarSize::Sm, _) => (t.primary, t.primary_foreground),
        (_, BadgeTone::Ai) => (t.ai, t.on_ai),
        (_, BadgeTone::Warning) => (t.warning_soft, t.warning_foreground),
        (_, BadgeTone::Info) => (t.info_soft, t.info_foreground),
        (_, BadgeTone::Success) => (t.success_soft, t.success_foreground),
        (_, BadgeTone::Destructive) => (t.destructive_soft, t.destructive_text),
    };
    let (rect, _id) = ui.allocate_exact_size(egui::vec2(d, d), egui::Sense::hover());
    ui.painter().circle_filled(rect.center(), d / 2.0, bg);
    let font = egui::FontId::proportional(match size {
        AvatarSize::Lg => 12.0,
        AvatarSize::Md => 10.5,
        AvatarSize::Sm => 8.5,
    });
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        initials,
        font,
        fg,
    );
    ui.advance_cursor_after_rect(rect);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AvatarSize {
    Lg,
    Md,
    Sm,
}

pub fn assignee(ui: &mut egui::Ui, name: Option<&str>) {
    match name {
        Some(n) => {
            let initials: String = n
                .split_whitespace()
                .filter_map(|w| w.chars().next())
                .take(2)
                .collect();
            avatar(
                ui,
                AvatarSize::Sm,
                &initials.to_uppercase(),
                BadgeTone::Neutral,
            );
            ui.label(
                egui::RichText::new(n)
                    .size(scale::text::META_HALF)
                    .color(tokens(ui).muted_foreground),
            );
        }
        None => {
            ui.label(
                egui::RichText::new("Unassigned")
                    .size(scale::text::META_HALF)
                    .italics()
                    .color(tokens(ui).muted_foreground),
            );
        }
    }
}

pub fn armed_danger_button(ui: &mut egui::Ui, id: &str, label: &str, confirm_label: &str) -> bool {
    let armed_id = egui::Id::unique(("spp-armed", id));
    let armed_frame = ui.ctx().data(|d| d.get_temp::<u64>(armed_id)).unwrap_or(0);
    let armed = armed_frame > 0;
    if armed {
        let settle = ui.ctx().cumulative_frame_nr() > armed_frame + 1;
        let confirm = settle && button(ui, confirm_label, ButtonVariant::Destructive).clicked();
        let esc = ui.input(|i| i.key_pressed(egui::Key::Escape));
        if esc || confirm {
            ui.ctx().data_mut(|d| d.insert_temp(armed_id, 0_u64));
        }
        return confirm;
    }
    if button(ui, label, ButtonVariant::Ghost).clicked() {
        let frame = ui.ctx().cumulative_frame_nr();
        ui.ctx().data_mut(|d| d.insert_temp(armed_id, frame));
    }
    false
}

pub(crate) fn detail_panel(
    ctx: &egui::Context,
    id: &str,
    open: &mut bool,
    title: &str,
    header: impl FnOnce(&mut egui::Ui),
    body: impl FnOnce(&mut egui::Ui),
) {
    if !*open {
        return;
    }
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        *open = false;
        return;
    }
    let t = tokens_for(ctx);
    let screen = ctx.input(|i| i.viewport_rect());
    let scrim = egui::Area::new(egui::Id::unique(("spp-scrim", id)))
        .order(egui::Order::Middle)
        .anchor(egui::Align2::LEFT_TOP, egui::vec2(0.0, 0.0))
        .show(ctx, |ui| {
            ui.painter()
                .rect_filled(screen, egui::CornerRadius::ZERO, t.scrim);
            let resp = ui.interact(
                screen,
                egui::Id::unique(("spp-scrim-hit", id)),
                egui::Sense::click(),
            );
            if resp.clicked() {
                *open = false;
            }
        });
    let _ = scrim;
    let width = 420.0_f32.min(screen.width() * 0.94);
    egui::Area::new(egui::Id::unique(("spp-panel", id)))
        .order(egui::Order::Foreground)
        .anchor(egui::Align2::RIGHT_TOP, egui::vec2(0.0, 0.0))
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(t.card)
                .stroke(egui::Stroke::new(1.0, t.border))
                .corner_radius(egui::CornerRadius::ZERO)
                .show(ui, |ui| {
                    ui.set_width(width);
                    ui.set_min_height(screen.height());
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            ui.add_space(scale::space::SP4);
                            ui.label(egui::RichText::new(title).size(scale::text::LEAD).strong());
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    header(ui);
                                    if icon_button(ui, Icon::X, ButtonVariant::Ghost, "close panel")
                                        .clicked()
                                    {
                                        *open = false;
                                    }
                                    ui.add_space(scale::space::SP3);
                                },
                            );
                        });
                        divider(ui);
                        egui::ScrollArea::vertical()
                            .id_salt(format!("spp-panel-{id}"))
                            .show(ui, |ui| {
                                ui.add_space(scale::space::SP3);
                                ui.add_space(scale::space::SP1);
                                ui.horizontal(|ui| {
                                    ui.add_space(scale::space::SP4);
                                    ui.vertical(|ui| {
                                        ui.set_width(width - 32.0);
                                        body(ui);
                                    });
                                    ui.add_space(scale::space::SP4);
                                });
                            });
                    });
                });
        });
}

pub fn form_row(ui: &mut egui::Ui, label: &str, control: impl FnOnce(&mut egui::Ui)) {
    ui.horizontal(|ui| {
        ui.set_min_width(ui.available_width());
        ui.label(
            egui::RichText::new(label)
                .size(scale::text::BODY)
                .color(tokens(ui).muted_foreground),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            control(ui);
        });
    });
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageRole {
    Customer,
    Agent,
    Note,
    Ai,
}

pub fn message_card_with<B, A, R>(
    ui: &mut egui::Ui,
    role: MessageRole,
    name: &str,
    role_label: Option<&str>,
    time: &str,
    body: B,
    actions: A,
) where
    B: FnOnce(&mut egui::Ui),
    A: FnOnce(&mut egui::Ui) -> R,
{
    let t = tokens(ui);
    let (bg, border, accent) = match role {
        MessageRole::Customer => (
            t.card,
            if ui.visuals().dark_mode {
                Some(t.border)
            } else {
                None
            },
            None,
        ),
        MessageRole::Agent => (t.agent_surface, Some(t.agent_border), None),
        MessageRole::Note => (t.warning_soft, None, Some(t.warning)),
        MessageRole::Ai => (t.ai_soft, None, Some(t.ai)),
    };
    let frame = egui::Frame::new()
        .fill(bg)
        .stroke(border.map_or(egui::Stroke::NONE, |c| egui::Stroke::new(1.0, c)))
        .corner_radius(egui::CornerRadius::same(scale::radius::CARD))
        .inner_margin(egui::Margin {
            left: 14,
            right: 14,
            top: 11,
            bottom: 12,
        });
    let resp = frame
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                let (av_bg, av_fg) = match role {
                    MessageRole::Customer => (t.muted, t.muted_foreground),
                    MessageRole::Agent => (t.primary, t.primary_foreground),
                    MessageRole::Note => (t.card, t.warning_foreground),
                    MessageRole::Ai => (t.ai, t.on_ai),
                };
                let (rect, _id) =
                    ui.allocate_exact_size(egui::vec2(24.0, 24.0), egui::Sense::hover());
                ui.painter().circle_filled(rect.center(), 12.0, av_bg);
                let initials: String = name
                    .split_whitespace()
                    .filter_map(|w| w.chars().next())
                    .take(2)
                    .collect();
                ui.painter().text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    initials.to_uppercase(),
                    egui::FontId::proportional(9.5),
                    av_fg,
                );
                ui.label(egui::RichText::new(name).size(scale::text::BODY).strong());
                let role_label = role_label.unwrap_or(match role {
                    MessageRole::Customer => "Customer",
                    MessageRole::Agent => "Agent",
                    MessageRole::Note => "Internal note",
                    MessageRole::Ai => "AI",
                });
                let role_color = match role {
                    MessageRole::Ai => t.ai_foreground,
                    MessageRole::Note => t.warning_foreground,
                    _ => t.muted_foreground,
                };
                ui.label(
                    egui::RichText::new(role_label)
                        .size(scale::text::META)
                        .color(role_color),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        egui::RichText::new(time)
                            .size(scale::text::META)
                            .color(t.muted_foreground),
                    );
                });
            });
            ui.add_space(scale::space::SP2);
            body(ui);
            ui.add_space(scale::space::SP2);
            actions(ui);
        })
        .response;
    if let Some(accent) = accent {
        let r = resp.rect;
        ui.painter().rect_filled(
            egui::Rect::from_min_max(
                egui::pos2(r.min.x, r.min.y),
                egui::pos2(r.min.x + 3.0, r.max.y),
            ),
            egui::CornerRadius {
                nw: scale::radius::CARD,
                sw: scale::radius::CARD,
                ne: 0,
                se: 0,
            },
            accent,
        );
    }
}

pub fn day_divider(ui: &mut egui::Ui, label: &str) {
    let t = tokens(ui);
    ui.horizontal(|ui| {
        let w = ui.available_width();
        let half = (w - 120.0) / 2.0;
        let (r1, _id) = ui.allocate_exact_size(egui::vec2(half, 1.0), egui::Sense::hover());
        ui.painter()
            .line_segment([r1.min, r1.max], egui::Stroke::new(1.0, t.border_strong));
        egui::Frame::new()
            .fill(t.muted)
            .corner_radius(egui::CornerRadius::same(scale::radius::BADGE))
            .inner_margin(egui::Margin::symmetric(
                scale::space::SP2 as i8,
                scale::space::SP1 as i8,
            ))
            .show(ui, |ui| {
                ui.label(
                    egui::RichText::new(label)
                        .size(scale::text::SMALL)
                        .strong()
                        .color(t.muted_foreground),
                );
            });
        let (r2, _id2) = ui.allocate_exact_size(egui::vec2(half, 1.0), egui::Sense::hover());
        ui.painter()
            .line_segment([r2.min, r2.max], egui::Stroke::new(1.0, t.border_strong));
    });
}

pub fn list_end_note(ui: &mut egui::Ui, text: &str) {
    let t = tokens(ui);
    ui.add_space(scale::space::SP3);
    let w = ui.available_width();
    let (id, rect) = ui.allocate_space(egui::vec2(w, 1.0));
    ui.painter()
        .rect_filled(rect, egui::CornerRadius::ZERO, t.border_strong);
    let _ = id;
    ui.add_space(scale::space::SP1);
    ui.vertical_centered(|ui| {
        ui.label(
            egui::RichText::new(text)
                .size(scale::text::META)
                .color(t.muted_foreground),
        );
    });
    ui.add_space(scale::space::SP2);
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ComposerMode {
    #[default]
    Reply,
    Note,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ComposerAct {
    Send(ComposerMode, String),
    SavedReply(usize),
    AiInsert,
}

#[allow(clippy::too_many_lines)]
pub fn composer(
    ui: &mut egui::Ui,
    id_salt: &str,
    state: &mut ComposerState,
    saved_replies: &[String],
    before_body: impl FnOnce(&mut egui::Ui),
    after_body: impl FnOnce(&mut egui::Ui),
) -> Option<ComposerAct> {
    use egui::RichText as R;
    let t = tokens(ui);
    let mut act = None;
    let note = state.mode == ComposerMode::Note;

    card(ui, |ui| {
        ui.set_width(ui.available_width());

        ui.horizontal(|ui| {
            let mode_idx = usize::from(note);
            if let Some(i) = segmented(
                ui,
                &format!("composer-mode-{id_salt}"),
                &["Reply", "Note"],
                mode_idx,
            ) {
                state.mode = if i == 0 {
                    ComposerMode::Reply
                } else {
                    ComposerMode::Note
                };
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if state.draft_saved {
                    ui.label(
                        R::new("Draft saved")
                            .size(scale::text::META)
                            .color(t.muted_foreground),
                    );
                }
                if button_with_icon(ui, Icon::Sparkles, "AI Draft", ButtonVariant::Outline)
                    .on_hover_text("insert the latest LM Studio draft for this conversation")
                    .clicked()
                {
                    act = Some(ComposerAct::AiInsert);
                }
                let sr = button_with_icon(ui, Icon::Mail, "Saved Replies", ButtonVariant::Outline)
                    .on_hover_text("pick a saved reply template");
                let popup_id = ui.make_persistent_id(("spp-sr-menu", id_salt));
                egui::Popup::menu(&sr)
                    .id(popup_id)
                    .width(280.0)
                    .frame(
                        egui::Frame::new()
                            .fill(t.card)
                            .stroke(egui::Stroke::new(1.0, t.border))
                            .corner_radius(egui::CornerRadius::same(10))
                            .inner_margin(egui::Margin::same(4))
                            .shadow(egui::Shadow {
                                offset: [0, 8],
                                blur: 24,
                                spread: 0,
                                color: egui::Color32::from_black_alpha(60),
                            }),
                    )
                    .show(|ui| {
                        ui.set_min_width(270.0);
                        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                        if saved_replies.is_empty() {
                            ui.label(
                                R::new("No saved replies yet")
                                    .size(scale::text::SMALL_HALF)
                                    .color(t.muted_foreground),
                            );
                        }
                        for (i, reply) in saved_replies.iter().enumerate() {
                            if ui
                                .button(R::new(reply).size(scale::text::SMALL_HALF))
                                .clicked()
                            {
                                act = Some(ComposerAct::SavedReply(i));
                            }
                        }
                    });
            });
        });
        ui.add_space(scale::space::SP2);
        divider(ui);

        before_body(ui);

        let edit = egui::TextEdit::multiline(&mut state.draft)
            .id_salt(format!("spp-composer-{id_salt}"))
            .hint_text(if note {
                "add an internal note…"
            } else {
                "write a reply…"
            })
            .desired_width(ui.available_width())
            .min_size(egui::vec2(0.0, 96.0))
            .font(egui::TextStyle::Body);
        let edit = if note {
            edit.background_color(t.warning_soft)
        } else {
            edit
        };
        let out = edit.show(ui);
        state.focused = out.response.has_focus();
        state.caret = out.cursor_range.map(|range| range.primary.index.0);

        after_body(ui);

        ui.horizontal(|ui| {
            let context = if note {
                "note — visible to teammates only · Ctrl+Enter sends"
            } else {
                "reply — sent to the customer · Ctrl+Enter sends"
            };
            ui.add(
                egui::Label::new(
                    R::new(context)
                        .size(scale::text::META)
                        .color(t.muted_foreground),
                )
                .truncate(),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let send_label = if note { "Send Note" } else { "Send" };
                let send = if note {
                    let g = egui::WidgetText::from(
                        R::new(send_label).color(t.on_ai).size(scale::text::CONTROL),
                    )
                    .into_galley(
                        ui,
                        Some(egui::TextWrapMode::Extend),
                        f32::INFINITY,
                        egui::TextStyle::Button,
                    );
                    let size = egui::vec2(g.size().x + 28.0, 32.0);
                    let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::click());
                    ui.painter().rect_filled(
                        rect,
                        egui::CornerRadius::same(scale::radius::CONTROL),
                        t.warning,
                    );
                    ui.painter().galley(
                        egui::pos2(
                            rect.center().x - g.size().x / 2.0,
                            rect.center().y - g.size().y / 2.0,
                        ),
                        g,
                        t.on_ai,
                    );
                    resp.widget_info(|| {
                        egui::WidgetInfo::labeled(egui::Role::Button, true, send_label.to_owned())
                    });
                    resp
                } else {
                    button_with_icon(ui, Icon::Send, send_label, ButtonVariant::Primary)
                };
                let ctrl_enter = ui.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::Enter));
                if send.clicked() || ctrl_enter {
                    act = Some(ComposerAct::Send(state.mode, state.draft.clone()));
                    state.draft.clear();
                    state.draft_saved = false;
                }
            });
        });
    });
    act
}

#[derive(Debug, Clone, Default)]
pub struct ComposerState {
    pub mode: ComposerMode,
    pub draft: String,
    pub draft_saved: bool,
    pub caret: Option<usize>,
    pub focused: bool,
}

impl ComposerState {
    pub fn new() -> Self {
        Self::default()
    }
}

pub fn cop_suggest(
    ui: &mut egui::Ui,
    title: &str,
    bullets: &[String],
    actions: &[&str],
) -> Option<usize> {
    let t = tokens(ui);
    let mut clicked = None;
    let resp = egui::Frame::new()
        .fill(t.ai_soft)
        .corner_radius(egui::CornerRadius::same(scale::radius::CARD))
        .inner_margin(egui::Margin::same(12))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                badge(ui, "AI", BadgeTone::Ai);
                ui.label(
                    egui::RichText::new(title)
                        .size(scale::text::META_HALF)
                        .strong()
                        .color(t.ai_foreground),
                );
            });
            ui.add_space(scale::space::SP2 - 2.0);
            for bullet in bullets {
                ui.horizontal(|ui| {
                    ui.painter().circle_filled(
                        egui::pos2(ui.min_rect().left() + 4.0, ui.cursor().top() + 8.0),
                        2.0,
                        t.border_strong,
                    );
                    ui.add_space(12.0);
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(spp_core::richtext::break_long_tokens(bullet, 40))
                                .size(scale::text::SMALL_HALF)
                                .color(t.foreground),
                        )
                        .wrap(),
                    );
                });
            }
            if !actions.is_empty() {
                ui.add_space(scale::space::SP2 - 2.0);
                ui.horizontal(|ui| {
                    for (i, a) in actions.iter().enumerate() {
                        if button(ui, a, ButtonVariant::Outline).clicked() {
                            clicked = Some(i);
                        }
                    }
                });
            }
        })
        .response;
    let r = resp.rect;
    ui.painter().rect_filled(
        egui::Rect::from_min_max(
            egui::pos2(r.min.x, r.min.y),
            egui::pos2(r.min.x + 3.0, r.max.y),
        ),
        egui::CornerRadius {
            nw: scale::radius::CARD,
            sw: scale::radius::CARD,
            ne: 0,
            se: 0,
        },
        t.ai,
    );
    clicked
}

pub fn cop_chat(ui: &mut egui::Ui, assistant: bool, text: &str) {
    let t = tokens(ui);
    let (bg, border) = if assistant {
        (t.ai_soft, t.ai_border)
    } else {
        (t.muted, t.border)
    };
    let align_right = !assistant;
    ui.with_layout(
        if align_right {
            egui::Layout::right_to_left(egui::Align::TOP)
        } else {
            egui::Layout::left_to_right(egui::Align::TOP)
        },
        |ui| {
            let galley = egui::WidgetText::from(egui::RichText::new(text).size(scale::text::BODY))
                .into_galley(
                    ui,
                    Some(egui::TextWrapMode::Wrap),
                    320.0,
                    egui::TextStyle::Body,
                );
            let size = galley.size() + egui::vec2(24.0, 18.0);
            let (rect, _id) = ui.allocate_exact_size(size, egui::Sense::hover());
            ui.painter()
                .rect_filled(rect, egui::CornerRadius::same(12), bg);
            if assistant {
                ui.painter().rect_stroke(
                    rect,
                    egui::CornerRadius::same(12),
                    egui::Stroke::new(1.0, border),
                    egui::StrokeKind::Inside,
                );
            }
            ui.painter().galley(
                egui::pos2(rect.min.x + 12.0, rect.min.y + 9.0),
                galley,
                t.foreground,
            );
        },
    );
}

#[cfg(test)]
mod tests {
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
}
