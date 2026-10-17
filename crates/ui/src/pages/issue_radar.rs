use std::rc::Rc;

use leptos::*;
use leptos_router::{use_navigate, use_query_map};

use crate::components::state_view::{EmptyState, LoadingState};
use crate::components::GapsTab;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IssuesTab {
    Radar,
    Clusters,
    Known,
    Gaps,
    Reuse,
}

impl IssuesTab {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Radar => "radar",
            Self::Clusters => "clusters",
            Self::Known => "known",
            Self::Gaps => "gaps",
            Self::Reuse => "reuse",
        }
    }

    #[must_use]
    pub fn from_param(raw: Option<&str>) -> Self {
        match raw {
            Some("clusters") => Self::Clusters,
            Some("known") => Self::Known,
            Some("gaps") => Self::Gaps,
            Some("reuse") => Self::Reuse,
            _ => Self::Radar,
        }
    }
}

#[component]
pub fn IssueRadarPage() -> impl IntoView {
    let query_map = use_query_map();
    let tab = create_rw_signal(IssuesTab::Radar);
    create_effect(move |_| {
        let m = query_map.get();
        let next = IssuesTab::from_param(crate::url_state::query_str(&m, "tab").as_deref());
        if tab.get_untracked() != next {
            tab.set(next);
        }
    });

    view! {
        <div class="spp-page spp-page--issue-radar">
            <h2 class="spp-page__title">"Issues"</h2>

            <p class="spp-page__intro">
                "Issue intelligence from your local data — every alert links to underlying tickets; correlations are never claimed as causes."
            </p>

            <div class="spp-tabs">
                <button
                    class="spp-tab"
                    class:is-active=move || tab.get() == IssuesTab::Radar
                    on:click=move |_| tab.set(IssuesTab::Radar)
                >
                    "Issue Radar"
                </button>
                <button
                    class="spp-tab"
                    class:is-active=move || tab.get() == IssuesTab::Clusters
                    on:click=move |_| tab.set(IssuesTab::Clusters)
                >
                    "Clusters"
                </button>
                <button
                    class="spp-tab"
                    class:is-active=move || tab.get() == IssuesTab::Known
                    on:click=move |_| tab.set(IssuesTab::Known)
                >
                    "Known Issues"
                </button>
                <button
                    class="spp-tab"
                    class:is-active=move || tab.get() == IssuesTab::Gaps
                    on:click=move |_| tab.set(IssuesTab::Gaps)
                >
                    "Doc Gaps"
                </button>
                <button
                    class="spp-tab"
                    class:is-active=move || tab.get() == IssuesTab::Reuse
                    on:click=move |_| tab.set(IssuesTab::Reuse)
                >
                    "Answer Reuse"
                </button>
            </div>

            <Show when=move || tab.get() == IssuesTab::Radar fallback=|| ()>
                <RadarTab />
            </Show>
            <Show when=move || tab.get() == IssuesTab::Clusters fallback=|| ()>
                <ClustersTab />
            </Show>
            <Show when=move || tab.get() == IssuesTab::Known fallback=|| ()>
                <KnownTab />
            </Show>
            <Show when=move || tab.get() == IssuesTab::Gaps fallback=|| ()>
                <GapsTabLite />
            </Show>
            <Show when=move || tab.get() == IssuesTab::Reuse fallback=|| ()>
                <ReuseTab />
            </Show>
        </div>
    }
}

