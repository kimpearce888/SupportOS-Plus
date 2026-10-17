use leptos::prelude::*;
use leptos_router::hooks::*;

use crate::components::state_view::{EmptyState, LoadingState};

const SCOPES: &[(&str, &str)] = &[
    ("all", "All"),
    ("conversations", "Conversations"),
    ("customers", "Customers"),
    ("docs", "Docs"),
    ("knowledge", "Knowledge"),
    ("issues", "Issues"),
    ("incidents", "Incidents"),
];

#[component]
pub fn SearchPage() -> impl IntoView {
    let query = RwSignal::new(String::new());
    let results = RwSignal::new(Vec::<serde_json::Value>::new());
    let loading = RwSignal::new(false);
    let error_msg = RwSignal::new(None::<String>);
    let has_searched = RwSignal::new(false);
    let scope = RwSignal::new("all".to_string());
    let semantic = RwSignal::new(false);
    let navigate = StoredValue::new_local(use_navigate());

    let do_search = move || {
        let q = query.get();
        if q.trim().is_empty() {
            results.set(Vec::new());
            has_searched.set(false);
            return;
        }
        loading.set(true);
        error_msg.set(None);
        has_searched.set(true);
        wasm_bindgen_futures::spawn_local(async move {
            let body = serde_json::json!({
                "query": q,
                "scope": scope.get(),
                "semantic": semantic.get(),
            });
            match crate::api::post_json::<serde_json::Value>("/api/search", Some(&body)).await {
                Ok(data) => {
                    let r = data
                        .get("hits")
                        .and_then(|v| v.as_array())
                        .cloned()
                        .unwrap_or_default();
                    results.set(r);
                    loading.set(false);
                }
                Err(e) => {
                    error_msg.set(Some(e));
                    loading.set(false);
                }
            }
        });
    };

    view! {
        <div class="spp-page spp-page--search">
            <h2 class="spp-page__title">"Search"</h2>

            <p class="spp-page__intro">
                "Universal search across conversations + customers (FTS5-powered)."
            </p>

            <div class="spp-search__bar">
                <input
                    class="spp-search__input"
                    type="text"
                    placeholder="Search conversations, customers..."
                    prop:value=query
                    on:input=move |ev| query.set(event_target_value(&ev))
                    on:keydown=move |ev| {
                        if ev.key() == "Enter" {
                            do_search();
                        }
                    }
                />
                <button class="spp-button" on:click=move |_| do_search()>
                    "Search"
                </button>
            </div>

            <div class="spp-search__scopes">
                {SCOPES.iter().map(|(value, label)| {
                    let val1 = value.to_string();
                    let val2 = value.to_string();
                    view! {
                        <button
                            class=move || format!(
                                "spp-button spp-button--tab{}",
                                if scope.get() == val1 { " spp-button--tab-active" } else { "" }
                            )
                            on:click=move |_| scope.set(val2.clone())
                        >
                            {*label}
                        </button>
                    }
                }).collect::<Vec<_>>()}
            </div>

            <div class="spp-search__semantic">
                <label class="spp-search__toggle">
                    <input
                        type="checkbox"
                        prop:checked=semantic
                        on:change=move |ev| semantic.set(event_target_checked(&ev))
                    />
                    <span>"Semantic search (uses embeddings)"</span>
                </label>
                <Show when=move || semantic.get() fallback=|| ()>
                    <p class="spp-search__semantic-note">
                        "Semantic search uses AI embeddings for meaning-based matching. Requires an embedding model in Settings."
                    </p>
                </Show>
            </div>

            <Show when=move || loading.get() fallback=|| ()>
                <LoadingState />
            </Show>

            <Show when=move || error_msg.get().is_some() fallback=|| ()>
                <div class="spp-state spp-state--error">
                    <span class="spp-state__icon" aria-hidden="true">"⚠"</span>
                    <p class="spp-state__body">{move || error_msg.get().unwrap_or_default()}</p>
                </div>
            </Show>

            <Show
                when=move || !loading.get() && error_msg.get().is_none() && has_searched.get()
                fallback=|| ()
            >
                <Show
                    when=move || !results.with(|r| r.is_empty())
                    fallback=|| {
                        view! {
                            <EmptyState message="No results found. Try a different query." />
                        }
                    }
                >
                    <ul class="spp-search__results">
                        {move || results.with(|items| {
                            items.iter().map(|r| {
                                let resource_type = r.get("scope").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                let remote_id = r.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
                                let title = r.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                let snippet = r.get("snippet").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                let q = query.get();
                                let highlighted = highlight_matches(&snippet, &q);
                                let nav_target = navigate_for_scope(&resource_type, remote_id);
                                view! {
                                    <li
                                        class="spp-search__result"
                                        on:click=move |_| {
                                            if !nav_target.is_empty() {
                                                navigate.with_value(|n| n(&nav_target, Default::default()));
                                            }
                                        }
                                        style="cursor: pointer"
                                    >
                                        <div class="spp-search__result-header">
                                            <span class="spp-badge spp-badge--status">{resource_type}</span>
                                            <span class="spp-search__result-id">{"#"}{remote_id.to_string()}</span>
                                        </div>
                                        <div class="spp-search__result-title">{title}</div>
                                        <div class="spp-search__result-snippet">
                                            {highlighted}
                                        </div>
                                    </li>
                                }
                            }).collect::<Vec<_>>()
                        })}
                    </ul>
                </Show>
            </Show>
        </div>
    }
}

fn highlight_matches(text: &str, query: &str) -> Vec<String> {
    if query.trim().is_empty() {
        return vec![text.to_string()];
    }
    let query_lower = query.to_lowercase();
    let text_lower = text.to_lowercase();
    let mut result = Vec::new();
    let mut last_end = 0;
    let mut start = text_lower.find(&query_lower);
    while let Some(s) = start {
        if s > last_end {
            result.push(text[last_end..s].to_string());
        }
        result.push(format!("<mark>{}</mark>", &text[s..s + query.len()]));
        last_end = s + query.len();
        start = text_lower[last_end..]
            .find(&query_lower)
            .map(|i| last_end + i);
    }
    if last_end < text.len() {
        result.push(text[last_end..].to_string());
    }
    if result.is_empty() {
        result.push(text.to_string());
    }
    result
}

fn navigate_for_scope(scope: &str, id: i64) -> String {
    match scope {
        "conversations" => format!("/inbox?id={id}"),
        "customers" => format!("/customers/{id}"),
        "docs" => format!("/docs?article={id}"),
        "knowledge" => format!("/knowledge?doc={id}"),
        "issues" => format!("/issues/{id}"),
        "incidents" => format!("/incidents/{id}"),
        _ => String::new(),
    }
}
