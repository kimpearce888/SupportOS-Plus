# completed.md — completed and verified items

> Items land here from `progress.md` only after Rule 5 verification. Each entry records the
> work done and the verification evidence (test run, CI run, build result).

## Completed

### Batch 1 — 2026-10-18 (commit `560a5ce`, CI run `39112162370`: success)

**1. Add `LICENSE` (MIT, matching manifests)**
- Work: `LICENSE` file added at repo root with the standard MIT text (copyright 2026 kimpearce888).
- Verification: `cargo deny check licenses` passes with MIT in the allow list (local + CI);
  the workspace `license = "MIT"` declaration and the file now agree.

**2. Add CI: fmt + clippy + test + cargo-deny**
- Work: `.github/workflows/ci.yml` — runs on every push/PR: `cargo fmt --all --check`,
  `cargo clippy` (catalog/core/xtask, `--all-targets -D warnings`), `cargo test -p
  supportos-plusplus-core`, `cargo deny check advisories licenses` via taiki-e/install-action.
  `deny.toml`: permissive-license allow list (MIT, Apache-2.0, BSD, ISC, Zlib, MPL-2.0,
  CC0-1.0, BSL-1.0, …); 8 unmaintained-crate advisories from the leptos/tauri/xtask trees
  ignored with documented reasons (most disappear at the Phase 4 cutover). Clippy/test are
  scoped to catalog/core/xtask until the Phase 4 cutover (ui needs wasm32, app needs WebKitGTK).
- Verification: CI run `39112162370` on commit `560a5ce` completed with conclusion `success`
  (all four steps green). Also reproduced locally with cargo-deny 0.20.2: advisories ok,
  licenses ok.

**3. Characterization tests: `webhook_handler`**
- Work: 7 tests in `crates/core/src/webhook_handler.rs` — accept + persist + job enqueue
  (convo.created → sync_conversation), empty body → BadRequest, non-JSON body → BadRequest,
  wrong signature → SignatureInvalid, missing signature with secret set → SignatureInvalid,
  empty secret skips verification (characterizes CURRENT behavior; flips with plan item 6 /
  map row 1), duplicate event → same row id, drain_pending processes stored events.
- Verification: `cargo test -p supportos-plusplus-core` — 32/32 pass locally and in CI.

**4. Characterization tests: `backup_service`**
- Work: 4 tests in `crates/core/src/backup_service.rs` — backup creates a verified DB (VACUUM
  INTO, integrity_check ok) plus a `.settings.json` sidecar that redacts token/secret/password
  keys; verify_backup rejects non-database files; restore swaps the database file, removes
  stale `-wal`/`-shm`, and preserves data; restore reports missing backups; prune keeps the
  newest N and removes pruned sidecars.
- Verification: `cargo test -p supportos-plusplus-core` — 32/32 pass locally and in CI.

**5. Characterization tests: `encrypted_sync`**
- Work: 4 tests in `crates/core/src/encrypted_sync.rs` — export rejects passphrases < 8 chars;
  export→verify round-trip (SOSYNC magic present, no plaintext SQLite header in the bundle,
  right passphrase verifies, wrong passphrase rejected); tampered ciphertext fails GCM
  authentication; full import round-trip restores data with a `.pre-import-` safety backup.
- Verification: `cargo test -p supportos-plusplus-core` — 32/32 pass locally and in CI.

**Batch verification summary:** `cargo fmt --all --check` clean · `cargo clippy -D warnings`
clean (catalog/core/xtask, all targets) · `cargo test` 32/32 (15 new characterization tests +
17 pre-existing) · `cargo deny check advisories licenses` ok · GitHub Actions run
`39112162370` → success.

### Batch 2 — 2026-10-18 (commit `89f88b9`, CI run `39120945826`: success)

**6. Characterization tests: `hybrid_search` (RRF fusion vectors)**
- Work: 12 tests in `crates/core/src/hybrid_search.rs` — RRF constants pinned (k = 60 on both
  fusion paths), dense-only RRF math and order (1/61, 1/62, 1/63 exactly), item hit by both
  lists outranks single-list items, top-k truncation, payload prefers the dense source,
  custom-k variant, `merge_doc_hits` semantics (FTS+semantic label fusion, duplicate-id
  dedupe, tie-break by article id, list position not the rank field), cosine-similarity edge
  cases.
- Verification: `cargo test -p supportos-plusplus-core` — 61/61 pass locally and in CI.

**7. Characterization tests: SLA edge cases**
- Work: 13 new tests in `crates/core/src/business_hours.rs` — `parse_instant_ms` format matrix
  (date-only, explicit offset, space-separated naive, whitespace-trimmed, fractional seconds)
  and rejection matrix (blank, garbage, month 13, hour 25), equal bounds → 0.0, reversed range
  rejected, overnight windows unsupported (characterizes CURRENT behavior), weekend fully
  outside hours → 0.0 not None, DST transitions count real elapsed minutes, spans > 400 days
  rejected, `start_minute` clamped, out-of-range days dropped, SLA boundary inclusive, verdict
  matrix, timezone validation.
- Verification: `cargo test -p supportos-plusplus-core` — 61/61 pass locally and in CI.

**8. Add `xtask vector-count` command; record chunk count (D4 gate)**
- Work: `VectorCount` subcommand in `crates/xtask/src/bin/xtask.rs` — resolves the embedded
  qdrant store directory (new public `embedded_store_dir`, previously a private
  `EmbeddedQdrant::dir_for`), exact point count via new `EmbeddedQdrant::try_point_count`
  (`Ok(None)` when the collection does not exist yet), per-entity breakdown via new
  `count_by_entity` (knowledge / docs / conversation chunks), optional SQLite cross-check
  (row counts + `embedding_state = 'indexed'`), and the D4 gate verdict
  (< 100,000 chunks → sqlite-vec GO). `embeddings::entity_type_for` made public and
  documented. 2 unit tests over a seeded temp store.
- Verification: `cargo run -p supportos-plusplus-xtask -- vector-count` runs clean on an empty
  data dir (graceful "store not found → 0 points" path, prints `D4 gate -> 0 < 100000 chunks:
  sqlite-vec GO`); unit tests pass (61/61); CI green.
- Recorded count: 0 points (no production data yet) — re-run on seeded data before the
  Phase 5 swap (plan item 25).

**9. Pin toolchain to exact version; pin egui `=0.37` in workspace**
- Work: `rust-toolchain.toml` channel `stable` → `1.99.1` (exact pin; upgrades become
  deliberate, never silent). Workspace `Cargo.toml` gains `egui`/`eframe`/`egui_extras`/
  `egui_plot` all pinned `=0.37` (decision D1; first consumed by `crates/app-shell` in
  Phase 2). CI switched from `dtolnay/rust-toolchain@stable` to `rustup toolchain install`
  so `rust-toolchain.toml` is the single source of truth. `Cargo.lock` regenerated
  (egui tree + hmac/sha1/hex). wasm32 target retained until the Phase 4 cutover.
- Verification: local fmt/clippy/test all on 1.99.1; CI installed the pinned toolchain and
  ran green on it.

**10. Row 2: replace hand-rolled HMAC/SHA-1/base64/hex with crates**
- Work: `crates/core/src/webhook.rs` — `compute_signature`/`verify_signature` now use
  `Hmac<Sha1>` (`hmac` 0.13 + `sha1` 0.11). Verification uses `Mac::verify_slice`
  (constant-time tag comparison) against the base64-decoded provided signature; invalid
  base64 → `Error::WebhookSignature`. ~120 lines of hand-rolled SHA-1, base64, and hex
  helper code deleted. Core gains direct `hmac`, `sha1`, `hex` deps (base64 was already
  direct).
