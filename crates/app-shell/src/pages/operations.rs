use eframe::egui;
use spp_core::catalog::OperationsTileKey;
use spp_core::db::DbHandle;
use spp_core::workload::{CapacityModel, WorkloadSnapshot};

use super::relative_time;
use super::{ConfirmRequest, NavRequest};
use crate::design::{self, BadgeTone, ButtonVariant};
use crate::icons::Icon;
use crate::pages::PageKey;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tab {
    Tiles,
    Workload,
    Library,
}

impl Tab {
    const ALL: [Self; 3] = [Self::Tiles, Self::Workload, Self::Library];

    fn label(self) -> &'static str {
        match self {
            Self::Tiles => "Tiles",
            Self::Workload => "Workload",
            Self::Library => "Library",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Severity {
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
}

#[must_use]
pub(crate) fn tile_label(tile: OperationsTileKey) -> &'static str {
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

#[must_use]
pub(crate) fn tile_description(tile: OperationsTileKey) -> &'static str {
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

#[must_use]
pub(crate) fn tile_severity(tile: OperationsTileKey) -> Severity {
    match tile.severity() {
        "critical" => Severity::Critical,
        "warning" => Severity::Warning,
        _ => Severity::Info,
    }
}

#[must_use]
pub(crate) fn tile_drills_into_inbox(tile: OperationsTileKey) -> bool {
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

/// UX-14: every tile with an action gets a destination — the inbox-filter
/// tiles drill into the filtered inbox, and the system-health tiles open
/// the page where the failures can actually be inspected and acted on.
#[must_use]
pub(crate) fn tile_nav(tile: OperationsTileKey) -> Option<NavRequest> {
    match tile {
        OperationsTileKey::FailedJobs => Some(NavRequest::Page(PageKey::Diagnostics)),
        OperationsTileKey::SyncProblems => Some(NavRequest::Page(PageKey::SyncHealth)),
        OperationsTileKey::AutomationApprovals => Some(NavRequest::Page(PageKey::Automation)),
        tile if tile_drills_into_inbox(tile) => {
            Some(NavRequest::InboxTile(tile.as_str().to_string()))
        }
        _ => None,
    }
}

/// UX-14: the drill hint names the destination so the affordance is
/// explicit — no more guessing which tiles are clickable.
#[must_use]
pub(crate) fn tile_drill_line(tile: OperationsTileKey) -> Option<&'static str> {
    match tile {
        OperationsTileKey::FailedJobs => Some("diagnostics →"),
        OperationsTileKey::SyncProblems => Some("sync health →"),
        OperationsTileKey::AutomationApprovals => Some("automation →"),
        tile if tile_drills_into_inbox(tile) => Some("open →"),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct TileView {
    pub key: OperationsTileKey,
    pub count: u32,
}

impl TileView {
    fn count_label(&self) -> String {
        self.count.to_string()
    }
}

#[must_use]
pub(crate) fn group_tiles(tiles: &[TileView]) -> Vec<(Severity, Vec<TileView>)> {
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

#[must_use]
pub(crate) fn tiles_from_snapshot(
    snapshot: &spp_core::operations::OperationsSnapshot,
) -> Vec<TileView> {
    OperationsTileKey::ALL
        .into_iter()
        .filter_map(|key| {
            snapshot
                .count_of(key.as_str())
                .map(|count| TileView { key, count })
        })
        .collect()
}

pub(crate) fn adjust_capacity(
    conn: &rusqlite::Connection,
    model: &mut CapacityModel,
    user_id: i64,
    delta: i64,
) -> Result<(), String> {
    let next = (model.capacity_for(user_id) + delta).clamp(1, 500);
    model.per_user_max.insert(user_id.to_string(), next);
    spp_core::workload::set_capacity_model(conn, model)
}

pub struct OperationsPage {
    tab: Tab,
    snapshot: Option<spp_core::operations::OperationsSnapshot>,
    workload: Option<WorkloadSnapshot>,
    capacity: Option<CapacityModel>,
    suggestions: Vec<spp_core::workload::SuggestedAssignee>,
    suggestions_loaded: bool,
    error: Option<String>,
    library: Option<LibraryModel>,
    pending_confirm: Option<ConfirmRequest>,
    /// AU2-9: when the snapshot query fails, back off instead of re-querying
    /// every frame (the old loop hammered SQLite at refresh rate forever).
    failed_reload_at: Option<std::time::Instant>,
}

/// AU2-9: cooldown between retries after a failed operations snapshot.
const RELOAD_BACKOFF: std::time::Duration = std::time::Duration::from_secs(10);

impl OperationsPage {
    #[must_use]
    pub fn new() -> Self {
        Self {
            tab: Tab::Tiles,
            snapshot: None,
            workload: None,
            capacity: None,
            suggestions: Vec::new(),
            suggestions_loaded: false,
            error: None,
            library: None,
            pending_confirm: None,
            failed_reload_at: None,
        }
    }

    #[cfg(feature = "kittest-ui")]
    #[doc(hidden)]
    pub fn kittest_freeze_generated_at(&mut self) {
        if let Some(s) = &mut self.snapshot {
            s.generated_at = "2026-10-27T09:59:59.123Z".into();
        }
        if let Some(w) = &mut self.workload {
            w.generated_at = "2026-10-27T09:59:59.123Z".into();
        }
    }

    pub(crate) fn invalidate(&mut self) {
        self.snapshot = None;
        self.workload = None;
        self.suggestions_loaded = false;
        self.suggestions.clear();
        self.failed_reload_at = None;
        if let Some(library) = &mut self.library {
            library.stale = true;
        }
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
        // AU2-9: arm the backoff only when the read failed; a successful read
        // re-arms nothing.
        self.failed_reload_at = self.snapshot.is_none().then(std::time::Instant::now);
    }

    fn in_backoff(&self) -> bool {
        self.failed_reload_at
            .is_some_and(|at| at.elapsed() < RELOAD_BACKOFF)
    }

    fn ensure_loaded(&mut self, db: &DbHandle) {
        if self.snapshot.is_none() && !self.in_backoff() {
            self.reload(db);
        }
    }

    fn ensure_workload(&mut self, db: &DbHandle) {
        if self.workload.is_none() && !self.in_backoff() {
            self.reload(db);
        }
        if self.workload.is_some() && !self.suggestions_loaded {
            self.suggestions = db.with_read(|conn| {
                spp_core::workload::suggested_assignees(conn, 10).unwrap_or_default()
            });
            self.suggestions_loaded = true;
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) -> Option<NavRequest> {
        self.ensure_loaded(db);

        let mut nav = None;
        design::page_header(
            ui,
            "Operations Center",
            Some(
                "A live snapshot of workload, SLA risk, and system health. Tiles \
                 marked with an arrow open the matching view.",
            ),
            |_| {},
        );
        let labels: Vec<&str> = Tab::ALL.iter().map(|t| t.label()).collect();
        if let Some(i) = design::tabs(
            ui,
            &labels,
            Tab::ALL.iter().position(|t| *t == self.tab).unwrap_or(0),
        ) && let Some(tab) = Tab::ALL.get(i)
        {
            self.tab = *tab;
        }
        if let Some(err) = &self.error {
            design::error_text(ui, &format!("error: {err}"));
        }

        match self.tab {
            Tab::Tiles => nav = self.tiles_ui(ui),
            Tab::Workload => self.workload_ui(ui, db),
            Tab::Library => self.library_ui(ui, db),
        }
        if let Some(request) = self.pending_confirm.take() {
            return Some(NavRequest::Confirm(request));
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
                "updated {} · waiting threshold {threshold} min",
                relative_time(snapshot.generated_at.as_str())
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
                        let tone = match band {
                            Severity::Critical => BadgeTone::Destructive,
                            Severity::Warning => BadgeTone::Warning,
                            Severity::Info => BadgeTone::Info,
                        };
                        // UX-14: drill-ability now comes from tile_nav — every
                        // tile that can go somewhere shows its destination.
                        let drill = tile_nav(tile.key).is_some();
                        let drill_line = tile_drill_line(tile.key);
                        let resp = design::metric_tile(
                            ui,
                            &tile.count_label(),
                            tile_label(tile.key),
                            Some(tone),
                            drill,
                            drill_line,
                        )
                        .on_hover_text(tile_description(tile.key));
                        if resp.clicked()
                            && let Some(nav_request) = tile_nav(tile.key)
                            && nav.is_none()
                        {
                            nav = Some(nav_request);
                        }
                    }
                });
                ui.add_space(6.0);
            }
        });
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
                egui::ScrollArea::vertical()
                    .id_salt("ops-workload-rows")
                    .show_rows(ui, 24.0, agents.len(), |ui, range| {
                        design::table(
                            ui,
                            "ops-workload-table",
                            &[
                                "Agent", "Open", "Waiting", "Capacity", "Load", "Pressure",
                                "Actions",
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
                                    design::td_num(
                                        ui,
                                        &agent.customer_waiting_workload.to_string(),
                                    );
                                    design::td_num(ui, &agent.capacity.to_string());
                                    design::td_num(ui, &format!("{:.2}", agent.weighted_load));
                                    design::td_num(ui, &format!("{:.0}%", agent.pressure * 100.0));
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
                egui::ScrollArea::vertical()
                    .id_salt("ops-suggestions")
                    .show_rows(ui, 20.0, suggestions.len(), |ui, range| {
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

#[derive(Debug, Clone, PartialEq)]
struct LibraryMailbox {
    local_id: i64,
    remote_id: i64,
    name: String,
}

#[derive(Debug, Clone, PartialEq)]
struct SavedReplyRow {
    remote_id: i64,
    mailbox_local_id: Option<i64>,
    name: String,
    preview: Option<String>,
    text: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
struct WorkflowRow {
    remote_id: i64,
    name: String,
    kind: String,
    status: String,
}

#[derive(Debug, Clone, PartialEq)]
struct ReplyEdit {
    remote_id: Option<i64>,
    name: String,
    text: String,
}

struct LibraryModel {
    mailboxes: Vec<LibraryMailbox>,
    selected_mailbox: usize,
    replies: Vec<SavedReplyRow>,
    reply_edit: Option<ReplyEdit>,
    workflows: Vec<WorkflowRow>,
    workflow_number: String,
    notice: Option<String>,
    stale: bool,
}

#[must_use]
fn reply_write_payload(
    action: &str,
    mailbox_remote_id: i64,
    reply_remote_id: Option<i64>,
    name: &str,
    text: &str,
) -> String {
    serde_json::json!({
        "action": action,
        "mailboxRemoteId": mailbox_remote_id,
        "replyRemoteId": reply_remote_id,
        "name": name,
        "text": text,
    })
    .to_string()
}

enum LibraryAction {
    None,
    NewReply,
    CancelEdit,
    EditReply(i64),
    SaveReply {
        remote_id: Option<i64>,
        name: String,
        text: String,
    },
    DeleteReply(i64),
    RunWorkflow {
        workflow_remote_id: i64,
        conversation_number: i64,
    },
}

impl OperationsPage {
    fn reload_library(&mut self, db: &DbHandle) {
        let (mailboxes, replies, workflows) = db.with_read(|conn| {
            let mailboxes: Vec<LibraryMailbox> = conn
                .prepare("SELECT id, remote_id, name FROM mailboxes ORDER BY name")
                .and_then(|mut s| {
                    s.query_map([], |r| {
                        Ok(LibraryMailbox {
                            local_id: r.get(0)?,
                            remote_id: r.get(1)?,
                            name: r.get(2)?,
                        })
                    })
                    .map(|rows| rows.filter_map(Result::ok).collect())
                })
                .unwrap_or_default();
            let replies = conn
                .prepare(
                    "SELECT remote_id, mailbox_local_id, name, preview, text
                       FROM saved_replies ORDER BY name",
                )
                .and_then(|mut s| {
                    s.query_map([], |r| {
                        Ok(SavedReplyRow {
                            remote_id: r.get(0)?,
                            mailbox_local_id: r.get(1)?,
                            name: r.get(2)?,
                            preview: r.get(3)?,
                            text: r.get(4)?,
                        })
                    })
                    .map(|rows| rows.filter_map(Result::ok).collect())
                })
                .unwrap_or_default();
            let workflows = conn
                .prepare(
                    "SELECT remote_id, name, type, status FROM workflows
                      ORDER BY sort_order, name",
                )
                .and_then(|mut s| {
                    s.query_map([], |r| {
                        Ok(WorkflowRow {
                            remote_id: r.get(0)?,
                            name: r.get(1)?,
                            kind: r.get(2)?,
                            status: r.get(3)?,
                        })
                    })
                    .map(|rows| rows.filter_map(Result::ok).collect())
                })
                .unwrap_or_default();
            (mailboxes, replies, workflows)
        });
        let previous = self.library.take();
        let selected_mailbox = previous
            .as_ref()
            .map(|l| l.selected_mailbox.min(mailboxes.len().saturating_sub(1)))
            .unwrap_or(0);
        let reply_edit = previous.as_ref().and_then(|l| l.reply_edit.clone());
        let workflow_number = previous.map_or_else(String::new, |l| l.workflow_number);
        self.library = Some(LibraryModel {
            mailboxes,
            selected_mailbox,
            replies,
            reply_edit,
            workflows,
            workflow_number,
            notice: None,
            stale: false,
        });
    }

    fn library_ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) {
        if self.library.as_ref().is_none_or(|l| l.stale) {
            self.reload_library(db);
        }
        let Some(library) = &mut self.library else {
            ui.spinner();
            return;
        };
        if library.mailboxes.is_empty() {
            design::empty_state(
                ui,
                Icon::BookOpen,
                "No mailboxes yet",
                "Saved replies and workflows mirror per mailbox — connect Help \
                 Scout and run a sync first.",
            );
            return;
        }

        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(8.0, 8.0);
            for (idx, mailbox) in library.mailboxes.iter().enumerate() {
                if design::chip(ui, library.selected_mailbox == idx, &mailbox.name).clicked() {
                    library.selected_mailbox = idx;
                }
            }
        });
        let mailbox = library.mailboxes[library.selected_mailbox].clone();
        if let Some(notice) = &library.notice {
            design::hint_text(ui, notice);
        }

        let mut action = LibraryAction::None;

        design::section_title(ui, "Saved replies");
        if design::button(ui, "New Saved Reply", ButtonVariant::Primary).clicked() {
            action = LibraryAction::NewReply;
        }
        let rows: Vec<SavedReplyRow> = library
            .replies
            .iter()
            .filter(|r| r.mailbox_local_id.is_none_or(|m| m == mailbox.local_id))
            .cloned()
            .collect();
        for row in &rows {
            ui.horizontal(|ui| {
                ui.label(format!(
                    "{} — {}",
                    row.name,
                    row.preview.as_deref().unwrap_or("(no preview)")
                ));
                if design::button(ui, "Edit", ButtonVariant::Outline).clicked() {
                    action = LibraryAction::EditReply(row.remote_id);
                }
                if design::button(ui, "Delete", ButtonVariant::Destructive).clicked() {
                    action = LibraryAction::DeleteReply(row.remote_id);
                }
            });
        }
        if rows.is_empty() && library.reply_edit.is_none() {
            design::hint_text(
                ui,
                "no saved replies in this mailbox yet — the sync mirrors the platform's list",
            );
        }
        if let Some(edit) = &mut library.reply_edit {
            let title = if edit.remote_id.is_some() {
                "Edit saved reply"
            } else {
                "New saved reply"
            };
            action = design::card_titled(
                ui,
                title,
                Some(
                    "writes go straight to Help Scout; the table re-reads the \
                     platform's post-write list",
                ),
                |ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut edit.name)
                            .hint_text("name (required by the platform)"),
                    );
                    ui.add(
                        egui::TextEdit::multiline(&mut edit.text)
                            .desired_width(f32::INFINITY)
                            .hint_text("text (HTML)"),
                    );
                    let mut act = LibraryAction::None;
                    ui.horizontal(|ui| {
                        if edit.name.trim().is_empty() {
                            design::hint_text(ui, "the platform requires a name");
                        } else if design::button(ui, "Save to Help Scout", ButtonVariant::Primary)
                            .clicked()
                        {
                            act = LibraryAction::SaveReply {
                                remote_id: edit.remote_id,
                                name: edit.name.trim().to_string(),
                                text: edit.text.trim().to_string(),
                            };
                        }
                        if design::button(ui, "Cancel", ButtonVariant::Outline).clicked() {
                            act = LibraryAction::CancelEdit;
                        }
                    });
                    act
                },
            )
            .inner;
        }

        design::section_title(ui, "Workflows");
        ui.horizontal(|ui| {
            let wf_label = design::field_label(ui, "conversation #:");
            ui.add(
                egui::TextEdit::singleline(&mut library.workflow_number)
                    .desired_width(110.0)
                    .hint_text("e.g. 778"),
            )
            .labelled_by(wf_label.id);
        });
        design::hint_text(
            ui,
            "the run resolves the number against the local mirror; the platform \
             enforces same-inbox and manual-only (a 400 lands in the jobs table)",
        );
        let wf_rows = library.workflows.clone();
        for wf in &wf_rows {
            ui.horizontal(|ui| {
                let tone = if wf.status == "active" {
                    BadgeTone::Success
                } else {
                    BadgeTone::Neutral
                };
                design::badge(ui, &wf.status, tone);
                ui.label(format!("{} · {}", wf.name, wf.kind));
                let manual = wf.kind == "manual";
                let number_ok = library.workflow_number.trim().parse::<i64>().is_ok();
                if manual
                    && number_ok
                    && design::button(ui, "Run", ButtonVariant::Outline).clicked()
                    && let Ok(number) = library.workflow_number.trim().parse::<i64>()
                {
                    action = LibraryAction::RunWorkflow {
                        workflow_remote_id: wf.remote_id,
                        conversation_number: number,
                    };
                }
            });
        }
        if wf_rows.is_empty() {
            design::hint_text(ui, "no workflows mirrored yet");
        }
        match action {
            LibraryAction::None => {}
            LibraryAction::NewReply => {
                library.reply_edit = Some(ReplyEdit {
                    remote_id: None,
                    name: String::new(),
                    text: String::new(),
                });
            }
            LibraryAction::CancelEdit => library.reply_edit = None,
            LibraryAction::EditReply(remote_id) => {
                if let Some(row) = library.replies.iter().find(|r| r.remote_id == remote_id) {
                    library.reply_edit = Some(ReplyEdit {
                        remote_id: Some(remote_id),
                        name: row.name.clone(),
                        text: row.text.clone().unwrap_or_default(),
                    });
                }
            }
            LibraryAction::SaveReply {
                remote_id,
                name,
                text,
            } => {
                let what = if remote_id.is_some() {
                    "update"
                } else {
                    "create"
                };
                let payload = reply_write_payload(what, mailbox.remote_id, remote_id, &name, &text);
                match db.with_write(|conn| {
                    spp_core::jobs::enqueue_on(conn, "sync", "saved_reply_write", &payload, 1, 2)
                }) {
                    Ok(_) => {
                        library.reply_edit = None;
                        library.notice = Some(
                            "sent to Help Scout — the table re-reads the platform's \
                             post-write list when it lands"
                                .to_string(),
                        );
                    }
                    Err(e) => library.notice = Some(format!("could not queue the write: {e}")),
                }
            }
            LibraryAction::DeleteReply(remote_id) => {
                let name = library
                    .replies
                    .iter()
                    .find(|r| r.remote_id == remote_id)
                    .map(|r| r.name.clone())
                    .unwrap_or_else(|| format!("reply {remote_id}"));
                self.pending_confirm = Some(ConfirmRequest::delete_saved_reply(
                    mailbox.remote_id,
                    remote_id,
                    &name,
                ));
            }
            LibraryAction::RunWorkflow {
                workflow_remote_id,
                conversation_number,
            } => {
                let payload = serde_json::json!({
                    "workflowRemoteId": workflow_remote_id,
                    "conversationNumber": conversation_number,
                })
                .to_string();
                match db.with_write(|conn| {
                    spp_core::jobs::enqueue_on(conn, "sync", "run_workflow", &payload, 1, 2)
                }) {
                    Ok(_) => {
                        library.notice = Some(format!(
                            "workflow run requested on conversation #{conversation_number}"
                        ));
                    }
                    Err(e) => library.notice = Some(format!("could not queue the run: {e}")),
                }
            }
        }
    }

    pub(crate) fn delete_saved_reply(
        &mut self,
        db: &DbHandle,
        mailbox_remote_id: i64,
        remote_id: i64,
    ) -> spp_core::Result<()> {
        let payload = reply_write_payload("delete", mailbox_remote_id, Some(remote_id), "", "");
        let Some(library) = self.library.as_mut() else {
            return Err(spp_core::Error::Config(
                "the saved-replies library is not loaded".into(),
            ));
        };
        match db.with_write(|conn| {
            spp_core::jobs::enqueue_on(conn, "sync", "saved_reply_write", &payload, 1, 2)
        }) {
            Ok(_) => {
                library.notice = Some(
                    "delete sent to Help Scout — the table re-reads the \
                     platform's post-write list when it lands"
                        .to_string(),
                );
                Ok(())
            }
            Err(e) => {
                library.notice = Some(format!("could not queue the delete: {e}"));
                Err(e)
            }
        }
    }
}

impl Default for OperationsPage {
    fn default() -> Self {
        Self::new()
    }
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
        assert_eq!(groups[0].1.len(), 2);
        assert_eq!(groups[1].0, Severity::Info);
        assert_eq!(groups[1].1.len(), 1);

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

        let reread = db.with_read(spp_core::workload::get_capacity_model);
        assert_eq!(reread.capacity_for(5), before + 2);

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

    #[test]
    fn library_tab_mirrors_saved_replies_and_workflows() {
        let db = fixture(vec![]);
        db.with_write(|conn| {
            spp_core::sync_engine::upsert_saved_reply(
                conn,
                None,
                &spp_core::helpscout::HsSavedReply {
                    remote_id: 11,
                    name: "Refund path".into(),
                    preview: Some("…here is how…".into()),
                    text: Some("<p>…here is how…</p>".into()),
                },
            )
            .expect("seed reply");
            spp_core::sync_engine::upsert_workflow(
                conn,
                &spp_core::helpscout::HsWorkflow {
                    remote_id: 21,
                    mailbox_id: Some(3),
                    name: "Escalate".into(),
                    kind: "manual".into(),
                    status: "active".into(),
                    sort_order: 1,
                },
            )
            .expect("seed workflow");
        });

        let mut page = OperationsPage::new();
        page.reload_library(&db);
        let library = page.library.as_ref().expect("library loaded");
        assert_eq!(library.replies.len(), 1);
        assert_eq!(library.replies[0].name, "Refund path");
        assert_eq!(library.workflows.len(), 1);
        assert_eq!(library.workflows[0].kind, "manual");

        let payload = reply_write_payload("update", 3, Some(11), "Refund path v2", "<p>…</p>");
        let v: serde_json::Value = serde_json::from_str(&payload).expect("json payload");
        assert_eq!(v["action"], "update");
        assert_eq!(v["mailboxRemoteId"], 3);
        assert_eq!(v["replyRemoteId"], 11);
        assert_eq!(v["name"], "Refund path v2");

        page.invalidate();
        assert!(
            page.library.as_ref().is_some_and(|l| l.stale),
            "invalidation flags the mirror rows for a re-read"
        );

        page.reload_library(&db);
        assert!(page.library.as_ref().is_some_and(|l| !l.stale));
        assert_eq!(
            page.library.as_ref().expect("library").replies.len(),
            1,
            "the re-read still mirrors the platform rows"
        );
    }

    #[test]
    fn audit76_invalidation_keeps_the_open_library_edit_form() {
        let db = fixture(vec![]);
        db.with_write(|conn| {
            spp_core::sync_engine::upsert_saved_reply(
                conn,
                None,
                &spp_core::helpscout::HsSavedReply {
                    remote_id: 11,
                    name: "Refund path".into(),
                    preview: Some("…here is how…".into()),
                    text: Some("<p>…here is how…</p>".into()),
                },
            )
            .expect("seed reply");
        });

        let mut page = OperationsPage::new();
        page.reload_library(&db);

        let library = page.library.as_mut().expect("library");
        library.selected_mailbox = 0;
        library.reply_edit = Some(ReplyEdit {
            remote_id: Some(11),
            name: "Refund path v2".into(),
            text: "<p>edited while a sync landed</p>".into(),
        });
        library.workflow_number = "778".into();

        page.invalidate();
        assert!(page.library.as_ref().is_some_and(|l| l.stale));
        page.reload_library(&db);

        let library = page.library.as_ref().expect("library reloaded");
        assert_eq!(
            library.replies.len(),
            1,
            "the mirror rows re-read underneath the form"
        );
        let edit = library
            .reply_edit
            .as_ref()
            .expect("the open edit form survived the invalidation");
        assert_eq!(edit.name, "Refund path v2", "the typed name is kept");
        assert_eq!(
            edit.text, "<p>edited while a sync landed</p>",
            "the typed text is kept"
        );
        assert_eq!(
            library.workflow_number, "778",
            "the typed workflow number is kept"
        );
        assert!(!library.stale, "the reload cleared the stale flag");
    }
}