#[component]
fn ListStates(
    loading: RwSignal<bool>,
    error_msg: RwSignal<Option<String>>,
    empty: &'static str,
) -> impl IntoView {
    view! {
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
            <EmptyState message=empty />
        </Show>
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

fn value_loaded(v: &serde_json::Value) -> bool {
    match v {
        serde_json::Value::Null => false,
        serde_json::Value::Object(m) => !m.is_empty(),
        serde_json::Value::Array(a) => !a.is_empty(),
        _ => true,
    }
}

fn confirm_action(message: &str) -> bool {
    web_sys::window()
        .and_then(|w| w.confirm_with_message(message).ok())
        .unwrap_or(false)
}

#[component]
fn RadarTab() -> impl IntoView {
    let snapshot = create_rw_signal(serde_json::json!({}));
    let alerts = create_rw_signal(Vec::<serde_json::Value>::new());
    let loading = create_rw_signal(true);
    let error_msg = create_rw_signal(None::<String>);

    let sla = create_rw_signal(serde_json::json!({}));
    let sla_error = create_rw_signal(None::<String>);

    create_effect(move |_| {
        let snapshot = snapshot;
        let alerts = alerts;
        let loading = loading;
        let error_msg = error_msg;
        let sla = sla;
        let sla_error = sla_error;
        let _ = crate::queries::version("dashboard").get();
        wasm_bindgen_futures::spawn_local(async move {
            match crate::api::get_json::<serde_json::Value>("/api/reports/issue-radar").await {
                Ok(data) => {
                    let snap = data
                        .get("alerts")
                        .and_then(|v| v.as_array())
                        .and_then(|arr| arr.first())
                        .cloned()
                        .unwrap_or(data.clone());
                    alerts.set(
                        data.get("alerts")
                            .and_then(|v| v.as_array())
                            .cloned()
                            .unwrap_or_default(),
                    );
                    snapshot.set(snap);
                }
                Err(e) => error_msg.set(Some(e)),
            }
            loading.set(false);
        });
        wasm_bindgen_futures::spawn_local(async move {
            match crate::api::get_json::<serde_json::Value>("/api/issues/sla-alerts").await {
                Ok(v) => sla.set(v),
                Err(e) => sla_error.set(Some(e)),
            }
        });
    });

    view! {
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
            {move || {
                let s = snapshot.get();
                let active_known_issues = int_field(&s, "active_known_issues");
                let active_clusters = int_field(&s, "active_clusters");
                let active_incidents = int_field(&s, "active_incidents");

                view! {
                    <div class="spp-radar-grid">
                        <div class="spp-radar-card">
                            <span class="spp-radar-card__label">"Active known issues"</span>
                            <span class="spp-radar-card__value">{active_known_issues.to_string()}</span>
                            <span class="spp-radar-card__hint">"Documented issues currently affecting customers"</span>
                        </div>
                        <div class="spp-radar-card">
                            <span class="spp-radar-card__label">"Active clusters"</span>
                            <span class="spp-radar-card__value">{active_clusters.to_string()}</span>
                            <span class="spp-radar-card__hint">"Groups of similar issues identified by clustering"</span>
                        </div>
                        <div class="spp-radar-card spp-radar-card--incident">
                            <span class="spp-radar-card__label">"Active incidents"</span>
                            <span class="spp-radar-card__value">{active_incidents.to_string()}</span>
                            <span class="spp-radar-card__hint">"Unresolved incidents (status != resolved)"</span>
                        </div>
                    </div>

                    <SlaAlertsCard sla=sla error=sla_error />

                    <Show
                        when=move || !alerts.with(|a| a.is_empty())
                        fallback=move || {
                            view! {
                                <EmptyState message="No active issues, clusters, or incidents. The radar is clear." />
                            }
                        }
                    >
                        <div class="spp-card">
                            <h3 class="spp-card__title">"Alerts"</h3>
                            <ul class="spp-issues__alert-list">
                                {move || alerts.get().iter().map(|a| {
                                    let title = field(a, "title");
                                    let title = if title.is_empty() { "(alert)".to_string() } else { title };
                                    let detail = field(a, "detail");
                                    let severity = {
                                        let s = field(a, "severity");
                                        if s.is_empty() { "info".to_string() } else { s }
                                    };
                                    let conv_ids = a.get("conversation_ids")
                                        .and_then(|v| v.as_array())
                                        .map(|arr| arr.iter().filter_map(|c| c.as_i64()).collect::<Vec<_>>())
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
                                                {conv_ids.iter().take(6).map(|cid| {
                                                    view! {
                                                        <a class="spp-badge" href={format!("/inbox/conversation/{cid}")}>
                                                            {format!("#{cid}")}
                                                        </a>
                                                    }
                                                }).collect::<Vec<_>>()}
                                            </div>
                                        </li>
                                    }
                                }).collect::<Vec<_>>()}
                            </ul>
                        </div>
                    </Show>
                }.into_view()
            }}
        </Show>
    }
}

#[component]
fn SlaAlertsCard(
    sla: RwSignal<serde_json::Value>,
    error: RwSignal<Option<String>>,
) -> impl IntoView {
    view! {
        <Show when=move || error.get().is_some() fallback=|| ()>
            <div class="spp-card">
                <h3 class="spp-card__title">"SLA alerts"</h3>
                <p class="spp-muted spp-text-xs">
                    {move || format!("SLA alerts unavailable: {}", error.get().unwrap_or_default())}
                </p>
            </div>
        </Show>
        <Show when=move || error.get().is_none() && sla.with(value_loaded) fallback=|| ()>
            {move || {
                let s = sla.get();
                let breached = int_field(&s, "total_breached");
                let at_risk = int_field(&s, "total_at_risk");
                let alert_rows = s.get("alerts")
                    .and_then(|v| v.as_array())
                    .cloned()
                    .unwrap_or_default();
                let per_mailbox = s.get("per_mailbox")
                    .and_then(|v| v.as_array())
                    .cloned()
                    .unwrap_or_default();
                let unconfigured = s.get("unconfigured_mailboxes")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(str::to_string))
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                let note = field(&s, "note");
                let has_alerts = !alert_rows.is_empty();
                let alert_rows_view = alert_rows.clone();
                let has_mailbox = !per_mailbox.is_empty();
                let per_mailbox_view = per_mailbox.clone();
                let has_unconfigured = !unconfigured.is_empty();
                let unconfigured_view = unconfigured.clone();
                let has_note = !note.is_empty();
                let note_view = note.clone();

                view! {
                    <div class="spp-card">
                        <h3 class="spp-card__title">"SLA alerts (business hours)"</h3>
                        <div class="spp-radar-grid">
                            <div class="spp-radar-card spp-radar-card--incident">
                                <span class="spp-radar-card__label">"Breached"</span>
                                <span class="spp-radar-card__value">{breached.to_string()}</span>
                                <span class="spp-radar-card__hint">"Conversations past a target"</span>
                            </div>
                            <div class="spp-radar-card">
                                <span class="spp-radar-card__label">"At risk"</span>
                                <span class="spp-radar-card__value">{at_risk.to_string()}</span>
                                <span class="spp-radar-card__hint">"Approaching a target"</span>
                            </div>
                        </div>

                        <Show when=move || has_unconfigured fallback=|| ()>
                            <p class="spp-muted spp-text-xs">
                                "Unconfigured mailboxes (no business hours yet): "
                                {unconfigured_view.iter().map(|m| {
                                    view! { <span class="spp-badge">{m.clone()}</span> }
                                }).collect::<Vec<_>>()}
                            </p>
                        </Show>

                        <Show
                            when=move || has_alerts
                            fallback=|| view! { <EmptyState message="No SLA breaches or at-risk conversations right now." /> }
                        >
                            <table class="spp-table">
                                <thead>
                                    <tr>
                                        <th>"Conversation"</th>
                                        <th>"Mailbox"</th>
                                        <th>"State"</th>
                                        <th>"Waited (business min)"</th>
                                        <th>"Target"</th>
                                        <th>"Overdue (business min)"</th>
                                    </tr>
                                </thead>
                                <tbody>
                                    {alert_rows_view.iter().map(|a| {
                                        let cid = int_field(a, "conversation_id");
                                        let number = int_field(a, "number");
                                        let subject = field(a, "subject");
                                        let mailbox = field(a, "mailbox_name");
                                        let state = field(a, "state");
                                        let waited = int_field(a, "waited_business_min");
                                        let target_min = int_field(a, "target_min");
                                        let target_kind = field(a, "target_kind");
                                        let overdue = int_field(a, "overdue_business_min");
                                        view! {
                                            <tr>
                                                <td>
                                                    <a class="spp-text-xs" href={format!("/inbox/conversation/{cid}")}>
                                                        {if subject.is_empty() { format!("#{number}") } else { subject }}
                                                    </a>
                                                </td>
                                                <td class="spp-table__cell-muted">{mailbox}</td>
                                                <td>
                                                    <span class=if state == "breached" {
                                                        "spp-badge spp-badge--warn"
                                                    } else {
                                                        "spp-badge"
                                                    }>
                                                        {state}
                                                    </span>
                                                </td>
                                                <td>{waited.to_string()}</td>
                                                <td class="spp-table__cell-muted">
                                                    {format!("{target_kind} {target_min} min")}
                                                </td>
                                                <td>{overdue.to_string()}</td>
                                            </tr>
                                        }
                                    }).collect::<Vec<_>>()}
                                </tbody>
                            </table>
                        </Show>

                        <Show when=move || has_mailbox fallback=|| ()>
                            <p class="spp-muted spp-text-xs">
                                {per_mailbox_view.iter().map(|m| {
                                    let name = field(m, "mailbox_name");
                                    let b = int_field(m, "breached");
                                    let r = int_field(m, "at_risk");
                                    view! {
                                        <span class="spp-badge">{format!("{name}: {b} breached · {r} at risk")}</span>
                                    }
                                }).collect::<Vec<_>>()}
                            </p>
                        </Show>

                        <Show when=move || has_note fallback=|| ()>
                            <p class="spp-muted spp-text-xs">{note_view.clone()}</p>
                        </Show>
                    </div>
                }.into_view()
            }}
        </Show>
    }
}

