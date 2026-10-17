use std::sync::Arc;

use leptos::prelude::*;
use leptos_router::hooks::{use_location, use_navigate, use_query_map};

use crate::components::state_view::{EmptyState, LoadingState};

#[component]
pub fn DocsPage() -> impl IntoView {
    let stats = RwSignal::new(None::<serde_json::Value>);
    let collections = RwSignal::new(Vec::<serde_json::Value>::new());
    let articles = RwSignal::new(Vec::<serde_json::Value>::new());
    let loading = RwSignal::new(true);
    let error_msg = RwSignal::new(None::<String>);
    let search_query = RwSignal::new(String::new());
    let semantic = RwSignal::new(false);
    let collection_filter = RwSignal::new(String::new());
    let status_filter = RwSignal::new("all".to_string());
    let reload = RwSignal::new(0u32);

    Effect::new(move |_| {
        let _tick = reload.get();
        let stats = stats;
        let collections = collections;
        let articles = articles;
        let loading = loading;
        let error_msg = error_msg;
        let q = search_query.get();
        let sem = semantic.get();
        let cf = collection_filter.get();
        let sf = status_filter.get();
        wasm_bindgen_futures::spawn_local(async move {
            let stats_result = crate::api::get_json::<serde_json::Value>("/api/docs/stats").await;
            let collections_result =
                crate::api::get_json::<serde_json::Value>("/api/docs/collections").await;
            let articles_result = if q.trim().is_empty() {
                let mut params = vec!["limit=50".to_string()];
                if !cf.is_empty() {
                    params.push(format!("collection={cf}"));
                }
                if sf != "all" {
                    params.push(format!("status={sf}"));
                }
                crate::api::get_json::<serde_json::Value>(&format!(
                    "/api/docs/articles?{}",
                    params.join("&")
                ))
                .await
            } else {
                let body = serde_json::json!({
                    "query": q,
                    "semantic": sem,
                    "collection": cf,
                    "status": sf,
                });
                crate::api::post_json::<serde_json::Value>("/api/docs/search", Some(&body)).await
            };
            match (stats_result, collections_result, articles_result) {
                (Ok(s), Ok(c), Ok(a)) => {
                    stats.set(Some(s));
                    collections.set(
                        c.get("collections")
                            .and_then(|v| v.as_array())
                            .cloned()
                            .unwrap_or_default(),
                    );
                    let items = a
                        .get("articles")
                        .or_else(|| a.get("hits"))
                        .and_then(|v| v.as_array())
                        .cloned()
                        .unwrap_or_default();
                    articles.set(items);
                    loading.set(false);
                }
                (Err(e), _, _) | (_, Err(e), _) | (_, _, Err(e)) => {
                    error_msg.set(Some(e));
                    loading.set(false);
                }
            }
        });
    });

    let query_map = use_query_map();
    let location = use_location();
    let navigate = use_navigate();
    let reading = RwSignal::new(None::<i64>);
    Effect::new(move |_| {
        let m = query_map.get();
        let article = crate::url_state::query_pos_int(&m, "article");
        if reading.get_untracked() != article {
            reading.set(article);
        }
    });
    let open_article: Arc<dyn Fn(i64) + Send + Sync> = {
        let navigate = navigate.clone();
        let pathname = location.pathname;
        Arc::new(move |id: i64| {
            reading.set(Some(id));
            navigate(
                &format!(
                    "{}{}",
                    pathname.get_untracked(),
                    crate::url_state::query_string(&[("article", Some(id.to_string()))])
                ),
                leptos_router::NavigateOptions {
                    replace: true,
                    ..Default::default()
                },
            );
        })
    };
    let close_article: Arc<dyn Fn() + Send + Sync> = {
        let navigate = navigate.clone();
        let pathname = location.pathname;
        Arc::new(move || {
            reading.set(None);
            navigate(
                &pathname.get_untracked(),
                leptos_router::NavigateOptions {
                    replace: true,
                    ..Default::default()
                },
            );
        })
    };
    let open_article_stored = StoredValue::new_local(open_article);
    let close_article_stored = StoredValue::new_local(close_article);

    view! {
        <div class="spp-page spp-page--docs">
            <h2 class="spp-page__title">"Docs"</h2>
            <p class="spp-page__intro">
                "The documents knowledge base: collections, articles, and freshness across every synced source."
            </p>

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
                <div class="spp-docs__stats">
                    {move || {
                        let s = stats.get().unwrap_or_default();
                        let total = s.get("total_articles").and_then(|v| v.as_i64()).unwrap_or(0);
                        let fresh = s.get("fresh").and_then(|v| v.as_i64()).unwrap_or(0);
                        let stale = s.get("stale").and_then(|v| v.as_i64()).unwrap_or(0);
                        let sources = s.get("sources").and_then(|v| v.as_i64()).unwrap_or(0);
                        view! {
                            <div class="spp-docs__stat">
                                <span class="spp-docs__stat-value">{total.to_string()}</span>
                                <span class="spp-docs__stat-label">"articles"</span>
                            </div>
                            <div class="spp-docs__stat">
                                <span class="spp-docs__stat-value">{sources.to_string()}</span>
                                <span class="spp-docs__stat-label">"sources"</span>
                            </div>
                            <div class="spp-docs__stat">
                                <span class="spp-docs__stat-value">{fresh.to_string()}</span>
                                <span class="spp-docs__stat-label">"fresh"</span>
                            </div>
                            <div class="spp-docs__stat">
                                <span class="spp-docs__stat-value">{stale.to_string()}</span>
                                <span class="spp-docs__stat-label">"stale"</span>
                            </div>
                        }
                    }}
                </div>

                <div class="spp-docs__filters">
                    <input
                        class="spp-input"
                        type="text"
                        placeholder="Search docs (keyword + semantic)…"
                        prop:value=search_query
                        on:input=move |ev| {
                            search_query.set(event_target_value(&ev));
                            reload.set(reload.get_untracked() + 1);
                        }
                    />
                    <select
                        class="spp-input"
                        prop:value=collection_filter
                        on:change=move |ev| {
                            collection_filter.set(event_target_value(&ev));
                            reload.set(reload.get_untracked() + 1);
                        }
                    >
                        <option value="">"All collections"</option>
                        {move || collections.with(|cs| {
                            cs.iter().map(|c| {
                                let name = c.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                view! { <option value=name.clone()>{name.clone()}</option> }
                            }).collect::<Vec<_>>()
                        })}
                    </select>
                    <select
                        class="spp-input"
                        prop:value=status_filter
                        on:change=move |ev| {
                            status_filter.set(event_target_value(&ev));
                            reload.set(reload.get_untracked() + 1);
                        }
                    >
                        <option value="all">"All statuses"</option>
                        <option value="fresh">"Fresh"</option>
                        <option value="stale">"Stale"</option>
                    </select>
                    <label class="spp-docs__semantic-toggle">
                        <input
                            type="checkbox"
                            prop:checked=semantic
                            on:change=move |ev| {
                                semantic.set(event_target_checked(&ev));
                                reload.set(reload.get_untracked() + 1);
                            }
                        />
                        <span>"Semantic"</span>
                    </label>
                </div>

                <Show
                    when=move || !collections.with(|c| c.is_empty())
                    fallback=|| ()
                >
                    <section class="spp-docs__collections">
                        <h3>"Collections"</h3>
                        <ul class="spp-docs__collection-list">
                            {move || {
                                collections.with(|cs| {
                                    cs.iter()
                                        .map(|c| {
                                            let name = c
                                                .get("name")
                                                .and_then(|v| v.as_str())
                                                .unwrap_or("(unnamed)")
                                                .to_string();
                                            let count = c
                                                .get("article_count")
                                                .and_then(|v| v.as_i64())
                                                .unwrap_or(0);
                                            view! {
                                                <li class="spp-docs__collection">
                                                    <span>{name}</span>
                                                    <span class="spp-badge">{count.to_string()}</span>
                                                </li>
                                            }
                                        })
                                        .collect::<Vec<_>>()
                                })
                            }}
                        </ul>
                    </section>
                </Show>

                <Show
                    when=move || !articles.with(|a| a.is_empty())
                    fallback=|| {
                        view! {
                            <EmptyState message="No doc articles yet. They appear once knowledge docs are imported or synced." />
                        }
                    }
                >
                    <table class="spp-table">
                        <thead>
                            <tr>
                                <th>"Article"</th>
                                <th>"Collection"</th>
                                <th>"Status"</th>
                                <th>"Views"</th>
                                <th>"Updated"</th>
                            </tr>
                        </thead>
                        <tbody>
                            {move || {
                                articles.with(|arts| {
                                    arts.iter()
                                        .map(|a| {
                                            let id = a.get("id").and_then(|v| v.as_i64()).unwrap_or_default();
                                            let name = a
                                                .get("name")
                                                .and_then(|v| v.as_str())
                                                .unwrap_or("(untitled)")
                                                .to_string();
                                            let preview = a
                                                .get("preview")
                                                .and_then(|v| v.as_str())
                                                .map(|p| p.chars().take(110).collect::<String>())
                                                .unwrap_or_default();
                                            let collection = a
                                                .get("collection_name")
                                                .and_then(|v| v.as_str())
                                                .unwrap_or("—")
                                                .to_string();
                                            let category = a
                                                .get("category_name")
                                                .and_then(|v| v.as_str())
                                                .map(str::to_string);
                                            let status = a
                                                .get("status")
                                                .and_then(|v| v.as_str())
                                                .unwrap_or("—")
                                                .to_string();
                                            let views = a.get("views").and_then(|v| v.as_i64());
                                            let updated = a
                                                .get("remote_updated_at")
                                                .or_else(|| a.get("remote_created_at"))
                                                .and_then(|v| v.as_str())
                                                .unwrap_or("")
                                                .get(..10)
                                                .unwrap_or("")
                                                .to_string();
                                            let open_row = open_article_stored.with_value(Arc::clone);
                                            view! {
                                                <tr class="spp-table__row-clickable" on:click=move |_| open_row(id)>
                                                    <td>
                                                        <strong>{name}</strong>
                                                        {if !preview.is_empty() {
                                                            view! {
                                                                <div class="spp-muted spp-text-xs">{format!("{preview}…")}</div>
                                                            }.into_any()
                                                        } else {
                                                            ().into_any()
                                                        }}
                                                    </td>
                                                    <td>
                                                        {collection}
                                                        {if let Some(category) = category {
                                                            view! {
                                                                <div class="spp-muted spp-text-xs">{category}</div>
                                                            }.into_any()
                                                        } else {
                                                            ().into_any()
                                                        }}
                                                    </td>
                                                    <td>
                                                        <span class=match status.as_str() {
                                                            "published" => "spp-badge spp-badge--ok",
                                                            "internal" => "spp-badge spp-badge--warn",
                                                            _ => "spp-badge",
                                                        }>
                                                            {status.clone()}
                                                        </span>
                                                    </td>
                                                    <td class="spp-table__cell-muted">
                                                        {if let Some(v) = views { v.to_string() } else { "—".to_string() }}
                                                    </td>
                                                    <td class="spp-table__cell-muted">{updated}</td>
                                                </tr>
                                            }
                                        })
                                        .collect::<Vec<_>>()
                                })
                            }}
                        </tbody>
                    </table>
                </Show>
            </Show>

            <Show when=move || reading.get().is_some() fallback=|| ()>
                {move || {
                    let id = reading.get().unwrap_or_default();
                    let close = close_article_stored.with_value(Arc::clone);
                    view! { <ArticleReader id=id on_close=close /> }
                }}
            </Show>
        </div>
    }
}

