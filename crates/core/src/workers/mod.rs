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

/// A3: the boxed future chat helpers handed to `translation::translate`
/// and `coaching::review_draft` — named so the closures stay readable.
mod execute;

type TranslateChatOutput<'a> = std::result::Result<(Option<String>, String), String>;
type TranslateChatFuture<'a> =
    std::pin::Pin<Box<dyn std::future::Future<Output = TranslateChatOutput<'a>> + 'a>>;
type CoachingChatOutput<'a> = std::result::Result<(Option<String>, String, u64), String>;
type CoachingChatFuture<'a> =
    std::pin::Pin<Box<dyn std::future::Future<Output = CoachingChatOutput<'a>> + 'a>>;

/// Drive an AI pipeline future to completion on a private, self-contained
/// current-thread runtime — the same isolation the embeddings lane uses
/// with its dedicated "lmstudio-embed" runtime.
///
/// Why not `Handle::current().block_on`: every AI runner below holds the
/// DbHandle write guard for the whole pipeline call (the pipeline API is
/// `&Connection`), so while the future runs nobody else can take the write
/// lock. `Handle::current()` resolves to the runtime that spawned this
/// blocking-pool thread, and its `block_on` parks THIS thread until that
/// runtime drives the future — but the spawning runtime can be a
/// current-thread runtime whose only thread is also free to block on the
/// very same write mutex (the worker timer tasks whose first interval
/// tick fires immediately, the sweeps, the reply dispatcher). When both
/// happen, both sides wait forever: this thread never releases the guard,
/// the runtime thread never wakes. That is a guaranteed deadlock for the
/// integration tests (which drive `tick()` on a current-thread runtime)
/// and a production stall: every DB write in the app would freeze for
/// the whole LLM round-trip. A private runtime drives the HTTP half
/// locally, so the job always completes — or fails honestly — on its own,
/// no matter what the spawning runtime is doing.
fn block_ai_pipeline<F: std::future::Future>(fut: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("AI pipeline runtime")
        .block_on(fut)
}

/// A1: RAII latch over a per-tick `AtomicBool` guard. `tick()` and the
/// sweep timers used to clear their flag only on the happy-path last
/// line, so a mid-tick `stop()` (task abort at an await point) or a
/// contained panic stranded the flag in the set state forever — every
/// future tick of that timer then returned instantly and the job queue
/// (or the sweep) silently died until process restart. Dropping the
/// latch clears the flag on every exit path: normal return, panic
/// unwind (contained at the task boundary), and abort during worker
/// shutdown/restart.
struct TickGuard<'a> {
    flag: &'a AtomicBool,
}

impl<'a> TickGuard<'a> {
    /// Acquire the tick guard; `None` means a previous tick of this timer
    /// is still in flight and the caller must return immediately.
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
    /// Item 150: the intelligence sweep's tick guard (SLA-breach
    /// persistence, unmonitored-mailbox tracking, doc freshness).
    sweeping_intelligence: AtomicBool,
    refreshing_connectors: AtomicBool,
    /// R8 (D10): wakes the adaptive auto-sync timer immediately — armed
    /// after every successful outbound send ("poll right after the app
    /// sends anything").
    sync_nudge: std::sync::Arc<tokio::sync::Notify>,
    handles: Mutex<Vec<tokio::task::JoinHandle<()>>>,
    /// Audit fix (M16): the set of job ids CURRENTLY executing inside this
    /// process. The stale-job sweep consults it so a long-but-alive job
    /// (outreach batch, LLM pass) can never be re-queued mid-run — the old
    /// sweep double-executed anything running past 30 minutes, duplicating
    /// sends.
    executing_jobs: Mutex<std::collections::HashSet<i64>>,
    /// Unix timestamp of this manager's boot. Jobs with started_at before
    /// the boot belong to a dead process and are safe for the sweep to
    /// re-queue; everything newer has a live executor in this process.
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

