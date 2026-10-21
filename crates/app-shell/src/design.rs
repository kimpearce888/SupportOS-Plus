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
//! Text styles are re-scaled to a modern desktop size: Heading 22,
//! Body/Button 14, Monospace 13, Small 12 (egui embeds Ubuntu Light).
//!
//! ## Components
//! Every interactive component draws a visible keyboard focus ring in
//! the theme's ring color, and hover/press feedback through a tonal
//! overlay so solid variants (primary/destructive) respond like their
//! web counterparts.

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
}

const fn rgb(r: u8, g: u8, b: u8) -> egui::Color32 {
    egui::Color32::from_rgb(r, g, b)
}

/// shadcn zinc, light mode.
pub const LIGHT: Tokens = Tokens {
    background: rgb(255, 255, 255),
    sidebar: rgb(250, 250, 250), // zinc-50
    card: rgb(255, 255, 255),
    border: rgb(228, 228, 231),           // zinc-200
    border_strong: rgb(212, 212, 216),    // zinc-300
    foreground: rgb(9, 9, 11),            // zinc-950
    muted: rgb(244, 244, 245),            // zinc-100
    muted_foreground: rgb(113, 113, 122), // zinc-500
    accent: rgb(244, 244, 245),
    primary: rgb(24, 24, 27), // zinc-900
    primary_foreground: rgb(250, 250, 250),
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
    selection: rgb(244, 244, 245),
    selection_foreground: rgb(24, 24, 27),
    input: rgb(255, 255, 255),
    faint: rgb(250, 250, 250),
    hyperlink: rgb(37, 99, 235),
    chart_accent: rgb(37, 99, 235),
    chart_accent_2: rgb(24, 24, 27),
    hover_tint: rgb(9, 9, 11),
};

/// shadcn zinc, dark mode.
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
    faint: rgb(16, 16, 19),
    hyperlink: rgb(96, 165, 250),
    chart_accent: rgb(96, 165, 250),
    chart_accent_2: rgb(250, 250, 250),
    hover_tint: rgb(250, 250, 250),
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
    text_styles.insert(Heading, egui::FontId::new(22.0, Proportional));
    text_styles.insert(Body, egui::FontId::new(14.0, Proportional));
    text_styles.insert(Button, egui::FontId::new(14.0, Proportional));
    text_styles.insert(MonoStyle, egui::FontId::new(13.0, Monospace));
    text_styles.insert(Small, egui::FontId::new(12.0, Proportional));
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

/// Button variants, mirroring shadcn/ui's Button.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ButtonVariant {
    /// Solid primary (zinc-900 light / zinc-50 dark).
    #[default]
    Primary,
    /// Muted surface fill.
    Secondary,
    /// Bordered, no fill.
    Outline,
    /// No fill, no border; hover tints.
    Ghost,
    /// Solid destructive red.
    Destructive,
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
    let label = egui::RichText::new(text).color(fg).size(14.0);
    // One consistent 32px control height across every button, chip row
    // and combo box (shadcn default control height).
    let button = egui::Button::new(label)
        .fill(fill)
        .stroke(stroke)
        .min_size(egui::vec2(0.0, 32.0));
    let response = ui.add(button);
    if let Some(icon) = icon {
        // Paint the icon into the leading padding, keeping the label
        // layout egui computed.
        let icon_rect = egui::Rect::from_min_max(
            egui::pos2(response.rect.min.x + 8.0, response.rect.min.y),
            egui::pos2(response.rect.min.x + 26.0, response.rect.max.y),
        );
        icon.paint_at(ui.painter(), icon_rect, fg);
    }
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
}

/// A small status badge (soft fill + colored text), shadcn Badge style.
/// The (fill, foreground) pair a tone paints with — shared by badges and
/// the header freshness pill so they can never drift apart.
pub fn tone_colors(ui: &egui::Ui, tone: BadgeTone) -> (egui::Color32, egui::Color32) {
    let t = tokens(ui);
    match tone {
        BadgeTone::Neutral => (t.muted, t.secondary_foreground),
        BadgeTone::Success => (t.success_soft, t.success_foreground),
        BadgeTone::Warning => (t.warning_soft, t.warning_foreground),
        BadgeTone::Destructive => (t.destructive_soft, t.destructive_text),
        BadgeTone::Info => (t.info_soft, t.info_foreground),
    }
}

pub fn badge(ui: &mut egui::Ui, text: &str, tone: BadgeTone) -> egui::Response {
    let (bg, fg) = tone_colors(ui, tone);
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
    ui.painter().galley(
        egui::pos2(rect.min.x + pad.x, rect.center().y - galley.size().y / 2.0),
        galley,
        fg,
    );
    let response = ui.interact(rect, id, egui::Sense::hover());
    ui.advance_cursor_after_rect(rect);
    response
}

/// Conversation status → badge tone + label.
pub fn status_tone(status: &str) -> (BadgeTone, &'static str) {
    match status {
        "active" => (BadgeTone::Info, "Active"),
        "pending" => (BadgeTone::Warning, "Pending"),
        "closed" => (BadgeTone::Neutral, "Closed"),
        "spam" => (BadgeTone::Destructive, "Spam"),
        "open" => (BadgeTone::Info, "Open"),
        "waiting" => (BadgeTone::Warning, "Waiting"),
        "resolved" => (BadgeTone::Success, "Resolved"),
        "published" => (BadgeTone::Success, "Published"),
        "draft" => (BadgeTone::Neutral, "Draft"),
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

/// A dashboard metric card: muted label, large value, optional hint.
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
                    .size(24.0)
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
        assert!(contrast(LIGHT.muted_foreground, LIGHT.background) >= 4.5);
        assert!(contrast(DARK.muted_foreground, DARK.background) >= 4.5);
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