#[component]
fn ClustersTab() -> impl IntoView {
    let clusters = create_rw_signal(Vec::<serde_json::Value>::new());
    let loading = create_rw_signal(true);
    let error_msg = create_rw_signal(None::<String>);
    let selected = create_rw_signal(None::<i64>);
    let promoting = create_rw_signal(None::<serde_json::Value>);

    let reload = create_rw_signal(0u32);
    create_effect(move |_| {
        let _ = reload.get();
        loading.set(true);
        error_msg.set(None);
        wasm_bindgen_futures::spawn_local(async move {
            match crate::api::get_json::<serde_json::Value>("/api/issues/clusters").await {
                Ok(v) => clusters.set(
                    v.get("clusters")
                        .and_then(|c| c.as_array())
                        .cloned()
                        .unwrap_or_default(),
                ),
                Err(e) => error_msg.set(Some(e)),
            }
            loading.set(false);
        });
    });

    let back_to_list = StoredValue::new(Rc::new(move || selected.set(None)) as Rc<dyn Fn()>);
    let reload_clusters =
        StoredValue::new(Rc::new(move || reload.update(|t| *t += 1)) as Rc<dyn Fn()>);

    view! {
        <Show
            when=move || selected.get().is_none() && promoting.get().is_none()
            fallback=|| ()
        >
            <div class="spp-flex spp-flex--between spp-mb-8">
                <p class="spp-muted spp-text-xs">
                    "Clusters are AI-generated groups of similar conversations. Open one to review its members, promote it to a known issue, or delete it."
                </p>
            </div>
            <ListStates loading=loading error_msg=error_msg empty="No issue clusters yet. Clusters appear after AI analyses group similar tickets." />
            <Show
                when=move || !loading.get() && error_msg.get().is_none() && !clusters.with(|c| c.is_empty())
                fallback=|| ()
            >
                <div class="spp-card">
                    <table class="spp-table">
                        <thead>
                            <tr>
                                <th>"Cluster"</th>
                                <th>"Category"</th>
                                <th>"Conversations"</th>
                                <th>"Customers"</th>
                                <th>"Trend"</th>
                                <th>"Last seen"</th>
                                <th><span class="spp-visually-hidden">"Actions"</span></th>
                            </tr>
                        </thead>
                        <tbody>
                            {move || clusters.get().iter().map(|c| {
                                let id = int_field(c, "id");
                                let title = {
                                    let t = field(c, "title");
                                    if t.is_empty() { "(untitled)".to_string() } else { t }
                                };
                                let summary = field(c, "summary");
                                let category = field(c, "category");
                                let category = if category.is_empty() { "—".to_string() } else { category };
                                let count = int_field(c, "conversation_count");
                                let customers = int_field(c, "customer_count");
                                let trend = field(c, "trend");
                                let trend = if trend.is_empty() { "flat".to_string() } else { trend };
                                let last_seen = field(c, "last_seen_at");
                                let last_seen = last_seen.get(..10).unwrap_or("").to_string();

                                let open = move |_| selected.set(Some(id));
                                let do_delete = {
                                    let title_for_confirm = title.clone();
                                    move |ev: leptos::ev::MouseEvent| {
                                        ev.stop_propagation();
                                        if !confirm_action(&format!(
                                            "Delete cluster \"{title_for_confirm}\"? Its conversations are untouched; the grouping is removed."
                                        )) {
                                            return;
                                        }
                                        wasm_bindgen_futures::spawn_local(async move {
                                            let path = format!("/api/issues/clusters/{id}");
                                            match crate::api::delete_json::<serde_json::Value>(&path).await {
                                                Ok(r) => {
                                                    crate::toasts::success(
                                                        r.get("message")
                                                            .and_then(|v| v.as_str())
                                                            .unwrap_or("Cluster deleted."),
                                                    );
                                                    reload.update(|t| *t += 1);
                                                }
                                                Err(e) => crate::toasts::error(e),
                                            }
                                        });
                                    }
                                };
                                let do_promote = {
                                    let c = c.clone();
                                    move |ev: leptos::ev::MouseEvent| {
                                        ev.stop_propagation();
                                        promoting.set(Some(c.clone()));
                                    }
                                };

                                view! {
                                    <tr class="spp-table__row-clickable" on:click=open>
                                        <td>
                                            <strong>{title}</strong>
                                            {if !summary.is_empty() {
                                                view! { <div class="spp-muted spp-text-xs">{summary}</div> }.into_view()
                                            } else {
                                                ().into_view()
                                            }}
                                        </td>
                                        <td class="spp-table__cell-muted">{category}</td>
                                        <td><span class="spp-badge">{count.to_string()}</span></td>
                                        <td class="spp-table__cell-muted">{customers.to_string()}</td>
                                        <td><span class="spp-badge">{trend}</span></td>
                                        <td class="spp-table__cell-muted">{last_seen}</td>
                                        <td class="spp-table__cell-actions">
                                            <button
                                                class="spp-button spp-button--ghost spp-button--tiny"
                                                type="button"
                                                on:click=do_promote
                                            >
                                                "Promote"
                                            </button>
                                            <button
                                                class="spp-button spp-button--ghost spp-button--tiny"
                                                type="button"
                                                on:click=do_delete
                                            >
                                                "Delete"
                                            </button>
                                        </td>
                                    </tr>
                                }
                            }).collect::<Vec<_>>()}
                        </tbody>
                    </table>
                </div>
            </Show>
        </Show>

        <Show when=move || selected.get().is_some() fallback=|| ()>
            {move || {
                let id = selected.get().unwrap_or_default();
                let on_back = back_to_list.with_value(Rc::clone);
                let reload_fn = reload_clusters.with_value(Rc::clone);
                view! {
                    <ClusterDetail
                        id=id
                        on_back=on_back
                        reload=reload_fn
                    />
                }
            }}
        </Show>

        <Show when=move || promoting.get().is_some() fallback=|| ()>
            {move || {
                let cluster = promoting.get().unwrap_or_default();
                view! {
                    <KnownIssueEditor
                        seed=cluster
                        on_close=Rc::new(move || promoting.set(None))
                        on_saved=Rc::new(move || reload.update(|t| *t += 1))
                    />
                }
            }}
        </Show>
    }
}

#[component]
fn ClusterDetail(id: i64, on_back: Rc<dyn Fn()>, reload: Rc<dyn Fn()>) -> impl IntoView {
    let cluster = create_rw_signal(serde_json::json!({}));
    let conversations = create_rw_signal(Vec::<serde_json::Value>::new());
    let loading = create_rw_signal(true);
    let error_msg = create_rw_signal(None::<String>);
    let promoting = create_rw_signal(false);

    let on_back_stored = StoredValue::new(on_back);
    let reload_stored = StoredValue::new(reload);

    create_effect(move |_| {
        let id = id;
        loading.set(true);
        error_msg.set(None);
        wasm_bindgen_futures::spawn_local(async move {
            let path = format!("/api/issues/clusters/{id}");
            match crate::api::get_json::<serde_json::Value>(&path).await {
                Ok(v) => {
                    cluster.set(v.get("cluster").cloned().unwrap_or_default());
                    conversations.set(
                        v.get("conversations")
                            .and_then(|c| c.as_array())
                            .cloned()
                            .unwrap_or_default(),
                    );
                }
                Err(e) => error_msg.set(Some(e)),
            }
            loading.set(false);
        });
    });

    view! {
        <ListStates loading=loading error_msg=error_msg empty="Cluster not found." />
        <Show
            when=move || !loading.get() && error_msg.get().is_none() && cluster.with(value_loaded)
            fallback=|| ()
        >
            {move || {
                let c = cluster.get();
                let id = int_field(&c, "id");
                let title = {
                    let t = field(&c, "title");
                    if t.is_empty() { "(untitled)".to_string() } else { t }
                };
                let summary = field(&c, "summary");
                let category = field(&c, "category");
                let product = field(&c, "product");
                let feature = field(&c, "feature");
                let count = int_field(&c, "conversation_count");
                let customers = int_field(&c, "customer_count");
                let trend = field(&c, "trend");
                let first_seen = field(&c, "first_seen_at");
                let last_seen = field(&c, "last_seen_at");
                let provenance = field(&c, "provenance");
                let known_issue_id = c.get("known_issue_id").and_then(|v| v.as_i64());
                let ai_generated = c.get("ai_generated").and_then(|v| v.as_i64()).unwrap_or(0) == 1;
                let on_back_click = on_back_stored.with_value(Rc::clone);
                let do_delete_row = {
                    let on_back = on_back_stored.with_value(Rc::clone);
                    let reload = reload_stored.with_value(Rc::clone);
                    move |ev: leptos::ev::MouseEvent| {
                        ev.stop_propagation();
                        let c = cluster.get_untracked();
                        let title = field(&c, "title");
                        if !confirm_action(&format!(
                            "Delete cluster \"{title}\"? Its conversations are untouched; the grouping is removed."
                        )) {
                            return;
                        }
                        let id = int_field(&c, "id");
                        let on_back = Rc::clone(&on_back);
                        let reload = Rc::clone(&reload);
                        wasm_bindgen_futures::spawn_local(async move {
                            let path = format!("/api/issues/clusters/{id}");
                            match crate::api::delete_json::<serde_json::Value>(&path).await {
                                Ok(r) => {
                                    crate::toasts::success(
                                        r.get("message")
                                            .and_then(|v| v.as_str())
                                            .unwrap_or("Cluster deleted."),
                                    );
                                    on_back();
                                    reload();
                                }
                                Err(e) => crate::toasts::error(e),
                            }
                        });
                    }
                };

                view! {
                    <div class="spp-card">
                        <div class="spp-flex spp-flex--between">
                            <h3 class="spp-card__title">
                                {format!("Cluster #{id}: {title}")}
                                {if ai_generated {
                                    view! { <span class="spp-badge spp-badge--ai">"AI-generated"</span> }.into_view()
                                } else {
                                    ().into_view()
                                }}
                            </h3>
                            <div class="spp-flex">
                                <button class="spp-button spp-button--ghost" type="button" on:click=move |_| on_back_click()>
                                    "← Back to clusters"
                                </button>
                                <Show
                                    when=move || known_issue_id.is_some()
                                    fallback=|| ()
                                >
                                    <a
                                        class="spp-button spp-button--ghost"
                                        href="/issues?tab=known"
                                    >
                                        {format!("Known issue #{}", known_issue_id.unwrap_or_default())}
                                    </a>
                                </Show>
                            </div>
                        </div>

                        <dl class="spp-def-list">
                            <dt>"Summary"</dt>
                            <dd>{if summary.is_empty() { "—".to_string() } else { summary.clone() }}</dd>
                            <dt>"Category / product / feature"</dt>
                            <dd>{format!("{category} · {product} · {feature}")}</dd>
                            <dt>"Conversations / customers"</dt>
                            <dd>{format!("{count} · {customers}")}</dd>
                            <dt>"First / last seen"</dt>
                            <dd>{format!("{first_seen} → {last_seen}")}</dd>
                            <dt>"Trend"</dt>
                            <dd><span class="spp-badge">{trend}</span></dd>
                            <dt>"Provenance"</dt>
                            <dd>{if provenance.is_empty() { "—".to_string() } else { provenance.clone() }}</dd>
                        </dl>

                        <div class="spp-modal__actions spp-mt-8">
                            <button
                                class="spp-button spp-button--primary"
                                type="button"
                                disabled=move || promoting.get()
                                on:click=move |_| promoting.set(true)
                            >
                                {move || if promoting.get() { "Opening editor…" } else { "Promote to known issue" }}
                            </button>
                            <button class="spp-button spp-button--ghost" type="button" on:click=do_delete_row>
                                "Delete cluster"
                            </button>
                        </div>
                    </div>

                    <div class="spp-card">
                        <h3 class="spp-card__title">"Member conversations"</h3>
                        <Show
                            when=move || !conversations.with(|c| c.is_empty())
                            fallback=|| view! { <EmptyState message="No linked conversations." /> }
                        >
                            <table class="spp-table">
                                <thead>
                                    <tr>
                                        <th>"Number"</th>
                                        <th>"Subject"</th>
                                        <th>"Status"</th>
                                        <th>"Created"</th>
                                    </tr>
                                </thead>
                                <tbody>
                                    {conversations.get().iter().map(|cv| {
                                        let cid = int_field(cv, "id");
                                        let number = int_field(cv, "number");
                                        let subject = field(cv, "subject");
                                        let status = field(cv, "status");
                                        let created = field(cv, "remote_created_at");
                                        view! {
                                            <tr>
                                                <td>
                                                    <a class="spp-text-xs" href={format!("/inbox/conversation/{cid}")}>
                                                        {format!("#{number}")}
                                                    </a>
                                                </td>
                                                <td>{subject}</td>
                                                <td><span class="spp-badge">{status}</span></td>
                                                <td class="spp-table__cell-muted">{created}</td>
                                            </tr>
                                        }
                                    }).collect::<Vec<_>>()}
                                </tbody>
                            </table>
                        </Show>
                    </div>
                }.into_view()
            }}
        </Show>

        <Show when=move || promoting.get() fallback=|| ()>
            {move || {
                let c = cluster.get_untracked();
                view! {
                    <KnownIssueEditor
                        seed=c
                        on_close=Rc::new(move || promoting.set(false))
                        on_saved=Rc::new(move || reload_stored.with_value(|f| f()))
                    />
                }
            }}
        </Show>
    }
}

