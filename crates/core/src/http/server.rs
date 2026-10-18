use std::net::SocketAddr;
use std::sync::Arc;

use axum::{
    Router,
    extract::State,
    response::{IntoResponse, Response},
    routing::{any, get, post},
};
use tokio::net::TcpListener;
use tower_http::catch_panic::CatchPanicLayer;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

use crate::error::Result;

use super::EventBus;
use super::rate_limit::RateLimiter;
use super::routes;
use std::sync::{Mutex, MutexGuard};

pub fn lock_or_recover<T: ?Sized>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|poisoned| {
        tracing::warn!("AppState mutex was poisoned — recovering");
        poisoned.into_inner()
    })
}

#[derive(Clone)]
pub struct AppState {
    pub conn: Arc<std::sync::Mutex<rusqlite::Connection>>,
    pub data_dir: std::path::PathBuf,
    pub port: u16,
    pub host: String,
    pub demo_mode: bool,
    pub bus: EventBus,
    pub limiter: RateLimiter,
    pub sync: Option<Arc<crate::sync_engine::SyncEngine>>,
    pub real: Option<Arc<crate::helpscout_real::RealHelpScoutProvider>>,
    pub provider_kind: String,
    pub workers: Option<Arc<crate::workers::WorkerManager>>,
    pub qdrant: Arc<crate::vectorstore_qdrant::EmbeddedQdrant>,
}

impl AppState {
    pub fn qdrant_from_settings(
        conn: &rusqlite::Connection,
        data_dir: &std::path::Path,
    ) -> Arc<crate::vectorstore_qdrant::EmbeddedQdrant> {
        let url = crate::settings::get_string(conn, "qdrant_url")
            .ok()
            .flatten()
            .unwrap_or_else(|| crate::vectorstore_qdrant::DEFAULT_QDRANT_URL.to_string());
        let enabled = crate::settings::get_bool(conn, "qdrant_enabled", true).unwrap_or(true);
        Arc::new(crate::vectorstore_qdrant::EmbeddedQdrant::new(
            data_dir, &url, enabled,
        ))
    }

    pub fn conn_lock(&self) -> MutexGuard<'_, rusqlite::Connection> {
        lock_or_recover(&self.conn)
    }

    pub async fn run_ai<T>(
        &self,
        f: impl for<'a> FnOnce(
            &'a rusqlite::Connection,
        ) -> std::pin::Pin<Box<dyn std::future::Future<Output = T> + 'a>>
        + Send
        + 'static,
    ) -> std::result::Result<T, String>
    where
        T: Send + 'static,
    {
        let db_path: Option<std::path::PathBuf> = {
            let conn = lock_or_recover(&self.conn);
            crate::db::db_file_path(&conn)
        };
        let conn = self.conn.clone();
        tokio::task::spawn_blocking(move || -> std::result::Result<T, String> {
            let private;
            let shared_guard;
            let conn_ref: &rusqlite::Connection = match &db_path {
                Some(path) => {
                    private = crate::db::open(path)
                        .map_err(|e| format!("failed to open the AI pipeline connection: {e}"))?;
                    &private
                }
                None => {
                    shared_guard = lock_or_recover(&conn);
                    &shared_guard
                }
            };
            Ok(tokio::runtime::Handle::current().block_on(f(conn_ref)))
        })
        .await
        .map_err(|e| e.to_string())
        .and_then(|r| r)
    }
}

pub struct HttpServer {
    addr: SocketAddr,
    state: AppState,
}

