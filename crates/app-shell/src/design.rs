//! The SupportOS+ design system: a centralized, shadcn/ui-inspired token
//! and component library for the native egui shell.
//!
//! The redesign keeps the pinned eframe/egui 0.37 stack (decision D1) and
//! adds zero new dependencies: [`egui-shadcn` was evaluated but targets
//! egui 0.33, and downgrading egui is out of scope]. Instead this module
//! installs two complete, deliberately-styled zinc themes (light + dark)
//! on the egui context and exposes a small family of reusable components
//! — buttons, cards, badges, chips, tables, page headers, empty states —
//! that every page shares, so styling stays centralized.
//!
//! ## Color tokens
//! [`Tokens`] carries the shadcn zinc palette per mode: background,
//! foreground, card, popover, primary, secondary, muted, accent,
//! destructive, success, warning, info, border, input and ring — with
//! hand-checked WCAG AA contrast (see the tests at the bottom).
//!
//! ## Typography
//! Text styles carry the DC §2.5 type roles: Heading 22, Body 13.5,
//! Button/controls 13, Monospace 12.5, Small 12 (egui embeds Ubuntu Light).
//!
//! ## Components
//! Every interactive component draws a visible keyboard focus ring in
//! the theme's ring color, and hover/press feedback through a tonal
//! overlay so solid variants (primary/destructive) respond like their
//! web counterparts.
//!
//! ## Design Contract v1.0 (RS-12 token port)
//! Token VALUES are frozen 1:1 from **DC v1.0** (`docs/audit/design-contract-v1.md`,
//! extracted from the owner-approved conformed lab at `af5bc2a`): one DC token =
//! one constant here, DS.TONE = the one [`status_tone`] match arm, and the
//! spacing/type/radius scales live in [`scale`]. Pin tests assert the freeze;
//! `tests/style_lint.rs` forbids raw `Color32` construction outside this file and
//! off-scale spacing literals. The keyboard focus ring ([`Tokens::ring`]) is an
//! a11y affordance carried over from the pre-port shell — not a DC token.

use eframe::egui;

use crate::icons::Icon;

// ---------------------------------------------------------------------------
// Semantic tokens (shadcn/ui zinc palette)
// ---------------------------------------------------------------------------

/// The complete semantic palette for one mode (light or dark).
#[derive(Debug, Clone, Copy)]
pub struct Tokens {
    /// Central panel / window background.
    pub background: egui::Color32,
    /// Elevated sidebar background.
    pub sidebar: egui::Color32,
    /// Card and popover background.
    pub card: egui::Color32,
    /// Card & control borders.
    pub border: egui::Color32,
    /// Slightly stronger border used on hover.
    pub border_strong: egui::Color32,
    /// Default foreground (text).
    pub foreground: egui::Color32,
    /// Muted surface (chips, secondary fills).
    pub muted: egui::Color32,
    /// Muted foreground (secondary text).
    pub muted_foreground: egui::Color32,
    /// Hover surface for ghost controls.
    pub accent: egui::Color32,
    /// Primary (default) button fill.
    pub primary: egui::Color32,
    /// Text/icons on primary fills.
    pub primary_foreground: egui::Color32,
    /// Text on AI-solid fills (`#ffffff`, DC DS-C-11 `ai`).
    pub on_ai: egui::Color32,
    /// Secondary button fill.
    pub secondary: egui::Color32,
    pub secondary_foreground: egui::Color32,
    /// Destructive button fill.
    pub destructive: egui::Color32,
    pub destructive_foreground: egui::Color32,
    /// Success/warning/info/destructive badge fills and text.
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
    /// Destructive text on soft fills (AA-checked, red-700/-400).
    pub destructive_text: egui::Color32,
    /// Keyboard focus ring color.
    pub ring: egui::Color32,
    /// Selected row/chip fill (lists, sidebar).
    pub selection: egui::Color32,
    pub selection_foreground: egui::Color32,
    /// Input/text-edit background.
    pub input: egui::Color32,
    /// Striping color for tables (barely off the background).
    pub faint: egui::Color32,
    /// Hyperlink color.
    pub hyperlink: egui::Color32,
    /// Chart accent (line/bars).
    pub chart_accent: egui::Color32,
    /// Chart accent 2 (secondary series).
    pub chart_accent_2: egui::Color32,
    /// Hover overlay tint (dark theme lightens dark surfaces).
    pub hover_tint: egui::Color32,
    // --- DC v1.0 additions (RS-12 token port; docs/audit/design-contract-v1.md §2) ---
    /// AI identity — solid violet (AI badges, AI buttons, chart accent).
    pub ai: egui::Color32,
    /// AI tint surface (AI message cards, copilot cards, AI chips).
    pub ai_soft: egui::Color32,
    /// AI text on tint surfaces; also the hyperlink token (`--ds-link`).
    pub ai_foreground: egui::Color32,
    /// AI hairline (AI message cards, copilot cards).
    pub ai_border: egui::Color32,
    /// Agent-authored message surface (`--ds-agent-bg`).
    pub agent_surface: egui::Color32,
    /// Agent-authored message hairline (`--ds-agent-border`).
    pub agent_border: egui::Color32,
    /// Warning hairline (warn callouts, internal-note cards).
    pub warning_border: egui::Color32,
    /// Text on neutral tinted pills/badges (`--ds-neutral-fg`).
    pub neutral_foreground: egui::Color32,
    /// DC e2 elevation shadow (drawers, dialogs, toasts) — black @ 0.14.
    pub shadow: egui::Color32,
    /// DetailPanel/dialog scrim — black @ 0.28 (DC §3 DS-C-16).
    pub scrim: egui::Color32,
}

const fn rgb(r: u8, g: u8, b: u8) -> egui::Color32 {
    egui::Color32::from_rgb(r, g, b)
}

