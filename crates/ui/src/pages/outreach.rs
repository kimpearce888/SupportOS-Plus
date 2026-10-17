use leptos::*;
use std::rc::Rc;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;

use crate::components::condition_editor::{
    default_node, describe_condition, inject_uid, strip_uid, ConditionEditor,
};
use crate::components::state_view::{EmptyState, LoadingState};
use crate::toasts;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tab {
    Campaigns,
    New,
    Segments,
    Dnc,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WizardStep {
    Audience,
    Recipients,
    Compose,
    Review,
}

impl WizardStep {
    fn label(self) -> &'static str {
        match self {
            Self::Audience => "Audience",
            Self::Recipients => "Recipients",
            Self::Compose => "Compose",
            Self::Review => "Final review",
        }
    }
}

#[derive(Debug, Clone, Default)]
struct Draft {
    name: String,
    subject: String,
    body: String,
    mailbox_local_id: i64,
    tags: String,
}

fn split_definition(
    def: &serde_json::Value,
) -> (String, Vec<serde_json::Value>, Vec<serde_json::Value>) {
    let combinator = if def.get("combinator").and_then(|v| v.as_str()) == Some("any") {
        "any".to_string()
    } else {
        "all".to_string()
    };
    let conditions = def
        .get("conditions")
        .and_then(|v| v.as_array())
        .map(|a| a.iter().map(inject_uid).collect())
        .unwrap_or_default();
    let exclude = def
        .get("exclude")
        .and_then(|v| v.as_array())
        .map(|a| a.iter().map(inject_uid).collect())
        .unwrap_or_default();
    (combinator, conditions, exclude)
}

#[must_use]
pub fn assemble_definition(
    combinator: &str,
    conditions: &[serde_json::Value],
    exclude: &[serde_json::Value],
) -> serde_json::Value {
    serde_json::json!({
        "combinator": if combinator == "any" { "any" } else { "all" },
        "conditions": conditions.iter().map(strip_uid).collect::<Vec<_>>(),
        "exclude": exclude.iter().map(strip_uid).collect::<Vec<_>>(),
    })
}

fn date_short(iso: &str) -> String {
    let compact = iso.replace('T', " ");
    let chars: Vec<char> = compact.chars().collect();
    if chars.len() >= 16 {
        chars[..16].iter().collect()
    } else {
        compact
    }
}

struct IntervalGuard {
    #[allow(dead_code)]
    id: Option<i32>,
    #[allow(dead_code)]
    closure: Option<wasm_bindgen::JsValue>,
}

impl Drop for IntervalGuard {
    fn drop(&mut self) {
        #[cfg(target_arch = "wasm32")]
        if let Some(id) = self.id {
            if let Some(w) = web_sys::window() {
                let _ = w.clear_interval_with_handle(id);
            }
        }
    }
}

