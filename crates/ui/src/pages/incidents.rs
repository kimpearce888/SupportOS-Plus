use std::rc::Rc;

use leptos::*;
use leptos_router::{use_navigate, NavigateOptions, A};

use wasm_bindgen::JsCast;

use crate::components::overlays::ConfirmDialog;
use crate::components::state_view::{EmptyState, ErrorState, LoadingState};
use crate::toasts;

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

fn status_badge_class(status: &str) -> &'static str {
    match status {
        "resolved" => "spp-badge spp-badge--ok",
        "investigating" | "fix_in_progress" => "spp-badge spp-badge--warn",
        _ => "spp-badge",
    }
}

fn severity_badge_class(sev: &str) -> &'static str {
    match sev {
        "sev1" => "spp-badge spp-badge--err",
        "sev2" => "spp-badge spp-badge--warn",
        "sev4" => "spp-badge spp-badge--ok",
        _ => "spp-badge",
    }
}

fn humanize(s: &str) -> String {
    s.replace('_', " ")
}

fn short_date(iso: &str) -> String {
    iso.get(..10).unwrap_or(iso).to_string()
}

fn timeline_detail_summary(detail: Option<&str>) -> String {
    let Some(detail) = detail else {
        return String::new();
    };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(detail) else {
        return String::new();
    };
    let Some(obj) = v.as_object() else {
        return String::new();
    };
    obj.iter()
        .take(2)
        .map(|(k, val)| {
            let val = match val {
                serde_json::Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            let val = val.chars().take(60).collect::<String>();
            format!("{k}={val}")
        })
        .collect::<Vec<_>>()
        .join(" · ")
}

#[component]
pub fn IncidentsPage() -> impl IntoView {
    let incidents = create_rw_signal(Vec::<serde_json::Value>::new());
    let total = create_rw_signal(0i64);
    let loading = create_rw_signal(true);
    let error_msg = create_rw_signal(None::<String>);
    let q = create_rw_signal(String::new());
    let status_filter = create_rw_signal(String::new());
    let open_only = create_rw_signal(true);
    let create_open = create_rw_signal(false);
    let navigate = use_navigate();

    create_effect(move |_| {
        let q_now = q.get();
        let status_now = status_filter.get();
        let open_now = open_only.get();
        let incidents = incidents;
        let total = total;
        let loading = loading;
        let error_msg = error_msg;
        loading.set(true);
        wasm_bindgen_futures::spawn_local(async move {
            let mut path = format!(
                "/api/incidents?q={}&open={}",
                urlencode(&q_now),
                if open_now { "true" } else { "" }
            );
            if !status_now.is_empty() {
                path.push_str("&status=");
                path.push_str(&urlencode(&status_now));
            }
            match crate::api::get_json::<serde_json::Value>(&path).await {
                Ok(data) => {
                    incidents.set(
                        data.get("incidents")
                            .and_then(|v| v.as_array())
                            .cloned()
                            .unwrap_or_default(),
                    );
                    total.set(data.get("total").and_then(|v| v.as_i64()).unwrap_or(0));
                    error_msg.set(None);
                }
                Err(e) => error_msg.set(Some(e)),
            }
            loading.set(false);
        });
    });

    let on_open: Rc<dyn Fn(i64)> = {
        let navigate = navigate.clone();
        Rc::new(move |id| {
            navigate(&format!("/incidents/{id}"), NavigateOptions::default());
        })
    };
    let on_created: Rc<dyn Fn(i64)> = Rc::new(move |id| {
        toasts::success("Incident declared.");
        navigate(&format!("/incidents/{id}"), NavigateOptions::default());
    });
    let on_modal_close: Rc<dyn Fn()> = Rc::new(move || create_open.set(false));

    view! {
        <div class="spp-page spp-page--incidents">
            <header class="spp-page__header">
                <div>
                    <h2 class="spp-page__title">"Incidents"</h2>
                    <p class="spp-page__subtitle">
                        {move || format!(
                            "{} master issues · affected customers are derived from linked conversations (never ticket counts)",
                            total.get(),
                        )}
                    </p>
                </div>
                <div class="spp-flex">
                    <form
                        class="spp-searchbar"
                        on:submit=|ev| {
                            ev.prevent_default();
                        }
                    >
                        <input
                            class="spp-input"
                            placeholder="Search code or title…"
                            prop:value=q
                            on:input=move |ev| q.set(event_target_value(&ev))
                            aria-label="Search incidents"
                        />
                        <button class="spp-button" type="submit">"Search"</button>
                    </form>
                    <button class="spp-button" on:click=move |_| create_open.set(true)>
                        "+ Declare incident"
                    </button>
                </div>
            </header>

            <div class="spp-flex spp-flex--wrap spp-incidents__chips">
                {move || {
                    ["".to_string(), "investigating".into(), "identified".into(), "fix_in_progress".into(), "monitoring".into(), "resolved".into()]
                        .into_iter()
                        .map(|s| {
                            let label = if s.is_empty() {
                                "all statuses".to_string()
                            } else {
                                humanize(&s)
                            };
                            let s_for_class = s.clone();
                            let s_for_click = s.clone();
                            view! {
                                <button
                                    class=move || {
                                        format!(
                                            "spp-chip{}",
                                            if s_for_class == status_filter.get() { " spp-chip--active" } else { "" },
                                        )
                                    }
                                    on:click=move |_| status_filter.set(s_for_click.clone())
                                >
                                    {label}
                                </button>
                            }
                        })
                        .collect::<Vec<_>>()
                }}
                <button
                    class=move || {
                        format!(
                            "spp-chip{}",
                            if open_only.get() { " spp-chip--active" } else { "" },
                        )
                    }
                    title="Hide resolved incidents"
                    on:click=move |_| open_only.set(!open_only.get())
                >
                    "open only"
                </button>
            </div>

            <Show when=move || loading.get() fallback=|| ()>
                <LoadingState />
            </Show>
            <Show when=move || error_msg.get().is_some() fallback=|| ()>
                <ErrorState message="Could not load incidents" retry=None />
                <p class="spp-muted spp-text-xs">{move || error_msg.get().unwrap_or_default()}</p>
            </Show>

            <Show
                when=move || {
                    !loading.get() && error_msg.get().is_none() && !incidents.with(|i| i.is_empty())
                }
                fallback=|| ()
            >
                <IncidentsTable incidents=incidents on_open=Rc::clone(&on_open) />
            </Show>
            <Show
                when=move || {
                    !loading.get()
                        && error_msg.get().is_none()
                        && incidents.with(|i| i.is_empty())
                }
                fallback=|| ()
            >
                <EmptyState message="No incidents. Declare an incident from the Issues page (cluster or known issue) or create one here to group many conversations under one master issue." />
            </Show>

            <Show
                when=move || create_open.get()
                fallback=|| ()
            >
                <CreateIncidentModal
                    on_close=Rc::clone(&on_modal_close)
                    on_created=Rc::clone(&on_created)
                />
            </Show>
        </div>
    }
}

#[component]
fn IncidentsTable(
    incidents: RwSignal<Vec<serde_json::Value>>,
    on_open: Rc<dyn Fn(i64)>,
) -> impl IntoView {
    let rows = move || {
        incidents
            .get()
            .into_iter()
            .map(|row| {
                view! {
                    <IncidentListRow row=row on_open=Rc::clone(&on_open) />
                }
            })
            .collect::<Vec<_>>()
    };
    view! {
        <div class="spp-card spp-incidents__card">
            <table class="spp-table">
                <thead>
                    <tr>
                        <th>"Code"</th>
                        <th>"Title"</th>
                        <th>"Status"</th>
                        <th>"Severity"</th>
                        <th>"Conversations"</th>
                        <th>"Customers"</th>
                        <th>"Orgs"</th>
                        <th>"Owner"</th>
                        <th>"Updated"</th>
                    </tr>
                </thead>
                <tbody>{rows}</tbody>
            </table>
        </div>
    }
}

#[component]
fn IncidentListRow(row: serde_json::Value, on_open: Rc<dyn Fn(i64)>) -> impl IntoView {
    let id = row.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
    let code = row
        .get("code")
        .and_then(|v| v.as_str())
        .unwrap_or("INC-?")
        .to_string();
    let title = row
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("(untitled)")
        .to_string();
    let status = row
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let severity = row
        .get("severity")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let product = row
        .get("product")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let feature = row
        .get("feature")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let conversation_count = row
        .get("conversation_count")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let customer_count = row
        .get("customer_count")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let organization_count = row
        .get("organization_count")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let owner_name = row
        .get("owner_name")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    let updated = row
        .get("updated_at")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let href = format!("/incidents/{id}");

    let open_row = Rc::clone(&on_open);
    let open_link = Rc::clone(&on_open);
    view! {
        <tr class="spp-table__row--clickable" on:click=move |_| open_row(id)>
            <td class="spp-mono">
                <a
                    class="spp-table__link"
                    href=href
                    on:click=move |ev| {
                        ev.stop_propagation();
                        open_link(id);
                    }
                >
                    {code}
                </a>
            </td>
            <td>
                <strong class="spp-incidents__title">{title}</strong>
                {match (&product, &feature) {
                    (Some(p), Some(f)) => view! {
                        <div class="spp-muted spp-text-xs">{format!("{p} · {f}")}</div>
                    }.into_view(),
                    (Some(p), None) => view! {
                        <div class="spp-muted spp-text-xs">{p.clone()}</div>
                    }.into_view(),
                    _ => ().into_view(),
                }}
            </td>
            <td><span class=status_badge_class(&status)>{humanize(&status)}</span></td>
            <td><span class=severity_badge_class(&severity)>{severity.to_uppercase()}</span></td>
            <td><span class="spp-badge">{conversation_count.to_string()}</span></td>
            <td><span class="spp-badge">{customer_count.to_string()}</span></td>
            <td class="spp-table__cell-muted">
                {if organization_count > 0 { organization_count.to_string() } else { "—".to_string() }}
            </td>
            <td class="spp-table__cell-muted">
                {owner_name.unwrap_or_else(|| "—".to_string())}
            </td>
            <td class="spp-table__cell-muted">{short_date(&updated)}</td>
        </tr>
    }
}

#[component]
fn CreateIncidentModal(on_close: Rc<dyn Fn()>, on_created: Rc<dyn Fn(i64)>) -> impl IntoView {
    let title = create_rw_signal(String::new());
    let severity = create_rw_signal("sev3".to_string());
    let status = create_rw_signal("investigating".to_string());
    let description = create_rw_signal(String::new());
    let submitting = create_rw_signal(false);

    let declare = move |_| {
        let title_now = title.get();
        if title_now.trim().is_empty() || submitting.get() {
            return;
        }
        submitting.set(true);
        let body = serde_json::json!({
            "title": title_now,
            "severity": severity.get(),
            "status": status.get(),
            "description": if description.get().trim().is_empty() { serde_json::Value::Null } else { serde_json::Value::String(description.get()) },
        });
        let on_created = Rc::clone(&on_created);
        wasm_bindgen_futures::spawn_local(async move {
            match crate::api::post_json::<serde_json::Value>("/api/incidents", Some(&body)).await {
                Ok(r) if r.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) => {
                    let id = r
                        .pointer("/incident/id")
                        .and_then(|v| v.as_i64())
                        .unwrap_or_default();
                    on_created(id);
                }
                Ok(r) => {
                    toasts::error(
                        r.get("message")
                            .and_then(|v| v.as_str())
                            .unwrap_or("Could not create incident"),
                    );
                    submitting.set(false);
                }
                Err(e) => {
                    toasts::error(e);
                    submitting.set(false);
                }
            }
        });
    };

    view! {
        <div class="spp-overlay" role="dialog" aria-modal="true">
            <div class="spp-modal spp-modal--form">
                <h3 class="spp-modal__title">"Declare incident"</h3>
                <div class="spp-form-grid">
                    <label class="spp-form-grid__label">"Title"</label>
                    <input
                        class="spp-input"
                        placeholder="e.g. Timezone display issue"
                        prop:value=title
                        on:input=move |ev| title.set(event_target_value(&ev))
                    />
                    <label class="spp-form-grid__label">"Severity"</label>
                    <select
                        class="spp-input"
                        prop:value=severity
                        on:change=move |ev| severity.set(event_target_value(&ev))
                    >
                        <option value="sev1">"SEV1 — critical"</option>
                        <option value="sev2">"SEV2 — major"</option>
                        <option value="sev3">"SEV3 — moderate"</option>
                        <option value="sev4">"SEV4 — minor"</option>
                    </select>
                    <label class="spp-form-grid__label">"Status"</label>
                    <select
                        class="spp-input"
                        prop:value=status
                        on:change=move |ev| status.set(event_target_value(&ev))
                    >
                        <option value="investigating">"investigating"</option>
                        <option value="identified">"identified"</option>
                        <option value="fix_in_progress">"fix in progress"</option>
                        <option value="monitoring">"monitoring"</option>
                        <option value="resolved">"resolved"</option>
                    </select>
                    <label class="spp-form-grid__label">
                        "Description " <span class="spp-muted">"(optional)"</span>
                    </label>
                    <textarea
                        class="spp-input"
                        rows="3"
                        placeholder="What is going on?"
                        prop:value=description
                        on:input=move |ev| description.set(event_target_value(&ev))
                    >
                    </textarea>
                </div>
                <p class="spp-muted spp-text-xs">
                    "Conversations can be linked after creation. Everything here is LOCAL SupportOS data — nothing is written to Help Scout."
                </p>
                <div class="spp-modal__actions">
                    <button class="spp-button" on:click=move |_| on_close()>"Cancel"</button>
                    <button
                        class="spp-button spp-button--primary"
                        disabled=move || title.get().trim().is_empty() || submitting.get()
                        on:click=declare
                    >
                        "Declare"
                    </button>
                </div>
            </div>
        </div>
    }
}

