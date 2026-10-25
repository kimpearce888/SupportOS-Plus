# ARCHITECTURE.md — SupportOS-Plus (incremental, updated as discovery proceeds)

Status: DEEP-DIVE COMPLETE (2026-10-25) — initial map + workers/catalog/AI deep-dive from source inspection of main @ 3affe7e, cross-checked against runtime observations (E-001..E-157).

## 1. What the program is (evidence-based)

A **native desktop support-desk application** ("SupportOS+") built as a Rust workspace with an
**egui/eframe GUI** (`crates/app-shell`), a large **core domain library** (`crates/core`, SQLite-backed),
a **declarative catalog** of domain objects (`crates/catalog`), and an **xtask automation crate**
(`crates/xtask`). Version 1.7.0, edition 2024, Rust pinned at 1.99.1 (`rust-toolchain.toml`).
Local-first: embedded SQLite DB at `~/.local/share/supportos-plus/supportos-plus.db`; 52 migrations
applied on first run (observed at runtime). Syncs with **Help Scout** (mailboxes/conversations), offers
**AI features** (local LM Studio / OpenAI-compatible providers, embeddings, Qdrant vector store),
plus automation, outreach, reports, incidents, knowledge base, and support-graph surfaces.

## 2. Workspace layout (LOC = `wc -l` over crate .rs files, 2026-10-24)

| Crate | Files | LOC | Role (verified from source) |
|---|---|---|---|
| `crates/core` | 109 | 100,555 | Domain layer: db.rs/migrations/bootstrap (SQLite), workers (tokio timers), connectors (Help Scout), ai_* modules, sla, reports, knowledge, outreach, incidents, automation, eventing (EventBus broadcast), webhook_handler/loopback, config (env), crypto (secret envelope) |
| `crates/app-shell` | 48 | 32,983 | eframe GUI: `main.rs` (entry), `lib.rs` (shell bootstrap), `pages/` (23 page modules), `bridge.rs` (EventBus→repaint thread), `design.rs` (zinc design system), `icons.rs` (42 painted glyphs), `overlays.rs`, `toasts.rs`, `palette.rs`, `theme.rs` (light/dark), `single_instance.rs` (flock guard), `workers_host.rs`, `smoke.rs` (smoke-test hooks), `chart.rs`, `mention.rs` |
| `crates/catalog` | 10 | 1,329 | Declarative object/field catalog (drives Custom Objects + Reports builder) |
| `crates/xtask` | 5 | 1,560 | Task runner (icons verify, smoke, metadata checks — commit messages reference `cargo xtask` gates) |

## 3. Entry points and lifecycle (main.rs, verified reading)

1. `supportos_plus::main` → data dir `~/.local/share/supportos-plus` → `spp_core` DB open + migrations (52 observed).
2. flock single-instance guard (`single_instance.rs`, `instance.lock`) — replaces Tauri plugin; second instance exits non-zero with message (no IPC/focus-existing-window).
3. Boot services built (`lib.rs` "boot services built, mode: Empty") — **modes include `Empty`; other modes TBD in inventory** (candidate: seeded/demo modes).
4. Tokio background workers started (workers.rs: sync engine, job queue, webhook, embedding, maintenance timers; adaptive polling 30 s busy → quiet ceiling).
5. **Loopback HTTP listener** (`spp_core::loopback::LoopbackServer`): `POST /webhook` (Help Scout ingress) + `GET /oauth/callback` (OAuth redirect). Port selection: port baked into Help Scout redirect URI (default `http://localhost:3000/oauth/callback` → **:3000**) → `SPP_LOOPBACK_PORT` env → OS-assigned. On bind failure: WARN + app continues without inbound HTTP; pending webhook events drained from DB at worker start (workers.rs:219 `drain_pending` — code verified; runtime verification queued).
6. eframe/glow window (default renderer; `SPP_RENDERER=wgpu` switch observed in logs), reactive repaint driven by `bridge.rs` thread broadcasting `ServerEvent`s from the core EventBus to the UI via a queue + `request_repaint`.

## 4. UI structure (pages/mod.rs — 23 PageKey variants + Shell)

