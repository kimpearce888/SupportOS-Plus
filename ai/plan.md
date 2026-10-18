# SupportOs-Plus — Port Plan

> **Status:** Approved planning. No product code has been written yet.
> **Source repo:** `kimpearce888/SupportOS-PlusPlus` (upstream)
> **Port repo:** `kimpearce888/SupportOs-Plus` (this repo)
> **Base commit:** `ad2b081` ("Modernize entire stack to latest stable; clean-code refactor")
> **Plan date:** 2026-10-18

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
3. **Full feature parity** — all 24 pages, real-time updates, hybrid search, AI features,
   webhooks, encrypted sync.
4. **Future development friendly** — every risky choice sits behind a trait or facade so it is
   reversible at runtime cost, not rewrite cost. CI enforces license/advisory hygiene
   automatically.

Non-goals: cloud sync, telemetry, cross-platform packaging (Linux AppImage only), mobile.

---

## 2. Current stack (verified at HEAD `ad2b081`)

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

---

## 3. Migration map — verification results

The original map was written against the pre-modernization code (initial commit). Every row was
re-verified at HEAD. Verdicts:

| # | Map row | Verdict at HEAD |
|---|---|---|
| 1 | Empty webhook secret skips verification | **Confirmed** — `webhook_handler.rs:28`. Handler lives at `http/routes/webhook.rs` (`/api/webhooks/helpscout`); `loopback.rs` was **deleted** as dead code in the modernization commit |
| 2 | Hand-rolled SHA-1/HMAC/base64/hex | **Confirmed** — `webhook.rs:9-152`. Comparison is already constant-time (`subtle::ct_eq`); base64 dep is 0.23, not 0.22; RFC 2202 test vector already pinned by 3 unit tests |
| 3 | Unauthenticated localhost API (Host check + CORS only) | **Confirmed** — host-check middleware + CorsLayer in `server.rs`. Premise correction: no separate `loopback.rs` listener exists; webhook/oauth ride the main router. A minimal 2-endpoint listener must be **created** at cutover |
| 4 | Unbounded DOCX/PDF reads | **Confirmed +1** — `knowledge_store.rs:768` (`read_to_string`), `:749` (PDF). Map missed unbounded `std::fs::read(abs)` at `:705` |
| 5 | `query_row(...).ok()` in webhook dedup | **Confirmed ×3** — `webhook.rs:170`, `:211`, `:280`; DB errors swallowed as "not found" |
| 6 | Runtime DDL incl. DROP TABLE | **Confirmed** — `ensure_webhook_events_table` runs per insert path; legacy migration does RENAME + DROP (data-destroying) |
| 7 | Single `Arc<Mutex<Connection>>` | **Confirmed** — but premise wrong: `run_ai_job` uses the same shared conn (`workers.rs:1045`); nothing bypasses the mutex. Blast radius: 345 lock sites |
| 8 | Tauri → Slint | **Superseded** — see decision D1 (egui). Corrections found: `libc` is not a direct dep; Slint is GPL/royalty-free/commercial tri-licensed vs this MIT repo |
| 9 | Leptos → .slint | **Superseded** — Leptos already 0.8.22 (map said 0.6). Theme/sidebar localStorage confirmed (`ui/src/state.rs`) |
| 10 | SSE/EventBus/CorsLayer → event-loop bridge | **Viable** — maps to `egui::Context::request_repaint()` |
| 11 | qdrant-edge → sqlite-vec | **Viable with 2 gaps** — sqlite-vec has no sparse search (must go FTS5, kept anyway); sqlite-vec is 0.1.x alpha; `snapshot()` needs new impl. `qdrant_url`/`DEFAULT_QDRANT_URL` leftover confirmed |
| 12 | tokio full → slim | **Viable** — needed: rt-multi-thread, macros, net, signal, sync, time |
| 13 | Webhook secret in plain settings | **Confirmed — worse than mapped** — `secrets` table exists with a comment claiming encryption, but nothing ever writes to it and `get_secret` returns raw BLOB. Encryption at rest is fiction; must be built through `core::crypto` |
| 14 | Bundle targets → single AppImage | **Confirmed** — tauri.conf.json: deb/appimage/msi/nsis/dmg/app + hand-written WebKitGTK deb depends |
| 15 | Pin toolchain, drop wasm32 | **Confirmed** — `channel="stable"` + wasm32 target |
| 16 | rusqlite/thiserror/rand/axum → current majors | **Already done at HEAD** (axum 0.8.9, rusqlite 0.40.2, thiserror 2, rand 0.10). Survivor: duplicate reqwest 0.12.28 + 0.13.5 in lockfile — bump workspace reqwest to dedupe |