    /// Panic-guard wrapper that preserves the tick's value (0 on panic),
    /// for the adaptive auto-sync timer which reads the processed count.
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
        // A1: a restart used to inherit tick-guard flags stranded mid-tick
        // by the abort inside `stop()`; every subsequent tick of that
        // timer returned instantly and the queue/sweep never ran again
        // until process restart. Restart from a provably clean state.
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
            // 1.3.0: rows stuck mid-send by a crash (the pre-1.3.0 world
            // never dispatched at all, but a crash mid-POST is real now)
            // re-enter the queue so the user's reply eventually goes out.
            if let Err(e) = outbound::recover_stale_sending(&conn) {
                tracing::warn!(error = %e, "outbound recovery failed");
            }
            // 1.3.0: a sync_state stranded in INITIALIZING/BACKFILLING by
            // an unclean shutdown used to make the auto-sync timer skip
            // itself forever ("sync in progress", eternally). Recover it
            // before the first tick so the account actually fills.
            if crate::sync_engine::recover_interrupted_state(&conn) {
                tracing::info!(
                    operation = "sync_recover",
                    "Interrupted sync state recovered at worker start"
                );
            }
            // The same shutdown also strands the run ROWS in
            // INITIALIZING forever (17 on one machine); close them out so
            // Sync reports shows honest states. Safe here — no sync can
            // be running yet.
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
            // 1.4.0 R8 (D18): adaptive cadence replaces the fixed interval.
            // Busy accounts poll every 30 s; a quiet account backs off by
            // doubling toward the configured ceiling (the DB setting, with
            // the SYNC_INTERVAL_MINUTES env var finally consumed as its
            // fallback — both defaults now agree on 1 minute, closing the
            // two-divergent-defaults defect); and every send nudges the
            // timer awake immediately (see `nudge_sync`).
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
                    // Immediate poll after a send (R8/D10): `timeout`
                    // races the nudge against the quiet delay without
                    // needing the tokio macros feature. A notify permit
                    // stored while a run was in flight wakes the next
                    // tick instantly.
                    let _ = tokio::time::timeout(quiet_delay, nudge.notified()).await;
                    let processed = Self::contained_value("auto_sync", this.auto_sync()).await;
                    // Busy (changes arrived) keeps the 30 s poll; quiet
                    // doubles the delay toward the ceiling.
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
                        // Audit fix (M16): the sweep used to re-queue ANY job
                        // running longer than 30 minutes — including ones
                        // actively executing right here (outreach batches,
                        // LLM passes), which double-executed them and
                        // duplicated sends. Now it only re-queues jobs whose
                        // executor is provably gone: not in this process's
                        // executing set AND started before this process
                        // booted (i.e., orphaned by a previous process).
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
            // Item 150: the intelligence sweep cadence. Breach persistence
            // is deduped per conversation + kind, so a slow cadence loses
            // nothing; 15 minutes keeps the tracked log near the live page.
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
        if self.stopped.load(Ordering::SeqCst) {
            return;
        }
        // A1: the latch clears the flag on every exit path (see TickGuard).
        let Some(_tick_guard) = TickGuard::acquire(&self.processing) else {
            return;
        };
        // Outbound dispatch runs before the background job kinds: a
        // queued user reply outranks any maintenance work.
        self.flush_outbound().await;
        for _ in 0..10 {
            let claimed = {
                let mut conn = self.lock();
                jobs::claim_next(&mut conn, None).unwrap_or(None)
            };
            let Some(job) = claimed else { break };
            // Audit fix (M16): register the job as actively executing so the
            // stale-job sweep can never re-queue it mid-run.
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
                // Clean-audit item 86: a panic is a code defect, not a
                // transient fault — it recurs deterministically on
                // re-run, and each retry re-executed the job's early
                // side effects (a panicking copilot chat re-persisted
                // the user's message once per attempt). The contained
                // job fails terminally; the honest error sits on the
                // job row for the UI to surface.
                let _ = jobs::fail(
                    &conn,
                    job.id,
                    "job panicked (contained at worker boundary)",
                    false,
                );
                drop(conn);
                // R5 (D4): a panicking background job used to be visible
                // only in the stdout log; it must reach the UI too.
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

    /// Drain queued outbound jobs (replies, notes, status changes,
    /// assignments) to the live Help Scout provider — the half the
    /// Phase 4 cutover lost and 1.3.0 restores. Runs before the
    /// background job kinds every tick: a user reply outranks any
    /// maintenance work.
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

    /// Execute one outbound job against the provider, then either confirm
    /// (+ pull the remote truth back into the mirror) or retry/fail.
    async fn execute_outbound(&self, job: &outbound::OutboundJob) {
        let started = std::time::Instant::now();
        let payload: serde_json::Value =
            serde_json::from_str(&job.payload).unwrap_or(serde_json::Value::Null);

        // Defense in depth: eval mode must never reach the wire, even for
        // rows queued before the toggle flipped on.
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

        // Fail fast and legibly when no Help Scout account is connected —
        // otherwise the inert provider answers a cryptic 404.
        if self.provider.kind() != "real" {
            let conn = self.lock();
            let _ =
                outbound::mark_failed(&conn, job.id, crate::conversation_ops::NOT_CONNECTED, false);
            self.emit_outbound_finished(job, false, Some(crate::conversation_ops::NOT_CONNECTED));
            return;
        }

        let num = |key: &str| -> Option<i64> {
            payload.get(key).and_then(|v| v.as_i64()).or_else(|| {
                payload
                    .get(key)
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.parse().ok())
            })
        };
        let str_or = |key: &str, fallback: &str| -> String {
            payload
                .get(key)
                .and_then(|v| v.as_str())
                .unwrap_or(fallback)
                .to_string()
        };

        // Resolve local -> remote ids for the conversation, its customer
        // (the POST /reply body requires a customer object) and the
        // assignee (the PATCH assignTo value is a remote user/team id).
        let conversation_local = job.conversation_local_id.or_else(|| num("conversationId"));
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
                let text = str_or("text", "");
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
                    let assign_to = num("assignTo");
                    // R4 (D7): the reply endpoint requires the customer
                    // object, and a mirror row without a resolvable
                    // customer used to be a guaranteed 400. Resolve the
                    // primary customer from the live conversation and
                    // repair the mirror row so the next send (and every
                    // copilot surface) sees it too.
                    let customer_remote = if customer_remote.is_some() {
                        customer_remote
                    } else {
                        match self.provider.get_conversation(remote_id).await {
                            Ok(Some(conv)) if conv.customer_id > 0 => {
                                let cust_remote = conv.customer_id;
                                {
                                    let conn = self.lock();
                                    if let Some(customer_local) = crate::sync_engine::local_id(
                                        &conn,
                                        "customers",
                                        cust_remote,
                                    ) {
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
                            .map(|created| {
                                format!("reply delivered (thread {})", created.thread_id)
                            }),
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
            "create_note" => {
                let text = str_or("text", "");
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
            "update_status" => {
                let status = str_or("status", "");
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
            "assign" => {
                let user_local = num("userId");
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
                    // Local-only user (never synced): keep the local write,
                    // but tell the user the remote assignment cannot go out.
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
            // Audit item 19: automation-approved tag adds push through
            // the same outbound lane the manual inbox uses — a typed
            // local write with no push used to be reverted by the next
            // sync. The payload carries the LOCAL conversation id (the
            // lane's resolver owns local→remote) and the full tag list,
            // mirroring how update_tags works on the platform.
            "update_tags" => {
                let tags: Vec<String> = payload["tags"]
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .filter_map(|v| v.as_str().map(str::to_string))
                            .collect()
                    })
                    .unwrap_or_default();
                if tags.is_empty() {
                    Err(Error::Other("tag update with an empty tag list".into()))
                } else {
                    self.provider
                        .update_tags(remote_id, tags)
                        .await
                        .map(|_| format!("tags updated on conversation {remote_id}"))
                }
            }
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
                // Pull the remote truth back: the new thread (and any
                // status side effects) flow into the mirror and the UI.
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
                // R8 (D10): a send is the strongest busy signal there is —
                // the adaptive timer polls immediately instead of waiting
                // out its quiet back-off.
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
                // R5 (D4): also broadcast as a first-class error so the
                // invalidation map covers Diagnostics even when the
                // specific OutboundJobFinished handler changes. The shell
                // suppresses the duplicate toast for service="outbound" —
                // the outbound event already carries the exact message.
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

    /// True when a live engine run holds the state machine (the only
    /// legitimate reason to skip an auto-sync tick).
    fn engine_engaged(&self) -> bool {
        self.engine.as_ref().is_some_and(|e| e.is_running())
    }

    /// Wake the adaptive auto-sync timer for an immediate poll (R8/D10:
    /// every send is followed by a poll — no fixed-interval wait). Public
    /// so the outbound dispatcher can call it from its job context.
    pub fn nudge_sync(&self) {
        self.sync_nudge.notify_one();
    }

    /// One adaptive-timer tick. Returns how many records the run processed
    /// (0 = quiet; the timer backs off, R8/D18).
    async fn auto_sync(&self) -> i64 {
        if self.stopped.load(Ordering::SeqCst) {
            return 0;
        }
        // R2 (D1): the timer refuses to run against the empty provider.
        // Before this guard, a fresh install without credentials ran a
        // "successful" zero-record sync every minute — checkpoints
        // stamped forward and sync_state flipped to LIVE — so when real
        // credentials arrived the engine believed it was already up to
        // date and never pulled the account. (The engine has the same
        // guard; this one keeps the timer from even reading state.)
        if self.provider.kind() == "empty" {
            return 0;
        }
        let state = {
            let conn = self.lock();
            // Belt-and-braces: the same recovery that runs at boot, in
            // case the process died between a state write and here.
            crate::sync_engine::recover_interrupted_state(&conn);
            crate::sync_engine::get_state(&conn)
        };
        // A run in progress owns the state machine; do not pile onto it.
        if (state == "INITIALIZING" || state == "BACKFILLING") && self.engine_engaged() {
            return 0;
        }
        let Some(engine) = self.engine.clone() else {
            return 0;
        };
        // 1.2.0: `NEW` used to be a "do nothing" state — a freshly
        // connected account sat empty forever because nothing in the
        // production path ever started the initial sync (only the manual
        // buttons on Onboarding / Sync Health did). `NEW` now means
        // "kick off the initial sync"; the engine's own running guard
        // prevents a second run from stacking behind it.
        let result = if state == "NEW" {
            tracing::info!(state, "first sync: starting the initial pull");
            engine.initial_sync(crate::events::SyncTrigger::Auto).await
        } else {
            engine
                .incremental_sync(crate::events::SyncTrigger::Auto)
                .await
        };
        match result {
            Ok(results) => {
                let processed: i64 = results.iter().map(|r| r.processed).sum();
                tracing::debug!(processed, operation = "auto_sync", "Adaptive tick finished");
                processed
            }
            Err(e) => {
                // "A sync is already running" is the healthy overlap case
                // (a manual button run); it is not a failure worth reporting.
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
                    // R5 (D4): background failures reach the UI live, not
                    // only through the Diagnostics page.
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
            crate::maintenance::compute_trends(&conn)?;
            let _ = crate::maintenance::refresh_products(&conn);
            // Clean-audit item 22, repair half — see
            // [`enqueue_activity_rebuild_repair_once`].
            enqueue_activity_rebuild_repair_once(&conn);
            // A2: issue clustering cadence — once a day at most, feature-
            // gated, provider-gated (the job itself skips honestly when
            // no Classify endpoint is routed).
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
            // K1: the gap-detection pipeline — friction findings over every
            // conversation with a customer thread, then gap candidates from
            // repeated questions cross-checked against the knowledge base
            // AND the help-center mirror. Deterministic (no model needed);
            // provider-gated because its inputs (analysis runs, clusters)
            // come from AI jobs that never ran on an empty provider.
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
            // K4: the weekly digest cadence — once a week at most,
            // feature-gated, provider-gated (the digest summarizes a
            // synced mirror; an empty provider has nothing to summarize).
            if crate::ai_hub::ai_feature_enabled(&conn, "weekly_digest") && provider_kind != "empty"
            {
                let last_run_unix =
                    crate::settings::get_i64(&conn, "ai_digest_last_run", 0).unwrap_or(0);
                let now_unix = chrono::Utc::now().timestamp();
                if now_unix - last_run_unix >= 7 * 24 * 3600 {
                    // Scheduled digests only run when a model is genuinely
                    // configured (an explicit role assignment, or a legacy
                    // base URL): a fresh install must not archive weekly
                    // "model unavailable" rows. The on-demand button stays
                    // ungated - its failure is visible setup truth, once.
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
            // A6: the golden set re-runs when the analysis prompt version
            // or the role map changed since the last run (upgrade → new
            // prompt constants; AI Center → new model routing).
            if crate::ai_hub::ai_feature_enabled(&conn, "golden_eval")
                && provider_kind != "empty"
                && crate::ai_eval::golden_fingerprint_stale(&conn)
            {
                let _ = crate::jobs::enqueue_on(&conn, "ai", "golden_eval", "{}", 3, 2);
            }
            // A3: keep the post-resolution QA table warm for the Reports
            // Quality tab (deterministic; the AI layer stays on demand).
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
            // P4 (D21): the webhook relay guard — Help Scout silently
            // deactivates a hook after ten discarded events (a relay
            // outage does exactly that), so the maintenance tick also
            // enqueues the health check on a ~6-hour cadence. The job
            // itself stamps the clock when it actually runs; the empty
            // provider records not-configured honestly instead of
            // hammering the API without credentials.
            if provider_kind != "empty"
                && crate::webhook_health::cadence_due(&conn).unwrap_or(false)
            {
                let _ = crate::jobs::enqueue_on(&conn, "sync", "webhook_health_check", "{}", 3, 2);
            }
            let _ = crate::notification_sweep::sweep(&conn, Some(&bus));
            // Audit fix (M17): the backup decision is made on the writer
            // guard, but the backup itself (VACUUM INTO — a full DB copy)
            // used to run while still holding the exclusive UI writer lane,
            // stalling every write for the whole copy on large accounts.
            // The guard is dropped first; the backup runs on its own
            // long-op connection.
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
        if self.stopped.load(Ordering::SeqCst) {
            return;
        }
        // A1: latch clears the guard on every exit path (see TickGuard).
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

    pub fn customer_event_sweep_tick(&self) {
        if self.stopped.load(Ordering::SeqCst) {
            return;
        }
        // A1: latch clears the guard on every exit path (see TickGuard).
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

    /// Item 150: the intelligence sweep — persist the live SLA verdicts
    /// (deduped) into `sla_breaches`, track mailboxes the business-hours
    /// system does not monitor against their `sla_configs` thresholds,
    /// and refresh the tracked doc-freshness state. These are the writers
    /// the M018/M019 tables never had; the dashboard breach metric, the
    /// Knowledge gaps list and the stale count read what lands here.
    pub fn intelligence_sweep_tick(&self) {
        if self.stopped.load(Ordering::SeqCst) {
            return;
        }
        // A1: latch clears the guard on every exit path (see TickGuard).
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
        // A1: latch clears the guard on every exit path, including the
        // awaits below (see TickGuard).
        let Some(_guard) = TickGuard::acquire(&self.refreshing_connectors) else {
            return;
        };
        let due: Vec<i64> = {
            let conn = self.lock();
            crate::connectors::due_for_refresh(&conn).unwrap_or_default()
        };
        let data_dir = self.data_dir.clone();
        for id in due {
            match crate::connectors::refresh(&self.db, id, &data_dir).await {
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

/// Clean-audit item 22, repair half: databases that finished their
/// initial sync before this fix never ran `rebuild_activity` — the job
/// existed but nothing ever enqueued it, so the derived response-state
/// columns (first_customer_message_at, customer_waiting_since,
/// response_state, …) are still NULL/schema defaults. One-time repair
/// for any database that already carries conversations; fresh installs
/// are covered by the `on_after_initial_sync` enqueue instead.
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
        // Clean-audit item 87: the conflict path used to touch only
        // rating/comments/last_synced_at — a rating stored before its
        // conversation or customer was mirrored (the satisfaction
        // webhook routinely beats the conversation sync) stayed
        // unlinked FOREVER, because every later re-delivery of the same
        // rating upserted without re-linking. The conflict path now
        // upgrades NULL links from the freshly-resolved ids while never
        // downgrading an existing link to NULL (COALESCE keeps the
        // stored value when the new fetch could not resolve one).
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
mod batch20_audit22_tests {
    use super::*;

    fn temp_db() -> crate::db::DbHandle {
        let dir = std::env::temp_dir().join(format!(
            "spp-workers-b20-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or_default()
        ));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let mut conn = crate::db::open(&dir.join("workers.db")).expect("open db");
        crate::bootstrap::apply_all(&mut conn).expect("migrations");
        crate::db::DbHandle::new(conn)
    }

    fn manager(db: &crate::db::DbHandle) -> Arc<WorkerManager> {
        Arc::new(WorkerManager::new(
            db.clone(),
            None,
            Arc::new(crate::helpscout::EmptyHelpScoutProvider::new()),
            crate::event_bus::EventBus::default(),
            std::env::temp_dir(),
            None,
        ))
    }

    fn rebuild_activity_jobs(db: &crate::db::DbHandle) -> i64 {
        db.with_read(|conn| {
            conn.query_row(
                "SELECT COUNT(*) FROM jobs WHERE type = 'rebuild_activity'",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0)
        })
    }

    /// Audit item 22: `on_after_initial_sync` enqueues the rebuild — the
    /// threads just landed, so the derived response-state columns get
    /// computed from them instead of staying at schema defaults forever.
    #[test]
    fn audit22_rebuild_activity_is_enqueued_after_initial_sync() {
        let db = temp_db();
        let manager = manager(&db);
        manager.on_after_initial_sync();
        assert_eq!(
            rebuild_activity_jobs(&db),
            1,
            "the activity rebuild must ride along with the post-sync jobs"
        );
    }

    /// Audit item 22, repair half: an existing database (conversations
    /// already mirrored) gets a one-time rebuild from the maintenance
    /// sweep; an empty one does not (fresh installs are covered by the
    /// post-initial-sync enqueue); and the marker makes it exactly once.
    #[test]
    fn audit22_repair_runs_once_and_only_for_existing_mirrors() {
        let db = temp_db();

        // Empty database: no repair, no marker consumed.
        db.with_write(|conn| enqueue_activity_rebuild_repair_once(conn));
        assert_eq!(rebuild_activity_jobs(&db), 0, "nothing to rebuild yet");
        assert_eq!(
            db.with_read(|conn| {
                crate::settings::get_i64(conn, "activity_rebuild_repaired", 0).unwrap_or(0)
            }),
            0,
            "the marker must not be consumed by an empty database"
        );

        // Existing mirror: repair enqueues once.
        db.with_write(|conn| {
            conn.execute(
                "INSERT INTO conversations (remote_id, number, subject, preview, status)
                 VALUES (9401, 94, 'Pre-fix mirror', 'preview', 'active')",
                [],
            )
            .expect("conversation")
        });
        db.with_write(|conn| enqueue_activity_rebuild_repair_once(conn));
        assert_eq!(rebuild_activity_jobs(&db), 1);

        // Marker holds: a second sweep never re-enqueues.
        db.with_write(|conn| enqueue_activity_rebuild_repair_once(conn));
        assert_eq!(
            rebuild_activity_jobs(&db),
            1,
            "the repair is strictly one-time"
        );
    }
}

#[cfg(test)]
mod auto_sync_tests {
    use super::*;

    fn temp_db() -> crate::db::DbHandle {
        let dir = std::env::temp_dir().join(format!(
            "spp-workers-autosync-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or_default()
        ));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let mut conn = crate::db::open(&dir.join("workers.db")).expect("open db");
        crate::bootstrap::apply_all(&mut conn).expect("migrations");
        crate::db::DbHandle::new(conn)
    }

    /// R2 (D1) regression: the auto-sync timer must refuse to run against
    /// the empty provider. Before the guard, a fresh install without
    /// credentials "completed" a zero-record initial sync every minute —
    /// stamping checkpoints forward and flipping the state to LIVE — so
    /// real credentials that arrived later never triggered the full
    /// history pull.
    #[test]
    fn auto_sync_skips_the_empty_provider_and_stamps_nothing() {
        let db = temp_db();
        // A poisoned pre-1.4.0 state: pretend a previous empty session
        // already stamped LIVE. The timer must leave it exactly as it is
        // and write no run rows.
        db.with_write(|conn| crate::sync_engine::set_state(conn, "LIVE"));
        let engine = Arc::new(crate::sync_engine::SyncEngine::new(
            db.clone(),
            Arc::new(crate::helpscout::EmptyHelpScoutProvider::new()),
        ));
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        // Enter before start_workers: start() spawns its timer tasks with
        // tokio::spawn, which requires an ambient runtime on this thread.
        // The current-thread tasks only advance while a block_on drives
        // them, so the test below keeps full control of what runs.
        let _enter = rt.enter();
        let manager = start_workers(
            db.clone(),
            Some(engine),
            Arc::new(crate::helpscout::EmptyHelpScoutProvider::new()),
            crate::event_bus::EventBus::default(),
            std::env::temp_dir(),
            None,
        );
        rt.block_on(manager.auto_sync());

        let (state, runs, checkpoints) = db.with_read(|conn| {
            (
                crate::sync_engine::get_state(conn),
                conn.query_row("SELECT COUNT(*) FROM sync_runs", [], |r| r.get::<_, i64>(0))
                    .unwrap_or(0),
                conn.query_row(
                    "SELECT COUNT(*) FROM sync_checkpoints WHERE status = 'ok'",
                    [],
                    |r| r.get::<_, i64>(0),
                )
                .unwrap_or(0),
            )
        });
        assert_eq!(state, "LIVE", "the timer must not touch the state");
        assert_eq!(runs, 0, "the timer must not start a run");
        assert_eq!(checkpoints, 0, "the timer must not stamp watermarks");
        manager.stop();
    }
}

#[cfg(test)]
mod batch14_tests {
    use super::*;
    use crate::helpscout::HelpScoutProvider;
    use crate::mockserver::{MockRequest, MockResponse};

    fn secrets_guard() -> std::sync::MutexGuard<'static, ()> {
        crate::settings::SECRETS_TEST_LOCK
            .lock()
            .unwrap_or_else(|p| p.into_inner())
    }

    fn temp_db() -> crate::db::DbHandle {
        let dir = std::env::temp_dir().join(format!(
            "spp-workers-b14-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or_default()
        ));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let mut conn = crate::db::open(&dir.join("workers.db")).expect("open db");
        crate::bootstrap::apply_all(&mut conn).expect("migrations");
        crate::db::DbHandle::new(conn)
    }

    fn creds_for(
        mock: &crate::mockserver::MockServer,
        docs_key: &str,
    ) -> crate::helpscout_real::HsCredentials {
        crate::helpscout_real::HsCredentials {
            client_id: "b14-client".into(),
            client_secret: "b14-secret".into(),
            redirect_uri: format!("{}/oauth/callback", mock.url()),
            api_base: mock.url(),
            webhook_secret: "b14-hook".into(),
            docs_api_key: docs_key.into(),
            docs_api_base: mock.url(),
        }
    }

    fn manager_with(
        db: &crate::db::DbHandle,
        provider: Arc<dyn HelpScoutProvider>,
    ) -> Arc<WorkerManager> {
        Arc::new(WorkerManager::new(
            db.clone(),
            None,
            provider,
            crate::event_bus::EventBus::default(),
            std::env::temp_dir(),
            None,
        ))
    }

    /// Route the Docs API publish chain: create (POST /v1/articles),
    /// draft upsert (PUT /v1/articles/{id}/drafts), publish
    /// (PUT /v1/articles/{id}), verify (GET /v1/articles/{id}).
    fn docs_router() -> crate::mockserver::Handler {
        std::sync::Arc::new(|req: &MockRequest| -> MockResponse {
            match (req.method.as_str(), req.path.as_str()) {
                ("POST", "/v1/articles") => MockResponse::json(
                    200,
                    serde_json::json!({
                        "article": {
                            "id": "abc909abc909abc909abc909", "status": "notpublished",
                            "name": "Fix the export timeout",
                            "collectionId": 55
                        }
                    }),
                ),
                ("PUT", p)
                    if p == "/v1/articles/abc909abc909abc909abc909"
                        || p == "/v1/articles/abc909abc909abc909abc909/drafts" =>
                {
                    MockResponse::json(
                        200,
                        serde_json::json!({"article": {"id": "abc909abc909abc909abc909", "status": "published"}}),
                    )
                }
                ("GET", "/v1/articles/abc909abc909abc909abc909") => MockResponse::json(
                    200,
                    serde_json::json!({
                        "article": {
                            "id": "abc909abc909abc909abc909", "status": "published",
                            "name": "Fix the export timeout",
                            "text": "Clear the export cache and re-run.",
                            "collectionId": 55,
                            "slug": "fix-export-timeout",
                            "number": 4,
                            "viewCount": 0,
                            "createdDate": "2026-10-21T00:00:00Z",
                            "modifiedDate": "2026-10-21T00:00:00Z"
                        }
                    }),
                ),
                _ => crate::mockserver::not_found()(req),
            }
        })
    }

    /// Seed an approved review-queue draft plus its target collection;
    /// returns (draft_id, collection_local_id).
    fn seed_approved_draft(db: &crate::db::DbHandle) -> (i64, i64) {
        db.with_write(|conn| {
            conn.execute(
                "INSERT INTO docs_collections (remote_id, name) VALUES ('col55abc55abc55abc55abc', 'Troubleshooting')",
                [],
            )
            .expect("collection");
            let collection_local = conn.last_insert_rowid();
            let draft_id = crate::knowledge_store::insert_article_draft(
                conn,
                None,
                "Fix the export timeout",
                "Clear the export cache and re-run.",
                &serde_json::json!([]),
                None,
            )
            .expect("draft");
            crate::knowledge_store::decide_article_draft(conn, draft_id, "approved", None)
                .expect("approve");
            (draft_id, collection_local)
        })
    }

    /// K3: the full publish chain over the real provider + mock Docs API —
    /// upsert (create), publish, verify re-sync — ending `verified` with
    /// the article in the local mirror.
    #[test]
    fn publish_job_runs_the_full_chain_and_verifies() {
        let _secrets = secrets_guard();
        crate::settings::set_master_key_for_tests([11u8; 32]);
        let db = temp_db();
        let mock = crate::mockserver::MockServer::start(docs_router()).expect("mock");
        let provider = crate::helpscout_real::RealHelpScoutProvider::new(
            db.clone(),
            creds_for(&mock, "docs-key-1"),
        );
        provider
            .save_tokens("b14-token", None, Some(3_600), None)
            .expect("seed token");
        let (draft_id, collection_local) = seed_approved_draft(&db);
        db.with_write(|conn| {
            jobs::enqueue(
                conn,
                "publish_article_draft",
                &serde_json::json!({"draftId": draft_id, "collectionLocalId": collection_local})
                    .to_string(),
            )
            .expect("enqueue");
        });

        let manager = manager_with(&db, Arc::new(provider));
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        rt.block_on(async { manager.tick().await });

        // Audit fix (C2, live): the remote article id is a hex ObjectId string.
        let (state, remote, mirrored): (String, Option<String>, Option<String>) =
            db.with_read(|conn| {
                conn.query_row(
                    "SELECT publish_state, remote_article_id,
                        (SELECT text FROM docs_articles WHERE remote_id = 'abc909abc909abc909abc909')
                 FROM article_drafts WHERE id = ?1",
                    rusqlite::params![draft_id],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
                )
                .unwrap_or(("missing".into(), None, None))
            });
        assert_eq!(state, "verified", "the chain must reach verified");
        assert_eq!(remote.as_deref(), Some("abc909abc909abc909abc909"));
        assert!(
            mirrored
                .as_deref()
                .unwrap_or_default()
                .contains("export cache"),
            "the verify re-sync mirrored the published text"
        );

        // The wire saw the create->publish->verify chain with Basic auth.
        let hits = mock.hits();
        let saw = |m: &str, p: &str| {
            hits.iter().any(|h| {
                h.method == m
                    && h.path == p
                    && h.header("authorization")
                        .is_some_and(|a| a.starts_with("Basic "))
            })
        };
        assert!(saw("POST", "/v1/articles"), "create must run first");
        assert!(
            saw("PUT", "/v1/articles/abc909abc909abc909abc909"),
            "publish"
        );
        assert!(
            saw("GET", "/v1/articles/abc909abc909abc909abc909"),
            "verify re-sync"
        );

        // Re-publishing the now-linked draft takes the update path:
        // PUT /v1/articles/{id}/drafts upserts the draft copy of the
        // existing article (the K3 one-click upsert), then publishes.
        db.with_write(|conn| {
            jobs::enqueue(
                conn,
                "publish_article_draft",
                &serde_json::json!({"draftId": draft_id, "collectionLocalId": collection_local})
                    .to_string(),
            )
            .expect("re-enqueue");
        });
        rt.block_on(async { manager.tick().await });
        let hits = mock.hits();
        let saw = |m: &str, p: &str| {
            hits.iter().any(|h| {
                h.method == m
                    && h.path == p
                    && h.header("authorization")
                        .is_some_and(|a| a.starts_with("Basic "))
            })
        };
        assert!(
            saw("PUT", "/v1/articles/abc909abc909abc909abc909/drafts"),
            "the linked-article update upserts the draft copy"
        );
        let state: String = db.with_read(|conn| {
            conn.query_row(
                "SELECT publish_state FROM article_drafts WHERE id = ?1",
                rusqlite::params![draft_id],
                |r| r.get(0),
            )
            .unwrap_or("missing".into())
        });
        assert_eq!(state, "verified", "the update run re-verifies");
    }

    /// K3: the review queue is the only path — an unapproved draft refuses
    /// with the row untouched and nothing sent to the Docs API.
    #[test]
    fn publish_job_refuses_an_unapproved_draft() {
        let _secrets = secrets_guard();
        crate::settings::set_master_key_for_tests([12u8; 32]);
        let db = temp_db();
        let mock = crate::mockserver::MockServer::start(docs_router()).expect("mock");
        let provider = crate::helpscout_real::RealHelpScoutProvider::new(
            db.clone(),
            creds_for(&mock, "docs-key-2"),
        );
        let draft_id = db.with_write(|conn| {
            crate::knowledge_store::insert_article_draft(
                conn,
                None,
                "Draft still in review",
                "body",
                &serde_json::json!([]),
                None,
            )
            .expect("draft")
        });
        db.with_write(|conn| {
            jobs::enqueue(
                conn,
                "publish_article_draft",
                &serde_json::json!({"draftId": draft_id, "collectionLocalId": 1}).to_string(),
            )
            .expect("enqueue");
        });
        let manager = manager_with(&db, Arc::new(provider));
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        rt.block_on(async { manager.tick().await });

        let (state, status): (String, String) = db.with_read(|conn| {
            conn.query_row(
                "SELECT publish_state, status FROM article_drafts WHERE id = ?1",
                rusqlite::params![draft_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap_or(("missing".into(), "missing".into()))
        });
        assert_eq!(state, "unpublished");
        assert_eq!(status, "drafted");
        assert!(mock.hits().is_empty(), "nothing reached the Docs API");
        let failed: i64 = db.with_read(|conn| {
            conn.query_row(
                "SELECT COUNT(*) FROM jobs WHERE type = 'publish_article_draft' AND status = 'failed'",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0)
        });
        assert_eq!(failed, 1, "the refusal fails the job honestly");
    }

    /// K5: the fetch job caches a 200 report and marks the capability
    /// available; a 403 answer sets the plan-blocked verdict.
    #[test]
    fn fetch_hs_report_job_classifies_capability() {
        let _secrets = secrets_guard();
        crate::settings::set_master_key_for_tests([13u8; 32]);
        let db = temp_db();
        let mock = crate::mockserver::MockServer::start(std::sync::Arc::new(
            |req: &MockRequest| -> MockResponse {
                match req.path.as_str() {
                    "/v2/reports/happiness" => MockResponse::json(
                        200,
                        serde_json::json!({
                            "current": {"great": 20, "okay": 3, "notGood": 1},
                            "previous": {"great": 12, "okay": 4, "notGood": 2}
                        }),
                    ),
                    "/v2/reports/company" => {
                        MockResponse::json(403, serde_json::json!({"message": "forbidden"}))
                    }
                    _ => crate::mockserver::not_found()(req),
                }
            },
        ))
        .expect("mock");
        let provider =
            crate::helpscout_real::RealHelpScoutProvider::new(db.clone(), creds_for(&mock, ""));
        provider
            .save_tokens("b14-token", None, Some(3_600), None)
            .expect("seed token");
        let manager = manager_with(&db, Arc::new(provider));
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");

        db.with_write(|conn| {
            jobs::enqueue(
                conn,
                "fetch_hs_report",
                &serde_json::json!({
                    "key": "happiness",
                    "start": "2026-10-14T00:00:00Z",
                    "end": "2026-10-21T00:00:00Z"
                })
                .to_string(),
            )
            .expect("enqueue happiness");
        });
        rt.block_on(async { manager.tick().await });
        assert_eq!(
            db.with_read(crate::reports_api::get_capability),
            crate::reports_api::ReportsCapability::Available
        );
        let (_, _, _, data) = db
            .with_read(|conn| crate::reports_api::cached_report(conn, "happiness"))
            .expect("cached");
        assert_eq!(data["current"]["great"], 20);

        db.with_write(|conn| {
            jobs::enqueue(
                conn,
                "fetch_hs_report",
                &serde_json::json!({
                    "key": "company",
                    "start": "2026-10-14T00:00:00Z",
                    "end": "2026-10-21T00:00:00Z"
                })
                .to_string(),
            )
            .expect("enqueue company");
        });
        rt.block_on(async { manager.tick().await });
        assert_eq!(
            db.with_read(crate::reports_api::get_capability),
            crate::reports_api::ReportsCapability::PlanBlocked,
            "the 403 wall must set the honest plan-blocked verdict"
        );
    }

    /// K5: with no Help Scout connection at all the fetch job records the
    /// not-connected verdict — not a silent empty chart.
    #[test]
    fn fetch_hs_report_job_records_not_connected() {
        let db = temp_db();
        let manager = manager_with(
            &db,
            Arc::new(crate::helpscout::EmptyHelpScoutProvider::new()),
        );
        db.with_write(|conn| {
            jobs::enqueue(
                conn,
                "fetch_hs_report",
                &serde_json::json!({"key": "happiness", "start": "s", "end": "e"}).to_string(),
            )
            .expect("enqueue");
        });
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        rt.block_on(async { manager.tick().await });
        assert_eq!(
            db.with_read(crate::reports_api::get_capability),
            crate::reports_api::ReportsCapability::NotConnected
        );
    }

    /// K2: the draft job skips honestly when no Draft role backend is
    /// routed — no queue row, no fabricated article.
    #[test]
    fn draft_article_job_skips_without_a_routed_backend() {
        let db = temp_db();
        let manager = manager_with(
            &db,
            Arc::new(crate::helpscout::EmptyHelpScoutProvider::new()),
        );
        db.with_write(|conn| {
            jobs::enqueue_on(conn, "ai", "draft_article", r#"{"candidateId": 1}"#, 3, 2)
                .expect("enqueue");
        });
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        rt.block_on(async { manager.tick().await });
        let drafts: i64 = db.with_read(|conn| {
            conn.query_row("SELECT COUNT(*) FROM article_drafts", [], |r| r.get(0))
                .unwrap_or(0)
        });
        assert_eq!(drafts, 0);
    }

    /// K4: the digest job runs end to end against a mock model server —
    /// facts collected, narrative written, digest archived complete.
    #[test]
    fn weekly_digest_job_writes_the_narrative_end_to_end() {
        let _secrets = secrets_guard();
        crate::settings::set_master_key_for_tests([15u8; 32]);
        let db = temp_db();
        let mock = crate::mockserver::MockServer::start(std::sync::Arc::new(
            |req: &MockRequest| -> MockResponse {
                if req.method == "POST" && req.path == "/v1/chat/completions" {
                    return MockResponse::json(
                        200,
                        serde_json::json!({
                            "model": "test-model",
                            "choices": [{
                                "message": {
                                    "role": "assistant",
                                    "content": "{\"narrative\": \"Volume held steady; the export-timeout cluster dominated; one volume spike stands out and the backlog grew.\"}"
                                },
                                "finish_reason": "stop"
                            }],
                            "usage": {"prompt_tokens": 100, "completion_tokens": 40, "total_tokens": 140}
                        }),
                    );
                }
                crate::mockserver::not_found()(req)
            },
        ))
        .expect("mock");
        // Route the Summarize role at the mock so the job uses a real,
        // local backend instead of the unrouted legacy fallback.
        db.with_write(|conn| {
            crate::settings::set_string(conn, "lmstudio_base_url", &mock.url()).expect("base url");
        });
        let manager = manager_with(
            &db,
            Arc::new(crate::helpscout::EmptyHelpScoutProvider::new()),
        );
        db.with_write(|conn| {
            jobs::enqueue_on(
                conn,
                "ai",
                "weekly_digest",
                r#"{"triggerKind":"manual","days":7}"#,
                3,
                2,
            )
            .expect("enqueue");
        });
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        rt.block_on(async { manager.tick().await });
        let rows = db.with_read(|conn| crate::digest::list_digests(conn, None, 5));
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].status, "complete", "facts + AI narrative archived");
        assert_eq!(rows[0].trigger_kind, "manual");
        assert!(
            rows[0]
                .narrative
                .as_deref()
                .unwrap_or_default()
                .contains("export-timeout"),
            "the narrative came from the model"
        );
        let runs: i64 = db.with_read(|conn| {
            conn.query_row(
                "SELECT COUNT(*) FROM ai_runs WHERE type = 'weekly_digest' AND status = 'completed'",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0)
        });
        assert_eq!(runs, 1, "the model call is in the ledger");
    }

    /// P1: the satisfaction-webhook job fetches the rating by id through
    /// the real provider and lands it in the local mirror — the
    /// relay-ready ingestion path (a relay forwards the webhook; the
    /// rating itself is fetched from Help Scout by id).
    #[test]
    fn satisfaction_ratings_job_fetches_and_stores_the_rating() {
        let _secrets = secrets_guard();
        crate::settings::set_master_key_for_tests([14u8; 32]);
        let db = temp_db();
        let mock = crate::mockserver::MockServer::start(std::sync::Arc::new(
            |req: &MockRequest| -> MockResponse {
                if req.method == "GET" && req.path == "/v2/ratings/501" {
                    MockResponse::json(
                        200,
                        serde_json::json!({
                            "id": 501,
                            "conversationId": 900,
                            "rating": "great",
                            "comments": "Fast, clear help",
                            "customer": {"id": 77, "firstName": "Rina", "lastName": "Chowdhury"},
                            "createdAt": "2026-10-21T01:02:03Z"
                        }),
                    )
                } else {
                    crate::mockserver::not_found()(req)
                }
            },
        ))
        .expect("mock");
        db.with_write(|conn| {
            conn.execute(
                "INSERT INTO conversations (remote_id, number, subject, preview, status, remote_created_at)
                 VALUES (900, 900, 'Rated conversation', 'p', 'closed', '2026-10-20T08:00:00.000Z')",
                [],
            )
            .expect("conversation");
        });
        let provider =
            crate::helpscout_real::RealHelpScoutProvider::new(db.clone(), creds_for(&mock, ""));
        provider
            .save_tokens("b14-token", None, Some(3_600), None)
            .expect("seed token");
        let manager = manager_with(&db, Arc::new(provider));
        db.with_write(|conn| {
            jobs::enqueue(
                conn,
                "sync_conversation_ratings",
                &serde_json::json!({"ratingId": 501}).to_string(),
            )
            .expect("enqueue");
        });
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        rt.block_on(async { manager.tick().await });

        let (rating, comment, conv_number): (String, Option<String>, Option<i64>) =
            db.with_read(|conn| {
                conn.query_row(
                    "SELECT r.rating, r.comments, c.number
                       FROM ratings r
                       LEFT JOIN conversations c ON c.id = r.conversation_id
                      WHERE r.remote_id = 501",
                    [],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
                )
                .unwrap_or(("missing".into(), None, None))
            });
        assert_eq!(rating, "great");
        assert_eq!(comment.as_deref(), Some("Fast, clear help"));
        assert_eq!(conv_number, Some(900));
        assert!(
            mock.hits().iter().any(|h| h.path == "/v2/ratings/501"),
            "the rating is fetched by id from Help Scout"
        );
    }
}

/// Clean-audit batch 27 (items 84/85/86): the jobs::fail calls carrying
/// the real job id instead of literal 0, the rating fetch errors
/// surfaced with a requeue instead of being swallowed, and the panic
/// containment failing terminally instead of re-running side effects.
#[cfg(test)]
mod batch27_workers_tests {
    use super::*;
    use crate::helpscout::HelpScoutProvider;
    use crate::mockserver::{MockRequest, MockResponse};

    fn secrets_guard() -> std::sync::MutexGuard<'static, ()> {
        crate::settings::SECRETS_TEST_LOCK
            .lock()
            .unwrap_or_else(|p| p.into_inner())
    }

    fn temp_db() -> crate::db::DbHandle {
        let dir = std::env::temp_dir().join(format!(
            "spp-workers-b27-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or_default()
        ));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let mut conn = crate::db::open(&dir.join("workers.db")).expect("open db");
        crate::bootstrap::apply_all(&mut conn).expect("migrations");
        crate::db::DbHandle::new(conn)
    }

    fn manager_with(
        db: &crate::db::DbHandle,
        provider: Arc<dyn HelpScoutProvider>,
    ) -> Arc<WorkerManager> {
        Arc::new(WorkerManager::new(
            db.clone(),
            None,
            provider,
            crate::event_bus::EventBus::default(),
            std::env::temp_dir(),
            None,
        ))
    }

    fn job_state(db: &crate::db::DbHandle, id: i64) -> (String, Option<String>, i64) {
        db.with_read(|conn| {
            conn.query_row(
                "SELECT status, error, attempt FROM jobs WHERE id = ?1",
                rusqlite::params![id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .expect("job row")
        })
    }

    fn run_one_tick(manager: &Arc<WorkerManager>) {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        rt.block_on(async { manager.tick().await });
    }

    /// Item 84: `draft_article` without a candidateId used to call
    /// `jobs::fail(&conn, 0, …)` — the failure updated nothing and the
    /// real job row sat in 'running' forever with its honest error
    /// written nowhere.
    #[test]
    fn draft_article_missing_candidate_fails_the_real_job_row() {
        let db = temp_db();
        let manager = manager_with(
            &db,
            Arc::new(crate::helpscout::EmptyHelpScoutProvider::new()),
        );
        let id = db.with_write(|conn| jobs::enqueue(conn, "draft_article", "{}").expect("enqueue"));
        run_one_tick(&manager);

        let (status, error, _) = job_state(&db, id);
        assert_eq!(status, "failed", "the real job row takes the failure");
        assert!(
            error
                .as_deref()
                .unwrap_or_default()
                .contains("no candidateId"),
            "the honest error is on the row: {error:?}"
        );
    }

    /// Item 84: same defect on the `fetch_hs_report` path.
    #[test]
    fn fetch_hs_report_unknown_key_fails_the_real_job_row() {
        let db = temp_db();
        let manager = manager_with(
            &db,
            Arc::new(crate::helpscout::EmptyHelpScoutProvider::new()),
        );
        let id = db.with_write(|conn| {
            jobs::enqueue(
                conn,
                "fetch_hs_report",
                &serde_json::json!({"key": "not_a_real_key"}).to_string(),
            )
            .expect("enqueue")
        });
        run_one_tick(&manager);

        let (status, error, _) = job_state(&db, id);
        assert_eq!(status, "failed");
        assert!(
            error
                .as_deref()
                .unwrap_or_default()
                .contains("unknown report key"),
            "the honest error is on the row: {error:?}"
        );
    }

    /// Item 85: a failed rating fetch used to be swallowed (`let
    /// Ok(Some(r))` matched nothing) — the rating was lost forever while
    /// the job reported success. The error now requeues the job (the
    /// queue's retry machinery re-fetches it).
    #[test]
    fn rating_fetch_failure_requeues_the_job_instead_of_swallowing() {
        let _secrets = secrets_guard();
        crate::settings::set_master_key_for_tests([27u8; 32]);
        let db = temp_db();
        // The platform answers 500 for the rating — a transient fetch
        // failure, exactly the case the pre-fix code swallowed.
        let mock = crate::mockserver::MockServer::start(std::sync::Arc::new(
            |req: &MockRequest| -> MockResponse {
                if req.method == "GET" && req.path == "/v2/ratings/501" {
                    MockResponse::json(500, serde_json::json!({"error": "boom"}))
                } else {
                    crate::mockserver::not_found()(req)
                }
            },
        ))
        .expect("mock");
        let provider = crate::helpscout_real::RealHelpScoutProvider::new(
            db.clone(),
            crate::helpscout_real::HsCredentials {
                client_id: "b27-client".into(),
                client_secret: "b27-secret".into(),
                redirect_uri: format!("{}/oauth/callback", mock.url()),
                api_base: mock.url(),
                webhook_secret: "b27-hook".into(),
                docs_api_key: String::new(),
                docs_api_base: mock.url(),
            },
        );
        provider
            .save_tokens("b27-token", None, Some(3_600), None)
            .expect("seed token");
        let manager = manager_with(&db, Arc::new(provider));
        let id = db.with_write(|conn| {
            jobs::enqueue(
                conn,
                "sync_conversation_ratings",
                &serde_json::json!({"ratingId": 501}).to_string(),
            )
            .expect("enqueue")
        });
        run_one_tick(&manager);

        let (status, error, attempt) = job_state(&db, id);
        assert_eq!(
            status, "queued",
            "the fetch failure requeues the job for a retry"
        );
        assert_eq!(attempt, 1, "one attempt spent");
        assert!(
            error.is_some(),
            "the failure is recorded on the row, not swallowed: {error:?}"
        );
        let ratings: i64 = db.with_read(|conn| {
            conn.query_row("SELECT COUNT(*) FROM ratings", [], |r| r.get(0))
                .unwrap_or(0)
        });
        assert_eq!(ratings, 0, "nothing stored yet — the retry will fetch it");
    }

    /// Item 86: the panic containment used to mark the job retryable —
    /// a deterministic panic re-ran once per attempt, re-executing
    /// early side effects (a panicking copilot chat re-persisted the
    /// user's message every retry). The contained job now fails
    /// terminally.
    #[test]
    fn panicking_job_fails_terminally_instead_of_requeueing() {
        let db = temp_db();
        let manager = manager_with(
            &db,
            Arc::new(crate::helpscout::EmptyHelpScoutProvider::new()),
        );
        let id = db.with_write(|conn| jobs::enqueue(conn, "__test_panic", "{}").expect("enqueue"));
        run_one_tick(&manager);

        let (status, error, attempt) = job_state(&db, id);
        assert_eq!(
            status, "failed",
            "a contained panic is terminal, not retryable"
        );
        assert_eq!(attempt, 1, "exactly one attempt — no re-runs");
        assert!(
            error.as_deref().unwrap_or_default().contains("panicked"),
            "the honest panic error sits on the row: {error:?}"
        );

        // A later tick must not resurrect it.
        run_one_tick(&manager);
        let (status, _, attempt) = job_state(&db, id);
        assert_eq!(status, "failed", "the failed job stays failed");
        assert_eq!(attempt, 1, "no second execution of the panicking job");
    }
}

/// Clean-audit batch 28 (items 87/90): the ratings upsert re-linking its
/// conversation/customer on conflict, and the post-initial-sync AI
/// backfill covering every conversation instead of the first 50.
#[cfg(test)]
mod batch28_workers_tests {
    use super::*;

    fn temp_db() -> crate::db::DbHandle {
        let dir = std::env::temp_dir().join(format!(
            "spp-workers-b28-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or_default()
        ));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let mut conn = crate::db::open(&dir.join("workers.db")).expect("open db");
        crate::bootstrap::apply_all(&mut conn).expect("migrations");
        crate::db::DbHandle::new(conn)
    }

    fn manager(db: &crate::db::DbHandle) -> Arc<WorkerManager> {
        Arc::new(WorkerManager::new(
            db.clone(),
            None,
            Arc::new(crate::helpscout::EmptyHelpScoutProvider::new()),
            crate::event_bus::EventBus::default(),
            std::env::temp_dir(),
            None,
        ))
    }

    fn rating(remote_id: i64) -> crate::helpscout::HsRating {
        crate::helpscout::HsRating {
            remote_id,
            conversation_id: None,
            thread_id: None,
            rating: Some("great".into()),
            comment: Some("Fast, clear help".into()),
            customer_id: None,
            customer_name: Some("Rina Chowdhury".into()),
            user_id: None,
            created_at: Some("2026-10-22T01:02:03Z".into()),
        }
    }

    fn rating_links(
        db: &crate::db::DbHandle,
        remote_id: i64,
    ) -> (Option<i64>, Option<i64>, Option<i64>) {
        db.with_read(|conn| {
            conn.query_row(
                "SELECT conversation_id, customer_local_id, user_local_id
                   FROM ratings WHERE remote_id = ?1",
                rusqlite::params![remote_id],
                |r| {
                    Ok((
                        r.get::<_, Option<i64>>(0)?,
                        r.get::<_, Option<i64>>(1)?,
                        r.get::<_, Option<i64>>(2)?,
                    ))
                },
            )
            .expect("rating row")
        })
    }

    fn seed_conversations(conn: &Connection, n: i64) {
        for i in 1..=n {
            conn.execute(
                "INSERT INTO conversations (remote_id, number, subject, preview, status, remote_created_at)
                 VALUES (?1, ?1, 'Backfill subject', 'p', 'closed', '2026-10-20T08:00:00.000Z')",
                rusqlite::params![9000 + i],
            )
            .expect("conversation");
        }
    }

    fn jobs_of_kind(db: &crate::db::DbHandle, kind: &str) -> i64 {
        db.with_read(|conn| {
            conn.query_row(
                "SELECT COUNT(*) FROM jobs WHERE type = ?1 AND status = 'queued'",
                rusqlite::params![kind],
                |r| r.get(0),
            )
            .unwrap_or(0)
        })
    }

    /// Item 87: the satisfaction webhook routinely beats the conversation
    /// sync — the rating lands with NULL links. A later re-delivery of
    /// the same rating (platform retries, re-enqueue after a fix) used to
    /// upsert WITHOUT re-linking, so the rating stayed orphaned forever.
    /// The conflict path now upgrades NULL links from the fresh fetch.
    #[test]
    fn audit87_rating_upsert_relinks_conversation_customer_on_conflict() {
        let db = temp_db();
        // The rating arrives before its conversation exists locally.
        db.with_write(|conn| {
            let inserted = upsert_rating_row(conn, &rating(501), None, None, None).expect("insert");
            assert!(inserted, "the first upsert is a fresh insert");
        });
        assert_eq!(
            rating_links(&db, 501),
            (None, None, None),
            "stored unlinked"
        );

        // The conversation and customer are mirrored afterwards; the same
        // rating is re-delivered with resolvable ids.
        db.with_write(|conn| {
            conn.execute(
                "INSERT INTO conversations (remote_id, number, subject, preview, status, remote_created_at)
                 VALUES (900, 900, 'Rated conversation', 'p', 'closed', '2026-10-20T08:00:00.000Z')",
                [],
            )
            .expect("conversation");
            conn.execute(
                "INSERT INTO customers (remote_id, first_name, last_name) VALUES (77, 'Rina', 'Chowdhury')",
                [],
            )
            .expect("customer");
        });
        let (conv_local, customer_local): (i64, i64) = db.with_read(|conn| {
            (
                conn.query_row(
                    "SELECT id FROM conversations WHERE remote_id = 900",
                    [],
                    |r| r.get(0),
                )
                .expect("conv local"),
                conn.query_row("SELECT id FROM customers WHERE remote_id = 77", [], |r| {
                    r.get(0)
                })
                .expect("customer local"),
            )
        });
        db.with_write(|conn| {
            let inserted = upsert_rating_row(
                conn,
                &rating(501),
                Some(conv_local),
                Some(customer_local),
                None,
            )
            .expect("conflict upsert");
            assert!(!inserted, "the second upsert is a conflict, not an insert");
        });
        assert_eq!(
            rating_links(&db, 501),
            (Some(conv_local), Some(customer_local), None),
            "the conflict path re-links the freshly-resolved ids"
        );

        // A later fetch that cannot resolve the links anymore (say the
        // conversation mirror was reset) must not DOWNGRADE the stored
        // links to NULL — COALESCE keeps them.
        db.with_write(|conn| {
            upsert_rating_row(conn, &rating(501), None, None, None).expect("null upsert");
        });
        assert_eq!(
            rating_links(&db, 501),
            (Some(conv_local), Some(customer_local), None),
            "an unresolved re-fetch never clears an existing link"
        );
    }

    /// Item 90: the post-initial-sync AI backfill capped both queries at
    /// LIMIT 50 — conversation 51+ of any sizeable import never gained
    /// an analysis, because the hook runs once and nothing re-selects.
    #[test]
    fn audit90_post_sync_ai_backfill_covers_every_conversation() {
        let db = temp_db();
        // 60 conversations, none analyzed, none with attributes.
        db.with_write(|conn| seed_conversations(conn, 60));
        let manager = manager(&db);
        manager.on_after_initial_sync();

        assert_eq!(
            jobs_of_kind(&db, "analyze_ticket"),
            60,
            "every unanalyzed conversation is enqueued — the 50 cap is gone"
        );
        assert_eq!(
            jobs_of_kind(&db, "extract_attributes"),
            60,
            "the attributes backfill covers every conversation too"
        );
    }

    /// Item 90, deletion filter: soft-deleted conversations are not
    /// backfill work.
    #[test]
    fn audit90_post_sync_ai_backfill_skips_deleted_conversations() {
        let db = temp_db();
        db.with_write(|conn| {
            seed_conversations(conn, 3);
            conn.execute(
                "UPDATE conversations SET deleted_at = datetime('now') WHERE remote_id = 9002",
                [],
            )
            .expect("soft delete");
        });
        let manager = manager(&db);
        manager.on_after_initial_sync();
        assert_eq!(jobs_of_kind(&db, "analyze_ticket"), 2, "only the live two");
    }
}
