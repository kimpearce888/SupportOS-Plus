//! Operations Center page (port of the web `pages/operations.rs`).
//!
//! Redesigned with the shadcn-style design system (`crate::design`):
//! severity-toned card tiles, a striped workload table and section cards —
//! presentation only, all tile navigation and capacity behavior intact.
//!
//! Two tabs, same as the web page:
//! * **Tiles** — the 16 operations tiles from `spp_core::operations::snapshot`
//!   (the SQL behind `/api/operations/center`), grouped by severity. Tiles
//!   whose web counterpart linked into the inbox raise a
//!   [`NavRequest::InboxTile`] that the shell resolves by switching to the
//!   inbox page with the tile's conversation filter pre-applied (the port of
//!   `/inbox?view={tile}`).
//! * **Workload** — `spp_core::workload::snapshot` (the `/api/operations/workload`
//!   SQL) with the agent table, per-agent capacity +/- buttons backed by the
//!   real capacity model (`set_capacity_model`, the `/api/operations/capacity`
//!   route logic — the web buttons were display-only no-ops), and the
//!   suggested-assignee list (`workload::suggested_assignees`).

use eframe::egui;
use spp_core::catalog::OperationsTileKey;
use spp_core::db::DbHandle;
use spp_core::workload::{CapacityModel, WorkloadSnapshot};

use super::NavRequest;
use crate::design::{self, ButtonVariant};
use crate::icons::Icon;

/// Report tabs (web: the `active_tab` signal).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tab {
    Tiles,
    Workload,
}

impl Tab {
    const ALL: [Self; 2] = [Self::Tiles, Self::Workload];

    fn label(self) -> &'static str {
        match self {
            Self::Tiles => "Tiles",
            Self::Workload => "Workload",
        }
    }
}

/// Tile severity band (web: `tile_severity` → `Severity`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Critical,
    Warning,
    Info,
}

impl Severity {
    fn label(self) -> &'static str {
        match self {
            Self::Critical => "Critical",
            Self::Warning => "Warning",
            Self::Info => "Info",
        }
    }

    /// Theme-aware value color for the tile counters (redesign: token
    /// tones replace the raw RED/YELLOW/LIGHT_BLUE).
    fn count_color(self, t: &design::Tokens) -> egui::Color32 {
        match self {
            Self::Critical => t.destructive_text,
            Self::Warning => t.warning_foreground,
            Self::Info => t.info_foreground,
        }
    }
}

/// Tile label (port of the web `tile_label`).
#[must_use]
pub fn tile_label(tile: OperationsTileKey) -> &'static str {
    match tile {
        OperationsTileKey::Unassigned => "Unassigned",
        OperationsTileKey::NeedsFirstResponse => "Needs first response",
        OperationsTileKey::CustomerWaiting => "Customer waiting",
        OperationsTileKey::WaitingOverThreshold => "Waiting over threshold",
        OperationsTileKey::Urgent => "Urgent",
        OperationsTileKey::SlaAtRisk => "SLA at risk",
        OperationsTileKey::SlaBreached => "SLA breached",
        OperationsTileKey::HighEffort => "High effort",
        OperationsTileKey::RepeatedIssue => "Repeated issue",
        OperationsTileKey::KnownIssue => "Known issue",
        OperationsTileKey::AiEscalation => "AI escalation",
        OperationsTileKey::IssueSpike => "Issue spike",
        OperationsTileKey::AutomationApprovals => "Automation approvals",
        OperationsTileKey::FailedJobs => "Failed jobs",
        OperationsTileKey::SyncProblems => "Sync problems",
        OperationsTileKey::CampaignActivity => "Campaign activity",
    }
}

