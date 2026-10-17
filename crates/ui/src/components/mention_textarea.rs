use leptos::prelude::*;

use super::mention::{MentionDirectory, use_mention_directory};

#[must_use]
pub fn caret_query(value: &str, caret: usize) -> Option<(String, usize)> {
    let upto = &value[..caret.min(value.len())];
    let mut idx = None;
    for (i, ch) in upto.char_indices().rev() {
        if ch == '@' {
            let before = upto[..i].chars().next_back();
            if before.is_none() || before.is_some_and(char::is_whitespace) {
                idx = Some(i);
            }
            break;
        }
        if ch.is_whitespace() {
            break;
        }
        if !is_token_char(ch) {
            break;
        }
    }
    let at = idx?;
    let text = upto[at + 1..].to_string();
    Some((text, at))
}

fn is_token_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-'
}

#[must_use]
pub fn filter_matches(dir: &MentionDirectory, query: &str) -> Vec<(String, String, bool)> {
    let q = query.to_lowercase();
    if q.chars().any(|c| !is_token_char(c)) {
        return Vec::new();
    }
    dir.entries()
        .into_iter()
        .filter(|(token, display, _)| {
            token.to_lowercase().starts_with(&q) || display.to_lowercase().contains(&q)
        })
        .take(8)
        .collect()
}

#[must_use]
pub fn apply_completion(value: &str, start: usize, token: &str, caret: usize) -> String {
    let caret = caret.min(value.len());
    let after = &value[caret..];
    let sep = if after.is_empty() || after.starts_with(' ') {
        ""
    } else {
        " "
    };
    format!("{}@{}{}{}", &value[..start], token, sep, after)
}

#[component]
pub fn MentionTextarea(
    #[prop(into)] value: RwSignal<String>,
    #[prop(default = "Type @ to mention a teammate or team".to_string(), into)] placeholder: String,
    #[prop(default = 3)] rows: u8,
    #[prop(default = false)] submit_on_enter: bool,
    #[prop(default = 3)] submit_key: u8,
    #[prop(default = 13)] submit_key_code: u32,
    #[prop(optional)] on_submit: Option<std::sync::Arc<dyn Fn() + Send + Sync>>,
) -> impl IntoView {
    let _ = submit_key;
    let directory = use_mention_directory();
    let popup_open = RwSignal::new(false);
    let active = RwSignal::new(0usize);
    let caret = RwSignal::new(0usize);

    let matches = Memo::new(move |_| {
        let value_now = value.get();
        let caret_now = caret.get();
        caret_query(&value_now, caret_now)
            .map(|(q, _)| filter_matches(&directory.get(), &q))
            .unwrap_or_default()
    });

    let complete = move |token: String| {
        let value_now = value.get();
        let caret_now = caret.get();
        if let Some((_, start)) = caret_query(&value_now, caret_now) {
            value.set(apply_completion(&value_now, start, &token, caret_now));
        }
        popup_open.set(false);
    };

    view! {
        <div class="spp-mention-wrap">
            <textarea
                class="spp-mention-input"
                rows=rows
                placeholder=placeholder
                prop:value=move || value.get()
                on:input=move |ev| {
                    let el: web_sys::HtmlTextAreaElement = event_target(&ev);
                    caret.set(el.selection_start().unwrap_or(None).unwrap_or(0) as usize);
                    value.set(event_target_value(&ev));
                    popup_open.set(true);
                }
                on:keyup=move |ev| {
                    let el: web_sys::HtmlTextAreaElement = event_target(&ev);
                    caret.set(el.selection_start().unwrap_or(None).unwrap_or(0) as usize);
                }
                on:blur=move |_| {
                    set_timeout(move || popup_open.set(false), std::time::Duration::from_millis(150));
                }
                on:keydown=move |ev| {
                    let open = popup_open.get();
                    let n = matches.get().len();
                    if open && n > 0 {
                        match ev.key().as_str() {
                            "ArrowDown" => {
                                ev.prevent_default();
                                active.update(|a| *a = (*a + 1) % n);
                            }
                            "ArrowUp" => {
                                ev.prevent_default();
                                active.update(|a| *a = (*a + n - 1) % n);
                            }
                            "Enter" | "Tab" => {
                                ev.prevent_default();
                                let list = matches.get();
                                let i = active.get().min(list.len().saturating_sub(1));
                                if let Some((token, _, _)) = list.get(i).cloned() {
                                    complete(token);
                                }
                            }
                            "Escape" => {
                                popup_open.set(false);
                            }
                            _ => {}
                        }
                    } else if submit_on_enter
                        && ev.key_code() == submit_key_code
                        && (ev.ctrl_key() || ev.meta_key())
                    {
                        ev.prevent_default();
                        if let Some(on_submit) = on_submit.as_ref() {
                            on_submit();
                        }
                    }
                }
            ></textarea>
            <Show when=move || popup_open.get() && !matches.get().is_empty() fallback=|| ()>
                <div class="spp-mention-popup" role="listbox">
                    <div class="spp-mention-popup__head">"@ mentions"</div>
                    {move || {
                        matches.get()
                            .into_iter()
                            .enumerate()
                            .map(|(i, (token, display, _))| {
                                let token_for_click = token.clone();
                                view! {
                                    <button
                                        type="button"
                                        class="spp-mention-option"
                                        class:is-active=move || active.get() == i
                                        role="option"
                                        aria-selected=move || active.get() == i
                                        on:mousedown=move |ev| {
                                            ev.prevent_default();
                                            complete(token_for_click.clone());
                                        }
                                        on:mouseenter=move |_| active.set(i)
                                    >
                                        {display.clone()}
                                    </button>
                                }
                            })
                            .collect::<Vec<_>>()
                    }}
                </div>
            </Show>
        </div>
    }
}
