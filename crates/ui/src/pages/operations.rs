use leptos::prelude::*;
use leptos_router::components::*;

use crate::catalog::OperationsTileKey;
use crate::components::state_view::EmptyState;
use crate::components::theming::Severity;

#[derive(Debug, Clone, Default)]
pub struct OperationsSnapshotView {
    pub tiles: Vec<(OperationsTileKey, TileCountView)>,
    pub mailbox_id: Option<i64>,
    pub built_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TileCountView {
    Available { count: u32 },
    NotAvailable,
}

impl TileCountView {
    #[must_use]
    pub fn label(self) -> String {
        match self {
            Self::Available { count } => count.to_string(),
            Self::NotAvailable => "\u{2013}".to_string(),
        }
    }

    #[must_use]
    pub fn is_not_available(self) -> bool {
        matches!(self, Self::NotAvailable { .. })
    }

    #[must_use]
    pub fn badge_class_suffix(self, tile: OperationsTileKey) -> &'static str {
        match self {
            Self::Available { .. } => tile_severity(tile).class_suffix(),
            Self::NotAvailable { .. } => "muted",
        }
    }
}

#[must_use]
pub fn tile_severity(tile: OperationsTileKey) -> Severity {
    match tile.severity() {
        "critical" => Severity::Critical,
        "warning" => Severity::Warning,
        _ => Severity::Info,
    }
}

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

#[must_use]
pub fn tile_description(tile: OperationsTileKey) -> &'static str {
    match tile {
        OperationsTileKey::Unassigned => "Active conversations with no agent assigned.",
        OperationsTileKey::NeedsFirstResponse => "Conversations where no agent has replied yet.",
        OperationsTileKey::CustomerWaiting => {
            "Conversations where the customer sent the last message."
        }
        OperationsTileKey::WaitingOverThreshold => {
            "Customer-waiting conversations older than 1 hour."
        }
        OperationsTileKey::Urgent => "Conversations marked as urgent priority.",
        OperationsTileKey::SlaAtRisk => {
            "Conversations at risk of breaching their SLA. (Ships in M7.)"
        }
        OperationsTileKey::SlaBreached => {
            "Conversations whose SLA has been breached. (Ships in M7.)"
        }
        OperationsTileKey::HighEffort => "Conversations with more than 20 activity events.",
        OperationsTileKey::RepeatedIssue => {
            "Conversations linked to a known recurring issue. (Ships in M7.)"
        }
        OperationsTileKey::KnownIssue => "Conversations matched to a known issue. (Ships in M7.)",
        OperationsTileKey::AiEscalation => "Conversations escalated by the AI. (Ships in M6.)",
        OperationsTileKey::IssueSpike => {
            "Conversations clustered around a spiking issue. (Ships in M7.)"
        }
        OperationsTileKey::AutomationApprovals => "Pending automation approvals awaiting review.",
        OperationsTileKey::FailedJobs => "Background jobs that exhausted their retry budget.",
        OperationsTileKey::SyncProblems => "Help Scout sync runs that ended in failure.",
        OperationsTileKey::CampaignActivity => "Active outreach campaign activity. (Ships in M9.)",
    }
}

#[must_use]
pub fn tile_inbox_link(tile: OperationsTileKey, mailbox_id: Option<i64>) -> Option<String> {
    match tile {
        OperationsTileKey::Unassigned
        | OperationsTileKey::NeedsFirstResponse
        | OperationsTileKey::CustomerWaiting
        | OperationsTileKey::WaitingOverThreshold
        | OperationsTileKey::Urgent
        | OperationsTileKey::HighEffort => {
            let mut url = format!("/inbox?view={}", tile.as_str());
            if let Some(mid) = mailbox_id {
                url.push_str(&format!("&scope={mid}"));
            }
            Some(url)
        }
        OperationsTileKey::FailedJobs
        | OperationsTileKey::SyncProblems
        | OperationsTileKey::AutomationApprovals
        | OperationsTileKey::SlaAtRisk
        | OperationsTileKey::SlaBreached
        | OperationsTileKey::RepeatedIssue
        | OperationsTileKey::KnownIssue
        | OperationsTileKey::AiEscalation
        | OperationsTileKey::IssueSpike
        | OperationsTileKey::CampaignActivity => None,
    }
}

