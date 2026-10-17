use leptos::*;
use leptos_router::A;

use crate::components::state_view::{EmptyState, LoadingState};

#[derive(Clone, Copy, PartialEq, Eq)]
enum AiTab {
    Health,
    Analytics,
    Attributes,
    Copilot,
    Jobs,
    Evaluation,
}

#[component]
pub fn AiCenterPage() -> impl IntoView {
    let tab = create_rw_signal(AiTab::Health);
    let clustering = create_rw_signal(false);

    let run_clustering = move |_| {
        if clustering.get() {
            return;
        }
        clustering.set(true);
        wasm_bindgen_futures::spawn_local(async move {
            match crate::api::post_json::<serde_json::Value>(
                "/api/ai/cluster-issues",
                Some(&serde_json::json!({ "days": 60 })),
            )
            .await
            {
                Ok(r) => {
                    let ok = r.get("ok").and_then(|v| v.as_bool()).unwrap_or(false);
                    if ok {
                        let n = r
                            .get("clusters")
                            .and_then(|v| v.as_array())
                            .map(|a| a.len())
                            .unwrap_or(0);
                        crate::toasts::success(format!("Discovered {n} clusters."));
                    } else {
                        crate::toasts::error(
                            r.get("error")
                                .and_then(|v| v.as_str())
                                .unwrap_or("Clustering failed"),
                        );
                    }
                }
                Err(e) => crate::toasts::error(e),
            }
            clustering.set(false);
        });
    };

    view! {
        <div class="spp-page spp-page--ai-center">
            <header class="spp-page__header">
                <div>
                    <h2 class="spp-page__title">"AI Center"</h2>
                    <p class="spp-page__subtitle">
                        "Local AI (LM Studio) — no cloud providers. The app stays fully usable when AI is off."
                    </p>
                </div>
                <div class="spp-page__header-actions">
                    <button class="spp-button" on:click=run_clustering disabled=move || clustering.get()>
                        {move || if clustering.get() { "Clustering…" } else { "✦ Run issue clustering" }}
                    </button>
                </div>
            </header>

            <div class="spp-tabs">
                <button
                    class="spp-tab"
                    class:is-active=move || tab.get() == AiTab::Health
                    on:click=move |_| tab.set(AiTab::Health)
                >
                    "Local AI health"
                </button>
                <button
                    class="spp-tab"
                    class:is-active=move || tab.get() == AiTab::Analytics
                    on:click=move |_| tab.set(AiTab::Analytics)
                >
                    "AI analytics"
                </button>
                <button
                    class="spp-tab"
                    class:is-active=move || tab.get() == AiTab::Attributes
                    on:click=move |_| tab.set(AiTab::Attributes)
                >
                    "Tags Attributes"
                </button>
                <button
                    class="spp-tab"
                    class:is-active=move || tab.get() == AiTab::Copilot
                    on:click=move |_| tab.set(AiTab::Copilot)
                >
                    "Copilot"
                </button>
                <button
                    class="spp-tab"
                    class:is-active=move || tab.get() == AiTab::Jobs
                    on:click=move |_| tab.set(AiTab::Jobs)
                >
                    "AI jobs"
                </button>
                <button
                    class="spp-tab"
                    class:is-active=move || tab.get() == AiTab::Evaluation
                    on:click=move |_| tab.set(AiTab::Evaluation)
                >
                    "⚗ Evaluation"
                </button>
            </div>

            <Show when=move || tab.get() == AiTab::Health fallback=|| ()>
                <HealthTab />
            </Show>
            <Show when=move || tab.get() == AiTab::Analytics fallback=|| ()>
                <AnalyticsTab />
            </Show>
            <Show when=move || tab.get() == AiTab::Attributes fallback=|| ()>
                <AttributesTab />
            </Show>
            <Show when=move || tab.get() == AiTab::Copilot fallback=|| ()>
                <CopilotTab />
            </Show>
            <Show when=move || tab.get() == AiTab::Jobs fallback=|| ()>
                <JobsTab />
            </Show>
            <Show when=move || tab.get() == AiTab::Evaluation fallback=|| ()>
                <EvaluationTab />
            </Show>
        </div>
    }
}

