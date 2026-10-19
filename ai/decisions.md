# SupportOs-Plus — Decisions & Reference

> This file holds the approved planning context. Execution tracking lives in `plan.md`
> (remaining items), `progress.md` (active five), and `completed.md` (verified items) —
> governed by `rule.md`.

---

## 1. Context and goals

SupportOs-Plus is the port repo for SupportOS-PlusPlus. The port is a **stack change**: the
webview-based desktop stack (Tauri 2 + Leptos/WASM + loopback HTTP API) is replaced with a
native pure-Rust UI, while the ~90K-line pure-Rust business core is kept and hardened.

Goals, in priority order:

1. **Stable** — pinned toolchain and dependencies, always-green `main`, no alpha crates in the
   critical path without an isolation seam.
2. **Easily maintainable** — one language (Rust), no UI DSL, no codegen step, small dependency
   tree, solo-maintainer scale.
3. **Full feature parity** — every page of the original's 24-file `ui/src/pages` tree: 20 egui
   pages plus the command palette overlay (`palette.rs`) and the native Shell page (21
   `PageKey`s); `docs.rs` folded into the knowledge page's help-center tab and `not_found.rs`
   is unreachable natively (the sidebar cannot 404). Real-time updates, hybrid search, AI
   features, webhooks, encrypted sync.
4. **Future development friendly** — every risky choice sits behind a trait or facade so it is
   reversible at runtime cost, not rewrite cost. CI enforces license/advisory hygiene
   automatically.

Non-goals: cloud sync, telemetry, cross-platform packaging (Linux AppImage only), mobile.

## 2. Current stack (verified at base `ad2b081`)

| Layer | Tech | Version | Scale |
|---|---|---|---|
| Language | Rust, edition 2024, MSRV 1.85, resolver 3 | stable | ~125,400 LOC, 5 crates |
| Desktop shell | Tauri 2 + single-instance plugin | 2.12.4 | 118 LOC (thin) |
| Frontend | Leptos CSR + leptos_router (WASM) | 0.8.22 | 24 pages, 22 components, 33,645 LOC |
| Backend HTTP | axum, loopback-only `127.0.0.1:3000` | 0.8.9 | 332 routes, 318 handlers, ~21K LOC |
| Runtime | tokio `features=["full"]` | 1.53 | 38 call sites, zero fs/process usage |
| Database | rusqlite (bundled SQLite) | 0.40.2 | ~80 domain modules, FTS5 |
| Concurrency | single `Arc<Mutex<Connection>>` | — | 345 lock sites (http/ + workers) |
| Vector store | qdrant-edge (embedded) | =0.8.0 | dense + sparse, RRF hybrid |
| AI adapters | reqwest + SSE → LM Studio / Ollama / OpenAI-compat | 0.12 | localhost only |
| Crypto | aes-gcm 0.11, scrypt 0.12, sha2, subtle | — | unified `core::crypto`, 15 unit tests |
| Integrations | HelpScout (real + OAuth), webhooks | — | |
| Misc | ammonia, pdf-extract, zip, quick-xml, chrono-tz | — | docs ingestion, SLA math |

Crate map: `catalog` 1,339 LOC · `core` 89,910 · `ui` 33,645 · `xtask` 461 · `app/src-tauri` 118.

## 3. Migration map — verification results (16 rows)

The original map was written against the pre-modernization code. Every row was re-verified at
`ad2b081`:

