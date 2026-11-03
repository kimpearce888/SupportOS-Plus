use super::*;
pub(super) fn chip_impl(
    ui: &mut egui::Ui,
    selected: bool,
    text: &str,
    icon: Option<Icon>,
) -> egui::Response {
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

pub(crate) fn card_titled<R>(
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
