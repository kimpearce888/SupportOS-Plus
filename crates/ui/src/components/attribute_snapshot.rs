use leptos::*;

use crate::toasts;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct AiAttributeRow {
    pub attribute: String,
    pub value: String,
    pub confidence: String,
    pub source: String,
    pub evidence: Vec<AttributeEvidence>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct AttributeEvidence {
    pub excerpt: String,
    pub thread_local_id: Option<i64>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct AttributeSnapshot {
    pub attributes: Vec<AiAttributeRow>,
    pub unknown: Vec<String>,
    pub computed_at: Option<String>,
}

#[must_use]
pub fn parse_attribute_snapshot(v: &serde_json::Value) -> AttributeSnapshot {
    AttributeSnapshot {
        attributes: v
            .get("attributes")
            .and_then(|a| a.as_array())
            .map(|rows| {
                rows.iter()
                    .map(|a| AiAttributeRow {
                        attribute: a
                            .get("attribute")
                            .and_then(|x| x.as_str())
                            .unwrap_or_default()
                            .to_string(),
                        value: a
                            .get("value")
                            .and_then(|x| x.as_str())
                            .unwrap_or_default()
                            .to_string(),
                        confidence: a
                            .get("confidence")
                            .and_then(|x| x.as_str())
                            .unwrap_or("medium")
                            .to_string(),
                        source: a
                            .get("source")
                            .and_then(|x| x.as_str())
                            .unwrap_or("deterministic")
                            .to_string(),
                        evidence: a
                            .get("evidence")
                            .and_then(|e| e.as_array())
                            .map(|rows| {
                                rows.iter()
                                    .map(|e| AttributeEvidence {
                                        excerpt: e
                                            .get("excerpt")
                                            .and_then(|x| x.as_str())
                                            .unwrap_or_default()
                                            .to_string(),
                                        thread_local_id: e
                                            .get("thread_local_id")
                                            .and_then(|x| x.as_i64()),
                                    })
                                    .collect()
                            })
                            .unwrap_or_default(),
                    })
                    .collect()
            })
            .unwrap_or_default(),
        unknown: v
            .get("unknown")
            .and_then(|u| u.as_array())
            .map(|rows| {
                rows.iter()
                    .filter_map(|u| u.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default(),
        computed_at: v
            .get("computed_at")
            .and_then(|x| x.as_str())
            .map(str::to_string),
    }
}

#[must_use]
pub fn confidence_badge_class(confidence: &str) -> &'static str {
    match confidence {
        "high" => "spp-badge--ok",
        "medium" => "spp-badge--warn",
        _ => "",
    }
}

#[component]
pub fn AttributeSnapshotCard(conversation_id: i64) -> impl IntoView {
    let snapshot = create_rw_signal(None::<AttributeSnapshot>);
    let loading = create_rw_signal(true);
    let error_msg = create_rw_signal(None::<String>);
    let recomputing = create_rw_signal(false);
    let show_unknown = create_rw_signal(false);
    let expanded_attr = create_rw_signal(None::<String>);

    let load = {
        move || {
            loading.set(true);
            let snapshot = snapshot;
            let loading = loading;
            let error_msg = error_msg;
            spawn_local(async move {
                match crate::api::get_json::<serde_json::Value>(&format!(
                    "/api/attributes/conversation/{conversation_id}"
                ))
                .await
                {
                    Ok(v) => {
                        error_msg.set(None);
                        snapshot.set(Some(parse_attribute_snapshot(&v)));
                    }
                    Err(e) => error_msg.set(Some(e)),
                }
                loading.set(false);
            });
        }
    };
    load();

    let recompute = move |_| {
        if recomputing.get_untracked() {
            return;
        }
        recomputing.set(true);
        let body = serde_json::json!({ "force": true });
        let recomputing = recomputing;
        let load = load;
        spawn_local(async move {
            match crate::api::post_json::<serde_json::Value>(
                &format!("/api/attributes/conversation/{conversation_id}/recompute"),
                Some(&body),
            )
            .await
            {
                Ok(r) => {
                    let ok = r.get("ok").and_then(|v| v.as_bool()).unwrap_or(false);
                    if ok {
                        toasts::success("Attribute snapshot recomputed.");
                        load();
                    } else {
                        toasts::error("Recompute failed.");
                    }
                }
                Err(e) => toasts::error(e),
            }
            recomputing.set(false);
        });
    };

    view! {
        <div class="spp-ai-sidebar-section">
            <div class="spp-ai-sidebar-section__head">
                <h4>"AI attributes"</h4>
                <button
                    class="spp-button spp-button--ghost spp-button--small"
                    on:click=recompute
                    disabled=move || recomputing.get()
                    title="Recompute now (deterministic always; AI when enabled)"
                >
                    {move || if recomputing.get() { "Working…" } else { "Recompute" }.to_string()}
                </button>
            </div>
            <Show when=move || loading.get() fallback=|| ()>
                <div class="spp-muted spp-text-xs">"Loading attributes…"</div>
            </Show>
            <Show when=move || error_msg.get().is_some() fallback=|| ()>
                <div class="spp-state spp-state--error">
                    <p class="spp-state__body">"Could not load AI attributes."</p>
                    <p class="spp-state__detail">{move || error_msg.get().unwrap_or_default()}</p>
                </div>
            </Show>
            {move || {
                let s = snapshot.clone().get().unwrap_or_default();
                if s.attributes.is_empty() && !loading.get() {
                    view! {
                        <div class="spp-state spp-state--empty">
                            <p class="spp-state__title">"No attributes yet"</p>
                            <p class="spp-state__hint">"Recompute to build the deterministic layer now; AI slots fill when LM Studio analyzes the ticket."</p>
                        </div>
                    }.into_view()
                } else {
                    view! {
                        <div class="spp-attribute-grid">
                            {s.attributes
                                .iter()
                                .map(|a| {
                                    let key = a.attribute.clone();
                                    let is_expanded = expanded_attr.get() == Some(key.clone());
                                    let source_badge = if a.source == "ai" { "AI" } else { "det." };
                                    let source_title = if a.source == "ai" {
                                        "Extracted by the local model (LM Studio), evidence-backed"
                                    } else {
                                        "Computed from observable local facts — no AI"
                                    };
                                    view! {
                                        <div class="spp-attribute-row" title=format!("{} ({})", a.attribute, a.source)>
                                            <span class="spp-muted spp-text-xs spp-attribute-key">{a.attribute.clone()}</span>
                                            <span class="spp-text-xs spp-attribute-value">{a.value.clone()}</span>
                                            <span class=format!("spp-badge {}", confidence_badge_class(&a.confidence))>
                                                {a.confidence.clone()}
                                            </span>
                                            <span class="spp-badge" title=source_title>{source_badge}</span>
                                            {if !a.evidence.is_empty() {
                                                let key_for_toggle = key.clone();
                                                view! {
                                                    <button
                                                        class="spp-button spp-button--ghost spp-button--small"
                                                        title="Show evidence"
                                                        on:click=move |_| {
                                                            expanded_attr.set(if is_expanded { None } else { Some(key_for_toggle.clone()) });
                                                        }
                                                    >
                                                        {if is_expanded { "▾" } else { "▸" }.to_string()}
                                                    </button>
                                                }.into_view()
                                            } else {
                                                ().into_view()
                                            }}
                                            {if is_expanded && !a.evidence.is_empty() {
                                                view! {
                                                    <div class="spp-attribute-evidence spp-text-xs">
                                                        {a.evidence
                                                            .iter()
                                                            .map(|e| {
                                                                let note = match e.thread_local_id {
                                                                    Some(t) => format!(" (thread #{t})"),
                                                                    None => String::new(),
                                                                };
                                                                view! {
                                                                    <blockquote>
                                                                        "\u{201c}" {e.excerpt.clone()} "\u{201d}"
                                                                        <span class="spp-muted">{note.clone()}</span>
                                                                    </blockquote>
                                                                }
                                                            })
                                                            .collect::<Vec<_>>()}
                                                    </div>
                                                }.into_view()
                                            } else {
                                                ().into_view()
                                            }}
                                        </div>
                                    }
                                })
                                .collect::<Vec<_>>()}
                        </div>
                        {if !s.unknown.is_empty() {
                            view! {
                                <div class="spp-attribute-unknown">
                                    <button
                                        class="spp-button spp-button--ghost spp-button--small"
                                        title="Keys with no stored value — honest unknown, never fabricated"
                                        on:click=move |_| show_unknown.update(|u| *u = !*u)
                                    >
                                        {format!("{} unknown", s.unknown.len())}
                                    </button>
                                    <Show when=move || show_unknown.get() fallback=|| ()>
                                        <div class="spp-muted spp-text-xs">
                                            {s.unknown
                                                .iter()
                                                .map(|k| {
                                                    view! {
                                                        <span class="spp-badge" title="No stored value (honest unknown)">
                                                            {format!("{k}: unknown")}
                                                        </span>
                                                    }
                                                })
                                                .collect::<Vec<_>>()}
                                        </div>
                                    </Show>
                                </div>
                            }.into_view()
                        } else {
                            ().into_view()
                        }}
                        {if let Some(at) = &s.computed_at {
                            view! {
                                <p class="spp-muted spp-text-xs">
                                    {format!("Local layer · computed {at} · never written to Help Scout")}
                                </p>
                            }.into_view()
                        } else {
                            ().into_view()
                        }}
                    }.into_view()
                }
            }}
        </div>
    }
}
