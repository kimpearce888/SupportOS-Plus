use leptos::prelude::*;
use std::sync::Arc;

use crate::components::state_view::EmptyState;
use crate::toasts;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutomationRuleView {
    pub id: i64,
    pub name: String,
    pub trigger: TriggerView,
    pub action: ActionView,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TriggerView {
    StatusChanged {
        from_status: Option<String>,
        to_status: Option<String>,
    },
    TagAdded {
        tag: String,
    },
    SlaRisk,
}

impl TriggerView {
    #[must_use]
    pub fn label(&self) -> String {
        match self {
            Self::StatusChanged {
                from_status,
                to_status,
            } => {
                let from = from_status.clone().unwrap_or_else(|| "*".into());
                let to = to_status.clone().unwrap_or_else(|| "*".into());
                format!("Status: {from} → {to}")
            }
            Self::TagAdded { tag } => format!("Tag added: {tag}"),
            Self::SlaRisk => "SLA at risk".into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionView {
    Assign { assignee_remote_id: i64 },
    AddTag { tag: String },
    SendNote { body: String },
    ChangeStatus { new_status: String },
    SetPriority { new_priority: String },
}

impl ActionView {
    #[must_use]
    pub fn label(&self) -> String {
        match self {
            Self::Assign { assignee_remote_id } => {
                format!("Assign to user #{assignee_remote_id}")
            }
            Self::AddTag { tag } => format!("Add tag: {tag}"),
            Self::SendNote { body } => {
                if body.len() > 32 {
                    format!("Note: {}…", &body[..32])
                } else {
                    format!("Note: {body}")
                }
            }
            Self::ChangeStatus { new_status } => format!("Set status: {new_status}"),
            Self::SetPriority { new_priority } => format!("Set priority: {new_priority}"),
        }
    }

    #[must_use]
    pub fn requires_approval(&self) -> bool {
        match self {
            Self::Assign { .. } | Self::ChangeStatus { .. } => true,
            Self::SetPriority { new_priority } => new_priority == "urgent",
            Self::AddTag { .. } | Self::SendNote { .. } => false,
        }
    }
}

#[component]
pub fn AutomationPage() -> impl IntoView {
    let rules = RwSignal::new(Vec::<AutomationRuleView>::new());
    let automation_enabled = RwSignal::new(None::<bool>);
    let risk_tier_note = RwSignal::new(String::new());
    let loading = RwSignal::new(true);
    let error_msg = RwSignal::new(None::<String>);
    let creating = RwSignal::new(false);
    let new_name = RwSignal::new(String::new());
    let new_trigger = RwSignal::new("sla_risk".to_string());
    let new_action = RwSignal::new("add_tag".to_string());
    let new_tag = RwSignal::new(String::new());
    let new_status = RwSignal::new("pending".to_string());
    let new_priority = RwSignal::new("medium".to_string());

    let load: Arc<dyn Fn() + Send + Sync> = Arc::new(move || {
        let rules = rules;
        let automation_enabled = automation_enabled;
        let risk_tier_note = risk_tier_note;
        let loading = loading;
        let error_msg = error_msg;
        wasm_bindgen_futures::spawn_local(async move {
            match crate::api::get_json::<serde_json::Value>("/api/automation/rules").await {
                Ok(data) => {
                    let views: Vec<AutomationRuleView> = data
                        .get("rules")
                        .and_then(|v| v.as_array())
                        .map(|arr| arr.iter().filter_map(parse_rule_view).collect())
                        .unwrap_or_default();
                    rules.set(views);
                    automation_enabled
                        .set(data.get("automation_enabled").and_then(|v| v.as_bool()));
                    risk_tier_note.set(
                        data.pointer("/risk_tiers/note")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                    );
                    error_msg.set(None);
                    loading.set(false);
                }
                Err(e) => {
                    error_msg.set(Some(e));
                    loading.set(false);
                }
            }
        });
    });
    load();
    let load_stored = StoredValue::new_local(Arc::clone(&load));

    let toggle_engine: Arc<dyn Fn(bool) + Send + Sync> = Arc::new({
        let load = Arc::clone(&load);
        move |on: bool| {
            let load = Arc::clone(&load);
            wasm_bindgen_futures::spawn_local(async move {
                let body = serde_json::json!({ "automation_enabled": on });
                match crate::api::patch_json::<serde_json::Value>("/api/settings", &body).await {
                    Ok(_) => {
                        toasts::success("Automation engine setting updated.");
                        load();
                    }
                    Err(e) => toasts::error(e),
                }
            });
        }
    });

    view! {
        <div class="spp-page spp-page--automation">
            <h2 class="spp-page__title">"Automation"</h2>
            <p class="spp-page__subtitle">
                "Rules trigger actions when conversations match certain conditions. "
                "High-impact actions (assigning, status change, urgent priority) require approval; "
                "low-impact actions (add tag, send note) execute directly."
            </p>

            <Show when=move || error_msg.get().is_some() fallback=|| ()>
                <div class="spp-state spp-state--error">
                    <span class="spp-state__icon" aria-hidden="true">"⚠"</span>
                    <p class="spp-state__body">{move || error_msg.get().unwrap_or_default()}</p>
                </div>
            </Show>

            <section class="spp-automation-summary">
                <div class="spp-rule-card__row">
                    <span class="spp-rule-card__label">"Automation:"</span>
                    <label class="spp-flex spp-gap-4" style="cursor:pointer;">
                        <input
                            type="checkbox"
                            prop:checked=move || automation_enabled.get().unwrap_or(false)
                            on:change=move |ev| {
                                let on = event_target_checked(&ev);
                                toggle_engine(on);
                            }
                        />
                        <span class="spp-text-sm">
                            {move || format!("Automation engine {}", if automation_enabled.get().unwrap_or(false) { "ON" } else { "OFF" })}
                        </span>
                    </label>
                </div>
                {move || {
                    let note = risk_tier_note.get();
                    if note.is_empty() { ().into_any() } else {
                        view! { <p class="spp-page__subtitle">{note}</p> }.into_any()
                    }
                }}
            </section>

            <section class="spp-automation-rules">
                <div class="spp-automation-rules__header">
                    <h3 class="spp-automation-rules__title">"Rules"</h3>
                    <button
                        class="spp-button spp-button--primary"
                        on:click=move |_| creating.set(true)
                    >
                        "New Rule"
                    </button>
                </div>
                <Show
                    when=move || !rules.get().is_empty()
                    fallback=move || {
                        view! {
                            <EmptyState message="No automation rules yet. Create a rule to automate repetitive actions." />
                        }
                    }
                >
                    <RulesList rules=rules reload=Arc::clone(&load) />
                </Show>
            </section>

            <Show when=move || creating.get() fallback=|| ()>
                <div class="spp-overlay" role="dialog" aria-modal="true">
                    <div class="spp-modal">
                        <h3 class="spp-modal__title">"New Rule"</h3>
                        <div class="spp-form">
                            <label class="spp-form__label">
                                "Name"
                                <input
                                    class="spp-input"
                                    type="text"
                                    placeholder="Rule name…"
                                    prop:value=new_name
                                    on:input=move |ev| new_name.set(event_target_value(&ev))
                                />
                            </label>
                            <label class="spp-form__label">
                                "Trigger"
                                <select
                                    class="spp-input"
                                    prop:value=new_trigger
                                    on:change=move |ev| new_trigger.set(event_target_value(&ev))
                                >
                                    <option value="sla_risk">"SLA at risk"</option>
                                    <option value="status_changed">"Status changed"</option>
                                    <option value="tag_added">"Tag added"</option>
                                </select>
                            </label>
                            <label class="spp-form__label">
                                "Action"
                                <select
                                    class="spp-input"
                                    prop:value=new_action
                                    on:change=move |ev| new_action.set(event_target_value(&ev))
                                >
                                    <option value="add_tag">"Add tag"</option>
                                    <option value="send_note">"Send note"</option>
                                    <option value="change_status">"Change status"</option>
                                    <option value="set_priority">"Set priority"</option>
                                    <option value="assign">"Assign"</option>
                                </select>
                            </label>
                            <Show when=move || new_action.get() == "add_tag" fallback=|| ()>
                                <label class="spp-form__label">
                                    "Tag"
                                    <input
                                        class="spp-input"
                                        type="text"
                                        placeholder="tag-name"
                                        prop:value=new_tag
                                        on:input=move |ev| new_tag.set(event_target_value(&ev))
                                    />
                                </label>
                            </Show>
                            <Show when=move || new_action.get() == "change_status" fallback=|| ()>
                                <label class="spp-form__label">
                                    "New status"
                                    <select
                                        class="spp-input"
                                        prop:value=new_status
                                        on:change=move |ev| new_status.set(event_target_value(&ev))
                                    >
                                        <option value="active">"active"</option>
                                        <option value="pending">"pending"</option>
                                        <option value="closed">"closed"</option>
                                        <option value="spam">"spam"</option>
                                    </select>
                                </label>
                            </Show>
                            <Show when=move || new_action.get() == "set_priority" fallback=|| ()>
                                <label class="spp-form__label">
                                    "New priority"
                                    <select
                                        class="spp-input"
                                        prop:value=new_priority
                                        on:change=move |ev| new_priority.set(event_target_value(&ev))
                                    >
                                        <option value="none">"none"</option>
                                        <option value="low">"low"</option>
                                        <option value="medium">"medium"</option>
                                        <option value="high">"high"</option>
                                        <option value="urgent">"urgent"</option>
                                    </select>
                                </label>
                            </Show>
                        </div>
                        <div class="spp-modal__actions">
                            <button class="spp-button" on:click=move |_| creating.set(false)>
                                "Cancel"
                            </button>
                            <button
                                class="spp-button spp-button--primary"
                                on:click=move |_| {
                                    let name = new_name.get();
                                    if name.trim().is_empty() {
                                        return;
                                    }
                                    let trigger = new_trigger.get();
                                    let action = new_action.get();
                                    let body = serde_json::json!({
                                        "name": name,
                                        "trigger": {"kind": trigger},
                                        "actions": [{"kind": action}],
                                        "enabled": false,
                                        "priority": 100,
                                        "requires_approval": true,
                                    });
                                    let load = load_stored.with_value(Arc::clone);
                                    wasm_bindgen_futures::spawn_local(async move {
                                        let _ = crate::api::post_json::<serde_json::Value>(
                                            "/api/automation/rules",
                                            Some(&body),
                                        ).await;
                                        creating.set(false);
                                        load();
                                    });
                                }
                            >
                                "Create Rule"
                            </button>
                        </div>
                    </div>
                </div>
            </Show>
        </div>
    }
}

#[component]
fn RulesList(
    rules: RwSignal<Vec<AutomationRuleView>>,
    reload: Arc<dyn Fn() + Send + Sync>,
) -> impl IntoView {
    let cards = move || {
        rules
            .get()
            .into_iter()
            .map(|r| {
                view! {
                    <RuleCard rule=r reload=Arc::clone(&reload) />
                }
                .into_any()
            })
            .collect::<Vec<_>>()
    };
    view! {
        <div class="spp-rules-list">
            {cards}
        </div>
    }
}

#[component]
fn RuleCard(rule: AutomationRuleView, reload: Arc<dyn Fn() + Send + Sync>) -> impl IntoView {
    let name = rule.name.clone();
    let trigger_label = rule.trigger.label();
    let action_label = rule.action.label();
    let requires_approval = rule.action.requires_approval();
    let enabled = rule.enabled;
    let rule_id = rule.id;

    let toggle_rule = {
        let reload = Arc::clone(&reload);
        move |_| {
            let reload = Arc::clone(&reload);
            wasm_bindgen_futures::spawn_local(async move {
                let path = format!("/api/automation/rules/{rule_id}");
                let body = serde_json::json!({ "enabled": !enabled });
                match crate::api::patch_json::<serde_json::Value>(&path, &body).await {
                    Ok(_) => reload(),
                    Err(e) => toasts::error(e),
                }
            });
        }
    };
    let delete_rule = {
        let reload = Arc::clone(&reload);
        move |_| {
            let reload = Arc::clone(&reload);
            wasm_bindgen_futures::spawn_local(async move {
                let path = format!("/api/automation/rules/{rule_id}");
                match crate::api::delete_json::<serde_json::Value>(&path).await {
                    Ok(_) => reload(),
                    Err(e) => toasts::error(e),
                }
            });
        }
    };

    view! {
        <div class="spp-rule-card">
            <div class="spp-rule-card__header">
                <span class="spp-rule-card__name">{name}</span>
                <div class="spp-flex spp-gap-4">
                    <button
                        class="spp-rule-card__toggle"
                        type="button"
                        role="switch"
                        aria-checked=enabled
                        title=move || if enabled { "Click to disable" } else { "Click to enable" }
                        on:click=toggle_rule
                    >
                        {move || if enabled { "Enabled" } else { "Disabled" }}
                    </button>
                    <button
                        class="spp-button spp-button--ghost spp-button--tiny"
                        type="button"
                        on:click=delete_rule
                    >
                        "Delete"
                    </button>
                </div>
            </div>
            <div class="spp-rule-card__body">
                <div class="spp-rule-card__row">
                    <span class="spp-rule-card__label">"Trigger:"</span>
                    <span class="spp-rule-card__value">{trigger_label}</span>
                </div>
                <div class="spp-rule-card__row">
                    <span class="spp-rule-card__label">"Action:"</span>
                    <span class="spp-rule-card__value">{action_label}</span>
                    <Show when=move || requires_approval fallback=|| ().into_any()>
                        <span class="spp-rule-card__badge">"requires approval"</span>
                    </Show>
                </div>
                <div class="spp-rule-card__row">
                    <span class="spp-rule-card__label">"Rule ID:"</span>
                    <span class="spp-rule-card__value">#{rule_id}</span>
                </div>
            </div>
        </div>
    }
}

fn parse_rule_view(r: &serde_json::Value) -> Option<AutomationRuleView> {
    let id = r.get("id")?.as_i64()?;
    let name = r.get("name")?.as_str()?.to_string();
    let enabled = r.get("enabled").and_then(|v| v.as_bool()).unwrap_or(false);
    let trigger = match r.get("trigger")?.get("kind")?.as_str()? {
        "status_changed" => TriggerView::StatusChanged {
            from_status: r
                .pointer("/trigger/from_status")
                .and_then(|v| v.as_str())
                .map(str::to_string),
            to_status: r
                .pointer("/trigger/to_status")
                .and_then(|v| v.as_str())
                .map(str::to_string),
        },
        "tag_added" => TriggerView::TagAdded {
            tag: r
                .pointer("/trigger/tag")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string(),
        },
        "sla_risk" => TriggerView::SlaRisk,
        _ => return None,
    };
    let action = match r.get("action")?.get("kind")?.as_str()? {
        "assign" => ActionView::Assign {
            assignee_remote_id: r
                .pointer("/action/assignee_remote_id")
                .and_then(|v| v.as_i64())
                .unwrap_or_default(),
        },
        "add_tag" => ActionView::AddTag {
            tag: r
                .pointer("/action/tag")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string(),
        },
        "send_note" => ActionView::SendNote {
            body: r
                .pointer("/action/body")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string(),
        },
        "change_status" => ActionView::ChangeStatus {
            new_status: r
                .pointer("/action/new_status")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string(),
        },
        "set_priority" => ActionView::SetPriority {
            new_priority: r
                .pointer("/action/new_priority")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string(),
        },
        _ => return None,
    };
    Some(AutomationRuleView {
        id,
        name,
        trigger,
        action,
        enabled,
    })
}