/// Tile hover description (port of the web `tile_description`).
#[must_use]
pub fn tile_description(tile: OperationsTileKey) -> &'static str {
    match tile {
        OperationsTileKey::Unassigned => "Active conversations with no agent assigned.",
        OperationsTileKey::NeedsFirstResponse => "Conversations where no agent has replied yet.",
        OperationsTileKey::CustomerWaiting => {
            "Conversations where the customer sent the last message."
        }
        OperationsTileKey::WaitingOverThreshold => {
            "Customer-waiting conversations older than the threshold setting."
        }
        OperationsTileKey::Urgent => "Conversations marked as urgent priority.",
        OperationsTileKey::SlaAtRisk => "Conversations at risk of breaching their SLA.",
        OperationsTileKey::SlaBreached => "Conversations whose SLA has been breached.",
        OperationsTileKey::HighEffort => "Conversations with elevated customer effort signals.",
        OperationsTileKey::RepeatedIssue => "Conversations linked to a known recurring issue.",
        OperationsTileKey::KnownIssue => "Conversations matched to a known issue.",
        OperationsTileKey::AiEscalation => "Conversations escalated by the AI.",
        OperationsTileKey::IssueSpike => "Conversations clustered around a spiking issue.",
        OperationsTileKey::AutomationApprovals => "Pending automation approvals awaiting review.",
        OperationsTileKey::FailedJobs => "Background jobs that exhausted their retry budget.",
        OperationsTileKey::SyncProblems => "Help Scout sync runs that ended in failure.",
        OperationsTileKey::CampaignActivity => "Active outreach campaign activity.",
    }
}

/// Tile severity (port of the web `tile_severity`).
#[must_use]
pub fn tile_severity(tile: OperationsTileKey) -> Severity {
    match tile.severity() {
        "critical" => Severity::Critical,
        "warning" => Severity::Warning,
        _ => Severity::Info,
    }
}

/// Whether the tile drills into the inbox (port of `tile_inbox_link`).
#[must_use]
pub fn tile_drills_into_inbox(tile: OperationsTileKey) -> bool {
    matches!(
        tile,
        OperationsTileKey::Unassigned
            | OperationsTileKey::NeedsFirstResponse
            | OperationsTileKey::CustomerWaiting
            | OperationsTileKey::WaitingOverThreshold
            | OperationsTileKey::Urgent
            | OperationsTileKey::HighEffort
    )
}

/// One renderable tile: key + count resolved from the snapshot.
#[derive(Debug, Clone, Copy)]
pub struct TileView {
    pub key: OperationsTileKey,
    pub count: u32,
}

impl TileView {
    /// Count display (web `TileCountView::label`; unavailable tiles render as
    /// `–` and carry a note instead).
    fn count_label(&self) -> String {
        self.count.to_string()
    }
}

/// Group tiles by severity band (web `OperationsTileGrid`).
#[must_use]
pub fn group_tiles(tiles: &[TileView]) -> Vec<(Severity, Vec<TileView>)> {
    let mut out: Vec<(Severity, Vec<TileView>)> = Vec::new();
    for band in [Severity::Critical, Severity::Warning, Severity::Info] {
        let group: Vec<TileView> = tiles
            .iter()
            .filter(|t| tile_severity(t.key) == band)
            .cloned()
            .collect();
        if !group.is_empty() {
            out.push((band, group));
        }
    }
    out
}

/// Map a core snapshot into ordered tile views (every known tile key,
/// unavailable ones dropped — the web showed them with "Not yet available").
#[must_use]
pub fn tiles_from_snapshot(snapshot: &spp_core::operations::OperationsSnapshot) -> Vec<TileView> {
    OperationsTileKey::ALL
        .into_iter()
        .filter_map(|key| {
            snapshot
                .count_of(key.as_str())
                .map(|count| TileView { key, count })
        })
        .collect()
}

/// Adjust one agent's capacity and persist the model
/// (the `/api/operations/capacity` route logic, in-process).
///
/// # Errors
/// Propagates the store failure from `set_capacity_model`.
pub fn adjust_capacity(
    conn: &rusqlite::Connection,
    model: &mut CapacityModel,
    user_id: i64,
    delta: i64,
) -> Result<(), String> {
    let next = (model.capacity_for(user_id) + delta).clamp(1, 500);
    model.per_user_max.insert(user_id.to_string(), next);
    spp_core::workload::set_capacity_model(conn, model)
}

/// The interactive operations page state.
pub struct OperationsPage {
    tab: Tab,
    snapshot: Option<spp_core::operations::OperationsSnapshot>,
    workload: Option<WorkloadSnapshot>,
    capacity: Option<CapacityModel>,
    suggestions: Vec<spp_core::workload::SuggestedAssignee>,
    error: Option<String>,
}

impl OperationsPage {
    #[must_use]
    pub fn new() -> Self {
        Self {
            tab: Tab::Tiles,
            snapshot: None,
            workload: None,
            capacity: None,
            suggestions: Vec::new(),
            error: None,
        }
    }

    /// Live-update invalidation (web: `sse_refresh` version bump).
    pub fn invalidate(&mut self) {
        self.snapshot = None;
        self.workload = None;
        self.suggestions.clear();
    }