impl HttpServer {
    pub fn new(state: AppState) -> Self {
        Self {
            addr: SocketAddr::from(([127, 0, 0, 1], state.port)),
            state,
        }
    }

    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    pub fn build_router(&self) -> Router {
        let state = self.state.clone();
        let limiter = self.state.limiter.clone();

        let dns_guard = axum::middleware::from_fn(
            |req: axum::extract::Request, next: axum::middleware::Next| async move {
                let host = req
                    .headers()
                    .get(axum::http::header::HOST)
                    .and_then(|h| h.to_str().ok())
                    .unwrap_or("");
                if !is_loopback_host(host) {
                    return axum::response::Response::builder()
                    .status(axum::http::StatusCode::FORBIDDEN)
                    .header("Content-Type", "application/json")
                    .body(axum::body::Body::from(
                        r#"{"statusCode":403,"error":"Forbidden","message":"SupportOS is a local application: requests with non-loopback Host headers are refused (DNS-rebinding guard)."}"#,
                    ))
                    .unwrap();
                }
                next.run(req).await
            },
        );

        let rate_limit = axum::middleware::from_fn(
            move |req: axum::extract::Request, next: axum::middleware::Next| {
                let limiter = limiter.clone();
                async move {
                    let method = req.method().clone();
                    let path = req.uri().path().to_string();
                    let key = req
                        .extensions()
                        .get::<axum::extract::ConnectInfo<std::net::SocketAddr>>()
                        .map(|ci| ci.ip().to_string())
                        .unwrap_or_else(|| "local".to_string());
                    if let Err(retry_after) = limiter.check(&method, &path, &key) {
                        return RateLimiter::too_many_requests_response(retry_after);
                    }
                    next.run(req).await
                }
            },
        );

        let cors = CorsLayer::new()
            .allow_origin([
                format!("http://localhost:{}", self.state.port)
                    .parse()
                    .unwrap(),
                format!("http://127.0.0.1:{}", self.state.port)
                    .parse()
                    .unwrap(),
                format!("http://[::1]:{}", self.state.port).parse().unwrap(),
                "http://localhost:5173".parse().unwrap(),
                "http://localhost:5174".parse().unwrap(),
                "http://localhost:5175".parse().unwrap(),
                "http://127.0.0.1:5173".parse().unwrap(),
                "http://127.0.0.1:5174".parse().unwrap(),
                "http://127.0.0.1:5175".parse().unwrap(),
                "http://tauri.localhost".parse().unwrap(),
                "https://tauri.localhost".parse().unwrap(),
                "tauri://localhost".parse().unwrap(),
                "http://127.0.0.1:1420".parse().unwrap(),
                "http://localhost:1420".parse().unwrap(),
            ])
            .allow_methods([
                axum::http::Method::GET,
                axum::http::Method::POST,
                axum::http::Method::PUT,
                axum::http::Method::PATCH,
                axum::http::Method::DELETE,
                axum::http::Method::OPTIONS,
            ])
            .allow_headers([
                axum::http::header::CONTENT_TYPE,
                axum::http::header::AUTHORIZATION,
            ]);

        Router::new()
            .route("/health", get(routes::system::health))
            .route("/health/detailed", get(routes::system::health_detailed))
            .route("/api/system/db", get(routes::system::db_stats))
            .route(
                "/api/system/capabilities",
                get(routes::system::capabilities),
            )
            .route("/api/system/tables", get(routes::system::table_stats))
            .route("/api/onboarding", get(routes::system::onboarding))
            .route(
                "/api/onboarding/step",
                post(routes::system::onboarding_step),
            )
            .route(
                "/api/onboarding/complete",
                post(routes::system::onboarding_complete),
            )
            .route("/api/demo/enable", post(routes::system::demo_enable))
            .route(
                "/api/demo/simulate-incoming",
                post(routes::system::demo_simulate_incoming),
            )
            .route(
                "/api/demo/simulate-rating",
                post(routes::system::demo_simulate_rating),
            )
            .route(
                "/api/demo/simulate-webhook",
                post(routes::system::demo_simulate_webhook),
            )
            .route("/api/events", get(routes::events::sse_handler))
            .route("/api/conversations", get(routes::conversations::list))
            .route("/api/conversations/{id}", get(routes::conversations::get))
            .route(
                "/api/conversations/{id}/reply",
                post(routes::conversations::reply),
            )
            .route(
                "/api/conversations/{id}/note",
                post(routes::conversations::note),
            )
            .route(
                "/api/conversations/{id}/status",
                post(routes::conversations::status),
            )
            .route(
                "/api/conversations/{id}/assign",
                post(routes::conversations::assign),
            )
            .route(
                "/api/conversations/{id}/priority",
                post(routes::conversations::priority),
            )
            .route(
                "/api/conversations/{id}/subject",
                post(routes::conversations::subject),
            )
            .route(
                "/api/conversations/{id}/state",
                post(routes::conversations::set_state),
            )
            .route(
                "/api/conversations/{id}/move",
                post(routes::conversations::move_to_inbox),
            )
            .route(
                "/api/conversations/{id}/tags",
                post(routes::conversations::update_tags_route),
            )
            .route(
                "/api/conversations/{id}/fields",
                post(routes::conversations::update_fields_route),
            )
            .route(
                "/api/conversations/{id}/snooze",
                post(routes::conversations::snooze_route)
                    .delete(routes::conversations::unsnooze_route),
            )
            .route(
                "/api/conversations/{id}/schedule",
                post(routes::conversations::schedule_route)
                    .delete(routes::conversations::schedule_delete_route),
            )
            .route(
                "/api/conversations/{id}/schedule/publish",
                post(routes::conversations::schedule_publish_route),
            )
            .route(
                "/api/conversations/bulk",
                post(routes::conversations::bulk_route),
            )
            .route(
                "/api/conversations/{id}/refresh",
                post(routes::conversations::refresh_route),
            )
            .route(
                "/api/conversations/{id}/workflow/{workflowId}",
                post(routes::conversations::workflow_route),
            )
            .route(
                "/api/attachments/{id}/download",
                post(routes::conversations::attachment_download_route),
            )
            .route(
                "/api/attachments/{id}/file",
                get(routes::system::attachment_file),
            )
            .route(
                "/api/conversations/{id}/events",
                get(routes::conversations::events),
            )
            .route(
                "/api/conversations/activity/rebuild",
                post(routes::conversations::activity_rebuild),
            )
            .route(
                "/api/ticket-states",
                get(routes::conversations::list_ticket_states)
                    .post(routes::conversations::create_ticket_state),
            )
            .route(
                "/api/ticket-states/{id}",
                axum::routing::patch(routes::conversations::update_ticket_state)
                    .delete(routes::conversations::delete_ticket_state),
            )
            .route("/api/mailboxes", get(routes::conversations::list_mailboxes))
            .route("/api/tags", get(routes::conversations::list_tags))
            .route("/api/users", get(routes::conversations::list_users))
            .route("/api/teams", get(routes::conversations::list_teams))
            .route(
                "/api/saved-replies",
                get(routes::conversations::list_saved_replies),
            )
            .route(
                "/api/inbox-fields",
                get(routes::conversations::inbox_fields),
            )
            .route("/api/workflows", get(routes::conversations::workflows))
            .route(
                "/api/users/statuses",
                get(routes::conversations::user_statuses),
            )
            .route(
                "/api/webhook-configs",
                get(routes::conversations::webhook_configs),
            )
            .route(
                "/api/customers",
                get(routes::people::list_customers).post(routes::people::create_customer),
            )
            .route(
                "/api/customers/{id}",
                get(routes::people::get_customer).patch(routes::people::update_customer),
            )
            .route(
                "/api/customers/{id}/timeline",
                get(routes::people::customer_timeline),
            )
            .route(
                "/api/customers/{id}/support-health",
                get(routes::people::customer_support_health),
            )
            .route(
                "/api/organizations",
                get(routes::people::list_organizations).post(routes::people::create_organization),
            )
            .route(
                "/api/organizations/{id}",
                get(routes::people::get_organization).patch(routes::people::update_organization),
            )
            .route(
                "/api/organizations/{id}/timeline",
                get(routes::people::organization_timeline),
            )
            .route(
                "/api/organizations/{id}/support-health",
                get(routes::people::organization_support_health),
            )
            .route(
                "/api/timeline/rebuild",
                post(routes::people::timeline_rebuild),
            )
            .route("/api/search", post(routes::search::search))
            .route("/api/inbox-views", get(routes::views::list_views))
            .route("/api/inbox-views/{id}", get(routes::views::get_view))
            .route("/api/inbox-views", post(routes::views::create_view))
            .route(
                "/api/inbox-views/{id}",
                axum::routing::patch(routes::views::update_view),
            )
            .route(
                "/api/inbox-views/{id}",
                axum::routing::delete(routes::views::delete_view),
            )
            .route(
                "/api/inbox-views/preview",
                post(routes::views::preview_view),
            )
            .route("/api/operations/center", get(routes::operations::center))
            .route(
                "/api/operations/workload",
                get(routes::operations::workload),
            )
            .route(
                "/api/operations/capacity",
                axum::routing::put(routes::operations::set_capacity),
            )
            .route(
                "/api/operations/waiting-threshold",
                axum::routing::put(routes::operations::set_waiting_threshold),
            )
            .route(
                "/api/operations/suggested-assignees",
                get(routes::operations::suggested_assignees),
            )
            .route("/api/notifications", get(routes::notifications::list))
            .route(
                "/api/notifications/unread-count",
                get(routes::notifications::unread_count),
            )
            .route(
                "/api/notifications/{id}/read",
                post(routes::notifications::mark_read),
            )
            .route(
                "/api/notifications/read-all",
                post(routes::notifications::mark_all_read),
            )
            .route(
                "/api/notifications/prefs",
                get(routes::notifications::list_prefs),
            )
            .route(
                "/api/notifications/prefs/{type}",
                axum::routing::put(routes::notifications::set_pref),
            )
            .route(
                "/api/notifications/mentions",
                get(routes::notifications::mentions),
            )
            .route(
                "/api/notifications/sweep",
                post(routes::notifications::sweep),
            )
            .route("/api/settings", get(routes::settings::get_settings))
            .route(
                "/api/settings",
                axum::routing::patch(routes::settings::update_settings),
            )
            .route(
                "/api/settings/lmstudio",
                get(routes::settings::get_lmstudio),
            )
            .route(
                "/api/settings/lmstudio",
                axum::routing::patch(routes::settings::update_lmstudio),
            )
            .route(
                "/api/settings/lmstudio/test",
                post(routes::settings::test_lmstudio),
            )
            .route("/api/settings/qdrant", get(routes::settings::get_qdrant))
            .route(
                "/api/settings/qdrant",
                axum::routing::patch(routes::settings::update_qdrant),
            )
            .route(
                "/api/settings/qdrant/test",
                post(routes::settings::qdrant_test),
            )
            .route(
                "/api/settings/appearance",
                get(routes::settings::appearance),
            )
            .route(
                "/api/settings/business-hours",
                get(routes::settings::get_business_hours),
            )
            .route(
                "/api/settings/business-hours/{mailboxId}",
                axum::routing::put(routes::settings::set_business_hours),
            )
            .route(
                "/api/settings/business-hours/{mailboxId}",
                axum::routing::delete(routes::settings::delete_business_hours),
            )
            .route("/api/sync/status", get(routes::sync::status))
            .route("/api/sync/initial", post(routes::sync::initial))
            .route("/api/sync/incremental", post(routes::sync::incremental))
            .route("/api/sync/reconcile", post(routes::sync::reconcile))
            .route("/api/sync/cancel", post(routes::sync::cancel))
            .route("/api/sync/encrypted", get(routes::sync::encrypted_list))
            .route(
                "/api/sync/encrypted/export",
                post(routes::sync::encrypted_export),
            )
            .route(
                "/api/sync/encrypted/verify",
                post(routes::sync::encrypted_verify),
            )
            .route(
                "/api/sync/encrypted/import",
                post(routes::sync::encrypted_import),
            )
            .route(
                "/api/sync/encrypted/upload",
                post(routes::sync::encrypted_upload)
                    .layer(axum::extract::DefaultBodyLimit::max(512 * 1024 * 1024)),
            )
            .route("/api/audit", get(routes::system::audit_log))
            .route("/api/errors", get(routes::system::errors))
            .route("/api/backups", get(routes::system::backups_list))
            .route("/api/backups/create", post(routes::system::backups_create))
            .route(
                "/api/backups/export-json",
                post(routes::system::backups_export_json),
            )
            .route(
                "/api/backups/export-csv",
                post(routes::system::backups_export_csv),
            )
            .route(
                "/api/sync/rebuild-search-index",
                post(routes::sync::rebuild_search_index),
            )
            .route(
                "/api/sync/rebuild-embeddings",
                post(routes::sync::rebuild_embeddings),
            )
            .route(
                "/api/webhooks/register",
                post(routes::sync::register_webhook),
            )
            .route(
                "/api/webhooks/{remoteId}",
                axum::routing::delete(routes::sync::unregister_webhook),
            )
            .route("/api/queue", get(routes::sync::queue))
            .route("/api/queue/{id}/retry", post(routes::sync::retry_job))
            .route("/api/queue/{id}/cancel", post(routes::sync::cancel_job))
            .route(
                "/api/queue/clear-completed",
                post(routes::sync::clear_completed),
            )
            .route(
                "/api/oauth/authorize-url",
                get(routes::oauth::authorize_url),
            )
            .route(
                "/api/oauth/client-credentials",
                post(routes::oauth::client_credentials),
            )
            .route("/api/oauth/status", get(routes::oauth::status))
            .route("/api/oauth/disconnect", post(routes::oauth::disconnect))
            .route("/oauth/callback", get(routes::oauth::callback))
            .route("/api/webhooks/helpscout", post(routes::webhook::handle))
            .route(
                "/api/analytics/dashboard",
                get(routes::analytics::dashboard),
            )
            .route("/api/analytics/ai", get(routes::analytics::ai_analytics))
            .route("/api/reports/sla", get(routes::analytics::sla_report))
            .route(
                "/api/reports/why-contacting",
                get(routes::analytics::why_contacting),
            )
            .route(
                "/api/reports/top-questions",
                get(routes::analytics::top_questions),
            )
            .route("/api/reports/doc-gaps", get(routes::analytics::doc_gaps))
            .route(
                "/api/reports/answer-reuse",
                get(routes::analytics::answer_reuse),
            )
            .route(
                "/api/reports/issue-radar",
                get(routes::analytics::issue_radar),
            )
            .route(
                "/api/reports/metric-definitions",
                get(routes::analytics::metric_definitions),
            )
            .route(
                "/api/reports/builder/run",
                post(routes::analytics::builder_run),
            )
            .route(
                "/api/reports/builder/saved",
                get(routes::analytics::builder_saved_list)
                    .post(routes::analytics::builder_saved_create),
            )
            .route(
                "/api/reports/builder/saved/{id}",
                axum::routing::delete(routes::analytics::builder_saved_delete),
            )
            .route(
                "/api/reports/effectiveness",
                get(routes::analytics::effectiveness),
            )
            .route(
                "/api/reports/release-correlation",
                get(routes::analytics::release_correlation),
            )
            .route(
                "/api/reports/release-events",
                post(routes::analytics::release_events),
            )
            .route(
                "/api/reports/helpscout/{reportKey}",
                get(routes::analytics::helpscout_report),
            )
            .route("/api/reports/narrative", post(routes::analytics::narrative))
            .route(
                "/api/reports/builder/catalog",
                get(routes::analytics::report_catalog),
            )
            .route("/api/ai/status", get(routes::ai::status))
            .route(
                "/api/ai/analyze/{conversationId}",
                post(routes::ai::analyze),
            )
            .route("/api/ai/draft/{conversationId}", post(routes::ai::draft))
            .route("/api/ai/similar/{conversationId}", get(routes::ai::similar))
            .route("/api/ai/memory/{customerId}", get(routes::ai::get_memory))
            .route("/api/ai/memory/{customerId}", post(routes::ai::set_memory))
            .route("/api/ai/jobs", get(routes::ai::jobs))
            .route("/api/ai/analytics", get(routes::ai::ai_analytics))
            .route("/api/ai/evaluation", get(routes::ai::evaluation))
            .route("/api/ai/evaluation/run", post(routes::ai::evaluation_run))
            .route("/api/ai/cluster-issues", post(routes::ai::cluster_issues))
            .route("/api/ai/draft/{draftId}/rewrite", post(routes::ai::rewrite))
            .route("/api/ai/draft/{draftId}/verify", post(routes::ai::verify))
            .route(
                "/api/ai/draft/{draftId}/feedback",
                post(routes::ai::feedback),
            )
            .route("/api/issues/clusters", get(routes::issues::list_clusters))
            .route("/api/issues/sla-alerts", get(routes::issues::sla_alerts))
            .route("/api/issues/known", get(routes::issues::list_known))
            .route("/api/issues/known", post(routes::issues::create_known))
            .route("/api/issues/known/{id}", get(routes::issues::get_known))
            .route(
                "/api/issues/known/{id}",
                axum::routing::patch(routes::issues::update_known),
            )
            .route(
                "/api/issues/known/{id}",
                axum::routing::delete(routes::issues::delete_known),
            )
            .route(
                "/api/issues/known/{id}/impact",
                get(routes::issues::known_impact),
            )
            .route("/api/issues/cases", get(routes::issues::list_cases))
            .route(
                "/api/issues/cases/from-conversation/{conversationId}",
                post(routes::issues::case_from_conversation),
            )
            .route(
                "/api/issues/clusters/{id}",
                get(routes::issues::get_cluster),
            )
            .route(
                "/api/issues/clusters/{id}",
                axum::routing::delete(routes::issues::delete_cluster),
            )
            .route("/api/issues/known/{id}/refs", post(routes::issues::add_ref))
            .route(
                "/api/issues/known/{id}/link/{conversationId}",
                post(routes::issues::link_known),
            )
            .route(
                "/api/issues/known/{id}/link/{conversationId}",
                axum::routing::delete(routes::issues::unlink_known),
            )
            .route("/api/automation/rules", get(routes::automation::list_rules))
            .route(
                "/api/automation/rules",
                post(routes::automation::create_rule),
            )
            .route(
                "/api/automation/rules/{id}",
                axum::routing::patch(routes::automation::update_rule),
            )
            .route(
                "/api/automation/rules/{id}",
                axum::routing::delete(routes::automation::delete_rule),
            )
            .route(
                "/api/automation/rules/{id}/trigger/{conversationId}",
                post(routes::automation::trigger),
            )
            .route(
                "/api/conversations/{id}/side-threads",
                get(routes::collaboration::list_side_threads),
            )
            .route(
                "/api/conversations/{id}/side-threads",
                post(routes::collaboration::create_side_thread),
            )
            .route(
                "/api/side-threads/{id}",
                get(routes::collaboration::get_side_thread),
            )
            .route(
                "/api/side-threads/{id}/messages",
                post(routes::collaboration::add_message),
            )
            .route(
                "/api/side-threads/{id}/resolve",
                post(routes::collaboration::resolve),
            )
            .route(
                "/api/side-threads/{id}/reopen",
                post(routes::collaboration::reopen),
            )
            .route(
                "/api/side-threads/{id}/participants",
                post(routes::collaboration::add_participants),
            )
            .route(
                "/api/mention-directory",
                get(routes::collaboration::mention_directory),
            )
            .route("/api/copilot/sessions", get(routes::copilot::list_sessions))
            .route("/api/copilot/chat", post(routes::copilot::chat))
            .route(
                "/api/copilot/sessions/{id}",
                get(routes::copilot::get_session),
            )
            .route(
                "/api/copilot/sessions/{id}",
                axum::routing::delete(routes::copilot::delete_session),
            )
            .route("/api/copilot/tools", get(routes::copilot::tools))
            .route(
                "/api/copilot/starter-questions/{conversationId}",
                get(routes::copilot::starter_questions),
            )
            .route(
                "/api/knowledge/sources",
                get(routes::knowledge::list_sources),
            )
            .route(
                "/api/knowledge/documents",
                get(routes::knowledge::list_documents),
            )
            .route(
                "/api/knowledge/documents/{id}",
                get(routes::knowledge::get_document),
            )
            .route("/api/knowledge/search", get(routes::knowledge::search))
            .route(
                "/api/knowledge/freshness",
                get(routes::knowledge::freshness),
            )
            .route("/api/knowledge/reindex", post(routes::knowledge::reindex))
            .route("/api/knowledge/import", post(routes::knowledge::import))
            .route(
                "/api/knowledge/importable",
                get(routes::knowledge::importable),
            )
            .route(
                "/api/knowledge/import-file",
                post(routes::knowledge::import_file),
            )
            .route(
                "/api/knowledge/documents/{id}",
                axum::routing::delete(routes::knowledge::delete_document),
            )
            .route(
                "/api/knowledge/documents/{id}/review",
                post(routes::knowledge::review_document),
            )
            .route(
                "/api/knowledge/documents/{id}/verify",
                post(routes::knowledge::verify_document),
            )
            .route("/api/outreach/meta", get(routes::outreach::meta))
            .route(
                "/api/outreach/segments",
                get(routes::outreach::list_segments),
            )
            .route(
                "/api/outreach/segments",
                post(routes::outreach::create_segment),
            )
            .route(
                "/api/outreach/segments/{id}",
                axum::routing::delete(routes::outreach::delete_segment),
            )
            .route(
                "/api/outreach/segments/preview",
                post(routes::outreach::preview_segment),
            )
            .route(
                "/api/outreach/segments/estimate",
                post(routes::outreach::estimate_segment),
            )
            .route(
                "/api/outreach/segments/suggest",
                post(routes::outreach::suggest_segment),
            )
            .route(
                "/api/outreach/campaigns",
                get(routes::outreach::list_campaigns),
            )
            .route(
                "/api/outreach/campaigns",
                post(routes::outreach::create_campaign),
            )
            .route(
                "/api/outreach/campaigns/{id}",
                get(routes::outreach::get_campaign),
            )
            .route(
                "/api/outreach/campaigns/{id}/validate",
                get(routes::outreach::campaign_validate_route),
            )
            .route("/api/outreach/render", post(routes::outreach::render_route))
            .route(
                "/api/outreach/campaigns/{id}/preview",
                post(routes::outreach::campaign_preview_route),
            )
            .route(
                "/api/outreach/campaigns/{id}/queue",
                post(routes::outreach::campaign_queue_route),
            )
            .route(
                "/api/outreach/campaigns/{id}/pause",
                post(routes::outreach::campaign_pause_route),
            )
            .route(
                "/api/outreach/campaigns/{id}/resume",
                post(routes::outreach::campaign_resume_route),
            )
            .route(
                "/api/outreach/campaigns/{id}/cancel",
                post(routes::outreach::campaign_cancel_route),
            )
            .route(
                "/api/outreach/campaigns/{id}/retry",
                post(routes::outreach::campaign_retry_route),
            )
            .route(
                "/api/outreach/campaigns/{id}/reconcile",
                post(routes::outreach::campaign_reconcile_route),
            )
            .route(
                "/api/outreach/campaigns/{id}/report",
                get(routes::outreach::campaign_report_route),
            )
            .route(
                "/api/outreach/campaigns/{id}",
                axum::routing::delete(routes::outreach::campaign_delete_route),
            )
            .route("/api/outreach/dnc", get(routes::outreach::list_dnc_route))
            .route("/api/outreach/dnc", post(routes::outreach::add_dnc_route))
            .route(
                "/api/outreach/dnc/{customerLocalId}",
                axum::routing::delete(routes::outreach::remove_dnc_route),
            )
            .route(
                "/api/custom-objects/types",
                get(routes::custom_objects::list_types),
            )
            .route(
                "/api/custom-objects/types",
                post(routes::custom_objects::create_type),
            )
            .route(
                "/api/custom-objects/types/{id}",
                get(routes::custom_objects::get_type),
            )
            .route(
                "/api/custom-objects/types/{id}",
                axum::routing::patch(routes::custom_objects::update_type),
            )
            .route(
                "/api/custom-objects/types/{id}",
                axum::routing::delete(routes::custom_objects::delete_type),
            )
            .route(
                "/api/custom-objects",
                get(routes::custom_objects::list_objects),
            )
            .route(
                "/api/custom-objects",
                post(routes::custom_objects::create_object),
            )
            .route(
                "/api/custom-objects/{id}",
                get(routes::custom_objects::get_object),
            )
            .route(
                "/api/custom-objects/{id}",
                axum::routing::patch(routes::custom_objects::update_object),
            )
            .route(
                "/api/custom-objects/{id}",
                axum::routing::delete(routes::custom_objects::delete_object),
            )
            .route(
                "/api/custom-objects/report",
                get(routes::custom_objects::report),
            )
            .route(
                "/api/custom-objects/for/{targetKind}/{targetId}",
                get(routes::custom_objects::list_for_target),
            )
            .route(
                "/api/custom-objects/{id}/links",
                post(routes::custom_objects::create_link),
            )
            .route(
                "/api/custom-objects/{id}/links/{targetKind}/{targetLocalId}",
                axum::routing::delete(routes::custom_objects::delete_link),
            )
            .route("/api/connectors", get(routes::connectors::list))
            .route("/api/connectors", post(routes::connectors::create))
            .route("/api/connectors/{id}", get(routes::connectors::get))
            .route(
                "/api/connectors/{id}",
                axum::routing::patch(routes::connectors::update),
            )
            .route(
                "/api/connectors/{id}",
                axum::routing::delete(routes::connectors::delete),
            )
            .route(
                "/api/connectors/{id}/refresh",
                post(routes::connectors::refresh),
            )
            .route("/api/connectors/{id}/rows", get(routes::connectors::rows))
            .route("/api/connectors/{id}/test", post(routes::connectors::test))
            .route("/api/incidents", get(routes::incidents::list))
            .route("/api/incidents", post(routes::incidents::create))
            .route("/api/incidents/{id}", get(routes::incidents::get))
            .route(
                "/api/incidents/{id}",
                axum::routing::patch(routes::incidents::update),
            )
            .route(
                "/api/incidents/{id}",
                axum::routing::delete(routes::incidents::delete),
            )
            .route("/api/incidents/{id}/impact", get(routes::incidents::impact))
            .route(
                "/api/incidents/{id}/notes",
                post(routes::incidents::add_note),
            )
            .route("/api/incidents/{id}/refs", post(routes::incidents::add_ref))
            .route(
                "/api/incidents/{id}/related",
                post(routes::incidents::add_related),
            )
            .route(
                "/api/incidents/{id}/related/{targetKind}/{targetLocalId}",
                axum::routing::delete(routes::incidents::delete_related),
            )
            .route(
                "/api/incidents/{id}/releases",
                post(routes::incidents::add_release),
            )
            .route(
                "/api/incidents/{id}/releases/{releaseId}",
                axum::routing::delete(routes::incidents::delete_release),
            )
            .route(
                "/api/incidents/{id}/conversations/{conversationId}",
                post(routes::incidents::link_conversation),
            )
            .route(
                "/api/incidents/{id}/conversations/{conversationId}",
                axum::routing::delete(routes::incidents::unlink_conversation),
            )
            .route(
                "/api/incidents/{id}/refs/{refId}",
                axum::routing::delete(routes::incidents::delete_ref),
            )
            .route(
                "/api/incidents/from-cluster/{clusterId}",
                post(routes::incidents::from_cluster),
            )
            .route(
                "/api/incidents/from-known-issue/{knownIssueId}",
                post(routes::incidents::from_known_issue),
            )
            .route("/api/graph/stats", get(routes::graph::stats))
            .route("/api/graph/meta", get(routes::graph::meta))
            .route("/api/graph/search", get(routes::graph::search))
            .route("/api/graph/node/{kind}/{id}", get(routes::graph::node))
            .route(
                "/api/graph/neighbors/{kind}/{id}",
                get(routes::graph::neighbors),
            )
            .route(
                "/api/graph/subgraph/{kind}/{id}",
                get(routes::graph::subgraph),
            )
            .route("/api/graph/edges", get(routes::graph::list_edges))
            .route("/api/graph/edges", post(routes::graph::create_edge))
            .route(
                "/api/graph/edges/{id}",
                axum::routing::delete(routes::graph::delete_edge),
            )
            .route("/api/attributes/catalog", get(routes::attributes::catalog))
            .route(
                "/api/attributes/conversation/{id}",
                get(routes::attributes::conversation_attributes),
            )
            .route(
                "/api/attributes/conversation/{id}/history/{attribute}",
                get(routes::attributes::conversation_attribute_history),
            )
            .route(
                "/api/attributes/conversation/{id}/recompute",
                post(routes::attributes::recompute),
            )
            .route(
                "/api/attributes/conversations",
                get(routes::attributes::conversations),
            )
            .route("/api/attributes/report", get(routes::attributes::report))
            .route(
                "/api/attributes/values/{attribute}",
                get(routes::attributes::values),
            )
            .route("/api/coaching/meta", get(routes::coaching::meta))
            .route("/api/coaching/{conversationId}", get(routes::coaching::get))
            .route(
                "/api/coaching/{conversationId}/review",
                post(routes::coaching::review),
            )
            .route("/api/translation/meta", get(routes::translation::meta))
            .route("/api/translation/detect", post(routes::translation::detect))
            .route(
                "/api/translation/conversation/{id}",
                get(routes::translation::conversation),
            )
            .route(
                "/api/translation/translate",
                post(routes::translation::translate),
            )
            .route("/api/memory/meta", get(routes::memory::meta))
            .route("/api/memory/{customerId}", get(routes::memory::get))
            .route(
                "/api/memory/{customerId}/entries",
                post(routes::memory::add_entry),
            )
            .route(
                "/api/memory/{customerId}/entries/{entryId}",
                axum::routing::delete(routes::memory::delete_entry),
            )
            .route(
                "/api/interaction/{conversationId}",
                get(routes::interactions::get),
            )
            .route(
                "/api/interaction/{conversationId}/refresh",
                post(routes::interactions::refresh),
            )
            .route(
                "/api/interaction/{conversationId}/evidence",
                get(routes::interactions::evidence),
            )
            .route(
                "/api/interaction/profile/{customerId}",
                get(routes::interactions::profile),
            )
            .route(
                "/api/interaction/profile/{customerId}/override",
                post(routes::interactions::set_override),
            )
            .route(
                "/api/interaction/profile/{customerId}/override/{field}",
                axum::routing::delete(routes::interactions::clear_override),
            )
            .route("/api/knowledge/gaps", get(routes::quality::list_gaps))
            .route(
                "/api/knowledge/gaps/rebuild",
                post(routes::quality::rebuild_gaps),
            )
            .route(
                "/api/knowledge/gaps/candidates/{id}/decide",
                post(routes::quality::decide_candidate),
            )
            .route(
                "/api/knowledge/gaps/candidates/{id}/draft",
                get(routes::quality::draft_candidate),
            )
            .route("/api/qa/overview", get(routes::quality::qa_overview))
            .route("/api/qa/rebuild", post(routes::quality::qa_rebuild))
            .route(
                "/api/qa/{conversationId}",
                get(routes::quality::qa_conversation),
            )
            .route(
                "/api/qa/{conversationId}/analyze",
                post(routes::quality::qa_analyze),
            )
            .route(
                "/api/friction/overview",
                get(routes::quality::friction_overview),
            )
            .route(
                "/api/friction/rebuild",
                post(routes::quality::friction_rebuild),
            )
            .route(
                "/api/friction/{conversationId}",
                get(routes::quality::friction_conversation),
            )
            .route("/api/docs/collections", get(routes::docs::collections))
            .route("/api/docs/stats", get(routes::docs::stats))
            .route("/api/docs/search", get(routes::docs::search))
            .route("/api/docs/articles", get(routes::docs::articles))
            .route("/api/docs/articles/{id}", get(routes::docs::article))
            .fallback(any(routes::not_found))
            .layer(axum::middleware::from_fn_with_state(
                state.clone(),
                log_500_to_application_errors,
            ))
            .with_state(state)
            .layer(CatchPanicLayer::custom(catch_panic_response))
            .layer(axum::middleware::from_fn(rewrite_405_to_404))
            .layer(axum::extract::DefaultBodyLimit::max(20 * 1024 * 1024))
            .layer(cors)
            .layer(TraceLayer::new_for_http())
            .layer(rate_limit)
            .layer(dns_guard)
    }