#[component]
pub fn IncidentDetailPage(incident_id: i64) -> impl IntoView {
    let data = create_rw_signal(None::<serde_json::Value>);
    let loading = create_rw_signal(true);
    let error_msg = create_rw_signal(None::<String>);
    let refresh = create_rw_signal(0u32);

    create_effect(move |_| {
        let _tick = refresh.get();
        let data = data;
        let loading = loading;
        let error_msg = error_msg;
        loading.set(true);
        wasm_bindgen_futures::spawn_local(async move {
            let path = format!("/api/incidents/{incident_id}");
            match crate::api::get_json::<serde_json::Value>(&path).await {
                Ok(v) if v.get("incident").is_some() => {
                    data.set(Some(v));
                    error_msg.set(None);
                }
                Ok(_) => {
                    error_msg.set(Some("Incident not found.".to_string()));
                    data.set(None);
                }
                Err(e) => {
                    error_msg.set(Some(e));
                    data.set(None);
                }
            }
            loading.set(false);
        });
    });

    let reload: Rc<dyn Fn()> = Rc::new(move || refresh.update(|t| *t += 1));

    view! {
        <div class="spp-page spp-page--incident-detail">
            <Show when=move || loading.get() fallback=|| ()>
                <LoadingState />
            </Show>
            <Show when=move || error_msg.get().is_some() fallback=|| ()>
                <ErrorState message="Could not load incident" retry=None />
                <p class="spp-muted spp-text-xs">{move || error_msg.get().unwrap_or_default()}</p>
            </Show>
            <IncidentDetailContent data=data reload=Rc::clone(&reload) />
        </div>
    }
}

