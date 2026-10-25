//! Command palette (port of the web `components/command_palette.rs` and
//! `pages/command_palette.rs`).
//!
//! Ctrl/Cmd+K opens a global overlay: one text field debounced through the
//! local search engine (`spp_core::search::search`, the `/api/search`
//! route without the HTTP hop), arrow keys move the selection, Enter
//! resolves the hit's web href (`/inbox/conversation/{id}`,
//! `/customers/{id}`, `/knowledge?doc={id}`) into a
//! [`NavRequest`](crate::pages::NavRequest) the shell applies.

use eframe::egui;
use spp_core::db::DbHandle;

use crate::pages::NavRequest;

/// Palette hit cap (web: `MAX_HITS`).
pub const MAX_HITS: usize = 12;

/// One palette hit (port of the web `PaletteHit`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaletteHit {
    pub scope: String,
    pub id: i64,
    pub title: String,
    pub snippet: String,
    pub href: String,
}

impl PaletteHit {
    /// Parse from a search-hit JSON row.
    #[must_use]
    pub fn from_json(v: &serde_json::Value) -> Self {
        Self {
            scope: v
                .get("scope")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_string(),
            id: v.get("id").and_then(serde_json::Value::as_i64).unwrap_or(0),
            title: v
                .get("title")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_string(),
            snippet: v
                .get("snippet")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_string(),
            href: v
                .get("href")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_string(),
        }
    }

    /// Build from a typed search hit (the in-process search result).
    #[must_use]
    pub fn from_search_hit(hit: &spp_core::search::SearchHit) -> Self {
        Self {
            scope: hit.scope.clone(),
            id: hit.id,
            title: hit.title.clone(),
            snippet: hit.snippet.clone(),
            href: hit.href.clone(),
        }
    }
}

/// Cap the hit list (web: `take_hits`).
#[must_use]
pub fn take_hits(hits: &[PaletteHit]) -> &[PaletteHit] {
    &hits[..hits.len().min(MAX_HITS)]
}

/// Move the selection by `delta`, clamped (web: `move_selection`).
#[must_use]
pub fn move_selection(selected: usize, len: usize, delta: i64) -> usize {
    if len == 0 {
        return 0;
    }
    let next = selected as i64 + delta;
    next.clamp(0, len as i64 - 1) as usize
}

