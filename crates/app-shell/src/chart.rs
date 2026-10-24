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

use crate::design::{self, Tokens};

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
///
/// Restyled for the shadcn design system: card background, hairline
/// gridlines, the theme accent for the series plus a soft area fill, and
/// endpoint dots with the first/last values annotated under the frame.
pub fn line_chart(ui: &mut egui::Ui, _id: &str, points: &[Point], label: &str) -> egui::Rect {
    let t = design::tokens(ui);
    let (rect, _response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), 180.0),
        egui::Sense::hover(),
    );
    paint_frame(ui, rect, t);
    if let Some(scaled) = scale_points(points, rect) {
        paint_grid(ui, rect, t);
        // Soft area fill under the line. Clean-audit item 134: a general
        // series is usually CONCAVE — `Shape::convex_polygon`
        // fan-triangulates from the first vertex, so the fill bled past
        // the polyline and left holes wherever the series bent. The fill
        // is now an explicit mesh: two triangles per segment, exact for
        // any series shape.
        let fill = t.chart_accent.gamma_multiply(0.16);
        let baseline_y = rect.max.y - 8.0;
        ui.painter()
            .add(egui::Shape::Mesh(std::sync::Arc::new(area_fill_mesh(
                &scaled, baseline_y, fill,
            ))));
        let endpoints = [scaled[0], scaled[scaled.len() - 1]];
        ui.painter().add(egui::Shape::line(
            scaled,
            egui::Stroke::new(2.0, t.chart_accent),
        ));
        // Endpoint dots.
        for point in endpoints {
            ui.painter().circle_filled(point, 3.0, t.chart_accent);
        }
        // Endpoints annotation below the frame, inside the card padding.
        let first = points.first().map(|p| p[1]).unwrap_or_default();
        let last = points.last().map(|p| p[1]).unwrap_or_default();
        ui.painter().text(
            rect.left_bottom() + egui::vec2(4.0, 12.0),
            egui::Align2::LEFT_TOP,
            format!("{label} · {first} → {last}"),
            egui::FontId::proportional(11.5),
            t.muted_foreground,
        );
    } else {
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "(not enough data to plot)",
            egui::FontId::proportional(12.5),
            t.muted_foreground,
        );
    }
    rect
}

/// Draw a bar chart (one bar per `(index, value)`) into the available
/// space, returns the drawn rect.
pub fn bar_chart(ui: &mut egui::Ui, _id: &str, bars: &[Point], label: &str) -> egui::Rect {
    let t = design::tokens(ui);
    let (rect, _response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), 180.0),
        egui::Sense::hover(),
    );
    paint_frame(ui, rect, t);
    if bars.is_empty() {
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "(no data to plot)",
            egui::FontId::proportional(12.5),
            t.muted_foreground,
        );
        return rect;
    }
    paint_grid(ui, rect, t);
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
    let baseline = plot_area.max.y;
    for (i, bar) in bars.iter().enumerate() {
        let height = (bar[1] / max) as f32 * plot_area.height();
        let center_x = plot_area.min.x + plot_area.width() * (i as f32 + 0.5) / bars.len() as f32;
        let bar_rect = egui::Rect::from_min_max(
            egui::pos2(center_x - bar_width / 2.0, baseline - height),
            egui::pos2(center_x + bar_width / 2.0, baseline),
        );
        ui.painter()
            .rect_filled(bar_rect, egui::CornerRadius::same(3), t.chart_accent);
    }
    ui.painter().text(
        rect.left_bottom() + egui::vec2(4.0, 12.0),
        egui::Align2::LEFT_TOP,
        format!("{label} · max {max:.0}"),
        egui::FontId::proportional(11.5),
        t.muted_foreground,
    );
    rect
}

/// Card-like chart frame (rounded, hairline border).
fn paint_frame(ui: &mut egui::Ui, rect: egui::Rect, t: &Tokens) {
    ui.painter()
        .rect_filled(rect, egui::CornerRadius::same(8), t.card);
    ui.painter().rect_stroke(
        rect,
        egui::CornerRadius::same(8),
        egui::Stroke::new(1.0, t.border),
        egui::StrokeKind::Inside,
    );
}

