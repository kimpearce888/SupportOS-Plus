use leptos::prelude::*;
use leptos_router::hooks::*;

use crate::components::command_palette::{
    DEBOUNCE_MS, PaletteHit, move_selection, response_is_current, take_hits,
};

#[component]
pub fn CommandPalettePage() -> impl IntoView {
    let query = RwSignal::new(String::new());
    let hits = RwSignal::new(Vec::<PaletteHit>::new());
    let selected = RwSignal::new(0usize);
    let searching = RwSignal::new(false);
    let generation = RwSignal::new(0u64);
    let navigate = use_navigate();

    Effect::new(move |_| {
        let q = query.get();
        let req_id = generation.get() + 1;
        generation.set(req_id);
        if q.trim().is_empty() {
            hits.set(Vec::new());
            selected.set(0);
            searching.set(false);
            return;
        }
        searching.set(true);
        set_timeout(
            move || {
                if !response_is_current(req_id, generation.get_untracked()) {
                    return;
                }
                wasm_bindgen_futures::spawn_local(async move {
                    let body = serde_json::json!({ "query": q, "scope": "all" });
                    let result =
                        crate::api::post_json::<serde_json::Value>("/api/search", Some(&body))
                            .await;
                    if !response_is_current(req_id, generation.get_untracked()) {
                        return;
                    }
                    match result {
                        Ok(v) => {
                            let parsed = v
                                .get("hits")
                                .and_then(serde_json::Value::as_array)
                                .map(|arr| {
                                    arr.iter().map(PaletteHit::from_json).collect::<Vec<_>>()
                                })
                                .unwrap_or_default();
                            hits.set(take_hits(&parsed).to_vec());
                            selected.set(0);
                        }
                        Err(_) => hits.set(Vec::new()),
                    }
                    searching.set(false);
                });
            },
            std::time::Duration::from_millis(DEBOUNCE_MS),
        );
    });

    let go = {
        let navigate = navigate.clone();
        move |hit: Option<PaletteHit>| {
            if let Some(hit) = hit
                && !hit.href.is_empty()
            {
                navigate(&hit.href, Default::default());
            }
        }
    };
    let go_key = go.clone();

    view! {
        <div class="spp-page spp-page--command-palette">
            <h2 class="spp-page__title">"Command Palette"</h2>

            <p class="spp-page__intro">
                "Quick search across tickets, customers, knowledge, known issues, saved replies and AI analyses. Press Cmd/Ctrl+K anywhere in the app to open the palette overlay, or use this page directly."
            </p>

            <div class="spp-command-palette__search">
                <input
                    class="spp-command-palette__input spp-command-palette__field--page"
                    type="text"
                    placeholder="Search tickets, customers, knowledge, issues…"
                    prop:value=query
                    on:input=move |ev| query.set(event_target_value(&ev))
                    on:keydown=move |ev| {
                        let len = hits.get_untracked().len();
                        match ev.key().as_str() {
                            "ArrowDown" => {
                                ev.prevent_default();
                                selected.set(move_selection(selected.get_untracked(), len, 1));
                            }
                            "ArrowUp" => {
                                ev.prevent_default();
                                selected.set(move_selection(selected.get_untracked(), len, -1));
                            }
                            "Enter" => {
                                let idx = selected.get_untracked();
                                let hit = hits.get_untracked().get(idx).cloned();
                                go_key(hit);
                            }
                            _ => {}
                        }
                    }
                    autofocus=true
                />
            </div>

            <ul class="spp-command-palette__list">
                {move || {
                    hits.get()
                        .into_iter()
                        .enumerate()
                        .map(|(i, hit)| {
                            let is_selected = move || selected.get() == i;
                            let go_row = go.clone();
                            let hit_click = hit.clone();
                            let scope = hit.scope.clone();
                            let title = hit.title.clone();
                            let snippet = hit.snippet.clone();
                            view! {
                                <li
                                    class="spp-command-palette__item"
                                    class:is-selected=is_selected
                                    on:mouseenter=move |_| selected.set(i)
                                    on:click=move |_| go_row(Some(hit_click.clone()))
                                >
                                    <span class="spp-command-palette__label">
                                        <span class="spp-badge">{scope}</span>
                                        <span class="spp-command-palette__item-title">{title}</span>
                                    </span>
                                    {if !snippet.is_empty() {
                                        view! {
                                            <span class="spp-command-palette__desc">{snippet}</span>
                                        }.into_any()
                                    } else {
                                        ().into_any()
                                    }}
                                </li>
                            }
                        })
                        .collect::<Vec<_>>()
                }}
            </ul>

            <Show when=move || {
                let q = query.get();
                !q.trim().is_empty() && hits.get().is_empty() && !searching.get()
            } fallback=|| ()>
                <div class="spp-state">
                    <p class="spp-state__body">
                        {move || format!("No results for \u{201c}{}\u{201d}", query.get())}
                    </p>
                </div>
            </Show>
        </div>
    }
}
