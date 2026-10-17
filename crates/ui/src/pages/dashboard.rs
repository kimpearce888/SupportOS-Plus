use leptos::prelude::*;
use leptos_router::components::A;
use leptos_router::hooks::{use_location, use_navigate, use_query_map};
use std::sync::Arc;

use crate::components::state_view::{EmptyState, LoadingState};

const RANGES: [(u32, &str); 4] = [
    (7, "7 days"),
    (30, "30 days"),
    (90, "90 days"),
    (365, "1 year"),
];

const CHANNELS: [(&str, &str); 3] = [
    ("", "All channels"),
    ("email", "Email"),
    ("chat", "Chat (Beacon)"),
];

type SetParamFn = Arc<dyn Fn(&str, Option<&str>) + Send + Sync>;

#[component]
pub fn DashboardPage() -> impl IntoView {
    let query_map = use_query_map();
    let location = use_location();
    let navigate = use_navigate();

    let days = Memo::new(move |_| {
        let m = query_map.get();
        crate::url_state::query_pos_int(&m, "days")
            .map(|d| (d as u32).clamp(1, 3650))
            .unwrap_or(30)
    });
    let mailbox_ids = Memo::new(move |_| {
        let m = query_map.get();
        crate::url_state::query_str(&m, "mailboxes")
            .map(|raw| {
                raw.split(',')
                    .filter_map(|t| t.trim().parse::<i64>().ok().filter(|id| *id > 0))
                    .collect::<Vec<i64>>()
            })
            .unwrap_or_default()
    });
    let channel = Memo::new(move |_| {
        let m = query_map.get();
        match crate::url_state::query_str(&m, "channel").as_deref() {
            Some("email") => Some("email".to_string()),
            Some("chat") => Some("chat".to_string()),
            _ => None,
        }
    });

    let set_param: SetParamFn = {
        let navigate = navigate.clone();
        let pathname = location.pathname;
        Arc::new(move |key: &str, value: Option<&str>| {
            let m = query_map.get_untracked();
            let mut pairs: Vec<crate::url_state::Param> = Vec::new();
            for k in ["days", "mailboxes", "channel"] {
                if k == key {
                    pairs.push((k, value.map(str::to_string)));
                } else {
                    pairs.push((k, crate::url_state::query_str(&m, k)));
                }
            }
            crate::url_state::replace_query(&navigate, &pathname.get_untracked(), &pairs);
        })
    };
    let set_param_stored = StoredValue::new_local(set_param);

    let toggle_mailbox_stored = StoredValue::new_local({
        Arc::new(move |id: i64| {
            let mut next: Vec<i64> = mailbox_ids.get_untracked();
            if next.contains(&id) {
                next.retain(|m| *m != id);
            } else {
                next.push(id);
            }
            let joined = next
                .iter()
                .map(std::string::ToString::to_string)
                .collect::<Vec<_>>()
                .join(",");
            set_param_stored.with_value(|f| {
                f(
                    "mailboxes",
                    if next.is_empty() {
                        None
                    } else {
                        Some(joined.as_str())
                    },
                )
            });
        }) as Arc<dyn Fn(i64) + Send + Sync>
    });

    let metrics = RwSignal::new(None::<serde_json::Value>);
    let metrics_loading = RwSignal::new(true);
    let metrics_error = RwSignal::new(None::<String>);
    let mailboxes = RwSignal::new(Vec::<serde_json::Value>::new());
    let health = RwSignal::new(None::<serde_json::Value>);
    let radar = RwSignal::new(Vec::<serde_json::Value>::new());

    Effect::new(move |_| {
        let days = days.get();
        let mailbox_ids = mailbox_ids.get();
        let channel = channel.get();
        let _ = crate::queries::version("dashboard").get();
        let metrics = metrics;
        let metrics_loading = metrics_loading;
        let metrics_error = metrics_error;
        metrics_loading.set(true);
        wasm_bindgen_futures::spawn_local(async move {
            let mut path = format!("/api/analytics/dashboard?days={days}");
            if !mailbox_ids.is_empty() {
                let joined = mailbox_ids
                    .iter()
                    .map(std::string::ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(",");
                path.push_str(&format!("&mailboxIds={joined}"));
            }
            if let Some(channel) = channel.as_deref() {
                path.push_str(&format!("&channel={channel}"));
            }
            match crate::api::get_json::<serde_json::Value>(&path).await {
                Ok(v) => {
                    metrics.set(Some(v));
                    metrics_error.set(None);
                }
                Err(e) => metrics_error.set(Some(e)),
            }
            metrics_loading.set(false);
        });
    });

    wasm_bindgen_futures::spawn_local(async move {
        if let Ok(v) = crate::api::get_json::<serde_json::Value>("/api/mailboxes").await {
            mailboxes.set(
                v.get("mailboxes")
                    .and_then(|m| m.as_array())
                    .cloned()
                    .unwrap_or_default(),
            );
        }
    });

    let load_health = move || {
        let health = health;
        wasm_bindgen_futures::spawn_local(async move {
            if let Ok(v) =
                crate::api::get_json::<serde_json::Value>("/health/detailed?format=ui").await
            {
                health.set(Some(v));
            }
        });
    };
    load_health();
    set_interval(load_health, std::time::Duration::from_secs(60));

    let load_radar = move || {
        let radar = radar;
        wasm_bindgen_futures::spawn_local(async move {
            if let Ok(v) =
                crate::api::get_json::<serde_json::Value>("/api/reports/issue-radar").await
            {
                radar.set(
                    v.get("alerts")
                        .and_then(|a| a.as_array())
                        .cloned()
                        .unwrap_or_default(),
                );
            }
        });
    };
    load_radar();
    set_interval(load_radar, std::time::Duration::from_secs(120));

    view! {
        <div class="spp-page spp-page--dashboard">
            <div class="spp-flex spp-flex--between">
                <div>
                    <h2 class="spp-page__title">"Dashboard"</h2>
                    <p class="spp-page__intro">
                        "Local metrics · sources labeled Help Scout / local / AI-derived everywhere"
                        {move || {
                            let n = mailbox_ids.get().len();
                            if n > 0 {
                                format!(" · {n} mailbox{} selected", if n > 1 { "es" } else { "" })
                            } else {
                                " · all mailboxes".to_string()
                            }
                        }}
                        {move || match channel.get().as_deref() {
                            Some("chat") => " · chat (Beacon) channel".to_string(),
                            Some("email") => " · email channel".to_string(),
                            _ => String::new(),
                        }}
                    </p>
                </div>
                <div class="spp-flex spp-dashboard__ranges" role="group" aria-label="Date range">
                    {RANGES.iter().map(|(d, label)| {
                        let d = *d;
                        let label = *label;
                        view! {
                            <button
                                class=move || {
                                    if days.get() == d {
                                        "spp-button spp-button--primary spp-button--small"
                                    } else {
                                        "spp-button spp-button--small"
                                    }
                                }
                                on:click=move |_| {
                                    set_param_stored.with_value(|f| f("days", Some(d.to_string().as_str())));
                                }
                            >
                                {label}
                            </button>
                        }
                    }).collect::<Vec<_>>()}
                </div>
            </div>

            <div class="spp-card spp-dashboard__scope">
                <div class="spp-flex spp-flex--wrap spp-dashboard__scope-row">
                    <span class="spp-dashboard__scope-label">"Mailboxes"</span>
                    <button
                        class=move || {
                            if mailbox_ids.get().is_empty() {
                                "spp-button spp-button--primary spp-button--small"
                            } else {
                                "spp-button spp-button--small"
                            }
                        }
                        on:click=move |_| {
                            set_param_stored.with_value(|f| f("mailboxes", None));
                        }
                    >
                        "All"
                    </button>
                    {move || {
                        mailboxes.get().iter().map(|m| {
                            let id = m.get("id").and_then(|v| v.as_i64()).unwrap_or_default();
                            let name = m.get("name").and_then(|v| v.as_str()).unwrap_or("(unnamed)").to_string();
                            view! {
                                <button
                                    class=move || {
                                        if mailbox_ids.get().contains(&id) {
                                            "spp-button spp-button--primary spp-button--small"
                                        } else {
                                            "spp-button spp-button--small"
                                        }
                                    }
                                    aria-pressed=move || mailbox_ids.get().contains(&id)
                                    on:click=move |_| toggle_mailbox_stored.with_value(|f| f(id))
                                >
                                    {name}
                                </button>
                            }
                        }).collect::<Vec<_>>()
                    }}
                    <span class="spp-dashboard__scope-label spp-dashboard__scope-label--gap">"Channel"</span>
                    {CHANNELS.iter().map(|(value, label)| {
                        let value = *value;
                        let label = *label;
                        view! {
                            <button
                                class=move || {
                                    let active = match value {
                                        "" => channel.get().is_none(),
                                        v => channel.get().as_deref() == Some(v),
                                    };
                                    if active {
                                        "spp-button spp-button--primary spp-button--small"
                                    } else {
                                        "spp-button spp-button--small"
                                    }
                                }
                                aria-pressed=move || {
                                    match value {
                                        "" => channel.get().is_none(),
                                        v => channel.get().as_deref() == Some(v),
                                    }
                                }
                                on:click=move |_| {
                                    let v = if value.is_empty() { None } else { Some(value) };
                                    set_param_stored.with_value(|f| f("channel", v));
                                }
                            >
                                {label}
                            </button>
                        }
                    }).collect::<Vec<_>>()}
                </div>
            </div>

            <Show when=move || {
                health.get()
                    .and_then(|h| h.get("status").and_then(|s| s.as_str()).map(str::to_string))
                    .is_some_and(|s| s != "ok")
            } fallback=|| ()>
                <div class=move || {
                    match health.get()
                        .and_then(|h| h.get("status").and_then(|s| s.as_str().map(str::to_string)))
                        .as_deref()
                    {
                        Some("error") => "spp-banner spp-banner--error",
                        _ => "spp-banner spp-banner--warn",
                    }
                }>
                    {move || {
                        let h = health.get().unwrap_or_default();
                        let status = h
                            .get("status")
                            .and_then(|s| s.as_str().map(str::to_string))
                            .unwrap_or_else(|| "degraded".to_string());
                        let hs_connected = h
                            .pointer("/helpscout/connected")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(true);
                        let demo = h
                            .pointer("/helpscout/demo_mode")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false);
                        let lm = h
                            .pointer("/lmstudio/connected")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(true);
                        let qdrant = h
                            .pointer("/qdrant/connected")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(true);
                        view! {
                            <span>
                                "System status: " <strong>{status}</strong>". "
                                {if !hs_connected && !demo {
                                    "Help Scout is unreachable — remote actions disabled, local data remains browsable. "
                                } else {
                                    ""
                                }}
                                {if !lm {
                                    "LM Studio is offline — AI features unavailable (everything else works). "
                                } else {
                                    ""
                                }}
                                {if !qdrant {
                                    "Qdrant is offline — keyword search remains fully functional. "
                                } else {
                                    ""
                                }}
                                <A href="/sync-health" attr:class="spp-dashboard__banner-link">"Open Sync Health"</A>
                            </span>
                        }
                    }}
                </div>
            </Show>

            <Show when=move || metrics_loading.get() fallback=|| ()>
                <LoadingState />
            </Show>
            <Show when=move || metrics_error.get().is_some() fallback=|| ()>
                <div class="spp-state spp-state--error">
                    <span class="spp-state__icon" aria-hidden="true">"⚠"</span>
                    <p class="spp-state__body">{move || metrics_error.get().unwrap_or_default()}</p>
                </div>
            </Show>
            <Show
                when=move || !metrics_loading.get() && metrics_error.get().is_none()
                fallback=|| ()
            >
                {move || {
                    let Some(data) = metrics.get() else {
                        return view! { <div></div> }.into_any();
                    };
                    let active = data.get("active_conversations").and_then(|v| v.as_i64()).unwrap_or(0);
                    let pending = data.get("pending_conversations").and_then(|v| v.as_i64()).unwrap_or(0);
                    let closed = data.get("closed_conversations").and_then(|v| v.as_i64()).unwrap_or(0);
                    let unassigned = data.get("unassigned").and_then(|v| v.as_i64()).unwrap_or(0);
                    let backlog = data.get("backlog").and_then(|v| v.as_i64()).unwrap_or(0);
                    let replies_sent = data.get("replies_sent").and_then(|v| v.as_i64()).unwrap_or(0);
                    let first_response = data.get("first_response_time_avg_min").and_then(|v| v.as_f64());
                    let resolution_time = data.get("resolution_time_avg_min").and_then(|v| v.as_f64());
                    let great = data.pointer("/ratings/great").and_then(|v| v.as_i64()).unwrap_or(0);
                    let okay = data.pointer("/ratings/okay").and_then(|v| v.as_i64()).unwrap_or(0);
                    let not_good = data.pointer("/ratings/not-good").and_then(|v| v.as_i64()).unwrap_or(0);
                    let total_ratings = great + okay + not_good;
                    let great_share = if total_ratings > 0 {
                        format!("{}%", (great * 100) / total_ratings)
                    } else {
                        "—".to_string()
                    };
                    let daily_new = data.get("daily_new").and_then(|v| v.as_array()).cloned().unwrap_or_default();
                    let max_daily = daily_new
                        .iter()
                        .map(|d| d.get("value").and_then(|v| v.as_i64()).unwrap_or(0))
                        .max()
                        .unwrap_or(1)
                        .max(1);
                    view! {
                        <div class="spp-dashboard-grid">
                            <A href="/inbox?view=active" attr:class="spp-kpi-card spp-kpi-card--link">
                                <span class="spp-kpi-card__label">"Active"</span>
                                <span class="spp-kpi-card__value">{fmt_u64(Some(active))}</span>
                            </A>
                            <A href="/inbox?view=pending" attr:class="spp-kpi-card spp-kpi-card--link">
                                <span class="spp-kpi-card__label">"Pending"</span>
                                <span class="spp-kpi-card__value">{fmt_u64(Some(pending))}</span>
                            </A>
                            <A href="/inbox?view=closed" attr:class="spp-kpi-card spp-kpi-card--link">
                                <span class="spp-kpi-card__label">"Closed (in range)"</span>
                                <span class="spp-kpi-card__value">{fmt_u64(Some(closed))}</span>
                            </A>
                            <A href="/inbox?view=unassigned" attr:class="spp-kpi-card spp-kpi-card--link">
                                <span class="spp-kpi-card__label">"Unassigned"</span>
                                <span class="spp-kpi-card__value">{fmt_u64(Some(unassigned))}</span>
                            </A>
                            <div class="spp-kpi-card">
                                <span class="spp-kpi-card__label">"Backlog (7d+)"</span>
                                <span class="spp-kpi-card__value">{fmt_u64(Some(backlog))}</span>
                                <span class="spp-kpi-card__hint">"local definition"</span>
                            </div>
                            <div class="spp-kpi-card">
                                <span class="spp-kpi-card__label">"First response (avg)"</span>
                                <span class="spp-kpi-card__value">{fmt_min(first_response)}</span>
                                <span class="spp-kpi-card__hint">"local definition"</span>
                            </div>
                            <div class="spp-kpi-card">
                                <span class="spp-kpi-card__label">"Resolution (avg)"</span>
                                <span class="spp-kpi-card__value">{fmt_min(resolution_time)}</span>
                                <span class="spp-kpi-card__hint">"local definition"</span>
                            </div>
                            <div class="spp-kpi-card">
                                <span class="spp-kpi-card__label">"Replies sent"</span>
                                <span class="spp-kpi-card__value">{fmt_u64(Some(replies_sent))}</span>
                            </div>
                            <div class="spp-kpi-card">
                                <span class="spp-kpi-card__label">"Great ratings"</span>
                                <span class="spp-kpi-card__value">{great_share}</span>
                                <span class="spp-kpi-card__hint">
                                    {format!("{great} great · {okay} okay · {not_good} not-good · live via /api/events")}
                                </span>
                            </div>
                        </div>

                        <div class="spp-dashboard-grid spp-dashboard-grid--two">
                            <div class="spp-card">
                                <h3 class="spp-card__title">"New conversations per day (local)"</h3>
                                {if daily_new.is_empty() {
                                    view! { <EmptyState message="No conversations in range" /> }.into_any()
                                } else {
                                    view! {
                                        <div class="spp-barchart" role="img" aria-label="New conversations per day">
                                            {daily_new.iter().map(|d| {
                                                let value = d.get("value").and_then(|v| v.as_i64()).unwrap_or(0);
                                                let date = d.get("date").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                                let height = (value.max(0) * 100) / max_daily;
                                                view! {
                                                    <div
                                                        class="spp-barchart__bar"
                                                        style=format!("height: {height}%")
                                                        title=format!("{date}: {value}")
                                                    ></div>
                                                }
                                            }).collect::<Vec<_>>()}
                                        </div>
                                        <div class="spp-flex spp-flex--between spp-dashboard__chart-ends">
                                            <span class="spp-muted spp-text-xs">
                                                {daily_new.first()
                                                    .and_then(|d| d.get("date").and_then(|v| v.as_str()).map(str::to_string))
                                                    .unwrap_or_default()}
                                            </span>
                                            <span class="spp-muted spp-text-xs">
                                                {daily_new.last()
                                                    .and_then(|d| d.get("date").and_then(|v| v.as_str()).map(str::to_string))
                                                    .unwrap_or_default()}
                                            </span>
                                        </div>
                                    }.into_any()
                                }}
                            </div>
                            <div class="spp-card">
                                <h3 class="spp-card__title">"Issue Radar (AI + local)"</h3>
                                <Show
                                    when=move || !radar.with(|r| r.is_empty())
                                    fallback=|| {
                                        view! {
                                            <EmptyState message="No alerts right now. Alerts appear when clusters rise, new issues appear or volume spikes." />
                                        }
                                    }
                                >
                                    {move || radar.get().iter().take(6).map(|a| {
                                        let title = a.get("title").and_then(|v| v.as_str()).unwrap_or("(alert)").to_string();
                                        let detail = a.get("detail").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                        let severity = a.get("severity").and_then(|v| v.as_str()).unwrap_or("info").to_string();
                                        let conv_links: Vec<(String, String)> = a
                                            .get("conversation_ids")
                                            .and_then(|v| v.as_array())
                                            .map(|arr| {
                                                arr.iter()
                                                    .filter_map(|c| c.as_i64())
                                                    .take(6)
                                                    .map(|cid| {
                                                        (
                                                            format!("/inbox/conversation/{cid}"),
                                                            format!("#{cid}"),
                                                        )
                                                    })
                                                    .collect()
                                            })
                                            .unwrap_or_default();
                                        view! {
                                            <div class=match severity.as_str() {
                                                "critical" => "spp-issues__alert spp-issues__alert--critical",
                                                "warning" => "spp-issues__alert spp-issues__alert--warning",
                                                _ => "spp-issues__alert spp-issues__alert--info",
                                            }>
                                                <strong>{title}</strong>
                                                <p class="spp-muted spp-text-xs">{detail}</p>
                                                <div class="spp-issues__alert-convs">
                                                    {conv_links.iter().map(|(href, label)| {
                                                        let href = href.clone();
                                                        let label = label.clone();
                                                        view! {
                                                            <A href=href attr:class="spp-badge">
                                                                {label}
                                                            </A>
                                                        }
                                                    }).collect::<Vec<_>>()}
                                                </div>
                                            </div>
                                        }
                                    }).collect::<Vec<_>>()}
                                </Show>
                                <A href="/issues" attr:class="spp-dashboard__banner-link">"Open Issues screen →"</A>
                            </div>
                        </div>

                        <div class="spp-dashboard-grid spp-dashboard-grid--two">
                            <div class="spp-card">
                                <h3 class="spp-card__title">"Channel mix & speed"</h3>
                                {move || {
                                    let data = metrics.get().unwrap_or_default();
                                    let rows = data.get("channel_metrics").and_then(|v| v.as_array()).cloned().unwrap_or_default();
                                    if rows.is_empty() {
                                        view! { <EmptyState message="No conversations in range" /> }.into_any()
                                    } else {
                                        rows.iter().map(|c| {
                                            let channel = c.get("channel").and_then(|v| v.as_str()).unwrap_or("email").to_string();
                                            let count = c.get("count").and_then(|v| v.as_i64()).unwrap_or(0);
                                            let first = c.get("first_response_avg_min").and_then(|v| v.as_f64());
                                            let resolution = c.get("resolution_avg_min").and_then(|v| v.as_f64());
                                            let label = if channel == "chat" {
                                                "Chat (Beacon)".to_string()
                                            } else {
                                                channel.clone()
                                            };
                                            view! {
                                                <div class="spp-dashboard__channel-row">
                                                    <div class="spp-flex spp-flex--between">
                                                        <A href={format!("/inbox?view=all&channel={}", crate::url_state::encode_value(&channel))} attr:class="spp-badge">
                                                            {label}
                                                        </A>
                                                        <span class="spp-badge">{count.to_string()}</span>
                                                    </div>
                                                    <div class="spp-muted spp-text-xs">
                                                        {format!("first response {} · resolution {}", fmt_min(first), fmt_min(resolution))}
                                                    </div>
                                                </div>
                                            }
                                        }).collect::<Vec<_>>().into_any()
                                    }
                                }}
                                <p class="spp-muted spp-text-xs">
                                    "Chat sessions are Beacon conversations (type=chat, source via=beacon) — see the Docs page for the mirror overview."
                                </p>
                            </div>
                            <div class="spp-card">
                                <h3 class="spp-card__title">"Mailbox comparison"</h3>
                                {move || {
                                        let data = metrics.get().unwrap_or_default();
                                    let rows = data.get("mailbox_comparison").and_then(|v| v.as_array()).cloned().unwrap_or_default();
                                    let max_new = rows.iter()
                                        .map(|m| m.get("new_conversations").and_then(|v| v.as_i64()).unwrap_or(0))
                                        .max()
                                        .unwrap_or(1)
                                        .max(1);
                                    if rows.is_empty() {
                                        view! { <EmptyState message="No mailboxes yet" /> }.into_any()
                                    } else {
                                        rows.iter().map(move |m| {
                                            let mailbox_id = m.get("mailbox_id").and_then(|v| v.as_i64()).unwrap_or_default();
                                            let name = m.get("name").and_then(|v| v.as_str()).unwrap_or("(unnamed)").to_string();
                                            let new_conversations = m.get("new_conversations").and_then(|v| v.as_i64()).unwrap_or(0);
                                            let active = m.get("active_conversations").and_then(|v| v.as_i64()).unwrap_or(0);
                                            let closed = m.get("closed_conversations").and_then(|v| v.as_i64()).unwrap_or(0);
                                            let backlog = m.get("backlog").and_then(|v| v.as_i64()).unwrap_or(0);
                                            let first = m.get("first_response_avg_min").and_then(|v| v.as_f64());
                                            let resolution = m.get("resolution_avg_min").and_then(|v| v.as_f64());
                                            let great_ratings = m.get("great_ratings").and_then(|v| v.as_i64()).unwrap_or(0);
                                            let total_ratings = m.get("total_ratings").and_then(|v| v.as_i64()).unwrap_or(0);
                                            let width = (new_conversations.max(0) * 100) / max_new;
                                            let toggle = toggle_mailbox_stored;
                                            view! {
                                                <div class="spp-dashboard__mailbox-row">
                                                    <div class="spp-flex spp-flex--between">
                                                        <button
                                                            class="spp-button spp-button--ghost spp-button--small"
                                                            on:click=move |_| {
                                                                set_param_stored.with_value(|f| f("mailboxes", Some(mailbox_id.to_string().as_str())));
                                                            }
                                                        >
                                                            {name}
                                                        </button>
                                                        <span class="spp-badge">{format!("{new_conversations} new")}</span>
                                                    </div>
                                                    <div class="spp-dashboard__meter">
                                                        <div class="spp-dashboard__meter-fill" style=format!("width: {width}%")></div>
                                                    </div>
                                                    <div class="spp-muted spp-text-xs">
                                                        {format!(
                                                            "{active} active · {closed} closed · backlog {backlog} · first response {} · resolution {}",
                                                            fmt_min(first),
                                                            fmt_min(resolution)
                                                        )}
                                                        {" · "}
                                                        {if total_ratings > 0 {
                                                            format!("{}% great", (great_ratings * 100) / total_ratings)
                                                        } else {
                                                            "no ratings".to_string()
                                                        }}
                                                    </div>
                                                    <button class="spp-button spp-button--tiny" on:click=move |_| toggle.with_value(|f| f(mailbox_id))>
                                                        {if mailbox_ids.get().contains(&mailbox_id) { "Remove from scope" } else { "Add to scope" }}
                                                    </button>
                                                </div>
                                            }
                                        }).collect::<Vec<_>>().into_any()
                                    }
                                }}
                            </div>
                        </div>

                        <div class="spp-dashboard-grid spp-dashboard-grid--three">
                            <div class="spp-card">
                                <h3 class="spp-card__title">"Tickets by inbox"</h3>
                                <NameCountList data=data.clone() key="by_mailbox" link="inbox" />
                            </div>
                            <div class="spp-card">
                                <h3 class="spp-card__title">"Tickets by tag"</h3>
                                <NameCountList data=data.clone() key="by_tag" link="tag" />
                            </div>
                            <div class="spp-card">
                                <h3 class="spp-card__title">"Tickets by agent"</h3>
                                <NameCountList data=data.clone() key="by_agent" link="none" />
                                <Show when=move || {
                                    metrics.get()
                                        .and_then(|d| d.get("by_team").and_then(|t| t.as_array()).map(|a| !a.is_empty()))
                                        .unwrap_or(false)
                                } fallback=|| ()>
                                    <div class="spp-dashboard__scope-label spp-dashboard__scope-label--gap">"By team"</div>
                                    <NameCountList data=data.clone() key="by_team" link="none" />
                                </Show>
                            </div>
                        </div>

                        <p class="spp-muted spp-text-xs">
                            "All metrics are local calculations from the synchronized mirror; definitions and limitations are listed in Reports → Metric definitions. AI-derived numbers are labeled as AI-derived. Ratings update in real time over Server-Sent Events (/api/events)."
                        </p>
                    }.into_any()
                }}
            </Show>
        </div>
    }
}

