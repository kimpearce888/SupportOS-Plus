use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use futures_util::FutureExt;
use rusqlite::Connection;
use std::panic::AssertUnwindSafe;

use crate::error::{Error, Result};
use crate::eventing::EventBus;
use crate::helpscout::HelpScoutProvider;
use crate::jobs;
use crate::sync_engine::SyncEngine;

pub const RATINGS_REFRESH_DEFAULT_SECONDS: i64 = 30;

pub struct WorkerManager {
    conn: Arc<Mutex<Connection>>,
    engine: Option<Arc<SyncEngine>>,
    provider: Arc<dyn HelpScoutProvider>,
    bus: EventBus,
    data_dir: PathBuf,
    qdrant: Option<Arc<crate::vectorstore_qdrant::EmbeddedQdrant>>,
    running: AtomicBool,
    stopped: AtomicBool,
    processing: AtomicBool,
    refreshing_ratings: AtomicBool,
    sweeping_notifications: AtomicBool,
    sweeping_customer_events: AtomicBool,
    refreshing_connectors: AtomicBool,
    handles: Mutex<Vec<tokio::task::JoinHandle<()>>>,
}

impl WorkerManager {
    #[must_use]
    pub fn new(
        conn: Arc<Mutex<Connection>>,
        engine: Option<Arc<SyncEngine>>,
        provider: Arc<dyn HelpScoutProvider>,
        bus: EventBus,
        data_dir: PathBuf,
        qdrant: Option<Arc<crate::vectorstore_qdrant::EmbeddedQdrant>>,
    ) -> Self {
        Self {
            conn,
            engine,
            provider,
            bus,
            data_dir,
            qdrant,
            running: AtomicBool::new(false),
            stopped: AtomicBool::new(false),
            processing: AtomicBool::new(false),
            refreshing_ratings: AtomicBool::new(false),
            sweeping_notifications: AtomicBool::new(false),
            sweeping_customer_events: AtomicBool::new(false),
            refreshing_connectors: AtomicBool::new(false),
            handles: Mutex::new(Vec::new()),
        }
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    fn lock(&self) -> MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(|p| p.into_inner())
    }

    fn spawn(&self, task: impl std::future::Future<Output = ()> + Send + 'static) {
        let handle = tokio::spawn(task);
        self.handles
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .push(handle);
    }

    async fn contained<F: std::future::Future>(label: &'static str, f: F) {
        if AssertUnwindSafe(f).catch_unwind().await.is_err() {
            tracing::error!(
                timer = label,
                "timer tick panicked (contained at the task boundary; timer continues)"
            );
        }
    }

    pub fn start(self: &Arc<Self>) {
        if self.running.swap(true, Ordering::SeqCst) {
            return;
        }
        self.stopped.store(false, Ordering::SeqCst);
        {
            let conn = self.lock();
            match jobs::recover_stale_jobs(&conn) {
                Ok(n) if n > 0 => {
                    tracing::info!(
                        count = n,
                        operation = "recover",
                        "Recovered stale jobs after restart"
                    );
                }
                Ok(_) => {}
                Err(e) => tracing::warn!(error = %e, "stale-job recovery failed"),
            }
            let drained = crate::webhook_handler::drain_pending(&conn);
            if drained > 0 {
                tracing::info!(
                    count = drained,
                    operation = "webhook_drain",
                    "Drained pending webhook events after restart"
                );
            }
        }

        {
            let this = self.clone();
            self.spawn(async move {
                let mut interval = tokio::time::interval(Duration::from_secs(2));
                loop {
                    interval.tick().await;
                    Self::contained("job_loop", this.tick()).await;
                }
            });
        }
        {
            let (minutes, raw) = {
                let conn = self.lock();
                let raw = crate::settings::get_i64(&conn, "sync_interval_minutes", 5).unwrap_or(5);
                let clamped = raw.clamp(1, 1440);
                (clamped, raw)
            };
            let this = self.clone();
            self.spawn(async move {
                let mut interval = tokio::time::interval(Duration::from_secs(
                    u64::try_from(minutes * 60).unwrap_or(300),
                ));
                loop {
                    interval.tick().await;
                    Self::contained("auto_sync", this.auto_sync()).await;
                }
            });
            tracing::info!(
                operation = "start",
                sync_interval_minutes = raw,
                "worker auto-sync timer armed"
            );
        }
        {
            let seconds = {
                let conn = self.lock();
                crate::settings::get_i64(
                    &conn,
                    "ratings_refresh_seconds",
                    RATINGS_REFRESH_DEFAULT_SECONDS,
                )
                .unwrap_or(RATINGS_REFRESH_DEFAULT_SECONDS)
                .clamp(0, 3600)
            };
            if seconds > 0 {
                let this = self.clone();
                self.spawn(async move {
                    let mut interval = tokio::time::interval(Duration::from_secs(
                        u64::try_from(seconds).unwrap_or(30),
                    ));
                    loop {
                        interval.tick().await;
                        Self::contained("refresh_ratings", this.refresh_ratings()).await;
                    }
                });
            }
        }
        {
            let this = self.clone();
            self.spawn(async move {
                let mut interval = tokio::time::interval(Duration::from_secs(6 * 3600));
                loop {
                    interval.tick().await;
                    Self::contained("maintenance", this.maintenance()).await;
                }
            });
        }
        {
            let this = self.clone();
            self.spawn(async move {
                let mut interval = tokio::time::interval(Duration::from_secs(10 * 60));
                loop {
                    interval.tick().await;
                    Self::contained("stale_job_sweep", async {
                        let conn = this.lock();
                        match jobs::requeue_stale_running_jobs(&conn, 30) {
                            Ok(n) if n > 0 => tracing::warn!(
                                count = n,
                                operation = "stale_job_sweep",
                                "Re-queued stale running job(s) (running longer than 30 minutes)"
                            ),
                            Ok(_) => {}
                            Err(_) => {}
                        }
                    })
                    .await;
                }
            });
        }
        {
            let seconds = {
                let conn = self.lock();
                crate::settings::get_i64(&conn, "notification_sweep_seconds", 15)
                    .unwrap_or(15)
                    .clamp(5, 3600)
            };
            self.notification_sweep_tick();
            let this = self.clone();
            self.spawn(async move {
                let mut interval = tokio::time::interval(Duration::from_secs(
                    u64::try_from(seconds).unwrap_or(15),
                ));
                loop {
                    interval.tick().await;
                    Self::contained("notification_sweep", async {
                        this.notification_sweep_tick();
                    })
                    .await;
                }
            });
        }
        {
            let seconds = {
                let conn = self.lock();
                crate::settings::get_i64(&conn, "customer_event_sweep_seconds", 60)
                    .unwrap_or(60)
                    .clamp(15, 3600)
            };
            self.customer_event_sweep_tick();
            let this = self.clone();
            self.spawn(async move {
                let mut interval = tokio::time::interval(Duration::from_secs(
                    u64::try_from(seconds).unwrap_or(60),
                ));
                loop {
                    interval.tick().await;
                    Self::contained("customer_event_sweep", async {
                        this.customer_event_sweep_tick();
                    })
                    .await;
                }
            });
        }
        {
            let this = self.clone();
            self.spawn(async move {
                let mut interval = tokio::time::interval(Duration::from_secs(30));
                loop {
                    interval.tick().await;
                    Self::contained("connector_refresh", this.connector_refresh_tick()).await;
                }
            });
        }
        tracing::info!(operation = "start", "Background workers started");
    }