| # | Map row | Verdict |
|---|---|---|
| 1 | Empty webhook secret skips verification | Confirmed — `webhook_handler.rs:28`; handler at `http/routes/webhook.rs` (loopback.rs deleted as dead code) |
| 2 | Hand-rolled SHA-1/HMAC/base64/hex | Confirmed — `webhook.rs:9-152`; already constant-time compare; base64 0.23; RFC vector tests exist |
| 3 | Unauthenticated localhost API | Confirmed — host-check + CORS; no separate loopback listener exists; must create minimal 2-endpoint listener at cutover |
| 4 | Unbounded DOCX/PDF reads | Confirmed +1 — also unbounded `std::fs::read` at `knowledge_store.rs:705` |
| 5 | `query_row(...).ok()` in webhook dedup | Confirmed ×3 — `webhook.rs:170`, `:211`, `:280` |
| 6 | Runtime DDL incl. DROP TABLE | Confirmed — runs per insert path; RENAME+DROP destroys legacy data |
| 7 | Single `Arc<Mutex<Connection>>` | Confirmed — run_ai uses the same shared conn; 345 lock sites |
| 8 | Tauri → Slint | Superseded by D1 (egui); libc not a direct dep; Slint tri-license vs MIT repo |
| 9 | Leptos → .slint | Superseded — Leptos already 0.8.22; localStorage theme confirmed |
| 10 | SSE/EventBus/CorsLayer → event-loop | Viable — maps to `egui::Context::request_repaint()` |
| 11 | qdrant-edge → sqlite-vec | Viable with gaps — no sparse search (FTS5 instead); 0.1.x alpha; snapshot needs new impl |
| 12 | tokio full → slim | Viable — rt-multi-thread, macros, net, signal, sync, time |
| 13 | Webhook secret in plain settings | Confirmed, worse — secrets table has no writers and no encryption; build through `core::crypto` |
| 14 | Bundle targets → single AppImage | Confirmed — deb/appimage/msi/nsis/dmg/app + WebKitGTK deb depends |
| 15 | Pin toolchain, drop wasm32 | Confirmed — `channel="stable"` + wasm32 target |
| 16 | rusqlite/thiserror/rand/axum → current majors | Already done at HEAD; survivor: duplicate reqwest 0.12+0.13 in lockfile |

Keep list (verified present): Rust, rusqlite bundled + FTS5, aes-gcm/scrypt/subtle, chrono,
chrono-tz, tracing, serde, ammonia (ingestion), reqwest+rustls, async-trait, LM Studio module.

Add list (corrected): 15 unit tests exist (crypto, HMAC, SLA); zero for webhook_handler,
backup_service, encrypted_sync, hybrid_search. CI and LICENSE absent.

## 4. Decision ledger

### D1 — UI toolkit: egui/eframe, pinned `=0.37`
MIT OR Apache-2.0 — no licensing decision, no blocker. One language, no DSL/codegen;
`egui_extras::TableBuilder` + `egui_plot` fit this table-heavy app; immediate mode +
`request_repaint` maps 1:1 to the EventBus; tiny dep tree → clean AppImage. Frequent breaking
releases neutralized by exact pinning. UI layer stays thin over `catalog`+`core`.
One-time toolkit-agnostic costs: HTML→RichText converter (`core::richtext`), custom
mention-textarea widget.
Rejected: Slint (GPL decision + DSL), iced (no virtualized tables), relm4/GTK4 (system deps),
Dioxus native (not production-ready).

### D2 — Sandbox: caps now, Landlock+seccomp deferred
Caps in Phase 1; sandbox as post-port Linux-only feature-flagged epic. Never couple two
re-architectures.

### D3 — DB pool: deferred, behind a facade
Port binds to a `DbHandle` facade wrapping the current `Arc<Mutex<Connection>>`. After router
deletion (~245 of 345 sites die), the pool (1 writer thread + 2 WAL readers) lands on the
remaining ~100 sites incrementally; ported UI code never changes.

### D4 — Vector store: conditional GO on sqlite-vec
Gate: `xtask vector-count` on the live DB. `<100k` chunks → GO (dense vec0, sparse FTS5, RRF
untouched, exact pin). `>=100k` → qdrant-edge stays, only dead `qdrant_url` removed.
`VectorStore` trait keeps it reversible.

### D5 — Workflow: trunk-based, always-green, machine-enforced
No port branch; egui shell runs side-by-side with Leptos on `main`; each page is a small
revertible commit. CI: fmt + clippy + test + `cargo deny` from Phase 0. Characterization
tests are the port insurance. Theme/sidebar: no migration (defaults + re-pick).

## 5. Phase definitions (design detail for the TODO items in plan.md)

