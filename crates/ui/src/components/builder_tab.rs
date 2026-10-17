use leptos::*;

use crate::components::state_view::{EmptyState, LoadingState};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct MetricEntry {
    pub key: String,
    pub label: String,
    pub definition: String,
    pub limitations: String,
    pub format: String,
    pub needs_attribute: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DimensionEntry {
    pub key: String,
    pub label: String,
    pub definition: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct BuilderCatalog {
    pub metrics: Vec<MetricEntry>,
    pub dimensions: Vec<DimensionEntry>,
    pub note: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SavedReportRow {
    pub id: i64,
    pub name: String,
    pub config: serde_json::Value,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ResultRow {
    pub dimension_value: String,
    pub dimension_label: String,
    pub value: f64,
    pub sample_conversation_ids: Vec<i64>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct BuilderResult {
    pub metric_key: String,
    pub metric_label: String,
    pub metric_format: String,
    pub dimension_key: String,
    pub dimension_label: String,
    pub date_from: String,
    pub date_to: String,
    pub comparison_from: Option<String>,
    pub comparison_to: Option<String>,
    pub rows: Vec<ResultRow>,
    pub comparison_rows: Option<Vec<ResultRow>>,
    pub notes: Vec<String>,
}

pub const TOTAL_ONLY_KEYS: [&str; 5] = [
    "campaign_sent",
    "campaign_replies",
    "campaign_reply_rate",
    "avg_state_hours",
    "state_changes",
];

#[must_use]
pub fn is_total_only(key: &str, catalog: &BuilderCatalog) -> bool {
    catalog
        .metrics
        .iter()
        .any(|m| m.key == key && m.needs_attribute)
        || TOTAL_ONLY_KEYS.contains(&key)
}

#[must_use]
pub fn fmt_value(v: f64, format: &str) -> String {
    match format {
        "rate" => format!("{:.1}%", v * 100.0),
        "minutes" => format!("{} min", v.round() as i64),
        "hours" => format!("{} h", v.round() as i64),
        "score" => format!("{v:.1}"),
        _ => format!("{}", v.round() as i64),
    }
}

#[must_use]
pub fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[must_use]
pub fn iso_from_epoch_ms(ms: f64) -> String {
    let days = (ms / 86_400_000.0).floor() as i64;
    let (y, m, d) = civil_from_days(days);
    format!("{y:04}-{m:02}-{d:02}")
}

#[must_use]
pub fn iso_days_ago(days: i64) -> String {
    iso_from_epoch_ms(js_sys::Date::now() - (days as f64) * 86_400_000.0)
}

#[must_use]
pub fn parse_catalog(v: &serde_json::Value) -> BuilderCatalog {
    BuilderCatalog {
        metrics: v
            .get("metrics")
            .and_then(|m| m.as_array())
            .map(|rows| {
                rows.iter()
                    .map(|m| MetricEntry {
                        key: m
                            .get("key")
                            .and_then(|x| x.as_str())
                            .unwrap_or_default()
                            .to_string(),
                        label: m
                            .get("label")
                            .and_then(|x| x.as_str())
                            .unwrap_or_default()
                            .to_string(),
                        definition: m
                            .get("definition")
                            .and_then(|x| x.as_str())
                            .unwrap_or_default()
                            .to_string(),
                        limitations: m
                            .get("limitations")
                            .and_then(|x| x.as_str())
                            .unwrap_or_default()
                            .to_string(),
                        format: m
                            .get("format")
                            .and_then(|x| x.as_str())
                            .unwrap_or("count")
                            .to_string(),
                        needs_attribute: m
                            .get("needsAttribute")
                            .and_then(|x| x.as_bool())
                            .unwrap_or(false),
                    })
                    .collect()
            })
            .unwrap_or_default(),
        dimensions: v
            .get("dimensions")
            .and_then(|d| d.as_array())
            .map(|rows| {
                rows.iter()
                    .map(|d| DimensionEntry {
                        key: d
                            .get("key")
                            .and_then(|x| x.as_str())
                            .unwrap_or_default()
                            .to_string(),
                        label: d
                            .get("label")
                            .and_then(|x| x.as_str())
                            .unwrap_or_default()
                            .to_string(),
                        definition: d
                            .get("definition")
                            .and_then(|x| x.as_str())
                            .unwrap_or_default()
                            .to_string(),
                    })
                    .collect()
            })
            .unwrap_or_default(),
        note: v
            .get("note")
            .and_then(|x| x.as_str())
            .unwrap_or_default()
            .to_string(),
    }
}

#[must_use]
pub fn parse_saved(v: &serde_json::Value) -> Vec<SavedReportRow> {
    v.get("saved")
        .and_then(|s| s.as_array())
        .map(|rows| {
            rows.iter()
                .map(|s| SavedReportRow {
                    id: s.get("id").and_then(|x| x.as_i64()).unwrap_or(0),
                    name: s
                        .get("name")
                        .and_then(|x| x.as_str())
                        .unwrap_or_default()
                        .to_string(),
                    config: s.get("config").cloned().unwrap_or(serde_json::Value::Null),
                })
                .collect()
        })
        .unwrap_or_default()
}

#[must_use]
pub fn parse_result(v: &serde_json::Value) -> BuilderResult {
    BuilderResult {
        metric_key: v
            .pointer("/metric/key")
            .and_then(|x| x.as_str())
            .unwrap_or_default()
            .to_string(),
        metric_label: v
            .pointer("/metric/label")
            .and_then(|x| x.as_str())
            .unwrap_or_default()
            .to_string(),
        metric_format: v
            .pointer("/metric/format")
            .and_then(|x| x.as_str())
            .unwrap_or("count")
            .to_string(),
        dimension_key: v
            .pointer("/dimension/key")
            .and_then(|x| x.as_str())
            .unwrap_or_default()
            .to_string(),
        dimension_label: v
            .pointer("/dimension/label")
            .and_then(|x| x.as_str())
            .unwrap_or_default()
            .to_string(),
        date_from: v
            .pointer("/date_range/dateFrom")
            .and_then(|x| x.as_str())
            .unwrap_or_default()
            .to_string(),
        date_to: v
            .pointer("/date_range/dateTo")
            .and_then(|x| x.as_str())
            .unwrap_or_default()
            .to_string(),
        comparison_from: v
            .pointer("/comparison_range/dateFrom")
            .and_then(|x| x.as_str())
            .map(str::to_string),
        comparison_to: v
            .pointer("/comparison_range/dateTo")
            .and_then(|x| x.as_str())
            .map(str::to_string),
        rows: v
            .get("rows")
            .and_then(|r| r.as_array())
            .map(|rows| rows.iter().map(parse_result_row).collect())
            .unwrap_or_default(),
        comparison_rows: v
            .get("comparison_rows")
            .and_then(|c| c.as_array())
            .map(|rows| rows.iter().map(parse_result_row).collect()),
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

fn parse_result_row(r: &serde_json::Value) -> ResultRow {
    ResultRow {
        dimension_value: r
            .get("dimension_value")
            .and_then(|x| x.as_str())
            .unwrap_or_default()
            .to_string(),
        dimension_label: r
            .get("dimension_label")
            .and_then(|x| x.as_str())
            .unwrap_or_default()
            .to_string(),
        value: r.get("value").and_then(|x| x.as_f64()).unwrap_or(0.0),
        sample_conversation_ids: r
            .get("sample_conversation_ids")
            .and_then(|s| s.as_array())
            .map(|rows| rows.iter().filter_map(|x| x.as_i64()).collect())
            .unwrap_or_default(),
    }
}

#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn config_body(
    metric: &str,
    dimension: &str,
    date_from: &str,
    date_to: &str,
    comparison: &str,
    sort: &str,
    limit: i64,
    attribute_key: &str,
    state_key: &str,
) -> serde_json::Value {
    let filters = if !attribute_key.is_empty() {
        serde_json::json!({ "attributeKey": attribute_key, "attributeValue": "" })
    } else if !state_key.is_empty() {
        serde_json::json!({ "stateKey": state_key })
    } else {
        serde_json::json!({})
    };
    let mut v = serde_json::json!({
        "metric": metric,
        "dimension": dimension,
        "dateFrom": date_from,
        "dateTo": date_to,
        "comparison": comparison,
        "sort": sort,
        "limit": limit,
        "filters": filters,
    });
    if filters.as_object().is_some_and(|f| f.is_empty()) {
        v["filters"] = serde_json::json!({});
    }
    v
}

pub const ATTRIBUTE_KEYS: [&str; 10] = [
    "intent",
    "urgency",
    "frustration_cues",
    "technical_familiarity",
    "question_count",
    "escalation_signal",
    "response_style",
    "known_issue",
    "issue_cluster",
    "customer_goal",
];

pub const STATE_KEYS: [&str; 6] = [
    "new",
    "investigating",
    "waiting-customer",
    "waiting-engineering",
    "ready-verify",
    "resolved",
];

#[component]
pub fn BuilderTab() -> impl IntoView {
    let catalog = create_rw_signal(None::<BuilderCatalog>);
    let catalog_error = create_rw_signal(false);
    let saved = create_rw_signal(Vec::<SavedReportRow>::new());
    let result = create_rw_signal(None::<BuilderResult>);
    let running = create_rw_signal(false);
    let notice = create_rw_signal(None::<(String, String)>);

    let metric = create_rw_signal("conversations".to_string());
    let dimension = create_rw_signal("day".to_string());
    let date_from = create_rw_signal(iso_days_ago(30));
    let date_to = create_rw_signal(iso_days_ago(0));
    let comparison = create_rw_signal("previous_period".to_string());
    let sort = create_rw_signal("dimension_asc".to_string());
    let limit = create_rw_signal(30i64);
    let attribute_key = create_rw_signal(String::new());
    let state_key = create_rw_signal(String::new());
    let name = create_rw_signal(String::new());
    let chart_bar = create_rw_signal(true);

    let load_saved = {
        move || {
            let saved = saved;
            spawn_local(async move {
                match crate::api::get_json::<serde_json::Value>("/api/reports/builder/saved").await
                {
                    Ok(v) => saved.set(parse_saved(&v)),
                    Err(e) => notice.set(Some(("err".to_string(), e))),
                }
            });
        }
    };

    let load_catalog = {
        move || {
            let catalog = catalog;
            let catalog_error = catalog_error;
            spawn_local(async move {
                match crate::api::get_json::<serde_json::Value>("/api/reports/builder/catalog")
                    .await
                {
                    Ok(v) => {
                        catalog_error.set(false);
                        catalog.set(Some(parse_catalog(&v)));
                    }
                    Err(_) => catalog_error.set(true),
                }
            });
        }
    };
    load_catalog();
    load_saved();

    let run_config = move |cfg: serde_json::Value| {
        if running.get_untracked() {
            return;
        }
        running.set(true);
        result.set(None);
        let running = running;
        let result = result;
        let notice = notice;
        spawn_local(async move {
            match crate::api::post_json::<serde_json::Value>("/api/reports/builder/run", Some(&cfg))
                .await
            {
                Ok(v) => result.set(Some(parse_result(&v))),
                Err(e) => notice.set(Some(("err".to_string(), e))),
            }
            running.set(false);
        });
    };

    let run_from_form = move |_| {
        let cfg = config_body(
            &metric.get_untracked(),
            &dimension.get_untracked(),
            &date_from.get_untracked(),
            &date_to.get_untracked(),
            &comparison.get_untracked(),
            &sort.get_untracked(),
            limit.get_untracked(),
            &attribute_key.get_untracked(),
            &state_key.get_untracked(),
        );
        run_config(cfg);
    };

    let save = move |_| {
        let name_v = name.get_untracked();
        if name_v.trim().is_empty() {
            return;
        }
        let mut cfg = config_body(
            &metric.get_untracked(),
            &dimension.get_untracked(),
            &date_from.get_untracked(),
            &date_to.get_untracked(),
            &comparison.get_untracked(),
            &sort.get_untracked(),
            limit.get_untracked(),
            &attribute_key.get_untracked(),
            &state_key.get_untracked(),
        );
        cfg["name"] = serde_json::json!(name_v);
        let notice = notice;
        let name = name;
        let load_saved = load_saved;
        spawn_local(async move {
            match crate::api::post_json::<serde_json::Value>(
                "/api/reports/builder/saved",
                Some(&cfg),
            )
            .await
            {
                Ok(_) => {
                    notice.set(Some((
                        "ok".to_string(),
                        "Report definition saved.".to_string(),
                    )));
                    name.set(String::new());
                    load_saved();
                }
                Err(e) => notice.set(Some(("err".to_string(), e))),
            }
        });
    };

    let remove_saved = move |id: i64| {
        let load_saved = load_saved;
        let notice = notice;
        spawn_local(async move {
            let path = format!("/api/reports/builder/saved/{id}");
            match crate::api::delete_json::<serde_json::Value>(&path).await {
                Ok(_) => load_saved(),
                Err(e) => notice.set(Some(("err".to_string(), e))),
            }
        });
    };

    let use_saved = move |s: &SavedReportRow| {
        let c = &s.config;
        let get_str = |k: &str| {
            c.get(k)
                .and_then(|x| x.as_str())
                .unwrap_or_default()
                .to_string()
        };
        metric.set(get_str("metric"));
        dimension.set(get_str("dimension"));
        date_from.set(get_str("dateFrom"));
        date_to.set(get_str("dateTo"));
        comparison.set(get_str("comparison"));
        sort.set(get_str("sort"));
        limit.set(c.get("limit").and_then(|x| x.as_i64()).unwrap_or(30));
        let attr = c
            .pointer("/filters/attributeKey")
            .and_then(|x| x.as_str())
            .unwrap_or_default()
            .to_string();
        if !attr.is_empty() {
            attribute_key.set(attr);
        }
        let st = c
            .pointer("/filters/stateKey")
            .and_then(|x| x.as_str())
            .unwrap_or_default()
            .to_string();
        if !st.is_empty() {
            state_key.set(st);
        }
        run_config(c.clone());
    };

    view! {
        <div class="spp-flex spp-flex--col spp-gap-12">
            <Show when=move || catalog_error.get() fallback=|| ()>
                <div class="spp-state spp-state--error">
                    <p class="spp-state__body">"The report catalog failed to load."</p>
                </div>
            </Show>
            {move || {
                let cat = catalog.clone().get();
                if cat.is_none() {
                    return view! { <LoadingState /> }.into_view();
                }
                let cat = cat.unwrap_or_default();
                let metric_entry = cat
                    .metrics
                    .iter()
                    .find(|m| m.key == metric.get_untracked())
                    .cloned()
                    .unwrap_or_default();
                let dimension_entry = cat
                    .dimensions
                    .iter()
                    .find(|d| d.key == dimension.get_untracked())
                    .cloned()
                    .unwrap_or_default();
                let total_only = is_total_only(&metric.get_untracked(), &cat);
                let needs_attribute = metric_entry.needs_attribute;
                let needs_state =
                    metric.get_untracked() == "avg_state_hours" || metric.get_untracked() == "state_changes";
                let cat_for_change = cat.clone();
                view! {
                    <div class="spp-card">
                        <h3 class="spp-card__title">"Custom report builder"</h3>
                        <p class="spp-muted spp-text-sm spp-mb-12">{cat.note.clone()}</p>
                        <div class="spp-builder-grid">
                            <label class="spp-builder-field">
                                <span class="spp-muted">"Metric"</span>
                                <select
                                    class="spp-input"
                                    value=move || metric.get()
                                    on:change=move |ev| {
                                        let next = event_target_value(&ev);
                                        if is_total_only(&next, &cat_for_change) {
                                            dimension.set("none".to_string());
                                        }
                                        metric.set(next);
                                    }
                                >
                                    {cat.metrics
                                        .iter()
                                        .map(|m| view! { <option value=m.key.clone()>{m.label.clone()}</option> })
                                        .collect::<Vec<_>>()}
                                </select>
                            </label>
                            <label class="spp-builder-field">
                                <span class="spp-muted">"Group by"</span>
                                <select
                                    class="spp-input"
                                    value=move || dimension.get()
                                    prop:disabled=total_only
                                    on:change=move |ev| dimension.set(event_target_value(&ev))
                                >
                                    {cat.dimensions
                                        .iter()
                                        .map(|d| {
                                            let disabled = total_only && d.key != "none";
                                            view! {
                                                <option value=d.key.clone() disabled=disabled>{d.label.clone()}</option>
                                            }
                                        })
                                        .collect::<Vec<_>>()}
                                </select>
                            </label>
                            <label class="spp-builder-field">
                                <span class="spp-muted">"From"</span>
                                <input
                                    class="spp-input"
                                    type="date"
                                    value=move || date_from.get()
                                    on:change=move |ev| date_from.set(event_target_value(&ev))
                                />
                            </label>
                            <label class="spp-builder-field">
                                <span class="spp-muted">"To (inclusive)"</span>
                                <input
                                    class="spp-input"
                                    type="date"
                                    value=move || date_to.get()
                                    on:change=move |ev| date_to.set(event_target_value(&ev))
                                />
                            </label>
                            <label class="spp-builder-field">
                                <span class="spp-muted">"Comparison"</span>
                                <select
                                    class="spp-input"
                                    value=move || comparison.get()
                                    on:change=move |ev| comparison.set(event_target_value(&ev))
                                >
                                    <option value="none">"None"</option>
                                    <option value="previous_period">"Previous period"</option>
                                </select>
                            </label>
                            <label class="spp-builder-field">
                                <span class="spp-muted">"Sort"</span>
                                <select
                                    class="spp-input"
                                    value=move || sort.get()
                                    on:change=move |ev| sort.set(event_target_value(&ev))
                                >
                                    <option value="metric_desc">"Metric (high to low)"</option>
                                    <option value="metric_asc">"Metric (low to high)"</option>
                                    <option value="dimension_asc">"Dimension (A to Z)"</option>
                                </select>
                            </label>
                            <label class="spp-builder-field">
                                <span class="spp-muted">"Max rows"</span>
                                <input
                                    class="spp-input"
                                    type="number"
                                    min="1"
                                    max="200"
                                    value=move || limit.get().to_string()
                                    on:input=move |ev| {
                                        let v = event_target_value(&ev).parse::<i64>().unwrap_or(30);
                                        limit.set(v.clamp(1, 200));
                                    }
                                />
                            </label>
                            <label class="spp-builder-field">
                                <span class="spp-muted">"Display"</span>
                                <select
                                    class="spp-input"
                                    value=move || if chart_bar.get() { "bar".to_string() } else { "table".to_string() }
                                    on:change=move |ev| chart_bar.set(event_target_value(&ev) == "bar")
                                >
                                    <option value="bar">"Bar chart"</option>
                                    <option value="table">"Table"</option>
                                </select>
                            </label>
                            {if needs_attribute {
                                view! {
                                    <label class="spp-builder-field">
                                        <span class="spp-muted">"AI attribute key"</span>
                                        <select
                                            class="spp-input"
                                            value=move || attribute_key.get()
                                            on:change=move |ev| attribute_key.set(event_target_value(&ev))
                                        >
                                            <option value="">"(choose an attribute)"</option>
                                            {ATTRIBUTE_KEYS
                                                .iter()
                                                .map(|k| view! { <option value=*k>{*k}</option> })
                                                .collect::<Vec<_>>()}
                                        </select>
                                    </label>
                                }.into_view()
                            } else {
                                ().into_view()
                            }}
                            {if needs_state {
                                view! {
                                    <label class="spp-builder-field">
                                        <span class="spp-muted">"Custom state"</span>
                                        <select
                                            class="spp-input"
                                            value=move || state_key.get()
                                            on:change=move |ev| state_key.set(event_target_value(&ev))
                                        >
                                            <option value="">"(choose a state)"</option>
                                            {STATE_KEYS
                                                .iter()
                                                .map(|k| view! { <option value=*k>{*k}</option> })
                                                .collect::<Vec<_>>()}
                                        </select>
                                    </label>
                                }.into_view()
                            } else {
                                ().into_view()
                            }}
                        </div>
                        <div class="spp-flex spp-gap-8 spp-mt-12 spp-flex--wrap">
                            <button
                                class="spp-button spp-button--primary"
                                on:click=run_from_form
                                disabled=move || running.get()
                            >
                                {move || if running.get() { "Running…" } else { "Run report" }.to_string()}
                            </button>
                            <input
                                class="spp-input spp-grow"
                                placeholder="Name to save this report definition…"
                                maxlength="120"
                                value=move || name.get()
                                on:input=move |ev| name.set(event_target_value(&ev))
                            />
                            <button
                                class="spp-button"
                                on:click=save
                                disabled=move || name.get().trim().is_empty()
                            >
                                "Save definition"
                            </button>
                        </div>
                        {if !metric_entry.key.is_empty() {
                            view! {
                                <div class="spp-alert spp-alert--info spp-mt-12">
                                    <div class="spp-text-sm">
                                        <strong>{metric_entry.label.clone()}</strong>
                                        " — "
                                        {metric_entry.definition.clone()}
                                    </div>
                                    <div class="spp-muted">
                                        {format!("Limitations: {}", metric_entry.limitations)}
                                    </div>
                                    {if !dimension_entry.key.is_empty() && dimension_entry.key != "none" {
                                        view! {
                                            <div class="spp-muted">
                                                {format!("Grouping: {}", dimension_entry.definition)}
                                            </div>
                                        }.into_view()
                                    } else {
                                        ().into_view()
                                    }}
                                </div>
                            }.into_view()
                        } else {
                            ().into_view()
                        }}
                    </div>
                }.into_view()
            }}
            {move || {
                let notice = notice.get();
                if let Some((kind, message)) = notice {
                    let class = if kind == "ok" {
                        "spp-alert spp-alert--ok"
                    } else {
                        "spp-alert spp-alert--error"
                    };
                    view! { <div class=class><div class="spp-text-sm">{message}</div></div> }.into_view()
                } else {
                    ().into_view()
                }
            }}
            {move || {
                let r = result.clone().get();
                let Some(r) = r else {
                    return ().into_view();
                };
                let has_comparison = r.comparison_rows.is_some();
                let title = if r.dimension_key != "none" {
                    format!("{} by {}", r.metric_label, r.dimension_label.to_lowercase())
                } else {
                    r.metric_label.clone()
                };
                let range_note = match (&r.comparison_from, &r.comparison_to) {
                    (Some(f), Some(t)) => format!("{} → {} (vs {} → {})", r.date_from, r.date_to, f, t),
                    _ => format!("{} → {}", r.date_from, r.date_to),
                };
                view! {
                    <div class="spp-card">
                        <div class="spp-flex spp-flex--between spp-mb-8">
                            <h3 class="spp-card__title">{title}</h3>
                            <span class="spp-muted spp-text-sm">{range_note}</span>
                        </div>
                        {if r.rows.is_empty() {
                            view! {
                                <EmptyState message="No data in this range. Try a wider date range or different filters." />
                            }.into_view()
                        } else if chart_bar.get() && r.dimension_key != "none" {
                            let max = r.rows.iter().map(|x| x.value).fold(0.0_f64, f64::max).max(1.0);
                            view! {
                                <div class="spp-barchart">
                                    {r.rows
                                        .iter()
                                        .map(|row| {
                                            let cmp = r.comparison_rows.as_ref().and_then(|cs| {
                                                cs.iter().find(|c| c.dimension_value == row.dimension_value)
                                            });
                                            let width = ((row.value / max) * 100.0).max(2.0);
                                            let cmp_width = cmp
                                                .map(|c| ((c.value / max) * 100.0).max(1.0))
                                                .filter(|w| w > &0.0 && cmp.is_some_and(|c| c.value > 0.0));
                                            let label = match cmp {
                                                Some(c) => format!(
                                                    "{}: {} (previous: {})",
                                                    row.dimension_label,
                                                    fmt_value(row.value, &r.metric_format),
                                                    fmt_value(c.value, &r.metric_format)
                                                ),
                                                None => format!(
                                                    "{}: {}",
                                                    row.dimension_label,
                                                    fmt_value(row.value, &r.metric_format)
                                                ),
                                            };
                                            view! {
                                                <div class="spp-bar-row" title=label>
                                                    <span class="spp-bar-label">{row.dimension_label.clone()}</span>
                                                    <div class="spp-bar-track">
                                                        <div class="spp-bar" style=format!("width: {width:.1}%")>
                                                            {fmt_value(row.value, &r.metric_format)}
                                                        </div>
                                                        {if let Some(cw) = cmp_width {
                                                            view! {
                                                                <div class="spp-bar spp-bar--compare" style=format!("width: {cw:.1}%")></div>
                                                            }.into_view()
                                                        } else {
                                                            ().into_view()
                                                        }}
                                                    </div>
                                                </div>
                                            }
                                        })
                                        .collect::<Vec<_>>()}
                                </div>
                            }.into_view()
                        } else {
                            view! {
                                <table class="spp-table">
                                    <thead>
                                        <tr>
                                            <th>{if r.dimension_key == "none" { "Total".to_string() } else { r.dimension_label.clone() }}</th>
                                            <th>{r.metric_label.clone()}</th>
                                            {if has_comparison {
                                                view! { <th>"Previous period"</th> }.into_view()
                                            } else {
                                                ().into_view()
                                            }}
                                            <th>"Samples"</th>
                                        </tr>
                                    </thead>
                                    <tbody>
                                        {r.rows
                                            .iter()
                                            .map(|row| {
                                                let cmp = r.comparison_rows.as_ref().and_then(|cs| {
                                                    cs.iter().find(|c| c.dimension_value == row.dimension_value)
                                                });
                                                let delta: Option<f64> = cmp.map(|c| {
                                                    if c.value != 0.0 {
                                                        ((row.value - c.value) / c.value.abs()) * 100.0
                                                    } else {
                                                        f64::NAN
                                                    }
                                                }).filter(|d| d.is_finite());
                                                view! {
                                                    <tr>
                                                        <td>{row.dimension_label.clone()}</td>
                                                        <td class="spp-mono">{fmt_value(row.value, &r.metric_format)}</td>
                                                        {if has_comparison {
                                                            view! {
                                                                <td class="spp-mono">
                                                                    {cmp.map(|c| fmt_value(c.value, &r.metric_format)).unwrap_or_else(|| "—".to_string())}
                                                                    {if let Some(d) = delta {
                                                                        let badge = if d > 0.0 {
                                                                            "spp-badge spp-badge--warn spp-ml-4"
                                                                        } else {
                                                                            "spp-badge spp-badge--ok spp-ml-4"
                                                                        };
                                                                        view! {
                                                                            <span class=badge>{format!("{}{:.0}%", if d > 0.0 { "+" } else { "" }, d)}</span>
                                                                        }.into_view()
                                                                    } else {
                                                                        ().into_view()
                                                                    }}
                                                                </td>
                                                            }.into_view()
                                                        } else {
                                                            ().into_view()
                                                        }}
                                                        <td>
                                                            <div class="spp-flex spp-gap-4 spp-flex--wrap">
                                                                {row.sample_conversation_ids
                                                                    .iter()
                                                                    .map(|id| view! {
                                                                        <a
                                                                            class="spp-button spp-button--tiny spp-button--ghost"
                                                                            href=format!("/inbox/conversation/{id}")
                                                                        >
                                                                            {format!("#{id}")}
                                                                        </a>
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
                            }.into_view()
                        }}
                        <div class="spp-alert spp-alert--info spp-mt-12">
                            {r.notes
                                .iter()
                                .map(|n| view! { <div class="spp-text-sm">{n.clone()}</div> })
                                .collect::<Vec<_>>()}
                        </div>
                    </div>
                }.into_view()
            }}
            {move || {
                let s = saved.get();
                if s.is_empty() {
                    return ().into_view();
                }
                view! {
                    <div class="spp-card">
                        <h3 class="spp-card__title">"Saved report definitions"</h3>
                        <div class="spp-flex spp-flex--col spp-gap-4 spp-mt-8">
                            {s.iter()
                                .map(|row| {
                                    let metric_label = row
                                        .config
                                        .get("metric")
                                        .and_then(|x| x.as_str())
                                        .unwrap_or_default()
                                        .to_string();
                                    let dimension_label = row
                                        .config
                                        .get("dimension")
                                        .and_then(|x| x.as_str())
                                        .unwrap_or_default()
                                        .to_string();
                                    let use_row = row.clone();
                                    let del_id = row.id;
                                    view! {
                                        <div class="spp-flex spp-flex--between spp-gap-8">
                                            <button
                                                class="spp-button spp-button--small spp-button--ghost"
                                                on:click=move |_| use_saved(&use_row)
                                            >
                                                {row.name.clone()}
                                                <span class="spp-muted spp-text-xs">
                                                    {format!(" ({metric_label} · {dimension_label})")}
                                                </span>
                                            </button>
                                            <button
                                                class="spp-button spp-button--tiny"
                                                on:click=move |_| remove_saved(del_id)
                                            >
                                                "Delete"
                                            </button>
                                        </div>
                                    }
                                })
                                .collect::<Vec<_>>()}
                        </div>
                    </div>
                }.into_view()
            }}
            <p class="spp-muted spp-text-xs">
                "Every number above is a local calculation from the local mirror. Origin: local."
            </p>
        </div>
    }
}
