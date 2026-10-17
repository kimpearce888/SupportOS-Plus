use leptos::*;

use super::mention_textarea::MentionTextarea;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SideThreadSummary {
    pub id: i64,
    pub title: String,
    pub team_name: Option<String>,
    pub status: String,
    pub message_count: i64,
    pub updated_at: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SideThreadMessage {
    pub id: i64,
    pub author_first_name: Option<String>,
    pub author_last_name: Option<String>,
    pub created_at: String,
    pub body: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SideThreadParticipant {
    pub user_local_id: i64,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub mention: Option<String>,
    pub added_at: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SideThreadDetail {
    pub id: i64,
    pub title: String,
    pub team_name: Option<String>,
    pub status: String,
    pub participants: Vec<SideThreadParticipant>,
    pub messages: Vec<SideThreadMessage>,
}

fn opt_str(v: &serde_json::Value, key: &str) -> Option<String> {
    v.get(key).and_then(|x| x.as_str()).map(str::to_string)
}

#[must_use]
pub fn parse_side_thread_summary(v: &serde_json::Value) -> SideThreadSummary {
    SideThreadSummary {
        id: v.get("id").and_then(|x| x.as_i64()).unwrap_or(0),
        title: v
            .get("title")
            .and_then(|x| x.as_str())
            .unwrap_or_default()
            .to_string(),
        team_name: opt_str(v, "team_name"),
        status: v
            .get("status")
            .and_then(|x| x.as_str())
            .unwrap_or("open")
            .to_string(),
        message_count: v.get("message_count").and_then(|x| x.as_i64()).unwrap_or(0),
        updated_at: opt_str(v, "updated_at").unwrap_or_default(),
    }
}

#[must_use]
pub fn parse_side_thread_detail(v: &serde_json::Value) -> SideThreadDetail {
    let t = v.get("side_thread").cloned().unwrap_or_default();
    SideThreadDetail {
        id: t.get("id").and_then(|x| x.as_i64()).unwrap_or(0),
        title: t
            .get("title")
            .and_then(|x| x.as_str())
            .unwrap_or_default()
            .to_string(),
        team_name: opt_str(&t, "team_name"),
        status: t
            .get("status")
            .and_then(|x| x.as_str())
            .unwrap_or("open")
            .to_string(),
        participants: t
            .get("participants")
            .and_then(|p| p.as_array())
            .map(|rows| {
                rows.iter()
                    .map(|p| SideThreadParticipant {
                        user_local_id: p.get("user_local_id").and_then(|x| x.as_i64()).unwrap_or(0),
                        first_name: opt_str(p, "first_name"),
                        last_name: opt_str(p, "last_name"),
                        mention: opt_str(p, "mention"),
                        added_at: opt_str(p, "added_at").unwrap_or_default(),
                    })
                    .collect()
            })
            .unwrap_or_default(),
        messages: t
            .get("messages")
            .and_then(|m| m.as_array())
            .map(|rows| {
                rows.iter()
                    .map(|m| SideThreadMessage {
                        id: m.get("id").and_then(|x| x.as_i64()).unwrap_or(0),
                        author_first_name: opt_str(m, "author_first_name"),
                        author_last_name: opt_str(m, "author_last_name"),
                        created_at: opt_str(m, "created_at").unwrap_or_default(),
                        body: m
                            .get("body")
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
pub fn split_mention_segments(body: &str, known: &[String]) -> Vec<(String, bool)> {
    let lower_known: Vec<String> = known.iter().map(|k| k.to_lowercase()).collect();
    let mut out = Vec::new();
    let mut rest = body;
    while let Some(at) = rest.find('@') {
        if at > 0 {
            out.push((rest[..at].to_string(), false));
        }
        let after = &rest[at + 1..];
        let token_len = after
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '.' || *c == '_' || *c == '-')
            .map(char::len_utf8)
            .sum::<usize>();
        let token = &after[..token_len];
        if !token.is_empty() && lower_known.iter().any(|k| k == &token.to_lowercase()) {
            out.push((format!("@{token}"), true));
        } else {
            out.push((format!("@{token}"), false));
        }
        rest = &after[token_len..];
    }
    if !rest.is_empty() {
        out.push((rest.to_string(), false));
    }
    out
}

#[component]
pub fn SideThreadsPanel(conversation_id: i64) -> impl IntoView {
    let threads = create_rw_signal(Vec::<SideThreadSummary>::new());
    let loading = create_rw_signal(true);
    let error_msg = create_rw_signal(None::<String>);
    let creating = create_rw_signal(false);
    let open_thread_id = create_rw_signal(None::<i64>);

    let load = {
        move || {
            loading.set(true);
            let threads = threads;
            let loading = loading;
            let error_msg = error_msg;
            spawn_local(async move {
                match crate::api::get_json::<serde_json::Value>(&format!(
                    "/api/conversations/{conversation_id}/side-threads"
                ))
                .await
                {
                    Ok(v) => {
                        error_msg.set(None);
                        let rows = v
                            .get("side_threads")
                            .and_then(|s| s.as_array())
                            .map(|rows| rows.iter().map(parse_side_thread_summary).collect())
                            .unwrap_or_default();
                        threads.set(rows);
                    }
                    Err(e) => error_msg.set(Some(e)),
                }
                loading.set(false);
            });
        }
    };
    load();

    view! {
        <div class="spp-side-threads">
            <div class="spp-side-threads__head">
                <h3 class="spp-card__title">
                    "Side threads"
                    <span
                        class="spp-badge spp-badge--tag"
                        title="Internal only — never synced to Help Scout, never customer-visible"
                    >
                        "internal only"
                    </span>
                </h3>
                <button
                    class="spp-button spp-button--small"
                    on:click=move |_| {
                        creating.update(|c| *c = !*c);
                        open_thread_id.set(None);
                    }
                >
                    "New side thread"
                </button>
            </div>

            <Show when=move || loading.get() fallback=|| ()>
                <div class="spp-muted spp-text-xs">"Loading side threads…"</div>
            </Show>
            <Show when=move || error_msg.get().is_some() fallback=|| ()>
                <div class="spp-state spp-state--error">
                    <p class="spp-state__body">"Could not load side threads."</p>
                    <p class="spp-state__detail">{move || error_msg.get().unwrap_or_default()}</p>
                </div>
            </Show>

            <Show
                when=move || !loading.get() && threads.get().is_empty() && !creating.get()
                fallback=|| ()
            >
                <div class="spp-state spp-state--empty">
                    <p class="spp-state__title">"No side threads yet"</p>
                    <p class="spp-state__hint">
                        "Spin up an internal discussion attached to this conversation — e.g. Support, Engineering, Billing — without touching the customer-visible thread."
                    </p>
                </div>
            </Show>

            <Show when=move || creating.get() fallback=|| ()>
                <CreateThreadForm conversation_id=conversation_id on_done=move || {
                    creating.set(false);
                    load();
                } />
            </Show>

            <Show when=move || !threads.get().is_empty() fallback=|| ()>
                <div class="spp-side-threads__list">
                    {move || {
                        threads.get()
                            .into_iter()
                            .map(|t| {
                                let tid = t.id;
                                let is_open = open_thread_id.get() == Some(tid);
                                let title = t.title.clone();
                                let count_label = format!("{} msg", t.message_count);
                                let updated = t.updated_at.clone();
                                view! {
                                    <button
                                        class="spp-side-thread-row"
                                        class:is-active=move || open_thread_id.get() == Some(tid)
                                        on:click=move |_| {
                                            open_thread_id.set(if is_open { None } else { Some(tid) });
                                        }
                                    >
                                        <span class="spp-side-thread-row__title">{title.clone()}</span>
                                        {if let Some(team) = &t.team_name {
                                            view! { <span class="spp-badge spp-badge--tag">{team.clone()}</span> }.into_view()
                                        } else {
                                            ().into_view()
                                        }}
                                        {if t.status == "resolved" {
                                            view! { <span class="spp-badge spp-badge--ok">"resolved"</span> }.into_view()
                                        } else {
                                            ().into_view()
                                        }}
                                        <span class="spp-muted spp-text-xs">{count_label.clone()}</span>
                                        <span class="spp-muted spp-text-xs">{updated.clone()}</span>
                                    </button>
                                }
                            })
                            .collect::<Vec<_>>()
                    }}
                </div>
            </Show>

            {move || {
                match open_thread_id.get() {
                    Some(tid) => view! { <SideThreadDetail thread_id=tid on_change=load /> }.into_view(),
                    None => ().into_view(),
                }
            }}
        </div>
    }
}

#[component]
fn CreateThreadForm(conversation_id: i64, on_done: impl Fn() + 'static) -> impl IntoView {
    let directory = super::mention::use_mention_directory();
    let title = create_rw_signal(String::new());
    let team = create_rw_signal(String::new());
    let participants = create_rw_signal(Vec::<i64>::new());
    let first_message = create_rw_signal(String::new());
    let saving = create_rw_signal(false);
    let on_done = std::rc::Rc::new(on_done);
    let on_done_for_create = std::rc::Rc::clone(&on_done);

    let create = move || {
        let t = title.get_untracked().trim().to_string();
        if t.is_empty() || saving.get_untracked() {
            return;
        }
        saving.set(true);
        let team_id = team.get_untracked().parse::<i64>().ok();
        let body = serde_json::json!({
            "title": t,
            "team_local_id": team_id,
            "participant_user_ids": participants.get_untracked(),
            "first_message": if first_message.get_untracked().trim().is_empty() {
                serde_json::Value::Null
            } else {
                serde_json::json!(first_message.get_untracked().trim())
            },
        });
        let saving = saving;
        let on_done_for_create = std::rc::Rc::clone(&on_done_for_create);
        spawn_local(async move {
            if crate::api::post_json::<serde_json::Value>(
                &format!("/api/conversations/{conversation_id}/side-threads"),
                Some(&body),
            )
            .await
            .is_ok()
            {
                on_done_for_create();
            }
            saving.set(false);
        });
    };

    view! {
        <div class="spp-card spp-side-thread-create">
            <div class="spp-side-thread-create__row">
                <input
                    class="spp-input"
                    type="text"
                    maxlength=120
                    placeholder="Thread title (e.g. Engineering escalation)"
                    prop:value=move || title.get()
                    on:input=move |ev| title.set(event_target_value(&ev))
                />
                <select
                    class="spp-input"
                    aria-label="Anchor team (optional)"
                    prop:value=move || team.get()
                    on:change=move |ev| team.set(event_target_value(&ev))
                >
                    <option value="">"No team"</option>
                    {move || {
                        directory.get().teams
                            .iter()
                            .map(|t| {
                                let id = t.team_local_id.to_string();
                                view! { <option value=id.clone()>{t.name.clone()}</option> }
                            })
                            .collect::<Vec<_>>()
                    }}
                </select>
            </div>
            <div class="spp-side-thread-create__participants">
                <span class="spp-muted spp-text-xs">"participants:"</span>
                {move || {
                    directory.get().users
                        .iter()
                        .map(|u| {
                            let uid = u.user_local_id;
                            let checked = move || participants.get().contains(&uid);
                            view! {
                                <label class="spp-text-xs">
                                    <input
                                        type="checkbox"
                                        prop:checked=checked
                                        on:change=move |ev| {
                                            if event_target_checked(&ev) {
                                                participants.update(|p| {
                                                    if !p.contains(&uid) {
                                                        p.push(uid);
                                                    }
                                                });
                                            } else {
                                                participants.update(|p| p.retain(|x| *x != uid));
                                            }
                                        }
                                    />
                                    {u.display_name.clone()}
                                </label>
                            }
                        })
                        .collect::<Vec<_>>()
                }}
            </div>
            <div class="spp-side-thread-create__message">
                <MentionTextarea
                    value=first_message
                    placeholder="First message (optional) — @mention teammates to pull them in"
                    rows=3
                />
            </div>
            <div class="spp-side-thread-create__actions">
                <button
                    class="spp-button spp-button--primary spp-button--small"
                    disabled=move || saving.get() || title.get().trim().is_empty()
                    on:click=move |_| create()
                >
                    "Create"
                </button>
                <button class="spp-button spp-button--small" on:click=move |_| on_done()>
                    "Cancel"
                </button>
            </div>
        </div>
    }
}

#[component]
fn SideThreadDetail(thread_id: i64, on_change: impl Fn() + 'static + Copy) -> impl IntoView {
    let detail = create_rw_signal(None::<SideThreadDetail>);
    let loading = create_rw_signal(true);
    let error_msg = create_rw_signal(None::<String>);
    let body = create_rw_signal(String::new());
    let participants_open = create_rw_signal(false);
    let directory = super::mention::use_mention_directory();

    let load = {
        move || {
            loading.set(true);
            let detail = detail;
            let loading = loading;
            let error_msg = error_msg;
            spawn_local(async move {
                match crate::api::get_json::<serde_json::Value>(&format!(
                    "/api/side-threads/{thread_id}"
                ))
                .await
                {
                    Ok(v) => {
                        error_msg.set(None);
                        detail.set(Some(parse_side_thread_detail(&v)));
                    }
                    Err(e) => error_msg.set(Some(e)),
                }
                loading.set(false);
            });
        }
    };
    load();

    let send = move || {
        let text = body.get_untracked().trim().to_string();
        if text.is_empty() {
            return;
        }
        let payload = serde_json::json!({ "body": text });
        let body_sig = body;
        let load = load;
        let on_change = on_change;
        spawn_local(async move {
            if crate::api::post_json::<serde_json::Value>(
                &format!("/api/side-threads/{thread_id}/messages"),
                Some(&payload),
            )
            .await
            .is_ok()
            {
                body_sig.set(String::new());
                load();
                on_change();
            }
        });
    };

    let set_status = move |status: &'static str| {
        let action = if status == "resolved" {
            "resolve"
        } else {
            "reopen"
        };
        let load = load;
        let on_change = on_change;
        spawn_local(async move {
            if crate::api::post_json::<serde_json::Value>(
                &format!("/api/side-threads/{thread_id}/{action}"),
                None,
            )
            .await
            .is_ok()
            {
                load();
                on_change();
            }
        });
    };

    view! {
        <div class="spp-side-thread-detail">
            <Show when=move || loading.get() fallback=|| ()>
                <div class="spp-muted spp-text-xs">"Loading thread…"</div>
            </Show>
            <Show when=move || error_msg.get().is_some() fallback=|| ()>
                <div class="spp-state spp-state--error">
                    <p class="spp-state__body">"Could not load this side thread."</p>
                </div>
            </Show>
            {move || {
                match detail.get() {
                    Some(t) => {
                        let known: Vec<String> = {
                            let d = directory.get();
                            let mut k: Vec<String> = d
                                .users
                                .iter()
                                .filter_map(|u| u.mention.clone().map(|m| m.to_lowercase()))
                                .collect();
                            for u in &d.users {
                                if let Some(first) = u.display_name.split(' ').next() {
                                    k.push(first.to_lowercase());
                                }
                                k.push(u.display_name.to_lowercase().replace(' ', ""));
                            }
                            for team in &d.teams {
                                k.push(team.name.to_lowercase());
                            }
                            k
                        };
                        view! {
                            <div class="spp-side-thread-detail__head">
                                <div class="spp-side-thread-detail__id">
                                    <strong>{t.title.clone()}</strong>
                                    {if let Some(team) = &t.team_name {
                                        view! { <span class="spp-badge spp-badge--tag">{team.clone()}</span> }.into_view()
                                    } else {
                                        ().into_view()
                                    }}
                                    <span class=format!(
                                        "spp-badge {}",
                                        if t.status == "open" { "spp-badge--warn" } else { "spp-badge--ok" }
                                    )>
                                        {t.status.clone()}
                                    </span>
                                    <button
                                        class="spp-button spp-button--ghost spp-button--small"
                                        on:click=move |_| participants_open.update(|p| *p = !*p)
                                    >
                                        {format!("{} participants", t.participants.len())}
                                    </button>
                                </div>
                                <div>
                                    {if t.status == "open" {
                                        view! {
                                            <button class="spp-button spp-button--small" on:click=move |_| set_status("resolved")>
                                                "Resolve"
                                            </button>
                                        }.into_view()
                                    } else {
                                        view! {
                                            <button class="spp-button spp-button--small" on:click=move |_| set_status("open")>
                                                "Reopen"
                                            </button>
                                        }.into_view()
                                    }}
                                </div>
                            </div>
                            <Show when=move || participants_open.get() fallback=|| ()>
                                <div class="spp-side-thread-detail__participants">
                                    {t.participants
                                        .iter()
                                        .map(|p| {
                                            let name = [p.first_name.clone(), p.last_name.clone()]
                                                .iter()
                                                .flatten()
                                                .cloned()
                                                .collect::<Vec<_>>()
                                                .join(" ");
                                            let label = match &p.mention {
                                                Some(m) => format!("{name} (@{m})"),
                                                None => name,
                                            };
                                            view! {
                                                <span class="spp-badge spp-badge--tag" title=format!("added {}", p.added_at)>
                                                    {label}
                                                </span>
                                            }
                                        })
                                        .collect::<Vec<_>>()}
                                </div>
                            </Show>
                            <div class="spp-side-thread-messages">
                                {if t.messages.is_empty() {
                                    view! {
                                        <div class="spp-state spp-state--empty">
                                            <p class="spp-state__title">"No messages yet"</p>
                                        </div>
                                    }.into_view()
                                } else {
                                    t.messages
                                        .iter()
                                        .map(|m| {
                                            let author = [m.author_first_name.clone(), m.author_last_name.clone()]
                                                .iter()
                                                .flatten()
                                                .cloned()
                                                .collect::<Vec<_>>()
                                                .join(" ");
                                            let author = if author.is_empty() { "Unknown".to_string() } else { author };
                                            let segments = split_mention_segments(&m.body, &known);
                                            view! {
                                                <div class="spp-side-thread-message">
                                                    <div class="spp-side-thread-message__head">
                                                        <span class="spp-side-thread-message__author">{author.clone()}</span>
                                                        <span class="spp-muted spp-text-xs">{m.created_at.clone()}</span>
                                                    </div>
                                                    <div class="spp-side-thread-message__body">
                                                        {segments
                                                            .into_iter()
                                                            .map(|(text, is_mention)| {
                                                                if is_mention {
                                                                    view! { <span class="spp-mention-token">{text}</span> }.into_view()
                                                                } else {
                                                                    view! { <span>{text}</span> }.into_view()
                                                                }
                                                            })
                                                            .collect::<Vec<_>>()}
                                                    </div>
                                                </div>
                                            }
                                        })
                                        .collect::<Vec<_>>()
                                        .into_view()
                                }}
                            </div>
                            {if t.status == "open" {
                                view! {
                                    <div class="spp-side-thread-composer">
                                        <MentionTextarea
                                            value=body
                                            placeholder="Write to the team — @mentions notify instantly"
                                            rows=2
                                        />
                                        <button
                                            class="spp-button spp-button--primary spp-button--small"
                                            disabled=move || body.get().trim().is_empty()
                                            on:click=move |_| send()
                                        >
                                            "Send"
                                        </button>
                                    </div>
                                }.into_view()
                            } else {
                                ().into_view()
                            }}
                        }.into_view()
                    }
                    None => ().into_view(),
                }
            }}
        </div>
    }
}
