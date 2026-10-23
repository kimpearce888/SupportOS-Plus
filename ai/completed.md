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

### Batch 11 (Phase R, v1.4.0 items R1–R5) — 2026-10-21 (commits `c8de6e4`+`91479ef`, CI run `39508562660`: success)

First batch of the owner-approved v2 blueprint (plan.md phases R/A/K/P). Authored
2026-10-20 in the pre-reset rebuilt workspace (which had no compiler and no remote at
the time), then verified end-to-end on 2026-10-21 after the origin was restored —
per Rule 5 nothing below counted as done before this verification.

**R1. Connection Doctor; auth flow honesty (D3)**
- Work: new `crates/core/src/connection_doctor.rs` — five sequential checks (reach
  api.helpscout.net → client-credentials token exchange → `GET /v2/users/me` → list
  mailboxes → read rate-limit headers), each step reporting plain-language detail and,
  on failure, a concrete remedy; the doctor stops at the first hard failure so every
  rendered report is complete evidence of the chain up to the break. Settings runs it
  after every credential change; client credentials are promoted to the primary,
  recommended flow and the authorization-code browser flow is demoted to a
  clearly-labeled secondary with the exact-match redirect-URL warning.

**R2. Provider-kind guard and watermark integrity (D1)**
- Work: the auto-sync timer refuses to run against the empty provider; checkpoints
  and cursors never advance on empty-provider results; the new
  `sync_engine::on_provider_kind_transition` forces `sync_state` back to `NEW` on an
  Empty→Real transition so the full initial sync actually runs after credentials
  arrive — closing the "connected but nothing received" poisoning path for good.
- Tests: `workers::auto_sync_tests::auto_sync_skips_the_empty_provider_and_stamps_nothing`
  (a poisoned pre-1.4.0 `LIVE` state stays untouched: no run rows, no checkpoint
  stamps) and `sync_engine::tests::r2_transition_policy_resets_a_poisoned_empty_session`.

**R3. In-session credential activation (D2)**
- Work: new `crates/core/src/provider_handle.rs` — one shared hot-swappable provider
  cell (`RwLock`-guarded, kind-tagged Real/Empty) held for the whole process by the
  sync engine, the worker host and the loopback listener. Saving or clearing
  credentials swaps the inner provider in place; no `Arc` holder is rebuilt and no
  restart ritual is needed — the running engine sees the new provider on its next
  call, and the activation is observable in the UI.

**R4. Send honesty (D7, D8, D9)**
- Work: the reply dispatch resolves the customer remote id from the live conversation
  (`GET /v3/conversations/{id}` → `primaryCustomer.id`) and repairs the mirror row
  before `POST /v2/conversations/{id}/reply`; an enqueue failure now renders an error
  and preserves the composed text instead of reporting green success; the
  settings-backed `statusAfter` (default `pending`) rides on every reply send; a
  single-conversation refresh runs immediately after delivery.
- Test: `it_helpscout::reply_dispatch_resolves_the_customer_from_the_api_and_repairs_the_mirror`
  over the real reqwest stack and a mock TCP server — proves exactly one reply POST,
  with `customer.id` resolved onto the body and `status: pending` riding along; the
  mirror row is repaired and the job lands `confirmed`.

**R5. Error surfacing and complete invalidation (D4, D5, D15, D16, D19, D20)**
- Work: real `ErrorOccurred` emitters from sync, outbound, connect and worker failures
  (toast + Diagnostics); the page invalidation map extended to every page (11 were
  previously missing); zero-record sync completions render an explicit "up to date,
  nothing new" outcome; docs-sync missing-key degradation and inbox-detail error
  swallowing surface honestly.
- Tests: `pages::tests::r5_error_occurred_invalidates_diagnostics_and_the_service_surface`,
  `app::tests::r5_d20_zero_record_syncs_and_error_runs_have_visible_outcomes`.

**Verification (Rule 5)**
- The two new R2/R4 tests initially panicked with "there is no reactor running": they
  called `start_workers` (whose `start()` spawns its timers via `tokio::spawn`) before
  creating a runtime. Fixed by creating and entering the current-thread runtime
  before `start_workers` in both tests; both then pass.
- Local, pinned toolchain 1.99.1: `cargo fmt --all --check` clean; `cargo clippy
  --workspace --all-targets -- -D warnings` clean; `cargo test --workspace` green
  (163 app-shell + 203 core + xtask suites); `cargo clippy/test -p
  supportos-plus-core --features sandbox` clean with 208 tests green;
  `cargo-deny 0.20.2 check advisories licenses` ok (after the bump below).
- First main push (`c8de6e4`, CI run `39507409148`): fmt, clippy, workspace tests and
  the sandbox feature all passed on the runner, but the deny advisory gate failed on a
  brand-new RUSTSEC advisory against `rustls-webpki < 0.103.16` (trust-anchor path
  validation accepting intermediates whose private keys are now well known) that
  landed after the v1.3.0 release. Fixed with a lockfile-only
  `cargo update -p rustls-webpki` (`0.103.15` → `0.103.16`) as commit `91479ef`.
- **CI run `39508562660` on `91479ef`: success on both jobs** (fmt, clippy, test,
  deny + AppImage/.deb build with clean smoke boots). `main` is green.

### Batch 12 (R6, R7, R8, A1, A2) — 2026-10-21 (commits `a2c5309`+`943a122`, CI runs `39522823488`+`39523717163`: success; Release run `39523722592` on `v1.4.0`: success; verify_release: ALL CHECKS PASSED)

Phase R completion batch plus the first two Phase A items. v1.4.0 shipped
from this batch (Phase R, "Prove It").

**R6. Honest stubs (D11, D12)**
- Work: `INITIAL_SYNC_ORDER` drops the chats/attachments/ratings stages —
  Help Scout exposes **no list endpoints** for Beacon chats
  (`GET /v2/chats/{id}` only) or ratings (`GET /v2/ratings/{id}` only);
  both arrive via webhooks and are fetched by id (the satisfaction-webhook
  job was always the real ratings path and stays). The real provider's
  `list_ratings`/`list_beacon_chats` now return explicit platform-reality
  errors instead of silent empty lists; the `report_data` match arm is gone
  (unknown resources already error). Migration m052 deletes the stale
  checkpoint rows so Sync Health stops rendering stages that can never
  run. The misdesigned 30-second ratings poller is retired
  (`refresh_ratings`, its re-entrancy flag, `is_dangling_rating`, the
  `RatingsRefreshed` event chain and its test; `upsert_rating_row` stays —
  the webhook path uses it). Reports carries a one-line provenance note:
  ratings come from the satisfaction webhook; Help Scout's own happiness
  reports need a Plus/Pro plan and are not fetched. Note: P1's
  "30-second timer retired" clause is thereby delivered early; P1 retains
  the happiness-report path and CSAT rework.
- Tests: `helpscout_real::r6_honest_stub_tests` (both stubs must refuse
  with the platform reality); `it_sync` checkpoint assertion tightened to
  exactly `INITIAL_SYNC_ORDER.len()` (19).

**R7. Token encryption, residue purge, version transparency, no-simulation gate (D13, D14)**
- Work: `save_tokens` seals access/refresh tokens into the AES-256-GCM
  envelope (`helpscout_oauth_access_token`/`helpscout_oauth_refresh_token`);
  the `oauth_tokens` row keeps metadata only, columns permanently empty.
  Migration m053 (boot step 44) seals pre-1.4.0 plaintext tokens and
  scrubs the columns the first time a master key exists; `token_row` reads
  envelope-first with the plaintext columns as a transitional fallback
  (a missing/corrupt envelope surfaces as an error, never silence).
  `revoke` and the settings disconnect purge the envelope entirely.
  `require_auth` goes through the new envelope-backed
  `helpscout_real::has_stored_tokens`; the legacy `oauth.rs` module is
  deleted (its one caller migrated). Residue purged: the
  `FakeAiProvider`/`NoopAiProvider` demo doubles (zero callers), the
  "Simulated attachment content…" fabrication (the empty provider now
  refuses — the inert world has no bytes to give), the dead simulated
  rating handler (with R6), the legacy verified-draft and
  interaction-signal CRUD (zero callers; the tables stay as inert
  schema), the stale boot comment in `main.rs`, stale "demo mode"
  comments, and user-visible copy reworded ("seeded content", the
  onboarding honesty line). Version transparency: new
  `core::version_check` (semver-aware comparison with pre-release
  handling; strictly user-initiated release check with a 404-honest
  private-repo message) + the Settings "About & updates" card; the app
  never phones home on its own. The CI gate
  `scripts/gate_no_simulation.py` scans production code for
  simulated/fake/demo markers (comments and `#[cfg(test)]` regions
  excluded, explicit justified allowlist for the m050 purge machinery),
  self-tested to catch a planted violation, wired into both `ci.yml` and
  `release.yml`.
- Tests: `helpscout_real::r7_token_envelope_tests`
  (m053 seals-and-scrubs; without a key the legacy row stays intact);
  `it_helpscout`/`it_loopback` reworked to assert the encrypted shape
  (envelope holds the token, columns empty, disconnect/revocation purge)
  under the process-wide secrets lock; `version_check` comparison tests.

**R8. Unified refresh, adaptive cadence, freshness ring (D6, D10, D18)**
- Work: `spawn_sync_action` returns whether it started — a click during a
  running sync is answered everywhere (Inbox, Sync Health, Onboarding),
  never swallowed. The Dashboard/Reports/Customers/Organizations "Reload"
  buttons became real "Sync now" scoped fetches (engine handed to those
  pages; the completion toast reports the outcome; without an engine they
  degrade to a labeled local re-read). The fixed auto-sync interval is
  replaced by adaptive cadence: 30 s while changes arrive, doubling
  back-off toward the configured ceiling; `SYNC_INTERVAL_MINUTES` is
  finally consumed as the ceiling fallback and both divergent defaults
  are unified at 1 minute (the setting relabeled "max quiet sync
  interval" with the behavior on hover). Every successful send nudges the
  timer for an immediate poll (`WorkerManager::nudge_sync` +
  `tokio::sync::Notify`; no macros feature needed — `timeout` over
  `notified()`). The header gains the global freshness ring —
  "syncing…" / "synced Ns ago" / "never synced", tone-colored by age,
  clickable through to Sync Health — reading the new
  `sync_engine::last_sync_age_seconds`.
- Tests: `sync_engine::r8_freshness_tests` (never-synced vs stamped;
  the newest ok checkpoint wins).

**A1. Provider hub**
- Work: new `core::ai_hub` — endpoints as a list (seeded from the legacy
  `lmstudio_*` settings: local LM Studio + optional Ollama; user-added
  OpenAI-compatible cloud endpoints BYOK with keys in the encrypted
  envelope, referenced by key name only), the five model roles of
  blueprint Table 6 (Embed/Classify/Summarize/Draft/Agent) with
  per-role endpoint+model routing and a legacy single-model fallback,
  per-endpoint health probes (`list_models` → green/red with model count
  and latency), the feature registry (`ai_feature_enabled` under the
  global `ai_enabled` kill switch; analysis/note/draft keep their
  existing settings keys, attributes/memory/clustering/copilot are new
  AI Center toggles), and the cost ledger: `ChatOptsResult` now carries
  token usage → `complete_run_with_usage` persists
  `ai_runs.token_usage` + the new `endpoint_id` (m054) →
  `usage_ledger` aggregates tokens/latency/estimated spend per feature
  per day, priced by the endpoint's USD/mtok fields. All backend call
  sites route through `backend_for_role`: workers jobs (Analyze→Classify,
  Draft→Draft, Note→Summarize, each feature-gated at execution),
  automation (Classify), the composer availability check (Draft), and
  each AI job checks its own kill switch at execution, not just enqueue.
  AI Center renders the hub: endpoint rows with probe/set-key/remove,
  the add-endpoint BYOK form, the role-routing grid, the feature
  toggles + global kill switch, and the ledger table.
- Tests: `ai_hub::tests` ×4 — legacy seeding + role resolution with a
  BYOK key; endpoint removal clears its key and assignments; the kill
  switch dominates and per-feature defaults hold; the ledger prices a
  run by endpoint and leaves legacy rows at zero.

**A2. Wire batch 1**
- Work: the 14-key attribute extraction is live — job kind
  `extract_attributes` runs `ai_attributes::compute_with_backend`
  (deterministic records always land with no model configured; the AI
  layer fills intent/product/feature/issue/customer_goal/response_style
  through the Classify role), enqueued after every completed analysis
  (or directly on customer-thread arrival when analysis is off) and
  backfilled for up to 50 attribute-less conversations after each
  initial sync — the Attributes tab and the `ai_attribute` automation
  conditions read live rows. The inbox header renders triage chips
  (intent/urgency/frustration/escalation/cluster with confidence and
  source; heuristic sentiment where no attribute exists) — clicking an
  attribute chip opens the catalog vocabulary and a human override
  supersedes the machine row (source "human", confidence high; "unknown"
  clears it). Issue clustering is real: job `cluster_issues` runs the
  orphaned `cluster_issues` pipeline over the last 14 days, triggered
  daily by the maintenance timer (once per ~20 h, feature-gated) and
  manually via the new Issue Radar "Cluster now" button (with honest
  disabled/queued messages). Customer memory extraction runs as job
  `extract_memories` (Summarize role, feature-gated, skipped honestly
  without an endpoint), and its output lands in the inbox "know before
  you reply" briefing panel and in the evidence pack: `EvidenceContext`
  gains a `customer_memory` block, quarantine-filtered through
  `ai_memory_guard`, rendered into every analysis/draft/verification
  prompt as "DURABLE CUSTOMER FACTS".
- Tests: `ai_attributes::a2_wiring_tests` (deterministic snapshot lands
  with a disabled backend; a human override supersedes and the current
  row reads source=human) and
  `ai_evidence::a2_memory_feed_tests` (the evidence pack carries the
  durable fact and the memory guard keeps health-quarantined material
  out of the rendered prompt).

**Verification (Rule 5)**
- Local, pinned 1.99.1: `cargo fmt --all --check` clean; `cargo clippy
  --workspace --all-targets -- -D warnings` clean (two late let-return
  lints fixed); `cargo test --workspace` green — 163 app-shell + 214
  core (+11 new tests since batch 11) + 6 xtask; sandbox feature clippy
  clean and 219 tests green; `cargo-deny 0.20.2 check advisories
  licenses` ok; `scripts/gate_no_simulation.py` clean (and self-tested
  to catch a planted violation).
- First push (`a2c5309`): CI run `39522823488` — both jobs success on the
  runner (the no-simulation gate runs first, before the toolchain).
- Tag `v1.4.0` (initially at `a2c5309`): the Release run failed at the
  new gate step — the ubuntu:22.04 build container ships no `python3`
  (exit 127). Fixed by adding `python3` to the container's install step
  (`943a122`); tag deleted and recreated at the fix commit.
- CI run `39523717163` on `943a122`: success both jobs.
- **Release run `39523722592` on `v1.4.0`: success on all six jobs**
  (build+test+package on the glibc 2.35 baseline, .deb verified in
  clean debian:12/ubuntu:22.04/ubuntu:24.04 containers, AppImage
  verified, published). `verify_release.py v1.4.0`: ALL CHECKS PASSED —
  all four assets re-downloaded, SHA-256 verified against both
  SHA256SUMS.txt and the GitHub API digests, BUILD-INFO matches the tag
  commit `943a122…`; assets archived under
  `download/SupportOS-Plus-v1.4.0/`.

### Batch 13 (A3, A4, A5, A6, K1) — 2026-10-21 (commit `a50e73c`, CI run `39548532474`: success — both jobs green: "fmt, clippy, test, deny" and "AppImage + .deb build, smoke")

Phase A batch 2 plus the first Phase K item: the AI layer becomes real
end to end — every surface wired, every feature kill-switched and
double-gated, every new AI feature registered in the hub toggle grid.

**A3. Composer AI menu, post-resolution QA pages, interaction AI stage**
- Work: the composer gains an AI menu — pre-send draft coaching
  ("review_draft" job → `coaching::review_draft`: 9 deterministic checks
  + the optional AI layer) rendered as an advisory card above the send
  row (advisory only: it never blocks a send, and eval-mode semantics are
  untouched since coaching is a local read-only op); rewrite
  (shorten/expand/warmer/more_direct → "rewrite_composer" job →
  `ai_pipeline::rewrite_composer_text` behind the Draft role, refusing
  honestly without a routed backend) and translation
  ("translate_composer" job → `translation::translate`, 18 supported
  targets, per-translation cache rows) as composer transforms. The
  post-resolution QA trio goes live: maintenance warms the deterministic
  table (`quality::rebuild_qa`, 20 h cadence, feature-gated
  `post_resolution_qa`), the Reports page gains a Quality tab rendering
  `qa_overview` plus per-conversation flags, and the AI layer runs
  on demand ("qa_ai_layer" job → `compute_qa_ai_layer`, honest
  disabled-backend error). The interaction-intelligence AI stage rides
  the already-wired heuristic stage: `on_conversation_changed` enqueues
  "analyze_interaction" (feature `interaction_intelligence`) →
  `ai_pipeline::analyze_interaction`'s two-stage
  observation+recommendation merged onto the heuristic card, and the
  Customers profile renders the merged heuristic+AI card with the
  recommendation.
- Tests: `ai_pipeline::batch13_a3_a6_tests`
  (rewrite_composer_text refuses without a backend;
  record_feedback computes edit distance).

**A4. Retrieval spine**
- Work: `chunk_conversation` finally has its caller — the tail of
  `ingest_conversation` re-chunks every ingested conversation
  idempotently (chunk failures are logged, never break sync), closed
  conversations gain a resolution-excerpt chunk type (migration m055
  `chunk_kind`: 'thread' | 'resolution'), and fresh chunk writes enqueue
  an "embed_conversation_chunks" job on the "embeddings" queue (once at a
  time, feature-gated `retrieval`). Embeddings route through the hub
  **Embed role** (`backend_for_role`), so vector generation follows the
  provider hub instead of the legacy settings. The evidence engine's
  semantic-hits hook receives real sqlite-vec hits: similar-issues is now
  FTS fused with vector hits and re-ranked with match reasons; the
  22-tool agent's similar lane rides the same spine (`ai_tools::execute`
  takes the vector index). The Search page drops its hardcoded
  semantic=false and fuses BM25 + vector hits via the existing RRF k=60
  with real reported flags.