fn page_interval(f: impl Fn() + 'static, ms: i32) -> IntervalGuard {
    #[cfg(target_arch = "wasm32")]
    {
        if let Some(w) = web_sys::window() {
            let closure: wasm_bindgen::closure::Closure<dyn Fn()> =
                wasm_bindgen::closure::Closure::new(f);
            if let Ok(id) = w.set_interval_with_callback_and_timeout_and_arguments_0(
                closure.as_ref().unchecked_ref(),
                ms,
            ) {
                let js = closure.into_js_value();
                return IntervalGuard {
                    id: Some(id),
                    closure: Some(js),
                };
            }
        }
        IntervalGuard {
            id: None,
            closure: None,
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = (f, ms);
        IntervalGuard {
            id: None,
            closure: None,
        }
    }
}

#[component]
pub fn OutreachPage() -> impl IntoView {
    let tab = create_rw_signal(Tab::Campaigns);
    let wizard_step = create_rw_signal(WizardStep::Audience);
    let combinator = create_rw_signal("all".to_string());
    let conditions = create_rw_signal(Vec::<serde_json::Value>::new());
    let exclude = create_rw_signal(Vec::<serde_json::Value>::new());
    let selected_ids = create_rw_signal(Vec::<i64>::new());
    let loading_recipients = create_rw_signal(false);
    let recipient_rows = create_rw_signal(Vec::<serde_json::Value>::new());
    let saved_segment_id = create_rw_signal(None::<i64>);
    let draft = create_rw_signal(Draft::default());
    let created_campaign_id = create_rw_signal(None::<i64>);
    let creating = create_rw_signal(false);

    let meta = create_rw_signal(None::<serde_json::Value>);
    let segments = create_rw_signal(Vec::<serde_json::Value>::new());
    let campaigns = create_rw_signal(Vec::<serde_json::Value>::new());
    let campaigns_error = create_rw_signal(None::<String>);
    let preview = create_rw_signal(None::<serde_json::Value>);
    let preview_loading = create_rw_signal(false);
    let preview_error = create_rw_signal(None::<String>);

    let load_meta = move || {
        let meta = meta;
        spawn_local(async move {
            match crate::api::get_json::<serde_json::Value>("/api/outreach/meta").await {
                Ok(v) => meta.set(Some(v)),
                Err(e) => toasts::error(e),
            }
        });
    };
    let load_segments = move || {
        let segments = segments;
        spawn_local(async move {
            match crate::api::get_json::<serde_json::Value>("/api/outreach/segments").await {
                Ok(v) => segments.set(
                    v.get("segments")
                        .and_then(|s| s.as_array())
                        .cloned()
                        .unwrap_or_default(),
                ),
                Err(e) => toasts::error(e),
            }
        });
    };
    let load_campaigns = move || {
        let campaigns = campaigns;
        let campaigns_error = campaigns_error;
        spawn_local(async move {
            match crate::api::get_json::<serde_json::Value>("/api/outreach/campaigns").await {
                Ok(v) => {
                    campaigns_error.set(None);
                    campaigns.set(
                        v.get("campaigns")
                            .and_then(|c| c.as_array())
                            .cloned()
                            .unwrap_or_default(),
                    );
                }
                Err(e) => campaigns_error.set(Some(e)),
            }
        });
    };
    load_meta();
    load_segments();
    load_campaigns();
    {
        let guard = page_interval(load_campaigns, 15_000);
        on_cleanup(move || drop(guard));
    }

    let debounced = create_rw_signal(None::<serde_json::Value>);
    create_effect(move |_| {
        let def = assemble_definition(&combinator.get(), &conditions.get(), &exclude.get());
        set_timeout(
            move || debounced.set(Some(def)),
            std::time::Duration::from_millis(300),
        );
    });
    create_effect(move |_| {
        let Some(def) = debounced.get() else { return };
        preview_loading.set(true);
        spawn_local(async move {
            let mut body = def;
            body["page"] = serde_json::json!(1);
            body["pageSize"] = serde_json::json!(100);
            match crate::api::post_json::<serde_json::Value>(
                "/api/outreach/segments/preview",
                Some(&body),
            )
            .await
            {
                Ok(v) => {
                    preview_error.set(None);
                    preview.set(Some(v));
                }
                Err(e) => preview_error.set(Some(e)),
            }
            preview_loading.set(false);
        });
    });

    let go_recipients = move || {
        if loading_recipients.get_untracked() {
            return;
        }
        let current = preview.get_untracked();
        let mut rows: Vec<serde_json::Value> = current
            .as_ref()
            .and_then(|p| p.get("rows"))
            .and_then(|r| r.as_array())
            .cloned()
            .unwrap_or_default();
        let matched = current
            .as_ref()
            .and_then(|p| p.get("matched"))
            .and_then(|m| m.as_i64())
            .unwrap_or(0);
        let def = assemble_definition(
            &combinator.get_untracked(),
            &conditions.get_untracked(),
            &exclude.get_untracked(),
        );
        loading_recipients.set(true);
        spawn_local(async move {
            let result: Result<(), String> = async {
                if matched > rows.len() as i64 {
                    let pages = (matched.min(5000) as f64 / 100.0).ceil() as i64;
                    for p in 2..=pages {
                        let mut body = def.clone();
                        body["page"] = serde_json::json!(p);
                        body["pageSize"] = serde_json::json!(100);
                        let next = crate::api::post_json::<serde_json::Value>(
                            "/api/outreach/segments/preview",
                            Some(&body),
                        )
                        .await?;
                        let next_rows = next
                            .get("rows")
                            .and_then(|r| r.as_array())
                            .cloned()
                            .unwrap_or_default();
                        if next_rows.is_empty() {
                            break;
                        }
                        rows.extend(next_rows);
                    }
                }
                Ok(())
            }
            .await;
            match result {
                Ok(()) => {
                    if (rows.len() as i64) < matched {
                        toasts::warning(format!(
                            "Loaded {} of {matched} matching recipients (the rest failed to load) - continue only if that is intended.",
                            rows.len()
                        ));
                    }
                    selected_ids.set(
                        rows.iter()
                            .filter(|r| {
                                !r.get("excluded").and_then(|v| v.as_bool()).unwrap_or(false)
                                    && r.get("chosen_email")
                                        .and_then(|v| v.as_str())
                                        .is_some_and(|e| !e.is_empty())
                            })
                            .filter_map(|r| r.get("customer_local_id").and_then(|v| v.as_i64()))
                            .collect(),
                    );
                    recipient_rows.set(rows);
                    wizard_step.set(WizardStep::Recipients);
                }
                Err(e) => toasts::error(e),
            }
            loading_recipients.set(false);
        });
    };

    let create_campaign = move || {
        if creating.get_untracked() {
            return;
        }
        let d = draft.get_untracked();
        creating.set(true);
        let body = serde_json::json!({
            "name": d.name,
            "subject": d.subject,
            "body": d.body,
            "mailbox_local_id": d.mailbox_local_id,
            "tags": d.tags.split(',').map(|t| t.trim()).filter(|t| !t.is_empty())
                .map(str::to_string).collect::<Vec<_>>(),
            "segment_id": saved_segment_id.get_untracked(),
            "definition": assemble_definition(
                &combinator.get_untracked(),
                &conditions.get_untracked(),
                &exclude.get_untracked(),
            ),
            "customer_ids": selected_ids.get_untracked(),
        });
        let load_campaigns = load_campaigns;
        spawn_local(async move {
            match crate::api::post_json::<serde_json::Value>("/api/outreach/campaigns", Some(&body))
                .await
            {
                Ok(r) => {
                    let ok = r.get("ok").and_then(|v| v.as_bool()).unwrap_or(false);
                    let message = r
                        .get("message")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Campaign creation failed")
                        .to_string();
                    if !ok {
                        toasts::error(message);
                    } else {
                        if let Some(id) = r.get("id").and_then(|v| v.as_i64()) {
                            created_campaign_id.set(Some(id));
                        }
                        load_campaigns();
                        toasts::success(message);
                    }
                }
                Err(e) => toasts::error(e),
            }
            creating.set(false);
        });
    };

    let done_wizard = move || {
        tab.set(Tab::Campaigns);
        wizard_step.set(WizardStep::Audience);
        combinator.set("all".to_string());
        conditions.set(Vec::new());
        exclude.set(Vec::new());
        draft.set(Draft::default());
        created_campaign_id.set(None);
        saved_segment_id.set(None);
        selected_ids.set(Vec::new());
        recipient_rows.set(Vec::new());
    };

    let apply_definition = move |def: serde_json::Value, seg_id: Option<i64>| {
        let (c, conds, excl) = split_definition(&def);
        combinator.set(c);
        conditions.set(conds);
        exclude.set(excl);
        saved_segment_id.set(seg_id);
    };

    view! {
        <div class="spp-page spp-page--outreach">
            <div class="spp-page__header">
                <div>
                    <h2 class="spp-page__title">"Outreach"</h2>
                    <p class="spp-page__intro">
                        "Contact-first segments \u{b7} individual Help Scout conversations \u{b7} full audit trail"
                    </p>
                </div>
            </div>

            <div class="spp-tabs">
                <button
                    class="spp-tab"
                    class:is-active=move || tab.get() == Tab::Campaigns
                    on:click=move |_| tab.set(Tab::Campaigns)
                >
                    "Campaigns"
                </button>
                <button
                    class="spp-tab"
                    class:is-active=move || tab.get() == Tab::New
                    on:click=move |_| tab.set(Tab::New)
                >
                    "New campaign"
                </button>
                <button
                    class="spp-tab"
                    class:is-active=move || tab.get() == Tab::Segments
                    on:click=move |_| tab.set(Tab::Segments)
                >
                    "Saved segments"
                </button>
                <button
                    class="spp-tab"
                    class:is-active=move || tab.get() == Tab::Dnc
                    on:click=move |_| tab.set(Tab::Dnc)
                >
                    "Do Not Contact"
                </button>
            </div>

            <Show when=move || tab.get() == Tab::Campaigns fallback=|| ()>
                <CampaignsPanel campaigns campaigns_error load_campaigns=Rc::new(load_campaigns) />
            </Show>

            <Show when=move || tab.get() == Tab::New fallback=|| ()>
                <div>
                    <div class="spp-wizard-steps">
                        {vec![
                            WizardStep::Audience,
                            WizardStep::Recipients,
                            WizardStep::Compose,
                            WizardStep::Review,
                        ]
                        .into_iter()
                        .enumerate()
                        .map(|(i, s)| {
                            view! {
                                <span class="spp-wizard-step" class:is-active=move || wizard_step.get() == s>
                                    {format!("{}. {}", i + 1, s.label())}
                                </span>
                            }
                        })
                        .collect::<Vec<_>>()}
                    </div>

                    <Show when=move || wizard_step.get() == WizardStep::Audience fallback=|| ()>
                        <AudienceStep
                            meta
                            combinator
                            conditions
                            exclude
                            preview
                            preview_loading
                            preview_error
                            segments
                            saved_segment_id
                            on_apply=Rc::new(move |def| apply_definition(def, None))
                            on_next=Rc::new(go_recipients)
                            loading_recipients
                            load_segments=Rc::new(load_segments)
                        />
                    </Show>
                    <Show when=move || wizard_step.get() == WizardStep::Recipients fallback=|| ()>
                        <RecipientsStep
                            rows=recipient_rows
                            selected_ids
                            on_back=Rc::new(move || wizard_step.set(WizardStep::Audience))
                            on_next=Rc::new(move || wizard_step.set(WizardStep::Compose))
                        />
                    </Show>
                    <Show when=move || wizard_step.get() == WizardStep::Compose fallback=|| ()>
                        <ComposeStep
                            meta
                            draft
                            preview
                            selected_ids
                            on_back=Rc::new(move || wizard_step.set(WizardStep::Recipients))
                            on_next=Rc::new(move || wizard_step.set(WizardStep::Review))
                        />
                    </Show>
                    <Show when=move || wizard_step.get() == WizardStep::Review fallback=|| ()>
                        <ReviewStep
                            draft
                            selected_ids
                            created_campaign_id
                            creating
                            create_campaign=Rc::new(create_campaign)
                            load_campaigns=Rc::new(load_campaigns)
                            on_back=Rc::new(move || wizard_step.set(WizardStep::Compose))
                            on_done=Rc::new(done_wizard)
                        />
                    </Show>
                </div>
            </Show>

            <Show when=move || tab.get() == Tab::Segments fallback=|| ()>
                <SegmentsPanel
                    segments
                    load_segments=Rc::new(load_segments)
                    on_use=Rc::new(move |s: serde_json::Value| {
                        apply_definition(
                            s.get("definition").cloned().unwrap_or(serde_json::json!({})),
                            s.get("id").and_then(|v| v.as_i64()),
                        );
                        tab.set(Tab::New);
                        wizard_step.set(WizardStep::Audience);
                    })
                />
            </Show>

            <Show when=move || tab.get() == Tab::Dnc fallback=|| ()>
                <DncPanel />
            </Show>
        </div>
    }
}

#[component]
fn AudienceStep(
    meta: RwSignal<Option<serde_json::Value>>,
    combinator: RwSignal<String>,
    conditions: RwSignal<Vec<serde_json::Value>>,
    exclude: RwSignal<Vec<serde_json::Value>>,
    preview: RwSignal<Option<serde_json::Value>>,
    preview_loading: RwSignal<bool>,
    preview_error: RwSignal<Option<String>>,
    segments: RwSignal<Vec<serde_json::Value>>,
    saved_segment_id: RwSignal<Option<i64>>,
    on_apply: Rc<dyn Fn(serde_json::Value)>,
    on_next: Rc<dyn Fn()>,
    loading_recipients: RwSignal<bool>,
    load_segments: Rc<dyn Fn()>,
) -> impl IntoView {
    let save_name = create_rw_signal(String::new());
    let saving = create_rw_signal(false);

    let condition_rows = move || {
        conditions
            .get()
            .into_iter()
            .filter(|n| n.get("kind").and_then(|v| v.as_str()) != Some("group"))
            .collect::<Vec<_>>()
    };
    let exclude_rows = move || {
        exclude
            .get()
            .into_iter()
            .filter(|n| n.get("kind").and_then(|v| v.as_str()) != Some("group"))
            .collect::<Vec<_>>()
    };

    let add_condition = move |kind: &'static str| {
        let m = meta.get_untracked();
        conditions.update(|l| l.push(default_node(kind, m.as_ref())));
    };

    let save_segment = move |_| {
        if saving.get_untracked() {
            return;
        }
        let name = save_name.get_untracked();
        if name.trim().is_empty() {
            return;
        }
        saving.set(true);
        let body = serde_json::json!({
            "name": name,
            "definition": assemble_definition(&combinator.get_untracked(), &conditions.get_untracked(), &exclude.get_untracked()),
        });
        let load_segments = load_segments.clone();
        spawn_local(async move {
            match crate::api::post_json::<serde_json::Value>("/api/outreach/segments", Some(&body))
                .await
            {
                Ok(r) => {
                    let ok = r.get("ok").and_then(|v| v.as_bool()).unwrap_or(false);
                    let message = r
                        .get("message")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Request failed.")
                        .to_string();
                    if ok {
                        toasts::success(message);
                        save_name.set(String::new());
                        load_segments();
                    } else {
                        toasts::error(message);
                    }
                }
                Err(e) => toasts::error(e),
            }
            saving.set(false);
        });
    };

    view! {
        <div class="spp-grid-2">
            <div class="spp-card">
                <h3 class="spp-card__title">"Audience builder"</h3>
                <p class="spp-muted spp-text-xs">
                    "Properties answer \u{201c}which customers?\u{201d} \u{b7} tags answer \u{201c}which tickets?\u{201d} \u{b7} SupportOS resolves tickets to unique contacts."
                </p>
                <SuggestBox on_apply />
                <div class="spp-flex spp-flex--wrap spp-gap-8 spp-mb-8">
                    <span class="spp-muted spp-text-xs">"Match"</span>
                    <select
                        class="spp-input spp-cond-editor__kind"
                        value=move || combinator.get()
                        on:change=move |ev| combinator.set(event_target_value(&ev))
                    >
                        <option value="all">"ALL"</option>
                        <option value="any">"ANY"</option>
                    </select>
                    <span class="spp-muted spp-text-xs">"of the conditions below (one row each)"</span>
                </div>

                <For
                    each=condition_rows
                    key=|n| n.get("_uid").and_then(|v| v.as_u64()).unwrap_or(0)
                    children=move |n| {
                        let uid = n.get("_uid").and_then(|v| v.as_u64()).unwrap_or(0);
                        view! { <ConditionEditor list=conditions uid=uid meta=meta /> }
                    }
                />
                <div class="spp-flex spp-flex--wrap">
                    <button class="spp-button spp-button--small" on:click=move |_| add_condition("customer_property")>
                        "+ Customer property"
                    </button>
                    <button class="spp-button spp-button--small" on:click=move |_| add_condition("ticket")>
                        "+ Ticket condition"
                    </button>
                    <button class="spp-button spp-button--small" on:click=move |_| add_condition("contact")>
                        "+ Contact field"
                    </button>
                    <button class="spp-button spp-button--small" on:click=move |_| add_condition("history")>
                        "+ Support history"
                    </button>
                    <button class="spp-button spp-button--small" on:click=move |_| add_condition("organization_property")>
                        "+ Organization"
                    </button>
                    <button class="spp-button spp-button--small" on:click=move |_| add_condition("history_issue")>
                        "+ Previous issues"
                    </button>
                    <button class="spp-button spp-button--small" on:click=move |_| add_condition("incident_exposure")>
                        "+ Incident exposure"
                    </button>
                    <button class="spp-button spp-button--small" on:click=move |_| add_condition("campaign_history")>
                        "+ Campaign history"
                    </button>
                    <button class="spp-button spp-button--small" on:click=move |_| add_condition("support_health")>
                        "+ Support health"
                    </button>
                    <button class="spp-button spp-button--small" on:click=move |_| add_condition("custom_object_link")>
                        "+ Custom object"
                    </button>
                    <button class="spp-button spp-button--small" on:click=move |_| add_condition("customer_event")>
                        "+ Timeline event"
                    </button>
                </div>

                <h3 class="spp-card__title spp-mt-16">"Exclusions"</h3>
                <p class="spp-muted spp-text-xs">
                    "Customers matching ANY exclusion are removed. The Do-Not-Contact list always applies on top."
                </p>
                <For
                    each=exclude_rows
                    key=|n| n.get("_uid").and_then(|v| v.as_u64()).unwrap_or(0)
                    children=move |n| {
                        let uid = n.get("_uid").and_then(|v| v.as_u64()).unwrap_or(0);
                        view! { <ConditionEditor list=exclude uid=uid meta=meta /> }
                    }
                />
                <button
                    class="spp-button spp-button--small"
                    on:click=move |_| {
                        let m = meta.get_untracked();
                        exclude.update(|l| l.push(default_node("ticket", m.as_ref())));
                    }
                >
                    "+ Exclusion condition"
                </button>

                <div class="spp-flex spp-flex--wrap spp-mt-16">
                    <input
                        class="spp-input spp-cond-editor__control--wide"
                        placeholder="Save this segment as\u{2026}"
                        maxlength=200
                        value=move || save_name.get()
                        on:input=move |ev| save_name.set(event_target_value(&ev))
                    />
                    <button
                        class="spp-button spp-button--small"
                        disabled=move || save_name.get().trim().is_empty() || saving.get()
                        on:click=save_segment
                    >
                        "Save segment"
                    </button>
                </div>
            </div>

            <div class="spp-card">
                <h3 class="spp-card__title">"Live preview"</h3>
                <Show when=move || preview_loading.get() fallback=|| ()>
                    <LoadingState />
                </Show>
                <Show when=move || preview_error.get().is_some() fallback=|| ()>
                    <div class="spp-state spp-state--error">
                        <span class="spp-state__icon" aria-hidden="true">"\u{26a0}"</span>
                        <p class="spp-state__body">
                            "Preview failed - check your conditions. "
                            {move || preview_error.get().unwrap_or_default()}
                        </p>
                    </div>
                </Show>
                {move || {
                    let Some(p) = preview.get() else { return ().into_view() };
                    let on_next = on_next.clone();
                    let matched = p.get("matched").and_then(|v| v.as_i64()).unwrap_or(0);
                    let excluded = p.get("excluded").and_then(|v| v.as_i64()).unwrap_or(0);
                    let without_email = p.get("without_email").and_then(|v| v.as_i64()).unwrap_or(0);
                    let on_dnc = p.get("on_dnc").and_then(|v| v.as_i64()).unwrap_or(0);
                    let notes: Vec<String> = p
                        .get("notes")
                        .and_then(|v| v.as_array())
                        .map(|a| a.iter().filter_map(|n| n.as_str().map(str::to_string)).collect())
                        .unwrap_or_default();
                    let rows = p
                        .get("rows")
                        .and_then(|v| v.as_array())
                        .cloned()
                        .unwrap_or_default();
                    let has_excluded = excluded > 0;
                    let has_without_email = without_email > 0;
                    let has_on_dnc = on_dnc > 0;
                    view! {
                        <div class="spp-flex spp-flex--wrap spp-gap-8 spp-mb-8">
                            <span class="spp-badge spp-badge--active">
                                {format!("{matched} matching customers")}
                            </span>
                            <Show when=move || has_excluded fallback=|| ()>
                                <span class="spp-badge spp-badge--warn">{format!("{excluded} excluded")}</span>
                            </Show>
                            <Show when=move || has_without_email fallback=|| ()>
                                <span class="spp-badge spp-badge--warn">{format!("{without_email} without email")}</span>
                            </Show>
                            <Show when=move || has_on_dnc fallback=|| ()>
                                <span class="spp-badge spp-badge--err">{format!("{on_dnc} on Do-Not-Contact")}</span>
                            </Show>
                        </div>
                        {notes
                            .into_iter()
                            .map(|n| view! { <p class="spp-muted spp-text-xs" style="margin:2px 0;">"\u{b7} "{n}</p> })
                            .collect::<Vec<_>>()}
                        <div class="spp-preview-list">
                            {rows
                                .iter()
                                .take(20)
                                .map(|r| {
                                    let first = r.get("first_name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                    let last = r.get("last_name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                    let email = r
                                        .get("chosen_email")
                                        .and_then(|v| v.as_str())
                                        .filter(|e| !e.is_empty())
                                        .map(str::to_string)
                                        .unwrap_or_else(|| "no email".to_string());
                                    let why: Vec<String> = r
                                        .get("why")
                                        .and_then(|v| v.as_array())
                                        .map(|a| {
                                            a.iter()
                                                .take(2)
                                                .filter_map(|w| {
                                                    w.get("text").and_then(|t| t.as_str()).map(str::to_string)
                                                })
                                                .collect()
                                        })
                                        .unwrap_or_default();
                                    view! {
                                        <div class="spp-preview-row">
                                            <div class="spp-flex--between">
                                                <strong class="spp-text-sm">{format!("{first} {last}")}</strong>
                                                <span class="spp-muted spp-text-xs">{email}</span>
                                            </div>
                                            <div class="spp-muted spp-text-xs">{why.join(" \u{b7} ")}</div>
                                        </div>
                                    }
                                })
                                .collect::<Vec<_>>()}
                            {if rows.len() > 20 {
                                vec![view! {
                                    <p class="spp-muted spp-text-xs">
                                        {format!("\u{2026}and {} more (full list in the next step)", rows.len() - 20)}
                                    </p>
                                }.into_view()]
                            } else if rows.is_empty() {
                                vec![view! {
                                    <EmptyState message="No customers match - loosen a condition or check the honest notes above." />
                                }.into_view()]
                            } else {
                                Vec::<View>::new()
                            }}
                        </div>
                        <button
                            class="spp-button spp-mt-16"
                            disabled=move || matched == 0 || loading_recipients.get()
                            on:click=move |_| on_next()
                        >
                            {move || {
                                if loading_recipients.get() {
                                    "Loading all matching recipients\u{2026}".to_string()
                                } else {
                                    format!("Review {matched} recipients \u{2192}")
                                }
                            }}
                        </button>
                    }
                    .into_view()
                }}
                <div class="spp-mt-16">
                    <p class="spp-muted spp-text-xs" style="margin:0;">"Load a saved segment:"</p>
                    <select
                        class="spp-input"
                        value=move || saved_segment_id.get().map(|i| i.to_string()).unwrap_or_default()
                        on:change=move |ev| {
                            let raw = event_target_value(&ev);
                            let id = raw.parse::<i64>().ok();
                            saved_segment_id.set(id);
                            if let Some(id) = id {
                                if let Some(s) = segments.get().into_iter().find(|s| {
                                    s.get("id").and_then(|v| v.as_i64()) == Some(id)
                                }) {
                                    let def = s.get("definition").cloned().unwrap_or_default();
                                    let (c, conds, excl) = split_definition(&def);
                                    combinator.set(c);
                                    conditions.set(conds);
                                    exclude.set(excl);
                                }
                            }
                        }
                    >
                        <option value="">"\u{2014} none \u{2014}"</option>
                        {move || {
                            segments.get()
                                .into_iter()
                                .map(|s| {
                                    let id = s.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
                                    let name = s.get("name").and_then(|v| v.as_str()).unwrap_or("");
                                    let version = s.get("version").and_then(|v| v.as_i64()).unwrap_or(1);
                                    view! {
                                        <option value=id.to_string()>{format!("{name} (v{version})")}</option>
                                    }
                                })
                                .collect::<Vec<_>>()
                        }}
                    </select>
                </div>
            </div>
        </div>
    }
}

#[component]
fn SuggestBox(on_apply: Rc<dyn Fn(serde_json::Value)>) -> impl IntoView {
    let request = create_rw_signal(String::new());
    let result = create_rw_signal(None::<(serde_json::Value, String, Vec<String>)>);
    let pending = create_rw_signal(false);

    let suggest = move || {
        if pending.get_untracked() {
            return;
        }
        let req = request.get_untracked();
        if req.trim().len() < 3 {
            return;
        }
        pending.set(true);
        let body = serde_json::json!({ "request": req });
        spawn_local(async move {
            match crate::api::post_json::<serde_json::Value>(
                "/api/outreach/segments/suggest",
                Some(&body),
            )
            .await
            {
                Ok(r) => {
                    result.set(Some((
                        r.get("definition").cloned().unwrap_or_default(),
                        r.get("model")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                        r.get("notes")
                            .and_then(|v| v.as_array())
                            .map(|a| {
                                a.iter()
                                    .filter_map(|n| n.as_str().map(str::to_string))
                                    .collect()
                            })
                            .unwrap_or_default(),
                    )));
                }
                Err(e) => toasts::error(e),
            }
            pending.set(false);
        });
    };

    view! {
        <div class="spp-suggest-box">
            <div class="spp-flex spp-flex--wrap spp-gap-8">
                <input
                    class="spp-input spp-grow"
                    placeholder="Describe the audience in plain language (e.g. \u{201c}customers in Europe with an open billing ticket who replied to the last campaign\u{201d})\u{2026}"
                    maxlength=500
                    value=move || request.get()
                    on:input=move |ev| request.set(event_target_value(&ev))
                    on:keydown=move |ev| {
                        if ev.key().as_str() == "Enter" && request.get_untracked().trim().len() >= 3 {
                            suggest();
                        }
                    }
                />
                <button
                    class="spp-button spp-button--small"
                    disabled=move || pending.get() || request.get().trim().len() < 3
                    on:click=move |_| suggest()
                >
                    {move || {
                        if pending.get() {
                            "Asking the local model\u{2026}".to_string()
                        } else {
                            "Suggest segment (AI)".to_string()
                        }
                    }}
                </button>
            </div>
            <p class="spp-muted spp-text-xs spp-mt-4" style="margin-bottom:0;">
                "The local model only proposes the condition definition; the deterministic engine selects recipients. Nothing is saved - apply it below and review."
            </p>
            {move || {
                let Some((definition, model, notes)) = result.get() else { return ().into_view() };
                let on_apply = on_apply.clone();
                let conds: Vec<serde_json::Value> = definition
                    .get("conditions")
                    .and_then(|v| v.as_array())
                    .cloned()
                    .unwrap_or_default();
                let excl: Vec<serde_json::Value> = definition
                    .get("exclude")
                    .and_then(|v| v.as_array())
                    .cloned()
                    .unwrap_or_default();
                view! {
                    <div class="spp-suggest-result spp-mt-8">
                        <div class="spp-text-sm">
                            <strong>"Proposed definition"</strong>
                            <span class="spp-muted spp-text-xs">{format!(" (model: {model})")}</span>
                        </div>
                        <ul class="spp-text-xs spp-mt-4" style="padding-left:18px;margin-bottom:0;">
                            {conds
                                .iter()
                                .map(|n| view! { <li>{describe_condition(n)}</li> })
                                .collect::<Vec<_>>()}
                            {excl
                                .iter()
                                .map(|n| view! { <li><strong>"exclude: "</strong>{describe_condition(n)}</li> })
                                .collect::<Vec<_>>()}
                        </ul>
                        <div class="spp-flex spp-gap-8 spp-mt-8">
                            <button
                                class="spp-button spp-button--small spp-button--primary"
                                on:click=move |_| {
                                    on_apply(definition.clone());
                                    request.set(String::new());
                                    result.set(None);
                                    toasts::success(
                                        "Definition applied to the builder - review it below.".to_string(),
                                    );
                                }
                            >
                                "Apply to builder"
                            </button>
                            <button
                                class="spp-button spp-button--ghost spp-button--small"
                                on:click=move |_| result.set(None)
                            >
                                "Discard"
                            </button>
                        </div>
                        {notes
                            .iter()
                            .map(|n| view! { <div class="spp-muted spp-text-xs spp-mt-4">{n.clone()}</div> })
                            .collect::<Vec<_>>()}
                    </div>
                }
                .into_view()
            }}
        </div>
    }
}

#[component]
fn RecipientsStep(
    rows: RwSignal<Vec<serde_json::Value>>,
    selected_ids: RwSignal<Vec<i64>>,
    on_back: Rc<dyn Fn()>,
    on_next: Rc<dyn Fn()>,
) -> impl IntoView {
    let expanded = create_rw_signal(None::<i64>);
    let selectable_ids = move || {
        rows.get()
            .iter()
            .filter(|r| {
                !r.get("excluded").and_then(|v| v.as_bool()).unwrap_or(false)
                    && r.get("chosen_email")
                        .and_then(|v| v.as_str())
                        .is_some_and(|e| !e.is_empty())
            })
            .filter_map(|r| r.get("customer_local_id").and_then(|v| v.as_i64()))
            .collect::<Vec<i64>>()
    };
    view! {
        <div class="spp-card" style="padding:0;">
            <div class="spp-flex--between" style="padding:10px 12px;">
                <div>
                    <h3 class="spp-card__title" style="margin-bottom:0;">
                        {move || {
                            format!(
                                "Recipient review \u{2014} {} selected of {} matched",
                                selected_ids.get().len(),
                                rows.get().len()
                            )
                        }}
                    </h3>
                    <p class="spp-muted spp-text-xs" style="margin:2px 0 0;">
                        "One email per customer. Every row explains why it matched; clicking a row shows the matching tickets."
                    </p>
                </div>
                <div class="spp-flex spp-gap-4">
                    <button class="spp-button spp-button--ghost spp-button--small"
                        on:click=move |_| selected_ids.set(selectable_ids())
                    >
                        "Select all"
                    </button>
                    <button class="spp-button spp-button--ghost spp-button--small"
                        on:click=move |_| selected_ids.set(Vec::new())
                    >
                        "Clear"
                    </button>
                    <button
                        class="spp-button spp-button--ghost spp-button--small"
                        on:click=move |_| {
                            let all = selectable_ids();
                            let current = selected_ids.get_untracked();
                            selected_ids.set(
                                all.into_iter().filter(|id| !current.contains(id)).collect()
                            );
                        }
                    >
                        "Invert"
                    </button>
                </div>
            </div>
            <table class="spp-table">
                <thead>
                    <tr>
                        <th style="width:30px;"></th>
                        <th>"Customer"</th>
                        <th>"Email"</th>
                        <th>"Properties"</th>
                        <th>"Open"</th>
                        <th>"Why selected"</th>
                        <th>"Matching tickets"</th>
                    </tr>
                </thead>
                <tbody>
                    {move || {
                        rows.get()
                            .into_iter()
                            .map(|r| {
                                let cid = r.get("customer_local_id").and_then(|v| v.as_i64()).unwrap_or(0);
                                let first = r.get("first_name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                let last = r.get("last_name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                let org = r.get("organization").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                let email = r
                                    .get("chosen_email")
                                    .and_then(|v| v.as_str())
                                    .filter(|e| !e.is_empty())
                                    .map(str::to_string);
                                let excluded = r.get("excluded").and_then(|v| v.as_bool()).unwrap_or(false);
                                let open = r.get("open_tickets").and_then(|v| v.as_i64()).unwrap_or(0);
                                let props: Vec<String> = r
                                    .get("properties")
                                    .and_then(|v| v.as_array())
                                    .map(|a| {
                                        a.iter()
                                            .filter_map(|p| {
                                                let name = p.get("name").and_then(|v| v.as_str())?;
                                                let val = p.get("value").and_then(|v| v.as_str())?;
                                                Some(format!("{name}={val}"))
                                            })
                                            .collect()
                                    })
                                    .unwrap_or_default();
                                let why: Vec<String> = r
                                    .get("why")
                                    .and_then(|v| v.as_array())
                                    .map(|a| {
                                        a.iter()
                                            .filter_map(|w| w.get("text").and_then(|t| t.as_str()).map(str::to_string))
                                            .collect()
                                    })
                                    .unwrap_or_default();
                                let tickets = r
                                    .get("matching_tickets")
                                    .and_then(|v| v.as_array())
                                    .cloned()
                                    .unwrap_or_default();
                                view! {
                                    <>
                                        <tr
                                            class="spp-table__row--clickable"
                                            on:click=move |_| {
                                                expanded.set(
                                                    if expanded.get_untracked() == Some(cid) { None } else { Some(cid) }
                                                );
                                            }
                                        >
                                            <td>
                                                <input
                                                    type="checkbox"
                                                    prop:checked=move || selected_ids.get().contains(&cid)
                                                    disabled=excluded || email.is_none()
                                                    on:click=|ev| {
                                                        ev.stop_propagation();
                                                    }
                                                    on:change=move |ev| {
                                                        let checked = event_target_checked(&ev);
                                                        selected_ids.update(|ids| {
                                                            if checked {
                                                                if !ids.contains(&cid) { ids.push(cid); }
                                                            } else {
                                                                ids.retain(|id| *id != cid);
                                                            }
                                                        });
                                                    }
                                                />
                                            </td>
                                            <td>
                                                <strong class="spp-text-sm">{format!("{first} {last}")}</strong>
                                                {if org.is_empty() {
                                                    Vec::<View>::new()
                                                } else {
                                                    vec![view! { <div class="spp-muted spp-text-xs">{org}</div> }.into_view()]
                                                }}
                                            </td>
                                            <td class="spp-text-sm">
                                                {match email {
                                                    Some(e) => view! { {e} }.into_view(),
                                                    None => view! { <span class="spp-badge spp-badge--warn">"no email"</span> }.into_view(),
                                                }}
                                            </td>
                                            <td class="spp-text-xs">
                                                {if props.is_empty() { "\u{2014}".to_string() } else { props.join(" \u{b7} ") }}
                                            </td>
                                            <td class="spp-text-sm">
                                                {if open > 0 {
                                                    view! { <span class="spp-badge spp-badge--active">{open.to_string()}</span> }.into_view()
                                                } else {
                                                    "0".into_view()
                                                }}
                                            </td>
                                            <td class="spp-text-xs">
                                                {why
                                                    .iter()
                                                    .take(2)
                                                    .map(|w| view! { <div>{"\u{2713} "}{w.clone()}</div> })
                                                    .collect::<Vec<_>>()}
                                                {if why.len() > 2 {
                                                    vec![view! { <span class="spp-muted">{format!("+{} more", why.len() - 2)}</span> }.into_view()]
                                                } else {
                                                    Vec::<View>::new()
                                                }}
                                            </td>
                                            <td class="spp-text-sm">
                                                {if !tickets.is_empty() {
                                                    view! {
                                                        <span class="spp-badge">
                                                            {format!(
                                                                "{} ticket{}",
                                                                tickets.len(),
                                                                if tickets.len() > 1 { "s" } else { "" }
                                                            )}
                                                        </span>
                                                    }
                                                    .into_view()
                                                } else {
                                                    "\u{2014}".into_view()
                                                }}
                                            </td>
                                        </tr>
                                        {move || {
                                            if expanded.get() == Some(cid) {
                                                view! {
                                                    <tr class="spp-row-detail">
                                                        <td colspan=7>
                                                            <div class="spp-text-xs">
                                                                <strong>"Why selected:"</strong>
                                                                {why
                                                                    .iter()
                                                                    .map(|w| view! { <div>{"\u{2713} "}{w.clone()}</div> })
                                                                    .collect::<Vec<_>>()}
                                                            </div>
                                                            {if !tickets.is_empty() {
                                                                view! {
                                                                    <div class="spp-mt-8">
                                                                        <strong class="spp-text-xs">"Matching conversations:"</strong>
                                                                        {tickets
                                                                            .iter()
                                                                            .map(|t| {
                                                                                let conv = t.get("conversationId").and_then(|v| v.as_i64()).unwrap_or(0);
                                                                                let number = t.get("number").and_then(|v| v.as_i64()).unwrap_or(0);
                                                                                let subject = t.get("subject").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                                                                let status = t.get("status").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                                                                let tags: Vec<String> = t
                                                                                    .get("tags")
                                                                                    .and_then(|v| v.as_array())
                                                                                    .map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect())
                                                                                    .unwrap_or_default();
                                                                                view! {
                                                                                    <div class="spp-flex spp-gap-8" style="padding:2px 0;">
                                                                                        <a class="spp-text-xs" href=format!("/inbox/conversation/{conv}")>
                                                                                            {format!("#{number}")}
                                                                                        </a>
                                                                                        <span class="spp-text-xs spp-grow">{subject}</span>
                                                                                        <span class="spp-badge spp-badge--tag">{tags.join(", ")}</span>
                                                                                        <span class="spp-badge">{status}</span>
                                                                                    </div>
                                                                                }
                                                                            })
                                                                            .collect::<Vec<_>>()}
                                                                    </div>
                                                                }
                                                                .into_view()
                                                            } else {
                                                                ().into_view()
                                                            }}
                                                        </td>
                                                    </tr>
                                                }
                                                .into_view()
                                            } else {
                                                ().into_view()
                                            }
                                        }}
                                    </>
                                }
                            })
                            .collect::<Vec<_>>()
                    }}
                </tbody>
            </table>
            <div class="spp-flex spp-flex--between" style="padding:10px 12px;">
                <button class="spp-button spp-button--ghost" on:click=move |_| on_back()>
                    "\u{2190} Back to audience"
                </button>
                <button
                    class="spp-button"
                    disabled=move || selected_ids.get().is_empty()
                    on:click=move |_| on_next()
                >
                    {move || format!("Compose for {} recipients \u{2192}", selected_ids.get().len())}
                </button>
            </div>
        </div>
    }
}

#[component]
fn ComposeStep(
    meta: RwSignal<Option<serde_json::Value>>,
    draft: RwSignal<Draft>,
    preview: RwSignal<Option<serde_json::Value>>,
    selected_ids: RwSignal<Vec<i64>>,
    on_back: Rc<dyn Fn()>,
    on_next: Rc<dyn Fn()>,
) -> impl IntoView {
    let preview_customer_id = create_rw_signal(None::<i64>);
    let rendered = create_rw_signal(None::<serde_json::Value>);
    let rendering = create_rw_signal(false);

    create_effect(move |_| {
        let customer = preview_customer_id.get();
        let d = draft.get();
        let Some(customer) =
            customer.filter(|_| !d.subject.trim().is_empty() && !d.body.trim().is_empty())
        else {
            rendered.set(None);
            return;
        };
        rendering.set(true);
        let body = serde_json::json!({
            "customer_local_id": customer,
            "subject": d.subject,
            "body": d.body,
        });
        let rendered = rendered;
        let rendering = rendering;
        spawn_local(async move {
            match crate::api::post_json::<serde_json::Value>("/api/outreach/render", Some(&body))
                .await
            {
                Ok(v) => rendered.set(Some(v)),
                Err(e) => toasts::error(e),
            }
            rendering.set(false);
        });
    });

    let insert_variable = move |v: String| {
        draft.update(|d| d.body = format!("{}{{{{{v}}}}}", d.body));
    };

    view! {
        <div class="spp-grid-2">
            <div class="spp-card">
                <h3 class="spp-card__title">"Campaign message"</h3>
                <label class="spp-cond-editor__field">
                    <span class="spp-muted">"Campaign name"</span>
                    <input
                        class="spp-input"
                        placeholder="Timezone issue update"
                        value=move || draft.get().name
                        on:input=move |ev| draft.update(|d| d.name = event_target_value(&ev))
                    />
                </label>
                <label class="spp-cond-editor__field">
                    <span class="spp-muted">"From mailbox"</span>
                    <select
                        class="spp-input"
                        value=move || {
                            let id = draft.get().mailbox_local_id;
                            if id > 0 { id.to_string() } else { String::new() }
                        }
                        on:change=move |ev| {
                            draft.update(|d| d.mailbox_local_id = event_target_value(&ev).parse().unwrap_or(0));
                        }
                    >
                        <option value="">"choose a mailbox\u{2026}"</option>
                        {move || {
                            let m = meta.get();
                            m.as_ref()
                                .and_then(|v| v.get("mailboxes"))
                                .and_then(|v| v.as_array())
                                .cloned()
                                .unwrap_or_default()
                                .into_iter()
                                .map(|mb| {
                                    let id = mb.get("local_id").and_then(|v| v.as_i64()).unwrap_or(0);
                                    let name = mb.get("name").and_then(|v| v.as_str()).unwrap_or("");
                                    let email = mb.get("email").and_then(|v| v.as_str()).unwrap_or("");
                                    let label = if email.is_empty() {
                                        name.to_string()
                                    } else {
                                        format!("{name} ({email})")
                                    };
                                    view! { <option value=id.to_string()>{label}</option> }
                                })
                                .collect::<Vec<_>>()
                        }}
                    </select>
                </label>
                <label class="spp-cond-editor__field">
                    <span class="spp-muted">"Subject"</span>
                    <input
                        class="spp-input"
                        placeholder="Update on the timezone issue you reported"
                        value=move || draft.get().subject
                        on:input=move |ev| draft.update(|d| d.subject = event_target_value(&ev))
                    />
                </label>
                <label class="spp-cond-editor__field">
                    <span class="spp-muted">"Message (HTML or plain text)"</span>
                    <textarea
                        class="spp-input"
                        rows=10
                        prop:value=move || draft.get().body
                        placeholder="Hi {{first_name}},\n\nWe wanted to share an update\u{2026}"
                        on:input=move |ev| draft.update(|d| d.body = event_target_value(&ev))
                    ></textarea>
                </label>
                <div class="spp-flex spp-flex--wrap spp-gap-4">
                    <span class="spp-muted spp-text-xs">"Personalization:"</span>
                    {move || {
                        let m = meta.get();
                        let vars = m
                            .as_ref()
                            .and_then(|v| v.get("personalization_variables"))
                            .and_then(|v| v.as_array())
                            .map(|a| {
                                a.iter()
                                    .filter_map(|x| x.as_str().map(str::to_string))
                                    .collect::<Vec<_>>()
                            })
                            .unwrap_or_else(|| vec!["first_name".to_string()]);
                        vars.into_iter()
                            .map(|v| {
                                let v2 = v.clone();
                                view! {
                                    <button
                                        type="button"
                                        class="spp-chip"
                                        on:click=move |_| insert_variable(v2.clone())
                                    >
                                        {format!("{{{{{v}}}}}")}
                                    </button>
                                }
                            })
                            .collect::<Vec<_>>()
                    }}
                </div>
                <label class="spp-cond-editor__field">
                    <span class="spp-muted">"Tags on created conversations (comma separated)"</span>
                    <input
                        class="spp-input"
                        placeholder="outreach, timezone"
                        value=move || draft.get().tags
                        on:input=move |ev| draft.update(|d| d.tags = event_target_value(&ev))
                    />
                </label>
                <div class="spp-flex spp-flex--between spp-mt-12">
                    <button class="spp-button spp-button--ghost" on:click=move |_| on_back()>
                        "\u{2190} Back to recipients"
                    </button>
                    <button
                        class="spp-button"
                        disabled=move || {
                            let d = draft.get();
                            d.name.trim().is_empty()
                                || d.subject.trim().is_empty()
                                || d.body.trim().is_empty()
                                || d.mailbox_local_id <= 0
                        }
                        on:click=move |_| on_next()
                    >
                        "Final review \u{2192}"
                    </button>
                </div>
            </div>
            <div class="spp-card">
                <h3 class="spp-card__title">"Preview personalized message"</h3>
                <p class="spp-muted spp-text-xs">
                    "Rendering uses the same code path as the send. Nothing is personalized silently - inspect it here first."
                </p>
                <select
                    class="spp-input"
                    value=move || preview_customer_id.get().map(|i| i.to_string()).unwrap_or_default()
                    on:change=move |ev| {
                        preview_customer_id.set(event_target_value(&ev).parse::<i64>().ok());
                    }
                >
                    <option value="">"choose a recipient\u{2026}"</option>
                    {move || {
                        let selected = selected_ids.get();
                        preview.get()
                            .and_then(|p| p.get("rows").and_then(|r| r.as_array()).cloned())
                            .unwrap_or_default()
                            .into_iter()
                            .filter(|r| {
                                r.get("customer_local_id")
                                    .and_then(|v| v.as_i64())
                                    .map(|id| selected.contains(&id))
                                    .unwrap_or(false)
                            })
                            .take(50)
                            .map(|r| {
                                let id = r.get("customer_local_id").and_then(|v| v.as_i64()).unwrap_or(0);
                                let first = r.get("first_name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                let last = r.get("last_name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                view! { <option value=id.to_string()>{format!("{first} {last}")}</option> }
                            })
                            .collect::<Vec<_>>()
                    }}
                </select>
                {move || {
                    let Some(v) = rendered.get() else {
                        return view! {
                            <EmptyState message="Pick a recipient - the rendered message appears here before you send anything." />
                        }
                        .into_view();
                    };
                    let rendered_msg = v.get("rendered").cloned().unwrap_or_default();
                    let subject = rendered_msg.get("subject").and_then(|x| x.as_str()).unwrap_or("").to_string();
                    let body = rendered_msg.get("body").and_then(|x| x.as_str()).unwrap_or("").to_string();
                    let unresolved: Vec<String> = rendered_msg
                        .get("unresolved")
                        .and_then(|x| x.as_array())
                        .map(|a| a.iter().filter_map(|u| u.as_str().map(str::to_string)).collect())
                        .unwrap_or_default();
                    let sources: Vec<String> = v
                        .get("sources")
                        .and_then(|x| x.as_array())
                        .map(|a| {
                            a.iter()
                                .filter_map(|s| {
                                    s.get("number").and_then(|n| n.as_i64()).map(|n| format!("#{n}"))
                                })
                                .collect()
                        })
                        .unwrap_or_default();
                    view! {
                        <div class="spp-mt-16">
                            <div class="spp-flex--between">
                                <strong class="spp-text-sm">"Subject"</strong>
                                {if !unresolved.is_empty() {
                                    vec![view! {
                                        <span class="spp-badge spp-badge--warn">
                                            {format!("unresolved: {}", unresolved.join(", "))}
                                        </span>
                                    }.into_view()]
                                } else {
                                    Vec::<View>::new()
                                }}
                            </div>
                            <div class="spp-text-sm" style="padding:6px 0;">{subject}</div>
                            <strong class="spp-text-sm">"Body"</strong>
                            <pre class="spp-text-sm spp-pre">
                                {body}
                            </pre>
                            {if !sources.is_empty() {
                                vec![view! {
                                    <p class="spp-muted spp-text-xs">
                                        {format!("Sources used for personalization: {}", sources.join(", "))}
                                    </p>
                                }.into_view()]
                            } else {
                                Vec::<View>::new()
                            }}
                        </div>
                    }
                    .into_view()
                }}
                <Show when=move || rendering.get() fallback=|| ()>
                    <LoadingState />
                </Show>
            </div>
        </div>
    }
}

#[component]
fn ReviewStep(
    draft: RwSignal<Draft>,
    selected_ids: RwSignal<Vec<i64>>,
    created_campaign_id: RwSignal<Option<i64>>,
    creating: RwSignal<bool>,
    create_campaign: Rc<dyn Fn()>,
    load_campaigns: Rc<dyn Fn()>,
    on_back: Rc<dyn Fn()>,
    on_done: Rc<dyn Fn()>,
) -> impl IntoView {
    let validation = create_rw_signal(None::<serde_json::Value>);
    let queueing = create_rw_signal(false);

    create_effect(move |_| {
        let Some(id) = created_campaign_id.get() else {
            validation.set(None);
            return;
        };
        let validation = validation;
        spawn_local(async move {
            let path = format!("/api/outreach/campaigns/{id}/validate");
            match crate::api::get_json::<serde_json::Value>(&path).await {
                Ok(v) => validation.set(Some(v)),
                Err(e) => toasts::error(e),
            }
        });
    });

    let queue_campaign = {
        let load_campaigns = load_campaigns.clone();
        let on_done = on_done.clone();
        Rc::new(move || {
            let Some(id) = created_campaign_id.get_untracked() else {
                return;
            };
            if queueing.get_untracked() {
                return;
            }
            queueing.set(true);
            let path = format!("/api/outreach/campaigns/{id}/queue");
            let load_campaigns = load_campaigns.clone();
            let on_done = on_done.clone();
            spawn_local(async move {
                match crate::api::post_json::<serde_json::Value>(&path, None).await {
                    Ok(r) => {
                        let ok = r.get("ok").and_then(|v| v.as_bool()).unwrap_or(false);
                        let message = r
                            .get("message")
                            .and_then(|v| v.as_str())
                            .unwrap_or("Request failed.")
                            .to_string();
                        if ok {
                            toasts::success(message);
                            load_campaigns();
                            on_done();
                        } else {
                            toasts::error(message);
                        }
                    }
                    Err(e) => toasts::error(e),
                }
                queueing.set(false);
            });
        })
    };

    view! {
        {move || {
            if created_campaign_id.get().is_none() {
                let create_campaign = create_campaign.clone();
                let on_back = on_back.clone();
                view! {
                    <div class="spp-card">
                        <h3 class="spp-card__title">"Final review"</h3>
                        <div class="spp-mb-8">
                            <Kv2 k="Campaign" value=move || draft.get().name />
                            <Kv2 k="Subject" value=move || draft.get().subject />
                            <Kv2 k="Mailbox" value=move || draft.get().mailbox_local_id.to_string() />
                            <Kv2 k="Tags" value=move || {
                                let t = draft.get().tags;
                                if t.is_empty() { "\u{2014}".to_string() } else { t }
                            } />
                            <Kv2 k="Recipients (selected)" value=move || selected_ids.get().len().to_string() />
                        </div>
                        <p class="spp-muted spp-text-xs">
                            "Sending is explicit: creating the campaign snapshots its recipients (audience changes later will NOT alter it), then you queue it on the next screen. Each selected customer receives their own Help Scout conversation - never a shared BCC email."
                        </p>
                        <div class="spp-flex spp-flex--between spp-mt-12">
                            <button class="spp-button spp-button--ghost" on:click=move |_| on_back()>
                                "\u{2190} Back to compose"
                            </button>
                            <button
                                class="spp-button"
                                disabled=move || creating.get()
                                on:click=move |_| create_campaign()
                            >
                                {move || format!("Create campaign ({} recipients)", selected_ids.get().len())}
                            </button>
                        </div>
                    </div>
                }
            } else {
                let on_done = on_done.clone();
                let queue_campaign = queue_campaign.clone();
                view! {
                    <div class="spp-card">
                        <h3 class="spp-card__title">"Campaign created \u{2014} final check before sending"</h3>
                        {move || {
                            let Some(v) = validation.get() else {
                                return view! { <LoadingState /> }.into_view();
                            };
                            let on_done = on_done.clone();
                            let queue_campaign = queue_campaign.clone();
                            let counts = v.get("counts").cloned().unwrap_or_default();
                            let recipients = counts.get("recipients").and_then(|x| x.as_i64()).unwrap_or(0);
                            let ready = counts.get("ready").and_then(|x| x.as_i64()).unwrap_or(0);
                            let invalid_email = counts.get("invalid_email").and_then(|x| x.as_i64()).unwrap_or(0);
                            let on_dnc = counts.get("on_dnc").and_then(|x| x.as_i64()).unwrap_or(0);
                            let no_email = counts.get("no_email").and_then(|x| x.as_i64()).unwrap_or(0);
                            let ok = v.get("ok").and_then(|x| x.as_bool()).unwrap_or(false);
                            let has_invalid_email = invalid_email > 0;
                            let has_on_dnc = on_dnc > 0;
                            let has_no_email = no_email > 0;
                            let errors: Vec<String> = v
                                .get("errors")
                                .and_then(|x| x.as_array())
                                .map(|a| a.iter().filter_map(|e| e.as_str().map(str::to_string)).collect())
                                .unwrap_or_default();
                            let warnings: Vec<String> = v
                                .get("warnings")
                                .and_then(|x| x.as_array())
                                .map(|a| a.iter().filter_map(|w| w.as_str().map(str::to_string)).collect())
                                .unwrap_or_default();
                            view! {
                                <div class="spp-flex spp-flex--wrap spp-gap-8 spp-mb-8">
                                    <span class="spp-badge spp-badge--active">{format!("recipients: {recipients}")}</span>
                                    <span class="spp-badge spp-badge--ok">{format!("ready: {ready}")}</span>
                                    <Show when=move || has_invalid_email fallback=|| ()>
                                        <span class="spp-badge spp-badge--err">{format!("invalid email: {invalid_email}")}</span>
                                    </Show>
                                    <Show when=move || has_on_dnc fallback=|| ()>
                                        <span class="spp-badge spp-badge--warn">{format!("do-not-contact: {on_dnc}")}</span>
                                    </Show>
                                    <Show when=move || has_no_email fallback=|| ()>
                                        <span class="spp-badge spp-badge--warn">{format!("no email: {no_email}")}</span>
                                    </Show>
                                </div>
                                {errors
                                    .iter()
                                    .map(|e| view! { <p class="spp-text-sm spp-outreach__error">{"\u{2717} "}{e.clone()}</p> })
                                    .collect::<Vec<_>>()}
                                {warnings
                                    .iter()
                                    .map(|w| view! { <p class="spp-text-sm spp-outreach__warning">{"\u{26a0} "}{w.clone()}</p> })
                                    .collect::<Vec<_>>()}
                                <div class="spp-flex spp-flex--between spp-mt-12">
                                    <button class="spp-button spp-button--ghost" on:click=move |_| on_done()>
                                        "Done for now (campaign stays in drafts)"
                                    </button>
                                    <button
                                        class="spp-button spp-button--danger"
                                        disabled=move || !ok || queueing.get()
                                        on:click=move |_| queue_campaign()
                                    >
                                        {move || format!("Send to {ready} clients")}
                                    </button>
                                </div>
                            }
                            .into_view()
                        }}
                    </div>
                }
            }
        }}
    }
}

#[component]
fn Kv2(k: &'static str, value: impl Fn() -> String + 'static) -> impl IntoView {
    view! {
        <div class="spp-flex spp-gap-8">
            <span class="spp-muted spp-text-xs" style="width:160px;">{k}</span>
            <span class="spp-text-sm">{value}</span>
        </div>
    }
}

fn status_pill_class(status: &str) -> &'static str {
    match status {
        "queued" => "spp-badge spp-badge--warn",
        "sending" => "spp-badge spp-badge--active",
        "paused" => "spp-badge spp-badge--warn",
        "completed" => "spp-badge spp-badge--ok",
        _ => "spp-badge",
    }
}

fn recipient_state_pill_class(state: &str) -> &'static str {
    match state {
        "queued" => "spp-badge spp-badge--warn",
        "sending" => "spp-badge spp-badge--active",
        "sent" => "spp-badge spp-badge--ok",
        "failed" => "spp-badge spp-badge--err",
        "skipped" => "spp-badge spp-badge--warn",
        "unknown" => "spp-badge spp-badge--warn",
        _ => "spp-badge",
    }
}