Workspace: Dashboard, Inbox, Live chats, Operations · Customers & knowledge: Customers, Organizations,
Knowledge · Insights: Reports, Issue Radar, Incidents · Automation & AI: Custom Objects, Automation,
AI Center, Connectors · plus Diagnostics, Notifications, Onboarding, Outreach, Search, Settings,
Support Graph, Sync Health. `ServerEvent`→`Vec<PageKey>` mapping (pages/mod.rs ~318–378) drives
cross-page invalidation (e.g. SyncCompleted refreshes multiple pages; ErrorOccurred routes by kind).

## 5. Data & state

- Embedded SQLite (single file), migrations in `crates/core/src/migrations.rs` + `bootstrap.rs` (52 applied at first run; labels m001…m061 observed in logs).
- `EventBus` (tokio broadcast) fan-out to UI bridge and workers; ServerEvent enum routes invalidations.
- Secret storage: crypto/secret envelope (m049) — details queued for inventory (SAFE-relevant).
- App data dir: `~/.local/share/supportos-plus/` (DB + instance.lock; backup created at shutdown per run1 log "Automatic backup created").

## 6. External integrations

Help Scout (sync engine, webhooks, OAuth redirect loopback), LM Studio (`http://127.0.0.1:1234` default),
Qdrant (`http://127.0.0.1:6333` default), OpenAI-compatible providers (ai_provider.rs), Beacon chat (Chat (Beacon) channel chip on Dashboard). All require configuration; verification via Connectors/Settings pages queued.

## 7. Findings register (resolved into FINDINGS.md)

The discovery candidates became formal findings: F-CAND-001 → **FIND-001**
(CONFIRMED Medium — port-preference chain + log-invisible degradation;
runtime matrix executed, exceptions EXC-001..005 for the credential-gated
remediation surfaces). The second-instance watch item → **FIND-002**,
CLOSED by W-009 (guard correct; feedback inventory recorded; stays
observation). See FINDINGS.md (FIND-001..FIND-011) — this section is
retired to avoid a divergent copy.

## 8. Open discovery questions (tracked) — status after deep-dive

1. Boot modes ("mode: Empty") — PARTIALLY ANSWERED: boot services are real
   when credentials exist, empty otherwise (main.rs boot order doc +
   provider hot-swap since 1.4.0); deep mode-seeding inventory not needed
   for the audit scope.
2. Worker job-kind list and cadences — ANSWERED (§9).
3. `smoke.rs` role — recorded as a discovered QA affordance (SPP_SMOKE
   harness); deliberately NOT used for evidence (non-default build).
4. Catalog → Custom Objects/Reports wiring — ANSWERED at the architecture
   level (§11: catalog enums/entries drive chip/tile/builder surfaces);
   per-combo behavior verified via pass-1/pass-2 page walks.
5. AI pipeline providers/embedding dims/health-fallback — ANSWERED at the
   architecture level (§10); runtime AI round-trips need a live LM Studio
   endpoint (external; W-005's enqueued jobs completed trivially on zero
   data).
6. Settings surface — ANSWERED (535-row control ledger + W-010 + QA45:
   tabs, chips, backup/export/restore gate, clear-log, theme persistence).

## 9. Deep-dive: background worker host (workers.rs, runtime cross-checked)

`WorkerManager` (crates/core/src/workers.rs) owns every background activity
on a dedicated 2-thread tokio runtime (main.rs boot order step 4). All
timers are tick-guarded and panic-isolated:

- **TickGuard (A1):** RAII latch over per-timer `AtomicBool`s — a mid-tick
  `stop()`, contained panic, or abort previously stranded the flag and
  silently killed that timer until process restart; the latch clears on
  every exit path, and `start()` resets all sweep flags "from a provably
  clean state".
- **`contained(name, fut)`** wraps each task body: a panic is logged to
  `application_errors` (service = task name) instead of killing the worker.

Timers observed in `start()` (interval; purpose):
1. **Job processor — 2 s.** Drains the `jobs` table
   (`jobs::ensure_jobs_table`: queue/type/status queued/running/completed/
   failed/cancelled/awaiting_approval). Outbound reply dispatch runs BEFORE
   background job kinds. AI-queue kinds observed in source: cluster_issues,
   rebuild_gaps, golden_eval, draft_article, ticket_analysis,
   interaction_observation/recommendation, memory_extraction, customer
   draft + draft_verification (the pipeline menu of ai_pipeline.rs).
