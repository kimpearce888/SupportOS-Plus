use std::sync::Arc;

use leptos::prelude::*;
use leptos_router::hooks::{use_location, use_navigate, use_query_map};

use crate::components::GapsTab;
use crate::components::state_view::{EmptyState, LoadingState};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KnowledgeTab {
    Documents,
    Sources,
    Freshness,
    Gaps,
}

#[component]
pub fn KnowledgePage() -> impl IntoView {
    let tab = RwSignal::new(KnowledgeTab::Documents);
    let reload_documents = RwSignal::new(0u32);
    let importing = RwSignal::new(false);

    let query_map = use_query_map();
    let location = use_location();
    let navigate = use_navigate();
    let reading = RwSignal::new(None::<i64>);
    Effect::new(move |_| {
        let m = query_map.get();
        let doc = crate::url_state::query_pos_int(&m, "doc");
        if reading.get_untracked() != doc {
            reading.set(doc);
        }
    });
    let open_doc: Arc<dyn Fn(i64) + Send + Sync> = {
        let navigate = navigate.clone();
        let pathname = location.pathname;
        Arc::new(move |id: i64| {
            reading.set(Some(id));
            navigate(
                &format!(
                    "{}{}",
                    pathname.get_untracked(),
                    crate::url_state::query_string(&[("doc", Some(id.to_string()))])
                ),
                leptos_router::NavigateOptions {
                    replace: true,
                    ..Default::default()
                },
            );
        })
    };
    let close_doc: Arc<dyn Fn() + Send + Sync> = {
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
    let open_doc_stored = StoredValue::new_local(open_doc);
    let close_doc_stored = StoredValue::new_local(close_doc);

    let do_reindex = move |_| {
        wasm_bindgen_futures::spawn_local(async move {
            match crate::api::post_json::<serde_json::Value>("/api/knowledge/reindex", None).await {
                Ok(r) => {
                    let message = r
                        .get("message")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Reindexing queued.");
                    crate::toasts::success(message);
                    reload_documents.update(|t| *t += 1);
                }
                Err(e) => crate::toasts::error(e),
            }
        });
    };

    view! {
        <div class="spp-page spp-page--knowledge">
            <div class="spp-flex spp-flex--between">
                <div>
                    <h2 class="spp-page__title">"Knowledge"</h2>
                    <p class="spp-page__intro">
                        "Local knowledge base — indexed by FTS and (optionally) vector search. Visibility separates customer-safe from internal-only."
                    </p>
                </div>
                <div class="spp-flex">
                    <button class="spp-button" on:click=do_reindex>"Reindex"</button>
                    <button
                        class="spp-button spp-button--primary"
                        on:click=move |_| importing.set(true)
                    >
                        "+ Import"
                    </button>
                </div>
            </div>

            <div class="spp-tabs">
                <button
                    class="spp-tab"
                    class:is-active=move || tab.get() == KnowledgeTab::Documents
                    on:click=move |_| tab.set(KnowledgeTab::Documents)
                >
                    "Documents"
                </button>
                <button
                    class="spp-tab"
                    class:is-active=move || tab.get() == KnowledgeTab::Sources
                    on:click=move |_| tab.set(KnowledgeTab::Sources)
                >
                    "Sources"
                </button>
                <button
                    class="spp-tab"
                    class:is-active=move || tab.get() == KnowledgeTab::Freshness
                    on:click=move |_| tab.set(KnowledgeTab::Freshness)
                >
                    "Freshness"
                </button>
                <button
                    class="spp-tab"
                    class:is-active=move || tab.get() == KnowledgeTab::Gaps
                    on:click=move |_| tab.set(KnowledgeTab::Gaps)
                >
                    "Gaps"
                </button>
            </div>

            <Show
                when=move || tab.get() == KnowledgeTab::Documents
                fallback=|| ()
            >
                {move || {
                    let open = open_doc_stored.with_value(Arc::clone);
                    view! { <DocumentsTab reload=reload_documents open_doc=open /> }
                }}
            </Show>
            <Show
                when=move || tab.get() == KnowledgeTab::Sources
                fallback=|| ()
            >
                <SourcesTab />
            </Show>
            <Show
                when=move || tab.get() == KnowledgeTab::Freshness
                fallback=|| ()
            >
                <FreshnessTab />
            </Show>
            <Show
                when=move || tab.get() == KnowledgeTab::Gaps
                fallback=|| ()
            >
                <GapsTab />
            </Show>

            <Show when=move || importing.get() fallback=|| ()>
                <ImportKnowledgeModal
                    on_close=Arc::new(move || importing.set(false))
                    on_imported=Arc::new(move || {
                        importing.set(false);
                        reload_documents.update(|t| *t += 1);
                    })
                />
            </Show>

            <Show when=move || reading.get().is_some() fallback=|| ()>
                {move || {
                    let id = reading.get().unwrap_or_default();
                    let close = close_doc_stored.with_value(Arc::clone);
                    view! { <DocReader id=id on_close=close /> }
                }}
            </Show>
        </div>
    }
}

#[component]
fn DocumentsTab(reload: RwSignal<u32>, open_doc: Arc<dyn Fn(i64) + Send + Sync>) -> impl IntoView {
    let documents = RwSignal::new(Vec::<serde_json::Value>::new());
    let loading = RwSignal::new(true);
    let error_msg = RwSignal::new(None::<String>);
    let open_doc_for_table = StoredValue::new_local(open_doc);

    Effect::new(move |_| {
        let _tick = reload.get();
        let documents = documents;
        let loading = loading;
        let error_msg = error_msg;
        loading.set(true);
        wasm_bindgen_futures::spawn_local(async move {
            match crate::api::get_json::<serde_json::Value>("/api/knowledge/documents").await {
                Ok(v) => {
                    let items = v
                        .get("documents")
                        .and_then(|d| d.as_array())
                        .cloned()
                        .unwrap_or_default();
                    documents.set(items);
                    error_msg.set(None);
                }
                Err(e) => error_msg.set(Some(e)),
            }
            loading.set(false);
        });
    });

    view! {
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
            <Show
                when=move || !documents.with(|d| d.is_empty())
                fallback=|| {
                    view! {
                        <EmptyState message="No knowledge documents yet. Import Markdown/TXT/CSV/JSON/HTML files or paste content directly. Knowledge feeds AI drafts and search." />
                    }
                }
            >
                {move || {
                    let open = open_doc_for_table.with_value(Arc::clone);
                    view! { <DocumentsTable documents=documents reload=reload open_doc=open /> }
                }}
            </Show>
        </Show>
    }
}

#[component]
fn DocumentsTable(
    documents: RwSignal<Vec<serde_json::Value>>,
    reload: RwSignal<u32>,
    open_doc: Arc<dyn Fn(i64) + Send + Sync>,
) -> impl IntoView {
    let rows = move || {
        documents
            .get()
            .into_iter()
            .map(|doc| {
                view! {
                    <DocumentRow doc=doc reload=reload open_doc=Arc::clone(&open_doc) />
                }
            })
            .collect::<Vec<_>>()
    };
    view! {
        <div class="spp-card spp-knowledge__card">
            <table class="spp-table">
                <thead>
                    <tr>
                        <th>"Title"</th>
                        <th>"Visibility"</th>
                        <th>"Version"</th>
                        <th>"Chunks"</th>
                        <th>"Updated"</th>
                        <th></th>
                    </tr>
                </thead>
                <tbody>{rows}</tbody>
            </table>
        </div>
    }
}

#[component]
fn DocumentRow(
    doc: serde_json::Value,
    reload: RwSignal<u32>,
    open_doc: Arc<dyn Fn(i64) + Send + Sync>,
) -> impl IntoView {
    let id = doc.get("id").and_then(|v| v.as_i64()).unwrap_or_default();
    let title = doc
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("(untitled)")
        .to_string();
    let preview: String = doc
        .get("content_preview")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .chars()
        .take(90)
        .collect();
    let visibility = doc
        .get("visibility")
        .and_then(|v| v.as_str())
        .unwrap_or("internal_only")
        .to_string();
    let version = doc.get("version").and_then(|v| v.as_i64()).unwrap_or(1);
    let chunk_count = doc.get("chunk_count").and_then(|v| v.as_i64()).unwrap_or(0);
    let updated = doc
        .get("updated_at")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let do_delete = move |_| {
        wasm_bindgen_futures::spawn_local(async move {
            let path = format!("/api/knowledge/documents/{id}");
            match crate::api::delete_json::<serde_json::Value>(&path).await {
                Ok(_) => {
                    crate::toasts::success("Document deleted.");
                    reload.update(|t| *t += 1);
                }
                Err(e) => crate::toasts::error(e),
            }
        });
    };

    view! {
        <tr class="spp-table__row-clickable" on:click=move |_| open_doc(id)>
            <td>
                <strong class="spp-knowledge__title">{title.clone()}</strong>
                <div class="spp-muted spp-text-xs">{format!("{preview}…")}</div>
            </td>
            <td>
                <span class=if visibility == "customer_safe" {
                    "spp-badge spp-badge--ok"
                } else {
                    "spp-badge"
                }>
                    {if visibility == "customer_safe" { "customer-safe" } else { "internal-only" }}
                </span>
            </td>
            <td class="spp-table__cell-muted">{format!("v{version}")}</td>
            <td>{chunk_count.to_string()}</td>
            <td class="spp-table__cell-muted">{updated.get(..10).unwrap_or("").to_string()}</td>
            <td>
                <button
                    class="spp-button spp-button--ghost spp-button--tiny"
                    aria-label=format!("Delete {title}")
                    on:click=move |ev: leptos::ev::MouseEvent| {
                        ev.stop_propagation();
                        let confirmed = web_sys::window()
                            .and_then(|w| {
                                w.confirm_with_message(&format!(
                                    "Delete \"{title}\"? This also removes its search-index entries."
                                ))
                                .ok()
                            })
                            .unwrap_or(false);
                        if confirmed {
                            do_delete(());
                        }
                    }
                >
                    "Delete"
                </button>
            </td>
        </tr>
    }
}

#[component]
fn ImportKnowledgeModal(
    on_close: Arc<dyn Fn() + Send + Sync>,
    on_imported: Arc<dyn Fn() + Send + Sync>,
) -> impl IntoView {
    let title = RwSignal::new(String::new());
    let content = RwSignal::new(String::new());
    let visibility = RwSignal::new("internal_only".to_string());
    let source_name = RwSignal::new("Manual import".to_string());
    let submitting = RwSignal::new(false);
    let importable = RwSignal::new(None::<serde_json::Value>);
    let file_pending = RwSignal::new(None::<String>);

    wasm_bindgen_futures::spawn_local(async move {
        if let Ok(v) = crate::api::get_json::<serde_json::Value>("/api/knowledge/importable").await
        {
            importable.set(Some(v));
        }
    });

    let submit = {
        let on_imported = Arc::clone(&on_imported);
        move |ev: leptos::ev::SubmitEvent| {
            ev.prevent_default();
            if submitting.get() {
                return;
            }
            if let Some(file) = file_pending.get() {
                file_pending.set(None);
                submitting.set(true);
                let on_imported = Arc::clone(&on_imported);
                wasm_bindgen_futures::spawn_local(async move {
                    let body = serde_json::json!({
                        "path": file,
                        "visibility": "internal_only",
                    });
                    match crate::api::post_json::<serde_json::Value>(
                        "/api/knowledge/import-file",
                        Some(&body),
                    )
                    .await
                    {
                        Ok(r) if r.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) => {
                            let n = r.get("imported").and_then(|v| v.as_i64()).unwrap_or(0);
                            crate::toasts::success(format!("Imported {n} document(s) from file."));
                            on_imported();
                        }
                        Ok(r) => {
                            crate::toasts::error(
                                r.get("message")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("Import failed"),
                            );
                        }
                        Err(e) => crate::toasts::error(e),
                    }
                });
                return;
            }
            let title_now = title.get();
            let content_now = content.get();
            if title_now.trim().is_empty() || content_now.trim().is_empty() {
                return;
            }
            submitting.set(true);
            let body = serde_json::json!({
                "sourceName": source_name.get(),
                "visibility": visibility.get(),
                "documents": [{
                    "title": title_now,
                    "content": content_now,
                    "format": "markdown",
                }],
            });
            let on_imported = Arc::clone(&on_imported);
            wasm_bindgen_futures::spawn_local(async move {
                match crate::api::post_json::<serde_json::Value>(
                    "/api/knowledge/import",
                    Some(&body),
                )
                .await
                {
                    Ok(r) if r.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) => {
                        let n = r.get("imported").and_then(|v| v.as_i64()).unwrap_or(0);
                        crate::toasts::success(format!("Imported {n} document(s)."));
                        on_imported();
                    }
                    Ok(r) => {
                        crate::toasts::error(
                            r.get("message")
                                .and_then(|v| v.as_str())
                                .unwrap_or("Import failed"),
                        );
                    }
                    Err(e) => crate::toasts::error(e),
                }
            });
        }
    };

    view! {
        <div class="spp-overlay" role="dialog" aria-modal="true">
            <div class="spp-modal spp-modal--form spp-modal--wide">
                <h3 class="spp-modal__title">"Import knowledge"</h3>
                <form on:submit=submit>
                    <div class="spp-form-grid">
                        <label class="spp-form-grid__label">"Title *"</label>
                        <input
                            class="spp-input"
                            prop:value=title
                            on:input=move |ev| title.set(event_target_value(&ev))
                            required=true
                        />
                        <label class="spp-form-grid__label">"Visibility"</label>
                        <select
                            class="spp-input"
                            prop:value=visibility
                            on:change=move |ev| visibility.set(event_target_value(&ev))
                        >
                            <option value="internal_only">"internal-only (never in customer drafts)"</option>
                            <option value="customer_safe">"customer-safe (may support customer drafts)"</option>
                        </select>
                    </div>
                    <div class="spp-form-grid spp-mt-8">
                        <label class="spp-form-grid__label">"Source name"</label>
                        <input
                            class="spp-input"
                            prop:value=source_name
                            on:input=move |ev| source_name.set(event_target_value(&ev))
                        />
                        <label class="spp-form-grid__label">"Content (Markdown) *"</label>
                        <textarea
                            class="spp-input spp-knowledge__import-textarea"
                            prop:value=content
                            on:input=move |ev| content.set(event_target_value(&ev))
                            required=true
                        >
                        </textarea>
                    </div>
                    <button
                        class="spp-button spp-button--primary spp-mt-8"
                        type="submit"
                        disabled=move || {
                            submitting.get()
                                || (file_pending.get().is_none()
                                    && (title.get().trim().is_empty()
                                        || content.get().trim().is_empty()))
                        }
                    >
                        "Import document"
                    </button>
                </form>

                <div class="spp-knowledge__import-divider"></div>
                <h4 class="spp-card__title">"Import from the knowledge-import folder"</h4>
                <p class="spp-muted spp-text-xs">
                    "For safety, file imports must live in "
                    <span class="spp-mono">
                        {move || {
                            importable
                                .get()
                                .and_then(|v| v.get("dir").and_then(|d| d.as_str()).map(str::to_string))
                                .unwrap_or_else(|| "./knowledge-import".to_string())
                        }}
                    </span>
                    ". Supported: MD, TXT, CSV, JSON, HTML, PDF, DOCX."
                </p>
                {move || {
                    let listing = importable.get();
                    let files: Vec<String> = listing
                        .as_ref()
                        .and_then(|v| v.get("files").and_then(|f| f.as_array()).cloned())
                        .map(|rows| {
                            rows.iter()
                                .filter_map(|f| f.as_str().map(str::to_string))
                                .collect()
                        })
                        .unwrap_or_default();
                    if files.is_empty() {
                        view! {
                            <span class="spp-muted">
                                "The folder is empty or does not exist. Create it and copy your documents there."
                            </span>
                        }
                        .into_any()
                    } else {
                        files
                            .into_iter()
                            .map(|f| {
                                let file_label = f.clone();
                                let is_pending_class = {
                                    let f = f.clone();
                                    move || file_pending.get().as_deref() == Some(f.as_str())
                                };
                                let is_pending_label = {
                                    let f = f.clone();
                                    move || file_pending.get().as_deref() == Some(f.as_str())
                                };
                                view! {
                                    <div class="spp-flex spp-flex--between spp-knowledge__import-file">
                                        <span class="spp-mono">{file_label}</span>
                                        <button
                                            class=move || {
                                                format!(
                                                    "spp-button spp-button--small{}",
                                                    if is_pending_class() { " spp-button--warn" } else { "" },
                                                )
                                            }
                                            on:click=move |_| {
                                                file_pending.set(Some(f.clone()));
                                            }
                                        >
                                            {move || {
                                                if is_pending_label() {
                                                    "Selected — Import document".to_string()
                                                } else {
                                                    "Import".to_string()
                                                }
                                            }}
                                        </button>
                                    </div>
                                }.into_any()
                            })
                            .collect::<Vec<_>>()
                            .into_any()
                    }
                }}
            </div>
            <div class="spp-knowledge__modal-close">
                <button class="spp-button" on:click=move |_| on_close()>"Close"</button>
            </div>
        </div>
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct KnowledgeSource {
    pub id: i64,
    pub name: String,
    pub kind: String,
    pub visibility: String,
    pub document_count: i64,
}

#[must_use]
pub fn parse_sources(v: &serde_json::Value) -> Vec<KnowledgeSource> {
    v.get("sources")
        .and_then(|s| s.as_array())
        .map(|rows| {
            rows.iter()
                .map(|s| KnowledgeSource {
                    id: s.get("id").and_then(|x| x.as_i64()).unwrap_or(0),
                    name: s
                        .get("name")
                        .and_then(|x| x.as_str())
                        .unwrap_or_default()
                        .to_string(),
                    kind: s
                        .get("kind")
                        .and_then(|x| x.as_str())
                        .unwrap_or_default()
                        .to_string(),
                    visibility: s
                        .get("visibility")
                        .and_then(|x| x.as_str())
                        .unwrap_or_default()
                        .to_string(),
                    document_count: s
                        .get("document_count")
                        .and_then(|x| x.as_i64())
                        .unwrap_or(0),
                })
                .collect()
        })
        .unwrap_or_default()
}

#[component]
fn SourcesTab() -> impl IntoView {
    let sources = RwSignal::new(Vec::<KnowledgeSource>::new());
    let loading = RwSignal::new(true);
    let error_msg = RwSignal::new(None::<String>);

    Effect::new(move |_| {
        let sources = sources;
        let loading = loading;
        let error_msg = error_msg;
        wasm_bindgen_futures::spawn_local(async move {
            match crate::api::get_json::<serde_json::Value>("/api/knowledge/sources").await {
                Ok(v) => {
                    sources.set(parse_sources(&v));
                    loading.set(false);
                }
                Err(e) => {
                    error_msg.set(Some(e));
                    loading.set(false);
                }
            }
        });
    });

    view! {
        <div class="spp-card">
            <h3 class="spp-card__title">"Knowledge sources"</h3>
            <Show when=move || loading.get() fallback=|| ()>
                <LoadingState />
            </Show>
            <Show when=move || error_msg.get().is_some() fallback=|| ()>
                <div class="spp-state spp-state--error">
                    <p class="spp-state__body">{move || error_msg.get().unwrap_or_default()}</p>
                </div>
            </Show>
            <Show
                when=move || !loading.get() && error_msg.get().is_none()
                fallback=|| ()
            >
                <Show
                    when=move || !sources.with(|s| s.is_empty())
                    fallback=|| view! { <EmptyState message="No sources yet." /> }
                >
                    <div class="spp-flex spp-flex--col spp-gap-4">
                        {sources.get()
                            .iter()
                            .map(|s| {
                                let badge = if s.visibility == "customer_safe" {
                                    "spp-badge spp-badge--ok"
                                } else {
                                    "spp-badge"
                                };
                                let label = if s.visibility == "customer_safe" {
                                    "customer-safe"
                                } else {
                                    "internal-only"
                                };
                                view! {
                                    <div class="spp-flex spp-flex--between">
                                        <strong class="spp-text-sm">{s.name.clone()}</strong>
                                        <span class="spp-text-xs spp-muted">
                                            {format!("{} docs · {}", s.document_count, s.kind)}
                                            <span class=badge>{label}</span>
                                        </span>
                                    </div>
                                }
                            })
                            .collect::<Vec<_>>()}
                    </div>
                </Show>
            </Show>
        </div>
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct FreshnessOverview {
    pub stale: i64,
    pub fresh: i64,
    pub documents: Vec<FreshnessDocument>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct FreshnessDocument {
    pub id: i64,
    pub title: String,
    pub freshness_status: String,
    pub last_reviewed_at: Option<String>,
    pub last_verified_at: Option<String>,
    pub source_name: Option<String>,
    pub visibility: String,
    pub version: i64,
    pub days_since_update: Option<i64>,
    pub days_since_review: Option<i64>,
    pub usage_count: i64,
    pub stale: bool,
    pub unreviewed_long: bool,
    pub unverified: bool,
    pub low_usage: bool,
    pub conflicting: bool,
    pub recurring_questions: bool,
}

#[must_use]
pub fn parse_freshness(v: &serde_json::Value) -> FreshnessOverview {
    FreshnessOverview {
        stale: v.get("stale").and_then(|x| x.as_i64()).unwrap_or(0),
        fresh: v.get("fresh").and_then(|x| x.as_i64()).unwrap_or(0),
        documents: v
            .get("documents")
            .and_then(|d| d.as_array())
            .map(|rows| {
                rows.iter()
                    .map(|d| FreshnessDocument {
                        id: d.get("id").and_then(|x| x.as_i64()).unwrap_or(0),
                        title: d
                            .get("title")
                            .and_then(|x| x.as_str())
                            .unwrap_or_default()
                            .to_string(),
                        freshness_status: d
                            .get("freshness_status")
                            .and_then(|x| x.as_str())
                            .unwrap_or_default()
                            .to_string(),
                        last_reviewed_at: d
                            .get("last_reviewed_at")
                            .and_then(|x| x.as_str())
                            .map(str::to_string),
                        last_verified_at: d
                            .get("last_verified_at")
                            .and_then(|x| x.as_str())
                            .map(str::to_string),
                        source_name: d
                            .get("source_name")
                            .and_then(|x| x.as_str())
                            .map(str::to_string),
                        visibility: d
                            .get("visibility")
                            .and_then(|x| x.as_str())
                            .unwrap_or_default()
                            .to_string(),
                        version: d.get("version").and_then(|x| x.as_i64()).unwrap_or(1),
                        days_since_update: d.get("days_since_update").and_then(|x| x.as_i64()),
                        days_since_review: d.get("days_since_review").and_then(|x| x.as_i64()),
                        usage_count: d.get("usage_count").and_then(|x| x.as_i64()).unwrap_or(0),
                        stale: d
                            .pointer("/flags/stale")
                            .and_then(|x| x.as_bool())
                            .unwrap_or(false),
                        unreviewed_long: d
                            .pointer("/flags/unreviewed_long")
                            .and_then(|x| x.as_bool())
                            .unwrap_or(false),
                        unverified: d
                            .pointer("/flags/unverified")
                            .and_then(|x| x.as_bool())
                            .unwrap_or(false),
                        low_usage: d
                            .pointer("/flags/low_usage")
                            .and_then(|x| x.as_bool())
                            .unwrap_or(false),
                        conflicting: d
                            .pointer("/flags/conflicting")
                            .and_then(|x| x.as_bool())
                            .unwrap_or(false),
                        recurring_questions: d
                            .pointer("/flags/recurring_questions")
                            .and_then(|x| x.as_bool())
                            .unwrap_or(false),
                    })
                    .collect()
            })
            .unwrap_or_default(),
    }
}

#[component]
fn FreshnessTab() -> impl IntoView {
    let overview = RwSignal::new(None::<FreshnessOverview>);
    let loading = RwSignal::new(true);
    let error_msg = RwSignal::new(None::<String>);
    let reload = RwSignal::new(0u32);

    Effect::new(move |_| {
        let _tick = reload.get();
        let overview = overview;
        let loading = loading;
        let error_msg = error_msg;
        loading.set(true);
        wasm_bindgen_futures::spawn_local(async move {
            match crate::api::get_json::<serde_json::Value>("/api/knowledge/freshness").await {
                Ok(v) => {
                    overview.set(Some(parse_freshness(&v)));
                    error_msg.set(None);
                }
                Err(e) => error_msg.set(Some(e)),
            }
            loading.set(false);
        });
    });

    view! {
        <div class="spp-card">
            <h3 class="spp-card__title">"Knowledge freshness"</h3>
            <Show when=move || loading.get() fallback=|| ()>
                <LoadingState />
            </Show>
            <Show when=move || error_msg.get().is_some() fallback=|| ()>
                <div class="spp-state spp-state--error">
                    <p class="spp-state__body">{move || error_msg.get().unwrap_or_default()}</p>
                </div>
            </Show>
            {move || {
                let o = overview.clone().get();
                if loading.get() {
                    return ().into_any();
                }
                let Some(o) = o else {
                    return ().into_any();
                };
                view! {
                    <div class="spp-flex spp-gap-8 spp-mb-12">
                        <span class="spp-badge spp-badge--warn">{format!("{} stale", o.stale)}</span>
                        <span class="spp-badge spp-badge--ok">{format!("{} fresh", o.fresh)}</span>
                    </div>
                    {if o.documents.is_empty() {
                        view! { <EmptyState message="Nothing flagged in this view. Flags appear as documents age or go unreviewed." /> }.into_any()
                    } else {
                        o.documents
                            .iter()
                            .map(|d| {
                                view! {
                                    <FreshnessRow
                                        row=d.clone()
                                        reload=reload
                                    />
                                }
                            })
                            .collect::<Vec<_>>()
                            .into_any()
                    }}
                    <p class="spp-muted spp-text-xs spp-mt-8">
                        "Review and verify are human-only timestamps — SupportOS never edits or publishes knowledge automatically."
                    </p>
                }.into_any()
            }}
        </div>
    }
}

#[component]
fn FreshnessRow(row: FreshnessDocument, reload: RwSignal<u32>) -> impl IntoView {
    let id = row.id;
    let title = row.title.clone();
    let reviewed = row
        .last_reviewed_at
        .clone()
        .unwrap_or_else(|| "never".to_string());
    let verified = row
        .last_verified_at
        .clone()
        .unwrap_or_else(|| "never".to_string());
    let updated = match row.days_since_update {
        Some(d) => format!("{d}d ago"),
        None => "—".to_string(),
    };
    let badge = if row.stale {
        "spp-badge spp-badge--warn"
    } else {
        "spp-badge spp-badge--ok"
    };

    let mark = move |action: &'static str| {
        move |_| {
            wasm_bindgen_futures::spawn_local(async move {
                let path = format!("/api/knowledge/documents/{id}/{action}");
                match crate::api::post_json::<serde_json::Value>(&path, None).await {
                    Ok(r) if r.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) => {
                        let message = r.get("message").and_then(|v| v.as_str()).unwrap_or("Done.");
                        crate::toasts::success(message);
                        reload.update(|t| *t += 1);
                    }
                    Ok(r) => {
                        crate::toasts::error(
                            r.get("message")
                                .and_then(|v| v.as_str())
                                .unwrap_or("Failed."),
                        );
                    }
                    Err(e) => crate::toasts::error(e),
                }
            });
        }
    };
    let do_review = mark("review");
    let do_verify = mark("verify");

    view! {
        <div class="spp-flex spp-flex--between spp-knowledge__fresh-row">
            <div>
                <span class="spp-text-sm">{title}</span>
                <div class="spp-muted spp-text-xs">
                    {format!(
                        "{} · v{} · updated {} · reviewed {} · verified {}",
                        row.source_name.clone().unwrap_or_else(|| "—".to_string()),
                        row.version,
                        updated,
                        reviewed.get(..10).unwrap_or(&reviewed),
                        verified.get(..10).unwrap_or(&verified),
                    )}
                </div>
            </div>
            <div class="spp-flex">
                {if row.stale {
                    view! { <span class=badge>"stale"</span> }.into_any()
                } else {
                    view! { <span class=badge>"fresh"</span> }.into_any()
                }}
                {if row.unreviewed_long {
                    view! { <span class="spp-badge spp-badge--warn">"needs review"</span> }.into_any()
                } else {
                    ().into_any()
                }}
                {if row.unverified {
                    view! { <span class="spp-badge spp-badge--warn">"unverified"</span> }.into_any()
                } else {
                    ().into_any()
                }}
                {if row.low_usage {
                    view! { <span class="spp-badge spp-badge--info">"low usage"</span> }.into_any()
                } else {
                    ().into_any()
                }}
                {if row.conflicting {
                    view! { <span class="spp-badge spp-badge--danger">"conflicting"</span> }.into_any()
                } else {
                    ().into_any()
                }}
                {if row.recurring_questions {
                    view! { <span class="spp-badge spp-badge--warn">"recurring questions"</span> }.into_any()
                } else {
                    ().into_any()
                }}
                <span class="spp-badge spp-badge--muted">
                    {format!("{} citations", row.usage_count)}
                </span>
                <button
                    class="spp-button spp-button--small"
                    title="Mark reviewed (human action; nothing is published)"
                    on:click=do_review
                >
                    "Review"
                </button>
                <button
                    class="spp-button spp-button--small"
                    title="Mark verified (human action; nothing is published)"
                    on:click=do_verify
                >
                    "Verify"
                </button>
            </div>
        </div>
    }
}

#[component]
fn DocReader(id: i64, on_close: Arc<dyn Fn() + Send + Sync>) -> impl IntoView {
    let document = RwSignal::new(None::<serde_json::Value>);
    let related_ticket_estimate = RwSignal::new(0i64);
    let related_known_issues = RwSignal::new(Vec::<serde_json::Value>::new());
    let loading = RwSignal::new(true);
    let error_msg = RwSignal::new(None::<String>);

    Effect::new(move |_| {
        let document = document;
        let related_ticket_estimate = related_ticket_estimate;
        let related_known_issues = related_known_issues;
        let loading = loading;
        let error_msg = error_msg;
        loading.set(true);
        wasm_bindgen_futures::spawn_local(async move {
            let path = format!("/api/knowledge/documents/{id}");
            match crate::api::get_json::<serde_json::Value>(&path).await {
                Ok(v) => {
                    document.set(v.get("document").cloned());
                    related_ticket_estimate.set(
                        v.get("related_ticket_estimate")
                            .and_then(|x| x.as_i64())
                            .unwrap_or(0),
                    );
                    related_known_issues.set(
                        v.get("related_known_issues")
                            .and_then(|x| x.as_array())
                            .cloned()
                            .unwrap_or_default(),
                    );
                    error_msg.set(None);
                }
                Err(e) => error_msg.set(Some(e)),
            }
            loading.set(false);
        });
    });

    view! {
        <div class="spp-overlay" role="dialog" aria-modal="true" aria-label="Document reader">
            <div class="spp-modal spp-modal--reader">
                <div class="spp-modal__head">
                    <h3 class="spp-modal__title">
                        {move || {
                            document
                                .get()
                                .and_then(|d| d.get("title").and_then(|t| t.as_str()).map(str::to_string))
                                .unwrap_or_else(|| "Document".to_string())
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
                        <p class="spp-state__body">"Could not open this document."</p>
                        <p class="spp-muted spp-text-xs">{move || error_msg.get().unwrap_or_default()}</p>
                    </div>
                </Show>
                <Show
                    when=move || !loading.get() && error_msg.get().is_none()
                    fallback=|| ()
                >
                    {move || {
                        let d = match document.get() {
                            Some(d) => d,
                            None => return view! { <div></div> }.into_any(),
                        };
                        let visibility = d
                            .get("visibility")
                            .and_then(|v| v.as_str())
                            .unwrap_or("internal_only")
                            .to_string();
                        let version = d.get("version").and_then(|v| v.as_i64()).unwrap_or(1);
                        let source_name = d
                            .get("source_name")
                            .and_then(|v| v.as_str())
                            .unwrap_or("—")
                            .to_string();
                        let chunk_count = d.get("chunk_count").and_then(|v| v.as_i64()).unwrap_or(0);
                        let content = d
                            .get("content")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let estimate = related_ticket_estimate.get();
                        let issues = related_known_issues.get();
                        view! {
                            <div class="spp-doc-reader__badges">
                                <span class=if visibility == "customer_safe" {
                                    "spp-badge spp-badge--ok"
                                } else {
                                    "spp-badge"
                                }>
                                    {if visibility == "customer_safe" { "customer-safe" } else { "internal-only" }}
                                </span>
                                <span class="spp-badge">{format!("v{version}")}</span>
                                <span class="spp-badge">{format!("source: {source_name}")}</span>
                                <span class="spp-badge">{format!("{chunk_count} chunks")}</span>
                            </div>
                            <div class="spp-doc-reader__content">{content}</div>
                            <div class="spp-doc-reader__related">
                                <strong>"Related:"</strong>
                                " cited by AI analysis in "
                                {format!("{estimate} conversation{}", if estimate == 1 { "" } else { "s" })}
                                {if issues.is_empty() {
                                    ().into_any()
                                } else {
                                    view! {
                                        " · "
                                        {issues.iter().map(|ki| {
                                            let title = ki
                                                .get("title")
                                                .and_then(|t| t.as_str())
                                                .unwrap_or("(untitled)")
                                                .to_string();
                                            view! { <span class="spp-chip">{title}</span> }
                                        }).collect::<Vec<_>>()}
                                    }.into_any()
                                }}
                            </div>
                        }.into_any()
                    }}
                </Show>
            </div>
        </div>
    }
}