    fn reload(&mut self, db: &DbHandle) {
        let (snapshot, workload, capacity) = db.with_read(|conn| {
            (
                spp_core::operations::snapshot(conn, None).ok(),
                spp_core::workload::snapshot(conn).ok(),
                Some(spp_core::workload::get_capacity_model(conn)),
            )
        });
        self.error = snapshot
            .is_none()
            .then(|| "operations snapshot failed".to_string());
        self.snapshot = snapshot;
        self.workload = workload;
        self.capacity = capacity;
    }

    fn ensure_loaded(&mut self, db: &DbHandle) {
        if self.snapshot.is_none() {
            self.reload(db);
        }
    }

    fn ensure_workload(&mut self, db: &DbHandle) {
        if self.workload.is_none() {
            self.reload(db);
        }
        if self.workload.is_some() && self.suggestions.is_empty() {
            self.suggestions = db.with_read(|conn| {
                spp_core::workload::suggested_assignees(conn, 10).unwrap_or_default()
            });
        }
    }

    /// Render the page; returns a navigation request when a tile with an
    /// inbox drill-down was clicked.
    pub fn ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) -> Option<NavRequest> {
        self.ensure_loaded(db);

        let mut nav = None;
        design::page_header(
            ui,
            "Operations Center",
            Some(
                "16 tiles cover the team's current workload, SLA risk, and system health. \
                 Click a tile to filter the inbox.",
            ),
            |_| {},
        );
        ui.horizontal(|ui| {
            for tab in Tab::ALL {
                if design::chip(ui, self.tab == tab, tab.label()).clicked() {
                    self.tab = tab;
                }
            }
        });
        if let Some(err) = &self.error {
            design::error_text(ui, &format!("error: {err}"));
        }
        design::divider(ui);

        match self.tab {
            Tab::Tiles => nav = self.tiles_ui(ui),
            Tab::Workload => self.workload_ui(ui, db),
        }
        nav
    }

    fn tiles_ui(&mut self, ui: &mut egui::Ui) -> Option<NavRequest> {
        let Some(snapshot) = &self.snapshot else {
            ui.spinner();
            return None;
        };
        let tiles = tiles_from_snapshot(snapshot);
        if tiles.is_empty() {
            design::empty_state(
                ui,
                Icon::Activity,
                "No operations data yet",
                "No activity yet — connect Help Scout and run a sync to populate tiles.",
            );
            return None;
        }

        let threshold = snapshot.waiting_threshold_minutes;
        design::hint_text(
            ui,
            &format!(
                "generated {} · waiting threshold {threshold} min",
                snapshot.generated_at
            ),
        );
        ui.add_space(4.0);

        let mut nav = None;
        let groups = group_tiles(&tiles);
        egui::ScrollArea::vertical().show(ui, |ui| {
            for (band, group) in groups {
                design::section_title(ui, band.label());
                ui.add_space(4.0);
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(8.0, 8.0);
                    for tile in group {
                        let clicked = tile_ui(ui, &tile);
                        if clicked && tile_drills_into_inbox(tile.key) {
                            // First clicked drill wins; web navigates on click.
                            if nav.is_none() {
                                nav = Some(NavRequest::InboxTile(tile.key.as_str().to_string()));
                            }
                        }
                    }
                });
                ui.add_space(6.0);
            }
        });
        design::hint_text(ui, "(tiles marked with an arrow drill into the inbox)");
        nav
    }

    fn workload_ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) {
        self.ensure_workload(db);
        let Some(workload) = self.workload.clone() else {
            ui.spinner();
            return;
        };
        let Some(model) = self.capacity.clone() else {
            ui.spinner();
            return;
        };

        let suggestions = self.suggestions.clone();
        let agents = workload.agents.clone();
        design::card_titled(
            ui,
            "Team Workload",
            Some(&format!(
                "generated {} · unassigned work {}",
                workload.generated_at, workload.unassigned_work
            )),
            |ui| {
                egui::ScrollArea::vertical().show_rows(ui, 24.0, agents.len(), |ui, range| {
                    design::table(
                        ui,
                        "ops-workload-table",
                        &[
                            "Agent", "Open", "Waiting", "Capacity", "Load", "Pressure", "Actions",
                        ],
                        28.0,
                        |ui| {
                            let t = design::tokens(ui);
                            for agent in &agents[range] {
                                ui.label(
                                    egui::RichText::new(agent.display_name.as_str())
                                        .size(13.0)
                                        .color(t.foreground),
                                );
                                design::td_num(ui, &agent.open_workload.to_string());
                                design::td_num(ui, &agent.customer_waiting_workload.to_string());
                                design::td_num(ui, &agent.capacity.to_string());
                                design::td_num(ui, &format!("{:.0}%", agent.pressure * 100.0));
                                design::td_num(ui, &format!("{:.2}", agent.weighted_load));
                                ui.horizontal(|ui| {
                                    let mut new_model = model.clone();
                                    if design::button(ui, "+", ButtonVariant::Outline)
                                        .on_hover_text("Increase capacity")
                                        .clicked()
                                    {
                                        let res = db.with_write(|conn| {
                                            adjust_capacity(
                                                conn,
                                                &mut new_model,
                                                agent.user_local_id,
                                                1,
                                            )
                                        });
                                        if let Err(e) = res {
                                            self.error = Some(e);
                                        } else {
                                            self.capacity = Some(new_model);
                                            self.workload = None;
                                        }
                                    }
                                    if design::button(ui, "−", ButtonVariant::Outline)
                                        .on_hover_text("Decrease capacity")
                                        .clicked()
                                    {
                                        let mut new_model = model.clone();
                                        let res = db.with_write(|conn| {
                                            adjust_capacity(
                                                conn,
                                                &mut new_model,
                                                agent.user_local_id,
                                                -1,
                                            )
                                        });
                                        if let Err(e) = res {
                                            self.error = Some(e);
                                        } else {
                                            self.capacity = Some(new_model);
                                            self.workload = None;
                                        }
                                    }
                                });
                                ui.end_row();
                            }
                        },
                    );
                });
            },
        );
        ui.add_space(10.0);

        if suggestions.is_empty() {
            design::card_titled(
                ui,
                "Suggested Assignees",
                Some(
                    "Agents with the lowest load are suggested for new assignments. \
                     Use the +/− buttons to adjust individual capacity.",
                ),
                |ui| {
                    design::empty_state(
                        ui,
                        Icon::Users,
                        "No unassigned conversations waiting",
                        "Suggestions appear when active work has no assignee.",
                    );
                },
            );
            return;
        }
        design::card_titled(
            ui,
            "Suggested Assignees",
            Some(
                "Agents with the lowest load are suggested for new assignments. \
                 Use the +/− buttons to adjust individual capacity.",
            ),
            |ui| {
                let t = design::tokens(ui);
                egui::ScrollArea::vertical().show_rows(ui, 20.0, suggestions.len(), |ui, range| {
                    for s in &suggestions[range] {
                        let subject = s.subject.clone().unwrap_or_default();
                        ui.label(
                            egui::RichText::new(format!(
                                "#{} {subject} → {} ({})",
                                s.conversation_number.unwrap_or(s.conversation_id),
                                s.suggested_display_name
                                    .clone()
                                    .unwrap_or_else(|| "(any)".into()),
                                s.reason
                            ))
                            .size(13.0)
                            .color(t.foreground),
                        );
                    }
                });
            },
        );
    }
}