#[component]
fn NameCountList(data: serde_json::Value, key: &'static str, link: &'static str) -> impl IntoView {
    let rows: Vec<serde_json::Value> = data
        .get(key)
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let rows = if key == "by_tag" {
        rows.into_iter().take(8).collect::<Vec<_>>()
    } else {
        rows
    };
    view! {
        {if rows.is_empty() {
            view! { <span class="spp-muted spp-text-xs">{if key == "by_tag" { "No tags yet" } else { "Unassigned work only" } }</span> }.into_any()
        } else {
            rows.iter().map(|r| {
                let name = r.get("name").and_then(|v| v.as_str()).unwrap_or("—").to_string();
                let count = r.get("count").and_then(|v| v.as_i64()).unwrap_or(0);
                let href = match link {
                    "inbox" => "/inbox?view=all".to_string(),
                    "tag" => format!("/inbox?view=all&tag={}", crate::url_state::encode_value(&name)),
                    _ => String::new(),
                };
                view! {
                    <div class="spp-flex spp-flex--between spp-dashboard__name-row">
                        {if href.is_empty() {
                            view! { <span>{name}</span> }.into_any()
                        } else {
                            view! { <A href=href attr:class="spp-dashboard__name-link">{name}</A> }.into_any()
                        }}
                        <span class="spp-badge">{count.to_string()}</span>
                    </div>
                }
            }).collect::<Vec<_>>().into_any()
        }}
    }
}

fn fmt_min(m: Option<f64>) -> String {
    let Some(m) = m else {
        return "—".to_string();
    };
    if m < 60.0 {
        format!("{}m", m.round() as i64)
    } else if m < 60.0 * 24.0 {
        format!("{}h", (m / 60.0).round() as i64)
    } else {
        format!("{}d", (m / (60.0 * 24.0)).round() as i64)
    }
}

fn fmt_u64(v: Option<i64>) -> String {
    v.unwrap_or(0).to_string()
}
