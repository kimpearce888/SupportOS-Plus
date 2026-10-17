use leptos::*;
use leptos_router::A;

use crate::components::state_view::{EmptyState, LoadingState};

#[component]
pub fn OrganizationsPage() -> impl IntoView {
    let organizations = create_rw_signal(Vec::<serde_json::Value>::new());
    let total = create_rw_signal(0i64);
    let loading = create_rw_signal(true);
    let error_msg = create_rw_signal(None::<String>);
    let query = create_rw_signal(String::new());
    let creating = create_rw_signal(false);
    let reload = create_rw_signal(0u32);
    let page = create_rw_signal(0u32);
    let per_page: u32 = 25;

    create_effect(move |_| {
        let _ = reload.get();
        let p = page.get();
        let organizations = organizations;
        let total = total;
        let loading = loading;
        let error_msg = error_msg;
        wasm_bindgen_futures::spawn_local(async move {
            let offset = p * per_page;
            let url = format!("/api/organizations?limit={per_page}&offset={offset}");
            match crate::api::get_json::<serde_json::Value>(&url).await {
                Ok(v) => {
                    let items = v
                        .get("organizations")
                        .and_then(|o| o.as_array())
                        .cloned()
                        .unwrap_or_default();
                    let t = v.get("total").and_then(|t| t.as_i64()).unwrap_or(0);
                    organizations.set(items);
                    total.set(t);
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
        <div class="spp-page spp-page--organizations">
            <header class="spp-page__header">
                <div>
                    <h2 class="spp-page__title">"Organizations"</h2>
                    <p class="spp-page__subtitle">
                        {move || format!("{} organizations", total.get())}
                    </p>
                </div>
                <div class="spp-page__header-actions">
                    <input
                        class="spp-input"
                        type="text"
                        placeholder="Search organizations…"
                        aria-label="Search organizations"
                        prop:value=query
                        on:input=move |ev| query.set(event_target_value(&ev))
                    />
                    <button
                        class="spp-button spp-button--primary"
                        on:click=move |_| creating.set(true)
                    >
                        "New organization"
                    </button>
                </div>
            </header>

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
                    when=move || !filtered_organizations(organizations, query).is_empty()
                    fallback=move || {
                        view! {
                            <EmptyState message="No organizations yet. Create one, or connect Help Scout to sync your customer companies." />
                        }
                    }
                >
                    <table class="spp-table">
                        <thead>
                            <tr>
                                <th>"Organization"</th>
                                <th>"Domains"</th>
                                <th>"Customers"</th>
                                <th>"Conversations"</th>
                            </tr>
                        </thead>
                        <tbody>
                            {move || {
                                filtered_organizations(organizations, query)
                                    .into_iter()
                                    .map(|org| {
                                        let id = org.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
                                        let name = org
                                            .get("name")
                                            .and_then(|v| v.as_str())
                                            .unwrap_or("(unnamed)")
                                            .to_string();
                                        let domains = org
                                            .get("domains")
                                            .and_then(|v| v.as_array())
                                            .map(|d| {
                                                d.iter()
                                                    .filter_map(|x| x.as_str())
                                                    .collect::<Vec<_>>()
                                                    .join(", ")
                                            })
                                            .filter(|s| !s.is_empty())
                                            .unwrap_or_else(|| "—".to_string());
                                        let customers = org
                                            .get("customer_count")
                                            .and_then(|v| v.as_i64())
                                            .map(|c| c.to_string())
                                            .unwrap_or_else(|| "—".to_string());
                                        let conversations = org
                                            .get("conversation_count")
                                            .and_then(|v| v.as_i64())
                                            .map(|c| c.to_string())
                                            .unwrap_or_else(|| "—".to_string());
                                        let href = format!("/organizations/{id}");
                                        view! {
                                            <tr class="spp-table__row--clickable">
                                                <td>
                                                    <A href=href class="spp-table__link">
                                                        {name}
                                                    </A>
                                                </td>
                                                <td class="spp-table__cell-muted">{domains}</td>
                                                <td>
                                                    <span class="spp-badge">{customers}</span>
                                                </td>
                                                <td>
                                                    <span class="spp-badge">{conversations}</span>
                                                </td>
                                            </tr>
                                        }
                                    })
                                    .collect::<Vec<_>>()
                            }}
                        </tbody>
                    </table>

                    <div class="spp-pagination">
                        <button
                            class="spp-button spp-button--small"
                            disabled=move || page.get() == 0
                            on:click=move |_| page.set(page.get().saturating_sub(1))
                        >
                            "Previous"
                        </button>
                        <span class="spp-pagination__info">
                            {move || {
                                let p = page.get();
                                let t = total.get();
                                let start = if t == 0 { 0 } else { p * per_page + 1 };
                                let end = ((p + 1) * per_page).min(t as u32);
                                format!("{start}–{end} of {t}")
                            }}
                        </span>
                        <button
                            class="spp-button spp-button--small"
                            disabled=move || (page.get() + 1) * per_page >= total.get() as u32
                            on:click=move |_| page.set(page.get() + 1)
                        >
                            "Next"
                        </button>
                    </div>
                </Show>
            </Show>

            <Show when=move || creating.get() fallback=|| ()>
                <EditOrganizationModal
                    organization=None
                    on_close=std::rc::Rc::new(move || creating.set(false))
                    on_saved=std::rc::Rc::new(move || {
                        creating.set(false);
                        reload.set(reload.get_untracked() + 1);
                    })
                />
            </Show>
        </div>
    }
}

#[component]
pub fn OrganizationDetailPage(organization_id: i64) -> impl IntoView {
    let organization = create_rw_signal(None::<serde_json::Value>);
    let health = create_rw_signal(None::<serde_json::Value>);
    let timeline = create_rw_signal(Vec::<serde_json::Value>::new());
    let loading = create_rw_signal(true);
    let error_msg = create_rw_signal(None::<String>);
    let editing = create_rw_signal(false);
    let reload = create_rw_signal(0u32);

    create_effect(move |_| {
        let _ = reload.get();
        let organization = organization;
        let health = health;
        let timeline = timeline;
        let loading = loading;
        let error_msg = error_msg;
        wasm_bindgen_futures::spawn_local(async move {
            let base = format!("/api/organizations/{organization_id}");
            let org_result = crate::api::get_json::<serde_json::Value>(&base).await;
            let health_result =
                crate::api::get_json::<serde_json::Value>(&format!("{base}/support-health")).await;
            let tl_result =
                crate::api::get_json::<serde_json::Value>(&format!("{base}/timeline")).await;
            match (org_result, health_result, tl_result) {
                (Ok(org), health_result, Ok(tl)) => {
                    organization.set(Some(org));
                    if let Ok(h) = health_result {
                        health.set(Some(h));
                    }
                    timeline.set(
                        tl.get("events")
                            .and_then(|v| v.as_array())
                            .cloned()
                            .unwrap_or_default(),
                    );
                    loading.set(false);
                }
                (Err(e), _, _) | (_, _, Err(e)) => {
                    error_msg.set(Some(e));
                    loading.set(false);
                }
            }
        });
    });

    view! {
        <div class="spp-page spp-page--organization-detail">
            <header class="spp-page__header">
                <h2 class="spp-page__title">"Organization"</h2>
                <div class="spp-page__header-actions">
                    <button
                        class="spp-button spp-button--ghost spp-button--small"
                        on:click=move |_| editing.set(true)
                    >
                        "Edit"
                    </button>
                    <A href="/organizations" class="spp-button spp-button--ghost spp-button--small">
                        "← All organizations"
                    </A>
                </div>
            </header>

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
                    when=move || organization.get().is_some()
                    fallback=|| {
                        view! {
                            <EmptyState message="Organization not found. It may have been deleted or never synced." />
                        }
                    }
                >
                    {move || {
                        let org = organization.get().unwrap_or_default();
                        let name = org
                            .get("name")
                            .and_then(|v| v.as_str())
                            .unwrap_or("(unnamed)")
                            .to_string();
                        let domains = org
                            .get("domains")
                            .and_then(|v| v.as_array())
                            .map(|d| {
                                d.iter()
                                    .filter_map(|x| x.as_str())
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            })
                            .filter(|s| !s.is_empty())
                            .unwrap_or_else(|| "—".to_string());
                        let customer_count = org
                            .get("customer_count")
                            .and_then(|v| v.as_i64())
                            .unwrap_or(0);
                        let conversation_count = org
                            .get("conversation_count")
                            .and_then(|v| v.as_i64())
                            .unwrap_or(0);
                        let open_count = org
                            .get("open_conversation_count")
                            .and_then(|v| v.as_i64())
                            .unwrap_or(0);
                        let members = org
                            .get("members")
                            .and_then(|v| v.as_array())
                            .cloned()
                            .unwrap_or_default();
                        let h = health.get().unwrap_or_default();
                        let health_kind = h
                            .get("health")
                            .and_then(|v| v.as_str())
                            .unwrap_or("unknown")
                            .to_string();
                        let resolution_rate = h
                            .get("resolution_rate")
                            .and_then(|v| v.as_f64())
                            .map(|r| format!("{:.0}%", r * 100.0))
                            .unwrap_or_else(|| "—".to_string());
                        let last_activity = h
                            .get("last_activity_at")
                            .and_then(|v| v.as_str())
                            .unwrap_or("—")
                            .to_string();
                        let members_for_show = members.clone();
                        view! {
                            <div class="spp-org-detail">
                                <div class="spp-card spp-org-detail__summary">
                                    <div class="spp-org-detail__identity">
                                        <h3 class="spp-card__title">{name}</h3>
                                        <p class="spp-page__subtitle">{domains}</p>
                                    </div>
                                    <dl class="spp-org-detail__stats">
                                        <div>
                                            <dt>"Customers"</dt>
                                            <dd>{customer_count.to_string()}</dd>
                                        </div>
                                        <div>
                                            <dt>"Conversations"</dt>
                                            <dd>{conversation_count.to_string()}</dd>
                                        </div>
                                        <div>
                                            <dt>"Open"</dt>
                                            <dd>{open_count.to_string()}</dd>
                                        </div>
                                        <div>
                                            <dt>"Resolution rate"</dt>
                                            <dd>{resolution_rate}</dd>
                                        </div>
                                        <div>
                                            <dt>"Last activity"</dt>
                                            <dd>{last_activity}</dd>
                                        </div>
                                        <div>
                                            <dt>"Support health"</dt>
                                            <dd>
                                                <span class="spp-badge">{health_kind}</span>
                                            </dd>
                                        </div>
                                    </dl>
                                </div>

                                <section class="spp-org-detail__section">
                                    <h3>"Members"</h3>
                                    <Show
                                        when=move || !members_for_show.is_empty()
                                        fallback=|| {
                                            view! {
                                                <EmptyState message="No customers linked to this organization yet." />
                                            }
                                        }
                                    >
                                        <table class="spp-table">
                                            <thead>
                                                <tr>
                                                    <th>"Customer"</th>
                                                    <th>"Email"</th>
                                                    <th>"Conversations"</th>
                                                    <th>"Open"</th>
                                                    <th>"Last activity"</th>
                                                </tr>
                                            </thead>
                                            <tbody>
                                                {members.iter().map(|m| {
                                                    let id = m.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
                                                    let member_name = m.get("name").and_then(|v| v.as_str()).unwrap_or("(unnamed)").to_string();
                                                    let email = m.get("email").and_then(|v| v.as_str()).unwrap_or("—").to_string();
                                                    let convs = m.get("conversation_count").and_then(|v| v.as_i64()).unwrap_or(0).to_string();
                                                    let open = m.get("open_count").and_then(|v| v.as_i64()).unwrap_or(0).to_string();
                                                    let last = m.get("last_activity_at").and_then(|v| v.as_str()).unwrap_or("—").to_string();
                                                    let href = format!("/customers/{id}");
                                                    view! {
                                                        <tr>
                                                            <td>
                                                                <A href=href class="spp-table__link">{member_name}</A>
                                                            </td>
                                                            <td class="spp-table__cell-muted">{email}</td>
                                                            <td><span class="spp-badge">{convs}</span></td>
                                                            <td><span class="spp-badge">{open}</span></td>
                                                            <td class="spp-table__cell-muted">{last}</td>
                                                        </tr>
                                                    }
                                                }).collect::<Vec<_>>()}
                                            </tbody>
                                        </table>
                                    </Show>
                                </section>

                                <section class="spp-org-detail__section">
                                    <h3>"Timeline"</h3>
                                    <Show
                                        when=move || !timeline.with(|t| t.is_empty())
                                        fallback=|| {
                                            view! {
                                                <EmptyState message="No member events yet. They appear as customers converse, rate and get exposed to incidents." />
                                            }
                                        }
                                    >
                                        <div class="spp-customer-profile__timeline">
                                            {timeline.get().iter().map(|e| {
                                                let kind = e.get("event_kind").and_then(|v| v.as_str()).or_else(|| e.get("event_type").and_then(|v| v.as_str())).unwrap_or("").to_string();
                                                let title = e.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                                let who = e.get("customer_name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                                let when = e.get("occurred_at").and_then(|v| v.as_str()).or_else(|| e.get("created_at").and_then(|v| v.as_str())).unwrap_or("").to_string();
                                                view! {
                                                    <div class="spp-timeline-entry">
                                                        <div class="spp-timeline-entry__header">
                                                            <span class="spp-timeline-entry__type">{kind}</span>
                                                            <span class="spp-timeline-entry__actor">{who}</span>
                                                            <span class="spp-timeline-entry__time">{when}</span>
                                                        </div>
                                                        {if !title.is_empty() {
                                                            view! {
                                                                <div class="spp-timeline-entry__body">{title.clone()}</div>
                                                            }.into_view()
                                                        } else {
                                                            ().into_view()
                                                        }}
                                                    </div>
                                                }
                                            }).collect::<Vec<_>>()}
                                        </div>
                                    </Show>
                                </section>
                            </div>
                        }
                    }}
                </Show>
            </Show>

            <Show when=move || editing.get() fallback=|| ()>
                <EditOrganizationModal
                    organization=organization.get_untracked()
                    on_close=std::rc::Rc::new(move || editing.set(false))
                    on_saved=std::rc::Rc::new(move || {
                        editing.set(false);
                        reload.set(reload.get_untracked() + 1);
                    })
                />
            </Show>
        </div>
    }
}

#[component]
fn EditOrganizationModal(
    organization: Option<serde_json::Value>,
    on_close: std::rc::Rc<dyn Fn()>,
    on_saved: std::rc::Rc<dyn Fn()>,
) -> impl IntoView {
    let existing = organization.unwrap_or_default();
    let editing_id = existing.get("id").and_then(|v| v.as_i64());
    let name = create_rw_signal(
        existing
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
    );
    let domains_text = create_rw_signal(
        existing
            .get("domains")
            .and_then(|v| v.as_array())
            .map(|d| {
                d.iter()
                    .filter_map(|x| x.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_default(),
    );
    let submitting = create_rw_signal(false);

    let submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        if submitting.get() {
            return;
        }
        let name_now = name.get();
        if name_now.trim().is_empty() {
            return;
        }
        let domains: Vec<String> = domains_text
            .get()
            .split(',')
            .map(|d| d.trim())
            .filter(|d| !d.is_empty())
            .map(|d| d.to_string())
            .collect();
        submitting.set(true);
        let on_saved = std::rc::Rc::clone(&on_saved);
        wasm_bindgen_futures::spawn_local(async move {
            let result = match editing_id {
                Some(id) => {
                    let path = format!("/api/organizations/{id}");
                    crate::api::patch_json::<serde_json::Value>(
                        &path,
                        &serde_json::json!({"name": name_now, "domains": domains}),
                    )
                    .await
                }
                None => {
                    crate::api::post_json::<serde_json::Value>(
                        "/api/organizations",
                        Some(&serde_json::json!({"name": name_now, "domains": domains})),
                    )
                    .await
                }
            };
            match result {
                Ok(r) if r.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) => {
                    if editing_id.is_some() {
                        crate::toasts::success("Organization saved.");
                    } else {
                        crate::toasts::success("Organization created.");
                    }
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
                <h3 class="spp-modal__title">
                    {if editing_id.is_some() { "Edit organization" } else { "New organization" }}
                </h3>
                <p class="spp-modal__message">
                    "Organizations are local-only (they never sync back to Help Scout). Domains are comma-separated, e.g. acme.com, acme.co.uk."
                </p>
                <form on:submit=submit>
                    <div class="spp-form-grid">
                        <label class="spp-form-grid__label">"Name *"</label>
                        <input
                            class="spp-input"
                            maxlength=200
                            prop:value=name
                            on:input=move |ev| name.set(event_target_value(&ev))
                            required=true
                        />
                        <label class="spp-form-grid__label">"Domains"</label>
                        <input
                            class="spp-input"
                            placeholder="acme.com, acme.co.uk"
                            prop:value=domains_text
                            on:input=move |ev| domains_text.set(event_target_value(&ev))
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
                            {move || {
                                if submitting.get() {
                                    "Saving…"
                                } else if editing_id.is_some() {
                                    "Save changes"
                                } else {
                                    "Create organization"
                                }
                            }}
                        </button>
                    </div>
                </form>
            </div>
        </div>
    }
}

fn filtered_organizations(
    organizations: leptos::RwSignal<Vec<serde_json::Value>>,
    query: leptos::RwSignal<String>,
) -> Vec<serde_json::Value> {
    let needle = query.get().to_lowercase();
    organizations
        .get()
        .into_iter()
        .filter(|org| {
            let name = org
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_lowercase();
            needle.is_empty() || name.contains(&needle)
        })
        .collect()
}
