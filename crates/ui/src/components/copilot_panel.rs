use leptos::*;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CopilotMessage {
    pub id: i64,
    pub role: String,
    pub content: String,
    pub tool_calls: i64,
    pub citations: Vec<CopilotCitation>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CopilotCitation {
    pub index: i64,
    pub label: String,
    pub conversation_id: Option<i64>,
}

#[must_use]
pub fn parse_copilot_message(v: &serde_json::Value) -> CopilotMessage {
    CopilotMessage {
        id: v.get("id").and_then(|x| x.as_i64()).unwrap_or(0),
        role: v
            .get("role")
            .and_then(|x| x.as_str())
            .unwrap_or("assistant")
            .to_string(),
        content: v
            .get("content")
            .and_then(|x| x.as_str())
            .unwrap_or_default()
            .to_string(),
        tool_calls: v.get("tool_calls").and_then(|x| x.as_i64()).unwrap_or(0),
        citations: v
            .get("citations")
            .and_then(|c| c.as_array())
            .map(|rows| {
                rows.iter()
                    .map(|c| CopilotCitation {
                        index: c.get("index").and_then(|x| x.as_i64()).unwrap_or(0),
                        label: c
                            .get("label")
                            .and_then(|x| x.as_str())
                            .unwrap_or_default()
                            .to_string(),
                        conversation_id: c.get("conversation_id").and_then(|x| x.as_i64()),
                    })
                    .collect()
            })
            .unwrap_or_default(),
    }
}

#[component]
pub fn CopilotPanel(conversation_id: i64) -> impl IntoView {
    let session_id = create_rw_signal(None::<i64>);
    let messages = create_rw_signal(Vec::<CopilotMessage>::new());
    let question = create_rw_signal(String::new());
    let sending = create_rw_signal(false);
    let failure = create_rw_signal(None::<String>);
    let starters = create_rw_signal(Vec::<(String, String)>::new());

    let load_latest = move || {
        let session_id = session_id;
        let messages = messages;
        let cid = conversation_id;
        spawn_local(async move {
            let Ok(data) =
                crate::api::get_json::<serde_json::Value>("/api/copilot/sessions?limit=50").await
            else {
                return;
            };
            let mine: Vec<i64> = data
                .get("sessions")
                .and_then(|s| s.as_array())
                .map(|rows| {
                    rows.iter()
                        .filter(|s| s.get("conversation_id").and_then(|c| c.as_i64()) == Some(cid))
                        .filter_map(|s| s.get("id").and_then(|i| i.as_i64()))
                        .collect()
                })
                .unwrap_or_default();
            if session_id.get_untracked().is_none() {
                if let Some(latest) = mine.first() {
                    session_id.set(Some(*latest));
                    if let Ok(r) = crate::api::get_json::<serde_json::Value>(&format!(
                        "/api/copilot/sessions/{latest}"
                    ))
                    .await
                    {
                        let parsed = r
                            .get("messages")
                            .and_then(|m| m.as_array())
                            .map(|rows| rows.iter().map(parse_copilot_message).collect())
                            .unwrap_or_default();
                        messages.set(parsed);
                    }
                }
            }
        });
    };
    load_latest();

    spawn_local(async move {
        if let Ok(r) = crate::api::get_json::<serde_json::Value>(&format!(
            "/api/copilot/starter-questions/{conversation_id}"
        ))
        .await
        {
            let qs = r
                .get("questions")
                .and_then(|q| q.as_array())
                .map(|rows| {
                    rows.iter()
                        .filter_map(|q| {
                            let question = q.get("question")?.as_str()?.to_string();
                            let why = q
                                .get("why")
                                .and_then(|w| w.as_str())
                                .unwrap_or_default()
                                .to_string();
                            Some((question, why))
                        })
                        .take(8)
                        .collect()
                })
                .unwrap_or_default();
            starters.set(qs);
        }
    });

    let ask = move |text: String| {
        let text = text.trim().to_string();
        if text.is_empty() || sending.get_untracked() {
            return;
        }
        sending.set(true);
        failure.set(None);
        question.set(String::new());
        let session = session_id.get_untracked();
        let body = serde_json::json!({
            "question": text,
            "conversationId": conversation_id,
            "sessionId": session,
        });
        spawn_local(async move {
            match crate::api::post_json::<serde_json::Value>("/api/copilot/chat", Some(&body)).await
            {
                Ok(res) => {
                    if let Some(s) = res
                        .get("session")
                        .and_then(|s| s.get("id"))
                        .and_then(|i| i.as_i64())
                    {
                        session_id.set(Some(s));
                    }
                    let user_msg = res.get("user_message").map(parse_copilot_message);
                    let assistant_msg = res.get("assistant_message").map(parse_copilot_message);
                    messages.update(|m| {
                        m.retain(|msg| msg.role != "tool");
                        if let Some(u) = user_msg {
                            m.push(u);
                        }
                        if let Some(a) = assistant_msg {
                            m.push(a);
                        }
                    });
                }
                Err(e) => {
                    failure.set(Some(if e.contains("503") || e.contains("AI") {
                        format!("{e} — check the LM Studio connection in Settings.")
                    } else {
                        e
                    }));
                }
            }
            sending.set(false);
        });
    };

    let delete_session = move |_| {
        let sid = match session_id.get_untracked() {
            Some(s) => s,
            None => return,
        };
        let session_id = session_id;
        let messages = messages;
        spawn_local(async move {
            if crate::api::delete_json::<serde_json::Value>(&format!("/api/copilot/sessions/{sid}"))
                .await
                .is_ok()
            {
                session_id.set(None);
                messages.set(Vec::new());
                load_latest();
            }
        });
    };

    view! {
        <div class="spp-copilot-panel">
            <div class="spp-copilot-panel__head">
                <h3 class="spp-copilot-panel__title">
                    "Local Copilot"
                    <span
                        class="spp-badge spp-badge--tag"
                        title="Read-only tools; never writes to Help Scout; runs fully locally via LM Studio"
                    >
                        "read-only · local"
                    </span>
                </h3>
                <Show when=move || session_id.get().is_some() fallback=|| ()>
                    <button
                        class="spp-button spp-button--ghost spp-button--small"
                        on:click=delete_session
                        title="Delete this session"
                    >
                        "Delete"
                    </button>
                </Show>
            </div>

            <div class="spp-copilot-panel__safety">
                "Answers cite local evidence from read-only tools. Nothing is sent to the customer."
            </div>

            <div class="spp-copilot-panel__list">
                <Show
                    when=move || !messages.get().is_empty()
                    fallback=move || {
                        let starters_list = starters.get();
                        view! {
                            <div class="spp-copilot-panel__starters">
                                <p class="spp-muted spp-text-xs">"Start with a question, or pick one:"</p>
                                <div class="spp-copilot-panel__starter-row">
                                    {starters_list
                                        .into_iter()
                                        .map(|(q, why)| {
                                            let ask_q = q.clone();
                                            view! {
                                                <button
                                                    class="spp-chip"
                                                    title=why
                                                    on:click=move |_| ask(ask_q.clone())
                                                >
                                                    {q.clone()}
                                                </button>
                                            }
                                        })
                                        .collect::<Vec<_>>()}
                                </div>
                            </div>
                        }
                    }
                >
                    {move || {
                        messages.get()
                            .into_iter()
                            .map(|m| {
                                let is_user = m.role == "user";
                                let tool_note = if m.tool_calls > 0 {
                                    Some(format!("{} tool calls", m.tool_calls))
                                } else {
                                    None
                                };
                                let citations = m.citations.clone();
                                view! {
                                    <div class=if is_user {
                                        "spp-copilot-msg spp-copilot-msg--user"
                                    } else {
                                        "spp-copilot-msg spp-copilot-msg--assistant"
                                    }>
                                        <strong class="spp-text-xs">
                                            {if is_user { "You" } else { "Copilot" }}
                                        </strong>
                                        <p class="spp-copilot-msg__content">{m.content.clone()}</p>
                                        {if let Some(t) = tool_note {
                                            view! {
                                                <span class="spp-badge spp-badge--tag" title="Number of read-only tool calls used">
                                                    {t}
                                                </span>
                                            }.into_view()
                                        } else {
                                            ().into_view()
                                        }}
                                        {if !citations.is_empty() {
                                            view! {
                                                <div class="spp-copilot-citations">
                                                    <div class="spp-muted spp-text-xs">
                                                        "Sources (from tools the server actually executed):"
                                                    </div>
                                                    {citations
                                                        .into_iter()
                                                        .map(|c| {
                                                            let label = c.label.clone();
                                                            let idx = format!("[{}]", c.index);
                                                            let open_link = c.conversation_id.map(|cid| {
                                                                format!("/inbox/conversation/{cid}")
                                                            });
                                                            view! {
                                                                <div class="spp-copilot-citation">
                                                                    <span class="spp-copilot-citation__index">{idx}</span>
                                                                    <span class="spp-text-xs">{label.clone()}</span>
                                                                    {if let Some(href) = open_link {
                                                                        view! {
                                                                            <a class="spp-text-xs" href=href>"open"</a>
                                                                        }.into_view()
                                                                    } else {
                                                                        ().into_view()
                                                                    }}
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
                                }
                            })
                            .collect::<Vec<_>>()
                    }}
                </Show>
                <Show when=move || sending.get() fallback=|| ()>
                    <div class="spp-copilot-msg spp-copilot-msg--assistant spp-copilot-msg--pending">
                        <strong class="spp-text-xs">"Copilot"</strong>
                        <p class="spp-muted">"Thinking (local model)…"</p>
                    </div>
                </Show>
                <Show when=move || failure.get().is_some() fallback=|| ()>
                    <div class="spp-state spp-state--error">
                        <p class="spp-state__body">"The Copilot is unavailable."</p>
                        <p class="spp-state__detail">{move || failure.get().unwrap_or_default()}</p>
                    </div>
                </Show>
            </div>

            <form
                class="spp-copilot-panel__composer"
                on:submit=move |ev| {
                    ev.prevent_default();
                    ask(question.get_untracked());
                }
            >
                <input
                    class="spp-input"
                    type="text"
                    placeholder="Ask about this ticket, the customer, past cases, docs..."
                    maxlength=4000
                    prop:value=move || question.get()
                    on:input=move |ev| question.set(event_target_value(&ev))
                    disabled=move || sending.get()
                    aria-label="Ask the Local Copilot"
                />
                <button
                    class="spp-button spp-button--small"
                    type="submit"
                    disabled=move || sending.get() || question.get().trim().is_empty()
                >
                    "Ask"
                </button>
            </form>
        </div>
    }
}
