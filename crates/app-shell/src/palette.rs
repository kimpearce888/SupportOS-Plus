use eframe::egui;
use spp_core::db::DbHandle;

use crate::pages::NavRequest;

pub(crate) const MAX_HITS: usize = 12;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PaletteHit {
    pub scope: String,
    pub id: i64,
    pub title: String,
    pub snippet: String,
    pub href: String,
}

impl PaletteHit {
    #[must_use]
    pub(crate) fn from_search_hit(hit: &spp_core::search::SearchHit) -> Self {
        Self {
            scope: hit.scope.clone(),
            id: hit.id,
            title: hit.title.clone(),
            snippet: hit.snippet.clone(),
            href: hit.href.clone(),
        }
    }
}

#[must_use]
pub(crate) fn take_hits(hits: &[PaletteHit]) -> &[PaletteHit] {
    &hits[..hits.len().min(MAX_HITS)]
}

#[must_use]
pub(crate) fn move_selection(selected: usize, len: usize, delta: i64) -> usize {
    if len == 0 {
        return 0;
    }
    let next = selected as i64 + delta;
    next.clamp(0, len as i64 - 1) as usize
}

#[must_use]
pub(crate) fn href_to_nav(href: &str) -> Option<NavRequest> {
    let rest = href.strip_prefix('/')?;
    if let Some(id) = rest.strip_prefix("inbox/conversation/") {
        return id
            .parse::<i64>()
            .ok()
            .map(|_| NavRequest::Page(crate::pages::PageKey::Inbox));
    }
    if rest.starts_with("customers") {
        return Some(NavRequest::Page(crate::pages::PageKey::Customers));
    }
    if rest.starts_with("knowledge") {
        return Some(NavRequest::Page(crate::pages::PageKey::Knowledge));
    }
    if rest.starts_with("issues") {
        return Some(NavRequest::Page(crate::pages::PageKey::IssueRadar));
    }
    None
}

pub(crate) struct CommandPalette {
    open: bool,
    query: String,
    hits: Vec<PaletteHit>,
    selected: usize,
    wants_focus: bool,
    palette_take: bool,
    last_window_rect: egui::Rect,
    just_opened: bool,
}

impl CommandPalette {
    #[must_use]
    pub fn new() -> Self {
        Self {
            open: false,
            query: String::new(),
            hits: Vec::new(),
            selected: 0,
            wants_focus: false,
            palette_take: false,
            last_window_rect: egui::Rect::NOTHING,
            just_opened: false,
        }
    }

    pub fn open(&mut self) {
        self.open = true;
        self.query.clear();
        self.hits.clear();
        self.selected = 0;
        self.wants_focus = true;
        self.just_opened = true;
    }

    pub(crate) fn toggle(&mut self) {
        if self.open {
            self.close();
        } else {
            self.open();
        }
    }

    pub fn close(&mut self) {
        self.open = false;
    }

    pub(crate) fn toggle_shortcut(&mut self, ctx: &egui::Context) {
        let pressed = (ctx.input(|i| i.modifiers.command || i.modifiers.ctrl)
            && ctx.input(|i| i.key_pressed(egui::Key::K)))
            || ctx.input(|i| i.key_pressed(egui::Key::Escape) && self.open);
        if pressed {
            if self.open {
                self.close();
            } else {
                self.open();
            }
        }
    }

