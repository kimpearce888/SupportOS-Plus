//! Lucide-style stroke icon family for the SupportOS+ design system.
//!
//! The redesign calls for professional icons from one consistent icon
//! family, without adding a runtime dependency (the egui-embedded fonts
//! only ship five brand glyphs). Instead of a font, every icon is drawn
//! with the egui painter on a 24x24 design grid scaled into the target
//! rectangle: a 2px round-capped stroke, the same geometry style, and the
//! current foreground color for every glyph. That keeps the whole family
//! visually consistent in both themes at any size and needs no new crate.
//!
//! The geometry is a light port of the shapes of the ISC-licensed Lucide
//! icon set (https://lucide.dev), reduced to the handful of glyphs the
//! shell needs and adapted to hand-sampled polylines, circles and
//! rounded rects.

use eframe::egui;

/// Stroke width on the 24-grid (Lucide uses 2).
const DESIGN_STROKE: f32 = 2.0;

/// A uniform (x, y) point on the 24-grid.
type P = [f32; 2];

/// One icon of the family.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    // Navigation / pages.
    Dashboard,
    Inbox,
    Activity,
    Users,
    Building,
    BookOpen,
    BarChart,
    Target,
    AlertTriangle,
    Boxes,
    Zap,
    Sparkles,
    Megaphone,
    Bell,
    Search,
    ShareGraph,
    Plug,
    HeartPulse,
    Sliders,
    Compass,
    Terminal,
    // Header / controls.
    Sun,
    Moon,
    Check,
    X,
    ChevronDown,
    ChevronRight,
    Plus,
    Send,
    Refresh,
    Filter,
    Clock,
    Mail,
    ExternalLink,
    AlertCircle,
    Info,
    Pencil,
    Trash,
    Tag,
    Eye,
    Download,
    Star,
}

impl Icon {
    /// The sidebar icon for a page key (shared by the shell and the
    /// command-palette-like surfaces).
    #[must_use]
    pub fn for_page(key: crate::pages::PageKey) -> Self {
        use crate::pages::PageKey as K;
        match key {
            K::Dashboard => Self::Dashboard,
            K::Inbox => Self::Inbox,
            K::Operations => Self::Activity,
            K::Customers => Self::Users,
            K::Organizations => Self::Building,
            K::Knowledge => Self::BookOpen,
            K::Reports => Self::BarChart,
            K::IssueRadar => Self::Target,
            K::Incidents => Self::AlertTriangle,
            K::CustomObjects => Self::Boxes,
            K::Automation => Self::Zap,
            K::AiCenter => Self::Sparkles,
            K::Outreach => Self::Megaphone,
            K::Notifications => Self::Bell,
            K::Search => Self::Search,
            K::SupportGraph => Self::ShareGraph,
            K::Connectors => Self::Plug,
            K::SyncHealth => Self::HeartPulse,
            K::Settings => Self::Sliders,
            K::Onboarding => Self::Compass,
            K::Shell => Self::Terminal,
        }
    }

    /// Allocate a `size` x `size` square and paint the icon in `color`.
    pub fn show(self, ui: &mut egui::Ui, size: f32, color: egui::Color32) -> egui::Response {
        let (id, rect) = ui.allocate_space(egui::vec2(size, size));
        self.paint_at(ui.painter(), rect, color);
        let response = ui.interact(rect, id, egui::Sense::hover());
        ui.advance_cursor_after_rect(rect);
        response
    }

    /// Paint the icon into an explicit rect (caller owns layout).
    pub fn paint_at(self, painter: &egui::Painter, rect: egui::Rect, color: egui::Color32) {
        // Uniform scale from the 24-grid into the rect, keeping the
        // stroke proportional so icons stay crisp at any size.
        let side = rect.width().min(rect.height());
        let scale = side / 24.0;
        let origin = rect.center() - egui::vec2(12.0 * scale, 12.0 * scale);
        let ctx = Paint {
            painter,
            scale,
            origin,
            color,
        };
        ctx.draw(self);
    }
}

/// Painting helper holding the grid transform for one icon.
struct Paint<'p> {
    painter: &'p egui::Painter,
    scale: f32,
    origin: egui::Pos2,
    color: egui::Color32,
}