const KNOWN_STATUSES: [&str; 5] = [
    "open",
    "investigating",
    "identified",
    "monitoring",
    "resolved",
];

const KNOWN_PROVENANCES: [&str; 2] = ["human_local", "ai_generated"];

#[component]
fn KnownTab() -> impl IntoView {
    let known = create_rw_signal(Vec::<serde_json::Value>::new());
    let loading = create_rw_signal(true);
    let error_msg = create_rw_signal(None::<String>);
    let selected = create_rw_signal(None::<i64>);
    let creating = create_rw_signal(None::<serde_json::Value>);
    let editing = create_rw_signal(None::<serde_json::Value>);

    let reload = create_rw_signal(0u32);
    create_effect(move |_| {
        let _ = reload.get();
        loading.set(true);
        error_msg.set(None);
        wasm_bindgen_futures::spawn_local(async move {
            match crate::api::get_json::<serde_json::Value>("/api/issues/known").await {
                Ok(v) => known.set(
                    v.get("known_issues")
                        .and_then(|k| k.as_array())
                        .cloned()
                        .unwrap_or_default(),
                ),
                Err(e) => error_msg.set(Some(e)),
            }
            loading.set(false);
        });
    });

    let back_to_list = StoredValue::new(Rc::new(move || selected.set(None)) as Rc<dyn Fn()>);
    let reload_known =
        StoredValue::new(Rc::new(move || reload.update(|t| *t += 1)) as Rc<dyn Fn()>);

    view! {
        <Show
            when=move || selected.get().is_none() && creating.get().is_none() && editing.get().is_none()
            fallback=|| ()
        >
            <div class="spp-flex spp-flex--between spp-mb-8">
                <p class="spp-muted spp-text-xs">
                    "Known issues document diagnosed problems with customer-safe explanations. Links stay local; provenance marks whether a human or the AI created the record."
                </p>
                <button
                    class="spp-button spp-button--primary"
                    type="button"
                    on:click=move |_| creating.set(Some(serde_json::json!({})))
                >
                    "+ New known issue"
                </button>
            </div>

            <ListStates loading=loading error_msg=error_msg empty="No known issues documented yet." />
            <Show
                when=move || !loading.get() && error_msg.get().is_none() && !known.with(|k| k.is_empty())
                fallback=|| ()
            >
                <div class="spp-card">
                    <table class="spp-table">
                        <thead>
                            <tr>
                                <th>"Issue"</th>
                                <th>"Status"</th>
                                <th>"Conversations"</th>
                                <th>"Provenance"</th>
                                <th>"Last seen"</th>
                                <th><span class="spp-visually-hidden">"Actions"</span></th>
                            </tr>
                        </thead>
                        <tbody>
                            {move || known.get().iter().map(|k| {
                                let id = int_field(k, "id");
                                let title = {
                                    let t = field(k, "title");
                                    if t.is_empty() { "(untitled)".to_string() } else { t }
                                };
                                let symptoms = field(k, "symptoms");
                                let status = field(k, "status");
                                let count = int_field(k, "conversation_count");
                                let provenance = field(k, "provenance");
                                let provenance = if provenance.is_empty() { "—".to_string() } else { provenance };
                                let last_seen = field(k, "last_seen_at");
                                let last_seen = last_seen.get(..10).unwrap_or("—").to_string();

                                let open = move |_| selected.set(Some(id));
                                let do_edit = {
                                    let k = k.clone();
                                    move |ev: leptos::ev::MouseEvent| {
                                        ev.stop_propagation();
                                        editing.set(Some(k.clone()));
                                    }
                                };
                                let do_delete = {
                                    let title_for_confirm = title.clone();
                                    move |ev: leptos::ev::MouseEvent| {
                                        ev.stop_propagation();
                                        if !confirm_action(&format!(
                                            "Delete known issue \"{title_for_confirm}\"? Its conversation links, engineering refs and search-index entries are removed too."
                                        )) {
                                            return;
                                        }
                                        wasm_bindgen_futures::spawn_local(async move {
                                            let path = format!("/api/issues/known/{id}");
                                            match crate::api::delete_json::<serde_json::Value>(&path).await {
                                                Ok(r) => {
                                                    crate::toasts::success(
                                                        r.get("message")
                                                            .and_then(|v| v.as_str())
                                                            .unwrap_or("Known issue deleted."),
                                                    );
                                                    reload.update(|t| *t += 1);
                                                }
                                                Err(e) => crate::toasts::error(e),
                                            }
                                        });
                                    }
                                };
                                let do_status = {
                                    move |ev: leptos::ev::Event| {
                                        ev.stop_propagation();
                                        let next = event_target_value(&ev);
                                        wasm_bindgen_futures::spawn_local(async move {
                                            let path = format!("/api/issues/known/{id}");
                                            let body = serde_json::json!({ "status": next });
                                            match crate::api::patch_json::<serde_json::Value>(&path, &body).await {
                                                Ok(r) if r.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) => {
                                                    crate::toasts::success("Status updated.");
                                                    reload.update(|t| *t += 1);
                                                }
                                                Ok(r) => crate::toasts::error(
                                                    r.get("message")
                                                        .and_then(|v| v.as_str())
                                                        .unwrap_or("Update failed"),
                                                ),
                                                Err(e) => crate::toasts::error(e),
                                            }
                                        });
                                    }
                                };

                                view! {
                                    <tr class="spp-table__row-clickable" on:click=open>
                                        <td>
                                            <strong>{title}</strong>
                                            {if !symptoms.is_empty() {
                                                view! { <div class="spp-muted spp-text-xs">{symptoms}</div> }.into_view()
                                            } else {
                                                ().into_view()
                                            }}
                                        </td>
                                        <td>
                                            <select
                                                class="spp-input spp-input--tiny"
                                                aria-label="Status"
                                                prop:value=move || status.clone()
                                                on:change=do_status
                                            >
                                                {KNOWN_STATUSES.iter().map(|s| {
                                                    view! { <option value={*s}>{*s}</option> }
                                                }).collect::<Vec<_>>()}
                                            </select>
                                        </td>
                                        <td><span class="spp-badge">{count.to_string()}</span></td>
                                        <td class="spp-table__cell-muted">{provenance}</td>
                                        <td class="spp-table__cell-muted">{last_seen}</td>
                                        <td class="spp-table__cell-actions">
                                            <button
                                                class="spp-button spp-button--ghost spp-button--tiny"
                                                type="button"
                                                on:click=do_edit
                                            >
                                                "Edit"
                                            </button>
                                            <button
                                                class="spp-button spp-button--ghost spp-button--tiny"
                                                type="button"
                                                on:click=do_delete
                                            >
                                                "Delete"
                                            </button>
                                        </td>
                                    </tr>
                                }
                            }).collect::<Vec<_>>()}
                        </tbody>
                    </table>
                </div>
            </Show>
        </Show>

        <Show when=move || selected.get().is_some() fallback=|| ()>
            {move || {
                let id = selected.get().unwrap_or_default();
                let on_back = back_to_list.with_value(Rc::clone);
                let reload_fn = reload_known.with_value(Rc::clone);
                view! {
                    <KnownIssueDetail id=id on_back=on_back reload=reload_fn />
                }
            }}
        </Show>

        <Show when=move || creating.get().is_some() fallback=|| ()>
            {move || {
                let seed = creating.get().unwrap_or_default();
                view! {
                    <KnownIssueEditor
                        seed=seed
                        on_close=Rc::new(move || creating.set(None))
                        on_saved=Rc::new(move || reload.update(|t| *t += 1))
                    />
                }
            }}
        </Show>

        <Show when=move || editing.get().is_some() fallback=|| ()>
            {move || {
                let existing = editing.get().unwrap_or_default();
                view! {
                    <KnownIssueEditor
                        existing=existing
                        on_close=Rc::new(move || editing.set(None))
                        on_saved=Rc::new(move || reload.update(|t| *t += 1))
                    />
                }
            }}
        </Show>
    }
}