    fn run_search(&mut self, db: &DbHandle) {
        let query = self.query.trim().to_string();
        if query.is_empty() {
            self.hits.clear();
            self.selected = 0;
            return;
        }
        let result = db.with_read(|conn| {
            spp_core::search::search(
                conn,
                &query,
                "all",
                &spp_core::search::SearchFilters::default(),
            )
        });
        self.hits = result
            .map(|response| {
                response
                    .hits
                    .iter()
                    .map(PaletteHit::from_search_hit)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        self.selected = 0;
    }

    pub fn ui(&mut self, ctx: &egui::Context, db: &DbHandle) -> Option<NavRequest> {
        self.toggle_shortcut(ctx);
        if !self.open {
            return None;
        }
        let screen = ctx.input(|i| i.viewport_rect());
        let catcher = egui::Area::new(egui::Id::unique("spp_command_palette_catcher"))
            .order(egui::Order::Foreground)
            .movable(false)
            .fixed_pos(screen.min)
            .show(ctx, |ui| {
                ui.allocate_response(screen.size(), egui::Sense::click())
            })
            .inner;
        if !self.just_opened && catcher.clicked() {
            self.close();
        }
        if !self.just_opened
            && ctx.input(|i| i.pointer.any_click())
            && ctx
                .pointer_interact_pos()
                .is_some_and(|p| !self.last_window_rect.contains(p))
        {
            self.close();
        }
        self.just_opened = false;
        if !self.open {
            return None;
        }
        let mut nav = None;
        let mut query_changed = false;
        let frame_tokens = crate::design::tokens_for(ctx);
        let frame_fill = frame_tokens.card;
        let frame_border = frame_tokens.border;
        let icon_muted = frame_tokens.muted_foreground;
        let win = egui::Window::new("Command Palette")
            .order(egui::Order::Foreground)
            .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 80.0))
            .fixed_size([560.0, 360.0])
            .collapsible(false)
            .resizable(false)
            .fade_in(false)
            .title_bar(false)
            .frame(
                egui::Frame::new()
                    .fill(frame_fill)
                    .stroke(egui::Stroke::new(1.0, frame_border))
                    .corner_radius(12)
                    .inner_margin(egui::Margin::same(12))
                    .shadow(egui::Shadow {
                        offset: [0, 8],
                        blur: 32,
                        spread: 0,
                        color: frame_tokens.shadow,
                    }),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let (id, rect) = ui.allocate_space(egui::vec2(17.0, 17.0));
                    crate::icons::Icon::Search.paint_at(ui.painter(), rect, icon_muted);
                    ui.advance_cursor_after_rect(rect);
                    let _ = id;
                    let mut query = self.query.clone();
                    let response = ui.add_sized(
                        [ui.available_width(), 30.0],
                        egui::TextEdit::singleline(&mut query)
                            .hint_text("search tickets, customers, knowledge, issues…"),
                    );
                    if self.wants_focus {
                        response.request_focus();
                        self.wants_focus = false;
                    }
                    if response.changed() {
                        self.query = query.clone();
                        query_changed = true;
                    }
                    let take =
                        response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    self.palette_take = take;
                    if take && (query_changed || take_hits(&self.hits).is_empty()) {
                        response.request_focus();
                    }
                });
                let take = self.palette_take;
                ui.add_space(6.0);
                let hits = take_hits(&self.hits);
                if hits.is_empty() {
                    crate::design::empty_state(
                        ui,
                        crate::icons::Icon::Search,
                        if self.query.trim().is_empty() {
                            "Type to search"
                        } else {
                            "No hits"
                        },
                        if self.query.trim().is_empty() {
                            "Search tickets, customers, knowledge, and issues."
                        } else {
                            "Nothing matches — try a shorter query."
                        },
                    );
                } else {
                    egui::ScrollArea::vertical()
                        .id_salt("palette-hits")
                        .show_rows(ui, 40.0, hits.len(), |ui, range| {
                            for (idx, hit) in hits[range.clone()].iter().enumerate() {
                                let global_idx = range.start + idx;
                                let selected = self.selected == global_idx;
                                if palette_hit_row(ui, hit, selected).clicked() {
                                    nav = href_to_nav(&hit.href);
                                    self.open = false;
                                }
                            }
                        });
                    ui.add_space(4.0);
                    crate::design::hint_text(ui, "↑ ↓ to move · Enter to open · Esc to close");
                }
                if take
                    && !query_changed
                    && let Some(hit) = hits.get(self.selected)
                {
                    nav = href_to_nav(&hit.href);
                    self.open = false;
                }
                if ui.input(|i| i.key_pressed(egui::Key::ArrowDown)) {
                    self.selected = move_selection(self.selected, hits.len(), 1);
                }
                if ui.input(|i| i.key_pressed(egui::Key::ArrowUp)) {
                    self.selected = move_selection(self.selected, hits.len(), -1);
                }
            });
        if let Some(win) = &win {
            self.last_window_rect = win.response.rect;
        }
        if query_changed {
            self.run_search(db);
        }
        nav
    }
}

