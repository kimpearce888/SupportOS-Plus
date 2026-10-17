use leptos::prelude::*;

use crate::components::state_view::{EmptyState, LoadingState};

#[component]
pub fn SupportGraphPage() -> impl IntoView {
    let nodes = RwSignal::new(Vec::<serde_json::Value>::new());
    let selected_node = RwSignal::new(None::<(String, i64)>);
    let neighbors = RwSignal::new(Vec::<serde_json::Value>::new());
    let loading = RwSignal::new(true);
    let error_msg = RwSignal::new(None::<String>);
    let search_query = RwSignal::new(String::new());
    let stats = RwSignal::new(None::<serde_json::Value>);

    Effect::new(move |_| {
        let nodes = nodes;
        let loading = loading;
        let error_msg = error_msg;
        let stats = stats;
        let q = search_query.get();
        wasm_bindgen_futures::spawn_local(async move {
            let url = format!("/api/graph/search?q={}", url_encode(&q));
            match crate::api::get_json::<serde_json::Value>(&url).await {
                Ok(data) => {
                    let items = data
                        .get("results")
                        .and_then(|v| v.as_array())
                        .cloned()
                        .unwrap_or_default();
                    nodes.set(items);
                    stats.set(data.get("stats").cloned());
                    loading.set(false);
                }
                Err(e) => {
                    error_msg.set(Some(e));
                    loading.set(false);
                }
            }
        });
    });

    Effect::new(move |_| {
        let neighbors = neighbors;
        let error_msg = error_msg;
        if let Some((kind, nid)) = selected_node.get() {
            wasm_bindgen_futures::spawn_local(async move {
                let path = format!("/api/graph/neighbors/{kind}/{nid}");
                match crate::api::get_json::<serde_json::Value>(&path).await {
                    Ok(data) => {
                        neighbors.set(
                            data.get("edges")
                                .and_then(|v| v.as_array())
                                .cloned()
                                .unwrap_or_default(),
                        );
                    }
                    Err(e) => {
                        error_msg.set(Some(e));
                    }
                }
            });
        } else {
            neighbors.set(Vec::new());
        }
    });

    view! {
        <div class="spp-page spp-page--support-graph">
            <h2 class="spp-page__title">"Support Graph"</h2>

            <p class="spp-page__intro">
                "Graph of support entities (customers, conversations, issues, incidents) and their relationships. Click a node to see its neighbors."
            </p>

            <Show when=move || stats.get().is_some() fallback=|| ()>
                <div class="spp-graph__stats">
                    {move || {
                        let s = stats.get().unwrap_or_default();
                        let total_nodes = s.get("total_nodes").and_then(|v| v.as_i64()).unwrap_or(0);
                        let total_edges = s.get("total_edges").and_then(|v| v.as_i64()).unwrap_or(0);
                        let by_kind = s.get("by_kind").and_then(|v| v.as_object()).cloned().unwrap_or_default();
                        view! {
                            <div class="spp-graph__stat">
                                <span class="spp-graph__stat-value">{total_nodes.to_string()}</span>
                                <span class="spp-graph__stat-label">"Nodes"</span>
                            </div>
                            <div class="spp-graph__stat">
                                <span class="spp-graph__stat-value">{total_edges.to_string()}</span>
                                <span class="spp-graph__stat-label">"Edges"</span>
                            </div>
                            {by_kind.iter().map(|(kind, count)| {
                                let k = kind.clone();
                                let c = count.as_i64().unwrap_or(0);
                                view! {
                                    <div class="spp-graph__stat">
                                        <span class="spp-graph__stat-value">{c.to_string()}</span>
                                        <span class="spp-graph__stat-label">{k}</span>
                                    </div>
                                }
                            }).collect::<Vec<_>>()}
                        }
                    }}
                </div>
            </Show>

            <div class="spp-graph__search">
                <input
                    class="spp-input"
                    type="text"
                    placeholder="Search nodes by label or id…"
                    prop:value=search_query
                    on:input=move |ev| search_query.set(event_target_value(&ev))
                />
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
                when=move || !loading.get() && error_msg.get().is_none()
                fallback=|| ()
            >
                <div class="spp-graph__layout">
                    <aside class="spp-graph__nodes">
                        <h3>"Nodes"</h3>
                        <Show
                            when=move || !nodes.with(|n| n.is_empty())
                            fallback=|| {
                                view! {
                                    <EmptyState message="No graph nodes. The graph is built as conversations, issues, and incidents are linked." />
                                }
                            }
                        >
                            <ul class="spp-graph__node-list">
                                {move || nodes.with(|items| {
                                    items.iter().map(|n| {
                                        let id = n.get("local_id").and_then(|v| v.as_i64()).unwrap_or(0);
                                        let kind = n.get("kind").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                        let label = n.get("label").and_then(|v| v.as_str()).unwrap_or("(no label)").to_string();
                                        let kind_for_click = kind.clone();
                                        let kind_for_badge = kind.clone();
                                        let kind_for_sel = kind.clone();
                                        let is_selected =
                                            move || selected_node.get() == Some((kind_for_sel.clone(), id));
                                        view! {
                                            <li
                                                class="spp-graph__node"
                                                class:is-selected=is_selected
                                                on:click=move |_| {
                                                    selected_node.set(Some((kind_for_click.clone(), id)));
                                                }
                                            >
                                                <span class="spp-badge spp-badge--status">{kind_for_badge}</span>
                                                <span class="spp-graph__node-label">{label}</span>
                                                <span class="spp-graph__node-id">{"#"}{id.to_string()}</span>
                                            </li>
                                        }
                                    }).collect::<Vec<_>>()
                                })}
                            </ul>
                        </Show>
                    </aside>

                    <main class="spp-graph__neighbors">
                        <Show
                            when=move || selected_node.get().is_some()
                            fallback=|| {
                                view! {
                                    <EmptyState message="Select a node to view its neighbors." />
                                }
                            }
                        >
                            <Show
                                when=move || !neighbors.with(|n| n.is_empty())
                                fallback=|| {
                                    view! {
                                        <EmptyState message="This node has no neighbors (no edges connected)." />
                                    }
                                }
                            >
                                <h3>"Neighbors"</h3>
                                <ul class="spp-graph__neighbor-list">
                                    {move || neighbors.with(|items| {
                                        items.iter().map(|n| {
                                            let far = n.get("target").unwrap_or(n);
                                            let id = far.get("local_id").and_then(|v| v.as_i64()).unwrap_or(0);
                                            let kind = far.get("kind").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                            let label = far.get("label").and_then(|v| v.as_str()).unwrap_or("(no label)").to_string();
                                            let relation = n.get("relation").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                            let origin = n.get("origin").and_then(|v| v.as_str()).unwrap_or("sync").to_string();
                                            let origin_class = if origin == "human" { "spp-badge--human" } else { "spp-badge--sync" };
                                            view! {
                                                <li class="spp-graph__neighbor">
                                                    <span class="spp-badge">{kind}</span>
                                                    <span class="spp-graph__neighbor-label">{label}</span>
                                                    <span class="spp-badge spp-badge--status">{relation}</span>
                                                    <span class=format!("spp-badge {origin_class}")>{origin}</span>
                                                    <span class="spp-graph__neighbor-id">{"#"}{id.to_string()}</span>
                                                </li>
                                            }
                                        }).collect::<Vec<_>>()
                                    })}
                                </ul>
                            </Show>
                        </Show>
                    </main>
                </div>
            </Show>
        </div>
    }
}

fn url_encode(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            ' ' => "+".to_string(),
            c if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' || c == '~' => {
                c.to_string()
            }
            c => format!("%{:02X}", c as u8),
        })
        .collect()
}