- Verification: 4 new tests — RFC 2202 vectors (tests 1, 2, and 6 with a key longer than the
  64-byte block), the classic HMAC-SHA1 digest vector, determinism, and valid/tampered-body/
  wrong-key/invalid-encoding rejection. 61/61 pass locally and in CI.

**Batch verification summary:** `cargo fmt --all --check` clean · `cargo clippy --all-targets
-- -D warnings` clean (catalog/core/xtask, forced fresh run after touching all changed
inputs) · `cargo test -p supportos-plusplus-core` 61/61 (29 new + 32 pre-existing) ·
`cargo run -p supportos-plusplus-xtask -- vector-count` executes and prints the D4 gate
verdict · `cargo deny check advisories licenses` ok · GitHub Actions run `39120945826` →
success on commit `89f88b9`.

### Batch 3 — 2026-10-18 (commit `cc3a288`, CI run `39126901850`: success)

**11. Row 1: empty webhook secret → hard reject outside demo mode**
- Work: `webhook_handler::process_webhook` gains a `demo_mode` parameter and a new
  `WebhookProcessResult::SecretNotConfigured` variant. An empty secret outside demo mode
  returns it immediately (HTTP 403 on the webhook route with an explanatory message);
  inside demo mode unsigned events still flow so the simulation tools exercise the pipeline.
  Callers updated: http/routes/webhook.rs (passes `state.demo_mode`), demo.rs and the
  `/api/demo/simulate-webhook` route (demo tools, secret always set there).
- Verification: `empty_secret_hard_rejects_outside_demo_mode` (incl. provided-signature case)
  and `empty_secret_allows_unsigned_events_in_demo_mode`; 68/68 tests locally and in CI.

**12. Row 5: replace `query_row(...).ok()` with `.optional()?`**
- Work: the three sites in `webhook.rs` (legacy schema probe, insert-duplicate lookup,
  persist_event lookup) now use rusqlite's `OptionalExtension::optional()?`, so genuine DB
  errors propagate instead of being silently treated as "row not found". The same fix was
  applied to `settings::get_string` — required because the m049 migration reads it and a
  swallowed error would silently skip secret migration.
- Verification: clippy `-D warnings` clean; 68/68 tests; behavior unchanged for happy paths.

**13. Row 6: webhook DDL to a numbered migration; no per-insert ensure**
- Work: `ensure_webhook_events_table` renamed to `apply_m048_webhook_events` (legacy-body
  schema drop + CREATE TABLE/INDEX) and wired as dedicated boot step v39 `m048`;
  removed from the v32 `runtime_table_guards` bundle. `insert_webhook_event`,
  `persist_event`, and `get_pending_webhook_events` no longer call it — schema comes from
  the migration runner, real errors surface. Test fixtures now open databases through
  `bootstrap::apply_all` exactly like the app.
- Verification: 68/68 tests (webhook persistence/dedup/drain suite runs on a
  migration-opened connection); CI green.

**14. Row 13: encrypted secrets envelope + webhook_secret boot migration**
- Work: `settings::set_secret`/`get_secret` seal values as `0x01 || nonce || AES-256-GCM
  ciphertext` (via `core::crypto`) under a per-installation master key stored at
  `<data_dir>/secrets.key` (0600, create-new, never overwritten). `bootstrap::apply_all`
  initializes the key before migrations run (db file path discovered via
  `PRAGMA database_list`; in-memory DBs skip). New boot step v40 `m049` moves the legacy
  plaintext `webhook_secret` from `application_settings` into the envelope and deletes the
  plaintext row (idempotent; skips with a warning when no key). Readers switched to the
  envelope: webhook route, sync status (`has_secret`), webhook registration, demo simulator
  (demo-default fallback). Settings PATCH now accepts `webhook_secret` (null/empty clears,
  16-256 chars) and routes it to `set_secret`; a failed secret write returns 500 instead of
  being silently dropped. `db::db_file_path` moved to `db.rs` as shared infrastructure.
- Verification: `secrets_envelope_lifecycle` covers no-key failure, 0600 key creation,
  round-trip, plaintext-never-in-table, delete/redaction, wrong-installation-key error,
  corrupt-envelope error, m049 migration + idempotency, fresh-boot key, malformed key file;
  68/68 tests; CI green.
- Known limitation (by design): a sync bundle imported into a different installation
  carries envelope rows the new master key cannot open; those reads error and callers fall
  back to env/default — the secret must be re-entered once.

**15. Row 4a: byte caps — file import, zip declared size + Read::take, PDF input**
- Work: `knowledge_store.rs` gains `MAX_IMPORT_FILE_BYTES` (64 MiB, checked via metadata
  before `std::fs::read`), `MAX_PDF_INPUT_BYTES` (32 MiB, checked before the CPU-heavy
  extractor), `MAX_DOCX_ENTRY_BYTES` (64 MiB, checked against the zip entry's declared
  uncompressed size AND enforced via `Read::take(cap+1)` plus a produced-length re-check,
  because declared sizes can lie). HTTP manual-paste imports remain bounded by the existing
  axum `DefaultBodyLimit`.
- Verification: 4 new tests — oversized file rejected pre-read (small file still imports),
  PDF over cap rejected by the size guard (small junk still reaches the parser), small-DOCX
  extraction regression for the take() refactor, and a 64 MiB+1 declared-size docx entry
  rejected. 68/68 tests; CI green.

**Batch verification summary:** `cargo fmt --all --check` clean · `cargo clippy --all-targets
-- -D warnings` clean (catalog/core/xtask) · `cargo test -p supportos-plusplus-core` 68/68
(7 net new) · `cargo deny check advisories licenses` ok · `xtask vector-count` still runs
after the bootstrap changes · GitHub Actions run `39126901850` → success on `cc3a288`.

### Batch 4 — 2026-10-18 (commit `8875597`, CI run `39132228740`: success)

**16. Row 16 remainder: workspace reqwest 0.12 → 0.13 (dedupe lockfile)**
- Work: workspace `reqwest` bumped to 0.13.5 with features `json`, `rustls`, `form`.
  0.13 renamed `rustls-tls` → `rustls` (now the aws-lc-rs provider plus
  `rustls-platform-verifier` roots instead of the old ring/webpki-roots setup) and moved
  `RequestBuilder::form` behind the `form` feature — the only call-site break, fixed in
  `helpscout_real.rs`. `Cargo.lock` now contains a single reqwest entry shared by core,
  xtask, and tauri 2.12.4; the xtask `blocking` feature carried over unchanged.
- Verification: `Cargo.lock` has exactly one `name = "reqwest"` (0.13.5); clippy
  `-D warnings` clean on catalog/core/xtask/app-shell (all targets, xtask binary included);
  94/94 core tests; `cargo deny` ok; `xtask vector-count` links and runs on the new tree.

**17. `crates/app-shell`: eframe shell, settings-table theme, flock single-instance**
- Work: new workspace crate `crates/app-shell` (lib `spp_shell` + bin `supportos-plusplus`)
  on the pinned eframe 0.37. Implements the new `App::logic`/`App::ui` split; `ui` renders
  a CentralPanel placeholder home (identity/version, theme toggle, database path + schema
  version, live bridge counters). Theme persists in the settings table under `"theme"`
  (`"light"` default, matching the old API's default; no migration per D5 — localStorage
  does not exist natively). Single-instance via `flock(2)` (`LOCK_EX|LOCK_NB`) on
  `<data_dir>/instance.lock` through `libc` — the only unsafe code in the crate, module
  `#[allow]`ed against the crate-level `deny(unsafe_code)`; a second process exits with a
  clear message and code 1. `open_db` mirrors the Tauri shell boot (`db::open` +
  `bootstrap::apply_all`). Workspace members list deduplicated (`app/src-tauri` was listed
  twice). CI clippy/test scope extended to app-shell — egui needs no system packages to
  build, link, or run headless tests.