Keep list (all verified present): Rust, rusqlite bundled + FTS5, aes-gcm/scrypt/subtle, chrono,
chrono-tz, tracing, serde, ammonia (ingestion), reqwest+rustls, async-trait, LM Studio module.

Add list (corrected): 15 unit tests exist at HEAD (crypto, HMAC, SLA); zero tests for
webhook_handler, backup_service, encrypted_sync, hybrid_search. CI and LICENSE genuinely absent.

---

## 4. Decision ledger

### D1 — UI toolkit: **egui/eframe, pinned `=0.37`**

License MIT OR Apache-2.0 — no licensing decision, no blocker. Chosen over Slint/iced/relm4
because: one language and no DSL/codegen (maintainability); `egui_extras::TableBuilder`
virtualized tables + `egui_plot` match this table-heavy app (feature fit); largest pure-Rust GUI
community (stability); immediate-mode + `request_repaint` maps 1:1 to the EventBus (row 10);
tiny dep tree → clean single AppImage (row 14). Frequent breaking releases are neutralized by
exact pinning. The UI layer stays thin over `catalog`+`core`, keeping future UI swaps cheap.

One-time costs, toolkit-agnostic: HTML→RichText converter for conversation bodies (new
`core::richtext` module, fixture-tested) and a custom mention-textarea widget.

Rejected: Slint (GPL/relicense decision + DSL), iced (no virtualized tables; heavier custom
widget work), relm4/GTK4 (drags system deps back into the AppImage), Dioxus native (renderer
not production-ready).

### D2 — Sandbox: **caps now, Landlock+seccomp deferred**

Byte caps land in Phase 1 (`Read::take`, zip declared-size validation, file-size cap). The
seccomp/Landlock subprocess sandbox becomes a post-port, Linux-only, feature-flagged epic.
Rationale: never couple two re-architectures; parsers are safe Rust; caps remove the actual
unbounded-memory risk.

### D3 — DB pool: **deferred, behind a facade**

The port binds to a thin `DbHandle` facade that initially wraps the existing
`Arc<Mutex<Connection>>`. After router deletion (~245 of 345 lock sites die with it), the real
pool (1 dedicated writer thread + 2 read-only WAL connections) lands on the remaining ~100
sites incrementally — ported UI code never changes.

### D4 — Vector store: **conditional GO on sqlite-vec**

Gate: `xtask vector-count` against the live DB. `< 100k` chunks → GO: dense via vec0 virtual
tables, sparse via FTS5, RRF fusion untouched, sqlite-vec pinned exact (alpha). `>= 100k` →
qdrant-edge stays; only the dead `qdrant_url` setting is removed. Either way the
`VectorStore` trait makes the choice reversible.

### D5 — Workflow: **trunk-based, always-green, machine-enforced**

No long-lived port branch. The egui shell runs side-by-side with the Leptos UI on `main`
(core is UI-agnostic; axum stays alive for the old UI until the last page is ported). Each
page port is a small revertible commit. CI: fmt + clippy + test + `cargo deny`
(license/advisory) from Phase 0. Characterization tests (<2 min) are the port's insurance.
Theme/sidebar state: no migration (two booleans; defaults + re-pick).

---

## 5. Execution plan

```
Phase 0  LICENSE · CI (fmt/clippy/test/deny) · characterization tests · vector-count · pin egui+toolchain
Phase 1  Core hardening — rows 1, 2, 4a, 5, 6, 13 + reqwest dedupe
Phase 2  egui shell + DbHandle facade + core::richtext converter     (side-by-side begins)
Phase 3  Page-by-page port, 24 pages · EventBus→request_repaint · settings-table theme
Phase 4  Cutover: delete router (~21K LOC) + Leptos + Tauri + wasm32 · AppImage packaging · flock
Phase 5  DB pool (facade guts swap, ~100 sites) · sqlite-vec swap · tokio slim
Phase 6  Sandbox epic (Landlock+seccomp, Linux, feature-flagged) · CI AppImage job
```

### Phase 0 — Baseline and gates (no product code)

- **0.1** `LICENSE` (MIT, matching manifests).
- **0.2** CI skeleton: fmt + clippy (deny warnings) + `cargo test` + `cargo deny`
  (licenses + advisories) on Linux.
- **0.3** Characterization tests, written first, pinning current behavior:
  `webhook_handler` (accept/duplicate/signature-reject), `backup_service`,
  `encrypted_sync`, `hybrid_search` (RRF vectors), SLA edge cases (extend the existing 15).
- **0.4** `xtask vector-count` — counts chunks in the live DB; decides D4.
- **0.5** Pin toolchain to an exact version (≥1.95 for egui 0.37) and pin egui `=0.37`;
  keep wasm32 target until Phase 4.
- **Done when:** CI green on a tag, tests pass, chunk count recorded here.

### Phase 1 — Core hardening (rows 1, 2, 4a, 5, 6, 13; UI-independent)