#[component]
fn IncidentDetailContent(
    data: RwSignal<Option<serde_json::Value>>,
    reload: Rc<dyn Fn()>,
) -> impl IntoView {
    let content = move || match data.get() {
        Some(payload) => view! {
            <IncidentDetailLoaded payload=payload reload=Rc::clone(&reload) />
        }
        .into_view(),
        None => ().into_view(),
    };
    view! {
        <div class="spp-incidents__detail-content">
            {content}
        </div>
    }
}

#[component]
fn IncidentDetailLoaded(payload: serde_json::Value, reload: Rc<dyn Fn()>) -> impl IntoView {
    let inc = payload
        .get("incident")
        .cloned()
        .unwrap_or_else(|| serde_json::json!({}));
    let impact = payload
        .get("impact")
        .cloned()
        .unwrap_or_else(|| serde_json::json!({}));
    let conversations = payload
        .get("conversations")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let affected_customers = payload
        .get("affected_customers")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let affected_organizations = payload
        .get("affected_organizations")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let related = payload
        .get("related")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let refs = payload
        .get("refs")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let releases = payload
        .get("releases")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let notes = payload
        .get("notes")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let timeline = payload
        .get("timeline")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let incident_id = inc.get("id").and_then(|v| v.as_i64()).unwrap_or_default();
    let code = inc
        .get("code")
        .and_then(|v| v.as_str())
        .unwrap_or("INC-?")
        .to_string();
    let title = inc
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("(untitled)")
        .to_string();
    let status = inc
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let severity = inc
        .get("severity")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let product = inc
        .get("product")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let feature = inc
        .get("feature")
        .and_then(|v| v.as_str())
        .map(str::to_string);

    let set_status = {
        let reload = Rc::clone(&reload);
        move |ev: leptos::ev::Event| {
            let value = event_target_value(&ev);
            let reload = Rc::clone(&reload);
            wasm_bindgen_futures::spawn_local(async move {
                let path = format!("/api/incidents/{incident_id}");
                let body = serde_json::json!({ "status": value });
                match crate::api::patch_json::<serde_json::Value>(&path, &body).await {
                    Ok(_) => reload(),
                    Err(e) => toasts::error(e),
                }
            });
        }
    };
    let set_severity = {
        let reload = Rc::clone(&reload);
        move |ev: leptos::ev::Event| {
            let value = event_target_value(&ev);
            let reload = Rc::clone(&reload);
            wasm_bindgen_futures::spawn_local(async move {
                let path = format!("/api/incidents/{incident_id}");
                let body = serde_json::json!({ "severity": value });
                match crate::api::patch_json::<serde_json::Value>(&path, &body).await {
                    Ok(_) => reload(),
                    Err(e) => toasts::error(e),
                }
            });
        }
    };

    let append_resolution = {
        let reload = Rc::clone(&reload);
        let existing = inc
            .get("resolution")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        move |ev: leptos::ev::FocusEvent| {
            let added = event_target_value(&ev).trim().to_string();
            if added.is_empty() {
                return;
            }
            let joined = if existing.is_empty() {
                added
            } else {
                format!("{existing}\n\n{added}")
            };
            let _ = ev
                .target()
                .and_then(|t| t.dyn_into::<web_sys::HtmlTextAreaElement>().ok())
                .map(|ta| ta.set_value(""));
            let reload = Rc::clone(&reload);
            wasm_bindgen_futures::spawn_local(async move {
                let path = format!("/api/incidents/{incident_id}");
                let body = serde_json::json!({ "resolution": joined });
                match crate::api::patch_json::<serde_json::Value>(&path, &body).await {
                    Ok(_) => reload(),
                    Err(e) => toasts::error(e),
                }
            });
        }
    };

    let link_number = create_rw_signal(String::new());
    let submit_link = {
        let reload = Rc::clone(&reload);
        move |ev: leptos::ev::SubmitEvent| {
            ev.prevent_default();
            let raw = link_number.get().trim().replace('#', "");
            let Ok(num) = raw.parse::<i64>() else {
                return;
            };
            if num <= 0 {
                return;
            }
            link_number.set(String::new());
            let reload = Rc::clone(&reload);
            wasm_bindgen_futures::spawn_local(async move {
                let list_path = format!("/api/conversations?number={num}&view=all&pageSize=5");
                match crate::api::get_json::<serde_json::Value>(&list_path).await {
                    Ok(list) => {
                        let found = list
                            .get("conversations")
                            .and_then(|v| v.as_array())
                            .and_then(|rows| {
                                rows.iter()
                                    .find(|c| c.get("number").and_then(|n| n.as_i64()) == Some(num))
                                    .cloned()
                            });
                        match found {
                            Some(conv) => {
                                let conv_id =
                                    conv.get("id").and_then(|v| v.as_i64()).unwrap_or_default();
                                let link_path =
                                    format!("/api/incidents/{incident_id}/conversations/{conv_id}");
                                match crate::api::post_json::<serde_json::Value>(&link_path, None)
                                    .await
                                {
                                    Ok(_) => reload(),
                                    Err(e) => toasts::error(e),
                                }
                            }
                            None => {
                                toasts::error(format!(
                                    "No conversation #{num} in the local mirror."
                                ));
                            }
                        }
                    }
                    Err(e) => toasts::error(e),
                }
            });
        }
    };

    let note_body = create_rw_signal(String::new());
    let add_note = {
        let reload = Rc::clone(&reload);
        move |ev: leptos::ev::SubmitEvent| {
            ev.prevent_default();
            let body = note_body.get().trim().to_string();
            if body.is_empty() {
                return;
            }
            note_body.set(String::new());
            let reload = Rc::clone(&reload);
            wasm_bindgen_futures::spawn_local(async move {
                let path = format!("/api/incidents/{incident_id}/notes");
                let body = serde_json::json!({ "body": body });
                match crate::api::post_json::<serde_json::Value>(&path, Some(&body)).await {
                    Ok(_) => reload(),
                    Err(e) => toasts::error(e),
                }
            });
        }
    };

    let add_ref = {
        let reload = Rc::clone(&reload);
        move |_| {
            let Some(window) = web_sys::window() else {
                return;
            };
            let Some(system) = window
                .prompt_with_message("Reference system (e.g. linear, jira, github)")
                .ok()
                .flatten()
                .filter(|s| !s.trim().is_empty())
            else {
                return;
            };
            let Some(reference) = window
                .prompt_with_message("Reference id (e.g. ENG-4471)")
                .ok()
                .flatten()
                .filter(|s| !s.trim().is_empty())
            else {
                return;
            };
            let reload = Rc::clone(&reload);
            wasm_bindgen_futures::spawn_local(async move {
                let path = format!("/api/incidents/{incident_id}/refs");
                let body =
                    serde_json::json!({ "system": system.trim(), "reference": reference.trim() });
                match crate::api::post_json::<serde_json::Value>(&path, Some(&body)).await {
                    Ok(_) => reload(),
                    Err(e) => toasts::error(e),
                }
            });
        }
    };

    let confirm_delete = create_rw_signal(false);
    let release_open = create_rw_signal(false);
    let do_delete = {
        let navigate = use_navigate();
        move |_| {
            let navigate = navigate.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let path = format!("/api/incidents/{incident_id}");
                match crate::api::delete_json::<serde_json::Value>(&path).await {
                    Ok(_) => {
                        toasts::success("Incident deleted.");
                        navigate("/incidents", NavigateOptions::default());
                    }
                    Err(e) => toasts::error(e),
                }
            });
        }
    };
    let on_release_close: Rc<dyn Fn()> = Rc::new(move || release_open.set(false));
    let on_release_saved: Rc<dyn Fn()> = Rc::new({
        let reload = Rc::clone(&reload);
        move || {
            release_open.set(false);
            reload();
        }
    });
    let delete_requested = create_rw_signal(false);
    create_effect(move |_| {
        if delete_requested.get() {
            delete_requested.set(false);
            do_delete(());
        }
    });
    let on_confirm_delete: std::sync::Arc<dyn Fn() + Send + Sync> =
        std::sync::Arc::new(move || {
            confirm_delete.set(false);
            delete_requested.set(true);
        });
    let on_cancel_delete: std::sync::Arc<dyn Fn() + Send + Sync> =
        std::sync::Arc::new(move || confirm_delete.set(false));

    let affected_conversations = impact
        .get("affected_conversations")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let affected_customers_n = impact
        .get("affected_customers")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let affected_organizations_n = impact
        .get("affected_organizations")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let waiting_count = impact
        .get("customer_waiting_count")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let first_seen = impact
        .get("first_seen_at")
        .and_then(|v| v.as_str())
        .map(short_date)
        .unwrap_or_else(|| "unknown".to_string());
    let last_seen = impact
        .get("last_seen_at")
        .and_then(|v| v.as_str())
        .map(short_date)
        .unwrap_or_else(|| "unknown".to_string());
    let growth = impact
        .get("growth_rate_7d")
        .cloned()
        .unwrap_or_else(|| serde_json::json!({}));
    let growth_line = format!(
        "{} recent vs {} previous ({})",
        growth.get("recent").and_then(|v| v.as_i64()).unwrap_or(0),
        growth.get("previous").and_then(|v| v.as_i64()).unwrap_or(0),
        growth
            .get("direction")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown"),
    );
    let trend = impact
        .get("trend")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();
    let open_n = impact
        .pointer("/open_closed_distribution/open")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let closed_n = impact
        .pointer("/open_closed_distribution/closed")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let inboxes_line = impact
        .get("affected_inboxes")
        .and_then(|v| v.as_array())
        .map(|rows| {
            rows.iter()
                .map(|m| {
                    format!(
                        "{} ({})",
                        m.get("mailbox")
                            .and_then(|v| v.as_str())
                            .unwrap_or("unassigned"),
                        m.get("conversations").and_then(|v| v.as_i64()).unwrap_or(0),
                    )
                })
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default();
    let tags_line = impact
        .get("top_tags")
        .and_then(|v| v.as_array())
        .map(|rows| {
            rows.iter()
                .map(|t| {
                    format!(
                        "{} ({})",
                        t.get("tag").and_then(|v| v.as_str()).unwrap_or("?"),
                        t.get("conversations").and_then(|v| v.as_i64()).unwrap_or(0),
                    )
                })
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default();
    let products_line = impact
        .get("products")
        .and_then(|v| v.as_array())
        .map(|rows| {
            rows.iter()
                .map(|p| {
                    format!(
                        "{} ({})",
                        p.get("product").and_then(|v| v.as_str()).unwrap_or("?"),
                        p.get("conversations").and_then(|v| v.as_i64()).unwrap_or(0),
                    )
                })
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default();
    let impact_note = impact
        .get("note")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    view! {
        <header class="spp-page__header">
            <div>
                <h2 class="spp-page__title">
                    <span class="spp-mono">{code.clone()}</span>
                    " "
                    {title.clone()}
                </h2>
                <p class="spp-page__subtitle">
                    <span class=status_badge_class(&status)>{humanize(&status)}</span>
                    " "
                    <span class=severity_badge_class(&severity)>{severity.to_uppercase()}</span>
                    {match (&product, &feature) {
                        (Some(p), Some(f)) => view! { " · " {format!("{p} · {f}")} }.into_view(),
                        (Some(p), None) => view! { " · " {p.clone()} }.into_view(),
                        _ => ().into_view(),
                    }}
                    {format!(
                        " · {} conversations · {} customers · {} organizations",
                        affected_conversations, affected_customers_n, affected_organizations_n,
                    )}
                </p>
            </div>
            <div class="spp-flex">
                <select
                    class="spp-input spp-incidents__select"
                    prop:value=status.clone()
                    on:change=set_status
                    aria-label="Incident status"
                >
                    <option value="investigating">"investigating"</option>
                    <option value="identified">"identified"</option>
                    <option value="fix_in_progress">"fix in progress"</option>
                    <option value="monitoring">"monitoring"</option>
                    <option value="resolved">"resolved"</option>
                </select>
                <select
                    class="spp-input spp-incidents__select"
                    prop:value=severity.clone()
                    on:change=set_severity
                    aria-label="Incident severity"
                >
                    <option value="sev1">"SEV1"</option>
                    <option value="sev2">"SEV2"</option>
                    <option value="sev3">"SEV3"</option>
                    <option value="sev4">"SEV4"</option>
                </select>
                <A href="/incidents" class="spp-button">"← All incidents"</A>
                <button class="spp-button spp-button--danger" on:click=move |_| confirm_delete.set(true)>
                    "Delete"
                </button>
            </div>
        </header>

        <div class="spp-grid-2">
            <div class="spp-card">
                <h3 class="spp-card__title">"Impact — deterministic, derived from linked conversations"</h3>
                <div class="spp-impact-grid">
                    <div class="spp-impact-stat">
                        <span class="spp-impact-value">{affected_conversations.to_string()}</span>
                        <span class="spp-impact-label">"conversations"</span>
                    </div>
                    <div class="spp-impact-stat">
                        <span class="spp-impact-value">{affected_customers_n.to_string()}</span>
                        <span class="spp-impact-label">"unique customers"</span>
                    </div>
                    <div class="spp-impact-stat">
                        <span class="spp-impact-value">{affected_organizations_n.to_string()}</span>
                        <span class="spp-impact-label">"organizations"</span>
                    </div>
                    <div class="spp-impact-stat">
                        <span class="spp-impact-value">{waiting_count.to_string()}</span>
                        <span class="spp-impact-label">"waiting now"</span>
                    </div>
                </div>
                <dl class="spp-kv-list">
                    <div class="spp-kv-row"><dt>"First seen"</dt><dd>{first_seen}</dd></div>
                    <div class="spp-kv-row"><dt>"Last seen"</dt><dd>{last_seen}</dd></div>
                    <div class="spp-kv-row"><dt>"7-day growth"</dt><dd>{growth_line}</dd></div>
                    <div class="spp-kv-row"><dt>"Trend"</dt><dd>{trend}</dd></div>
                    <div class="spp-kv-row">
                        <dt>"Open / closed"</dt>
                        <dd>{format!("{open_n} open · {closed_n} closed")}</dd>
                    </div>
                    {if !inboxes_line.is_empty() {
                        view! {
                            <div class="spp-kv-row"><dt>"Inboxes"</dt><dd>{inboxes_line}</dd></div>
                        }.into_view()
                    } else { ().into_view() }}
                    {if !tags_line.is_empty() {
                        view! {
                            <div class="spp-kv-row"><dt>"Tags"</dt><dd>{tags_line}</dd></div>
                        }.into_view()
                    } else { ().into_view() }}
                    {if !products_line.is_empty() {
                        view! {
                            <div class="spp-kv-row"><dt>"Products (AI attributes)"</dt><dd>{products_line}</dd></div>
                        }.into_view()
                    } else {
                        view! {
                            <div class="spp-kv-row">
                                <dt>"Products (AI attributes)"</dt>
                                <dd class="spp-muted">"unknown until conversations are analyzed"</dd>
                            </div>
                        }.into_view()
                    }}
                </dl>
                <p class="spp-muted spp-text-xs spp-incidents__note">{impact_note}</p>
            </div>

            <div class="spp-card">
                <h3 class="spp-card__title">"Explanations & resolution"</h3>
                <p class="spp-muted spp-text-xs">
                    "Internal explanation and engineering references are never exposed to customers; the customer-safe text is what outreach and drafts may use."
                </p>
                <ExplanationsCard incident=inc.clone() />
                <textarea
                    class="spp-input spp-mt-8"
                    rows="2"
                    placeholder="Append to resolution…"
                    on:blur=append_resolution
                >
                </textarea>
            </div>
        </div>

        <div class="spp-card spp-mt-16">
            <div class="spp-flex spp-flex--between">
                <h3 class="spp-card__title">
                    {format!("Linked conversations ({})", conversations.len())}
                </h3>
                <form class="spp-flex" on:submit=submit_link>
                    <input
                        class="spp-input spp-incidents__link-input"
                        placeholder="Link #number…"
                        prop:value=link_number
                        on:input=move |ev| link_number.set(event_target_value(&ev))
                        aria-label="Link conversation by number"
                    />
                    <button class="spp-button spp-button--small" type="submit">"+ Link"</button>
                </form>
            </div>
            {if conversations.is_empty() {
                view! {
                    <EmptyState message="No conversations linked yet. Link conversations by number to make affected customers and impact counts real." />
                }.into_view()
            } else {
                view! {
                    <table class="spp-table">
                        <thead>
                            <tr>
                                <th>"#"</th>
                                <th>"Subject"</th>
                                <th>"Status"</th>
                                <th>"Customer"</th>
                                <th>"Mailbox"</th>
                                <th>"Created"</th>
                                <th></th>
                            </tr>
                        </thead>
                        <tbody>
                            {conversations
                                .iter()
                                .map(|row| {
                                    view! {
                                        <ConversationRow
                                            incident_id=incident_id
                                            row=row.clone()
                                            reload=Rc::clone(&reload)
                                        />
                                    }
                                })
                                .collect::<Vec<_>>()}
                        </tbody>
                    </table>
                }.into_view()
            }}
        </div>

        <div class="spp-grid-2 spp-mt-16">
            <div class="spp-card">
                <h3 class="spp-card__title">
                    {format!("Affected customers ({})", affected_customers.len())}
                </h3>
                <p class="spp-muted spp-text-xs">
                    "Distinct customers derived from the linked conversations — a ticket count is never used as a customer count."
                </p>
                {if affected_customers.is_empty() {
                    view! { <span class="spp-muted">"None yet."</span> }.into_view()
                } else {
                    view! {
                        <table class="spp-table">
                            <thead>
                                <tr><th>"Customer"</th><th>"Organization"</th><th>"Conversations"</th><th>"Open"</th></tr>
                            </thead>
                            <tbody>
                                {affected_customers
                                    .iter()
                                    .map(|c| view! { <AffectedCustomerRow row=c.clone() /> })
                                    .collect::<Vec<_>>()}
                            </tbody>
                        </table>
                    }.into_view()
                }}
                {if !affected_organizations.is_empty() {
                    view! {
                        <h3 class="spp-card__title spp-mt-16">"Affected organizations"</h3>
                        {affected_organizations
                            .iter()
                            .map(|o| {
                                let name = o
                                    .get("name")
                                    .and_then(|v| v.as_str())
                                    .map(str::to_string)
                                    .unwrap_or_else(|| {
                                        format!(
                                            "organization #{}",
                                            o.get("organization_id").and_then(|v| v.as_i64()).unwrap_or(0),
                                        )
                                    });
                                let counts = format!(
                                    "{} customers · {} conversations",
                                    o.get("customers").and_then(|v| v.as_i64()).unwrap_or(0),
                                    o.get("conversations").and_then(|v| v.as_i64()).unwrap_or(0),
                                );
                                view! {
                                    <div class="spp-flex spp-flex--between spp-incidents__org-row">
                                        <span class="spp-incidents__org-name">{name}</span>
                                        <span class="spp-muted spp-text-xs">{counts}</span>
                                    </div>
                                }
                            })
                            .collect::<Vec<_>>()}
                    }.into_view()
                } else { ().into_view() }}
            </div>

            <div class="spp-card">
                <h3 class="spp-card__title">"Engineering references, releases & related"</h3>
                {if refs.is_empty() && releases.is_empty() && related.is_empty() {
                    view! {
                        <EmptyState message="No references yet. Attach the Jira/Linear ticket, the release that may be associated, related known issues, knowledge articles or campaigns." />
                    }.into_view()
                } else { ().into_view() }}
                {refs
                    .iter()
                    .map(|r| {
                        view! {
                            <RefRow incident_id=incident_id row=r.clone() reload=Rc::clone(&reload) />
                        }
                    })
                    .collect::<Vec<_>>()}
                {releases
                    .iter()
                    .map(|r| {
                        view! {
                            <ReleaseRow incident_id=incident_id row=r.clone() reload=Rc::clone(&reload) />
                        }
                    })
                    .collect::<Vec<_>>()}
                {related
                    .iter()
                    .map(|r| {
                        view! {
                            <RelatedRow incident_id=incident_id row=r.clone() reload=Rc::clone(&reload) />
                        }
                    })
                    .collect::<Vec<_>>()}
                <div class="spp-flex spp-mt-8">
                    <button class="spp-button spp-button--small" on:click=move |_| release_open.set(true)>
                        "Add release"
                    </button>
                    <button class="spp-button spp-button--small" on:click=add_ref>
                        "Add engineering ref"
                    </button>
                </div>
            </div>
        </div>

        <div class="spp-grid-2 spp-mt-16">
            <div class="spp-card">
                <h3 class="spp-card__title">"Notes"</h3>
                <form on:submit=add_note>
                    <textarea
                        class="spp-input"
                        rows="2"
                        placeholder="Add an operational note…"
                        prop:value=note_body
                        on:input=move |ev| note_body.set(event_target_value(&ev))
                    >
                    </textarea>
                    <button
                        class="spp-button spp-button--small spp-mt-8"
                        type="submit"
                        disabled=move || note_body.get().trim().is_empty()
                    >
                        "Add note"
                    </button>
                </form>
                {notes
                    .iter()
                    .map(|n| {
                        let author = n
                            .get("author_name")
                            .and_then(|v| v.as_str())
                            .filter(|s| !s.is_empty())
                            .unwrap_or("local user")
                            .to_string();
                        let created = n
                            .get("created_at")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let body = n
                            .get("body")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        view! {
                            <div class="spp-incidents__note">
                                <div class="spp-flex spp-flex--between">
                                    <strong>{author}</strong>
                                    <span class="spp-muted spp-text-xs">{short_date(&created)}</span>
                                </div>
                                <div>{body}</div>
                            </div>
                        }
                    })
                    .collect::<Vec<_>>()}
            </div>

            <div class="spp-card">
                <h3 class="spp-card__title">"Incident timeline"</h3>
                <p class="spp-muted spp-text-xs">
                    "Append-only, idempotent by dedup key — created, status/severity changes, links, notes, refs and releases."
                </p>
                {timeline
                    .iter()
                    .map(|ev| {
                        let event_type = ev
                            .get("event_type")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let occurred = ev
                            .get("occurred_at")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let detail = timeline_detail_summary(
                            ev.get("detail").and_then(|v| v.as_str()),
                        );
                        view! {
                            <div class="spp-incidents__timeline-row">
                                <span class="spp-badge">{humanize(&event_type)}</span>
                                <span class="spp-muted spp-text-xs">{short_date(&occurred)}</span>
                                {if detail.is_empty() {
                                    ().into_view()
                                } else {
                                    view! { <span class="spp-text-xs">{detail}</span> }.into_view()
                                }}
                            </div>
                        }
                    })
                    .collect::<Vec<_>>()}
            </div>
        </div>

        <Show when=move || release_open.get() fallback=|| ()>
            <ReleaseForm
                incident_id=incident_id
                on_close=Rc::clone(&on_release_close)
                on_saved=Rc::clone(&on_release_saved)
            />
        </Show>
        <Show when=move || confirm_delete.get() fallback=|| ()>
            <ConfirmDialog
                title="Delete incident"
                message=format!(
                    "Delete {code}? Linked conversations stay untouched — only the local incident workspace is removed."
                )
                confirm_label="Delete".to_string()
                danger=true
                on_confirm=std::sync::Arc::clone(&on_confirm_delete)
                on_cancel=std::sync::Arc::clone(&on_cancel_delete)
            />
        </Show>
    }
}

fn explanation_field(incident: &serde_json::Value, label: &str, key: &str) -> impl IntoView {
    let label = label.to_string();
    let value = incident
        .get(key)
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| "—".to_string());
    view! {
        <div class="spp-incidents__explanation">
            <strong class="spp-text-xs">{label}</strong>
            <div class="spp-incidents__explanation-body">{value}</div>
        </div>
    }
}

#[component]
fn ExplanationsCard(incident: serde_json::Value) -> impl IntoView {
    view! {
        <div>
            {explanation_field(&incident, "Description", "description")}
            {explanation_field(&incident, "Internal explanation", "internal_explanation")}
            {explanation_field(&incident, "Customer-safe explanation", "customer_safe_explanation")}
            {explanation_field(&incident, "Known cause", "known_cause")}
            {explanation_field(&incident, "Workaround", "workaround")}
            {explanation_field(&incident, "Resolution", "resolution")}
        </div>
    }
}

#[component]
fn ConversationRow(
    incident_id: i64,
    row: serde_json::Value,
    reload: Rc<dyn Fn()>,
) -> impl IntoView {
    let conversation_id = row
        .get("conversation_id")
        .and_then(|v| v.as_i64())
        .unwrap_or_default();
    let number = row.get("number").and_then(|v| v.as_i64()).unwrap_or(0);
    let subject = row
        .get("subject")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("(no subject)")
        .to_string();
    let status = row
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let customer_id = row.get("customer_local_id").and_then(|v| v.as_i64());
    let customer_name = row
        .get("customer_name")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    let mailbox = row
        .get("mailbox")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    let created = row
        .get("remote_created_at")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let unlink = {
        let reload = Rc::clone(&reload);
        move |_| {
            let reload = Rc::clone(&reload);
            wasm_bindgen_futures::spawn_local(async move {
                let path = format!("/api/incidents/{incident_id}/conversations/{conversation_id}");
                match crate::api::delete_json::<serde_json::Value>(&path).await {
                    Ok(_) => reload(),
                    Err(e) => toasts::error(e),
                }
            });
        }
    };

    view! {
        <tr>
            <td class="spp-mono">
                <A href=format!("/inbox/conversation/{conversation_id}") class="spp-table__link">
                    {format!("#{number}")}
                </A>
            </td>
            <td>
                <A href=format!("/inbox/conversation/{conversation_id}") class="spp-table__link">
                    {subject}
                </A>
            </td>
            <td><span class="spp-badge spp-badge--status">{humanize(&status)}</span></td>
            <td class="spp-table__cell-muted">
                {match (customer_id, customer_name) {
                    (Some(cid), name) => view! {
                        <A href=format!("/customers/{cid}") class="spp-table__link">
                            {name.unwrap_or_else(|| "—".to_string())}
                        </A>
                    }.into_view(),
                    (None, _) => view! { "—" }.into_view(),
                }}
            </td>
            <td class="spp-table__cell-muted">{mailbox.unwrap_or_else(|| "—".to_string())}</td>
            <td class="spp-table__cell-muted">{short_date(&created)}</td>
            <td>
                <button class="spp-button spp-button--ghost spp-button--tiny" title="Unlink" on:click=unlink>
                    "unlink"
                </button>
            </td>
        </tr>
    }
}

#[component]
fn AffectedCustomerRow(row: serde_json::Value) -> impl IntoView {
    let customer_id = row
        .get("customer_local_id")
        .and_then(|v| v.as_i64())
        .unwrap_or_default();
    let name = row
        .get("name")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| format!("customer #{customer_id}"));
    let email = row
        .get("email")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let organization = row
        .get("organization")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    let conversations = row
        .get("conversations")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let open = row
        .get("open_conversations")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    view! {
        <tr>
            <td>
                <A href=format!("/customers/{customer_id}") class="spp-table__link spp-incidents__customer-name">
                    {name}
                </A>
                <div class="spp-muted spp-text-xs">{email}</div>
            </td>
            <td class="spp-table__cell-muted">{organization.unwrap_or_else(|| "—".to_string())}</td>
            <td><span class="spp-badge">{conversations.to_string()}</span></td>
            <td>
                {if open > 0 {
                    view! { <span class="spp-badge spp-badge--ok">{open.to_string()}</span> }.into_view()
                } else {
                    view! { "—" }.into_view()
                }}
            </td>
        </tr>
    }
}

#[component]
fn RefRow(incident_id: i64, row: serde_json::Value, reload: Rc<dyn Fn()>) -> impl IntoView {
    let ref_id = row.get("id").and_then(|v| v.as_i64()).unwrap_or_default();
    let system = row
        .get("system")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let reference = row
        .get("reference")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let title = row
        .get("title")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| format!(" · {s}"));
    let status = row
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let notes = row
        .get("notes")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let remove = {
        let reload = Rc::clone(&reload);
        move |_| {
            let reload = Rc::clone(&reload);
            wasm_bindgen_futures::spawn_local(async move {
                let path = format!("/api/incidents/{incident_id}/refs/{ref_id}");
                match crate::api::delete_json::<serde_json::Value>(&path).await {
                    Ok(_) => reload(),
                    Err(e) => toasts::error(e),
                }
            });
        }
    };

    view! {
        <div class="spp-incidents__ref">
            <div class="spp-flex spp-flex--between">
                <strong class="spp-text-xs">
                    {format!("{system}: {reference}")}
                    {title.unwrap_or_default()}
                </strong>
                <button class="spp-button spp-button--ghost spp-button--tiny" on:click=remove>
                    "remove"
                </button>
            </div>
            <div class="spp-muted spp-text-xs">{format!("{status}{notes}")}</div>
        </div>
    }
}

#[component]
fn ReleaseRow(incident_id: i64, row: serde_json::Value, reload: Rc<dyn Fn()>) -> impl IntoView {
    let release_id = row.get("id").and_then(|v| v.as_i64()).unwrap_or_default();
    let label = row
        .get("version_label")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let released_at = row
        .get("released_at")
        .and_then(|v| v.as_str())
        .map(short_date);
    let correlation = row
        .get("correlation")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| format!(" — {s}"));

    let remove = {
        let reload = Rc::clone(&reload);
        move |_| {
            let reload = Rc::clone(&reload);
            wasm_bindgen_futures::spawn_local(async move {
                let path = format!("/api/incidents/{incident_id}/releases/{release_id}");
                match crate::api::delete_json::<serde_json::Value>(&path).await {
                    Ok(_) => reload(),
                    Err(e) => toasts::error(e),
                }
            });
        }
    };

    view! {
        <div class="spp-incidents__ref">
            <div class="spp-flex spp-flex--between">
                <strong class="spp-text-xs">{format!("Release {label}")}</strong>
                <button class="spp-button spp-button--ghost spp-button--tiny" on:click=remove>
                    "remove"
                </button>
            </div>
            <div class="spp-muted spp-text-xs">
                {format!(
                    "{}{}",
                    released_at.map(|d| format!("released {d}")).unwrap_or_else(|| "no date".to_string()),
                    correlation.unwrap_or_default(),
                )}
            </div>
        </div>
    }
}

#[component]
fn RelatedRow(incident_id: i64, row: serde_json::Value, reload: Rc<dyn Fn()>) -> impl IntoView {
    let target_kind = row
        .get("target_kind")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let target_local_id = row
        .get("target_local_id")
        .and_then(|v| v.as_i64())
        .unwrap_or_default();
    let target_label = row
        .get("target_label")
        .and_then(|v| v.as_str())
        .unwrap_or("#?")
        .to_string();
    let note = row
        .get("note")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| format!(" · {s}"));

    let unlink = {
        let reload = Rc::clone(&reload);
        let target_kind = target_kind.clone();
        move |_| {
            let reload = Rc::clone(&reload);
            let target_kind = target_kind.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let path =
                    format!("/api/incidents/{incident_id}/related/{target_kind}/{target_local_id}");
                match crate::api::delete_json::<serde_json::Value>(&path).await {
                    Ok(_) => reload(),
                    Err(e) => toasts::error(e),
                }
            });
        }
    };

    view! {
        <div class="spp-flex spp-flex--between spp-incidents__ref">
            <span class="spp-text-xs">
                <span class="spp-badge">{humanize(&target_kind)}</span>
                " "
                {target_label}
                {note.unwrap_or_default()}
            </span>
            <button class="spp-button spp-button--ghost spp-button--tiny" on:click=unlink>
                "unlink"
            </button>
        </div>
    }
}