- **Phase 0 — baseline/gates:** LICENSE, CI, characterization tests, vector-count, pins.
- **Phase 1 — core hardening:** rows 1, 2, 4a, 5, 6, 13 + reqwest dedupe.
- **Phase 2 — egui shell + seams:** `crates/app-shell` (eframe, settings theme, flock),
  `DbHandle` facade, `core::richtext`, task bridge.
- **Phase 3 — page-by-page port:** dashboard → inbox → customers → knowledge/docs → reports →
  operations → remaining 17; event bus → repaint; palette/overlays/toasts.
- **Phase 4 — cutover:** delete router + Leptos + Tauri + wasm32 + CSP plumbing; create
  minimal loopback listener (webhook + OAuth); AppImage-only packaging; final pins.
- **Phase 5 — data layer:** DB pool in facade, sqlite-vec swap per D4, tokio slim.
- **Phase 6 — hardening:** Landlock+seccomp epic, CI AppImage job.

## 6. Risks and mitigations

| Risk | Mitigation |
|---|---|
| egui breaking releases | pin `=0.37`; upgrades are deliberate |
| HTML rendering gaps | `core::richtext` fixture tests; plain-text fallback |
| Mention textarea complexity | isolated custom widget; basic first |
| sqlite-vec alpha | exact pin + trait seam; count gate |
| 345-site refactor regression | facade now, pool later, incremental sweeps |
| Port stalls mid-way | side-by-side on `main`; no cliff |
| Secret migration breaks installs | env override kept; idempotent boot migration |

## 7. Change log

- 2026-10-18 — Initial plan approved and committed (plan.md @ `cd5a8cb`).
- 2026-10-18 — Narrative moved here from plan.md; plan.md restructured into the TODO list
  required by rule.md. No content discarded (Rule 6: zero items lost in the restructure).
- 2026-10-19 — Final batch (36-39) design notes:
  - **Worker host (36):** `app-shell::workers_host` owns a std thread hosting a
    fixed 2-worker-thread multi-thread tokio runtime; `main` starts it at boot and
    the settings page restarts it on sync-interval change (old settings-route
    behavior). Tests keep the host out of `ShellApp` so page tests stay timer-free.
  - **tokio slim (37):** features `rt, rt-multi-thread, time, sync` — exactly the
    port's call sites; net/io arrive via reqwest's own dependency (unification),
    fs/process/signal have zero call sites.
  - **Sandbox (38):** `core::sandbox` (Linux, `--features sandbox`): Landlock
    deny-all (BestEffort) + seccomp allow-list with `EPERM` mismatch instead of
    kill — a killed thread cannot notify its channel (caller would hang to the
    timeout), while EPERM still blocks every escape path; `write` pinned to fds
    1/2. Off by default; CI clippy+tests it on a Landlock-enforcing kernel.
  - **CI smoke (39):** `appimage` job builds via `xtask package` and runs the
    artifact with `APPIMAGE_EXTRACT_AND_RUN=1 SPP_SMOKE=1` — the env flag makes
    the binary verify logging, lock, migrations, boot services, workers and the
    loopback listener, then exit 0 headless.
- 2026-10-19 — Final batch (items 45-46) design notes:
  - **Mock-server integration suite (45):** all modules are `#[cfg(test)]` (zero release
    footprint). The mock server mirrors `crate::loopback`'s listener shape (thread per
    connection, bounded reads) but is generic over handler closures; the real client
    stacks (reqwest + ApiQueue + rate limiter + `SyncEngine` + `LoopbackServer`) run
    end-to-end over TCP against it. Real-service verification stays explicitly out of
    scope: no credentials exist in this environment and none were invented; the live
    LM Studio test is env-gated on `SPP_LM_STUDIO_URL` and honestly ignored without it.
  - **Two production fixes from 45:** `oauth_tokens` saves pin `id = 1` (the schema's
    `CHECK (id = 1)` made every token refresh fail — inherited from the original);
    `ai_lm_studio` surfaces HTTP status + body on non-2xx instead of a parse error.
  - **Bench (46):** harness + procedures documented in `bench.rs`'s module doc;
    measurements and the two bench-harness bug fixes (cursor infinite loop, missing
    post-sync FTS rebuild step) are recorded in `completed.md` batch 10.
