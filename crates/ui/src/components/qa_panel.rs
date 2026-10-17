use leptos::*;

use crate::toasts;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct QaDeterministic {
    pub closed: bool,
    pub back_and_forth_count: i64,
    pub repeated_information_count: i64,
    pub messages_after_close: i64,
    pub handoff_count: i64,
    pub handoff_history_complete: bool,
    pub customer_question_count: i64,
    pub agent_reply_count: i64,
    pub first_response_minutes: Option<i64>,
    pub resolution_minutes: Option<i64>,
    pub repeated_information_evidence: Vec<RepeatedSpan>,
    pub computed_honestly: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct RepeatedSpan {
    pub thread_id: i64,
    pub excerpt: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct QaAi {
    pub answered: Option<QaJudgment>,
    pub evidence_supported: Option<QaJudgment>,
    pub correct_issue: Option<QaJudgment>,
    pub suggestions_kb_improve: Option<bool>,
    pub suggestions_kb_reason: Option<String>,
    pub suggestions_saved_reply: Option<bool>,
    pub suggestions_saved_reply_title: Option<String>,
    pub suggestions_issue_association: Option<String>,
    pub model: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct QaJudgment {
    pub value: String,
    pub reasoning: String,
    pub evidence_thread_ids: Vec<i64>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct QaPayload {
    pub deterministic: Option<QaDeterministic>,
    pub ai: Option<QaAi>,
    pub ai_available: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct FrictionFinding {
    pub kind: String,
    pub severity: String,
    pub detail: String,
    pub evidence: Vec<FrictionEvidence>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct FrictionEvidence {
    pub author_type: String,
    pub excerpt: String,
}

fn parse_deterministic(v: &serde_json::Value) -> QaDeterministic {
    QaDeterministic {
        closed: v.get("closed").and_then(|x| x.as_bool()).unwrap_or(false),
        back_and_forth_count: v
            .get("back_and_forth_count")
            .and_then(|x| x.as_i64())
            .unwrap_or(0),
        repeated_information_count: v
            .get("repeated_information_count")
            .and_then(|x| x.as_i64())
            .unwrap_or(0),
        messages_after_close: v
            .get("messages_after_close")
            .and_then(|x| x.as_i64())
            .unwrap_or(0),
        handoff_count: v.get("handoff_count").and_then(|x| x.as_i64()).unwrap_or(0),
        handoff_history_complete: v
            .get("handoff_history_complete")
            .and_then(|x| x.as_bool())
            .unwrap_or(false),
        customer_question_count: v
            .get("customer_question_count")
            .and_then(|x| x.as_i64())
            .unwrap_or(0),
        agent_reply_count: v
            .get("agent_reply_count")
            .and_then(|x| x.as_i64())
            .unwrap_or(0),
        first_response_minutes: v.get("first_response_minutes").and_then(|x| x.as_i64()),
        resolution_minutes: v.get("resolution_minutes").and_then(|x| x.as_i64()),
        repeated_information_evidence: v
            .get("repeated_information_evidence")
            .and_then(|r| r.as_array())
            .map(|rows| {
                rows.iter()
                    .map(|r| RepeatedSpan {
                        thread_id: r.get("thread_id").and_then(|x| x.as_i64()).unwrap_or(0),
                        excerpt: r
                            .get("excerpt")
                            .and_then(|x| x.as_str())
                            .unwrap_or_default()
                            .to_string(),
                    })
                    .collect()
            })
            .unwrap_or_default(),
        computed_honestly: v
            .get("computed_honestly")
            .and_then(|c| c.as_array())
            .map(|rows| {
                rows.iter()
                    .filter_map(|c| c.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default(),
    }
}

fn parse_judgment(v: &serde_json::Value) -> QaJudgment {
    QaJudgment {
        value: v
            .get("value")
            .and_then(|x| x.as_str())
            .unwrap_or("unknown")
            .to_string(),
        reasoning: v
            .get("reasoning")
            .and_then(|x| x.as_str())
            .unwrap_or_default()
            .to_string(),
        evidence_thread_ids: v
            .get("evidence_thread_ids")
            .and_then(|e| e.as_array())
            .map(|rows| rows.iter().filter_map(|e| e.as_i64()).collect())
            .unwrap_or_default(),
    }
}

fn parse_ai(v: &serde_json::Value) -> QaAi {
    let suggestions = v.get("suggestions").cloned().unwrap_or_default();
    QaAi {
        answered: v
            .get("answered")
            .filter(|j| !j.is_null())
            .map(parse_judgment),
        evidence_supported: v
            .get("evidence_supported")
            .filter(|j| !j.is_null())
            .map(parse_judgment),
        correct_issue: v
            .get("correct_issue")
            .filter(|j| !j.is_null())
            .map(parse_judgment),
        suggestions_kb_improve: suggestions.get("kb_improve").and_then(|x| x.as_bool()),
        suggestions_kb_reason: suggestions
            .get("kb_reason")
            .and_then(|x| x.as_str())
            .map(str::to_string),
        suggestions_saved_reply: suggestions
            .get("saved_reply_suggested")
            .and_then(|x| x.as_bool()),
        suggestions_saved_reply_title: suggestions
            .get("saved_reply_title")
            .and_then(|x| x.as_str())
            .map(str::to_string),
        suggestions_issue_association: suggestions
            .get("issue_association")
            .and_then(|x| x.as_str())
            .map(str::to_string),
        model: v.get("model").and_then(|x| x.as_str()).map(str::to_string),
    }
}

#[must_use]
pub fn parse_qa_response(v: &serde_json::Value) -> (Option<QaPayload>, Vec<FrictionFinding>) {
    let qa = v.get("qa").filter(|q| !q.is_null()).map(|q| QaPayload {
        deterministic: q
            .get("deterministic")
            .filter(|d| !d.is_null())
            .map(parse_deterministic),
        ai: q.get("ai").filter(|a| !a.is_null()).map(parse_ai),
        ai_available: q
            .get("ai_available")
            .and_then(|x| x.as_bool())
            .unwrap_or(false),
    });
    let friction = v
        .get("friction")
        .and_then(|f| f.as_array())
        .map(|rows| {
            rows.iter()
                .map(|f| FrictionFinding {
                    kind: f
                        .get("kind")
                        .and_then(|x| x.as_str())
                        .unwrap_or_default()
                        .to_string(),
                    severity: f
                        .get("severity")
                        .and_then(|x| x.as_str())
                        .unwrap_or_default()
                        .to_string(),
                    detail: f
                        .get("detail")
                        .and_then(|x| x.as_str())
                        .unwrap_or_default()
                        .to_string(),
                    evidence: f
                        .get("evidence")
                        .and_then(|e| e.as_array())
                        .map(|rows| {
                            rows.iter()
                                .map(|e| FrictionEvidence {
                                    author_type: e
                                        .get("author_type")
                                        .and_then(|x| x.as_str())
                                        .unwrap_or_default()
                                        .to_string(),
                                    excerpt: e
                                        .get("excerpt")
                                        .and_then(|x| x.as_str())
                                        .unwrap_or_default()
                                        .to_string(),
                                })
                                .collect()
                        })
                        .unwrap_or_default(),
                })
                .collect()
        })
        .unwrap_or_default();
    (qa, friction)
}

#[must_use]
pub fn severity_class(severity: &str) -> &'static str {
    match severity {
        "high" => "spp-badge--err",
        "moderate" => "spp-badge--warn",
        _ => "",
    }
}

#[component]
pub fn QaPanel(conversation_id: i64, #[prop(default = false)] closed: bool) -> impl IntoView {
    let open = create_rw_signal(false);
    let include_ai = create_rw_signal(false);
    let qa = create_rw_signal(None::<QaPayload>);
    let friction = create_rw_signal(Vec::<FrictionFinding>::new());
    let loading = create_rw_signal(false);
    let analyzing = create_rw_signal(false);

    let load = move || {
        loading.set(true);
        let qa = qa;
        let friction = friction;
        let loading = loading;
        spawn_local(async move {
            match crate::api::get_json::<serde_json::Value>(&format!("/api/qa/{conversation_id}"))
                .await
            {
                Ok(v) => {
                    let (q, f) = parse_qa_response(&v);
                    qa.set(q);
                    friction.set(f);
                }
                Err(_) => {
                    qa.set(None);
                    friction.set(Vec::new());
                }
            }
            loading.set(false);
        });
    };

    create_effect(move |_| {
        if open.get() && qa.get_untracked().is_none() && !loading.get_untracked() {
            load();
        }
    });

    let analyze = move |_| {
        if analyzing.get_untracked() {
            return;
        }
        analyzing.set(true);
        let body = if include_ai.get_untracked() {
            serde_json::json!({ "includeAi": true })
        } else {
            serde_json::json!({})
        };
        let analyzing = analyzing;
        let load = load;
        spawn_local(async move {
            match crate::api::post_json::<serde_json::Value>(
                &format!("/api/qa/{conversation_id}/analyze"),
                Some(&body),
            )
            .await
            {
                Ok(r) => {
                    if let Some(note) = r.get("error").and_then(|v| v.as_str()) {
                        toasts::info(note);
                    } else {
                        toasts::success("QA recomputed.");
                    }
                }
                Err(e) => toasts::error(e),
            }
            analyzing.set(false);
            load();
        });
    };

    view! {
        <section class="spp-card spp-qa-panel">
            <button
                class="spp-card__title spp-card__title--collapsible"
                on:click=move |_| open.update(|o| *o = !*o)
            >
                {move || {
                    format!(
                        "Post-resolution QA {} {}",
                        if closed { "" } else { "(conversation still open)" },
                        if open.get() { "▾" } else { "▸" }
                    )
                }}
            </button>
            <Show when=move || open.get() fallback=|| ()>
                <p class="spp-muted spp-text-xs">
                    "After-close quality signals, separate from pre-send draft verification. The deterministic layer is always computable; the AI layer is optional (local model only)."
                </p>
                <div class="spp-qa-panel__actions">
                    <button
                        class="spp-button spp-button--small"
                        on:click=analyze
                        disabled=move || analyzing.get()
                    >
                        {move || if analyzing.get() { "Analyzing…".to_string() } else { "Recompute (deterministic)".to_string() }}
                    </button>
                    <label class="spp-qa-panel__ai-toggle">
                        <input
                            type="checkbox"
                            prop:checked=move || include_ai.get()
                            on:change=move |ev| include_ai.set(event_target_checked(&ev))
                        />
                        " Include AI layer (LM Studio)"
                    </label>
                </div>

                <Show when=move || loading.get() fallback=|| ()>
                    <div class="spp-muted spp-text-xs">"Loading…"</div>
                </Show>

                <Show when=move || qa.get().is_some_and(|q| q.deterministic.is_some()) fallback=|| ()>
                    {move || {
                        let q = qa.get().unwrap_or_default();
                        let det = q.deterministic.clone().unwrap_or_default();
                        let ai = q.ai.clone();
                        view! {
                            <div class="spp-qa-panel__grid">
                                <div>
                                    <h4 class="spp-card__title">"Deterministic signals"</h4>
                                    <dl class="spp-kv-list">
                                        <div class="spp-kv-row"><dt>"Closed"</dt><dd>{if det.closed { "yes" } else { "no" }}</dd></div>
                                        <div class="spp-kv-row"><dt>"Back-and-forth after first reply"</dt><dd>{det.back_and_forth_count.to_string()}</dd></div>
                                        <div class="spp-kv-row"><dt>"Repeated information spans"</dt><dd>{det.repeated_information_count.to_string()}</dd></div>
                                        <div class="spp-kv-row"><dt>"Customer messages after close"</dt><dd>{det.messages_after_close.to_string()}</dd></div>
                                        <div class="spp-kv-row">
                                            <dt>"Handoffs observed"</dt>
                                            <dd>{format!(
                                                "{}{}",
                                                det.handoff_count,
                                                if det.handoff_history_complete { "" } else { " (pre-sync unknown)" }
                                            )}</dd>
                                        </div>
                                        <div class="spp-kv-row">
                                            <dt>"Customer questions vs agent replies"</dt>
                                            <dd>{format!("{} / {}", det.customer_question_count, det.agent_reply_count)}</dd>
                                        </div>
                                        <div class="spp-kv-row">
                                            <dt>"First response"</dt>
                                            <dd>{match det.first_response_minutes {
                                                Some(m) => format!("{m} min"),
                                                None => "—".to_string(),
                                            }}</dd>
                                        </div>
                                        <div class="spp-kv-row">
                                            <dt>"Resolution"</dt>
                                            <dd>{match det.resolution_minutes {
                                                Some(m) => format!("{m} min"),
                                                None => "—".to_string(),
                                            }}</dd>
                                        </div>
                                    </dl>
                                    {if !det.repeated_information_evidence.is_empty() {
                                        view! {
                                            <div class="spp-qa-panel__evidence">
                                                <div class="spp-muted spp-text-xs">"Repeated spans:"</div>
                                                {det.repeated_information_evidence
                                                    .iter()
                                                    .map(|e| {
                                                        view! {
                                                            <div class="spp-mono spp-text-xs">
                                                                {format!("#{}: {}", e.thread_id, e.excerpt)}
                                                            </div>
                                                        }
                                                    })
                                                    .collect::<Vec<_>>()}
                                            </div>
                                        }.into_view()
                                    } else {
                                        ().into_view()
                                    }}
                                </div>
                                <div>
                                    <h4 class="spp-card__title">
                                        {format!("AI layer {}", if q.ai_available { "" } else { "(AI disabled)" })}
                                    </h4>
                                    {match ai {
                                        Some(ai) => view! {
                                            <div class="spp-qa-panel__ai">
                                                {if let Some(j) = ai.answered {
                                                    view! {
                                                        <div>
                                                            <strong>"Question answered: "</strong>
                                                            <span class=format!("spp-badge {}", judgment_class(&j.value))>{j.value.clone()}</span>
                                                            <div class="spp-muted spp-text-xs">{j.reasoning.clone()}</div>
                                                            {if !j.evidence_thread_ids.is_empty() {
                                                                view! {
                                                                    <div class="spp-muted spp-text-xs">
                                                                        {format!("Evidence threads: {}", j.evidence_thread_ids.iter().map(std::string::ToString::to_string).collect::<Vec<_>>().join(", "))}
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
                                                {if let Some(j) = ai.evidence_supported {
                                                    view! {
                                                        <div>
                                                            <strong>"Response supported by evidence: "</strong>
                                                            <span class=format!("spp-badge {}", judgment_class(&j.value))>{j.value.clone()}</span>
                                                            <div class="spp-muted spp-text-xs">{j.reasoning.clone()}</div>
                                                        </div>
                                                    }.into_view()
                                                } else {
                                                    ().into_view()
                                                }}
                                                {if let Some(j) = ai.correct_issue {
                                                    view! {
                                                        <div>
                                                            <strong>"Correct issue identified: "</strong>
                                                            <span class=format!("spp-badge {}", judgment_class(&j.value))>{j.value.clone()}</span>
                                                            <div class="spp-muted spp-text-xs">{j.reasoning.clone()}</div>
                                                        </div>
                                                    }.into_view()
                                                } else {
                                                    ().into_view()
                                                }}
                                                {if let (Some(kb), reason) = (ai.suggestions_kb_improve, ai.suggestions_kb_reason.clone()) {
                                                    view! {
                                                        <div class="spp-alert spp-alert--info spp-text-xs">
                                                            <div>{format!(
                                                                "Knowledge base improvement suggested: {}",
                                                                if kb {
                                                                    format!("yes - {}", reason.unwrap_or_default())
                                                                } else {
                                                                    "no".to_string()
                                                                }
                                                            )}</div>
                                                            <div>{format!(
                                                                "Saved reply suggested: {}",
                                                                if ai.suggestions_saved_reply.unwrap_or(false) {
                                                                    format!("yes ({})", ai.suggestions_saved_reply_title.clone().unwrap_or_else(|| "untitled".to_string()))
                                                                } else {
                                                                    "no".to_string()
                                                                }
                                                            )}</div>
                                                            {if let Some(assoc) = ai.suggestions_issue_association.clone() {
                                                                view! { <div>{format!("Issue association: {assoc}")}</div> }.into_view()
                                                            } else {
                                                                ().into_view()
                                                            }}
                                                            <div class="spp-muted">"Suggestions are recommendations for humans — nothing applies automatically."</div>
                                                        </div>
                                                    }.into_view()
                                                } else {
                                                    ().into_view()
                                                }}
                                                <div class="spp-muted spp-text-xs">
                                                    {format!("Model: {}", ai.model.clone().unwrap_or_else(|| "unknown".to_string()))}
                                                </div>
                                            </div>
                                        }.into_view(),
                                        None => view! {
                                            <div class="spp-state spp-state--empty">
                                                <p class="spp-state__title">"AI layer not computed"</p>
                                                <p class="spp-state__hint">{
                                                    if q.ai_available {
                                                        "Check \"Include AI layer\" and press recompute to run the local model analysis."
                                                    } else {
                                                        "AI is disabled in Settings; the deterministic layer above works without it."
                                                    }
                                                }</p>
                                            </div>
                                        }.into_view(),
                                    }}
                                </div>
                            </div>
                        }
                    }}
                </Show>

                {move || {
                    let findings = friction.get();
                    if !findings.is_empty() {
                        view! {
                            <div class="spp-qa-panel__friction">
                                <h4 class="spp-card__title">{format!("Friction findings ({})", findings.len())}</h4>
                                {findings
                                    .iter()
                                    .map(|f| {
                                        view! {
                                            <div class="spp-friction-finding">
                                                <div class="spp-friction-finding__head">
                                                    <strong>{f.kind.replace('_', " ")}</strong>
                                                    <span class=format!("spp-badge {}", severity_class(&f.severity))>{f.severity.clone()}</span>
                                                </div>
                                                <div class="spp-muted spp-text-xs">{f.detail.clone()}</div>
                                                {f.evidence
                                                    .iter()
                                                    .take(3)
                                                    .map(|e| {
                                                        let text: String = e.excerpt.chars().take(120).collect();
                                                        view! {
                                                            <div class="spp-mono spp-text-xs">
                                                                {format!("[{}] {}", e.author_type, text)}
                                                            </div>
                                                        }
                                                    })
                                                    .collect::<Vec<_>>()}
                                            </div>
                                        }
                                    })
                                    .collect::<Vec<_>>()}
                            </div>
                        }.into_view()
                    } else {
                        view! { <div class="spp-muted spp-text-xs">"No friction findings detected for this conversation."</div> }.into_view()
                    }
                }}
            </Show>
        </section>
    }
}

fn judgment_class(value: &str) -> &'static str {
    match value {
        "yes" => "spp-badge--ok",
        "no" => "spp-badge--err",
        _ => "spp-badge--warn",
    }
}