    pub async fn serve(mut self) -> std::io::Result<()> {
        let provider: Arc<dyn crate::helpscout::HelpScoutProvider> =
            match (self.state.sync.as_ref(), self.state.provider_kind.as_str()) {
                (Some(sync), _) => sync.provider().clone(),
                (None, "real") => self
                    .state
                    .real
                    .clone()
                    .map(|r| r as Arc<dyn crate::helpscout::HelpScoutProvider>)
                    .unwrap_or_else(|| {
                        Arc::new(crate::helpscout::FakeHelpScoutProvider::new_demo())
                    }),
                (None, _) => Arc::new(crate::helpscout::FakeHelpScoutProvider::new_demo()),
            };
        let manager = crate::workers::start_workers(
            self.state.conn.clone(),
            self.state.sync.clone(),
            provider,
            self.state.bus.clone(),
            self.state.data_dir.clone(),
            Some(self.state.qdrant.clone()),
        );
        if self.state.provider_kind == "fake" {
            let conversation_count: i64 = {
                let conn = self.state.conn_lock();
                conn.query_row("SELECT COUNT(*) FROM conversations", [], |r| r.get(0))
                    .unwrap_or(0)
            };
            if conversation_count == 0 {
                if let Some(engine) = self.state.sync.clone() {
                    let conn = self.state.conn.clone();
                    let demo_mode = self.state.demo_mode;
                    tokio::spawn(async move {
                        if let Err(e) = engine.initial_sync().await {
                            tracing::warn!(error = %e, "Demo initial sync failed");
                            return;
                        }
                        let c = conn.lock().unwrap_or_else(|p| p.into_inner());
                        let _ = crate::jobs::enqueue_on(
                            &c,
                            "embeddings",
                            "embed_knowledge_chunks",
                            "{}",
                            4,
                            2,
                        );
                        let seeded = crate::demo::seed_demo_data(&c, demo_mode);
                        if seeded {
                            let _ = crate::settings::set_string(&c, "demo_data_loaded", "true");
                        }
                    });
                }
            }
        }
        let workers_handle = manager.clone();
        self.state.workers = Some(manager);
        let app = self
            .build_router()
            .into_make_service_with_connect_info::<std::net::SocketAddr>();
        let listener = TcpListener::bind(self.addr).await?;
        tracing::info!(addr = %self.addr, "HTTP API server bound");
        axum::serve(listener, app)
            .with_graceful_shutdown(async move {
                wait_for_shutdown_signal().await;
                workers_handle.stop();
            })
            .await?;
        Ok(())
    }
}