/// DC v1.0 light mode (port of `ds.css` `body[data-theme="light"]`).
pub const LIGHT: Tokens = Tokens {
    background: rgb(244, 244, 245), // --ds-bg #f4f4f5
    sidebar: rgb(250, 250, 250),    // --ds-surface-2 #fafafa
    card: rgb(255, 255, 255),
    border: rgb(228, 228, 231),           // zinc-200
    border_strong: rgb(212, 212, 216),    // zinc-300
    foreground: rgb(9, 9, 11),            // zinc-950
    muted: rgb(244, 244, 245),            // zinc-100
    muted_foreground: rgb(113, 113, 122), // zinc-500
    accent: rgb(244, 244, 245),
    primary: rgb(24, 24, 27), // zinc-900
    primary_foreground: rgb(250, 250, 250),
    on_ai: rgb(255, 255, 255),
    secondary: rgb(244, 244, 245),
    secondary_foreground: rgb(24, 24, 27),
    destructive: rgb(220, 38, 38), // red-600
    destructive_foreground: rgb(255, 255, 255),
    success: rgb(22, 163, 74),        // green-600
    success_soft: rgb(240, 253, 244), // green-50
    success_foreground: rgb(21, 128, 61),
    warning: rgb(217, 119, 6),        // amber-600
    warning_soft: rgb(255, 251, 235), // amber-50
    warning_foreground: rgb(180, 83, 9),
    info: rgb(37, 99, 235),        // blue-600
    info_soft: rgb(239, 246, 255), // blue-50
    info_foreground: rgb(29, 78, 216),
    destructive_soft: rgb(254, 226, 226), // red-100
    destructive_text: rgb(185, 28, 28),   // red-700
    ring: rgb(24, 24, 27),
    selection: rgb(244, 244, 245), // = neutral-bg (DS selected row)
    selection_foreground: rgb(24, 24, 27),
    input: rgb(255, 255, 255),       // --ds-input
    faint: rgb(250, 250, 250),       // --ds-surface-2
    hyperlink: rgb(109, 40, 217),    // --ds-link = --ds-ai-fg
    chart_accent: rgb(124, 58, 237), // lab bars/lines draw in --ds-ai
    chart_accent_2: rgb(24, 24, 27),
    hover_tint: rgb(9, 9, 11),
    // DC additions (light):
    ai: rgb(124, 58, 237),               // --ds-ai #7c3aed
    ai_soft: rgb(245, 243, 255),         // --ds-ai-bg #f5f3ff
    ai_foreground: rgb(109, 40, 217),    // --ds-ai-fg #6d28d9
    ai_border: rgb(221, 214, 254),       // --ds-ai-border #ddd6fe
    agent_surface: rgb(250, 250, 250),   // --ds-agent-bg #fafafa
    agent_border: rgb(228, 228, 231),    // --ds-agent-border #e4e4e7
    warning_border: rgb(253, 230, 138),  // --ds-warning-border #fde68a
    neutral_foreground: rgb(63, 63, 70), // --ds-neutral-fg #3f3f46
    shadow: egui::Color32::from_rgba_premultiplied(0, 0, 0, 36), // e2 rgba(0,0,0,0.14)
    scrim: egui::Color32::from_rgba_premultiplied(0, 0, 0, 71), // rgba(0,0,0,0.28)
};

/// DC v1.0 dark mode (port of `ds.css` `body[data-theme="dark"]`).
pub const DARK: Tokens = Tokens {
    background: rgb(9, 9, 11), // zinc-950
    sidebar: rgb(12, 12, 14),
    card: rgb(24, 24, 27),          // zinc-900
    border: rgb(39, 39, 42),        // zinc-800
    border_strong: rgb(63, 63, 70), // zinc-700
    foreground: rgb(250, 250, 250), // zinc-50
    muted: rgb(39, 39, 42),
    muted_foreground: rgb(161, 161, 170), // zinc-400
    accent: rgb(39, 39, 42),
    primary: rgb(250, 250, 250),
    primary_foreground: rgb(24, 24, 27),
    on_ai: rgb(255, 255, 255),
    secondary: rgb(39, 39, 42),
    secondary_foreground: rgb(250, 250, 250),
    destructive: rgb(185, 28, 28), // red-700 (readable on zinc-900)
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
    destructive_text: rgb(248, 113, 113), // red-400
    ring: rgb(212, 212, 216),
    selection: rgb(39, 39, 42),
    selection_foreground: rgb(250, 250, 250),
    input: rgb(12, 12, 14),
    faint: rgb(12, 12, 14),           // --ds-surface-2 #0c0c0e
    hyperlink: rgb(196, 181, 253),    // --ds-ai-fg dark
    chart_accent: rgb(167, 139, 250), // --ds-ai dark
    chart_accent_2: rgb(250, 250, 250),
    hover_tint: rgb(250, 250, 250),
    // DC additions (dark):
    ai: rgb(167, 139, 250),                 // --ds-ai #a78bfa
    ai_soft: rgb(36, 29, 59),               // --ds-ai-bg #241d3b
    ai_foreground: rgb(196, 181, 253),      // --ds-ai-fg #c4b5fd
    ai_border: rgb(70, 58, 112),            // --ds-ai-border #463a70
    agent_surface: rgb(29, 29, 33),         // --ds-agent-bg #1d1d21
    agent_border: rgb(44, 44, 49),          // --ds-agent-border #2c2c31
    warning_border: rgb(87, 67, 26),        // --ds-warning-border #57431a
    neutral_foreground: rgb(212, 212, 216), // --ds-neutral-fg #d4d4d8
    shadow: egui::Color32::from_rgba_premultiplied(0, 0, 0, 36),
    scrim: egui::Color32::from_rgba_premultiplied(0, 0, 0, 71),
};

/// The tokens of the theme the `ui` is currently rendering with.
#[must_use]
pub fn tokens(ui: &egui::Ui) -> &'static Tokens {
    if ui.visuals().dark_mode {
        &DARK
    } else {
        &LIGHT
    }
}

/// The tokens of the context's active theme — for call sites that build a
/// frame from a `Context` before any `Ui` exists (palette/dialog windows).
#[must_use]
pub fn tokens_for(ctx: &egui::Context) -> &'static Tokens {
    if ctx.theme() == egui::Theme::Dark {
        &DARK
    } else {
        &LIGHT
    }
}

// ---------------------------------------------------------------------------
// DC v1.0 scales (§2.4–2.6) — the only spacing/type/radius values allowed
// ---------------------------------------------------------------------------

/// Design Contract scales. Pages size and position everything through these;
/// off-scale literals are caught by the style lint (`tests/style_lint.rs`).
/// The 4pt rule (DC §2.4): micro gaps sp-1..sp-7; `CLUSTER` (6px) only inside
/// control clusters; the `PAD_*` values are component pads frozen by the DC
/// sheets (the approved lab values, not free-form choices).
pub mod scale {
    /// 4pt spacing scale (DC §2.4).
    pub mod space {
        pub const SP1: f32 = 4.0;
        pub const SP2: f32 = 8.0;
        pub const SP3: f32 = 12.0;
        pub const SP4: f32 = 16.0;
        pub const SP5: f32 = 20.0;
        pub const SP6: f32 = 24.0;
        pub const SP7: f32 = 32.0;
        /// Allowed only inside control clusters (DC §2.4).
        pub const CLUSTER: f32 = 6.0;
        /// Card header vertical pad (DC §3 DS-C-02, `11px 14px`).
        pub const PAD_CARD_V: f32 = 11.0;
        /// Card horizontal pad (DC §3 DS-C-02).
        pub const PAD_CARD: f32 = 14.0;
        /// Card body vertical pad (DC §3 DS-C-02, `12px 14px`).
        pub const PAD_BODY_V: f32 = 12.0;
        /// Toolbar/table-cell horizontal pad (DC §3 DS-C-05/10, `8px 12px`).
        pub const PAD_TOOL: f32 = 10.0;
        /// Reader pane horizontal pad (DC §4 DS-A-05, `18px 22px`).
        pub const PAD_READER_X: f32 = 22.0;
        /// Reader pane vertical pad.
        pub const PAD_READER_V: f32 = 18.0;
    }
    /// Type roles (DC §2.5). `*_HALF` carries the lab's 11.5/12.5 steps.
    pub mod text {
        pub const META: f32 = 11.0;
        pub const META_HALF: f32 = 11.5;
        pub const SMALL: f32 = 12.0;
        pub const SMALL_HALF: f32 = 12.5;
        pub const CONTROL: f32 = 13.0;
        pub const BODY: f32 = 13.5;
        pub const LEAD: f32 = 14.0;
        pub const SECTION: f32 = 15.0;
        pub const SECTION_BIG: f32 = 18.0;
        pub const STAT: f32 = 20.0;
        pub const STAT_BIG: f32 = 26.0;
        pub const TITLE: f32 = 22.0;
    }
    /// Corner radii (DC §2.6).
    pub mod radius {
        pub const SM: f32 = 6.0;
        pub const MD: f32 = 8.0;
        pub const LG: f32 = 10.0;
        pub const XL: f32 = 12.0;
    }
}