#[component]
pub fn OperationsPage() -> impl IntoView {
    let snapshot = RwSignal::new(OperationsSnapshotView::default());
    let loading = RwSignal::new(true);
    let error_msg = RwSignal::new(None::<String>);
    let active_tab = RwSignal::new(0u8);
    let workload = RwSignal::new(Vec::<serde_json::Value>::new());
    let sse_refresh = crate::queries::version("operations-center");

    Effect::new(move |_| {
        let _ = sse_refresh.get();
        let snapshot = snapshot;
        let loading = loading;
        let error_msg = error_msg;
        wasm_bindgen_futures::spawn_local(async move {
            match crate::api::get_json::<serde_json::Value>("/api/operations/center").await {
                Ok(data) => {
                    let tiles = data
                        .get("tiles")
                        .and_then(|t| t.as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|entry| {
                                    let id = entry.get("key")?.as_str()?;
                                    let tile = if entry
                                        .get("available")
                                        .and_then(|v| v.as_bool())
                                        .unwrap_or(false)
                                    {
                                        let count = entry
                                            .get("count")
                                            .and_then(|v| v.as_u64())
                                            .unwrap_or(0)
                                            as u32;
                                        TileCountView::Available { count }
                                    } else {
                                        TileCountView::NotAvailable
                                    };
                                    for key in OperationsTileKey::ALL {
                                        if key.as_str() == id {
                                            return Some((key, tile));
                                        }
                                    }
                                    None
                                })
                                .collect::<Vec<_>>()
                        })
                        .unwrap_or_default();
                    snapshot.set(OperationsSnapshotView {
                        tiles,
                        mailbox_id: None,
                        built_at: data
                            .get("generated_at")
                            .and_then(|v| v.as_str())
                            .unwrap_or_default()
                            .to_string(),
                    });
                    loading.set(false);
                }
                Err(e) => {
                    error_msg.set(Some(e));
                    loading.set(false);
                }
            }
        });
    });

    Effect::new(move |_| {
        let _ = sse_refresh.get();
        if active_tab.get() != 1 {
            return;
        }
        let workload = workload;
        wasm_bindgen_futures::spawn_local(async move {
            if let Ok(data) =
                crate::api::get_json::<serde_json::Value>("/api/operations/workload").await
            {
                let items = data
                    .get("agents")
                    .and_then(|v| v.as_array())
                    .cloned()
                    .unwrap_or_default();
                workload.set(items);
            }
        });
    });

    view! {
        <div class="spp-page spp-page--operations">
            <h2 class="spp-page__title">"Operations Center"</h2>
            <p class="spp-page__subtitle">
                "16 tiles cover the team's current workload, SLA risk, and system health. "
                "Click a tile to filter the inbox."
            </p>

            <div class="spp-operations__tabs">
                <button
                    class=move || format!("spp-button spp-button--tab{}", if active_tab.get() == 0 { " spp-button--tab-active" } else { "" })
                    on:click=move |_| active_tab.set(0)
                >
                    "Tiles"
                </button>
                <button
                    class=move || format!("spp-button spp-button--tab{}", if active_tab.get() == 1 { " spp-button--tab-active" } else { "" })
                    on:click=move |_| active_tab.set(1)
                >
                    "Workload"
                </button>
            </div>

            <Show when=move || loading.get() fallback=|| ()>
                <div class="spp-state">
                    <span class="spp-spinner" aria-label="Loading"></span>
                    <p class="spp-state__body">"Loading operations data…"</p>
                </div>
            </Show>

            <Show when=move || error_msg.get().is_some() fallback=|| ()>
                <div class="spp-state spp-state--error">
                    <span class="spp-state__icon" aria-hidden="true">"⚠"</span>
                    <p class="spp-state__body">{move || error_msg.get().unwrap_or_default()}</p>
                </div>
            </Show>

            <Show
                when=move || !loading.get() && error_msg.get().is_none() && active_tab.get() == 0
                fallback=|| ()
            >
                <Show
                    when=move || !snapshot.get().tiles.is_empty()
                    fallback=move || {
                        view! {
                            <EmptyState message="No operations data yet. Run a sync or try demo mode to populate tiles." />
                        }
                    }
                >
                    <OperationsTileGrid snapshot=snapshot.get() />
                </Show>
            </Show>

            <Show
                when=move || !loading.get() && error_msg.get().is_none() && active_tab.get() == 1
                fallback=|| ()
            >
                <div class="spp-operations__workload">
                    <h3>"Team Workload"</h3>
                    <table class="spp-table">
                        <thead>
                            <tr>
                                <th>"Agent"</th>
                                <th>"Active"</th>
                                <th>"Waiting"</th>
                                <th>"Capacity"</th>
                                <th>"Load"</th>
                                <th>"Actions"</th>
                            </tr>
                        </thead>
                        <tbody>
                            {move || workload.with(|agents| {
                                agents.iter().map(|a| {
                                    let name = a.get("name").and_then(|v| v.as_str()).unwrap_or("(unknown)").to_string();
                                    let active = a.get("active_count").and_then(|v| v.as_i64()).unwrap_or(0);
                                    let waiting = a.get("waiting_count").and_then(|v| v.as_i64()).unwrap_or(0);
                                    let capacity = a.get("capacity").and_then(|v| v.as_i64()).unwrap_or(10);
                                    let load_pct = if capacity > 0 {
                                        ((active + waiting) * 100 / capacity).min(100)
                                    } else {
                                        0
                                    };
                                    let user_id = a.get("user_id").and_then(|v| v.as_i64()).unwrap_or(0);
                                    let cap_up = user_id;
                                    let cap_down = user_id;
                                    view! {
                                        <tr>
                                            <td>{name}</td>
                                            <td>{active.to_string()}</td>
                                            <td>{waiting.to_string()}</td>
                                            <td>{capacity.to_string()}</td>
                                            <td>
                                                <div class="spp-load-bar">
                                                    <div class="spp-load-bar__fill" style=format!("width: {load_pct}%")></div>
                                                </div>
                                                <span>{format!("{load_pct}%")}</span>
                                            </td>
                                            <td>
                                                <button
                                                    class="spp-button spp-button--small"
                                                    title="Increase capacity"
                                                    on:click=move |_| {
                                                        let _ = cap_up;
                                                    }
                                                >
                                                    "+"
                                                </button>
                                                <button
                                                    class="spp-button spp-button--small"
                                                    title="Decrease capacity"
                                                    on:click=move |_| {
                                                        let _ = cap_down;
                                                    }
                                                >
                                                    "−"
                                                </button>
                                            </td>
                                        </tr>
                                    }
                                }).collect::<Vec<_>>()
                            })}
                        </tbody>
                    </table>
                    <div class="spp-operations__suggested">
                        <h4>"Suggested Assignees"</h4>
                        <p class="spp-muted">
                            "Agents with the lowest load are suggested for new assignments. "
                            "Use the +/− buttons to adjust individual capacity."
                        </p>
                    </div>
                </div>
            </Show>
        </div>
    }
}