impl Default for OperationsPage {
    fn default() -> Self {
        Self::new()
    }
}

/// One tile card; returns whether it was clicked.
///
/// Redesigned as a shadcn-style card tile: severity-toned counter, muted
/// drill hint, full-card hitbox for the inbox drill-down (same action as
/// the web tile's `<A>` link).
fn tile_ui(ui: &mut egui::Ui, tile: &TileView) -> bool {
    let band = tile_severity(tile.key);
    let t = design::tokens(ui);
    let count_color = band.count_color(t);
    let mut clicked = false;
    let drills = tile_drills_into_inbox(tile.key);
    let card = design::card(ui, |ui| {
        // Wider min-width keeps tile labels on one line; the wrapped row
        // breaks into two tidy rows on narrow windows.
        ui.set_min_width(196.0);
        ui.set_min_height(52.0);
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(tile.count_label())
                        .size(20.0)
                        .strong()
                        .color(count_color),
                );
                ui.label(
                    egui::RichText::new(tile_label(tile.key))
                        .size(13.0)
                        .color(t.foreground),
                );
            });
            if drills {
                ui.label(
                    egui::RichText::new("open in inbox →")
                        .small()
                        .color(t.muted_foreground),
                );
            }
        });
    });
    let card_rect = card.response.rect;
    card.response.on_hover_text(tile_description(tile.key));
    if drills {
        // The whole card acts as the web tile's <A> link: the hitbox is
        // the card's own rect (full-card semantics).
        if ui
            .interact(
                card_rect,
                egui::Id::unique(tile.key.as_str()),
                egui::Sense::click(),
            )
            .clicked()
        {
            clicked = true;
        }
    }
    clicked
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testsupport::fixture;

    #[test]
    fn snapshot_maps_to_ordered_tile_views() {
        let db = fixture(vec![("conv", 1, "Help export", "active", "email", 1, 5, 9)]);
        let snapshot = db
            .with_read(|conn| spp_core::operations::snapshot(conn, None))
            .expect("snapshot");
        let tiles = tiles_from_snapshot(&snapshot);
        // Unassigned tile: the seeded conversation has an assignee, so a
        // second conversation without one must make it count 1.
        assert!(!tiles.is_empty());
        let unassigned = tiles
            .iter()
            .find(|t| t.key == OperationsTileKey::Unassigned)
            .expect("unassigned tile present");
        assert_eq!(unassigned.count, 0);

        db.with_write(|conn| {
            crate::testsupport::seed_conversation(conn, 2, "Orphan", "active", "email", 1, 0, 10);
        });
        let snapshot2 = db
            .with_read(|conn| spp_core::operations::snapshot(conn, None))
            .expect("snapshot 2");
        let tiles2 = tiles_from_snapshot(&snapshot2);
        let unassigned2 = tiles2
            .iter()
            .find(|t| t.key == OperationsTileKey::Unassigned)
            .expect("unassigned tile 2");
        assert_eq!(unassigned2.count, 1);
    }

    #[test]
    fn tiles_group_by_severity_band_in_order() {
        let tiles = vec![
            TileView {
                key: OperationsTileKey::FailedJobs,
                count: 0,
            },
            TileView {
                key: OperationsTileKey::Urgent,
                count: 2,
            },
            TileView {
                key: OperationsTileKey::Unassigned,
                count: 7,
            },
        ];
        let groups = group_tiles(&tiles);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].0, Severity::Warning);
        assert_eq!(groups[0].1.len(), 2); // FailedJobs + Urgent are warnings
        assert_eq!(groups[1].0, Severity::Info); // Unassigned
        assert_eq!(groups[1].1.len(), 1);

        // SLA breach is the only critical tile.
        let critical = vec![TileView {
            key: OperationsTileKey::SlaBreached,
            count: 1,
        }];
        let critical_groups = group_tiles(&critical);
        assert_eq!(critical_groups.len(), 1);
        assert_eq!(critical_groups[0].0, Severity::Critical);
    }

    #[test]
    fn workload_reload_and_capacity_adjust_round_trip() {
        let db = fixture(vec![("conv", 1, "Help export", "active", "email", 1, 5, 9)]);
        let mut page = OperationsPage::new();
        page.ensure_workload(&db);
        let workload = page.workload.clone().expect("workload");
        assert_eq!(workload.agents.len(), 2, "Ada and Grace");
        assert!(page.capacity.is_some());

        let mut model = page.capacity.clone().expect("model");
        let before = model.capacity_for(5);
        db.with_write(|conn| adjust_capacity(conn, &mut model, 5, 2))
            .expect("adjust up");
        assert_eq!(model.capacity_for(5), before + 2);

        // Persisted: a fresh read sees the override.
        let reread = db.with_read(spp_core::workload::get_capacity_model);
        assert_eq!(reread.capacity_for(5), before + 2);

        // Reload reflects the new capacity in the agent row.
        page.workload = None;
        page.ensure_workload(&db);
        let workload2 = page.workload.clone().expect("workload 2");
        let ada = workload2
            .agents
            .iter()
            .find(|a| a.user_local_id == 5)
            .expect("Ada");
        assert_eq!(ada.capacity, before + 2);
    }

    #[test]
    fn drill_tiles_are_exactly_the_conversation_tiles() {
        assert!(tile_drills_into_inbox(OperationsTileKey::Unassigned));
        assert!(tile_drills_into_inbox(OperationsTileKey::HighEffort));
        assert!(!tile_drills_into_inbox(OperationsTileKey::FailedJobs));
        assert!(!tile_drills_into_inbox(OperationsTileKey::CampaignActivity));
    }

    #[test]
    fn tile_metadata_covers_all_sixteen_keys() {
        for key in OperationsTileKey::ALL {
            assert!(!tile_label(key).is_empty());
            assert!(!tile_description(key).is_empty());
            let _band = tile_severity(key);
        }
    }
}