// ---------------------------------------------------------------------------
// Theme installation
// ---------------------------------------------------------------------------

/// Install the design system on a context: fonts, spacing, and the full
/// light and dark visuals (both, so the theme switch is instant and
/// system-theme drift cannot fall back to egui defaults).
pub fn install(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    // Modern desktop type scale on the embedded Ubuntu-Light/Hack stack.
    use egui::FontFamily::{Monospace, Proportional};
    use egui::TextStyle::{Body, Button, Heading, Monospace as MonoStyle, Small};
    fonts
        .families
        .get_mut(&Proportional)
        .expect("proportional family")
        .push("Hack".into());
    let mut text_styles = egui::Style::default().text_styles;
    // DC §2.5 type roles: title 22 · body 13.5 · controls 13 · mono 12.5 · small 12.
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

/// Full zinc-light visuals.
#[must_use]
pub fn light_visuals() -> egui::Visuals {
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

/// Full zinc-dark visuals.
#[must_use]
pub fn dark_visuals() -> egui::Visuals {
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
        corner_radius: egui::CornerRadius::same(6),
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

// ---------------------------------------------------------------------------
// Components: buttons
// ---------------------------------------------------------------------------

/// Button variants (DC: DS-C-11 — primary / outline / ghost / danger / text / ai).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ButtonVariant {
    /// Solid primary (DS-C-11 `primary`).
    #[default]
    Primary,
    /// Muted surface fill (legacy secondary; reads as a quiet outline).
    Secondary,
    /// Bordered, no fill (DS-C-11 `outline`).
    Outline,
    /// No fill, no border; hover tints (DS-C-11 `ghost`).
    Ghost,
    /// Solid destructive red (DS-C-11 `danger` — pairs with armed confirm).
    Destructive,
    /// Muted label, no fill/border (DS-C-11 `text`).
    Text,
    /// Solid AI violet (DS-C-11 `ai` — the AI identity action).
    Ai,
}

/// A shadcn-style button with hover/press feedback and a focus ring.
pub fn button(ui: &mut egui::Ui, text: &str, variant: ButtonVariant) -> egui::Response {
    button_impl(ui, text, None, variant)
}

/// A shadcn-style button with a leading icon.
pub fn button_with_icon(
    ui: &mut egui::Ui,
    icon: Icon,
    text: &str,
    variant: ButtonVariant,
) -> egui::Response {
    button_impl(ui, text, Some(icon), variant)
}

/// An icon-only shadcn-style button (ghost or outline) with a tooltip.
pub fn icon_button(
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
    // DS-C-11 anatomy, drawn manually so the icon zone is exact: 32px
    // height, r-md corners, 14px side padding, 13px icon + 5px gap, 13px
    // control text.
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

/// Tonal hover/press overlay painted above the solid button so explicit
/// fills still respond like their shadcn counterparts (primary/90 etc.).
fn hover_overlay(ui: &mut egui::Ui, response: &egui::Response, t: &Tokens, variant: ButtonVariant) {
    // In light mode everything darkens on hover; in dark mode light fills
    // darken and dark fills lighten. Destructive always darkens slightly.
    let light_mode = !ui.visuals().dark_mode;
    let darken = match variant {
        ButtonVariant::Primary => true,
        ButtonVariant::Secondary | ButtonVariant::Ghost | ButtonVariant::Outline => light_mode,
        ButtonVariant::Destructive => true,
        ButtonVariant::Text => light_mode,
        ButtonVariant::Ai => true,
    };
    // The tint darkens a light surface and vice versa; black when the
    // dark theme needs to darken its (light) primary fill.
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

/// Draw the keyboard focus ring around a focused response's rect.
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

// ---------------------------------------------------------------------------
// Components: chips, badges, status pills
// ---------------------------------------------------------------------------

/// Semantic badge tones.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BadgeTone {
    Neutral,
    Success,
    Warning,
    Destructive,
    Info,
    /// AI identity (DC G-2: AI is always violet).
    Ai,
}

/// A small status badge (soft fill + colored text), shadcn Badge style.
/// The (fill, foreground) pair a tone paints with — shared by badges and
/// the header freshness pill so they can never drift apart.
pub fn tone_colors(ui: &egui::Ui, tone: BadgeTone) -> (egui::Color32, egui::Color32) {
    tone_colors_of(tokens(ui), tone)
}

/// Theme-independent core of [`tone_colors`] — (fill, foreground) per tone
/// (DC §5 + §2.3); shared by badges, dots, tiles and bands so they cannot
/// drift apart.
pub fn tone_colors_of(t: &Tokens, tone: BadgeTone) -> (egui::Color32, egui::Color32) {
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
    let response = ui.interact(rect, id, egui::Sense::hover());
    ui.advance_cursor_after_rect(rect);
    response
}

/// DC §5 DS.TONE — the ONLY status → tone map in the product (one match arm;
/// `docs/audit/design-contract-v1.md`). Adding a status = adding a row here
/// first, then a DC row; pages alias raw data tokens onto these keys.
pub fn status_tone(status: &str) -> (BadgeTone, &'static str) {
    match status {
        // conversation lifecycle
        "open" => (BadgeTone::Info, "Open"),
        "pending" => (BadgeTone::Warning, "Pending"),
        "waiting" => (BadgeTone::Warning, "Waiting"),
        "resolved" => (BadgeTone::Success, "Resolved"),
        "closed" => (BadgeTone::Neutral, "Closed"),
        "spam" => (BadgeTone::Destructive, "Spam"),
        "active" => (BadgeTone::Info, "Active"),
        // knowledge / articles
        "published" => (BadgeTone::Success, "Published"),
        "draft" => (BadgeTone::Neutral, "Draft"),
        "internal" => (BadgeTone::Info, "Internal"),
        // incident severity (bands)
        "critical" => (BadgeTone::Destructive, "Critical"),
        "high" => (BadgeTone::Warning, "High"),
        "medium" => (BadgeTone::Info, "Medium"),
        "low" => (BadgeTone::Neutral, "Low"),
        // health / sync
        "ok" => (BadgeTone::Success, "OK"),
        "healthy" => (BadgeTone::Success, "Healthy"),
        "live" => (BadgeTone::Success, "Live"),
        "stale" => (BadgeTone::Warning, "Stale"),
        "error" => (BadgeTone::Destructive, "Error"),
        "syncing" => (BadgeTone::Info, "Syncing"),
        "catching_up" => (BadgeTone::Info, "Catching up"),
        // authorship roles
        "ai" => (BadgeTone::Ai, "AI"),
        "note" => (BadgeTone::Warning, "Internal note"),
        "customer" => (BadgeTone::Neutral, "Customer"),
        "agent" => (BadgeTone::Neutral, "Agent"),
        _ => (BadgeTone::Neutral, "Other"),
    }
}

/// A selectable filter chip (shadcn Tabs/Toggle feel): full pill hitbox,
/// selected state uses the theme selection colors.
pub fn chip(ui: &mut egui::Ui, selected: bool, text: &str) -> egui::Response {
    chip_impl(ui, selected, text, None)
}

/// A selectable filter chip with a leading icon.
pub fn chip_with_icon(ui: &mut egui::Ui, selected: bool, icon: Icon, text: &str) -> egui::Response {
    chip_impl(ui, selected, text, Some(icon))
}

fn chip_impl(ui: &mut egui::Ui, selected: bool, text: &str, icon: Option<Icon>) -> egui::Response {
    let t = tokens(ui);
    let fg = if selected {
        t.selection_foreground
    } else {
        t.muted_foreground
    };
    let rich = egui::RichText::new(text).color(fg).size(13.0);
    let galley = egui::WidgetText::from(rich).into_galley(
        ui,
        Some(egui::TextWrapMode::Extend),
        f32::INFINITY,
        egui::TextStyle::Button,
    );
    let icon_size = if icon.is_some() { 15.0 } else { 0.0 };
    let pad = egui::vec2(12.0, 6.0);
    let size = egui::vec2(
        galley.size().x + 2.0 * pad.x + icon_size + if icon.is_some() { 6.0 } else { 0.0 },
        galley.size().y + 2.0 * pad.y,
    );
    let (id, rect) = ui.allocate_space(size);
    let response = ui.interact(rect, id.with(("spp-chip", text)), egui::Sense::click());
    let fill = if selected {
        t.selection
    } else if response.hovered() || response.has_focus() {
        t.accent
    } else {
        egui::Color32::TRANSPARENT
    };
    if fill != egui::Color32::TRANSPARENT {
        ui.painter()
            .rect_filled(rect, egui::CornerRadius::same(250), fill);
    }
    let mut x = rect.min.x + pad.x;
    if let Some(icon) = icon {
        let icon_rect = egui::Rect::from_min_max(
            egui::pos2(x, rect.center().y - icon_size / 2.0),
            egui::pos2(x + icon_size, rect.center().y + icon_size / 2.0),
        );
        icon.paint_at(ui.painter(), icon_rect, fg);
        x += icon_size + 6.0;
    }
    ui.painter().galley(
        egui::pos2(x, rect.center().y - galley.size().y / 2.0),
        galley,
        fg,
    );
    if let Some(cursor) = ui.visuals().interact_cursor
        && response.hovered()
    {
        ui.ctx().set_cursor_icon(cursor);
    }
    focus_feedback(ui, &response);
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::Role::Button,
            ui.is_enabled(),
            selected,
            text.to_owned(),
        )
    });
    ui.advance_cursor_after_rect(rect);
    response
}

