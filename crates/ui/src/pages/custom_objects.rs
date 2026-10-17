use leptos::*;
use std::sync::Arc;

use crate::components::overlays::ConfirmDialog;
use crate::components::state_view::{EmptyState, LoadingState};

const TARGET_KINDS: [&str; 6] = [
    "customer",
    "organization",
    "conversation",
    "known_issue",
    "incident",
    "campaign",
];

#[component]
pub fn CustomObjectsPage() -> impl IntoView {
    let types = create_rw_signal(Vec::<serde_json::Value>::new());
    let objects = create_rw_signal(Vec::<serde_json::Value>::new());
    let report = create_rw_signal(serde_json::Value::Null);
    let loading = create_rw_signal(true);
    let error_msg = create_rw_signal(None::<String>);
    let selected_type_id = create_rw_signal(None::<i64>);
    let query = create_rw_signal(String::new());
    let type_modal_open = create_rw_signal(false);
    let object_modal =
        create_rw_signal(Option::<(serde_json::Value, Option<serde_json::Value>)>::None);
    let confirm_delete = create_rw_signal(Option::<(String, i64, String)>::None);
    let reload = create_rw_signal(0u32);

    create_effect(move |_| {
        let _ = reload.get();
        let types = types;
        let report = report;
        let loading = loading;
        let error_msg = error_msg;
        wasm_bindgen_futures::spawn_local(async move {
            let types_result =
                crate::api::get_json::<serde_json::Value>("/api/custom-objects/types").await;
            let report_result =
                crate::api::get_json::<serde_json::Value>("/api/custom-objects/report").await;
            match (types_result, report_result) {
                (Ok(t), Ok(r)) => {
                    types.set(
                        t.get("types")
                            .and_then(|v| v.as_array())
                            .cloned()
                            .unwrap_or_default(),
                    );
                    report.set(r.get("report").cloned().unwrap_or(serde_json::Value::Null));
                    loading.set(false);
                }
                (Err(e), _) | (_, Err(e)) => {
                    error_msg.set(Some(e));
                    loading.set(false);
                }
            }
        });
    });

    create_effect(move |_| {
        let _ = reload.get();
        let type_now = selected_type_id.get();
        let q = query.get();
        let objects = objects;
        wasm_bindgen_futures::spawn_local(async move {
            let mut path = "/api/custom-objects?pageSize=100".to_string();
            if let Some(id) = type_now {
                path.push_str(&format!("&typeId={id}"));
            }
            if !q.trim().is_empty() {
                path.push_str(&format!("&q={}", urlencode(q.trim())));
            }
            match crate::api::get_json::<serde_json::Value>(&path).await {
                Ok(data) => {
                    objects.set(
                        data.get("objects")
                            .and_then(|v| v.as_array())
                            .cloned()
                            .unwrap_or_default(),
                    );
                }
                Err(_) => objects.set(Vec::new()),
            }
        });
    });

    let active_type = move || {
        let id = selected_type_id.get()?;
        types
            .get()
            .into_iter()
            .find(|t| t.get("id").and_then(|v| v.as_i64()) == Some(id))
    };

    let total_objects = move || {
        report
            .get()
            .get("total_objects")
            .and_then(|v| v.as_i64())
            .unwrap_or(0)
    };
    let total_links = move || {
        report
            .get()
            .get("total_links")
            .and_then(|v| v.as_i64())
            .unwrap_or(0)
    };

    let run_delete = move |(kind, id, _label): (String, i64, String)| {
        wasm_bindgen_futures::spawn_local(async move {
            let path = if kind == "type" {
                format!("/api/custom-objects/types/{id}")
            } else {
                format!("/api/custom-objects/{id}")
            };
            match crate::api::delete_json::<serde_json::Value>(&path).await {
                Ok(r) if r.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) => {
                    crate::toasts::success("Deleted.");
                    confirm_delete.set(None);
                    if kind == "type" {
                        selected_type_id.set(None);
                    }
                }
                Ok(r) => {
                    crate::toasts::error(
                        r.get("message")
                            .and_then(|v| v.as_str())
                            .unwrap_or("Failed."),
                    );
                    confirm_delete.set(None);
                }
                Err(e) => {
                    crate::toasts::error(e);
                    confirm_delete.set(None);
                }
            }
            reload.update(|n| *n = n.wrapping_add(1));
        });
    };

    view! {
        <div class="spp-page spp-page--custom-objects">
            <header class="spp-page__header">
                <div>
                    <h2 class="spp-page__title">"Custom Objects"</h2>
                    <p class="spp-page__subtitle">
                        {move || format!(
                            "{} local objects · {} relationships · purely local, never synced to Help Scout",
                            total_objects(),
                            total_links(),
                        )}
                    </p>
                </div>
                <div class="spp-page__header-actions">
                    <input
                        class="spp-input"
                        type="text"
                        placeholder="Search objects…"
                        aria-label="Search custom objects"
                        prop:value=query
                        on:input=move |ev| query.set(event_target_value(&ev))
                    />
                    <button
                        class="spp-button"
                        on:click=move |_| type_modal_open.set(true)
                    >
                        "▦ New type"
                    </button>
                    {move || {
                        if let Some(t) = active_type() {
                            if let Some(_t_id) = t.get("id").and_then(|v| v.as_i64()) {
                                let name = t.get("name").and_then(|v| v.as_str()).unwrap_or("object").to_string();
                                view! {
                                    <button
                                        class="spp-button spp-button--primary"
                                        on:click=move |_| object_modal.set(Some((t.clone(), None)))
                                    >
                                        {format!("+ New {name}")}
                                    </button>
                                }
                                .into_view()
                            } else {
                                ().into_view()
                            }
                        } else {
                            ().into_view()
                        }
                    }}
                </div>
            </header>

            <div class="spp-custom-objects__chips">
                <button
                    class="spp-chip"
                    class:is-on=move || selected_type_id.get().is_none()
                    on:click=move |_| selected_type_id.set(None)
                >
                    "all types"
                </button>
                {move || types.get().iter().map(|t| {
                    let t_id = t.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
                    let name = t.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    let object_count = t.get("object_count").and_then(|v| v.as_i64()).unwrap_or(0);
                    let is_on = move || selected_type_id.get() == Some(t_id);
                    view! {
                        <button
                            class="spp-chip"
                            class:is-on=is_on
                            on:click=move |_| selected_type_id.set(Some(t_id))
                        >
                            {name.clone()} " "
                            <span class="spp-badge">{object_count.to_string()}</span>
                        </button>
                    }
                }).collect::<Vec<_>>()}
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
                <Show
                    when=move || !types.get().is_empty()
                    fallback=|| {
                        view! {
                            <EmptyState message="No object types yet. Define a type (e.g. Account, Subscription, Deployment) with typed fields, then create objects and link them to customers, organizations, conversations, issues, incidents or campaigns." />
                        }
                    }
                >
                    {move || {
                        let Some(t) = active_type() else { return ().into_view(); };
                        let fields: Vec<serde_json::Value> = t
                            .get("fields")
                            .and_then(|v| v.as_array())
                            .cloned()
                            .unwrap_or_default();
                        let column_fields: Vec<serde_json::Value> = fields.iter().take(4).cloned().collect();
                        let type_name = t.get("name").and_then(|v| v.as_str()).unwrap_or("objects").to_string();
                        let rows: Vec<serde_json::Value> = objects.get();
                        if rows.is_empty() {
                            return view! {
                                <EmptyState message=format!("No {type_name} objects yet.") />
                            }.into_view();
                        }
                        view! {
                            <table class="spp-table">
                                <thead>
                                    <tr>
                                        <th>"Title"</th>
                                        {column_fields.iter().map(|f| {
                                            let label = f.get("label").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                            view! { <th>{label}</th> }
                                        }).collect::<Vec<_>>()}
                                        <th>"Updated"</th>
                                        <th></th>
                                    </tr>
                                </thead>
                                <tbody>
                                    {rows.iter().map(|o| {
                                        let title = o.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                        let updated_at = o.get("updated_at").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                        let id = o.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
                                        let properties = o.get("properties").cloned().unwrap_or(serde_json::Value::Null);
                                        let t_for_edit = t.clone();
                                        let id_for_edit = id;
                                        let title_for_delete = title.clone();
                                        let id_for_delete = id;
                                        view! {
                                            <tr>
                                                <td><strong class="spp-text-sm">{title}</strong></td>
                                                {column_fields.iter().map(|f| {
                                                    let key = f.get("key").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                                    let shown = format_value(properties.get(&key));
                                                    view! { <td class="spp-text-sm">{shown}</td> }
                                                }).collect::<Vec<_>>()}
                                                <td class="spp-text-xs">{updated_at}</td>
                                                <td>
                                                    <div class="spp-custom-objects__row-actions">
                                                        <button
                                                            class="spp-button spp-button--small spp-button--ghost"
                                                            on:click=move |_| {
                                                                let t_for_edit = t_for_edit.clone();
                                                                wasm_bindgen_futures::spawn_local(async move {
                                                                    let path = format!("/api/custom-objects/{id_for_edit}");
                                                                    match crate::api::get_json::<serde_json::Value>(&path).await {
                                                                        Ok(r) => {
                                                                            if let Some(object) = r.get("object").cloned() {
                                                                                object_modal.set(Some((t_for_edit, Some(object))));
                                                                            } else {
                                                                                crate::toasts::error("Object not found.");
                                                                            }
                                                                        }
                                                                        Err(e) => crate::toasts::error(e),
                                                                    }
                                                                });
                                                            }
                                                        >
                                                            "edit"
                                                        </button>
                                                        <button
                                                            class="spp-button spp-button--small spp-button--ghost"
                                                            title="Delete"
                                                            on:click=move |_| {
                                                                confirm_delete.set(Some(("object".into(), id_for_delete, title_for_delete.clone())));
                                                            }
                                                        >
                                                            "✕"
                                                        </button>
                                                    </div>
                                                </td>
                                            </tr>
                                        }
                                    }).collect::<Vec<_>>()}
                                </tbody>
                            </table>
                        }.into_view()
                    }}

                    {move || {
                        if active_type().is_some() {
                            return ().into_view();
                        }
                        if objects.get().is_empty() {
                            return view! {
                                <EmptyState message="No objects match." />
                            }.into_view();
                        }
                        let types_snapshot = types.get();
                        view! {
                            <table class="spp-table">
                                <thead>
                                    <tr><th>"Object"</th><th>"Type"</th><th>"Properties"</th><th>"Updated"</th></tr>
                                </thead>
                                <tbody>
                                    {move || objects.get().iter().map(|o| {
                                        let o = o.clone();
                                        let title = o.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                        let type_id = o.get("type_id").and_then(|v| v.as_i64());
                                        let updated_at = o.get("updated_at").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                        let properties = o.get("properties").cloned().unwrap_or(serde_json::Value::Null);
                                        let type_name = type_id
                                            .and_then(|tid| types_snapshot.iter().find(|t| t.get("id").and_then(|v| v.as_i64()) == Some(tid)))
                                            .and_then(|t| t.get("name").and_then(|v| v.as_str()).map(str::to_string))
                                            .unwrap_or_else(|| type_id.map(|tid| tid.to_string()).unwrap_or_default());
                                        view! {
                                            <tr>
                                                <td><strong class="spp-text-sm">{title}</strong></td>
                                                <td><span class="spp-badge">{type_name}</span></td>
                                                <td class="spp-text-xs spp-text-muted">{properties_summary(&properties)}</td>
                                                <td class="spp-text-xs">{updated_at}</td>
                                            </tr>
                                        }
                                    }).collect::<Vec<_>>()}
                                </tbody>
                            </table>
                        }.into_view()
                    }}

                    {move || {
                        let report_types: Vec<serde_json::Value> = report
                            .get()
                            .get("types")
                            .and_then(|v| v.as_array())
                            .cloned()
                            .unwrap_or_default();
                        if report_types.is_empty() {
                            return ().into_view();
                        }
                        view! {
                            <div class="spp-card spp-mt-4">
                                <h3 class="spp-card__title">"Relationships by type"</h3>
                                <p class="spp-text-xs spp-text-muted">
                                    "Custom objects relate to core entities through link edges; counts are deterministic reads."
                                </p>
                                {report_types.iter().map(|t| {
                                    let name = t.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                    let object_count = t.get("object_count").and_then(|v| v.as_i64()).unwrap_or(0);
                                    let link_counts = t.get("link_counts").cloned().unwrap_or(serde_json::Value::Null);
                                    let entries: Vec<(String, i64)> = link_counts
                                        .as_object()
                                        .map(|m| {
                                            m.iter()
                                                .filter_map(|(k, v)| v.as_i64().map(|n| (k.clone(), n)))
                                                .collect()
                                        })
                                        .unwrap_or_default();
                                    let summary = if entries.is_empty() {
                                        "no links yet".to_string()
                                    } else {
                                        entries
                                            .iter()
                                            .map(|(kind, n)| format!("{}: {}", kind.replace('_', " "), n))
                                            .collect::<Vec<_>>()
                                            .join(" · ")
                                    };
                                    view! {
                                        <div class="spp-flex-between spp-custom-objects__report-row">
                                            <strong class="spp-text-sm">
                                                {name} " " <span class="spp-badge">{object_count.to_string()}</span>
                                            </strong>
                                            <span class="spp-text-xs spp-text-muted">{summary}</span>
                                        </div>
                                    }
                                }).collect::<Vec<_>>()}
                            </div>
                        }.into_view()
                    }}
                </Show>
            </Show>

            <Show when=move || type_modal_open.get() fallback=|| ()>
                <TypeModal
                    on_close=std::rc::Rc::new(move || type_modal_open.set(false))
                    on_saved=std::rc::Rc::new(move || {
                        type_modal_open.set(false);
                        reload.update(|n| *n = n.wrapping_add(1));
                    })
                />
            </Show>

            {move || {
                if let Some((t, existing)) = object_modal.get() {
                    view! {
                        <ObjectModal
                            type_definition=t
                            existing=existing
                            on_close=std::rc::Rc::new(move || object_modal.set(None))
                            on_saved=std::rc::Rc::new(move || {
                                object_modal.set(None);
                                reload.update(|n| *n = n.wrapping_add(1));
                            })
                        />
                    }
                    .into_view()
                } else {
                    ().into_view()
                }
            }}

            {move || {
                if let Some((kind, id, label)) = confirm_delete.get() {
                    let message = if kind == "type" {
                        format!("Delete type \"{label}\"? Only possible when it has no objects.")
                    } else {
                        format!("Delete object \"{label}\"? Links are removed with it; nothing outside SupportOS is affected.")
                    };
                    let title = format!("Delete {kind}");
                    let on_confirm = Arc::new(move || run_delete((kind.clone(), id, label.clone())));
                    let on_cancel = Arc::new(move || confirm_delete.set(None));
                    view! {
                        <ConfirmDialog
                            title
                            message
                            confirm_label="Delete"
                            danger=true
                            on_confirm
                            on_cancel
                        />
                    }
                    .into_view()
                } else {
                    ().into_view()
                }
            }}
        </div>
    }
}

