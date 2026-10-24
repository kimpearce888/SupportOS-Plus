# ARCHITECTURE.md — SupportOS-Plus (incremental, updated as discovery proceeds)

Status: IN PROGRESS — initial map from source inspection of main @ 3affe7e. Refined as batches complete.

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

## 7. Candidate findings register (from discovery; to be confirmed per 04-FINDINGS §2)

- **F-CAND-001 (design/degraded-path):** default OAuth/webhook loopback port is **:3000** (config.rs:209) — extremely common dev port; collision silently degrades to no-inbound-HTTP mode with only a WARN log (main.rs:129). No UI indicator identified yet. Impact: Help Scout webhook ingress + OAuth reconnect fail while app appears fully functional. Fix direction (to be finalized): fail-loud UI badge + config surface for the port + docs. Runtime confirmation: launch with port free vs occupied, compare logs; verify `drain_pending` actually recovers queued webhooks.
- Watch: second-instance behavior (flock, non-zero exit, message shown?) — needs runtime test.

## 8. Open discovery questions (tracked)

1. Boot modes ("mode: Empty") — what modes exist and what do they seed?
2. Full worker job-kind list and cadences (workers.rs ~1000–1700).
3. `smoke.rs` role (SPP_SMOKE env? 24 pages / 4070 shapes per commit message) — smoke harness may aid UI automation.
4. Catalog → Custom Objects/Reports builder wiring depth (catalog-driven metric/dimension combos per batch-36 notes).
5. AI pipeline: providers, embedding dims, health/fallback paths (ai_hub.rs, ai_center.rs).
6. Settings surface: every config key, validation, persistence.