- Verification: 11/11 app-shell tests — theme default/unknown-fallback/roundtrip/apply
  (asserted via `ctx.theme()`), flock second-acquire rejected + re-acquire after release,
  `open_db` idempotent with schema ≥ v40 and correct file path; CI run green.

**18. `DbHandle` facade over `Arc<Mutex<Connection>>` (D3 seam)**
- Work: `core::db::DbHandle` — a `Clone` facade over `Arc<Mutex<Connection>>` offering
  `with_read`/`with_write` closure access (the shape the post-cutover pool will provide),
  a poison-recovering `lock` mirroring `http::server::lock_or_recover`, `from_shared` to
  adopt the HTTP `AppState` Arc during the side-by-side port period, and `file_path`. All
  app-shell code binds to `DbHandle` exclusively, never to the raw Arc — per D3, ported UI
  code must not change when the pool lands.
- Verification: 8 tests — with_read/with_write roundtrips, transaction inside with_write,
  clones share the connection, from_shared adoption, in-memory → no path / file-backed →
  path, mutex-poison recovery via `catch_unwind`.

**19. `core::richtext`: HTML → styled spans, fixture tests**
- Work: new core module converts possibly-untrusted HTML into toolkit-agnostic spans.
  Pipeline: ammonia sanitize (converter's tag whitelist; scripty tags removed with
  content; https/http/mailto hrefs only; no link_rel injection) → quick-xml 0.42 event
  parser with a style snapshot stack. `SpanStyle` covers bold, italic, underline,
  strikethrough, code, quote, heading level, and link href. Emission policy: newlines live
  only in plain default-styled separator spans (`"\n"` / `"\n\n"`), never inside styled
  runs; whitespace collapses to single spaces outside `<pre>`; `<ul>/<ol>` items get
  bullet/number prefixes; entities (incl. numeric and `&nbsp;`) are decoded; unknown tags
  degrade to plain text; `javascript:` hrefs are stripped by the sanitizer.
- Verification: 18 fixture tests — nesting, entity decoding, link href with `&amp;`
  decoded, javascript-scheme stripping, script content dropped, heading levels, `pre`
  whitespace preservation, inline code, unordered/ordered lists, blockquote, block-boundary
  whitespace collapse, unknown-tag fallback, br handling, empty/whitespace-only input.

**20. Background task bridge → `request_repaint`**
- Work: `app-shell`'s `TaskBridge` runs a dedicated OS thread that blocks on the core
  `EventBus` broadcast receiver (`blocking_recv`; `Lagged` logs a warning and continues,
  `Closed` exits), pushes each event into a poison-recovering queue, and then fires an
  injected repaint callback — production wiring is `egui::Context::request_repaint`.
  Push-before-repaint ordering guarantees a woken frame observes the triggering event.
  `ShellApp::logic` drains the queue once per frame — and eframe also calls `logic` while
  the window is hidden whenever a repaint was requested, which is exactly when background
  events matter. This is the native mapping of the old SSE/EventBus/CORS layer
  (migration map row 10).
- Verification: 3 tests — events flow with one repaint callback per event
  (counter-injected callback, deterministic), bridge survives a capacity-1 burst (exercises
  the `Lagged` path) and still delivers afterwards, and the production `spawn` with a real
  `egui::Context` delivers end-to-end.

