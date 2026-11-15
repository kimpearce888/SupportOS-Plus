use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::FutureExt;
use rusqlite::Connection;
use std::panic::AssertUnwindSafe;

use crate::error::{Error, Result};
use crate::event_bus::EventBus;
use crate::helpscout::{ConversationPatch, CreateThreadInput, HelpScoutProvider};
use crate::jobs;
use crate::outbound;
use crate::sync_engine::SyncEngine;

mod execute;

type TranslateChatOutput<'a> = std::result::Result<(Option<String>, String), String>;
type TranslateChatFuture<'a> =
    std::pin::Pin<Box<dyn std::future::Future<Output = TranslateChatOutput<'a>> + 'a>>;
type CoachingChatOutput<'a> = std::result::Result<(Option<String>, String, u64), String>;
type CoachingChatFuture<'a> =
    std::pin::Pin<Box<dyn std::future::Future<Output = CoachingChatOutput<'a>> + 'a>>;

fn block_ai_pipeline<F: std::future::Future>(fut: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("AI pipeline runtime")
        .block_on(fut)
}

struct TickGuard<'a> {
    flag: &'a AtomicBool,
}

impl<'a> TickGuard<'a> {
    fn acquire(flag: &'a AtomicBool) -> Option<Self> {
        (!flag.swap(true, Ordering::SeqCst)).then_some(Self { flag })
    }
}

impl Drop for TickGuard<'_> {
    fn drop(&mut self) {
        self.flag.store(false, Ordering::SeqCst);
    }
}

pub struct WorkerManager {
    db: crate::db::DbHandle,
    engine: Option<Arc<SyncEngine>>,
    provider: Arc<dyn HelpScoutProvider>,
    bus: EventBus,
    data_dir: PathBuf,
    qdrant: Option<Arc<crate::vectorstore_sqlitevec::VectorIndex>>,
    running: AtomicBool,
    stopped: AtomicBool,
    processing: AtomicBool,
    sweeping_notifications: AtomicBool,
    sweeping_customer_events: AtomicBool,
    sweeping_intelligence: AtomicBool,
    refreshing_connectors: AtomicBool,
    sync_nudge: std::sync::Arc<tokio::sync::Notify>,
    handles: Mutex<Vec<tokio::task::JoinHandle<()>>>,
    executing_jobs: Mutex<std::collections::HashSet<i64>>,
    boot_unix: i64,
}