| Step | Work | Notes |
|---|---|---|
| 1.1 | Row 2: swap to `hmac` + `sha1` crates + existing `base64` + `Mac::verify_slice` | existing RFC-vector tests become the net |
| 1.2 | Row 1: empty secret → hard reject outside demo mode | after 1.1 |
| 1.3 | Row 5: `.ok()` → `.optional()?` at the 3 sites | |
| 1.4 | Row 6: DDL → numbered migration; stop ensuring schema per-insert | |
| 1.5 | Row 13: `set_secret`/`get_secret` through `core::crypto` — per-value random salt+nonce envelope (`salt‖nonce‖ciphertext`); boot-time migration of `webhook_secret` from plain settings; env var stays for CI | |
| 1.6 | Row 4a: byte caps — file cap (~25 MB), zip declared-size check + `Read::take` on document.xml, PDF input cap | |
| 1.7 | Row 16 remainder: workspace reqwest 0.12 → 0.13 (dedupe lockfile) | |
| 1.8 | Row 4b (sandbox): **deferred to Phase 6** per D2 | |

### Phase 2 — egui shell + seams (side-by-side begins)

- **2.1** `crates/app-shell` (working name): eframe app, default fonts bundled, window
  persistence on, dark/light theme read from the `settings` table (row 9), flock
  single-instance in `$XDG_RUNTIME_DIR` (`libc` added as direct dep).
- **2.2** `DbHandle` facade over `Arc<Mutex<Connection>>` (D3). All new UI code binds to
  `catalog` types + facade only.
- **2.3** `core::richtext`: ammonia-sanitized HTML → styled spans (bold/italic/links/mentions),
  fixture-tested against real HelpScout bodies.
- **2.4** Background task bridge: core async work → results channel → `request_repaint`.
- **Done when:** empty shell opens, shows dashboard data via direct core calls.

### Phase 3 — Page-by-page port (24 pages, 22 components)

- Port order: dashboard → inbox (largest, proves mention widget + richtext) → customers →
  knowledge/docs → reports → operations → remaining pages.
- `ui/sse.rs` + `sse_bridge.rs` equivalents: event bus subscription → repaint; delete nothing
  yet — Leptos UI keeps running from the same core.
- Command palette, overlays, toasts as egui widgets.
- **Done when:** every page reachable in the egui shell; Leptos UI still functional.

### Phase 4 — Cutover (one-way, last step of the port)

- Delete the 332-route axum router (~21K LOC) and the Leptos/WASM/Trunk/Tauri stack
  (`crates/ui`, `crates/app/src-tauri`, wasm32 target, CSP/localhost plumbing,
  `tauri-plugin-single-instance`).
- **Create** the minimal loopback listener (webhook + OAuth callback only — OAuth requires a
  real HTTP callback; ~100 lines of axum on one route or raw hyper).
- Packaging: single AppImage via `xtask package` (linuxdeploy/appimagetool); deb/msi/nsis/dmg/
  app targets deleted (row 14).
- Pin final toolchain; drop wasm32 (row 15).
- **Done when:** AppImage builds in CI and runs the full app.

### Phase 5 — Data layer completion

- **5.1** DB pool inside the `DbHandle` facade (D3): writer thread + 2 WAL readers, WAL +
  `busy_timeout`; migrate remaining ~100 lock sites mechanically, tests green per sweep.
- **5.2** sqlite-vec swap behind `VectorStore` (D4 outcome; FTS5 sparse; delete qdrant-edge +
  `qdrant_url` if GO).
- **5.3** tokio feature slim + 2-thread runtime (row 12; recheck feature set post-cutover).

### Phase 6 — Hardening epic

- Landlock + seccomp sandbox for pdf/zip parsing (Linux-only, feature-flagged, subprocess
  workers) per D2.
- CI: AppImage build + smoke-test job.

---

## 6. Risks and mitigations

| Risk | Mitigation |
|---|---|
| egui breaking releases | pin `=0.37`; upgrades are deliberate, scheduled tasks |
| HTML rendering gaps (HelpScout bodies) | `core::richtext` with fixture tests; display plain text fallback, never fail a page |
| Mention textarea complexity | custom widget isolated in one module; ship basic first, enhance later |
| sqlite-vec alpha quality | exact pin + `VectorStore` trait = swap-out seam; count gate before commit |
| 345-site refactor regression | facade now, pool later, incremental sweeps, characterization tests |
| Port stalls mid-way | side-by-side on `main` — every intermediate state is shippable; no cliff |
| Secret migration breaks existing installs | env var override kept; boot migration idempotent; backup before write |

---

## 7. Document change log

- 2026-10-18 — Initial plan: map verified at `ad2b081`, decision ledger (D1-D5), phases 0-6.
