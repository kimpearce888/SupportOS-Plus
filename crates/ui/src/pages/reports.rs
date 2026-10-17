use leptos::prelude::*;

use crate::components::state_view::{EmptyState, LoadingState};
use crate::components::{BuilderTab, EffectivenessTab, FrictionTab};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportsTab {
    Overview,
    Effectiveness,
    Friction,
    Sla,
    WhyContacting,
    Intelligence,
    HelpScout,
    Definitions,
    Releases,
    Builder,
}

#[component]
pub fn ReportsPage() -> impl IntoView {
    let tab = RwSignal::new(ReportsTab::Overview);

    view! {
        <div class="spp-page spp-page--reports">
            <h2 class="spp-page__title">"Reports"</h2>
            <p class="spp-page__intro">
                "Every number is labeled: Help Scout (native API) · local calculation · AI-derived"
            </p>

            <div class="spp-tabs">
                <button
                    class="spp-tab"
                    class:is-active=move || tab.get() == ReportsTab::Overview
                    on:click=move |_| tab.set(ReportsTab::Overview)
                >
                    "Overview"
                </button>
                <button
                    class="spp-tab"
                    class:is-active=move || tab.get() == ReportsTab::Effectiveness
                    on:click=move |_| tab.set(ReportsTab::Effectiveness)
                >
                    "Response effectiveness"
                </button>
                <button
                    class="spp-tab"
                    class:is-active=move || tab.get() == ReportsTab::Friction
                    on:click=move |_| tab.set(ReportsTab::Friction)
                >
                    "Friction"
                </button>
                <button
                    class="spp-tab"
                    class:is-active=move || tab.get() == ReportsTab::Sla
                    on:click=move |_| tab.set(ReportsTab::Sla)
                >
                    "SLA"
                </button>
                <button
                    class="spp-tab"
                    class:is-active=move || tab.get() == ReportsTab::WhyContacting
                    on:click=move |_| tab.set(ReportsTab::WhyContacting)
                >
                    "Why contacting"
                </button>
                <button
                    class="spp-tab"
                    class:is-active=move || tab.get() == ReportsTab::Intelligence
                    on:click=move |_| tab.set(ReportsTab::Intelligence)
                >
                    "Intelligence"
                </button>
                <button
                    class="spp-tab"
                    class:is-active=move || tab.get() == ReportsTab::HelpScout
                    on:click=move |_| tab.set(ReportsTab::HelpScout)
                >
                    "Help Scout"
                </button>
                <button
                    class="spp-tab"
                    class:is-active=move || tab.get() == ReportsTab::Definitions
                    on:click=move |_| tab.set(ReportsTab::Definitions)
                >
                    "Definitions"
                </button>
                <button
                    class="spp-tab"
                    class:is-active=move || tab.get() == ReportsTab::Releases
                    on:click=move |_| tab.set(ReportsTab::Releases)
                >
                    "Releases"
                </button>
                <button
                    class="spp-tab"
                    class:is-active=move || tab.get() == ReportsTab::Builder
                    on:click=move |_| tab.set(ReportsTab::Builder)
                >
                    "Report builder"
                </button>
            </div>

            <Show when=move || tab.get() == ReportsTab::Overview fallback=|| ()>
                <OverviewTab />
            </Show>
            <Show when=move || tab.get() == ReportsTab::Effectiveness fallback=|| ()>
                <EffectivenessTab />
            </Show>
            <Show when=move || tab.get() == ReportsTab::Friction fallback=|| ()>
                <FrictionTab />
            </Show>
            <Show when=move || tab.get() == ReportsTab::Sla fallback=|| ()>
                <SlaTab />
            </Show>
            <Show when=move || tab.get() == ReportsTab::WhyContacting fallback=|| ()>
                <WhyContactingTab />
            </Show>
            <Show when=move || tab.get() == ReportsTab::Intelligence fallback=|| ()>
                <IntelligenceTab />
            </Show>
            <Show when=move || tab.get() == ReportsTab::HelpScout fallback=|| ()>
                <HelpScoutTab />
            </Show>
            <Show when=move || tab.get() == ReportsTab::Definitions fallback=|| ()>
                <DefinitionsTab />
            </Show>
            <Show when=move || tab.get() == ReportsTab::Releases fallback=|| ()>
                <ReleasesTab />
            </Show>
            <Show when=move || tab.get() == ReportsTab::Builder fallback=|| ()>
                <BuilderTab />
            </Show>
        </div>
    }
}