#[component]
fn KnownIssueDetail(id: i64, on_back: Rc<dyn Fn()>, reload: Rc<dyn Fn()>) -> impl IntoView {
    let issue = create_rw_signal(serde_json::json!({}));
    let conversations = create_rw_signal(Vec::<serde_json::Value>::new());
    let loading = create_rw_signal(true);
    let error_msg = create_rw_signal(None::<String>);
    let impact = create_rw_signal(serde_json::json!({}));
    let link_input = create_rw_signal(String::new());
    let ref_system = create_rw_signal(String::new());
    let ref_reference_id = create_rw_signal(String::new());
    let ref_url = create_rw_signal(String::new());
    let ref_title = create_rw_signal(String::new());
    let ref_status = create_rw_signal(String::new());
    let ref_notes = create_rw_signal(String::new());
    let editing = create_rw_signal(false);

    let refresh = create_rw_signal(0u32);
    create_effect(move |_| {
        let _ = refresh.get();
        loading.set(true);
        error_msg.set(None);
        wasm_bindgen_futures::spawn_local(async move {
            let path = format!("/api/issues/known/{id}");
            match crate::api::get_json::<serde_json::Value>(&path).await {
                Ok(v) => {
                    issue.set(v.get("known_issue").cloned().unwrap_or_default());
                    conversations.set(
                        v.get("conversations")
                            .and_then(|c| c.as_array())
                            .cloned()
                            .unwrap_or_default(),
                    );
                }
                Err(e) => error_msg.set(Some(e)),
            }
            loading.set(false);
        });
        wasm_bindgen_futures::spawn_local(async move {
            let path = format!("/api/issues/known/{id}/impact");
            match crate::api::get_json::<serde_json::Value>(&path).await {
                Ok(v) => impact.set(v.get("impact").cloned().unwrap_or_default()),
                Err(_) => impact.set(serde_json::json!({})),
            }
        });
    });

    let reload_stored = StoredValue::new(reload);

    let do_unlink = move |conversation_id: i64| {
        wasm_bindgen_futures::spawn_local(async move {
            let path = format!("/api/issues/known/{id}/link/{conversation_id}");
            match crate::api::delete_json::<serde_json::Value>(&path).await {
                Ok(r) => {
                    crate::toasts::success(
                        r.get("message")
                            .and_then(|v| v.as_str())
                            .unwrap_or("Conversation unlinked."),
                    );
                    refresh.update(|t| *t += 1);
                }
                Err(e) => crate::toasts::error(e),
            }
        });
    };

    let do_link = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let raw = link_input.get_untracked();
        let Ok(conversation_id) = raw.trim().parse::<i64>() else {
            crate::toasts::error("Enter the numeric conversation id to link.");
            return;
        };
        if conversation_id <= 0 {
            crate::toasts::error("Enter the numeric conversation id to link.");
            return;
        }
        wasm_bindgen_futures::spawn_local(async move {
            let path = format!("/api/issues/known/{id}/link/{conversation_id}");
            match crate::api::post_json::<serde_json::Value>(&path, None).await {
                Ok(r) if r.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) => {
                    crate::toasts::success("Conversation linked to known issue.");
                    link_input.set(String::new());
                    refresh.update(|t| *t += 1);
                }
                Ok(r) => crate::toasts::error(
                    r.get("message")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Link failed"),
                ),
                Err(e) => crate::toasts::error(e),
            }
        });
    };

    let do_add_ref = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let system = ref_system.get_untracked();
        let reference_id = ref_reference_id.get_untracked();
        if system.trim().is_empty() || reference_id.trim().is_empty() {
            crate::toasts::error("System and reference id are required.");
            return;
        }
        let body = serde_json::json!({
            "system": system.trim(),
            "reference_id": reference_id.trim(),
            "url": ref_url.get_untracked(),
            "title": ref_title.get_untracked(),
            "status": ref_status.get_untracked(),
            "notes": ref_notes.get_untracked(),
        });
        wasm_bindgen_futures::spawn_local(async move {
            let path = format!("/api/issues/known/{id}/refs");
            match crate::api::post_json::<serde_json::Value>(&path, Some(&body)).await {
                Ok(r) if r.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) => {
                    crate::toasts::success("Engineering reference added.");
                    ref_system.set(String::new());
                    ref_reference_id.set(String::new());
                    ref_url.set(String::new());
                    ref_title.set(String::new());
                    ref_status.set(String::new());
                    ref_notes.set(String::new());
                    refresh.update(|t| *t += 1);
                }
                Ok(r) => crate::toasts::error(
                    r.get("message")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Add failed"),
                ),
                Err(e) => crate::toasts::error(e),
            }
        });
    };

    let on_back_stored = StoredValue::new(on_back);
    let do_unlink_stored = StoredValue::new(do_unlink);
    let do_link_stored = StoredValue::new(do_link);
    let do_add_ref_stored = StoredValue::new(do_add_ref);

    view! {
        <ListStates loading=loading error_msg=error_msg empty="Known issue not found." />
        <Show
            when=move || !loading.get() && error_msg.get().is_none() && issue.with(value_loaded)
            fallback=|| ()
        >
            {move || {
                let k = issue.get();
                let title = field(&k, "title");
                let status = field(&k, "status");
                let provenance = field(&k, "provenance");
                let refs = k.get("engineering_refs")
                    .and_then(|v| v.as_array())
                    .cloned()
                    .unwrap_or_default();
                let refs_rows = refs.clone();
                let impact_v = impact.get();
                let impact_card = impact_v.clone();
                let has_impact = value_loaded(&impact_v);
                let on_back_click = on_back_stored.with_value(Rc::clone);
                let do_link_row = do_link_stored.with_value(Clone::clone);
                let do_add_ref_row = do_add_ref_stored.with_value(Clone::clone);
                let do_unlink_source = do_unlink_stored.with_value(Clone::clone);

                view! {
                    <div class="spp-card">
                        <div class="spp-flex spp-flex--between">
                            <h3 class="spp-card__title">{title}</h3>
                            <div class="spp-flex">
                                <button class="spp-button spp-button--ghost" type="button" on:click=move |_| on_back_click()>
                                    "← Back to known issues"
                                </button>
                                <button
                                    class="spp-button spp-button--primary"
                                    type="button"
                                    on:click=move |_| editing.set(true)
                                >
                                    "Edit"
                                </button>
                            </div>
                        </div>

                        <div class="spp-flex spp-mt-8">
                            <span class=match status.as_str() {
                                "resolved" | "monitoring" => "spp-badge spp-badge--ok",
                                "investigating" | "identified" => "spp-badge spp-badge--warn",
                                _ => "spp-badge",
                            }>
                                {status}
                            </span>
                            <span class="spp-badge">{format!("provenance: {provenance}")}</span>
                        </div>

                        <dl class="spp-def-list">
                            <dt>"Symptoms"</dt>
                            <dd>{field(&k, "symptoms")}</dd>
                            <dt>"Product / feature"</dt>
                            <dd>{format!("{} · {}", field(&k, "product"), field(&k, "feature"))}</dd>
                            <dt>"Known cause"</dt>
                            <dd>{field(&k, "known_cause")}</dd>
                            <dt>"Workaround"</dt>
                            <dd>{field(&k, "workaround")}</dd>
                            <dt>"Customer-safe explanation"</dt>
                            <dd>{field(&k, "customer_safe_explanation")}</dd>
                            <dt>"Internal explanation"</dt>
                            <dd>{field(&k, "internal_explanation")}</dd>
                            <dt>"First / last seen"</dt>
                            <dd>{format!("{} → {}", field(&k, "first_seen_at"), field(&k, "last_seen_at"))}</dd>
                        </dl>
                    </div>

                    <Show when=move || has_impact fallback=|| ()>
                        <ImpactCard impact=impact_card.clone() />
                    </Show>

                    <div class="spp-card">
                        <h3 class="spp-card__title">"Linked conversations"</h3>
                        <form class="spp-flex spp-mb-8" on:submit=do_link_row>
                            <label class="spp-form-grid__label" for="known-link-input">"Link conversation"</label>
                            <input
                                id="known-link-input"
                                class="spp-input"
                                type="number"
                                min=1
                                placeholder="Conversation id"
                                prop:value=link_input
                                on:input=move |ev| link_input.set(event_target_value(&ev))
                            />
                            <button class="spp-button spp-button--primary" type="submit">"Link"</button>
                        </form>
                        <Show
                            when=move || !conversations.with(|c| c.is_empty())
                            fallback=|| view! { <EmptyState message="No linked conversations yet." /> }
                        >
                            <table class="spp-table">
                                <thead>
                                    <tr>
                                        <th>"Number"</th>
                                        <th>"Subject"</th>
                                        <th>"Status"</th>
                                        <th><span class="spp-visually-hidden">"Actions"</span></th>
                                    </tr>
                                </thead>
                                <tbody>
                                    {move || conversations.get().iter().map(|cv| {
                                        let cid = int_field(cv, "id");
                                        let number = int_field(cv, "number");
                                        let subject = field(cv, "subject");
                                        let status = field(cv, "status");
                                        let do_unlink = {
                                            let do_unlink = do_unlink_source;
                                            move |ev: leptos::ev::MouseEvent| {
                                                ev.stop_propagation();
                                                do_unlink(cid);
                                            }
                                        };
                                        view! {
                                            <tr>
                                                <td>
                                                    <a class="spp-text-xs" href={format!("/inbox/conversation/{cid}")}>
                                                        {format!("#{number}")}
                                                    </a>
                                                </td>
                                                <td>{subject}</td>
                                                <td><span class="spp-badge">{status}</span></td>
                                                <td class="spp-table__cell-actions">
                                                    <button
                                                        class="spp-button spp-button--ghost spp-button--tiny"
                                                        type="button"
                                                        on:click=do_unlink
                                                    >
                                                        "Unlink"
                                                    </button>
                                                </td>
                                            </tr>
                                        }
                                    }).collect::<Vec<_>>()}
                                </tbody>
                            </table>
                        </Show>
                    </div>

                    <div class="spp-card">
                        <h3 class="spp-card__title">"Engineering references"</h3>
                        <Show
                            when=move || !refs.is_empty()
                            fallback=|| view! { <EmptyState message="No engineering references recorded yet." /> }
                        >
                            <table class="spp-table spp-mb-8">
                                <thead>
                                    <tr>
                                        <th>"System"</th>
                                        <th>"Reference"</th>
                                        <th>"Title"</th>
                                        <th>"Status"</th>
                                        <th>"Notes"</th>
                                    </tr>
                                </thead>
                                <tbody>
                                    {refs_rows.iter().map(|r| {
                                        let url = field(r, "url");
                                        let link_view = if url.is_empty() {
                                            view! { <span>{field(r, "reference_id")}</span> }.into_view()
                                        } else {
                                            view! {
                                                <a class="spp-text-xs" href=url target="_blank" rel="noopener noreferrer">
                                                    {field(r, "reference_id")}
                                                </a>
                                            }.into_view()
                                        };
                                        view! {
                                            <tr>
                                                <td>{field(r, "system")}</td>
                                                <td>{link_view}</td>
                                                <td>{field(r, "title")}</td>
                                                <td><span class="spp-badge">{field(r, "status")}</span></td>
                                                <td class="spp-table__cell-muted">{field(r, "notes")}</td>
                                            </tr>
                                        }
                                    }).collect::<Vec<_>>()}
                                </tbody>
                            </table>
                        </Show>

                        <form on:submit=do_add_ref_row>
                            <div class="spp-form-grid">
                                <label class="spp-form-grid__label">"System *"</label>
                                <input
                                    class="spp-input"
                                    maxlength=100
                                    placeholder="e.g. JIRA"
                                    prop:value=ref_system
                                    on:input=move |ev| ref_system.set(event_target_value(&ev))
                                />
                                <label class="spp-form-grid__label">"Reference id *"</label>
                                <input
                                    class="spp-input"
                                    maxlength=200
                                    placeholder="e.g. SUP-1234"
                                    prop:value=ref_reference_id
                                    on:input=move |ev| ref_reference_id.set(event_target_value(&ev))
                                />
                                <label class="spp-form-grid__label">"URL"</label>
                                <input
                                    class="spp-input"
                                    maxlength=2000
                                    placeholder="https://…"
                                    prop:value=ref_url
                                    on:input=move |ev| ref_url.set(event_target_value(&ev))
                                />
                                <label class="spp-form-grid__label">"Title"</label>
                                <input
                                    class="spp-input"
                                    maxlength=500
                                    prop:value=ref_title
                                    on:input=move |ev| ref_title.set(event_target_value(&ev))
                                />
                                <label class="spp-form-grid__label">"Status"</label>
                                <input
                                    class="spp-input"
                                    maxlength=100
                                    placeholder="e.g. in progress"
                                    prop:value=ref_status
                                    on:input=move |ev| ref_status.set(event_target_value(&ev))
                                />
                                <label class="spp-form-grid__label">"Notes"</label>
                                <textarea
                                    class="spp-input"
                                    rows=2
                                    maxlength=5000
                                    prop:value=ref_notes
                                    on:input=move |ev| ref_notes.set(event_target_value(&ev))
                                ></textarea>
                            </div>
                            <div class="spp-modal__actions spp-mt-8">
                                <button class="spp-button spp-button--primary" type="submit">"Add reference"</button>
                            </div>
                        </form>
                    </div>
                }.into_view()
            }}
        </Show>

        <Show when=move || editing.get() fallback=|| ()>
            {move || {
                let existing = issue.get_untracked();
                view! {
                    <KnownIssueEditor
                        existing=existing
                        on_close=Rc::new(move || editing.set(false))
                        on_saved=Rc::new(move || {
                            refresh.update(|t| *t += 1);
                            reload_stored.with_value(|f| f());
                        })
                    />
                }
            }}
        </Show>
    }
}

