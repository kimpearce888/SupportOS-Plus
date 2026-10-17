use leptos::prelude::*;

use crate::components::button::{Button, ButtonStyle};
use crate::components::state_view::EmptyState;

pub const SUPPORTED_WEBHOOK_EVENTS: [&str; 12] = [
    "convo.created",
    "convo.updated",
    "convo.assigned",
    "convo.status",
    "convo.customer.reply.created",
    "convo.agent.reply.created",
    "convo.note.created",
    "satisfaction.ratings",
    "customer.created",
    "customer.updated",
    "conversation.merged",
    "team.updated",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebhookPushState {
    NotConfigured,
    Registered,
    Receiving,
    Error,
}

impl WebhookPushState {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::NotConfigured => "Not configured",
            Self::Registered => "Registered",
            Self::Receiving => "Receiving",
            Self::Error => "Error",
        }
    }

    #[must_use]
    pub fn css_class(self) -> &'static str {
        match self {
            Self::NotConfigured => "spp-webhook-state--not-configured",
            Self::Registered => "spp-webhook-state--registered",
            Self::Receiving => "spp-webhook-state--receiving",
            Self::Error => "spp-webhook-state--error",
        }
    }

    #[must_use]
    pub fn is_realtime(self) -> bool {
        matches!(self, Self::Receiving)
    }
}

#[must_use]
pub fn derive_push_state(status: &serde_json::Value) -> WebhookPushState {
    let events = status
        .pointer("/webhook/events/total")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let configured = status
        .pointer("/webhook/configured")
        .and_then(|v| v.as_array())
        .map(|a| !a.is_empty())
        .unwrap_or(false);
    if events > 0 {
        WebhookPushState::Receiving
    } else if configured {
        WebhookPushState::Registered
    } else {
        WebhookPushState::NotConfigured
    }
}