fn palette_hit_row(ui: &mut egui::Ui, hit: &PaletteHit, selected: bool) -> egui::Response {
    let t = crate::design::tokens(ui);
    let (auto_id, rect) = ui.allocate_space(egui::vec2(ui.available_width(), 40.0));
    let response = ui.interact(
        rect,
        auto_id.with(("palette-hit", hit.scope.as_str(), hit.id)),
        egui::Sense::click(),
    );
    let fill = if selected {
        t.selection
    } else if response.hovered() {
        t.accent
    } else {
        egui::Color32::TRANSPARENT
    };
    if fill != egui::Color32::TRANSPARENT {
        ui.painter()
            .rect_filled(rect, egui::CornerRadius::same(8), fill);
    }
    let title = egui::WidgetText::from(
        egui::RichText::new(hit.title.as_str())
            .size(13.5)
            .strong()
            .color(if selected {
                t.selection_foreground
            } else {
                t.foreground
            }),
    )
    .into_galley(
        ui,
        Some(egui::TextWrapMode::Truncate),
        rect.width() - 96.0,
        egui::TextStyle::Button,
    );
    let scope_galley = egui::WidgetText::from(
        egui::RichText::new(hit.scope.as_str())
            .size(11.0)
            .strong()
            .color(t.info_foreground),
    )
    .into_galley(
        ui,
        Some(egui::TextWrapMode::Extend),
        f32::INFINITY,
        egui::TextStyle::Small,
    );
    let scope_w = scope_galley.size().x + 16.0;
    let badge_rect = egui::Rect::from_min_max(
        egui::pos2(
            rect.min.x + 8.0,
            rect.center().y - scope_galley.size().y / 2.0 - 3.0,
        ),
        egui::pos2(
            rect.min.x + 8.0 + scope_w,
            rect.center().y + scope_galley.size().y / 2.0 + 3.0,
        ),
    );
    ui.painter()
        .rect_filled(badge_rect, egui::CornerRadius::same(6), t.info_soft);
    ui.painter().galley(
        egui::pos2(
            badge_rect.min.x + 8.0,
            badge_rect.center().y - scope_galley.size().y / 2.0,
        ),
        scope_galley,
        t.info_foreground,
    );
    ui.painter().galley(
        egui::pos2(
            badge_rect.max.x + 10.0,
            rect.center().y - title.size().y / 2.0,
        ),
        title,
        if selected {
            t.selection_foreground
        } else {
            t.foreground
        },
    );
    if !hit.snippet.is_empty() {
        let snippet = egui::WidgetText::from(
            egui::RichText::new(hit.snippet.as_str())
                .small()
                .color(t.muted_foreground),
        )
        .into_galley(
            ui,
            Some(egui::TextWrapMode::Truncate),
            rect.width() - 16.0,
            egui::TextStyle::Small,
        );
        ui.painter().galley(
            egui::pos2(rect.min.x + 8.0, rect.max.y - snippet.size().y - 6.0),
            snippet,
            t.muted_foreground,
        );
    }
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::Role::Button,
            ui.is_enabled(),
            selected,
            hit.title.clone(),
        )
    });
    response
}

impl Default for CommandPalette {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testsupport::fixture;

    #[test]
    fn take_hits_caps_at_twelve() {
        let hits: Vec<PaletteHit> = (0..20)
            .map(|i| PaletteHit {
                scope: "tickets".into(),
                id: i,
                title: format!("hit {i}"),
                snippet: String::new(),
                href: "/inbox/conversation/1".into(),
            })
            .collect();
        assert_eq!(take_hits(&hits).len(), MAX_HITS);
        assert_eq!(take_hits(&hits)[0].id, 0);
        assert_eq!(take_hits(&hits)[MAX_HITS - 1].id, MAX_HITS as i64 - 1);
    }

    #[test]
    fn selection_moves_and_clamps() {
        assert_eq!(move_selection(0, 0, 1), 0);
        assert_eq!(move_selection(0, 5, -3), 0);
        assert_eq!(move_selection(2, 5, 2), 4);
        assert_eq!(move_selection(4, 5, 1), 4, "clamped to len-1");
        assert_eq!(move_selection(3, 5, -2), 1);
    }

    #[test]
    fn hrefs_map_to_pages() {
        assert!(matches!(
            href_to_nav("/inbox/conversation/42"),
            Some(NavRequest::Page(crate::pages::PageKey::Inbox))
        ));
        assert!(matches!(
            href_to_nav("/customers/9"),
            Some(NavRequest::Page(crate::pages::PageKey::Customers))
        ));
        assert!(matches!(
            href_to_nav("/knowledge?doc=3"),
            Some(NavRequest::Page(crate::pages::PageKey::Knowledge))
        ));
        assert!(matches!(
            href_to_nav("/issues/known/7"),
            Some(NavRequest::Page(crate::pages::PageKey::IssueRadar))
        ));
        assert!(href_to_nav("/unknown/page").is_none());
        assert!(href_to_nav("not-a-href").is_none());
    }