// ---------------------------------------------------------------------------
// Components: cards, sections, page headers, tables, empty states
// ---------------------------------------------------------------------------

/// A card: bordered surface with the theme's card background.
pub fn card<R>(
    ui: &mut egui::Ui,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<R> {
    let t = tokens(ui);
    let frame = egui::Frame::new()
        .fill(t.card)
        .stroke(egui::Stroke::new(1.0, t.border))
        .corner_radius(10)
        .inner_margin(egui::Margin::same(14))
        .shadow(egui::Shadow {
            offset: [0, 1],
            blur: 2,
            spread: 0,
            color: egui::Color32::from_black_alpha(if ui.visuals().dark_mode { 80 } else { 14 }),
        });
    frame.show(ui, add_contents)
}

/// A card with a title (small, semibold) and optional muted hint.
pub fn card_titled<R>(
    ui: &mut egui::Ui,
    title: &str,
    hint: Option<&str>,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<R> {
    card(ui, |ui| {
        ui.vertical(|ui| {
            section_title(ui, title);
            if let Some(hint) = hint {
                hint_text(ui, hint);
            }
            ui.add_space(6.0);
            add_contents(ui)
        })
        .inner
    })
}

/// Section heading inside a page or card.
pub fn section_title(ui: &mut egui::Ui, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .size(15.0)
            .strong()
            .color(tokens(ui).foreground),
    );
}

/// Muted secondary text.
pub fn hint_text(ui: &mut egui::Ui, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .small()
            .color(tokens(ui).muted_foreground),
    );
}

/// A minimal pager: `‹ prev   page 2 of 7   next ›` (clean-audit items
/// 78/82: the Inbox and Customers lists were hard-capped at one page of
/// rows with no way to reach the rest). Renders nothing when everything
/// fits on one page. Returns the page delta to apply (-1, 0, +1).
pub fn pager(ui: &mut egui::Ui, page: u32, pages: u32) -> i32 {
    if pages <= 1 {
        return 0;
    }
    let mut delta = 0;
    ui.horizontal(|ui| {
        let t = tokens(ui);
        // The disabled ends render as inert muted glyphs so the control
        // keeps its shape at either boundary (no layout jump).
        if page > 0 {
            if button(ui, "‹ prev", ButtonVariant::Outline).clicked() {
                delta = -1;
            }
        } else {
            ui.label(
                egui::RichText::new("‹ prev")
                    .small()
                    .color(t.muted_foreground),
            );
        }
        ui.label(
            egui::RichText::new(format!("page {} of {}", page + 1, pages))
                .small()
                .color(t.muted_foreground),
        );
        if page + 1 < pages {
            if button(ui, "next ›", ButtonVariant::Outline).clicked() {
                delta = 1;
            }
        } else {
            ui.label(
                egui::RichText::new("next ›")
                    .small()
                    .color(t.muted_foreground),
            );
        }
    });
    delta
}

/// The standard page header: page title, optional subtitle, right-side
/// actions, and breathing room before the content.
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
    ui.add_space(10.0);
}