#[component]
pub fn SyncHealthPage() -> impl IntoView {
    let status = RwSignal::new(serde_json::Value::Null);
    let push_state = RwSignal::new(WebhookPushState::NotConfigured);
    let loading = RwSignal::new(true);
    let error_msg = RwSignal::new(None::<String>);
    let action_msg = RwSignal::new(None::<String>);
    let running = RwSignal::new(false);
    let webhook_url_input = RwSignal::new(String::new());
    let selected_events = RwSignal::new(
        SUPPORTED_WEBHOOK_EVENTS
            .iter()
            .map(|e| (*e, true))
            .collect::<Vec<(&'static str, bool)>>(),
    );
    let reload = RwSignal::new(0u32);

    Effect::new(move |_| {
        let _ = reload.get();
        let status = status;
        let push_state = push_state;
        let running = running;
        let loading = loading;
        let error_msg = error_msg;
        wasm_bindgen_futures::spawn_local(async move {
            match crate::api::get_json::<serde_json::Value>("/api/sync/status").await {
                Ok(data) => {
                    running.set(
                        data.get("running")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false),
                    );
                    push_state.set(derive_push_state(&data));
                    status.set(data);
                    loading.set(false);
                }
                Err(e) => {
                    error_msg.set(Some(e));
                    push_state.set(WebhookPushState::NotConfigured);
                    loading.set(false);
                }
            }
        });
    });

    let run_action = move |action: &'static str| {
        let reload = reload;
        let action_msg = action_msg;
        let running = running;
        let action = action.to_string();
        wasm_bindgen_futures::spawn_local(async move {
            running.set(true);
            let result = crate::api::post_json::<serde_json::Value>(
                &format!("/api/sync/{action}"),
                Some(&serde_json::json!({ "wait": true })),
            )
            .await;
            match result {
                Ok(body) => {
                    let message = body
                        .get("message")
                        .and_then(|v| v.as_str())
                        .map(str::to_string)
                        .unwrap_or_else(|| format!("{action} completed."));
                    action_msg.set(Some(message));
                }
                Err(e) => action_msg.set(Some(e)),
            }
            running.set(false);
            reload.set(reload.get_untracked() + 1);
        });
    };

    let on_register = move || {
        let url = webhook_url_input.get_untracked();
        let events: Vec<String> = selected_events
            .get_untracked()
            .iter()
            .filter(|(_, on)| *on)
            .map(|(e, _)| e.to_string())
            .collect();
        let reload = reload;
        let action_msg = action_msg;
        wasm_bindgen_futures::spawn_local(async move {
            let result = crate::api::post_json::<serde_json::Value>(
                "/api/webhooks/register",
                Some(&serde_json::json!({ "url": url, "events": events })),
            )
            .await;
            let message = match result {
                Ok(body) => {
                    let ok = body.get("ok").and_then(|v| v.as_bool()).unwrap_or(false);
                    let text = body
                        .get("message")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Registered.")
                        .to_string();
                    if ok {
                        format!("Webhook registered. {text}")
                    } else {
                        text
                    }
                }
                Err(e) => e,
            };
            action_msg.set(Some(message));
            reload.set(reload.get_untracked() + 1);
        });
    };

    let on_delete = move |remote_id: i64| {
        let reload = reload;
        let action_msg = action_msg;
        wasm_bindgen_futures::spawn_local(async move {
            let result =
                crate::api::delete_json::<serde_json::Value>(&format!("/api/webhooks/{remote_id}"))
                    .await;
            let message = match result {
                Ok(body) => body
                    .get("message")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Deleted.")
                    .to_string(),
                Err(e) => e,
            };
            action_msg.set(Some(message));
            reload.set(reload.get_untracked() + 1);
        });
    };

    view! {
        <div class="spp-page spp-page--sync-health">
            <h2 class="spp-page__title">"Sync Health"</h2>

            <Show when=move || error_msg.get().is_some() fallback=|| ().into_any()>
                <div class="spp-state spp-state--error">
                    <p class="spp-state__body">{move || error_msg.get().unwrap_or_default()}</p>
                </div>
            </Show>

            <section class="spp-sync-health-cards">
                {move || {
                    let s = status.get();
                    let cards = if s.is_null() {
                        vec![]
                    } else {
                        let state = s
                            .get("state")
                            .and_then(|v| v.as_str())
                            .unwrap_or("never")
                            .to_string();
                        let last = s
                            .pointer("/last_success/finished_at")
                            .and_then(|v| v.as_str())
                            .unwrap_or("never")
                            .to_string();
                        let records = s
                            .pointer("/last_success/records_processed")
                            .and_then(|v| v.as_i64())
                            .unwrap_or(0);
                        let pending = s
                            .pointer("/webhook/events/pending")
                            .and_then(|v| v.as_i64())
                            .unwrap_or(0);
                        vec![
                            ("Sync state", state, "state"),
                            ("Running", if running.get() { "yes".into() } else { "no".into() }, "running"),
                            ("Last successful sync", format!("{last} ({records} records)"), "last-success"),
                            ("Webhook events pending", pending.to_string(), "webhook-pending"),
                        ]
                    };
                    cards
                        .into_iter()
                        .map(|(title, value, key)| {
                            view! {
                                <div class=format!("spp-sync-health-card spp-sync-health-card--{key}")>
                                    <span class="spp-sync-health-card__title">{title}</span>
                                    <span class="spp-sync-health-card__value">{value}</span>
                                </div>
                            }
                        })
                        .collect_view()
                }}
            </section>

            <section class="spp-sync-actions">
                <h3 class="spp-sync-actions__title">"Sync actions"</h3>
                <div class="spp-sync-actions__row">
                    <Button on_click=move || run_action("initial") style=ButtonStyle::Primary>
                        "Full sync"
                    </Button>
                    <Button on_click=move || run_action("incremental") style=ButtonStyle::Ghost>
                        "Incremental sync"
                    </Button>
                    <Button on_click=move || run_action("reconcile") style=ButtonStyle::Ghost>
                        "Reconcile"
                    </Button>
                    <Button on_click=move || run_action("cancel") style=ButtonStyle::Ghost>
                        "Cancel"
                    </Button>
                </div>
                <Show when=move || action_msg.get().is_some() fallback=|| ().into_any()>
                    <div class="spp-state spp-state--info">
                        <p class="spp-state__body">{move || action_msg.get().unwrap_or_default()}</p>
                    </div>
                </Show>
                <p class="spp-muted spp-text-xs">
                    "Actions run against the connected Help Scout mailbox; the fake provider in demo mode serves seeded data."
                </p>
            </section>

            <section class="spp-sync-checkpoints">
                <h3 class="spp-sync-checkpoints__title">"Checkpoints"</h3>
                <Show
                    when=move || !loading.get() && status.get().get("checkpoints").and_then(|v| v.as_array()).map(|a| !a.is_empty()).unwrap_or(false)
                    fallback=move || {
                        view! {
                            <EmptyState message="No checkpoints yet — they appear after the first sync." />
                        }
                    }
                >
                    <table class="spp-table">
                        <thead>
                            <tr>
                                <th>"Resource"</th>
                                <th>"Status"</th>
                                <th>"Last success"</th>
                                <th>"Processed"</th>
                                <th>"Failed"</th>
                                <th>"Retries"</th>
                            </tr>
                        </thead>
                        <tbody>
                            {move || {
                                status.get()
                                    .get("checkpoints")
                                    .and_then(|v| v.as_array())
                                    .cloned()
                                    .unwrap_or_default()
                                    .iter()
                                    .map(|c| {
                                        view! {
                                            <tr>
                                                <td>{c.get("resource").and_then(|v| v.as_str()).unwrap_or_default().to_string()}</td>
                                                <td>{c.get("status").and_then(|v| v.as_str()).unwrap_or_default().to_string()}</td>
                                                <td>{c.get("last_success_at").and_then(|v| v.as_str()).unwrap_or("—").to_string()}</td>
                                                <td>{c.get("records_processed").and_then(|v| v.as_i64()).unwrap_or(0).to_string()}</td>
                                                <td>{c.get("records_failed").and_then(|v| v.as_i64()).unwrap_or(0).to_string()}</td>
                                                <td>{c.get("retry_count").and_then(|v| v.as_i64()).unwrap_or(0).to_string()}</td>
                                            </tr>
                                        }
                                    })
                                    .collect_view()
                            }}
                        </tbody>
                    </table>
                </Show>
            </section>

            <section class="spp-sync-runs">
                <h3 class="spp-sync-runs__title">"Recent runs"</h3>
                <Show
                    when=move || !loading.get() && status.get().get("recent_runs").and_then(|v| v.as_array()).map(|a| !a.is_empty()).unwrap_or(false)
                    fallback=move || {
                        view! {
                            <EmptyState message="No sync runs recorded yet." />
                        }
                    }
                >
                    <table class="spp-table">
                        <thead>
                            <tr>
                                <th>"#"</th>
                                <th>"Kind"</th>
                                <th>"State"</th>
                                <th>"Started"</th>
                                <th>"Finished"</th>
                                <th>"Records"</th>
                                <th>"Errors"</th>
                            </tr>
                        </thead>
                        <tbody>
                            {move || {
                                status.get()
                                    .get("recent_runs")
                                    .and_then(|v| v.as_array())
                                    .cloned()
                                    .unwrap_or_default()
                                    .iter()
                                    .map(|r| {
                                        view! {
                                            <tr>
                                                <td>{r.get("id").and_then(|v| v.as_i64()).unwrap_or(0).to_string()}</td>
                                                <td>{r.get("kind").and_then(|v| v.as_str()).unwrap_or_default().to_string()}</td>
                                                <td>{r.get("state").and_then(|v| v.as_str()).unwrap_or_default().to_string()}</td>
                                                <td>{r.get("started_at").and_then(|v| v.as_str()).unwrap_or("—").to_string()}</td>
                                                <td>{r.get("finished_at").and_then(|v| v.as_str()).unwrap_or("running…").to_string()}</td>
                                                <td>{r.get("records_processed").and_then(|v| v.as_i64()).unwrap_or(0).to_string()}</td>
                                                <td>{r.get("errors").and_then(|v| v.as_i64()).unwrap_or(0).to_string()}</td>
                                            </tr>
                                        }
                                    })
                                    .collect_view()
                            }}
                        </tbody>
                    </table>
                </Show>
            </section>

            <section class="spp-webhook-push">
                <h3 class="spp-webhook-push__title">"Webhook push"</h3>

                <p class="spp-webhook-push__explanation">
                    "Help Scout can push real-time events to a URL on your machine. "
                    "You need to make your machine reachable from the internet (e.g. via port forwarding or a reverse proxy) "
                    "and enter that public URL below. Help Scout will POST events to that URL; "
                    "SupportOS++ verifies each event's HMAC signature and processes it locally."
                </p>

                <div class="spp-webhook-push__state">
                    <span class=move || format!("spp-webhook-state-badge {}", push_state.get().css_class())>
                        {move || push_state.get().label()}
                    </span>
                    <Show when=move || push_state.get().is_realtime() fallback=|| ()>
                        <span class="spp-webhook-realtime-indicator">"● Real-time"</span>
                    </Show>
                </div>

                <div class="spp-webhook-push__form">
                    <label class="spp-webhook-push__label" for="webhook-url-input">
                        "Public https URL"
                        <input
                            id="webhook-url-input"
                            class="spp-input"
                            type="url"
                            placeholder="https://your-public-host/api/webhooks/helpscout"
                            prop:value=move || webhook_url_input.get()
                            on:input=move |ev| webhook_url_input.set(event_target_value(&ev))
                        />
                    </label>
                    <fieldset class="spp-webhook-push__events">
                        <legend>"Events"</legend>
                        {selected_events.get_untracked().iter().map(|(event, _)| {
                            let event = *event;
                            view! {
                                <label class="spp-webhook-push__event">
                                    <input
                                        type="checkbox"
                                        prop:checked=move || {
                                            selected_events.get()
                                                .iter()
                                                .find(|(e, _)| *e == event)
                                                .map(|(_, on)| *on)
                                                .unwrap_or(false)
                                        }
                                        on:change=move |_| {
                                            selected_events.update(|list| {
                                                if let Some(entry) =
                                                    list.iter_mut().find(|(e, _)| *e == event)
                                                {
                                                    entry.1 = !entry.1;
                                                }
                                            });
                                        }
                                    />
                                    {event}
                                </label>
                            }
                        }).collect_view()}
                    </fieldset>
                    <Button on_click=on_register style=ButtonStyle::Primary>
                        "Register webhook"
                    </Button>
                </div>

                <Show
                    when=move || status.get().pointer("/webhook/configured").and_then(|v| v.as_array()).map(|a| !a.is_empty()).unwrap_or(false)
                    fallback=|| ().into_any()
                >
                    <div class="spp-webhook-push__configured">
                        <span class="spp-webhook-push__label">"Registered webhooks:"</span>
                        {move || {
                            status.get()
                                .pointer("/webhook/configured")
                                .and_then(|v| v.as_array())
                                .cloned()
                                .unwrap_or_default()
                                .iter()
                                .map(|w| {
                                    let remote_id = w.get("remote_id").and_then(|v| v.as_i64()).unwrap_or(0);
                                    let url = w.get("url").and_then(|v| v.as_str()).unwrap_or_default().to_string();
                                    view! {
                                        <div class="spp-webhook-push__url">
                                            <code>{url}</code>
                                            <Button on_click=move || on_delete(remote_id) style=ButtonStyle::Ghost>
                                "Delete"
                            </Button>
                                        </div>
                                    }
                                })
                                .collect_view()
                        }}
                    </div>
                </Show>

                <p class="spp-webhook-push__polling-note">
                    "Incremental polling runs every 5 minutes regardless of webhook configuration. "
                    "Webhook push is optional and provides real-time updates when configured."
                </p>
            </section>

            <section class="spp-sync-webhook-queue">
                <h3 class="spp-sync-webhook-queue__title">"Webhook events"</h3>
                <Show
                    when=move || status.get().pointer("/webhook/recent").and_then(|v| v.as_array()).map(|a| !a.is_empty()).unwrap_or(false)
                    fallback=move || {
                        view! {
                            <EmptyState message="No webhook events received yet." />
                        }
                    }
                >
                    <table class="spp-table">
                        <thead>
                            <tr>
                                <th>"Event"</th>
                                <th>"Received"</th>
                                <th>"State"</th>
                                <th>"Attempts"</th>
                                <th>"Error"</th>
                            </tr>
                        </thead>
                        <tbody>
                            {move || {
                                status.get()
                                    .pointer("/webhook/recent")
                                    .and_then(|v| v.as_array())
                                    .cloned()
                                    .unwrap_or_default()
                                    .iter()
                                    .map(|e| {
                                        view! {
                                            <tr>
                                                <td>{e.get("event_type").and_then(|v| v.as_str()).unwrap_or_default().to_string()}</td>
                                                <td>{e.get("received_at").and_then(|v| v.as_str()).unwrap_or("—").to_string()}</td>
                                                <td>{e.get("processing_state").and_then(|v| v.as_str()).unwrap_or_default().to_string()}</td>
                                                <td>{e.get("attempts").and_then(|v| v.as_i64()).unwrap_or(0).to_string()}</td>
                                                <td>{e.get("processing_error").and_then(|v| v.as_str()).unwrap_or("—").to_string()}</td>
                                            </tr>
                                        }
                                    })
                                    .collect_view()
                            }}
                        </tbody>
                    </table>
                </Show>
            </section>
        </div>
    }
}