#[component]
fn ImpactCard(impact: serde_json::Value) -> impl IntoView {
    let conversations = int_field(&impact, "affected_conversations");
    let customers = int_field(&impact, "affected_customers");
    let organizations = impact
        .get("affected_organizations")
        .and_then(|v| v.as_i64());
    let first_seen = field(&impact, "first_seen_at");
    let last_seen = field(&impact, "last_seen_at");
    let growth = impact.get("growth_rate_7d").cloned().unwrap_or_default();
    let recent = int_field(&growth, "recent");
    let previous = int_field(&growth, "previous");
    let direction = field(&growth, "direction");
    let trend = field(&impact, "trend");
    let waiting = int_field(&impact, "customer_waiting_count");
    let dist = impact
        .get("open_closed_distribution")
        .cloned()
        .unwrap_or_default();
    let open = int_field(&dist, "open");
    let closed = int_field(&dist, "closed");
    let note = field(&impact, "note");

    let pair_rows = |key: &str, label_key: &str| -> Vec<(String, i64)> {
        impact
            .get(key)
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .map(|p| (field(p, label_key), int_field(p, "conversations")))
                    .collect()
            })
            .unwrap_or_default()
    };
    let inboxes = pair_rows("affected_inboxes", "mailbox");
    let tags = pair_rows("top_tags", "tag");
    let products = pair_rows("products", "product");
    let has_inboxes = !inboxes.is_empty();
    let inboxes_view = inboxes.clone();
    let has_tags = !tags.is_empty();
    let tags_view = tags.clone();
    let has_products = !products.is_empty();
    let products_view = products.clone();
    let has_note = !note.is_empty();
    let note_view = note.clone();

    view! {
        <div class="spp-card">
            <h3 class="spp-card__title">"Impact"</h3>
            <div class="spp-radar-grid">
                <div class="spp-radar-card">
                    <span class="spp-radar-card__label">"Conversations"</span>
                    <span class="spp-radar-card__value">{conversations.to_string()}</span>
                    <span class="spp-radar-card__hint">{format!("{open} open · {closed} closed · {waiting} waiting")}</span>
                </div>
                <div class="spp-radar-card">
                    <span class="spp-radar-card__label">"Customers"</span>
                    <span class="spp-radar-card__value">{customers.to_string()}</span>
                    <span class="spp-radar-card__hint">
                        {match organizations {
                            Some(n) => format!("{n} organizations"),
                            None => "organizations unknown".to_string(),
                        }}
                    </span>
                </div>
                <div class="spp-radar-card">
                    <span class="spp-radar-card__label">"7-day growth"</span>
                    <span class="spp-radar-card__value">{direction}</span>
                    <span class="spp-radar-card__hint">{format!("{recent} recent vs {previous} previous · trend {trend}")}</span>
                </div>
            </div>

            <p class="spp-muted spp-text-xs">
                {format!("First seen {first_seen} · last seen {last_seen}")}
            </p>

            <Show when=move || has_inboxes fallback=|| ()>
                <p class="spp-muted spp-text-xs">
                    "Inboxes: "
                    {inboxes_view.iter().map(|(name, n)| {
                        view! { <span class="spp-badge">{format!("{name}: {n}")}</span> }
                    }).collect::<Vec<_>>()}
                </p>
            </Show>
            <Show when=move || has_tags fallback=|| ()>
                <p class="spp-muted spp-text-xs">
                    "Tags: "
                    {tags_view.iter().map(|(name, n)| {
                        view! { <span class="spp-badge">{format!("{name}: {n}")}</span> }
                    }).collect::<Vec<_>>()}
                </p>
            </Show>
            <Show when=move || has_products fallback=|| ()>
                <p class="spp-muted spp-text-xs">
                    "Products: "
                    {products_view.iter().map(|(name, n)| {
                        view! { <span class="spp-badge">{format!("{name}: {n}")}</span> }
                    }).collect::<Vec<_>>()}
                </p>
            </Show>

            <Show when=move || has_note fallback=|| ()>
                <p class="spp-muted spp-text-xs">{note_view.clone()}</p>
            </Show>
        </div>
    }
}

