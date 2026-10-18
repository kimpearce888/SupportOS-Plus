//! Minimal native charts for the ported pages.
//!
//! The workspace pinned `egui_plot =0.37`, but that release builds
//! against egui 0.36 — no egui_plot release targets egui 0.37 yet
//! (releases lag one minor version). Pulling it in would compile a second
//! full copy of egui and break the single-version integrity the shell
//! relies on. Until egui_plot catches up, the ported pages draw their two
//! chart shapes (line series + bars) with the egui painter directly.
//!
//! The mapping math is pure and unit-tested; painting stays thin.

use eframe::egui;

/// One chart point in data space.
pub type Point = [f64; 2];

/// Scale data-space points into a pixel rect.
///
/// Returns `None` when there are fewer than two points or the span is
/// zero (nothing to draw). The output rect keeps a small inset so lines
/// are not clipped at the frame edges.
#[must_use]
pub fn scale_points(points: &[Point], rect: egui::Rect) -> Option<Vec<egui::Pos2>> {
    if points.len() < 2 {
        return None;
    }
    let min_x = points.iter().map(|p| p[0]).fold(f64::INFINITY, f64::min);
    let max_x = points
        .iter()
        .map(|p| p[0])
        .fold(f64::NEG_INFINITY, f64::max);
    let min_y = points.iter().map(|p| p[1]).fold(f64::INFINITY, f64::min);
    let max_y = points
        .iter()
        .map(|p| p[1])
        .fold(f64::NEG_INFINITY, f64::max);
    if (max_x - min_x).abs() < f64::EPSILON || (max_y - min_y).abs() < f64::EPSILON {
        return None;
    }
    let inset = 6.0;
    let inner = egui::Rect::from_min_max(
        egui::pos2(rect.min.x + inset, rect.min.y + inset),
        egui::pos2(rect.max.x - inset, rect.max.y - inset),
    );
    Some(
        points
            .iter()
            .map(|p| {
                let t_x = (p[0] - min_x) / (max_x - min_x);
                let t_y = (p[1] - min_y) / (max_y - min_y);
                egui::pos2(
                    inner.min.x + t_x as f32 * inner.width(),
                    inner.max.y - t_y as f32 * inner.height(),
                )
            })
            .collect(),
    )
}

/// Draw a line chart into the available space, returns the drawn rect.
pub fn line_chart(ui: &mut egui::Ui, _id: &str, points: &[Point], label: &str) -> egui::Rect {
    let (rect, _response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), 160.0),
        egui::Sense::hover(),
    );
    ui.painter().rect_stroke(
        rect,
        4.0,
        ui.visuals().widgets.noninteractive.bg_stroke,
        egui::StrokeKind::Inside,
    );
    let stroke = egui::Stroke::new(2.0, egui::Color32::from_rgb(52, 120, 198));
    if let Some(scaled) = scale_points(points, rect) {
        ui.painter().add(egui::Shape::line(scaled, stroke));
        // Endpoints annotation: first/last value under the frame.
        let first = points.first().map(|p| p[1]).unwrap_or_default();
        let last = points.last().map(|p| p[1]).unwrap_or_default();
        ui.painter().text(
            rect.left_bottom() + egui::vec2(8.0, -2.0),
            egui::Align2::LEFT_BOTTOM,
            format!("{label} · {first} → {last}"),
            egui::FontId::proportional(11.0),
            ui.visuals().weak_text_color(),
        );
    } else {
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "(not enough data to plot)",
            egui::FontId::proportional(12.0),
            ui.visuals().weak_text_color(),
        );
    }
    rect
}

/// Draw a bar chart (one bar per `(index, value)`) into the available
/// space, returns the drawn rect.
pub fn bar_chart(ui: &mut egui::Ui, _id: &str, bars: &[Point], label: &str) -> egui::Rect {
    let (rect, _response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), 160.0),
        egui::Sense::hover(),
    );
    ui.painter().rect_stroke(
        rect,
        4.0,
        ui.visuals().widgets.noninteractive.bg_stroke,
        egui::StrokeKind::Inside,
    );
    if bars.is_empty() {
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "(no data to plot)",
            egui::FontId::proportional(12.0),
            ui.visuals().weak_text_color(),
        );
        return rect;
    }
    let max = bars
        .iter()
        .map(|b| b[1])
        .fold(f64::NEG_INFINITY, f64::max)
        .max(1.0);
    let inset = 8.0;
    let plot_area = egui::Rect::from_min_max(
        egui::pos2(rect.min.x + inset, rect.min.y + inset),
        egui::pos2(rect.max.x - inset, rect.max.y - inset),
    );
    let bar_width = (plot_area.width() / bars.len() as f32 * 0.6).max(6.0);
    let color = egui::Color32::from_rgb(210, 120, 40);
    let baseline = plot_area.max.y;
    for (i, bar) in bars.iter().enumerate() {
        let height = (bar[1] / max) as f32 * plot_area.height();
        let center_x = plot_area.min.x + plot_area.width() * (i as f32 + 0.5) / bars.len() as f32;
        let bar_rect = egui::Rect::from_min_max(
            egui::pos2(center_x - bar_width / 2.0, baseline - height),
            egui::pos2(center_x + bar_width / 2.0, baseline),
        );
        ui.painter().rect_filled(bar_rect, 2.0, color);
    }
    ui.painter().text(
        rect.left_bottom() + egui::vec2(8.0, -2.0),
        egui::Align2::LEFT_BOTTOM,
        format!("{label} · max {max:.0}"),
        egui::FontId::proportional(11.0),
        ui.visuals().weak_text_color(),
    );
    rect
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect() -> egui::Rect {
        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(200.0, 100.0))
    }

    #[test]
    fn scale_maps_data_space_to_pixels_bottom_left_origin() {
        let scaled = scale_points(&[[0.0, 0.0], [10.0, 10.0]], rect()).expect("scaled");
        assert!((scaled[0].x - 6.0).abs() < 0.01, "first x at left inset");
        assert!((scaled[0].y - 94.0).abs() < 0.01, "min y at bottom");
        assert!((scaled[1].x - 194.0).abs() < 0.01, "max x at right inset");
        assert!((scaled[1].y - 6.0).abs() < 0.01, "max y at top inset");
    }

    #[test]
    fn scale_degenerate_inputs_return_none() {
        assert!(scale_points(&[], rect()).is_none());
        assert!(scale_points(&[[1.0, 1.0]], rect()).is_none());
        // Zero span in either axis → nothing to draw.
        assert!(scale_points(&[[0.0, 0.0], [0.0, 5.0]], rect()).is_none());
        assert!(scale_points(&[[0.0, 5.0], [3.0, 5.0]], rect()).is_none());
    }

    #[test]
    fn scale_handles_descending_series() {
        let scaled = scale_points(&[[0.0, 10.0], [1.0, 2.0]], rect()).expect("scaled");
        assert!(scaled[0].y < scaled[1].y, "higher value maps higher");
    }
}