#[component]
fn ReleaseForm(incident_id: i64, on_close: Rc<dyn Fn()>, on_saved: Rc<dyn Fn()>) -> impl IntoView {
    let version_label = create_rw_signal(String::new());
    let released_at = create_rw_signal(String::new());
    let notes = create_rw_signal(String::new());
    let submitting = create_rw_signal(false);

    let add = {
        let on_saved = Rc::clone(&on_saved);
        let on_close = Rc::clone(&on_close);
        move |_| {
            if version_label.get().trim().is_empty() || submitting.get() {
                return;
            }
            submitting.set(true);
            let body = serde_json::json!({
                "versionLabel": version_label.get(),
                "releasedAt": if released_at.get().trim().is_empty() {
                    serde_json::Value::Null
                } else {
                    serde_json::Value::String(released_at.get())
                },
                "notes": if notes.get().trim().is_empty() {
                    serde_json::Value::Null
                } else {
                    serde_json::Value::String(notes.get())
                },
            });
            let on_saved = Rc::clone(&on_saved);
            let on_close = Rc::clone(&on_close);
            wasm_bindgen_futures::spawn_local(async move {
                let path = format!("/api/incidents/{incident_id}/releases");
                match crate::api::post_json::<serde_json::Value>(&path, Some(&body)).await {
                    Ok(r) if r.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) => {
                        toasts::success("Release added.");
                        on_saved();
                    }
                    Ok(r) => {
                        toasts::error(
                            r.get("message")
                                .and_then(|v| v.as_str())
                                .unwrap_or("Failed."),
                        );
                        on_close();
                    }
                    Err(e) => {
                        toasts::error(e);
                        on_close();
                    }
                }
            });
        }
    };

    view! {
        <div class="spp-overlay" role="dialog" aria-modal="true">
            <div class="spp-modal spp-modal--form">
                <h3 class="spp-modal__title">"Add release"</h3>
                <div class="spp-form-grid">
                    <label class="spp-form-grid__label">"Version label"</label>
                    <input
                        class="spp-input"
                        placeholder="e.g. v4.12.0"
                        prop:value=version_label
                        on:input=move |ev| version_label.set(event_target_value(&ev))
                    />
                    <label class="spp-form-grid__label">
                        "Released at " <span class="spp-muted">"(optional)"</span>
                    </label>
                    <input
                        class="spp-input"
                        type="date"
                        prop:value=released_at
                        on:input=move |ev| released_at.set(event_target_value(&ev))
                    />
                    <label class="spp-form-grid__label">
                        "Notes " <span class="spp-muted">"(optional)"</span>
                    </label>
                    <textarea
                        class="spp-input"
                        rows="2"
                        prop:value=notes
                        on:input=move |ev| notes.set(event_target_value(&ev))
                    >
                    </textarea>
                </div>
                <p class="spp-muted spp-text-xs">
                    "Release correlation is reported as a temporal association only — SupportOS never claims the release caused the incident."
                </p>
                <div class="spp-modal__actions">
                    <button class="spp-button" on:click=move |_| on_close()>"Cancel"</button>
                    <button
                        class="spp-button spp-button--primary"
                        disabled=move || version_label.get().trim().is_empty() || submitting.get()
                        on:click=add
                    >
                        "Add"
                    </button>
                </div>
            </div>
        </div>
    }
}