#[component]
fn KnownIssueEditor(
    #[prop(optional)] existing: Option<serde_json::Value>,
    #[prop(optional)] seed: Option<serde_json::Value>,
    on_close: Rc<dyn Fn()>,
    on_saved: Rc<dyn Fn()>,
) -> impl IntoView {
    let edit_row = existing.unwrap_or_default();
    let is_edit = int_field(&edit_row, "id") > 0;
    let seed = seed.unwrap_or_default();

    let prefill = |key: &str| -> String {
        if is_edit {
            field(&edit_row, key)
        } else {
            field(&seed, key)
        }
    };
    let seed_symptoms = if is_edit {
        String::new()
    } else {
        field(&seed, "summary")
    };

    let title = create_rw_signal(prefill("title"));
    let symptoms = create_rw_signal(if is_edit {
        prefill("symptoms")
    } else {
        seed_symptoms
    });
    let product = create_rw_signal(prefill("product"));
    let feature = create_rw_signal(prefill("feature"));
    let known_cause = create_rw_signal(if is_edit {
        prefill("known_cause")
    } else {
        String::new()
    });
    let workaround = create_rw_signal(if is_edit {
        prefill("workaround")
    } else {
        String::new()
    });
    let customer_safe_explanation = create_rw_signal(if is_edit {
        prefill("customer_safe_explanation")
    } else {
        String::new()
    });
    let internal_explanation = create_rw_signal(if is_edit {
        prefill("internal_explanation")
    } else {
        String::new()
    });
    let status = create_rw_signal({
        let s = prefill("status");
        if KNOWN_STATUSES.contains(&s.as_str()) {
            s
        } else {
            "investigating".to_string()
        }
    });
    let provenance = create_rw_signal("human_local".to_string());
    let submitting = create_rw_signal(false);
    let on_close_view = Rc::clone(&on_close);

    let seed_conversation_ids: Vec<i64> = if is_edit {
        Vec::new()
    } else {
        seed.get("conversation_ids")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|c| c.as_i64()).collect())
            .unwrap_or_default()
    };
    let seed_count = seed_conversation_ids.len();

    let submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        if submitting.get() {
            return;
        }
        let title = title.get_untracked();
        if title.trim().is_empty() {
            crate::toasts::error("Title is required (1-300 characters).");
            return;
        }
        submitting.set(true);
        let is_edit = is_edit;
        let id = int_field(&edit_row, "id");
        let on_close = Rc::clone(&on_close);
        let on_saved = Rc::clone(&on_saved);
        let seed_ids = seed_conversation_ids.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let result = if is_edit {
                let body = serde_json::json!({
                    "title": title,
                    "symptoms": symptoms.get_untracked(),
                    "product": product.get_untracked(),
                    "feature": feature.get_untracked(),
                    "known_cause": known_cause.get_untracked(),
                    "workaround": workaround.get_untracked(),
                    "customer_safe_explanation": customer_safe_explanation.get_untracked(),
                    "internal_explanation": internal_explanation.get_untracked(),
                    "status": status.get_untracked(),
                });
                let path = format!("/api/issues/known/{id}");
                crate::api::patch_json::<serde_json::Value>(&path, &body).await
            } else {
                let body = serde_json::json!({
                    "title": title,
                    "symptoms": symptoms.get_untracked(),
                    "product": product.get_untracked(),
                    "feature": feature.get_untracked(),
                    "known_cause": known_cause.get_untracked(),
                    "workaround": workaround.get_untracked(),
                    "customer_safe_explanation": customer_safe_explanation.get_untracked(),
                    "internal_explanation": internal_explanation.get_untracked(),
                    "status": status.get_untracked(),
                    "provenance": provenance.get_untracked(),
                    "conversation_ids": seed_ids,
                });
                crate::api::post_json::<serde_json::Value>("/api/issues/known", Some(&body)).await
            };
            match result {
                Ok(r) if r.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) => {
                    crate::toasts::success(
                        r.get("message")
                            .and_then(|v| v.as_str())
                            .unwrap_or("Saved."),
                    );
                    on_close();
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
                    {if is_edit { "Edit known issue" } else { "New known issue" }}
                </h3>
                <p class="spp-modal__message">
                    "Known issues are local records — they never sync back to Help Scout. The customer-safe explanation is the only text intended for customers."
                    {if seed_count > 0 {
                        format!(" {seed_count} cluster conversations will be linked.")
                    } else {
                        String::new()
                    }}
                </p>
                <form on:submit=submit>
                    <div class="spp-form-grid">
                        <label class="spp-form-grid__label">"Title *"</label>
                        <input
                            class="spp-input"
                            maxlength=300
                            placeholder="Short, recognizable name"
                            prop:value=title
                            on:input=move |ev| title.set(event_target_value(&ev))
                        />

                        <label class="spp-form-grid__label">"Symptoms"</label>
                        <textarea
                            class="spp-input"
                            rows=3
                            maxlength=5000
                            placeholder="What customers report"
                            prop:value=symptoms
                            on:input=move |ev| symptoms.set(event_target_value(&ev))
                        ></textarea>

                        <label class="spp-form-grid__label">"Product"</label>
                        <input
                            class="spp-input"
                            maxlength=200
                            prop:value=product
                            on:input=move |ev| product.set(event_target_value(&ev))
                        />

                        <label class="spp-form-grid__label">"Feature"</label>
                        <input
                            class="spp-input"
                            maxlength=200
                            prop:value=feature
                            on:input=move |ev| feature.set(event_target_value(&ev))
                        />

                        <label class="spp-form-grid__label">"Known cause"</label>
                        <textarea
                            class="spp-input"
                            rows=3
                            maxlength=10000
                            placeholder="The diagnosed root cause"
                            prop:value=known_cause
                            on:input=move |ev| known_cause.set(event_target_value(&ev))
                        ></textarea>

                        <label class="spp-form-grid__label">"Workaround"</label>
                        <textarea
                            class="spp-input"
                            rows=3
                            maxlength=10000
                            prop:value=workaround
                            on:input=move |ev| workaround.set(event_target_value(&ev))
                        ></textarea>

                        <label class="spp-form-grid__label">"Customer-safe explanation"</label>
                        <textarea
                            class="spp-input"
                            rows=3
                            maxlength=10000
                            placeholder="Safe to paste into a reply"
                            prop:value=customer_safe_explanation
                            on:input=move |ev| customer_safe_explanation.set(event_target_value(&ev))
                        ></textarea>

                        <label class="spp-form-grid__label">"Internal explanation"</label>
                        <textarea
                            class="spp-input"
                            rows=3
                            maxlength=20000
                            placeholder="Engineering detail — never sent to customers"
                            prop:value=internal_explanation
                            on:input=move |ev| internal_explanation.set(event_target_value(&ev))
                        ></textarea>

                        <label class="spp-form-grid__label">"Status"</label>
                        <select
                            class="spp-input"
                            aria-label="Status"
                            prop:value=move || status.get()
                            on:change=move |ev| status.set(event_target_value(&ev))
                        >
                            {KNOWN_STATUSES.iter().map(|s| {
                                view! { <option value={*s}>{*s}</option> }
                            }).collect::<Vec<_>>()}
                        </select>

                        <Show when=move || !is_edit fallback=|| ()>
                            <label class="spp-form-grid__label">"Provenance"</label>
                            <select
                                class="spp-input"
                                aria-label="Provenance"
                                prop:value=move || provenance.get()
                                on:change=move |ev| provenance.set(event_target_value(&ev))
                            >
                                {KNOWN_PROVENANCES.iter().map(|p| {
                                    view! { <option value={*p}>{*p}</option> }
                                }).collect::<Vec<_>>()}
                            </select>
                        </Show>
                    </div>
                    <div class="spp-modal__actions spp-mt-8">
                        <button class="spp-button spp-button--ghost" type="button" on:click=move |_| on_close_view()>
                            "Cancel"
                        </button>
                        <button
                            class="spp-button spp-button--primary"
                            type="submit"
                            disabled=move || submitting.get()
                        >
                            {move || if submitting.get() { "Saving…" } else { "Save known issue" }}
                        </button>
                    </div>
                </form>
            </div>
        </div>
    }
}