    pub fn stop(&self) {
        self.stopped.store(true, Ordering::SeqCst);
        self.running.store(false, Ordering::SeqCst);
        let mut handles = self.handles.lock().unwrap_or_else(|p| p.into_inner());
        for h in handles.drain(..) {
            h.abort();
        }
        tracing::info!(operation = "stop", "Background workers stopped");
    }

    pub async fn tick(&self) {
        if self.stopped.load(Ordering::SeqCst) || self.processing.swap(true, Ordering::SeqCst) {
            return;
        }
        for _ in 0..10 {
            let claimed = {
                let mut conn = self.lock();
                jobs::claim_next(&mut conn, None).unwrap_or(None)
            };
            let Some(job) = claimed else { break };
            if AssertUnwindSafe(self.execute_job(job.id, &job.kind, &job.payload))
                .catch_unwind()
                .await
                .is_err()
            {
                tracing::error!(
                    job_id = job.id,
                    kind = %job.kind,
                    "job panicked (contained at the worker boundary): marking failed"
                );
                let conn = self.lock();
                let _ = jobs::fail(
                    &conn,
                    job.id,
                    "job panicked (contained at worker boundary)",
                    true,
                );
            }
        }
        self.processing.store(false, Ordering::SeqCst);
    }

    async fn auto_sync(&self) {
        if self.stopped.load(Ordering::SeqCst) {
            return;
        }
        let state = {
            let conn = self.lock();
            crate::sync_engine::get_state(&conn)
        };
        if state == "NEW" || state == "INITIALIZING" || state == "BACKFILLING" {
            return;
        }
        let Some(engine) = self.engine.clone() else {
            return;
        };
        if let Err(e) = engine.incremental_sync().await {
            tracing::warn!(error = %e, operation = "incremental_sync", "Incremental sync failed");
        }
    }

    async fn refresh_ratings(&self) {
        if self.stopped.load(Ordering::SeqCst)
            || self.refreshing_ratings.swap(true, Ordering::SeqCst)
        {
            return;
        }
        let result: Result<()> = async {
            let ratings = self.provider.list_ratings().await?;
            let mut fresh = 0usize;
            for r in &ratings {
                let (inserted, conv_local, conv_number, customer_local) = {
                    let conn = self.lock();
                    let conv_local = r.conversation_id.and_then(|id| {
                        if id > 0 {
                            crate::sync_engine::conversation_local_id(&conn, id)
                        } else {
                            None
                        }
                    });
                    let conv_number = conv_local.and_then(|local| {
                        conn.query_row(
                            "SELECT number FROM conversations WHERE id = ?1",
                            rusqlite::params![local],
                            |row| row.get::<_, i64>(0),
                        )
                        .ok()
                    });
                    let customer_local = r.customer_id.and_then(|id| {
                        if id > 0 {
                            crate::sync_engine::local_id(&conn, "customers", id)
                        } else {
                            None
                        }
                    });
                    let user_local = r.user_id.and_then(|id| {
                        if id > 0 {
                            crate::sync_engine::local_id(&conn, "users", id)
                        } else {
                            None
                        }
                    });
                    (
                        upsert_rating_row(&conn, r, conv_local, customer_local, user_local)?,
                        conv_local,
                        conv_number,
                        customer_local,
                    )
                };
                if inserted {
                    fresh += 1;
                    crate::eventing::notify_rating_received(
                        &self.bus,
                        r.rating.as_deref(),
                        conv_local,
                        conv_number,
                        customer_local,
                        r.customer_name.as_deref(),
                        r.comment.as_deref(),
                    );
                }
            }
            if fresh > 0 {
                crate::eventing::notify_ratings_refreshed(
                    &self.bus,
                    ratings.len() as u32,
                    fresh as u32,
                );
                tracing::info!(
                    operation = "ratings_refresh",
                    fresh,
                    processed = ratings.len(),
                    "Ratings refresh pushed new ratings"
                );
            }
            Ok(())
        }
        .await;
        if let Err(e) = result {
            tracing::warn!(error = %e, operation = "ratings_refresh", "Ratings refresh failed");
        }
        self.refreshing_ratings.store(false, Ordering::SeqCst);
    }