- Tests: `embeddings::batch13_a4_tests` (chunking is idempotent; a closed
  conversation gains the resolution chunk; an open one does not);
  `ai_evidence::batch13_a4_evidence_tests` (semantic hits are empty
  without a routed Embed role; they respect the feature gate);
  `search::batch13_a4_search_tests` (fused search shape + reported
  flags).

**A5. Copilot chat on the 22-tool agent**
- Work: the dead tool-calling agent is the AI Center Chat surface and
  the inbox assistant. `copilot::chat` takes the vector index (its
  similar lane is semantic-aware), a "copilot_chat" job kind runs the
  conversation-scoped assistant inside the inbox (session per
  conversation, message log, pending-job polling, verbatim job-failure
  errors), and the AI Center gains the Chat section — 22 read-only
  allowlisted tools, redacted SQL, bounded loop, sessions, citations and
  the audit trail unchanged; answers cite real mirror rows.

**A6. Feedback loop, golden-set runner, AI usage dashboard**
- Work: the feedback loop is wired — the inbox tracks the inserted
  draft id (provenance), a send records `record_feedback` (edit distance
  vs the original AI text) plus `set_draft_state("accepted")`, and the
  draft panel's accept/reject buttons write the same pair; edit-rate is
  now real in analytics. The golden-set runner lives in the new
  `ai_eval` module: the 9 seeded cases execute on demand and via
  `golden_context_fingerprint` drift (prompt-version constants or model
  role-map changes — maintenance enqueues "golden_eval" when the stored
  fingerprint differs), each run writing `golden_test_runs` with
  passed/failed/results; without a backend it writes an honest failed
  run, never a fabricated pass. The AI Center gains a Usage tab: the cost
  ledger grouped per feature/day/model/endpoint (tokens, latency, cost,
  runs), the live edit rate and the golden scores; the ledger SQL now
  COALESCEs started_at/created_at (start_run never populated started_at,
  so every production run was excluded from the dashboard — fixed).
- Tests: `ai_eval::batch13_a6_tests` (the fingerprint tracks role
  changes; a disabled backend writes an honest failed run);
  `ai_pipeline::batch13_a3_a6_tests` (edit distance); shell page tests
  in `ai_center.rs`/`inbox.rs` cover the new surfaces headless.

**K1. Gap detection pipeline**
- Work: the orphaned `quality.rs` machinery is wired to the worker path —
  maintenance (20 h cadence, feature-gated `gap_detection`,
  provider-gated) enqueues "rebuild_gaps" → `run_gap_rebuild_job` →
  friction findings over every conversation with a customer thread plus
  "asked often, documented nowhere" gap candidates from resolved and
  reopened conversations (reopen signal from the `status_changed`
  activity events), cross-checked against the knowledge base and the
  help-center mirror; Issue Radar gains a Gaps tab rendering the
  deterministic candidates with counts and examples.
- Tests: `quality::batch13_k1_tests` (gap kinds and the rebuild path).

