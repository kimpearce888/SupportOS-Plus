use super::*;

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