#[component]
fn CampaignsPanel(
    campaigns: RwSignal<Vec<serde_json::Value>>,
    campaigns_error: RwSignal<Option<String>>,
    load_campaigns: Rc<dyn Fn()>,
) -> impl IntoView {
    let open_id = create_rw_signal(None::<i64>);
    let detail = create_rw_signal(None::<serde_json::Value>);
    let events = create_rw_signal(Vec::<serde_json::Value>::new());
    let report = create_rw_signal(None::<serde_json::Value>);
    let acting = create_rw_signal(false);

    let load_detail = move |with_report: bool| {
        let Some(id) = open_id.get_untracked() else {
            return;
        };
        let detail = detail;
        let events = events;
        let report = report;
        spawn_local(async move {
            let path = format!("/api/outreach/campaigns/{id}");
            match crate::api::get_json::<serde_json::Value>(&path).await {
                Ok(v) => {
                    detail.set(v.get("campaign").cloned());
                    events.set(
                        v.get("events")
                            .and_then(|e| e.as_array())
                            .cloned()
                            .unwrap_or_default(),
                    );
                }
                Err(e) => toasts::error(e),
            }
            if with_report {
                let rpath = format!("/api/outreach/campaigns/{id}/report");
                match crate::api::get_json::<serde_json::Value>(&rpath).await {
                    Ok(r) => report.set(Some(r)),
                    Err(e) => toasts::error(e),
                }
            }
        });
    };

    create_effect(move |_| {
        let open = open_id.get();
        if open.is_some() {
            load_detail(true);
        } else {
            detail.set(None);
            events.set(Vec::new());
            report.set(None);
        }
    });
    {
        let tick = create_rw_signal(0u32);
        let guard = page_interval(
            move || {
                if open_id.get_untracked().is_some() {
                    tick.update(|t| *t += 1);
                    #[allow(clippy::manual_is_multiple_of)]
                    let third = tick.get_untracked() % 3 == 0;
                    load_detail(third);
                }
            },
            10_000,
        );
        on_cleanup(move || drop(guard));
    }

    let act = Rc::new(move |id: i64, action: &'static str| {
        if acting.get_untracked() {
            return;
        }
        acting.set(true);
        let path = format!("/api/outreach/campaigns/{id}/{action}");
        let load_campaigns = load_campaigns.clone();
        let load_detail = load_detail;
        spawn_local(async move {
            match crate::api::post_json::<serde_json::Value>(&path, None).await {
                Ok(r) => {
                    let ok = r.get("ok").and_then(|v| v.as_bool()).unwrap_or(false);
                    let message = r
                        .get("message")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Request failed.")
                        .to_string();
                    if ok {
                        toasts::success(message);
                        load_campaigns();
                        if open_id.get_untracked() == Some(id) {
                            load_detail(true);
                        }
                    } else {
                        toasts::error(message);
                    }
                }
                Err(e) => toasts::error(e),
            }
            acting.set(false);
        });
    });

    view! {
        <Show when=move || campaigns_error.get().is_some() fallback=|| ()>
            <div class="spp-state spp-state--error">
                <span class="spp-state__icon" aria-hidden="true">"\u{26a0}"</span>
                <p class="spp-state__body">{move || campaigns_error.get().unwrap_or_default()}</p>
            </div>
        </Show>
        {move || {
            let list = campaigns.get();
            if list.is_empty() {
                return view! {
                    <EmptyState message="No campaigns yet. Build an audience from your local mirror, review why each customer matched, and send each of them an individual Help Scout conversation." />
                }
                .into_view();
            }
            view! {
                <div class="spp-card" style="padding:0;">
                    <table class="spp-table">
                        <thead>
                            <tr>
                                <th>"Campaign"</th>
                                <th>"Status"</th>
                                <th>"Recipients"</th>
                                <th>"Sent"</th>
                                <th>"Failed"</th>
                                <th>"Replied"</th>
                                <th>"Created"</th>
                                <th></th>
                            </tr>
                        </thead>
                        <tbody>
                            {list
                                .into_iter()
                                .map(|c| {
                                    let id = c.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
                                    let name = c.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                    let subject = c.get("subject").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                    let status = c.get("status").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                    let recipients = c.get("recipients").and_then(|v| v.as_i64()).unwrap_or(0);
                                    let sent = c.get("sent").and_then(|v| v.as_i64()).unwrap_or(0);
                                    let failed = c.get("failed").and_then(|v| v.as_i64()).unwrap_or(0);
                                    let unknown = c.get("unknown").and_then(|v| v.as_i64()).unwrap_or(0);
                                    let replied = c.get("replied").and_then(|v| v.as_i64()).unwrap_or(0);
                                    let created = c.get("created_at").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                    view! {
                                        <tr
                                            class="spp-table__row--clickable"
                                            on:click=move |_| {
                                                open_id.set(
                                                    if open_id.get_untracked() == Some(id) { None } else { Some(id) }
                                                );
                                            }
                                        >
                                            <td>
                                                <strong class="spp-text-sm">{name}</strong>
                                                <div class="spp-muted spp-text-xs">{subject}</div>
                                            </td>
                                            <td>
                                                {let status_label = status.clone();
                                                view! {
                                                    <span class=move || status_pill_class(&status_label)>{status.clone()}</span>
                                                }}
                                            </td>
                                            <td class="spp-text-sm">{recipients.to_string()}</td>
                                            <td class="spp-text-sm">
                                                {if sent > 0 {
                                                    view! { <span class="spp-badge spp-badge--ok">{sent.to_string()}</span> }.into_view()
                                                } else {
                                                    "0".into_view()
                                                }}
                                            </td>
                                            <td class="spp-text-sm">
                                                {if failed > 0 {
                                                    view! { <span class="spp-badge spp-badge--err">{failed.to_string()}</span> }.into_view()
                                                } else {
                                                    "0".into_view()
                                                }}
                                            </td>
                                            <td class="spp-text-sm">{replied.to_string()}</td>
                                            <td class="spp-muted spp-text-xs">{date_short(&created)}</td>
                                            <td on:click=|ev| ev.stop_propagation()>
                                                <div class="spp-flex spp-gap-4">
                                                    {if status == "draft" || status == "paused" {
                                                        let act = act.clone();
                                                        vec![view! {
                                                            <button
                                                                class="spp-button spp-button--small"
                                                                on:click=move |_| act(id, "queue")
                                                            >
                                                                "Queue"
                                                            </button>
                                                        }.into_view()]
                                                    } else {
                                                        Vec::<View>::new()
                                                    }}
                                                    {if status == "queued" || status == "sending" {
                                                        let act = act.clone();
                                                        vec![view! {
                                                            <button
                                                                class="spp-button spp-button--ghost spp-button--small"
                                                                on:click=move |_| act(id, "pause")
                                                            >
                                                                "Pause"
                                                            </button>
                                                        }.into_view()]
                                                    } else {
                                                        Vec::<View>::new()
                                                    }}
                                                    {if status == "paused" {
                                                        let act = act.clone();
                                                        vec![view! {
                                                            <button
                                                                class="spp-button spp-button--small"
                                                                on:click=move |_| act(id, "resume")
                                                            >
                                                                "Resume"
                                                            </button>
                                                        }.into_view()]
                                                    } else {
                                                        Vec::<View>::new()
                                                    }}
                                                    {if status == "queued" || status == "sending" || status == "paused" {
                                                        let act = act.clone();
                                                        vec![view! {
                                                            <button
                                                                class="spp-button spp-button--ghost spp-button--small"
                                                                on:click=move |_| act(id, "cancel")
                                                            >
                                                                "Cancel rest"
                                                            </button>
                                                        }.into_view()]
                                                    } else {
                                                        Vec::<View>::new()
                                                    }}
                                                    {if failed > 0 {
                                                        let act = act.clone();
                                                        vec![view! {
                                                            <button
                                                                class="spp-button spp-button--ghost spp-button--small"
                                                                on:click=move |_| act(id, "retry")
                                                            >
                                                                "Retry failed"
                                                            </button>
                                                        }.into_view()]
                                                    } else {
                                                        Vec::<View>::new()
                                                    }}
                                                    {if unknown > 0 {
                                                        let act = act.clone();
                                                        vec![view! {
                                                            <button
                                                                class="spp-button spp-button--ghost spp-button--small"
                                                                on:click=move |_| act(id, "reconcile")
                                                            >
                                                                "Reconcile"
                                                            </button>
                                                        }.into_view()]
                                                    } else {
                                                        Vec::<View>::new()
                                                    }}
                                                </div>
                                            </td>
                                        </tr>
                                    }
                                })
                                .collect::<Vec<_>>()}
                        </tbody>
                    </table>
                </div>
            }
            .into_view()
        }}
        {move || {
            let open = open_id.get();
            let Some(d) = detail.get() else { return ().into_view() };
            let _ = open;
            let name = d.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let recipients_list = d
                .get("recipients_list")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            view! {
                <div class="spp-card spp-mt-16">
                    <div class="spp-flex--between">
                        <h3 class="spp-card__title" style="margin-bottom:0;">
                            {format!("{name} \u{2014} recipient monitor")}
                        </h3>
                        <button
                            class="spp-button spp-button--ghost spp-button--small"
                            on:click=move |_| open_id.set(None)
                        >
                            "Close"
                        </button>
                    </div>
                    {move || {
                        let Some(r) = report.get() else { return ().into_view() };
                        let t = r.get("totals").cloned().unwrap_or_default();
                        let reply_rate = t.get("reply_rate").and_then(|x| x.as_f64());
                        let reply_label = match reply_rate {
                            Some(rate) => format!(
                                "replied {} ({}%)",
                                t.get("replied").and_then(|x| x.as_i64()).unwrap_or(0),
                                (rate * 100.0).round() as i64
                            ),
                            None => format!("replied {}", t.get("replied").and_then(|x| x.as_i64()).unwrap_or(0)),
                        };
                        view! {
                            <div class="spp-flex spp-flex--wrap spp-gap-4 spp-mt-8">
                                <span class="spp-badge">{format!("recipients {}", t.get("recipients").and_then(|x| x.as_i64()).unwrap_or(0))}</span>
                                <span class="spp-badge spp-badge--ok">{format!("sent {}", t.get("sent").and_then(|x| x.as_i64()).unwrap_or(0))}</span>
                                <span class="spp-badge spp-badge--err">{format!("failed {}", t.get("failed").and_then(|x| x.as_i64()).unwrap_or(0))}</span>
                                <span class="spp-badge spp-badge--warn">{format!("skipped {}", t.get("skipped").and_then(|x| x.as_i64()).unwrap_or(0))}</span>
                                <span class="spp-badge spp-badge--warn">{format!("cancelled {}", t.get("cancelled").and_then(|x| x.as_i64()).unwrap_or(0))}</span>
                                <span class="spp-badge spp-badge--warn">{format!("unknown {}", t.get("unknown").and_then(|x| x.as_i64()).unwrap_or(0))}</span>
                                <span class="spp-badge spp-badge--active">{reply_label}</span>
                            </div>
                        }
                        .into_view()
                    }}
                    <table class="spp-table spp-table--compact spp-mt-8">
                        <thead>
                            <tr>
                                <th>"Customer"</th>
                                <th>"Email"</th>
                                <th>"State"</th>
                                <th>"Attempts"</th>
                                <th>"Conversation"</th>
                                <th>"Sent"</th>
                                <th>"Replied"</th>
                                <th>"Why selected"</th>
                            </tr>
                        </thead>
                        <tbody>
                            {recipients_list
                                .iter()
                                .map(|r| {
                                    let first = r.get("first_name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                    let last = r.get("last_name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                    let email = r.get("email").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                    let state = r.get("state").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                    let attempts = r.get("attempts").and_then(|v| v.as_i64()).unwrap_or(0);
                                    let last_error = r.get("last_error").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                    let state_class = state.clone();
                                    let conv_local = r.get("conversation_local_id").and_then(|v| v.as_i64());
                                    let hs_number = r.get("hs_conversation_number").and_then(|v| v.as_i64());
                                    let sent_at = r.get("sent_at").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                    let replied_at = r.get("replied_at").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                    let why: Vec<String> = r
                                        .get("why")
                                        .and_then(|v| v.as_array())
                                        .map(|a| {
                                            a.iter()
                                                .take(2)
                                                .filter_map(|w| w.get("text").and_then(|t| t.as_str()).map(str::to_string))
                                                .collect()
                                        })
                                        .unwrap_or_default();
                                    let tickets = r
                                        .get("matching_tickets")
                                        .and_then(|v| v.as_array())
                                        .cloned()
                                        .unwrap_or_default();
                                    view! {
                                        <tr>
                                            <td class="spp-text-sm">{format!("{first} {last}")}</td>
                                            <td class="spp-text-xs">
                                                {if email.is_empty() { "\u{2014}".to_string() } else { email.to_string() }}
                                            </td>
                                            <td>
                                                <span class=move || recipient_state_pill_class(&state_class)>{state}</span>
                                                {if !last_error.is_empty() {
                                                    vec![view! {
                                                        <div class="spp-text-xs spp-outreach__error">
                                                            {last_error.chars().take(140).collect::<String>()}
                                                        </div>
                                                    }.into_view()]
                                                } else {
                                                    Vec::<View>::new()
                                                }}
                                            </td>
                                            <td class="spp-text-sm">{attempts.to_string()}</td>
                                            <td class="spp-text-sm">
                                                {match (conv_local, hs_number) {
                                                    (Some(c), Some(n)) => view! {
                                                        <a class="spp-text-xs" href=format!("/inbox/conversation/{c}")>
                                                            {format!("#{n}")}
                                                        </a>
                                                    }.into_view(),
                                                    (None, Some(n)) => view! { {format!("#{n}")} }.into_view(),
                                                    _ => "\u{2014}".into_view(),
                                                }}
                                            </td>
                                            <td class="spp-muted spp-text-xs">{date_short(&sent_at)}</td>
                                            <td class="spp-muted spp-text-xs">{date_short(&replied_at)}</td>
                                            <td class="spp-text-xs">
                                                {why
                                                    .iter()
                                                    .map(|w| view! { <div>{"\u{2713} "}{w.clone()}</div> })
                                                    .collect::<Vec<_>>()}
                                                {tickets
                                                    .iter()
                                                    .take(2)
                                                    .map(|t| {
                                                        let conv = t.get("conversationId").and_then(|v| v.as_i64()).unwrap_or(0);
                                                        let number = t.get("number").and_then(|v| v.as_i64()).unwrap_or(0);
                                                        view! {
                                                            <a class="spp-text-xs" href=format!("/inbox/conversation/{conv}")>
                                                                {format!("#{number}")}
                                                            </a>
                                                        }
                                                    })
                                                    .collect::<Vec<_>>()}
                                            </td>
                                        </tr>
                                    }
                                })
                                .collect::<Vec<_>>()}
                        </tbody>
                    </table>
                    {move || {
                        let Some(r) = report.get() else { return ().into_view() };
                        let replies = r
                            .get("replies")
                            .and_then(|v| v.as_array())
                            .cloned()
                            .unwrap_or_default();
                        if replies.is_empty() {
                            return ().into_view();
                        }
                        let note = r.get("note").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        view! {
                            <div class="spp-mt-16">
                                <h4 class="spp-card__title">"Reply intelligence"</h4>
                                {replies
                                    .iter()
                                    .map(|r| {
                                        let customer = r.get("customer").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                        let conv_local = r.get("conversation_local_id").and_then(|v| v.as_i64());
                                        let conv_number = r.get("conversation_number").and_then(|v| v.as_i64());
                                        let replied_at = r.get("replied_at").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                        view! {
                                            <div class="spp-flex spp-gap-8" style="padding:3px 0;">
                                                <strong class="spp-text-sm">{customer}</strong>
                                                {match (conv_local, conv_number) {
                                                    (Some(c), Some(n)) => vec![view! {
                                                        <a class="spp-text-xs" href=format!("/inbox/conversation/{c}")>
                                                            {format!("#{n}")}
                                                        </a>
                                                    }.into_view()],
                                                    _ => Vec::<View>::new(),
                                                }}
                                                <span class="spp-muted spp-text-xs">{format!("replied {replied_at}")}</span>
                                            </div>
                                        }
                                    })
                                    .collect::<Vec<_>>()}
                                <p class="spp-muted spp-text-xs">{note}</p>
                            </div>
                        }
                        .into_view()
                    }}
                    <div class="spp-mt-16">
                        <h4 class="spp-card__title">"Audit events"</h4>
                        <div class="spp-events-list">
                            {move || {
                                events.get()
                                    .into_iter()
                                    .map(|e| {
                                        let at = e.get("at").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                        let event = e.get("event").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                        let detail_txt = e.get("detail").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                        view! {
                                            <div class="spp-flex spp-gap-8">
                                                <span class="spp-muted spp-text-xs">{at}</span>
                                                <span class="spp-text-xs">{event}</span>
                                                <span class="spp-muted spp-text-xs">{detail_txt}</span>
                                            </div>
                                        }
                                    })
                                    .collect::<Vec<_>>()
                            }}
                        </div>
                    </div>
                </div>
            }
            .into_view()
        }}
    }
}