#[component]
fn HealthTab() -> impl IntoView {
    let status = create_rw_signal(Option::<serde_json::Value>::None);
    let error_msg = create_rw_signal(None::<String>);
    let reload = create_rw_signal(0u32);

    create_effect(move |_| {
        let _ = reload.get();
        let status = status;
        let error_msg = error_msg;
        wasm_bindgen_futures::spawn_local(async move {
            match crate::api::get_json::<serde_json::Value>("/api/ai/status").await {
                Ok(s) => {
                    status.set(Some(s));
                    error_msg.set(None);
                }
                Err(e) => error_msg.set(Some(e)),
            }
        });
    });

    view! {
        <Show when=move || error_msg.get().is_some() fallback=|| ()>
            <div class="spp-state spp-state--error">
                <span class="spp-state__icon" aria-hidden="true">"⚠"</span>
                <p class="spp-state__body">{move || error_msg.get().unwrap_or_default()}</p>
            </div>
        </Show>
        <Show when=move || status.get().is_none() && error_msg.get().is_none() fallback=|| ()>
            <LoadingState />
        </Show>
        {move || {
            let Some(s) = status.get() else { return ().into_view(); };
            let settings = s.get("settings").cloned().unwrap_or(serde_json::Value::Null);
            let lmstudio = s.get("lmstudio").cloned().unwrap_or(serde_json::Value::Null);
            let index = s.get("index").cloned().unwrap_or(serde_json::Value::Null);
            let last_inference = s.get("last_inference").cloned().unwrap_or(serde_json::Value::Null);
            let connected = lmstudio.get("connected").and_then(|v| v.as_bool()).unwrap_or(false);
            let models: Vec<String> = lmstudio
                .get("models")
                .and_then(|v| v.as_array())
                .map(|a| a.iter().filter_map(|m| m.as_str().map(str::to_string)).collect())
                .unwrap_or_default();
            let lmstudio_error = str_field(&lmstudio, "error");
            let ai_enabled = s.get("ai_enabled").and_then(|v| v.as_bool()).unwrap_or(false);
            let queued = s.get("queued_ai_jobs").and_then(|v| v.as_i64()).unwrap_or(0);
            let failed = s.get("failed_ai_jobs").and_then(|v| v.as_i64()).unwrap_or(0);
            view! {
                <div class="spp-grid-2">
                    <div class="spp-card">
                        <h3 class="spp-card__title">"LM Studio connection"</h3>
                        <div class="spp-kv"><span>"Base URL"</span><code>{str_field(&settings, "base_url")}</code></div>
                        <div class="spp-kv">
                            <span>"Chat model"</span>
                            {opt_str(str_field(&settings, "chat_model"), "auto (first loaded)")}
                        </div>
                        <div class="spp-kv">
                            <span>"Embedding model"</span>
                            {opt_str(str_field(&settings, "embedding_model"), "not configured")}
                        </div>
                        <div class="spp-kv">
                            <span>"Reachable"</span>
                            {if connected {
                                view! {
                                    <span class="spp-badge spp-badge--ok">
                                        {format!("connected · {} models", models.len())}
                                    </span>
                                }.into_view()
                            } else {
                                view! { <span class="spp-badge spp-badge--err">"offline"</span> }.into_view()
                            }}
                        </div>
                        {if !lmstudio_error.is_empty() {
                            view! {
                                <div class="spp-state spp-state--warning spp-mt-2">
                                    <p class="spp-state__body">{lmstudio_error}</p>
                                </div>
                            }.into_view()
                        } else {
                            ().into_view()
                        }}
                        {if !models.is_empty() {
                            view! {
                                <div class="spp-ai-center__models spp-mt-2">
                                    {models.iter().take(8).map(|m| {
                                        view! { <span class="spp-badge">{m.clone()}</span> }
                                    }).collect::<Vec<_>>()}
                                </div>
                            }.into_view()
                        } else {
                            ().into_view()
                        }}
                        <div class="spp-mt-4">
                            <A href="/settings" class="spp-button spp-button--small">"Configure in Settings →"</A>
                        </div>
                    </div>
                    <div class="spp-card">
                        <h3 class="spp-card__title">"Indexing + jobs"</h3>
                        <div class="spp-kv">
                            <span>"AI enabled"</span>
                            {if ai_enabled {
                                view! { <span class="spp-badge spp-badge--ok">"enabled"</span> }.into_view()
                            } else {
                                view! { <span class="spp-badge">"disabled"</span> }.into_view()
                            }}
                        </div>
                        <div class="spp-kv">
                            <span>"Last inference"</span>
                            {if last_inference.is_null() {
                                "—".to_string().into_view()
                            } else {
                                format!(
                                    "{} · {}ms",
                                    str_field(&last_inference, "at"),
                                    last_inference.get("latencyMs").and_then(|v| v.as_i64()).unwrap_or(0)
                                ).into_view()
                            }}
                        </div>
                        <div class="spp-kv"><span>"Queued AI jobs"</span>{queued.to_string()}</div>
                        <div class="spp-kv">
                            <span>"Failed AI jobs"</span>
                            {if failed > 0 {
                                view! { <span class="spp-badge spp-badge--err">{failed.to_string()}</span> }.into_view()
                            } else {
                                failed.to_string().into_view()
                            }}
                        </div>
                        <div class="spp-kv">
                            <span>"Indexed knowledge chunks"</span>
                            {format!(
                                "{} (pending {}, failed {})",
                                index.get("chunks_indexed").and_then(|v| v.as_i64()).unwrap_or(0),
                                index.get("chunks_pending").and_then(|v| v.as_i64()).unwrap_or(0),
                                index.get("chunks_failed").and_then(|v| v.as_i64()).unwrap_or(0),
                            )}
                        </div>
                        <div class="spp-kv">
                            <span>"Thread embeddings"</span>
                            {index.get("conversations_indexed").and_then(|v| v.as_i64()).unwrap_or(0).to_string()}
                        </div>
                    </div>
                </div>
                <div class="spp-card spp-mt-4">
                    <h3 class="spp-card__title">"Pipeline safety"</h3>
                    <ul class="spp-ai-center__safety">
                        <li>"AI never sends customer replies automatically — drafts require explicit human review (verified-answer mode)."</li>
                        <li>"Verification checks: unanswered questions, unsupported claims, invented timeframes, internal leakage."</li>
                        <li>"Read tools only (search_conversations, search_knowledge, …) with server-side permission validation — no SQL access."</li>
                        <li>"Redaction layer masks payment data, tokens, API keys before prompting."</li>
                        <li>"Every AI result records model, prompt version, latency and sources for auditability."</li>
                    </ul>
                </div>
            }.into_view()
        }}
    }
}