fn field(v: &serde_json::Value, key: &str) -> String {
    v.get(key)
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

fn int_field(v: &serde_json::Value, key: &str) -> i64 {
    v.get(key).and_then(|v| v.as_i64()).unwrap_or(0)
}

const RANGES: [(u32, &str); 4] = [
    (7, "7 days"),
    (30, "30 days"),
    (90, "90 days"),
    (365, "1 year"),
];

#[component]
fn DaysRanges(days: RwSignal<u32>) -> impl IntoView {
    view! {
        <div class="spp-flex spp-dashboard__ranges" role="group" aria-label="Date range">
            {RANGES
                .iter()
                .map(|(d, label)| {
                    let d = *d;
                    view! {
                        <button
                            class=move || {
                                if days.get() == d {
                                    "spp-button spp-button--primary spp-button--small"
                                } else {
                                    "spp-button spp-button--small"
                                }
                            }
                            type="button"
                            on:click=move |_| days.set(d)
                        >
                            {*label}
                        </button>
                    }
                })
                .collect::<Vec<_>>()}
        </div>
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct OverviewFacts {
    pub new_conversations: i64,
    pub replies_sent: i64,
    pub closed_conversations: i64,
    pub ratings_great: i64,
    pub ratings_okay: i64,
    pub ratings_not_good: i64,
}

#[must_use]
pub fn parse_overview(v: &serde_json::Value) -> OverviewFacts {
    OverviewFacts {
        new_conversations: v
            .get("new_conversations")
            .and_then(|x| x.as_i64())
            .unwrap_or(0),
        replies_sent: v.get("replies_sent").and_then(|x| x.as_i64()).unwrap_or(0),
        closed_conversations: v
            .get("closed_conversations")
            .and_then(|x| x.as_i64())
            .unwrap_or(0),
        ratings_great: v
            .pointer("/ratings/great")
            .and_then(|x| x.as_i64())
            .unwrap_or(0),
        ratings_okay: v
            .pointer("/ratings/okay")
            .and_then(|x| x.as_i64())
            .unwrap_or(0),
        ratings_not_good: v
            .pointer("/ratings/not-good")
            .and_then(|x| x.as_i64())
            .unwrap_or(0),
    }
}

#[component]
fn OverviewTab() -> impl IntoView {
    let facts = RwSignal::new(None::<OverviewFacts>);
    let loading = RwSignal::new(true);
    let error_msg = RwSignal::new(None::<String>);

    Effect::new(move |_| {
        let facts = facts;
        let loading = loading;
        let error_msg = error_msg;
        wasm_bindgen_futures::spawn_local(async move {
            match crate::api::get_json::<serde_json::Value>("/api/analytics/dashboard?daysBack=30")
                .await
            {
                Ok(v) => {
                    facts.set(Some(parse_overview(&v)));
                    loading.set(false);
                }
                Err(e) => {
                    error_msg.set(Some(e));
                    loading.set(false);
                }
            }
        });
    });

    view! {
        <div class="spp-grid-2">
            <div class="spp-card">
                <h3 class="spp-card__title">"Volume (local)"</h3>
                <Show when=move || loading.get() fallback=|| ()>
                    <LoadingState />
                </Show>
                <Show when=move || error_msg.get().is_some() fallback=|| ()>
                    <div class="spp-state spp-state--error">
                        <p class="spp-state__body">"This report failed to load."</p>
                        <p class="spp-state__detail">{move || error_msg.get().unwrap_or_default()}</p>
                    </div>
                </Show>
                {move || {
                    let f = facts.clone().get();
                    if loading.get() {
                        return ().into_any();
                    }
                    let Some(f) = f else {
                        return ().into_any();
                    };
                    view! {
                        <div class="spp-flex spp-flex--col spp-gap-4">
                            <div class="spp-flex spp-flex--between">
                                <span class="spp-muted spp-text-sm">"New conversations"</span>
                                <span class="spp-text-sm">{f.new_conversations.to_string()}</span>
                            </div>
                            <div class="spp-flex spp-flex--between">
                                <span class="spp-muted spp-text-sm">"Replies sent"</span>
                                <span class="spp-text-sm">{f.replies_sent.to_string()}</span>
                            </div>
                            <div class="spp-flex spp-flex--between">
                                <span class="spp-muted spp-text-sm">"Closed in range"</span>
                                <span class="spp-text-sm">{f.closed_conversations.to_string()}</span>
                            </div>
                            <div class="spp-flex spp-flex--between">
                                <span class="spp-muted spp-text-sm">"Ratings"</span>
                                <span class="spp-text-sm">
                                    {format!(
                                        "{} great / {} okay / {} not-good",
                                        f.ratings_great, f.ratings_okay, f.ratings_not_good
                                    )}
                                </span>
                            </div>
                        </div>
                        <p class="spp-muted spp-text-xs spp-mt-8">
                            "All numbers are local calculations over the mirror (last 30 days)."
                        </p>
                    }.into_any()
                }}
            </div>
            <div class="spp-card">
                <h3 class="spp-card__title">"Where to look next"</h3>
                <p class="spp-muted spp-text-sm">
                    "SLA reports measure BUSINESS minutes against per-mailbox targets; Why contacting and Intelligence are AI-derived; Help Scout numbers come from the native API; Definitions documents every local metric; Releases correlates conversation volume around release events."
                </p>
                <EmptyState message="Open a tab to run its report." />
            </div>
        </div>
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SlaMailboxView {
    pub mailbox_id: i64,
    pub mailbox_name: String,
    pub business_hours_configured: bool,
    pub schedule: String,
    pub conversations_in_range: i64,
    pub first_response: SlaDurationView,
    pub resolution: SlaDurationView,
    pub waiting: SlaWaitingView,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SlaDurationView {
    pub count: i64,
    pub avg_business_min: Option<i64>,
    pub median_business_min: Option<i64>,
    pub met: i64,
    pub missed: i64,
    pub no_target: i64,
    pub target_min: Option<i64>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SlaWaitingView {
    pub count: i64,
    pub at_risk: i64,
    pub oldest_business_min: Option<i64>,
}

#[must_use]
fn parse_duration(v: &serde_json::Value) -> SlaDurationView {
    SlaDurationView {
        count: int_field(v, "count"),
        avg_business_min: v.get("avg_business_min").and_then(|x| x.as_i64()),
        median_business_min: v.get("median_business_min").and_then(|x| x.as_i64()),
        met: int_field(v, "met"),
        missed: int_field(v, "missed"),
        no_target: int_field(v, "no_target"),
        target_min: v.get("target_min").and_then(|x| x.as_i64()),
    }
}

#[must_use]
fn parse_waiting(v: &serde_json::Value) -> SlaWaitingView {
    SlaWaitingView {
        count: int_field(v, "count"),
        at_risk: int_field(v, "at_risk"),
        oldest_business_min: v.get("oldest_business_min").and_then(|x| x.as_i64()),
    }
}

fn fmt_opt_min(v: Option<i64>) -> String {
    match v {
        Some(n) => format!("{n} min"),
        None => "—".to_string(),
    }
}

#[must_use]
pub fn parse_sla_report(v: &serde_json::Value) -> (Vec<SlaMailboxView>, Vec<String>) {
    let rows = v
        .get("mailboxes")
        .and_then(|m| m.as_array())
        .map(|arr| {
            arr.iter()
                .map(|m| {
                    let schedule = m
                        .get("schedule")
                        .map(|s| {
                            format!(
                                "{} {}-{} ({})",
                                field(s, "timezone"),
                                int_field(s, "startMinute"),
                                int_field(s, "endMinute"),
                                s.get("days")
                                    .and_then(|d| d.as_array())
                                    .map(|days| {
                                        days.iter().filter_map(|d| d.as_i64()).count().to_string()
                                    })
                                    .unwrap_or_else(|| "0".to_string())
                                    + " days"
                            )
                        })
                        .unwrap_or_default();
                    SlaMailboxView {
                        mailbox_id: int_field(m, "mailbox_id"),
                        mailbox_name: field(m, "mailbox_name"),
                        business_hours_configured: m
                            .get("business_hours_configured")
                            .and_then(|b| b.as_bool())
                            .unwrap_or(false),
                        schedule,
                        conversations_in_range: int_field(m, "conversations_in_range"),
                        first_response: parse_duration(
                            m.get("first_response").unwrap_or(&serde_json::Value::Null),
                        ),
                        resolution: parse_duration(
                            m.get("resolution").unwrap_or(&serde_json::Value::Null),
                        ),
                        waiting: parse_waiting(
                            m.get("waiting").unwrap_or(&serde_json::Value::Null),
                        ),
                    }
                })
                .collect()
        })
        .unwrap_or_default();
    let unconfigured = v
        .get("unconfigured_mailboxes")
        .and_then(|u| u.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|u| u.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    (rows, unconfigured)
}

#[component]
fn SlaTab() -> impl IntoView {
    let days = RwSignal::new(30u32);
    let rows = RwSignal::new(Vec::<SlaMailboxView>::new());
    let unconfigured = RwSignal::new(Vec::<String>::new());
    let range = RwSignal::new(String::new());
    let loading = RwSignal::new(true);
    let error_msg = RwSignal::new(None::<String>);

    Effect::new(move |_| {
        let days = days.get();
        let rows = rows;
        let unconfigured = unconfigured;
        let range = range;
        let loading = loading;
        let error_msg = error_msg;
        loading.set(true);
        error_msg.set(None);
        wasm_bindgen_futures::spawn_local(async move {
            let path = format!("/api/reports/sla?days={days}");
            match crate::api::get_json::<serde_json::Value>(&path).await {
                Ok(v) => {
                    let (parsed, uncfg) = parse_sla_report(&v);
                    rows.set(parsed);
                    unconfigured.set(uncfg);
                    let from = v
                        .pointer("/range/from")
                        .and_then(|x| x.as_str())
                        .unwrap_or("");
                    let to = v
                        .pointer("/range/to")
                        .and_then(|x| x.as_str())
                        .unwrap_or("");
                    range.set(format!("{from} → {to}"));
                }
                Err(e) => error_msg.set(Some(e)),
            }
            loading.set(false);
        });
    });

    view! {
        <div class="spp-flex spp-flex--between spp-mb-8">
            <p class="spp-muted spp-text-xs">
                {move || format!(
                    "Business-minutes SLA per mailbox ({}). Met/missed count only mailboxes with a target; no-target pairs are listed separately.",
                    range.get()
                )}
            </p>
            <DaysRanges days=days />
        </div>

        <Show when=move || loading.get() fallback=|| ()>
            <LoadingState />
        </Show>
        <Show when=move || error_msg.get().is_some() fallback=|| ()>
            <div class="spp-state spp-state--error">
                <span class="spp-state__icon" aria-hidden="true">"⚠"</span>
                <p class="spp-state__body">{move || error_msg.get().unwrap_or_default()}</p>
            </div>
        </Show>
        <Show
            when=move || !loading.get() && error_msg.get().is_none()
            fallback=|| ()
        >
            <Show when=move || !unconfigured.get().is_empty() fallback=|| ()>
                <p class="spp-muted spp-text-xs spp-mb-8">
                    "Unconfigured mailboxes (no business hours yet): "
                    {move || {
                        unconfigured
                            .get()
                            .iter()
                            .map(|m| {
                                view! { <span class="spp-badge">{m.clone()}</span> }
                            })
                            .collect::<Vec<_>>()
                    }}
                </p>
            </Show>
            <Show
                when=move || !rows.with(|r| r.is_empty())
                fallback=|| {
                    view! {
                        <EmptyState message="No conversations in range — nothing to measure yet." />
                    }
                }
            >
                <div class="spp-card">
                    <table class="spp-table">
                        <thead>
                            <tr>
                                <th>"Mailbox"</th>
                                <th>"Conversations"</th>
                                <th>"First response (business min)"</th>
                                <th>"Resolution (business min)"</th>
                                <th>"Waiting"</th>
                            </tr>
                        </thead>
                        <tbody>
                            {move || {
                                rows.get()
                                    .iter()
                                    .map(|m| {
                                        view! {
                                            <tr>
                                                <td>
                                                    <strong>{m.mailbox_name.clone()}</strong>
                                                    {if m.business_hours_configured {
                                                        view! {
                                                            <div class="spp-muted spp-text-xs">
                                                                {m.schedule.clone()}
                                                            </div>
                                                        }.into_any()
                                                    } else {
                                                        view! {
                                                            <div class="spp-muted spp-text-xs">
                                                                "no business hours configured"
                                                            </div>
                                                        }.into_any()
                                                    }}
                                                </td>
                                                <td>{m.conversations_in_range.to_string()}</td>
                                                <td class="spp-table__cell-muted">
                                                    {format!(
                                                        "avg {} · median {} · {}/{} met (target {})",
                                                        fmt_opt_min(m.first_response.avg_business_min),
                                                        fmt_opt_min(m.first_response.median_business_min),
                                                        m.first_response.met,
                                                        m.first_response.met + m.first_response.missed,
                                                        m.first_response
                                                            .target_min
                                                            .map(|t| format!("{t} min"))
                                                            .unwrap_or_else(|| "—".to_string()),
                                                    )}
                                                    {if m.first_response.missed > 0 {
                                                        view! {
                                                            <span class="spp-badge spp-badge--warn">
                                                                {format!("{} missed", m.first_response.missed)}
                                                            </span>
                                                        }.into_any()
                                                    } else {
                                                        ().into_any()
                                                    }}
                                                </td>
                                                <td class="spp-table__cell-muted">
                                                    {format!(
                                                        "avg {} · median {} · {}/{} met",
                                                        fmt_opt_min(m.resolution.avg_business_min),
                                                        fmt_opt_min(m.resolution.median_business_min),
                                                        m.resolution.met,
                                                        m.resolution.met + m.resolution.missed,
                                                    )}
                                                    {if m.resolution.no_target > 0 {
                                                        view! {
                                                            <span class="spp-badge">
                                                                {format!("{} no-target", m.resolution.no_target)}
                                                            </span>
                                                        }.into_any()
                                                    } else {
                                                        ().into_any()
                                                    }}
                                                </td>
                                                <td class="spp-table__cell-muted">
                                                    {format!(
                                                        "{} waiting · {} at risk · oldest {}",
                                                        m.waiting.count,
                                                        m.waiting.at_risk,
                                                        fmt_opt_min(m.waiting.oldest_business_min),
                                                    )}
                                                </td>
                                            </tr>
                                        }
                                    })
                                    .collect::<Vec<_>>()
                            }}
                        </tbody>
                    </table>
                </div>
                <p class="spp-muted spp-text-xs spp-mt-8">
                    "Local calculation over the mirror — business minutes follow each mailbox's configured hours."
                </p>
            </Show>
        </Show>
    }
}

#[component]
fn WhyContactingTab() -> impl IntoView {
    let days = RwSignal::new(30u32);
    let categories = RwSignal::new(Vec::<serde_json::Value>::new());
    let questions = RwSignal::new(Vec::<serde_json::Value>::new());
    let loading = RwSignal::new(true);
    let error_msg = RwSignal::new(None::<String>);

    Effect::new(move |_| {
        let days = days.get();
        let categories = categories;
        let questions = questions;
        let loading = loading;
        let error_msg = error_msg;
        loading.set(true);
        error_msg.set(None);
        wasm_bindgen_futures::spawn_local(async move {
            let why = crate::api::get_json::<serde_json::Value>(&format!(
                "/api/reports/why-contacting?days={days}"
            ))
            .await;
            let top = crate::api::get_json::<serde_json::Value>(&format!(
                "/api/reports/top-questions?days={days}"
            ))
            .await;
            match (why, top) {
                (Ok(w), Ok(t)) => {
                    categories.set(
                        w.get("categories")
                            .and_then(|c| c.as_array())
                            .cloned()
                            .unwrap_or_default(),
                    );
                    questions.set(
                        t.get("questions")
                            .and_then(|q| q.as_array())
                            .cloned()
                            .unwrap_or_default(),
                    );
                }
                (Err(e), _) | (_, Err(e)) => error_msg.set(Some(e)),
            }
            loading.set(false);
        });
    });

    view! {
        <div class="spp-flex spp-flex--between spp-mb-8">
            <p class="spp-muted spp-text-xs">
                "AI-derived: the latest completed ticket analysis per conversation, grouped by its issue-cluster candidate and primary question."
            </p>
            <DaysRanges days=days />
        </div>

        <Show when=move || loading.get() fallback=|| ()>
            <LoadingState />
        </Show>
        <Show when=move || error_msg.get().is_some() fallback=|| ()>
            <div class="spp-state spp-state--error">
                <span class="spp-state__icon" aria-hidden="true">"⚠"</span>
                <p class="spp-state__body">{move || error_msg.get().unwrap_or_default()}</p>
            </div>
        </Show>
        <Show when=move || !loading.get() && error_msg.get().is_none() fallback=|| ()>
            <div class="spp-card spp-mb-8">
                <h3 class="spp-card__title">"Why are customers contacting us?"</h3>
                <Show
                    when=move || !categories.with(|c| c.is_empty())
                    fallback=|| view! { <EmptyState message="No completed AI analyses in range yet." /> }
                >
                    <table class="spp-table">
                        <thead>
                            <tr>
                                <th>"Category"</th>
                                <th>"Conversations"</th>
                                <th>"Examples"</th>
                            </tr>
                        </thead>
                        <tbody>
                            {move || {
                                categories.get()
                                    .iter()
                                    .map(|c| {
                                        let name = field(c, "category");
                                        let count = int_field(c, "count");
                                        let ids = c.get("conversation_ids")
                                            .and_then(|v| v.as_array())
                                            .map(|arr| {
                                                arr.iter().filter_map(|i| i.as_i64()).collect::<Vec<_>>()
                                            })
                                            .unwrap_or_default();
                                        view! {
                                            <tr>
                                                <td>{name}</td>
                                                <td><span class="spp-badge">{count.to_string()}</span></td>
                                                <td>
                                                    {ids.iter().take(5).map(|cid| {
                                                        view! {
                                                            <a class="spp-badge" href={format!("/inbox/conversation/{cid}")}>
                                                                {format!("#{cid}")}
                                                            </a>
                                                        }
                                                    }).collect::<Vec<_>>()}
                                                </td>
                                            </tr>
                                        }
                                    })
                                    .collect::<Vec<_>>()
                            }}
                        </tbody>
                    </table>
                </Show>
            </div>

            <div class="spp-card">
                <h3 class="spp-card__title">"Top questions"</h3>
                <Show
                    when=move || !questions.with(|q| q.is_empty())
                    fallback=|| view! { <EmptyState message="No recurring questions detected in range." /> }
                >
                    <table class="spp-table">
                        <thead>
                            <tr>
                                <th>"Question"</th>
                                <th>"Conversations"</th>
                                <th>"Examples"</th>
                            </tr>
                        </thead>
                        <tbody>
                            {move || {
                                questions.get()
                                    .iter()
                                    .map(|q| {
                                        let text = field(q, "question");
                                        let count = int_field(q, "count");
                                        let ids = q.get("conversation_ids")
                                            .and_then(|v| v.as_array())
                                            .map(|arr| {
                                                arr.iter().filter_map(|i| i.as_i64()).collect::<Vec<_>>()
                                            })
                                            .unwrap_or_default();
                                        view! {
                                            <tr>
                                                <td>{text}</td>
                                                <td><span class="spp-badge">{count.to_string()}</span></td>
                                                <td>
                                                    {ids.iter().take(5).map(|cid| {
                                                        view! {
                                                            <a class="spp-badge" href={format!("/inbox/conversation/{cid}")}>
                                                                {format!("#{cid}")}
                                                            </a>
                                                        }
                                                    }).collect::<Vec<_>>()}
                                                </td>
                                            </tr>
                                        }
                                    })
                                    .collect::<Vec<_>>()
                            }}
                        </tbody>
                    </table>
                </Show>
            </div>
        </Show>
    }
}

#[component]
fn IntelligenceTab() -> impl IntoView {
    let alerts = RwSignal::new(Vec::<serde_json::Value>::new());
    let loading = RwSignal::new(true);
    let error_msg = RwSignal::new(None::<String>);

    Effect::new(move |_| {
        let _ = crate::queries::version("dashboard").get();
        let alerts = alerts;
        let loading = loading;
        let error_msg = error_msg;
        loading.set(true);
        error_msg.set(None);
        wasm_bindgen_futures::spawn_local(async move {
            match crate::api::get_json::<serde_json::Value>("/api/reports/issue-radar").await {
                Ok(v) => alerts.set(
                    v.get("alerts")
                        .and_then(|a| a.as_array())
                        .cloned()
                        .unwrap_or_default(),
                ),
                Err(e) => error_msg.set(Some(e)),
            }
            loading.set(false);
        });
    });

    view! {
        <p class="spp-muted spp-text-xs spp-mb-8">
            "AI-derived issue intelligence: associations over the issue clusters, top questions, escalations and ratings. Correlations are never claimed as causes — every alert links to its underlying tickets."
        </p>

        <Show when=move || loading.get() fallback=|| ()>
            <LoadingState />
        </Show>
        <Show when=move || error_msg.get().is_some() fallback=|| ()>
            <div class="spp-state spp-state--error">
                <span class="spp-state__icon" aria-hidden="true">"⚠"</span>
                <p class="spp-state__body">{move || error_msg.get().unwrap_or_default()}</p>
            </div>
        </Show>
        <Show when=move || !loading.get() && error_msg.get().is_none() fallback=|| ()>
            <Show
                when=move || !alerts.with(|a| a.is_empty())
                fallback=|| view! { <EmptyState message="No intelligence alerts. The radar is clear." /> }
            >
                <div class="spp-card">
                    <ul class="spp-issues__alert-list">
                        {move || {
                            alerts.get()
                                .iter()
                                .map(|a| {
                                    let title = {
                                        let t = field(a, "title");
                                        if t.is_empty() { "(alert)".to_string() } else { t }
                                    };
                                    let detail = field(a, "detail");
                                    let severity = {
                                        let s = field(a, "severity");
                                        if s.is_empty() { "info".to_string() } else { s }
                                    };
                                    let ids = a.get("conversation_ids")
                                        .and_then(|v| v.as_array())
                                        .map(|arr| {
                                            arr.iter().filter_map(|i| i.as_i64()).collect::<Vec<_>>()
                                        })
                                        .unwrap_or_default();
                                    view! {
                                        <li class=match severity.as_str() {
                                            "critical" => "spp-issues__alert spp-issues__alert--critical",
                                            "warning" => "spp-issues__alert spp-issues__alert--warning",
                                            _ => "spp-issues__alert spp-issues__alert--info",
                                        }>
                                            <strong>{title}</strong>
                                            <p class="spp-muted spp-text-xs">{detail}</p>
                                            <div class="spp-issues__alert-convs">
                                                {ids.iter().take(6).map(|cid| {
                                                    view! {
                                                        <a class="spp-badge" href={format!("/inbox/conversation/{cid}")}>
                                                            {format!("#{cid}")}
                                                        </a>
                                                    }
                                                }).collect::<Vec<_>>()}
                                            </div>
                                        </li>
                                    }
                                })
                                .collect::<Vec<_>>()
                        }}
                    </ul>
                </div>
            </Show>
        </Show>
    }
}

const HS_KEYS: [(&str, &str); 4] = [
    ("company", "Company"),
    ("conversations", "Conversations"),
    ("happiness", "Happiness"),
    ("productivity", "Productivity"),
];

#[component]
fn HelpScoutTab() -> impl IntoView {
    let key = RwSignal::new("company");
    let days = RwSignal::new(30u32);
    let report = RwSignal::new(serde_json::json!({}));
    let note = RwSignal::new(String::new());
    let loading = RwSignal::new(true);
    let error_msg = RwSignal::new(None::<String>);

    Effect::new(move |_| {
        let key = key.get();
        let days = days.get();
        let report = report;
        let note = note;
        let loading = loading;
        let error_msg = error_msg;
        loading.set(true);
        error_msg.set(None);
        wasm_bindgen_futures::spawn_local(async move {
            let path = format!("/api/reports/helpscout/{key}?days={days}");
            match crate::api::get_json::<serde_json::Value>(&path).await {
                Ok(v) => {
                    if v.get("ok").and_then(|o| o.as_bool()).unwrap_or(false) {
                        report.set(v.get("report").cloned().unwrap_or_default());
                        note.set(field(&v, "note"));
                    } else {
                        error_msg.set(Some(
                            v.get("message")
                                .and_then(|m| m.as_str())
                                .unwrap_or("Help Scout report unavailable")
                                .to_string(),
                        ));
                    }
                }
                Err(e) => error_msg.set(Some(e)),
            }
            loading.set(false);
        });
    });

    view! {
        <div class="spp-flex spp-flex--between spp-mb-8">
            <div class="spp-tabs" role="group" aria-label="Help Scout report">
                {HS_KEYS
                    .iter()
                    .map(|(k, label)| {
                        let k = *k;
                        view! {
                            <button
                                class=move || {
                                    if key.get() == k { "spp-tab is-active" } else { "spp-tab" }
                                }
                                type="button"
                                on:click=move |_| key.set(k)
                            >
                                {*label}
                            </button>
                        }
                    })
                    .collect::<Vec<_>>()}
            </div>
            <DaysRanges days=days />
        </div>

        <Show when=move || loading.get() fallback=|| ()>
            <LoadingState />
        </Show>
        <Show when=move || error_msg.get().is_some() fallback=|| ()>
            <div class="spp-state spp-state--error">
                <span class="spp-state__icon" aria-hidden="true">"⚠"</span>
                <p class="spp-state__body">{move || error_msg.get().unwrap_or_default()}</p>
            </div>
        </Show>
        <Show when=move || !loading.get() && error_msg.get().is_none() fallback=|| ()>
            <div class="spp-card">
                <h3 class="spp-card__title">
                    {move || {
                        HS_KEYS
                            .iter()
                            .find(|(k, _)| *k == key.get())
                            .map(|(_, label)| format!("{label} (Help Scout native)"))
                            .unwrap_or_default()
                    }}
                </h3>
                <Show
                    when=move || !report.with(|r| r.as_object().map(|o| !o.is_empty()).unwrap_or(false))
                    fallback=|| view! { <EmptyState message="This report returned no rows." /> }
                >
                    {move || {
                        report.get()
                            .as_object()
                            .map(|obj| {
                                obj.iter()
                                    .map(|(k, v)| {
                                        let value = match v {
                                            serde_json::Value::Number(n) => n.to_string(),
                                            serde_json::Value::String(s) => s.clone(),
                                            serde_json::Value::Bool(b) => b.to_string(),
                                            serde_json::Value::Array(a) => format!("{} rows", a.len()),
                                            other => serde_json::to_string(other).unwrap_or_default(),
                                        };
                                        view! {
                                            <div class="spp-flex spp-flex--between">
                                                <span class="spp-muted spp-text-sm">{k.clone()}</span>
                                                <span class="spp-text-sm">{value}</span>
                                            </div>
                                        }
                                    })
                                    .collect::<Vec<_>>()
                            })
                            .unwrap_or_default()
                    }}
                </Show>
                <Show when=move || !note.with(|n| n.is_empty()) fallback=|| ()>
                    <p class="spp-muted spp-text-xs spp-mt-8">{move || note.get()}</p>
                </Show>
            </div>
        </Show>
    }
}

#[component]
fn DefinitionsTab() -> impl IntoView {
    let definitions = RwSignal::new(Vec::<serde_json::Value>::new());
    let loading = RwSignal::new(true);
    let error_msg = RwSignal::new(None::<String>);

    Effect::new(move |_| {
        let definitions = definitions;
        let loading = loading;
        let error_msg = error_msg;
        loading.set(true);
        error_msg.set(None);
        wasm_bindgen_futures::spawn_local(async move {
            match crate::api::get_json::<serde_json::Value>("/api/reports/metric-definitions").await
            {
                Ok(v) => definitions.set(
                    v.get("definitions")
                        .and_then(|d| d.as_array())
                        .cloned()
                        .unwrap_or_default(),
                ),
                Err(e) => error_msg.set(Some(e)),
            }
            loading.set(false);
        });
    });

    view! {
        <p class="spp-muted spp-text-xs spp-mb-8">
            "Every local metric is documented with its formula, source and limitations. Numbers elsewhere on this page link back to these definitions."
        </p>

        <Show when=move || loading.get() fallback=|| ()>
            <LoadingState />
        </Show>
        <Show when=move || error_msg.get().is_some() fallback=|| ()>
            <div class="spp-state spp-state--error">
                <span class="spp-state__icon" aria-hidden="true">"⚠"</span>
                <p class="spp-state__body">{move || error_msg.get().unwrap_or_default()}</p>
            </div>
        </Show>
        <Show when=move || !loading.get() && error_msg.get().is_none() fallback=|| ()>
            <Show
                when=move || !definitions.with(|d| d.is_empty())
                fallback=|| view! { <EmptyState message="No metric definitions found." /> }
            >
                <div class="spp-card">
                    <table class="spp-table">
                        <thead>
                            <tr>
                                <th>"Metric"</th>
                                <th>"Formula"</th>
                                <th>"Source"</th>
                                <th>"Limitations"</th>
                            </tr>
                        </thead>
                        <tbody>
                            {move || {
                                definitions.get()
                                    .iter()
                                    .map(|d| {
                                        let name = field(d, "name");
                                        let key = field(d, "key");
                                        let description = field(d, "description");
                                        let formula = field(d, "formula");
                                        let source = field(d, "source");
                                        let limitations = field(d, "limitations");
                                        view! {
                                            <tr>
                                                <td>
                                                    <strong>{name}</strong>
                                                    <div class="spp-muted spp-text-xs">{key}</div>
                                                    {if !description.is_empty() {
                                                        view! {
                                                            <div class="spp-muted spp-text-xs">{description}</div>
                                                        }.into_any()
                                                    } else {
                                                        ().into_any()
                                                    }}
                                                </td>
                                                <td class="spp-table__cell-muted">{formula}</td>
                                                <td><span class="spp-badge">{source}</span></td>
                                                <td class="spp-table__cell-muted">{limitations}</td>
                                            </tr>
                                        }
                                    })
                                    .collect::<Vec<_>>()
                            }}
                        </tbody>
                    </table>
                </div>
            </Show>
        </Show>
    }
}

#[component]
fn ReleasesTab() -> impl IntoView {
    let releases = RwSignal::new(Vec::<serde_json::Value>::new());
    let note = RwSignal::new(String::new());
    let loading = RwSignal::new(true);
    let error_msg = RwSignal::new(None::<String>);
    let name = RwSignal::new(String::new());
    let version = RwSignal::new(String::new());
    let occurred_at = RwSignal::new(String::new());
    let notes = RwSignal::new(String::new());
    let submitting = RwSignal::new(false);

    let reload = RwSignal::new(0u32);
    Effect::new(move |_| {
        let _ = reload.get();
        loading.set(true);
        error_msg.set(None);
        wasm_bindgen_futures::spawn_local(async move {
            match crate::api::get_json::<serde_json::Value>("/api/reports/release-correlation")
                .await
            {
                Ok(v) => {
                    releases.set(
                        v.get("releases")
                            .and_then(|r| r.as_array())
                            .cloned()
                            .unwrap_or_default(),
                    );
                    note.set(field(&v, "note"));
                }
                Err(e) => error_msg.set(Some(e)),
            }
            loading.set(false);
        });
    });

    let do_record = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let name_v = name.get_untracked();
        if name_v.trim().is_empty() {
            crate::toasts::error("Name is required (1-200 characters).");
            return;
        }
        let occurred_v = occurred_at.get_untracked();
        if occurred_v.trim().is_empty() {
            crate::toasts::error("Occurred-at date is required (YYYY-MM-DD).");
            return;
        }
        if submitting.get_untracked() {
            return;
        }
        submitting.set(true);
        let body = serde_json::json!({
            "name": name_v.trim(),
            "version": version.get_untracked(),
            "occurredAt": occurred_v.trim(),
            "notes": notes.get_untracked(),
        });
        wasm_bindgen_futures::spawn_local(async move {
            let result = crate::api::post_json::<serde_json::Value>(
                "/api/reports/release-events",
                Some(&body),
            )
            .await;
            match result {
                Ok(r) if r.get("ok").and_then(|o| o.as_bool()).unwrap_or(false) => {
                    crate::toasts::success("Release event recorded.");
                    name.set(String::new());
                    version.set(String::new());
                    occurred_at.set(String::new());
                    notes.set(String::new());
                    reload.update(|t| *t += 1);
                }
                Ok(r) => crate::toasts::error(
                    r.get("message")
                        .and_then(|m| m.as_str())
                        .unwrap_or("Save failed"),
                ),
                Err(e) => crate::toasts::error(e),
            }
            submitting.set(false);
        });
    };

    view! {
        <p class="spp-muted spp-text-xs spp-mb-8">
            {move || note.get()}
        </p>

        <Show when=move || loading.get() fallback=|| ()>
            <LoadingState />
        </Show>
        <Show when=move || error_msg.get().is_some() fallback=|| ()>
            <div class="spp-state spp-state--error">
                <span class="spp-state__icon" aria-hidden="true">"⚠"</span>
                <p class="spp-state__body">{move || error_msg.get().unwrap_or_default()}</p>
            </div>
        </Show>
        <Show when=move || !loading.get() && error_msg.get().is_none() fallback=|| ()>
            <Show
                when=move || !releases.with(|r| r.is_empty())
                fallback=|| view! { <EmptyState message="No release events recorded yet — add the first one below." /> }
            >
                <div class="spp-card spp-mb-8">
                    <h3 class="spp-card__title">"Release correlation"</h3>
                    <table class="spp-table">
                        <thead>
                            <tr>
                                <th>"Release"</th>
                                <th>"Version"</th>
                                <th>"Occurred at"</th>
                                <th>"Conversations 7d before"</th>
                                <th>"Conversations 7d after"</th>
                            </tr>
                        </thead>
                        <tbody>
                            {move || {
                                releases.get()
                                    .iter()
                                    .map(|r| {
                                        let name = field(r, "release");
                                        let version = field(r, "version");
                                        let occurred = field(r, "occurred_at");
                                        let before = int_field(r, "before_7d");
                                        let after = int_field(r, "after_7d");
                                        view! {
                                            <tr>
                                                <td>{name}</td>
                                                <td class="spp-table__cell-muted">{version}</td>
                                                <td class="spp-table__cell-muted">{occurred}</td>
                                                <td>{before.to_string()}</td>
                                                <td>
                                                    {after.to_string()}
                                                    {if after > before {
                                                        view! {
                                                            <span class="spp-badge spp-badge--warn">
                                                                {format!("+{}", after - before)}
                                                            </span>
                                                        }.into_any()
                                                    } else {
                                                        ().into_any()
                                                    }}
                                                </td>
                                            </tr>
                                        }
                                    })
                                    .collect::<Vec<_>>()
                            }}
                        </tbody>
                    </table>
                </div>
            </Show>

            <div class="spp-card">
                <h3 class="spp-card__title">"Record a release event"</h3>
                <p class="spp-muted spp-text-xs spp-mb-8">
                    "Local record for correlation only — this never writes to Help Scout."
                </p>
                <form on:submit=do_record>
                    <div class="spp-form-grid">
                        <label class="spp-form-grid__label">"Name *"</label>
                        <input
                            class="spp-input"
                            maxlength=200
                            placeholder="e.g. checkout-service"
                            prop:value=name
                            on:input=move |ev| name.set(event_target_value(&ev))
                        />
                        <label class="spp-form-grid__label">"Version"</label>
                        <input
                            class="spp-input"
                            maxlength=100
                            placeholder="e.g. 2.4.1"
                            prop:value=version
                            on:input=move |ev| version.set(event_target_value(&ev))
                        />
                        <label class="spp-form-grid__label">"Occurred at *"</label>
                        <input
                            class="spp-input"
                            type="date"
                            prop:value=occurred_at
                            on:input=move |ev| occurred_at.set(event_target_value(&ev))
                        />
                        <label class="spp-form-grid__label">"Notes"</label>
                        <textarea
                            class="spp-input"
                            rows=2
                            maxlength=2000
                            prop:value=notes
                            on:input=move |ev| notes.set(event_target_value(&ev))
                        ></textarea>
                    </div>
                    <div class="spp-modal__actions spp-mt-8">
                        <button
                            class="spp-button spp-button--primary"
                            type="submit"
                            disabled=move || submitting.get()
                        >
                            {move || if submitting.get() { "Recording…" } else { "Record release" }}
                        </button>
                    </div>
                </form>
            </div>
        </Show>
    }
}
