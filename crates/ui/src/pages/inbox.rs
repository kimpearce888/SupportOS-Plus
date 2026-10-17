use leptos::*;
use leptos_router::{use_location, use_navigate, use_query_map};

use crate::components::attribute_snapshot::AttributeSnapshotCard;
use crate::components::coaching_panel::CoachingPanel;
use crate::components::copilot_panel::CopilotPanel;
use crate::components::memory_panel::MemoryPanel;
use crate::components::mention_textarea::MentionTextarea;
use crate::components::overlays::ConfirmDialog;
use crate::components::qa_panel::QaPanel;
use crate::components::safe_html::SafeHtml;
use crate::components::side_threads::SideThreadsPanel;
use crate::components::state_view::{EmptyState, LoadingState};
use crate::components::translation_panel::TranslationPanel;
use crate::toasts;

fn urlencode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[derive(Debug, Clone, Default)]
pub struct ConversationListItem {
    pub id: i64,
    pub remote_id: i64,
    pub number: i64,
    pub subject: Option<String>,
    pub preview: Option<String>,
    pub status: String,
    pub mailbox_id: i64,
    pub mailbox_name: Option<String>,
    pub assignee_id: Option<i64>,
    pub assignee_name: Option<String>,
    pub customer_id: i64,
    pub customer_name: Option<String>,
    pub priority: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub closed_at: Option<String>,
    pub response_state: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ThreadEntry {
    pub id: i64,
    pub conversation_id: i64,
    pub thread_type: String,
    pub body: Option<String>,
    pub actor_type: String,
    pub actor_id: Option<i64>,
    pub actor_name: Option<String>,
    pub created_at: String,
    pub state: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ConversationDetail {
    pub id: i64,
    pub remote_id: i64,
    pub number: i64,
    pub subject: Option<String>,
    pub preview: Option<String>,
    pub status: String,
    pub mailbox_id: i64,
    pub mailbox_name: Option<String>,
    pub assignee_id: Option<i64>,
    pub assignee_name: Option<String>,
    pub customer_id: i64,
    pub customer_name: Option<String>,
    pub customer_email: Option<String>,
    pub priority: Option<String>,
    pub response_state: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub closed_at: Option<String>,
    pub thread: Vec<ThreadEntry>,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct SavedView {
    pub id: Option<i64>,
    pub name: String,
    pub mailbox_id: Option<i64>,
}

#[derive(Debug, Clone, Default)]
pub struct InboxFilters {
    pub status: Option<String>,
    pub mailbox_id: Option<i64>,
    pub priority: Option<String>,
    pub query: Option<String>,
    pub tag: Option<String>,
    pub channel: Option<String>,
    pub saved_view_id: Option<i64>,
    pub page: u32,
}

impl InboxFilters {
    fn reset_page(&mut self) {
        self.page = 1;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComposerMode {
    Reply,
    Note,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextTab {
    Ai,
    Customer,
    Copilot,
    Activity,
}

#[derive(Debug, Clone, Default)]
pub struct SimilarConversation {
    pub conversation_id: i64,
    pub number: i64,
    pub subject: String,
    pub resolution: String,
    pub score: f64,
    pub why: Vec<String>,
}

#[must_use]
fn parse_similar_conversation(v: &serde_json::Value) -> SimilarConversation {
    SimilarConversation {
        conversation_id: v
            .get("conversation_id")
            .and_then(|x| x.as_i64())
            .unwrap_or(0),
        number: v.get("number").and_then(|x| x.as_i64()).unwrap_or(0),
        subject: v
            .get("subject")
            .and_then(|x| x.as_str())
            .unwrap_or_default()
            .to_string(),
        resolution: v
            .get("resolution")
            .and_then(|x| x.as_str())
            .unwrap_or_default()
            .to_string(),
        score: v.get("score").and_then(|x| x.as_f64()).unwrap_or(0.0),
        why: v
            .get("why")
            .and_then(|w| w.as_array())
            .map(|rows| {
                rows.iter()
                    .filter_map(|w| w.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default(),
    }
}

fn parse_recipients(raw: &str) -> Vec<String> {
    raw.split([',', ';'])
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(str::to_string)
        .collect()
}

#[component]
pub fn InboxPage(#[prop(optional, into)] conversation_id: Option<i64>) -> impl IntoView {
    let conversations = create_rw_signal(Vec::<ConversationListItem>::new());
    let total = create_rw_signal(0u32);
    let filters = create_rw_signal(InboxFilters::default());
    let selected_id = create_rw_signal(conversation_id);
    let detail = create_rw_signal(None::<ConversationDetail>);
    let detail_loading = create_rw_signal(false);
    let detail_error = create_rw_signal(None::<String>);
    let list_loading = create_rw_signal(true);
    let list_error = create_rw_signal(None::<String>);
    let saved_views = create_rw_signal(Vec::<SavedView>::new());
    let composer_mode = create_rw_signal(ComposerMode::Reply);
    let composer_body = create_rw_signal(String::new());
    let composer_error = create_rw_signal(None::<String>);
    let selected_ids = create_rw_signal(Vec::<i64>::new());
    let context_tab = create_rw_signal(ContextTab::Ai);
    let context_open = create_rw_signal(true);
    let confirm_send = create_rw_signal(false);
    let ai_analysis = create_rw_signal(None::<serde_json::Value>);
    let ai_analyzing = create_rw_signal(false);
    let similar = create_rw_signal(Vec::<SimilarConversation>::new());

    let users = create_rw_signal(Vec::<(i64, i64, String)>::new());
    let mailboxes = create_rw_signal(Vec::<(i64, String)>::new());
    let tag_names = create_rw_signal(Vec::<String>::new());
    let saved_replies = create_rw_signal(Vec::<(String, String)>::new());

    let composer_cc = create_rw_signal(String::new());
    let composer_bcc = create_rw_signal(String::new());
    let ai_drafting = create_rw_signal(false);

    let editing_subject = create_rw_signal(false);
    let subject_draft = create_rw_signal(String::new());
    let snooze_until = create_rw_signal(String::new());

    let bulk_action = create_rw_signal("close".to_string());
    let bulk_param = create_rw_signal(String::new());

    let tag_input = create_rw_signal(String::new());

    let saving_view = create_rw_signal(false);
    let view_name = create_rw_signal(String::new());
    let saved_reply_pick = create_rw_signal(String::new());

    let sse_refresh = crate::queries::version("conversations");

    let query_map = use_query_map();
    let location = use_location();
    let navigate = use_navigate();
    create_effect(move |_| {
        let m = query_map.get();
        let url_status = crate::url_state::query_str(&m, "view");
        let url_tag = crate::url_state::query_str(&m, "tag");
        let url_channel = crate::url_state::query_str(&m, "channel");
        let url_page = crate::url_state::query_pos_int(&m, "page")
            .unwrap_or(1)
            .max(1) as u32;
        let url_view_id = crate::url_state::query_pos_int(&m, "savedViewId");
        let url_mailbox = crate::url_state::query_pos_int(&m, "mailboxId");
        let mut changed = false;
        filters.update(|f| {
            if f.status != url_status {
                f.status = url_status;
                changed = true;
            }
            if f.tag != url_tag {
                f.tag = url_tag;
                changed = true;
            }
            if f.channel != url_channel {
                f.channel = url_channel;
                changed = true;
            }
            if f.page != url_page {
                f.page = url_page;
                changed = true;
            }
            if f.saved_view_id != url_view_id {
                f.saved_view_id = url_view_id;
                changed = true;
            }
            if f.mailbox_id != url_mailbox {
                f.mailbox_id = url_mailbox;
                changed = true;
            }
        });
        if changed {
            list_loading.set(true);
        }
    });
    let sync_url = StoredValue::new({
        let navigate = navigate.clone();
        let pathname = location.pathname;
        std::rc::Rc::new(move || {
            let f = filters.get_untracked();
            crate::url_state::replace_query(
                &navigate,
                &pathname.get_untracked(),
                &[
                    ("view", f.status.clone()),
                    ("tag", f.tag.clone()),
                    ("channel", f.channel.clone()),
                    ("savedViewId", f.saved_view_id.map(|id| id.to_string())),
                    ("mailboxId", f.mailbox_id.map(|id| id.to_string())),
                    (
                        "page",
                        if f.page > 1 {
                            Some(f.page.to_string())
                        } else {
                            None
                        },
                    ),
                ],
            );
        }) as std::rc::Rc<dyn Fn()>
    });

    create_effect(move |_| {
        let users = users;
        let mailboxes = mailboxes;
        let tag_names = tag_names;
        let saved_replies = saved_replies;
        wasm_bindgen_futures::spawn_local(async move {
            if let Ok(v) = crate::api::get_json::<serde_json::Value>("/api/users").await {
                let rows = v
                    .get("users")
                    .and_then(|u| u.as_array())
                    .cloned()
                    .unwrap_or_default();
                users.set(
                    rows.iter()
                        .filter_map(|u| {
                            Some((
                                u.get("id").and_then(|x| x.as_i64())?,
                                u.get("remote_id").and_then(|x| x.as_i64())?,
                                u.get("display_name")
                                    .and_then(|x| x.as_str())
                                    .unwrap_or("")
                                    .to_string(),
                            ))
                        })
                        .collect(),
                );
            }
            if let Ok(v) = crate::api::get_json::<serde_json::Value>("/api/mailboxes").await {
                mailboxes.set(
                    v.as_array()
                        .map(|rows| {
                            rows.iter()
                                .filter_map(|m| {
                                    Some((
                                        m.get("id").and_then(|x| x.as_i64())?,
                                        m.get("name")
                                            .and_then(|x| x.as_str())
                                            .unwrap_or("")
                                            .to_string(),
                                    ))
                                })
                                .collect()
                        })
                        .unwrap_or_default(),
                );
            }
            if let Ok(v) = crate::api::get_json::<serde_json::Value>("/api/tags").await {
                tag_names.set(
                    v.as_array()
                        .map(|rows| {
                            rows.iter()
                                .filter_map(|t| {
                                    t.get("name").and_then(|x| x.as_str()).map(str::to_string)
                                })
                                .collect()
                        })
                        .unwrap_or_default(),
                );
            }
            if let Ok(v) = crate::api::get_json::<serde_json::Value>("/api/saved-replies").await {
                saved_replies.set(
                    v.get("saved_replies")
                        .and_then(|r| r.as_array())
                        .map(|rows| {
                            rows.iter()
                                .filter_map(|r| {
                                    let name = r.get("name").and_then(|x| x.as_str())?.to_string();
                                    let text = r
                                        .get("text")
                                        .and_then(|x| x.as_str())
                                        .unwrap_or("")
                                        .to_string();
                                    Some((name, text))
                                })
                                .collect()
                        })
                        .unwrap_or_default(),
                );
            }
        });
    });

    create_effect(move |_| {
        let conversations = conversations;
        let total = total;
        let list_loading = list_loading;
        let list_error = list_error;
        let current_filters = filters.get();
        let _ = sse_refresh.get();
        wasm_bindgen_futures::spawn_local(async move {
            let mut path = String::from("/api/conversations?pageSize=50");
            if let Some(status) = current_filters.status {
                path.push_str(&format!("&view={status}"));
            }
            if let Some(priority) = current_filters.priority {
                path.push_str(&format!("&priority={priority}"));
            }
            if let Some(q) = current_filters.query {
                path.push_str(&format!("&q={}", urlencode(&q)));
            }
            if let Some(tag) = current_filters.tag {
                path.push_str(&format!("&tag={}", urlencode(&tag)));
            }
            if let Some(channel) = current_filters.channel {
                path.push_str(&format!("&channel={channel}"));
            }
            if let Some(mailbox_id) = current_filters.mailbox_id {
                path.push_str(&format!("&mailboxId={mailbox_id}"));
            }
            if let Some(view_id) = current_filters.saved_view_id {
                path.push_str(&format!("&savedViewId={view_id}"));
            }
            let page = current_filters.page.max(1);
            path.push_str(&format!("&page={page}"));
            match crate::api::get_json::<serde_json::Value>(&path).await {
                Ok(data) => {
                    let items = data
                        .get("conversations")
                        .and_then(|v| v.as_array())
                        .map(|arr| {
                            arr.iter()
                                .map(parse_conversation_list_item)
                                .collect::<Vec<_>>()
                        })
                        .unwrap_or_default();
                    let total_val = data.get("total").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                    conversations.set(items);
                    total.set(total_val);
                    list_loading.set(false);
                    list_error.set(None);
                }
                Err(e) => {
                    list_error.set(Some(e));
                    list_loading.set(false);
                }
            }
        });
    });

    create_effect(move |_| {
        let saved_views = saved_views;
        wasm_bindgen_futures::spawn_local(async move {
            if let Ok(payload) = crate::api::get_json::<serde_json::Value>("/api/inbox-views").await
            {
                let arr = payload
                    .get("views")
                    .and_then(|v| v.as_array())
                    .cloned()
                    .unwrap_or_default();
                let views: Vec<SavedView> = arr
                    .iter()
                    .map(|v| SavedView {
                        id: v.get("id").and_then(|x| x.as_i64()),
                        name: v
                            .get("name")
                            .and_then(|x| x.as_str())
                            .unwrap_or("")
                            .to_string(),
                        mailbox_id: v.get("mailbox_id").and_then(|x| x.as_i64()),
                    })
                    .collect();
                saved_views.set(views);
            }
        });
    });

    create_effect(move |_| {
        let id = selected_id.get();
        let detail = detail;
        let detail_loading = detail_loading;
        let detail_error = detail_error;
        if let Some(id) = id {
            detail_loading.set(true);
            detail_error.set(None);
            wasm_bindgen_futures::spawn_local(async move {
                let path = format!("/api/conversations/{id}");
                match crate::api::get_json::<serde_json::Value>(&path).await {
                    Ok(data) => {
                        detail.set(Some(parse_conversation_detail(&data)));
                        detail_loading.set(false);
                    }
                    Err(e) => {
                        let msg = if e.contains("404") {
                            "Conversation not found".to_string()
                        } else {
                            e
                        };
                        detail_error.set(Some(msg));
                        detail.set(None);
                        detail_loading.set(false);
                    }
                }
            });
        } else {
            detail.set(None);
        }
    });

    create_effect(move |_| {
        let id = selected_id.get();
        ai_analysis.set(None);
        similar.set(Vec::new());
        if let Some(id) = id {
            let similar = similar;
            wasm_bindgen_futures::spawn_local(async move {
                if let Ok(r) =
                    crate::api::get_json::<serde_json::Value>(&format!("/api/ai/similar/{id}"))
                        .await
                {
                    let rows = r
                        .get("similar")
                        .and_then(|s| s.as_array())
                        .map(|arr| arr.iter().map(parse_similar_conversation).collect())
                        .unwrap_or_default();
                    similar.set(rows);
                }
            });
        }
    });

    let run_analyze = move || {
        let Some(id) = selected_id.get_untracked() else {
            return;
        };
        if ai_analyzing.get_untracked() {
            return;
        }
        ai_analyzing.set(true);
        let body = serde_json::json!({ "force": true });
        let ai_analysis = ai_analysis;
        let ai_analyzing = ai_analyzing;
        wasm_bindgen_futures::spawn_local(async move {
            match crate::api::post_json::<serde_json::Value>(
                &format!("/api/ai/analyze/{id}"),
                Some(&body),
            )
            .await
            {
                Ok(r) => {
                    if r.get("ok").and_then(|x| x.as_bool()).unwrap_or(false) {
                        toasts::success("AI analysis completed.");
                        if let Some(a) = r.get("analysis") {
                            ai_analysis.set(Some(a.clone()));
                        }
                    } else {
                        let reason = r
                            .get("error")
                            .and_then(|x| x.as_str())
                            .unwrap_or("Analysis failed");
                        toasts::error(reason);
                    }
                }
                Err(e) => toasts::error(e),
            }
            ai_analyzing.set(false);
        });
    };

    let send_composer = move || {
        let body = composer_body.get();
        if body.trim().is_empty() {
            composer_error.set(Some("Body cannot be empty".to_string()));
            return;
        }
        let mode = composer_mode.get();
        let conv_local_id = detail.with(|d| d.as_ref().map(|d| d.id));
        let conv_local_id = match conv_local_id {
            Some(id) => id,
            None => {
                composer_error.set(Some("No conversation selected".to_string()));
                return;
            }
        };

        let sub_path = match mode {
            ComposerMode::Reply => "reply",
            ComposerMode::Note => "note",
        };
        let is_reply = mode == ComposerMode::Reply;
        let path = format!("/api/conversations/{conv_local_id}/{sub_path}");
        let body_payload = if is_reply {
            let cc = parse_recipients(&composer_cc.get_untracked());
            let bcc = parse_recipients(&composer_bcc.get_untracked());
            serde_json::json!({ "text": body, "cc": cc, "bcc": bcc })
        } else {
            serde_json::json!({ "text": body })
        };
        let composer_error = composer_error;
        let composer_body = composer_body;
        let selected_id = selected_id;
        let composer_cc = composer_cc;
        let composer_bcc = composer_bcc;
        wasm_bindgen_futures::spawn_local(async move {
            match crate::api::post_json::<serde_json::Value>(&path, Some(&body_payload)).await {
                Ok(result) => {
                    let ok = result.get("ok").and_then(|v| v.as_bool()).unwrap_or(false);
                    let message = result
                        .get("message")
                        .and_then(|v| v.as_str())
                        .unwrap_or(if ok { "Sent" } else { "Operation rejected" })
                        .to_string();
                    let detail = result
                        .get("detail")
                        .and_then(|v| v.as_str())
                        .map(str::to_string);
                    if ok {
                        composer_error.set(None);
                        composer_body.set(String::new());
                        composer_cc.set(String::new());
                        composer_bcc.set(String::new());
                        if let Some(id) = selected_id.get() {
                            selected_id.set(None);
                            selected_id.set(Some(id));
                        }
                    }
                    let kind = if ok {
                        crate::toasts::ToastKind::Success
                    } else {
                        crate::toasts::ToastKind::Error
                    };
                    crate::toasts::push(kind, message, detail);
                }
                Err(e) => {
                    let prefix = if is_reply {
                        "Your text is preserved in the composer. "
                    } else {
                        "Your text is preserved. "
                    };
                    toasts::error(format!("{prefix}{e}"));
                }
            }
        });
    };

    let request_submit = move || {
        if composer_mode.get() == ComposerMode::Reply {
            confirm_send.set(true);
        } else {
            send_composer();
        }
    };

    let change_status = move |new_status: String| {
        if new_status == "closed" {
            let confirmed = web_sys::window()
                .and_then(|w| {
                    w.confirm_with_message(
                        "Close this conversation? The customer can no longer reply unless it is reopened.",
                    )
                    .ok()
                })
                .unwrap_or(false);
            if !confirmed {
                return;
            }
        }
        let conv_local_id = detail.with(|d| d.as_ref().map(|d| d.id));
        if let Some(conv_local_id) = conv_local_id {
            let selected_id = selected_id;
            wasm_bindgen_futures::spawn_local(async move {
                let path = format!("/api/conversations/{conv_local_id}/status");
                let payload = serde_json::json!({ "status": new_status });
                match crate::api::post_json::<serde_json::Value>(&path, Some(&payload)).await {
                    Ok(result) => {
                        let ok = result.get("ok").and_then(|v| v.as_bool()).unwrap_or(false);
                        let message = result
                            .get("message")
                            .and_then(|v| v.as_str())
                            .unwrap_or(if ok {
                                "Status updated."
                            } else {
                                "Status change rejected"
                            })
                            .to_string();
                        if ok {
                            if let Some(id) = selected_id.get() {
                                selected_id.set(None);
                                selected_id.set(Some(id));
                            }
                        }
                        let kind = if ok {
                            crate::toasts::ToastKind::Success
                        } else {
                            crate::toasts::ToastKind::Error
                        };
                        crate::toasts::push(kind, message, None);
                    }
                    Err(e) => toasts::error(e),
                }
            });
        }
    };

    let assign_to = move |assignee_id: Option<i64>| {
        let conv_local_id = detail.with(|d| d.as_ref().map(|d| d.id));
        if let Some(conv_local_id) = conv_local_id {
            let selected_id = selected_id;
            wasm_bindgen_futures::spawn_local(async move {
                let path = format!("/api/conversations/{conv_local_id}/assign");
                let payload = serde_json::json!({ "userId": assignee_id });
                match crate::api::post_json::<serde_json::Value>(&path, Some(&payload)).await {
                    Ok(result) => {
                        let ok = result.get("ok").and_then(|v| v.as_bool()).unwrap_or(false);
                        let message = result
                            .get("message")
                            .and_then(|v| v.as_str())
                            .unwrap_or(if ok {
                                "Assignee updated."
                            } else {
                                "Assignment rejected"
                            })
                            .to_string();
                        if ok {
                            if let Some(id) = selected_id.get() {
                                selected_id.set(None);
                                selected_id.set(Some(id));
                            }
                        }
                        let kind = if ok {
                            crate::toasts::ToastKind::Success
                        } else {
                            crate::toasts::ToastKind::Error
                        };
                        crate::toasts::push(kind, message, None);
                    }
                    Err(e) => toasts::error(e),
                }
            });
        }
    };

    let refresh_detail = move || {
        if let Some(id) = selected_id.get_untracked() {
            selected_id.set(None);
            selected_id.set(Some(id));
        }
    };

    let change_priority = move |new_priority: String| {
        let conv_local_id = detail.with(|d| d.as_ref().map(|d| d.id));
        if let Some(conv_local_id) = conv_local_id {
            let refresh_detail = refresh_detail;
            wasm_bindgen_futures::spawn_local(async move {
                let path = format!("/api/conversations/{conv_local_id}/priority");
                let payload = serde_json::json!({ "priority": new_priority });
                match crate::api::post_json::<serde_json::Value>(&path, Some(&payload)).await {
                    Ok(result) => {
                        let ok = result.get("ok").and_then(|v| v.as_bool()).unwrap_or(false);
                        let message = result
                            .get("message")
                            .and_then(|v| v.as_str())
                            .unwrap_or(if ok {
                                "Priority updated."
                            } else {
                                "Priority rejected"
                            })
                            .to_string();
                        if ok {
                            refresh_detail();
                        }
                        let kind = if ok {
                            crate::toasts::ToastKind::Success
                        } else {
                            crate::toasts::ToastKind::Error
                        };
                        crate::toasts::push(kind, message, None);
                    }
                    Err(e) => toasts::error(e),
                }
            });
        }
    };

    let save_subject = move || {
        let conv_local_id = detail.with(|d| d.as_ref().map(|d| d.id));
        let Some(conv_local_id) = conv_local_id else {
            return;
        };
        let subject = subject_draft.get_untracked();
        if subject.trim().is_empty() {
            toasts::error("Subject cannot be empty.");
            return;
        }
        let refresh_detail = refresh_detail;
        wasm_bindgen_futures::spawn_local(async move {
            let path = format!("/api/conversations/{conv_local_id}/subject");
            let payload = serde_json::json!({ "subject": subject.trim() });
            match crate::api::post_json::<serde_json::Value>(&path, Some(&payload)).await {
                Ok(result) => {
                    let ok = result.get("ok").and_then(|v| v.as_bool()).unwrap_or(false);
                    let message = result
                        .get("message")
                        .and_then(|v| v.as_str())
                        .unwrap_or(if ok {
                            "Subject updated."
                        } else {
                            "Subject rejected"
                        })
                        .to_string();
                    if ok {
                        editing_subject.set(false);
                        refresh_detail();
                    }
                    let kind = if ok {
                        crate::toasts::ToastKind::Success
                    } else {
                        crate::toasts::ToastKind::Error
                    };
                    crate::toasts::push(kind, message, None);
                }
                Err(e) => toasts::error(e),
            }
        });
    };

    let apply_tag_change = move |add: Vec<String>, remove: Vec<String>| {
        let conv_local_id = detail.with(|d| d.as_ref().map(|d| d.id));
        if let Some(conv_local_id) = conv_local_id {
            let refresh_detail = refresh_detail;
            wasm_bindgen_futures::spawn_local(async move {
                let path = format!("/api/conversations/{conv_local_id}/tags");
                let payload = serde_json::json!({ "add": add, "remove": remove });
                match crate::api::post_json::<serde_json::Value>(&path, Some(&payload)).await {
                    Ok(result) => {
                        let ok = result.get("ok").and_then(|v| v.as_bool()).unwrap_or(false);
                        let message = result
                            .get("message")
                            .and_then(|v| v.as_str())
                            .unwrap_or(if ok {
                                "Tags updated."
                            } else {
                                "Tag change rejected"
                            })
                            .to_string();
                        if ok {
                            refresh_detail();
                        }
                        let kind = if ok {
                            crate::toasts::ToastKind::Success
                        } else {
                            crate::toasts::ToastKind::Error
                        };
                        crate::toasts::push(kind, message, None);
                    }
                    Err(e) => toasts::error(e),
                }
            });
        }
    };

    let do_snooze = move || {
        let conv_local_id = detail.with(|d| d.as_ref().map(|d| d.id));
        let Some(conv_local_id) = conv_local_id else {
            return;
        };
        let until = snooze_until.get_untracked();
        if until.trim().is_empty() {
            toasts::error("Pick a snooze-until date/time first.");
            return;
        }
        let refresh_detail = refresh_detail;
        wasm_bindgen_futures::spawn_local(async move {
            let path = format!("/api/conversations/{conv_local_id}/snooze");
            let payload =
                serde_json::json!({ "snoozedUntil": until, "unsnoozeOnCustomerReply": true });
            match crate::api::post_json::<serde_json::Value>(&path, Some(&payload)).await {
                Ok(result) => {
                    let ok = result.get("ok").and_then(|v| v.as_bool()).unwrap_or(false);
                    let message = result
                        .get("message")
                        .and_then(|v| v.as_str())
                        .unwrap_or(if ok { "Snoozed." } else { "Snooze rejected" })
                        .to_string();
                    if ok {
                        snooze_until.set(String::new());
                        refresh_detail();
                    }
                    let kind = if ok {
                        crate::toasts::ToastKind::Success
                    } else {
                        crate::toasts::ToastKind::Error
                    };
                    crate::toasts::push(kind, message, None);
                }
                Err(e) => toasts::error(e),
            }
        });
    };

    let do_unsnooze = move || {
        let conv_local_id = detail.with(|d| d.as_ref().map(|d| d.id));
        if let Some(conv_local_id) = conv_local_id {
            let refresh_detail = refresh_detail;
            wasm_bindgen_futures::spawn_local(async move {
                let path = format!("/api/conversations/{conv_local_id}/snooze");
                match crate::api::delete_json::<serde_json::Value>(&path).await {
                    Ok(result) => {
                        let ok = result.get("ok").and_then(|v| v.as_bool()).unwrap_or(false);
                        let message = result
                            .get("message")
                            .and_then(|v| v.as_str())
                            .unwrap_or(if ok {
                                "Snooze removed."
                            } else {
                                "Unsnooze rejected"
                            })
                            .to_string();
                        if ok {
                            refresh_detail();
                        }
                        let kind = if ok {
                            crate::toasts::ToastKind::Success
                        } else {
                            crate::toasts::ToastKind::Error
                        };
                        crate::toasts::push(kind, message, None);
                    }
                    Err(e) => toasts::error(e),
                }
            });
        }
    };

    let schedule_draft = move |thread_id: i64| {
        let conv_local_id = detail.with(|d| d.as_ref().map(|d| d.id));
        let Some(conv_local_id) = conv_local_id else {
            return;
        };
        let confirmed = web_sys::window()
            .and_then(|w| {
                w.prompt_with_message(
                    "Send this draft at (ISO date-time, e.g. 2026-10-09T09:00:00Z):",
                )
                .ok()
                .flatten()
            })
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        let Some(scheduled_for) = confirmed else {
            return;
        };
        let refresh_detail = refresh_detail;
        wasm_bindgen_futures::spawn_local(async move {
            let path = format!("/api/conversations/{conv_local_id}/schedule");
            let payload = serde_json::json!({ "threadId": thread_id, "scheduledFor": scheduled_for, "unscheduleOnCustomerReply": true });
            match crate::api::post_json::<serde_json::Value>(&path, Some(&payload)).await {
                Ok(result) => {
                    let ok = result.get("ok").and_then(|v| v.as_bool()).unwrap_or(false);
                    let message = result
                        .get("message")
                        .and_then(|v| v.as_str())
                        .unwrap_or(if ok {
                            "Reply scheduled."
                        } else {
                            "Schedule rejected"
                        })
                        .to_string();
                    if ok {
                        refresh_detail();
                    }
                    let kind = if ok {
                        crate::toasts::ToastKind::Success
                    } else {
                        crate::toasts::ToastKind::Error
                    };
                    crate::toasts::push(kind, message, None);
                }
                Err(e) => toasts::error(e),
            }
        });
    };

    let cancel_schedule = move |thread_id: i64| {
        let conv_local_id = detail.with(|d| d.as_ref().map(|d| d.id));
        if let Some(conv_local_id) = conv_local_id {
            let refresh_detail = refresh_detail;
            wasm_bindgen_futures::spawn_local(async move {
                let path = format!("/api/conversations/{conv_local_id}/schedule");
                let payload = serde_json::json!({ "threadId": thread_id });
                match crate::api::delete_json_with_body::<serde_json::Value>(&path, &payload).await
                {
                    Ok(result) => {
                        let ok = result.get("ok").and_then(|v| v.as_bool()).unwrap_or(false);
                        let message = result
                            .get("message")
                            .and_then(|v| v.as_str())
                            .unwrap_or(if ok {
                                "Schedule removed."
                            } else {
                                "Cancel rejected"
                            })
                            .to_string();
                        if ok {
                            refresh_detail();
                        }
                        let kind = if ok {
                            crate::toasts::ToastKind::Success
                        } else {
                            crate::toasts::ToastKind::Error
                        };
                        crate::toasts::push(kind, message, None);
                    }
                    Err(e) => toasts::error(e),
                }
            });
        }
    };

    let apply_bulk = move || {
        let ids = selected_ids.get_untracked();
        if ids.is_empty() {
            return;
        }
        let action = bulk_action.get_untracked();
        let param = bulk_param.get_untracked();
        let params = match action.as_str() {
            "tag" | "untag" => {
                if param.trim().is_empty() {
                    toasts::error("Enter a tag name for the bulk tag action.");
                    return;
                }
                serde_json::json!({ "tag": param.trim() })
            }
            "assign" => {
                let remote = users.with(|rows| {
                    param.trim().parse::<i64>().ok().and_then(|local| {
                        rows.iter()
                            .find(|(l, _, _)| *l == local)
                            .map(|(_, r, _)| *r)
                    })
                });
                let Some(remote) = remote else {
                    toasts::error("Pick a user for the bulk assign action.");
                    return;
                };
                serde_json::json!({ "userId": remote })
            }
            "status" => {
                if param.is_empty() {
                    toasts::error("Pick a status for the bulk status action.");
                    return;
                }
                serde_json::json!({ "status": param })
            }
            _ => serde_json::json!({}),
        };
        if action == "close" {
            let confirmed = web_sys::window()
                .and_then(|w| {
                    w.confirm_with_message(&format!(
                        "Close {} selected conversation(s)?",
                        ids.len()
                    ))
                    .ok()
                })
                .unwrap_or(false);
            if !confirmed {
                return;
            }
        }
        let open_in_selection = selected_id
            .get_untracked()
            .is_some_and(|open| ids.contains(&open));
        let filters = filters;
        let selected_id = selected_id;
        let selected_ids = selected_ids;
        wasm_bindgen_futures::spawn_local(async move {
            let body = serde_json::json!({
                "conversationIds": ids,
                "action": action,
                "params": params,
            });
            match crate::api::post_json::<serde_json::Value>("/api/conversations/bulk", Some(&body))
                .await
            {
                Ok(r) => {
                    let ok = r.get("ok").and_then(|v| v.as_bool()).unwrap_or(false);
                    let message = r
                        .get("message")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Bulk action failed")
                        .to_string();
                    if ok {
                        if open_in_selection {
                            if let Some(id) = selected_id.get() {
                                selected_id.set(None);
                                selected_id.set(Some(id));
                            }
                        }
                        let current = filters.get();
                        filters.set(InboxFilters::default());
                        filters.set(current);
                    }
                    let kind = if ok {
                        crate::toasts::ToastKind::Success
                    } else {
                        crate::toasts::ToastKind::Error
                    };
                    crate::toasts::push(kind, message, None);
                }
                Err(e) => toasts::error(e),
            }
            selected_ids.set(Vec::new());
        });
    };

    let run_ai_draft = move || {
        let Some(id) = selected_id.get_untracked() else {
            return;
        };
        if ai_drafting.get_untracked() {
            return;
        }
        ai_drafting.set(true);
        let body = serde_json::json!({ "mode": "verified_answer" });
        let ai_drafting = ai_drafting;
        let composer_body = composer_body;
        wasm_bindgen_futures::spawn_local(async move {
            match crate::api::post_json::<serde_json::Value>(
                &format!("/api/ai/draft/{id}"),
                Some(&body),
            )
            .await
            {
                Ok(r) => {
                    if r.get("ok").and_then(|x| x.as_bool()).unwrap_or(false) {
                        if let Some(text) = r
                            .get("draft")
                            .and_then(|d| d.get("text"))
                            .and_then(|x| x.as_str())
                        {
                            composer_body.set(text.to_string());
                            toasts::success("AI draft inserted — review before sending.");
                        } else {
                            toasts::error("The AI returned no draft text.");
                        }
                    } else {
                        let reason = r
                            .get("error")
                            .and_then(|x| x.as_str())
                            .or_else(|| r.get("message").and_then(|x| x.as_str()))
                            .unwrap_or("Draft failed");
                        toasts::error(reason);
                    }
                }
                Err(e) => toasts::error(e),
            }
            ai_drafting.set(false);
        });
    };

    let save_current_view = move || {
        let name = view_name.get_untracked();
        if name.trim().is_empty() {
            toasts::error("Enter a name for the view.");
            return;
        }
        let f = filters.get_untracked();
        let mut conditions: Vec<serde_json::Value> = Vec::new();
        if let Some(status) = f.status.as_deref() {
            conditions.push(serde_json::json!({
                "kind": "status", "statuses": [status]
            }));
        }
        if let Some(mailbox_id) = f.mailbox_id {
            conditions.push(serde_json::json!({
                "kind": "mailbox", "mailboxLocalIds": [mailbox_id]
            }));
        }
        if let Some(tag) = f.tag.as_deref() {
            conditions.push(serde_json::json!({ "kind": "tag", "tag": tag }));
        }
        if let Some(channel) = f.channel.as_deref() {
            conditions.push(serde_json::json!({
                "kind": "channel", "channels": [channel]
            }));
        }
        let definition = serde_json::json!({
            "combinator": "all",
            "conditions": conditions,
        });
        let saved_views = saved_views;
        let filters = filters;
        let view_name = view_name;
        let saving_view = saving_view;
        wasm_bindgen_futures::spawn_local(async move {
            let payload = serde_json::json!({
                "name": name.trim(),
                "definition": definition,
            });
            match crate::api::post_json::<serde_json::Value>("/api/inbox-views", Some(&payload))
                .await
            {
                Ok(r) => {
                    let ok = r.get("ok").and_then(|v| v.as_bool()).unwrap_or(false);
                    let message = r
                        .get("message")
                        .and_then(|v| v.as_str())
                        .unwrap_or(if ok { "View saved." } else { "Save failed" })
                        .to_string();
                    if ok {
                        if let Some(id) = r.pointer("/view/id").and_then(|x| x.as_i64()) {
                            filters.update(|f| {
                                f.saved_view_id = Some(id);
                                f.reset_page();
                            });
                        }
                        view_name.set(String::new());
                        saving_view.set(false);
                        if let Ok(v) =
                            crate::api::get_json::<serde_json::Value>("/api/inbox-views").await
                        {
                            let arr = v
                                .get("views")
                                .and_then(|x| x.as_array())
                                .cloned()
                                .unwrap_or_default();
                            saved_views.set(
                                arr.iter()
                                    .map(|v| SavedView {
                                        id: v.get("id").and_then(|x| x.as_i64()),
                                        name: v
                                            .get("name")
                                            .and_then(|x| x.as_str())
                                            .unwrap_or("")
                                            .to_string(),
                                        mailbox_id: v.get("mailbox_id").and_then(|x| x.as_i64()),
                                    })
                                    .collect(),
                            );
                        }
                    }
                    let kind = if ok {
                        crate::toasts::ToastKind::Success
                    } else {
                        crate::toasts::ToastKind::Error
                    };
                    crate::toasts::push(kind, message, None);
                }
                Err(e) => toasts::error(e),
            }
        });
    };

    let delete_saved_view = move |view_id: i64| {
        let saved_views = saved_views;
        let filters = filters;
        wasm_bindgen_futures::spawn_local(async move {
            let path = format!("/api/inbox-views/{view_id}");
            match crate::api::delete_json::<serde_json::Value>(&path).await {
                Ok(r) => {
                    let ok = r.get("ok").and_then(|v| v.as_bool()).unwrap_or(false);
                    let message = r
                        .get("message")
                        .and_then(|v| v.as_str())
                        .unwrap_or(if ok { "View deleted." } else { "Delete failed" })
                        .to_string();
                    if ok {
                        if filters.with_untracked(|f| f.saved_view_id == Some(view_id)) {
                            filters.update(|f| {
                                f.saved_view_id = None;
                                f.reset_page();
                            });
                        }
                        saved_views.update(|views| {
                            views.retain(|v| v.id != Some(view_id));
                        });
                    }
                    let kind = if ok {
                        crate::toasts::ToastKind::Success
                    } else {
                        crate::toasts::ToastKind::Error
                    };
                    crate::toasts::push(kind, message, None);
                }
                Err(e) => toasts::error(e),
            }
        });
    };

    view! {
        <div class="spp-inbox">
            <aside class="spp-inbox__list">
                <div class="spp-inbox__list-header">
                    <h2 class="spp-inbox__title">"Inbox"</h2>
                    <span class="spp-inbox__count">
                        {move || format!("{} conversations", total.get())}
                    </span>
                </div>

                <div class="spp-inbox__filters">
                    <select
                        class="spp-inbox__filter"
                        prop:value=move || filters.with(|f| f.status.clone().unwrap_or_default())
                        on:change=move |ev| {
                            let val = event_target_value(&ev);
                            let val = if val.is_empty() { None } else { Some(val) };
                            filters.update(|f| {
                                f.status = val;
                                f.reset_page();
                            });
                            sync_url.with_value(|f| f());
                            list_loading.set(true);
                        }
                    >
                        <option value="">"All statuses"</option>
                        <option value="active">"Active"</option>
                        <option value="pending">"Pending"</option>
                        <option value="closed">"Closed"</option>
                        <option value="spam">"Spam"</option>
                    </select>
                    <select
                        class="spp-inbox__filter"
                        prop:value=move || {
                            filters.with(|f| f.mailbox_id.map(|id| id.to_string()).unwrap_or_default())
                        }
                        on:change=move |ev| {
                            let val = event_target_value(&ev);
                            let id = if val.is_empty() { None } else { val.parse::<i64>().ok() };
                            filters.update(|f| {
                                f.mailbox_id = id;
                                f.reset_page();
                            });
                            sync_url.with_value(|f| f());
                            list_loading.set(true);
                        }
                    >
                        <option value="">"All mailboxes"</option>
                        {move || {
                            mailboxes.with(|rows| {
                                rows.iter()
                                    .map(|(id, name)| {
                                        view! {
                                            <option value={id.to_string()}>{name.clone()}</option>
                                        }
                                    })
                                    .collect::<Vec<_>>()
                            })
                        }}
                    </select>
                    <select
                        class="spp-inbox__filter"
                        prop:value=move || filters.with(|f| f.channel.clone().unwrap_or_default())
                        on:change=move |ev| {
                            let val = event_target_value(&ev);
                            let val = if val.is_empty() { None } else { Some(val) };
                            filters.update(|f| {
                                f.channel = val;
                                f.reset_page();
                            });
                            sync_url.with_value(|f| f());
                            list_loading.set(true);
                        }
                    >
                        <option value="">"All channels"</option>
                        <option value="email">"Email"</option>
                        <option value="chat">"Chat (Beacon)"</option>
                    </select>
                    <select
                        class="spp-inbox__filter"
                        prop:value=move || filters.with(|f| f.tag.clone().unwrap_or_default())
                        on:change=move |ev| {
                            let val = event_target_value(&ev);
                            let val = if val.is_empty() { None } else { Some(val) };
                            filters.update(|f| {
                                f.tag = val;
                                f.reset_page();
                            });
                            sync_url.with_value(|f| f());
                            list_loading.set(true);
                        }
                    >
                        <option value="">"All tags"</option>
                        {move || {
                            tag_names.with(|names| {
                                names.iter()
                                    .map(|name| {
                                        view! { <option value={name.clone()}>{name.clone()}</option> }
                                    })
                                    .collect::<Vec<_>>()
                            })
                        }}
                    </select>
                    <select
                        class="spp-inbox__filter"
                        on:change=move |ev| {
                            let val = event_target_value(&ev);
                            let val = if val.is_empty() { None } else { Some(val) };
                            filters.update(|f| {
                                f.priority = val;
                                f.reset_page();
                            });
                            list_loading.set(true);
                        }
                    >
                        <option value="">"All priorities"</option>
                        <option value="low">"Low"</option>
                        <option value="normal">"Normal"</option>
                        <option value="high">"High"</option>
                        <option value="urgent">"Urgent"</option>
                    </select>
                    <input
                        class="spp-inbox__search"
                        type="text"
                        placeholder="Search subject + preview..."
                        on:input=move |ev| {
                            let val = event_target_value(&ev);
                            let val = if val.is_empty() { None } else { Some(val) };
                            filters.update(|f| {
                                f.query = val;
                                f.reset_page();
                            });
                            list_loading.set(true);
                        }
                    />
                </div>

                <Show when=move || filters.with(|f| f.tag.is_some() || f.channel.is_some()) fallback=|| ()>
                    <div class="spp-inbox__url-chips">
                        {move || {
                            let f = filters.get();
                            let mut chips: Vec<leptos::View> = Vec::new();
                            if let Some(channel) = f.channel {
                                let clear = {
                                    let filters = filters;
                                    let sync_url = sync_url;
                                    move |_| {
                                        filters.update(|f| f.channel = None);
                                        sync_url.with_value(|f| f());
                                        list_loading.set(true);
                                    }
                                };
                                let label = if channel == "chat" {
                                    "channel: chat (Beacon)".to_string()
                                } else {
                                    format!("channel: {channel}")
                                };
                                chips.push(
                                    view! {
                                        <span class="spp-chip">
                                            <span class="spp-chip__label">{label}</span>
                                            <button class="spp-chip__clear" aria-label="Clear channel filter" on:click=clear>"\u{d7}"</button>
                                        </span>
                                    }.into_view(),
                                );
                            }
                            if let Some(tag) = f.tag {
                                let clear = {
                                    let filters = filters;
                                    let sync_url = sync_url;
                                    move |_| {
                                        filters.update(|f| f.tag = None);
                                        sync_url.with_value(|f| f());
                                        list_loading.set(true);
                                    }
                                };
                                chips.push(
                                    view! {
                                        <span class="spp-chip">
                                            <span class="spp-chip__label">{format!("tag: {tag}")}</span>
                                            <button class="spp-chip__clear" aria-label="Clear tag filter" on:click=clear>"\u{d7}"</button>
                                        </span>
                                    }.into_view(),
                                );
                            }
                            chips
                        }}
                    </div>
                </Show>

                <div class="spp-inbox__saved-views">
                    <select
                        class="spp-inbox__filter"
                        prop:value=move || {
                            filters.with(|f| f.saved_view_id.map(|id| id.to_string()).unwrap_or_default())
                        }
                        on:change=move |ev| {
                            let val = event_target_value(&ev);
                            let id = if val.is_empty() { None } else { val.parse::<i64>().ok() };
                            filters.update(|f| {
                                f.saved_view_id = id;
                                f.reset_page();
                            });
                            sync_url.with_value(|f| f());
                            list_loading.set(true);
                        }
                    >
                        <option value="">"— standard filters —"</option>
                        {move || {
                            saved_views.with(|views| {
                                views.iter()
                                    .map(|v| {
                                        let id = v.id.unwrap_or(0).to_string();
                                        let name = v.name.clone();
                                        view! {
                                            <option value=id>{name.clone()}</option>
                                        }
                                    })
                                    .collect::<Vec<_>>()
                            })
                        }}
                    </select>
                    <button
                        class="spp-button spp-button--ghost spp-button--tiny"
                        type="button"
                        on:click=move |_| saving_view.update(|s| *s = !*s)
                    >
                        "Save current…"
                    </button>
                    <Show when=move || {
                        filters.with(|f| f.saved_view_id.is_some())
                            && !saved_views.with(|v| v.is_empty())
                    } fallback=|| ()>
                        <button
                            class="spp-button spp-button--ghost spp-button--tiny"
                            type="button"
                            on:click=move |_| {
                                if let Some(view_id) = filters.with_untracked(|f| f.saved_view_id) {
                                    delete_saved_view(view_id);
                                }
                            }
                        >
                            "Delete view"
                        </button>
                    </Show>
                    <Show when=move || saving_view.get() fallback=|| ()>
                        <div class="spp-flex spp-mt-8">
                            <input
                                class="spp-input"
                                type="text"
                                placeholder="View name"
                                maxlength=100
                                prop:value=view_name
                                on:input=move |ev| view_name.set(event_target_value(&ev))
                            />
                            <button
                                class="spp-button spp-button--primary spp-button--small"
                                type="button"
                                on:click=move |_| save_current_view()
                            >
                                "Save"
                            </button>
                        </div>
                    </Show>
                </div>

                <div class="spp-inbox__list-body">
                    <Show when=move || list_loading.get() fallback=|| ()>
                        <LoadingState />
                    </Show>
                    <Show when=move || list_error.get().is_some() fallback=|| ()>
                        <div class="spp-state spp-state--error">
                            <span class="spp-state__icon" aria-hidden="true">"⚠"</span>
                            <p class="spp-state__body">
                                {move || list_error.get().unwrap_or_default()}
                            </p>
                        </div>
                    </Show>
                    <Show
                        when=move || !list_loading.get() && list_error.get().is_none()
                        fallback=|| ()
                    >
                        <Show
                            when=move || !conversations.with(|c| c.is_empty())
                            fallback=move || {
                                view! {
                                    <EmptyState message="No conversations match the current filters. Try clearing filters or syncing with Help Scout." />
                                }
                            }
                        >
                            {move || conversations.with(|items| {
                                items.iter().map(|item| {
                                    let id = item.id;
                                    let is_selected = move || selected_id.get() == Some(id);
                                    let is_checked = move || selected_ids.with(|ids| ids.contains(&id));
                                    view! {
                                        <div
                                            class="spp-inbox__item"
                                            class:is-selected=is_selected
                                            on:click=move |_| {
                                                selected_id.set(Some(id));
                                            }
                                        >
                                            <input
                                                type="checkbox"
                                                class="spp-inbox__item-checkbox"
                                                checked=is_checked
                                                on:click=move |ev| {
                                                    ev.stop_propagation();
                                                    selected_ids.update(|ids| {
                                                        if ids.contains(&id) {
                                                            ids.retain(|x| *x != id);
                                                        } else {
                                                            ids.push(id);
                                                        }
                                                    });
                                                }
                                            />
                                            <div class="spp-inbox__item-body">
                                                <div class="spp-inbox__item-subject">
                                                    {item.subject.clone().unwrap_or_else(|| format!("#{}", item.number))}
                                                </div>
                                                <div class="spp-inbox__item-preview">
                                                    {item.preview.clone().unwrap_or_default()}
                                                </div>
                                                <div class="spp-inbox__item-meta">
                                                    <span class="spp-inbox__item-status">
                                                        {item.status.clone()}
                                                    </span>
                                                    <span class="spp-inbox__item-priority">
                                                        {item.priority.clone().unwrap_or_default()}
                                                    </span>
                                                    <span class="spp-inbox__item-customer">
                                                        {item.customer_name.clone().unwrap_or_else(|| format!("#{}", item.customer_id))}
                                                    </span>
                                                </div>
                                                <div class="spp-inbox__item-response-state">
                                                    {item.response_state.clone().unwrap_or_default()}
                                                </div>
                                            </div>
                                        </div>
                                    }
                                }).collect::<Vec<_>>()
                            })}
                        </Show>
                    </Show>
                </div>

                <Show when=move || !selected_ids.with(|ids| ids.is_empty()) fallback=|| ()>
                    <div class="spp-inbox__bulk-actions">
                        <span>
                            {move || format!("{} selected", selected_ids.with(|ids| ids.len()))}
                        </span>
                        <select
                            class="spp-inbox__filter"
                            prop:value=move || bulk_action.get()
                            on:change=move |ev| {
                                bulk_action.set(event_target_value(&ev));
                                bulk_param.set(String::new());
                            }
                        >
                            <option value="close">"Close"</option>
                            <option value="status">"Set status"</option>
                            <option value="assign">"Assign to"</option>
                            <option value="unassign">"Unassign"</option>
                            <option value="tag">"Add tag"</option>
                            <option value="untag">"Remove tag"</option>
                        </select>
                        <Show when=move || bulk_action.get() == "status" fallback=|| ()>
                            <select
                                class="spp-inbox__filter"
                                prop:value=move || bulk_param.get()
                                on:change=move |ev| bulk_param.set(event_target_value(&ev))
                            >
                                <option value="">"— status —"</option>
                                <option value="active">"Active"</option>
                                <option value="pending">"Pending"</option>
                                <option value="spam">"Spam"</option>
                            </select>
                        </Show>
                        <Show when=move || bulk_action.get() == "assign" fallback=|| ()>
                            <select
                                class="spp-inbox__filter"
                                prop:value=move || bulk_param.get()
                                on:change=move |ev| bulk_param.set(event_target_value(&ev))
                            >
                                <option value="">"— user —"</option>
                                {move || {
                                    users.with(|rows| {
                                        rows.iter()
                                            .map(|(local, _remote, name)| {
                                                view! {
                                                    <option value={local.to_string()}>{name.clone()}</option>
                                                }
                                            })
                                            .collect::<Vec<_>>()
                                    })
                                }}
                            </select>
                        </Show>
                        <Show
                            when=move || bulk_action.get() == "tag" || bulk_action.get() == "untag"
                            fallback=|| ()
                        >
                            <input
                                class="spp-inbox__search"
                                type="text"
                                placeholder="Tag name"
                                prop:value=bulk_param
                                on:input=move |ev| bulk_param.set(event_target_value(&ev))
                            />
                        </Show>
                        <button
                            class="spp-button spp-button--primary spp-button--small"
                            on:click=move |_| apply_bulk()
                        >
                            "Apply"
                        </button>
                        <button
                            class="spp-button spp-button--ghost"
                            on:click=move |_| {
                                selected_ids.set(Vec::new());
                            }
                        >
                            "Clear"
                        </button>
                    </div>
                </Show>

                <div class="spp-inbox__pagination">
                    <button
                        class="spp-button spp-button--ghost spp-button--small"
                        type="button"
                        disabled=move || filters.with(|f| f.page <= 1)
                        on:click=move |_| {
                            filters.update(|f| {
                                if f.page > 1 {
                                    f.page -= 1;
                                }
                            });
                            sync_url.with_value(|f| f());
                            list_loading.set(true);
                        }
                    >
                        "← Prev"
                    </button>
                    <span class="spp-muted spp-text-xs">
                        {move || {
                            let page = filters.with(|f| f.page).max(1);
                            let pages = total.get().div_ceil(50);
                            format!("Page {page} of {}", pages.max(1))
                        }}
                    </span>
                    <button
                        class="spp-button spp-button--ghost spp-button--small"
                        type="button"
                        disabled=move || {
                            let page = filters.with(|f| f.page);
                            let pages = total.get().div_ceil(50);
                            page >= pages.max(1)
                        }
                        on:click=move |_| {
                            filters.update(|f| f.page += 1);
                            sync_url.with_value(|f| f());
                            list_loading.set(true);
                        }
                    >
                        "Next →"
                    </button>
                </div>
            </aside>

            <main class="spp-inbox__detail">
                <Show
                    when=move || selected_id.get().is_some()
                    fallback=|| {
                        view! {
                            <EmptyState message="Select a conversation from the list to view its details." />
                        }
                    }
                >
                    <Show when=move || detail_loading.get() fallback=|| ()>
                        <LoadingState />
                    </Show>
                    <Show when=move || detail_error.get().is_some() fallback=|| ()>
                        <div class="spp-state spp-state--error">
                            <span class="spp-state__icon" aria-hidden="true">"⚠"</span>
                            <p class="spp-state__body">
                                {move || detail_error.get().unwrap_or_default()}
                            </p>
                        </div>
                    </Show>
                    <Show
                        when=move || !detail_loading.get() && detail_error.get().is_none()
                        fallback=|| ()
                    >
                        {move || detail.with(|d| {
                            let d = match d {
                                Some(d) => d.clone(),
                                None => return view! { <div></div> }.into_view(),
                            };
                            let d_status = d.status.clone();
                            let d_priority = d.priority.clone();
                            let d_response_state = d.response_state.clone();
                            let d_assignee_id = d.assignee_id.unwrap_or(0).to_string();
                            let d_thread = d.thread.clone();
                            let d_thread_clone = d_thread.clone();
                            let d_id = d.id;
                            let d_closed = d.status == "closed";
                            let d_customer_opt = if d.customer_id > 0 { Some(d.customer_id) } else { None };
                            view! {
                                <div class="spp-inbox__detail-content">
                                    <header class="spp-inbox__detail-header">
                                        <h3 class="spp-inbox__detail-subject">
                                            {move || {
                                                if editing_subject.get() {
                                                    view! {
                                                        <div class="spp-flex">
                                                            <input
                                                                class="spp-input"
                                                                maxlength=200
                                                                prop:value=subject_draft
                                                                on:input=move |ev| {
                                                                    subject_draft.set(event_target_value(&ev));
                                                                }
                                                            />
                                                            <button
                                                                class="spp-button spp-button--primary spp-button--small"
                                                                type="button"
                                                                on:click=move |_| save_subject()
                                                            >
                                                                "Save"
                                                            </button>
                                                            <button
                                                                class="spp-button spp-button--ghost spp-button--small"
                                                                type="button"
                                                                on:click=move |_| editing_subject.set(false)
                                                            >
                                                                "Cancel"
                                                            </button>
                                                        </div>
                                                    }.into_view()
                                                } else {
                                                    view! {
                                                        {move || {
                                                            detail.with(|d| {
                                                                d.as_ref()
                                                                    .map(|d| {
                                                                        d.subject.clone()
                                                                            .unwrap_or_else(|| format!("#{}", d.number))
                                                                    })
                                                                    .unwrap_or_default()
                                                            })
                                                        }}
                                                        <button
                                                            class="spp-button spp-button--ghost spp-button--tiny"
                                                            type="button"
                                                            aria-label="Edit subject"
                                                            on:click=move |_| {
                                                                let subject = detail.with_untracked(|d| {
                                                                    d.as_ref()
                                                                        .and_then(|d| d.subject.clone())
                                                                        .unwrap_or_default()
                                                                });
                                                                subject_draft.set(subject);
                                                                editing_subject.set(true);
                                                            }
                                                        >
                                                            "Edit"
                                                        </button>
                                                    }.into_view()
                                                }
                                            }}
                                        </h3>
                                        <div class="spp-inbox__detail-meta">
                                            <span class="spp-badge spp-badge--status">
                                                {d_status.clone()}
                                            </span>
                                            <span class="spp-badge spp-badge--priority">
                                                {d_priority.clone().unwrap_or_default()}
                                            </span>
                                            <span class="spp-badge spp-badge--response-state">
                                                {d_response_state.clone().unwrap_or_default()}
                                            </span>
                                        </div>
                                    </header>

                                    <div class="spp-inbox__controls">
                                        <label>"Status:"</label>
                                        <select
                                            class="spp-inbox__control"
                                            value=d_status.clone()
                                            on:change=move |ev| {
                                                change_status(event_target_value(&ev));
                                            }
                                        >
                                            <option value="active">"Active"</option>
                                            <option value="pending">"Pending"</option>
                                            <option value="closed">"Closed"</option>
                                            <option value="spam">"Spam"</option>
                                        </select>

                                        <label>"Assignee:"</label>
                                        <select
                                            class="spp-inbox__control"
                                            value=d_assignee_id.clone()
                                            on:change=move |ev| {
                                                let val = event_target_value(&ev);
                                                let remote: i64 = val.parse().unwrap_or(0);
                                                let assignee = if remote == 0 { None } else { Some(remote) };
                                                assign_to(assignee);
                                            }
                                        >
                                            <option value="0">"Unassigned"</option>
                                            {move || {
                                                users.with(|rows| {
                                                    rows.iter()
                                                        .map(|(_local, remote, name)| {
                                                            view! {
                                                                <option value={remote.to_string()}>{name.clone()}</option>
                                                            }
                                                        })
                                                        .collect::<Vec<_>>()
                                                })
                                            }}
                                        </select>

                                        <label>"Priority:"</label>
                                        <select
                                            class="spp-inbox__control"
                                            value=d_priority.clone().unwrap_or_default()
                                            on:change=move |ev| {
                                                change_priority(event_target_value(&ev));
                                            }
                                        >
                                            <option value="">"—"</option>
                                            <option value="low">"Low"</option>
                                            <option value="normal">"Normal"</option>
                                            <option value="high">"High"</option>
                                            <option value="urgent">"Urgent"</option>
                                        </select>

                                        <label>"Snooze until:"</label>
                                        <input
                                            class="spp-inbox__control"
                                            type="datetime-local"
                                            prop:value=snooze_until
                                            on:input=move |ev| snooze_until.set(event_target_value(&ev))
                                        />
                                        <button
                                            class="spp-button spp-button--ghost spp-button--small"
                                            type="button"
                                            on:click=move |_| do_snooze()
                                        >
                                            "Snooze"
                                        </button>
                                        <button
                                            class="spp-button spp-button--ghost spp-button--small"
                                            type="button"
                                            on:click=move |_| do_unsnooze()
                                        >
                                            "Unsnooze"
                                        </button>
                                    </div>

                                    <SideThreadsPanel conversation_id=d_id />

                                    <div class="spp-inbox__thread">
                                        {if d_thread_clone.is_empty() {
                                            view! {
                                                <EmptyState message="No thread entries yet. Send a reply or note to start the conversation." />
                                            }.into_view()
                                        } else {
                                            d_thread_clone.iter().map(|entry| {
                                                let class = match entry.thread_type.as_str() {
                                                    "customer_message" => "spp-thread-entry spp-thread-entry--customer",
                                                    "reply" => "spp-thread-entry spp-thread-entry--reply",
                                                    "note" => "spp-thread-entry spp-thread-entry--note",
                                                    "system" => "spp-thread-entry spp-thread-entry--system",
                                                    _ => "spp-thread-entry",
                                                };
                                                let entry_type = entry.thread_type.clone();
                                                let entry_actor = entry.actor_name.clone().unwrap_or_else(|| entry.actor_type.clone());
                                                let entry_time = entry.created_at.clone();
                                                let entry_body = entry.body.clone();
                                                let is_draft = entry.state.as_deref() == Some("draft");
                                                let entry_id = entry.id;
                                                view! {
                                                    <div class={class}>
                                                        <div class="spp-thread-entry__header">
                                                            <span class="spp-thread-entry__type">
                                                                {entry_type}
                                                            </span>
                                                            <span class="spp-thread-entry__actor">
                                                                {entry_actor}
                                                            </span>
                                                            <span class="spp-thread-entry__time">
                                                                {entry_time}
                                                            </span>
                                                            {if is_draft {
                                                                view! {
                                                                    <span class="spp-badge spp-badge--warn">"draft"</span>
                                                                }.into_view()
                                                            } else {
                                                                ().into_view()
                                                            }}
                                                        </div>
                                                        <SafeHtml html=entry_body fallback_text=String::new() />
                                                        {if is_draft {
                                                            view! {
                                                                <div class="spp-flex spp-mt-8">
                                                                    <button
                                                                        class="spp-button spp-button--ghost spp-button--tiny"
                                                                        type="button"
                                                                        on:click=move |_| schedule_draft(entry_id)
                                                                    >
                                                                        "Schedule send"
                                                                    </button>
                                                                    <button
                                                                        class="spp-button spp-button--ghost spp-button--tiny"
                                                                        type="button"
                                                                        on:click=move |_| cancel_schedule(entry_id)
                                                                    >
                                                                        "Cancel schedule"
                                                                    </button>
                                                                </div>
                                                            }.into_view()
                                                        } else {
                                                            ().into_view()
                                                        }}
                                                    </div>
                                                }
                                            }).collect::<Vec<_>>().into_view()
                                        }}
                                    </div>

                                    <QaPanel conversation_id=d_id closed=d_closed />
                                    <TranslationPanel conversation_id=d_id />
                                    <MemoryPanel
                                        customer_id=d_customer_opt
                                        conversation_id=d_id
                                    />

                                    <div class="spp-inbox__composer">
                                        <div class="spp-inbox__composer-tabs">
                                            <button
                                                class="spp-inbox__composer-tab"
                                                class:is-active=move || composer_mode.get() == ComposerMode::Reply
                                                on:click=move |_| composer_mode.set(ComposerMode::Reply)
                                            >
                                                "Reply"
                                            </button>
                                            <button
                                                class="spp-inbox__composer-tab"
                                                class:is-active=move || composer_mode.get() == ComposerMode::Note
                                                on:click=move |_| composer_mode.set(ComposerMode::Note)
                                            >
                                                "Internal note"
                                            </button>
                                        </div>
                                        <Show
                                            when=move || composer_mode.get() == ComposerMode::Reply
                                            fallback=move || view! {
                                                <MentionTextarea
                                                    value=composer_body
                                                    placeholder="Type an internal note (visible only to your team) — @ to mention"
                                                    rows=6
                                                    submit_on_enter=true
                                                    submit_key_code=13
                                                    on_submit={std::rc::Rc::new(request_submit) as std::rc::Rc<dyn Fn()>}
                                                />
                                            }
                                        >
                                            <MentionTextarea
                                                value=composer_body
                                                placeholder="Type your reply to the customer... (Ctrl+Enter sends)"
                                                rows=6
                                                submit_on_enter=true
                                                submit_key_code=13
                                                on_submit={std::rc::Rc::new(request_submit) as std::rc::Rc<dyn Fn()>}
                                            />
                                        </Show>
                                        <Show
                                            when=move || composer_mode.get() == ComposerMode::Reply
                                            fallback=|| ()
                                        >
                                            <div class="spp-inbox__composer-recipients">
                                                <input
                                                    class="spp-input"
                                                    type="text"
                                                    placeholder="Cc (comma-separated emails)"
                                                    prop:value=composer_cc
                                                    on:input=move |ev| composer_cc.set(event_target_value(&ev))
                                                />
                                                <input
                                                    class="spp-input"
                                                    type="text"
                                                    placeholder="Bcc (comma-separated emails)"
                                                    prop:value=composer_bcc
                                                    on:input=move |ev| composer_bcc.set(event_target_value(&ev))
                                                />
                                            </div>
                                            <Show when=move || !saved_replies.with(|r| r.is_empty()) fallback=|| ()>
                                                <div class="spp-flex spp-mt-8">
                                                    <select
                                                        class="spp-inbox__filter"
                                                        aria-label="Insert saved reply"
                                                        prop:value=move || saved_reply_pick.get()
                                                        on:change=move |ev| {
                                                            saved_reply_pick.set(event_target_value(&ev));
                                                        }
                                                    >
                                                        <option value="">"— insert saved reply —"</option>
                                                        {move || {
                                                            saved_replies.with(|rows| {
                                                                rows.iter()
                                                                    .map(|(name, _text)| {
                                                                        view! {
                                                                            <option value={name.clone()}>{name.clone()}</option>
                                                                        }
                                                                    })
                                                                    .collect::<Vec<_>>()
                                                            })
                                                        }}
                                                    </select>
                                                    <button
                                                        class="spp-button spp-button--ghost spp-button--small"
                                                        type="button"
                                                        on:click=move |_| {
                                                            let pick = saved_reply_pick.get_untracked();
                                                            if pick.is_empty() {
                                                                return;
                                                            }
                                                            if let Some((_name, text)) = saved_replies
                                                                .with_untracked(|rows| {
                                                                    rows.iter()
                                                                        .find(|(name, _)| *name == pick)
                                                                        .cloned()
                                                                })
                                                            {
                                                                let current = composer_body.get_untracked();
                                                                if current.is_empty() {
                                                                    composer_body.set(text);
                                                                } else {
                                                                    composer_body
                                                                        .set(format!("{current}\n\n{text}"));
                                                                }
                                                                toasts::success(
                                                                    "Saved reply inserted — review before sending.",
                                                                );
                                                            }
                                                        }
                                                    >
                                                        "Insert"
                                                    </button>
                                                </div>
                                            </Show>
                                        </Show>
                                        <div class="spp-inbox__composer-actions">
                                            <button
                                                class="spp-button spp-button--ghost spp-button--small"
                                                type="button"
                                                disabled=move || ai_drafting.get()
                                                on:click=move |_| run_ai_draft()
                                            >
                                                {move || if ai_drafting.get() { "Drafting…" } else { "AI draft" }.to_string()}
                                            </button>
                                            <button
                                                class="spp-button"
                                                on:click=move |_| request_submit()
                                            >
                                                {move || match composer_mode.get() {
                                                    ComposerMode::Reply => "Send reply",
                                                    ComposerMode::Note => "Add note",
                                                }}
                                            </button>
                                        </div>
                                        <Show when=move || composer_mode.get() == ComposerMode::Reply fallback=|| ()>
                                            <CoachingPanel conversation_id=d_id draft=composer_body />
                                        </Show>
                                        <Show when=move || composer_error.get().is_some() fallback=|| ()>
                                            <div class="spp-state spp-state--error">
                                                {move || composer_error.get().unwrap_or_default()}
                                            </div>
                                        </Show>
                                        <Show when=move || confirm_send.get() fallback=|| ()>
                                            {move || {
                                                let email = detail.get()
                                                    .and_then(|d| d.customer_email)
                                                    .unwrap_or_else(|| "the customer".to_string());
                                                let on_confirm = std::sync::Arc::new(move || {
                                                    confirm_send.set(false);
                                                    send_composer();
                                                });
                                                let on_cancel = std::sync::Arc::new(move || confirm_send.set(false));
                                                view! {
                                                    <ConfirmDialog
                                                        title="Send reply to customer"
                                                        message=format!(
                                                            "Send this reply to {email} via Help Scout? This is a customer-visible action."
                                                        )
                                                        confirm_label="Send reply"
                                                        on_confirm
                                                        on_cancel
                                                    />
                                                }
                                            }}
                                        </Show>
                                    </div>
                                </div>
                            }.into_view()
                        })}
                    </Show>
                </Show>
            </main>

            <aside class="spp-inbox__context">
                <Show
                    when=move || selected_id.get().is_some()
                    fallback=|| {
                        view! {
                            <EmptyState message="Customer context will appear here when a conversation is selected." />
                        }
                    }
                >
                    {move || {
                        let conv_id = selected_id.get().unwrap_or(0);
                        view! {
                            <div class="spp-inbox__context-content">
                                <div class="spp-inbox__context-tabs">
                                    <div class="spp-inbox__context-tabbar">
                                        <button
                                            class="spp-inbox__context-tab"
                                            class:is-active=move || context_tab.get() == ContextTab::Ai
                                            on:click=move |_| context_tab.set(ContextTab::Ai)
                                        >
                                            "AI"
                                        </button>
                                        <button
                                            class="spp-inbox__context-tab"
                                            class:is-active=move || context_tab.get() == ContextTab::Customer
                                            on:click=move |_| context_tab.set(ContextTab::Customer)
                                        >
                                            "Customer"
                                        </button>
                                        <button
                                            class="spp-inbox__context-tab"
                                            class:is-active=move || context_tab.get() == ContextTab::Copilot
                                            on:click=move |_| context_tab.set(ContextTab::Copilot)
                                            title="Ask the Local Copilot about this ticket (read-only, evidence-cited)"
                                        >
                                            "Copilot"
                                        </button>
                                        <button
                                            class="spp-inbox__context-tab"
                                            class:is-active=move || context_tab.get() == ContextTab::Activity
                                            on:click=move |_| context_tab.set(ContextTab::Activity)
                                            title="The event timeline and the audit trail for this conversation"
                                        >
                                            "Activity"
                                        </button>
                                    </div>
                                    <button
                                        class="spp-button spp-button--ghost spp-button--small"
                                        aria-label="Hide context pane"
                                        on:click=move |_| context_open.set(false)
                                    >
                                        "Hide"
                                    </button>
                                </div>

                                <Show
                                    when=move || !context_open.get()
                                    fallback=|| ()
                                >
                                    <button
                                        class="spp-button spp-button--small spp-inbox__context-reopen"
                                        on:click=move |_| context_open.set(true)
                                    >
                                        "Show context"
                                    </button>
                                </Show>

                                <Show
                                    when=move || context_open.get() && context_tab.get() == ContextTab::Ai
                                    fallback=|| ()
                                >
                                    <div class="spp-inbox__context-body">
                                        <AttributeSnapshotCard conversation_id=conv_id />

                                        <div class="spp-ai-sidebar-section">
                                            <h4>"What is the customer asking?"</h4>
                                            <button
                                                class="spp-button spp-button--small"
                                                on:click=move |_| run_analyze()
                                                disabled=move || ai_analyzing.get()
                                            >
                                                {move || if ai_analyzing.get() { "Analyzing…" } else { "Analyze" }.to_string()}
                                            </button>
                                            {move || {
                                                match ai_analysis.get() {
                                                    Some(a) => {
                                                        let intent = a.get("intent").and_then(|x| x.as_str()).unwrap_or("—").to_string();
                                                        let confidence = a.get("confidence").and_then(|x| x.as_str()).unwrap_or("unknown").to_string();
                                                        let topics = a.get("topics")
                                                            .and_then(|t| t.as_array())
                                                            .map(|rows| rows.iter().filter_map(|t| t.as_str().map(str::to_string)).collect::<Vec<_>>())
                                                            .unwrap_or_default();
                                                        view! {
                                                            <div class="spp-text-sm spp-ai-analysis">
                                                                <div class="spp-ai-analysis__row">
                                                                    <strong>"Intent: "</strong>
                                                                    {intent.clone()}
                                                                    <span class="spp-badge">{format!("conf: {confidence}")}</span>
                                                                </div>
                                                                {if !topics.is_empty() {
                                                                    view! {
                                                                        <div class="spp-ai-analysis__row">
                                                                            <strong>"Topics: "</strong>
                                                                            {topics.join(", ")}
                                                                        </div>
                                                                    }.into_view()
                                                                } else {
                                                                    ().into_view()
                                                                }}
                                                            </div>
                                                        }.into_view()
                                                    }
                                                    None => ().into_view(),
                                                }
                                            }}

                                            <h4>"Similar past cases"</h4>
                                            {move || {
                                                let rows = similar.get();
                                                if rows.is_empty() {
                                                    view! {
                                                        <div class="spp-muted spp-text-xs">"No similar conversations found yet."</div>
                                                    }.into_view()
                                                } else {
                                                    rows.iter().map(|s| {
                                                        let href = format!("/inbox/conversation/{}", s.conversation_id);
                                                        view! {
                                                            <div class="spp-ai-similar">
                                                                <a class="spp-ai-similar__subject" href=href.clone()>
                                                                    {format!("#{} {}", s.number, s.subject.clone())}
                                                                </a>
                                                                <span class="spp-badge">{format!("{:.0}%", s.score * 100.0)}</span>
                                                                {if !s.why.is_empty() {
                                                                    view! {
                                                                        <div class="spp-muted spp-text-xs">{s.why.join(" · ")}</div>
                                                                    }.into_view()
                                                                } else {
                                                                    ().into_view()
                                                                }}
                                                                {if !s.resolution.is_empty() {
                                                                    view! {
                                                                        <div class="spp-text-xs spp-ai-similar__resolution">{s.resolution.clone()}</div>
                                                                    }.into_view()
                                                                } else {
                                                                    ().into_view()
                                                                }}
                                                            </div>
                                                        }
                                                    }).collect::<Vec<_>>().into_view()
                                                }
                                            }}
                                        </div>
                                    </div>
                                </Show>

                                <Show
                                    when=move || context_open.get() && context_tab.get() == ContextTab::Customer
                                    fallback=|| ()
                                >
                                    {move || detail.with(|d| {
                                        let d = match d {
                                            Some(d) => d.clone(),
                                            None => return view! { <div></div> }.into_view(),
                                        };
                                        let c_name = d.customer_name.clone().unwrap_or_else(|| format!("#{}", d.customer_id));
                                        let c_email = d.customer_email.clone();
                                        let c_mailbox = d.mailbox_name.clone().unwrap_or_else(|| format!("#{}", d.mailbox_id));
                                        let c_assignee = d.assignee_name.clone().unwrap_or_else(|| "Unassigned".to_string());
                                        let c_created = d.created_at.clone().unwrap_or_default();
                                        let c_updated = d.updated_at.clone().unwrap_or_default();
                                        let c_tags = d.tags.clone();
                                        let c_tags_clone = c_tags.clone();
                                        view! {
                                            <div class="spp-inbox__context-body">
                                                <h3>"Customer"</h3>
                                                <dl class="spp-inbox__context-list">
                                                    <dt>"Name"</dt>
                                                    <dd>{c_name}</dd>
                                                    <dt>"Email"</dt>
                                                    <dd>{c_email.clone().unwrap_or_default()}</dd>
                                                    <dt>"Mailbox"</dt>
                                                    <dd>{c_mailbox}</dd>
                                                    <dt>"Assignee"</dt>
                                                    <dd>{c_assignee}</dd>
                                                    <dt>"Created"</dt>
                                                    <dd>{c_created}</dd>
                                                    <dt>"Updated"</dt>
                                                    <dd>{c_updated}</dd>
                                                </dl>

                                                {if !c_tags_clone.is_empty() {
                                                    view! {
                                                        <h3>"Tags"</h3>
                                                        <div class="spp-inbox__tags">
                                                            {c_tags_clone.iter().map(|tag| {
                                                                let tag = tag.clone();
                                                                view! {
                                                                    <span class="spp-inbox__tag">
                                                                        {tag.clone()}
                                                                        <button
                                                                            class="spp-chip__clear"
                                                                            aria-label={format!("Remove tag {tag}")}
                                                                            on:click=move |_| {
                                                                                apply_tag_change(
                                                                                    Vec::new(),
                                                                                    vec![tag.clone()],
                                                                                );
                                                                            }
                                                                        >
                                                                            "\u{d7}"
                                                                        </button>
                                                                    </span>
                                                                }
                                                            }).collect::<Vec<_>>()}
                                                        </div>
                                                    }.into_view()
                                                } else {
                                                    view! {
                                                        <h3>"Tags"</h3>
                                                        <div class="spp-muted spp-text-xs">"No tags yet."</div>
                                                    }.into_view()
                                                }}
                                                <div class="spp-flex spp-mt-8">
                                                    <input
                                                        class="spp-input"
                                                        type="text"
                                                        placeholder="Add a tag"
                                                        maxlength=100
                                                        prop:value=tag_input
                                                        on:input=move |ev| tag_input.set(event_target_value(&ev))
                                                    />
                                                    <button
                                                        class="spp-button spp-button--primary spp-button--small"
                                                        type="button"
                                                        on:click=move |_| {
                                                            let raw = tag_input.get_untracked();
                                                            let name = raw.trim().to_string();
                                                            if name.is_empty() {
                                                                return;
                                                            }
                                                            apply_tag_change(vec![name], Vec::new());
                                                            tag_input.set(String::new());
                                                        }
                                                    >
                                                        "Add tag"
                                                    </button>
                                                </div>
                                            </div>
                                        }.into_view()
                                    })}
                                </Show>

                                <Show
                                    when=move || context_open.get() && context_tab.get() == ContextTab::Copilot
                                    fallback=|| ()
                                >
                                    <div class="spp-inbox__context-body">
                                        <CopilotPanel conversation_id=conv_id />
                                    </div>
                                </Show>

                                <Show
                                    when=move || context_open.get() && context_tab.get() == ContextTab::Activity
                                    fallback=|| ()
                                >
                                    <ActivityTab conversation_id=conv_id />
                                </Show>
                            </div>
                        }.into_view()
                    }}
                </Show>
            </aside>
        </div>
    }
}

#[component]
fn ActivityTab(conversation_id: i64) -> impl IntoView {
    let events = create_rw_signal(Vec::<serde_json::Value>::new());
    let audit = create_rw_signal(Vec::<serde_json::Value>::new());
    let loading = create_rw_signal(true);
    let error_msg = create_rw_signal(None::<String>);

    create_effect(move |_| {
        let conversation_id = conversation_id;
        let events = events;
        let audit = audit;
        let loading = loading;
        let error_msg = error_msg;
        loading.set(true);
        error_msg.set(None);
        wasm_bindgen_futures::spawn_local(async move {
            let ev = crate::api::get_json::<serde_json::Value>(&format!(
                "/api/conversations/{conversation_id}/events"
            ))
            .await;
            let au = crate::api::get_json::<serde_json::Value>(&format!(
                "/api/audit?conversationId={conversation_id}"
            ))
            .await;
            match (ev, au) {
                (Ok(e), Ok(a)) => {
                    events.set(
                        e.get("events")
                            .and_then(|x| x.as_array())
                            .cloned()
                            .unwrap_or_default(),
                    );
                    audit.set(
                        a.get("entries")
                            .and_then(|x| x.as_array())
                            .cloned()
                            .unwrap_or_default(),
                    );
                }
                (Err(e), _) | (_, Err(e)) => error_msg.set(Some(e)),
            }
            loading.set(false);
        });
    });

    view! {
        <div class="spp-inbox__context-body">
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
                <h3>"Events"</h3>
                <Show
                    when=move || !events.with(|e| e.is_empty())
                    fallback=|| view! { <EmptyState message="No events recorded yet." /> }
                >
                    <ul class="spp-inbox__activity-list">
                        {move || {
                            events.get()
                                .iter()
                                .map(|e| {
                                    let kind = e.get("type").and_then(|x| x.as_str()).unwrap_or("event").to_string();
                                    let actor = e
                                        .get("actor_name")
                                        .and_then(|x| x.as_str())
                                        .unwrap_or("system")
                                        .to_string();
                                    let at = e
                                        .get("created_at")
                                        .and_then(|x| x.as_str())
                                        .unwrap_or("")
                                        .to_string();
                                    view! {
                                        <li class="spp-inbox__activity-item">
                                            <span class="spp-badge">{kind}</span>
                                            <span class="spp-text-xs">{actor}</span>
                                            <span class="spp-muted spp-text-xs">{at}</span>
                                        </li>
                                    }
                                })
                                .collect::<Vec<_>>()
                        }}
                    </ul>
                </Show>

                <h3 class="spp-mt-8">"Audit trail"</h3>
                <Show
                    when=move || !audit.with(|a| a.is_empty())
                    fallback=|| view! { <EmptyState message="No audit entries for this conversation." /> }
                >
                    <ul class="spp-inbox__activity-list">
                        {move || {
                            audit.get()
                                .iter()
                                .map(|a| {
                                    let action = a
                                        .get("action")
                                        .and_then(|x| x.as_str())
                                        .unwrap_or("audit")
                                        .to_string();
                                    let at = a
                                        .get("created_at")
                                        .and_then(|x| x.as_str())
                                        .unwrap_or("")
                                        .to_string();
                                    view! {
                                        <li class="spp-inbox__activity-item">
                                            <span class="spp-badge">{action}</span>
                                            <span class="spp-muted spp-text-xs">{at}</span>
                                        </li>
                                    }
                                })
                                .collect::<Vec<_>>()
                        }}
                    </ul>
                </Show>
            </Show>
        </div>
    }
}

fn parse_conversation_list_item(v: &serde_json::Value) -> ConversationListItem {
    ConversationListItem {
        id: v.get("id").and_then(|v| v.as_i64()).unwrap_or(0),
        remote_id: v.get("remote_id").and_then(|v| v.as_i64()).unwrap_or(0),
        number: v.get("number").and_then(|v| v.as_i64()).unwrap_or(0),
        subject: v
            .get("subject")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        preview: v
            .get("preview")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        status: v
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("active")
            .to_string(),
        mailbox_id: v.get("mailbox_id").and_then(|v| v.as_i64()).unwrap_or(0),
        mailbox_name: v
            .get("mailbox_name")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        assignee_id: v.get("assignee_id").and_then(|v| v.as_i64()),
        assignee_name: v
            .get("assignee_name")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        customer_id: v.get("customer_id").and_then(|v| v.as_i64()).unwrap_or(0),
        customer_name: v
            .get("customer_name")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        priority: v
            .get("priority")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        created_at: v
            .get("created_at")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        updated_at: v
            .get("updated_at")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        closed_at: v
            .get("closed_at")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        response_state: v
            .get("response_state")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
    }
}

fn parse_conversation_detail(v: &serde_json::Value) -> ConversationDetail {
    let thread = v
        .get("thread")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().map(parse_thread_entry).collect::<Vec<_>>())
        .unwrap_or_default();
    let tags = v
        .get("tags")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    ConversationDetail {
        id: v.get("id").and_then(|v| v.as_i64()).unwrap_or(0),
        remote_id: v.get("remote_id").and_then(|v| v.as_i64()).unwrap_or(0),
        number: v.get("number").and_then(|v| v.as_i64()).unwrap_or(0),
        subject: v
            .get("subject")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        preview: v
            .get("preview")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        status: v
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("active")
            .to_string(),
        mailbox_id: v.get("mailbox_id").and_then(|v| v.as_i64()).unwrap_or(0),
        mailbox_name: v
            .get("mailbox_name")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        assignee_id: v.get("assignee_id").and_then(|v| v.as_i64()),
        assignee_name: v
            .get("assignee_name")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        customer_id: v.get("customer_id").and_then(|v| v.as_i64()).unwrap_or(0),
        customer_name: v
            .get("customer_name")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        customer_email: v
            .get("customer_email")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        priority: v
            .get("priority")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        response_state: v
            .get("response_state")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        created_at: v
            .get("created_at")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        updated_at: v
            .get("updated_at")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        closed_at: v
            .get("closed_at")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        thread,
        tags,
    }
}

fn parse_thread_entry(v: &serde_json::Value) -> ThreadEntry {
    ThreadEntry {
        id: v.get("id").and_then(|v| v.as_i64()).unwrap_or(0),
        conversation_id: v
            .get("conversation_id")
            .and_then(|v| v.as_i64())
            .unwrap_or(0),
        thread_type: v
            .get("thread_type")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        body: v
            .get("body")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        actor_type: v
            .get("actor_type")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        actor_id: v.get("actor_id").and_then(|v| v.as_i64()),
        actor_name: v
            .get("actor_name")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        created_at: v
            .get("created_at")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        state: v
            .get("state")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
    }
}