    async fn maintenance(&self) {
        let conn = self.conn.clone();
        let data_dir = self.data_dir.clone();
        let bus = self.bus.clone();
        let result = tokio::task::spawn_blocking(move || -> Result<()> {
            let conn = conn.lock().unwrap_or_else(|p| p.into_inner());
            crate::maintenance::compute_trends(&conn)?;
            let _ = crate::maintenance::refresh_products(&conn);
            let _ = crate::notification_sweep::sweep(&conn, Some(&bus));
            let backup_hours =
                crate::settings::get_i64(&conn, "backup_interval_hours", 24).unwrap_or(24);
            if backup_hours > 0 {
                let backups_dir = data_dir.join("backups");
                let backups = crate::backup_service::list_backups(&backups_dir);
                let newest = backups.first();
                let due = match newest {
                    None => true,
                    Some(b) => {
                        let created = b
                            .get("created_at")
                            .and_then(|v| v.as_str())
                            .and_then(parse_iso_to_unix);
                        match created {
                            Some(ts) => std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .map(|d| d.as_secs() as i64 - ts >= backup_hours * 3600)
                                .unwrap_or(true),
                            None => true,
                        }
                    }
                };
                if due {
                    let result = crate::backup_service::backup(&conn, &backups_dir);
                    if result.ok {
                        tracing::info!(operation = "backup", "Automatic backup created");
                    }
                }
            }
            let backups_dir = data_dir.join("backups");
            let pruned = crate::backup_service::prune_backups(&backups_dir, 20);
            if pruned > 0 {
                tracing::info!(operation = "backup", pruned, "Pruned old backups");
            }
            let retention_days = crate::settings::get_i64(&conn, "retention_days", 0).unwrap_or(0);
            let removed = crate::maintenance::enforce_retention(&conn, retention_days)?;
            if removed > 0 {
                tracing::info!(operation = "retention", removed, "Retention pruning");
            }
            Ok(())
        })
        .await
        .map_err(|e| crate::error::Error::Config(format!("maintenance task: {e}")))
        .and_then(|r| r);
        if let Err(e) = result {
            tracing::warn!(error = %e, operation = "maintenance", "Maintenance failed");
        }
    }

    pub fn notification_sweep_tick(&self) {
        if self.stopped.load(Ordering::SeqCst)
            || self.sweeping_notifications.swap(true, Ordering::SeqCst)
        {
            return;
        }
        let conn = self.lock();
        match crate::notification_sweep::sweep(&conn, Some(&self.bus)) {
            Ok(result) if result.created > 0 => {
                tracing::debug!(
                    operation = "notification_sweep",
                    created = result.created,
                    "Notification sweep created notifications"
                );
            }
            Ok(_) => {}
            Err(e) => {
                tracing::warn!(error = %e, operation = "notification_sweep", "Notification sweep failed")
            }
        }
        self.sweeping_notifications.store(false, Ordering::SeqCst);
    }

    pub fn customer_event_sweep_tick(&self) {
        if self.stopped.load(Ordering::SeqCst)
            || self.sweeping_customer_events.swap(true, Ordering::SeqCst)
        {
            return;
        }
        let conn = self.lock();
        match crate::customer_events::sweep(&conn) {
            Ok(created) if created > 0 => {
                tracing::debug!(
                    operation = "customer_event_sweep",
                    created,
                    "Customer event sweep derived events"
                );
            }
            Ok(_) => {}
            Err(e) => {
                tracing::warn!(error = %e, operation = "customer_event_sweep", "Customer event sweep failed")
            }
        }
        self.sweeping_customer_events.store(false, Ordering::SeqCst);
    }

    async fn connector_refresh_tick(&self) {
        if self.stopped.load(Ordering::SeqCst)
            || self.refreshing_connectors.swap(true, Ordering::SeqCst)
        {
            return;
        }
        let due: Vec<i64> = {
            let conn = self.lock();
            crate::connectors::due_for_refresh(&conn).unwrap_or_default()
        };
        let data_dir = self.data_dir.clone();
        for id in due {
            match crate::connectors::refresh(&self.conn, id, &data_dir).await {
                Ok(result) if !result.ok => {
                    tracing::warn!(
                        operation = "connector_refresh",
                        connector = id,
                        error = result.error.as_deref().unwrap_or("unknown"),
                        "Connector refresh failed"
                    );
                }
                Ok(_) => {}
                Err(e) => tracing::warn!(
                    operation = "connector_refresh",
                    connector = id,
                    error = %e,
                    "Connector refresh failed"
                ),
            }
        }
        self.refreshing_connectors.store(false, Ordering::SeqCst);
    }

