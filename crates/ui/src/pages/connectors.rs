use leptos::*;
use std::sync::Arc;

use crate::components::overlays::ConfirmDialog;
use crate::components::state_view::{EmptyState, LoadingState};

#[component]
pub fn ConnectorsPage() -> impl IntoView {
    let connectors = create_rw_signal(Vec::<serde_json::Value>::new());
    let loading = create_rw_signal(true);
    let error_msg = create_rw_signal(None::<String>);
    let selected = create_rw_signal(None::<i64>);
    let rows = create_rw_signal(Vec::<serde_json::Value>::new());
    let rows_total = create_rw_signal(0i64);
    let rows_loading = create_rw_signal(false);
    let row_query = create_rw_signal(String::new());
    let reload = create_rw_signal(0u32);
    let rows_reload = create_rw_signal(0u32);
    let create_open = create_rw_signal(false);
    let confirm_delete = create_rw_signal(None::<serde_json::Value>);

    create_effect(move |_| {
        let _ = reload.get();
        let connectors = connectors;
        let loading = loading;
        let error_msg = error_msg;
        wasm_bindgen_futures::spawn_local(async move {
            match crate::api::get_json::<serde_json::Value>("/api/connectors").await {
                Ok(data) => {
                    let items = data
                        .get("connectors")
                        .and_then(|v| v.as_array())
                        .cloned()
                        .unwrap_or_default();
                    connectors.set(items);
                    loading.set(false);
                }
                Err(e) => {
                    error_msg.set(Some(e));
                    loading.set(false);
                }
            }
        });
    });

    create_effect(move |_| {
        let _ = rows_reload.get();
        let selected_now = selected.get();
        let q = row_query.get();
        let rows = rows;
        let rows_total = rows_total;
        let rows_loading = rows_loading;
        if selected_now.is_none() {
            rows.set(Vec::new());
            rows_total.set(0);
            return;
        }
        rows_loading.set(true);
        wasm_bindgen_futures::spawn_local(async move {
            let path = match selected_now {
                Some(id) => format!("/api/connectors/{id}/rows?q={}", urlencode(&q)),
                None => return,
            };
            match crate::api::get_json::<serde_json::Value>(&path).await {
                Ok(data) => {
                    rows.set(
                        data.get("rows")
                            .and_then(|v| v.as_array())
                            .cloned()
                            .unwrap_or_default(),
                    );
                    rows_total.set(data.get("total").and_then(|v| v.as_i64()).unwrap_or(0));
                }
                Err(_) => {
                    rows.set(Vec::new());
                    rows_total.set(0);
                }
            }
            rows_loading.set(false);
        });
    });

    let active = move || {
        let id = selected.get()?;
        connectors
            .get()
            .into_iter()
            .find(|c| c.get("id").and_then(|v| v.as_i64()) == Some(id))
    };

    let refresh_now = move |id: i64| {
        wasm_bindgen_futures::spawn_local(async move {
            let path = format!("/api/connectors/{id}/refresh");
            match crate::api::post_json::<serde_json::Value>(&path, None).await {
                Ok(r) => {
                    let ok = r.get("ok").and_then(|v| v.as_bool()).unwrap_or(false);
                    if ok {
                        let result = r.get("result").cloned().unwrap_or(serde_json::json!({}));
                        crate::toasts::success(format!(
                            "Refreshed: {} rows ({} pruned).",
                            result.get("rows").and_then(|v| v.as_i64()).unwrap_or(0),
                            result.get("pruned").and_then(|v| v.as_i64()).unwrap_or(0),
                        ));
                    } else {
                        crate::toasts::error(
                            r.get("message")
                                .and_then(|v| v.as_str())
                                .unwrap_or("Refresh failed."),
                        );
                    }
                }
                Err(e) => crate::toasts::error(e),
            }
            reload.update(|n| *n = n.wrapping_add(1));
            rows_reload.update(|n| *n = n.wrapping_add(1));
        });
    };

    let toggle_ai = move |id: i64, allowed: bool| {
        wasm_bindgen_futures::spawn_local(async move {
            let path = format!("/api/connectors/{id}");
            match crate::api::patch_json::<serde_json::Value>(
                &path,
                &serde_json::json!({ "allowedAi": allowed }),
            )
            .await
            {
                Ok(r) if r.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) => {
                    crate::toasts::success("AI visibility updated.");
                }
                Ok(r) => crate::toasts::error(
                    r.get("message")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Update failed."),
                ),
                Err(e) => crate::toasts::error(e),
            }
            reload.update(|n| *n = n.wrapping_add(1));
        });
    };

    let toggle_enabled = move |id: i64, enabled: bool| {
        wasm_bindgen_futures::spawn_local(async move {
            let path = format!("/api/connectors/{id}");
            match crate::api::patch_json::<serde_json::Value>(
                &path,
                &serde_json::json!({ "enabled": enabled }),
            )
            .await
            {
                Ok(r) if r.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) => {
                    crate::toasts::success("Connector updated.");
                }
                Ok(r) => crate::toasts::error(
                    r.get("message")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Update failed."),
                ),
                Err(e) => crate::toasts::error(e),
            }
            reload.update(|n| *n = n.wrapping_add(1));
        });
    };

    let delete_connector = move |row: serde_json::Value| {
        let Some(id) = row.get("id").and_then(|v| v.as_i64()) else {
            return;
        };
        wasm_bindgen_futures::spawn_local(async move {
            let path = format!("/api/connectors/{id}");
            match crate::api::delete_json::<serde_json::Value>(&path).await {
                Ok(r) if r.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) => {
                    crate::toasts::success("Connector deleted.");
                    confirm_delete.set(None);
                    selected.set(None);
                }
                Ok(r) => {
                    crate::toasts::error(
                        r.get("message")
                            .and_then(|v| v.as_str())
                            .unwrap_or("Delete failed."),
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
        <div class="spp-page spp-page--connectors">
            <header class="spp-page__header">
                <div>
                    <h2 class="spp-page__title">"Connectors"</h2>
                    <p class="spp-page__subtitle">
                        "Approved local data sources · HTTP targets are SSRF-guarded (private networks, localhost and metadata endpoints refused) · AI sees only explicitly allowed data"
                    </p>
                </div>
                <div class="spp-page__header-actions">
                    <button
                        class="spp-button spp-button--primary"
                        on:click=move |_| create_open.set(true)
                    >
                        "+ Add connector"
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
                    when=move || !connectors.with(|c| c.is_empty())
                    fallback=|| {
                        view! {
                            <EmptyState message="No connectors yet. Connect a local JSON file, a CSV, a SQLite database or an HTTP endpoint. Files live in the connectors/ folder of the project; HTTP targets must be public (SSRF-guarded)." />
                        }
                    }
                >
                    <table class="spp-table">
                        <thead>
                            <tr>
                                <th>"Name"</th>
                                <th>"Kind"</th>
                                <th>"Source"</th>
                                <th>"Health"</th>
                                <th>"Rows"</th>
                                <th>"AI visibility"</th>
                                <th>"Last sync"</th>
                                <th></th>
                            </tr>
                        </thead>
                        <tbody>
                            {move || connectors.with(|items| {
                                items.iter().map(|c| {
                                    let row = c.clone();
                                    let id = row.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
                                    let name = str_field(&row, "name");
                                    let kind = str_field(&row, "kind");
                                    let enabled = row.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true);
                                    let allowed_ai = row.get("allowed_ai").and_then(|v| v.as_bool()).unwrap_or(false);
                                    let health = str_field(&row, "health");
                                    let last_sync_error = str_field(&row, "last_sync_error");
                                    let row_count = row.get("row_count").and_then(|v| v.as_i64()).unwrap_or(0);
                                    let last_sync_at = str_field(&row, "last_sync_at");
                                    let refresh_method = str_field(&row, "refresh_method");
                                    let refresh_seconds = row.get("refresh_seconds").and_then(|v| v.as_i64()).unwrap_or(3600);
                                    let is_selected = move || selected.get() == Some(id);
                                    view! {
                                        <tr
                                            class="spp-table__row--clickable"
                                            class:is-selected=is_selected
                                            on:click=move |_| {
                                                selected.update(|s| {
                                                    *s = if *s == Some(id) { None } else { Some(id) };
                                                });
                                                rows_reload.update(|n| *n = n.wrapping_add(1));
                                            }
                                        >
                                            <td>
                                                <strong class="spp-text-sm">{name.clone()}</strong>
                                                {if !enabled {
                                                    view! {
                                                        <span class="spp-badge spp-badge--warn" title="disabled">"disabled"</span>
                                                    }.into_view()
                                                } else {
                                                    ().into_view()
                                                }}
                                            </td>
                                            <td>
                                                <span class=format!("spp-badge {}", kind_badge_class(&kind))>
                                                    {kind.replace('_', " ")}
                                                </span>
                                            </td>
                                            <td class="spp-table__cell-muted spp-text-xs">{source_label(&row)}</td>
                                            <td>
                                                <span class=format!("spp-badge {}", health_badge_class(&health))>{health.clone()}</span>
                                                {if health == "error" && !last_sync_error.is_empty() {
                                                    let truncated: String = last_sync_error.chars().take(60).collect();
                                                    view! {
                                                        <div class="spp-text-xs spp-text-err" title=last_sync_error.clone()>{truncated}</div>
                                                    }.into_view()
                                                } else {
                                                    ().into_view()
                                                }}
                                            </td>
                                            <td><span class="spp-badge">{row_count.to_string()}</span></td>
                                            <td>
                                                <button
                                                    class=move || format!(
                                                        "spp-button spp-button--small {}",
                                                        if allowed_ai { "spp-button--primary" } else { "spp-button--ghost" }
                                                    )
                                                    title=move || if allowed_ai {
                                                        "AI (Local Copilot) may search this connector".to_string()
                                                    } else {
                                                        "AI may NOT see this data - explicit allow required".to_string()
                                                    }
                                                    on:click=move |ev| {
                                                        ev.stop_propagation();
                                                        toggle_ai(id, !allowed_ai);
                                                    }
                                                >
                                                    {if allowed_ai { "allowed" } else { "private" }}
                                                </button>
                                            </td>
                                            <td class="spp-text-xs">
                                                {if last_sync_at.is_empty() { "never".to_string() } else { last_sync_at.clone() }}
                                                {if refresh_method == "interval" {
                                                    view! {
                                                        <span class="spp-text-muted">
                                                            {format!(" · every {}m", (refresh_seconds / 60).max(1))}
                                                        </span>
                                                    }.into_view()
                                                } else {
                                                    ().into_view()
                                                }}
                                            </td>
                                            <td>
                                                <div class="spp-connectors__row-actions">
                                                    <button
                                                        class="spp-button spp-button--small"
                                                        title="Refresh now"
                                                        on:click=move |ev| {
                                                            ev.stop_propagation();
                                                            refresh_now(id);
                                                        }
                                                    >
                                                        "⟳"
                                                    </button>
                                                    <button
                                                        class="spp-button spp-button--small spp-button--ghost"
                                                        title=move || if enabled { "Disable".to_string() } else { "Enable".to_string() }
                                                        on:click=move |ev| {
                                                            ev.stop_propagation();
                                                            toggle_enabled(id, !enabled);
                                                        }
                                                    >
                                                        {if enabled { "disable" } else { "enable" }}
                                                    </button>
                                                    <button
                                                        class="spp-button spp-button--small spp-button--ghost"
                                                        title="Delete"
                                                        on:click=move |ev| {
                                                            ev.stop_propagation();
                                                            confirm_delete.set(Some(row.clone()));
                                                        }
                                                    >
                                                        "✕"
                                                    </button>
                                                </div>
                                            </td>
                                        </tr>
                                    }
                                }).collect::<Vec<_>>()
                            })}
                        </tbody>
                    </table>
                </Show>

                {move || {
                    let Some(active_row) = active() else {
                        return ().into_view();
                    };
                    let name = str_field(&active_row, "name");
                    let row_count = active_row.get("row_count").and_then(|v| v.as_i64()).unwrap_or(0);
                    let schema_json = active_row.get("schema_json").and_then(|v| v.as_array()).cloned().unwrap_or_default();
                    let auth_mode = active_row
                        .get("auth")
                        .and_then(|a| a.get("mode"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("none")
                        .to_string();
                    let columns: Vec<String> = schema_columns(&active_row);
                    let schema_line = schema_json
                        .iter()
                        .filter_map(|s| {
                            let n = s.get("name").and_then(|v| v.as_str())?;
                            let t = s.get("type").and_then(|v| v.as_str()).unwrap_or("?");
                            Some(format!("{n} ({t})"))
                        })
                        .collect::<Vec<_>>()
                        .join(" · ");
                    let rows_loading_view = rows_loading;
                    let rows_view = rows;
                    let rows_total_view = rows_total;
                    view! {
                        <div class="spp-card spp-mt-4">
                            <div class="spp-flex-between">
                                <div>
                                    <h3 class="spp-card__title">
                                        {name} " — cached rows"
                                    </h3>
                                    <p class="spp-text-xs spp-text-muted">
                                        {format!("{row_count} rows · schema inferred from the source · snapshot semantics: vanished rows are pruned on refresh")}
                                    </p>
                                </div>
                                <form
                                    class="spp-connectors__filter"
                                    on:submit=move |ev| {
                                        ev.prevent_default();
                                        rows_reload.update(|n| *n = n.wrapping_add(1));
                                    }
                                >
                                    <input
                                        class="spp-input"
                                        type="text"
                                        placeholder="Filter rows…"
                                        aria-label="Filter connector rows"
                                        prop:value=row_query
                                        on:input=move |ev| row_query.set(event_target_value(&ev))
                                    />
                                    <button class="spp-button" type="submit">"Go"</button>
                                </form>
                            </div>
                            {if !schema_line.is_empty() {
                                view! {
                                    <p class="spp-text-xs spp-text-muted">{format!("Inferred schema: {schema_line}")}</p>
                                }.into_view()
                            } else {
                                ().into_view()
                            }}
                            <Show when=move || rows_loading_view.get() fallback=|| ()>
                                <LoadingState />
                            </Show>
                            <Show
                                when=move || !rows_loading_view.get() && !rows_view.get().is_empty()
                                fallback=|| {
                                    view! {
                                        <EmptyState message="No rows cached. Refresh the connector to pull its first snapshot." />
                                    }
                                }
                            >
                                <div class="spp-table-scroll">
                                    <table class="spp-table">
                                        <thead>
                                            <tr>
                                                <th>"key"</th>
                                                {columns.iter().map(|col| {
                                                    view! { <th>{col.clone()}</th> }
                                                }).collect::<Vec<_>>()}
                                                <th>"fetched"</th>
                                            </tr>
                                        </thead>
                                        <tbody>
                                            {move || {
                                                let cols = active()
                                                    .map(|row| schema_columns(&row))
                                                    .unwrap_or_default();
                                                rows_view.get().iter().map(|r| {
                                                let row = r.clone();
                                                let row_key = str_field(&row, "row_key");
                                                let key_short: String = row_key.chars().take(24).collect();
                                                let fetched = str_field(&row, "fetched_at");
                                                let data = row.get("data").cloned().unwrap_or(serde_json::Value::Null);
                                                view! {
                                                    <tr>
                                                        <td class="spp-text-xs">{key_short}</td>
                                                        {cols.iter().map(|col| {
                                                            let v = data.get(col).map(|v| v.to_string()).unwrap_or_default();
                                                            let shown: String = v.chars().take(60).collect();
                                                            view! { <td class="spp-text-sm">{shown}</td> }
                                                        }).collect::<Vec<_>>()}
                                                        <td class="spp-text-xs">{fetched}</td>
                                                    </tr>
                                                }
                                                }).collect::<Vec<_>>()
                                            }}
                                        </tbody>
                                    </table>
                                </div>
                                <p class="spp-text-xs spp-text-muted">
                                    {move || format!("{} of {} rows", rows_view.get().len(), rows_total_view.get())}
                                </p>
                            </Show>
                            {if auth_mode != "none" {
                                view! {
                                    <p class="spp-text-xs spp-text-muted">
                                        {format!("Auth material ({auth_mode}) is stored locally only and always redacted in API responses.")}
                                    </p>
                                }.into_view()
                            } else {
                                ().into_view()
                            }}
                        </div>
                    }.into_view()
                }}
            </Show>

            <Show when=move || create_open.get() fallback=|| ()>
                <ConnectorCreateModal
                    on_close=std::rc::Rc::new(move || create_open.set(false))
                    on_saved=std::rc::Rc::new(move || {
                        create_open.set(false);
                        reload.update(|n| *n = n.wrapping_add(1));
                    })
                />
            </Show>

            {move || {
                if let Some(row) = confirm_delete.get() {
                    let name = str_field(&row, "name");
                    let on_confirm = {
                        let row = row.clone();
                        Arc::new(move || delete_connector(row.clone()))
                    };
                    let on_cancel = Arc::new(move || confirm_delete.set(None));
                    view! {
                        <ConfirmDialog
                            title="Delete connector"
                            message=format!("Delete \"{name}\" and its cached rows? The source file/endpoint itself is untouched.")
                            confirm_label="Delete"
                            danger=true
                            on_confirm
                            on_cancel
                        />
                    }.into_view()
                } else {
                    ().into_view()
                }
            }}
        </div>
    }
}

#[component]
fn ConnectorCreateModal(
    on_close: std::rc::Rc<dyn Fn()>,
    on_saved: std::rc::Rc<dyn Fn()>,
) -> impl IntoView {
    let name = create_rw_signal(String::new());
    let kind = create_rw_signal("local_json".to_string());
    let file = create_rw_signal(String::new());
    let url = create_rw_signal(String::new());
    let table = create_rw_signal(String::new());
    let key_column = create_rw_signal(String::new());
    let auth_mode = create_rw_signal("none".to_string());
    let header_name = create_rw_signal(String::new());
    let header_value = create_rw_signal(String::new());
    let bearer_token = create_rw_signal(String::new());
    let refresh_method = create_rw_signal("manual".to_string());
    let refresh_seconds = create_rw_signal(3600i64);
    let allowed_ai = create_rw_signal(false);
    let submitting = create_rw_signal(false);

    let can_submit = move || {
        !name.get().trim().is_empty()
            && (if kind.get() == "http" {
                !url.get().trim().is_empty()
            } else {
                !file.get().trim().is_empty()
            })
    };

    let submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        if submitting.get() || !can_submit() {
            return;
        }
        submitting.set(true);
        let kind_now = kind.get();
        let mut config = if kind_now == "http" {
            serde_json::json!({ "kind": kind_now, "url": url.get().trim() })
        } else if kind_now == "sqlite" {
            serde_json::json!({ "kind": kind_now, "file": file.get().trim(), "table": table.get().trim() })
        } else {
            serde_json::json!({ "kind": kind_now, "file": file.get().trim() })
        };
        let kc = key_column.get().trim().to_string();
        if !kc.is_empty() {
            config["keyColumn"] = serde_json::json!(kc);
        }
        let auth = match auth_mode.get().as_str() {
            "header" => serde_json::json!({
                "mode": "header",
                "headerName": header_name.get().trim(),
                "headerValue": header_value.get().trim(),
            }),
            "bearer" => serde_json::json!({
                "mode": "bearer",
                "token": bearer_token.get().trim(),
            }),
            _ => serde_json::json!({ "mode": "none" }),
        };
        let payload = serde_json::json!({
            "name": name.get().trim(),
            "config": config,
            "auth": auth,
            "refreshMethod": refresh_method.get(),
            "refreshSeconds": refresh_seconds.get(),
            "allowedAi": allowed_ai.get(),
        });
        let on_saved = std::rc::Rc::clone(&on_saved);
        wasm_bindgen_futures::spawn_local(async move {
            match crate::api::post_json::<serde_json::Value>("/api/connectors", Some(&payload))
                .await
            {
                Ok(r) => {
                    let ok = r.get("ok").and_then(|v| v.as_bool()).unwrap_or(false);
                    if ok {
                        crate::toasts::success("Connector created.");
                        if let Some(id) = r
                            .get("connector")
                            .and_then(|c| c.get("id"))
                            .and_then(|v| v.as_i64())
                        {
                            let path = format!("/api/connectors/{id}/refresh");
                            if let Ok(rr) =
                                crate::api::post_json::<serde_json::Value>(&path, None).await
                            {
                                if rr.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
                                    crate::toasts::success("First refresh completed.");
                                }
                            }
                        }
                        on_saved();
                    } else {
                        crate::toasts::error(
                            r.get("message")
                                .and_then(|v| v.as_str())
                                .unwrap_or("Failed."),
                        );
                        submitting.set(false);
                    }
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
                <h3 class="spp-modal__title">"Add connector"</h3>
                <form on:submit=submit>
                    <div class="spp-form-grid">
                        <label class="spp-form-grid__label">"Name"</label>
                        <input
                            class="spp-input"
                            maxlength=80
                            placeholder="e.g. Product releases"
                            prop:value=name
                            on:input=move |ev| name.set(event_target_value(&ev))
                        />
                        <label class="spp-form-grid__label">"Kind"</label>
                        <select
                            class="spp-input"
                            prop:value=kind
                            on:change=move |ev| kind.set(event_target_value(&ev))
                        >
                            <option value="local_json">"local JSON file"</option>
                            <option value="csv">"CSV file"</option>
                            <option value="sqlite">"SQLite database"</option>
                            <option value="http">"HTTP endpoint"</option>
                        </select>
                        <Show
                            when=move || kind.get() != "http"
                            fallback=|| ()
                        >
                            <label class="spp-form-grid__label">"File (inside the connectors/ folder)"</label>
                            <input
                                class="spp-input"
                                placeholder="e.g. product-releases.json"
                                prop:value=file
                                on:input=move |ev| file.set(event_target_value(&ev))
                            />
                        </Show>
                        <Show
                            when=move || kind.get() == "http"
                            fallback=|| ()
                        >
                            <label class="spp-form-grid__label">"URL (public only — SSRF-guarded)"</label>
                            <input
                                class="spp-input"
                                placeholder="https://api.example.com/releases"
                                prop:value=url
                                on:input=move |ev| url.set(event_target_value(&ev))
                            />
                        </Show>
                        <Show
                            when=move || kind.get() == "sqlite"
                            fallback=|| ()
                        >
                            <label class="spp-form-grid__label">"Table"</label>
                            <input
                                class="spp-input"
                                placeholder="releases"
                                prop:value=table
                                on:input=move |ev| table.set(event_target_value(&ev))
                            />
                        </Show>
                        <label class="spp-form-grid__label">"Key column (optional — stable row identity)"</label>
                        <input
                            class="spp-input"
                            placeholder="e.g. version"
                            prop:value=key_column
                            on:input=move |ev| key_column.set(event_target_value(&ev))
                        />
                        <label class="spp-form-grid__label">"Authentication"</label>
                        <select
                            class="spp-input"
                            prop:value=auth_mode
                            on:change=move |ev| auth_mode.set(event_target_value(&ev))
                        >
                            <option value="none">"none"</option>
                            <option value="header">"header"</option>
                            <option value="bearer">"bearer token"</option>
                        </select>
                        <Show
                            when=move || auth_mode.get() == "header"
                            fallback=|| ()
                        >
                            <label class="spp-form-grid__label">"Header name"</label>
                            <input
                                class="spp-input"
                                placeholder="X-API-Key"
                                prop:value=header_name
                                on:input=move |ev| header_name.set(event_target_value(&ev))
                            />
                            <label class="spp-form-grid__label">"Header value"</label>
                            <input
                                class="spp-input"
                                type="password"
                                prop:value=header_value
                                on:input=move |ev| header_value.set(event_target_value(&ev))
                            />
                        </Show>
                        <Show
                            when=move || auth_mode.get() == "bearer"
                            fallback=|| ()
                        >
                            <label class="spp-form-grid__label">"Bearer token"</label>
                            <input
                                class="spp-input"
                                type="password"
                                prop:value=bearer_token
                                on:input=move |ev| bearer_token.set(event_target_value(&ev))
                            />
                        </Show>
                        <label class="spp-form-grid__label">"Refresh"</label>
                        <div class="spp-connectors__refresh-row">
                            <select
                                class="spp-input"
                                prop:value=refresh_method
                                on:change=move |ev| refresh_method.set(event_target_value(&ev))
                            >
                                <option value="manual">"manual"</option>
                                <option value="interval">"interval"</option>
                            </select>
                            <Show
                                when=move || refresh_method.get() == "interval"
                                fallback=|| ()
                            >
                                <input
                                    class="spp-input"
                                    type="number"
                                    min=60
                                    max=86400
                                    title="Seconds between automatic refreshes (min 60)"
                                    prop:value=move || refresh_seconds.get().to_string()
                                    on:input=move |ev| {
                                        refresh_seconds.set(
                                            event_target_value(&ev).parse::<i64>().unwrap_or(3600)
                                        );
                                    }
                                />
                            </Show>
                        </div>
                        <label class="spp-form-grid__label">"AI visibility"</label>
                        <label class="spp-connectors__checkbox">
                            <input
                                type="checkbox"
                                prop:checked=allowed_ai
                                on:change=move |ev| allowed_ai.set(event_target_checked(&ev))
                            />
                            <span class="spp-text-sm">"Allow the Local Copilot to search this connector's data"</span>
                        </label>
                    </div>
                    <p class="spp-text-xs spp-text-muted">
                        "File connectors read only from the project's connectors/ folder (path-jail). HTTP connectors are refused for localhost, private ranges and cloud metadata endpoints — and DNS-resolved addresses are re-checked before every request. Auth material stays in the local database, redacted in every read."
                    </p>
                    <div class="spp-modal__actions spp-mt-4">
                        <button class="spp-button spp-button--ghost" type="button" on:click=move |_| on_close()>
                            "Cancel"
                        </button>
                        <button
                            class="spp-button spp-button--primary"
                            type="submit"
                            disabled=move || submitting.get() || !can_submit()
                        >
                            {move || if submitting.get() { "Adding…" } else { "Add connector" }}
                        </button>
                    </div>
                </form>
            </div>
        </div>
    }
}

fn schema_columns(c: &serde_json::Value) -> Vec<String> {
    c.get("schema_json")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .take(6)
                .filter_map(|s| s.get("name").and_then(|v| v.as_str()).map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

fn source_label(c: &serde_json::Value) -> String {
    let config = c.get("config").cloned().unwrap_or(serde_json::Value::Null);
    if c.get("kind").and_then(|v| v.as_str()) == Some("http") {
        return str_field(&config, "url");
    }
    let file = str_field(&config, "file");
    if c.get("kind").and_then(|v| v.as_str()) == Some("sqlite") {
        let table = str_field(&config, "table");
        if table.is_empty() {
            file
        } else {
            format!("{file} · {table}")
        }
    } else {
        file
    }
}

fn kind_badge_class(kind: &str) -> &'static str {
    match kind {
        "local_json" => "spp-badge--ok",
        "sqlite" => "spp-badge--warn",
        "http" => "spp-badge--ai",
        _ => "",
    }
}

fn health_badge_class(health: &str) -> &'static str {
    match health {
        "ok" => "spp-badge--ok",
        "error" => "spp-badge--err",
        _ => "",
    }
}

fn str_field(v: &serde_json::Value, key: &str) -> String {
    v.get(key)
        .and_then(|f| f.as_str())
        .unwrap_or_default()
        .to_string()
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
