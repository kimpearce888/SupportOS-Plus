use leptos::prelude::*;
use leptos::task::*;

use crate::components::state_view::{EmptyState, LoadingState};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct FrictionSample {
    pub conversation_id: i64,
    pub conversation_number: i64,
    pub severity: String,
    pub detail: String,
    pub evidence: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct FrictionKind {
    pub kind: String,
    pub label: String,
    pub conversations: i64,
    pub high_severity: i64,
    pub sample: Vec<FrictionSample>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct AffectedCustomer {
    pub customer_local_id: i64,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub findings: i64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct FrictionOverview {
    pub kinds: Vec<FrictionKind>,
    pub customers_most_affected: Vec<AffectedCustomer>,
    pub notes: Vec<String>,
}

#[must_use]
pub fn parse_friction_overview(v: &serde_json::Value) -> FrictionOverview {
    FrictionOverview {
        kinds: v
            .get("kinds")
            .and_then(|k| k.as_array())
            .map(|rows| {
                rows.iter()
                    .map(|k| FrictionKind {
                        kind: k
                            .get("kind")
                            .and_then(|x| x.as_str())
                            .unwrap_or_default()
                            .to_string(),
                        label: k
                            .get("label")
                            .and_then(|x| x.as_str())
                            .unwrap_or_default()
                            .to_string(),
                        conversations: k.get("conversations").and_then(|x| x.as_i64()).unwrap_or(0),
                        high_severity: k.get("high_severity").and_then(|x| x.as_i64()).unwrap_or(0),
                        sample: k
                            .get("sample")
                            .and_then(|s| s.as_array())
                            .map(|rows| {
                                rows.iter()
                                    .map(|s| FrictionSample {
                                        conversation_id: s
                                            .get("conversation_id")
                                            .and_then(|x| x.as_i64())
                                            .unwrap_or(0),
                                        conversation_number: s
                                            .get("conversation_number")
                                            .and_then(|x| x.as_i64())
                                            .unwrap_or(0),
                                        severity: s
                                            .get("severity")
                                            .and_then(|x| x.as_str())
                                            .unwrap_or_default()
                                            .to_string(),
                                        detail: s
                                            .get("detail")
                                            .and_then(|x| x.as_str())
                                            .unwrap_or_default()
                                            .to_string(),
                                        evidence: s
                                            .get("evidence")
                                            .and_then(|e| e.as_array())
                                            .map(|rows| {
                                                rows.iter()
                                                    .filter_map(|e| {
                                                        e.get("excerpt")
                                                            .and_then(|x| x.as_str())
                                                            .map(str::to_string)
                                                    })
                                                    .collect()
                                            })
                                            .unwrap_or_default(),
                                    })
                                    .collect()
                            })
                            .unwrap_or_default(),
                    })
                    .collect()
            })
            .unwrap_or_default(),
        customers_most_affected: v
            .get("customers_most_affected")
            .and_then(|c| c.as_array())
            .map(|rows| {
                rows.iter()
                    .map(|c| AffectedCustomer {
                        customer_local_id: c
                            .get("customer_local_id")
                            .and_then(|x| x.as_i64())
                            .unwrap_or(0),
                        first_name: c
                            .get("first_name")
                            .and_then(|x| x.as_str())
                            .map(str::to_string),
                        last_name: c
                            .get("last_name")
                            .and_then(|x| x.as_str())
                            .map(str::to_string),
                        findings: c.get("findings").and_then(|x| x.as_i64()).unwrap_or(0),
                    })
                    .collect()
            })
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

#[must_use]
pub fn severity_badge_class(severity: &str) -> &'static str {
    match severity {
        "high" => "spp-badge spp-badge--err",
        "moderate" => "spp-badge spp-badge--warn",
        _ => "spp-badge",
    }
}

#[must_use]
pub fn affected_customer_name(c: &AffectedCustomer) -> String {
    let joined = [c.first_name.clone(), c.last_name.clone()]
        .into_iter()
        .flatten()
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    if joined.is_empty() {
        format!("customer #{}", c.customer_local_id)
    } else {
        joined
    }
}

#[component]
pub fn FrictionTab() -> impl IntoView {
    let overview = RwSignal::new(None::<FrictionOverview>);
    let loading = RwSignal::new(true);
    let error_msg = RwSignal::new(None::<String>);
    let notice = RwSignal::new(None::<(String, String)>);
    let days = RwSignal::new(30i64);
    let rebuilding = RwSignal::new(false);

    let load = {
        move |days: i64| {
            loading.set(true);
            let overview = overview;
            let loading = loading;
            let error_msg = error_msg;
            spawn_local(async move {
                let path = format!("/api/friction/overview?days={days}");
                match crate::api::get_json::<serde_json::Value>(&path).await {
                    Ok(v) => {
                        error_msg.set(None);
                        overview.set(Some(parse_friction_overview(&v)));
                    }
                    Err(e) => error_msg.set(Some(e)),
                }
                loading.set(false);
            });
        }
    };
    load(30);

    let rebuild = move |_| {
        if rebuilding.get_untracked() {
            return;
        }
        rebuilding.set(true);
        let body = serde_json::json!({});
        let rebuilding = rebuilding;
        let load = load;
        let notice = notice;
        let current_days = days.get_untracked();
        spawn_local(async move {
            match crate::api::post_json::<serde_json::Value>("/api/friction/rebuild", Some(&body))
                .await
            {
                Ok(r) => {
                    let findings = r.get("findings").and_then(|x| x.as_i64()).unwrap_or(0);
                    let conversations =
                        r.get("conversations").and_then(|x| x.as_i64()).unwrap_or(0);
                    notice.set(Some((
                        "ok".to_string(),
                        format!(
                            "Friction rebuild complete: {findings} findings across {conversations} conversations."
                        ),
                    )));
                    load(current_days);
                }
                Err(e) => notice.set(Some(("err".to_string(), e))),
            }
            rebuilding.set(false);
        });
    };

    view! {
        <div class="spp-card">
            <div class="spp-flex spp-flex--between spp-mb-8">
                <h3 class="spp-card__title">"Conversation friction (customer effort)"</h3>
                <div class="spp-flex spp-gap-4">
                    {[7i64, 30, 90]
                        .iter()
                        .map(|d| {
                            let d = *d;
                            view! {
                                <button
                                    class="spp-button spp-button--small"
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
                    <button
                        class="spp-button spp-button--small"
                        on:click=rebuild
                        disabled=move || rebuilding.get()
                    >
                        {move || if rebuilding.get() { "Rebuilding…" } else { "Rebuild findings" }.to_string()}
                    </button>
                </div>
            </div>
            <Show when=move || loading.get() fallback=|| ()>
                <LoadingState />
            </Show>
            <Show when=move || error_msg.get().is_some() fallback=|| ()>
                <div class="spp-state spp-state--error">
                    <p class="spp-state__body">"The friction report failed to load."</p>
                    <p class="spp-state__detail">{move || error_msg.get().unwrap_or_default()}</p>
                </div>
            </Show>
            {move || {
                let notice = notice.get();
                if let Some((kind, message)) = notice {
                    let class = if kind == "ok" {
                        "spp-alert spp-alert--ok"
                    } else {
                        "spp-alert spp-alert--error"
                    };
                    view! { <div class=class><div class="spp-text-sm">{message}</div></div> }.into_any()
                } else {
                    ().into_any()
                }
            }}
            {move || {
                let o = overview.clone().get().unwrap_or_default();
                if loading.get() {
                    return ().into_any();
                }
                let all_zero = o.kinds.iter().all(|k| k.conversations == 0);
                view! {
                    {if all_zero {
                        view! {
                            <EmptyState message="No friction findings yet. Findings appear after conversations are analyzed. Press Rebuild to cover existing history." />
                        }.into_any()
                    } else {
                        o.kinds
                            .iter()
                            .map(|k| {
                                view! {
                                    <div class="spp-friction-kind">
                                        <div class="spp-flex spp-flex--between">
                                            <strong>{k.label.clone()}</strong>
                                            <span class="spp-muted spp-text-sm">
                                                {format!(
                                                    "{} conversation{}",
                                                    k.conversations,
                                                    if k.conversations == 1 { "" } else { "s" }
                                                )}
                                                {if k.high_severity > 0 {
                                                    view! {
                                                        <span class="spp-badge spp-badge--warn">
                                            {format!(" {} high", k.high_severity)}
                                                        </span>
                                                    }.into_any()
                                                } else {
                                                    ().into_any()
                                                }}
                                            </span>
                                        </div>
                                        {if !k.sample.is_empty() {
                                            view! {
                                                <table class="spp-table spp-table--compact spp-mt-8">
                                                    <thead>
                                                        <tr>
                                                            <th>"Conversation"</th>
                                                            <th>"Severity"</th>
                                                            <th>"Detail"</th>
                                                        </tr>
                                                    </thead>
                                                    <tbody>
                                                        {k.sample
                                                            .iter()
                                                            .take(5)
                                                            .map(|f| {
                                                                let badge = severity_badge_class(&f.severity);
                                                                view! {
                                                                    <tr>
                                                                        <td>
                                                                            <a
                                                                                class="spp-button spp-button--tiny spp-button--ghost"
                                                                                href=format!("/inbox/conversation/{}", f.conversation_id)
                                                                            >
                                                                                {format!("#{}", f.conversation_number)}
                                                                            </a>
                                                                        </td>
                                                                        <td><span class=badge>{f.severity.clone()}</span></td>
                                                                        <td class="spp-text-sm">
                                                                            <div>{f.detail.clone()}</div>
                                                                            {if !f.evidence.is_empty() {
                                                                                view! {
                                                                                    <div class="spp-muted spp-text-xs spp-mt-4">
                                                                                        "Evidence: "
                                                                                        {f.evidence
                                                                                            .iter()
                                                                                            .take(2)
                                                                                            .map(|e| {
                                                                                                let clipped: String = e.chars().take(80).collect();
                                                                                                view! { <span class="spp-mono">{format!(" {clipped} ")}</span> }
                                                                                            })
                                                                                            .collect::<Vec<_>>()}
                                                                                    </div>
                                                                                }.into_any()
                                                                            } else {
                                                                                ().into_any()
                                                                            }}
                                                                        </td>
                                                                    </tr>
                                                                }
                                                            })
                                                            .collect::<Vec<_>>()}
                                                    </tbody>
                                                </table>
                                            }.into_any()
                                        } else {
                                            ().into_any()
                                        }}
                                    </div>
                                }
                            })
                            .collect::<Vec<_>>()
                            .into_any()
                    }}
                    {if !o.customers_most_affected.is_empty() {
                        view! {
                            <div class="spp-mt-16">
                                <h4 class="spp-card__title">"Customers with most findings"</h4>
                                <div class="spp-flex spp-gap-8 spp-flex--wrap spp-mt-8">
                                    {o.customers_most_affected
                                        .iter()
                                        .map(|c| {
                                            let name = affected_customer_name(c);
                                            view! {
                                                <a
                                                    class="spp-button spp-button--small spp-button--ghost"
                                                    href=format!("/customers/{}", c.customer_local_id)
                                                >
                                                    {name}
                                                    <span class="spp-badge spp-badge--warn spp-ml-4">
                                                        {format!("{} findings", c.findings)}
                                                    </span>
                                                </a>
                                            }
                                        })
                                        .collect::<Vec<_>>()}
                                </div>
                            </div>
                        }.into_any()
                    } else {
                        ().into_any()
                    }}
                    <div class="spp-alert spp-alert--info spp-mt-12">
                        {o.notes
                            .iter()
                            .map(|n| view! { <div class="spp-text-sm">{n.clone()}</div> })
                            .collect::<Vec<_>>()}
                    </div>
                }.into_any()
            }}
        </div>
    }
}
