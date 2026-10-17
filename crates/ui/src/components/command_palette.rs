use leptos::*;
use leptos_router::*;

pub const MAX_HITS: usize = 12;

pub const DEBOUNCE_MS: u64 = 200;

#[derive(Debug, Clone, PartialEq)]
pub struct PaletteHit {
    pub scope: String,
    pub id: i64,
    pub title: String,
    pub snippet: String,
    pub href: String,
}

impl PaletteHit {
    pub fn from_json(v: &serde_json::Value) -> Self {
        Self {
            scope: v
                .get("scope")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("")
                .to_string(),
            id: v.get("id").and_then(serde_json::Value::as_i64).unwrap_or(0),
            title: v
                .get("title")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("")
                .to_string(),
            snippet: v
                .get("snippet")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("")
                .to_string(),
            href: v
                .get("href")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("")
                .to_string(),
        }
    }
}

#[must_use]
pub fn take_hits(hits: &[PaletteHit]) -> &[PaletteHit] {
    &hits[..hits.len().min(MAX_HITS)]
}

#[must_use]
pub fn move_selection(selected: usize, len: usize, delta: i64) -> usize {
    if len == 0 {
        return 0;
    }
    let next = selected as i64 + delta;
    next.clamp(0, len as i64 - 1) as usize
}

#[must_use]
pub fn response_is_current(issued_generation: u64, latest_generation: u64) -> bool {
    issued_generation == latest_generation
}

#[component]
pub fn CommandPalette<F>(#[prop(into)] open: MaybeSignal<bool>, on_close: F) -> impl IntoView
where
    F: Fn() + 'static,
{
    let query = create_rw_signal(String::new());
    let hits = create_rw_signal(Vec::<PaletteHit>::new());
    let selected = create_rw_signal(0usize);
    let searching = create_rw_signal(false);
    let generation = create_rw_signal(0u64);

    let close_handler = StoredValue::new(std::sync::Arc::new(on_close) as std::sync::Arc<dyn Fn()>);

    create_effect(move |_| {
        let q = query.get();
        let gen = generation.get() + 1;
        generation.set(gen);
        if q.trim().is_empty() {
            hits.set(Vec::new());
            selected.set(0);
            searching.set(false);
            return;
        }
        searching.set(true);
        set_timeout(
            move || {
                if !response_is_current(gen, generation.get_untracked()) {
                    return;
                }
                wasm_bindgen_futures::spawn_local(async move {
                    let body = serde_json::json!({ "query": q, "scope": "all" });
                    let result =
                        crate::api::post_json::<serde_json::Value>("/api/search", Some(&body))
                            .await;
                    if !response_is_current(gen, generation.get_untracked()) {
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
                            let capped = take_hits(&parsed).to_vec();
                            hits.set(capped);
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

    create_effect(move |_| {
        if open.get() {
            query.set(String::new());
            hits.set(Vec::new());
            selected.set(0);
            generation.update(|g| *g += 1);
        }
    });

    let go = StoredValue::new({
        let navigate = use_navigate();
        std::rc::Rc::new(move |hit: Option<PaletteHit>| {
            if let Some(hit) = hit {
                if !hit.href.is_empty() {
                    close_handler.with_value(|f| f());
                    navigate(&hit.href, Default::default());
                }
            }
        }) as std::rc::Rc<dyn Fn(Option<PaletteHit>)>
    });

    view! {
        <Show when=move || open.get() fallback=|| ()>
            <div
                class="spp-command-palette-overlay"
                role="presentation"
                on:mousedown=move |e| {
                    if e.target() == e.current_target() {
                        close_handler.with_value(|f| f());
                    }
                }
            >
                <div class="spp-command-palette" role="dialog" aria-label="Command palette">
                    <div class="spp-command-palette__input">
                        <span class="spp-command-palette__icon" aria-hidden="true">"⌘"</span>
                        <input
                            type="text"
                            class="spp-command-palette__field"
                            placeholder="Search tickets, customers, knowledge, issues…"
                            prop:value=move || query.get()
                            on:input=move |e| query.set(event_target_value(&e))
                            on:keydown=move |e| {
                                let len = hits.get_untracked().len();
                                match e.key().as_str() {
                                    "ArrowDown" => {
                                        e.prevent_default();
                                        selected.set(move_selection(selected.get_untracked(), len, 1));
                                    }
                                    "ArrowUp" => {
                                        e.prevent_default();
                                        selected.set(move_selection(selected.get_untracked(), len, -1));
                                    }
                                    "Enter" => {
                                        let idx = selected.get_untracked();
                                        let hit = hits.get_untracked().get(idx).cloned();
                                        go.with_value(|f| f(hit));
                                    }
                                    "Escape" => close_handler.with_value(|f| f()),
                                    _ => {}
                                }
                            }
                        />
                    </div>
                    <div class="spp-command-palette__results">
                        <div class="spp-command-palette__list">
                            {move || {
                                hits.get()
                                    .into_iter()
                                    .enumerate()
                                    .map(|(i, hit)| {
                                        let is_selected = move || selected.get() == i;
                                        let hit_for_click = hit.clone();
                                        let scope = hit.scope.clone();
                                        let title = hit.title.clone();
                                        let snippet = hit.snippet.clone();
                                        let go_row = go;
                                        view! {
                                            <div
                                                class="spp-command-palette__item"
                                                class:is-selected=is_selected
                                                on:mouseenter=move |_| selected.set(i)
                                                on:click=move |_| go_row.with_value(|f| f(Some(hit_for_click.clone())))
                                            >
                                                <div class="spp-command-palette__item-label">
                                                    <span class="spp-badge">{scope}</span>
                                                    <span class="spp-command-palette__item-title">{title}</span>
                                                </div>
                                                {if !snippet.is_empty() {
                                                    view! {
                                                        <div class="spp-command-palette__item-desc">
                                                            {snippet}
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
                        </div>
                        {move || {
                            let q = query.get();
                            let empty = hits.get().is_empty();
                            if !q.trim().is_empty() && empty && !searching.get() {
                                view! {
                                    <div class="spp-state">
                                        <p class="spp-state__body">
                                            {format!("No results for \u{201c}{q}\u{201d}")}
                                        </p>
                                    </div>
                                }.into_view()
                            } else if q.trim().is_empty() {
                                view! {
                                    <p class="spp-command-palette__hint">
                                        "Type to search across tickets, customers, knowledge, known issues, saved replies and AI analyses. "
                                        <span class="spp-kbd">"↵"</span> " open · "
                                        <span class="spp-kbd">"↑↓"</span> " navigate"
                                    </p>
                                }.into_view()
                            } else {
                                ().into_view()
                            }
                        }}
                    </div>
                    {move || {
                        let n = hits.get().len();
                        if n > 0 {
                            view! {
                                <div class="spp-command-palette__footer">
                                    <span class="spp-command-palette__footer-hint">
                                        <span class="spp-kbd">"↵"</span> " open selected"
                                    </span>
                                    <span class="spp-command-palette__footer-count">
                                        {format!("{n} results")}
                                    </span>
                                </div>
                            }.into_view()
                        } else {
                            ().into_view()
                        }
                    }}
                </div>
            </div>
        </Show>
    }
}