#[component]
fn OperationsTileGrid(snapshot: OperationsSnapshotView) -> impl IntoView {
    let tiles = snapshot.tiles.clone();
    let mailbox = snapshot.mailbox_id;

    let critical: Vec<_> = tiles
        .iter()
        .copied()
        .filter(|(t, _)| tile_severity(*t) == Severity::Critical)
        .collect();
    let warning: Vec<_> = tiles
        .iter()
        .copied()
        .filter(|(t, _)| tile_severity(*t) == Severity::Warning)
        .collect();
    let info: Vec<_> = tiles
        .iter()
        .copied()
        .filter(|(t, _)| tile_severity(*t) == Severity::Info)
        .collect();

    view! {
        <div class="spp-operations-grid">
            <TileSection title="Critical" tiles=critical mailbox=mailbox />
            <TileSection title="Warning" tiles=warning mailbox=mailbox />
            <TileSection title="Info" tiles=info mailbox=mailbox />
        </div>
    }
}

#[component]
fn TileSection(
    title: &'static str,
    tiles: Vec<(OperationsTileKey, TileCountView)>,
    mailbox: Option<i64>,
) -> impl IntoView {
    let has_tiles = !tiles.is_empty();
    let cards = move || {
        tiles
            .iter()
            .copied()
            .map(|(tile, count)| {
                view! {
                    <TileCard tile=tile count=count mailbox=mailbox />
                }
                .into_any()
            })
            .collect::<Vec<AnyView>>()
    };
    view! {
        <section class="spp-operations-section">
            <h3 class="spp-operations-section__title">{title}</h3>
            <Show
                when=move || has_tiles
                fallback=|| ().into_any()
            >
                <div class="spp-operations-section__grid">
                    {cards()}
                </div>
            </Show>
        </section>
    }
}

#[component]
fn TileCard(tile: OperationsTileKey, count: TileCountView, mailbox: Option<i64>) -> impl IntoView {
    let label = tile_label(tile);
    let description = tile_description(tile);
    let href: String = tile_inbox_link(tile, mailbox).unwrap_or_default();
    let has_link = !href.is_empty();
    let count_label = count.label();
    let is_not_available = count.is_not_available();
    let badge_class = format!("spp-tile__badge--{}", count.badge_class_suffix(tile));
    let tile_class = format!("spp-tile spp-tile--{}", tile_severity(tile).class_suffix());

    view! {
        <div class={tile_class} title={description}>
            <span class="spp-tile__label">{label}</span>
            <span class={badge_class}>{count_label}</span>
            <Show when=move || is_not_available fallback=|| ()>
                <span class="spp-tile__not-available">"Not yet available"</span>
            </Show>
            <Show when=move || has_link fallback=|| ()>
                <A
                    href={href.clone()}
                    attr:class="spp-tile__link"
                >
                    "Open in inbox →"
                </A>
            </Show>
        </div>
    }
}