impl Paint<'_> {
    fn pt(&self, p: P) -> egui::Pos2 {
        egui::pos2(
            self.origin.x + p[0] * self.scale,
            self.origin.y + p[1] * self.scale,
        )
    }

    fn stroke(&self) -> egui::Stroke {
        egui::Stroke::new(DESIGN_STROKE * self.scale, self.color)
    }

    /// A polyline through the given grid points (not closed).
    fn line(&self, pts: &[P]) {
        if pts.len() >= 2 {
            self.painter.add(egui::Shape::line(
                pts.iter().map(|p| self.pt(*p)).collect(),
                self.stroke(),
            ));
        }
    }

    /// A closed, stroked (optionally filled) polygon.
    fn poly(&self, pts: &[P], fill: bool) {
        let points: Vec<egui::Pos2> = pts.iter().map(|p| self.pt(*p)).collect();
        if fill {
            self.painter.add(egui::Shape::convex_polygon(
                points,
                self.color,
                egui::Stroke::NONE,
            ));
        } else {
            self.painter
                .add(egui::Shape::closed_line(points, self.stroke()));
        }
    }

    fn circle(&self, center: P, radius: f32) {
        self.painter
            .circle_stroke(self.pt(center), radius * self.scale, self.stroke());
    }

    fn dot(&self, center: P, radius: f32) {
        self.painter
            .circle_filled(self.pt(center), radius * self.scale, self.color);
    }

    fn rect(&self, x: f32, y: f32, w: f32, h: f32, corner: f32) {
        let min = self.pt([x, y]);
        let rect = egui::Rect::from_min_size(min, egui::vec2(w * self.scale, h * self.scale));
        self.painter.rect_stroke(
            rect,
            egui::CornerRadius::same((corner * self.scale).round() as u8),
            self.stroke(),
            egui::StrokeKind::Middle,
        );
    }

    /// Sample a quadratic bezier polyline.
    fn quad(&self, p0: P, p1: P, p2: P) -> Vec<P> {
        let mut pts = vec![p0];
        for i in 1..=10u16 {
            let t = f32::from(i) / 10.0;
            let u = 1.0 - t;
            pts.push([
                u * u * p0[0] + 2.0 * u * t * p1[0] + t * t * p2[0],
                u * u * p0[1] + 2.0 * u * t * p1[1] + t * t * p2[1],
            ]);
        }
        pts
    }

    fn draw(&self, icon: Icon) {
        match icon {
            Icon::Dashboard => {
                self.rect(3.0, 3.0, 7.5, 9.5, 1.5);
                self.rect(14.5, 3.0, 6.5, 6.5, 1.5);
                self.rect(14.5, 12.5, 6.5, 8.5, 1.5);
                self.rect(3.0, 15.5, 7.5, 5.5, 1.5);
            }
            Icon::Inbox => {
                // Tray: a rounded open-top box.
                let mut tray = vec![[3.0, 19.5], [3.0, 5.5]];
                tray.extend(self.quad([3.0, 4.0], [4.0, 3.0], [5.5, 3.0]));
                tray.push([18.5, 3.0]);
                tray.extend(self.quad([20.0, 3.0], [21.0, 4.0], [21.0, 5.5]));
                tray.push([21.0, 19.5]);
                tray.extend(self.quad([21.0, 21.0], [20.0, 21.0], [18.5, 21.0]));
                tray.push([5.5, 21.0]);
                tray.extend(self.quad([4.0, 21.0], [3.0, 20.0], [3.0, 19.5]));
                self.line(&tray);
                // Fold + contents line.
                self.line(&[
                    [3.0, 13.0],
                    [8.0, 13.0],
                    [10.5, 10.0],
                    [13.5, 10.0],
                    [16.0, 13.0],
                    [21.0, 13.0],
                ]);
            }
            Icon::Activity => {
                self.line(&[
                    [3.0, 12.0],
                    [7.0, 12.0],
                    [10.0, 4.5],
                    [14.0, 19.5],
                    [17.0, 12.0],
                    [21.0, 12.0],
                ]);
            }
            Icon::Users => {
                self.circle([9.0, 8.0], 3.6);
                // Shoulders of the front person.
                let mut s = self.quad([2.5, 20.5], [2.5, 15.0], [9.0, 15.0]);
                s.extend(self.quad([15.5, 15.0], [15.5, 20.5], [15.5, 20.5]));
                self.line(&s);
                // Back person: head + partial shoulder.
                self.circle([17.2, 9.2], 2.8);
                let mut b = self.quad([17.2, 14.8], [15.0, 15.0], [13.5, 15.8]);
                b.push([13.5, 20.5]);
                self.line(&b);
            }
            Icon::Building => {
                // Main tower.
                let mut t = vec![[4.5, 21.0], [4.5, 4.0]];
                t.extend(self.quad([4.5, 3.0], [5.0, 3.0], [5.5, 3.0]));
                t.push([13.5, 3.0]);
                t.extend(self.quad([14.5, 3.0], [14.5, 4.0], [14.5, 4.0]));
                t.push([14.5, 21.0]);
                self.line(&t);
                // Wing.
                self.line(&[[14.5, 9.0], [18.5, 9.0], [18.5, 21.0]]);
                // Windows.
                for (wx, wy) in [
                    (7.0, 7.0),
                    (11.0, 7.0),
                    (7.0, 11.0),
                    (11.0, 11.0),
                    (7.0, 15.0),
                    (11.0, 15.0),
                    (17.0, 13.0),
                    (17.0, 17.0),
                ] {
                    self.dot([wx, wy], 0.6);
                }
                // Door.
                self.line(&[[7.0, 21.0], [12.0, 21.0]]);
            }
            Icon::BookOpen => {
                // Spine.
                self.line(&[[12.0, 6.0], [12.0, 20.0]]);
                // Left page.
                let mut l = vec![[12.0, 6.0]];
                l.extend(self.quad([12.0, 6.0], [8.0, 4.0], [4.0, 4.0]));
                l.extend(self.quad([3.0, 4.0], [3.0, 5.0], [3.0, 5.0]));
                l.push([3.0, 18.0]);
                l.extend(self.quad([3.0, 19.0], [4.0, 19.0], [4.0, 19.0]));
                l.extend(self.quad([6.0, 19.0], [10.0, 17.5], [12.0, 20.0]));
                self.line(&l);
                // Right page (mirror).
                let mut r = vec![[12.0, 6.0]];
                r.extend(self.quad([12.0, 6.0], [16.0, 4.0], [20.0, 4.0]));
                r.extend(self.quad([21.0, 4.0], [21.0, 5.0], [21.0, 5.0]));
                r.push([21.0, 18.0]);
                r.extend(self.quad([21.0, 19.0], [20.0, 19.0], [20.0, 19.0]));
                r.extend(self.quad([18.0, 19.0], [14.0, 17.5], [12.0, 20.0]));
                self.line(&r);
            }
            Icon::BarChart => {
                self.line(&[[6.0, 20.0], [6.0, 12.0]]);
                self.line(&[[12.0, 20.0], [12.0, 4.5]]);
                self.line(&[[18.0, 20.0], [18.0, 9.5]]);
            }
            Icon::Target => {
                self.circle([12.0, 12.0], 8.5);
                self.circle([12.0, 12.0], 4.8);
                self.dot([12.0, 12.0], 1.3);
            }
            Icon::AlertTriangle => {
                self.poly(&[[12.0, 3.5], [21.0, 19.0], [3.0, 19.0]], false);
                self.line(&[[12.0, 9.0], [12.0, 14.0]]);
                self.dot([12.0, 17.0], 0.7);
            }
            Icon::Boxes => {
                self.rect(3.0, 3.0, 7.5, 7.5, 1.5);
                self.rect(13.5, 3.0, 7.5, 7.5, 1.5);
                self.rect(8.0, 13.5, 8.0, 7.5, 1.5);
            }
            Icon::Zap => {
                self.poly(
                    &[
                        [13.0, 2.5],
                        [4.5, 14.0],
                        [11.0, 14.0],
                        [10.5, 21.5],
                        [19.5, 10.0],
                        [13.0, 10.0],
                    ],
                    false,
                );
            }
            Icon::Sparkles => {
                self.poly(
                    &[
                        [12.0, 3.0],
                        [13.7, 10.3],
                        [21.0, 12.0],
                        [13.7, 13.7],
                        [12.0, 21.0],
                        [10.3, 13.7],
                        [3.0, 12.0],
                        [10.3, 10.3],
                    ],
                    false,
                );
                self.poly(
                    &[
                        [19.0, 2.5],
                        [19.6, 4.1],
                        [21.2, 4.7],
                        [19.6, 5.3],
                        [19.0, 6.9],
                        [18.4, 5.3],
                        [16.8, 4.7],
                        [18.4, 4.1],
                    ],
                    false,
                );
            }
            Icon::Megaphone => {
                self.poly(
                    &[[3.0, 11.0], [20.5, 4.0], [20.5, 18.0], [3.0, 11.0]],
                    false,
                );
                // Handle.
                let mut mouth = self.quad([5.2, 12.0], [5.0, 17.5], [7.5, 19.8]);
                self.line(&mouth);
                mouth.clear();
                self.line(&[[7.5, 19.8], [7.5, 20.5]]);
            }
            Icon::Bell => {
                // Dome.
                let mut dome = self.quad([6.0, 10.0], [6.0, 3.5], [12.0, 3.5]);
                dome.extend(self.quad([12.0, 3.5], [18.0, 3.5], [18.0, 10.0]));
                dome.push([18.0, 15.5]);
                dome.extend(self.quad([19.6, 17.0], [20.5, 19.5], [20.5, 19.5]));
                dome.push([3.5, 19.5]);
                dome.extend(self.quad([3.5, 19.5], [4.4, 17.0], [6.0, 15.5]));
                self.line(&dome);
                // Clapper.
                let mut clap = self.quad([9.4, 19.5], [9.8, 21.6], [12.0, 21.6]);
                clap.extend(self.quad([14.2, 21.6], [14.6, 19.5], [14.6, 19.5]));
                self.line(&clap);
            }
            Icon::Search => {
                self.circle([10.8, 10.8], 6.8);
                self.line(&[[15.8, 15.8], [21.0, 21.0]]);
            }
            Icon::ShareGraph => {
                self.circle([18.0, 5.2], 2.9);
                self.circle([6.0, 12.0], 2.9);
                self.circle([18.0, 18.8], 2.9);
                self.line(&[[8.8, 10.8], [15.2, 6.4]]);
                self.line(&[[8.8, 13.2], [15.2, 17.6]]);
            }
            Icon::Plug => {
                self.line(&[[9.5, 2.5], [9.5, 7.5]]);
                self.line(&[[14.5, 2.5], [14.5, 7.5]]);
                // Body.
                let mut body = vec![[7.0, 7.5], [17.0, 7.5]];
                body.extend(self.quad([17.0, 13.5], [12.0, 16.5], [7.0, 13.5]));
                body.push([7.0, 7.5]);
                self.line(&body);
                self.line(&[[12.0, 16.5], [12.0, 21.5]]);
            }
            Icon::HeartPulse => {
                let mut heart = vec![[12.0, 20.5]];
                heart.extend(self.quad([12.0, 20.5], [3.5, 15.0], [3.5, 10.5]));
                heart.extend(self.quad([3.5, 6.8], [7.5, 4.0], [10.0, 6.0]));
                heart.push([12.0, 8.0]);
                heart.extend(self.quad([14.0, 6.0], [17.5, 4.0], [20.0, 10.5]));
                heart.extend(self.quad([20.0, 15.0], [12.0, 20.5], [12.0, 20.5]));
                self.line(&heart);
                // Pulse.
                self.line(&[
                    [6.5, 12.0],
                    [9.5, 12.0],
                    [11.0, 9.5],
                    [13.0, 14.5],
                    [14.5, 12.0],
                    [17.5, 12.0],
                ]);
            }
            Icon::Sliders => {
                self.line(&[[3.5, 6.0], [20.5, 6.0]]);
                self.line(&[[3.5, 12.0], [20.5, 12.0]]);
                self.line(&[[3.5, 18.0], [20.5, 18.0]]);
                // Knobs (filled with the panel gap color would need a hole;
                // small filled dots read as knobs).
                self.dot([16.0, 6.0], 1.6);
                self.dot([8.0, 12.0], 1.6);
                self.dot([13.5, 18.0], 1.6);
            }
            Icon::Compass => {
                self.circle([12.0, 12.0], 9.0);
                self.poly(
                    &[[16.2, 7.8], [13.9, 13.9], [7.8, 16.2], [10.1, 10.1]],
                    true,
                );
            }
            Icon::Terminal => {
                self.rect(3.0, 4.5, 18.0, 15.0, 2.0);
                self.line(&[[7.0, 9.5], [10.0, 12.0], [7.0, 14.5]]);
                self.line(&[[12.5, 14.5], [16.5, 14.5]]);
            }
            Icon::Sun => {
                self.circle([12.0, 12.0], 4.2);
                let d = std::f32::consts::FRAC_1_SQRT_2;
                let rays: [(f32, f32); 8] = [
                    (0.0, -1.0),
                    (0.0, 1.0),
                    (-1.0, 0.0),
                    (1.0, 0.0),
                    (-d, -d),
                    (d, -d),
                    (-d, d),
                    (d, d),
                ];
                let (inner, outer) = (6.8f32, 9.4f32);
                for (dx, dy) in rays {
                    self.line(&[
                        [12.0 + dx * inner, 12.0 + dy * inner],
                        [12.0 + dx * outer, 12.0 + dy * outer],
                    ]);
                }
            }
            Icon::Moon => {
                let mut path = self.quad([20.5, 13.5], [17.0, 20.5], [8.5, 20.5]);
                path.extend(self.quad([3.5, 20.5], [3.5, 3.5], [8.5, 3.5]));
                path.extend(self.quad([14.5, 3.5], [20.5, 13.5], [20.5, 13.5]));
                self.line(&path);
            }
            Icon::Check => {
                self.line(&[[5.0, 13.0], [9.8, 17.5], [19.0, 6.5]]);
            }
            Icon::X => {
                self.line(&[[6.0, 6.0], [18.0, 18.0]]);
                self.line(&[[18.0, 6.0], [6.0, 18.0]]);
            }
            Icon::ChevronDown => {
                self.line(&[[6.0, 9.5], [12.0, 15.0], [18.0, 9.5]]);
            }
            Icon::ChevronRight => {
                self.line(&[[9.5, 6.0], [15.0, 12.0], [9.5, 18.0]]);
            }
            Icon::Plus => {
                self.line(&[[12.0, 5.0], [12.0, 19.0]]);
                self.line(&[[5.0, 12.0], [19.0, 12.0]]);
            }
            Icon::Send => {
                self.poly(
                    &[[21.0, 3.0], [3.5, 10.2], [10.3, 13.4], [13.4, 20.2]],
                    false,
                );
                self.line(&[[10.3, 13.4], [21.0, 3.0]]);
            }
            Icon::Refresh => {
                // Almost a full circle, with an arrowhead at the top.
                let mut pts = Vec::new();
                let (cx, cy, r) = (12.0f32, 12.0f32, 8.0f32);
                for i in 0..=26u16 {
                    let a = -90.0f32.to_radians() + f32::from(i) / 26.0 * 5.75;
                    pts.push([cx + a.cos() * r, cy + a.sin() * r]);
                }
                self.line(&pts);
                self.poly(
                    &[
                        [cx, cy - r - 2.6],
                        [cx - 4.2, cy - r + 1.8],
                        [cx + 4.2, cy - r + 1.8],
                    ],
                    true,
                );
            }
            Icon::Filter => {
                self.poly(
                    &[
                        [3.5, 4.5],
                        [20.5, 4.5],
                        [14.0, 11.5],
                        [14.0, 17.0],
                        [10.0, 20.0],
                        [10.0, 11.5],
                    ],
                    false,
                );
            }
            Icon::Clock => {
                self.circle([12.0, 12.0], 8.6);
                self.line(&[[12.0, 7.2], [12.0, 12.0], [15.4, 13.8]]);
            }
            Icon::Mail => {
                self.rect(3.0, 5.0, 18.0, 14.0, 2.0);
                self.line(&[[3.6, 7.0], [12.0, 13.0], [20.4, 7.0]]);
            }
            Icon::ExternalLink => {
                self.line(&[
                    [13.5, 11.0],
                    [13.5, 4.5],
                    [4.5, 4.5],
                    [4.5, 19.5],
                    [19.5, 19.5],
                    [19.5, 10.5],
                ]);
                self.line(&[[11.0, 13.0], [20.5, 3.5]]);
                self.line(&[[15.5, 3.5], [20.5, 3.5], [20.5, 8.5]]);
            }
            Icon::AlertCircle => {
                self.circle([12.0, 12.0], 8.6);
                self.line(&[[12.0, 8.0], [12.0, 13.0]]);
                self.dot([12.0, 16.2], 0.7);
            }
            Icon::Info => {
                self.circle([12.0, 12.0], 8.6);
                self.line(&[[12.0, 11.0], [12.0, 16.0]]);
                self.dot([12.0, 7.8], 0.7);
            }
            Icon::Pencil => {
                let mut shaft = vec![[4.5, 19.5], [5.4, 15.5], [15.5, 5.4]];
                shaft.extend(self.quad([16.0, 4.9], [19.1, 8.0], [19.1, 8.0]));
                shaft.push([9.0, 18.1]);
                shaft.extend(self.quad([6.4, 19.2], [4.5, 19.5], [4.5, 19.5]));
                self.line(&shaft);
                self.line(&[[13.9, 7.0], [17.0, 10.1]]);
            }
            Icon::Trash => {
                self.line(&[[4.5, 6.5], [19.5, 6.5]]);
                self.line(&[[9.5, 6.5], [9.5, 3.5], [14.5, 3.5], [14.5, 6.5]]);
                let mut can = vec![[6.5, 6.5], [7.6, 20.0]];
                can.extend(self.quad([8.0, 20.5], [9.0, 20.5], [9.0, 20.5]));
                can.push([15.0, 20.5]);
                can.extend(self.quad([16.0, 20.5], [16.4, 20.0], [16.4, 20.0]));
                can.push([17.5, 6.5]);
                self.line(&can);
                self.line(&[[10.2, 10.5], [10.6, 16.5]]);
                self.line(&[[13.8, 10.5], [13.4, 16.5]]);
            }
            Icon::Tag => {
                let mut tag = vec![[3.5, 3.5], [11.5, 3.5]];
                tag.extend(self.quad([12.3, 3.5], [12.9, 3.8], [13.6, 4.5]));
                tag.push([20.5, 11.4]);
                tag.extend(self.quad([21.2, 12.1], [21.2, 13.1], [20.5, 13.8]));
                tag.push([13.8, 20.5]);
                tag.extend(self.quad([13.1, 21.2], [12.1, 21.2], [11.4, 20.5]));
                tag.push([4.5, 13.6]);
                tag.extend(self.quad([3.8, 12.9], [3.5, 12.3], [3.5, 11.5]));
                tag.push([3.5, 3.5]);
                self.line(&tag);
                self.dot([7.5, 7.5], 1.0);
            }
            Icon::Eye => {
                let mut eye = self.quad([3.0, 12.0], [6.0, 5.5], [12.0, 5.5]);
                eye.extend(self.quad([18.0, 5.5], [21.0, 12.0], [21.0, 12.0]));
                eye.extend(self.quad([18.0, 18.5], [12.0, 18.5], [12.0, 18.5]));
                eye.extend(self.quad([6.0, 18.5], [3.0, 12.0], [3.0, 12.0]));
                self.line(&eye);
                self.circle([12.0, 12.0], 3.2);
            }
            Icon::Download => {
                self.line(&[[12.0, 4.0], [12.0, 15.0]]);
                self.line(&[[7.5, 11.0], [12.0, 15.5], [16.5, 11.0]]);
                self.line(&[[4.5, 19.5], [19.5, 19.5]]);
            }
            Icon::Star => {
                let mut star = Vec::new();
                let (cx, cy) = (12.0f32, 12.0f32);
                for i in 0..10u16 {
                    let a =
                        -90.0f32.to_radians() + f32::from(i) * std::f32::consts::FRAC_PI_2 / 2.5;
                    let r = if i % 2 == 0 { 9.0 } else { 3.8 };
                    star.push([cx + a.cos() * r, cy + a.sin() * r]);
                }
                self.poly(&star, false);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_page_key_has_an_icon() {
        for key in crate::pages::PageKey::all() {
            let _ = Icon::for_page(key);
        }
    }

    #[test]
    fn every_icon_paints_without_panicking() {
        let ctx = egui::Context::default();
        ctx.run_pass(egui::RawInput::default(), |ctx| {
            ctx.root_ui(|ui| {
                egui::CentralPanel::default().show(ui, |ui| {
                    let all = [
                        Icon::Dashboard,
                        Icon::Inbox,
                        Icon::Activity,
                        Icon::Users,
                        Icon::Building,
                        Icon::BookOpen,
                        Icon::BarChart,
                        Icon::Target,
                        Icon::AlertTriangle,
                        Icon::Boxes,
                        Icon::Zap,
                        Icon::Sparkles,
                        Icon::Megaphone,
                        Icon::Bell,
                        Icon::Search,
                        Icon::ShareGraph,
                        Icon::Plug,
                        Icon::HeartPulse,
                        Icon::Sliders,
                        Icon::Compass,
                        Icon::Terminal,
                        Icon::Sun,
                        Icon::Moon,
                        Icon::Check,
                        Icon::X,
                        Icon::ChevronDown,
                        Icon::ChevronRight,
                        Icon::Plus,
                        Icon::Send,
                        Icon::Refresh,
                        Icon::Filter,
                        Icon::Clock,
                        Icon::Mail,
                        Icon::ExternalLink,
                        Icon::AlertCircle,
                        Icon::Info,
                        Icon::Pencil,
                        Icon::Trash,
                        Icon::Tag,
                        Icon::Eye,
                        Icon::Download,
                        Icon::Star,
                    ];
                    for icon in all {
                        let _ = icon.show(ui, 16.0, egui::Color32::BLACK);
                    }
                });
            });
        })
        .drop_without_applying_deltas();
    }
}