impl WorkerManager {
    #[must_use]
    pub fn new(
        db: crate::db::DbHandle,
        engine: Option<Arc<SyncEngine>>,
        provider: Arc<dyn HelpScoutProvider>,
        bus: EventBus,
        data_dir: PathBuf,
        qdrant: Option<Arc<crate::vectorstore_sqlitevec::VectorIndex>>,
    ) -> Self {
        Self {
            db,
            engine,
            provider,
            bus,
            data_dir,
            qdrant,
            running: AtomicBool::new(false),
            stopped: AtomicBool::new(false),
            processing: AtomicBool::new(false),
            sweeping_notifications: AtomicBool::new(false),
            sweeping_customer_events: AtomicBool::new(false),
            sweeping_intelligence: AtomicBool::new(false),
            refreshing_connectors: AtomicBool::new(false),
            sync_nudge: std::sync::Arc::new(tokio::sync::Notify::new()),
            handles: Mutex::new(Vec::new()),
            executing_jobs: Mutex::new(std::collections::HashSet::new()),
            boot_unix: chrono::Utc::now().timestamp(),
        }
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    fn lock(&self) -> crate::db::DbWriteGuard<'_> {
        self.db.write_guard()
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

    async fn contained_value<F: std::future::Future<Output = i64>>(
        label: &'static str,
        f: F,
    ) -> i64 {
        match AssertUnwindSafe(f).catch_unwind().await {
            Ok(processed) => processed,
            Err(_) => {
                tracing::error!(
                    timer = label,
                    "timer tick panicked (contained at the task boundary; timer continues)"
                );
                0
            }
        }
    }

    pub fn start(self: &Arc<Self>) {
        if self.running.swap(true, Ordering::SeqCst) {
            return;
        }
        self.stopped.store(false, Ordering::SeqCst);
        self.processing.store(false, Ordering::SeqCst);
        self.sweeping_notifications.store(false, Ordering::SeqCst);
        self.sweeping_customer_events.store(false, Ordering::SeqCst);
        self.sweeping_intelligence.store(false, Ordering::SeqCst);
        self.refreshing_connectors.store(false, Ordering::SeqCst);
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
            if let Err(e) = outbound::recover_stale_sending(&conn) {
                tracing::warn!(error = %e, "outbound recovery failed");
            }
            if crate::sync_engine::recover_interrupted_state(&conn) {
                tracing::info!(
                    operation = "sync_recover",
                    "Interrupted sync state recovered at worker start"
                );
            }
            if let Err(e) = crate::sync_engine::cancel_orphaned_runs(&conn) {
                tracing::warn!(error = %e, "orphaned sync run cleanup failed");
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
            let (ceiling_minutes, raw) = {
                let conn = self.lock();
                let env_fallback = crate::config::load_env_config().sync.interval_minutes;
                let raw = crate::settings::get_i64(&conn, "sync_interval_minutes", env_fallback)
                    .unwrap_or(env_fallback);
                let clamped = raw.clamp(1, 1440);
                (clamped, raw)
            };
            let nudge = std::sync::Arc::clone(&self.sync_nudge);
            let this = self.clone();
            self.spawn(async move {
                let busy = Duration::from_secs(30);
                let ceiling =
                    Duration::from_secs(u64::try_from(ceiling_minutes * 60).unwrap_or(3600));
                let mut quiet_delay = busy;
                loop {
                    let _ = tokio::time::timeout(quiet_delay, nudge.notified()).await;
                    let processed = Self::contained_value("auto_sync", this.auto_sync()).await;
                    quiet_delay = if processed > 0 {
                        busy
                    } else {
                        std::cmp::min(quiet_delay.saturating_mul(2), ceiling)
                    };
                }
            });
            tracing::info!(
                operation = "start",
                quiet_ceiling_minutes = raw,
                busy_poll_seconds = 30,
                "worker auto-sync timer armed (adaptive: 30 s busy, backing off toward the ceiling)"
            );
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
                        let executing = this
                            .executing_jobs
                            .lock()
                            .unwrap_or_else(|p| p.into_inner())
                            .clone();
                        let conn = this.lock();
                        match jobs::requeue_stale_running_jobs(
                            &conn,
                            30,
                            &executing,
                            this.boot_unix,
                        ) {
                            Ok(n) if n > 0 => tracing::warn!(
                                count = n,
                                operation = "stale_job_sweep",
                                "Re-queued stale running job(s) orphaned by a previous process"
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
            let seconds = {
                let conn = self.lock();
                crate::settings::get_i64(&conn, "intelligence_sweep_seconds", 900)
                    .unwrap_or(900)
                    .clamp(60, 86400)
            };
            self.intelligence_sweep_tick();
            let this = self.clone();
            self.spawn(async move {
                let mut interval = tokio::time::interval(Duration::from_secs(
                    u64::try_from(seconds).unwrap_or(900),
                ));
                loop {
                    interval.tick().await;
                    Self::contained("intelligence_sweep", async {
                        this.intelligence_sweep_tick();
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
        {
            let minutes = {
                let conn = self.lock();
                crate::settings::get_i64(&conn, "reconcile_interval_minutes", 1_440)
                    .unwrap_or(1_440)
                    .clamp(0, 10_080)
            };
            if minutes > 0 {
                tracing::info!(
                    operation = "start",
                    interval_minutes = minutes,
                    "scheduled reconcile timer armed (closes the 10-minute incremental-overlap gap)"
                );
                let this = self.clone();
                self.spawn(async move {
                    let mut interval = tokio::time::interval(Duration::from_secs(
                        u64::try_from(minutes * 60).unwrap_or(86_400),
                    ));
                    interval.tick().await;
                    loop {
                        interval.tick().await;
                        Self::contained("scheduled_reconcile", this.scheduled_reconcile_tick())
                            .await;
                    }
                });
            }
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

    pub(crate) async fn tick(&self) {
        if self.stopped.load(Ordering::SeqCst) {
            return;
        }
        let Some(_tick_guard) = TickGuard::acquire(&self.processing) else {
            return;
        };
        self.flush_outbound().await;
        for _ in 0..10 {
            let claimed = {
                let mut conn = self.lock();
                jobs::claim_next(&mut conn, None).unwrap_or(None)
            };
            let Some(job) = claimed else { break };
            self.executing_jobs
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .insert(job.id);
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
                    false,
                );
                drop(conn);
                crate::event_bus::notify_error(
                    &self.bus,
                    "error",
                    &format!("background job {} panicked and was marked failed", job.kind),
                    "jobs",
                    None,
                );
            };
            self.executing_jobs
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .remove(&job.id);
        }
    }

    async fn flush_outbound(&self) {
        for _ in 0..outbound::OUTBOUND_BATCH_PER_TICK {
            let claimed = {
                let mut conn = self.lock();
                outbound::claim_next(&mut conn).unwrap_or(None)
            };
            let Some(job) = claimed else { break };
            if AssertUnwindSafe(self.execute_outbound(&job))
                .catch_unwind()
                .await
                .is_err()
            {
                tracing::error!(
                    job_id = job.id,
                    kind = %job.kind,
                    "outbound job panicked (contained at the worker boundary)"
                );
                let conn = self.lock();
                let _ = outbound::mark_failed(
                    &conn,
                    job.id,
                    "dispatcher panicked (contained at worker boundary)",
                    true,
                );
                self.emit_outbound_finished(&job, false, Some("dispatcher panicked"));
                crate::event_bus::notify_error(
                    &self.bus,
                    "error",
                    "the outbound dispatcher panicked while delivering; the job was marked failed",
                    "outbound",
                    job.conversation_local_id,
                );
            }
        }
    }

    async fn execute_outbound(&self, job: &outbound::OutboundJob) {
        let started = std::time::Instant::now();
        let payload: serde_json::Value =
            serde_json::from_str(&job.payload).unwrap_or(serde_json::Value::Null);

        let blocked_by_eval_mode = {
            let conn = self.lock();
            if crate::conversation_ops::eval_mode_on(&conn) {
                let _ = outbound::mark_failed(
                    &conn,
                    job.id,
                    "AI evaluation mode is ON: no replies, notes or status changes are sent.",
                    false,
                );
                self.emit_outbound_finished(job, false, Some("blocked by AI evaluation mode"));
                true
            } else {
                false
            }
        };
        if blocked_by_eval_mode {
            return;
        }

        if self.provider.kind() != "real" {
            let conn = self.lock();
            let _ =
                outbound::mark_failed(&conn, job.id, crate::conversation_ops::NOT_CONNECTED, false);
            self.emit_outbound_finished(job, false, Some(crate::conversation_ops::NOT_CONNECTED));
            return;
        }

        let conversation_local = job
            .conversation_local_id
            .or_else(|| outbound_num(&payload, "conversationId"));
        let Some(conversation_local) = conversation_local else {
            let conn = self.lock();
            let _ =
                outbound::mark_failed(&conn, job.id, "outbound job has no conversation id", false);
            self.emit_outbound_finished(job, false, Some("missing conversation id"));
            return;
        };
        let ctx = {
            let conn = self.lock();
            conn.query_row(
                "SELECT c.remote_id,
                        (SELECT cu.remote_id FROM customers cu WHERE cu.id = c.customer_local_id)
                   FROM conversations c WHERE c.id = ?1",
                rusqlite::params![conversation_local],
                |r| Ok((r.get::<_, i64>(0)?, r.get::<_, Option<i64>>(1)?)),
            )
            .ok()
        };
        let Some((remote_id, customer_remote)) = ctx else {
            let conn = self.lock();
            let _ = outbound::mark_failed(
                &conn,
                job.id,
                "conversation no longer exists locally",
                false,
            );
            self.emit_outbound_finished(job, false, Some("conversation not found locally"));
            return;
        };

        let summary = format!("{} conversation {remote_id}", job.kind);
        let result: Result<String> = match job.kind.as_str() {
            "create_reply" => {
                self.send_create_reply(&payload, remote_id, customer_remote, conversation_local)
                    .await
            }
            "create_note" => self.send_create_note(&payload, remote_id).await,
            "update_status" => self.send_update_status(&payload, remote_id).await,
            "assign" => self.send_assign(&payload, remote_id).await,
            "update_tags" => self.send_update_tags(&payload, remote_id).await,
            "snooze" => self.send_snooze(&payload, remote_id).await,
            "unsnooze" => self
                .provider
                .unsnooze_conversation(remote_id)
                .await
                .map(|_| "snooze removed".to_string()),
            other => Err(Error::Other(
                format!("unknown outbound job kind {other}").into(),
            )),
        };

        let latency_ms = i64::try_from(started.elapsed().as_millis()).unwrap_or(0);
        match result {
            Ok(remote_result) => {
                {
                    let conn = self.lock();
                    let _ = jobs::record_outbound_attempt(
                        &conn,
                        job.id,
                        job.attempts + 1,
                        &summary,
                        None,
                        Some(&remote_result),
                        Some(latency_ms),
                    );
                    let _ = outbound::mark_confirmed(&conn, job.id, &remote_result);
                    let action = match job.kind.as_str() {
                        "create_reply" => "reply_sent",
                        "create_note" => "note_sent",
                        "update_status" => "status_changed",
                        "update_tags" => "tags_updated",
                        "snooze" => "snoozed",
                        "unsnooze" => "unsnoozed",
                        _ => "conversation_assigned",
                    };
                    let _ = crate::jobs::audit(
                        &conn,
                        "user",
                        action,
                        Some(conversation_local),
                        None,
                        None,
                        Some(&summary),
                        Some(&remote_result),
                        false,
                    );
                    self.emit_outbound_finished(job, true, None);
                }
                if let Err(e) = self
                    .run_engine(|e| async move {
                        e.sync_single_conversation(remote_id).await.map(|_| ())
                    })
                    .await
                {
                    tracing::warn!(
                        job_id = job.id,
                        error = %e,
                        "post-send refresh failed (the delivery itself succeeded)"
                    );
                }
                self.emit_conversation_updated_for_remote(remote_id);
                if let Some(local) = self.conversation_local_id_for_remote(remote_id) {
                    self.on_conversation_changed(local);
                }
                self.nudge_sync();
            }
            Err(e) => {
                let retryable = crate::helpscout_real::hs_error(&e)
                    .map(|hs| hs.retryable)
                    .unwrap_or(false);
                let msg = crate::helpscout_real::hs_error(&e)
                    .map(|hs| hs.friendly.clone())
                    .unwrap_or_else(|| e.to_string());
                let conn = self.lock();
                let _ = jobs::record_outbound_attempt(
                    &conn,
                    job.id,
                    job.attempts + 1,
                    &summary,
                    crate::helpscout_real::hs_status(&e).map(i64::from),
                    Some(&msg),
                    Some(latency_ms),
                );
                let will_retry =
                    outbound::mark_failed(&conn, job.id, &msg, retryable).unwrap_or(false);
                let _ = crate::audit::log_event(
                    &conn,
                    if will_retry { "warn" } else { "error" },
                    "outbound",
                    &format!(
                        "{} failed{}: {msg}",
                        job.kind,
                        if will_retry { " (will retry)" } else { "" }
                    ),
                    None,
                    Some(&serde_json::json!({ "conversation_id": conversation_local })),
                );
                tracing::warn!(
                    job_id = job.id,
                    kind = %job.kind,
                    retry = will_retry,
                    error = %msg,
                    "outbound job failed"
                );
                self.emit_outbound_finished(job, false, Some(&msg));
                crate::event_bus::notify_error(
                    &self.bus,
                    if will_retry { "warn" } else { "error" },
                    &msg,
                    "outbound",
                    job.conversation_local_id,
                );
            }
        }
    }
    async fn send_create_reply(
        &self,
        payload: &serde_json::Value,
        remote_id: i64,
        customer_remote: Option<i64>,
        conversation_local: i64,
    ) -> Result<String> {
        let text = outbound_str_or(payload, "text", "");
        if text.trim().is_empty() {
            Err(Error::Other("reply text is empty".into()))
        } else {
            let cc: Vec<String> = payload["cc"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default();
            let bcc: Vec<String> = payload["bcc"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default();
            let status_after = payload["statusAfter"]
                .as_str()
                .filter(|s| !s.is_empty())
                .map(str::to_string);
            let assign_to = outbound_num(payload, "assignTo");
            let customer_remote = if customer_remote.is_some() {
                customer_remote
            } else {
                match self.provider.get_conversation(remote_id).await {
                    Ok(Some(conv)) if conv.customer_id > 0 => {
                        let cust_remote = conv.customer_id;
                        {
                            let conn = self.lock();
                            if let Some(customer_local) =
                                crate::sync_engine::local_id(&conn, "customers", cust_remote)
                            {
                                let _ = conn.execute(
                                    "UPDATE conversations SET customer_local_id = ?1
                                       WHERE id = ?2 AND customer_local_id IS NULL",
                                    rusqlite::params![customer_local, conversation_local],
                                );
                            }
                        }
                        tracing::info!(
                            remote_id,
                            customer_remote_id = cust_remote,
                            "reply dispatch: customer resolved from the live conversation \
                             (mirror row repaired)"
                        );
                        Some(cust_remote)
                    }
                    Ok(_) => None,
                    Err(e) => {
                        tracing::warn!(
                            error = %e,
                            "reply dispatch: could not resolve the customer from the API"
                        );
                        None
                    }
                }
            };
            match customer_remote {
                Some(customer_id) => self
                    .provider
                    .create_reply_thread(CreateThreadInput {
                        conversation_id: remote_id,
                        text,
                        draft: false,
                        cc,
                        bcc,
                        status_after,
                        assign_to,
                        customer_id: Some(customer_id),
                    })
                    .await
                    .map(|created| format!("reply delivered (thread {})", created.thread_id)),
                None => Err(Error::Other(
                    "the reply cannot be delivered: Help Scout requires the customer \
                     being replied to, and this conversation has no customer in the \
                     local mirror or on the server (it may have been deleted or \
                     merged). Refresh the conversation and retry."
                        .into(),
                )),
            }
        }
    }

    async fn send_create_note(
        &self,
        payload: &serde_json::Value,
        remote_id: i64,
    ) -> Result<String> {
        let text = outbound_str_or(payload, "text", "");
        if text.trim().is_empty() {
            Err(Error::Other("note text is empty".into()))
        } else {
            self.provider
                .create_note_thread(CreateThreadInput {
                    conversation_id: remote_id,
                    text,
                    draft: false,
                    cc: vec![],
                    bcc: vec![],
                    status_after: None,
                    assign_to: None,
                    customer_id: None,
                })
                .await
                .map(|created| format!("note delivered (thread {})", created.thread_id))
        }
    }

    async fn send_update_status(
        &self,
        payload: &serde_json::Value,
        remote_id: i64,
    ) -> Result<String> {
        let status = outbound_str_or(payload, "status", "");
        if !matches!(status.as_str(), "active" | "pending" | "closed" | "spam") {
            Err(Error::Other(format!("invalid status {status}").into()))
        } else {
            self.provider
                .update_conversation(
                    remote_id,
                    ConversationPatch {
                        status: Some(status.clone()),
                        ..Default::default()
                    },
                )
                .await
                .map(|_| format!("status set to {status}"))
        }
    }

    async fn send_assign(&self, payload: &serde_json::Value, remote_id: i64) -> Result<String> {
        let user_local = outbound_num(payload, "userId");
        let assign_remote = user_local.and_then(|local| {
            let conn = self.lock();
            conn.query_row(
                "SELECT remote_id FROM users WHERE id = ?1",
                rusqlite::params![local],
                |r| r.get::<_, i64>(0),
            )
            .ok()
        });
        if user_local.is_some() && assign_remote.is_none() {
            Err(Error::Other(
                "assignee is not a synced Help Scout user (cannot assign remotely)".into(),
            ))
        } else {
            self.provider
                .update_conversation(
                    remote_id,
                    ConversationPatch {
                        assign_to: Some(assign_remote),
                        ..Default::default()
                    },
                )
                .await
                .map(|_| match assign_remote {
                    Some(remote) => format!("assigned to user {remote}"),
                    None => "unassigned".to_string(),
                })
        }
    }

    async fn send_update_tags(
        &self,
        payload: &serde_json::Value,
        remote_id: i64,
    ) -> Result<String> {
        match payload.get("tags").and_then(|v| v.as_array()) {
            None => Err(Error::Other("tag update job has no tags list".into())),
            Some(values) => {
                let tags: Vec<String> = values
                    .iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect();
                self.provider
                    .update_tags(remote_id, tags)
                    .await
                    .map(|_| format!("tags updated on conversation {remote_id}"))
            }
        }
    }

    async fn send_snooze(&self, payload: &serde_json::Value, remote_id: i64) -> Result<String> {
        let until = payload["snoozedUntil"]
            .as_str()
            .filter(|s| !s.trim().is_empty())
            .map(str::to_string);
        match until {
            None => Err(Error::Other(
                "snooze job is missing the snoozedUntil timestamp".into(),
            )),
            Some(snoozed_until) => {
                let unsnooze_on_reply =
                    payload["unsnoozeOnCustomerReply"].as_bool().unwrap_or(true);
                self.provider
                    .snooze_conversation(remote_id, snoozed_until.clone(), unsnooze_on_reply)
                    .await
                    .map(|_| format!("snoozed until {snoozed_until}"))
            }
        }
    }

    fn emit_outbound_finished(
        &self,
        job: &outbound::OutboundJob,
        success: bool,
        error: Option<&str>,
    ) {
        crate::event_bus::notify_outbound_finished(
            &self.bus,
            job.id,
            &job.kind,
            job.conversation_local_id,
            success,
            error,
        );
    }

    fn engine_engaged(&self) -> bool {
        self.engine.as_ref().is_some_and(|e| e.is_running())
    }

    pub(crate) fn nudge_sync(&self) {
        self.sync_nudge.notify_one();
    }

    async fn scheduled_reconcile_tick(&self) {
        if self.stopped.load(Ordering::SeqCst) {
            return;
        }
        if self.provider.kind() == "empty" {
            return;
        }
        let Some(engine) = self.engine.clone() else {
            return;
        };
        if engine.is_running() {
            tracing::debug!(
                operation = "scheduled_reconcile",
                "a sync is already running — reconcile deferred to the next tick"
            );
            return;
        }
        match engine.reconcile().await {
            Ok(result) => {
                tracing::info!(
                    operation = "scheduled_reconcile",
                    checked = result.checked,
                    added = result.added,
                    updated = result.updated,
                    deleted = result.deleted,
                    skipped = result.skipped,
                    "Scheduled reconcile finished"
                );
                let conn = self.lock();
                let _ = crate::audit::log_event(
                    &conn,
                    "info",
                    "sync",
                    &format!(
                        "Scheduled reconcile: {} checked, {} added, {} updated, {} deleted",
                        result.checked, result.added, result.updated, result.deleted
                    ),
                    None,
                    None,
                );
            }
            Err(e) => {
                tracing::warn!(
                    operation = "scheduled_reconcile",
                    error = %e,
                    "Scheduled reconcile failed"
                );
                let conn = self.lock();
                let _ = crate::jobs::log_error(
                    &conn,
                    "sync",
                    &format!("Scheduled reconcile failed: {e}"),
                );
            }
        }
    }

    async fn auto_sync(&self) -> i64 {
        if self.stopped.load(Ordering::SeqCst) {
            return 0;
        }
        if self.provider.kind() == "empty" {
            return 0;
        }
        let state = {
            let conn = self.lock();
            crate::sync_engine::recover_interrupted_state(&conn);
            crate::sync_engine::get_state(&conn)
        };
        if (state == "INITIALIZING" || state == "BACKFILLING") && self.engine_engaged() {
            return 0;
        }
        let Some(engine) = self.engine.clone() else {
            return 0;
        };
        let initial_pull = state == "NEW";
        let result = if initial_pull {
            tracing::info!(state, "first sync: starting the initial pull");
            engine.initial_sync(crate::events::SyncTrigger::Auto).await
        } else {
            engine
                .incremental_sync(crate::events::SyncTrigger::Auto)
                .await
        };
        match result {
            Ok(results) => {
                if initial_pull {
                    self.on_after_initial_sync();
                }
                let processed: i64 = results.iter().map(|r| r.processed).sum();
                tracing::debug!(processed, operation = "auto_sync", "Adaptive tick finished");
                processed
            }
            Err(e) => {
                let msg = e.to_string();
                if !msg.contains("already running") {
                    tracing::warn!(error = %e, operation = "auto_sync", "Automatic sync failed");
                    let conn = self.lock();
                    let _ = crate::audit::log_event(
                        &conn,
                        "error",
                        "sync",
                        &format!("Automatic sync failed: {msg}"),
                        None,
                        None,
                    );
                    drop(conn);
                    crate::event_bus::notify_error(
                        &self.bus,
                        "error",
                        &format!("Automatic sync failed: {msg}"),
                        "sync",
                        None,
                    );
                }
                0
            }
        }
    }

    async fn maintenance(&self) {
        let db_handle = self.db.clone();
        let data_dir = self.data_dir.clone();
        let bus = self.bus.clone();
        let provider_kind = self.provider.kind().to_string();
        let result = tokio::task::spawn_blocking(move || -> Result<()> {
            let conn = db_handle.write_guard();
            let backup_hours =
                crate::settings::get_i64(&conn, "backup_interval_hours", 24).unwrap_or(24);
            let mut backup_due = false;
            if backup_hours > 0 {
                let backups_dir = data_dir.join("backups");
                let backups = crate::backup_service::list_backups(&backups_dir);
                let newest = backups.first();
                backup_due = match newest {
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
            }
            drop(conn);
            if backup_due {
                let backups_dir = data_dir.join("backups");
                let long_conn = db_handle.long_op_guard();
                let result = crate::backup_service::backup(&long_conn, &backups_dir);
                if result.ok {
                    tracing::info!(operation = "backup", "Automatic backup created");
                }
            }
            let backups_dir = data_dir.join("backups");
            let pruned = crate::backup_service::prune_backups(&backups_dir, 20);
            if pruned > 0 {
                tracing::info!(operation = "backup", pruned, "Pruned old backups");
            }
            let conn = db_handle.write_guard();
            if let Err(e) = crate::maintenance::compute_trends(&conn) {
                tracing::warn!(error = %e, operation = "maintenance", "compute_trends failed");
            }
            let _ = crate::maintenance::refresh_products(&conn);
            enqueue_activity_rebuild_repair_once(&conn);
            if crate::ai_hub::ai_feature_enabled(&conn, "issue_clustering")
                && provider_kind != "empty"
            {
                let last_run_unix =
                    crate::settings::get_i64(&conn, "ai_clustering_last_run", 0).unwrap_or(0);
                let now_unix = chrono::Utc::now().timestamp();
                if now_unix - last_run_unix >= 20 * 3600 {
                    let _ = crate::jobs::enqueue_on(&conn, "ai", "cluster_issues", "{}", 3, 2);
                    let _ = crate::settings::set_i64(&conn, "ai_clustering_last_run", now_unix);
                }
            }
            if crate::ai_hub::ai_feature_enabled(&conn, "gap_detection") && provider_kind != "empty"
            {
                let last_run_unix =
                    crate::settings::get_i64(&conn, "ai_gaps_last_run", 0).unwrap_or(0);
                let now_unix = chrono::Utc::now().timestamp();
                if now_unix - last_run_unix >= 20 * 3600 {
                    let _ = crate::jobs::enqueue_on(&conn, "ai", "rebuild_gaps", "{}", 3, 2);
                    let _ = crate::settings::set_i64(&conn, "ai_gaps_last_run", now_unix);
                }
            }
            if crate::ai_hub::ai_feature_enabled(&conn, "weekly_digest") && provider_kind != "empty"
            {
                let last_run_unix =
                    crate::settings::get_i64(&conn, "ai_digest_last_run", 0).unwrap_or(0);
                let now_unix = chrono::Utc::now().timestamp();
                if now_unix - last_run_unix >= 7 * 24 * 3600 {
                    let model_configured =
                        crate::ai_hub::resolve_role(&conn, crate::ai_hub::AiRole::Summarize)
                            .is_some()
                            || crate::settings::get_string(&conn, "lmstudio_base_url")
                                .ok()
                                .flatten()
                                .is_some_and(|s| !s.is_empty());
                    if model_configured {
                        let _ = crate::jobs::enqueue_on(
                            &conn,
                            "ai",
                            "weekly_digest",
                            "{\"triggerKind\":\"scheduled\",\"days\":7}",
                            3,
                            2,
                        );
                        let _ = crate::settings::set_i64(&conn, "ai_digest_last_run", now_unix);
                    }
                }
            }
            if crate::ai_hub::ai_feature_enabled(&conn, "golden_eval")
                && provider_kind != "empty"
                && crate::ai_eval::golden_fingerprint_stale(&conn)
            {
                let _ = crate::jobs::enqueue_on(&conn, "ai", "golden_eval", "{}", 3, 2);
            }
            if crate::ai_hub::ai_feature_enabled(&conn, "post_resolution_qa")
                && provider_kind != "empty"
            {
                let last_run_unix =
                    crate::settings::get_i64(&conn, "ai_qa_rebuild_last_run", 0).unwrap_or(0);
                let now_unix = chrono::Utc::now().timestamp();
                if now_unix - last_run_unix >= 20 * 3600 {
                    let rebuilt = crate::quality::rebuild_qa(&conn);
                    let _ = crate::settings::set_i64(&conn, "ai_qa_rebuild_last_run", now_unix);
                    if rebuilt > 0 {
                        tracing::info!(
                            operation = "maintenance",
                            rebuilt,
                            "post-resolution QA table rebuilt"
                        );
                    }
                }
            }
            if provider_kind != "empty"
                && crate::webhook_health::cadence_due(&conn).unwrap_or(false)
            {
                let _ = crate::jobs::enqueue_on(&conn, "sync", "webhook_health_check", "{}", 3, 2);
            }
            let _ = crate::notification_sweep::sweep(&conn, Some(&bus));
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

    pub(crate) fn notification_sweep_tick(&self) {
        if self.stopped.load(Ordering::SeqCst) {
            return;
        }
        let Some(_guard) = TickGuard::acquire(&self.sweeping_notifications) else {
            return;
        };
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
    }

    pub(crate) fn customer_event_sweep_tick(&self) {
        if self.stopped.load(Ordering::SeqCst) {
            return;
        }
        let Some(_guard) = TickGuard::acquire(&self.sweeping_customer_events) else {
            return;
        };
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
    }

    pub(crate) fn intelligence_sweep_tick(&self) {
        if self.stopped.load(Ordering::SeqCst) {
            return;
        }
        let Some(_guard) = TickGuard::acquire(&self.sweeping_intelligence) else {
            return;
        };
        let conn = self.lock();
        match crate::intelligence_features::sweep(&conn) {
            Ok(()) => {}
            Err(e) => {
                tracing::warn!(error = %e, operation = "intelligence_sweep", "Intelligence sweep failed")
            }
        }
    }

    async fn connector_refresh_tick(&self) {
        if self.stopped.load(Ordering::SeqCst) {
            return;
        }
        let Some(_guard) = TickGuard::acquire(&self.refreshing_connectors) else {
            return;
        };
        let due: Vec<i64> = {
            let conn = self.lock();
            crate::connectors::due_for_refresh(&conn).unwrap_or_default()
        };
        let data_dir = self.data_dir.clone();
        for id in due {
            match crate::connectors::refresh(&self.db, Some(&self.bus), id, &data_dir).await {
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
    }
}

fn outbound_num(payload: &serde_json::Value, key: &str) -> Option<i64> {
    payload.get(key).and_then(|v| v.as_i64()).or_else(|| {
        payload
            .get(key)
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse().ok())
    })
}

fn outbound_str_or(payload: &serde_json::Value, key: &str, fallback: &str) -> String {
    payload
        .get(key)
        .and_then(|v| v.as_str())
        .unwrap_or(fallback)
        .to_string()
}

fn enqueue_activity_rebuild_repair_once(conn: &Connection) {
    let conversations: i64 = conn
        .query_row("SELECT COUNT(*) FROM conversations", [], |r| r.get(0))
        .unwrap_or(0);
    let repaired = crate::settings::get_i64(conn, "activity_rebuild_repaired", 0).unwrap_or(0);
    if conversations > 0 && repaired == 0 {
        let _ = jobs::enqueue_on(conn, "sync", "rebuild_activity", "{}", 3, 2);
        let _ = crate::settings::set_i64(conn, "activity_rebuild_repaired", 1);
        tracing::info!(
            operation = "maintenance",
            "one-time activity rebuild enqueued for the pre-fix mirror (audit item 22)"
        );
    }
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
            conversation_id = COALESCE(excluded.conversation_id, ratings.conversation_id),
            customer_local_id = COALESCE(excluded.customer_local_id, ratings.customer_local_id),
            user_local_id = COALESCE(excluded.user_local_id, ratings.user_local_id),
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
    db: crate::db::DbHandle,
    engine: Option<Arc<SyncEngine>>,
    provider: Arc<dyn HelpScoutProvider>,
    bus: EventBus,
    data_dir: PathBuf,
    qdrant: Option<Arc<crate::vectorstore_sqlitevec::VectorIndex>>,
) -> Arc<WorkerManager> {
    let manager = Arc::new(WorkerManager::new(
        db, engine, provider, bus, data_dir, qdrant,
    ));
    manager.start();
    manager
}

#[cfg(test)]
mod audp2_reconcile_tests;
#[cfg(test)]
mod audp2_workers_tests;
#[cfg(test)]
mod auto_sync_tests;
#[cfg(test)]
mod batch14_tests;
#[cfg(test)]
mod batch20_audit22_tests;
#[cfg(test)]
mod batch27_workers_tests;
#[cfg(test)]
mod batch28_workers_tests;