/// Resolve a web href into a navigation request (the router navigation
/// the web palette performed).
#[must_use]
pub fn href_to_nav(href: &str) -> Option<NavRequest> {
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

/// The palette overlay state.
pub struct CommandPalette {
    open: bool,
    query: String,
    hits: Vec<PaletteHit>,
    selected: usize,
    /// Set by [`Self::open`] so the next rendered pass requests keyboard
    /// focus for the query field. DEFECT-03 (qa-audit/20261019T050847Z):
    /// the palette used to open unfocused and silently swallowed
    /// keystrokes until the user clicked inside the input.
    wants_focus: bool,
    /// Deferred Enter handling: the query row renders inside a nested
    /// closure, so the take flag is stored here and read after the row.
    palette_take: bool,
    /// FIND-011: the palette window's on-screen rect from the last
    /// rendered frame — clicks outside it dismiss the overlay. The old
    /// full-screen Area catcher's hit-testing was runtime-disproven
    /// (clicks fell through to the UI underneath at most positions), so
    /// dismissal now keys off the window rect directly: layer-independent
    /// by construction.
    last_window_rect: egui::Rect,
    /// FIND-011: set by [`Self::open`] so the very click that opened the
    /// palette (same frame) cannot immediately dismiss it again.
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

    /// Whether the overlay is open.
    #[must_use]
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Open the palette.
    pub fn open(&mut self) {
        self.open = true;
        self.query.clear();
        self.hits.clear();
        self.selected = 0;
        self.wants_focus = true;
        self.just_opened = true;
    }

    /// Toggle between open and closed. FIND-011: the mouse triggers only
    /// ever called [`Self::open`], so with the palette open they could
    /// never dismiss it.
    pub fn toggle(&mut self) {
        if self.open {
            self.close();
        } else {
            self.open();
        }
    }

    /// Close the palette.
    pub fn close(&mut self) {
        self.open = false;
    }

    /// Toggle from a Ctrl/Cmd+K key press handled by the shell.
    pub fn toggle_shortcut(&mut self, ctx: &egui::Context) {
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
        // The web debounced 200ms and raced generations; the local query
        // is fast enough to run per keystroke change.
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

    /// Render the overlay; returns a navigation request when Enter was
    /// pressed on a hit.
    pub fn ui(&mut self, ctx: &egui::Context, db: &DbHandle) -> Option<NavRequest> {
        self.toggle_shortcut(ctx);
        if !self.open {
            return None;
        }
        // FIND-011: dismiss on any click OUTSIDE the palette window rect.
        // The previous full-screen Area catcher (clean-audit item 133)
        // claimed "the catcher takes everything else", but the runtime
        // click matrix proved the opposite at most positions — clicks
        // fell through to the UI underneath while the overlay stayed open
        // (one even spawned a real sync action). A pointer-position check
        // against the remembered window rect is layer-independent by
        // construction. The overlay stays non-modal (that is what runtime
        // actually did); the mouse triggers now `toggle()` so they dismiss
        // it too.
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
                        color: egui::Color32::from_black_alpha(60),
                    }),
            )
            .show(ctx, |ui| {
                // Query row with a leading search icon.
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
                        // DEFECT-03: open with the keyboard already in the
                        // query field, like the web palette auto-focus.
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
                    // Clean-audit item 72: egui 0.37 single-line TextEdits
                    // surrender focus on Enter — with nothing to confirm
                    // (zero hits) the field used to sit defocused and every
                    // following keystroke was swallowed until the user
                    // clicked back in. Grab the focus right back so typing
                    // continues (a hit-bearing Enter keeps the old
                    // confirm-and-close path).
                    // Clean-audit item 133: a same-frame Enter after a
                    // query change is suppressed below — the field keeps
                    // focus so Enter can be pressed again on fresh hits.
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
                    // Clean-audit item 133: an Enter processed in the same
                    // frame the query changed must not confirm the PREVIOUS
                    // query's result set — the search for the new text only
                    // runs after this window closes. Suppress the stale
                    // confirm; the fresh hits land next frame.
                    && !query_changed
                    && let Some(hit) = hits.get(self.selected)
                {
                    nav = href_to_nav(&hit.href);
                    self.open = false;
                }
                // Arrow-key navigation.
                if ui.input(|i| i.key_pressed(egui::Key::ArrowDown)) {
                    self.selected = move_selection(self.selected, hits.len(), 1);
                }
                if ui.input(|i| i.key_pressed(egui::Key::ArrowUp)) {
                    self.selected = move_selection(self.selected, hits.len(), -1);
                }
            });
        // FIND-011: remember the window rect for the outside-click check
        // (the anchor and size are fixed, so last frame's rect is current).
        if let Some(win) = &win {
            self.last_window_rect = win.response.rect;
        }
        if query_changed {
            self.run_search(db);
        }
        nav
    }
}