/// The exact area fill between a polyline and a horizontal baseline, as
/// a triangle mesh (clean-audit item 134).
///
/// Two triangles per segment (`p_i, p_i+1, base_i, base_i+1`) tile the
/// region under the line — a general data series is concave, and the
/// previous `Shape::convex_polygon` fill fan-triangulated it from the
/// first vertex, over-drawing outside the polygon and leaving holes
/// wherever the line bent. Triangles carry no convexity assumption, so
/// the mesh is exact for any series shape.
#[must_use]
pub fn area_fill_mesh(points: &[egui::Pos2], baseline_y: f32, fill: egui::Color32) -> egui::Mesh {
    let mut mesh = egui::Mesh::default();
    for pair in points.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let base_a = egui::pos2(a.x, baseline_y);
        let base_b = egui::pos2(b.x, baseline_y);
        let idx = mesh.vertices.len() as u32;
        for p in [a, b, base_a, base_b] {
            mesh.colored_vertex(p, fill);
        }
        mesh.indices
            .extend_from_slice(&[idx, idx + 1, idx + 2, idx + 1, idx + 3, idx + 2]);
    }
    mesh
}

/// Three faint horizontal gridlines inside the chart area.
fn paint_grid(ui: &mut egui::Ui, rect: egui::Rect, t: &Tokens) {
    let inset = 8.0;
    for i in 1..=3u16 {
        let y = rect.min.y + inset + (rect.height() - 2.0 * inset) * f32::from(i) / 4.0;
        ui.painter().hline(
            rect.min.x + inset..=rect.max.x - inset,
            y,
            egui::Stroke::new(1.0, t.faint),
        );
    }
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

    /// Clean-audit item 134: the area fill mesh carries exactly two
    /// triangles per segment and colors every vertex.
    #[test]
    fn area_fill_mesh_counts_scale_with_segments() {
        let fill = egui::Color32::from_rgba_unmultiplied(1, 2, 3, 4);
        let pts = [
            egui::pos2(0.0, 10.0),
            egui::pos2(10.0, 4.0),
            egui::pos2(20.0, 8.0),
            egui::pos2(30.0, 2.0),
        ];
        let mesh = area_fill_mesh(&pts, 20.0, fill);
        assert_eq!(mesh.vertices.len(), 4 * (pts.len() - 1));
        assert_eq!(mesh.indices.len(), 6 * (pts.len() - 1));
        assert!(mesh.vertices.iter().all(|v| v.color == fill));
        // Every index stays inside the vertex list.
        let n = mesh.vertices.len() as u32;
        assert!(mesh.indices.iter().all(|i| *i < n));
    }

    /// Clean-audit item 134: the per-segment triangles tile the region
    /// under the polyline EXACTLY — their total area equals the shoelace
    /// area of the baseline-closed polygon, including for the concave
    /// series whose convex-polygon fill used to bleed.
    #[test]
    fn area_fill_mesh_area_matches_the_polygon_even_when_concave() {
        let baseline = 20.0_f32;
        // A V-shaped (concave) series: deep dip in the middle.
        let pts = [
            egui::pos2(0.0, 5.0),
            egui::pos2(10.0, 18.0),
            egui::pos2(20.0, 4.0),
        ];
        let mesh = area_fill_mesh(&pts, baseline, egui::Color32::BLACK);

        let tri_area = |a: egui::Pos2, b: egui::Pos2, c: egui::Pos2| {
            ((b.x - a.x) * (c.y - a.y) - (c.x - a.x) * (b.y - a.y)).abs() / 2.0
        };
        let mesh_area: f32 = mesh
            .indices
            .as_chunks::<3>()
            .0
            .iter()
            .map(|t| {
                tri_area(
                    mesh.vertices[t[0] as usize].pos,
                    mesh.vertices[t[1] as usize].pos,
                    mesh.vertices[t[2] as usize].pos,
                )
            })
            .sum();

        // Shoelace over [base_a, p0, p1, p2, base_b] (closed — the last
        // edge wraps back to base_a).
        let mut poly = vec![egui::pos2(pts[0].x, baseline)];
        poly.extend_from_slice(&pts);
        poly.push(egui::pos2(pts[pts.len() - 1].x, baseline));
        let polygon_area: f32 = {
            let mut acc = 0.0_f32;
            for w in poly.windows(2) {
                acc += w[0].x * w[1].y - w[1].x * w[0].y;
            }
            let (first, last) = (poly[0], *poly.last().unwrap());
            acc += last.x * first.y - first.x * last.y;
            (acc / 2.0).abs()
        };
        assert!(
            (mesh_area - polygon_area).abs() < 0.01,
            "mesh tiles {mesh_area} but the polygon encloses {polygon_area}"
        );
    }

    /// Clean-audit item 134: fewer than two points yield an empty mesh —
    /// nothing to fill (mirrors scale_points' degenerate contract).
    #[test]
    fn area_fill_mesh_degenerate_input_is_empty() {
        let mesh = area_fill_mesh(&[egui::pos2(0.0, 5.0)], 20.0, egui::Color32::BLACK);
        assert!(mesh.is_empty());
    }
}
