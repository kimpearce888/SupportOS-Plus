use leptos::*;

use crate::components::state_view::{EmptyState, LoadingState};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct GapCandidate {
    pub id: i64,
    pub kind: Option<String>,
    pub question: String,
    pub status: String,
    pub explanation: String,
    pub method: String,
    pub evidence_conversation_ids: Vec<i64>,
    pub decision_note: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct GapKind {
    pub kind: String,
    pub label: String,
    pub candidates: Vec<GapCandidate>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct GapReport {
    pub kinds: Vec<GapKind>,
    pub totals_candidates: i64,
    pub totals_approved: i64,
    pub totals_rejected: i64,
    pub notes: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct GapDraft {
    pub candidate_id: i64,
    pub suggested_title: String,
    pub suggested_outline: Vec<String>,
    pub evidence_conversations: Vec<(i64, i64)>,
    pub note: String,
}

#[must_use]
pub fn parse_gap_report(v: &serde_json::Value) -> GapReport {
    GapReport {
        kinds: v
            .get("kinds")
            .and_then(|k| k.as_array())
            .map(|rows| {
                rows.iter()
                    .map(|k| GapKind {
                        kind: k
                            .get("kind")
                            .and_then(|x| x.as_str())
                            .unwrap_or_default()
                            .to_string(),
                        label: k
                            .get("label")
                            .and_then(|x| x.as_str())
                            .unwrap_or_default()
                            .to_string(),
                        candidates: k
                            .get("candidates")
                            .and_then(|c| c.as_array())
                            .map(|rows| rows.iter().map(parse_candidate).collect())
                            .unwrap_or_default(),
                    })
                    .collect()
            })
            .unwrap_or_default(),
        totals_candidates: v
            .pointer("/totals/candidates")
            .and_then(|x| x.as_i64())
            .unwrap_or(0),
        totals_approved: v
            .pointer("/totals/approved")
            .and_then(|x| x.as_i64())
            .unwrap_or(0),
        totals_rejected: v
            .pointer("/totals/rejected")
            .and_then(|x| x.as_i64())
            .unwrap_or(0),
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

fn parse_candidate(c: &serde_json::Value) -> GapCandidate {
    GapCandidate {
        id: c.get("id").and_then(|x| x.as_i64()).unwrap_or(0),
        kind: c.get("kind").and_then(|x| x.as_str()).map(str::to_string),
        question: c
            .get("question")
            .and_then(|x| x.as_str())
            .unwrap_or_default()
            .to_string(),
        status: c
            .get("status")
            .and_then(|x| x.as_str())
            .unwrap_or_default()
            .to_string(),
        explanation: c
            .pointer("/detail/explanation")
            .and_then(|x| x.as_str())
            .unwrap_or_default()
            .to_string(),
        method: c
            .pointer("/detail/method")
            .and_then(|x| x.as_str())
            .unwrap_or_default()
            .to_string(),
        evidence_conversation_ids: c
            .get("evidence_conversation_ids")
            .and_then(|e| e.as_array())
            .map(|rows| rows.iter().filter_map(|x| x.as_i64()).collect())
            .unwrap_or_default(),
        decision_note: c
            .get("decision_note")
            .and_then(|x| x.as_str())
            .map(str::to_string),
    }
}

#[must_use]
pub fn parse_gap_draft(v: &serde_json::Value) -> GapDraft {
    GapDraft {
        candidate_id: v.get("candidate_id").and_then(|x| x.as_i64()).unwrap_or(0),
        suggested_title: v
            .get("suggested_title")
            .and_then(|x| x.as_str())
            .unwrap_or_default()
            .to_string(),
        suggested_outline: v
            .get("suggested_outline")
            .and_then(|o| o.as_array())
            .map(|rows| {
                rows.iter()
                    .filter_map(|x| x.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default(),
        evidence_conversations: v
            .get("evidence_conversations")
            .and_then(|e| e.as_array())
            .map(|rows| {
                rows.iter()
                    .map(|e| {
                        (
                            e.get("conversation_local_id")
                                .and_then(|x| x.as_i64())
                                .unwrap_or(0),
                            e.get("number").and_then(|x| x.as_i64()).unwrap_or(0),
                        )
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
pub fn is_undecided(status: &str) -> bool {
    status == "candidate" || status == "open"
}

#[must_use]
pub fn gap_status_badge_class(status: &str) -> &'static str {
    match status {
        "approved" => "spp-badge spp-badge--ok",
        "rejected" => "spp-badge spp-badge--err",
        _ => "spp-badge spp-badge--warn",
    }
}

#[component]
pub fn GapsTab() -> impl IntoView {
    let report = create_rw_signal(None::<GapReport>);
    let loading = create_rw_signal(true);
    let error_msg = create_rw_signal(None::<String>);
    let notice = create_rw_signal(None::<(String, String)>);
    let rebuilding = create_rw_signal(false);
    let deciding_id = create_rw_signal(None::<i64>);
    let draft = create_rw_signal(None::<GapDraft>);

    let load = {
        move || {
            loading.set(true);
            let report = report;
            let loading = loading;
            let error_msg = error_msg;
            spawn_local(async move {
                match crate::api::get_json::<serde_json::Value>("/api/knowledge/gaps").await {
                    Ok(v) => {
                        error_msg.set(None);
                        report.set(Some(parse_gap_report(&v)));
                    }
                    Err(e) => error_msg.set(Some(e)),
                }
                loading.set(false);
            });
        }
    };
    load();

    let rebuild = move |_| {
        if rebuilding.get_untracked() {
            return;
        }
        rebuilding.set(true);
        let body = serde_json::json!({ "days": 90 });
        let rebuilding = rebuilding;
        let load = load;
        let notice = notice;
        spawn_local(async move {
            match crate::api::post_json::<serde_json::Value>(
                "/api/knowledge/gaps/rebuild",
                Some(&body),
            )
            .await
            {
                Ok(r) => {
                    let candidates = r.get("candidates").and_then(|x| x.as_i64()).unwrap_or(0);
                    let new = r.get("new").and_then(|x| x.as_i64()).unwrap_or(0);
                    notice.set(Some((
                        "ok".to_string(),
                        format!(
                            "Gap rebuild complete: {candidates} candidates ({new} new). Human decisions were preserved."
                        ),
                    )));
                    load();
                }
                Err(e) => notice.set(Some(("err".to_string(), e))),
            }
            rebuilding.set(false);
        });
    };

    let decide = move |id: i64, decision: &'static str| {
        if deciding_id.get_untracked().is_some() {
            return;
        }
        deciding_id.set(Some(id));
        let body = serde_json::json!({ "decision": decision });
        let deciding_id = deciding_id;
        let load = load;
        let notice = notice;
        spawn_local(async move {
            let path = format!("/api/knowledge/gaps/candidates/{id}/decide");
            match crate::api::post_json::<serde_json::Value>(&path, Some(&body)).await {
                Ok(r) => {
                    let status = r
                        .pointer("/candidate/status")
                        .and_then(|x| x.as_str())
                        .unwrap_or("decided")
                        .to_string();
                    notice.set(Some((
                        "ok".to_string(),
                        format!("Candidate {status}. Nothing is published automatically."),
                    )));
                    load();
                }
                Err(e) => notice.set(Some(("err".to_string(), e))),
            }
            deciding_id.set(None);
        });
    };

    let toggle_draft = move |id: i64| {
        if draft.get_untracked().is_some_and(|d| d.candidate_id == id) {
            draft.set(None);
            return;
        }
        let draft = draft;
        spawn_local(async move {
            let path = format!("/api/knowledge/gaps/candidates/{id}/draft");
            match crate::api::get_json::<serde_json::Value>(&path).await {
                Ok(v) => draft.set(Some(parse_gap_draft(&v))),
                Err(e) => notice.set(Some(("err".to_string(), e))),
            }
        });
    };

    view! {
        <div class="spp-flex spp-flex--col spp-gap-12">
            <Show when=move || loading.get() fallback=|| ()>
                <LoadingState />
            </Show>
            <Show when=move || error_msg.get().is_some() fallback=|| ()>
                <div class="spp-state spp-state--error">
                    <p class="spp-state__body">"The gap report failed to load."</p>
                    <p class="spp-state__detail">{move || error_msg.get().unwrap_or_default()}</p>
                </div>
            </Show>
            {move || {
                let notice = notice.get();
                if let Some((kind, message)) = notice {
                    let class = if kind == "ok" {
                        "spp-alert spp-alert--ok"
                    } else {
                        "spp-alert spp-alert--error"
                    };
                    view! {
                        <div class=class>
                            <div class="spp-text-sm">{message}</div>
                        </div>
                    }.into_view()
                } else {
                    ().into_view()
                }
            }}
            {move || {
                let r = report.clone().get().unwrap_or_default();
                if loading.get() {
                    return ().into_view();
                }
                view! {
                    <div class="spp-card">
                        <div class="spp-flex spp-flex--between">
                            <div>
                                <h3 class="spp-card__title">"Knowledge gap engine"</h3>
                                <p class="spp-muted spp-text-sm">
                                    {format!(
                                        "{} open candidates · {} approved · {} rejected",
                                        r.totals_candidates, r.totals_approved, r.totals_rejected
                                    )}
                                </p>
                            </div>
                            <button
                                class="spp-button spp-button--small"
                                on:click=rebuild
                                disabled=move || rebuilding.get()
                            >
                                {move || if rebuilding.get() { "Rebuilding…" } else { "Rebuild detections" }.to_string()}
                            </button>
                        </div>
                        <div class="spp-alert spp-alert--info spp-gap-tab__notes">
                            {r.notes
                                .iter()
                                .map(|n| view! { <div class="spp-text-sm">{n.clone()}</div> })
                                .collect::<Vec<_>>()}
                        </div>
                    </div>
                    {r.kinds
                        .iter()
                        .map(|k| {
                            let kind_title = if k.label.is_empty() { k.kind.clone() } else { k.label.clone() };
                            view! {
                                <div class="spp-card">
                                    <h3 class="spp-card__title">{kind_title}</h3>
                                    {if k.candidates.is_empty() {
                                        view! {
                                            <EmptyState message="No candidates of this kind. This detection found nothing in the current data." />
                                        }.into_view()
                                    } else {
                                        k.candidates
                                            .iter()
                                            .map(|c| {
                                                let id = c.id;
                                                let question_short: String = c
                                                    .question
                                                    .chars()
                                                    .take(120)
                                                    .collect::<String>()
                                                    + if c.question.chars().count() > 120 { "…" } else { "" };
                                                let status_badge = gap_status_badge_class(&c.status);
                                                let status = c.status.clone();
                                                let undecided = is_undecided(&c.status);
                                                let evidence_ids = c.evidence_conversation_ids.iter().take(6).cloned().collect::<Vec<_>>();
                                                let explanation = c.explanation.clone();
                                                let method = c.method.clone();
                                                let decision_note = c.decision_note.clone();
                                                let d = draft.clone().get();
                                                let active_draft = d.filter(|d| d.candidate_id == id);
                                                view! {
                                                    <div class=format!("spp-gap-candidate spp-gap-candidate--{}", c.status)>
                                                        <div class="spp-flex spp-flex--between spp-gap-8 spp-flex--wrap">
                                                            <div class="spp-grow">
                                                                <div class="spp-text-sm">
                                                                    <strong>{question_short.clone()}</strong>
                                                                    <span class=status_badge>{status.clone()}</span>
                                                                </div>
                                                                <div class="spp-muted spp-text-xs spp-mt-4">{explanation.clone()}</div>
                                                                <div class="spp-muted spp-text-xs">{format!("Method: {method}")}</div>
                                                                {if !evidence_ids.is_empty() {
                                                                    view! {
                                                                        <div class="spp-flex spp-gap-4 spp-flex--wrap spp-mt-4">
                                                                            <span class="spp-muted spp-text-xs">"Evidence:"</span>
                                                                            {evidence_ids
                                                                                .iter()
                                                                                .map(|id| view! {
                                                                                    <a class="spp-button spp-button--tiny spp-button--ghost" href=format!("/inbox/conversation/{id}")>
                                                                                        {format!("#{id}")}
                                                                                    </a>
                                                                                })
                                                                                .collect::<Vec<_>>()}
                                                                        </div>
                                                                    }.into_view()
                                                                } else {
                                                                    ().into_view()
                                                                }}
                                                                {if let Some(note) = decision_note {
                                                                    view! {
                                                                        <div class="spp-text-xs spp-mt-4">{format!("Decision note: {note}")}</div>
                                                                    }.into_view()
                                                                } else {
                                                                    ().into_view()
                                                                }}
                                                                {if let Some(d) = active_draft {
                                                                    view! {
                                                                        <div class="spp-gap-draft spp-mt-8">
                                                                            <div><strong>"Suggested title: "</strong>{d.suggested_title.clone()}</div>
                                                                            <ol class="spp-text-sm spp-gap-draft__outline">
                                                                                {d.suggested_outline
                                                                                    .iter()
                                                                                    .map(|o| view! { <li>{o.clone()}</li> })
                                                                                    .collect::<Vec<_>>()}
                                                                            </ol>
                                                                            {if !d.evidence_conversations.is_empty() {
                                                                                view! {
                                                                                    <div class="spp-flex spp-gap-4 spp-flex--wrap spp-mt-4">
                                                                                        <span class="spp-muted spp-text-xs">"Read first:"</span>
                                                                                        {d.evidence_conversations
                                                                                            .iter()
                                                                                            .map(|(cid, number)| view! {
                                                                                                <a class="spp-button spp-button--tiny spp-button--ghost" href=format!("/inbox/conversation/{cid}")>
                                                                                                    {format!("#{number}")}
                                                                                                </a>
                                                                                            })
                                                                                            .collect::<Vec<_>>()}
                                                                                    </div>
                                                                                }.into_view()
                                                                            } else {
                                                                                ().into_view()
                                                                            }}
                                                                            <div class="spp-muted spp-text-xs spp-mt-4">{d.note.clone()}</div>
                                                                        </div>
                                                                    }.into_view()
                                                                } else {
                                                                    ().into_view()
                                                                }}
                                                            </div>
                                                            <div class="spp-flex spp-flex--col spp-gap-4">
                                                                {if undecided {
                                                                                    view! {
                                                                                        <button
                                                                                            class="spp-button spp-button--small spp-button--primary"
                                                                                            on:click=move |_| decide(id, "approved")
                                                                                            disabled=move || deciding_id.get().is_some()
                                                                                        >
                                                                                            "Approve"
                                                                                        </button>
                                                                                        <button
                                                                                            class="spp-button spp-button--small"
                                                                                            on:click=move |_| decide(id, "rejected")
                                                                                            disabled=move || deciding_id.get().is_some()
                                                                                        >
                                                                                            "Reject"
                                                                                        </button>
                                                                                    }.into_view()
                                                                } else {
                                                                    ().into_view()
                                                                }}
                                                                <button
                                                                    class="spp-button spp-button--small spp-button--ghost"
                                                                    on:click=move |_| toggle_draft(id)
                                                                >
                                                                    {if draft.get().is_some_and(|d| d.candidate_id == id) {
                                                                        "Hide draft".to_string()
                                                                    } else {
                                                                        "Draft outline".to_string()
                                                                    }}
                                                                </button>
                                                            </div>
                                                        </div>
                                                    </div>
                                                }
                                            })
                                            .collect::<Vec<_>>()
                                            .into_view()
                                    }}
                                </div>
                            }.into_view()
                        })
                        .collect::<Vec<_>>()}
                }.into_view()
            }}
        </div>
    }
}