#[component]
fn SegmentsPanel(
    segments: RwSignal<Vec<serde_json::Value>>,
    load_segments: Rc<dyn Fn()>,
    on_use: Rc<dyn Fn(serde_json::Value)>,
) -> impl IntoView {
    let deleting = create_rw_signal(false);
    let del = Rc::new(move |id: i64| {
        if deleting.get_untracked() {
            return;
        }
        deleting.set(true);
        let path = format!("/api/outreach/segments/{id}");
        let load_segments = load_segments.clone();
        spawn_local(async move {
            match crate::api::delete_json::<serde_json::Value>(&path).await {
                Ok(r) => {
                    let ok = r.get("ok").and_then(|v| v.as_bool()).unwrap_or(false);
                    let message = r
                        .get("message")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Request failed.")
                        .to_string();
                    if ok {
                        toasts::success(message);
                        load_segments();
                    } else {
                        toasts::error(message);
                    }
                }
                Err(e) => toasts::error(e),
            }
            deleting.set(false);
        });
    });

    view! {
        {move || {
            let list = segments.get();
            if list.is_empty() {
                return view! {
                    <EmptyState message="No saved segments. Save an audience rule from the New campaign builder to reuse it later." />
                }
                .into_view();
            }
            view! {
                <div class="spp-card" style="padding:0;">
                    <table class="spp-table">
                        <thead>
                            <tr>
                                <th>"Segment"</th>
                                <th>"Rules"</th>
                                <th>"Version"</th>
                                <th>"Updated"</th>
                                <th></th>
                            </tr>
                        </thead>
                        <tbody>
                            {list
                                .into_iter()
                                .map(|s| {
                                    let on_use = on_use.clone();
                                    let del = del.clone();
                                    let id = s.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
                                    let name = s.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                    let description = s
                                        .get("description")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("")
                                        .to_string();
                                    let version = s.get("version").and_then(|v| v.as_i64()).unwrap_or(1);
                                    let updated = s.get("updated_at").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                    let definition = s.get("definition").cloned().unwrap_or_default();
                                    let combinator = if definition.get("combinator").and_then(|v| v.as_str()) == Some("any") {
                                        "ANY"
                                    } else {
                                        "ALL"
                                    };
                                    let conds: Vec<serde_json::Value> = definition
                                        .get("conditions")
                                        .and_then(|v| v.as_array())
                                        .cloned()
                                        .unwrap_or_default();
                                    let excl: Vec<serde_json::Value> = definition
                                        .get("exclude")
                                        .and_then(|v| v.as_array())
                                        .cloned()
                                        .unwrap_or_default();
                                    let rules_summary = format!(
                                        "{combinator} of {} condition{}",
                                        conds.len(),
                                        if conds.len() == 1 { "" } else { "s" }
                                    );
                                    view! {
                                        <tr>
                                            <td>
                                                <strong class="spp-text-sm">{name}</strong>
                                                {if !description.is_empty() {
                                                    vec![view! {
                                                        <div class="spp-muted spp-text-xs">{description}</div>
                                                    }.into_view()]
                                                } else {
                                                    Vec::<View>::new()
                                                }}
                                            </td>
                                            <td class="spp-text-xs">
                                                <div>{rules_summary}</div>
                                                {conds
                                                    .iter()
                                                    .take(3)
                                                    .map(|c| view! {
                                                        <div class="spp-muted">{"\u{b7} "}{describe_condition(c)}</div>
                                                    })
                                                    .collect::<Vec<_>>()}
                                                {if !excl.is_empty() {
                                                    vec![view! {
                                                        <div class="spp-muted">
                                                            {format!("excluding {} rule(s)", excl.len())}
                                                        </div>
                                                    }.into_view()]
                                                } else {
                                                    Vec::<View>::new()
                                                }}
                                            </td>
                                            <td class="spp-text-sm">{format!("v{version}")}</td>
                                            <td class="spp-muted spp-text-xs">{date_short(&updated)}</td>
                                            <td>
                                                <div class="spp-flex spp-gap-4">
                                                    <button
                                                        class="spp-button spp-button--small"
                                                        on:click=move |_| on_use(s.clone())
                                                    >
                                                        "Use"
                                                    </button>
                                                    <button
                                                        class="spp-button spp-button--ghost spp-button--small"
                                                        on:click=move |_| del(id)
                                                    >
                                                        "Delete"
                                                    </button>
                                                </div>
                                            </td>
                                        </tr>
                                    }
                                })
                                .collect::<Vec<_>>()}
                        </tbody>
                    </table>
                </div>
            }
            .into_view()
        }}
    }
}

