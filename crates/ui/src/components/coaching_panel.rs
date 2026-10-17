use leptos::prelude::*;
use leptos::task::*;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CoachingCheck {
    pub kind: String,
    pub layer: String,
    pub label: String,
    pub status: String,
    pub detail: String,
    pub findings: Vec<CoachingFinding>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CoachingFinding {
    pub draft_excerpt: Option<String>,
    pub evidence: Vec<CoachingEvidence>,
    pub advice: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CoachingEvidence {
    pub description: String,
    pub excerpt: String,
    pub incident_code: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CoachingReview {
    pub flagged: i64,
    pub checks_run: i64,
    pub draft_words: i64,
    pub note: String,
    pub checks: Vec<CoachingCheck>,
    pub ai_available: bool,
    pub ai_model: Option<String>,
    pub ai_error: Option<String>,
}

#[must_use]
pub fn parse_coaching_review(v: &serde_json::Value) -> CoachingReview {
    let summary = v.get("summary").cloned().unwrap_or_default();
    let ai = v.get("ai").cloned().unwrap_or_default();
    CoachingReview {
        flagged: summary.get("flagged").and_then(|x| x.as_i64()).unwrap_or(0),
        checks_run: summary
            .get("checks_run")
            .and_then(|x| x.as_i64())
            .unwrap_or(0),
        draft_words: v.get("draft_words").and_then(|x| x.as_i64()).unwrap_or(0),
        note: v
            .get("note")
            .and_then(|x| x.as_str())
            .unwrap_or_default()
            .to_string(),
        checks: v
            .get("checks")
            .and_then(|c| c.as_array())
            .map(|rows| rows.iter().map(parse_coaching_check).collect())
            .unwrap_or_default(),
        ai_available: ai
            .get("available")
            .and_then(|x| x.as_bool())
            .unwrap_or(false),
        ai_model: ai.get("model").and_then(|x| x.as_str()).map(str::to_string),
        ai_error: ai.get("error").and_then(|x| x.as_str()).map(str::to_string),
    }
}

fn parse_coaching_check(v: &serde_json::Value) -> CoachingCheck {
    CoachingCheck {
        kind: v
            .get("kind")
            .and_then(|x| x.as_str())
            .unwrap_or_default()
            .to_string(),
        layer: v
            .get("layer")
            .and_then(|x| x.as_str())
            .unwrap_or_default()
            .to_string(),
        label: v
            .get("label")
            .and_then(|x| x.as_str())
            .unwrap_or_default()
            .to_string(),
        status: v
            .get("status")
            .and_then(|x| x.as_str())
            .unwrap_or_default()
            .to_string(),
        detail: v
            .get("detail")
            .and_then(|x| x.as_str())
            .unwrap_or_default()
            .to_string(),
        findings: v
            .get("findings")
            .and_then(|f| f.as_array())
            .map(|rows| {
                rows.iter()
                    .map(|f| CoachingFinding {
                        draft_excerpt: f
                            .get("draft_excerpt")
                            .and_then(|x| x.as_str())
                            .map(str::to_string),
                        evidence: f
                            .get("evidence")
                            .and_then(|e| e.as_array())
                            .map(|rows| {
                                rows.iter()
                                    .map(|e| CoachingEvidence {
                                        description: e
                                            .get("description")
                                            .and_then(|x| x.as_str())
                                            .unwrap_or_default()
                                            .to_string(),
                                        excerpt: e
                                            .get("excerpt")
                                            .and_then(|x| x.as_str())
                                            .unwrap_or_default()
                                            .to_string(),
                                        incident_code: e
                                            .get("incident_code")
                                            .and_then(|x| x.as_str())
                                            .map(str::to_string),
                                    })
                                    .collect()
                            })
                            .unwrap_or_default(),
                        advice: f
                            .get("advice")
                            .and_then(|x| x.as_str())
                            .unwrap_or_default()
                            .to_string(),
                    })
                    .collect()
            })
            .unwrap_or_default(),
    }
}

#[must_use]
pub fn status_class(status: &str) -> &'static str {
    match status {
        "pass" => "spp-badge--ok",
        "flagged" => "spp-badge--err",
        "unavailable" => "spp-badge--warn",
        _ => "",
    }
}

#[component]
pub fn CoachingPanel(conversation_id: i64, #[prop(into)] draft: RwSignal<String>) -> impl IntoView {
    let open = RwSignal::new(false);
    let include_ai = RwSignal::new(false);
    let review = RwSignal::new(None::<CoachingReview>);
    let checking = RwSignal::new(false);

    let run_check = move || {
        let text = draft.get_untracked();
        if text.trim().is_empty() || checking.get_untracked() {
            return;
        }
        open.set(true);
        checking.set(true);
        let body = serde_json::json!({
            "draft": text,
            "includeAi": if include_ai.get_untracked() { Some(true) } else { None::<bool> },
        });
        let review = review;
        let checking = checking;
        spawn_local(async move {
            match crate::api::post_json::<serde_json::Value>(
                &format!("/api/coaching/{conversation_id}/review"),
                Some(&body),
            )
            .await
            {
                Ok(v) => review.set(Some(parse_coaching_review(&v))),
                Err(_) => review.set(None),
            }
            checking.set(false);
        });
    };

    view! {
        <section class="spp-coaching-panel">
            <div class="spp-coaching-panel__bar">
                <button
                    class=move || {
                        let warn = review.get().is_some_and(|r| r.flagged > 0);
                        format!(
                            "spp-button spp-button--small{}",
                            if warn { " spp-button--warn" } else { "" }
                        )
                    }
                    on:click=move |_| run_check()
                    disabled=move || checking.get()
                    title="Optional, advisory-only review of your draft before sending"
                >
                    {move || if checking.get() { "Checking…".to_string() } else { "Check draft (coaching)".to_string() }}
                </button>
                <label class="spp-coaching-panel__ai-toggle">
                    <input
                        type="checkbox"
                        prop:checked=move || include_ai.get()
                        on:change=move |ev| include_ai.set(event_target_checked(&ev))
                    />
                    " Include AI layer (LM Studio)"
                </label>
                <Show when=move || review.get().is_some() fallback=|| ()>
                    <span class="spp-muted spp-text-xs">
                        {move || {
                            match review.get() {
                                Some(r) => format!(
                                    "{} flagged · {} checks · {} words",
                                    r.flagged, r.checks_run, r.draft_words
                                ),
                                None => String::new(),
                            }
                        }}
                    </span>
                </Show>
                <Show when=move || review.get().is_some() && open.get() fallback=|| ()>
                    <button
                        class="spp-button spp-button--ghost spp-button--small"
                        on:click=move |_| open.set(false)
                    >
                        "Hide"
                    </button>
                </Show>
            </div>

            <Show when=move || review.get().is_some() && open.get() fallback=|| ()>
                {move || {
                    let r = review.get().unwrap_or_default();
                    view! {
                        <div class="spp-coaching-panel__results">
                            <p class="spp-muted spp-text-xs">{r.note.clone()}</p>
                            {r.checks
                                .iter()
                                .map(|c| {
                                    let badge = if c.status == "not_applicable" {
                                        "n/a".to_string()
                                    } else {
                                        c.status.clone()
                                    };
                                    let layer_note = if c.layer == "ai" { " · AI" } else { "" };
                                    view! {
                                        <div class=format!("spp-coaching-check spp-coaching-check--{}", c.status)>
                                            <div class="spp-coaching-check__head">
                                                <strong>{c.label.clone()}</strong>
                                                <span class=format!("spp-badge {}", status_class(&c.status))>
                                                    {format!("{badge}{layer_note}")}
                                                </span>
                                            </div>
                                            <div class="spp-muted spp-text-xs">{c.detail.clone()}</div>
                                            {c.findings
                                                .iter()
                                                .map(|f| {
                                                    let show_excerpt = f
                                                        .draft_excerpt
                                                        .as_deref()
                                                        .is_some_and(|x| {
                                                            !x.is_empty()
                                                                && x != "(the draft does not appear to address this)"
                                                                && x != "(no sorry / understand / appreciate / patience wording found)"
                                                        });
                                                    view! {
                                                        <div class="spp-coaching-finding">
                                                            {if show_excerpt {
                                                                let excerpt = f.draft_excerpt.clone().unwrap_or_default();
                                                                let truncated: String = excerpt.chars().take(200).collect();
                                                                view! {
                                                                    <div class="spp-text-xs">
                                                                        <span class="spp-muted">"Draft: "</span>
                                                                        "\u{201c}" {truncated} "\u{201d}"
                                                                    </div>
                                                                }.into_any()
                                                            } else {
                                                                ().into_any()
                                                            }}
                                                            {f.evidence
                                                                .iter()
                                                                .map(|e| {
                                                                    let mut text = format!("{}: {}", e.description, e.excerpt);
                                                                    if let Some(code) = &e.incident_code {
                                                                        text.push_str(&format!(" [{code}]"));
                                                                    }
                                                                    let truncated: String = text.chars().take(200).collect();
                                                                    view! {
                                                                        <div class="spp-coaching-finding__evidence">{truncated}</div>
                                                                    }
                                                                })
                                                                .collect::<Vec<_>>()}
                                                            <div class="spp-text-xs">
                                                                <span class="spp-muted">"Advice: "</span>
                                                                {f.advice.clone()}
                                                            </div>
                                                        </div>
                                                    }
                                                })
                                                .collect::<Vec<_>>()}
                                        </div>
                                    }
                                })
                                .collect::<Vec<_>>()}
                            {if r.ai_available && r.ai_model.is_some() {
                                view! {
                                    <div class="spp-muted spp-text-xs">
                                        {format!("AI model: {}", r.ai_model.clone().unwrap_or_default())}
                                    </div>
                                }.into_any()
                            } else {
                                ().into_any()
                            }}
                        </div>
                    }
                }}
            </Show>
        </section>
    }
}