    #[test]
    fn palette_search_finds_seeded_conversations() {
        let db = fixture(vec![(
            "conv",
            1,
            "Export broken",
            "active",
            "email",
            1,
            5,
            9,
        )]);
        db.with_write(|conn| spp_core::search::rebuild_indexes(conn))
            .expect("rebuild");
        let mut palette = CommandPalette::new();
        palette.open();
        palette.query = "Export".into();
        palette.run_search(&db);
        assert!(!palette.hits.is_empty());
        assert!(palette.hits.iter().any(|h| h.title.contains("Export")));
        assert!(href_to_nav(&palette.hits[0].href).is_some());

        palette.query = String::new();
        palette.run_search(&db);
        assert!(palette.hits.is_empty());
    }

    fn seeded_palette_ctx() -> (egui::Context, CommandPalette, DbHandle) {
        let db = fixture(vec![(
            "conv",
            1,
            "Export broken",
            "active",
            "email",
            1,
            5,
            9,
        )]);
        db.with_write(|conn| spp_core::search::rebuild_indexes(conn))
            .expect("rebuild");
        let ctx = egui::Context::default();
        let mut palette = CommandPalette::new();
        palette.open();
        (ctx, palette, db)
    }

    #[test]
    fn palette_takes_focus_on_open_and_keystrokes_reach_the_query() {
        let (ctx, mut palette, db) = seeded_palette_ctx();

        ctx.run_pass(egui::RawInput::default(), |ctx| {
            palette.ui(ctx, &db);
        })
        .drop_without_applying_deltas();
        assert!(
            ctx.memory(|mem| mem.focused().is_some()),
            "DEFECT-03: the query field must take keyboard focus when the palette opens"
        );

        let mut typing = egui::RawInput::default();
        typing.events.push(egui::Event::Text("Export".into()));
        ctx.run_pass(typing, |ctx| {
            palette.ui(ctx, &db);
        })
        .drop_without_applying_deltas();
        assert_eq!(palette.query, "Export");
        assert!(
            !palette.hits.is_empty(),
            "the typed query must produce hits on the seeded fixture"
        );
    }

    #[test]
    fn palette_enter_confirms_the_selected_hit() {
        let (ctx, mut palette, db) = seeded_palette_ctx();

        ctx.run_pass(egui::RawInput::default(), |ctx| {
            palette.ui(ctx, &db);
        })
        .drop_without_applying_deltas();
        let mut typing = egui::RawInput::default();
        typing.events.push(egui::Event::Text("Export".into()));
        ctx.run_pass(typing, |ctx| {
            palette.ui(ctx, &db);
        })
        .drop_without_applying_deltas();

        let mut enter = egui::RawInput::default();
        enter.events.push(egui::Event::Key {
            key: egui::Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        });
        let mut nav = None;
        ctx.run_pass(enter, |ctx| {
            nav = palette.ui(ctx, &db);
        })
        .drop_without_applying_deltas();

        assert!(
            matches!(nav, Some(NavRequest::Page(crate::pages::PageKey::Inbox))),
            "DEFECT-02: Enter must confirm the selected palette hit"
        );
        assert!(!palette.open);
    }

    #[test]
    fn audit133_same_frame_enter_confirms_nothing_stale() {
        let (ctx, mut palette, db) = seeded_palette_ctx();

        ctx.run_pass(egui::RawInput::default(), |ctx| {
            palette.ui(ctx, &db);
        })
        .drop_without_applying_deltas();

        let mut typing = egui::RawInput::default();
        typing.events.push(egui::Event::Text("Export".into()));
        ctx.run_pass(typing, |ctx| {
            palette.ui(ctx, &db);
        })
        .drop_without_applying_deltas();
        assert!(!palette.hits.is_empty(), "the fixture matches Export");

        let mut type_and_enter = egui::RawInput::default();
        type_and_enter.events.push(egui::Event::Text("zz".into()));
        type_and_enter.events.push(egui::Event::Key {
            key: egui::Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        });
        let mut nav = None;
        ctx.run_pass(type_and_enter, |ctx| {
            nav = palette.ui(ctx, &db);
        })
        .drop_without_applying_deltas();
        assert!(nav.is_none(), "the stale hit must not be confirmed");
        assert!(palette.open, "the palette stays open for fresh hits");
        assert_eq!(palette.query, "Exportzz", "the new query was recorded");

        assert!(palette.hits.is_empty(), "Exportzz matches nothing");
        let mut enter = egui::RawInput::default();
        enter.events.push(egui::Event::Key {
            key: egui::Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        });
        let mut nav2 = None;
        ctx.run_pass(enter, |ctx| {
            nav2 = palette.ui(ctx, &db);
        })
        .drop_without_applying_deltas();
        assert!(nav2.is_none(), "nothing to confirm on the fresh query");
        assert!(palette.open, "a dry Enter keeps the palette open");
    }