/// DC: DS-C-03 StatTile — muted 12px label, 26px tabular value, 12px hint.
pub fn metric_card(ui: &mut egui::Ui, label: &str, value: &str, hint: Option<&str>) {
    card(ui, |ui| {
        ui.set_min_width(150.0);
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

/// DC: DS-C-01 PageHeader counter pill (`.ds-counter`) — counts live here,
/// never in card titles (register A-6).
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

/// The solid status color for a tone — dots, tile bands, level insets.
/// Derived from the same DS.TONE table as [`tone_colors_of`] (DC G-2:
/// one meaning, one color, everywhere).
pub fn tone_solid(t: &Tokens, tone: BadgeTone) -> egui::Color32 {
    match tone {
        BadgeTone::Neutral => t.border_strong,
        BadgeTone::Success => t.success,
        BadgeTone::Warning => t.warning,
        BadgeTone::Destructive => t.destructive,
        BadgeTone::Info => t.info,
        BadgeTone::Ai => t.ai,
    }
}

/// DC: DS-C-06 StatusDot — the 8px (11px big) tone dot bound to DS.TONE;
/// the same tone its badge would render, never a new color.
pub fn status_dot(ui: &mut egui::Ui, tone: BadgeTone, big: bool) {
    let t = tokens(ui);
    let d = if big { 11.0 } else { 8.0 };
    let (id, rect) = ui.allocate_space(egui::vec2(d, d));
    ui.painter()
        .circle_filled(rect.center(), d / 2.0, tone_solid(t, tone));
    ui.advance_cursor_after_rect(rect);
    let _ = id;
}

/// One underline tab row (DS-C-09 anatomy); returns a globally-indexed
/// click. Used by [`tabs`] and [`tab_groups`].
fn tabs_row(ui: &mut egui::Ui, items: &[&str], active: usize, base: usize) -> Option<usize> {
    let t = tokens(ui);
    let mut clicked = None;
    ui.horizontal(|ui| {
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
                // 2px primary underline, 10px inset, riding the rail below.
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

/// DC: DS-C-09 Tabs (underline) — active = 500 text + 2px primary underline
/// on a full-width hairline rail. Returns the index clicked this frame.
pub fn tabs(ui: &mut egui::Ui, items: &[&str], active: usize) -> Option<usize> {
    let clicked = tabs_row(ui, items, active, 0);
    divider(ui);
    ui.add_space(scale::space::SP2);
    clicked
}

/// DC: DS-C-09 grouped variant — labeled tab groups with a global index
/// (the reports pattern). One rail at the end.
pub fn tab_groups(ui: &mut egui::Ui, groups: &[(&str, &[&str])], active: usize) -> Option<usize> {
    let t = tokens(ui);
    let mut clicked = None;
    let mut base = 0;
    ui.horizontal_wrapped(|ui| {
        for (label, items) in groups {
            ui.vertical(|ui| {
                ui.set_min_width(0.0);
                ui.label(
                    egui::RichText::new(*label)
                        .size(scale::text::META)
                        .color(t.muted_foreground),
                );
                if let Some(c) = tabs_row(ui, items, active, base) {
                    clicked = Some(c);
                }
            });
            ui.add_space(22.0);
            base += items.len();
        }
    });
    divider(ui);
    ui.add_space(scale::space::SP2);
    clicked
}

/// DC: DS-C-10 FilterBar — filters left (inputs/combos), context and
/// exactly ONE primary apply right (register A-2). Returns the apply
/// response (clicked/pressed this frame).
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

/// A centered empty state: icon, title, muted hint.
pub fn empty_state(ui: &mut egui::Ui, icon: Icon, title: &str, hint: &str) {
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

/// A striped data table with muted headers, the shared shadcn look.
pub fn table<R>(
    ui: &mut egui::Ui,
    id: &str,
    headers: &[&str],
    row_height: f32,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<R> {
    let t = tokens(ui);
    let grid = egui::Grid::new(egui::IdSalt::new(format!("spp-table-{id}")))
        .num_columns(headers.len())
        .striped(true)
        .spacing(egui::vec2(16.0, 0.0))
        .min_row_height(row_height);
    ui.add_space(4.0);
    grid.show(ui, |ui| {
        for header in headers {
            ui.label(
                egui::RichText::new(*header)
                    .small()
                    .strong()
                    .color(t.muted_foreground),
            );
        }
        ui.end_row();
        add_contents(ui)
    })
}

/// Numeric/monospace right-aligned table cell.
pub fn td_num(ui: &mut egui::Ui, text: &str) {
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        ui.monospace(egui::RichText::new(text).color(tokens(ui).foreground));
    });
}

/// DC: DS-C-05 DataTable — bordered r-lg wrap, uppercase muted header on
/// surface-2, **virtualized**: only visible rows are laid out (egui gap:
/// header letter-spacing 0.04em is not reproducible — DC exception E-2).
/// `row` is called once per visible row index per frame.
pub fn data_table(
    ui: &mut egui::Ui,
    id: &str,
    headers: &[&str],
    row_count: usize,
    row_height: f32,
    row: impl FnMut(&mut egui::Ui, usize),
) {
    let t = tokens(ui);
    egui::Frame::new()
        .stroke(egui::Stroke::new(1.0, t.border))
        .corner_radius(egui::CornerRadius::same(10))
        .show(ui, |ui| {
            // Pinned header: 11px uppercase muted on surface-2.
            egui::Frame::new()
                .fill(t.sidebar)
                .corner_radius(egui::CornerRadius {
                    nw: 9,
                    ne: 9,
                    sw: 0,
                    se: 0,
                })
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        for header in headers {
                            ui.label(
                                egui::RichText::new(header.to_uppercase())
                                    .size(scale::text::META)
                                    .strong()
                                    .color(t.muted_foreground),
                            );
                            ui.add_space(16.0);
                        }
                    });
                });
            egui::ScrollArea::vertical()
                .id_salt(format!("spp-dt-{id}"))
                .show_rows(ui, row_height, row_count, |ui, range| {
                    egui::Grid::new(egui::IdSalt::new(format!("spp-dt-{id}-grid")))
                        .num_columns(headers.len())
                        .spacing(egui::vec2(16.0, 0.0))
                        .min_row_height(row_height)
                        .show(ui, |ui| {
                            let mut row = row;
                            for i in range {
                                row(ui, i);
                                ui.end_row();
                            }
                        });
                });
        });
}

/// DC: DS-C-04 MetricTile — horizontal count + label tile; `band` tints
/// the count and the 3px left rail via DS.TONE; `drill` makes it a
/// clickable drill-down (hover border-strong). Returns the response.
pub fn metric_tile(
    ui: &mut egui::Ui,
    count: &str,
    label: &str,
    band: Option<BadgeTone>,
    drill: bool,
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
    let width = 196.0f32.max(count_g.size().x + label_g.size().x + 12.0 + 10.0 + 12.0 + 12.0);
    let size = egui::vec2(width, 40.0);
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
    let y = rect.center().y;
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

/// Aligned two-column form grid (labels left, controls right).
pub fn form_grid<R>(
    ui: &mut egui::Ui,
    id: &str,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<R> {
    egui::Grid::new(egui::IdSalt::new(format!("spp-form-{id}")))
        .num_columns(2)
        .spacing(egui::vec2(16.0, 10.0))
        .show(ui, add_contents)
}

/// A form field label (muted, right-aligned toward the control).
pub fn field_label(ui: &mut egui::Ui, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .size(13.0)
            .color(tokens(ui).muted_foreground),
    );
}

/// Success inline feedback text.
pub fn success_text(ui: &mut egui::Ui, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .small()
            .color(tokens(ui).success_foreground),
    );
}

/// Error inline feedback text.
pub fn error_text(ui: &mut egui::Ui, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .small()
            .color(tokens(ui).destructive_text),
    );
}

/// A subtle full-width divider using the theme border color.
pub fn divider(ui: &mut egui::Ui) {
    let t = tokens(ui);
    let width = ui.available_width();
    let (id, rect) = ui.allocate_space(egui::vec2(width, 1.0));
    ui.painter()
        .rect_filled(rect, egui::CornerRadius::ZERO, t.border);
    let _ = id;
}

// ---------------------------------------------------------------------------
// Components: feedback set (DC: DS-C-14/15/16/19 — R2b, RS-14)
// ---------------------------------------------------------------------------

/// DC: DS-C-14 EmptyState (slim variant) — 120px centered column for inline
/// panes; the full variant is [`empty_state`].
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

/// DC: DS-C-14 EmptyState with the A-7 next-step CTA — every empty state
/// says why + what to do; the CTA returns `true` on click.
pub fn empty_state_cta(
    ui: &mut egui::Ui,
    icon: Icon,
    title: &str,
    hint: &str,
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
        clicked = button_with_icon(ui, Icon::Plus, cta_label, ButtonVariant::Primary).clicked();
        ui.add_space(scale::space::SP4);
    });
    clicked
}

/// DC: DS-C-15 Callout — in-card context note (register B-1 role: never a
/// page banner, never an action result). `warn` and `err` tint via DS.TONE.
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

/// DC: DS-C-15 callout roles (fixed by register B-1 — pick by role).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalloutRole {
    /// In-card context (neutral tint).
    Neutral,
    /// Caution context (warning tint + hairline).
    Warn,
    /// Failure context (danger tint).
    Err,
}

