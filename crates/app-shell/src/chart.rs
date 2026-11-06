use eframe::egui;

use crate::design::{self, Tokens};

pub type Point = [f64; 2];

#[must_use]
pub(crate) fn scale_points(points: &[Point], rect: egui::Rect) -> Option<Vec<egui::Pos2>> {
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

/// UX-9: `line_chart` with readable axes — y min/max value labels, x
/// endpoint labels (e.g. the date range), horizontal gridlines, and a hover
/// readout that names the nearest point.
pub(crate) fn line_chart_axes(
    ui: &mut egui::Ui,
    points: &[Point],
    label: &str,
    x_endpoints: Option<(&str, &str)>,
) -> egui::Rect {
    let t = design::tokens(ui);
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), 180.0),
        egui::Sense::hover(),
    );
    paint_frame(ui, rect, t);
    if let Some(scaled) = scale_points(points, rect) {
        paint_grid(ui, rect, t);
        let fill = t.chart_accent.gamma_multiply(0.16);
        let baseline_y = rect.max.y - 8.0;
        ui.painter()
            .add(egui::Shape::Mesh(std::sync::Arc::new(area_fill_mesh(
                &scaled, baseline_y, fill,
            ))));
        let endpoints = [scaled[0], scaled[scaled.len() - 1]];
        ui.painter().add(egui::Shape::line(
            scaled.clone(),
            egui::Stroke::new(2.0, t.chart_accent),
        ));
        for point in endpoints {
            ui.painter().circle_filled(point, 3.0, t.chart_accent);
        }
        let first = points.first().map(|p| p[1]).unwrap_or_default();
        let last = points.last().map(|p| p[1]).unwrap_or_default();

        // y-axis: min/max value labels anchored to the plot edges
        let max_y = points
            .iter()
            .map(|p| p[1])
            .fold(f64::NEG_INFINITY, f64::max);
        let min_y = points.iter().map(|p| p[1]).fold(f64::INFINITY, f64::min);
        let axis_font = egui::FontId::proportional(10.5);
        ui.painter().text(
            rect.left_top() + egui::vec2(8.0, 5.0),
            egui::Align2::LEFT_TOP,
            format_y(max_y),
            axis_font.clone(),
            t.muted_foreground,
        );
        ui.painter().text(
            rect.left_bottom() + egui::vec2(8.0, -18.0),
            egui::Align2::LEFT_TOP,
            format_y(min_y),
            axis_font.clone(),
            t.muted_foreground,
        );

        // x-axis: endpoint labels (dates/range) inside the frame bottom
        if let Some((xa, xb)) = x_endpoints {
            ui.painter().text(
                rect.left_bottom() + egui::vec2(8.0, -5.0),
                egui::Align2::LEFT_BOTTOM,
                xa,
                axis_font.clone(),
                t.muted_foreground,
            );
            ui.painter().text(
                rect.right_bottom() + egui::vec2(-8.0, -5.0),
                egui::Align2::RIGHT_BOTTOM,
                xb,
                axis_font,
                t.muted_foreground,
            );
        }

        // hover readout: nearest point gets a guide line + value bubble
        if let Some(pos) = response.hover_pos()
            && let Some((idx, nearest)) = scaled.iter().enumerate().min_by(|a, b| {
                (a.1.x - pos.x)
                    .abs()
                    .partial_cmp(&(b.1.x - pos.x).abs())
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            && rect.contains(*nearest)
        {
            ui.painter().line_segment(
                [
                    egui::pos2(nearest.x, rect.min.y + 6.0),
                    egui::pos2(nearest.x, baseline_y),
                ],
                egui::Stroke::new(1.0, t.faint),
            );
            let value = points[idx][1];
            let x_note = x_endpoints
                .as_ref()
                .map(|(xa, xb)| interpolate_x_note(xa, xb, idx, points.len()))
                .unwrap_or_default();
            let bubble = if x_note.is_empty() {
                format!("{label} · {}", format_y(value))
            } else {
                format!("{label} · {x_note} · {}", format_y(value))
            };
            let anchor = if nearest.x > rect.center().x {
                egui::pos2(nearest.x - 8.0, rect.min.y + 6.0)
            } else {
                egui::pos2(nearest.x + 8.0, rect.min.y + 6.0)
            };
            let align = if nearest.x > rect.center().x {
                egui::Align2::RIGHT_TOP
            } else {
                egui::Align2::LEFT_TOP
            };
            ui.painter().text(
                anchor,
                align,
                bubble,
                egui::FontId::proportional(11.0),
                t.foreground,
            );
        }

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

fn format_y(v: f64) -> String {
    if v.abs() >= 1000.0 || v.fract().abs() < f64::EPSILON {
        format!("{v:.0}")
    } else {
        format!("{v:.1}")
    }
}

fn interpolate_x_note(xa: &str, xb: &str, idx: usize, len: usize) -> String {
    if len <= 1 {
        return xa.to_string();
    }
    if idx == 0 {
        return xa.to_string();
    }
    if idx == len - 1 {
        return xb.to_string();
    }
    // Interior point: show the fractional position between the endpoints.
    format!("{:.0}%", (idx as f64 / (len - 1) as f64) * 100.0)
}

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

#[must_use]
pub(crate) fn area_fill_mesh(
    points: &[egui::Pos2],
    baseline_y: f32,
    fill: egui::Color32,
) -> egui::Mesh {
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
        assert!(scale_points(&[[0.0, 0.0], [0.0, 5.0]], rect()).is_none());
        assert!(scale_points(&[[0.0, 5.0], [3.0, 5.0]], rect()).is_none());
    }

    #[test]
    fn scale_handles_descending_series() {
        let scaled = scale_points(&[[0.0, 10.0], [1.0, 2.0]], rect()).expect("scaled");
        assert!(scaled[0].y < scaled[1].y, "higher value maps higher");
    }

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
        let n = mesh.vertices.len() as u32;
        assert!(mesh.indices.iter().all(|i| *i < n));
    }

    #[test]
    fn area_fill_mesh_area_matches_the_polygon_even_when_concave() {
        let baseline = 20.0_f32;
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

    #[test]
    fn area_fill_mesh_degenerate_input_is_empty() {
        let mesh = area_fill_mesh(&[egui::pos2(0.0, 5.0)], 20.0, egui::Color32::BLACK);
        assert!(mesh.is_empty());
    }
}