#[component]
fn GapsTabLite() -> impl IntoView {
    view! {
        <GapsTab />
    }
}

const REUSE_RANGES: [(u32, &str); 4] = [
    (7, "7 days"),
    (30, "30 days"),
    (90, "90 days"),
    (365, "1 year"),
];

#[component]
fn ReuseTab() -> impl IntoView {
    let query_map = use_query_map();
    let location = leptos_router::use_location();
    let navigate = use_navigate();

    let days = create_memo(move |_| {
        let m = query_map.get();
        crate::url_state::query_pos_int(&m, "days")
            .map(|d| (d as u32).clamp(1, 3650))
            .unwrap_or(90)
    });

    let rows = create_rw_signal(Vec::<serde_json::Value>::new());
    let loading = create_rw_signal(true);
    let error_msg = create_rw_signal(None::<String>);

    create_effect(move |_| {
        let days = days.get();
        let rows = rows;
        let loading = loading;
        let error_msg = error_msg;
        loading.set(true);
        error_msg.set(None);
        wasm_bindgen_futures::spawn_local(async move {
            let path = format!("/api/reports/answer-reuse?days={days}");
            match crate::api::get_json::<serde_json::Value>(&path).await {
                Ok(v) => rows.set(
                    v.get("candidates")
                        .and_then(|c| c.as_array())
                        .cloned()
                        .unwrap_or_default(),
                ),
                Err(e) => error_msg.set(Some(e)),
            }
            loading.set(false);
        });
    });

    let set_days = {
        let navigate = navigate.clone();
        let pathname = location.pathname;
        move |next: u32| {
            let m = query_map.get_untracked();
            let mut pairs: Vec<crate::url_state::Param> = Vec::new();
            for k in ["tab", "days"] {
                if k == "days" {
                    pairs.push((k, Some(next.to_string())));
                } else if let Some(v) = crate::url_state::query_str(&m, k) {
                    pairs.push((k, Some(v)));
                }
            }
            crate::url_state::replace_query(&navigate, &pathname.get_untracked(), &pairs);
        }
    };

    view! {
        <div class="spp-flex spp-flex--between spp-mb-8">
            <p class="spp-muted spp-text-xs">
                "Questions asked in 2+ conversations with a shared published resolution. Detection is a recommendation — review before turning anything into a saved reply or document."
            </p>
            <div class="spp-flex spp-dashboard__ranges" role="group" aria-label="Date range">
                {REUSE_RANGES.iter().map(|(d, label)| {
                    let set_days = {
                        let set_days = set_days.clone();
                        let d = *d;
                        move |_| set_days(d)
                    };
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
                            on:click=set_days
                        >
                            {*label}
                        </button>
                    }
                }).collect::<Vec<_>>()}
            </div>
        </div>

        <ListStates loading=loading error_msg=error_msg empty="No repeated questions detected in the selected range." />
        <Show
            when=move || !loading.get() && error_msg.get().is_none() && !rows.with(|r| r.is_empty())
            fallback=|| ()
        >
            <div class="spp-card">
                <table class="spp-table">
                    <thead>
                        <tr>
                            <th>"Question"</th>
                            <th>"Conversations"</th>
                            <th>"Common resolution"</th>
                            <th>"Saved reply"</th>
                            <th>"Knowledge doc"</th>
                        </tr>
                    </thead>
                    <tbody>
                        {move || rows.get().iter().map(|r| {
                            let question = field(r, "question");
                            let count = int_field(r, "conversation_count");
                            let resolution = {
                                let res = field(r, "common_resolution");
                                res.chars().take(160).collect::<String>()
                            };
                            let saved_reply = field(r, "saved_reply_name");
                            let knowledge_doc = field(r, "knowledge_doc_title");
                            view! {
                                <tr>
                                    <td>{question}</td>
                                    <td><span class="spp-badge">{count.to_string()}</span></td>
                                    <td class="spp-table__cell-muted">{resolution}</td>
                                    <td>
                                        {if saved_reply.is_empty() {
                                            view! { <span class="spp-muted">"—"</span> }.into_view()
                                        } else {
                                            view! { <span class="spp-badge spp-badge--ok">{saved_reply}</span> }.into_view()
                                        }}
                                    </td>
                                    <td>
                                        {if knowledge_doc.is_empty() {
                                            view! { <span class="spp-muted">"—"</span> }.into_view()
                                        } else {
                                            view! { <span class="spp-badge spp-badge--ok">{knowledge_doc}</span> }.into_view()
                                        }}
                                    </td>
                                </tr>
                            }
                        }).collect::<Vec<_>>()}
                    </tbody>
                </table>
            </div>
        </Show>
    }
}
