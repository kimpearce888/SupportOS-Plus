use leptos::prelude::*;
use leptos::task::*;

use crate::components::state_view::{EmptyState, LoadingState};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct EffectivenessBucket {
    pub style_key: String,
    pub style_label: String,
    pub kind: String,
    pub conversations: i64,
    pub follow_up_rate: Option<f64>,
    pub clarification_rate: Option<f64>,
    pub resolved_after_first_rate: Option<f64>,
    pub avg_effort_score: Option<f64>,
    pub high_friction_rate: Option<f64>,
    pub rating_distribution: Vec<(String, i64)>,
    pub sample_conversations: Vec<EffectivenessSample>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct EffectivenessSample {
    pub conversation_local_id: i64,
    pub number: i64,
    pub outcome_summary: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct EffectivenessReport {
    pub total_analyzed: i64,
    pub buckets: Vec<EffectivenessBucket>,
    pub notes: Vec<String>,
}

#[must_use]
pub fn parse_effectiveness(v: &serde_json::Value) -> EffectivenessReport {
    EffectivenessReport {
        total_analyzed: v
            .get("total_analyzed")
            .and_then(|x| x.as_i64())
            .unwrap_or(0),
        buckets: v
            .get("buckets")
            .and_then(|b| b.as_array())
            .map(|rows| rows.iter().map(parse_bucket).collect())
            .unwrap_or_default(),
        notes: v
            .get("notes")
            .and_then(|n| n.as_array())
            .map(|rows| {
                rows.iter()
                    .filter_map(|n| n.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default(),
    }
}

fn parse_bucket(b: &serde_json::Value) -> EffectivenessBucket {
    EffectivenessBucket {
        style_key: b
            .get("style_key")
            .and_then(|x| x.as_str())
            .unwrap_or_default()
            .to_string(),
        style_label: b
            .get("style_label")
            .and_then(|x| x.as_str())
            .unwrap_or_default()
            .to_string(),
        kind: b
            .get("kind")
            .and_then(|x| x.as_str())
            .unwrap_or_default()
            .to_string(),
        conversations: b.get("conversations").and_then(|x| x.as_i64()).unwrap_or(0),
        follow_up_rate: b.get("follow_up_rate").and_then(|x| x.as_f64()),
        clarification_rate: b.get("clarification_rate").and_then(|x| x.as_f64()),
        resolved_after_first_rate: b.get("resolved_after_first_rate").and_then(|x| x.as_f64()),
        avg_effort_score: b.get("avg_effort_score").and_then(|x| x.as_f64()),
        high_friction_rate: b.get("high_friction_rate").and_then(|x| x.as_f64()),
        rating_distribution: b
            .get("rating_distribution")
            .and_then(|r| r.as_array())
            .map(|rows| {
                rows.iter()
                    .filter(|r| r.get("count").and_then(|c| c.as_i64()).unwrap_or(0) > 0)
                    .map(|r| {
                        (
                            r.get("rating")
                                .and_then(|x| x.as_str())
                                .unwrap_or_default()
                                .replace("not-good", "not good"),
                            r.get("count").and_then(|x| x.as_i64()).unwrap_or(0),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default(),
        sample_conversations: b
            .get("sample_conversations")
            .and_then(|s| s.as_array())
            .map(|rows| {
                rows.iter()
                    .map(|s| EffectivenessSample {
                        conversation_local_id: s
                            .get("conversation_local_id")
                            .and_then(|x| x.as_i64())
                            .unwrap_or(0),
                        number: s.get("number").and_then(|x| x.as_i64()).unwrap_or(0),
                        outcome_summary: s
                            .get("outcome_summary")
                            .and_then(|x| x.as_str())
                            .unwrap_or_default()
                            .to_string(),
                    })
                    .collect()
            })
            .unwrap_or_default(),
    }
}

#[must_use]
pub fn fmt_rate(v: Option<f64>) -> String {
    match v {
        None => "—".to_string(),
        Some(x) => format!("{}%", (x * 100.0).round() as i64),
    }
}

#[must_use]
pub fn fmt_score(v: Option<f64>) -> String {
    match v {
        None => "—".to_string(),
        Some(x) => format!("{x:.1}"),
    }
}

#[component]
pub fn EffectivenessTab() -> impl IntoView {
    let report = RwSignal::new(None::<EffectivenessReport>);
    let loading = RwSignal::new(true);
    let error_msg = RwSignal::new(None::<String>);
    let days = RwSignal::new(90i64);

    let load = {
        move |days: i64| {
            loading.set(true);
            let report = report;
            let loading = loading;
            let error_msg = error_msg;
            spawn_local(async move {
                let path = format!("/api/reports/effectiveness?days={days}");
                match crate::api::get_json::<serde_json::Value>(&path).await {
                    Ok(v) => {
                        error_msg.set(None);
                        report.set(Some(parse_effectiveness(&v)));
                    }
                    Err(e) => error_msg.set(Some(e)),
                }
                loading.set(false);
            });
        }
    };
    load(90);

    view! {
        <div class="spp-card">
            <div class="spp-flex spp-flex--between spp-mb-8">
                <h3 class="spp-card__title">"Response style and observed outcomes"</h3>
                <div class="spp-flex spp-gap-4">
                    {[30i64, 90, 365]
                        .iter()
                        .map(|d| {
                            let d = *d;
                            let class = if days.get_untracked() == d {
                                "spp-button spp-button--small spp-button--primary"
                            } else {
                                "spp-button spp-button--small"
                            };
                            view! {
                                <button
                                    class=class
                                    class:is-active=move || days.get() == d
                                    on:click=move |_| {
                                        days.set(d);
                                        load(d);
                                    }
                                >
                                    {format!("{d} days")}
                                </button>
                            }
                        })
                        .collect::<Vec<_>>()}
                </div>
            </div>
            <Show when=move || loading.get() fallback=|| ()>
                <LoadingState />
            </Show>
            <Show when=move || error_msg.get().is_some() fallback=|| ()>
                <div class="spp-state spp-state--error">
                    <p class="spp-state__body">"The effectiveness report failed to load."</p>
                    <p class="spp-state__detail">{move || error_msg.get().unwrap_or_default()}</p>
                </div>
            </Show>
            {move || {
                let r = report.clone().get().unwrap_or_default();
                if loading.get() {
                    return ().into_any();
                }
                let first_note = r.notes.first().cloned().unwrap_or_default();
                view! {
                    <p class="spp-muted spp-text-sm spp-mb-12">
                        {format!("{} analyzed conversations. {}", r.total_analyzed, first_note)}
                    </p>
                    {if r.buckets.is_empty() {
                        view! {
                            <EmptyState message="No analyzed conversations yet. The interaction engine analyzes conversations as they sync; style outcome data appears here once outcomes exist." />
                        }.into_any()
                    } else {
                        view! {
                            <table class="spp-table">
                                <thead>
                                    <tr>
                                        <th>"Style / characteristic"</th>
                                        <th>"n"</th>
                                        <th>"Follow-up rate"</th>
                                        <th>"Clarification rate"</th>
                                        <th>"Resolved after 1st"</th>
                                        <th>"Avg effort"</th>
                                        <th>"High friction"</th>
                                        <th>"Ratings"</th>
                                        <th>"Samples"</th>
                                    </tr>
                                </thead>
                                <tbody>
                                    {r.buckets
                                        .iter()
                                        .map(|b| {
                                            let kind_note = if b.kind == "characteristic" {
                                                "characteristic (not mutually exclusive)".to_string()
                                            } else {
                                                "response style".to_string()
                                            };
                                            let ratings = if b.rating_distribution.is_empty() {
                                                "—".to_string()
                                            } else {
                                                b.rating_distribution
                                                    .iter()
                                                    .map(|(r, c)| format!("{r}: {c}"))
                                                    .collect::<Vec<_>>()
                                                    .join(" · ")
                                            };
                                            view! {
                                                <tr>
                                                    <td>
                                                        <div><strong>{b.style_label.clone()}</strong></div>
                                                        <div class="spp-muted spp-text-xs">{kind_note}</div>
                                                    </td>
                                                    <td class="spp-mono">{b.conversations.to_string()}</td>
                                                    <td class="spp-mono">{fmt_rate(b.follow_up_rate)}</td>
                                                    <td class="spp-mono">{fmt_rate(b.clarification_rate)}</td>
                                                    <td class="spp-mono">{fmt_rate(b.resolved_after_first_rate)}</td>
                                                    <td class="spp-mono">{fmt_score(b.avg_effort_score)}</td>
                                                    <td class="spp-mono">{fmt_rate(b.high_friction_rate)}</td>
                                                    <td class="spp-text-xs">{ratings}</td>
                                                    <td>
                                                        <div class="spp-flex spp-gap-4 spp-flex--wrap">
                                                            {b.sample_conversations
                                                                .iter()
                                                                .map(|s| {
                                                                    let title = s.outcome_summary.clone();
                                                                    view! {
                                                                        <a
                                                                            class="spp-button spp-button--tiny spp-button--ghost"
                                                                            href=format!("/inbox/conversation/{}", s.conversation_local_id)
                                                                            title=title
                                                                        >
                                                                            {format!("#{}", s.number)}
                                                                        </a>
                                                                    }
                                                                })
                                                                .collect::<Vec<_>>()}
                                                        </div>
                                                    </td>
                                                </tr>
                                            }
                                        })
                                        .collect::<Vec<_>>()}
                                </tbody>
                            </table>
                        }.into_any()
                    }}
                    <div class="spp-alert spp-alert--info spp-mt-12">
                        {r.notes
                            .iter()
                            .map(|n| view! { <div class="spp-text-sm">{n.clone()}</div> })
                            .collect::<Vec<_>>()}
                    </div>
                }.into_any()
            }}
        </div>
    }
}