pub async fn wait_for_shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        let mut term = signal(SignalKind::terminate()).expect("failed to install SIGTERM handler");
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {},
            _ = term.recv() => {},
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}

async fn rewrite_405_to_404(
    req: axum::http::Request<axum::body::Body>,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let resp = next.run(req).await;
    if resp.status() == axum::http::StatusCode::METHOD_NOT_ALLOWED {
        return (
            axum::http::StatusCode::NOT_FOUND,
            [("Content-Type", "application/json")],
            r#"{"statusCode":404,"error":"NotFound","message":"Unknown API endpoint."}"#,
        )
            .into_response();
    }
    resp
}

async fn log_500_to_application_errors(
    State(state): State<AppState>,
    req: axum::http::Request<axum::body::Body>,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let method = req.method().clone();
    let uri = req.uri().path().to_string();
    let resp = next.run(req).await;
    if resp.status().as_u16() >= 500 {
        let status = resp.status().as_u16();
        let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
        let _ = crate::audit::log_error(
            &conn,
            "api",
            &format!("{status} on {method} {uri}"),
            None,
            None,
        );
    }
    resp
}

pub fn catch_panic_response(
    err: Box<dyn std::any::Any + Send + 'static>,
) -> axum::response::Response {
    let detail = if let Some(s) = err.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = err.downcast_ref::<String>() {
        s.clone()
    } else {
        "no panic message".to_string()
    };
    tracing::error!(panic = %detail, "handler panic contained (C3): serving 500");
    (
        axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        [("Content-Type", "application/json")],
        serde_json::json!({
            "statusCode": 500,
            "error": "InternalError",
            "message": "Internal error",
            "detail": detail
        })
        .to_string(),
    )
        .into_response()
}

fn is_loopback_host(host: &str) -> bool {
    if host.is_empty() {
        return true;
    }
    let mut h = host.to_lowercase();
    if !h.ends_with(']') {
        if let Some(idx) = h.rfind(':') {
            if idx > 0 && h[idx + 1..].chars().all(|c| c.is_ascii_digit()) {
                h.truncate(idx);
            }
        }
    }
    h = h.trim_start_matches('[').trim_end_matches(']').to_string();
    h == "localhost" || h == "127.0.0.1" || h == "::1" || h == "::ffff:127.0.0.1"
}