/// One full-width palette hit row (scope badge + title + muted snippet).
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
    // Scope badge on the left, title next, snippet muted below.
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

    #[test]
    fn hit_parsing_reads_the_json_shape() {
        let v = serde_json::json!({
            "scope": "tickets",
            "id": 5,
            "title": "Login broken",
            "snippet": "customer cannot log in",
            "href": "/inbox/conversation/5"
        });
        let hit = PaletteHit::from_json(&v);
        assert_eq!(hit.scope, "tickets");
        assert_eq!(hit.id, 5);
        assert_eq!(hit.title, "Login broken");
        assert_eq!(hit.snippet, "customer cannot log in");
        let empty = PaletteHit::from_json(&serde_json::json!({}));
        assert_eq!(empty.id, 0);
        assert!(empty.title.is_empty());
    }

    /// Headless egui passes for the DEFECT-02/03 proofs: render the
    /// palette the way the shell does and drive the real event loop.
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

        // Pass 1: the palette renders once; the query field requests focus.
        ctx.run_pass(egui::RawInput::default(), |ctx| {
            palette.ui(ctx, &db);
        })
        .drop_without_applying_deltas();
        assert!(
            ctx.memory(|mem| mem.focused().is_some()),
            "DEFECT-03: the query field must take keyboard focus when the palette opens"
        );

        // Pass 2: typing lands in the query with no click — proof that the
        // auto-focus actually delivers keystrokes.
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

        // Pass 3: Enter while the field is focused. egui 0.37 single-line
        // TextEdits surrender focus on Enter, so `lost_focus() && Enter`
        // (the web-port idiom audited as DEFECT-02) must fire and confirm
        // the selected hit.
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
        assert!(!palette.is_open());
    }

    /// Clean-audit item 133: an Enter processed in the same frame the
    /// query changed must not confirm the PREVIOUS query's result set —
    /// the hits still belong to the old text. The confirm is suppressed,
    /// the palette stays open, and the fresh search lands.
    #[test]
    fn audit133_same_frame_enter_confirms_nothing_stale() {
        let (ctx, mut palette, db) = seeded_palette_ctx();

        // Pass 1: open + focus.
        ctx.run_pass(egui::RawInput::default(), |ctx| {
            palette.ui(ctx, &db);
        })
        .drop_without_applying_deltas();

        // Pass 2: type "Export" — hits land for that query.
        let mut typing = egui::RawInput::default();
        typing.events.push(egui::Event::Text("Export".into()));
        ctx.run_pass(typing, |ctx| {
            palette.ui(ctx, &db);
        })
        .drop_without_applying_deltas();
        assert!(!palette.hits.is_empty(), "the fixture matches Export");

        // Pass 3: append text AND press Enter in the SAME frame. The hits
        // still belong to "Export" — the search for the new text has not
        // run. Pre-fix this navigated to the stale hit and closed.
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
        assert!(palette.is_open(), "the palette stays open for fresh hits");
        assert_eq!(palette.query, "Exportzz", "the new query was recorded");

        // Pass 4: the fresh (empty — nothing matches "Exportzz") results
        // have landed and the field holds focus; a dry Enter keeps the
        // palette open and the keystrokes flowing (item 72 intact).
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
        assert!(palette.is_open(), "a dry Enter keeps the palette open");
    }

    /// Clean-audit item 133: clicking outside the palette window closes
    /// it (the web overlay's backdrop), and the click does not leak to
    /// the UI underneath.
    #[test]
    fn audit133_clicking_outside_the_palette_closes_it() {
        let (ctx, mut palette, db) = seeded_palette_ctx();

        ctx.run_pass(egui::RawInput::default(), |ctx| {
            palette.ui(ctx, &db);
        })
        .drop_without_applying_deltas();
        assert!(palette.is_open());

        // A press, then a release in the bottom-left corner — far from
        // the center-top anchored window (560 wide, starts at y=80). egui
        // registers the click on the release pass; the motion event puts
        // the synthetic pointer on screen first.
        let mut move_pointer = egui::RawInput::default();
        move_pointer
            .events
            .push(egui::Event::PointerMoved(egui::pos2(10.0, 300.0)));
        ctx.run_pass(move_pointer, |ctx| {
            palette.ui(ctx, &db);
        })
        .drop_without_applying_deltas();

        let mut press = egui::RawInput::default();
        press.events.push(egui::Event::PointerButton {
            pos: egui::pos2(10.0, 300.0),
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        });
        ctx.run_pass(press, |ctx| {
            palette.ui(ctx, &db);
        })
        .drop_without_applying_deltas();
        assert!(palette.is_open(), "the press alone does not close");

        let mut release = egui::RawInput::default();
        release.events.push(egui::Event::PointerButton {
            pos: egui::pos2(10.0, 300.0),
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        });
        ctx.run_pass(release, |ctx| {
            palette.ui(ctx, &db);
        })
        .drop_without_applying_deltas();
        assert!(!palette.is_open(), "an outside click closes the palette");
    }

    /// Clean-audit item 72: Enter with ZERO hits must not strand the
    /// query field defocused — egui single-line TextEdits surrender
    /// focus on Enter, and the pre-fix palette left it surrendered, so
    /// every keystroke after a fruitless Enter was swallowed until the
    /// user clicked back in. The field re-grabs focus; typing continues.
    #[test]
    fn audit72_enter_on_zero_hits_keeps_the_query_focused() {
        let (ctx, mut palette, db) = seeded_palette_ctx();

        // Pass 1: open + focus.
        ctx.run_pass(egui::RawInput::default(), |ctx| {
            palette.ui(ctx, &db);
        })
        .drop_without_applying_deltas();

        // Type a query that matches nothing.
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

        // Pass 3: Enter — nothing to confirm, so the palette stays open
        // AND the query field must still hold keyboard focus afterwards.
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
        assert!(palette.is_open(), "the palette stays open on a dry Enter");

        // Pass 4: the proof — the next keystroke still lands in the query
        // field without any click. Pre-fix, the field sat defocused and
        // this character vanished.
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