**The fix that made the batch committable** — the IT test
`reply_dispatch_resolves_the_customer_from_the_api_and_repairs_the_mirror`
hung forever (and `revoke_clears_the_token_and_blocks_requests` queued
behind it on the process-global secrets lock). Root cause, reproduced
and confirmed with per-thread syscall dumps: every AI runner's
`spawn_blocking` closure held the DbHandle write guard across
`Handle::current().block_on` — the blocking-pool thread parked waiting
for the spawning runtime to drive its future, while that runtime (a
current-thread runtime in the tests and in the shell's worker) had its
only thread free to block on the very same write mutex (the worker
timer tasks whose first `interval.tick()` fires immediately) — a
mutual wait with no exit; in production the same shape froze every DB
write in the app for the length of each LLM round-trip (up to the 120 s
LM Studio timeout). All 11 runners now drive their pipeline on a
private self-contained current-thread runtime (`block_ai_pipeline`, the
embeddings lane's "lmstudio-embed" precedent). The previously-hanging
test passes in 1.3 s alone and inside the combined 25-test IT module.

Verification (local, mirroring CI end to end): `gate_no_simulation.py`
clean; `cargo fmt --all --check` clean; `cargo clippy --workspace
--all-targets -- -D warnings` clean; `cargo test --workspace` — app-shell
163 passed, core 228 passed/4 ignored (including all 25 it_helpscout
integration tests), catalog 6 passed; sandbox clippy clean and
`cargo test -p supportos-plus-core --features sandbox` 233 passed/4
ignored; `cargo deny check advisories licenses` ok; `SPP_SMOKE=1` boot
chain green on the freshly built binary (all 46 migrations incl. m055,
boot services, workers, loopback listener; exit 0). CI run
`39548532474` on `a50e73c`: both jobs success.

### Batch 14 (K2, K3, K4, K5, P1) — 2026-10-21 (commit `c384d2b`, CI run `39586925347`: success — both jobs green: "fmt, clippy, test, deny" and "AppImage + .deb build, smoke")

Phase K completion batch plus the first Phase P item: the knowledge loop
closes (draft → human review → Docs API publish → verify → freshness)
and the reports layer gains the weekly AI digest, the Reports API behind
capability detection, and the honest happiness/CSAT path.

**K2. Real AI article drafting + review queue**
- Work: `ai_pipeline::draft_article` drafts a knowledge article for a gap
  candidate grounded ONLY in `quality::best_resolutions` — the closed,
  published-reply evidence conversations (newest closure first, ≤ 6,
  excerpt-capped), the same excerpt logic the A4 retrieval spine stores
  as `resolution` chunks. The prompt (`ai_prompts::ARTICLE_DRAFT_SYSTEM`,
  `article_draft_v1`) forces per-claim `[^N]` citations bound to numbered
  sources and demands open questions instead of guesses where the
  resolutions disagree. The draft lands in the new `article_drafts` table
  (migration m056, boot step 47) as `drafted`; `decide_article_draft` is
  the only exit (approved/rejected, one transition, decided_at + user);
  a publish in flight can never be un-approved from under the publisher.
  Honest refusals leave no run rows and no queue entries: unknown
  candidate, duplicate queue row (`article_draft_exists_for_candidate`),
  no grounded evidence ("nothing grounded to draft from" — an article
  without resolutions would be invention), no routed Draft backend (the
  composer's AI_DISABLED contract, checked before `start_run`). The
  "draft_article" job (ai queue) + the Knowledge page's new "Article
  drafts" tab complete the loop: pick an open gap, request the draft,
  review title/body/citations, Approve or Reject. Feature:
  `article_drafting` (AI Center grid, default on).
- Tests: `ai_pipeline::batch14_k2_k4_tests::draft_article_refuses_honestly`
  (all three refusals, zero run rows); `quality::k2_tests` (closed-with-
  published-reply selection, newest-closure order, empty/open/silent
  conversations skipped, limit cap); `workers::batch14_tests::
  draft_article_job_skips_without_a_routed_backend`.

**K3. Docs API publish path**
- Work: the provider trait gains `get_doc_article` (GET
  /v1/articles/{id}), `upsert_doc_article_draft` (POST
  /v1/articles?reload=true creating a `notpublished` article, or PUT
  /v1/articles/{id}/drafts for a linked article) and `publish_doc_article`
  (PUT /v1/articles/{id} with status published); the real provider
  implements them through a new `docs_request_send` (POST/PUT under the
  same Basic-auth envelope + HsApiError mapping as the GET side; the
  missing-key case refuses with the honest setup message, and the
  trait defaults reject — an inert connection never pretends to
  publish). The "publish_article_draft" job runs the chain:
  approved-drafts-only guard (terminal failure otherwise) → collection
  resolution → Docs API draft upsert (`draft_upserted`) → publish
  (`published`) → verify by re-syncing the article through
  `sync_engine::upsert_doc_article` so FTS + chunks re-index locally
  (`verified`); every failure records `publish_failed` + the error on
  the queue row. Freshness now scores every source: `freshness_report`
  gained a `help_center` section scoring `docs_articles` with the same
  staleness threshold as local documents. The Knowledge drafts tab
  wires the UI: Approve unlocks (never performs) publishing; the
  explicit publish button + collection picker queues the job.
- Tests: `workers::batch14_tests::publish_job_runs_the_full_chain_and_verifies`
  (create→publish→verify over the real provider + mock Docs API with
  Basic-auth assertions, then the linked-article update path via PUT
  /drafts), `publish_job_refuses_an_unapproved_draft` (row untouched,
  nothing on the wire, job failed honestly).

**K4. Weekly AI digest + anomaly alerts**
- Work: new `digest` module (migration m057, boot step 48). Facts are
  all SQL over the local mirror: volume (new/closed/backlog + previous-
  period delta + per-day series), deterministic anomalies (any day > 2σ
  above the period mean with ≥3 conversations; backlog jump > 50%
  week-over-week with a floor of 10), topics (top issue clusters, top
  tags), sentiment (latest completed ticket_analysis run per
  conversation; absent-not-zero when no analyses exist), workload
  (replies per user, busiest hour), happiness (webhook ratings), and
  "what changed" (new gap candidates + changed help-center articles +
  new local documents since the previous archived digest).
  `ai_pipeline::weekly_digest` (prompt `weekly_digest_v1`, Summarize
  role) hands the facts to the model under the report_narrative
  contract (facts authoritative, never recalculated, "requires
  verification" flags) and archives either `complete` (facts +
  narrative) or `facts_only` (no backend routed — the deterministic
  half is still archived; a failed model run archives `failed` with the
  facts kept). The archive is searchable (narrative + facts JSON).
  Generation is on demand (Reports → Digest → "Generate digest now")
  and scheduled (maintenance: 7-day cadence, provider-gated, and only
  when a model is genuinely configured — a fresh install never
  archives weekly "model unavailable" rows). Feature: `weekly_digest`.
- Tests: `digest::tests` (archive roundtrip + search through narrative
  and facts, volume/anomaly/happiness collection with the loud day
  flagged, out-of-window rows excluded, sentiment absent-not-zero);
  `ai_pipeline::batch14_k2_k4_tests::weekly_digest_archives_facts_without_backend`;
  `workers::batch14_tests::weekly_digest_job_writes_the_narrative_end_to_end`
  (mock model server → status complete, narrative content asserted,
  run row in the ledger).

**K5. Help Scout Reports API behind capability detection**
- Work: new `reports_api` module (migration m058, boot step 49): the
  full documented endpoint registry — 32 unique GET paths (the docs
  sitemap's "33 report endpoint pages" counts the reports index itself;
  the verified 2026-10-21 research table's 30 rows include two
  two-endpoint rows) across 7 families, each with key/name/path.
  The four hard-coded named report trait methods (`get_company_overall_report`
  & co.) are replaced by one registry-driven `get_report(key, start, end)`;
  the empty provider's fabricated-from-InertWorld report impls are
  DELETED (R6's honesty rule extends to reports: an inert connection
  answers `Ok(None)`, never invented numbers) and `hs_range_report`
  is retired. Capability detection: `ReportsCapability` (Unknown /
  Available / PlanBlocked / NotConnected) persisted in settings,
  written only from definitive evidence (HTTP 403 → PlanBlocked;
  2xx → Available; empty provider → NotConnected; 401/network stay
  Unknown and retry) and invalidated on every credential save/clear
  (`store_in_envelope`/`clear_envelope` call `reset_capability`).
  The "fetch_hs_report" job fetches, caches into `hs_report_cache`
  (key, fetched_at, start, end, data — one row per key, overwritten),
  and records the verdict; errors keep their HsApiError status so the
  403 wall is classifiable. The Reports page's "Help Scout" tab renders
  the registry picker + fetch button, the cached reports (flattened
  label/value rows via `flatten_report`), and the capability banner —
  decisions D-c/D-g: Standard plans get one honest sentence ("Your
  Help Scout plan does not include the Reports API — it is available
  on Plus and Pro plans…") instead of empty charts.
- Tests: `reports_api::tests` (registry: 32 entries, unique keys/paths,
  all under /v2/reports; capability roundtrip + reset + classification
  table; cache upsert/overwrite + flatten); `workers::batch14_tests::
  fetch_hs_report_job_classifies_capability` (200 → cached + Available;
  403 → PlanBlocked), `fetch_hs_report_job_records_not_connected`.

**P1. Ratings and CSAT rework**
- Work: the local happiness readout `reports::happiness_overview` —
  great/okay/not-good counts in range, CSAT share (great ÷ all), and
  the recent ratings list, every number from `ratings` rows the
  satisfaction webhook delivered (Help Scout has no list-ratings API;
  an empty table means "none received", never "zero happened", and the
  source note says so). The Reports page gains the "Happiness" tab
  (metric cards + recent list + the Plus/Pro cross-link when the
  capability probe says Available). The relay-ready ingestion path is
  proven end to end over real TCP: `it_loopback::
  satisfaction_webhook_ingests_over_tcp` (signed satisfaction.ratings
  event through the loopback listener → verified → persisted →
  fetch-by-id job enqueued; unsigned rejected), and
  `workers::batch14_tests::satisfaction_ratings_job_fetches_and_stores_the_rating`
  (the job fetches GET /v2/ratings/{id} through the real provider and
  lands the row with conversation/customer joins). The misdesigned
  30-second ratings poller stays retired (R6 removed refresh_ratings,
  its re-entrancy flag, the dangling-rating sweep and the event chain;
  verified absent). D11 fully resolved.
- Tests: `reports::p1_tests` (counts + CSAT rounding + recent list;
  empty-is-absent-not-zero; out-of-range ratings never count) plus the
  two end-to-end tests above.

**Verification (Rule 5)**
- Local, pinned toolchain 1.99.1 (reinstalled after another sandbox
  reset wiped ~/.cargo): `cargo fmt --all --check` clean; `cargo clippy
  --workspace --all-targets -- -D warnings` clean; `cargo test
  --workspace` green — app-shell 163, core 248 passed/4 ignored
  (was 228+4; +20 new tests: 3 digest, 3 reports_api, 2 k2_tests,
  2 batch14_k2_k4, 7 workers batch14, 1 it_loopback satisfaction,
  plus 2 p1_tests counted within), catalog 6; `cargo clippy -p
  supportos-plus-core --all-targets --features sandbox -- -D warnings`
  clean; `cargo test -p supportos-plus-core --features sandbox` 253
  passed/4 ignored; `cargo deny check advisories licenses` ok
  (cargo-deny 0.20.2 reinstalled); `python3 scripts/gate_no_simulation.py`
  clean; release build (`cargo run -p supportos-plus-xtask -- package`
  compile stage) finished in 2m52s and `SPP_SMOKE=1
  ./target/release/supportos-plus` exit 0 — all 49 migrations
  (including m056/m057/m058) applied on a fresh DB, boot services,
  background workers, loopback listener, clean shutdown.
- Honest environment note: the local AppImage pack step refuses — the
  sandbox host lacks libxkbcommon-x11.so.0/libxcb-xkb.so.1 and has no
  root to install them (the xtask's designed error, not a code
  defect). CI's packages job apt-installs exactly those libs and runs
  both smokes on the runner — confirmed: the "AppImage + .deb build,
  smoke" job on run `39586925347` completed success, so the packaging
  (AppImage + .deb) and both boot-chain smokes are verified on the
  runner, the same verification model as batches 12 and 13.
- The empty-provider report fabrication removal is the batch's one
  behavioral regression risk: the UI never called the four named
  methods (grep-verified), so no surface changes.

## Batch 15 (final) — 2026-10-21 — Phase P complete: P2, P3, P4, P5 → v1.7.0

The last batch — four items, not five, because the plan is otherwise
exhausted (Rule 6). With it, every work item of the approved v2 blueprint
(Phases R, A, K, P) has passed through this file, and the engagement's one
remaining owner action is the token revocation noted under P5.

**P2 — live chat.** Chats arrive through the five `beacon.chat.*` webhook
events (Help Scout has no chat list endpoint — the webhook IS the data
path) into a new local mirror (m059) that keeps ended chats, because the
platform stops serving them (`GET /v2/chats/{id}` answers 404 once a chat
collapses into a conversation). Sending uses the documented
`PUT /v2/chats/{id}/messages/{uuid}` contract with a caller-generated
message id, so a retry after a timeout can never double-send (the
platform's 422 "id value is already in use" answer is treated as
delivered). The send box enforces the platform's assigned-agent rule in
plain language — unassigned chats note that sending accepts them, chats
held by another agent are blocked with the 400 reason, ended chats point
at the conversation they became. The Live chats page sits in the Workspace
nav group, with the SyncCompleted/ConversationUpdated invalidations wired.

**P3 — the library surfaces.** Saved replies become manageable from
Operations: create, edit and delete go straight to the platform
(`POST`/`PUT`/`DELETE /v2/mailboxes/{id}/saved-replies`, the platform's
required `name` enforced), and the table re-reads the platform's
post-write list, never an optimistic local edit. Workflows, synced all
along but invisible, surface with their type/status and a run action
(`POST /v2/workflows/{id}/run`) that resolves the conversation number
from the local mirror.

**P4 — the webhook relay guard (closes D21).** A silently deactivated
Help Scout webhook now heals itself: the health check lists the account's
hooks, verifies ours is registered, enabled and subscribed to the events
the app processes, and re-registers it (with the stored secret, capped at
the platform's 40 characters) when the platform dropped, disabled or
pruned it — then removes the stale twin. It runs every ~6 hours and on
demand (Settings → "check now"), and its outcome surfaces in Settings and
Sync Health. The user-run tunnel guide lives next to it: the listener
stays loopback-only by design, the card documents cloudflared/ngrok/ssh
relays, the probe that proves the chain (a POST to the webhook endpoint
must answer 401), and the honest limit that the API never reveals a
hook's stored secret, so a rotated local secret needs a re-register.

**P5 — remote and release hygiene.** Release cadence verified end to
end: v1.7.0 ships through the same CI + Release pipeline as v1.4.0 — the
code commit below went through both CI jobs green, then the tag ran the
Release workflow that builds, smokes and publishes the artifacts. Token
rotation reminder delivered to the owner with the final batch report: the
GitHub token used for pushes (`ghp_Hx0B…`) must be revoked by the owner
now that the engagement's plan is exhausted.

**Verification (Rule 5).**

- Local gates (toolchain 1.99.1): `cargo fmt --all --check` clean;
  `cargo clippy --workspace --all-targets -- -D warnings` clean;
  `cargo test --workspace` — app-shell 169 passed, core 273 passed +
  4 ignored, catalog 6 passed; sandbox feature: clippy clean, tests
  278 passed + 4 ignored; `cargo deny check advisories licenses` ok;
  `python3 scripts/gate_no_simulation.py` clean.
- Release build (release profile, sandbox feature) completed and
  `SPP_SMOKE=1` on the locally built binary exited 0 — the boot chain
  (logging, single-instance lock, migrations, background workers,
  loopback listener, clean shutdown) output archived in the batch log.
- Honest environment note: the local AppImage pack step still refuses —
  the sandbox host lacks libxkbcommon-x11.so.0/libxcb-xkb.so.1 and has
  no root to install them (the xtask's designed error, not a code
  defect). CI's packages job apt-installs exactly those libs and runs
  both smokes on the runner — the same verification model as batches
  12–14.
- Security: the staged diff was token-scanned clean before the push (no
  credentials in the commit).
- Runner verification: code commit `4850f71af8156539d22936b6eb39385f96470645` — `CI run 39625936192 (CI): success`.
  v1.7.0 released: `release run 39627683265 (Release): success; published 4 assets: BUILD-INFO.txt (0 KB), SHA256SUMS.txt (0 KB), SupportOS-Plus-1.7.0-x86_64.AppImage (12234 KB), supportos-plus_1.7.0_amd64.deb (9715 KB)`.


## Batch 16 — 2026-10-22 — Clean-source audit: items 1-5 (first five audit findings)

**Source**: the owner-mandated v1.7.0 clean-source audit — source code only,
no test suite, no descriptive docs, report nothing without proof. 13 parallel
read-only audit agents read every non-test build-required file (core ~80k
non-test lines, app-shell ~27.3k, xtask/catalog, CI/deny/gate configs); all
197 cited evidence anchors were re-verified personally with display-safe
byte encodings after the audit pane itself was caught eating the two bytes
`[m` (which had produced — and then retracted, with od/base64 proof — a false
`ci.yml` trigger finding). Full report with all 184 findings:
`/home/z/my-project/download/SupportOS-Plus-v1.7.0-clean-audit.md`.
The remaining 160 findings are queued in `ai/plan.md` (items 6-165) for
future five-at-a-time batches.

1. **Provider delegations** (high|bug) — `ProviderHandle` never delegated 8
   trait methods; saved-reply CRUD, workflow runs and attachment downloads
   always hit the trait's rejecting defaults even with a live provider
   connected (impl ended at `docs_configured`; workers.rs:3236/3360/3521
   enqueue through the handle). Added `create_saved_reply`,
   `update_saved_reply`, `delete_saved_reply`, `delete_thread_schedule`,
   `run_workflow`, `get_attachment_data`, `get_routing_configuration`,
   `ping` (+ the `AttachmentData` import).
2. **chunk_text UTF-8 panic** (high|bug) — knowledge_store.rs:53 backed
   `start` up `overlap` bytes without a boundary check: any multibyte
   document (accents, CJK, emoji) panicked at import ("byte index is not a
   char boundary"). `start` now walks forward to the next boundary.
3. **not_equals evaluated as equality** (high|bug) — automation.rs:1577-1579
   fired exclusion rules on exactly the excluded value. condition_matches
   now implements the accepted operator set per field semantics (subject
   equals/not_equals; tag not_equals = "no tag equals the value"; body and
   mailbox not_equals = negated contains; unknown operators return false
   instead of silently matching; ai_verification not_equals added).
4. **Dead 'customer_message' thread-type literal** (medium|bug) —
   people_store.rs:478 and reports.rs:574 filtered a type no writer ever
   emits, so the customer-effort metric and the CustomerReplies metric were
   structurally always 0. Both corrected to 'customer' (the type every
   writer emits).
5. **No-simulation gate blind spots** (high|bug) — scripts/gate_no_simulation.py:
   a `#[cfg(test)]` marker inside a // comment triggered region mode and
   braceless `mod x;` declarations ran the region into the next `{`-bearing
   production line (verified trips: app-shell lib.rs:31-38, core lib.rs:121-136
   swallowing `pub use error::{Error, Result};`, mockserver.rs:1-16);
   removing lines also shifted every later reported line number.
   `strip_test_regions` rewritten: comment lines never trigger regions,
   braceless `mod x;` regions end at their declaration, consumed lines are
   blanked in place so violation line numbers are true file lines.

Verification (Rule 5):

- `cargo fmt --all --check` clean; `cargo clippy --workspace -- -D warnings`
  clean on the pinned 1.99.1 toolchain (reinstalled from rust-toolchain.toml
  after the janitor rewind had wiped ~/.cargo); `python3 scripts/gate_no_simulation.py` clean.
- Fresh harness `/home/z/my-project/scripts/verify_gate_fix.py` (not the
  existing suite): 161/161 scanned files preserve line counts; all 4
  previously-swallowed production lines are scanned again; an injected marker
  on a previously-swallowed line is caught by the new algorithm and provably
  missed by the old one; real cfg(test) regions still blanked — ALL OK.
- Audit-evidence harness `/home/z/my-project/scripts/verify_audit_citations.py`:
  all 197 cited anchors re-printed and matched.
- Runner verification: code commit `48022b7f44b7f23b11561e15b87f5155e3a1b704` — `CI run 39669015404 (CI): success`.
- Security: the staged diff was token-scanned clean before the push (no
  credentials in the commit).

## Batch 17 — 2026-10-22 — Clean-source audit: items 6-10 (second five audit findings)

**Source**: `ai/plan.md` items 6-10, executed five-at-a-time per `rule.md`
on the owner's "continue". Audit IDs A1 / A2+J2 / A3+B2 / C1 / D1; full
evidence in `SupportOS-Plus-v1.7.0-clean-audit.md` and the AUD-A..AUD-M
worklog records.

1. **Worker restart mid-tick permanently killed the job queue** (high|bug,
   A1) — `stop()` aborts timer tasks at await points, and `tick()` /
   the three sweep timers cleared their `processing`/`sweeping_*` guard
   flags only on the happy-path last line: a settings-driven restart
   that landed mid-tick (or a contained tick panic) stranded the flag set
   forever, after which every future tick of that timer returned
   instantly — queued replies/notes/AI jobs never dispatched until process
   restart. Fixed twice over: (a) a RAII `TickGuard` latch now clears each
   flag on every exit path — normal return, panic unwind (contained at
   the task boundary), and abort during shutdown/restart; (b) `start()`
   resets all four flags so a restart begins from a provably clean state.
   workers.rs:339-341,346,387 + 145-149 (pre-fix line numbers).

2. **Writer lane held across full LLM round-trips** (high|bug, A2+J2) —
   all 14 background long-operation sites in workers.rs took the
   exclusive DB writer mutex and held it across entire AI pipelines
   (multi-call LLM round-trips, 120s timeouts); every UI `with_write`
   (settings save, assignment, reply send, chip override) blocks on that
   same mutex synchronously on the egui thread → the whole window froze
   for the duration of AI jobs. New `DbHandle::long_op_guard()`
   (`DbLongOpGuard`, Deref/DerefMut to Connection): file-backed databases
   get a fresh, tuned connection on the same file (WAL reads, writes
   serialized against the writer lane at the SQLite level inside the
   shared 5s busy_timeout — ai_attributes' own transactions are short
   and DB-only, verified), so no lane is pinned across network I/O;
   in-memory databases keep the shared single connection (a second
   `:memory:` handle would be an empty private DB). All 13
   `block_ai_pipeline` sites + the deterministic gap-rebuild scan site
   now use it; the manager's short-lane `lock()` accessor is untouched.
   workers.rs:1883-1885 (+13 more sites), db.rs.

3. **Duplicate outbound sends on ambiguous failure** (high|bug, A3+B2) —
   ambiguous outcomes on non-idempotent POSTs (read timeouts, broken
   pipe mid-body, 5xx — the server may have persisted the resource and
   failed answering) were blind-retried in-client (up to 3x on
   "timeout"/"error sending request" message matching AND 5xx) and then
   re-queued by the outbound job layer via `retryable` — the customer
   could receive the same reply twice. `request_inner` now computes a
   replay-safety policy (`idempotent = method != POST`): connect-phase
   transport failures (`e.is_connect()` — provably never delivered) and
   replay-safe methods still retry in-client; ambiguous create failures
   surface honestly ("Delivery could not be confirmed - the request was
   NOT re-sent automatically to avoid duplicates...") with
   `retryable: false` so nothing blindly re-POSTs. 429 keeps its safe
   in-client retry (rate-limited requests are never processed); 401
   token-refresh retry untouched. This also de-fangs the saved-reply
   create re-POST (audit A6/plan item 30's root cause) and the campaign
   create re-queue path (item 17's in-client half). helpscout_real.rs:
   1156-1170/1247-1250/1290-1297 + workers.rs:751-768 (pre-fix).

4. **`raw_json_hash` never written → stale analyses served forever**
   (high|bug, C1) — `conversation_threads.raw_json_hash` existed in the
   schema with zero writers and exactly one reader (the analysis-cache
   signature); an in-place thread edit (edited body, state flip
   re-synced through `upsert_thread`'s ON CONFLICT UPDATE) changed
   neither the thread COUNT nor MAX(id), so the cached pre-edit analysis
   was served as current forever. Fixed on both sides of the contract:
   `upsert_thread` now writes `raw_json_hash` (SHA-256 of the thread's
   serialized content) on INSERT and on every in-place update, and
   `get_analysis_signature` folds EVERY live thread's hash into the
   signature (not just the newest row's) so an edit to any thread
   invalidates the cache. Pre-fix rows contribute an empty string —
   deterministic, one honest re-analysis after upgrade.
   ai_pipeline.rs:456-459 + sync_engine.rs:662-712 (pre-fix).

5. **LM Studio probes never sent the stored API key** (high|bug, D1) —
   `is_available()`/`list_models()` issued unauthenticated GET /models
   while chat/embed attach `bearer_auth`, so keyed (BYOK cloud)
   endpoints showed "offline" and attribute extraction silently degraded
   to deterministic-only. Both probes now authenticate exactly like the
   real calls. ai_lm_studio.rs:118-151 (pre-fix).

Verification (Rule 5):

- `cargo fmt --all --check` clean; `cargo clippy --workspace -- -D warnings`
  clean (one iteration: clippy's own `explicit_auto_deref` suggestion was
  rejected because auto-deref of the `expect` chain trips E0515 — the guard
  was restructured as an enum, zero allows); `python3
  scripts/gate_no_simulation.py` clean, all on the pinned 1.99.1
  toolchain (reinstalled after the janitor rewind wiped ~/.cargo; default
  re-set this session).
- Fresh harness `scripts/verify_batch17.py` (committed; also archived at
  /home/z/my-project/scripts/): 41/41 checks pass — TickGuard
  acquire/Drop pairing, all four start() resets, all four tick entries
  latch-converted with no orphan swap/trailing store; 14 long-op sites
  bound to `db.long_op_guard()` with zero remaining `db.write_guard()`
  pipeline acquisitions; replay-safety policy + all three retry sites +
  honest ambiguity message + old string-match retry provably gone;
  hash written on INSERT and UPDATE + all-thread signature fold with the
  LIMIT-1 latest-row read provably gone; both probes attach the key with
  chat unregressed.
- Runner verification: code commit
  `f9fd4c8` — CI run 39677607892: success (fmt/clippy/test/deny +
  AppImage/deb build + smoke all green; the existing suite's
  retryable-semantics assertions held — connect-phase failures remain
  retryable).
- Security: staged diff token-scanned clean before the push; remote
  unmoved at push time (fetch-first).

---

## Batch 18 — clean-audit items 11-15 (HIGH tier: backup restore, QA wipe, SLA waiting, the never-firing gap, campaign mailbox)

_Completed 2026-10-22. Code commit `6366425` — CI run 39778901297:
success. All five fixes carry fresh regression tests plus the batch
harness (scripts/verify_batch18.py, committed; also archived at
/home/z/my-project/scripts/)._

1. **Backup restore swapped an unverified file over the live database**
   (high|bug) — `restore()` copied the chosen backup to a temp file and
   renamed it over the live DB with no integrity check, no fsync, a
   stale-WAL window (the new file was renamed in BEFORE the old `-wal`
   was deleted, so a crash in between would replay the old WAL onto
   the fresh content), and one unconfirmed UI click. Now: the backup
   must pass integrity_check AND a SupportOS structural probe (the
   base-schema `app_state` table — a foreign SQLite file is refused)
   before anything on disk is touched; the staged copy is fsynced
   before any swap; the live database AND its WAL/SHM move aside as
   one unit (between the renames the db path does not exist, so no
   cross-generation WAL pairing is possible); the swapped file and
   its directory entry are fsynced; a failed swap rolls the live
   database back; no residue survives success; and the Settings
   restore button is a two-step confirm (arm → "really restore"
   Destructive) that states the consequence before the second click.
   backup_service.rs:179-284 + pages/settings.rs (pre-fix
   backup_service.rs:186-198 + settings.rs:2124-2131).

2. **The >=20h QA rebuild wiped every stored AI layer** (high|bug) —
   `compute_qa_deterministic` DELETEd the whole `post_resolution_qa`
   row and re-INSERTed only the deterministic half, so the maintenance
   sweep (workers.rs, every 20 hours while the feature flag is on)
   destroyed the on-demand AI layer (ai / ai_run_id / recomputed_at)
   for every closed conversation — permanent whenever the AI stage
   could not re-run. The recompute now UPDATEs the deterministic layer
   in place (an INSERT only when the row is missing) and never touches
   the AI-checked columns. quality.rs:1645-1663 (pre-fix 1636-1644).

3. **SLA waiting stats measured from `updated_at`** (high|bug) — any
   status change, tag edit, assignment or sync touch refreshed
   `updated_at` and reset the waiting clock, silently understating SLA
   pressure. Waiting now runs from the newest published, non-deleted
   CUSTOMER thread (MAX(COALESCE(remote_created_at, created_at))),
   falling back to the conversation's own created_at when the customer
   has not spoken. The maintained `customer_waiting_since` column was
   deliberately NOT used as the source: it is derived by the activity
   machinery that audit items 20/22 show is currently broken (never
   enqueued / corrupted), while the thread-derived value is ground
   truth from the raw mirror. sla.rs:509-527 (pre-fix 510).

4. **"repeated_question_unsolved" could never fire** (high|bug) — the
   gap arm counted friction through `friction_scores`, a table whose
   only writer (`reports::compute_friction`) has no runtime caller,
   whose serialized JSON keys never matched the query
   (`message_count`/`resolution_hours`/`reopen_count`/`escalated` vs
   the queried `clarification_count`/`follow_up_count`), joined
   through a remote-id namespace — so the count was structurally
   always zero and the documented gap kind never triggered. It now
   reads the WIRED interaction engine (`client_support_outcomes`:
   real columns, written on every customer reply and baseline
   rebuild, UNIQUE per conversation) with local ids on both sides, so
   a documented question whose askers kept showing follow-up or
   clarification friction becomes a real gap candidate — and
   cleanly-resolved questions provably do not. quality.rs:338-369
   (pre-fix 344-348).

5. **Campaign sends passed the LOCAL mailbox row id as the remote
   `mailboxId`** (high|bug) — `CreateConversationInput.mailbox_id`
   went straight to the platform carrying the campaign's local mirror
   row id (1, 2, ...), which is not a valid Help Scout mailbox, so
   every campaign send failed. `send_batch` now resolves
   `mailboxes.remote_id` once per batch and sends that; a mailbox that
   never synced fails the whole batch permanently and honestly
   ("mailbox not synced") instead of sending garbage. Implementing the
   fix exposed a latent self-deadlock in BOTH early-fail branches: the
   let-else kept the writer guard alive across `finalize_if_drained`,
   which re-acquires the same non-reentrant write lane — the
   no-mailbox path would have hung the outreach worker thread forever.
   Both branches now scope the guard. outreach.rs:1814-1871 (pre-fix
   1871).

Verification (Rule 5):

- `cargo fmt --all --check` clean; `cargo clippy --workspace
  --all-targets -- -D warnings` clean (one iteration: two
  `doc_overindented_list_items` in a test doc comment); clippy with
  the `ui-screenshot` and `sandbox` features clean; `python3
  scripts/gate_no_simulation.py` clean — all on the pinned 1.99.1
  toolchain (reinstalled after the janitor rewind wiped ~/.cargo).
- Fresh harness `scripts/verify_batch18.py` (committed; also archived
  at /home/z/my-project/scripts/): 31/31 checks pass — pre-swap
  integrity + structural verification, staged fsync, live-db-before-
  new-file ordering, WAL-moves-aside-with-the-db, post-swap file and
  directory fsync, rollback, residue cleanup, the two-step UI arming;
  in-place QA update with the row-wiping DELETE provably gone and the
  AI-layer writer untouched; customer-thread since with the
  updated_at shape gone; the gap arm on client_support_outcomes with
  real columns and no json_extract; remote-mailbox resolution, the
  honest not-synced branch, and scoped writer guards on both
  early-fail paths.
- Tests: core 283 passed / 0 failed / 4 ignored (8 new batch-18
  regressions: the unsolved gap fires on real interaction data + the
  clean-resolution negative control, the AI layer survives the
  recompute + the fresh-row insert path, waiting-from-last-customer-
  message + the recently-touched discrimination, the remote mailbox
  id reaches the platform + the unsynced mailbox fails honestly);
  sandbox-feature suite 288 passed / 0 failed / 4 ignored; app-shell
  170 passed / 0 failed (incl. the new page-level restore test: a
  verified backup lands with the restart message, junk is refused
  with the live database untouched); xtask 6; catalog 0.
- Runner verification: code commit `6366425` — CI run 39778901297:
  success (fmt/clippy/test/deny + AppImage/deb build + smoke, all
  green on the runner).
- Security: staged diff token-scanned clean before the push; remote
  unmoved at push time (fetch-first).

## Batch 19 — clean-audit items 16-20 (HIGH tier: outreach attempts/claim, automation trigger poisoning, platform-pushing write actions, local-id activity keys)

_Completed 2026-10-22. Code commit `0de77ff` — CI run 39797355477:
success (both jobs). All five fixes carry fresh regression tests
(15 new) plus the batch harness (`scripts/verify_batch19.py`, committed;
also archived at `/home/z/my-project/scripts/`)._

1. **Recipient `attempts` never incremented** (high|bug) — every
   `record_outcome` state UPDATE wrote the ledger row but never touched
   `outreach_recipients.attempts`, so it stayed 0 forever:
   `classify_error`'s `attempts + 1 >= SEND_MAX_ATTEMPTS` cap on
   429/5xx answers could never fire and the send-loop backstop read
   `attempt_no = 0 + 1` every time — rate-limit and server errors were
   retryable without limit, and because `pick_batch` orders by id the
   same stuck recipients filled every batch (infinite retry storm +
   head-of-line block). Every outcome now spends an attempt
   (`attempts = attempts + 1`) and refreshes the new staleness clock.
   outreach.rs:1737-1771 (pre-fix 1727-1756).

2. **No claim/idempotency across campaign sends** (high|bug) —
   `pick_batch` handed rows to a send loop that does network I/O
   before recording, with no in-flight state, so a crash (or any
   second picker) re-picked the same 'queued' rows and sent duplicate
   emails; the half-built recovery machinery was triple-dead: nothing
   ever set 'sending', the reconcile sweep's stale query bound the
   TEXT of a datetime() expression as a comparison parameter (never a
   past timestamp), and the `updated_at` column it filtered on does
   not exist. Now: picked rows are atomically claimed
   (`state='sending'` + `updated_at`, only while still pickable — a
   raced claim simply sends nothing); `updated_at` ships in the fresh
   schema and is lazily healed on old databases (constant-default
   ALTER + created_at backfill — SQLite rejects expression defaults
   on ADD COLUMN, which the pre-existing segments helper also trips
   over); every batch run first runs the recovery sweep
   (`reconcile_recipient_states`, extracted from the now-wired
   `campaign_reconcile`): stale claims older than 10 minutes probe the
   platform mirror by customer+subject first — a crashed send that
   landed is recovered as 'sent' (no duplicate), a genuinely
   undelivered one returns to 'queued' for a bounded retry, ambiguous
   'unknown' rows are reconciled the same way, and exhausted ones
   terminate honestly in 'failed' instead of lingering forever. The
   contradictory "safe to retry" → state='failed' branch now actually
   returns rows to the queue. outreach.rs:952-1222,1649-1696,1901-1962
   (pre-fix 1649-1673 + the dead reconcile arm).

3. **UI-created rules poisoned the engine's trigger deserialization**
   (high|bug) — the UI's rule form still serializes the M007 tagged
   enum (`{"kind":"tag_added","tag":"x"}`) while the live engine
   (`fire_trigger_for_conversation`) demands a JSON string, and
   `list_rule_records` propagated the FIRST parse error — so every
   UI-created rule killed the whole engine evaluation, silently
   (both fire sites swallow errors with `let _ =`). Trigger parsing
   now accepts both generations (the legacy enum maps to
   status_changed / tag_added / sla_risk), and `list_rule_records`
   skips an unparsable row with a warning instead of aborting —
   `get_rule_record` stays strict so the editor still surfaces real
   corruption. Documented limitation: the legacy trigger KINDS have no
   fire sites yet (nothing fires "status_changed"/"tag_added"/
   "sla_risk"); UI rules are now visible and inert, not toxic —
   wiring those fire sites is new-feature work outside this item.
   automation.rs:1068-1091,1168-1199 (pre-fix 1081-1087,1147).

4. **Approved automation write actions never reached Help Scout**
   (high|bug) — `execute_approved_action` ran the typed local mirror
   write (`ticket_ops::execute`) and stopped, and `add_tag` wrote the
   local tag list and stopped; the next sync reverted both. The manual
   inbox path queues an outbound push after the same local write —
   now automation does too: approved set_status/assign enqueue
   `update_status`/`assign` outbound jobs (LOCAL conversation id in
   the payload, deterministic idempotency keys
   `automation_status_/assign_{id}`; a failed enqueue propagates so
   the approval job retries honestly), the add_tag arm enqueues a new
   `update_tags` outbound job, and the outbound worker learned the
   `update_tags` kind (provider push + sync-back + honest empty-list
   rejection) with its own audit action name. The eval-mode gate and
   the no-connected-provider guard on the outbound lane cover
   automation pushes exactly like manual ones. automation.rs:1407-1434
   (add_tag),1476-1560 (approved), workers.rs:726-749,770-776
   (pre-fix 1365-1374,1448-1452).

5. **Activity events keyed by REMOTE id in a LOCAL-id column**
   (high|bug) — `activity_events.conversation_id` is a LOCAL-id
   column (that is what `rebuild_all` writes and the by-local-id
   reader consumes), but every ticket_ops execute_* inserted the
   REMOTE id there and then recomputed the derived response state
   from that wrong-keyed event set — so every user/automation action
   wiped `first_customer_message_at`/`last_customer_reply_at`/etc. on
   the correct conversations row while its real event history stayed
   invisible, and a numeric local/remote id collision would corrupt a
   different conversation's timeline. ticket_ops now resolves the
   local id once (`conversation_local_id`), keys all three event
   inserts with it, and refreshes derived columns through
   `update_derived_columns_by_local_id`; the remote-keyed
   `derive_response_state`/`update_derived_columns` twins are deleted
   (zero callers after the fix). Fixing the inserts exposed a second
   latent defect in the same lines: the dedup_key concatenation
   `... || ?actor_id || ...` becomes SQL NULL for anonymous/
   automation actors (actor_id NULL), so the plain INSERTs errored
   and the INSERT OR IGNORE silently dropped the event — automation
   actions never left an activity event at all. All three keys now
   COALESCE the actor id. ticket_ops.rs:107-298, activity.rs:318-324
   (pre-fix ticket_ops.rs:135-140,195-200,269-274 + activity.rs:149-260).
   Note: historical remote-keyed rows may linger in existing
   databases as invisible orphans (harmless — every reader is
   local-keyed now); a one-time cleanup was deliberately NOT added to
   keep this batch surgical.

Verification (Rule 5):

- `cargo fmt --all --check` clean; `cargo clippy --workspace
  --all-targets -- -D warnings` clean; clippy with the `sandbox`
  feature clean; `python3 scripts/gate_no_simulation.py` clean; cargo
  deny 0.20.2 advisories + licenses ok — all on the pinned 1.99.1
  toolchain (reinstalled once more after the janitor sweep wiped
  ~/.cargo between sessions).
- Tests: core 298 passed / 0 failed / 4 ignored (15 new batch-19
  regressions: retryable failures spend attempts and terminate at the
  cap; a fresh claim is invisible to a second pick; a stale claim that
  landed recovers as sent from the mirror without a re-send; a stale
  claim that never landed requeues and sends exactly once; a
  reconciled unknown returns to the queue under the cap; an exhausted
  unknown fails honestly; the staleness column is ensured by the batch
  run; legacy UI rules parse and no longer poison the engine; a single
  poisoned rule cannot kill the evaluation; approved set_status/assign
  and automation add_tag all queue their platform pushes with the
  right payloads; status/note/assign events are keyed by the local id;
  user actions preserve the derived customer history; an unknown
  remote conversation is rejected); sandbox-feature suite 303 passed
  / 0 failed / 4 ignored; app-shell 170 passed / 0 failed; xtask 6;
  catalog 0.
- Fresh harness `scripts/verify_batch19.py` (committed; also archived
  at /home/z/my-project/scripts/): all checks pass — attempt-spending
  in all four outcome arms, the atomic claim with its pickable-state
  guard, the constant-default ALTER + backfill heal, the inlined
  staleness threshold with the mirror-probe recovery, the wired-in
  recovery-only reconcile, dual-generation trigger parsing, the
  skip-with-warning listing, the three platform-push enqueues with
  their idempotency keys, the outbound update_tags lane and audit
  name, local-id event keying, COALESCE'd dedup keys, and the deleted
  remote-keyed machinery.
- Runner verification: code commit `0de77ff` — CI run 39797355477:
  success, both jobs (fmt/clippy/test/deny + AppImage/deb build and
  smokes, all green on the runner).
- Security: staged diff token-scanned clean before the push; remote
  unmoved at push time (fetch-first).

## Batch 20 — clean-audit items 21-25 (HIGH tier: vector point-id collisions, the never-enqueued activity rebuild, auto-sync toast spam, the stale composer-AI fill, the phantom "waiting" status)

_Completed 2026-10-22. Code commit `a5d0d1b` — CI run `39863156425`:
success (both jobs: fmt/clippy/test/deny and AppImage/deb build + smoke,
all green on the runner). Local verification: fmt/clippy (plain + sandbox
feature)/tests/gate/deny all green on the pinned 1.99.1 toolchain; the
fresh harness `scripts/verify_batch20.py` (committed; also archived at
`/home/z/my-project/scripts/`) passes all 45 checks._

**21. Vector point-id collisions across entity families** (high|bug) —
all three chunk families (knowledge/docs/conversation) upsert into the
single `supportos_vectors` collection with their raw per-table row ids as
the point id, so knowledge chunk 7 and conversation chunk 7 landed on the
same key and the upsert's DELETE+INSERT silently overwrote whichever
family wrote first — permanent, invisible vector loss. Point ids are now
namespaced by family (`namespaced_chunk_point_id`: knowledge band 0, docs
band 1, conversation band 2, stride 1e9 — an out-of-band chunk id fails
loudly instead of aliasing into a neighbour), `vector_point` derives every
production id through it and stamps `index_version: 2`, and the store
itself refuses a same-id write whose payload declares a different
`entity_type` ("cross-family point-id collision") so a future
regression-to-raw-ids is loud, not silent. Existing databases are
migrated once (`ensure_vector_id_schema_v2`, marker setting
`vector_id_schema`): the colliding collection is dropped, all three chunk
tables are reset to `not_indexed` (attempt counters included), and the
re-embed pipeline is restarted for every family — the corrupted vectors
are unrecoverable, so re-embedding is the honest repair. The migration
only runs while the vector store is live (enabled + openable); a disabled
index never consumes the marker, so pre-v2 vectors cannot survive a
disable/enable cycle into the namespaced era.
vectorstore_sqlitevec.rs, embeddings.rs.

**22. `rebuild_activity` never enqueued** (high|gap) — the worker arm
existed (workers.rs:1391) but no code path ever enqueued the job, so the
derived response-state columns (first_customer_message_at,
customer_waiting_since, response_state, …) stayed at their schema
defaults forever on every database; the only writer that ever touched
them was the per-action ticket_ops path from batch 19. The rebuild now
rides along with the other post-sync jobs in
`on_after_initial_sync` (threads just landed → derive the activity events
and recompute every conversation), and a one-time maintenance repair
(`enqueue_activity_rebuild_repair_once`, marker
`activity_rebuild_repaired`, gated on conversations > 0 so fresh installs
are not double-fed) backfills the databases that synced before this fix.
The job itself is idempotent (dedup-keyed inserts + recompute), so the
overlap of the two paths is harmless. workers.rs.

**23. Toast on every auto-sync completion** (high|bug) —
`SyncCompleted` carried no notion of who started the run, so the shell
toasted "Sync finished / Up to date" for the adaptive timer's ~30-second
busy ticks (~one toast a minute forever) and for the post-write
mirror-refresh nudges. `SyncCompletedEvent` now carries a `trigger`
(`SyncTrigger::Manual | Auto | Refresh`, serde-defaulted to manual so
legacy serialized payloads keep their visible outcome), every emitter
declares itself (engine entry points take the trigger; the timer says
Auto; the job lane and the manual buttons say Manual; the saved-reply
re-read and webhook-health refreshes say Refresh), and
`sync_completed_toast` returns None for non-manual triggers — page
invalidation is untouched, the toast lane only speaks for runs the user
asked for. events.rs, eventing.rs, sync_engine.rs, workers.rs, app.rs,
pages/settings.rs, pages/sync_health.rs.

**24. Composer-AI poll filled from a stale/foreign run** (high|bug) —
on job completion the inbox polled `ai_runs` with
`ORDER BY id DESC LIMIT 1` and `conversation_id = X OR conversation_id IS
NULL`, i.e. the newest matching run from ANY conversation or session: a
foreign historical run's text filled the composer, cache-hit
translations (which write no run row at all) resurrected an older run's
text, and the feature-off path claimed "Composer updated by the AI
transform" with somebody else's output. The job is the unit of work the
UI polls, so its result now belongs on the job row: `jobs` gains a
`result` column (fresh schema + lazy heal for old databases via the
ensure hook), `complete_with_result` pins the transform output at
completion, both handlers return their text (`Ok(None)` = honest
no-output), and the poll reads `SELECT status, error, result FROM jobs
WHERE id = ?1` — never `ai_runs`. A completed job without a result says
exactly that. jobs.rs, workers.rs, pages/inbox.rs.

**25. "waiting" is not in the platform's status vocabulary** (high|bug)
— the Settings "status after reply" dropdown offered
`waiting (await agent)`, but Help Scout's statuses are
open/pending/closed: every reply sent with that setting rode the wire as
`status: "waiting"` and was rejected by the platform (and silently
mapped by the loopback). The option is now `open (await agent)` — the
platform's own await-agent status; the stored legacy `waiting` value
loads as `open` (settings loader) and maps to `open` at send time, so
old installs stop sending the phantom status without any manual
migration; the loopback world maps `open`/legacy `waiting` onto its
`active`. pages/settings.rs, pages/inbox.rs, helpscout.rs.

Verification (Rule 5):

- `cargo fmt --all --check` clean; `cargo clippy --workspace
  --all-targets -- -D warnings` clean; clippy with the `sandbox`
  feature clean; `python3 scripts/gate_no_simulation.py` clean; cargo
  deny 0.20.2 advisories + licenses ok — all on the pinned 1.99.1
  toolchain.
- Tests: core 310 passed / 0 failed / 4 ignored (16 new batch-20
  regressions: namespaced ids keep chunk families apart on the same
  chunk id; the cross-family overwrite is refused loudly while
  same-family replace still works; unknown families and band overflow
  fail at mapping time; the one-time migration drops the collection,
  resets all three chunk families including attempt counters, restarts
  the re-embed pipeline for every family, and never runs twice; a
  disabled index does not consume the migration; rebuild_activity is
  enqueued after the initial sync; the maintenance repair fires once
  and only for existing mirrors; the completed event carries and
  round-trips its trigger with legacy-payload compatibility; the toast
  mapper stays silent for auto and refresh triggers across all
  outcome shapes; complete_with_result pins output to the job row and
  refuses terminal re-completion; old databases gain the result column
  lazily; the loopback maps open/waiting onto active); sandbox-feature
  suite 315 passed / 0 failed / 4 ignored; app-shell 174 passed /
  0 failed (4 new: the composer poll fills from the job's own result
  and stays empty on a resultless completion despite a newer foreign
  ai_runs row; open is the await-agent statusAfter and legacy waiting
  maps onto it; the stored waiting setting loads as open); xtask 6;
  catalog 0.
- Fresh harness `scripts/verify_batch20.py` (committed; also archived
  at /home/z/my-project/scripts/): all 45 checks pass — the namespacing
  helper with its three family bands and stride bound, the store's
  cross-family refusal, index_version 2 payloads, error propagation at
  both upsert sites, the marker-gated migration with its live-store
  gating / family resets / pipeline restart / call ordering, the
  post-sync and one-time repair enqueues with the handler still wired,
  the SyncTrigger enum on the event with serde defaults and the
  constructor/engine/timer/job-lane/refresh/manual-UI wiring, the
  Option-returning toast mapper with the shell gating, the result
  column in fresh and healed schemas with the pinned completion and
  surfaced QueueJob field, the dispatcher's capture-and-complete
  routing, the handlers' Result<Option<String>> shape, the poll's
  job-row read with the ai_runs scrape gone, and the open/waiting
  vocabulary across the dropdown, loader, submit path, and loopback.
- Runner verification: code commit `a5d0d1b` — CI run `39863156425`:
  success, both jobs (fmt/clippy/test/deny + AppImage/deb build and
  smokes, all green on the runner).
- Security: staged diff token-scanned clean before the push; remote
  unmoved at push time (fetch-first).

## Batch 21 — clean-audit items 26-30 (HIGH tier: the custom-objects form that could never create an object, the phantom field types; MEDIUM tier: the stale-thread mass-delete, phantom tags_changed events, duplicate saved replies)

**26. Custom-objects form sends numbers as JSON strings — object creation
always fails** (high|bug)
— the create/edit form serialised every field value as
`Value::String`, but `validate_properties` requires real JSON scalars:
a number field got `value.as_f64() → None` ("Invalid number") and a
boolean field `value.is_boolean() → false`, so any type with a number
or boolean field could never hold an object — creation failed on every
attempt. The form now looks each key up in the selected type's own
field definitions and coerces the trimmed input: integers parse as
JSON integers (then finite decimals as f64 — `inf`/`nan` and garbage
refuse with a clear form error before any write), booleans accept
true/false/1/0/yes/no/on/off and reject the rest, and text-ish fields
stay strings. The object table renders non-string scalars with their
JSON form instead of the string-only path that showed them blank.
pages/custom_objects.rs.

**27. Custom-objects type builder offers checkbox/url types core
rejects** (high|bug)
— the field-type chips included `checkbox` and `url`, but
`validate_properties` treats both as "Unknown field type", so a type
built with them created fine and then refused every object forever.
The builder now offers exactly the core's canonical six — text,
long_text (previously unreachable from the UI), number, date, select,
boolean. Types that already carry the phantom names keep working:
the core validates `checkbox` as boolean and `url` as text (legacy
aliases, still rejecting wrong value types and still refusing any
other unknown name), and the form coerces through the same aliases.
pages/custom_objects.rs, custom_objects.rs (core).

**28. Stale-thread deletion treats one unpaginated page as the full
thread set** (med|bug)
— the real provider's `list_threads` issued one GET on
`/v3/conversations/{id}/threads` and returned page 1 only; the sync's
stale-thread sweep then deleted every local thread whose remote id was
not on that page — on a conversation longer than one page, threads
51+ were mass-deleted from the mirror on every re-ingest. The read now
follows the HAL `_links.next` link in the same loop shape
`list_system_users` uses, stopping when a page has no next link (or
comes back empty), so the sweep compares against the real full set.
helpscout_real.rs.

**29. `tags_changed` emitted with `prev=null` on every re-ingest**
(med|bug)
— the re-ingest diff emitted `tags_changed` with `previous: null` and
`next: <current tags>`, so the comparison was `null != ["x"]` on every
pass: a phantom tag event per conversation per incremental sync,
consuming dedup keys and polluting the activity feed. The mirror's
pre-write tag set is now read (conversation_tags ⋈ tags) before the
rewrite and compared as a normalized set — sorted and deduplicated, so
a pure platform-side reorder is not an event — with only real
membership changes firing, carrying the true previous value.
sync.rs.

**30. Retried saved-reply create re-POSTs (duplicates on the
platform)** (med|bug)
— the saved-reply create job POSTed straight to
`/v2/mailboxes/{id}/saved-replies`, and the v2 API has no
client-supplied id: whenever the POST landed but the response or the
post-write re-read failed, the retry re-POSTed the same payload and
the platform gained a duplicate reply. The create is now idempotent by
its natural key (mailbox + name + text): the job pre-reads the
mailbox's saved replies, and a reply with the exact name and text is
recognized as this job's own earlier write — the POST is skipped and
the re-read completes the job into the mirror. A fresh create still
POSTs. workers.rs.

Verification (Rule 5):

- `cargo fmt --all --check` clean; `cargo clippy --workspace
  --all-targets -- -D warnings` clean; clippy with the `sandbox`
  feature clean; `python3 scripts/gate_no_simulation.py` clean; cargo
  deny 0.20.2 advisories + licenses ok — all on the pinned 1.99.1
  toolchain.
- Tests: core 317 passed / 0 failed / 4 ignored (7 new batch-21
  regressions: legacy checkbox/url fields accept objects while
  wrong-typed values and unknown field names are still refused; the
  threads read follows the HAL next link across pages and hits each
  page exactly once; tags_changed stays silent on first ingest, on
  reordered and repeated tag payloads, and on stable re-ingests while
  firing once with the true previous set on a membership change; the
  retried saved-reply create never re-POSTs after a failed re-read and
  completes via the twin check with the reply landing in the mirror);
  sandbox-feature suite 322 passed / 0 failed / 4 ignored; app-shell
  179 passed / 0 failed (5 new: the builder offers exactly the six
  canonical types; number/boolean form inputs become real JSON scalars
  (42 → number, 1.5 → 1.5, true → bool, select/text stay strings);
  garbage numbers and booleans produce clear form errors and no object;
  the truth-word matrix for booleans; legacy checkbox/url types keep
  working from the form); xtask 6; catalog 0.
- Fresh harness `scripts/verify_batch21.py` (committed; also archived
  at /home/z/my-project/scripts/): all 27 checks pass — the
  field-definition lookup and pre-write coercion with the blanket
  string serialisation gone, the int-preserving/finite-f64 number path,
  the truth-word boolean path, scalar rendering in the object table,
  the canonical six-type vocabulary with no phantom variants, the
  core's legacy aliases with the unknown-name refusal intact, the HAL
  loop shape with accumulation and the page-boundary stop, the
  pre-write tag capture ordered before the rewrite, normalized-set
  comparison on both sides with the null emission gone, the tag-name
  join, the natural-key twin match with the re-POST skip and the fresh
  POST preserved, and the re-read stage still running for every
  action.
- Runner verification: code commit `766578c` — CI run `39870507850`:
  success, both jobs (fmt/clippy/test/deny + AppImage/deb build and
  smoke, all green on the runner).
- Security: staged diff token-scanned clean before the push; remote
  unmoved at push time (fetch-first).

## Batch 22 — clean-audit items 31-35 (MEDIUM tier: the api_concurrency setting that never applied, the webhook secret cap disagreement, the team id in the user-assignee slot, the extraction parse that lost fenced answers, the extraction prompt that bypassed redaction)

**31. `api_concurrency` setting parsed/saved but never applied** (med|gap)
— the Settings form loaded and saved `api_concurrency` (DragValue,
range 1..=8) and the config layer read `SYNC_API_CONCURRENCY`, but
nothing ever called `RealHelpScoutProvider::set_api_concurrency`: every
provider — boot-built, mid-session activation, worker-built — ran both
API queues at the hard-coded default of 2 forever, and even a live call
would have resized only the conversations queue, never the docs queue.
The provider constructor now resolves the setting (stored value over
the env default; stored 0/negative clamps to 1; unparsable falls back
to the env default) so every construction path starts at the
configured width, `set_api_concurrency` resizes BOTH queues, the queue
exposes its current limit (`ApiQueue::concurrency`), and
`save_general` captures the pre-save value and pushes a change to the
live provider through the shared handle in-session. The form's save
message stays honest about what a restart would not have fixed.
api_queue.rs, helpscout_real.rs, pages/settings.rs.

**32. Webhook secret capped to 40 chars at registration but verified
with the full secret** (med|bug)
— the health check re-registered hooks with `capped_secret` (first 40
characters, the platform's rule), while the loopback's webhook ingress
verified delivery signatures with the FULL stored/env secret: any
configured secret over the cap meant the platform signed with the
truncation while the app verified with the whole string — every
delivery failed the check, ten discards silently deactivated the hook,
and the health check re-registered it into the same trap. One
canonical `webhook::effective_secret` (SECRET_CAP = 40) now lives at
the signature boundary; registration delegates to it and the loopback
resolves the stored/env secret through it before verification, so both
sides always agree. The credential form says the truncation out loud
when it stores more than the platform will keep.
webhook.rs, webhook_health.rs, loopback.rs, pages/settings.rs.

**33. Team id written into the user-assignee slot;
`assigned_team_id` unconsumed** (med|bug)
— `map_conversation` copied `assignee.id` into `assignee_id`
regardless of the assignee's type: a team-assigned conversation put
the TEAM id where only user ids belong, so the sync's users lookup
either found no match (team-assigned tickets showed "unassigned") or —
when a team and a user share a platform id — resolved to the WRONG
user. `assigned_team_id` was parsed (assignedTeam, or a team-typed
assignee) and then dropped: nothing consumed it, and the mirror's
`conversations.assigned_team_local_id` column (present since m040)
stayed forever NULL. The mapper now leaves the user slot empty for
team-typed assignees and carries the team in its own field; the sync
resolves and persists the team's local id; the inbox list, detail view
and assign combo surface the team by name ("team: Billing" instead of
a phantom "unassigned" or the wrong user), and an unmirrored team id
degrades to no-team rather than poisoning either slot.
helpscout_real.rs, sync.rs, inbox.rs, pages/inbox.rs.

**34. Attribute extraction parses raw output without
fence-stripping/json_mode** (med|bug)
— `finish_computation` ran `serde_json::from_str` on the model's raw
answer: a local model that wraps its JSON in markdown fences or
surrounds it with prose (the normal behavior of LM Studio/llama.cpp
served models) failed the strict parse, and the whole extraction
silently fell away — no attributes, no stored run, no error.
Fresh output now parses through the pipeline's canonical
`extract_json` fence-stripper (the same one `chat_json` uses), and the
request rides a new structured chat lane: `LocalAiProvider` gained a
defaulted `chat_structured` (json_mode + deterministic temperature,
matching the pipeline's lane) which `LmStudioProvider` maps onto
`chat_with_opts(response_format: json_object)`, so endpoints that
support structured output deliver clean JSON while those that don't
still work through the default with defensive parsing.
ai_attributes.rs, ai_provider.rs, ai_lm_studio.rs.

**35. Attribute extraction bypasses the prompt-redaction boundary**
(med|issue)
— `run_extraction` sent the prompt — raw customer messages, subject,
and thread text — straight to `provider.chat`, while every other AI
path (the pipeline's `chat_json`) crosses the prompt-redaction
boundary: `redaction_enabled` (default on) applies
`security::redact_text`, so card numbers, CVVs, credentials, bearer
tokens, AWS keys and private key blocks never reach the model. The
extraction now crosses the same boundary before the send (the
redacted prompt is what the model receives; redaction findings are
logged; `redaction_enabled=false` passes the text through verbatim,
honoring the setting like every other path).
ai_attributes.rs.

Out-of-band verification fix (not a plan item): the digest
`facts_collect_volume_anomaly_and_happiness` test seeded its "loud
day" at a fixed 09:00 UTC wall-clock — before 09:00 UTC that timestamp
lies in the future, outside `collect_facts`' now-ended collection
window, and the test failed (observed 2026-10-23 00:18 UTC, count 16 →
6). The loud day now seeds at *now* (same calendar day, always inside
the window); the six quiet days keep their fixed 09:00 stamps. No
production code touched. digest.rs (tests only).

Verification (Rule 5, all on the pinned 1.99.1 toolchain):
- `cargo fmt --all --check`: clean (the digest test fix initially
  missed the fmt pass and failed CI's format gate on commit `c4abd83`
  — run 39878955781, job `fmt, clippy, test, deny` failed at the
  Format check step only; the formatting followup `54185ff` is clean).
- `cargo clippy --workspace --all-targets -D warnings`: clean;
  `--features sandbox`: clean.
- Tests: core 325 passed / 0 failed / 4 ignored (10 new batch-22
  regressions: fenced and prose-wrapped extraction outputs parse with
  the run stored; the extraction prompt crosses the redaction boundary
  with the placeholder reaching the model, non-PII surviving, the
  structured lane used and the plain lane never, and
  redaction_enabled=false honored verbatim; the canonical cap asserted
  at the signature boundary including the truncation-verifies/full-
  fails pair; the over-the-cap loopback delivery verifies against the
  truncation while the full-secret signature still 401s; the stored
  setting reaches both provider queues with the live resize covering
  the docs queue, stored 0 clamping to 1 and garbage falling back to
  the default; the mapper keeps team ids out of the user slot across
  team/user/both/assignedTeam-only payload shapes; the end-to-end
  team-column regression with the id collision (user 42 ≠ team 42),
  inbox list + detail showing the team by name, the honest flip to a
  user assignment, and the unmirrored-team degradation); the digest
  volume-spike flake fix; sandbox-feature suite 330 passed / 0 failed /
  4 ignored; app-shell 180 passed / 0 failed (1 new: a changed
  api_concurrency reaches the live provider's queues in-session and a
  fresh provider construction applies the stored setting); xtask 6;
  catalog 0.
- Fresh harness `scripts/verify_batch22.py` (committed; also archived
  at /home/z/my-project/scripts/): all 30 checks pass.
- Runner verification: code commits `c4abd83` + fmt followup `54185ff`
  — CI run `39879565057` on `54185ff`: success, both jobs
  (fmt/clippy/test/deny + AppImage/deb build and smoke, all green on
  the runner).
- Security: staged diff token-scanned clean before each push; remote
  unmoved at push time (fetch-first).

## Batch 23 — clean-audit items 36-45 (MEDIUM tier: the extraction window that read the oldest deleted messages, the ledger-invisible extraction runs, the cache hit that mixed two runs' data, the draft that became a raw envelope, the unreachable interaction-strategy section, the AI rows relayed as human-written, the unbounded cluster prompt, the ledger-invisible copilot, the dangling tool_call ids, the swallowed message-insert failure)

Batch size note: the owner raised `rule.md`'s batch size from five to
ten items on 2026-10-23 (commit `8c66cb8`) — this is the first
ten-item batch.

**36. customer_messages includes soft-deleted threads, takes OLDEST 30**
(med|bug)
— the attribute extraction's message window was
`ORDER BY created_at ASC LIMIT 30` with no deleted filter: a
long-running ticket's recent context (the half that says what the
customer wants now) fell out of the window in favor of the oldest
messages, and soft-deleted threads kept feeding the prompt. The window
is now the NEWEST 30 live customer messages (`deleted_at IS NULL`),
presented oldest-first (the prompt's declared order and the
deterministic layer's `messages.first()` customer_goal both expect
chronological). Whitespace-only bodies are filtered SQL-side with an
explicit charset (`TRIM(..., char(32)||char(9)||char(10)||char(13))`)
so they cannot consume LIMIT slots the Rust-side trim would then drop
(SQLite's default TRIM only strips spaces — a tab/newline-only body
used to take a slot and vanish, quietly shrinking the window to 29).
ai_attributes.rs.

**37. Extraction runs stuck `queued`, invisible to the cost ledger**
(med|issue)
— `store_extraction_run` INSERTed bare columns into `ai_runs`; the
`status` column added by db_breadth defaults to `'queued'`, so every
extraction run landed "queued" forever with no type (default
'analysis'), no latency, no token usage, no endpoint — the cost
ledger's `WHERE status='completed'` never saw a single one. The run
row now lands as a completed `attributes` run carrying the ledger's
raw material: conversation, latency, `token_usage` (from the
provider's response) and `endpoint_id` (threaded from the backend
through `compute_with_backend` → `compute` → `finish_computation`),
and `run_extraction` returns the full `ExtractionOutput`
(content + latency + usage, latency measured around the structured
chat call) instead of just the text.
`lookup_cached_extraction` only treats completed runs as cache hits,
so pre-fix stuck rows self-heal: the next computation re-runs and
stores a proper completed row.
ai_attributes.rs.

**38. Cache-hit path mixes hash-matched run id with newest run's
analysis** (med|bug)
— `analyze_ticket`'s cache hit paired `cached.id` (the hash-matched
run) with `get_latest_analysis`'s NEWEST completed analysis for the
conversation — after the conversation evolved and its content
signature reverted, or a force re-run stored a newer row, the served
analysis came from a different run than its provenance claimed, and
the sources were the newest run's while the id pointed at the older
one. The cache hit now parses the hash-matched run's OWN
`response_json` and loads its OWN `ai_sources` rows (new
`sources_for_run`); an unparseable cached row degrades to a fresh
computation instead of serving mismatched data.
ai_pipeline.rs.

**39. Draft fallback stores the raw JSON envelope as the draft**
(med|bug)
— when the model's answer parsed as JSON but carried no "draft" key
(a wrong-key envelope like `{"reply": ...}`), the fallback stored
`res.raw` — the whole envelope — as the customer-facing draft. The
raw text is now only accepted when NO parseable JSON exists at all
(a plain-prose answer); a parsed envelope without a usable draft
field fails loudly ("The local model returned JSON without a draft
field") with the run marked failed and no draft row created. A bare
JSON string answer still counts as the draft.
ai_pipeline.rs.

**40. Interaction-strategy prompt block hard-coded None; section
unreachable** (med|gap)
— `build_interaction_strategy_block` returned a literal `None`, so
the draft prompt's COMMUNICATION APPROACH section — and the entire
interaction-recommendation machinery feeding it — never reached a
prompt. The block now renders real strategy data: the conversation's
stored AI interaction recommendation, falling back to the
deterministic `heuristic_recommendation` computed from its stored
signals (with baseline, changes and human overrides), plus the
customer's durable communication preferences and the facts they
already provided in this conversation (via `extract_provided_facts`
over the newest customer threads, so the draft never re-asks).
ai_pipeline.rs.

**41. get_customer_memory relays AI rows as "human-written"**
(med|issue)
— the tool's summary note claimed "human-written entries are the only
persisted rows" while the memory extraction writes `source='ai'`
rows into the same `customer_memory` table — the consuming model
treated machine-extracted facts as human statements. The note now
says exactly what each entry's own `source` means: 'human' rows were
written by support staff; 'ai' rows are machine-extracted,
confidence-tagged and not human-verified, to be treated as leads to
verify rather than customer statements.
ai_tools.rs.

**42. Cluster prompt has no conversation cap → context blowup at
volume** (med|issue)
— `cluster_issues` loaded EVERY conversation in the window into one
prompt; a busy week of thousands of tickets blew the model's context
window and the run failed. The list is now capped to the newest 250
conversations of the window (SQL LIMIT, newest-first, presented in
stable number order), and the prompt says the sampling out loud ("the
window holds N conversations; only the newest 250 are listed (context
cap). Cluster only the listed ones.") so the model does not treat the
sample as the population. `build_issue_cluster_user` gained the
window's true conversation count (a cheap COUNT query) to decide
whether the note is needed.
ai_pipeline.rs, ai_prompts.rs.

**43. Copilot runs invisible to the cost ledger (no usage/endpoint)**
(med|bug)
— the copilot chat completed its `ai_runs` row through bare
`complete_run` (no token usage, no endpoint) while the backend
destructure dropped `endpoint_id` on the floor: every copilot chat
was invisible to `usage_ledger`. Usage is now accumulated across
every chat round — tool rounds plus the final answer, including the
budget-forced final round — and recorded on the run row together
with the backend's endpoint id through `complete_run_with_usage`.
copilot.rs.

**44. Tool-budget cutoff leaves dangling tool_call ids (400 on strict
servers)** (med|bug)
— when a model round asked for more tool calls than the budget
allowed, the execution loop `break`ed: the assistant message carried
N tool_calls but only the executed ones got tool-role responses, and
the follow-up request — with dangling ids — was rejected by strict
OpenAI-compatible servers (400). Every un-executed tool_call now gets
an explicit refusal result ("Tool budget reached - this call was not
executed. Answer from the evidence you already have.") carrying its
id, so the wire invariant holds: every assistant tool_call id is
answered.
copilot.rs.

**45. insert_message swallows INSERT failure, echoes stale rowid**
(med|bug)
— `insert_message` did `let _ = conn.execute(INSERT ...)` and then
echoed `conn.last_insert_rowid()` — on a failed INSERT (disk error,
schema drift) the response presented the id of whatever row was last
successfully inserted on the connection as the new message's id. The
INSERT result is now checked: success reports the row's real id; a
failure reports a null id with the error text surfaced through
`persist_error` (and a warning logged), never a fabricated one.
copilot.rs.

Verification (Rule 5, all on the pinned 1.99.1 toolchain):
- `cargo fmt --all --check`: clean.
- `cargo clippy --workspace --all-targets -D warnings`: clean;
  `--features sandbox`: clean.
- Tests: core 336 passed / 0 failed / 4 ignored (11 new batch-23
  regressions: the extraction window is the newest 30 live messages,
  chronological, with soft-deleted and whitespace-only threads
  excluded and the oldest falling out; a fresh extraction lands as a
  completed 'attributes' run with type/status/latency/token_usage/
  endpoint/conversation all asserted plus the ledger row; a stuck
  'queued' row with a matching hash is a cache miss that self-heals
  into a completed row; the ticket-analysis cache hit serves the
  hash-matched run's own analysis and sources under its own id; the
  wrong-key envelope fails loudly with a failed run and no draft row
  while a prose answer still becomes the draft verbatim; the
  interaction-strategy block renders the heuristic path with
  COMMUNICATION APPROACH and ALREADY PROVIDED sections and then the
  stored AI recommendation's tone/strategy/avoid; the customer-memory
  note no longer claims human-only and both source labels relay; the
  cluster prompt declares the cap when the window exceeds the sample
  and stays silent otherwise; the copilot run row accumulates usage
  across both rounds (17/8/25 from 10/5/15 + 7/3/10) with the
  endpoint recorded and the ledger row visible; the budget refusal
  answers every one of 10 requested tool_call ids with 8 executed +
  2 explicit refusals asserted on the follow-up request wire; and
  insert_message reports a real id on success and null +
  persist_error when the INSERT fails); sandbox-feature suite 341
  passed / 0 failed / 4 ignored; app-shell 180 passed / 0 failed;
  xtask 6; catalog 0.
- Fresh harness `scripts/verify_batch23.py` (committed; also archived
  at /home/z/my-project/scripts/): all 37 checks pass.
- Runner verification: rule-2 commit `d65fa3c` — CI run `39885201407`:
  success; code commit `c7a753e` — CI run `39890385424`: success, both
  jobs (fmt/clippy/test/deny + AppImage/deb build and smoke, all green
  on the runner).
- Security: staged diff token-scanned clean before each push; remote
  unmoved at push time (fetch-first).

## Batch 24 — clean-audit items 46-55 (MEDIUM tier: the batch poisoned by one empty vector, the email patch that deleted every stored address, the plaintext temp that leaked on error paths, the reset that left customer documents on disk, the blocklist that CSS comments walked through, the DNS lookup that blocked the executor, the shared tag that was never shared, the at-risk count that judged replied tickets by the wrong target, the trend verdict that could never say "new", the backlog jump measured against a baseline that forgot the work that was done)

**46. Knowledge-chunk batch poisoned by one empty vector** (med|bug)
— the knowledge_chunks qdrant lane gated the whole batch on the FIRST
vector being non-empty, then upserted EVERY pending chunk —
substituting an empty vector for any chunk the model skipped — and
stamped the entire batch 'indexed'. One empty vector poisoned the
batch: a zero-dim point landed in the store and its chunk was marked
done, never retried. The lane now partitions the batch per chunk:
only chunks with a real vector are upserted and marked 'indexed';
chunks the model left empty are marked 'failed' and stay in the
retry pipeline; the all-empty corner upserts nothing. (The docs and
conversation lanes already filtered correctly; the local fallback
path was already per-chunk.)
embeddings.rs.

**47. Customer email patch deletes ALL stored emails, errors swallowed**
(med|bug)
— `update_customer`'s email handling did `DELETE FROM customer_emails
WHERE customer_id = ?1` then inserted the patched address, both with
`let _ =` — one patch of the PRIMARY email destroyed every secondary
address the platform sync had recorded (personal/other), and a failed
INSERT silently left the customer with none. The patch's email field
is the PRIMARY address: the old primary is captured before the UPDATE
and only that one VALUE is replaced in the set (case-insensitive
compare, every other row survives), clearing the primary leaves the
set untouched, and both statements' failures now propagate.
people_store.rs.

**48. verify_bundle leaks decrypted plaintext temp on error paths, no
sweep** (med|bug)
— the decrypted `.import-*.tmp` database was removed only on
verify_bundle's success path; a failed snapshot_info, schema-version
read, or schema check `?`-bailed past the cleanup and the fully
decrypted database stayed in the bundles dir forever, and nothing
ever swept leftovers from crashed runs. The temp is now removed on
EVERY path (after the closure, with the snapshot connection dropped
first for Windows open-handle safety), and every decrypt entry point
(verify + import share `decrypt_to_temp`) sweeps stale leftovers:
`.import-{millis}.tmp` embeds the creation time, and anything at
least an hour old (a verify/import finishes in seconds) is deleted —
fresh temps are never touched, so a concurrent operation is safe.
encrypted_sync.rs.

**49. Workspace reset leaves downloaded attachment files on disk**
(med|gap)
— the reset purged the `attachments` table and the vector store but
left `data_dir/attachments` untouched: customer documents kept
sitting in the data directory after the user wiped the workspace. The
reset now empties the directory with the same best-effort posture as
the vector purge (regular files removed, an undeletable entry warns
and never fails the reset, subdirectories left to their owner), and
the report counts what was removed
(`ResetReport.attachment_files_deleted`).
data_reset.rs.

**50. CSS blocklist bypassable via escapes/comments** (med|issue)
— the dangerous-CSS regex ran on the RAW style attribute value. CSS
comments may appear between any two tokens (`ur/*x*/l(` still means
`url(`) and backslash escapes spell any character (`\75 rl(` is
`url(`, `behavior\3a` is `behavior:`), so both families sailed past
the blocklist while the CSS engine still read the payload. The value
is now normalized first — comments stripped, escapes decoded to a
fixpoint (hex up to 6 digits + consumed whitespace, or literal
`\<char>`) — and the blocklist runs on the decoded form of everything
that can survive the 2000-char clip; the original clipped value is
kept only when the decoded form is clean. Layered-escape evasion
(`\5c` decodes to a backslash, building the next layer) still
carrying escapes after 64 productive decode rounds is rejected
outright — each productive round strictly shortens the value, so a
legitimate stylesheet never reaches the bound.
security.rs.

**51. Blocking DNS resolution inside async contexts** (med|issue)
— `validate_ssrf_full` blocks the thread on getaddrinfo, and the two
async connector lanes called it directly on the executor:
`validate_config` (the settings-page URL check) and `fetch_rows` (the
http branch of the async refresh pipeline) — a slow resolver stalled
every task on that worker. Both now call the new
`validate_ssrf_full_async`, whose literal checks stay inline (pure
CPU) and whose DNS resolution rides the blocking pool via the
existing `check_url_resolved` (spawn_blocking). The sync guard stays
for genuinely sync callers (data_tools, sync-side checks).
security.rs, connectors.rs.

**52. shared_tag display query joins a conversation's tags to itself**
(med|bug)
— the repeated_unresolved_interactions finding named a "shared" tag
via a query that constrained BOTH ct1 and ct2 to the current
conversation: the self-join returned an arbitrary tag of the current
conversation (alphabetically first), which none of the related
conversations might carry. The query now returns a tag that exists on
the current conversation AND on at least one related conversation of
the same customer (same 90-day window, liveness and deleted rules as
the related list), so the named tag is genuinely shared.
quality.rs.

**53. waiting_stats.at_risk counts breached-vs-first-response for
replied tickets** (med|bug)
— the SLA report's waiting panel judged every open ticket, replied or
not, against the first-response target: a long back-and-forth ticket
whose first response happened days ago was flagged "at risk" for
missing a target it had already met. The replied flag now rides along
in the waiting query and each conversation is judged against the
target it can actually miss — first-response while the first response
is still owed, resolution once an agent has answered (the same rule
the alerts lane applies); with no target configured the count stays
0, and the avg/oldest aggregates are unchanged.
sla.rs.

**54. Radar trend precedence: "new" unreachable for ≥3 clusters**
(med|issue)
— the trend classification tested the rising band before "new", and a
brand-new cluster (previous = 0, recent ≥ 3) always satisfied
`recent > 0 * 1.3`, so every genuinely new cluster with three or more
conversations was reported "rising" and "new" was unreachable exactly
where it matters most. "new" (previous = 0, recent > 0) is now
checked first; the rising (≥3 recent, >1.3× previous), falling
(<0.7× previous with a prior window) and stable bands keep their
meanings.
analytics.rs.

**55. backlog_jump compares against a false week-ago baseline**
(med|issue)
— the digest's backlog-jump baseline counted only CURRENTLY-open
tickets created before the period start, so every ticket that was
open a week ago and got closed during the period vanished from the
baseline — a team that worked down old tickets with flat volume was
told its backlog "jumped" >50%. The honest baseline counts what was
open AT the period start: existed before the start, not deleted, and
either still open now or closed at/after the start (tickets closed
before the period stay honestly excluded).
digest.rs.

Verification (Rule 5, all on the pinned 1.99.1 toolchain):
- `cargo fmt --all --check`: clean.
- `cargo clippy --workspace --all-targets -D warnings`: clean;
  `--features sandbox`: clean.
- Tests: core 364 passed / 0 failed / 4 ignored (28 new batch-24
  regressions: the mixed-batch embedding pass against a mock
  embedding server — the skipped chunk fails while the healthy one
  indexes, with exactly one point in the store — plus the all-empty
  corner; the email patch preserving the platform-synced secondary
  address, clearing the primary keeping the set, and a dropped
  customer_emails table failing the patch loudly; the crafted-garbage
  bundle leaving no plaintext temp behind, the successful verify
  leaving none either, and the sweep taking only the two-hour-old
  temp; the reset removing both downloaded attachment files while an
  unrelated data-dir subdirectory survives; comment-, escape-, and
  layered-escape style bypasses blocked, benign styles (including
  `content: "\2014"`) surviving, and the decoder's engine-view
  assertions; the async guard's deterministic offline verdicts (loop
  back/private/IPv6/file refused, public literal accepted) exercised
  end-to-end through connectors::validate_config; the shared tag
  being genuinely shared — the current-only tag never named — and no
  finding firing once nothing is shared; the replied ticket judged by
  resolution and the unreplied by first-response under mirror
  configs, with no targets still zero; brand-new clusters with 3+
  recent conversations saying "new" while established clusters keep
  rising/falling/stable; and the worked-down flat backlog not being
  a jump, the genuine 10→16 jump still firing, and pre-period
  closures staying excluded from the baseline); sandbox-feature suite
  369 passed / 0 failed / 4 ignored; app-shell 180 passed / 0
  failed; xtask 6; catalog 0.
- Fresh harness `scripts/verify_batch24.py` (committed; also archived
  at /home/z/my-project/scripts/): all 44 checks pass; the batch-23
  harness still passes.
- Runner verification: rule-2 commit `d3f51d9` — CI run `39894038354`:
  success; code commit `8dbd43c` — CI run `39900557760`: success, both
  jobs green (fmt/clippy/test workspace + sandbox + deny, and the
  AppImage/deb build with both boot-chain smoke tests).
- Security: staged diff token-scanned clean before each push; remote
  unmoved at push time (fetch-first).

## Batch 25 — clean-audit items 57-66 (MEDIUM tier: the trigger pair that fired on every customer thread, the knowledge lane a single multi-chunk document could flood, the fused ranking that buried its own semantic hits, the vector points that outlived their chunks, the type edit that bricked its objects, the mention parser that disagreed with its scanner, the sanitizer that leaked the text it claimed to drop, the rebuild that wrote every thread twice, the sweep cursor that sailed past its backlog, and the text extractor that swallowed bodies and double-decoded)

**57. new_conversation + customer_reply both fire on every customer
thread, no once-guard** (med|bug)
— `on_conversation_changed` fired BOTH the `new_conversation` and
`customer_reply` triggers whenever the newest non-deleted thread was
from the customer: a customer's reply re-ran "new conversation" rules
(auto-acks re-sent), the very first customer message already fired
"customer reply" rules, and a tag change or status flip on a
customer-latest conversation re-fired the pair. The pair now carries a
once-guard: `conversations.automation_last_customer_thread_id`
(migration m060, boot step 51) records the newest customer thread whose
firing the conversation has consumed — the first customer thread ever
seen fires `new_conversation` (once per conversation), every later one
fires `customer_reply` (once per thread), and a sync that delivers no
new customer thread fires neither. The decision lives in
`automation::customer_thread_trigger_decision` (a pure function,
pinned by its own decision-matrix tests); a failed firing stays
unrecorded so the next change retries it, and the AI-enqueue block
that shares the guard's outer condition is unchanged.
automation.rs, workers.rs, bootstrap.rs.

**58. Knowledge FTS lane never dedupes per document** (med|bug)
— `fts_knowledge` holds one row per CHUNK, and `search_knowledge_raw`
LIMITed raw chunk rows: a document split into ten matching chunks
consumed ten of the twenty slots (half the "top 20" could be one
document) while single-chunk documents were pushed out entirely. Rows
are now consumed rank-ordered and deduped per document — the best
(lowest-rank) chunk speaks for its document — until the requested
number of DISTINCT documents is collected (the lazy iterator stops
there, so nothing over-reads). The visibility filter is unchanged.
search.rs.

**59. Fused RRF scores sorted against FTS-only 1.0 scores** (med|bug)
— `search_with_semantics` ends by sorting the whole response by score,
but only the tickets/knowledge lanes get fused RRF scores
(≈ 0.016–0.033); the never-fused lanes (customers, issues,
saved_replies, ai) kept their FTS-era constant 1.0, so every one of
them outranked every fused hit — the response said "BM25 + sqlite-vec
fused" while the semantic results sat at the bottom of the list. When
fusion ran, the surviving FTS-only lanes are re-scored onto the same
RRF scale (1/(k + rank + 1) by lane rank — exactly how the fusion
loop scored their FTS halves), so the final sort interleaves lanes
meaningfully: a hit found by both lists (≈ 0.0325) outranks any
single-lane hit (≈ 0.0164). No fusion → nothing is re-scored.
search.rs.

**60. Orphaned vectors never cleaned; stale chunk text keeps matching**
(med|bug)
— every re-chunk lane (`rechunk_conversation`, `rechunk_doc_article`,
the knowledge re-import and delete) replaces chunk rows with NEW ids,
and none of them could reach a vector-store handle — the old points
stayed in the store forever, serving their stale text snapshots to
the semantic lanes (conversation/docs hits serve the payload text
directly) or crowding the dense top-k away from live chunks. The
store gains `prune_family_except` (batched deletes inside one
transaction, both halves: vec table + payload table) and a
`VectorIndex::prune_orphaned_points` facade; the sweep runs from
`embed_pending_chunks` — which holds both the main database (the live
chunk ids → namespaced point ids) and the store — BEFORE the pending
check, so pure deletions (nothing left to embed) are swept too; the
knowledge-delete UI path enqueues the embed pass a pure deletion
otherwise never gets. All three families are covered (their re-chunk
lanes all enqueue embed jobs).
vectorstore_sqlitevec.rs, embeddings.rs, app-shell/pages/knowledge.rs.

**61. Removing a type field bricks property updates of existing
objects** (med|bug)
— `patch_object` merged the object's STORED properties into
`validate_properties`, which rejects unknown keys — the moment a type
lost a field, every existing object still storing that field's value
failed EVERY future patch with "Unrecognized key: …" (even patches of
unrelated fields, even title-only patches, which also re-validate).
Stored keys whose fields no longer exist are dropped from the merge
before validation (the object's stored data converges to the current
schema on write); the patch INPUT is still validated strictly —
setting a field the type does not have remains an error.
custom_objects.rs.

**62. parse_mentions diverges from scan_for_mentions (ASCII-only, no
boundary)** (med|bug)
— the notification parser and the display scanner disagreed about
what a mention IS: parse_mentions tokenized on ASCII only (a username
like "josé" was recorded by the scanner but never resolved to a
notification — the token stopped at 'é'), had no left boundary (the
'@' inside "support@jane.doe" fired a mention for user jane.doe), and
dropped the `@team:name` prefixed form entirely (the token stopped at
the colon and the bare-name window never matched a "team:…" candidate
— the scanner recorded "@team:billing" in the thread's mentions_json
while nobody was notified). The parser now: takes Unicode word
characters in tokens (scanner `\w` parity; '.'/'-' stay for the
directory's dotted name keys), skips an '@' preceded by a word
character, resolves the `@team:([\w-]{1,64})` run the scanner's regex
matches (consuming it whole even when the team is unresolvable), and
never resolves a bare "@team" as a USER mention (mirroring the
scanner's exclusion). The bare multi-word team-name window matching
stays — parse's richer team form the regex cannot express.
mentions.rs.

**63. rm_tags is a no-op in ammonia 4.x; title/textarea content leaks
visible** (med|bug)
— `sanitize` called `.rm_tags([scripty family])`, but in ammonia 4.x
`rm_tags` removes tags from the ALLOWLIST — a no-op for tags that
were never allowed. Ammonia's default strips disallowed tags but
keeps their content, so the text inside a pasted `<title>`,
`<textarea>`, `<noscript>`, `<select>`… rendered as visible spans in
the thread body (the function's own doc comment always claimed
"scripty tags together with their content"). The family now rides
`add_clean_content_tags`, whose entire subtree ammonia removes
(script/style remain covered by the builder defaults); no allowlisted
tag overlaps the list, so the documented panic condition cannot
trigger. richtext.rs.

**64. rebuild_indexes double-inserts every thread into fts_threads**
(med|bug)
— `rebuild_indexes_tx` ran the per-conversation indexer (which writes
each non-deleted conversation's threads into fts_threads) and THEN a
bulk `INSERT INTO fts_threads SELECT … FROM conversation_threads` —
every live thread landed in the index twice per rebuild, doubling the
FTS5 corpus statistics and skewing rank across every thread search.
The bulk insert is gone; the per-conversation loop is the single
writer (threads of deleted conversations, which the loop skips, stay
out of the index exactly like the search lanes' outer filters expect)
and the fts_indexed bookkeeping UPDATE is unchanged. The sync
engine's own backfill already guarded with NOT EXISTS — the rebuild
was the only double-writer. search.rs.

**65. Notification sweep advances cursor past unprocessed events
(>500 skipped forever)** (med|bug)
— `sweep` read at most 500 activity events per pass but then set the
cursor to `max_event_id` unconditionally: any backlog beyond 500 rows
was skipped FOREVER (the cursor sailed past it; the next sweep started
at the ceiling). The event lane now reads one row PAST the cap
(EVENT_SWEEP_CAP): when the surplus row exists, only the first 500 run
and the cursor parks at the 500th row's id — the remainder is next
sweep's work; when the read matched everything (or nothing —
non-matching event types can never be selected by the filter), the
cursor may safely advance to the ceiling. The first-run anchor
(cursor := max_event_id on a fresh install, skip history) is
untouched. notification_sweep.rs.

**66. textutil: bare `<` swallows the rest of the body; double entity
decode** (med|bug)
— two defects in `html_to_text`: (a) ANY '<' switched the scanner
into tag mode — "price <100 USD…", "a < b" — and with no closing '>'
the rest of the body vanished from every AI prompt, quality metric
and interaction signal built on the text; (b) entities decoded
through sequential `str::replace` calls, so `&amp;lt;` became `&lt;`
by the `&amp;` pass and then `<` by the `&lt;` pass — a double decode
of text the author had carefully escaped. A '<' now opens a tag only
when the HTML5 tokenizer would say so (followed by a letter, '/', '!'
or '?'); anything else is literal text, and a plausible tag opening
that never closes within 500 bytes is flushed back out as text
instead of swallowing the document. Entities decode in a single pass
(the known set is unchanged — the fix is the decoder, not the
vocabulary): an '&' commits to a decode only when a ';' terminates a
known candidate, everything else is emitted verbatim and never
re-scanned. textutil.rs.

Verification (Rule 5, all on the pinned 1.99.1 toolchain):
- `cargo fmt --all --check`: clean.
- `cargo clippy --workspace --all-targets -D warnings`: clean;
  `--features sandbox`: clean.
- Tests: core 395 passed / 0 failed / 4 ignored (31 new batch-25
  regressions: the once-guard decision matrix — first-ever customer
  thread → new_conversation, later → customer_reply, consumed →
  silence, no customer thread → silence — and m060's idempotent
  column; the multi-chunk document occupying ONE slot while
  single-chunk documents stay visible and the dedupe respecting the
  visibility filter; the FTS-only lanes re-scored onto the RRF scale
  with a both-lists fused hit outranking single-lane hits, and the
  helper leaving fused-lane scopes alone; the orphan sweep removing
  exactly the dead points of one family (other families untouched,
  idempotent, empty-live-set wipes the family) and the facade
  removing the stale point while a disabled index reports
  unavailable; the type-field removal no longer bricking patches
  (unrelated-key and title-only patches survive, the stale stored key
  drops on write) while unknown INPUT keys still fail; José resolving
  on Unicode tokens, the email not firing, @team:billing and
  @team:tier-2 resolving, an unresolvable @team: consuming its run
  without leaking the tail, bare @team never a user mention, and the
  scanner/parser agreeing on a mixed body; title/textarea/noscript/
  select content no longer leaking into rendered spans with
  surrounding content surviving and script/style subtrees still
  removed; the rebuild writing three live threads as exactly three
  fts rows that still match; the 502-event backlog processing exactly
  500 with the cursor parked at the 500th row (the 501st untouched
  until the next sweep drains it and reaches the ceiling), the small
  backlog advancing fully, and non-matching events alone still
  advancing the cursor; and the bare-'<' bodies surviving verbatim,
  the runaway never-closing tag flushed back as text, entities
  decoding exactly once (&amp;lt; stays &lt;), unrecognized entity
  runs staying verbatim, and the known entities decoding as before);
  sandbox-feature suite 400 passed / 0 failed / 4 ignored; app-shell
  180 passed / 0 failed; xtask 6; catalog 0.
- Fresh harness `scripts/verify_batch25.py` (committed; also archived
  at /home/z/my-project/scripts/): all 50 checks pass; the batch-24
  harness still passes.
- Runner verification: rule-2 commit `4fb70a1` — CI run `39904667467`:
  success; code commit `ddcf3e2` — CI run `39910801578`: success, both
  jobs green (fmt/clippy/test workspace + sandbox + deny, and the
  AppImage/deb build with both boot-chain smoke tests).
- Security: staged diff token-scanned clean before each push; remote
  unmoved at push time (fetch-first).

## Batch 26 — clean-audit items 67-76 (MEDIUM tier: the translation cache that never pruned, the audit trail nothing ever showed, the toasts that expired behind a hidden window, the confirm dialog that was never armed, the smoke that returned before the renderer existed, the palette Enter that stranded the query field, the transform that clobbered in-flight typing, the role-model edit that saved per keystroke, the copilot that submitted on any Enter, and the sync invalidation that destroyed the open Library form)

**67. translation_cache never pruned; retention policy omits it**
(med|gap)
— `enforce_retention` swept `webhook_events`, `application_errors`,
`audit_log` and `ai_runs` — but not `translation_cache`: every cached
translation lived forever (the cache is written on every non-cached
translate; nothing else ever removed rows) while every other
retention-covered table was pruned. The cache is now the fifth row of
the retention table list (`("translation_cache", "created_at")` — the
column the schema already stamped), so the same `retention_days`
policy that governs the rest governs the cache, and a disabled policy
(`retention_days <= 0`) still prunes nothing. maintenance.rs.

**68. Audit trail never surfaced; dead reader drops non-JSON
payloads** (med|gap)
— `audit_log` was written all along (the outbound lane, the people
store, the copilot, the side-thread writers) but nothing in the shell
ever read it, and the only reader — `list_audit` — silently decoded a
non-JSON stored payload to `null`, dropping the recorded state. The
reader now surfaces a non-JSON `before_state`/`after_state`/
`remote_result` verbatim as a JSON string instead of decoding it to
null, and the Diagnostics page gains an "Audit trail" card: the newest
50 audited changes (who, what, when, which conversation, AI
involvement) plus the recorded before/after states and remote
outcomes, reloading with the page like the event log and sync reports
beside it. audit.rs, pages/diagnostics.rs.

**69. Toasts expire on wall-clock while hidden (errors silently
lost)** (med|bug)
— eframe 0.37 runs no ui pass at all while the window is hidden (only
`logic`), so a toast's wall-clock `expires_at` deadline kept running:
an error raised while the window was hidden expired unseen and
vanished silently the moment the window returned — the single most
important toast ("it did not go through") included. A toast's lifetime
is now a `remaining` budget that starts at the FIRST rendered pass
(`last_paint` is `None` until then — a toast pushed while hidden waits
with its full duration) and burns only across rendered passes, each
pass's burn capped at one repaint heartbeat (`MAX_PASS_GAP`, 1.6 s):
steady-state expiry keeps its wall-clock pace, a hidden window cannot
consume a lifetime, and a toast rendered before hiding burns at most
one heartbeat on return. The old deadline field is gone.
toasts.rs.

**70. Confirm-dialog system never armed; destructive actions
unconfirmed** (med|gap)
— the `ConfirmDialog` machinery existed and rendered, but nothing ever
armed it: every destructive action in the shell ran unconfirmed — the
automation page's delete button deleted the rule on the first click.
Pages can now raise `NavRequest::Confirm(ConfirmRequest)` carrying a
modeled `DestructiveAction` plus the dialog spec (title, message,
confirm label, danger); the automation delete button raises the
request instead of deleting, the shell arms the dialog and remembers
the action (`pending_confirm`), and ONLY a `DialogOutcome::Confirmed`
executes it — the confirmed execution routes the automation delete
through core, invalidates the owning page and toasts the outcome; a
cancelled dialog leaves the world untouched, and a dialog armed
outside the mechanism (no action) still reports its decision as
before. The remaining destructive surfaces (knowledge docs, rules,
types, objects, connectors, segments, clear-log — audit item 140) will
route through this mechanism in their own items.
pages/mod.rs, app.rs, pages/automation.rs.

**71. SPP_SMOKE returns before shell/renderer boot — renderer
regressions pass CI** (med|gap)
— the CI smoke hook returned after the boot chain (migrations, boot
services, workers, loopback) and BEFORE the shell/renderer existed: a
renderer regression could not fail CI because the binary under test
never rendered a frame. `run_shell_smoke` now constructs the REAL
`ShellApp` headlessly — the new `new_with_context` constructor takes
just an `egui::Context` (everything the shell builds at startup needs
no eframe window; the eframe `new` delegates to it), and the `logic`/
`ui` trait bodies live in `logic_pass`/`ui_pass` so the harness drives
the exact code eframe drives — then renders one full pass per page
key (plus the default Dashboard boot pass) with a non-empty-paint
assertion per pass: a panic or a blank frame fails the smoke, the
smoke exits non-zero, CI goes red. Verified end-to-end:
`SPP_SMOKE=1` binary exit 0, 24 pages rendered, 4043 shapes painted.
smoke.rs, main.rs, app.rs, lib.rs.

**72. Palette Enter on zero hits defocuses the input; keystrokes
swallowed** (med|bug)
— egui 0.37 single-line TextEdits surrender focus on Enter, and with
zero hits nothing consumed the key: the query field sat defocused and
every keystroke after a fruitless Enter was swallowed until the user
clicked back in. A dry Enter (take with an empty hit list) now
immediately re-grabs focus for the query field — typing continues
without a click; the hit-bearing Enter path (confirm-and-close) is
untouched. palette.rs.

**73. Transform completion overwrites typing made in flight**
(med|issue)
— a finishing rewrite/translate job replaced the composer wholesale,
clobbering whatever the user typed while the job ran (the very text
they were re-working). `PendingComposerAi` now snapshots the composer
text at enqueue (`origin_body`); on completion the result only
replaces the composer when the text still equals that origin — an
in-flight edit keeps the user's text and says so out loud ("you edited
the draft while it ran — your text was kept", a status message, not an
error), while the job still resolves and the untouched path still
lands the result. pages/inbox.rs.

**74. Role-model field persists + enqueues golden_eval per
keystroke** (med|bug)
— every keystroke in the model text field ran `save_roles` plus a
`golden_eval` enqueue: typing "qwen2.5-7b" was eleven saves and eleven
eval jobs. `RoleAssignment`/`RoleMap` now derive `PartialEq`/`Eq`; the
page tracks `hub_roles_saved` (the last PERSISTED map); keystrokes
only sync the live map in memory (the field's egui state and the page
agree — no revert, no focus loss); the persist + eval ride a blur
commit (`model_response.lost_focus() && roles != saved`) and the
endpoint combo's discrete selection; and `commit_role_map` — the one
shared commit method — is itself idempotent: an unchanged map is a
no-op (no second save, no second eval), so the eval count tracks real
edits no matter the caller. ai_hub.rs, pages/ai_center.rs.

**75. Copilot submits on a global Enter with no focus check**
(med|bug)
— the copilot submit listened for ANY Enter on the AI Center page: a
stray Enter with no field focused (or mid-typing in another field)
fired the chat job. The submit now rides the copilot input's OWN focus
loss (`response.lost_focus() && Enter && !input.is_empty()` — the
single-line TextEdit surrenders focus on Enter, so an Enter in the
field is a lost_focus event) while the button-click path is untouched.
pages/ai_center.rs.

**76. Sync invalidation destroys the open Library edit form**
(med|issue)
— `OperationsPage::invalidate` set `self.library = None`: a sync event
landing while the user had the saved-reply edit form open wiped their
typed name and text, the picked mailbox chip, and the typed workflow
number — the form was gone on the next render. The Library model now
carries a `stale` flag: invalidation marks the mirror data stale
instead of dropping the model, and a stale model re-reads its rows on
the next render while carrying the user's in-progress state forward —
the open edit form, the typed workflow number, and the picked mailbox
(clamped to the new mailbox list); only the mirror rows refresh.
pages/operations.rs.

Verification (Rule 5, all on the pinned 1.99.1 toolchain):
- `cargo fmt --all --check`: clean.
- `cargo clippy --workspace --all-targets -- -D warnings`: clean;
  `--features sandbox`: clean.
- Tests: core 398 passed / 0 failed / 4 ignored (3 new batch-26
  regressions: the 400-day-old translation-cache row pruned while the
  1-day-old row survives and a disabled policy pruning nothing; the
  plain-text audit payloads surfacing verbatim instead of null with the
  JSON row intact and the conversation filter narrowing); sandbox
  suite 403 passed / 0 failed / 4 ignored; app-shell 194 passed /
  0 failed (14 new batch-26 regressions: the audit-trail card listing
  recorded changes; the hidden window never burning an unrendered
  toast's lifetime, the hidden gap burning at most one heartbeat, and
  consecutive rendered passes burning real time; the Confirm request
  carrying the dialog spec and action, the page half where cancel
  keeps the rule and only the confirmed execution deletes, and the
  shell half where arming deletes nothing, cancel keeps the rule, and
  confirm executes with a toast; the shell rendering every page
  headlessly with non-empty frames; the palette dry Enter keeping the
  query focused with the next keystroke landing; the transform
  keeping in-flight typing with the decision said out loud while the
  untouched composer still receives the result; ten rendered frames
  of a divergent live map persisting nothing and enqueueing no eval,
  and a committed edit persisting once with exactly one eval while an
  identical re-commit is a no-op; the global Enter with nothing
  focused enqueuing no copilot job and keeping the input's text; and
  the Library invalidation keeping the typed name/text/workflow
  number while the mirror rows re-read underneath); xtask 6; catalog 0.
- End-to-end smoke: `SPP_SMOKE=1` on the freshly built binary —
  exit 0, boot chain + shell/renderer verified, 24 pages rendered,
  4043 shapes painted (item 71's deliverable exercised as CI runs it).
- Fresh harness `scripts/verify_batch26.py` (committed; also archived
  at /home/z/my-project/scripts/): all 59 checks pass; the batch-25
  harness still passes.
- Session note: this batch landed across two sessions — the previous
  session's tree did not yet compile (egui-0.37 panel/Context API
  misuse in three new tests, a moved-value error in the Library
  reload, a double-unwrap in the confirm test) and two test helpers
  queried a nonexistent `jobs.kind` column that `.unwrap_or(0)` masked
  as a pass; all were corrected this session before the verification
  above ran, and `commit_role_map` gained the idempotence its
  regression test specifies (an unchanged map is a no-op).
- Security: staged diff token-scanned clean before each push; remote
  unmoved at push time (fetch-first).
- Runner verification (added after the runs): code commit `ea5f84e` —
  CI run `39967607725`: the AppImage/deb job (which runs the item-71
  shell smoke) green; the fmt/clippy/test/deny job red on
  `cargo deny advisories` ONLY — RUSTSEC-2026-0385 (ctutils < 0.4.4,
  non-constant-time mask generation), a transitive crypto dependency
  advisory published after the last green run, unrelated to the batch's
  code. Fix commit `4ca77e1` (ctutils 0.4.3 → 0.4.5 via
  `cargo update -p ctutils`, no code change; workspace tests re-run
  green on 0.4.5) — CI run `39971217122`: success, both jobs green
  (fmt/clippy/test/deny AND AppImage/deb build with the shell smoke).

### Batch 27 — 2026-10-23 (code commit `b4aab92`)

**77. Sync invalidation silently reverts unsaved Settings edits**
(med|issue)
— the pre-fix `invalidate` dropped the General form wholesale
(`general = None`), so the next render's reload re-read the persisted
values over whatever the user was typing — a sync landing mid-edit lost
their changes. The form is user state now, with the same
dirty-guard shape batch 26 gave the Library: the page tracks the last
PERSISTED snapshot (`general_persisted`); invalidation only marks the
page `stale` (connection status, business hours and backups re-read on
the next render); and `reload` keeps a DIRTY form — one that differs
from its snapshot — instead of overwriting it, while a clean or absent
form still takes the fresh read. A successful save refreshes the
snapshot, so later invalidations take fresh reads again. The tab-switch
reload, the connect/disconnect completions and the workspace reset all
flow through the same guard — no path clobbers unsaved edits.
pages/settings.rs.

**78. Inbox hard-capped at 50, no pagination** (med|gap)
— `filters_from_bar` hardcoded `limit: Some(50), offset: Some(0)`, so
conversation 51+ was unreachable while the subtitle said "50 of 1234".
The cap is now a page size: `INBOX_PAGE_SIZE` with offset paging
compiled from a `page` field, a shared `design::pager` widget under the
list (prev/next with inert boundary glyphs), a range-reporting subtitle
("51-100 of 1234"), page resets on every filter change (view chips,
apply, search, ops clear, reset filters), and a clamp when the data
shrinks underneath the current page (a past-the-end page falls back to
the last page that still holds rows — one recursive step, never a dead
end). pages/inbox.rs, design.rs.

**79. Row click silently wipes the typed reply draft** (med|issue)
— clicking another conversation row cleared the composer wholesale:
a typed reply, cc/bcc, the note/reply mode — gone with no way back.
The composer state now travels with its conversation: `select` stashes
the outgoing conversation's draft (body/cc/bcc/mode + the AI-draft
provenance, so accept/edit-distance feedback still records against the
real row) and restores the incoming conversation's stash on reselect; a
successful submit spends the stash (reselecting a sent conversation
starts clean); and clicking the row that is ALREADY open touches the
composer not at all — typing continues. pages/inbox.rs.

**80. Approvals attributed to hardcoded user 5** (med|bug)
— `decide` wrote literal user 5 as `decided_by_user_id` on every
approval and rejection, so every decision by anyone was recorded as
Ada's in the approvals table. The decision now carries the REAL acting
user — `me_remote_id` (captured at the OAuth callback) resolved through
the same `me_user_local_id` helper the notifications page uses — with
Ada kept only as the no-identity stand-in (client-credentials/demo
sessions, where no user identity exists to attribute).
pages/automation.rs.

**81. Known-issue links accept dangling ids and count them**
(med|bug)
— `create_known_issue` inserted a `known_issue_links` row for every
id in the free-text field, existent or not (the table has no FK on
conversation_id), and `refresh_known_issue_counts` counted them all —
a typo'd id inflated the issue's conversation count forever. The link
INSERT now SELECTs from conversations itself (a dangling id inserts
zero rows), the count joins the conversations table (so pre-fix
dangling rows stop counting at the next refresh), and the create form
parses + dedupes + validates the ids against the table, reporting
skipped unknown ids out loud instead of dropping them silently.
pages/issue_radar.rs, core/intelligence_features.rs.

**82. Customers page fetches page 1 only (50 cap)** (med|gap)
— `reload_search` called `list_customers_summary(conn, query, 1, 50)`
— customer 51+ was unreachable behind the hard-coded page 1. The fetch
now takes the pager's page: `CUSTOMERS_PAGE_SIZE` with a `page` field,
the shared `design::pager` under the table, a range-reporting card
title, page resets on a new search and on back-from-profile, and the
same past-the-end clamp as the inbox. pages/customers.rs.

**83. LIKE input not escaped despite ESCAPE clause** (low|bug)
— three call sites interpolated raw user text into LIKE patterns: the
inbox query filter and `search_customers` (both with a declared but
dead `ESCAPE '\'` clause — a `%`/`_` the user typed acted as a
wildcard, and a trailing `\` escaped the wrapping percent so
backslash-containing searches matched nothing) and the saved-replies
mirror search (no ESCAPE clause at all — pure wildcard injection).
One shared `pub fn escape_like` in core/search.rs now feeds all three;
the mirror query also gains its ESCAPE clause. Literal means literal:
"50%" finds only rows containing 50%, "a_b" is not a single-char
wildcard, "path C:\temp" finds the backslash row.
core/inbox.rs, core/customers.rs, core/mirror_readouts.rs,
core/search.rs.

**84. jobs::fail called with literal job id 0 (2 sites)** (low|bug)
— the `draft_article` (missing candidateId) and `fetch_hs_report`
(unknown report key) validation paths called `jobs::fail(&conn, 0, …)`:
the failure updated nothing (job 0 does not exist) and the real job row
sat in 'running' forever with its honest error written nowhere. Both
functions now receive the real `job_id` from the dispatch, and the
failure lands on the job row: status 'failed', the message on the row
where the Knowledge page and the reports UI surface it.
core/workers.rs.

**85. Rating fetch errors swallowed; ratings permanently lost**
(low|bug)
— the satisfaction-webhook job fetched the rating with
`let Ok(Some(r)) = …`, so an Err (a 5xx, a timeout — anything
transient) matched nothing: the webhook fires once, the one fetch
failed once, and the rating was lost forever while the job reported
success. The fetch outcome is now matched explicitly: Ok(Some) stores
and notifies as before, Ok(None) is an honest logged no-op (the rating
genuinely does not exist platform-side), and Err fails the job — the
queue's retry machinery re-fetches it with exponential backoff up to
max_attempts, and the error is recorded on the job row.
core/workers.rs.

**86. Panic containment marks copilot jobs retryable (re-persists
user message)** (low|bug)
— the worker-boundary panic containment called
`jobs::fail(…, retryable: true)`: a deterministic panic re-ran once
per attempt, and each re-run re-executed the job's early side effects
— a panicking copilot chat re-persisted the user's message every
retry. A panic is a code defect, not a transient fault: the contained
job now fails terminally (status 'failed', the honest panic error on
the row, the UI notification unchanged), and no side effect runs twice.
core/workers.rs.

Verification (Rule 5, all on the pinned 1.99.1 toolchain):
- `cargo fmt --all --check`: clean.
- `cargo clippy --workspace --all-targets -- -D warnings`: clean;
  `--features sandbox`: clean.
- Tests: core 409 passed / 0 failed / 4 ignored (11 new batch-27
  regressions: the literal-`%`/`_`/trailing-backslash LIKE semantics on
  all three call sites; the draft_article and fetch_hs_report failures
  landing on the real job row; the rating fetch failure requeueing
  with nothing stored; and the panicking job failing terminally with
  no second execution); app-shell 209 passed / 0 failed (15 new
  batch-27 regressions: the invalidation keeping unsaved General edits
  while a clean form still takes fresh reads and a save clears the
  guard; page one reaching conversations 51-55 with no overlap and
  full coverage, the offset compiling from the page index, and the
  past-the-end clamp; the row-click stash/restore with mode, the
  same-row click touching nothing; the approval attributed to the
  acting user with the no-identity stand-in; the dangling ids gaining
  no links, the pre-fix dangling rows not counting, and the page
  reporting skipped ids after dedupe; and the customers page two
  reaching customers 51-55 with its own clamp); xtask 6; catalog 0.
- End-to-end smoke: `SPP_SMOKE=1` on the freshly built binary —
  exit 0, boot chain + shell/renderer verified, 24 pages rendered,
  4043 shapes painted.
- Fresh harness `scripts/verify_batch27.py` (committed): all 52
  checks pass; the batch-26 harness still passes.
- Security: staged diff token-scanned clean before each push; remote
  unmoved at push time (fetch-first).
- Runner verification (added after the run): code commit `b4aab92` —
  CI run `39994546286`: success, both jobs green (fmt/clippy/test/deny
  AND the AppImage/deb build with the SPP_SMOKE shell smoke). The
  batch-opening commit `f3da4c9` and the tracking commit `a24ce00`
  also ran green (runs `39987498920` and `39994712702`: success).

### Batch 28 — 2026-10-23 (code commit `d823fcf`)

**87. Ratings upsert never re-links conversation/customer on conflict**
(low|bug)
— the satisfaction webhook routinely beats the conversation sync: the
rating is fetched and stored while its conversation/customer are not
yet mirrored, so the row lands with NULL links. Every later
re-delivery of the same rating hit the `ON CONFLICT(remote_id)` clause
that only touched `rating`/`comments`/`last_synced_at` — the NULL
links were permanent. The conflict path now upgrades them:
`conversation_id = COALESCE(excluded.conversation_id,
ratings.conversation_id)` (and the same for `customer_local_id` and
`user_local_id`), so a re-delivery with freshly-resolved ids links the
rating, while a re-fetch that cannot resolve them anymore never
downgrades an existing link to NULL. core/workers.rs.

**88. Cancelled initial sync recorded LIVE; threads backfill never
re-runs** (low|bug)
— two halves of one stranded state. (a) A cancelled initial sync fell
through to the same verdict as a completed one: a mid-run cancel with
zero resource errors stamped `sync_state = LIVE` and the run row
`finished, LIVE`, so the auto-sync timer never re-ran the initial pass.
(b) `sync_missing_threads` ran only inside the initial sync — the
conversations the cancel (or a crash, or a rate-limited page) left
thread-less were never backfilled, because incrementals did not run
the pass. Now: a cancelled run lands the state machine in
`CATCHING_UP` (incrementals continue the pull), records the run row as
`CANCELLED`, says "cancelled partway" in the audit log, and recovery
counts a CANCELLED run as data-on-record; and the bounded threads
backfill (200 conversations/pass, same bound as the initial path)
rides every incremental too, draining any stranded backlog at the sync
cadence. core/sync_engine.rs.

**89. Threads pass hardcodes failed:0 then gets stamped ok**
(low|bug)
— `sync_missing_threads` returned `failed: 0, error: None` no matter
how many conversations failed their re-fetch: the caller's
`record_success` then stamped the checkpoint `status = 'ok'`,
`last_error = NULL` right over the per-row `record_failure` the pass
had written, and the run's error count never saw the failures. The
pass now counts its unreachable conversations and returns the honest
`failed` with a summarized error, which the standard resource
bookkeeping (initial loop and incremental `finish_resource` alike)
records once; the per-row `record_failure` inside the pass is gone
(no double count), each failure stays in the warn log with its
conversation id and error. core/sync_engine.rs.

**90. Post-initial-sync AI backfill covers only first 50 conversations**
(low|gap)
— `on_after_initial_sync` capped both backfill queries at `LIMIT 50`:
with more than 50 unanalyzed conversations (any sizeable import), the
51st onward never gained an analysis or an attributes snapshot — the
hook runs exactly once after the initial sync and nothing else ever
re-selected them. Both caps are gone; every unanalyzed, non-deleted
conversation is enqueued (the ai queue serializes the work at its own
pace, the attributes lane needs no model at all).
core/workers.rs.

**91. Platform-side deletions never propagate (saved replies/orgs/
workflows)** (low|gap)
— the sync upserted these full-list resources forever without ever
noticing a row that had vanished from the platform's listing: a
deleted saved reply, workflow or organization kept serving locally
forever. The three arms now collect the seen remote ids and stamp
`deleted_at` on rows absent from the complete, successfully fetched
list (`prune_missing_rows`, one shared helper); an EMPTY list is
deliberately not a wipe signal (a degraded or permission-downgraded
answer must not blank the mirror); a re-listed row comes back to life
(the upserts clear `deleted_at`); and the saved-replies listing now
walks every page (`list_all_v2`) before pruning, so a mailbox holding
more replies than one v2 page is never falsely pruned.
core/sync_engine.rs, core/helpscout_real.rs.

**92. Dead "-> 404" mapping in get_rating** (low|bug)
— the not-found branch matched the literal `-> 404` substring of the
error's Display text — but `HsApiError`'s Display prints the FRIENDLY
message ("Help Scout reports this record no longer exists…"), so the
mapping never fired: a genuinely absent rating surfaced as an Err,
which since batch 27's item 85 means a failed, retried job instead of
the honest `Ok(None)` logged no-op every other 404-tolerant fetch
returns. `get_rating` now uses the structured `is_not_found` check
(status-code based, same as get_chat/get_conversation).
core/helpscout_real.rs.

**93. OAuth TTL sweeper + redirect-URI verifier dead; states never
expire** (low|gap)
— three dead pieces of the OAuth state machinery. (a)
`sweep_consumed` had zero callers: `oauth_states` grew forever. (b)
Unconsumed states never expired — a state issued weeks ago stayed
redeemable for the process's lifetime, while the callback's failure
page promised "it may have expired". (c) The recorded `redirect_uri`
was never verified — the table doc said "we store it here to verify"
and nothing read it. Now: states live ten minutes
(`STATE_TTL_SECONDS`), consumed or not — `consume_state` enforces the
TTL itself and the sweeper (`sweep_states`, covering abandoned-old and
consumed-old alike) runs on every new authorization; and the callback
verifies the redeemed state's redirect port against the listener that
actually serves the request (`LoopbackCtx` carries the bound port), so
a state issued for one loopback port does not redeem on another
listener. core/oauth_state.rs, core/loopback.rs.

**94. Webhook prune deletes unprocessed pending events under backlog**
(low|bug)
— `prune_webhook_events` kept the newest N rows regardless of state,
and under a backlog the OLDEST rows are exactly the pending ones: an
offline weekend's burst had its head deleted before the drain ever
reached it. Only terminally-done rows are prunable now (processed, or
failed with the 5-attempt retry budget spent); rows that still owe
processing survive at any age, and the newest `keep` of everything
stay as diagnostic history. core/webhook.rs.

**95. weekly_digest completes the ledger row despite failed narrative**
(low|bug)
— the digest path called `complete_run_with_usage` BEFORE deciding
whether the narrative was usable: a model answer with no narrative
left an `ai_runs` row saying "completed" (and priced in the cost
ledger) while the digest row said "failed". The verdict now decides
the ledger — a usable narrative completes the run with its usage, an
unusable one fails it with the honest reason, and the digest row and
the ledger row tell the same story. core/ai_pipeline.rs.

**96. Empty `{}` analysis accepted+cached; eval gate disagrees**
(low|bug)
— `parse_ticket_analysis` happily parses `{}` into an all-empty
analysis, so a model answer with no usable field was COMPLETED and
CACHED — and then served from the cache forever (the golden-eval gate
already rejected the same output: "at least one non-empty core
field"). The pipeline now enforces the identical gate
(`analysis_has_core_content`: summary/intent/customer_goal, one
non-empty suffices) before anything is persisted — an empty answer
fails the run with the honest reason and caches nothing, and the
cache-hit path applies the same filter so a legacy completed row with
an empty output degrades to a fresh computation instead of being
served forever. core/ai_pipeline.rs.

Verification (Rule 5, all on the pinned 1.99.1 toolchain):
- `cargo fmt --all --check`: clean.
- `cargo clippy --workspace --all-targets -- -D warnings`: clean;
  `--features sandbox`: clean.
- Tests: core 426 passed / 0 failed / 4 ignored (17 new batch-28
  regressions: the rating upsert re-linking on conflict and never
  downgrading an existing link; the cancelled initial sync landing
  CATCHING_UP with a CANCELLED run row and recovery counting it, the
  incremental draining a thread-less conversation, and the threads
  pass counting its unreachable conversation with an honestly failed
  checkpoint; the post-sync backfill covering all 60 conversations and
  skipping the deleted one; platform deletions propagating and
  restoring across saved replies/workflows/organizations with the
  empty-list wipe guard; get_rating mapping a real 404 to Ok(None)
  while a 400 keeps its status; the callback rejecting a state issued
  for another port, the stale state expiring, and the sweeper removing
  both dead kinds on every new authorization; the prune keeping
  pending events under backlog and pruning only terminal rows outside
  the window; the digest ledger matching the narrative verdict in both
  directions; and the empty analysis rejected-not-cached plus the
  legacy empty cache row degrading to fresh computation);
  sandbox-feature suite 431 passed / 0 failed / 4 ignored; app-shell
  209 passed / 0 failed; xtask 6; catalog 0.
- End-to-end smoke: `SPP_SMOKE=1` on the freshly built binary —
  exit 0, boot chain + shell/renderer verified, 24 pages rendered,
  4043 shapes painted.
- Fresh harness `scripts/verify_batch28.py` (committed): all 50
  checks pass; the batch-25, batch-26 and batch-27 harnesses still
  pass.
- `cargo deny check advisories licenses`: ok (the CDLA-Permissive-2.0
  note is the pre-existing unmatched-allowance warning, not a failure).
- Security: staged diff token-scanned clean before each push; remote
  unmoved at push time (fetch-first).
