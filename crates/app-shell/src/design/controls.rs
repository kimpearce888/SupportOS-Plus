use super::*;
pub(super) fn chip_impl(
    ui: &mut egui::Ui,
    selected: bool,
    text: &str,
    icon: Option<Icon>,
) -> egui::Response {
    let t = tokens(ui);
    let fg = if selected {
        t.primary_foreground
    } else {
        t.muted_foreground
    };
    let mut rich = egui::RichText::new(text).color(fg).size(13.0);
    if selected {
        rich = rich.strong();
    }
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
        t.primary
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
    response
}

pub fn card<R>(
    ui: &mut egui::Ui,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<R> {
    let t = tokens(ui);
    let dark = ui.visuals().dark_mode;
    let (stroke, shadow) = if dark {
        (egui::Stroke::new(1.0, t.border), egui::Shadow::NONE)
    } else {
        (
            egui::Stroke::NONE,
            egui::Shadow {
                offset: [0, 1],
                blur: 3,
                spread: 0,
                color: egui::Color32::from_black_alpha(26),
            },
        )
    };
    let frame = egui::Frame::new()
        .fill(t.card)
        .stroke(stroke)
        .corner_radius(10)
        .inner_margin(egui::Margin::same(14))
        .shadow(shadow);
    frame.show(ui, add_contents)
}

pub(crate) const CARD_PAD_X: f32 = 30.0;

pub(crate) const CARD_PAD_Y: f32 = 30.0;

pub(crate) fn card_exact_wh<R>(
    ui: &mut egui::Ui,
    outer_w: f32,
    min_h: Option<f32>,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<R> {
    card(ui, |ui| {
        let inner_w = (outer_w - CARD_PAD_X).max(80.0);
        ui.set_min_width(inner_w);
        ui.set_max_width(inner_w);
        if let Some(h) = min_h {
            ui.set_min_height((h - CARD_PAD_Y).max(0.0));
        }
        add_contents(ui)
    })
}

pub(crate) fn card_titled<R>(
    ui: &mut egui::Ui,
    title: &str,
    hint: Option<&str>,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<R> {
    card_titled_sized(ui, title, hint, false, add_contents)
}

pub(crate) fn card_titled_wide<R>(
    ui: &mut egui::Ui,
    title: &str,
    hint: Option<&str>,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<R> {
    card_titled_sized(ui, title, hint, true, add_contents)
}

pub(crate) fn card_titled_sized<R>(
    ui: &mut egui::Ui,
    title: &str,
    hint: Option<&str>,
    wide: bool,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<R> {
    card(ui, |ui| {
        if wide {
            ui.set_min_width(ui.available_width());
        }
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

pub(crate) fn card_titled_exact<R>(
    ui: &mut egui::Ui,
    title: &str,
    hint: Option<&str>,
    outer_w: f32,
    min_h: Option<f32>,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<R> {
    card_exact_wh(ui, outer_w, min_h, |ui| {
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

pub(crate) fn section_title(ui: &mut egui::Ui, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .size(15.0)
            .strong()
            .color(tokens(ui).foreground),
    );
}

pub fn hint_text(ui: &mut egui::Ui, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .small()
            .color(tokens(ui).muted_foreground),
    );
}

pub(crate) fn pager(ui: &mut egui::Ui, page: u32, pages: u32) -> i32 {
    if pages <= 1 {
        return 0;
    }
    let mut delta = 0;
    ui.horizontal(|ui| {
        let t = tokens(ui);
        if page > 0 {
            if button(ui, "‹ Prev", ButtonVariant::Outline).clicked() {
                delta = -1;
            }
        } else {
            ui.label(
                egui::RichText::new("‹ Prev")
                    .small()
                    .color(t.muted_foreground),
            );
        }
        ui.label(
            egui::RichText::new(format!("Page {} of {}", page + 1, pages))
                .small()
                .color(t.muted_foreground),
        );
        if page + 1 < pages {
            if button(ui, "Next ›", ButtonVariant::Outline).clicked() {
                delta = 1;
            }
        } else {
            ui.label(
                egui::RichText::new("Next ›")
                    .small()
                    .color(t.muted_foreground),
            );
        }
    });
    delta
}
