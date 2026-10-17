use leptos::*;

use crate::toasts;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ConversationLanguageSummary {
    pub primary: LanguageDetection,
    pub notes: Vec<String>,
    pub per_message: Vec<PerMessageDetection>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct LanguageDetection {
    pub code: Option<String>,
    pub name: Option<String>,
    pub confidence: String,
    pub method: String,
    pub text: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PerMessageDetection {
    pub thread_id: i64,
    pub detection: LanguageDetection,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TranslationResult {
    pub source_text: String,
    pub translated_text: String,
    pub source_lang_name: Option<String>,
    pub target_lang_name: Option<String>,
    pub cached: bool,
    pub purpose: String,
    pub note: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ActiveMessage {
    pub text: String,
    pub translated: Option<TranslationResult>,
}

fn parse_detection(v: &serde_json::Value) -> LanguageDetection {
    LanguageDetection {
        code: v.get("code").and_then(|x| x.as_str()).map(str::to_string),
        name: v.get("name").and_then(|x| x.as_str()).map(str::to_string),
        confidence: v
            .get("confidence")
            .and_then(|x| x.as_str())
            .unwrap_or("unknown")
            .to_string(),
        method: v
            .get("method")
            .and_then(|x| x.as_str())
            .unwrap_or_default()
            .to_string(),
        text: v
            .get("text")
            .and_then(|x| x.as_str())
            .unwrap_or_default()
            .to_string(),
    }
}

#[must_use]
pub fn parse_language_summary(v: &serde_json::Value) -> ConversationLanguageSummary {
    ConversationLanguageSummary {
        primary: v
            .get("primary_language")
            .map(parse_detection)
            .unwrap_or_default(),
        notes: v
            .get("notes")
            .and_then(|n| n.as_array())
            .map(|rows| {
                rows.iter()
                    .filter_map(|n| n.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default(),
        per_message: v
            .get("per_message")
            .and_then(|p| p.as_array())
            .map(|rows| {
                rows.iter()
                    .map(|m| PerMessageDetection {
                        thread_id: m.get("thread_id").and_then(|x| x.as_i64()).unwrap_or(0),
                        detection: m.get("detection").map(parse_detection).unwrap_or_default(),
                    })
                    .collect()
            })
            .unwrap_or_default(),
    }
}

#[must_use]
pub fn parse_translation_result(v: &serde_json::Value) -> TranslationResult {
    TranslationResult {
        source_text: v
            .get("source_text")
            .and_then(|x| x.as_str())
            .unwrap_or_default()
            .to_string(),
        translated_text: v
            .get("translated_text")
            .and_then(|x| x.as_str())
            .unwrap_or_default()
            .to_string(),
        source_lang_name: v
            .get("source_lang_name")
            .and_then(|x| x.as_str())
            .map(str::to_string),
        target_lang_name: v
            .get("target_lang_name")
            .and_then(|x| x.as_str())
            .map(str::to_string),
        cached: v.get("cached").and_then(|x| x.as_bool()).unwrap_or(false),
        purpose: v
            .get("purpose")
            .and_then(|x| x.as_str())
            .unwrap_or_default()
            .to_string(),
        note: v
            .get("note")
            .and_then(|x| x.as_str())
            .unwrap_or_default()
            .to_string(),
    }
}

#[must_use]
pub fn should_auto_translate(detection: &LanguageDetection, target: &str) -> bool {
    match &detection.code {
        Some(code) => code != target,
        None => false,
    }
}

#[must_use]
pub fn confidence_class(confidence: &str) -> &'static str {
    match confidence {
        "high" => "spp-badge--ok",
        "low" | "unknown" => "spp-badge--warn",
        _ => "",
    }
}

#[component]
pub fn TranslationPanel(conversation_id: i64) -> impl IntoView {
    let open = create_rw_signal(false);
    let target = create_rw_signal("en".to_string());
    let languages = create_rw_signal(Vec::<(String, String)>::new());
    let summary = create_rw_signal(None::<ConversationLanguageSummary>);
    let loading = create_rw_signal(false);
    let draft = create_rw_signal(String::new());
    let translated_draft = create_rw_signal(None::<TranslationResult>);
    let active_message = create_rw_signal(None::<ActiveMessage>);
    let translating = create_rw_signal(false);

    spawn_local(async move {
        if let Ok(r) = crate::api::get_json::<serde_json::Value>("/api/translation/meta").await {
            let langs = r
                .get("languages")
                .and_then(|l| l.as_array())
                .map(|rows| {
                    rows.iter()
                        .filter_map(|l| {
                            let code = l.get("code")?.as_str()?.to_string();
                            let name = l
                                .get("name")
                                .and_then(|x| x.as_str())
                                .unwrap_or_default()
                                .to_string();
                            Some((code, name))
                        })
                        .collect()
                })
                .unwrap_or_default();
            languages.set(langs);
        }
    });

    create_effect(move |_| {
        if open.get() && summary.get_untracked().is_none() {
            loading.set(true);
            let summary = summary;
            let loading = loading;
            spawn_local(async move {
                match crate::api::get_json::<serde_json::Value>(&format!(
                    "/api/translation/conversation/{conversation_id}"
                ))
                .await
                {
                    Ok(v) => summary.set(Some(parse_language_summary(&v))),
                    Err(_) => summary.set(None),
                }
                loading.set(false);
            });
        }
    });

    let translate = move |text: String, purpose: &'static str| {
        let t = text.trim().to_string();
        if t.is_empty() || translating.get_untracked() {
            return;
        }
        translating.set(true);
        let body = serde_json::json!({
            "text": t,
            "from": "auto",
            "to": target.get_untracked(),
            "purpose": purpose,
        });
        let translating = translating;
        let active_message = active_message;
        let translated_draft = translated_draft;
        spawn_local(async move {
            match crate::api::post_json::<serde_json::Value>(
                "/api/translation/translate",
                Some(&body),
            )
            .await
            {
                Ok(v) => {
                    let r = parse_translation_result(&v);
                    if r.purpose == "customer_inbound" {
                        active_message.update(|m| {
                            if let Some(m) = m {
                                if m.text == r.source_text {
                                    m.translated = Some(r);
                                }
                            }
                        });
                    } else if r.purpose == "agent_draft" {
                        translated_draft.set(Some(r));
                    }
                    if v.get("cached").and_then(|c| c.as_bool()).unwrap_or(false) {
                        toasts::info("Served from the local translation cache.");
                    }
                }
                Err(e) => toasts::error(e),
            }
            translating.set(false);
        });
    };

    view! {
        <section class="spp-card spp-translation-panel">
            <button
                class="spp-card__title spp-card__title--collapsible"
                on:click=move |_| open.update(|o| *o = !*o)
            >
                {move || format!("Translation (local model) {}", if open.get() { "▾" } else { "▸" })}
            </button>
            <Show when=move || open.get() fallback=|| ()>
                <p class="spp-muted spp-text-xs">
                    "Language detection is deterministic and local. Translation uses only the locally configured model (no cloud); technical terms, URLs and emails are preserved. Nothing is sent automatically."
                </p>
                <div class="spp-translation-panel__target">
                    <label>"Translate to"</label>
                    <select
                        class="spp-input"
                        prop:value=move || target.get()
                        on:change=move |ev| target.set(event_target_value(&ev))
                    >
                        {move || {
                            languages.get()
                                .into_iter()
                                .map(|(code, name)| {
                                    view! { <option value=code.clone() selected=move || target.get() == code>{name.clone()}</option> }
                                })
                                .collect::<Vec<_>>()
                        }}
                    </select>
                </div>

                <Show when=move || loading.get() fallback=|| ()>
                    <div class="spp-muted spp-text-xs">"Loading…"</div>
                </Show>

                <Show when=move || summary.get().is_some() fallback=|| ()>
                    {move || {
                        let s = summary.get().unwrap_or_default();
                        view! {
                            <div class="spp-translation-panel__summary">
                                <div class="spp-text-sm">
                                    <strong>"Customer language: "</strong>
                                    {s.primary.name.clone().unwrap_or_else(|| "unknown".to_string())}
                                    {if s.primary.code.is_some() {
                                        let badge = format!(
                                            "{} confidence · {}",
                                            s.primary.confidence, s.primary.method
                                        );
                                        view! {
                                            <span class=format!("spp-badge {}", confidence_class(&s.primary.confidence))>
                                                {badge}
                                            </span>
                                        }.into_view()
                                    } else {
                                        ().into_view()
                                    }}
                                </div>
                                <div class="spp-muted spp-text-xs">{s.notes.join(" ")}</div>
                                {if !s.per_message.is_empty() {
                                    view! {
                                        <div class="spp-translation-panel__messages">
                                            {s.per_message
                                                .iter()
                                                .take(8)
                                                .map(|m| {
                                                    let detection = m.detection.clone();
                                                    let text: String = detection.text.chars().take(800).collect();
                                                    let preview: String = detection.text.chars().take(60).collect();
                                                    let row_label = format!(
                                                        "#{} · {} ({}) · {}…",
                                                        m.thread_id,
                                                        detection.name.clone().unwrap_or_else(|| "unknown".to_string()),
                                                        detection.confidence,
                                                        preview
                                                    );
                                                    let row_text = text.clone();
                                                    let det_for_auto = detection.clone();
                                                    view! {
                                                        <button
                                                            class="spp-button spp-button--ghost spp-button--tiny spp-translation-panel__msg"
                                                            title="Translate this message"
                                                            on:click=move |_| {
                                                                active_message.set(Some(ActiveMessage {
                                                                    text: row_text.clone(),
                                                                    translated: None,
                                                                }));
                                                                if should_auto_translate(&det_for_auto, &target.get()) {
                                                                    translate(row_text.clone(), "customer_inbound");
                                                                }
                                                            }
                                                        >
                                                            {row_label.clone()}
                                                        </button>
                                                    }
                                                })
                                                .collect::<Vec<_>>()}
                                        </div>
                                    }.into_view()
                                } else {
                                    ().into_view()
                                }}
                        </div>
                    }}}
                </Show>

                <Show when=move || active_message.get().is_some() fallback=|| ()>
                    {move || {
                        let m = active_message.get().unwrap_or_default();
                        let translated = m.translated.clone();
                        view! {
                            <div class="spp-translation-side-by-side">
                                <div class="spp-translation-col">
                                    <div class="spp-muted spp-text-xs">"Original"</div>
                                    <div class="spp-text-sm spp-pre-wrap">{m.text.clone()}</div>
                                </div>
                                <div class="spp-translation-col">
                                    <div class="spp-muted spp-text-xs">
                                        {move || if translating.get() { "Translated (working…)" } else { "Translated" }.to_string()}
                                    </div>
                                    <div class="spp-text-sm spp-pre-wrap">
                                        {match &translated {
                                            Some(t) => t.translated_text.clone(),
                                            None => "Press translate below.".to_string(),
                                        }}
                                    </div>
                                    {if let Some(t) = &translated {
                                        view! { <div class="spp-muted spp-text-xs">{t.note.clone()}</div> }.into_view()
                                    } else {
                                        ().into_view()
                                    }}
                                </div>
                                <div class="spp-translation-panel__row">
                                    <button
                                        class="spp-button spp-button--small"
                                        on:click=move |_| translate(m.text.clone(), "customer_inbound")
                                        disabled=move || translating.get()
                                    >
                                        "Translate message"
                                    </button>
                                    <button
                                        class="spp-button spp-button--ghost spp-button--small"
                                        on:click=move |_| active_message.set(None)
                                    >
                                        "Close"
                                    </button>
                                </div>
                            </div>
                        }
                    }}
                </Show>

                <div class="spp-translation-draft">
                    <div class="spp-muted spp-text-xs">
                        "Draft in your language, translate for the customer (review before using — nothing sends from here):"
                    </div>
                    <textarea
                        class="spp-input"
                        rows=3
                        maxlength=4000
                        placeholder="Write your reply draft here, then translate it…"
                        prop:value=move || draft.get()
                        on:input=move |ev| draft.set(event_target_value(&ev))
                    ></textarea>
                    <div class="spp-translation-panel__row">
                        <button
                            class="spp-button spp-button--small"
                            on:click=move |_| translate(draft.get_untracked(), "agent_draft")
                            disabled=move || translating.get() || draft.get().trim().is_empty()
                        >
                            "Translate draft"
                        </button>
                    </div>
                    <Show when=move || translated_draft.get().is_some() fallback=|| ()>
                        {move || {
                            let t = translated_draft.get().unwrap_or_default();
                            view! {
                                <div class="spp-translation-side-by-side">
                                    <div class="spp-translation-col">
                                        <div class="spp-muted spp-text-xs">
                                            {format!("Your draft ({})", t.source_lang_name.clone().unwrap_or_else(|| "source".to_string()))}
                                        </div>
                                        <div class="spp-text-sm spp-pre-wrap">{t.source_text.clone()}</div>
                                    </div>
                                    <div class="spp-translation-col">
                                        <div class="spp-muted spp-text-xs">
                                            {format!("{} translation", t.target_lang_name.clone().unwrap_or_else(|| "target".to_string()))}
                                        </div>
                                        <div class="spp-text-sm spp-pre-wrap">{t.translated_text.clone()}</div>
                                        <div class="spp-muted spp-text-xs">{t.note.clone()}</div>
                                    </div>
                                </div>
                            }
                        }}
                    </Show>
                </div>
            </Show>
        </section>
    }
}