#[component]
fn ArticleReader(id: i64, on_close: Arc<dyn Fn() + Send + Sync>) -> impl IntoView {
    let article = RwSignal::new(None::<serde_json::Value>);
    let loading = RwSignal::new(true);
    let error_msg = RwSignal::new(None::<String>);

    Effect::new(move |_| {
        let article = article;
        let loading = loading;
        let error_msg = error_msg;
        loading.set(true);
        wasm_bindgen_futures::spawn_local(async move {
            let path = format!("/api/docs/articles/{id}");
            match crate::api::get_json::<serde_json::Value>(&path).await {
                Ok(v) => {
                    article.set(v.get("article").cloned());
                    error_msg.set(None);
                }
                Err(e) => error_msg.set(Some(e)),
            }
            loading.set(false);
        });
    });

    view! {
        <div class="spp-overlay" role="dialog" aria-modal="true" aria-label="Article reader">
            <div class="spp-modal spp-modal--reader">
                <div class="spp-modal__head">
                    <h3 class="spp-modal__title">
                        {move || {
                            article
                                .get()
                                .and_then(|a| a.get("name").and_then(|n| n.as_str()).map(str::to_string))
                                .unwrap_or_else(|| "Article".to_string())
                        }}
                    </h3>
                    <button
                        class="spp-button spp-button--ghost spp-button--tiny"
                        aria-label="Close reader"
                        on:click=move |_| on_close()
                    >
                        "\u{d7}"
                    </button>
                </div>
                <Show when=move || loading.get() fallback=|| ()>
                    <LoadingState />
                </Show>
                <Show when=move || error_msg.get().is_some() fallback=|| ()>
                    <div class="spp-state spp-state--error">
                        <span class="spp-state__icon" aria-hidden="true">"⚠"</span>
                        <p class="spp-state__body">"Could not open this article."</p>
                        <p class="spp-muted spp-text-xs">{move || error_msg.get().unwrap_or_default()}</p>
                    </div>
                </Show>
                <Show
                    when=move || !loading.get() && error_msg.get().is_none()
                    fallback=|| ()
                >
                    {move || {
                        let a = match article.get() {
                            Some(a) => a,
                            None => return view! { <div></div> }.into_any(),
                        };
                        let status = a.get("status").and_then(|v| v.as_str()).unwrap_or("—").to_string();
                        let collection = a
                            .get("collection_name")
                            .and_then(|v| v.as_str())
                            .unwrap_or("—")
                            .to_string();
                        let category = a
                            .get("category_name")
                            .and_then(|v| v.as_str())
                            .map(str::to_string);
                        let views = a.get("views").and_then(|v| v.as_i64());
                        let text = a.get("text").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        view! {
                            <div class="spp-doc-reader__badges">
                                <span class=match status.as_str() {
                                    "published" => "spp-badge spp-badge--ok",
                                    "internal" => "spp-badge spp-badge--warn",
                                    _ => "spp-badge",
                                }>
                                    {status.clone()}
                                </span>
                                <span class="spp-badge">{collection}</span>
                                {if let Some(category) = category {
                                    view! { <span class="spp-badge">{category}</span> }.into_any()
                                } else {
                                    ().into_any()
                                }}
                                {if let Some(views) = views {
                                    view! {
                                        <span class="spp-badge">{format!("{views} views")}</span>
                                    }.into_any()
                                } else {
                                    ().into_any()
                                }}
                            </div>
                            {if text.is_empty() {
                                view! {
                                    <div class="spp-doc-reader__content spp-muted">
                                        "No text stored for this article."
                                    </div>
                                }.into_any()
                            } else {
                                view! { <div class="spp-doc-reader__content">{text}</div> }.into_any()
                            }}
                            <p class="spp-muted spp-text-xs">
                                "Mirrored read-only from Help Scout Docs; edits happen in Help Scout and arrive on the next sync."
                            </p>
                        }.into_any()
                    }}
                </Show>
            </div>
        </div>
    }
}
