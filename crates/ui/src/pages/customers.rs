use leptos::prelude::*;
use leptos_router::components::A;
use leptos_router::hooks::use_navigate;
use std::sync::Arc;

use crate::components::state_view::{EmptyState, LoadingState};

fn urlencode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

fn safe_external_href(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        Some(trimmed.to_string())
    } else {
        None
    }
}

fn date_part(iso: Option<&str>) -> String {
    iso.unwrap_or("").get(..10).unwrap_or("").to_string()
}

#[component]
pub fn CustomerSearchPage() -> impl IntoView {
    let query = RwSignal::new(String::new());
    let page = RwSignal::new(1i64);
    let rows = RwSignal::new(Vec::<serde_json::Value>::new());
    let total = RwSignal::new(0i64);
    let loading = RwSignal::new(true);
    let error_msg = RwSignal::new(None::<String>);
    let navigate = StoredValue::new_local(use_navigate());

    Effect::new(move |_| {
        let q = query.get();
        let page = page.get();
        let rows = rows;
        let total = total;
        let loading = loading;
        let error_msg = error_msg;
        loading.set(true);
        wasm_bindgen_futures::spawn_local(async move {
            let mut path = String::from("/api/customers?pageSize=50");
            if !q.trim().is_empty() {
                path.push_str(&format!("&q={}", urlencode(q.trim())));
            }
            path.push_str(&format!("&page={page}"));
            match crate::api::get_json::<serde_json::Value>(&path).await {
                Ok(v) => {
                    rows.set(
                        v.get("customers")
                            .and_then(|c| c.as_array())
                            .cloned()
                            .unwrap_or_default(),
                    );
                    total.set(v.get("total").and_then(|t| t.as_i64()).unwrap_or(0));
                    error_msg.set(None);
                }
                Err(e) => error_msg.set(Some(e)),
            }
            loading.set(false);
        });
    });

    view! {
        <div class="spp-page spp-page--customer-search">
            <div class="spp-flex spp-flex--between">
                <div>
                    <h2 class="spp-page__title">"Customers"</h2>
                    <p class="spp-page__intro">
                        {move || {
                            let t = total.get();
                            if loading.get() && t == 0 { "… customers in the local mirror".to_string() }
                            else { format!("{t} customers in the local mirror") }
                        }}
                    </p>
                </div>
                <form
                    class="spp-customer-search__bar"
                    on:submit=move |ev| {
                        ev.prevent_default();
                        page.set(1);
                    }
                >
                    <input
                        class="spp-customer-search__input"
                        type="text"
                        placeholder="Search name or email…"
                        aria-label="Search customers"
                        prop:value=query
                        on:input=move |ev| {
                            query.set(event_target_value(&ev));
                            page.set(1);
                        }
                    />
                    <button class="spp-button" type="submit">"Search"</button>
                </form>
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
                <Show
                    when=move || !rows.with(|r| r.is_empty())
                    fallback=|| {
                        view! {
                            <EmptyState message="No customers found." />
                        }
                    }
                >
                    <div class="spp-card">
                        <table class="spp-table">
                            <thead>
                                <tr>
                                    <th>"Customer"</th>
                                    <th>"Email"</th>
                                    <th>"Organization"</th>
                                    <th>"Conversations"</th>
                                    <th>"Open"</th>
                                    <th>"Last activity"</th>
                                </tr>
                            </thead>
                            <tbody>
                                {move || rows.get().iter().map(|c| {
                                    let id = c.get("id").and_then(|v| v.as_i64()).unwrap_or_default();
                                    let first = c.get("first_name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                    let last = c.get("last_name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                    let job_title = c.get("job_title").and_then(|v| v.as_str()).map(str::to_string);
                                    let email = c
                                        .get("emails")
                                        .and_then(|e| e.as_array())
                                        .and_then(|a| a.first().and_then(|e| e.as_str()).map(str::to_string))
                                        .unwrap_or_else(|| "—".to_string());
                                    let org = c
                                        .get("organization_name")
                                        .and_then(|v| v.as_str())
                                        .filter(|o| !o.is_empty())
                                        .unwrap_or("—")
                                        .to_string();
                                    let conv_count = c.get("conversation_count").and_then(|v| v.as_i64()).unwrap_or(0);
                                    let open_count = c.get("open_conversation_count").and_then(|v| v.as_i64()).unwrap_or(0);
                                    let last_activity = date_part(
                                        c.get("last_activity_at").and_then(|v| v.as_str()),
                                    );
                                    let last_activity = if last_activity.is_empty() {
                                        "—".to_string()
                                    } else {
                                        last_activity
                                    };
                                    let full_name = format!("{first} {last}");
                                    view! {
                                        <tr
                                            class="spp-table__row-clickable"
                                            on:click=move |_| navigate.with_value(|n| n(&format!("/customers/{id}"), Default::default()))
                                        >
                                            <td>
                                                <A href={format!("/customers/{id}")} attr:class="spp-customer-search__name-link">
                                                    {full_name}
                                                </A>
                                                {if let Some(title) = job_title {
                                                    view! { <div class="spp-muted spp-text-xs">{title}</div> }.into_any()
                                                } else {
                                                    ().into_any()
                                                }}
                                            </td>
                                            <td class="spp-table__cell-muted">{email}</td>
                                            <td class="spp-table__cell-muted">{org}</td>
                                            <td><span class="spp-badge">{conv_count.to_string()}</span></td>
                                            <td>
                                                {if open_count > 0 {
                                                    view! { <span class="spp-badge spp-badge--ok">{open_count.to_string()}</span> }.into_any()
                                                } else {
                                                    view! { <span class="spp-muted">"0"</span> }.into_any()
                                                }}
                                            </td>
                                            <td class="spp-table__cell-muted">{last_activity}</td>
                                        </tr>
                                    }
                                }).collect::<Vec<_>>()}
                            </tbody>
                        </table>
                    </div>

                    <Show when=move || total.get().gt(&50) fallback=|| ()>
                        <div class="spp-flex spp-flex--center spp-customer-search__pager">
                            <button
                                class="spp-button spp-button--small"
                                disabled=move || page.get() <= 1
                                on:click=move |_| page.update(|p| *p = (*p - 1).max(1))
                            >
                                "Previous"
                            </button>
                            <span class="spp-muted spp-text-xs">{move || format!("page {}", page.get())}</span>
                            <button
                                class="spp-button spp-button--small"
                                disabled=move || rows.with(|r| r.len() < 50)
                                on:click=move |_| page.update(|p| *p += 1)
                            >
                                "Next"
                            </button>
                        </div>
                    </Show>
                </Show>
            </Show>
        </div>
    }
}

#[component]
pub fn CustomerProfilePage(customer_id: i64) -> impl IntoView {
    let detail = RwSignal::new(None::<serde_json::Value>);
    let support_health = RwSignal::new(None::<serde_json::Value>);
    let interaction_profile = RwSignal::new(None::<serde_json::Value>);
    let timeline = RwSignal::new(Vec::<serde_json::Value>::new());
    let loading = RwSignal::new(true);
    let error_msg = RwSignal::new(None::<String>);
    let reload = RwSignal::new(0u32);
    let editing = RwSignal::new(false);

    Effect::new(move |_| {
        let _ = reload.get();
        let _ = crate::queries::version("customer").get();
        let _ = crate::queries::version("interaction-profile").get();
        let detail = detail;
        let support_health = support_health;
        let interaction_profile = interaction_profile;
        let timeline = timeline;
        let loading = loading;
        let error_msg = error_msg;
        loading.set(true);
        wasm_bindgen_futures::spawn_local(async move {
            let detail_path = format!("/api/customers/{customer_id}");
            let detail_result = crate::api::get_json::<serde_json::Value>(&detail_path).await;
            let health_result =
                crate::api::get_json::<serde_json::Value>(&format!("{detail_path}/support-health"))
                    .await;
            let profile_result = crate::api::get_json::<serde_json::Value>(&format!(
                "/api/interaction/profile/{customer_id}"
            ))
            .await;
            let tl_result =
                crate::api::get_json::<serde_json::Value>(&format!("{detail_path}/timeline")).await;
            match detail_result {
                Ok(d) => {
                    detail.set(Some(d));
                    error_msg.set(None);
                    if let Ok(h) = health_result {
                        support_health.set(h.get("report").cloned());
                    }
                    if let Ok(p) = profile_result {
                        interaction_profile.set(p.get("profile").cloned());
                    }
                    if let Ok(tl) = tl_result {
                        timeline.set(
                            tl.get("events")
                                .and_then(|v| v.as_array())
                                .cloned()
                                .unwrap_or_default(),
                        );
                    }
                }
                Err(e) => error_msg.set(Some(e)),
            }
            loading.set(false);
        });
    });

    view! {
        <div class="spp-page spp-page--customer">
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
                <Show
                    when=move || detail.get().is_some()
                    fallback=|| {
                        view! {
                            <EmptyState message="Customer not found. They may have been deleted or never synced." />
                        }
                    }
                >
                    {move || {
                        let d = match detail.get() {
                            Some(d) => d,
                            None => return view! { <div></div> }.into_any(),
                        };
                        let c = d.get("customer").cloned().unwrap_or_default();
                        let first_name = c.get("first_name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        let last_name = c.get("last_name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        let full_name = format!("{first_name} {last_name}");
                        let conv_count = c.get("conversation_count").and_then(|v| v.as_i64()).unwrap_or(0);
                        let open_count = c.get("open_conversation_count").and_then(|v| v.as_i64()).unwrap_or(0);
                        let org_name = c.get("organization_name").and_then(|v| v.as_str()).map(str::to_string);
                        let average_rating = c.get("average_rating").and_then(|v| v.as_f64());
                        let job_title = c.get("job_title").and_then(|v| v.as_str()).map(str::to_string);
                        let edit_open = move || editing.set(true);

                        view! {
                            <div class="spp-flex spp-flex--between">
                                <div>
                                    <h2 class="spp-page__title">{full_name}</h2>
                                    <p class="spp-page__intro">
                                        {format!("{conv_count} conversations · {open_count} open")}
                                        {if let Some(org) = org_name {
                                            format!(" · {org}")
                                        } else {
                                            String::new()
                                        }}
                                        {if let Some(rating) = average_rating {
                                            format!(" · avg rating {rating:.1}")
                                        } else {
                                            String::new()
                                        }}
                                        {if let Some(title) = job_title {
                                            format!(" — {title}")
                                        } else {
                                            String::new()
                                        }}
                                    </p>
                                </div>
                                <div class="spp-flex">
                                    <button
                                        class="spp-button spp-button--ghost spp-button--small"
                                        on:click=move |_| edit_open()
                                    >
                                        "Edit profile"
                                    </button>
                                    <A href="/customers" attr:class="spp-button">"← All customers"</A>
                                </div>
                            </div>

                            <div class="spp-dashboard-grid spp-dashboard-grid--two">
                                <ContactCard detail=d.clone() />
                                <MemoryCard memories=d.get("memories").and_then(|v| v.as_array()).cloned() />
                            </div>

                            <InteractionProfileSection profile=interaction_profile.get() />

                            <SupportHealthSection report=support_health.get() />

                            <TimelineSection timeline=timeline.get() />

                            <ConversationsCard conversations=d.get("conversations").and_then(|v| v.as_array()).cloned() />

                            <div class="spp-dashboard-grid spp-dashboard-grid--two">
                                <ResolutionsCard resolutions=d.get("resolutions").and_then(|v| v.as_array()).cloned() />
                                <RatingsCard ratings=d.get("ratings").and_then(|v| v.as_array()).cloned() />
                            </div>
                        }.into_any()
                    }}

                    <Show when=move || editing.get() fallback=|| ()>
                        {move || {
                            let customer = detail
                                .get()
                                .and_then(|d| d.get("customer").cloned());
                            view! {
                                <EditCustomerModal
                                    customer_id=customer_id
                                    customer=customer
                                    on_close=Arc::new(move || editing.set(false))
                                    on_saved=Arc::new(move || {
                                        editing.set(false);
                                        reload.set(reload.get_untracked() + 1);
                                    })
                                />
                            }
                        }}
                    </Show>
                </Show>
            </Show>
        </div>
    }
}

#[component]
fn ContactCard(detail: serde_json::Value) -> impl IntoView {
    let c = detail.get("customer").cloned().unwrap_or_default();
    let emails: Vec<String> = c
        .get("emails")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|e| e.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    let phones: Vec<String> = c
        .get("phones")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|e| e.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    let address = detail
        .get("address")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let websites: Vec<String> = detail
        .get("websites")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|w| w.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    let socials: Vec<(String, String)> = detail
        .get("social_profiles")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .map(|s| {
                    (
                        s.get("type")
                            .and_then(|t| t.as_str())
                            .unwrap_or("Social")
                            .to_string(),
                        s.get("value")
                            .and_then(|t| t.as_str())
                            .unwrap_or("—")
                            .to_string(),
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    let properties: Vec<(String, String)> = detail
        .get("properties")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .map(|p| {
                    (
                        p.get("name")
                            .and_then(|n| n.as_str())
                            .unwrap_or("property")
                            .to_string(),
                        p.get("value")
                            .and_then(|v| v.as_str())
                            .unwrap_or("—")
                            .to_string(),
                    )
                })
                .collect()
        })
        .unwrap_or_default();

    view! {
        <div class="spp-card">
            <h3 class="spp-card__title">"Contact"</h3>
            <dl class="spp-kv">
                {emails.iter().map(|e| {
                    view! {
                        <dt>"Email"</dt>
                        <dd><a class="spp-dashboard__banner-link" href={format!("mailto:{e}")}>{e.clone()}</a></dd>
                    }
                }).collect::<Vec<_>>()}
                {phones.iter().map(|p| {
                    view! { <dt>"Phone"</dt> <dd>{p.clone()}</dd> }
                }).collect::<Vec<_>>()}
                {if let Some(address) = address {
                    view! { <dt>"Address"</dt> <dd>{address}</dd> }.into_any()
                } else {
                    ().into_any()
                }}
                {websites.iter().map(|w| {
                    let href = safe_external_href(w);
                    view! {
                        <dt>"Website"</dt>
                        <dd>
                            {if let Some(href) = href {
                                view! {
                                    <a class="spp-dashboard__banner-link" href=href target="_blank" rel="noopener noreferrer">{w.clone()}</a>
                                }.into_any()
                            } else {
                                view! { <span>{w.clone()}</span> }.into_any()
                            }}
                        </dd>
                    }
                }).collect::<Vec<_>>()}
                {socials.iter().map(|(kind, value)| {
                    view! { <dt>{kind.clone()}</dt> <dd>{value.clone()}</dd> }
                }).collect::<Vec<_>>()}
                {properties.iter().map(|(name, value)| {
                    view! { <dt>{name.clone()}</dt> <dd>{value.clone()}</dd> }
                }).collect::<Vec<_>>()}
            </dl>
        </div>
    }
}

#[component]
fn MemoryCard(memories: Option<Vec<serde_json::Value>>) -> impl IntoView {
    let memories = memories.unwrap_or_default();
    let body = if memories.is_empty() {
        view! {
                <EmptyState message="No memories yet. Memories are extracted by the local AI when analyzing this customer's tickets." />
            }.into_any()
    } else {
        view! {
                {memories.iter().map(|m| {
                    let key = m.get("key").and_then(|v| v.as_str()).unwrap_or("(memory)").to_string();
                    let value = m.get("value").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    let source = m.get("source").and_then(|v| v.as_str()).unwrap_or("ai").to_string();
                    let confidence = m.get("confidence").and_then(|v| v.as_str()).map(str::to_string);
                    let last_seen = date_part(m.get("last_seen_at").and_then(|v| v.as_str()));
                    view! {
                        <div class="spp-customer__memory">
                            <div class="spp-flex spp-flex--between">
                                <strong class="spp-text-sm">{key}</strong>
                                <span class=if source == "ai" { "spp-badge spp-badge--info" } else { "spp-badge spp-badge--ok" }>
                                    {if source == "ai" {
                                        match confidence.as_deref() {
                                            Some(c) => format!("AI-derived · confidence {c}"),
                                            None => "AI-derived".to_string(),
                                        }
                                    } else {
                                        "human-entered".to_string()
                                    }}
                                </span>
                            </div>
                            <div class="spp-text-sm">{value}</div>
                            {if !last_seen.is_empty() {
                                view! { <div class="spp-muted spp-text-xs">{format!("last seen {last_seen}")}</div> }.into_any()
                            } else {
                                ().into_any()
                            }}
                        </div>
                    }
                }).collect::<Vec<_>>()}
            }.into_any()
    };
    view! {
        <div class="spp-card">
            <h3 class="spp-card__title">"AI customer memory"</h3>
            <p class="spp-muted spp-text-xs">
                "AI-derived entries are clearly marked; they are not Help Scout data."
            </p>
            {body}
        </div>
    }
}

type MonthRow = (String, i64, String, Vec<(String, String)>);

#[component]
fn InteractionProfileSection(profile: Option<serde_json::Value>) -> impl IntoView {
    let Some(p) = profile else {
        return view! { <div class="spp-customer__hidden"></div> }.into_any();
    };
    let client_kind = p
        .get("client_kind")
        .and_then(|v| v.as_str())
        .unwrap_or("first_time")
        .to_string();
    let kind_label = if client_kind == "returning" {
        "returning client"
    } else {
        "first-time client"
    };
    let baseline = p.get("baseline").cloned();
    let preferences: Vec<(String, String)> = p
        .get("preferences")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .map(|pref| {
                    (
                        pref.get("preference")
                            .and_then(|x| x.as_str())
                            .unwrap_or("")
                            .replace('_', " "),
                        pref.get("origin")
                            .and_then(|x| x.as_str())
                            .unwrap_or("")
                            .replace('_', " "),
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    let months: Vec<MonthRow> = p
        .get("timeline")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .map(|t| {
                    (
                        t.get("month")
                            .and_then(|x| x.as_str())
                            .unwrap_or("")
                            .to_string(),
                        t.get("conversation_count")
                            .and_then(|x| x.as_i64())
                            .unwrap_or(0),
                        t.get("summary")
                            .and_then(|x| x.as_str())
                            .unwrap_or("")
                            .to_string(),
                        t.get("conversation_local_ids")
                            .and_then(|x| x.as_array())
                            .map(|ids| {
                                ids.iter()
                                    .filter_map(|i| i.as_i64())
                                    .take(5)
                                    .map(|cid| {
                                        (format!("/inbox/conversation/{cid}"), format!("#{cid}"))
                                    })
                                    .collect::<Vec<_>>()
                            })
                            .unwrap_or_default(),
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    let outcomes = p.get("outcomes").cloned();
    let pct_rate = |key: &str| -> Option<i64> {
        outcomes
            .as_ref()
            .and_then(|o| o.get(key).and_then(|v| v.as_f64()))
            .map(|r| (r * 100.0).round() as i64)
    };
    let has_outcomes = outcomes.is_some();
    let first_response_pct = pct_rate("first_response_resolution_rate");
    let follow_up_pct = pct_rate("follow_up_rate");
    let clarification_pct = pct_rate("clarification_rate");
    let escalation_pct = pct_rate("escalation_rate");

    view! {
        <div class="spp-card">
            <h3 class="spp-card__title">"Client Interaction Profile"</h3>
            <p class="spp-muted spp-text-xs">
                {format!("Observable support-communication behavior — {kind_label}. Never a psychological assessment.")}
            </p>

            {if let Some(baseline) = baseline {
                let obs = baseline.get("observation_count").and_then(|v| v.as_i64()).unwrap_or(0);
                let convs = baseline.get("conversation_count").and_then(|v| v.as_i64()).unwrap_or(0);
                let dims: Vec<(String, String)> = baseline
                    .get("dimensions")
                    .and_then(|v| v.as_array())
                    .map(|a| {
                        a.iter()
                            .map(|d| {
                                (
                                    d.get("dimension").and_then(|x| x.as_str()).unwrap_or("").replace('_', " "),
                                    d.get("typical_value").and_then(|x| x.as_str()).unwrap_or("").replace('_', " "),
                                )
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                view! {
                    <div class="spp-customer__subsection">
                        <h4 class="spp-text-sm">"Historical interaction pattern"</h4>
                        <div class="spp-flex spp-flex--wrap">
                            {dims.iter().map(|(dim, typical)| {
                                view! { <span class="spp-badge">{format!("{dim}: usually {typical}")}</span> }
                            }).collect::<Vec<_>>()}
                        </div>
                        <div class="spp-muted spp-text-xs">
                            {format!("{obs} observations across {convs} conversations")}
                        </div>
                    </div>
                }.into_any()
            } else {
                ().into_any()
            }}

            <div class="spp-dashboard-grid spp-dashboard-grid--two">
                <div>
                    <h4 class="spp-text-sm">"Observed communication preferences"</h4>
                    {if preferences.is_empty() {
                        view! {
                            <span class="spp-muted spp-text-xs">
                                "No preferences yet — repeated evidence across 3+ interactions is required before a preference is inferred."
                            </span>
                        }.into_any()
                    } else {
                        preferences.iter().map(|(pref, origin)| {
                            view! {
                                <div class="spp-flex spp-flex--between spp-customer__pref">
                                    <span class="spp-text-sm">{pref.clone()}</span>
                                    <span class="spp-badge">{origin.clone()}</span>
                                </div>
                            }.into_any()
                        }).collect::<Vec<_>>().into_any()
                    }}
                </div>
                <div>
                    <h4 class="spp-text-sm">"Historical timeline"</h4>
                    {if months.is_empty() {
                        view! { <span class="spp-muted spp-text-xs">"No history yet."</span> }.into_any()
                    } else {
                        months.iter().map(|(month, count, summary, ids)| {
                            view! {
                                <div class="spp-customer__month">
                                    <div class="spp-flex spp-flex--between">
                                        <strong class="spp-text-sm">{month.clone()}</strong>
                                        <span class="spp-badge">{format!("{count} {}", if *count == 1 { "ticket" } else { "tickets" })}</span>
                                    </div>
                                    <div class="spp-muted spp-text-xs">{summary.clone()}</div>
                                    <div class="spp-issues__alert-convs">
                                        {ids.iter().map(|(href, label)| {
                                            let href = href.clone();
                                            let label = label.clone();
                                            view! {
                                                <A href=href attr:class="spp-badge">{label}</A>
                                            }
                                        }).collect::<Vec<_>>()}
                                    </div>
                                </div>
                            }
                        }).collect::<Vec<_>>().into_any()
                    }}
                </div>
            </div>

            {if has_outcomes {
                view! {
                    <div class="spp-customer__subsection">
                        <h4 class="spp-text-sm">"Previous support outcomes"</h4>
                        <div class="spp-flex spp-flex--wrap">
                            {if let Some(r) = first_response_pct {
                                view! { <span class="spp-badge spp-badge--ok">{format!("first-response resolution: {r}%")}</span> }.into_any()
                            } else {
                                ().into_any()
                            }}
                            {if let Some(r) = follow_up_pct {
                                view! { <span class="spp-badge">{format!("follow-up rate: {r}%")}</span> }.into_any()
                            } else {
                                ().into_any()
                            }}
                            {if let Some(r) = clarification_pct {
                                view! { <span class="spp-badge">{format!("clarification rate: {r}%")}</span> }.into_any()
                            } else {
                                ().into_any()
                            }}
                            {if let Some(r) = escalation_pct {
                                view! { <span class="spp-badge">{format!("escalation rate: {r}%")}</span> }.into_any()
                            } else {
                                ().into_any()
                            }}
                        </div>
                    </div>
                }.into_any()
            } else {
                ().into_any()
            }}
        </div>
    }.into_any()
}

#[component]
fn SupportHealthSection(report: Option<serde_json::Value>) -> impl IntoView {
    let Some(r) = report else {
        return view! { <div class="spp-customer__hidden"></div> }.into_any();
    };
    let flags: Vec<serde_json::Value> = r
        .get("flags")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let incidents: Vec<serde_json::Value> = r
        .get("incident_exposure")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let subject_label = r
        .get("subject_label")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    view! {
        <div class="spp-card">
            <div class="spp-flex spp-flex--between spp-flex--wrap">
                <div>
                    <h3 class="spp-card__title">"Support health"</h3>
                    <p class="spp-muted spp-text-xs">
                        "Operational facts with evidence only — no psychological or personal judgments, and deliberately no single \"score\"."
                    </p>
                </div>
                <span class="spp-muted spp-text-xs">{subject_label}</span>
            </div>

            {if flags.is_empty() {
                view! { <span class="spp-muted spp-text-xs">"No attention flags."</span> }.into_any()
            } else {
                view! {
                    <div class="spp-customer__subsection">
                        {flags.iter().map(|f| {
                            let label = f.get("label").and_then(|v| v.as_str()).unwrap_or("(flag)").to_string();
                            let detail = f.get("detail").and_then(|v| v.as_str()).unwrap_or("").to_string();
                            let severity = f.get("severity").and_then(|v| v.as_str()).unwrap_or("info").to_string();
                            let evidence: Vec<(String, String)> = f
                                .get("evidence_conversation_ids")
                                .and_then(|v| v.as_array())
                                .map(|a| {
                                    a.iter()
                                        .filter_map(|i| i.as_i64())
                                        .map(|cid| (
                                            format!("/inbox/conversation/{cid}"),
                                            format!("#{cid}"),
                                        ))
                                        .collect::<Vec<_>>()
                                })
                                .unwrap_or_default();
                            view! {
                                <div class=match severity.as_str() {
                                    "critical" => "spp-issues__alert spp-issues__alert--critical",
                                    "warning" => "spp-issues__alert spp-issues__alert--warning",
                                    _ => "spp-issues__alert spp-issues__alert--info",
                                }>
                                    <div class="spp-flex spp-flex--between">
                                        <strong class="spp-text-sm">{label.clone()}</strong>
                                        <span class="spp-badge">{severity.clone()}</span>
                                    </div>
                                    <p class="spp-text-sm">{detail}</p>
                                    <div class="spp-issues__alert-convs">
                                        {evidence.iter().map(|(href, label)| {
                                            let href = href.clone();
                                            let label = label.clone();
                                            view! {
                                                <A href=href attr:class="spp-badge">{label}</A>
                                            }
                                        }).collect::<Vec<_>>()}
                                    </div>
                                </div>
                            }
                        }).collect::<Vec<_>>()}
                    </div>
                }.into_any()
            }}

            {if incidents.is_empty() {
                ().into_any()
            } else {
                view! {
                    <div class="spp-customer__subsection">
                        <h4 class="spp-text-sm">"Current incident exposure"</h4>
                        {incidents.iter().map(|i| {
                            let id = i.get("incident_id").and_then(|v| v.as_i64()).unwrap_or_default();
                            let code = i.get("code").and_then(|v| v.as_str()).unwrap_or("").to_string();
                            let title = i.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string();
                            let severity = i.get("severity").and_then(|v| v.as_str()).unwrap_or("").to_string();
                            let status = i.get("status").and_then(|v| v.as_str()).unwrap_or("").replace('_', " ");
                            let convs = i.get("conversations").and_then(|v| v.as_i64()).unwrap_or(0);
                            view! {
                                <div class="spp-flex spp-flex--between spp-customer__incident">
                                    <A href={format!("/incidents/{id}")} attr:class="spp-text-sm">
                                        {code} " " {title}
                                    </A>
                                    <span class="spp-muted spp-text-xs">
                                        {format!("{} · {} · {} conversation(s)", severity.to_uppercase(), status, convs)}
                                    </span>
                                </div>
                            }
                        }).collect::<Vec<_>>()}
                    </div>
                }.into_any()
            }}
        </div>
    }.into_any()
}

#[component]
fn TimelineSection(timeline: Vec<serde_json::Value>) -> impl IntoView {
    let body = if timeline.is_empty() {
        view! {
            <EmptyState message="No timeline events yet. Send a reply or note to start the history." />
        }.into_any()
    } else {
        view! {
            <div class="spp-customer-profile__timeline">
                    {timeline.iter().map(|e| {
                        let kind = e.get("event_kind").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        let title = e.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        let time = e.get("occurred_at").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        view! {
                            <div class="spp-timeline-entry">
                                <div class="spp-timeline-entry__header">
                                    <span class="spp-timeline-entry__type">{kind}</span>
                                    <span class="spp-timeline-entry__time">{time}</span>
                                </div>
                                <div class="spp-timeline-entry__body">{title}</div>
                            </div>
                        }
                    }).collect::<Vec<_>>()}
            </div>
        }.into_any()
    };
    view! {
        <div class="spp-card">
            <h3 class="spp-card__title">"Timeline"</h3>
            {body}
        </div>
    }
}

#[component]
fn ConversationsCard(conversations: Option<Vec<serde_json::Value>>) -> impl IntoView {
    let rows = conversations.unwrap_or_default();
    let navigate = StoredValue::new_local(use_navigate());
    let body = if rows.is_empty() {
        view! { <EmptyState message="No conversations for this customer yet." /> }.into_any()
    } else {
        view! {
            <table class="spp-table">
                    <thead>
                        <tr>
                            <th>"#"</th>
                            <th>"Subject"</th>
                            <th>"Status"</th>
                            <th>"Assignee"</th>
                            <th>"Created"</th>
                            <th>"Closed"</th>
                        </tr>
                    </thead>
                    <tbody>
                        {rows.iter().map(|conv| {
                            let id = conv.get("id").and_then(|v| v.as_i64()).unwrap_or_default();
                            let number = conv.get("number").and_then(|v| v.as_i64()).unwrap_or(0);
                            let subject = conv.get("subject").and_then(|v| v.as_str()).unwrap_or("(no subject)").to_string();
                            let status = conv.get("status").and_then(|v| v.as_str()).unwrap_or("active").to_string();
                            let assignee = conv.get("assignee").and_then(|v| v.as_str()).filter(|a| !a.trim().is_empty()).map(str::to_string);
                            let created = date_part(conv.get("remote_created_at").and_then(|v| v.as_str()));
                            let closed = date_part(conv.get("closed_at").and_then(|v| v.as_str()));
                            view! {
                                <tr
                                    class="spp-table__row-clickable"
                                    on:click=move |_| navigate.with_value(|n| n(&format!("/inbox/conversation/{id}"), Default::default()))
                                >
                                    <td class="spp-table__cell-mono">{number.to_string()}</td>
                                    <td>
                                        <A href={format!("/inbox/conversation/{id}")} attr:class="spp-customer-search__name-link">{subject}</A>
                                    </td>
                                    <td>
                                        <span class=match status.as_str() {
                                            "closed" => "spp-badge",
                                            "pending" => "spp-badge spp-badge--warn",
                                            _ => "spp-badge spp-badge--ok",
                                        }>
                                            {status.clone()}
                                        </span>
                                    </td>
                                    <td class="spp-table__cell-muted">{assignee.unwrap_or_else(|| "—".to_string())}</td>
                                    <td class="spp-table__cell-muted">{created}</td>
                                    <td class="spp-table__cell-muted">{closed}</td>
                                </tr>
                            }
                        }).collect::<Vec<_>>()}
                </tbody>
            </table>
        }.into_any()
    };
    view! {
        <div class="spp-card">
            <h3 class="spp-card__title">"Conversations"</h3>
            {body}
        </div>
    }
}

#[component]
fn ResolutionsCard(resolutions: Option<Vec<serde_json::Value>>) -> impl IntoView {
    let rows = resolutions.unwrap_or_default();
    let body = if rows.is_empty() {
        view! { <span class="spp-muted spp-text-xs">"No closed conversations with replies yet."</span> }.into_any()
    } else {
        view! {
            {rows.iter().map(|r| {
                    let number = r.get("number").and_then(|v| v.as_i64()).unwrap_or(0);
                    let subject = r.get("subject").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    let resolution = r.get("resolution").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    let excerpt: String = resolution.chars().take(220).collect();
                    let closed = date_part(r.get("closed_at").and_then(|v| v.as_str()));
                    view! {
                        <div class="spp-customer__memory">
                            <div class="spp-flex spp-flex--between">
                                <strong class="spp-text-sm">{format!("#{number} {subject}")}</strong>
                                <span class="spp-muted spp-text-xs">{closed}</span>
                            </div>
                            <div class="spp-muted spp-text-xs">{excerpt}</div>
                        </div>
                    }
                }).collect::<Vec<_>>()}
        }.into_any()
    };
    view! {
        <div class="spp-card">
            <h3 class="spp-card__title">"Previous resolutions"</h3>
            {body}
        </div>
    }
}

#[component]
fn RatingsCard(ratings: Option<Vec<serde_json::Value>>) -> impl IntoView {
    let rows = ratings.unwrap_or_default();
    let body = if rows.is_empty() {
        view! { <span class="spp-muted spp-text-xs">"No ratings synced."</span> }.into_any()
    } else {
        view! {
            {rows.iter().map(|r| {
                    let rating = r.get("rating").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    let comments = r.get("comments").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    let conversation_id = r.get("conversation_id").and_then(|v| v.as_i64());
                    let created = date_part(r.get("created_at").and_then(|v| v.as_str()));
                    view! {
                        <div class="spp-flex spp-flex--wrap spp-customer__rating">
                            <span class=match rating.as_str() {
                                "great" => "spp-badge spp-badge--ok",
                                "okay" => "spp-badge spp-badge--warn",
                                _ => "spp-badge",
                            }>
                                {rating.clone()}
                            </span>
                            <span class="spp-text-sm">{comments}</span>
                            {if let Some(cid) = conversation_id {
                                view! {
                                    <A href={format!("/inbox/conversation/{cid}")} attr:class="spp-muted spp-text-xs">
                                        {format!("#{cid}")}
                                    </A>
                                }.into_any()
                            } else {
                                ().into_any()
                            }}
                            {if !created.is_empty() {
                                view! { <span class="spp-muted spp-text-xs">{created}</span> }.into_any()
                            } else {
                                ().into_any()
                            }}
                        </div>
                    }
                }).collect::<Vec<_>>()}
        }.into_any()
    };
    view! {
        <div class="spp-card">
            <h3 class="spp-card__title">"Ratings received"</h3>
            {body}
        </div>
    }
}

#[component]
fn EditCustomerModal(
    customer_id: i64,
    customer: Option<serde_json::Value>,
    on_close: Arc<dyn Fn() + Send + Sync>,
    on_saved: Arc<dyn Fn() + Send + Sync>,
) -> impl IntoView {
    let c = customer.unwrap_or_default();
    let field = |key: &str| -> String {
        c.get(key)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()
    };
    let first_name = RwSignal::new(field("first_name"));
    let last_name = RwSignal::new(field("last_name"));
    let first_email = c
        .get("emails")
        .and_then(|e| e.as_array())
        .and_then(|a| a.first().and_then(|e| e.as_str()))
        .unwrap_or("")
        .to_string();
    let email = RwSignal::new(first_email);
    let organization = RwSignal::new(field("organization_name"));
    let job_title = RwSignal::new(field("job_title"));
    let phone = RwSignal::new(field("phone"));
    let submitting = RwSignal::new(false);

    let submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        if submitting.get() {
            return;
        }
        submitting.set(true);
        let body = serde_json::json!({
            "firstName": first_name.get(),
            "lastName": last_name.get(),
            "email": email.get(),
            "organization": organization.get(),
            "jobTitle": job_title.get(),
            "phone": phone.get(),
        });
        let on_saved = Arc::clone(&on_saved);
        wasm_bindgen_futures::spawn_local(async move {
            let path = format!("/api/customers/{customer_id}");
            match crate::api::patch_json::<serde_json::Value>(&path, &body).await {
                Ok(r) if r.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) => {
                    crate::toasts::success("Customer profile saved.");
                    on_saved();
                }
                Ok(r) => {
                    crate::toasts::error(
                        r.get("message")
                            .and_then(|v| v.as_str())
                            .unwrap_or("Save failed"),
                    );
                    submitting.set(false);
                }
                Err(e) => {
                    crate::toasts::error(e);
                    submitting.set(false);
                }
            }
        });
    };

    view! {
        <div class="spp-overlay" role="dialog" aria-modal="true">
            <div class="spp-modal spp-modal--form">
                <h3 class="spp-modal__title">"Edit customer profile"</h3>
                <p class="spp-modal__message">
                    "Changes are local-only (they never sync back to Help Scout). Empty a field to clear it."
                </p>
                <form on:submit=submit>
                    <div class="spp-form-grid">
                        <label class="spp-form-grid__label">"First name"</label>
                        <input
                            class="spp-input"
                            maxlength=80
                            prop:value=first_name
                            on:input=move |ev| first_name.set(event_target_value(&ev))
                        />
                        <label class="spp-form-grid__label">"Last name"</label>
                        <input
                            class="spp-input"
                            maxlength=80
                            prop:value=last_name
                            on:input=move |ev| last_name.set(event_target_value(&ev))
                        />
                        <label class="spp-form-grid__label">"Email"</label>
                        <input
                            class="spp-input"
                            type="email"
                            maxlength=200
                            prop:value=email
                            on:input=move |ev| email.set(event_target_value(&ev))
                        />
                        <label class="spp-form-grid__label">"Organization"</label>
                        <input
                            class="spp-input"
                            maxlength=200
                            prop:value=organization
                            on:input=move |ev| organization.set(event_target_value(&ev))
                        />
                        <label class="spp-form-grid__label">"Job title"</label>
                        <input
                            class="spp-input"
                            maxlength=200
                            prop:value=job_title
                            on:input=move |ev| job_title.set(event_target_value(&ev))
                        />
                        <label class="spp-form-grid__label">"Phone"</label>
                        <input
                            class="spp-input"
                            maxlength=60
                            prop:value=phone
                            on:input=move |ev| phone.set(event_target_value(&ev))
                        />
                    </div>
                    <div class="spp-modal__actions spp-mt-8">
                        <button class="spp-button spp-button--ghost" type="button" on:click=move |_| on_close()>
                            "Cancel"
                        </button>
                        <button
                            class="spp-button spp-button--primary"
                            type="submit"
                            disabled=move || submitting.get()
                        >
                            {move || if submitting.get() { "Saving…" } else { "Save changes" }}
                        </button>
                    </div>
                </form>
            </div>
        </div>
    }
}