**Batch verification summary:** `cargo fmt --all --check` clean · `cargo clippy --all-targets
-- -D warnings` clean (catalog/core/xtask/app-shell) · `cargo test` 105/105 (94 core + 11
app-shell) · `cargo deny check advisories licenses` ok (OFL-1.1 and Ubuntu-font-1.0 added
for epaint's bundled fonts) · `xtask vector-count` smoke ok · GitHub Actions run
`39132228740` → success on `8875597`.

### Batch 5 — 2026-10-18 (commit `a1b4e31`, CI run `39139669523`: success)

**21. Port: dashboard page**
- Work: `app-shell/src/pages/dashboard.rs` — typed port of the `/api/analytics/dashboard`
  SQL (same predicates, same `round_min` rounding, no JSON layer, no HTTP hop): headline
  counts (new/active/pending/closed/unassigned/backlog), replies_sent, first-response and
  resolution averages, ratings split, by_mailbox/by_tag/by_agent/by_team breakdowns,
  daily_new series, by_channel, channel_metrics, and the mailbox comparison table. Reads
  are pure — the HTTP route's `daily_metrics` write-on-read cache upsert is deliberately
  not reproduced in the native page. View keeps the web information architecture: 7/30/90/
  365-day range buttons, mailbox chips (All + per-mailbox toggle), All/Email/Chat channel
  filter, metric cards, per-day line chart, breakdown grids, and the issue-radar panel via
  the promoted `issue_radar_impl`.
- Verification: 5 tests — counts/breakdowns/daily series over a 4-conversation fixture
  (incl. replies_sent=1, rating split, by_mailbox ordering, first-response Some), mailbox
  and channel scope filters (by_channel intentionally ignores the channel filter, matching
  the route), daily_new chart-point mapping, radar on empty store, mailbox listing.

**22. Port: inbox page (mention widget, richtext, live updates)**
- Work: `app-shell/src/pages/inbox.rs` + `app-shell/src/mention.rs`. List and detail read
  through the typed core functions the `/api/conversations` routes call
  (`inbox::list_conversations` / `get_conversation`), so the native page shows exactly what
  the web page showed. Thread bodies render through `richtext::html_to_spans` with full
  style mapping (bold/italic/underline/strikethrough/code/quote/heading sizes/links).
  Composer keeps the web features: reply/internal-note tabs, cc/bcc fields, Ctrl+Enter
  send, and the `@`-mention autocomplete popup (`caret_query`/`filter_matches`/
  `apply_completion` ported with byte-offset semantics + egui char-index conversion).
  Mutations are local-first through the durable outbound pipeline: the same
  `outbound_jobs` rows `op_send_reply`/`op_add_note` create (kinds `create_reply`/
  `create_note`/`update_status`/`assign`, the `reply:{remote}:{send}:{sha256-24}`
  duplicate-send key, audit entries, `queued` status — provider execution lands with the
  loopback/worker batch), local row changes via the typed `ticket_ops::execute` for
  status/assign, notes insert the local thread row (what sync brings back) plus
  `Mentioned` notifications (the same records the notification sweep creates, dedup
  `n:mention:{thread}:{user}`), AI-evaluation-mode guards reject every mutation, and
  every success emits `ConversationUpdated` on the bus for the live-update loop.
- Verification: 7 tests — filter-bar mapping, reply-key format contract, reply job +
  duplicate-send protection + distinct-text retry, note thread row + job + mention
  notification + bus event, status change writes locally and queues `update_status`,
  eval-mode blocks everything (zero jobs created), users/tags listing.

**23. Port: customers page**
- Work: `app-shell/src/pages/customers.rs` — search surface (query + Enter/button, results
  grid) and profile surface (back nav, contact card, support health, event-mix bars,
  interaction profile, timeline, conversations) fed by the same core functions the
  `/api/customers` routes call: `people_store::list_customers_summary` /
  `get_customer_detail` / `customer_support_health`, `customer_events::list_for_customer` +
  `kind_counts`, `interaction_engine::build_profile`, and `inbox::list_conversations` with
  a customer filter. View models (`row_from_summary`, `contact_from_detail` (nested
  `customer` object), `health_from_report`, `event_row`) map the actual JSON shapes.
- Verification: 6 tests — summary row mapping, contact/health extraction from the real
  detail/report shapes, event-row fallbacks, per-customer conversation filter, search +
  profile flow through core against the seeded DB, empty-DB search stays empty without
  error.

**24. Port: knowledge/docs pages**
- Work: `app-shell/src/pages/knowledge.rs` — four tabs. Documents/sources/freshness act
  through `spp_core::knowledge_store` (`list_documents`, `list_sources`,
  `freshness_report`, `mark_reviewed`, `mark_verified`, `delete_document`). The help-center
  tab ports the `/api/docs/*` route SQL as typed queries: stats, collections, article list
  with collection/status filters, FTS search building the same quoted-prefix pattern from
  the (now public) core tokenizer, and the article reader with full text. Deferred-action
  pattern keeps immediate-mode borrows sound.
- Verification: 6 tests — stats counting, collection/status filters, FTS search finds by
  token and returns empty (not error) for no-match and too-short tokens, article detail
  (summary + text, missing id), knowledge document actions round-trip (upsert → review →
  delete with list refresh).

**25. Port: reports page**
- Work: `app-shell/src/pages/reports.rs` — overview/SLA/why-contacting/top-questions/
  issue-radar tabs over the same data the `/api/reports/*` routes serve: overview from the
  dashboard port, typed `sla::sla_report` (per-mailbox first-response/resolution met/
  missed, waiting stats, unconfigured list), and the promoted `why_contacting_impl` /
  `top_questions_impl` / `issue_radar_impl`. Charts (per-day line, per-mailbox SLA bars)
  render through the new `app-shell/src/chart.rs` native painter with tested scaling math —
  egui_plot 0.37 builds against egui 0.36 and no 0.37-compatible release exists yet, so
  the workspace `egui_plot =0.37` pin was dropped with a re-adoption note in the workspace
  manifest (single-egui-version integrity preserved).
- Verification: 4 tests (reports) + 3 (chart) + 5 (mention) + 4 (pages/mod invalidation
  map) + 5 (app.rs routing) — SLA report loads on a fixture, overview/lists load and
  reload on range change, row-shape mapping, chart scaling (bottom-left origin, degenerate
  inputs, descending series), invalidation dedup across overlapping events.

**Batch verification summary:** `cargo fmt --all --check` clean · `cargo clippy --all-targets
-- -D warnings` clean (catalog/core/xtask/app-shell, forced fresh) · `cargo test` 144/144
(94 core + 50 app-shell; 39 new this batch) · `cargo deny check advisories licenses` ok ·
`xtask vector-count` smoke ok (D4: 0 points, sqlite-vec GO unchanged) · GitHub Actions run
`39139669523` → success on `a1b4e31`.

### Batch 6 — 2026-10-19 (commit `8e7ea0c`, CI run `39168057253`: success)

**26. Port: operations page**
- Work: `app-shell/src/pages/operations.rs` — tiles tab renders the 16 operations tiles
  from `spp_core::operations::snapshot` (the `/api/operations/center` SQL) grouped by
  severity band; clicking a conversation tile raises `NavRequest::InboxTile`, which the
  shell resolves by switching to the inbox with the tile's real `tile_fragment` as an
  `ops` filter (the web tile links produced `c.status = 'unassigned'` dead-end queries —
  the port wires the same fragment the `/api/conversations?ops=` path compiles, incl.
  the waiting-threshold binding and unknown-key rejection). Workload tab ports
  `workload::snapshot` (agents, capacity, pressure) with functional per-agent capacity
  +/− (the web buttons were no-ops) persisted through `workload::set_capacity_model`,
  plus suggested assignees. `DbHandle::shared()` added for engine adoption.
- Verification: 5 tests — snapshot→tile views with unassigned counts, severity grouping
  (FailedJobs/Urgent warnings, Unassigned info, SlaBreached the only critical), workload
  reload + capacity round-trip, drill-tile set, 16-key metadata coverage; the inbox ops
  filter has its own SQL drill-down test (only unassigned match; SQL-injection key
  rejected).

**27. Port: remaining 17 pages**
- Work: 16 new page models + views in `app-shell/src/pages/`: ai_center (settings status
  + background LM-Studio test, analytics SQL, attribute catalog + distributions, ai_runs
  table), automation (rules CRUD with trigger/action validation + pending approvals with
  approve/reject), connectors (list/rows/create/delete + background refresh), custom
  objects (type builder + object CRUD against the typed core API), incidents (list
  filters, create validation, detail workspace: conversations/customers/notes/releases,
  link + note), issue_radar (issue_radar alerts + SLA alerts, clusters, known-issue
  create with link/refresh/FTS), notifications (all/mentions/unread tabs, mark-read
  toggles, mark-all), onboarding (status route logic + demo-mode enable + engine-backed
  initial sync), organizations (list/filter/create + detail), outreach (campaign create
  with segment snapshot + queue/pause/resume/cancel/retry + report, v2 segments, DNC),
  search (FTS scopes), settings (general settings round-trip, Help Scout status +
  disconnect, LM Studio, Qdrant, business hours per mailbox, backups + JSON/CSV export),
  support_graph (graph_search + neighbors), sync_health (status SQL port, engine-backed
  sync actions, demo tools). not_found needs no model (the sidebar can't 404).
- Verification: 49 new page tests across the 16 models (round-trips, validation
  rejections, filter narrowing, seeding→reload flows). Fixed a real core bug found by
  the outreach tests: `save_segment_v2` never wrote the NOT NULL `criteria_json`
  column, so every v2 segment insert failed — now writes the legacy `'[]'` placeholder.
- Work (support): `testsupport.rs` seeds `customer_emails` rows (the segment engine and
  outreach validation read them) and maps assignee 0 → NULL (unassigned conversations).

**28. Port: command palette, overlays, toasts**
- Work: `app-shell/src/palette.rs` — Ctrl/Cmd+K overlay running the local search engine
  per keystroke (the web's 200ms debounce + generation racing is unnecessary in-process),
  arrow-key selection with `move_selection` clamping, `MAX_HITS = 12` cap, Enter/click
  resolving the web href contract (`/inbox/conversation/{id}`, `/customers/{id}`,
  `/knowledge`, `/issues`) into `NavRequest`. `app-shell/src/toasts.rs` — the toast stack
  with the web's per-kind timeouts (success/info 4s, warning 6s, error 10s) and expiry
  on the render clock. `app-shell/src/overlays.rs` — `ConfirmDialog` port with
  `DialogOutcome {Open, Confirmed, Cancelled}` (Escape/close/cancel never confirm).
- Verification: 11 tests — hit capping, selection clamping, href→page mapping (all four
  web href shapes + rejects), palette search against seeded FTS data, hit JSON parsing,
  toast push/expiry/kind timeouts/wire names, dialog builders and outcome lifecycle.

**29. Event bus → repaint wiring across all ported pages**
- Work: `pages::invalidated_pages` extended to the full web `keys_for_event` map:
  sync → inbox/dashboard/operations/sync-health/notifications (+knowledge on incremental
  rows), conversation → inbox/dashboard/operations/notifications, notification →
  inbox/notifications/operations, campaign → outreach, ratings → dashboard/customers.
  `ShellApp::apply_invalidations` routes each key to its page's `invalidate()`;
  `PageKey` grew to 21 entries with the web nav order; `ShellApp` wires every page,
  the shared `SyncEngine` (built at boot via `build_sync_engine`, demo/real provider
  choice mirroring the Tauri boot), the palette, toasts, and dialog slot.
- Verification: app.rs tests — dedup across a two-event batch, 21 labels, campaign →
  outreach exactly, notification fan-out coverage; the bridge tests already pin the
  EventBus → repaint wake path.

**30. Cutover: delete axum router + Leptos + Tauri + wasm32 target + CSP plumbing**
- Work: deleted `crates/core/src/http/` (axum router, routes, server, rate limiter),
  `crates/ui/` (Leptos), `crates/app/src-tauri` (Tauri 2 + CSP conf), and the
  wasm32-unknown-unknown target from `rust-toolchain.toml`. Promoted the survivors into
  core modules: `analytics.rs` (why_contacting/top_questions/issue_radar impls + the
  radar engine, ~690 lines), `eventing.rs` (EventBus — all 40+ `crate::http::EventBus`
  call sites re-pointed), `ai_memory_guard.rs` (the memory quarantine word lists and
  matcher, + new tests). `conversation_ops.rs` trimmed to its pure SQL operations
  (attachment persistence, outbound jobs, tags, eval-mode guards — the axum Response
  wrappers and AppState handlers deleted). CI now runs `--workspace` clippy/test;
  xtask `dev`/`package`/`verify-icons` re-pointed to the app-shell (icons preserved
  under `crates/app-shell/icons/`); workspace `axum`/`tower-http` deps and the ui/app
  members removed; `cargo update` pruned the lockfile (−68KB); README rewritten for the
  egui architecture. axum survives only transitively via qdrant-edge/tonic.
- Verification: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --
  -D warnings`, and `cargo test --workspace` (227 tests: 131 app-shell + 96 core) all
  green locally; CI run `39168057253` on `8e7ea0c` green with the workspace-wide steps
  (fmt, clippy, test, cargo-deny advisories+licenses).
### Batch 7 — 2026-10-19 (commit `790a56f`, CI run `39174410019`: success)

**31. Create minimal loopback listener (webhook + OAuth callback only)**
- Work: `crates/core/src/loopback.rs` — a 127.0.0.1-only hand-rolled HTTP/1.1 server with
  exactly two routes. `POST /webhook` resolves the secret (env `HELPSCOUT_WEBHOOK_SECRET`
  → encrypted envelope) and feeds `webhook_handler::process_webhook`, mapping the results
  onto the old axum route's status codes (200/200-duplicate/401/403/400). `GET
  /oauth/callback` ports the old route's flow onto the single-use `oauth_states` table
  (state consumed exactly once, code exchange + `get_me`, `me_remote_id` + audit
  persistence, success/fail HTML pages with the original copy). `begin_oauth` issues a
  state and returns the authorize URL; the Settings page gained a "connect via OAuth"
  entry showing it as a hyperlink. Boot order in `main.rs`: port from the localhost
  redirect URI, then `SPP_LOOPBACK_PORT`, then OS-assigned (logged); the listener degrades
  to a warning without blocking the shell. Requests are size-capped (16 KiB headers,
  1 MiB body) and served on per-connection threads so the OAuth exchange cannot head-of-
  line-block webhook bursts.
- Verification: 13 loopback tests — route dispatch (404/405/healthz/root), webhook status
  mapping incl. duplicate no-reenqueue, 401/403/400 paths with a stored secret, OAuth
  demo/error/missing-param/state-mismatch/replay/success/exchange-failure pages,
  `me_remote_id` + audit assertions, redirect-URI port extraction, `begin_oauth`
  single-use state. CI run `39174410019` green.

**32. AppImage packaging via `xtask package`; drop deb/msi/nsis/dmg/app targets**
- Work: `crates/xtask/src/package.rs` — `stage_appdir` builds the AppDir layout (release
  binary at `usr/bin`, `.desktop` entry at the root and under `usr/share/applications`,
  hicolor icons 32/128/256/1024 from the checked-in set, root icon + `.DirIcon`, `AppRun`
  sh shim); `package` drives the release build, staging, and `appimagetool` (resolution:
  `APPIMAGETOOL` env/flag → `target/tools/` cache → download from the official continuous
  release), run with `ARCH=x86_64 APPIMAGE_EXTRACT_AND_RUN=1 NO_STRIP=1` so FUSE-less
  builders work. The `xtask package` CLI gained `--out`, `--appimagetool`, `--skip-build`
  and cargo arg passthrough. deb/msi/nsis/dmg/app targets remain deleted with the Tauri
  tree; README documents the AppImage as the only desktop packaging target.
- Verification: 2 stage tests (full layout, executable AppRun, desktop content, root
  entry; clear failure without the release binary) + end-to-end run in this environment:
  `cargo xtask package` produced `target/appimage/SupportOs-Plus-x86_64.AppImage` (10.4 MB
  type-2 AppImage) and `--appimage-extract` verified the squashfs layout (AppRun, desktop,
  icons, `usr/bin/supportos-plusplus`). CI run `39174410019` green.

**33. Pin final toolchain; drop wasm32 target (row 15 complete)**
- Work: `rust-toolchain.toml` stays the single source of truth (channel `1.99.1`, no
  wasm32 target — removed with the Phase 4 cutover; CI installs the toolchain from the
  file). Workspace `rust-version` aligned from `1.85` to `1.99` (MSRV = the pin; older
  compilers are never tested). Dead `supportos-plusplus-ui` workspace path dependency
  removed (the crates/ui directory is gone). `deny.toml` advisory ignore list pruned: the
  bincode/paste/proc-macro-error/serde_cbor entries left with the leptos/tauri/qdrant
  trees; the ttf-parser/zvariant/zbus_names/zvariant_derive entries kept with corrected
  provenance (epaint / accesskit_unix).
- Verification: repo-wide grep finds no wasm32/toolchain leftovers (CI workflow, xtask,
  README); `cargo build`/`test`/`clippy` all run under the pinned toolchain; lockfile
  pruned of the stale trees. CI run `39174410019` green (toolchain install from
  rust-toolchain.toml + all steps).

**34. DB pool inside `DbHandle`: writer thread + 2 WAL readers; migrate remaining lock sites**
- Work: `crates/core/src/db.rs` — `DbHandle` now holds a lane: file-backed databases get
  an exclusive writer connection plus 2 WAL reader connections on the same file; in-memory
  databases (tests, tooling) keep the single shared connection (each `:memory:` DB is
  private, so pooling would see nothing — the reader pool degrades gracefully when a
  reader cannot be opened). `with_read` takes an idle reader (try-lock sweep, then FIFO
  blocking), `with_write` takes the writer lane, and the new `write_guard()` returns a
  Deref/DerefMut RAII guard that replaces the old `lock()` for scoped access. The "writer
  thread" is realized as the writer lane: SQLite WAL already serializes writers, and an OS
  writer thread would force `'static` bounds on every write closure (hundreds of sites)
  for no additional concurrency — documented in the module. All remaining raw lock sites
  migrated to `DbHandle`: `SyncEngine`, `RealHelpScoutProvider`, `HsRateLimiter`,
  `WorkerManager`/`start_workers`, `connectors::refresh`, `outreach::send_batch`, plus the
  four app-shell `.shared()` call sites; `DbHandle::lock`/`from_shared`/`shared` deleted.
- Verification: db.rs tests grew to 10 — pool sizing (2 readers file-backed, 0 in-memory),
  readers see committed writer state, concurrent second read while a reader is held, poison
  recovery, DerefMut transactions through the guard. Full suite green: 253 tests (131
  app-shell + 120 core + 2 xtask), fmt clean, clippy `--workspace --all-targets -D
  warnings` clean. CI run `39174410019` green.

**35. sqlite-vec swap behind `VectorStore` (per D4 gate outcome); remove qdrant-edge if GO**
- Work: D4 gate confirmed GO (0 chunks < 100k) — `vectorstore_qdrant.rs` deleted and the
  `qdrant-edge` dependency removed from the workspace (tonic/prost trees pruned from the
  lockfile). New `crates/core/src/vectorstore_sqlitevec.rs`: `SqliteVecStore` implements
  the full `VectorStore` trait over `vec0` virtual tables (`point_id INTEGER PRIMARY KEY,
  embedding FLOAT[dim] distance_metric=cosine`) in `<data_dir>/vectors/vectors.db`, with a
  `vec_meta` dimension pin (dim change = explicit rebuild + re-embed), a `vec_payload`
  companion table (JSON payload + `entity_type`/`entity_id` columns for the delete/count
  pipelines), KNN search with over-fetch + payload post-filter, snapshot/restore in the
  same wire format as the qdrant adapter, and sparse search pinned to the FTS5 lane by
  design. `VectorIndex` keeps the facade surface the worker pipeline consumed (lazy open,
  enable flag, `health`/`try_point_count`/`ensure_collection`/`upsert`/`search`/
  `delete_by_entity`/`count_by_entity`). The sqlite-vec extension registers via
  `sqlite3_auto_extension` (the only unsafe code in spp_core; crate lint downgraded
  forbid→deny for that one module). `xtask vector-count` re-pointed at the new store
  (the `--url` flag dropped); the Settings "Qdrant" tab became the sqlite-vec store info
  page.
- Verification: 11 sqlite-vec tests — vec_version availability, upsert/delete/count
  roundtrip with idempotent re-upsert, dimension mismatch error + explicit rebuild, KNN
  cosine ordering, payload post-filter, sparse-empty pin, snapshot/restore roundtrip,
  delete_by_entity both halves, generic Filter count scan, facade disabled/enabled health
  + full pipeline. CI run `39174410019` green (incl. cargo-deny advisories+licenses for
  the new sqlite-vec dependency).

### Final batch (8) — 2026-10-19 (commit `04567cf`, CI run `39179188225`: success)

**36. Wire WorkerManager into the shell boot (user-directed cutover-gap fix)**
- Work: new `crates/app-shell/src/workers_host.rs` — `WorkersHost` spawns a dedicated OS
  thread hosting a fixed 2-worker-thread multi-thread tokio runtime that owns the core
  `WorkerManager` (job drain + stale-job recovery, webhook drain, embedding passes,
  maintenance/backup, notification/customer-event sweeps, connector refresh, ratings) for
  the whole process lifetime; `main.rs` starts it after boot services with the shared
  DbHandle, sync engine, provider, event bus, data dir and a boot-built `VectorIndex`
  (enabled from the `qdrant_enabled` setting, store lazily opened — the embedding timers
  were no-ops with `None` before). `restart()` re-arms timers on the worker runtime via a
  captured `Handle` (`WorkerManager::start` needs a runtime context; the settings page
  runs on the UI thread); the Settings general save calls it when `sync_interval_minutes`
  changed (the old `PUT /api/settings` stop/start behavior). Drop = stop + join; the
  host lives in `main` (not `ShellApp`) so page tests stay timer-free.
- Verification: 2 `workers_host` tests (start/drop stop + restart re-arm, incl. the
  caught-and-fixed no-reactor bug); local debug + packaged AppImage smoke runs log
  "Background workers started" at boot and "Background workers stopped" at exit; CI green.

**37. tokio feature slim + 2-thread runtime (row 12)**
- Work: workspace `tokio` features `full` → `rt, rt-multi-thread, time, sync` (exactly the
  port's direct call sites: spawn/spawn_blocking/runtime builders, the 2-thread worker
  host, interval/sleep timers, broadcast/oneshot channels); net/io still arrive through
  reqwest's own dependency via feature unification; fs/process/signal have zero call
  sites. Rationale documented in Cargo.toml and decisions.md.
- Verification: `cargo check/clippy/test --workspace` green on the slimmed feature set
  (255 tests); CI `check` job green.

**38. Landlock + seccomp sandbox for pdf/zip parsing (Linux, feature-flagged)**
- Work: new `crates/core/src/sandbox.rs` (compiles only on Linux with
  `--features sandbox`): `run_sandboxed(label, f)` runs a closure on a fresh thread that
  first applies a Landlock deny-all ruleset (all V1 read/write path rights handled, zero
  rules; `CompatLevel::BestEffort` so pre-5.13 kernels degrade with a logged status) and
  then a seccomp BPF allow-list (read, close, brk, mmap/munmap/mprotect/madvise/mremap,
  futex, clock/nanosleep, rt_sig*, sigaltstack, getpid/gettid, sched_*, getrandom,
  seccomp, exit/exit_group; `write` pinned to fds 1/2 via arg conditions). Mismatch
  action is `EPERM` by design — a killed thread cannot notify its channel (caller would
  hang to the timeout) while EPERM still blocks every escape path, and filters only ever
  stack. Panics inside the sandbox surface as ordinary parse errors; a 120 s bounded
  receive guards against wedges. `knowledge_store::import_one_file` routes the pdf and
  docx parse arms through `sandbox_parse` (byte caps unchanged and still pre-parse);
  without the feature the parse runs inline exactly as before.
- Verification: 5 sandbox tests (pure computation, panic surfacing, Landlock FS denial —
  degrades to a no-op note on this 5.10 kernel, socket blocked by the allow-list,
  docx+pdf parse end-to-end under the filter) + full core suite with the feature on
  (125 tests). CI adds explicit sandbox clippy + test steps on an
  Landlock-enforcing ubuntu-latest kernel; run `39179188225` green. New deps (landlock
  0.4.7, seccompiler 0.5.0) pass `cargo deny` licenses.

**39. CI: AppImage build + smoke-test job**
- Work: `.github/workflows/ci.yml` gains an `appimage` job: pinned toolchain, rust-cache,
  `cargo run -p supportos-plusplus-xtask -- package` (release build + AppDir staging +
  type-2 AppImage, appimagetool downloaded/cached by the xtask), then a smoke step
  running `APPIMAGE_EXTRACT_AND_RUN=1 SPP_SMOKE=1 ./target/appimage/
  SupportOs-Plus-x86_64.AppImage`. The new `SPP_SMOKE=1` hook in `main.rs` (CI-only,
  never active in normal runs) verifies the full boot chain headless — logging,
  single-instance lock, all boot migrations, boot services, background workers, the
  loopback listener — then exits 0 without opening a window; the AppImage artifact is
  uploaded via actions/upload-artifact@v4.
- Verification: locally built the 10.8 MB type-2 AppImage and ran the exact smoke command
  (exit 0, boot chain logged, workers started/stopped cleanly); CI job
  `AppImage build + smoke test` conclusion `success` in run `39179188225`.

The plan is now exhausted: 39 of 39 items completed across 8 batches (7 × five + this
final four-item batch: three planned items plus the user-directed WorkerManager boot
gap). `plan.md` is empty, `progress.md` is empty.

### Batch 9 (audit fixes) — 2026-10-19 (items 40-44, from the read-only audit)

**40. Enable the `sandbox` feature in the shipped AppImage build (audit P1-1)**
- Work (two halves — the audit fix exposed a deeper gap): (a) `xtask package` now passes
  `--features supportos-plusplus-core/sandbox` to the release build (Linux-gated),
  `crates/xtask/src/package.rs:56-62`; (b) the audit's strings test showed LTO stripped
  the sandbox right back out because **the knowledge file-import UI was never ported** —
  `import_file` had zero callers app-wide. Ported the web `ImportKnowledgeModal` as an
  "Import documents" collapsible panel on the knowledge page
  (`pages/knowledge.rs`): watch-folder listing (`importable_files`) with per-file Import
  buttons, paste-import form (title/visibility/source/content with route-parity
  validation: `internal_only`/`customer_safe`, blank-source → "Manual import"), and the
  route's side effects (embeddings job enqueue + `knowledge_imported` audit row).
  `KnowledgePage::set_data_dir` mirrors the connectors/settings setter pattern.
- Verification: 5 new tests (file roundtrip incl. job+audit rows, path-escape rejection,
  manual validation/roundtrip, listing filter, page-state updates) — 13/13 knowledge
  tests, 138/138 app-shell, 260/260 workspace, 125/125 with `--features sandbox`; fmt +
  clippy `-D warnings` clean. Shipped-binary proof: 0 → 15 sandbox strings in the release
  binary and the AppImage's inner binary ("landlock layer applied", "seccomp layer
  applied (compute allow-list, EPERM on mismatch)", "spp-sandbox-"). AppImage smoke
  exit 0. **Runtime proof** (real GUI, Xvfb + llvmpipe, xdotool-driven): placed a
  hand-built `sample.pdf` in `<data_dir>/knowledge-import/`, clicked Knowledge → Import
  documents → Import; the log shows `landlock layer applied` (status "not enforced
  (kernel without Landlock)" — correct BestEffort degradation on this 5.10 kernel) and
  `seccomp layer applied`, the PDF parsed inside the restricted thread, the document
  landed in `knowledge_documents` (`sample`/`internal_only`), the audit row was written,
  and the background worker drained the enqueued `embed_knowledge_chunks` job
  (completed). Screenshot evidence: green "Imported 1 document(s) from file." in the UI.
- CI: the `appimage` job builds through the same xtask → the shipped artifact now
  compiles the sandbox in by construction.

**41. `Cargo.toml` repository/homepage URLs (audit P2-3)**
- Work: workspace `repository`/`homepage` corrected from
  `github.com/kimpearce888/SupportOS-PlusPlus` to `.../SupportOs-Plus` (the real remote).
- Verification: `cargo metadata --no-deps` parses cleanly and all four crates resolve the
  new URL; repo-wide grep finds zero remaining stale references in code/manifests/CI.

**42. README runtime-dependency honesty (audit P2-4)**
- Work: the Building section now states the actual runtime needs (X11/Wayland session,
  `libxkbcommon-x11`, a GL or Vulkan implementation — Mesa, with llvmpipe/lavapipe
  software rendering confirmed working) while keeping the true claim of no build-time
  system packages; the hardening bullet now documents the Landlock+seccomp sandbox being
  compiled into the shipped AppImage with its degradation behavior.
- Verification: every claim in the new text was proven empirically during the audit (the
  binary panicked without `libxkbcommon-x11`/`libxcb-xkb`; wgpu failed until Mesa; the
  full UI rendered under llvmpipe) and re-proven in this batch's GUI import run.

**43. Remove the dead `Error::NotImplemented` variant (audit P2-5)**
- Work: variant deleted from `crates/core/src/error.rs` (defined, never constructed).
- Verification: `rg NotImplemented` → 0 references in the workspace; clippy
  `-D warnings` clean; 120/120 core tests pass.

**44. Align `decisions.md` "all 24 pages" wording (audit P2-6)**
- Work: goal 3 now states the real mapping — every file of the original's 24-page tree is
  covered: 20 egui pages + the command palette overlay (`palette.rs`) + the native Shell
  page (21 `PageKey`s), with `docs.rs` folded into the knowledge help-center tab and
  `not_found.rs` unreachable natively. The historical stack table (line 35, facts about
  the original) is untouched.
- Verification: counted from the tree — `PageKey` has 21 variants, 20 page module files,
  and the mapping matches the original's `ui/src/pages` listing at `286edd4`.

**Batch verification summary:** `cargo fmt --all --check` clean · clippy
`--workspace --all-targets -- -D warnings` clean · `cargo test --workspace` 260/260
(138 app-shell incl. 5 new import tests + 120 core + 2 xtask) · core `--features sandbox`
125/125 · final AppImage rebuilt via the updated xtask (11.4 MB, 15 sandbox strings,
"Import documents" present, smoke exit 0) · startup baseline: ~10 ms boot chain, 40–100 ms
AppImage self-extract+boot · CI run `39187268977` on `810dd3e`: **both jobs success**
(fmt/clippy/test/sandbox/deny and AppImage build + smoke test, every step green).

Items 45 (real-service verification — blocked on user-provided Help Scout credentials)
and 46 (optional performance pass) remain in `plan.md` awaiting user input; they were
never silently dropped.

### Batch 10 (items 45-46) — 2026-10-19

**45. Integration verification pass — local mock-server suite (Help Scout, LM Studio)**
- Re-scoping (user-directed, 2026-10-19): no user-provided credentials, no production
  credentials, no mandatory cloud dependencies, no cloud fallbacks — comprehensive local
  integration tests against mock servers with generated throwaway credentials.
- Work: six `#[cfg(test)]`-only modules in `crates/core/src/` (nothing compiles into
  release builds): `mockserver.rs` (generic in-process HTTP/1.1 server, one thread per
  connection, bounded reads, per-request recording), `it_helpscout.rs` (24 tests),
  `it_sync.rs` (5), `it_loopback.rs` (5), `it_lm_studio.rs` (11 + 1 env-gated live),
  plus the item-46 bench harness. The tests run the **real client stacks end-to-end over
  TCP**: `RealHelpScoutProvider` (reqwest + ApiQueue + rate limiter + token persistence),
  `SyncEngine::initial_sync`/`incremental`/`sync_single_conversation`, the
  `LoopbackServer`, and `OpenAiCompatibleClient`.
- Two production fixes the new tests forced:
  1. `helpscout_real.rs` — the `oauth_tokens` save now supplies `id = 1` explicitly. The
     migration schema pins `id INTEGER PRIMARY KEY CHECK (id = 1)`; the old insert let
     SQLite auto-assign the rowid, so **every second save (i.e. every token refresh)
     failed with "CHECK constraint failed: id = 1"** — inherited from the original
     codebase, caught by `expired_token_refreshes_via_token_endpoint_and_retries`.
  2. `ai_lm_studio.rs` — non-2xx responses now surface `HTTP {status}` + a body snippet
     instead of an opaque JSON parse error (required by
     `server_error_surfaces_http_status_and_body`).
- Coverage mapping (all checklist items): API behavior —
  `conversations_threads_and_customers_roundtrip`, `create_conversation_and_thread_roundtrips`,
  `list_mailboxes_and_dependent_resources_roundtrip`, `get_me_roundtrip_sends_bearer_token`,
  `docs_api_*`; sync round-trip — `initial_sync_round_trips_the_full_fixture_into_the_mirror`,
  `initial_sync_is_idempotent_on_rerun`, `incremental_sync_picks_up_changed_conversations`,
  `sync_single_conversation_updates_and_soft_deletes`, `hard_upstream_failure_is_reported_not_swallowed`;
  pagination — v2 page-based (`v2_pagination_follows_page_numbers_until_total_pages`,
  `v2_pagination_stops_on_empty_page_safely`), v3 cursor (`v3_conversation_pagination_follows_hal_next_links`,
  `v3_customer_pagination_follows_next_cursor_field`, `system_users_follows_next_links`);
  errors — `error_statuses_map_to_friendly_non_retryable_errors`,
  `server_error_5xx_is_retried_and_then_recovers`, `invalid_json_from_success_response_is_reported`,
  `token_endpoint_rejection_is_friendly`, `unreachable_server_reports_offline_friendly_error`;
  rate limits — `rate_limit_429_is_honored_retried_and_recorded`,
  `rate_limit_headers_on_success_are_recorded`; webhook ingress + HMAC over real TCP —
  `signed_webhook_roundtrip_over_tcp`, `webhook_burst_is_served_concurrently`,
  `webhook_without_configured_secret_hard_rejects_outside_demo_mode`; OAuth
  authorization-code exchange end-to-end — `authorization_code_exchange_persists_refresh_token`,
  `client_credentials_login_exchanges_and_persists`,
  `expired_token_refreshes_via_token_endpoint_and_retries`, `refresh_token_grant_roundtrip`,
  `revoke_clears_the_token_and_blocks_requests`,
  `it_loopback::oauth_authorization_code_flow_over_tcp` (loopback callback →
  `exchange_code` → token persistence → `get_me`),
  `it_loopback::oauth_callback_reports_token_endpoint_rejection`; LM Studio —
  `list_models_and_role_inference_roundtrip`, `chat_roundtrip_against_mock_server`,
  `chat_with_opts_roundtrip_json_mode_and_usage`,
  `chat_with_tools_roundtrip_parses_tool_calls`, `embed_roundtrip_against_mock_server`,
  `availability_probe_reflects_server_state`,
  `connection_refused_reports_cleanly_and_client_recovers`, `malformed_success_body_is_a_parse_error`,
  `timeout_is_reported_and_does_not_leak_into_later_calls`.
- Verification: **44 of 44 mock-server integration tests pass** (24 it_helpscout + 11
  it_lm_studio + 5 it_sync + 5 it_loopback); `cargo test --workspace` 304/304; fmt +
  clippy `-D warnings` clean; **CI run `39198295485` on commit `5f2ca8d`: both jobs
  success**.
- Honest limitation statement (required by the item):
  - **Not performed — real Help Scout account verification**: requires external
    credentials (API token or OAuth client id/secret) that do not exist in this
    environment. The mock-server suite verifies the client stack's behavior against a
    faithful local mock, not the live `api.helpscout.net`. This remains the one thing
    only a user with credentials can test.
  - **Not performed — actual model inference**: probed `127.0.0.1:1234`, `localhost:1234`,
    `11434` (Ollama), `8080`, `5000`, `8000` — no local model server found.
    `it_lm_studio::live_roundtrip_when_server_provided` stays env-gated on
    `SPP_LM_STUDIO_URL` and is honestly reported as ignored; connection handling,
    error reporting, and recovery are verified against a mock OpenAI-compatible server.
  - No production credentials, no mandatory cloud dependencies, no cloud fallbacks were
    added: all fixtures and credentials are generated in-test.

**46. Performance pass**
- Work: the `#[ignore]`d measurement harness in `crates/core/src/bench.rs` (procedures
  documented in its module doc) plus this entry's recorded measurements. Two bench-harness
  bugs were found and fixed while making the harness actually runnable (both bench-only;
  the production provider is unaffected — verified against `helpscout_real.rs`'s HAL
  round-trip): (1) `LocalBenchProvider` parsed its own HAL-style `"/N"` next-cursor with a
  plain integer `parse()`, which fails and silently restarts at page 0 **forever** — the
  full bench never completed a single phase before this fix; (2) the seeding skipped the
  app's post-sync `rebuild_search_index` job step, so the FTS search phase found 0 hits
  and panicked. Also added seeding progress output and `bench_diagnose_write_path`
  (write-path isolation diagnostic).
- Reproducible procedures: `cargo test -p supportos-plusplus-core bench -- --ignored
  --nocapture` (full DB suite, ~1 min); `bench_boot_migrations_on_fresh_db`; startup and
  max-RSS measured on the smoke path (`SPP_SMOKE=1`, AppImage with
  `APPIMAGE_EXTRACT_AND_RUN=1`) via repeated wall-clock runs plus a getrusage-based peak
  RSS reader (`/usr/bin/time -v` where available; a python
  `resource.getrusage(RUSAGE_CHILDREN)` wrapper where not). Environment note: the
  container disk was 97% full when the first audit attempts ran; freed to ~80%
  (leaked test artifacts) before all measurements below.
- **Actual measurements** (dataset: 3 000 conversations × 3 threads, 5 000 customers,
  100 tags, 50 users, 3 mailboxes, 3 000 × 384-dim vectors; DB = 28.4 MiB + 9.7 MiB WAL;
  two full bench runs, both green, 58.99 s / 59.69 s total):
  - Startup: release binary 10–17 ms (smoke boot chain); AppImage self-extract + boot
    37–41 ms; `db::open` + all migrations 914 ms cold / 4.1–4.2 ms warm.
  - Memory (peak RSS): 15.2–15.8 MiB release-binary boot; 15.0–15.6 MiB AppImage boot;
    84.0 MiB during the full large-dataset bench.
  - DB operations: customers ingest 6 240–6 547/s (5 000 in 0.76–0.80 s); conversations
    + threads ingest 212–218/s (3 000 × 3 in 13.6–14.2 s); post-sync FTS rebuild
    9.4–9.7 s for 3 000 conversations; `interaction_current::backfill_all` 14.4–14.7 s;
    incremental re-walk (no changes) 19.7 s (153 conv/s).
  - Responsiveness (the query paths that bound UI latency; interactive GUI behavior was
    previously verified in batch 9's Xvfb drive): inbox list page 6.6 ms unfiltered /
    4.2–4.3 ms status filter / 17.5–18.1 ms tag filter / 15.7–17.1 ms deep page
    (offset 2000); conversation detail 188–210 µs; FTS search all scopes 98.6–102.6 ms
    (40 hits); vector KNN top-20 7.3–7.6 ms; `count_by_entity` 440–444 µs; vector upsert
    5 316–5 469 points/s.
  - Write-path diagnostic: raw INSERTs 60 608/s; `upsert_customer` direct 6 597/s;
    via `DbHandle::with_write` 6 680/s; guard per 50-row page 6 799/s — the DbHandle
    pool adds ~zero overhead.
- Confirmed bottlenecks found and fixed: the two bench-harness bugs above, plus (via
  item 45's tests) the production OAuth token-save `CHECK (id = 1)` failure and the
  LM Studio non-2xx error opacity. **No production DB bottleneck was confirmed at this
  scale**: PRAGMAs are already WAL + `synchronous=NORMAL` + 5 s busy timeout; the write
  path sustains 6.2 k customers/s; every read path is interactive (< 18 ms inbox pages,
  < 103 ms FTS, < 8 ms KNN). Conversation ingest at ~215/s is dominated by per-conversation
  thread fetching + tag joins — a background-sync cost, not a UI one (a 50k-conversation
  account's initial sync would take ≈ 4 min one-time; incremental syncs only re-walk
  changed rows).
- Verification: full bench green twice (all assertions: inbox < 5 s, FTS < 5 s,
  re-walk < 300 s, DB size sanity); boot-migration bench green; `cargo test --workspace`
  304/304 (+4 ignored: 2 bench measurements, 1 env-gated live LM Studio, 1 diagnostic);
  fmt + clippy `-D warnings` clean; AppImage smoke exit 0 (re-verified during the
  audit pass); **CI run `39200697607` on `fccbdfc` and run `39200713478` on `87a3d81`:
  both jobs success each**.

The plan is now fully exhausted: 46 of 46 items completed across 10 batches. `plan.md`
empty, `progress.md` empty. The one remaining untestable item is documented above under
item 45 (real Help Scout account verification requires external credentials).
