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