#[derive(Clone, Default)]
struct FieldDraft {
    key: String,
    label: String,
    field_type: String,
    required: bool,
    options: String,
}

#[component]
fn TypeModal(on_close: std::rc::Rc<dyn Fn()>, on_saved: std::rc::Rc<dyn Fn()>) -> impl IntoView {
    let name = create_rw_signal(String::new());
    let description = create_rw_signal(String::new());
    let fields = create_rw_signal(vec![FieldDraft::default()]);
    let submitting = create_rw_signal(false);

    let can_submit = move || {
        !name.get().trim().is_empty()
            && fields
                .get()
                .iter()
                .any(|f| !f.key.trim().is_empty() && !f.label.trim().is_empty())
    };

    let submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        if submitting.get() || !can_submit() {
            return;
        }
        submitting.set(true);
        let description_value = {
            let d = description.get();
            if d.trim().is_empty() {
                serde_json::Value::Null
            } else {
                serde_json::json!(d.trim())
            }
        };
        let payload = serde_json::json!({
            "name": name.get().trim(),
            "description": description_value,
            "fields": fields
                .get()
                .iter()
                .filter(|f| !f.key.trim().is_empty() && !f.label.trim().is_empty())
                .map(|f| {
                    let options: Vec<String> = if f.field_type == "select" {
                        f.options
                            .split(',')
                            .map(|s| s.trim().to_string())
                            .filter(|s| !s.is_empty())
                            .collect()
                    } else {
                        Vec::new()
                    };
                    serde_json::json!({
                        "key": f.key.trim(),
                        "label": f.label.trim(),
                        "fieldType": f.field_type,
                        "required": f.required,
                        "options": if f.field_type == "select" { serde_json::json!(options) } else { serde_json::Value::Null },
                    })
                })
                .collect::<Vec<_>>(),
        });
        let on_saved = std::rc::Rc::clone(&on_saved);
        wasm_bindgen_futures::spawn_local(async move {
            match crate::api::post_json::<serde_json::Value>(
                "/api/custom-objects/types",
                Some(&payload),
            )
            .await
            {
                Ok(r) if r.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) => {
                    crate::toasts::success("Type created.");
                    on_saved();
                }
                Ok(r) => {
                    crate::toasts::error(
                        r.get("message")
                            .and_then(|v| v.as_str())
                            .unwrap_or("Failed."),
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
            <div class="spp-modal spp-modal--form spp-modal--wide">
                <h3 class="spp-modal__title">"New object type"</h3>
                <form on:submit=submit>
                    <div class="spp-form-grid">
                        <label class="spp-form-grid__label">"Type name"</label>
                        <input
                            class="spp-input"
                            maxlength=80
                            placeholder="e.g. Account, Deployment, Subscription"
                            prop:value=name
                            on:input=move |ev| name.set(event_target_value(&ev))
                        />
                        <label class="spp-form-grid__label">"Description (optional)"</label>
                        <input
                            class="spp-input"
                            prop:value=description
                            on:input=move |ev| description.set(event_target_value(&ev))
                        />
                    </div>
                    <h4 class="spp-card__title spp-mt-4">"Fields"</h4>
                    <p class="spp-text-xs spp-text-muted">
                        "Keys are lowercase snake_case identifiers; values are validated by a schema built from these definitions — user data never becomes SQL."
                    </p>
                    {move || fields
                        .get()
                        .iter()
                        .enumerate()
                        .map(|(i, f)| {
                            let field_type = f.field_type.clone();
                            let field_type_for_select = field_type.clone();
                            let options_value = f.options.clone();
                            view! {
                                <div class="spp-custom-objects__field-row">
                                    <input
                                        class="spp-input"
                                        placeholder="key (snake_case)"
                                        prop:value=f.key.clone()
                                        on:input=move |ev| {
                                            let v = event_target_value(&ev).to_lowercase()
                                                .chars()
                                                .map(|c| if c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' { c } else { '_' })
                                                .collect::<String>();
                                            fields.update(|fs| fs[i].key = v);
                                        }
                                    />
                                    <input
                                        class="spp-input"
                                        placeholder="label"
                                        prop:value=f.label.clone()
                                        on:input=move |ev| fields.update(|fs| fs[i].label = event_target_value(&ev))
                                    />
                                    <select
                                        class="spp-input"
                                        prop:value=field_type_for_select
                                        on:change=move |ev| fields.update(|fs| fs[i].field_type = event_target_value(&ev))
                                    >
                                        <option value="text">"text"</option>
                                        <option value="long_text">"long text"</option>
                                        <option value="number">"number"</option>
                                        <option value="date">"date"</option>
                                        <option value="boolean">"boolean"</option>
                                        <option value="select">"select"</option>
                                    </select>
                                    {if field_type == "select" {
                                        view! {
                                            <input
                                                class="spp-input spp-custom-objects__options-input"
                                                placeholder="options, comma-separated"
                                                prop:value=options_value
                                                on:input=move |ev| fields.update(|fs| fs[i].options = event_target_value(&ev))
                                            />
                                        }.into_view()
                                    } else {
                                        ().into_view()
                                    }}
                                    <label class="spp-custom-objects__checkbox">
                                        <input
                                            type="checkbox"
                                            prop:checked=f.required
                                            on:change=move |ev| fields.update(|fs| fs[i].required = event_target_checked(&ev))
                                        />
                                        "required"
                                    </label>
                                    <button
                                        class="spp-button spp-button--ghost spp-button--small"
                                        type="button"
                                        title="Remove field"
                                        on:click=move |_| fields.update(|fs| {
                                            if fs.len() > 1 { fs.remove(i); }
                                        })
                                    >
                                        "✕"
                                    </button>
                                </div>
                            }
                        })
                        .collect::<Vec<_>>()}
                    <button
                        class="spp-button spp-button--small"
                        type="button"
                        on:click=move |_| fields.update(|fs| fs.push(FieldDraft::default()))
                    >
                        "+ Add field"
                    </button>
                    <div class="spp-modal__actions spp-mt-4">
                        <button class="spp-button spp-button--ghost" type="button" on:click=move |_| on_close()>
                            "Cancel"
                        </button>
                        <button
                            class="spp-button spp-button--primary"
                            type="submit"
                            disabled=move || submitting.get() || !can_submit()
                        >
                            {move || if submitting.get() { "Creating…" } else { "Create type" }}
                        </button>
                    </div>
                </form>
            </div>
        </div>
    }
}

#[component]
fn ObjectModal(
    type_definition: serde_json::Value,
    existing: Option<serde_json::Value>,
    on_close: std::rc::Rc<dyn Fn()>,
    on_saved: std::rc::Rc<dyn Fn()>,
) -> impl IntoView {
    let type_id = type_definition
        .get("id")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let type_name = type_definition
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("object")
        .to_string();
    let fields: Vec<serde_json::Value> = type_definition
        .get("fields")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let editing_id = existing
        .as_ref()
        .and_then(|o| o.get("id").and_then(|v| v.as_i64()));
    let title = create_rw_signal(
        existing
            .as_ref()
            .and_then(|o| o.get("title"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
    );
    let values = create_rw_signal({
        let mut out = std::collections::BTreeMap::new();
        for f in &fields {
            let key = f
                .get("key")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            let raw = existing
                .as_ref()
                .and_then(|o| o.get("properties"))
                .and_then(|p| p.get(&key));
            let field_type = f
                .get("fieldType")
                .and_then(|v| v.as_str())
                .unwrap_or("text");
            let draft = match raw {
                None | Some(serde_json::Value::Null) => String::new(),
                Some(v) if field_type == "boolean" => {
                    if v.as_bool().unwrap_or(false) {
                        "true".into()
                    } else {
                        "false".into()
                    }
                }
                Some(v) => v.to_string().trim_matches('"').to_string(),
            };
            out.insert(key, draft);
        }
        out
    });
    let links = create_rw_signal({
        existing
            .as_ref()
            .and_then(|o| o.get("links"))
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|l| {
                        Some((
                            l.get("target_kind")
                                .and_then(|v| v.as_str())
                                .unwrap_or("customer")
                                .to_string(),
                            l.get("target_local_id")
                                .and_then(|v| v.as_i64())?
                                .to_string(),
                        ))
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    });
    let submitting = create_rw_signal(false);
    let fields_for_submit = fields.clone();

    let can_submit = move || !title.get().trim().is_empty();

    let submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        if submitting.get() || !can_submit() {
            return;
        }
        submitting.set(true);
        let mut properties = serde_json::Map::new();
        for f in &fields_for_submit {
            let key = f.get("key").and_then(|v| v.as_str()).unwrap_or_default();
            let field_type = f
                .get("fieldType")
                .and_then(|v| v.as_str())
                .unwrap_or("text");
            let Some(raw) = values.get().get(key).cloned() else {
                continue;
            };
            if raw.is_empty() {
                continue;
            }
            let value = match field_type {
                "number" => match raw.parse::<f64>() {
                    Ok(n) => serde_json::json!(n),
                    Err(_) => continue,
                },
                "boolean" => serde_json::json!(raw == "true"),
                _ => serde_json::json!(raw),
            };
            properties.insert(key.to_string(), value);
        }
        let links_wire: Vec<serde_json::Value> = links
            .get()
            .iter()
            .filter_map(|(kind, id_str)| {
                let id: i64 = id_str.trim().parse().ok()?;
                Some(serde_json::json!({ "targetKind": kind, "targetLocalId": id }))
            })
            .collect();
        let payload = serde_json::json!({
            "typeId": type_id,
            "title": title.get().trim(),
            "properties": properties,
            "links": links_wire,
        });
        let on_saved = std::rc::Rc::clone(&on_saved);
        let editing_id = editing_id;
        wasm_bindgen_futures::spawn_local(async move {
            let result = match editing_id {
                Some(id) => {
                    let path = format!("/api/custom-objects/{id}");
                    crate::api::patch_json::<serde_json::Value>(&path, &payload).await
                }
                None => {
                    crate::api::post_json::<serde_json::Value>(
                        "/api/custom-objects",
                        Some(&payload),
                    )
                    .await
                }
            };
            match result {
                Ok(r) if r.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) => {
                    crate::toasts::success("Saved.");
                    on_saved();
                }
                Ok(r) => {
                    crate::toasts::error(
                        r.get("message")
                            .and_then(|v| v.as_str())
                            .unwrap_or("Failed."),
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
            <div class="spp-modal spp-modal--form spp-modal--wide">
                <h3 class="spp-modal__title">
                    {if editing_id.is_some() { format!("Edit {type_name}") } else { format!("New {type_name}") }}
                </h3>
                <form on:submit=submit>
                    <div class="spp-form-grid">
                        <label class="spp-form-grid__label">"Title"</label>
                        <input
                            class="spp-input"
                            maxlength=200
                            prop:value=title
                            on:input=move |ev| title.set(event_target_value(&ev))
                        />
                        {fields.iter().map(|f| {
                            let key = f.get("key").and_then(|v| v.as_str()).unwrap_or_default().to_string();
                            let label = f.get("label").and_then(|v| v.as_str()).unwrap_or("").to_string();
                            let field_type = f.get("fieldType").and_then(|v| v.as_str()).unwrap_or("text").to_string();
                            let required = f.get("required").and_then(|v| v.as_bool()).unwrap_or(false);
                            let options: Vec<String> = f
                                .get("options")
                                .and_then(|v| v.as_array())
                                .map(|a| a.iter().filter_map(|o| o.as_str().map(str::to_string)).collect())
                                .unwrap_or_default();
                            let key_bool = key.clone();
                            let key_bool2 = key.clone();
                            let key_select = key.clone();
                            let key_select2 = key.clone();
                            let key_date = key.clone();
                            let key_date2 = key.clone();
                            let key_number = key.clone();
                            let key_number2 = key.clone();
                            let key_text = key.clone();
                            let key_text2 = key.clone();
                            let key_plain = key.clone();
                            let key_plain2 = key.clone();
                            view! {
                                <label class="spp-form-grid__label">
                                    {label}
                                    {if required { " *" } else { "" }}
                                    <span class="spp-text-xs spp-text-muted">{format!(" ({field_type})")}</span>
                                </label>
                                {if field_type == "boolean" {
                                    view! {
                                        <select
                                            class="spp-input"
                                            prop:value=move || values.get().get(&key_bool).cloned().unwrap_or_default()
                                            on:change=move |ev| {
                                                let v = event_target_value(&ev);
                                                values.update(|m| { m.insert(key_bool2.clone(), v); });
                                            }
                                        >
                                            <option value="">"—"</option>
                                            <option value="true">"yes"</option>
                                            <option value="false">"no"</option>
                                        </select>
                                    }.into_view()
                                } else if field_type == "select" {
                                    view! {
                                        <select
                                            class="spp-input"
                                            prop:value=move || values.get().get(&key_select).cloned().unwrap_or_default()
                                            on:change=move |ev| {
                                                let v = event_target_value(&ev);
                                                values.update(|m| { m.insert(key_select2.clone(), v); });
                                            }
                                        >
                                            <option value="">"—"</option>
                                            {options.iter().map(|o| {
                                                view! { <option value=o.clone()>{o.clone()}</option> }
                                            }).collect::<Vec<_>>()}
                                        </select>
                                    }.into_view()
                                } else if field_type == "date" {
                                    view! {
                                        <input
                                            class="spp-input"
                                            type="date"
                                            prop:value=move || values.get().get(&key_date).cloned().unwrap_or_default()
                                            on:input=move |ev| {
                                                let v = event_target_value(&ev);
                                                values.update(|m| { m.insert(key_date2.clone(), v); });
                                            }
                                        />
                                    }.into_view()
                                } else if field_type == "number" {
                                    view! {
                                        <input
                                            class="spp-input"
                                            type="number"
                                            step="any"
                                            prop:value=move || values.get().get(&key_number).cloned().unwrap_or_default()
                                            on:input=move |ev| {
                                                let v = event_target_value(&ev);
                                                values.update(|m| { m.insert(key_number2.clone(), v); });
                                            }
                                        />
                                    }.into_view()
                                } else if field_type == "long_text" {
                                    view! {
                                        <textarea
                                            class="spp-input"
                                            rows=3
                                            prop:value=move || values.get().get(&key_text).cloned().unwrap_or_default()
                                            on:input=move |ev| {
                                                let v = event_target_value(&ev);
                                                values.update(|m| { m.insert(key_text2.clone(), v); });
                                            }
                                        >
                                        </textarea>
                                    }.into_view()
                                } else {
                                    view! {
                                        <input
                                            class="spp-input"
                                            prop:value=move || values.get().get(&key_plain).cloned().unwrap_or_default()
                                            on:input=move |ev| {
                                                let v = event_target_value(&ev);
                                                values.update(|m| { m.insert(key_plain2.clone(), v); });
                                            }
                                        />
                                    }.into_view()
                                }}
                            }
                        }).collect::<Vec<_>>()}
                    </div>
                    <h4 class="spp-card__title spp-mt-4">"Relationships"</h4>
                    <p class="spp-text-xs spp-text-muted">
                        "Link to local ids: customer, organization, conversation, known issue, incident or campaign. (Find ids on the respective pages.)"
                    </p>
                    {move || links
                        .get()
                        .iter()
                        .enumerate()
                        .map(|(i, (kind, id_str))| {
                            let kind = kind.clone();
                            let id_str = id_str.clone();
                            view! {
                                <div class="spp-custom-objects__link-row">
                                    <select
                                        class="spp-input"
                                        prop:value=kind
                                        on:change=move |ev| links.update(|ls| ls[i].0 = event_target_value(&ev))
                                    >
                                        {TARGET_KINDS.iter().map(|k| {
                                            view! { <option value=*k>{k.replace('_', " ")}</option> }
                                        }).collect::<Vec<_>>()}
                                    </select>
                                    <input
                                        class="spp-input"
                                        placeholder="local id"
                                        prop:value=id_str
                                        on:input=move |ev| links.update(|ls| ls[i].1 = event_target_value(&ev))
                                    />
                                    <button
                                        class="spp-button spp-button--ghost spp-button--small"
                                        type="button"
                                        on:click=move |_| links.update(|ls| { ls.remove(i); })
                                    >
                                        "✕"
                                    </button>
                                </div>
                            }
                        })
                        .collect::<Vec<_>>()}
                    <button
                        class="spp-button spp-button--small"
                        type="button"
                        on:click=move |_| links.update(|ls| ls.push(("customer".into(), String::new())))
                    >
                        "+ Add relationship"
                    </button>
                    <div class="spp-modal__actions spp-mt-4">
                        <button class="spp-button spp-button--ghost" type="button" on:click=move |_| on_close()>
                            "Cancel"
                        </button>
                        <button
                            class="spp-button spp-button--primary"
                            type="submit"
                            disabled=move || submitting.get() || !can_submit()
                        >
                            {move || {
                                if submitting.get() {
                                    "Saving…"
                                } else if editing_id.is_some() {
                                    "Save"
                                } else {
                                    "Create"
                                }
                            }}
                        </button>
                    </div>
                </form>
            </div>
        </div>
    }
}

fn format_value(v: Option<&serde_json::Value>) -> String {
    match v {
        None | Some(serde_json::Value::Null) => "—".to_string(),
        Some(serde_json::Value::Bool(b)) => {
            if *b {
                "yes".to_string()
            } else {
                "no".to_string()
            }
        }
        Some(serde_json::Value::Number(n)) => n.to_string(),
        Some(serde_json::Value::String(s)) => s.chars().take(60).collect(),
        Some(other) => other.to_string().chars().take(60).collect(),
    }
}

fn properties_summary(properties: &serde_json::Value) -> String {
    properties
        .as_object()
        .map(|m| {
            m.iter()
                .take(3)
                .map(|(k, v)| {
                    let val: String = v.to_string().chars().take(30).collect();
                    format!("{k}={val}")
                })
                .collect::<Vec<_>>()
                .join(" · ")
        })
        .unwrap_or_default()
}

fn urlencode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*b as char);
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}