    async fn execute_job(&self, job_id: i64, kind: &str, payload: &str) {
        let started = std::time::Instant::now();
        let payload: serde_json::Value =
            serde_json::from_str(payload).unwrap_or(serde_json::Value::Null);
        let num = |key: &str| -> Option<i64> {
            payload.get(key).and_then(|v| v.as_i64()).or_else(|| {
                payload
                    .get(key)
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.parse().ok())
            })
        };
        let outcome: Result<()> = async {
            match kind {
                "initial_sync" => {
                    self.run_engine(|e| async move {
                        e.initial_sync().await.map(|_| ())
                    })
                    .await?;
                    self.on_after_initial_sync();
                }
                "incremental_sync" => {
                    self.run_engine(|e| async move { e.incremental_sync().await.map(|_| ()) })
                        .await?;
                }
                "reconciliation" => {
                    self.run_engine(|e| async move { e.reconcile().await.map(|_| ()) })
                        .await?;
                }
                "sync_conversation" | "sync_conversation_merge" => {
                    if let Some(remote) = num("remoteId").or_else(|| num("conversationId")) {
                        self.run_engine(|e| async move { e.sync_single_conversation(remote).await.map(|_| ()) })
                            .await?;
                        if kind == "sync_conversation" {
                            self.emit_conversation_updated_for_remote(remote);
                            if let Some(local) = self.conversation_local_id_for_remote(remote) {
                                self.on_conversation_changed(local);
                            }
                        }
                    }
                }
                "delete_conversation" => {
                    if let Some(remote) = num("remoteId") {
                        let conn = self.lock();
                        conn.execute(
                            "UPDATE conversations SET status = 'deleted',
                                 deleted_at = COALESCE(deleted_at, datetime('now'))
                               WHERE remote_id = ?1",
                            rusqlite::params![remote],
                        )?;
                    }
                }
                "sync_customer" => {
                    if let Some(remote) = num("remoteId")
                        && let Some(customer) = self.provider.get_customer(remote).await? {
                            let conn = self.lock();
                            crate::sync::upsert_customer(&conn, &customer)?;
                        }
                }
                "delete_customer" => {
                    if let Some(remote) = num("remoteId") {
                        let conn = self.lock();
                        conn.execute(
                            "UPDATE customers SET deleted_at = datetime('now') WHERE remote_id = ?1",
                            rusqlite::params![remote],
                        )?;
                    }
                }
                "sync_organizations" => {
                    let orgs = self.provider.list_organizations().await?;
                    let conn = self.lock();
                    for o in &orgs {
                        crate::sync_engine::upsert_organization(&conn, o)?;
                    }
                }
                "sync_tags" => {
                    let tags = self.provider.list_tags().await?;
                    let conn = self.lock();
                    for t in &tags {
                        crate::sync::upsert_tag(&conn, t)?;
                    }
                }
                "sync_user_statuses" => {
                    self.run_engine(|e| async move { e.run_resource("user_statuses", false).await.map(|_| ()) })
                        .await?;
                }
                "sync_conversation_ratings" => {
                    let rating_id = num("ratingId");
                    let conv_remote = num("conversationId").or_else(|| num("remoteId"));
                    if let Some(rating_id) = rating_id
                        && let Ok(Some(r)) = self.provider.get_rating(rating_id).await {
                            let (inserted, conv_local, conv_number, customer_local) = {
                                let conn = self.lock();
                                let conv_local = r
                                    .conversation_id
                                    .and_then(|id| if id > 0 { crate::sync_engine::conversation_local_id(&conn, id) } else { None });
                                let conv_number = conv_local.and_then(|local| {
                                    conn.query_row(
                                        "SELECT number FROM conversations WHERE id = ?1",
                                        rusqlite::params![local],
                                        |row| row.get::<_, i64>(0),
                                    )
                                    .ok()
                                });
                                let customer_local = r
                                    .customer_id
                                    .and_then(|id| if id > 0 { crate::sync_engine::local_id(&conn, "customers", id) } else { None });
                                let user_local = r
                                    .user_id
                                    .and_then(|id| if id > 0 { crate::sync_engine::local_id(&conn, "users", id) } else { None });
                                (
                                    upsert_rating_row(&conn, &r, conv_local, customer_local, user_local)?,
                                    conv_local,
                                    conv_number,
                                    customer_local,
                                )
                            };
                            if inserted {
                                crate::eventing::notify_rating_received(
                                    &self.bus,
                                    r.rating.as_deref(),
                                    conv_local,
                                    conv_number,
                                    customer_local,
                                    r.customer_name.as_deref(),
                                    r.comment.as_deref(),
                                );
                            }
                        }
                    if let Some(conv_remote) = conv_remote {
                        self.run_engine(|e| async move {
                            e.sync_single_conversation(conv_remote).await.map(|_| ())
                        })
                        .await?;
                    }
                }
                "embed_knowledge_chunks" => {
                    let conn = self.lock();
                    let n = crate::embeddings::embed_pending_knowledge(
                        &conn,
                        self.qdrant.as_deref(),
                    )
                    .unwrap_or(0);
                    tracing::debug!(operation = kind, embedded = n, "embedding pass");
                }
                "embed_docs_chunks" => {
                    let conn = self.lock();
                    let n = crate::embeddings::embed_pending_docs(
                        &conn,
                        self.qdrant.as_deref(),
                    )
                    .unwrap_or(0);
                    tracing::debug!(operation = kind, embedded = n, "embedding pass");
                }
                "embed_conversation_chunks" => {
                    let conn = self.lock();
                    let n = crate::embeddings::embed_pending_conversation_chunks(
                        &conn,
                        self.qdrant.as_deref(),
                    )
                    .unwrap_or(0);
                    tracing::debug!(operation = kind, embedded = n, "embedding pass");
                }
                "rebuild_search_index" => {
                    let conn = self.lock();
                    let n = crate::search::rebuild_indexes(&conn)?;
                    tracing::info!(operation = kind, conversations = n, "Search index rebuilt");
                }
                "rebuild_activity" => {
                    let conn = self.lock();
                    let summary = crate::activity::rebuild_all(&conn)?;
                    tracing::info!(
                        operation = kind,
                        conversations = summary.conversations,
                        events_written = summary.events_written,
                        events_total = summary.events_total,
                        "Activity rebuilt from the thread mirror (rebuildAll)"
                    );
                }
                "rebuild_embeddings" => {
                    {
                        let conn = self.lock();
                        conn.execute(
                            "UPDATE knowledge_chunks SET embedding_state = 'not_indexed'",
                            [],
                        )?;
                        jobs::enqueue_on(&conn, "embeddings", "embed_knowledge_chunks", "{}", 4, 2)?;
                    }
                }
                "outreach_send_batch" => {
                    if let Some(campaign_id) = num("campaignId") {
                        let summary = crate::outreach::send_batch(
                            &self.conn,
                            &self.provider,
                            campaign_id,
                        )
                        .await;
                        tracing::info!(
                            operation = kind,
                            campaign_id = campaign_id,
                            remaining = summary["remaining"].as_i64().unwrap_or(-1),
                            "Outreach send batch processed"
                        );
                    } else {
                        let conn = self.lock();
                        jobs::fail(&conn, job_id, "outreach_send_batch missing campaignId", true)?;
                        return Ok(());
                    }
                }
                "analyze_ticket" => {
                    let Some(conv_id) = num("conversationId") else {
                        let conn = self.lock();
                        jobs::fail(&conn, job_id, "analyze_ticket missing conversationId", true)?;
                        return Ok(());
                    };
                    self.run_ai_job(kind, conv_id, AiJob::Analyze).await?;
                }
                "generate_draft" => {
                    let Some(conv_id) = num("conversationId") else {
                        let conn = self.lock();
                        jobs::fail(&conn, job_id, "generate_draft missing conversationId", true)?;
                        return Ok(());
                    };
                    self.run_ai_job(kind, conv_id, AiJob::Draft).await?;
                }
                "create_ai_note" => {
                    let Some(conv_id) = num("conversationId") else {
                        let conn = self.lock();
                        jobs::fail(&conn, job_id, "create_ai_note missing conversationId", true)?;
                        return Ok(());
                    };
                    self.run_ai_job(kind, conv_id, AiJob::Note).await?;
                }
                "automation_action_awaiting_approval" => {
                    let approved = payload
                        .get("approved")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    if approved {
                        let action = payload.get("action").cloned();
                        let rule_id = num("ruleId");
                        let conversation_id = num("conversationId");
                        if let (Some(action), Some(rule_id), Some(conversation_id)) =
                            (action, rule_id, conversation_id)
                        {
                            let executed = {
                                let mut conn = self.lock();
                                crate::automation::execute_approved_action(
                                    &mut conn,
                                    conversation_id,
                                    &action,
                                )?
                            };
                            if executed {
                                let kind = action
                                    .get("kind")
                                    .and_then(|k| k.as_str())
                                    .unwrap_or_default()
                                    .to_string();
                                let conn = self.lock();
                                crate::automation::record_approved_run(
                                    &conn,
                                    rule_id,
                                    conversation_id,
                                    &kind,
                                )?;
                            }
                        }
                    } else {
                        let conn = self.lock();
                        jobs::park_job(&conn, job_id)?;
                    }
                }
                "download_recent_attachments" => {
                    self.run_attachment_downloads().await;
                }
                k if k.starts_with("bulk_") => {
                    let action = k.trim_start_matches("bulk_");
                    if let Err(e) = self.execute_bulk_action(action, &payload).await {
                        let msg = e.to_string();
                        let conn = self.lock();
                        let _ = jobs::fail(&conn, job_id, &msg, true);
                        let _ = jobs::log_error(
                            &conn,
                            "workers",
                            &format!("Job {kind} failed: {msg}"),
                        );
                        return Ok(());
                    }
                }
                "rating.process" => {
                    self.persist_simulated_rating(&payload)?;
                }
                "sync.conversations" => {
                    self.run_engine(|e| async move { e.incremental_sync().await.map(|_| ()) })
                        .await?;
                }
                _ => {
                    let conn = self.lock();
                    jobs::fail(&conn, job_id, &format!("Unknown job type: {kind}"), false)?;
                    return Ok(());
                }
            }
            let conn = self.lock();
            jobs::complete(&conn, job_id)?;
            tracing::debug!(
                operation = kind,
                latency_ms = started.elapsed().as_millis() as u64,
                "Job completed"
            );
            Ok(())
        }
        .await;
        if let Err(e) = outcome {
            let msg = e.to_string();
            let conn = self.lock();
            let retryable = true;
            if retryable {
                let _ = jobs::fail(&conn, job_id, &msg, true);
            }
            let _ = jobs::log_error(&conn, "workers", &format!("Job {kind} failed: {msg}"));
            tracing::warn!(operation = kind, error = %msg, "Job failed");
        }
    }

    fn on_after_initial_sync(&self) {
        let conn = self.lock();
        let _ = crate::maintenance::refresh_products(&conn);
        let auto_download =
            crate::settings::get_i64(&conn, "attachment_auto_download", 1).unwrap_or(1) != 0;
        let _ = jobs::enqueue_on(&conn, "embeddings", "embed_knowledge_chunks", "{}", 4, 2);
        let _ = jobs::enqueue_on(&conn, "embeddings", "embed_conversation_chunks", "{}", 4, 2);
        if auto_download {
            let _ = jobs::enqueue_on(
                &conn,
                "attachments",
                "download_recent_attachments",
                "{}",
                4,
                2,
            );
        }
        let automatic_analysis =
            crate::settings::get_bool(&conn, "automatic_analysis_enabled", true).unwrap_or(true);
        let ai_enabled = !matches!(
            crate::ai_pipeline::backend_from_settings(&conn),
            crate::ai_pipeline::AiBackend::Disabled
        );
        if automatic_analysis && ai_enabled {
            let fresh: Vec<i64> = {
                let mut stmt = match conn.prepare(
                    "SELECT c.id FROM conversations c
                      WHERE NOT EXISTS (
                            SELECT 1 FROM ai_runs a
                             WHERE a.conversation_id = c.id
                               AND a.type = 'ticket_analysis'
                               AND a.status = 'completed')
                        AND c.deleted_at IS NULL
                      ORDER BY c.id LIMIT 50",
                ) {
                    Ok(s) => s,
                    Err(_) => return,
                };
                let ids: Vec<i64> = match stmt.query_map([], |r| r.get::<_, i64>(0)) {
                    Ok(rows) => rows.filter_map(|r| r.ok()).collect(),
                    Err(_) => Vec::new(),
                };
                ids
            };
            for id in fresh {
                let _ = jobs::enqueue_on(
                    &conn,
                    "ai",
                    "analyze_ticket",
                    &format!("{{\"conversationId\":{id}}}"),
                    3,
                    2,
                );
            }
        }
    }

    fn emit_conversation_updated_for_remote(&self, remote_id: i64) {
        use crate::events::ServerEvent;
        let conn = self.lock();
        let row: Option<(i64, i64, Option<i64>, Option<String>)> = conn
            .query_row(
                "SELECT id, number, mailbox_local_id, subject FROM conversations WHERE remote_id = ?1",
                rusqlite::params![remote_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .ok();
        if let Some((id, number, mailbox_id, subject)) = row {
            self.bus.emit(&ServerEvent::conversation_updated(
                Some(id),
                Some(number),
                mailbox_id,
                subject,
                "sync",
            ));
        }
    }

    fn conversation_local_id_for_remote(&self, remote_id: i64) -> Option<i64> {
        let conn = self.lock();
        conn.query_row(
            "SELECT id FROM conversations WHERE remote_id = ?1",
            rusqlite::params![remote_id],
            |r| r.get(0),
        )
        .ok()
    }

    fn on_conversation_changed(&self, conversation_local_id: i64) {
        let result: Result<()> = (|| {
            let last_thread_type: Option<String> = {
                let conn = self.lock();
                conn.query_row(
                    "SELECT type FROM conversation_threads
                      WHERE conversation_id = ?1 AND deleted_at IS NULL
                      ORDER BY created_at DESC, id DESC LIMIT 1",
                    rusqlite::params![conversation_local_id],
                    |r| r.get(0),
                )
                .ok()
            };
            if last_thread_type.as_deref() != Some("customer") {
                return Ok(());
            }
            {
                let mut conn = self.lock();
                let _ = crate::automation::fire_trigger_for_conversation(
                    &mut conn,
                    "new_conversation",
                    conversation_local_id,
                );
                let _ = crate::automation::fire_trigger_for_conversation(
                    &mut conn,
                    "customer_reply",
                    conversation_local_id,
                );
            }
            {
                let conn = self.lock();
                let automatic_analysis =
                    crate::settings::get_bool(&conn, "automatic_analysis_enabled", true)
                        .unwrap_or(true);
                let ai_enabled = !matches!(
                    crate::ai_pipeline::backend_from_settings(&conn),
                    crate::ai_pipeline::AiBackend::Disabled
                );
                if automatic_analysis && ai_enabled {
                    let _ = jobs::enqueue_on(
                        &conn,
                        "ai",
                        "analyze_ticket",
                        &format!("{{\"conversationId\":{conversation_local_id}}}"),
                        3,
                        2,
                    );
                }
                let _ = crate::interaction_current::record_current_interaction(
                    &conn,
                    conversation_local_id,
                );
            }
            Ok(())
        })();
        if let Err(e) = result {
            tracing::warn!(
                error = %e,
                conversation_id = conversation_local_id,
                operation = "on_conversation_changed",
                "Automation fire on conversation change failed (non-fatal)"
            );
        }
    }

    async fn run_engine<F, Fut>(&self, f: F) -> Result<()>
    where
        F: FnOnce(Arc<SyncEngine>) -> Fut,
        Fut: std::future::Future<Output = Result<()>>,
    {
        let Some(engine) = self.engine.clone() else {
            return Ok(());
        };
        f(engine).await
    }

    async fn run_ai_job(&self, kind: &str, conv_id: i64, job: AiJob) -> Result<()> {
        let backend = {
            let conn = self.lock();
            crate::ai_pipeline::backend_from_settings(&conn)
        };
        if matches!(backend, crate::ai_pipeline::AiBackend::Disabled) {
            tracing::info!(
                operation = kind,
                conversation_id = conv_id,
                "AI backend disabled - job completed without AI output"
            );
            return Ok(());
        }
        let conn_arc = self.conn.clone();
        let (result, note_text, remote_id) = tokio::task::spawn_blocking(move || {
            let conn = conn_arc.lock().unwrap_or_else(|p| p.into_inner());
            tokio::runtime::Handle::current().block_on(async {
                let remote: Option<i64> = conn
                    .query_row(
                        "SELECT remote_id FROM conversations WHERE id = ?1 AND deleted_at IS NULL",
                        rusqlite::params![conv_id],
                        |r| r.get(0),
                    )
                    .ok();
                let remote_id = match remote {
                    Some(r) => r,
                    None => {
                        return (
                            Err(Error::Other("Conversation not found locally.".into())),
                            None,
                            None,
                        );
                    }
                };
                match job {
                    AiJob::Analyze => {
                        let r = crate::ai_pipeline::analyze_ticket(&conn, &backend, conv_id, false)
                            .await
                            .map(|_| ())
                            .map_err(|e| Error::Other(e.to_string().into()));
                        (r, None, Some(remote_id))
                    }
                    AiJob::Draft => {
                        let mode = "standard".to_string();
                        let r = crate::ai_pipeline::generate_draft(
                            &conn, &backend, conv_id, &mode, false, None,
                        )
                        .await
                        .map(|_| ())
                        .map_err(|e| Error::Other(e.to_string().into()));
                        (r, None, Some(remote_id))
                    }
                    AiJob::Note => {
                        let analysis = match crate::ai_pipeline::analyze_ticket(
                            &conn, &backend, conv_id, false,
                        )
                        .await
                        {
                            Ok(outcome) => outcome.analysis,
                            Err(e) => {
                                return (
                                    Err(Error::Other(e.to_string().into())),
                                    None,
                                    Some(remote_id),
                                );
                            }
                        };
                        let mut body = String::new();
                        if let Some(summary) = &analysis.summary {
                            body.push_str(&format!("Summary: {summary}\n"));
                        }
                        if let Some(intent) = &analysis.intent {
                            body.push_str(&format!("Intent: {intent}\n"));
                        }
                        if let Some(goal) = &analysis.customer_goal {
                            body.push_str(&format!("Customer goal: {goal}\n"));
                        }
                        if let Some(sentiment) = &analysis.sentiment {
                            body.push_str(&format!("Sentiment: {sentiment}"));
                            if let Some(urgency) = &analysis.urgency {
                                body.push_str(&format!(" (urgency: {urgency})"));
                            }
                            body.push('\n');
                        }
                        if !analysis.missing_information.is_empty() {
                            body.push_str(&format!(
                                "Missing information: {}\n",
                                analysis.missing_information.join(", ")
                            ));
                        }
                        if body.trim().is_empty() {
                            return (
                                Err(Error::Other(
                                    "Ticket analysis carried no note content.".into(),
                                )),
                                None,
                                Some(remote_id),
                            );
                        }
                        body.push_str("\n(This note was drafted by SupportOS AI.)");
                        (Ok(()), Some(body), Some(remote_id))
                    }
                }
            })
        })
        .await
        .map_err(|e| Error::Other(e.to_string().into()))?;

        result?;
        if let (Some(text), Some(remote_id)) = (note_text, remote_id) {
            self.provider
                .create_note_thread(crate::helpscout::CreateThreadInput {
                    conversation_id: remote_id,
                    text,
                    draft: false,
                    cc: vec![],
                    bcc: vec![],
                    status_after: None,
                    assign_to: None,
                })
                .await?;
            let engine = self.engine.clone();
            if let Some(engine) = engine {
                engine.sync_single_conversation(remote_id).await?;
            }
            let conn = self.lock();
            let _ = crate::jobs::audit(
                &conn,
                "ai",
                "note_added",
                crate::sync_engine::conversation_local_id(&conn, remote_id),
                None,
                None,
                None,
                None,
                true,
            );
            self.emit_conversation_updated_for_remote(remote_id);
        }
        tracing::info!(
            operation = kind,
            conversation_id = conv_id,
            "AI job completed"
        );
        Ok(())
    }

    async fn run_attachment_downloads(&self) {
        let rows: Vec<(i64, i64, Option<i64>, Option<i64>)> = {
            let conn = self.lock();
            let Ok(mut stmt) = conn.prepare(
                "SELECT a.id, a.conversation_id, a.thread_id, a.remote_id
                   FROM attachments a
                   JOIN conversations c ON c.id = a.conversation_id
                  WHERE (a.state IS NULL OR a.state NOT IN ('downloaded'))
                    AND c.deleted_at IS NULL
                  ORDER BY a.id DESC LIMIT 100",
            ) else {
                return;
            };
            stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
                .map(|rows| rows.filter_map(|r| r.ok()).collect())
                .unwrap_or_default()
        };
        if rows.is_empty() {
            return;
        }
        let attachments_dir = self.data_dir.join("attachments");
        let mut downloaded = 0usize;
        for (id, conversation_id, thread_id, att_remote) in rows {
            let (conv_remote, thread_remote): (i64, Option<i64>) = {
                let conn = self.lock();
                let conv: i64 = conn
                    .query_row(
                        "SELECT remote_id FROM conversations WHERE id = ?1",
                        rusqlite::params![conversation_id],
                        |r| r.get(0),
                    )
                    .unwrap_or_default();
                let t: Option<i64> = thread_id.and_then(|tid| {
                    conn.query_row(
                        "SELECT remote_id FROM conversation_threads WHERE id = ?1",
                        rusqlite::params![tid],
                        |r| r.get(0),
                    )
                    .ok()
                });
                (conv, t)
            };
            match self
                .provider
                .get_attachment_data(
                    conv_remote,
                    thread_remote.unwrap_or(0),
                    att_remote.unwrap_or(0),
                )
                .await
            {
                Ok(Some(bytes)) => {
                    let conn = self.lock();
                    match crate::conversation_ops::persist_attachment_bytes(
                        &conn,
                        &attachments_dir,
                        id,
                        &bytes.data,
                    ) {
                        Ok(_) => downloaded += 1,
                        Err(msg) => {
                            tracing::warn!(attachment_id = id, error = %msg, "Attachment download failed");
                        }
                    }
                }
                Ok(None) => {
                    let conn = self.lock();
                    crate::conversation_ops::set_attachment_state(&conn, id, "failed", None, None);
                    tracing::warn!(attachment_id = id, "Attachment no longer available");
                }
                Err(e) => {
                    let conn = self.lock();
                    crate::conversation_ops::set_attachment_state(&conn, id, "failed", None, None);
                    tracing::warn!(attachment_id = id, error = %e, "Attachment download failed");
                }
            }
        }
        tracing::info!(count = downloaded, "Recent attachments downloaded");
    }

    async fn execute_bulk_action(&self, action: &str, payload: &serde_json::Value) -> Result<()> {
        use crate::helpscout::ConversationPatch;
        let Some(conv_id) = payload.get("conversationId").and_then(|v| v.as_i64()) else {
            return Err(Error::Other("bulk job missing conversationId".into()));
        };
        let row: Option<(i64, String)> = {
            let conn = self.lock();
            conn.query_row(
                "SELECT remote_id, status FROM conversations WHERE id = ?1 AND deleted_at IS NULL",
                rusqlite::params![conv_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .ok()
        };
        let Some((remote_id, _current_status)) = row else {
            return Err(Error::Other("Conversation not found locally.".into()));
        };
        match action {
            "tag" | "untag" => {
                let Some(tag) = payload.get("tag").and_then(|v| v.as_str()) else {
                    return Err(Error::Other(format!("bulk_{action} missing tag").into()));
                };
                let remote = self.provider.get_conversation(remote_id).await?;
                let current = remote.as_ref().map(|r| r.tags.clone()).unwrap_or_default();
                let mut tags = current;
                if action == "tag" {
                    if !tags.iter().any(|t| t.eq_ignore_ascii_case(tag)) {
                        tags.push(tag.to_string());
                    }
                } else {
                    tags.retain(|t| !t.eq_ignore_ascii_case(tag));
                }
                self.provider.update_tags(remote_id, tags).await?;
                {
                    let conn = self.lock();
                    let mut local = crate::conversation_ops::read_conversation_tags(&conn, conv_id);
                    if action == "tag" {
                        if !local.iter().any(|t| t.eq_ignore_ascii_case(tag)) {
                            local.push(tag.to_string());
                        }
                    } else {
                        local.retain(|t| !t.eq_ignore_ascii_case(tag));
                    }
                    crate::conversation_ops::write_conversation_tags(&conn, conv_id, &local);
                }
            }
            "assign" | "unassign" | "status" | "close" => {
                let patch = match action {
                    "close" => ConversationPatch {
                        status: Some("closed".into()),
                        ..Default::default()
                    },
                    "status" => {
                        let Some(status) = payload.get("status").and_then(|v| v.as_str()) else {
                            return Err(Error::Other("bulk_status missing status".into()));
                        };
                        const STATUSES: [&str; 4] = ["active", "closed", "pending", "spam"];
                        if !STATUSES.contains(&status) {
                            return Err(Error::Other(
                                format!("bulk_status invalid status {status}").into(),
                            ));
                        }
                        ConversationPatch {
                            status: Some(status.to_string()),
                            ..Default::default()
                        }
                    }
                    "assign" => {
                        let Some(user_remote) = payload.get("userId").and_then(|v| v.as_i64())
                        else {
                            return Err(Error::Other("bulk_assign missing userId".into()));
                        };
                        ConversationPatch {
                            assign_to: Some(Some(user_remote)),
                            ..Default::default()
                        }
                    }
                    _ => ConversationPatch {
                        assign_to: Some(None),
                        ..Default::default()
                    },
                };
                self.provider.update_conversation(remote_id, patch).await?;
                {
                    let mut conn = self.lock();
                    let assignee_local = if action == "assign" {
                        payload
                            .get("userId")
                            .and_then(|v| v.as_i64())
                            .and_then(|user_remote| {
                                let local =
                                    crate::sync_engine::local_id(&conn, "users", user_remote);
                                if local.is_none() {
                                    tracing::warn!(
                                        conversation_id = conv_id,
                                        user_remote,
                                        "bulk_assign: user not in local mirror; local assignee left unchanged"
                                    );
                                }
                                local
                            })
                    } else {
                        None
                    };
                    let tx = conn.transaction()?;
                    if let Some(status) = payload
                        .get("status")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                        .or_else(|| (action == "close").then(|| "closed".to_string()))
                        .filter(|s| !s.is_empty())
                    {
                        tx.execute(
                            "UPDATE conversations SET status = ?1,
                                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
                               WHERE id = ?2",
                            rusqlite::params![status, conv_id],
                        )?;
                        if status == "closed" {
                            tx.execute(
                                "UPDATE conversations SET closed_at = COALESCE(closed_at, datetime('now'))
                                   WHERE id = ?1",
                                rusqlite::params![conv_id],
                            )?;
                        }
                    }
                    if action == "assign"
                        && let Some(local) = assignee_local
                    {
                        tx.execute(
                            "UPDATE conversations SET assignee_local_id = ?1 WHERE id = ?2",
                            rusqlite::params![local, conv_id],
                        )?;
                    }
                    if action == "unassign" {
                        tx.execute(
                            "UPDATE conversations SET assignee_local_id = NULL WHERE id = ?1",
                            rusqlite::params![conv_id],
                        )?;
                    }
                    tx.commit()?;
                }
            }
            _ => {
                return Err(Error::Other(
                    format!("Unknown bulk action: {action}").into(),
                ));
            }
        }
        {
            let conn = self.lock();
            let _ = crate::jobs::audit(
                &conn,
                "user",
                &format!("bulk_{action}"),
                Some(conv_id),
                None,
                None,
                None,
                None,
                false,
            );
        }
        self.emit_conversation_updated_for_remote(remote_id);
        Ok(())
    }

    fn persist_simulated_rating(&self, payload: &serde_json::Value) -> Result<()> {
        let rating_num = payload.get("rating").and_then(|v| v.as_i64()).unwrap_or(5);
        let label = match rating_num {
            4 | 5 => "great",
            3 => "okay",
            _ => "not-good",
        };
        let conv_remote = payload
            .get("conversation_id")
            .and_then(|v| v.as_i64())
            .or_else(|| payload.get("conversationId").and_then(|v| v.as_i64()));
        let comment = payload.get("comment").and_then(|v| v.as_str());
        let conn = self.lock();
        let conv_local =
            conv_remote.and_then(|r| crate::sync_engine::conversation_local_id(&conn, r));
        conn.execute(
            "INSERT INTO ratings (conversation_id, rating, comments, raw_json, remote_created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![
                conv_local,
                label,
                comment,
                payload.to_string(),
                chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            ],
        )?;
        if let Some(conv_local) = conv_local {
            let _ = crate::jobs::audit(
                &conn,
                "user",
                "rating_simulated",
                Some(conv_local),
                None,
                Some(&format!("{{\"rating\":\"{label}\"}}")),
                None,
                None,
                false,
            );
        }
        tracing::info!(rating = label, "Simulated CSAT rating persisted");
        Ok(())
    }
}

#[derive(Clone, Copy)]
enum AiJob {
    Analyze,
    Draft,
    Note,
}

fn parse_iso_to_unix(s: &str) -> Option<i64> {
    let s = s.trim();
    if s.len() < 19 {
        return None;
    }
    let y: i64 = s.get(0..4)?.parse().ok()?;
    let mo: i64 = s.get(5..7)?.parse().ok()?;
    let d: i64 = s.get(8..10)?.parse().ok()?;
    let h: i64 = s.get(11..13)?.parse().ok()?;
    let mi: i64 = s.get(14..16)?.parse().ok()?;
    let se: i64 = s.get(17..19)?.parse().ok()?;
    let yy = if mo <= 2 { y - 1 } else { y };
    let era = yy.div_euclid(400);
    let yoe = yy - era * 400;
    let mp = if mo > 2 { mo - 3 } else { mo + 9 };
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Some(days * 86_400 + h * 3600 + mi * 60 + se)
}

fn upsert_rating_row(
    conn: &Connection,
    r: &crate::helpscout::HsRating,
    conversation_local_id: Option<i64>,
    customer_local_id: Option<i64>,
    user_local_id: Option<i64>,
) -> Result<bool> {
    let existing: Option<i64> = conn
        .query_row(
            "SELECT id FROM ratings WHERE remote_id = ?1",
            rusqlite::params![r.remote_id],
            |row| row.get(0),
        )
        .ok();
    conn.execute(
        "INSERT INTO ratings (remote_id, conversation_id, rating, comments,
             customer_local_id, user_local_id, remote_created_at, last_synced_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, datetime('now'))
         ON CONFLICT(remote_id) DO UPDATE SET
            rating = excluded.rating,
            comments = excluded.comments,
            last_synced_at = datetime('now')",
        rusqlite::params![
            r.remote_id,
            conversation_local_id,
            r.rating,
            r.comment,
            customer_local_id,
            user_local_id,
            r.created_at,
        ],
    )?;
    Ok(existing.is_none())
}

pub fn start_workers(
    conn: Arc<Mutex<Connection>>,
    engine: Option<Arc<SyncEngine>>,
    provider: Arc<dyn HelpScoutProvider>,
    bus: EventBus,
    data_dir: PathBuf,
    qdrant: Option<Arc<crate::vectorstore_qdrant::EmbeddedQdrant>>,
) -> Arc<WorkerManager> {
    let manager = Arc::new(WorkerManager::new(
        conn, engine, provider, bus, data_dir, qdrant,
    ));
    manager.start();
    manager
}