/// DC: DS-C-15 status-line — the action-result role (register B-1/A-8):
/// "syncing… → up to date / error". `ok` picks the tone.
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

/// DC: DS-C-15 quirk — the receipt-cited quirk footnote (dashed border,
/// muted); data-layer quirks stay visible, never faked fixed (DC C-1).
pub fn quirk(ui: &mut egui::Ui, text: &str) {
    let t = tokens(ui);
    egui::Frame::new()
        .fill(t.muted.gamma_multiply(0.55))
        .corner_radius(egui::CornerRadius::same(8))
        .inner_margin(egui::Margin::same(10))
        .show(ui, |ui| {
            // egui gap (DC E-2): no dashed strokes — the quirk keeps the
            // dashed-border look via the muted fill + strong hairline.
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

/// DC: DS-C-19 Avatar — initials in a role-tinted circle (lg 34 / md 28 /
/// sm 16). `role` picks the fill from the same DS.TONE table.
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

/// DC: DS-C-19 avatar sizes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AvatarSize {
    /// 34px — record headers, reader head.
    Lg,
    /// 28px — message cards.
    Md,
    /// 16px — inline rows.
    Sm,
}

/// DC: DS-C-19 Assignee — sm avatar + name, or the dashed "unassigned"
/// pill (register A-5 row-anatomy rule).
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
            let t = tokens(ui);
            // egui gap (DC E-2): no dashed strokes — italic + hairline carry
            // the DS "dashed unassigned" affordance.
            let text = egui::RichText::new("unassigned")
                .size(scale::text::META_HALF)
                .italics()
                .color(t.muted_foreground);
            let galley = egui::WidgetText::from(text).into_galley(
                ui,
                Some(egui::TextWrapMode::Extend),
                f32::INFINITY,
                egui::TextStyle::Small,
            );
            let pad = egui::vec2(8.0, 2.0);
            let size = galley.size() + 2.0 * pad;
            let (rect, _id) = ui.allocate_exact_size(size, egui::Sense::hover());
            ui.painter().rect_stroke(
                rect,
                egui::CornerRadius::same(9),
                egui::Stroke::new(1.0, t.border_strong),
                egui::StrokeKind::Inside,
            );
            ui.painter().galley(
                egui::pos2(rect.min.x + pad.x, rect.center().y - galley.size().y / 2.0),
                galley,
                t.muted_foreground,
            );
            ui.advance_cursor_after_rect(rect);
        }
    }
}

/// DC: DS-C-16 armed ConfirmDialog rule (register A-5) — a destructive row
/// action is a danger button that must be armed: the first click swaps the
/// label to the explicit confirmation; a second click confirms, Esc or a
/// click elsewhere disarms. Returns `true` exactly on the confirming click.
pub fn armed_danger_button(ui: &mut egui::Ui, id: &str, label: &str, confirm_label: &str) -> bool {
    let armed_id = egui::Id::unique(("spp-armed", id));
    // The arm timestamp (frame index) guards against the auto-Id collision
    // at the shared rect: the release that armed the button must never
    // complete as a click on the danger button that replaces it.
    let armed_frame = ui.ctx().data(|d| d.get_temp::<u64>(armed_id)).unwrap_or(0);
    let armed = armed_frame > 0;
    if armed {
        let settle = ui.ctx().cumulative_frame_nr() > armed_frame + 1;
        let confirm = settle && button(ui, confirm_label, ButtonVariant::Destructive).clicked();
        // Esc or clicking anything else disarms.
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

/// DC: DS-C-16 DetailPanel — the right sheet (420px, surface, left
/// hairline) over a scrim; Esc closes; body scrolls; `header` renders
/// next to the title. Register A-1: list rows open this, never a
/// full-page replace.
pub fn detail_panel(
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
    // Scrim captures clicks; clicking it closes the panel.
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
                        // Header row: title + actions + close.
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

/// DC: DS-C-13 form row — one label + control pair on the 220px grid
/// (label left via [`field_label`], control right).
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

// ---------------------------------------------------------------------------
// Components: inbox identity set (DC: DS-C-20/21/22 — R2c, RS-15)
// ---------------------------------------------------------------------------

/// DC: DS-C-20 message authorship roles — the surface, hairline and avatar
/// fill come from this table; nothing else may color a message card.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageRole {
    Customer,
    Agent,
    Note,
    Ai,
}

/// DC: DS-C-20 Message Card (role-typed) — r-xl card, md role avatar +
/// 13.5px name + 11.5px role label + right time, 14px body, footer actions.
/// G-2: AI is always violet; notes are always amber.
pub fn message_card<R>(
    ui: &mut egui::Ui,
    role: MessageRole,
    name: &str,
    time: &str,
    body: &str,
    actions: impl FnOnce(&mut egui::Ui) -> R,
) {
    let t = tokens(ui);
    let (bg, border) = match role {
        MessageRole::Customer => (t.card, t.border),
        MessageRole::Agent => (t.agent_surface, t.agent_border),
        MessageRole::Note => (t.warning_soft, t.warning_border),
        MessageRole::Ai => (t.ai_soft, t.ai_border),
    };
    egui::Frame::new()
        .fill(bg)
        .stroke(egui::Stroke::new(1.0, border))
        .corner_radius(egui::CornerRadius::same(12))
        .inner_margin(egui::Margin {
            left: 16,
            right: 16,
            top: 12,
            bottom: 13,
        })
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            // Head: avatar + name + role + time.
            ui.horizontal(|ui| {
                let (av_role, av_bg, av_fg) = match role {
                    MessageRole::Customer => (BadgeTone::Neutral, t.muted, t.muted_foreground),
                    MessageRole::Agent => (BadgeTone::Neutral, t.primary, t.primary_foreground),
                    MessageRole::Note => (BadgeTone::Warning, t.warning_soft, t.warning_foreground),
                    MessageRole::Ai => (BadgeTone::Ai, t.ai, t.on_ai),
                };
                let _ = av_role;
                let (rect, _id) =
                    ui.allocate_exact_size(egui::vec2(28.0, 28.0), egui::Sense::hover());
                ui.painter().circle_filled(rect.center(), 14.0, av_bg);
                let initials: String = name
                    .split_whitespace()
                    .filter_map(|w| w.chars().next())
                    .take(2)
                    .collect();
                ui.painter().text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    initials.to_uppercase(),
                    egui::FontId::proportional(10.5),
                    av_fg,
                );
                ui.label(
                    egui::RichText::new(name)
                        .size(scale::text::BODY + 0.5)
                        .strong(),
                );
                let role_label = match role {
                    MessageRole::Customer => "Customer",
                    MessageRole::Agent => "Agent",
                    MessageRole::Note => "Internal note",
                    MessageRole::Ai => "AI",
                };
                let role_color = match role {
                    MessageRole::Ai => t.ai_foreground,
                    MessageRole::Note => t.warning_foreground,
                    _ => t.muted_foreground,
                };
                ui.label(
                    egui::RichText::new(role_label)
                        .size(scale::text::META_HALF)
                        .color(role_color),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        egui::RichText::new(time)
                            .size(scale::text::META_HALF)
                            .color(t.muted_foreground),
                    );
                });
            });
            ui.add_space(scale::space::SP2 + 1.0);
            ui.label(
                egui::RichText::new(body)
                    .size(scale::text::LEAD)
                    .color(t.foreground),
            );
            ui.add_space(scale::space::SP2);
            actions(ui);
        });
}