#[component]
fn AnalyticsTab() -> impl IntoView {
    let analytics = create_rw_signal(Option::<serde_json::Value>::None);
    let error_msg = create_rw_signal(None::<String>);

    create_effect(move |_| {
        let analytics = analytics;
        let error_msg = error_msg;
        wasm_bindgen_futures::spawn_local(async move {
            match crate::api::get_json::<serde_json::Value>("/api/ai/analytics").await {
                Ok(a) => {
                    analytics.set(Some(a));
                    error_msg.set(None);
                }
                Err(e) => error_msg.set(Some(e)),
            }
        });
    });

    view! {
        <Show when=move || error_msg.get().is_some() fallback=|| ()>
            <div class="spp-state spp-state--error">
                <span class="spp-state__icon" aria-hidden="true">"⚠"</span>
                <p class="spp-state__body">{move || error_msg.get().unwrap_or_default()}</p>
            </div>
        </Show>
        <Show when=move || analytics.get().is_none() && error_msg.get().is_none() fallback=|| ()>
            <LoadingState />
        </Show>
        {move || {
            let Some(a) = analytics.get() else { return ().into_view(); };
            let patterns: Vec<serde_json::Value> = a
                .get("common_failure_patterns")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            view! {
                <div class="spp-grid-2">
                    <div class="spp-card">
                        <h3 class="spp-card__title">"Assistance metrics (local, from stored data)"</h3>
                        <div class="spp-kv"><span>"Tickets analyzed"</span>{num_str(&a, "tickets_analyzed")}</div>
                        <div class="spp-kv"><span>"Analysis success rate"</span>{pct(&a, "analysis_success_rate")}</div>
                        <div class="spp-kv"><span>"Drafts generated"</span>{num_str(&a, "draft_count")}</div>
                        <div class="spp-kv"><span>"Drafts accepted"</span>{num_str(&a, "draft_accepted")}</div>
                        <div class="spp-kv"><span>"Drafts rejected"</span>{num_str(&a, "draft_rejected")}</div>
                        <div class="spp-kv"><span>"Draft edit rate"</span>{pct(&a, "draft_edit_rate")}</div>
                        <div class="spp-kv"><span>"Verification warnings"</span>{num_str(&a, "verification_warnings")}</div>
                        <div class="spp-kv"><span>"Unsupported-claim rate"</span>{pct(&a, "unsupported_claim_rate")}</div>
                    </div>
                    <div class="spp-card">
                        <h3 class="spp-card__title">"Common AI failure patterns"</h3>
                        {if !patterns.is_empty() {
                            view! {
                                <div>
                                    {patterns
                                        .iter()
                                        .map(|p| {
                                            let pattern = str_field(p, "pattern");
                                            let count = p.get("count").and_then(|v| v.as_i64()).unwrap_or(0);
                                            view! {
                                                <div class="spp-flex-between spp-ai-center__pattern-row">
                                                    <span class="spp-text-sm">{pattern}</span>
                                                    <span class="spp-badge spp-badge--err">{count.to_string()}</span>
                                                </div>
                                            }
                                        })
                                        .collect::<Vec<_>>()}
                                </div>
                            }.into_view()
                        } else {
                            view! {
                                <EmptyState message="No failure patterns recorded. Warnings from draft verification accumulate here." />
                            }.into_view()
                        }}
                        <p class="spp-text-xs spp-text-muted spp-mt-2">
                            "No claims are made about AI performance beyond what is stored locally."
                        </p>
                    </div>
                </div>
            }.into_view()
        }}
    }
}