#[component]
fn DncPanel() -> impl IntoView {
    let dnc = create_rw_signal(Vec::<serde_json::Value>::new());
    let candidates = create_rw_signal(Vec::<serde_json::Value>::new());
    let loading = create_rw_signal(true);
    let error_msg = create_rw_signal(None::<String>);
    let acting = create_rw_signal(false);

    let load_dnc = move || {
        let dnc = dnc;
        spawn_local(async move {
            match crate::api::get_json::<serde_json::Value>("/api/outreach/dnc").await {
                Ok(v) => dnc.set(
                    v.get("dnc")
                        .and_then(|d| d.as_array())
                        .cloned()
                        .unwrap_or_default(),
                ),
                Err(e) => toasts::error(e),
            }
        });
    };
    let load_candidates = move || {
        let candidates = candidates;
        let loading = loading;
        let error_msg = error_msg;
        spawn_local(async move {
            let body = serde_json::json!({
                "combinator": "all",
                "conditions": [],
                "exclude": [],
                "page": 1,
                "pageSize": 200,
            });
            match crate::api::post_json::<serde_json::Value>(
                "/api/outreach/segments/preview",
                Some(&body),
            )
            .await
            {
                Ok(v) => {
                    error_msg.set(None);
                    candidates.set(
                        v.get("rows")
                            .and_then(|r| r.as_array())
                            .cloned()
                            .unwrap_or_default(),
                    );
                }
                Err(e) => error_msg.set(Some(e)),
            }
            loading.set(false);
        });
    };
    load_dnc();
    load_candidates();

    let add_dnc = move |customer_local_id: i64| {
        if acting.get_untracked() {
            return;
        }
        acting.set(true);
        let body = serde_json::json!({ "customer_local_id": customer_local_id });
        let load_dnc = load_dnc;
        spawn_local(async move {
            match crate::api::post_json::<serde_json::Value>("/api/outreach/dnc", Some(&body)).await
            {
                Ok(r) => {
                    let ok = r.get("ok").and_then(|v| v.as_bool()).unwrap_or(false);
                    let message = r
                        .get("message")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Request failed.")
                        .to_string();
                    if ok {
                        toasts::success(message);
                        load_dnc();
                    } else {
                        toasts::error(message);
                    }
                }
                Err(e) => toasts::error(e),
            }
            acting.set(false);
        });
    };
    let remove_dnc = move |customer_local_id: i64| {
        if acting.get_untracked() {
            return;
        }
        acting.set(true);
        let path = format!("/api/outreach/dnc/{customer_local_id}");
        let load_dnc = load_dnc;
        spawn_local(async move {
            match crate::api::delete_json::<serde_json::Value>(&path).await {
                Ok(r) => {
                    let message = r
                        .get("message")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Request failed.")
                        .to_string();
                    toasts::success(message);
                    load_dnc();
                }
                Err(e) => toasts::error(e),
            }
            acting.set(false);
        });
    };

    view! {
        <Show when=move || loading.get() fallback=|| ()>
            <LoadingState />
        </Show>
        <Show when=move || error_msg.get().is_some() fallback=|| ()>
            <div class="spp-state spp-state--error">
                <span class="spp-state__icon" aria-hidden="true">"\u{26a0}"</span>
                <p class="spp-state__body">{move || error_msg.get().unwrap_or_default()}</p>
            </div>
        </Show>
        <div class="spp-grid-2">
            <div class="spp-card">
                <h3 class="spp-card__title">"\u{1f6e1} Do-Not-Contact list"</h3>
                <p class="spp-muted spp-text-xs">
                    "Every campaign skips these customers - always, before any other check."
                </p>
                {move || {
                    let list = dnc.get();
                    if list.is_empty() {
                        return vec![view! { <EmptyState message="Nobody on the list." /> }.into_view()];
                    }
                    list.into_iter()
                        .map(|d| {
                            let cid = d.get("customer_local_id").and_then(|v| v.as_i64()).unwrap_or(0);
                            let first = d.get("first_name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                            let last = d.get("last_name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                            let reason = d.get("reason").and_then(|v| v.as_str()).unwrap_or("").to_string();
                            let created = d.get("created_at").and_then(|v| v.as_str()).unwrap_or("").to_string();
                            view! {
                                <div class="spp-flex--between spp-dnc-row">
                                    <div>
                                        <strong class="spp-text-sm">{format!("{first} {last}")}</strong>
                                        {if !reason.is_empty() {
                                            vec![view! {
                                                <div class="spp-muted spp-text-xs">{reason}</div>
                                            }.into_view()]
                                        } else {
                                            Vec::<View>::new()
                                        }}
                                    </div>
                                    <div class="spp-flex spp-gap-8">
                                        <span class="spp-muted spp-text-xs">{date_short(&created)}</span>
                                        <button
                                            class="spp-button spp-button--ghost spp-button--small"
                                            on:click=move |_| remove_dnc(cid)
                                        >
                                            "Remove"
                                        </button>
                                    </div>
                                </div>
                            }.into_view()
                        })
                        .collect::<Vec<_>>()
                }}
            </div>
            <div class="spp-card">
                <h3 class="spp-card__title">"\u{1f465} Add a customer"</h3>
                <p class="spp-muted spp-text-xs">"From your local mirror:"</p>
                <div class="spp-dnc-candidates">
                    {move || {
                        let on_dnc: std::collections::HashSet<i64> = dnc
                            .get()
                            .iter()
                            .filter_map(|d| d.get("customer_local_id").and_then(|v| v.as_i64()))
                            .collect();
                        candidates
                            .get()
                            .into_iter()
                            .filter(|r| {
                                !on_dnc.contains(
                                    &r.get("customer_local_id")
                                        .and_then(|v| v.as_i64())
                                        .unwrap_or(0),
                                )
                            })
                            .take(100)
                            .map(|r| {
                                let cid = r.get("customer_local_id").and_then(|v| v.as_i64()).unwrap_or(0);
                                let first = r.get("first_name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                let last = r.get("last_name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                let email = r.get("chosen_email").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                view! {
                                    <div class="spp-flex--between spp-dnc-row">
                                        <div>
                                            <strong class="spp-text-sm">{format!("{first} {last}")}</strong>
                                            <div class="spp-muted spp-text-xs">{email}</div>
                                        </div>
                                        <button
                                            class="spp-button spp-button--ghost spp-button--small"
                                            on:click=move |_| add_dnc(cid)
                                        >
                                            "Add"
                                        </button>
                                    </div>
                                }
                            })
                            .collect::<Vec<_>>()
                    }}
                </div>
            </div>
        </div>
    }
}