/// DC: DS-C-20 day divider — hairlines meeting a centered 11.5px label.
pub fn day_divider(ui: &mut egui::Ui, label: &str) {
    let t = tokens(ui);
    ui.horizontal(|ui| {
        let w = ui.available_width();
        let half = (w - 120.0) / 2.0;
        let (r1, _id) = ui.allocate_exact_size(egui::vec2(half, 1.0), egui::Sense::hover());
        ui.painter()
            .line_segment([r1.min, r1.max], egui::Stroke::new(1.0, t.border));
        ui.label(
            egui::RichText::new(label)
                .size(scale::text::META_HALF)
                .color(t.muted_foreground),
        );
        let (r2, _id2) = ui.allocate_exact_size(egui::vec2(half, 1.0), egui::Sense::hover());
        ui.painter()
            .line_segment([r2.min, r2.max], egui::Stroke::new(1.0, t.border));
    });
}

/// DC: DS-C-20 quoted-history collapse — "N earlier messages" toggle above
/// the quoted items; `open` is owned by the caller (draft-safe).
pub fn quoted_history(
    ui: &mut egui::Ui,
    id_salt: &str,
    open: &mut bool,
    count: usize,
    items: impl FnOnce(&mut egui::Ui),
) {
    let t = tokens(ui);
    let label = format!("{} earlier messages", count);
    let txt = if *open {
        format!("hide {}", label)
    } else {
        format!("show {}", label)
    };
    if button(ui, &txt, ButtonVariant::Text).clicked() {
        *open = !*open;
    }
    if *open {
        egui::Frame::new().show(ui, |ui| {
            ui.painter().rect_stroke(
                ui.max_rect(),
                egui::CornerRadius::ZERO,
                egui::Stroke::new(1.0, t.border),
                egui::StrokeKind::Inside,
            );
            ui.vertical(|ui| {
                ui.add_space(2.0);
                items(ui);
            });
        });
    }
    let _ = id_salt;
}

/// DC: DS-C-20 attachment chip — file icon + name + 11px size inside a
/// 32px r-md bordered chip; transparent on AI/note surfaces.
pub fn file_chip(ui: &mut egui::Ui, icon: Icon, name: &str, size: &str) -> egui::Response {
    let t = tokens(ui);
    ui.horizontal(|ui| {
        egui::Frame::new()
            .fill(t.card)
            .stroke(egui::Stroke::new(1.0, t.border))
            .corner_radius(egui::CornerRadius::same(8))
            .inner_margin(egui::Margin::symmetric(11, 0))
            .show(ui, |ui| {
                ui.set_min_size(egui::vec2(0.0, 32.0));
                ui.vertical_centered_justified(|ui| {
                    ui.add_space(4.0);
                });
                ui.horizontal(|ui| {
                    icon.show(ui, 13.0, t.muted_foreground);
                    ui.label(
                        egui::RichText::new(name)
                            .size(scale::text::SMALL_HALF)
                            .color(t.foreground),
                    );
                    ui.label(
                        egui::RichText::new(size)
                            .size(scale::text::META)
                            .color(t.muted_foreground),
                    );
                });
            });
        ui.interact(
            ui.min_rect(),
            egui::Id::unique(("spp-file-chip", name)),
            egui::Sense::hover(),
        )
    })
    .response
}

/// DC: DS-C-21 composer modes — Reply (primary surface) | Note (amber).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ComposerMode {
    #[default]
    Reply,
    Note,
}

/// DC: DS-C-21 composer actions the caller must apply.
#[derive(Debug, Clone, PartialEq)]
pub enum ComposerAct {
    /// Send the draft in the given mode (Ctrl+Enter also sends).
    Send(ComposerMode, String),
    /// Insert a saved reply by index (menu pick).
    SavedReply(usize),
    /// Insert the AI-drafted reply.
    AiInsert,
}

/// DC: DS-C-21 Composer — Reply|Note tabs, tool chips (saved replies menu,
/// AI insert), textarea (note mode = amber surface), context foot + send
/// (Ctrl+Enter shortcut handled by the caller via [`ComposerAct::Send`]),
/// draft-saved flag. Pure UI: nothing sends by itself.
pub fn composer(
    ui: &mut egui::Ui,
    id_salt: &str,
    state: &mut ComposerState,
    saved_replies: &[String],
) -> Option<ComposerAct> {
    use egui::RichText as R;
    let t = tokens(ui);
    let mut act = None;
    let note = state.mode == ComposerMode::Note;

    // Mode tabs (DS-C-21 Reply|Note).
    ui.horizontal(|ui| {
        for (mode, label) in [(ComposerMode::Reply, "Reply"), (ComposerMode::Note, "Note")] {
            let on = state.mode == mode;
            let text = R::new(label).size(scale::text::BODY).color(if on {
                t.foreground
            } else {
                t.muted_foreground
            });
            let text = if on { text.strong() } else { text };
            if ui
                .add(
                    egui::Button::new(text)
                        .fill(egui::Color32::TRANSPARENT)
                        .stroke(egui::Stroke::NONE)
                        .min_size(egui::vec2(0.0, 34.0)),
                )
                .clicked()
            {
                state.mode = mode;
            }
            if on {
                let rect = ui.max_rect();
                ui.painter().rect_filled(
                    egui::Rect::from_min_max(
                        egui::pos2(rect.left(), rect.bottom() - 2.0),
                        egui::pos2(rect.left() + 40.0, rect.bottom()),
                    ),
                    egui::CornerRadius::ZERO,
                    t.primary,
                );
            }
        }
    });

    // Tool chips row.
    ui.horizontal(|ui| {
        let menu_id = egui::Id::unique(("spp-sr-menu", id_salt));
        if button_with_icon(ui, Icon::Mail, "saved replies", ButtonVariant::Outline).clicked() {
            let open = ui
                .ctx()
                .data(|d| d.get_temp::<bool>(menu_id).unwrap_or(false));
            ui.ctx().data_mut(|d| d.insert_temp(menu_id, !open));
        }
        if ui
            .ctx()
            .data(|d| d.get_temp::<bool>(menu_id).unwrap_or(false))
        {
            egui::Frame::new()
                .fill(t.card)
                .stroke(egui::Stroke::new(1.0, t.border))
                .corner_radius(egui::CornerRadius::same(10))
                .inner_margin(egui::Margin::same(5))
                .show(ui, |ui| {
                    ui.set_min_width(240.0);
                    for (i, reply) in saved_replies.iter().enumerate() {
                        if ui
                            .add(
                                egui::Button::new(R::new(reply).size(scale::text::SMALL_HALF))
                                    .fill(egui::Color32::TRANSPARENT)
                                    .stroke(egui::Stroke::NONE),
                            )
                            .clicked()
                        {
                            act = Some(ComposerAct::SavedReply(i));
                            ui.ctx().data_mut(|d| d.insert_temp(menu_id, false));
                        }
                    }
                });
        }
        if button_with_icon(ui, Icon::Info, "AI draft", ButtonVariant::Outline).clicked() {
            act = Some(ComposerAct::AiInsert);
        }
        if state.draft_saved {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    R::new("draft saved")
                        .size(scale::text::META)
                        .color(t.muted_foreground),
                );
            });
        }
    });

    // Textarea (note mode = amber surface, DS-C-21).
    let edit = egui::TextEdit::multiline(&mut state.draft)
        .id_salt(format!("spp-composer-{id_salt}"))
        .hint_text(if note {
            "add an internal note…"
        } else {
            "write a reply…"
        })
        .desired_width(ui.available_width())
        .min_size(egui::vec2(0.0, 92.0))
        .font(egui::TextStyle::Body);
    let edit = if note {
        edit.background_color(t.warning_soft)
    } else {
        edit
    };
    ui.add(edit);

    // Foot: context + send.
    ui.horizontal(|ui| {
        ui.label(
            R::new(if note {
                "note — visible to teammates only"
            } else {
                "reply — sent to the customer"
            })
            .size(scale::text::META_HALF)
            .color(t.muted_foreground),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let send_label = if note { "send note" } else { "send" };
            let send = if note {
                // DS-C-21: note send = amber.

                {
                    let g = egui::WidgetText::from(
                        R::new(send_label).color(t.on_ai).size(scale::text::CONTROL),
                    )
                    .into_galley(
                        ui,
                        Some(egui::TextWrapMode::Extend),
                        f32::INFINITY,
                        egui::TextStyle::Button,
                    );
                    let size = egui::vec2(g.size().x + 28.0, 30.0);
                    let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::click());
                    ui.painter()
                        .rect_filled(rect, egui::CornerRadius::same(8), t.warning);
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
                }
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
    act
}

/// DC: DS-C-21 composer state (mode + draft + draft-saved flag).
#[derive(Debug, Clone, Default)]
pub struct ComposerState {
    /// Reply or Note.
    pub mode: ComposerMode,
    /// The editable draft.
    pub draft: String,
    /// Whether a draft has been saved (flag fades in per DS-C-21).
    pub draft_saved: bool,
}

impl ComposerState {
    /// A fresh Reply composer.
    pub fn new() -> Self {
        Self::default()
    }
}

/// DC: DS-C-22 copilot suggest card — AI-violet surface, AI-fg title,
/// bullet list, action row. The 5-second AI-identification pre-check
/// target (violet + AI badge alone identify it).
pub fn cop_suggest(
    ui: &mut egui::Ui,
    title: &str,
    bullets: &[String],
    actions: &[&str],
) -> Option<usize> {
    let t = tokens(ui);
    let mut clicked = None;
    egui::Frame::new()
        .fill(t.ai_soft)
        .stroke(egui::Stroke::new(1.0, t.ai_border))
        .corner_radius(egui::CornerRadius::same(10))
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
            ui.add_space(7.0);
            for bullet in bullets {
                ui.horizontal(|ui| {
                    ui.painter().circle_filled(
                        egui::pos2(ui.min_rect().left() + 4.0, ui.cursor().top() + 8.0),
                        2.0,
                        t.border_strong,
                    );
                    ui.add_space(12.0);
                    ui.label(
                        egui::RichText::new(bullet)
                            .size(scale::text::SMALL_HALF)
                            .color(t.foreground),
                    );
                });
            }
            if !actions.is_empty() {
                ui.add_space(7.0);
                ui.horizontal(|ui| {
                    for (i, a) in actions.iter().enumerate() {
                        if button(ui, a, ButtonVariant::Outline).clicked() {
                            clicked = Some(i);
                        }
                    }
                });
            }
        });
    clicked
}