#[component]
fn AttributesTab() -> impl IntoView {
    let report = create_rw_signal(Vec::<serde_json::Value>::new());
    let catalog = create_rw_signal(Vec::<serde_json::Value>::new());
    let loading = create_rw_signal(true);
    let error_msg = create_rw_signal(None::<String>);
    let attribute = create_rw_signal(String::new());
    let op = create_rw_signal("equals".to_string());
    let value = create_rw_signal(String::new());
    let drill = create_rw_signal(Vec::<serde_json::Value>::new());
    let drill_loaded = create_rw_signal(false);
    let reload = create_rw_signal(0u32);

    create_effect(move |_| {
        let _ = reload.get();
        let report = report;
        let catalog = catalog;
        let loading = loading;
        let error_msg = error_msg;
        wasm_bindgen_futures::spawn_local(async move {
            let report_result =
                crate::api::get_json::<serde_json::Value>("/api/attributes/report").await;
            let catalog_result =
                crate::api::get_json::<serde_json::Value>("/api/attributes/catalog").await;
            match (report_result, catalog_result) {
                (Ok(r), Ok(c)) => {
                    report.set(
                        r.get("distributions")
                            .and_then(|v| v.as_array())
                            .cloned()
                            .unwrap_or_default(),
                    );
                    catalog.set(
                        c.get("catalog")
                            .and_then(|v| v.as_array())
                            .cloned()
                            .unwrap_or_default(),
                    );
                    loading.set(false);
                }
                (Err(e), _) | (_, Err(e)) => {
                    error_msg.set(Some(e));
                    loading.set(false);
                }
            }
        });
    });

    let effective_op = move || {
        if value.get().trim().is_empty() {
            "unknown".to_string()
        } else {
            op.get()
        }
    };

    create_effect(move |_| {
        let attr = attribute.get();
        let effective = effective_op();
        let val = value.get();
        let drill = drill;
        let drill_loaded = drill_loaded;
        if attr.is_empty() {
            drill.set(Vec::new());
            drill_loaded.set(false);
            return;
        }
        wasm_bindgen_futures::spawn_local(async move {
            let path = format!(
                "/api/attributes/conversations?attribute={}&op={}&value={}&limit=25",
                urlencode(&attr),
                urlencode(&effective),
                urlencode(&val),
            );
            match crate::api::get_json::<serde_json::Value>(&path).await {
                Ok(data) => drill.set(
                    data.get("conversations")
                        .and_then(|v| v.as_array())
                        .cloned()
                        .unwrap_or_default(),
                ),
                Err(_) => drill.set(Vec::new()),
            }
            drill_loaded.set(true);
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
        <Show when=move || !loading.get() && error_msg.get().is_none() fallback=|| ()>
            <div class="spp-card">
                <div class="spp-flex-between">
                    <div>
                        <h3 class="spp-card__title">"Tags AI attribute layer — coverage report"</h3>
                        <p class="spp-text-xs spp-text-muted">
                            "Versioned local attributes (deterministic + AI layers). A missing value is honest 'unknown' — never fabricated. Nothing is written to Help Scout."
                        </p>
                    </div>
                    <button
                        class="spp-button spp-button--small"
                        on:click=move |_| reload.update(|n| *n = n.wrapping_add(1))
                    >
                        "⟳ Refresh"
                    </button>
                </div>
                <Show
                    when=move || !report.get().is_empty()
                    fallback=|| {
                        view! {
                            <EmptyState message="No attributes stored yet. Attributes are computed after the first AI analysis or recompute (deterministic layer needs no AI)." />
                        }
                    }
                >
                    {move || report.get().iter().map(|d| {
                        let total = d.get("total_conversations").and_then(|v| v.as_i64()).unwrap_or(0);
                        let known = d.get("known").and_then(|v| v.as_i64()).unwrap_or(0);
                        let unknown = d.get("unknown").and_then(|v| v.as_i64()).unwrap_or(0);
                        let known_pct = if total > 0 {
                            ((known as f64) / (total as f64) * 100.0).round() as i64
                        } else {
                            0
                        };
                        let label = str_field(d, "label");
                        let value_type = str_field(d, "value_type");
                        let attr_key = str_field(d, "attribute");
                        let values: Vec<serde_json::Value> = d
                            .get("values")
                            .and_then(|v| v.as_array())
                            .cloned()
                            .unwrap_or_default();
                        view! {
                            <div class="spp-ai-center__attr spp-mt-2">
                                <div class="spp-flex-between">
                                    <span class="spp-text-sm">
                                        <strong>{label}</strong>
                                        <span class="spp-text-xs spp-text-muted">{format!(" {value_type}")}</span>
                                    </span>
                                    <span class="spp-text-xs spp-text-muted">
                                        {format!("{known} known · {unknown} unknown · {known_pct}% coverage")}
                                    </span>
                                </div>
                                <div class="spp-attr-bar" title=format!("known {known} / unknown {unknown}")>
                                    <div class="spp-attr-bar-fill" style=format!("width: {known_pct}%;")></div>
                                </div>
                                {if !values.is_empty() {
                                    view! {
                                        <div class="spp-ai-center__chips">
                                            {values.iter().take(8).map(|v| {
                                                let v_str = str_field(v, "value");
                                                let count = v.get("count").and_then(|v| v.as_i64()).unwrap_or(0);
                                                let attr_for_chip = attr_key.clone();
                                                let chip_label = format!("{v_str} · {count}");
                                                let value_for_chip = v_str.clone();
                                                view! {
                                                    <button
                                                        class="spp-chip"
                                                        title=format!("{count} conversation(s)")
                                                        on:click=move |_| {
                                                            attribute.set(attr_for_chip.clone());
                                                            op.set("equals".into());
                                                            value.set(value_for_chip.clone());
                                                        }
                                                    >
                                                        {chip_label}
                                                    </button>
                                                }
                                            }).collect::<Vec<_>>()}
                                        </div>
                                    }.into_view()
                                } else {
                                    ().into_view()
                                }}
                            </div>
                        }
                    }).collect::<Vec<_>>()}
                </Show>
            </div>

            <div class="spp-card spp-mt-4">
                <h3 class="spp-card__title">"Search by attribute"</h3>
                <div class="spp-ai-center__search-row">
                    <select
                        class="spp-input"
                        aria-label="Attribute"
                        prop:value=attribute
                        on:change=move |ev| attribute.set(event_target_value(&ev))
                    >
                        <option value="">"Choose an attribute…"</option>
                        {move || catalog.get().iter().map(|c| {
                            let key = str_field(c, "key");
                            let label = str_field(c, "label");
                            view! { <option value=key.clone()>{label}</option> }
                        }).collect::<Vec<_>>()}
                    </select>
                    <select
                        class="spp-input"
                        aria-label="Operator"
                        prop:value=op
                        on:change=move |ev| op.set(event_target_value(&ev))
                    >
                        {["equals", "not_equals", "contains", "gt", "gte", "lt", "lte", "unknown"].iter().map(|o| {
                            view! {
                                <option value=*o>{if *o == "unknown" { "is unknown" } else { *o }}</option>
                            }
                        }).collect::<Vec<_>>()}
                    </select>
                    <input
                        class="spp-input"
                        type="text"
                        maxlength=120
                        placeholder="value (blank = is unknown)"
                        prop:value=value
                        on:input=move |ev| value.set(event_target_value(&ev))
                    />
                </div>
                <Show
                    when=move || !attribute.get().is_empty()
                    fallback=|| {
                        view! {
                            <p class="spp-text-xs spp-text-muted">
                                "Pick an attribute to see matching conversations. Values come from the local layer only."
                            </p>
                        }
                    }
                >
                    <Show
                        when=move || !drill.get().is_empty()
                        fallback=|| {
                            view! {
                                <EmptyState message="No conversations match. Unknown or missing values only match the 'is unknown' / 'equals unknown' operator." />
                            }
                        }
                    >
                        <table class="spp-table">
                            <thead>
                                <tr>
                                    <th>"#"</th>
                                    <th>"Subject"</th>
                                    <th>"Value"</th>
                                    <th>"Confidence"</th>
                                    <th>"Source"</th>
                                    <th>"Computed"</th>
                                </tr>
                            </thead>
                            <tbody>
                                {move || drill.get().iter().map(|c| {
                                    let conversation_id = c.get("conversation_id").and_then(|v| v.as_i64()).unwrap_or(0);
                                    let number = c.get("number").and_then(|v| v.as_i64()).unwrap_or(0);
                                    let subject = str_field(c, "subject");
                                    let val = str_field(c, "value");
                                    let confidence = str_field(c, "confidence");
                                    let confidence_class =
                                        format!("spp-badge {}", confidence_badge(&confidence));
                                    let source = str_field(c, "source");
                                    let computed_at = str_field(c, "computed_at");
                                    view! {
                                        <tr>
                                            <td>
                                                <A href=format!("/inbox/conversation/{conversation_id}") class="spp-table__link">
                                                    {format!("#{number}")}
                                                </A>
                                            </td>
                                            <td class="spp-text-sm">{if subject.is_empty() { "—".to_string() } else { subject }}</td>
                                            <td><span class="spp-badge">{val}</span></td>
                                            <td>
                                                <span class=confidence_class>{confidence}</span>
                                            </td>
                                            <td class="spp-text-xs">{source}</td>
                                            <td class="spp-text-xs">{computed_at}</td>
                                        </tr>
                                    }
                                }).collect::<Vec<_>>()}
                            </tbody>
                        </table>
                    </Show>
                </Show>
            </div>
        </Show>
    }
}

#[component]
fn CopilotTab() -> impl IntoView {
    let sessions = create_rw_signal(Vec::<serde_json::Value>::new());
    let loading = create_rw_signal(true);
    let reload = create_rw_signal(0u32);

    create_effect(move |_| {
        let _ = reload.get();
        let sessions = sessions;
        let loading = loading;
        wasm_bindgen_futures::spawn_local(async move {
            match crate::api::get_json::<serde_json::Value>("/api/copilot/sessions?limit=50").await
            {
                Ok(data) => {
                    sessions.set(
                        data.get("sessions")
                            .and_then(|v| v.as_array())
                            .cloned()
                            .unwrap_or_default(),
                    );
                }
                Err(_) => sessions.set(Vec::new()),
            }
            loading.set(false);
        });
    });

    let delete_session = move |id: i64| {
        wasm_bindgen_futures::spawn_local(async move {
            let path = format!("/api/copilot/sessions/{id}");
            match crate::api::delete_json::<serde_json::Value>(&path).await {
                Ok(r) if r.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) => {
                    crate::toasts::success("Copilot session deleted.");
                }
                Ok(r) => crate::toasts::error(
                    r.get("message")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Delete failed."),
                ),
                Err(e) => crate::toasts::error(e),
            }
            reload.update(|n| *n = n.wrapping_add(1));
        });
    };

    view! {
        <div class="spp-card">
            <h3 class="spp-card__title">"Local Copilot"</h3>
            <p class="spp-text-sm">
                "An interactive, read-only assistant inside every conversation (context pane → Copilot tab). It answers with local evidence — this ticket, the customer's history, similar cases, knowledge, known issues and the AI attribute layer — through an allowlisted read-only tool registry. It never writes to Help Scout and never sends anything to the customer; citations are generated by the server from the tools it actually executed."
            </p>
            <ul class="spp-ai-center__safety">
                <li>"Runs fully locally via LM Studio — no cloud LLM, ever."</li>
                <li>"The model never sees SQL: tools are parameter-validated reads."</li>
                <li>"Tool budget is bounded per turn; answers must cite real evidence."</li>
                <li>"If LM Studio is off, the Copilot says so instead of pretending."</li>
            </ul>
            <div class="spp-mt-4">
                <A href="/inbox" class="spp-button spp-button--small">"Open a conversation to use it →"</A>
            </div>
        </div>
        <div class="spp-card spp-mt-4">
            <div class="spp-flex-between">
                <h3 class="spp-card__title">"Copilot sessions"</h3>
                <button
                    class="spp-button spp-button--small"
                    on:click=move |_| reload.update(|n| *n = n.wrapping_add(1))
                >
                    "⟳ Refresh"
                </button>
            </div>
            <Show when=move || loading.get() fallback=|| ()>
                <LoadingState />
            </Show>
            <Show
                when=move || !loading.get() && !sessions.get().is_empty()
                fallback=|| {
                    view! {
                        <EmptyState message="No Copilot sessions yet. Ask a question from any conversation's Copilot tab." />
                    }
                }
            >
                <table class="spp-table">
                    <thead>
                        <tr>
                            <th>"Session"</th>
                            <th>"Conversation"</th>
                            <th>"Messages"</th>
                            <th>"Updated"</th>
                            <th></th>
                        </tr>
                    </thead>
                    <tbody>
                        {move || sessions.get().iter().map(|s| {
                            let row = s.clone();
                            let id = row.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
                            let title = str_field(&row, "title");
                            let conversation_id = row.get("conversation_id").and_then(|v| v.as_i64());
                            let conversation_number = row.get("conversation_number").and_then(|v| v.as_i64());
                            let message_count = row.get("message_count").and_then(|v| v.as_i64()).unwrap_or(0);
                            let updated_at = str_field(&row, "updated_at");
                            view! {
                                <tr>
                                    <td class="spp-text-sm">{title}</td>
                                    <td>
                                        {match (conversation_id, conversation_number) {
                                            (Some(cid), _) => {
                                                view! {
                                                    <A href=format!("/inbox/conversation/{cid}") class="spp-table__link">
                                                        {format!("#{}", conversation_number.unwrap_or(cid))}
                                                    </A>
                                                }.into_view()
                                            }
                                            (None, _) => {
                                                view! { <span class="spp-text-xs spp-text-muted">"global"</span> }.into_view()
                                            }
                                        }}
                                    </td>
                                    <td>{message_count.to_string()}</td>
                                    <td class="spp-text-xs">{updated_at}</td>
                                    <td>
                                        <button
                                            class="spp-button spp-button--ghost spp-button--small"
                                            title="Delete session"
                                            on:click=move |_| delete_session(id)
                                        >
                                            "✕"
                                        </button>
                                    </td>
                                </tr>
                            }
                        }).collect::<Vec<_>>()}
                    </tbody>
                </table>
            </Show>
        </div>
    }
}

#[component]
fn JobsTab() -> impl IntoView {
    let jobs = create_rw_signal(Vec::<serde_json::Value>::new());
    let loading = create_rw_signal(true);
    let error_msg = create_rw_signal(None::<String>);
    let reload = create_rw_signal(0u32);

    create_effect(move |_| {
        let _ = reload.get();
        let jobs = jobs;
        let loading = loading;
        let error_msg = error_msg;
        wasm_bindgen_futures::spawn_local(async move {
            match crate::api::get_json::<serde_json::Value>("/api/ai/jobs").await {
                Ok(data) => {
                    jobs.set(
                        data.get("jobs")
                            .and_then(|v| v.as_array())
                            .cloned()
                            .unwrap_or_default(),
                    );
                    error_msg.set(None);
                }
                Err(e) => error_msg.set(Some(e)),
            }
            loading.set(false);
        });
    });

    view! {
        <div class="spp-card">
            <div class="spp-flex-between">
                <h3 class="spp-card__title">"AI job history"</h3>
                <button
                    class="spp-button spp-button--small"
                    on:click=move |_| reload.update(|n| *n = n.wrapping_add(1))
                >
                    "⟳ Refresh"
                </button>
            </div>
            <Show when=move || error_msg.get().is_some() fallback=|| ()>
                <div class="spp-state spp-state--error">
                    <span class="spp-state__icon" aria-hidden="true">"⚠"</span>
                    <p class="spp-state__body">{move || error_msg.get().unwrap_or_default()}</p>
                </div>
            </Show>
            <Show when=move || loading.get() fallback=|| ()>
                <LoadingState />
            </Show>
            <Show
                when=move || !loading.get() && !jobs.get().is_empty()
                fallback=|| {
                    view! { <EmptyState message="No AI jobs yet." /> }
                }
            >
                <div class="spp-table-scroll">
                    <table class="spp-table">
                        <thead>
                            <tr>
                                <th>"ID"</th>
                                <th>"Type"</th>
                                <th>"Status"</th>
                                <th>"Conversation"</th>
                                <th>"Model"</th>
                                <th>"Latency"</th>
                                <th>"When"</th>
                            </tr>
                        </thead>
                        <tbody>
                            {move || jobs.get().iter().take(60).map(|j| {
                                let row = j.clone();
                                let id = row.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
                                let kind = str_field(&row, "type");
                                let status = str_field(&row, "status");
                                let status_class = format!("spp-badge {}", status_badge(&status));
                                let conversation_id = row.get("conversation_id").and_then(|v| v.as_i64());
                                let model = str_field(&row, "model");
                                let latency = row.get("latency_ms").and_then(|v| v.as_i64());
                                let created_at = str_field(&row, "created_at");
                                view! {
                                    <tr>
                                        <td class="spp-text-xs">{id.to_string()}</td>
                                        <td>{kind}</td>
                                        <td>
                                            <span class=status_class>{status}</span>
                                        </td>
                                        <td>
                                            {match conversation_id {
                                                Some(cid) => {
                                                    view! {
                                                        <A href=format!("/inbox/conversation/{cid}") class="spp-table__link">
                                                            {format!("#{cid}")}
                                                        </A>
                                                    }.into_view()
                                                }
                                                None => "—".to_string().into_view(),
                                            }}
                                        </td>
                                        <td class="spp-text-xs">{if model.is_empty() { "—".to_string() } else { model }}</td>
                                        <td class="spp-text-xs">
                                            {match latency {
                                                Some(ms) => format!("{ms}ms"),
                                                None => "—".to_string(),
                                            }}
                                        </td>
                                        <td class="spp-text-xs">{created_at}</td>
                                    </tr>
                                }
                            }).collect::<Vec<_>>()}
                        </tbody>
                    </table>
                </div>
            </Show>
        </div>
    }
}

#[component]
fn EvaluationTab() -> impl IntoView {
    let evaluation = create_rw_signal(Option::<serde_json::Value>::None);
    let error_msg = create_rw_signal(None::<String>);
    let toggling = create_rw_signal(false);
    let reload = create_rw_signal(0u32);
    let running = create_rw_signal(false);

    create_effect(move |_| {
        let _ = reload.get();
        let evaluation = evaluation;
        let error_msg = error_msg;
        wasm_bindgen_futures::spawn_local(async move {
            match crate::api::get_json::<serde_json::Value>("/api/ai/evaluation").await {
                Ok(e) => {
                    evaluation.set(Some(e));
                    error_msg.set(None);
                }
                Err(e) => error_msg.set(Some(e)),
            }
        });
    });

    let toggle_eval = move |on: bool| {
        if toggling.get() {
            return;
        }
        toggling.set(true);
        wasm_bindgen_futures::spawn_local(async move {
            match crate::api::patch_json::<serde_json::Value>(
                "/api/settings",
                &serde_json::json!({ "ai_evaluation_mode": on }),
            )
            .await
            {
                Ok(r) if r.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) => {
                    crate::toasts::success("AI evaluation mode updated.");
                    reload.update(|n| *n = n.wrapping_add(1));
                }
                Ok(r) => crate::toasts::error(
                    r.get("message")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Update failed."),
                ),
                Err(e) => crate::toasts::error(e),
            }
            toggling.set(false);
        });
    };

    let run_evaluation = move || {
        if running.get() {
            return;
        }
        running.set(true);
        let reload = reload;
        wasm_bindgen_futures::spawn_local(async move {
            match crate::api::post_json::<serde_json::Value>("/api/ai/evaluation/run", None).await {
                Ok(r) if r.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) => {
                    crate::toasts::success("Evaluation run recorded.");
                    reload.update(|n| *n = n.wrapping_add(1));
                }
                Ok(r) => crate::toasts::error(
                    r.get("message")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Evaluation run failed"),
                ),
                Err(e) => crate::toasts::error(e),
            }
            running.set(false);
        });
    };

    view! {
        <Show when=move || error_msg.get().is_some() fallback=|| ()>
            <div class="spp-state spp-state--error">
                <span class="spp-state__icon" aria-hidden="true">"⚠"</span>
                <p class="spp-state__body">{move || error_msg.get().unwrap_or_default()}</p>
            </div>
        </Show>
        {move || {
            let Some(e) = evaluation.get() else { return ().into_view(); };
            let tests: Vec<serde_json::Value> = e
                .get("tests")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            view! {
                <div class="spp-card">
                    <h3 class="spp-card__title">"⚗ Offline AI evaluation mode"</h3>
                    <p class="spp-text-sm">
                        "When enabled: no Help Scout writes, no notes, no replies, no status changes, no assignments — only local evaluation. Compare analysis outputs safely."
                    </p>
                    <label class="spp-ai-center__eval-toggle">
                        <input
                            type="checkbox"
                            prop:checked=move || evaluation.get()
                                .and_then(|e| e.get("evaluation_mode")
                                .and_then(|v| v.as_bool()))
                                .unwrap_or(false)
                            on:change=move |ev| toggle_eval(event_target_checked(&ev))
                        />
                        <span>
                            {move || {
                                let on = evaluation.get()
                                    .and_then(|e| e.get("evaluation_mode")
                                    .and_then(|v| v.as_bool()))
                                    .unwrap_or(false);
                                if on { "AI evaluation mode ON" } else { "AI evaluation mode OFF" }
                            }}
                        </span>
                    </label>
                </div>
                <div class="spp-card spp-mt-4">
                    <h3 class="spp-card__title">"☑ Golden test set (spec scenarios)"</h3>
                    <table class="spp-table">
                        <thead>
                            <tr><th>"Scenario"</th><th>"Category"</th><th>"Sample ticket"</th></tr>
                        </thead>
                        <tbody>
                            {tests.iter().map(|t| {
                                let name = str_field(t, "name");
                                let category = str_field(t, "category");
                                let payload = t.get("payload").cloned().unwrap_or(serde_json::Value::Null);
                                let subject = str_field(&payload, "subject");
                                view! {
                                    <tr>
                                        <td>{name}</td>
                                        <td><span class="spp-badge">{category}</span></td>
                                        <td class="spp-text-xs">{subject}</td>
                                    </tr>
                                }
                            }).collect::<Vec<_>>()}
                        </tbody>
                    </table>
                    <p class="spp-text-xs spp-text-muted spp-mt-2">
                        "The golden set evaluates classification, retrieval, draft generation, verification, internal leakage, missing questions and unsupported claims. Runs execute the real pipeline against the configured backend; the AI-disabled backend reports the honest failure reason per test."
                    </p>
                    <div class="spp-modal__actions spp-mt-8">
                        <button
                            class="spp-button spp-button--primary"
                            type="button"
                            disabled=move || running.get()
                            on:click=move |_| run_evaluation()
                        >
                            {move || if running.get() { "Running…" } else { "Run evaluation" }}
                        </button>
                    </div>
                </div>

                {move || {
                    let last = e.get("last_run").cloned();
                    let Some(last) = last.filter(|l| !l.is_null()) else {
                        return ().into_view();
                    };
                    let results: Vec<serde_json::Value> = last
                        .get("results")
                        .and_then(|v| v.as_array())
                        .cloned()
                        .unwrap_or_default();
                    let passed = last.get("passed").and_then(|v| v.as_i64()).unwrap_or(0);
                    let failed = last.get("failed").and_then(|v| v.as_i64()).unwrap_or(0);
                    let backend = str_field(&last, "backend");
                    let finished = str_field(&last, "finished_at");
                    let has_results = !results.is_empty();
                    view! {
                        <div class="spp-card spp-mt-4">
                            <h3 class="spp-card__title">
                                {format!("Last run ({backend}): {passed} passed / {failed} failed")}
                            </h3>
                            <p class="spp-muted spp-text-xs">{format!("finished {finished}")}</p>
                            <Show
                                when=move || has_results
                                fallback=|| view! { <EmptyState message="No per-test outcomes recorded." /> }
                            >
                                <table class="spp-table">
                                    <thead>
                                        <tr>
                                            <th>"Test"</th>
                                            <th>"Category"</th>
                                            <th>"Outcome"</th>
                                            <th>"Stages"</th>
                                            <th>"Error"</th>
                                        </tr>
                                    </thead>
                                    <tbody>
                                        {results.iter().map(|r| {
                                            let name = str_field(r, "name");
                                            let category = str_field(r, "category");
                                            let ok = r.get("passed").and_then(|v| v.as_bool()).unwrap_or(false);
                                            let error = str_field(r, "error");
                                            let stages = r.get("stages").cloned().unwrap_or_default();
                                            let analysis_ok = stages.get("analysis").and_then(|s| s.get("ok")).and_then(|v| v.as_bool()).unwrap_or(false);
                                            let draft_ok = stages.get("draft").and_then(|s| s.get("ok")).and_then(|v| v.as_bool()).unwrap_or(false);
                                            let verification_ok = stages.get("verification").and_then(|s| s.get("ok")).and_then(|v| v.as_bool()).unwrap_or(false);
                                            view! {
                                                <tr>
                                                    <td>{name}</td>
                                                    <td><span class="spp-badge">{category}</span></td>
                                                    <td>
                                                        <span class=if ok {
                                                            "spp-badge spp-badge--ok"
                                                        } else {
                                                            "spp-badge spp-badge--warn"
                                                        }>
                                                            {if ok { "pass" } else { "fail" }}
                                                        </span>
                                                    </td>
                                                    <td class="spp-text-xs">
                                                        {format!("analysis {} · draft {} · verification {}", if analysis_ok { "ok" } else { "-" }, if draft_ok { "ok" } else { "-" }, if verification_ok { "ok" } else { "-" })}
                                                    </td>
                                                    <td class="spp-table__cell-muted spp-text-xs">{error}</td>
                                                </tr>
                                            }
                                        }).collect::<Vec<_>>()}
                                    </tbody>
                                </table>
                            </Show>
                        </div>
                    }.into_view()
                }}
            }.into_view()
        }}
    }
}

fn str_field(v: &serde_json::Value, key: &str) -> String {
    v.get(key)
        .and_then(|f| f.as_str())
        .unwrap_or_default()
        .to_string()
}

fn num_str(v: &serde_json::Value, key: &str) -> String {
    v.get(key).and_then(|v| v.as_i64()).unwrap_or(0).to_string()
}

fn pct(v: &serde_json::Value, key: &str) -> String {
    let n = v.get(key).and_then(|v| v.as_f64()).unwrap_or(0.0);
    format!("{n}%")
}

fn opt_str(value: String, fallback: &str) -> impl IntoView {
    if value.is_empty() {
        fallback.to_string().into_view()
    } else {
        value.into_view()
    }
}

fn confidence_badge(confidence: &str) -> &'static str {
    match confidence {
        "high" => "spp-badge--ok",
        "medium" => "spp-badge--warn",
        _ => "",
    }
}

fn status_badge(status: &str) -> &'static str {
    match status {
        "completed" => "spp-badge--ok",
        "failed" => "spp-badge--err",
        "running" => "spp-badge--warn",
        _ => "",
    }
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