    #[test]
    fn audit133_clicking_outside_the_palette_closes_it() {
        let (ctx, mut palette, db) = seeded_palette_ctx();

        let victim_pos = egui::pos2(10.0, 300.0);
        let victim_clicked = std::rc::Rc::new(std::cell::Cell::new(false));
        let render_victim = |ctx: &egui::Context, flag: &std::cell::Cell<bool>| {
            let clicked = egui::Area::new(egui::Id::unique("m19_victim"))
                .fixed_pos(victim_pos)
                .show(ctx, |ui| {
                    ui.allocate_exact_size(egui::vec2(80.0, 30.0), egui::Sense::click())
                })
                .inner
                .1
                .clicked();
            if clicked {
                flag.set(true);
            }
        };

        ctx.run_pass(egui::RawInput::default(), |ctx| {
            render_victim(ctx, &victim_clicked);
            palette.ui(ctx, &db);
        })
        .drop_without_applying_deltas();
        assert!(palette.open);

        let mut move_pointer = egui::RawInput::default();
        move_pointer
            .events
            .push(egui::Event::PointerMoved(victim_pos));
        ctx.run_pass(move_pointer, |ctx| {
            render_victim(ctx, &victim_clicked);
            palette.ui(ctx, &db);
        })
        .drop_without_applying_deltas();

        let mut press = egui::RawInput::default();
        press.events.push(egui::Event::PointerButton {
            pos: victim_pos,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        });
        ctx.run_pass(press, |ctx| {
            render_victim(ctx, &victim_clicked);
            palette.ui(ctx, &db);
        })
        .drop_without_applying_deltas();
        assert!(palette.open, "the press alone does not close");

        let mut release = egui::RawInput::default();
        release.events.push(egui::Event::PointerButton {
            pos: victim_pos,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        });
        ctx.run_pass(release, |ctx| {
            render_victim(ctx, &victim_clicked);
            palette.ui(ctx, &db);
        })
        .drop_without_applying_deltas();
        assert!(!palette.open, "an outside click closes the palette");
        assert!(
            !victim_clicked.get(),
            "M19: the dismissing click must be absorbed by the catcher, \
             not pressed through onto the widget underneath"
        );
    }

    #[test]
    fn audit72_enter_on_zero_hits_keeps_the_query_focused() {
        let (ctx, mut palette, db) = seeded_palette_ctx();

        ctx.run_pass(egui::RawInput::default(), |ctx| {
            palette.ui(ctx, &db);
        })
        .drop_without_applying_deltas();

        let mut typing = egui::RawInput::default();
        typing.events.push(egui::Event::Text("zzzznomatch".into()));
        ctx.run_pass(typing, |ctx| {
            palette.ui(ctx, &db);
        })
        .drop_without_applying_deltas();
        assert!(
            palette.hits.is_empty(),
            "the fixture has no hit for this query"
        );

        let mut enter = egui::RawInput::default();
        enter.events.push(egui::Event::Key {
            key: egui::Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        });
        let mut nav = None;
        ctx.run_pass(enter, |ctx| {
            nav = palette.ui(ctx, &db);
        })
        .drop_without_applying_deltas();
        assert!(nav.is_none(), "nothing to confirm — no navigation");
        assert!(palette.open, "the palette stays open on a dry Enter");

        let mut more = egui::RawInput::default();
        more.events.push(egui::Event::Text("x".into()));
        ctx.run_pass(more, |ctx| {
            palette.ui(ctx, &db);
        })
        .drop_without_applying_deltas();
        assert_eq!(
            palette.query, "zzzznomatchx",
            "keystrokes after a dry Enter keep reaching the query field"
        );
    }
}