2. **Adaptive auto-sync (1.4.0 R8/D18).** Busy account polls every 30 s;
   quiet accounts double the delay toward a ceiling read from the
   `sync_interval_minutes` setting with the SYNC_INTERVAL_MINUTES env var
   as fallback ("both defaults now agree on 1 minute, closing the
   two-divergent-defaults defect"); every outbound send calls
   `nudge_sync()` which wakes the timer immediately via a
   `tokio::sync::Notify` (raced against the quiet delay).
3. **6-hour timer** — long-horizon maintenance (embedding/index refresh
   lane).
4. **Stale-job sweep — 10 min.** `jobs::requeue_stale_running_jobs(…, 30)`:
   jobs stuck `running` > 30 min re-enter the queue (crash resilience;
   complements the boot-time recovery below).
5. **Notification sweep** — interval from the `notification_sweep_seconds`
   setting (default 15 s).
6. **Customer-events sweep, intelligence sweep (item 150: SLA-breach
   persistence, unmonitored-mailbox tracking, doc freshness), connectors
   refresh** — their own guarded timers.
7. **30 s timer** — secondary maintenance lane.

Boot recovery (before first tick): rows stuck mid-send by a crash re-enter
the outbound queue; `sync_state` stranded in INITIALIZING/BACKFILLING by an
unclean shutdown is recovered (otherwise auto-sync would skip itself
forever); `webhook_handler::drain_pending` drains persisted webhook events
at worker start (workers.rs:219 — the FIND-001 mitigation; runtime drain
blocked externally, see EXC-003).

**AI pipeline isolation (block_ai_pipeline):** every AI runner holds the
DbHandle write guard for the whole pipeline call, so driving the future on
the spawning runtime could self-deadlock (the runtime's only thread may
itself be blocked on the same write mutex). Each pipeline call therefore
builds a PRIVATE current-thread tokio runtime — the job always completes or
fails honestly regardless of the spawning runtime. The embeddings lane uses
the same isolation with its dedicated "lmstudio-embed" runtime.

## 10. Deep-dive: AI subsystem (provider/pipeline/cache)

- **Provider trait** (ai_provider.rs): `LocalAiProvider` (async_trait) with
  ChatMessage/ChatResponse/TokenUsage; the only production implementation
  is `LmStudioProvider` (ai_lm_studio.rs, OpenAI-compatible HTTP client,
  default `http://127.0.0.1:1234`). Provider swap semantics documented in
  the trait docs (hot-swap; a provider that vanishes mid-run fails that
  job honestly — the queue is the recovery surface).
- **Pipeline** (ai_pipeline.rs): one function per task, each with a
  VERSIONED prompt (PROMPT_VERSIONS_* in ai_prompts.rs), structured
  serde output (TicketAnalysis, DraftVerification, clusters, …), and
  evidence assembly (ai_evidence.rs: build/find_similar/sources_for).
- **Cache:** `ai_runs` table (M008, embeddings.rs) keyed
  (input_hash, prompt_version, model) — identical prompts+model skip the
  round-trip; input hashing via Sha256.
- **Runtime guard rails:** ai_memory_guard.rs (memory caps), ai_eval.rs
  (golden_eval jobs), ai_local.rs fallback path; Issue Radar's
  "Detect gaps now"/"Cluster now" enqueue `ai` jobs (runtime-verified in
  W-005: jobs 0→2 ai/completed with zero data — FIND-008).

## 11. Deep-dive: catalog crate + eventing

- **catalog** (crates/catalog): declarative domain vocabulary — workspace
  (IncidentStatus/Severity/Source, CustomFieldType, ConnectorKind/
  AuthMode), operations (OperationsTileKey), reporting (ReportMetricKey/
  ReportDimensionKey + MetricCatalogEntry/DimensionCatalogEntry with
  `metric_entry()` lookup — the Reports builder's metric/dimension
  combos), graph (GraphNodeKind, GraphHumanRelation), attributes,
  copilot, notifications. UI pages consume these enums for chip/tile/
  builder surfaces (e.g. Dashboard range/channel/mailbox chips).
- **eventing** (eventing.rs): in-process `EventBus` — a tokio broadcast
  channel of `ServerEvent`s; background engines (sync, workers, sweeps)
  emit, the native shell subscribes through its repaint bridge (egui
  `ctx.request_repaint` path observed in the shell bridge). This is why
  background completions surface in the UI without polling.