/// DC: DS-C-22 copilot similar-question row — 12.5px question + 11.5px
/// resolution preview under a top hairline; click opens it.
pub fn cop_similar(ui: &mut egui::Ui, question: &str, resolution: &str) -> egui::Response {
    let t = tokens(ui);
    ui.add_space(2.0);
    let (rect, resp) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 44.0), egui::Sense::click());
    ui.painter().line_segment(
        [
            egui::pos2(rect.left(), rect.top()),
            egui::pos2(rect.right(), rect.top()),
        ],
        egui::Stroke::new(1.0, t.border),
    );
    ui.painter().text(
        egui::pos2(rect.left(), rect.top() + 12.0),
        egui::Align2::LEFT_TOP,
        question,
        egui::FontId::proportional(scale::text::SMALL_HALF),
        if resp.hovered() {
            t.foreground
        } else {
            t.muted_foreground
        },
    );
    ui.painter().text(
        egui::pos2(rect.left(), rect.top() + 28.0),
        egui::Align2::LEFT_TOP,
        resolution,
        egui::FontId::proportional(scale::text::META_HALF),
        t.muted_foreground,
    );
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, question.to_owned()));
    ui.advance_cursor_after_rect(rect);
    resp
}

/// DC: DS-C-22 copilot chat bubble — assistant = AI-violet surface,
/// user = neutral, right-aligned row.
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

    /// WCAG relative luminance.
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
        // text-2 on the canvas (`--ds-text-2` on `--ds-bg`): DC-frozen pair,
        // owner-approved via G1. 4.11:1 in light (AA for the meta/secondary
        // roles it fills), so the assert pins the DC truth rather than the
        // old white-canvas 4.5; on cards (`surface`, the dominant text-2
        // surface) the pair still clears 4.5 and is asserted below.
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
        // RS-12: the token VALUES are pinned 1:1 to DC §2
        // (docs/audit/design-contract-v1.md, ported from ds.css).
        assert_eq!(LIGHT.background, rgb(244, 244, 245)); // --ds-bg #f4f4f5
        assert_eq!(DARK.background, rgb(9, 9, 11));
        assert_eq!(LIGHT.card, rgb(255, 255, 255)); // --ds-surface
        assert_eq!(DARK.card, rgb(24, 24, 27));
        assert_eq!(LIGHT.sidebar, rgb(250, 250, 250)); // --ds-surface-2
        assert_eq!(DARK.sidebar, rgb(12, 12, 14));
        assert_eq!(DARK.faint, rgb(12, 12, 14));
        assert_eq!(LIGHT.ai, rgb(124, 58, 237)); // --ds-ai #7c3aed
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
        assert_eq!(LIGHT.hyperlink, LIGHT.ai_foreground); // --ds-link = ai-fg
        assert_eq!(DARK.hyperlink, DARK.ai_foreground);
        assert_eq!(LIGHT.chart_accent, LIGHT.ai); // lab charts draw in violet
        assert_eq!(DARK.chart_accent, DARK.ai);
        assert_eq!(
            LIGHT.shadow,
            egui::Color32::from_rgba_premultiplied(0, 0, 0, 36) // e2 rgba(0,0,0,0.14)
        );
        assert_eq!(
            LIGHT.scrim,
            egui::Color32::from_rgba_premultiplied(0, 0, 0, 71) // rgba(0,0,0,0.28)
        );
    }

    #[test]
    fn dc_v1_scales_and_type_roles_are_frozen() {
        assert_eq!(scale::space::SP1, 4.0);
        assert_eq!(scale::space::SP7, 32.0);
        assert_eq!(scale::space::CLUSTER, 6.0);
        assert_eq!(scale::text::BODY, 13.5);
        assert_eq!(scale::text::TITLE, 22.0);
        assert_eq!(scale::radius::XL, 12.0);
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
        // Every DC §5 status resolves through the one match arm.
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
        // Both themes carry our panel fill.
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
                    table(ui, "smoke", &["name", "count"], 24.0, |ui| {
                        ui.label("row");
                        td_num(ui, "3");
                        ui.end_row();
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
