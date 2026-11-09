# completed.md — completed and verified items

> Items land here from `progress.md` only after Rule 5 verification. Each entry records the
> work done and the verification evidence (test run, CI run, build result).

## Completed

## Batch AIE-1 — the AI reads the Help Center and the customers' own words; reply drafting is triggerable (2026-11-09)

**Owner directive:** "the program should read all the contents in inbox,
tickets and their answers from clients and support reps, and at the same
time all the contents in help center and other sources, and it should
write the reply to clients so it can be sent if ok to client via inbox,
that should be done with help of ai connected with lm studio … when we
start writing article on the program for helpcenter it should check all
the clients response from inbox their answers to help draft the contents
using ai connected with lm studio."

**Audit verdict first (code-traced end to end):** the LM Studio wiring
is real (AI Center model-role routing, Draft role → `chat_json` with
schema); the reply pipeline reads the full thread (every customer and
agent message with author/date/kind), customer history, similar past
conversations (FTS + semantic), customer memory, known issues, saved
replies and local knowledge documents; drafts land as an "LM Studio
draft" card with Insert → composer → Send (human-approved, verified
against unsupported claims/internal leakage); article drafting grounds
in closed conversations' published agent replies via the gap → draft →
review → publish-to-Help-Scout chain. Three gaps kept it short of the
directive — fixed as items below.

**Items:** AIE-1-1 help center in the reply evidence · AIE-1-2 the
customer's question in article citations · AIE-1-3 the reply-draft
trigger · AIE-1-4 the verification-surfaced customers-golden clock pin.

**Implementation:**
- AIE-1-1 (`crates/core/src/ai_evidence.rs`): `help_center_entries` —
  a `docs_fts` search (the index the Docs sync maintains) joining
  `docs_articles` for name/text/status; published → `customer_safe`,
  draft/internal → `internal_only` and only included when internal
  evidence is allowed, so `build_customer_draft_user`'s safety filter
  keeps them out of customer replies exactly like internal local
  documents. Entries ride the existing knowledge lane (4 local + 4 help
  center max); `sources_for` cites articles as `help_center_article`
  in their own id space (never the knowledge_documents join); the
  coaching leakage auditor's internal-material corpus now also includes
  non-published help center articles linked via ai_sources
  (`coaching/checks.rs`).
- AIE-1-2 (`crates/core/src/quality/mod.rs` + `ai_prompts.rs`):
  `ResolutionCitation.customer_question` — the customer's first
  published message (HTML→text, 400-char excerpt), rendered in
  `build_article_draft_user` as `Customer asked: "…"` above
  `Resolution (agent reply):` — the drafted article now answers what
  customers actually wrote, not just the agent's subject line.
- AIE-1-3 (`crates/app-shell/src/pages/inbox.rs`): the copilot pane
  carries a "Draft reply with LM Studio" button (Ai variant) above the
  draft card it feeds, guarded by `generative_backend_available`
  (Draft-role routing; unrouted installs show the routing hint,
  matching the pane's established guard pattern); it enqueues the
  existing `generate_draft` job via `enqueue_composer_ai` and the poll
  announces "LM Studio draft ready" when the card picks the row up —
  completing generate → review → Insert → Send entirely from the UI.
- AIE-1-4 (`crates/app-shell/tests/kittest_list_pages.rs` +
  `kittest_clock_pin.rs` comment): the six customers goldens' "Nd ago"
  Last Contact column ticked a day (pre-existing — proven by a stash
  re-run at clean HEAD `d6ac37c` failing identically); the fixture now
  pins `KITTEST_PINNED_NOW_ISO` like sync_health/operations/
  notifications/reports, and the six goldens regenerated frozen.

**Regression locks:** 5 core unit tests (help center rides the
knowledge lane with the published/internal visibility split; articles
cite their own source type; the customer's question rides the citation
with the agent reply intact and the no-customer-message fallback; the
article prompt carries/omits the Customer-asked line) + the new
`aie1_inbox_copilot_draft_button_1920_light` golden with a functional
probe (the button exists when a Draft model is routed).

**Local verification battery:** gates (no-simulation clean,
traceability OK); fmt clean; clippy `--workspace --all-targets
-D warnings` clean + the sandbox lane clean + the kittest lane clean
for the touched target (the pre-existing `show_inside` deprecation in
kittest_reports_page is untouched — CI does not clippy that lane);
tests core 464/464 (+3 ignored, 5 new) · app-shell lib 251/251 ·
sandbox 472/472 (+3 ignored) · cargo-deny advisories/licenses/bans ok;
kittest diff-locked across all 24 suites + style lint — exactly 7
intended golden changes (the copilot hint line on
r5_inbox_copilot_docked_1920_light + the six pinned customers goldens)
and 1 new golden, zero unexpected drift; SPP_SMOKE rc=0 (24 pages /
4348 shapes); VLM read-backs: the routed golden shows the "Draft reply
with LM Studio" button in the pane, the unrouted golden shows the
routing hint and no button.
- CI run 42540828574 on the batch push `d6e5888`: green 3/3 —
  fmt/clippy/test/deny, AppImage + .deb build & smoke, and the kittest
  snapshot drift gate (the 7 intended golden changes + the new copilot
  golden accepted, the clock-pinned customers goldens holding) all
  succeeded.

## Batch UXH-1 — help center articles render as articles, not HTML source (2026-11-09)

**Owner directive:** the screenshot report "it should show help center
articles in a correct way, it is showing them with html syntax" — the
Help center article reader printed the synced `docs_articles.text` (the
raw Help Scout Docs HTML body) verbatim in a monospace label, so
`<p>`/`<code>` tags showed as literal text.

**Items:** UXH-1-1 (single item) — render the article body through the
app's existing sanitized rich-text pipeline.

**Implementation (`crates/app-shell/src/pages/knowledge.rs` only —
display path; the stored mirror, sync, FTS and embedding pipelines are
untouched, so already-synced articles render correctly without a
re-sync):**
- `reader_ui` now rides the shared `design::centered_column` at
  `READER_MEASURE` (860 — the same reading-measure cap the inbox reader
  thread and composer hold), so back/title/meta/body read as one centered
  article surface on 2K/4K/8K displays instead of stretching across the
  1800 content column.
- New `article_body_ui`: the body goes through
  `spp_core::richtext::html_to_spans` — the exact ammonia-sanitized +
  quick-xml span pipeline the inbox reader uses for HTML mail bodies
  (allowlist tags, script/style dropped, entities decoded, links
  captured) — never the raw source.
- New `article_blocks`: the span stream is split at its newline
  separators (`\n\n` = paragraph gap, `\n` = soft break/list item) into
  rendering blocks; `article_block_ui` lays each block out as one
  `ui.horizontal_wrapped` flowing unit with the inbox message reader's
  span styling (bold/italic/underline/strikethrough, monospace `code`,
  weak `blockquote`, `hyperlink_to` links, `break_long_tokens` wrap
  safety), heading sizes tuned for the article register (h1/h2 → LEAD
  14 strong, h3+ → BODY 13.5 strong — every heading outranks the 13.5
  body line; the article title stays section_title 15 strong). Paragraph
  rhythm: SP3 between paragraph-gap blocks, the 8px item spacing for
  soft breaks. Plain-text bodies degrade to a single readable
  proportional-text block; empty bodies show a hint.
- Regression locks: 4 unit tests on the splitter through the real
  pipeline (the owner-reported `<p>…<code>Retry-After</code>…</p>` shape
  renders tag-free with the code run keeping its monospace style;
  paragraphs/headings/bulleted+numbered lists/code blocks split with the
  right gaps and bullets; plain-text single-block; empty → no blocks) +
  3 kittest reader goldens (`uxh1_help_center_reader_html_body_`
  1440 light/dark + 800 light) over an HTML-bodied fixture with
  functional probes (the `Retry-After` code run, the `Rate limits` h2
  and the bullet item each exist as their own label — none could exist
  in the raw-source rendering).

**Local verification battery (recycled sandbox, toolchain reinstalled at
the exact 1.99.1 pin, lavapipe ICD extracted locally for the kittest
lanes):**
- Gates: no-simulation clean; traceability OK (23 routes, 20 design.rs
  DC tags, 27 DC ids). fmt `--all --check` clean; verify-icons OK;
  clippy `--locked --workspace --all-targets -- -D warnings` clean;
  clippy sandbox lane clean.
- Tests: app-shell lib 251/251 (incl. the 4 new) · core 459/459 (+3
  ignored) · xtask 14/14 · catalog 2/2 · sandbox lane 467/467 (+3
  ignored; one disk-full false alarm mid-battery was cleared by freeing
  the incremental cache and re-run green) · cargo-deny advisories /
  licenses / bans all ok.
- Kittest drift gate: all 24 suites + style lint diff-locked green
  (202 tests) — zero drift on the 189 existing goldens, the 3 new reader
  goldens locked, then re-run diff-locked a second time.
- SPP_SMOKE rc=0 (boot chain + shell/renderer verified with the new
  reader code).
- VLM read-back of the new 1440-light golden: no raw HTML visible;
  title/meta/h2 heading, paragraph with monospace inline
  `Retry-After`, bulleted two-item list and monospace code line all
  render as formatted text in one centered readable column.
- CI run 42533135570 on the batch push `d1d8436`: green 3/3 —
  fmt/clippy/test/deny, AppImage + .deb build & smoke, and the kittest
  snapshot drift gate (the 3 new reader goldens accepted, zero drift on
  the 189 existing baselines) all succeeded.

## Batch REL-5 — Build & Release v1.9.6 from the final source (2026-11-08)

**Owner directive:** the standing Build-and-Release mandate (this
session's "continue" after the CI-1 close) — v1.9.5 (`9afef58`) predates
five owner-directed design batches and the packaging hardening; this
release publishes the final source.

**Items:** REL-5-1 version bump 1.9.5 → 1.9.6 · REL-5-2 CHANGELOG
`[1.9.6]` + link ref · REL-5-3 annotated tag `v1.9.6` · REL-5-4 publish
+ live post-publish verification.

**Release commit:** `81f1ccd` (main tip; carries UXD-1, UXE-1, UXE-2,
UXF-1 + UXF-2-1, UXG-1, CI-1 — all unreleased in v1.9.5).

**Local verification battery (seventh sandbox state, toolchain at the
exact 1.99.1 pin):**
- Version bump: root Cargo.toml (the single source of truth; members
  are `version.workspace = true`) + Cargo.lock diff = exactly the four
  member version lines; zero stale `1.9.5` strings in manifests, lock,
  README, USER_MANUAL, docs.
- Gates: no-simulation clean; traceability OK (23 routes, 20 design.rs
  DC tags, 27 DC ids). fmt `--all --check` clean; verify-icons OK;
  clippy `--locked --workspace --all-targets -- -D warnings` clean.
- Tests: core 459/459 (+3 ignored) · app-shell lib 247/247 · style
  lint 2/2 · xtask 14/14 · sandbox lane 467/467 (+3 ignored) — 1189
  passed, 0 failed, matching the CI-1/UXG-1 baselines.
- Release build (exact release profile, sandbox feature on): green in
  3m21s — 31,036,528 B binary; SPP_SMOKE rc=0 (boot chain, workers,
  loopback listener, automatic backup, 24 pages / 4348 shapes — the
  UXG-1 baseline exactly).
- `.deb` via `xtask deb --skip-build`: 9,393,540 B;
  Package/Version/Architecture + the 19-dep Depends set correct;
  extract + SPP_SMOKE rc=0 from the payload tree.
- AppImage preflight: `xtask package --skip-build` staged the AppDir
  and stopped exactly at `ensure_host_libs_bundled` (the sandbox host
  lacks libxkbcommon-x11/libxcb-xkb and has no root — disclosed, same
  as REL-4); the two missing BUNDLED_HOST_LIBS placed from the
  ubuntu:22.04 archive (libxkbcommon-x11 1.4.0-1, libxcb-xkb
  1.14-3ubuntu3) exactly as the step copies them; appimagetool 1.9.1 +
  the CI-1-pinned type-2 runtime (tag 20251108) downloaded at the
  xtask's pinned URLs, both SHA256-verified; AppImage packed with the
  NEW `--runtime-file` invocation — 12,851,704 B local;
  APPIMAGE_EXTRACT_AND_RUN + SPP_SMOKE rc=0.
- release.yml extraction simulation: version awk → 1.9.6; changelog
  section awk → the full 72-line `[1.9.6]` section.

**CI on the release commit:** run `42479929932` — 3/3 green before
tagging.

**Release run `42480983095` (tag push, 6/6 jobs green):** build+test+
package (ubuntu:22.04, glibc 2.35 baseline, dist staging with
SHA256SUMS.txt + BUILD-INFO.txt) · verify .deb on clean ubuntu:22.04 /
ubuntu:24.04 / debian:12 containers · verify AppImage (ubuntu-latest) ·
publish-after-green.

**Live post-publish verification (this sandbox, from the published
bytes):**
- Release `v1.9.6` = "SupportOS+ 1.9.6", draft=false, prerelease=false,
  target `81f1ccd5088d…` == release commit == main tip.
- All four assets downloaded live (AppImage 12,835,320 B; .deb
  10,204,616 B; SHA256SUMS.txt; BUILD-INFO.txt); `sha256sum -c`: both
  packages OK (AppImage `dd422384…71ee`, .deb `025776e2…cd77`).
- BUILD-INFO.txt: version 1.9.6, commit `81f1ccd5088d…`, built on
  ubuntu:22.04 at 2026-11-08T13:05:33Z.
- Published `.deb` extracted; SPP_SMOKE rc=0 from the payload tree
  (24 pages / 4348 shapes, first-run DB self-initialization).
- Published AppImage: APPIMAGE_EXTRACT_AND_RUN + SPP_SMOKE rc=0 —
  identical boot chain from the self-extracted pinned runtime.
- Release notes: 10/10 content checks — `[1.9.6]` changelog section
  (prepended), both exact SHA-256 hashes, commit binding, both install
  command blocks, system requirements (x86_64, glibc 2.35+, Ubuntu
  22.04 LTS+/Debian 12+), APT auto-dependency statement, the 2K/4K/8K
  and LM Studio product lines.

**Environmental limits disclosed (honest scope):** no sudo/root in
this sandbox → the APT install/upgrade/uninstall lanes,
desktop-file-validate and the Xvfb GUI launches ran only in CI's clean
containers (green ×3 distros + ubuntu-latest, on the exact published
artifacts); the AppImage host-libs bundling step ran locally with
ubuntu:22.04-archive libs (CI performs it natively in the ubuntu:22.04
build container — the published bytes are CI's, not the local
preflight's); one disk-full event during the test lanes (incremental
compilation artifacts) cleared by `rm -rf target/debug/incremental` +
CARGO_INCREMENTAL=0, lanes re-run green.

Rule 8 STOP — the queue is empty; the next work item requires a new
owner directive.

### Batch CI-1 — 2026-11-08 (retry-with-backoff for the appimagetool download; CI flakiness fix)

**CI-1-1 — Packaging lane must survive transient CDN failures**
- Context: CI run 42465641980 on `af6d3ae` (the UXG-1 batch push) failed
  in the AppImage job — not a code failure: the Rust release build
  finished clean in 3m48s, and the fmt/clippy/test/deny and kittest
  jobs passed on the same commit; the single appimagetool download
  attempt was killed by GitHub's own release CDN returning a 504
  Gateway Time-out. Live probing confirmed the asset is still served
  with the exact pinned sha256 (15,092,216 bytes, `ed4ce84f…84eb0`
  verified byte-for-byte) but the CDN is flapping — intermittent 504s
  mixed with 200s (~1-in-3 failing from this sandbox's edge at probe
  time). The download also cannot be satisfied from a cache:
  Swatinem/rust-cache's save-side sweep drops non-cargo files under
  `target/` (the failed run had a full 707 MB cache hit yet still
  downloaded), so every packaging run fetches the tool fresh.
- Work: `crates/xtask/src/package.rs` — the single-shot
  `client.get(...).send()?.error_for_status()?.bytes()?` in
  `download_appimagetool` became `fetch_appimagetool_with_retry`: 5
  attempts with 5s/15s/45s/90s backoff (155s total wait; at the observed
  1-in-3 failure rate the cumulative download failure odds drop to
  ~0.4%, and the all-attempts-fail worst case adds only ~3.4 min against
  the 60/90-min job budgets). Retries cover only transient outcomes —
  5xx responses, HTTP 408/429, transport errors, and body-read failures
  (policy factored into `is_retryable_status`, a pure function over
  `StatusCode`); non-transient statuses (404 etc.) still fail fast with
  the pinned URL in the message. Every successful read still lands in
  the same `verify_appimagetool` sha256 gate immediately after, so the
  trust story is unchanged. No new dependencies, no workflow edits.
- Tests: two new unit tests — `appimagetool_retry_policy_retries_only_
  transient_statuses` (500/502/503/504/408/429 retryable; 200/204/301/
  400/401/403/404/410 fail-fast) and `appimagetool_retry_schedule_stays_
  inside_ci_job_budgets` (schedule shape: strictly increasing, 4 waits →
  5 attempts, 155s total, far under any job budget).
- Verification (recycled sandbox, toolchain reinstalled at the exact
  1.99.1 pin): no-simulation gate OK; traceability gate OK (23 routes,
  27 DC ids); `cargo fmt --all --check` OK; `verify-icons` OK;
  `cargo clippy --locked --workspace --all-targets -- -D warnings` OK;
  tests 459 (core) + 247 (app-shell) + 13 (xtask, incl. the 2 new) + 467
  (core sandbox lane) — 0 failures; `cargo clippy --locked -p
  supportos-plus-core --all-targets --features sandbox -- -D warnings`
  OK; `cargo deny check advisories licenses bans` all ok; live CDN probe
  verified the pinned URL serves the pinned bytes. The kittest drift
  gate is unaffected by an xtask-only change and runs in CI unchanged.

**CI-1-2 — Pin the type-2 runtime; appimagetool must go offline**
- Context: the verification push for CI-1-1 (CI run 42472196649 on
  `ca95343`) proved the retry works — the appimagetool download survived
  three consecutive transient failures (503, transport error, 504,
  each logged with the attempt counter) and succeeded on attempt 4 — but
  the job then died at the next single-shot network call:
  appimagetool's own fetch of the type-2 runtime it embeds into every
  AppImage ("Failed to download runtime file, please download the
  runtime manually from https://github.com/AppImage/type2-runtime/
  releases and pass it to appimagetool with --runtime-file"). That
  download lives inside the external tool with no retry and no env-var
  override — the only lever is supplying the file ourselves.
- Work: `crates/xtask/src/package.rs` — new `APPIMAGE_RUNTIMES` pin
  table (x86_64 + aarch64, both pinned to the frozen type2-runtime
  release tag `20251108` — never the rolling `continuous` tag, whose
  assets are rebuilt without notice and would break the checksum pin;
  sha256 `2fca8b44…260d` / `00cbdfcf…7444`, verified against live
  downloads); `resolve_appimage_runtime` resolves/verifies/caches the
  runtime under `target/tools/runtime-<arch>` beside appimagetool; the
  retry fetch was generalized from `fetch_appimagetool_with_retry` into
  `fetch_pinned_with_retry(client, url, label)` (same 5-attempt
  5/15/45/90s policy) and `download_pinned_asset` + `verify_pinned_file`
  now serve both the tool and the runtime with one code path; the
  appimagetool invocation gained `--runtime-file <pinned runtime>` so
  the tool makes zero network calls of its own.
- Tests: `appimage_runtime_pins_cover_both_ci_arches_on_one_frozen_tag`
  (official-repo URL prefix, no `continuous`, 64-hex sha per row, URL
  asset name matches its arch row, both arches resolve, unmapped arches
  fail, one shared frozen tag).
- Verification: fmt/clippy (-D warnings)/tests re-run green (xtask 14
  passed incl. the new lock); live functional check of the exact new
  invocation shape — downloaded both pinned artifacts (checksums
  verified), built a minimal AppDir, ran `ARCH=x86_64
  APPIMAGE_EXTRACT_AND_RUN=1 NO_STRIP=1 appimagetool --runtime-file
  runtime-x86_64 <AppDir> <out>` → Success, and the output AppImage's
  ELF BuildID (`a87aaf5d…`) matches the pinned runtime's BuildID
  (proving the pinned runtime is what got embedded), and the resulting
  AppImage executes under APPIMAGE_EXTRACT_AND_RUN (the same mode the
  CI smoke step uses).
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
- Runner verification (added after the runs): batch-opening commit
  `2e90960` — CI run `40012720086`: success, both jobs green; code
  commit `d823fcf` — CI run `40022440515`: success, both jobs green
  (fmt/clippy/test/deny AND the AppImage/deb build with the SPP_SMOKE
  shell smoke); tracking commit `e30d0ec` — CI run `40022682471`:
  success, both jobs green.

### Batch 29 — 2026-10-23 (code commit `f1289fb`)

**97. Rewrite paths bypass redaction** (low|issue)
— the two rewrite lanes (the composer's pre-send "shorten/warmer/…"
menu and the stored-draft rewrite) handed the agent's draft text
straight to the model through a raw `chat_with_opts` call — while every
other lane already crossed the `redaction_enabled`-gated
`redact_text` boundary. Drafts routinely quote what customers sent:
card numbers, CVVs, API keys pasted into a reply for reference. The
model request now crosses the same boundary both paths share (a
`redact_for_model` helper reading the same setting, default on, with
findings logged through the operation log), and the setting stays the
honest switch — redaction off passes the text through verbatim.
core/ai_pipeline.rs.

**98. Tool args: as_i64 rejects schema-valid floats** (low|issue)
— the tool schemas declare `"type": "number"`, and models routinely
emit whole values as JSON floats (`"number": 95.0`), which
`Value::as_i64` rejects — so a schema-valid argument came back
"Invalid number" and the tool call failed. A shared `arg_i64` reader
now accepts any integral encoding (exact-integer floats inside i64's
range), never truncates a fraction (`9.5` still rejects), and all
thirteen argument sites plus `clamp_limit` read through it.
core/ai_tools.rs.

**99. get_issue_clusters returns 3× the documented limit with a query**
(low|issue)
— the schema documents limit as "Max clusters (default 5, max 10)",
but the SQL multiplied by three whenever a query was present — up to
30 clusters came back from a call whose contract said ten. The
multiplier is gone; the LIMIT binds the clamped limit alone, with or
without a query. core/ai_tools.rs.

**100. Knowledge chunks: no attempts cap → infinite retry** (low|issue)
— `knowledge_chunks` had no `embedding_attempts` column at all (its
two sibling tables carry one), its pending selection had no cap, and
its failure lane reset only the state — a chunk the embedding backend
kept rejecting (oversized, backend error) was re-selected and
re-attempted on every embed pass, forever. Migration m061 (boot step
52) adds the column idempotently for existing installs, the pending
selection caps at five attempts like the siblings, `mark_chunk_failed`
counts attempts for every family, and the vector-id-v2 re-embed
migration resets the counter with the rest. core/embeddings.rs,
core/bootstrap.rs.

**101. Malformed 200 → literal "Unknown" content** (low|issue)
— a well-formed JSON 200 whose choices were empty (or whose single
choice carried `content: null`) parsed as a SUCCESSFUL completion whose
content was the fabricated literal string "Unknown" — which downstream
lanes then persisted as if the model had written it. `parse_chat_response`
returns `Result` now: no choices and no content are labeled errors
("malformed 200 body"), never fabricated text; a readable body parses
unchanged. core/ai_lm_studio.rs.

**102. Draft signature picks an arbitrary agent's name** (low|issue)
— the local copilot's draft signature resolved `SELECT u.first_name
FROM users u ORDER BY u.id LIMIT 1`: whichever teammate happened to
hold the lowest row id signed every copilot draft, not the agent
working the ticket. The signature now resolves the conversation's
actual `assignee_local_id`; an unassigned conversation falls back to
the honest "Support" instead of a random colleague's name.
core/ai_local.rs.

**103. Org member_count capped by LIMIT 100** (low|bug)
— `get_organization_detail` derived member_count and the
open/conversation totals from its 100-row members list, so an org with
250 members reported 100 and summed only the first page. The members
list stays a display list (capped at 100), but the totals now come from
an unbounded aggregate over every member, computed in the same
per-member terms the list rows use. core/people_store.rs.

**104. last-activity/resolution subqueries miss deleted_at** (low|bug)
— two soft-delete blind spots: the customer list's
`MAX(last_activity_at)` subquery and its recency sort both read every
conversation of the customer, deleted ones included — a soft-deleted
ticket with a newer stamp shadowed the live activity and could pin a
customer to the top of the recency sort; and the customer detail's
resolution excerpt picked the newest `reply`-type thread without
filtering `deleted_at` — a deleted reply spoke for how a closed ticket
was resolved. All three subqueries filter soft-deleted rows now.
core/people_store.rs.

**105. Org name linking case-sensitive vs NOCASE elsewhere** (low|bug)
— every customer-to-organization name comparison was binary while the
org list itself sorts `COLLATE NOCASE` (and the platform matches org
names case-insensitively): a customer created as "acme corp" never
linked to the stored org "Acme Corp" — no id link at creation, no
re-link on patch or backfill, and the org detail/list counted the
member as belonging to nobody. All the linking sites (create/update/
timeline backfill, the members SQL, the org-list counts, and the
rename propagation) compare `COLLATE NOCASE` now. core/people_store.rs.

**106. CSV export lacks formula-injection guard** (low|issue)
— the conversations CSV export quoted its cells but never guarded
them: a conversation subject like `=HYPERLINK("http://evil","click")`
or `@WEBSERVICE(...)` (customer-controlled text) executed when the
exported file was opened in a spreadsheet. `csv_cell` now applies the
OWASP guard — cells starting with a formula trigger (`=`, `+`, `-`,
`@`, TAB, CR) are single-quote-prefixed into inert text — then the
normal CSV quoting on top; ordinary cells pass through unchanged.
core/backup_service.rs.

Verification (Rule 5, all on the pinned 1.99.1 toolchain):
- `python3 scripts/gate_no_simulation.py`: clean.
- `cargo fmt --all --check`: clean.
- `cargo clippy --workspace --all-targets -- -D warnings`: clean;
  `--features sandbox`: clean.
- Tests: core 440 passed / 0 failed / 4 ignored (14 new batch-29
  regressions: the rewrite prompt redacting a quoted card number and
  CVV through the boundary with the setting as the honest switch; the
  integral-float tool argument resolving its conversation while 9.5
  still rejects, and the fraction/string/oversized-float decode table;
  the cluster LIMIT never inflated by a query across four
  argument/seed combinations; the m061 column landing idempotently at
  boot, the capped chunk leaving the pending queue, and the failed
  knowledge chunk counting its attempt; the malformed 200 with no
  choices or no content failing with labeled errors while a readable
  body parses; the assignee — not the lowest-id teammate — signing the
  draft and the unassigned conversation falling back to "Support"; the
  120-member org reporting exact member/open/conversation totals with
  a 100-row display list; the deleted conversation's newer stamp
  ignored by the list row and the recency sort; the deleted thread
  never serving as a closed conversation's resolution; the
  case-differing organization linking at creation, counting through
  the members SQL, and counting in the org list; and the =, +, @, -
  cells quote-prefixed in the exported CSV with ordinary cells
  untouched); sandbox-feature suite 445 passed / 0 failed / 4
  ignored; app-shell 209 passed / 0 failed; xtask 6; catalog 0.
- End-to-end smoke: `SPP_SMOKE=1` on the freshly built binary —
  exit 0, boot chain + shell/renderer verified, 24 pages rendered,
  4043 shapes painted (boot step m061 exercised by the boot chain).
- Fresh harness `scripts/verify_batch29.py` (committed): all 51
  checks pass; the batch-18, batch-19, batch-21, batch-23, batch-24,
  batch-25, batch-26, batch-27 and batch-28 harnesses still pass.
  Pre-existing harness notes, proven against pre-batch HEAD by a
  stash-run and unrelated to this batch: batch-17's script holds an
  absolute path from the original engagement machine (its checks are
  environment-bound); batch-20's item-21 "upsert sites" needle counts
  a `collect` form that batch-24's per-chunk partitioning restructured
  (the error-propagation property itself still holds — the mapping
  call still returns through `?` at both sites); batch-22's flake
  needle expects a one-line `now_iso` literal that rustfmt has since
  wrapped. None of these regressions involve batch-29 code.
- `cargo deny check advisories licenses`: ok (the CDLA-Permissive-2.0
  note is the pre-existing unmatched-allowance warning, not a failure).
- Security: staged diff token-scanned clean before each push; remote
  unmoved at push time (fetch-first).
- Runner verification: batch-opening commit `0a237eb` — CI run
  `40028572277`: success; code commit `f1289fb` — CI run `40035215617`:
  success, both jobs green (fmt/clippy/test/deny AND the AppImage/deb
  build with the SPP_SMOKE shell smoke); tracking commit `61ff279` —
  CI run `40035306132`: success, both jobs green.

### Batch 30 — 2026-10-24 (code commit `ecc7228`)

**107. redact_string byte-slicing panic** (low|bug)
— the redaction helper sliced head/tail at raw byte offsets
(`&s[..2]`), so any secret whose second byte sits inside a
multi-byte character — a CJK, emoji, or accented-Latin client id —
panicked with "byte index 2 is not a char boundary" the moment the
redacted config was rendered. The helper now takes the first and
last two CHARACTERS and masks short strings by character count, not
bytes; the panic shape is impossible. core/config.rs.

**108. Digest doc counts: space-format vs ISO-T lexical comparison**
(low|bug)
— `change_facts` normalized the gap-candidates comparison through
`datetime()` but compared the two doc counts as raw strings: a
stored space-format stamp (`2026-10-20 11:30:00`) sorts before the
ISO-T `since` bound (`2026-10-20T10:00:00.000Z`) because `' '` < `'T'`,
so a doc updated later on the same day silently fell out of the
digest. The articles WHERE and ORDER BY and the knowledge-documents
count normalize through `datetime()` like the gaps query always did.
core/digest.rs.

**109. New-conv windows key on local_created_at; deleted-filter
claim false** (low|bug)
— the dashboard's "new conversations" window and build_report's
period windows (plus the day/week/month bucket expressions) keyed on
`local_created_at`, which is the LOCAL INSERT stamp (schema default
`datetime('now')`, never written from the platform payload) — every
backfilled conversation counted as new on the day it was synced and
the buckets reported sync time, not creation time. All of it keys on
`COALESCE(remote_created_at, created_at)` — the same creation stamp
the digest, segments and AI pipeline use. And the Conversations
metric's limitation text claimed "Does not include deleted
conversations" while no filter did: every report row and the
previous-period window now carry `deleted_at IS NULL`.
core/reports.rs.

**110. %W year-boundary week split** (low|issue)
— the week dimension paired `%Y` with `%W`, so one natural week at
the year boundary split into "2025-W53" and "2026-W00" (and W00 is
not a real week label). The dimension is ISO now — `strftime('%G-W%V')`
(supported by the bundled SQLite ≥ 3.44) — so Dec 30 and Jan 1 land
in the same `2026-W01` bucket, pinned by a test. core/reports.rs.

**111. LIMIT caps silently undercount alert claims** (low|issue)
— the escalation radar sample was capped at `LIMIT 1000` and the
rating-correlation sample at `LIMIT 2000` (newest by id): 2,500
escalated tickets were claimed as "1000 escalated tickets in the
last 30 days", and older not-good ratings silently vanished from the
cluster overlap counts. Both caps are gone — the claims match the
real sample, and the queries stay bounded by the 30-day window.
core/analytics.rs.

**112. Concentration denominator counts NULL-key conversations**
(low|issue)
— `customer_concentration` divided distinct customers by ALL cluster
conversations while the numerator set only carried attributed ones
(unattributed imports fired the alert on imaginary concentration),
and `inbox_concentration` divided by ALL conversations while only
mailbox-bearing rows could land in a bucket (NULL-mailbox rows
diluted the ratio and suppressed real single-inbox concentration).
Both checks now run over the attributed subset and the claim texts
report the attributed totals ("4 of 4", not "4 of 7").
core/analytics.rs.

**113. open_closed_distribution drops pending** (low|issue)
— the incident/known-issue impact distribution counted open as
`status = 'active'` and closed as `'closed'`, so a conversation in
the platform's `pending` state vanished between the buckets and
open + closed undercounted affected_conversations. Open counts
`status IN ('active','pending')` — the same convention workload,
operations and the digest already use. core/issue_impact.rs.

**114. email_domain filter unanchored, op ignored** (low|issue)
— every operator ran the same `LIKE %@{domain}%`: the trailing `%`
let `user@example.com.au` match an `example.com` filter,
`not_equals` returned the equals set, and `is_empty`/`is_not_empty`
were unreachable. The filter now extracts each stored address's
domain part (`LOWER(SUBSTR(ce.value, INSTR(ce.value, '@') + 1))`)
and dispatches the standard operator set onto it — equals (exact
domain), not_equals (negation over all customers), contains,
starts_with, ends_with — with is_empty/is_not_empty answering on
email presence without requiring a value. core/segment.rs.

**115. campaign_reconcile references nonexistent updated_at column**
(low|bug)
— the reconcile sweep's state-transition UPDATEs stamp
`outreach_recipients.updated_at`, but the column is missing on
databases created before the item-17 batch and the ensure ran only
in `send_batch` — so `campaign_reconcile`'s own path failed every
UPDATE with "no such column", swallowed the error (`let _ =`), left
the row in 'unknown' forever (re-logged by every reconcile pass) and
still counted it in the returned totals. The idempotent
`ensure_recipient_updated_at` self-heal now runs at the top of
`reconcile_recipient_states`, covering both entry paths, pinned by a
legacy-schema test (column dropped → reconcile still transitions the
exhausted row to 'failed' and re-adds the column).
core/outreach.rs.

**116. ORDER BY remote_created_at never populated by sync (2 sites)**
(low|bug)
— the sync writer (`sync_engine::upsert_thread`) never populates
`conversation_threads.remote_created_at`, so `customer_messages`
(interaction_current) and `compute_outcome` (interaction_engine)
sorted on a constant NULL and the customer-message sequence was
whatever the scan returned — miscounting follow-ups and first-reply
resolution. Both lanes order on
`COALESCE(remote_created_at, created_at)`, the fallback
ai_pipeline, ai_evidence, ai_local and translation already use, with
an insert-order test proving chronology is restored.
core/interaction_current.rs, core/interaction_engine.rs.

Verification (Rule 5, all on the pinned 1.99.1 toolchain):
- `python3 scripts/gate_no_simulation.py`: clean.
- `cargo fmt --all --check`: clean.
- `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- Tests: core 453 passed / 0 failed / 4 ignored (13 new batch-30
  regressions: redact_string rendering a 6-emoji client id where the
  old byte slice panicked; the space-format doc update after an
  ISO-T same-day bound counting in the digest facts; the backfilled
  conversation counting as new while the 60-day-old backfill and the
  deleted one do not; the 2025-12-30/2026-01-01 pair landing in one
  `2026-W01` bucket with the deleted boundary row excluded; the
  deleted conversation excluded from the 'all' report row; the
  inbox-concentration alert firing at 4 of 4 attributed conversations
  where the old 4-of-7 denominator suppressed it; the 2-attributed/4-
  unattributed mixture no longer firing customer concentration; the
  1001-escalated-ticket claim reporting 1001; pending counting as
  open in the impact distribution; the email_domain operator set
  (equals exact, .au and subdomain anchored out, ends_with including
  the subdomain, contains including the .au, not_equals and the
  email-presence ops); campaign_reconcile on a dropped-column legacy
  schema still transitioning the exhausted row to 'failed' with the
  column re-added; customer_messages restoring oldest-first order on
  reverse-inserted NULL-remote rows; and compute_outcome counting
  one follow-up where the constant-NULL order miscounted);
  sandbox-feature suite 458 passed / 0 failed / 4 ignored; app-shell
  209 passed / 0 failed; xtask 6; catalog 0.
- End-to-end smoke: `SPP_SMOKE=1` on the freshly built binary —
  exit 0, boot chain + shell/renderer verified, 24 pages rendered,
  4043 shapes painted (identical to the batch-29 baseline).
- Fresh harness `scripts/verify_batch30.py` (committed): all 36
  checks pass; the batch-18, batch-19, batch-21, batch-23, batch-24,
  batch-25, batch-26, batch-27, batch-28 and batch-29 harnesses still
  pass. Pre-existing stale needles in batch-20 (item-21 upsert-site
  needle restructured by batch-24's per-chunk partitioning) and
  batch-22 (the one-line fence literal rustfmt has since wrapped, and
  the volume-spike flake check) were proven pre-batch by a stash-run
  at the pre-batch commit — identical failures, unrelated to this
  batch; batch-17's script remains environment-bound as documented.
- `cargo deny check advisories licenses`: ok (the CDLA-Permissive-2.0
  note is the pre-existing unmatched-allowance warning, not a failure).
- Security: staged diff token-scanned clean before each push; remote
  fetched before each push (batch-29's runner-verification commit
  `b3d8904` landed mid-batch and was rebased onto, not overwritten).
- Runner verification: follows once CI on code commit `ecc7228`
  completes (recorded below when the runs finish).

### Batch 31 — 2026-10-24 (code commit `763824c`)

**117. refresh_replies can count the campaign's own message (clock
skew)** (low|bug)
— the campaign's own message is mirrored as a `type = 'customer'`
thread (`helpscout_real::create_conversation` sends it with
`"type": "customer"`), and the reply guard compared platform thread
stamps against a LOCAL clock stamp (`sent_at = datetime('now')`
taken right after the API call) — any platform-clock-ahead skew put
the campaign's own email after the guard and the sweep counted it as
the recipient's reply. Reply detection now anchors on the mirrored
campaign thread itself — the conversation's first thread
(create_conversation creates the conversation with exactly one
thread) — and accepts only a customer thread strictly after it (or
sharing its stamp but created after it); both sides of the
comparison read from the same clock. Pinned by a skew test (campaign
message 1s after sent_at must not count; the hour-later reply must)
and a suppression test. core/outreach.rs.

**118. Duplicate graph edges for mutual links** (low|bug)
— a mutual link is two `graph_edges` rows (A→B and B→A) and "both"
expansion returns both, so the subgraph BFS pushed the same logical
link once per expansion — and twice more when the far node expanded.
Pushed edges dedupe through a canonical unordered
(relation, node-pair) key now, so a mutual link renders once
regardless of how many expansions return it, while genuinely
distinct relations between the same pair stay distinct.
core/support_graph.rs.

**119. Tag-filter case-folding asymmetry** (low|bug)
— the Rust side folded filter values with Unicode rules
(`str::to_lowercase`) while SQL `LOWER` folds ASCII only: a stored
tag never matched its own uppercase spelling when the name carried a
non-ASCII letter (É folded to é on one side only). The Rust side
folds ASCII-only now, so both sides agree — ASCII tags match
case-insensitively, non-ASCII names compare symmetrically.
core/saved_views.rs.

**120. Entity decode order double-decodes; &apos; missing**
(low|bug)
— the html text extractor decoded `&amp;` FIRST, so an author's
literal `&amp;lt;` became `&lt;` and then decoded again into a real
`<`; `&apos;` never decoded at all. Single-decode semantics decode
every other entity first and `&amp;` LAST (`&amp;lt;` lands as the
literal text `&lt;`), with `&apos;` added alongside `&#39;`.
core/knowledge_store.rs.

**121. Byte-unsafe slicing helpers** (low|bug)
— three slicing classes trusted Unicode-lowercased or raw-byte
offsets against the original string: the knowledge html helpers
(`strip_tag_block`, `replace_ci`, `extract_html_title`) used the
lowercased haystack as an OFFSET MAP over the original — İ (U+0130)
expands to two chars under Unicode lowercasing, so the offsets
stopped matching and the helpers sliced garbage or panicked;
`valid_iso_date` sliced its prefix at raw byte offset 10 (any
multi-byte character in the first ten bytes panicked); and
`truncate_result` sliced the tool-output cap at a raw byte offset.
All of them fold/slice CHARACTER-safely now (ASCII lowercasing keeps
byte offsets 1:1 for the html helpers), each pinned by a
multi-byte-shape test. core/knowledge_store.rs, core/custom_objects.rs,
core/copilot.rs.

**122. Numeric customer_property filters CAST text→0** (low|bug)
— `CAST(cp.value AS REAL)` turns any non-numeric text into 0.0, so
"N/A", "blue" and "yes" satisfied every `lt`/`lte` comparison
against a positive threshold. Only values that are actually numeric
participate now: at least one digit and nothing outside a number's
character set (sign, digits, decimal point, exponent), trimmed
before the check. core/saved_views.rs.

**123. freshness_low_usage_days setting dead** (low|gap)
— the setting was read (default 60) and dropped: the low_usage flag
was `usage_count == 0`, firing for every unused document including
one created five minutes ago. The setting is the grace period now —
a document counts as low-usage once it has existed at least
`freshness_low_usage_days` (created_at, falling back to the update
stamp) and still has no recorded usage. core/knowledge_store.rs.

**124. Pre-release vs pre-release compares as Same** (low|bug)
— same-numeric-core versions both carrying pre-release suffixes
returned Same, so a running `1.4.0-beta.1` never learned that
`1.4.0-beta.2` existed. Pre-releases compare per semver identifier
rules now (numeric identifiers numerically and below alphanumeric
ones; a larger identifier set outranks its prefix), with the
release-vs-pre-release rule unchanged. core/version_check.rs.

**125. Sweep windows drop oldest of a burst (DESC+stamp)** (low|bug)
— six notification-sweep lanes (failed jobs, campaign replies, known
issues, issue spikes, AI escalations, not-good ratings) selected
`ORDER BY id DESC LIMIT N` FIRST and filtered by stamp in Rust
afterwards: already-swept rows filled the cap and the oldest rows of
a burst fell outside it — never notified, since the next sweep's
stamp skips them forever. The stamp filter runs in SQL before the
cap now (`datetime(col) > datetime(?1)`, normalizing the mixed
space/ISO-T formats exactly like the Rust parser did), the
redundant Rust filters and their now-dead helpers are removed, and
the ratings lane carries its stamp in the row instead of a per-row
re-query. Pinned by a burst test: five new failures buried under
205 already-swept ones all notify. core/notification_sweep.rs.

**126. parse_csv header/data index misalignment on empty headers**
(low|bug)
— empty headers were FILTERED OUT of the header list while data
records kept all their columns, so with a header row like `a,,c`
every data field after the gap shifted one column left (`c`
received the unnamed column's value). Empty headers keep their
positional slots now and simply produce no key.
core/connectors.rs.

Housekeeping: this batch's opening commit also repairs a pre-existing
`plan.md` typo on the parked item 56 (`ed|gap]` → `[med|gap]`,
introduced with the batch-16 tracking commit `68161a8`).

Verification (Rule 5, all on the pinned 1.99.1 toolchain):
- `python3 scripts/gate_no_simulation.py`: clean.
- `cargo fmt --all --check`: clean.
- `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- Tests: core 465 passed / 0 failed / 4 ignored (12 new batch-31
  regressions: the skew test where the campaign's own message must
  not count and the hour-later reply must; the no-reply suppression
  test; the mutual-link subgraph rendering one edge for two
  graph_edges rows; the tag filter matching a stored non-ASCII tag's
  own spelling and the ASCII pair case-insensitively; the
  single-pass entity decode with &apos; and the İ-prefixed html
  surviving the tag stripper; the multibyte date prefix and the
  multi-byte tool output across the truncation cap; the numeric
  filter matching 5 and " 7 " while N/A and blue stay out and 15
  joins at a higher threshold; the semver pre-release table;
  the five-new-failures-under-205-swept-ones burst notifying all
  five; and the parse_csv positional alignment); sandbox-feature
  suite 470 passed / 0 failed / 4 ignored; app-shell 209 passed /
  0 failed; xtask 6; catalog 0. (One full-suite rerun initially
  reported 68 failures: the disk filled mid-build and a stale
  partially-linked binary ran — after clearing 4.2 GB of build
  artifacts every suite was rebuilt and rerun cleanly; the failures
  were environmental, not code.)
- End-to-end smoke: `SPP_SMOKE=1` on the freshly built binary —
  exit 0, boot chain + shell/renderer verified, 24 pages rendered,
  4043 shapes painted (identical to the batch-29/30 baseline).
- Fresh harness `scripts/verify_batch31.py` (committed): all
  checks pass; the batch-18, batch-19, batch-21, batch-23, batch-24,
  batch-25, batch-26, batch-27, batch-28, batch-29 and batch-30
  harnesses still pass. The batch-20 and batch-22 stale needles are
  the same two documented pre-existing failures proven pre-batch by
  the batch-30 stash-run; batch-17 remains environment-bound.
- `cargo deny check advisories licenses`: ok (the CDLA-Permissive-2.0
  note is the pre-existing unmatched-allowance warning, not a failure).
- Security: staged diff token-scanned clean before each push; remote
  fetched before each push (unmoved at push time).
- Runner verification: follows once CI on code commit `763824c`
  completes (recorded below when the runs finish).

Runner verification (batch 31, code commit `763824c`): GitHub Actions run
`40047045028` completed — check runs "fmt, clippy, test, deny" and
"AppImage + .deb build, smoke" both concluded `success`.

### Batch 32 — 2026-10-24 (code commit `0458753`)

**127. Digest chip 16.max slice (dead truncation + OOB risk)** (low|bug)
— the chip formatted `&digest.created_at[..16.max(len)]`: with `max`,
a stamp SHORTER than 16 sliced at byte 16 (`byte index 16 is out of
bounds` panic) and a stamp LONGER than 16 sliced at its full length, so
the intended `YYYY-MM-DD HH:MM` truncation never happened — exactly
inverted. The cap is `16.min(len)` now, matching the sibling
period-start/period-end slices. pages/reports.rs.

**128. QA rows render `#Some(42)`** (low|bug)
— the recent-closures list printed `format!("#{id:?}")` with
`id: Option<i64>`, so every row read `#Some(42)` (and a missing id read
`#None`). A present id renders `#42`; a missing one `#?`.
pages/reports.rs.

**129. Literal `\` mid-sentence in 3 hover texts** (low|bug)
— three issue-radar strings ended their first line with an ESCAPED
backslash (`\\` = one literal `\` in the output) followed by a real
newline, so the rendered hover/error/notice text carried a stray `\`
and a broken, space-indented second line; the sibling strings correctly
use a single trailing `\` as the line continuation. All three now
continue the line like their siblings. pages/issue_radar.rs.

**130. Notification toggle reads the wrong list for Mentions rows**
(low|bug)
— the Mentions tab renders `self.mentions`, but `toggle_read` looked
the row up only in `self.records` (the All list, capped at 100 rows):
a READ mention pushed out of that window fell through to
`.unwrap_or(true)`, so its "unread" button re-marked it read — a
silent no-op. The lookup walks both lists now (every rendered row
lives in one of them). Pinned by a regression test that buries a read
mention under 100 fillers: the toggle must flip it back to unread.
pages/notifications.rs.

**131. Incidents: invalid ids silently dropped; filters without
refetch** (low|issue)
— two defects. The creator's id field silently dropped every
unparseable/non-positive token and every id with no live conversation
row (`filter_map(...ok())` then a silent existence filter), so a typo'd
id just vanished from the link list — the dropped tokens are collected
and named in a post-create notice now ("linked N conversation(s) —
unknown or nonexistent id(s) skipped: …", mirroring the issue-radar
known-issue form). And the status/severity selects plus the open-only
checkbox only mutated state — the list refetched only when the user
happened to hit "apply"; each control refetches on change now (the
text query keeps its explicit button — per-keystroke queries would
thrash). Pinned by a regression test naming "999" and "abc" in the
notice while the valid id still links. pages/incidents.rs.

**132. Diagnostics clear_log via reader lane, errors swallowed**
(low|issue)
— the event-log clear ran its DELETE through `with_read` (the pooled
reader lane) and `unwrap_or(0)` turned every failure into a success
message reading "event log cleared (0 rows removed)". The clear runs on
the writer lane now and a failure surfaces as an inline error instead
of a fake success. pages/diagnostics.rs.

**133. Palette: same-frame Enter confirms stale hit; no outside-click
close** (low|bug)
— two defects. `palette_take` was stored inside the query row and the
confirm consumed it AFTER `run_search` had been deferred to post-window,
so an Enter processed in the same frame the query changed (fast typing,
paste+Enter) confirmed the PREVIOUS query's result set — navigated to a
stale hit and closed before the new search ever ran; the confirm is
guarded by `!query_changed` now, the fresh search lands next frame, and
the field keeps focus so Enter can be pressed again on fresh hits (the
item-72 dry-Enter refocus widened to cover the suppressed case, its
regression test still green). And the palette had no outside-click
close: a full-screen backdrop area now sits behind the window — an
outside click closes the palette and no longer leaks to the UI
underneath. Both pinned by headless egui passes
(audit133_same_frame_enter_confirms_nothing_stale,
audit133_clicking_outside_the_palette_closes_it). palette.rs.

**134. Concave polygon to convex_polygon (area fill)** (low|bug)
— the line chart's soft area fill built
`[baseline-left, series..., baseline-right]` and drew it with
`Shape::convex_polygon`, which fan-triangulates from the first vertex —
a general data series is CONCAVE, so the fill bled past the polyline
and left holes wherever the line bent. The fill is an explicit mesh now
(`area_fill_mesh`): two triangles per segment, exact for any series
shape, pinned by a test asserting the mesh's total triangle area equals
the shoelace area of the baseline-closed polygon for a V-shaped series.
chart.rs.

**135. Stale inserted_draft provenance → phantom AI-draft acceptances**
(low|bug)
— `inserted_draft` survived composer content that did not come from the
inserted draft: the copilot suggested reply replaced the composer text
while the previous LM-draft provenance stayed armed, switching the
composer to internal note mode kept it (submit_composer records the
acceptance in whichever mode is active), and wiping the composer kept it
— all three recorded `accepted` + an edit distance against a draft the
sent text no longer derived from. The provenance dies with the draft
now: the copilot insert clears it (field writes — `report` borrows
`self.copilot` across the render closure, so a `&mut self` helper
cannot be called there), note-mode switching and composer wipes spend it
via `set_composer_mode`/`composer_edited`; edits that keep text keep the
provenance (the edit distance is the metric). pages/inbox.rs.

**136. Mention popup highlight never moves** (low|gap)
— `mention_active` was only ever reset to 0; the popup's highlighted
chip was immovable and only mouse clicks picked an entry, though the
hint text promised Enter/Tab completion of "the highlighted entry".
While the popup is open and the composer focused, ArrowDown/ArrowUp are
consumed BEFORE the TextEdit renders and move the highlight through
`move_selection` — consuming ahead of the field is load-bearing: an
un-consumed ArrowDown moves the caret, changes the @-query and closes
the popup on the very first press (and the popup code runs after the
field, too late to undo that). pages/inbox.rs.

Housekeeping: the batch-26 harness needle for item 72 was re-pointed to
the equivalent post-133 code (the confirm now guarded by
`!query_changed`; the dry-Enter refocus widened to the same disjunct) —
the invariant itself is unchanged and its regression test still passes.

Verification (Rule 5, all on the pinned 1.99.1 toolchain):
- `python3 scripts/gate_no_simulation.py`: clean.
- `cargo fmt --all --check`: clean.
- `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- Tests: core 465 passed / 0 failed / 4 ignored (unchanged — no core
  changes this batch); sandbox-feature suite 470 passed / 0 failed /
  4 ignored; app-shell 216 passed / 0 failed (7 new batch-32
  regressions: the read mention outside the All window flipping back to
  unread; the skipped-ids notice naming 999 and abc while the valid id
  links; the clear-log success asserting the writer-lane path and the
  "1 rows removed" message; the palette same-frame Enter confirming
  nothing stale with the palette staying open; the palette closing on
  an outside click; the area-fill mesh matching the shoelace polygon
  area on a V-shaped series plus the degenerate-input empty mesh; and
  the audit72 dry-Enter test still green on the widened refocus);
  xtask 6; catalog 0.
- End-to-end smoke: `SPP_SMOKE=1` on the freshly built binary —
  exit 0, boot chain + shell/renderer verified, 24 pages rendered,
  4043 shapes painted (identical to the batch-29/30/31 baseline).
- Fresh harness `scripts/verify_batch32.py` (committed): all 37 checks
  pass; the batch-18, batch-19, batch-21, batch-23, batch-24, batch-25,
  batch-26 (item-72 needles re-pointed to the post-133 code, documented
  above), batch-27, batch-28, batch-29, batch-30 and batch-31 harnesses
  all pass. Pre-existing stale needles in batch-20 (item-21 upsert-site
  needle restructured by batch-24's per-chunk partitioning) and batch-22
  (the one-line fence literal rustfmt has since wrapped, and the
  volume-spike flake check) unchanged — proven pre-batch by the batch-30
  stash-run; batch-17's script remains environment-bound as documented.
- `cargo deny check advisories licenses`: ok (the CDLA-Permissive-2.0
  note is the pre-existing unmatched-allowance warning, not a failure).
  (A mid-batch disk-full episode — 100% rootfs during the release
  build — was cleared by removing the 2.9 GB incremental cache; every
  suite was rebuilt and rerun cleanly, the debug binary for the smoke
  included.)
- Security: staged diff token-scanned clean before the push; remote
  fetched before the push.
- Runner verification: follows once CI on code commit `0458753`
  completes (recorded below when the runs finish).

Runner verification (batch 32, code commit `0458753` — CI on tracking
commit `94d7ea3`, which contains it): GitHub Actions run `40055344060`
completed — check runs "fmt, clippy, test, deny" and
"AppImage + .deb build, smoke" both concluded `success`.
(Also recorded: the batch-31 tracking commit's own CI run `40048669638`
on `41e9f01` — success; and run `40055912050` on the owner's batch-33
opening commit `31e2e58` is theirs to record.)

### Batch 33 — 2026-10-25 (code commit `b9d0f3c`; batch opened by the owner at `31e2e58`)

**137. Filter/assign refs load once per session** (low|gap)
— the inbox's users/mailboxes/tags/mention-directory loaded behind an
`is_empty` gate that fired exactly once: new teammates, mailboxes and
tags a sync brought in never reached the filter dropdowns or the mention
popup until restart. Invalidation clears the refs caches now, and the
next frame's empty-gate reloads them. Pinned by a regression test that
seeds a tag after load and asserts it reaches the tag filter following
an invalidate+reload. pages/inbox.rs.

**138. log_level saved, never read; display_timezone dead control**
(low|gap)
— two settings, two honest outcomes. `log_level`: the settings row was
written and never consumed — the tracing filter now sits behind a
reload layer; the stored level applies at boot (main, after the database
opens) and live on every settings save, with the pre-fix default's
directive shape preserved. `display_timezone`: written and loaded by the
form but consumed by NOTHING anywhere (the shell renders stored stamps
verbatim), so the control was a lie — it is removed from the struct,
load, save, and the General tab. The data-reset preservation test now
exercises `agent_language` (a live, wired setting) as its example row.
crates/core/src/logging.rs, crates/app-shell/src/main.rs,
pages/settings.rs. (The SPP_SMOKE shape count moves 4043 → 4040: the
removed timezone form row.)

**139. qdrant_enabled hint references a nonexistent control;
Integrations tab hint wrong** (low|gap)
— the Vector-index subtitle claimed "the index follows the
qdrant_enabled setting" but no UI control exposed it anywhere, and the
AI Center hint sent users to a Settings → Integrations tab that does not
exist. The toggle lives on the Vector-index tab now (applies on the next
start — the workers open the store at boot), and it is REMOVED from the
General form's blind load/save so a stale General save cannot clobber
the tab's toggle. The hint names Settings → LM Studio.
pages/settings.rs, pages/ai_center.rs.

**140. Unconfirmed destructive deletes** (low|issue)
— knowledge docs, custom-object types and records, connectors, outreach
segments and the diagnostics clear-log all deleted on a single click.
Every one now raises `NavRequest::Confirm` (the item-70 machinery) with
a dialog that names what is about to go; the shell's
`execute_confirmed` runs the owning page's method on confirm (the
page-level state handling — selection clears, reloads, the chunk-orphan
embed sweep — rides along) and the toast reports the honest outcome.
The confirmed path also fixed a silent swallow on the way: the knowledge
delete's `unwrap_or(false)` is gone (the core result reaches the
toast). pages/mod.rs, app.rs, pages/{knowledge,custom_objects,
connectors,outreach,diagnostics}.rs.

**141. Business-hours `days` unvalidated, silently ignored**
(low|issue)
— the editor saved the raw string; anything the SLA side could not
parse ("1-5", "mon-fri", "1; 2; 3") fell back to the default schedule
with no word to the user, and an all-out-of-range list ("[7,8]") passed
the raw is_empty check and then filtered to nothing — the SLA window
silently DISABLED. The save now parses, range-checks (0=Sun…6=Sat),
dedupes, sorts and normalizes the list (the JSON bracket form the form
pre-fills still parses), rejecting garbage with a message; the read
side degrades an all-out-of-range stored list to the default schedule
instead of disabling. Pinned by a save-side validation test and an
sla.rs effective_config regression. pages/settings.rs, sla.rs.

**142. Ops drill-down layers onto stale filters** (low|issue)
— clicking an Operations tile set `bar.ops` on top of whatever the user
had filtered in the inbox (view/query/mailbox), ANDing the tile fragment
with stale filters and shrinking the drill-down to a confusing subset.
The drill-down is a fresh navigation: the bar resets to the tile.
Pinned by a shell-level regression test. app.rs.

**143. `#778` number search silently dropped** (low|bug)
— the number field's own hint is `#`, but typing "#778" failed the
`parse::<i64>()` and `.ok()` silently dropped the filter. The entry
strips leading hashes now (`parse_number_entry`). Pinned by a parser
test. pages/inbox.rs.

**144. Support-graph invalidation clears results nothing refetches**
(low|gap)
— a sync event cleared the visible results of an active graph search
and nothing refetched them: the search stayed blank until the user
re-submitted. Invalidation schedules a refetch (`results_stale`) and
the next frame re-runs the query against the refreshed mirror. Pinned
by a regression test. pages/support_graph.rs.

**145. onboarding save_step dead; setting unread** (low|gap)
— `save_step` (the `/api/onboarding/step` route logic) had no caller
and the `onboarding_step` setting it wrote had no reader — the page's
step is derived from `first_run_done`. The dead writer, its test, and
the unread row are removed. pages/onboarding.rs.

**146. Recipient snapshot cap 5000, matched discarded** (low|gap)
— the campaign creator previewed a fixed 5000-row page and threw the
`matched` total away: every matched customer past the cap silently
never became a recipient while the segment said otherwise. The snapshot
now takes every matched row (page 1, unbounded page size) and the
success message reports the true count. Pinned by a regression test
matching 5001 customers and asserting all 5001 ship.
pages/outreach.rs.

Verification (Rule 5, all on the pinned 1.99.1 toolchain):
- `python3 scripts/gate_no_simulation.py`: clean.
- `cargo fmt --all --check`: clean.
- `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- Tests: core 467 passed / 0 failed / 4 ignored (2 new batch-33
  regressions: the audit141 out-of-range-days fallback; the
  apply_log_level no-panic guard); sandbox-feature suite 472 passed /
  0 failed / 4 ignored; app-shell 221 passed / 0 failed (5 new
  batch-33 regressions: the invalidation refs refresh, the ops
  drill-down reset, the hash-prefixed number entries, the
  support-graph refetch, and the 5001-recipient snapshot); xtask 6;
  catalog 0. (A mid-batch disk-full episode — 100% rootfs — produced
  68 environmental core failures identical to batch 31's; after
  clearing the incremental cache and stale /tmp fixtures every suite
  was rebuilt and rerun cleanly.)
- End-to-end smoke: `SPP_SMOKE=1` on the freshly built binary —
  exit 0, boot chain + shell/renderer verified, 24 pages rendered,
  4040 shapes (4043 − the removed display_timezone form row).
- Fresh harness `scripts/verify_batch33.py` (committed): all checks
  pass; the batch-18, batch-19, batch-21, batch-23, batch-24, batch-25,
  batch-26, batch-27, batch-28, batch-29, batch-30, batch-31 and
  batch-32 harnesses all pass. Pre-existing stale needles in batch-20
  (item-21 upsert-site needle restructured by batch-24's per-chunk
  partitioning) and batch-22 (the wrapped fence literal and the
  volume-spike flake check) unchanged — proven pre-batch during batch
  30; batch-17's script remains environment-bound as documented.
- `cargo deny check advisories licenses`: ok (the CDLA-Permissive-2.0
  note is the pre-existing unmatched-allowance warning, not a failure).
- Security: staged diff token-scanned clean before the push; remote
  fetched before the push (the owner's batch-33 opening commit
  `31e2e58` was rebased onto, not overwritten).
- Runner verification: follows once CI on code commit `b9d0f3c`
  completes (recorded below when the runs finish).

Runner verification (batch 33, code commit `b9d0f3c` — CI on tracking
commit `ef7e120`, which contains it): GitHub Actions run `40061678083`
completed — check runs "fmt, clippy, test, deny" and
"AppImage + .deb build, smoke" both concluded `success`.
(The owner's batch-34 opening commit `7bab5b1` — items 147-149, 152-158
moved to progress.md — is theirs to record.)

## Batch 35 — clean-audit items 159-165 (LOW tier: the AppImage arch that ignored its own host, the tool cache that could be corrupted by a crash and hung without a timeout, the bans promise nobody evaluated, the workspace deps nobody consumed, the catalog vectors nobody read, the icon verifier nobody ran, and the cargo invocations that rebuilt the lockfile silently) — opened 2026-10-24, executed concurrently with batch 34 at the owner's direction

Batch 35 ran alongside batch 34 (items 147-149, 152-158) by the owner's
explicit instruction ("another agent is working on other items"): this
batch touched only its own seven items' files; the `xtask/package.rs`
overlap with batch-34 items 157/158 was kept to this batch's regions and
every push rebased on the remote first.

**159. AppImage arch hardcoded x86_64 while the deb lane follows the host** (low|bug)
— `package()` hardcoded `x86_64` in both the artifact name
(`SupportOS-Plus-x86_64.AppImage`) and the appimagetool `ARCH` env, while
`package_deb()` mapped the build host through `deb_arch()` (x86_64→amd64,
aarch64→arm64): an aarch64 host shipped an arm64 .deb next to an AppImage
labeled — and runtime-pinned by ARCH — to the wrong architecture. New
`appimage_arch_for()`/`appimage_arch()` (same host set as `deb_arch`:
x86_64 and aarch64, anything else fails loudly) drive the artifact name
and the `ARCH` env. Pinned by the
`appimage_arch_covers_the_same_hosts_as_the_deb_lane` regression (both
mappings, the loud error branch, and host-agreement). Boundary note: a
full aarch64 AppImage lane additionally needs the appimagetool download
URL to be arch-aware — that URL is item 158's citation (batch 34) and was
deliberately left untouched here. crates/xtask/src/package.rs.

**160. Non-atomic tool-cache write + unbounded download** (low|bug)
— `download_appimagetool` wrote the download straight to the cached path
(`fs::write(dest)`): a crash or interrupt mid-write left a truncated file
at exactly the path `resolve_appimagetool` trusted on every later run
(`cached.is_file()` → run a corrupt "tool" → the package step failed with
a confusing cannot-execute error), and the download itself was a default
`reqwest::blocking::get` with no timeout — a stalled connection hung a CI
job forever. Now: a blocking client with connect_timeout(15s) +
timeout(300s), and the payload lands via `install_tool_bytes` — write +
fsync a `.part` sibling, chmod 0755, atomic rename (temp sibling removed
on error). Pinned by the `tool_cache_lands_atomically_at_the_final_path`
regression (install, re-install, no `.part` leftover, executable bit).
crates/xtask/src/package.rs.

**161. deny.toml stale "becomes deny" promise; bans never evaluated** (low|gap)
— `[bans] multiple-versions = "warn"` carried the comment "the reqwest
0.12/0.13 dedupe is a tracked plan item; this becomes deny after that
lands" — but the dedupe already landed (Cargo.lock carries exactly one
reqwest), no tracked item exists anymore, and CI ran only `cargo deny
check advisories licenses`, so the bans section was configured and then
evaluated by no one, ever. The promise is now enforced instead of
promised: `multiple-versions = "deny"`, and CI's deny step is `cargo deny
check advisories licenses bans`. A new duplicate crate version fails CI;
the pre-existing transitive duplicates of the checked
x86_64-unknown-linux-gnu graph are skipped by name with per-crate reasons
(crypto-stack generations, wayland stacks via winit, syn/thiserror
transitions, quick-xml 0.41 via pdf-extract, …); duplicates that exist
only on non-Linux targets (the Apple objc2 stack, windows-sys, jni-sys,
r-efi, redox_syscall) are outside the checked graph and documented as
re-add-on-target-widening instead of carried as dead skips.
deny.toml, .github/workflows/ci.yml.

**162. egui/egui_extras workspace deps unconsumed** (low|gap)
— the workspace declared `egui = "=0.37"` and `egui_extras = "=0.37"` in
`[workspace.dependencies]`, but no crate consumes either
(app-shell's only egui-stack dependency is `eframe`, which re-exports
egui); egui_extras was not even in Cargo.lock. Both entries removed; the
`eframe` entry (and its v1.1.1 glow-renderer rationale) stays, and the
section comment now states the policy: only consumed deps are declared.
`cargo metadata --locked` clean and Cargo.lock byte-unchanged by the
edit. Cargo.toml.

**163. catalog all_metric_entries/all_dimension_entries dead** (low|gap)
— both functions computed full catalog vectors (26 metrics, 13
dimensions) and had zero callers anywhere in the workspace; the per-key
`metric_entry`/`dimension_entry` constructors and the entry structs stay
(core/reports.rs pins every report's catalog entry with them, and the
`ALL` consts are consumed by core's str round-trips). The dead wrappers
are removed with a pointer comment at the site.
crates/catalog/src/reporting.rs.

**164. verify-icons never invoked by any automation** (low|gap)
— `xtask verify-icons` (full PNG-dimension, ICO-entry and ICNS-chunk
verification of the checked-in icon set) existed but nothing ever ran it:
CI didn't call it, and neither packaging lane did — packaging only
checked icon file existence. Now both `package()` and `package_deb()`
verify the set before staging anything (new `verify_icon_set`), and CI's
check job gains a `Verify icons` step (`cargo run --locked -p
supportos-plus-xtask -- verify-icons`) so icon corruption fails before
any expensive lane. The stale "consumes it in a later plan item" comment
at icons.rs:186 now states the real wiring. crates/xtask/src/icons.rs,
crates/xtask/src/package.rs, .github/workflows/ci.yml.

**165. No `--locked` on CI/release cargo invocations** (low|gap)
— every cargo build/test/clippy/run invocation in ci.yml (6) and
release.yml (7, including both sandbox-feature passes and the explicit
release build) could silently rewrite Cargo.lock on a drifted manifest
and build/test/ship artifacts from unpinned versions. All of them carry
`--locked` now, and xtask's own `release_build()` (the build CI's
`xtask package` performs) passes `--locked` too — the release path can
only build exactly the committed lock, and lock drift fails loudly
instead. `cargo metadata --locked` is clean on the committed tree.
crates/xtask/src/package.rs, .github/workflows/ci.yml,
.github/workflows/release.yml.

Verification (Rule 5, all on the pinned 1.99.1 toolchain):
- `python3 scripts/gate_no_simulation.py`: clean.
- `cargo fmt --all --check`: clean.
- `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- `cargo clippy -p supportos-plus-core --all-targets --features sandbox
  -- -D warnings`: clean.
- Tests: core 467 passed / 0 failed / 4 ignored; sandbox-feature suite
  472 passed / 0 failed / 4 ignored; app-shell 221 passed / 0 failed;
  xtask 8 passed / 0 failed (6 pre-existing + 2 new batch-35
  regressions: the appimage-arch mapping and the atomic tool-cache
  landing); catalog 0. (A disk-full episode — rootfs at 100% — produced
  39 environmental sandbox failures identical in class to batch 33's;
  after clearing the incremental cache and stale /tmp fixtures the
  identical suite was rerun cleanly: 472/0/4.)
- `cargo metadata --locked`: clean; the Cargo.lock stayed byte-identical
  through the manifest edit (item 162's premise, re-proven).
- `cargo deny check advisories licenses bans` (cargo-deny 0.20.2):
  advisories ok, bans ok, licenses ok — the bans verdict is new this
  batch; the only note is the pre-existing CDLA-Permissive-2.0
  unmatched-allowance warning recorded since batch 33, and the
  unmatched-skip warnings from platform-only skips are gone (the skip
  list carries exactly the checked graph's duplicates).
- `cargo run --locked -p supportos-plus-xtask -- verify-icons`: "icon set
  verified: png/ico/icns structures are consistent" (the exact CI step).
- Both workflow YAMLs re-parse (yaml.safe_load) after the edits.
- Harnesses: batch-18, 19, 21, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32 and
  33 all pass. The batch-20 (item-21 upsert-site needle restructured by
  batch-24's per-chunk partitioning) and batch-22 (the wrapped fence
  literal and the volume-spike flake check) failures are the documented
  pre-existing stale needles, unchanged from the pre-batch state (proven
  pre-batch during batch 30); batch-17's script remains
  environment-bound as documented.
- SPP_SMOKE: not run locally this batch — batch 35 changes no runtime app
  surface (xtask packaging code, a dead-code removal in catalog,
  manifests, CI and deny config only); the AppImage boot-chain smoke runs
  in the CI packages job on the pushed commit and is recorded below as
  the runner verification.
- Security: staged diff token-scanned clean before the push; remote
  fetched (and rebased when needed) before every push per the
  concurrent-batch discipline.

Runner verification (batch 35, code commit — CI on the tracking commit
that contains it): follows once CI completes (recorded below when the
runs finish).

### Batch 34 — 2026-10-25 (clean-audit items 147-149, 152-158)

**147. ensure_loaded-on-empty re-query pattern (7 pages)** (low|issue)
— six pages gated their first fetch on `if list.is_empty() { reload() }`
(outreach, organizations, connectors, notifications, incidents,
automation), so a genuinely empty mirror — the steady state of a fresh
install or an empty table — re-ran the full reload query on EVERY
rendered frame. Operations' suggestions lane had the same shape
(`workload.is_some() && suggestions.is_empty()` re-queried the
suggested-assignees read every frame when the list was legitimately
empty), and Search's auto-refetch latch re-ran its no-op reload every
frame for a whitespace-only query (`query.trim()` is never a response).
Every page now carries a `loaded` latch: `ensure_loaded` fires once,
`reload` sets the latch, `invalidate` clears it (the invalidation →
next-frame refetch semantics are unchanged), and the search guard trims
the query before latching. Pinned by a page-level regression
(`ensure_loaded_latches_and_invalidate_refetches`: first load, latch
holds across a re-entrant `ensure_loaded`, invalidation refetches).
pages/{outreach,organizations,connectors,notifications,incidents,
automation,operations,search}.rs.

**148. Dead Runner/JobRegistry/bulk_*/EventEmitter machinery**
(low|gap)
— `runner.rs` (Runner/JobRegistry/JobHandler/HandlerOutcome,
`run_until_idle`) had zero references outside its own module and is
deleted with its `pub mod runner;` wiring. The `EventEmitter` trait
(events.rs) and its only impl (on EventBus, eventing.rs) are removed —
every producer calls the inherent `EventBus::emit` directly. The
workers' `bulk_*` job-kind dispatch arm and its ~180-line
`execute_bulk_action` are removed: nothing in the workspace enqueues
jobs whose kind starts with `bulk_` (the UI's bulk actions ride the
outbound dispatcher), so the arm was unreachable and the handler dead.
runner.rs deleted; crates/core/src/{events,eventing,workers}.rs, lib.rs.

**149. Dead prompt surfaces** (low|gap)
— `ai_pipeline::report_narrative` (the standalone narrative runner) had
no caller — the weekly digest runs its own narrative lane with its own
run bookkeeping since batch 28 — and is removed with its now-orphaned
`REPORT_NARRATIVE_SYSTEM`, `PROMPT_VERSIONS_REPORT_NARRATIVE` and
`build_report_narrative_user`. The ai_prompts copies of the
attribute-extraction prompt (`ATTRIBUTE_EXTRACTION_SYSTEM` +
`build_attribute_extraction_user` + `PROMPT_VERSIONS_ATTRIBUTE_EXTRACTION`)
are removed — the feature owns its prompt and version in ai_attributes.rs
(the duplicate was the audit's "duplicate attribute prompt"). The dead
`ai_features` runtime below its migrations is removed: the coaching
suggestion parser, the customer-memory helper set (the live memory path
is ai_tools.rs, the copilot tool), the `TRANSLATION_PROMPT_VERSION =
"translation-v1"` pair — a mismatched duplicate of
`ai_prompts::PROMPT_VERSIONS_TRANSLATION = "translation_v1"`, the label
translation.rs actually writes — the post-resolution QA trio (the live QA
implementation is quality.rs), and the duplicate suggestions parser.
ai_pipeline.rs, ai_prompts.rs, ai_features.rs (migrations stay wired).

**152. Dead copilot/embedding helper surface** (low|gap)
— copilot.rs's `truncate_result`, `should_stop`,
`validate_tool_allowed`, `tool_allowlist` and the three constant
accessors had zero production callers (the live chat loop reads the same
constants directly and truncates inline); removed, and the item-121
byte-safety regression re-pinned against the live chat-loop truncation.
embeddings.rs's `embed_with_cache` (with its private `lookup_cache` /
`store_cache` helpers and `MAX_EMBED_RETRIES`) and the
`count_cache_entries` / `trim_cache_per_model` / `clear_cache` trio had
zero callers (the live embed pass writes the `ai_runs` ledger rows
directly); removed with the imports they orphaned — `content_hash`
stays (sync_engine consumes it). ai_lm_studio.rs's
`LmStudioProvider::new` / `with_base_url` / `Default` are removed (zero
callers of any kind); the it_lm_studio tests build the provider through
`with_client(OpenAiCompatibleClient::new(..))`. `with_client` stays
(production caller in ai_attributes.rs), as does
`with_base_url_and_timeout`. copilot.rs, embeddings.rs, ai_lm_studio.rs,
it_lm_studio.rs.

**153. AppConfig fields constructed, never consumed** (low|gap)
— of `AppConfig`, only `data_dir` was ever read (main.rs). The
`helpscout`, `ai`, `sync_interval_minutes` and `loopback_port` fields —
and the `HelpScoutConfig` / `AiConfig` structs with their `redacted()`
helper — were constructed and never consumed: credentials come from the
env (`EnvConfig`), settings live in the database, the sync interval is
the settings row, and the loopback listener binds an OS-assigned port.
The dead surface is removed and main.rs calls `default_data_dir()`
directly. `redact_string` (whose only production caller was the removed
`HelpScoutConfig::redacted`) moves into the test module so the item-107
byte-safety proof stays pinned; the batch-30 harness needle is
re-pointed to the indented test-module copy (documented in the harness).
config.rs, app-shell/main.rs, scripts/verify_batch30.py.

**154. Dead hybrid_search fusion stack** (low|gap)
— the module's entire fusion stack (`HybridResult`, `fuse_results(_with_k)`,
`merge_doc_hits` with its hit types, `cosine_similarity`, and the
`hybrid_search` entry point) had zero callers: the live search lanes
implement RRF directly in search.rs. The removed entry point was also
latent-broken — it fused sqlite-vec point ids (chunk-family namespaced
since item 21) against conversation remote ids, so a match between the
two lists was unrepresentable. The module shrinks to the one live
symbol, `pub const RRF_K: u32 = 60` (consumed by search.rs's three
fusion lanes), with the pin test kept. hybrid_search.rs.

**155. Legacy FTS tables + chunk_version column retired** (low|gap)
— `conversations_fts` and `customers_fts` ("legacy pre-parity tables")
were created, rebuilt and per-row written, and no search lane ever
queried them (the live lanes read `fts_conversations` /
`fts_threads` / `customers`). Their writers (`index_conversation`,
`index_customer`) had no callers and are removed with the CREATE
statements, the rebuild inserts and the rebuild list entries; the live
`index_conversation_fts` path is untouched. `chunk_version` was written
by five insert sites (two of them disagreeing: 1 in conversation_chunks
vs the schema default 2) and read by none; the column is dropped from
the four CREATE TABLE definitions and all five inserts (fresh databases
never create it; existing databases keep the column at its default,
harmless — nothing reads it). search.rs, mirror_tables.rs,
sync_engine.rs, knowledge_store.rs, embeddings.rs.

**156. Connector AI-search/auth-edit + data_tools runtime dead**
(low|gap)
— `connectors::search_for_ai` (the audit's AI-search anchor) and
`ai_visible_connector_names` had zero callers — the copilot's tool set
reads the local archive through ai_tools, not connectors — and the dead
lane carried a legacy-schema assumption (string rows through
`list_rows`) that drifted from the live storage shape; removed with the
private `get_by_name` helper they alone used. `connectors::patch` — the
connector edit/auth-change route logic — had no caller (the shell's
connector page creates, deletes and refreshes; it exposes no edit form),
so no code path could ever change a stored auth payload; removed.
`data_tools` shrinks to its `M026_TO_M027` migration: the module's
custom-object CRUD (owned by custom_objects.rs), connector CRUD (owned
by connectors.rs), db/settings export-import runtime (owned by
backup_service.rs + data_reset.rs) and `validate_ssrf` copy (superseded
by security::validate_ssrf_full*) all had zero callers and drifted from
the live schemas; removed with the bootstrap wiring untouched.
connectors.rs, data_tools.rs.

**157. AppDir staging never cleaned** (low|gap)
— `stage_appdir` created its directories and copied into whatever was
already on disk, so a leftover staging tree from a previous build
shipped inside the packed AppImage (a renamed artifact, a removed icon
size, a stale AppRun) — the .deb path already wiped `debroot` the same
way. `stage_appdir` now removes an existing AppDir before staging.
Pinned by a regression test that seeds a stale file and asserts the
re-stage drops it while the fresh layout lands.
crates/xtask/src/package.rs.

**158. appimagetool: unversioned URL, no checksum, executed in the
release path** (med|gap)
— the build downloaded whatever the moving `continuous` tag served that
day and executed it, unchecked, in the release path — a supply-chain
hole in the one step that produces the shipped artifact. The URL is
pinned to the versioned 1.9.1 release
(`appimagetool-x86_64.AppImage`) and the tool's SHA-256 is pinned
(ed4ce84f0d9caff66f50bcca6ff6f35aae54ce8135408b3fa33abfc3cb384eb0,
computed from the actual release artifact). The cached tool is verified
before every use; a fresh download is verified before execution; a
mismatch (tampered download, corrupted cache, upstream re-upload)
refuses to run with an explicit error instead of executing an unverified
binary. xtask hashes with sha2 (the same crate major the core already
pins). crates/xtask/src/package.rs, Cargo.toml, Cargo.lock.

Verification (Rule 5, all on the pinned 1.99.1 toolchain):
- `python3 scripts/gate_no_simulation.py`: clean.
- `cargo fmt --all --check`: clean.
- `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- Tests: core 455 passed / 0 failed / 4 ignored (467 − 13 removed
  hybrid_search dead-fusion tests + 1 new RRF pin test = 455; the
  copilot truncate test was replaced 1:1 by the live-path regression);
  sandbox-feature suite 460 passed / 0 failed / 4 ignored; app-shell
  222 passed / 0 failed (1 new batch-34 regression: the outreach
  reload-latch behavior); xtask 7 (6 + 1 new batch-34 regression: the
  stale-staging-tree clean); catalog 0. (A disk-full episode at 100%
  rootfs — the batch-33 environmental pattern — required clearing the
  incremental cache mid-suite; every suite then rebuilt and rerun
  cleanly with CARGO_INCREMENTAL=0.)
- End-to-end smoke: `SPP_SMOKE=1` on the freshly built binary — exit 0,
  boot chain + shell/renderer verified, all 52 boot migrations applied
  (the shrunk data_tools and ai_features migrations intact), 24 pages
  rendered, 4040 shapes (unchanged from batch 33 — dead-code removal and
  data-fetch latching are not visible geometry). The sandbox's
  loopback-port conflict produced the pre-existing environmental
  warning (the smoke's boot chain and shell verification are unaffected).
- Fresh harness `scripts/verify_batch34.py` (committed): 116 checks, all
  pass; the batch-18, 19, 21, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32 and
  33 harnesses all pass. Two documented needle re-points per the
  batch-26 protocol: batch-30's item-107 check now locates the
  test-module copy of `redact_string` (item 153 moved it with its
  regression), and batch-31's item-121 check now pins the live
  chat-loop truncation plus the removal of the dead helper (item 152).
- Security: staged diff token-scanned clean before the push (the only
  password/secret-shaped lines in the diff are the REMOVED dead
  data_tools export/import code); remote fetched (and rebased onto the
  owner's concurrent batch-35 commits) before the push.
- Concurrent-batch merge: the owner's batch 35 (items 159-165, commits
  0d24ece/8c72b95/9ffbc1b) landed while batch 34 ran. The rebase merged
  both batches' package.rs work (the item-158 version pin + SHA-256
  verify now ride batch 35's timeout-bounded atomic download, and the
  item-157 staging clean rides its verify-icons wiring); the tracking
  files were merged so both records live here and the queue is empty.
  The merged tree was re-verified locally end to end before the push
  (fmt/gate/clippy clean, core 455/0/4, sandbox 460/0/4, app-shell
  222/0, xtask 9/0, `cargo metadata --locked` clean, deny
  advisories+bans+licenses ok, harnesses 18-34 green, SPP_SMOKE exit 0
  with 24 pages / 4040 shapes).
- Runner verification (batch 34, code commit `2d3cdb4`, rebased onto
  batch 35's `8c72b95` — CI on tracking commit `1a9445d`, which contains
  both): GitHub Actions run `40071675943` completed with BOTH check
  runs concluded `success`, every step green — "fmt, clippy, test,
  deny" (no-simulation gate, fmt, verify-icons, clippy workspace +
  sandbox, tests workspace + sandbox, deny advisories/licenses/bans)
  and "AppImage + .deb build, smoke" (the real packaging path exercised
  the item-157 staging clean and the item-158 pinned/checksum-verified
  appimagetool, then booted the AppImage and the extracted .deb payload
  under SPP_SMOKE). The same run also completes batch 35's runner
  verification (its code commit rides in it).

Runner verification (batch 35, code commit `8c72b95` — CI on tracking
commit `9ffbc1b`, which contains it): GitHub Actions run `40069963833`
completed — check runs "fmt, clippy, test, deny" and "AppImage + .deb
build, smoke" both concluded `success`. Green among the steps are the
three this batch added: "Verify icons" (item 164's CI wiring), "Deny
(advisories, licenses, bans)" (item 161 — the bans section enforced and
evaluated for the first time), and "Build the AppImage (xtask package)"
running the arch-following, `--locked` release path (items 159/165) plus
both boot-chain smoke tests (the SPP_SMOKE evidence for this batch).

File repair note (completed.md only, no code touched): batch 34's
tracking commit `1a9445d` landed with two unresolved rebase-conflict
markers inside its batch-34 record (an orphaned `>>>>>>>` closer after
item 158 and a full `<<<<<<< HEAD` block whose HEAD side duplicated
batch 35's verification bullets above; the block's own batch-34 side —
the tests 455/460/222/7 list, the SPP_SMOKE record and the
verify_batch34 harness note — is kept verbatim). Both markers are
removed here so the file carries both batches' records cleanly;
plan.md/progress.md were verified marker-free.

---

## Batch 36 — clean-audit items 56, 150, 151 (2026-10-25)

The owner resolved the final three parked decisions with an explicit
"wire it" for each on 2026-10-25; the items moved from `plan.md` to
`progress.md` (opening commit `7166f2a`) before implementation began
(Rule 2). This batch WIRES the formerly computed-but-unrendered and
never-written surfaces — every formerly caller-less function named below
now has a live production caller, and every writer-less table now gains
rows.

### Item 56 — the report-builder/dashboard surface rendered (reports.rs:1110+)

- **Builder tab (Reports page):** the full builder form over the closed
  catalog — metric (21 entries) and dimension (14 entries) combos driven
  by `catalog::reporting` labels, the date range (YYYY-MM-DD, validated
  in the form before it reaches the engine), previous-period comparison
  toggle, sort, limit, and the ten-filter set (channel, statuses, tags
  any/none, min priority, attribute key/value, state key, mailbox and
  assignee id lists — CSV parsed with per-item validation). Run goes
  through `run_builder_report`; rows render grouped by
  `dimension_label` with the metric's catalog format (count/rate/
  minutes/hours/score) and the comparison delta column; the outcome's
  notes render verbatim under the table. `config_to_json` +
  `parse_report_config` back the saved-report CRUD: save (named),
  list, load-into-form, delete over `report_definitions`.
- **Insights tab (Reports page):** health facts
  (`get_health_facts` over `get_dashboard_metrics` — total, tracked SLA
  breaches with rate, high-friction with rate, avg first response,
  resolution rate), the QA effectiveness report (avg score, checks,
  low/medium/high distribution), and the graph-node surface
  (`list_graph_nodes` table, `add_graph_node` form with JSON-property
  validation and core-side kind validation, `get_graph_node` +
  `get_graph_neighbors` inspect action over the live human edges).
- **NEW write path (core):** `reports::recompute_recent_friction(conn,
  limit)` — the bounded recompute over the most recent 100 closed
  conversations that persists scores via compute_friction's own upsert
  and records a `customer_timeline` milestone for every conversation
  crossing HIGH_FRICTION_THRESHOLD, deduped per conversation through
  `json_extract(event_data_json, '$.conversation_id')` (a recompute
  never duplicates a milestone).
- **Support milestones timeline (Customers page):** the profile view
  gains a "Support milestones" card reading `get_timeline` — the reader
  for the milestone writers (breaches from the item-150 sweep,
  high-friction from the recompute above), rendered with the parsed
  breach kind / effort score per event.

### Item 150 — the intelligence tracking wired to real writers

- **Workers intelligence sweep:** `intelligence_sweep_tick` on the
  WorkerManager (own TickGuard latch `sweeping_intelligence`, restart
  reset, `intelligence_sweep_seconds` settings cadence defaulting to
  900 s clamped 60..86400, immediate first tick) running
  `intelligence_features::sweep` — three independent legs where one
  failure never eats the others' work.
- **SLA breaches gain rows from the LIVE evaluation:** the sweep
  persists every `sla::sla_alerts` Breached verdict into `sla_breaches`
  (deduped per conversation + breach kind via EXISTS), so
  `reports::get_dashboard_metrics`'s breach count — previously a
  permanently empty table — reflects the same authority the SLA page
  uses. Each persisted breach also records a `customer_timeline`
  "sla_breach" milestone for the conversation's
  `customer_local_id` (best-effort; a milestone failure never fails the
  sweep).
- **`sla_configs` write API wired:** mailboxes WITHOUT a
  `mailbox_business_hours` row (exactly the ones `sla_alerts` reports
  unconfigured) are tracked by a coarse wall-clock sweep against
  `get_sla_config` thresholds (schema defaults 24 h / 72 h) — disjoint
  from the live evaluator by construction, so no double-recording. The
  Reports SLA tab gains the "Persisted breach tracking" card: recorded
  count + per-mailbox threshold editor writing `set_sla_config` and
  running the unmonitored sweep immediately so the count reflects the
  new thresholds.
- **Knowledge gaps gain rows:** a non-blank Knowledge-page search that
  returns zero articles records `record_knowledge_gap` (deduped
  hit_count upsert in core), and the page renders the "Knowledge gaps
  (tracked)" card listing `list_knowledge_gaps` (top 10, hit order) —
  the actionable list of what agents searched for and never found.
- **Doc freshness gains rows:** `knowledge_store::upsert_document`
  stamps `update_doc_freshness` on BOTH paths (changed commit and
  unchanged re-ingest — a re-sync is still a sync), best-effort so
  freshness can never fail an ingest; the sweep prunes orphaned
  freshness rows, marks docs stale when their content has not been
  re-ingested within the live `freshness_stale_days` window (same
  setting the Knowledge freshness report uses), and clears the flag
  when content refreshed; the tracked stale count renders on the
  Knowledge page.

### Item 151 — the ai_settings write API wired

- The AI Center Health tab gains the "AI provider record (ai_settings)"
  panel writing all four previously caller-less setters on Save —
  `set_provider_kind` (None / LM Studio chips),
  `set_chat_model`, `set_embedding_model` (requires a positive
  integer dim whenever a model name is set; an empty model leaves the
  embedding columns untouched — the write API has no NULL-clearing
  path and none was invented), and `set_base_url`. The row is the
  pre-hub fallback layer `ai_pipeline::backend_from_settings` and
  `embeddings::lm_studio_embedding_settings` already consume, so the
  edits are load-bearing, not decorative; Settings → LM Studio remains
  the primary editor for the settings-key layer and the pipeline
  precedence is unchanged (no regression for existing installs).
- `ai_features_enabled` (previously caller-less) is the panel's
  "features ready" badge — provider configured AND chat model present.
- Form discipline follows the batch-27 convention: a clean form
  re-seeds from the stored record on every reload; a dirty (typed)
  form keeps its edits across invalidation until Save commits them
  through the core write API and re-reads the stored row.

### Verification (local, before the push)

- No-simulation gate clean; `cargo fmt --all --check` clean.
- `cargo clippy --workspace --all-targets -- -D warnings` clean;
  sandbox clippy (`-p supportos-plus-core --all-targets --features
  sandbox`) clean.
- Workspace tests: core **466/0/4** (455 + the 11 new batch-36
  regressions: ai_center setter round-trip + ready-indicator truth
  table; the breach sweep live-persistence + dedup + timeline, the
  unmonitored sweep honoring sla_configs, knowledge gaps upsert, doc
  freshness stamp/mark/clear/prune, the combined sweep; friction
  recompute with deduped milestones, builder-config JSON round-trip,
  saved-report CRUD, graph-node validation), app-shell **228/0** (222
  + 6 new: provider-record save round-trip / dim validation / dirty-form
  survival; builder run-save-load-delete / form validation / insights
  data + graph action), xtask 9/0, catalog 0.
- Sandbox suite: **471/0/4** (460 + the same 11 new core tests).
- `cargo metadata --locked` clean; `cargo deny check advisories
  licenses bans` all ok; `xtask verify-icons` green.
- SPP_SMOKE exit 0 on the built binary: boot chain + shell verified,
  **24 pages, 4070 shapes** (4040 at batch 34/35 + the rendered
  Builder/Insights tabs, provider panel, gaps card, tracking card and
  milestones card), all 52 boot migrations applied.
- Fresh harness `scripts/verify_batch36.py` (52 checks) all pass —
  needles pin every new call site listed above in non-comment source.
  Harnesses 18, 19, 21, 23–34 all green. The two documented
  pre-existing stale needles (batch-20's item-107 re-point, batch-22's
  item-121 re-point plus its 00:00–09:00 UTC volume-spike window check)
  fail exactly as recorded since batch 30; this batch touches none of
  the files those needles read, and batch-17 remains environment-bound.
- Environment note: the sandbox disk filled mid-verification (the
  recurring ENOSPC, 9.9 GB root); the debug incremental cache (1.9 GB)
  was removed and `CARGO_INCREMENTAL=0` held for every suite above.
- Security: the staged diff was token-scanned before the push — no
  secrets, no credential-shaped strings; the remote was re-fetched and
  is at the opening commit's descendant (no concurrent batches).
- Runner verification follows once CI on this code commit completes.

Runner verification (batch 36, code commit `7b96ada` — CI on tracking
commit `8b4c2af`, which contains it): GitHub Actions completed with BOTH
check runs concluded `success`, every step green — "fmt, clippy, test,
deny" (no-simulation gate, fmt, verify-icons, clippy workspace + sandbox,
tests workspace + sandbox, deny advisories/licenses/bans) and
"AppImage + .deb build, smoke" (the real packaging path exercising the
batch-36 wiring, then both boot-chain smoke tests on the packaged
artifacts). The v1.7.0 clean-audit queue stands fully consumed and
runner-verified.

### Batch RS-1 — 2026-10-28 (commit `af5bc2a` `design-lab-v4`, pushed; Gate G1 passed by owner `continue`)

Design-System Reskin Stage 1: design-lab conformance rebuild (RS-01..RS-10), touching
only `mockups/design-lab/` — zero Rust files (scope contract decisions.md D6).

- **RS-01 — engine:** `node --check` clean on `ds.js`/`pages-ds.js`/`reskin.js`; vm smoke harness: 23/23 registry
  pages resolve to exactly one archetype (`stack`=DS-A-02/A-04, `console`=A-06, `list`=A-01, `record`=A-03,
  `inbox`=A-05), every `meta/tabs/blocks/list/detail/hero` renders strings without data errors; zero page markup
  outside `DS.*` (pages-ds.js: 0 inline styles, 0 non-DS classes — grep evidence).
- **RS-02 — Inbox:** DS-A-05 workspace live; golden-path suite re-verified in browser: open conversation, view
  filters (Mine→2/All→6), reply send (agent card + durable-job toast), note tab + send, saved-reply insert,
  AI message insert, copilot resolution insert, copilot toggle, Ctrl+Enter send, `j`/`k` + `/` shortcuts,
  notes drawer open/close. Both themes screenshot-verified.
- **RS-03..RS-09 — pages:** full 23-page × both-theme browser sweep with **zero console errors**; spot-verified
  interactions: reports grouped tabs (Builder tab), search Enter run, notifications mark-read/mark-all,
  knowledge help-center single FilterBar (A-2), automation New Rule state, customers/organizations/incidents
  DetailPanels (DS-C-17) with DS-A-03 record-head composition.
- **RS-10 — burn-down:** `pages-reskin.js` deleted (receipts at a8492d0); `reskin.css` 756 → ~190 lines,
  shell-only, consumes `--ds-*` tokens; tone audit clean (all status via DS.TONE; TONE rows added by the
  map-row-first process: live, catching_up, incident + campaign statuses). Register A-1..A-8 + B-1..B-3
  **CLOSED** with receipts in `mismatch-register.md` + live on `ds.html#register`. Goldens: 23 pages ×
  2 themes × 1440/1024/800 = **138 PNGs** in `mockups/design-lab/goldens/v4/` + `audit.json` structural
  record (one page-head everywhere; filter/detail/header parity).

Verification summary: node --check ×3 clean; vm registry smoke 23/23; browser sweep 23 pages × 2 themes,
zero console errors; golden set 138 PNGs + audit.json; grep gates (0 inline styles, 0 non-DS classes in
pages-ds.js); mismatch register A-1..A-8 + B-1..B-3 closed with receipts; diff `af5bc2a` = 150 files,
+1911/−2078, 0 `crates/` paths.

**GATE G1:** owner reviewed the conformed lab and approved it as the final design — approval receipt is the
owner's `continue` (2026-10-28) following the Batch RS-1 STOP report, per the rule.md STOP→continue protocol
(same pattern that opened Batch RS-1 after the plan STOP). Stage 2 (Rust) is hereby unblocked; Batch RS-2
(RS-11..RS-20) opened.

---

## Active items — Batch RS-2 (Design-System Reskin, Stage 2: egui reskin R1–R3) — opened 2026-10-28

Scope: **Rust** (`crates/app-shell/` + workspace tooling) — pure egui reskin per decisions.md
D6/D1: presentation only, no feature/data/workflow/shortcut/page-set change, no HTML/CSS/JS
shipped. Design source of truth: Design System v1.0, frozen by Gate G1 (owner `continue`,
2026-10-28, after Batch RS-1 STOP; lab state `af5bc2a` = approved reference). Gate evidence
packs (G2 tokens, G3 Inbox pilot) are presented at the batch STOP; the owner may split this
batch at either gate on request.

**RS-11 — F0: Design Contract v1.0 freeze.** Extract `docs/audit/design-contract-v1.md`
from the conformed lab: token sheet (--ds-* values, both themes), 17 DS-C component
sheets (anatomy/variants/do-don't + egui mapping note each), 6 archetype sheets,
DS.TONE table, exception register, change-control rule (DC bump before code). Verify:
DC committed; every register A/B row closed in DC; RS-10 goldens referenced as the
visual baseline.

**RS-12 — R1: token port + style lint.** Rebase on latest origin/main; re-scan stray
hardcodes (raw `Color32::from_rgb` sites). `crates/app-shell/src/design.rs` LIGHT/DARK
token tables → DC v1.0 tokens 1:1; spacing/type/radius scales; DS.TONE → one match arm.
Style lint (CI test): forbid raw Color32 outside design.rs and off-scale spacing.
Verify: cargo fmt/clippy/test green; SPP_SMOKE per-page paint gate green; page
structure unchanged (colors/typography now DS — this IS the G2 token sign-off);
fresh goldens lock the token state against drift; G2 evidence pack (token diff table)
presented at batch STOP.

**RS-13 — R2a: structure widget set.** Add egui_kittest (pinned to the egui 0.37 line)
as dev-dependency + harness. Widgets 1:1 with `DC: DS-C-xx` doc-comments: PageHeader
(counter pill), DataTable (virtualized rows), StatTile/MetricTile, FilterBar, Tabs
(underline), Badge/Chip/StatusDot via DS.TONE, button set. Verify: kittest snapshots
per widget/state, light+dark; fmt/clippy/tests green.

**RS-14 — R2b: feedback widget set.** EmptyState (+slim variant), Callout set (five
fixed roles, B-1), Toast, armed ConfirmDialog, FormField/inputs, DetailPanel,
Avatar/Assignee. Verify: same harness + snapshots + DC mapping.

**RS-15 — R2c: inbox identity widget set.** Role-typed Message Card
(agent/customer/note/AI), day divider, quoted-history collapse, attachment chips,
Composer (Reply|Note, saved-replies, AI insert, draft-saved flag), AI badge set, typed
copilot cards. Verify: harness snapshots; 5-second AI-identification pre-check on the
isolated card.

**RS-16 — R3 pilot: Inbox rebuild.** Rebuild the Inbox UI strictly from the widget set
(list, reader, composer, copilot panel; `crates/app-shell/src/pages/inbox.rs`); zero
bespoke styling. Functional regression checklist: every existing interaction identical
(pagination past the 50-row cap, draft retention on row click, sync trigger, triage
loads). No new per-frame DB re-queries introduced by the rebuild (existing data-path
issues stay out of scope — data track, decisions.md D6). Verify: checklist recorded;
kittest goldens at 1440/1024/800 vs the RS-10 lab goldens; G3 evidence pack
(before/after + checklist) presented at batch STOP.

**RS-17 — R3 wave A: Dashboard, Customers, Organizations, LiveChats.**

**RS-18 — R3 wave B: Knowledge, Incidents, IssueRadar, Reports (+ Report Builder).**

**RS-19 — R3 wave C: Operations, AiCenter, Automation, CustomObjects, Outreach.**

**RS-20 — R3 wave D: Settings, Notifications, Search, SupportGraph, Connectors,
Diagnostics, SyncHealth, Onboarding, Shell.**

Each wave (RS-17..RS-20): rebuild from the widget set only; per-page functional
regression checklist; kittest goldens vs lab goldens; any egui-cannot-reproduce gap
becomes a registered exception in the DC — never silently approximated. Verify per
wave: checklist + goldens + fmt/clippy/tests + SPP_SMOKE green.

### Batch RS-2 status — checkpoint 2026-10-28 (RS-11..RS-15 verified; RS-16..RS-20 next)

- **RS-11 — F0 DC freeze:** `docs/audit/design-contract-v1.md` committed (f84d43c): token sheet both
  themes (exact hex), 17 DS-C sheets with egui mapping notes, 6 DS-A recipes, DS.TONE 25 rows,
  register A/B closed + C keeps + E-1, change-control rule, RS-10 goldens as visual baseline.
- **RS-12 — R1 token port:** design.rs LIGHT/DARK = DC tokens 1:1 (fbf106c): canvas #f4f4f5, ai/agent/
  warning-border/neutral-fg/on-ai/shadow/scrim added, hyperlink+charts → AI violet, dark faint #0c0c0e;
  type roles 22/13.5/13/12.5/12 wired into install(); DS.TONE = one 25-row match arm (BadgeTone::Ai,
  tone_colors_of shared core — inbox row-painter collapsed onto it); design::scale (space/type/radius +
  DC pads); **style lint CI test** (no raw Color32 outside design.rs, no off-scale spacing, B-3
  impossible); doctor colors + dialog/toast/palette shadows + scrim → tokens; 3px gaps normalized;
  DC pin tests. Contrast note: text-2 on canvas = DC-frozen 4.11:1 (4.7:1 on surface).
- **RS-13 — R2a structure set:** egui_kittest 0.37 dev-dep (kittest-ui feature, like ui-screenshot);
  page_counter (DS-C-01), status_dot/tone_solid (C-06), tabs/tab_groups underline (C-09),
  filter_bar one-primary-apply (C-10/A-2), metric_tile banded+drill (C-04), data_table virtualized
  pinned-header (C-05), metric_card→26px (C-03), ButtonVariant Text/Ai + manual DS-C-11 anatomy
  (fixes the real-shell icon/text overlap); 4 snapshots diff-locked; headless via Xvfb+lavapipe.
- **RS-14 — R2b feedback set:** empty_state_slim/cta (C-14/A-7), callout/status_line/quirk
  (C-15/B-1 roles), avatar/assignee (C-19), armed_danger_button with frame-guard vs auto-Id
  collision (C-16/A-5), detail_panel right-sheet + scrim + Esc (C-16/A-1), form_row (C-13);
  dashed strokes unavailable in egui → registered E-2; 8/8 snapshots diff-locked.
- **RS-15 — R2c inbox identity set:** message_card role-typed customer/agent/note/AI (C-20, G-2),
  day_divider, quoted_history, file_chip, composer (C-21 pure-UI act enum: send/saved-reply/AI-insert,
  Ctrl+Enter, draft-saved flag, amber note mode), cop_suggest/cop_similar/cop_chat (C-22); style-lint
  allowlist extended with DC-frozen sheet pads 5/7/11/13 (DC §2.4 note); 14/14 diff-locked incl. the
  5-second AI-identification pre-check + send/Ctrl+Enter interaction tests.

**Verification per item:** cargo fmt clean; clippy workspace -D warnings green (both feature configs);
cargo test: app-shell 233 (incl. DC pin tests) + core 485 + xtask 11 + style lint 2; kittest 14/14
(diff-locked twice: baseline write + clean diff); SPP_SMOKE exit 0 (fresh binary each time). Commits:
f84d43c, fbf106c, 8bb994f, b5e613d, 9718cd7 — all pushed to origin/main.

**Next in batch:** RS-16 (Inbox pilot rebuild from the widget set + regression checklist + G3 goldens),
then RS-17..RS-20 waves, then the G2/G3 evidence packs at the batch STOP.

**RS-16 — R3 pilot, Inbox rebuild (verified 2026-10-28):** `pages/inbox.rs` rebuilt on the
widget set (4004 lines): views rail via `design::view_item` (DS-A-05, counts beside the view,
A-6), list rows + reader via `message_card`/`message_card_with` (DS-C-20, rich-body closure —
`render_rich_spans` plain spans stay at HEAD sizing), composer/copilot panels unchanged in
behavior; a11y pass — the assign ComboBox now carries a programmatic label
(`widget_info` "assign: …", repo pattern) so kittest's accessibility check holds ON for the
real-page suite. Regression checklist all green in the 233: pagination past the 50-row cap
(page_one_reaches_conversations_past_the_cap, offset_compiles_from_the_page_index,
a_page_past_the_end_falls_back_to_the_last_page), draft retention on row click
(row_click_stashes_and_restores_the_typed_draft, reselecting_the_same_row_keeps_the_typing_
untouched), sync trigger (submit_reply_creates_durable_job_and_blocks_duplicates, r4_d8/r4_d9),
triage loads (ops_tile_filter_drills…, change_status_writes_locally_and_queues_remote_job,
invalidation_refreshes_the_filter_refs); no new per-frame DB re-queries (lazy empty-gate
reloads + event invalidation, same data path as HEAD). Zero bespoke styling — style lint 2/2
over the rebuilt file. **Page goldens:** new tests/kittest_inbox_page.rs renders the REAL
InboxPage (seeded in-memory DB, fixed timestamps for determinism) at 1440/1024/800 × 900,
light+dark = 6 goldens, diff-locked twice; lab reference for the G3 side-by-side is
mockups/design-lab/goldens/v4/{light,dark}/inbox-*.png (egui render ≠ HTML pixels by design;
side-by-side review = G3 evidence pack at batch STOP). Verification: fmt clean; clippy -D
warnings green (app-shell all-targets + core sandbox); cargo test: 233 + 485 + 11 + lint 2
+ core sandbox 493, workspace exit 0; kittest 20/20 (14 widgets + 6 page) with a11y check ON;
SPP_SMOKE exit 0. **CI note:** pushes since RS-13 (8bb994f..d172cfc) failed in CI — the
regenerated Cargo.lock (egui_kittest dev-dep tree) had never been committed; fixed by 8490eee,
both CI jobs fully green on it. **Next in batch:** RS-17 wave A (Dashboard, Customers,
Organizations, LiveChats).

**RS-17 — R3 wave A (verified 2026-10-28):** all four pages rebuilt onto their DC archetypes,
behavior byte-identical everywhere, a11y labels on every interactive control, zero bespoke
styling. **Organizations → DS-A-01** (1b9f521): page_header + DS-C-01 counter pill (lab's
"[counter] [Sync now] [status line]" order), hand-rolled filter row → DS-C-10 filter_bar,
chip list → DS-C-05 data_table (same 200-cap query), row open → DS-C-17 DetailPanel with the
DS-A-03 record composition; create form stays page-flow card_titled + form_row per the lab.
**LiveChats → DS-A-04** (632435e): counter pill "n open · m ended", the lab's view toolbar
(field label + two DS-C-06 chips — the chip pair IS the view switch), DS status-dot rows,
detail as card_titled record; the inclusive "show ended" filter (show_ended || is_open) kept
verbatim — original behavior per D6. **Dashboard → DS-A-02** (9aa93fc): the lab's block stack
(statGrid = DS-C-03 metric_card stats, DS-C-04 banded tiles with unchanged drill callbacks,
new **DS-C-23 bar_row** added to the design set — the lab's .ds-bar-row with the 3% fill
floor, real label/value text in the a11y tree); by-channel card keeps its labeled quirk; no
hero (A-4). **Customers → DS-A-01** (1d48482): counter pill, filter_bar, the 28px show_rows
list → data_table (same query/row semantics), detail → DS-C-17 + DS-A-03. **Goldens:** 4 new
real-page suites (kittest_list_pages.rs orgs+customers 14, kittest_live_chats_page.rs 7,
kittest_dashboard_page.rs 7, kittest_inbox_page.rs 6) — every page at 1440/1024/800 × 900
light+dark, diff-locked twice, a11y check ON, +4 interaction goldens (org detail open, chats
tab switch, dashboard scope-chip drill, customers detail open). Verification per commit:
fmt clean; clippy -D warnings; workspace 234+485+11+lint 2 exit 0; kittest 48/48; SPP_SMOKE
exit 0. **Next in batch:** RS-18 wave B (Knowledge, Incidents, IssueRadar, Reports+Builder).

**RS-18 — R3 wave B (verified 2026-10-29):** all four pages rebuilt onto their DC
archetypes, behavior byte-identical everywhere, a11y labels on every interactive
control, zero bespoke styling. **Incidents → DS-A-01** (4dc7133, pilot): page_header +
DS-C-01 counter pill ('[counter] [New Incident]', the lab's order), the M20 cap note
rides the subtitle; the filter card is DS-C-10 filter_bar with the item-131 semantics
preserved verbatim (combos + checkbox refetch on change, the text query keeps its
explicit apply); the chip list is DS-C-05 data_table (code strong / title / severity +
status badges / conversations mono / open act, same 100-capped list_incident_rows
query); row open opens the DS-C-17 DetailPanel with the DS-A-03 composition; store-
shape fix — incident_detail nests the row under 'incident' and the pre-rebuild page
read the top level, silently rendering an empty record head (same class of fix as
organizations' members in RS-17a). **IssueRadar → DS-A-02** (6aefd55): the chip row is
the DS-C-09 tab rail (same four labels/state); Alerts keeps its card + badge rows and
gains the DS-C-15 quirk receipt (raw severity tokens, DC C-1); SLA rows render as
DS-C-15 callout roles (breached → Err, at-risk → Warn); Gaps keeps totals badges +
per-kind cards — approve one click (Outline), reject the lab's armed two-click form
via armed_danger_button (DS-C-16/A-5; the decide_gap write unchanged). **Reports →
DS-C-09 grouped rail**: the flat eleven-chip row is the DS-C-09 grouped tab rail (the
lab's three groups over the same eleven labels from Tab::label and the same
global-index state machine); the SLA first-response card renders the DS-C-23 bar-row
list (real label/value per mailbox + the 'first response avg · max N' hint) instead
of the painter chart that kept no per-mailbox text; the radar tab gains the lab's
DS-C-15 quirk receipt (raw severity token, DC C-1); digest anomalies render through
section titles + the lab's warning-badge anomaly rows (same strings); Happiness
recent = the shared striped table with the Great link color + the Not-good danger
badge; Help Scout picker widget_info 'report: …' + outline fetch; Builder saved rows
pair outline load with the danger delete (arm stays the shell confirm, FIND-005).
**Knowledge → DS-C-09 rail**: the chip row is the underline tab rail (same five
labels, the rail's own divider replaces the hand-drawn one); the Documents table
keeps the three inline acts (mark reviewed / verify / delete — the lab's A-5 collapse
would remove the verify workflow step, a D6 feature change, so the triple stays;
delete rides its shell confirm arm, item 140); Drafts queue rows adopt the lab's
anatomy (strong id — title, status + publish states as DS.TONE badges with raw tokens
visible per DC C-1, outline Open, outline Reject — the one-click decide write
unchanged). **a11y (wave-wide):** every combo carries widget_info 'label: value' and
every TextInput binds its visible field label via Response::labelled_by —
design::field_label now returns the label response so inputs are named with zero
visual change (egui never reads a preceding label on its own). **Goldens:** 4 new
suites this wave (kittest_incidents_page.rs 7, kittest_issue_radar_page.rs 7,
kittest_reports_page.rs 7, kittest_knowledge_page.rs 7 — 6 real-page goldens each at
1440/1024/800 × 900 light+dark + the open-detail / Gaps-tab / Builder-tab /
Help-center-tab interaction goldens), diff-locked twice, a11y check ON. Verification
per commit: fmt clean; clippy -D warnings (workspace + core sandbox); app-shell 234 +
core 485 + core sandbox 493 + xtask 11 + lint 2 exit 0; kittest all suites green
across the eight files; SPP_SMOKE exit 0 (24 pages, 4168 shapes); CI green on
4dc7133. **Environment note (this sandbox, not a code event):** the root filesystem
hit 100% mid-verification, failing 72 filesystem core tests and one kittest link
(bus error) — freed by pruning target/debug executables + stale rlibs + ~/.cache,
everything re-ran green. **Next in batch:** RS-19 wave C (Operations, AiCenter,
Automation, CustomObjects, Outreach).

**RS-19 — R3 wave C (verified 2026-10-29):** all five pages rebuilt onto their DC
archetypes, behavior byte-identical everywhere, a11y labels on every interactive
control, zero bespoke styling. **Operations → DS-A-01 pilot** (7001cf8): DS-C-09
underline tab rail; the Tiles tab renders severity-banded DS-C-04 metric tiles
through design::metric_tile (band tints count + 3px rail per DS.TONE, drill tiles
carry the lab's 'open →' line, full-tile hitbox keeps the inbox drill); the
Workload table adopts the lab's presentation fix (weighted number under 'Load', %
under 'Pressure' — the pre-rebuild build swapped the values; quirk receipt added);
repair surfaced by the a11y-check-ON harness: the workload tab's two ScrollAreas
shared egui's auto-id chain (a pre-existing ID clash whose debug overlay panics
the kittest glyph check) — distinct id_salts now; kittest-only seam freezes the
snapshots' wall-clock generated_at (D6 — the lab's hint is a static fixture),
compiled out of prod. **AiCenter → DS-C-09 rail** (c6ecefd): the model-role
routing combo shows the endpoint NAME instead of its index (the lab's
presentation fix + quirk receipt) and both routing controls carry programmatic
names. **Automation** (643f671): approval rows adopt the lab's presentation fix
(the human-readable action renders first as a badge — the tagged-enum JSON parses
through core's Action and rides action_description, raw strings fall back
verbatim — and the raw JSON moves into a 'raw proposed action' collapse, quirk
receipt added); approve rides the lab's outline button (the one-click decide
write unchanged, item-80 attribution intact). **CustomObjects** (ecd0564): the
header is the DS-C-01 PageHeader with the New Type primary + the A-6 counter
pill; the object pane head becomes the lab's ds-tool-row (strong title, count
pill right) and the M20 50-row-cap honesty note moves beneath as a hint; the
search toolbar is the DS-C-10 FilterBar; object cards adopt the lab's kv-row
anatomy (muted key, mono value) and the empty state carries the lab's copy; the
select-options input (no visible label) carries the ai_center-style programmatic
name. **Outreach** (2444a1f): the chip row is the DS-C-09 rail; campaign cards
adopt the lab's split ds-tool-row (lifecycle acts left, cancel-remaining danger +
retry-failed outline right) and the lab's CAMP badge semantics (completed onto
the resolved success badge, every other lifecycle token neutral with the raw
token visible per DC C-1); the Campaign report card grows the lab's DS-C-15 warn
receipt (the report reads top-level keys while core nests them under totals —
outreach.rs:1336 — every row renders 0; data-layer bug, out of reskin scope,
documented not fixed, the reads stay byte-identical per the lab's own note);
segment rows adopt the lab's card anatomy (strong id — name, delete pushed right,
the criteria JSON as the mono line beneath — the definition was in the row JSON
all along but the pre-rebuild build kept it invisible); DNC rows push the
reason/ts/remove cluster right; every form input binds its visible label via
Response::labelled_by. **Goldens:** 5 new suites this wave (kittest_operations_page.rs
7, kittest_ai_center_page.rs 7, kittest_automation_page.rs 7,
kittest_custom_objects_page.rs 7, kittest_outreach_page.rs 8 — 6 real-page goldens
each at 1440/1024/800 × 900 light+dark + the interaction goldens: Workload-tab,
Usage-tab, New-Rule-form, New-Type-form, Segments-tab + New-Campaign-form),
diff-locked twice, a11y check ON. Verification per commit: fmt clean; clippy -D
warnings (workspace + core sandbox); app-shell 234 + core 485 + core sandbox 493 +
xtask 11 + lint 2 exit 0; kittest all suites green across the fourteen files;
SPP_SMOKE exit 0. **Environment note (this sandbox, not a code event):** the root
filesystem re-hit 100% mid-verification (bus error on two kittest links) — the
9.9G volume cannot hold the whole suite's test binaries at once; suites were run
sequentially with the just-linked binary pruned between runs, everything green.
**Next in batch:** RS-20 wave D (Settings, Notifications, Search, SupportGraph,
Connectors, Diagnostics, SyncHealth, Onboarding, Shell) — the final R3 wave,
then the G2/G3 evidence packs at the batch STOP.

**RS-20 — R3 wave D (verified 2026-10-29):** all nine pages rebuilt onto their DC
archetypes, behavior byte-identical everywhere, a11y labels on every interactive
control, zero bespoke styling. **Notifications** (5ddb782): DS-C-09 rail, the
mark-all-read act as the lab's text button, the card toolbar's real unread state
dot (the lab's ds-udot — the '● '-inside-the-badge quirk receipt rides beneath
the list), the raw type badge, the relative time with the raw UTC in the hover
title (calendar-day semantics per the lab's relmap — days-from-civil on the ISO
date prefixes, no new dependency; the formatter is deterministic so the goldens
diff-lock without a wall-clock seam), the read/unread act pushed right, the
lab's empty-state copy. **Search** (37621c9): the toolbar keeps the HEAD
lost-focus + Enter semantics, the clear act as text, the empty state carries the
lab's Try-"export" CTA (the se-demo act over the real FTS engine), hit cards
adopt the lab's anatomy (scope badge + mono strong title + muted subtitle row,
the why badges on their own chip row, the snippet hint last). **SupportGraph**
(8d17105): the result count rides the header as the DS-C-01 counter pill; the
results are the lab's ds-sel-rows (full-width selectable rows, the live-chats
chat_row pattern — tokens only, ListItem a11y with the selected state); the
Neighborhood edges render as DS.row pairs (relation strong + far-side label
left, origin mono-muted right); a kittest-only seam (the operations freeze
precedent) prefills the search through the real reload. **Connectors**
(fae27fa): the count pill beside the New Connector primary; the health cell as
the lab's dot + badge pair; the rows/refresh/delete triple collapses to refresh
+ delete per the lab's A-5 receipt (the chip already opens the rows viewer — no
workflow step lost) with the receipt verbatim; the New Connector form binds
every input. **Diagnostics** (82206f3): the filter row is DS chips (raw
selectable_labels gone) with the refresh/clear acts pushed right; the evRows
adopt the lab's anatomy (small muted timestamp, the DS-C-06 level dot, the
service badge + message); the audit trail's actor/ai cells ride the AI badge
tone; the sync-state badges follow the lab's SYNC map. **SyncHealth** (7e68541):
the engine state + last success ride the new DS-C-24 hero band (design::hero —
the lab's verbatim forensic note + the SYNC-map status badge right; the register
B-2 closure wires the engine hero to DS-C-24; the widget suite grows 14→16);
the action toolbar splits (three run acts left, destructive cancel right).
**Onboarding** (aae5e84): the Setup status card renders the lab's DS.row anatomy
(state dot via the lab's okFlag + strong key left, mono value right) over the
same five keys. **Shell** (fc4efc9): the header act rides the DS-C-11 outline
button (the raw egui::Button gone), the ported-pages hint carries the lab's
stale-checklist tag verbatim; registered harness exception — the page is bound
to the live App shell state, verified by SPP_SMOKE's 24-page paint gate.
**Settings** (bc9d92a): the tab rail moves onto the DS-C-09 rail below the
header (same six labels, same state machine); the a11y-check-ON harness flagged
the Drag values + combos + eleven inputs — every one now carries its visible
label via Response::labelled_by (zero visual change); the danger zones adopt the
lab's card naming ('Danger zone — Workspace data' with the honest 'irreversible'
sub); the arms (item-11 restore, purge) and every query/write byte-identical.
**Goldens:** 9 new suites this wave (kittest_notifications_page 7,
kittest_search_page 7, kittest_support_graph_page 7, kittest_connectors_page 7,
kittest_diagnostics_page 7, kittest_sync_health_page 7,
kittest_onboarding_page 7, kittest_settings_page 7, + the widget suite's hero
pair) — every page at 1440/1024/800 × 900 light+dark + the interaction goldens
(Mentions-tab, Try-export, Neighborhood, New-Connector-form, errors-filter,
Run-action, Mark-complete, Help-Scout-tab), diff-locked twice, a11y check ON.
The fixed-timestamp seeding discipline (events/audit/checkpoints/runs) keeps
the diff-locked rule honest against the wall-clock writers. Verification per
commit: fmt clean; clippy -D warnings; app-shell 236 + core 485 + core sandbox
493 + xtask 11 + lint 2 exit 0; kittest all 23 suites green (152 tests);
SPP_SMOKE exit 0. **Environment note (this sandbox, not a code event):** the
9.9G root volume re-hit 100% repeatedly (bus errors on test links) — the suites
were run sequentially with just-linked binaries pruned between runs and the
GUI-stack rlibs dropped for the core-only runs, everything re-ran green. **CI
note:** the first seven wave-D pushes failed the CI Format check — the
mid-verification python seed patches landed after the last local fmt pass;
feee18a applies rustfmt (formatting only) and re-runs. **R3 is complete: every
page of the app (24 routes) now renders through the DC widget set.** Next at the
batch STOP: the G2/G3 evidence packs, then the owner gate.

## Batch RS-2 completion record (Rule 4)

All ten items verified and closed. Commit chain: f84d43c (RS-11) → fbf106c
(RS-12) → 8bb994f (RS-13) → b5e613d (RS-14) → 9718cd7 (RS-15) → d172cfc (RS-15)
→ 8490eee (CI fix) → 7001cf8 (RS-19a) → c6ecefd (RS-19b) → 643f671 (RS-19c) →
ecd0564 (RS-19d) → 2444a1f (RS-19e) → c4b7315 (wave C record) → 5ddb782
(RS-20a) → 37621c9 (RS-20b) → 8d17105 (RS-20c) → fae27fa (RS-20d) → 82206f3
(RS-20e) → 7e68541 (RS-20f) → aae5e84 (RS-20g) → fc4efc9 (RS-20h) → bc9d92a
(RS-20i) → feee18a (rustfmt) → 5c12fd1 (this record). All pushed to origin/main.

**Final wave-end verification (2026-10-29):** fmt clean; clippy -D warnings 0
(workspace + core sandbox); app-shell 236 + core 485 + core sandbox 493 + xtask
11 + style-lint 2, all green; kittest 23 suites / 152 tests green (every page
golden diff-locked twice, a11y check ON); SPP_SMOKE exit 0 (24 pages painted on
the fresh binary). CI: the Format-check drift (feee18a) fixed; the
AppImage+.deb+smoke job green on the wave commits.

**G2 evidence pack (DC tokens → code):** RS-12's 1:1 token port + the DC pin
tests (app-shell suite) are the token-conformance evidence — `crates/app-shell`
design.rs LIGHT/DARK against `docs/audit/design-contract-v1.md` §1, the
style-lint CI test enforcing the no-raw-color / on-scale rules (crates/app-shell
tests/style_lint.rs, 2 tests).
**G3 evidence pack (page rebuilds):** the 23 kittest suites under
`crates/app-shell/tests/` lock the REAL pages (not mocks) at 1440/1024/800 ×
900 light+dark over seeded mirrors — 100+ page goldens diff-locked twice with
the a11y check ON — plus the lab reference set `mockups/design-lab/goldens/v4/`
for the side-by-side review.

**R3 is complete: every page of the app renders through the DC widget set.**

**STOP (Rule 8):** Batch RS-2 is closed. The queue holds RS-21 (Batch RS-3 —
Stage 2 close: R4 final acceptance + release, Gate G4). Work resumes on the
owner's `continue`.

## Batch RS-3 completion record (Rule 4)

**RS-21 — R4: final acceptance + release (verified 2026-10-29, Gate G4 presented).**
All five RS-21 work streams landed, verified and pushed (commit chain:
`b086a56` opened → `432f742` matrix+gate+CI locks → `fc8b5ec` instruments →
`d828b9f` acceptance PDF → `1c9c506` v1.9.0 bump → `401fb9e` ICD runtime
resolution → `a7ecfd3` settings 32px registered budget + diff artifacts).

1. **Traceability matrix** — `docs/audit/acceptance/traceability-matrix.md`
   traces every screen region of all 23 `PageKey` routes + the shell chrome
   (DC §6) to its DS-C id and DC row, with the archetype (DC §4), the
   `design.rs` widget and the golden-lock suite per page. One registered alias
   (G-7): DS-C-04 = the MetricTile half of the DC's DS-C-03 sheet.
   `scripts/gate_traceability.py` machine-checks the matrix against the frozen
   DC (id set + alias declaration), the route list and the golden-lock files;
   negative-tested before landing — a bogus id, a missing route section, a dead
   golden lock and an undeclared alias each FAIL the gate (clean matrix exits 0).
2. **CI snapshot locks** — the new `snapshots` job renders all 23 kittest
   suites (152 tests / 168 goldens) on the runner through Mesa lavapipe with
   UPDATE_SNAPSHOTS unset. Two runner fixes were needed and are recorded:
   the ICD filename is resolved at runtime (`lvp_icd.json` on the current
   image, not the legacy `lvp_icd.x86_64.json`), and the settings page goldens
   carry a registered 32-px failed-pixel budget — the runner's mesa 26.0.8
   anti-aliases 28 of ~1.3M pixels differently on the General tab (the only
   Drag-bearing page); the other 22 suites are byte-exact on the runner, the
   Help-Scout interaction golden keeps the zero-tolerance lock, and the job
   uploads `*.diff.png` artifacts on failure for diagnosis. Result: **the
   drift gate is green on the runner** (run 40943289183: check + packages +
   snapshots all success) — a visual regression on any rebuilt page now fails
   CI instead of shipping silently.
3. **Blind test + SUS** — `docs/audit/acceptance/blind-test-protocol.md` (the
   DS-C-22 5-second AI-identification test: ten stimuli cropped from the
   checked-in goldens, pass bar ≥9/10 with zero false-AI) and
   `docs/audit/acceptance/sus-questionnaire.md` (standard Brooke-1996 SUS,
   target ≥ 68) ship as the G4 instruments; the human administrations and the
   result tables execute at Gate G4 (the mechanism pre-check they exercise is
   enforced by the typed widget path + the single §5 tone map + the goldens).
4. **Acceptance records folded into the printable report series** (the
   deferred PDF task) — `docs/audit/generate_acceptance_report.py` (Template 07
   Crystal Blue, series-consistent) emits
   `docs/audit/SupportOS-Plus-Reskin-Acceptance-Report.pdf` (report 4 of 4):
   program record + gate receipts, traceability summary, blind test, SUS, CI
   locks, release v1.9.0 (the CHANGELOG section read verbatim at build time)
   and the G4 record table for the owner's verdict.
5. **Release v1.9.0** — workspace version bumped 1.8.3 → 1.9.0; Cargo.lock
   refreshed (textual diff verified: only the four member version lines, zero
   dependency drift); the CHANGELOG `[1.9.0]` section carries the release
   notes and the release workflow composes the GitHub release notes from it.

**Final verification (2026-10-29):** gates clean locally (no-simulation,
traceability); CI run 40943289183 on `a7ecfd3` — fmt/clippy/test/deny ✓,
AppImage + .deb + smoke ✓, kittest snapshot locks ✓ (23 suites / 152 tests /
168 goldens diff-locked on the runner, a11y check ON). The traceability gate
also rides the release workflow, so a tagged commit cannot regress it.

**GATE G4 — owner final acceptance:** the package (this record, the
traceability matrix, the blind test, the SUS instrument, the acceptance PDF,
release v1.9.0) is presented with the batch STOP. The owner's verdict, the
blind-test aggregate and the SUS score fill the record tables at the gate.

### G4 pre-acceptance addendum — 2026-10-29 (owner-directed doc correction)

During the owner's step-by-step verification request, the kittest lock-set count in the
living documents was found stale: the records above (and the R4 artifacts derived from
them) said **23 suites / 152 tests / 168 goldens**, while the machine-checkable facts are
**23 suites (22 kittest + style lint) / 172 tests (170 kittest + 2 style lint) / 168
goldens** — CI's snapshot-lock log totals 408 passed = 236 lib + 170 kittest + 2 style
lint, the local re-run matched it suite-by-suite, and `ls crates/app-shell/tests/snapshots`
= 168. The 152 figure was a mid-wave tally that never caught up with the wave-D/RS-21
suite growth. Corrected (doc-only, zero code change): `traceability-matrix.md` §Coverage
reconciliation, the `CHANGELOG.md` [1.9.0] drift-lock bullet, the `ci.yml` snapshots-job
comment, and `generate_acceptance_report.py` (PDF regenerated, stale-token sweep = NONE);
the historical batch records above keep their original text per append-only discipline —
this addendum is their correction. `gate_traceability.py` re-run: OK (23 routes, 27 ids,
alias declared). The full local re-verification that surfaced this: fmt clean; clippy -D
warnings 0; core 485 + app-shell 236 green; all 22 kittest suites re-run locally over
lavapipe+Xvfb — 170/170 green, a11y check ON; SPP_SMOKE exit 0 (24 pages, 4225 shapes);
no-simulation gate clean. G4 remains open: the owner's verdict, blind-test aggregate and
SUS score fill the record tables at the gate.

### G4 verdict — 2026-10-30 (owner): NOT ACCEPTED — the reskin fix wave reopens

The owner rejected at Gate G4: *"you need to fix the reskin there are a lot of
issues, i am not telling it is done yet."* Evidence: a live screenshot of
v1.9.0 running at ~2560×1440 (downscaled capture) with the inbox's copilot
panel open — the reader's cards and the composer paint **over** the copilot
panel, and the copilot's Summary card leaks raw markup (`<br>`, `<p>`) and
overflows its card. This addendum is the honest record of that verdict; the
blind test and SUS were not administered (the owner's verdict precedes them).

**Root cause (established by local reproduction, not by inspection alone):**
temporary kittest probes (`crates/app-shell/tests/zz_g4_reskin_probe.rs`,
NOT part of the lock set) rendered the real inbox page with the copilot open
at the owner's aspect (1625×828) and at the tested 1440×900 — both reproduce
the overlap, so the defect is state-dependent, not width-dependent. Minimal
panel probes (bare `Panel::left+left+right+bottom+CentralPanel`, root-level
and shell-nested, with and without `design::install`) all reserve correctly —
egui 0.37's panel reservation is NOT the defect. The overflow comes from the
reader's ScrollArea content: thread bodies render long unbreakable tokens
(the note card's URL) that egui will not break by default (`break_anywhere`
is false), the content's min width exceeds the reserved viewport, the scroll
content widens, and every message card (which sizes itself to the scroll
content's available width) paints past the viewport — over the panel that was
drawn earlier. The composer overflows the same way. Secondary findings:
the copilot summary/questions are built from the RAW thread body (the reader
strips HTML through `richtext::html_to_spans`, the analyzer does not), the
"Ctrl+Enter sends the composer" hint sits in the views rail instead of the
composer, blank-name customer rows render a headerless row (no email
fallback), and the Cc/Bcc inputs keep a fixed width inconsistent with the
composer.

**Coverage gap identified:** the copilot-open full-page state was never
golden-locked — `r3_inbox_page_*` lock the copilot CLOSED and the identity
goldens lock the pane widgets at 720 px (the blind-test stimuli were cropped
from those), so no lock in the 23-suite set ever exercised the failing state.

**Disposition:** plan.md is refilled with the fix wave **RS-4-1..RS-4-10**
(owner-directed reopen; the RS- numbering continues the reskin program).
Batch RS-4-1 (all ten items) moved to `progress.md` per Rule 2. G4 remains
open; it will be re-presented after the fix wave verifies.

## Batch RS-4 completion record (Rule 4) — G4-rejection fix wave (RS-4-1..RS-4-10)

Completed and verified 2026-10-30. Source: the owner's G4 rejection
(*"you need to fix the reskin there are a lot of issues"*, live screenshot of
v1.9.0 with the copilot panel open). All ten items landed; the wave root-caused
and fixed the reported defects plus the class behind them.

**Root cause (final):** egui lays widgets inside `ui.horizontal` out in
**Extend mode — text never wraps there**. The copilot pane's suggested-reply
card (`design::cop_suggest`) renders its bullets in a horizontal row; the
369-char multi-line reply laid itself out at full single-line width
(min_rect → x≈1882), the frame's response rect blew past the window edge,
and panel.rs's size guard re-clamped the 320 px panel anchored at that
blown-out right edge — the panel rendered mostly off-window and reserved
only **42.5 px** of its 320. The reader, composer and every "overlap" in the
owner's screenshot were downstream consequences: they correctly filled the
wrongly-reserved remainder. Proven with pixel-run measurements, temporary
rect instrumentation, and an instrumented egui build (reverted after the
diagnosis).

1. **RS-4-1** `core/richtext`: `strip_html` (sanitizer+parser pipeline →
   plain text) and `break_long_tokens` (U+200B every ≤40 chars of a token,
   UAX #14 break opportunity) — 7 unit tests.
2. **RS-4-2** `core/ai_local`: the analyzer consumes `strip_html`'d thread
   bodies (summary/questions/sentiment inputs) — the owner's live-data
   `<br>`/`<p>` leak cannot recur; unit test on the exact live shape.
3. **RS-4-3** `inbox.rs render_rich_spans`: span text passes through
   `break_long_tokens` (wrap contexts only — horizontal rows need the
   explicit wrap/truncate of RS-4-4).
4. **RS-4-4** the root cause: `cop_suggest` bullets render with an explicit
   wrapping `Label` + token breaking; the similar-issues row (also
   horizontal) truncates. This restores the panel's 320 px reservation.
5. **RS-4-5** composer: Cc/Bcc inputs span the composer width (were fixed
   half-width slots).
6. **RS-4-6** "Ctrl+Enter sends" hint moved from the views rail to the
   composer foot next to the send button (truncate-safe).
7. **RS-4-7** blank-name customers: both list and detail queries fall back
   to the email, then `customer` (the headerless Judy row); unit test
   covers blank-name+email, blank+blank, and real-name cases.
8. **RS-4-8** new permanent suite `kittest_inbox_copilot.rs` (3 tests) locks
   the never-before-locked copilot-open state class at the owner's live
   aspect (1625×828, light + dark) plus copilot-closed; temporary probe
   file and its snapshots deleted; suite registered in the traceability
   matrix.
9. **RS-4-9** verification (all green):
   - fmt clean; `clippy --workspace --all-targets -- -D warnings`: 0.
   - core: **494 passed / 0 failed** (4 ignored = LM Studio live; +9 RS-4
     tests over the 485 baseline). app-shell lib: **236 / 236**.
   - kittest: **24 suites / 175 tests (173 kittest + 2 style lint) /
     171 goldens**, a11y ON, over lavapipe+Xvfb; diff-locked-twice on every
     regenerated suite (clean re-run byte-identical).
   - Golden changes, all explained: NEW r4_inbox_copilot_{open light,dark,
     closed light}; r3_inbox_page_* ×6 (hint moved + Cc/Bcc width);
     r2c_composer_{light,dark} (composer foot hint). All other 158 goldens
     byte-identical, including every G4-kit stimulus source
     (r2c_inbox_identity_*), so the kit needs no rebuild.
   - Pre-existing defect discovered and contained (not silently absorbed):
     the dashboard chart subtitle and the reports builder date inputs are
     wall-clock-dependent — their goldens drift ~70–85 px at every midnight
     (byte-exact on 10-29, drifting on 10-30). Registered
     `max_failed_pixels(120)` budgets with full disclosure in both suite
     files (the settings-page runner-budget precedent); the proper clock
     seam is **RS-4-11**, queued in plan.md.
   - `gate_traceability.py` OK (23 routes, 27 ids, alias declared);
     `gate_no_simulation.py` clean; SPP_SMOKE exit 0 (24 pages, 4224
     shapes — one fewer than the 4225 baseline: the removed views-rail hint
     shape).
10. **RS-4-10** docs synced (stale-token sweep NONE): traceability matrix
    (counts + the copilot row now names both suites), CHANGELOG [1.9.0]
    drift-lock bullet, ci.yml snapshots comment, acceptance report
    regenerated (8 pages, "24 / 175 / 171" present, zero stale tokens).

**Historical note (append-only discipline):** the two "pre-existing" counts
corrected in the G4 pre-acceptance addendum (23/172/168 → documented as the
then-current lock set) describe the pre-wave state; this record's 24/175/171
is the post-wave set. The G4 blind-test/SUS record tables remain unfilled —
the owner's verdict re-opens G4; it will be re-presented with the STOP.

---

## Batch RS-5 — RS-4-11 (dashboard/reports determinism seam) — COMPLETE 2026-10-30

Opened on the owner's `continue` after the RS-4 STOP; executed per rule.md
(Rule 2 move → implementation → verification → this record). One item.

1. **RS-4-11** — the dashboard chart subtitle and the reports builder date
   inputs derived their 30-day window from the wall clock, so their goldens
   drifted ~70–85 px at every UTC midnight (held open since the RS-4
   verification by registered `max_failed_pixels(120)` budgets).

   **Fix (the clock seam, per the repo's fixed-timestamp doctrine):**
   - `spp_core::business_hours` gains the anchored variants
     `iso_days_ago_at(days, now)` / `now_iso_millis_at(now)`; the wall-clock
     fns delegate to them (one implementation, zero behavior change;
     unit-tested in core: `anchored_variants_match_the_wall_clock_shape`).
   - `DashboardScope` and `ReportsPage` gain `today: Option<DateTime<Utc>>`
     (production `None` = wall clock) consumed by `load_dashboard`,
     `ReportsPage::reload`, the builder's fetch-HS-report window, and
     `ReportsPage::pin_today` (re-seeds the builder date inputs; the page
     methods are `pin_today`, tests pin a FIXED anchor 2026-01-15T12:00Z).
   - **Second clock found and removed during verification:** the 7d-stale
     backlog predicate read the DB engine's `julianday('now')`
     (dashboard.rs, counts + mailbox-comparison queries) — a hidden wall
     clock the plan text did not name. It now reads the anchored window
     (`julianday(?2)`), so pinning the scope pins every clock the page
     reads. Production semantics unchanged (`?2` = the formatted wall
     clock). Regression test
     `backlog_staleness_derives_from_the_pinned_anchor` (pinned scope:
     8d-stale counts, 3d-fresh does not; wall-clock scope: both count).
   - Both kittest suites pin the fixture AND the page to the same fixed
     anchor (`anchor()`); `stamp()` derives seeds from it. All rendered
     date text (chart subtitle, builder inputs, saved-row timestamp) is
     constant forever.
   - **The `max_failed_pixels(120)` budgets are deleted** — both page
     suites lock byte-exact with zero tolerance.

   **Verification:** fmt clean; clippy `--workspace --all-targets
   -D warnings` 0; core 495/495 (+1 anchored-clock test); app-shell lib
   237/237 (+1 backlog-seam regression test); kittest 24 suites / 175
   tests / 171 goldens over lavapipe+Xvfb (sandbox rebuilt after an
   environment reset: Rust 1.99.1 + mesa lavapipe user-prefix + Xvfb :99),
   the two touched suites diff-locked-twice byte-exact with NO budget.
   Golden changes all explained by pixel-diff against HEAD: 7 dashboard
   goldens = the chart-subtitle date glyphs only (113×8 px strip);
   1 reports builder golden = the two date inputs + the saved-report row's
   seeded timestamp date glyphs; the six `r3_reports_page_*` goldens are
   byte-identical to HEAD (they render no date text — the sole divergence
   found mid-verification, the backlog tile flipping 1→3, was the hidden
   `julianday('now')` clock and is fixed at the source). SPP_SMOKE exit 0;
   gate_traceability OK (23 routes, 27 ids); gate_no_simulation clean.
   Docs synced: CHANGELOG [1.9.0] drift-lock bullet now discloses the seam
   and the budget removal; acceptance report regenerated (8 pages, RS-4-11
   present, `max_failed_pixels` absent); traceability matrix and ci.yml
   needed no change (counts unchanged, no budget text there).

---

## Gate G4 closure — owner verdict: accepted — 2026-10-30

Opened on the owner's message `blind test + SUS / accepted / continue`: the
owner chose the test path (Path A) and gave the final verdict **accepted** in
the same breath. The chat-fallback answer lines delivered earlier were held
unfilled per the no-simulation gate until exactly this authorization; they are
now transcribed verbatim into the record tables. No number below is invented —
every score is computed from the owner's delivered answers against the
checked-in keys.

1. **Blind test (RS-21, DS-C-22)** — administration: chat fallback (the owner
   viewed `g4-kit/g4-blind-test-slides.pdf`; deviation recorded, not dropped:
   the 5-second flash is not enforceable in chat). Answers transcribed:
   `1Y 2N 3N 4Y 5N 6Y 7N 8N 9Y 10N`. Scored against the administrator key
   (seeded shuffle): **5/10 correct, false-AI 2 of 5** — the pre-registered
   bar (≥ 9/10 AND zero false-AI) **NOT met**. Correct: cards 1, 5, 6, 7, 10.
   Misses on AI stimuli: cards 2, 3, 8 (AI draft cards light + dark, dark
   copilot panel read as human). False-AI: cards 4, 9 (the amber internal-note
   card and the Ada Lovelace agent reply in page context read as AI). The
   failure-action surfaces are named in the protocol record and stand as
   disclosed, owner-accepted limitations — the verdict was given with the
   score in view and supersedes the fix-and-rerun loop for this program.

2. **SUS (RS-21, target ≥ 68)** — chat fallback, transcribed:
   `1=4 2=2 3=5 4=1 5=4 6=1 7=4 8=1 9=4 10=2`. Odd (4−1)+(5−1)+(4−1)+(4−1)+(4−1)=16 ·
   even (5−2)+(5−1)+(5−1)+(5−1)+(5−2)=18 · sum 34 × 2.5 = **85.0** — bar met.
   Caveat recorded verbatim: n=1 (the owner is the program's only user); not a
   population estimate.

3. **Records synced (zero-stale):**
   - `docs/audit/acceptance/blind-test-protocol.md` §Result — filled
     (per-card detail, administration deviation, the NOT-met bar, the verdict).
   - `docs/audit/acceptance/sus-questionnaire.md` §Score sheet — filled
     (ten per-item values, scoring arithmetic, 85.0, n=1 caveat).
   - `docs/audit/generate_acceptance_report.py` — the four pending/awaiting
     placeholders replaced with the closed record (cover Status row, §4.2,
     §5, §8); acceptance report regenerated: 8 pages, every value verified in
     the rendered text, zero stale strings (`awaiting G4` / `pending G4` /
     `— (n = —)` all gone).
   - `ai/plan.md` — G4-closed blockquote appended; `ai/progress.md` — None.
   - Traceability matrix and CHANGELOG needed no change (no counts, no pending
     language; the matrix's G4 mention is historical and remains true).

4. **Gates:** gate_no_simulation clean; gate_traceability OK (23 routes,
   27 ids). No Rust code changed in this closure — the 24-suite / 175-test /
   171-golden set, SPP_SMOKE and the CI snapshot locks are untouched from the
   RS-5 verified state (commit `0ad0bf1`).

**The reskin program is closed:** RS-01..RS-21, the RS-4 rejection wave
(RS-4-1..10), RS-4-11, and G4 (verdict: accepted). `plan.md` holds nothing;
`progress.md` holds nothing. No further work is open or queued.

---

## Batch RS-6 — release v1.9.1 (the accepted state) — COMPLETE 2026-10-30

Opened on the owner's `release the new version with all the fix applied`;
executed per rule.md (plan entry → progress move → this record). The
established fact that shaped the batch: tag `v1.9.0` was cut AND pushed at the
RS-3 STOP (`464000d`, remote confirmed via `git ls-remote`) BEFORE the owner's
G4 rejection produced the RS-4 fix wave — the fixes live only on main. The
accepted state therefore ships as the **patch release v1.9.1**; re-pointing or
re-tagging v1.9.0 was never an option (it exists on the remote).

1. **RS-6-1 (workspace)** — root `Cargo.toml` version 1.9.0 → **1.9.1**
   (members inherit `version.workspace = true`); `Cargo.lock` synced via
   `cargo metadata` (toolchain 1.99.1) — all three workspace packages at
   1.9.1, zero `1.9.0` refs left in the lock. Required: the Release workflow
   runs every cargo step `--locked`, and its dist-staging step reads the
   version from the root `Cargo.toml` for the artifact names and BUILD-INFO.

2. **RS-6-2 (CHANGELOG)** — new `## [1.9.1] — 2026-10-30` section:
   `### Fixed — the Gate-G4 rejection fix wave (RS-4-1..RS-4-11), on the
   accepted state`. Self-contained for the release page (the Release workflow
   composes the GitHub release notes from exactly this section): the copilot
   42.5 px-sliver root cause (egui EXTEND-mode horizontal layout → blown
   frame rect → re-clamped 320 px panel) and the wrap fix, HTML-fragment
   leakage + strip_html, Cc/Bcc width, the Ctrl+Enter hint move, the
   blank-name fallback, the RS-4-11 clock seam + budget removal, and the new
   `kittest_inbox_copilot` lock. Precision fix applied during drafting: the
   snapshot gate does NOT run inside the Release workflow (the kittest suites
   are `kittest-ui`-feature-gated, off by default; ci.yml's `snapshots` job
   triggers on main pushes, not tags) — the correct statement, now in both
   the CHANGELOG and the report §6: the tag is cut on the main commit the
   snapshots job already diffed; the Release workflow re-runs fmt, clippy and
   the workspace tests before publishing. (The pre-existing report §6 claim
   "the same gate rides the release workflow" was corrected in RS-6-3 for the
   same reason.)

3. **RS-6-3 (acceptance report)** — generator synced: cover subtitle "R4
   Final Acceptance + Release v1.9.1", cover Status row names v1.9.1 and the
   v1.9.0 supersession, §6 sentence corrected (above), §7 rewritten
   (v1.9.0 tagged pre-fix-wave → accepted state ships as v1.9.1; annotated
   tag cut from the G4-accepted state), §7.1 prints the `[1.9.1]` section
   verbatim (regex switched), §8 record names v1.9.1. Regenerated: 8 pages;
   probes confirm the new content (Release v1.9.1, [1.9.1] section,
   42.5 px sliver, kittest_inbox_copilot, released-as-v1.9.1, tagged-commit
   mechanism); stale strings gone (Release v1.9.0, the-[1.9.0]-section,
   The-reskin-ships-as, tagged-and-published-with-checksums).

4. **RS-6-4 (gates/records)** — gate_no_simulation clean; gate_traceability
   OK (23 routes, 27 ids). No Rust source changed (version metadata only, no
   behavior) — the 24-suite / 175-test / 171-golden set stands as verified in
   RS-5; the Release workflow re-runs fmt/clippy/tests on the tagged commit
   and the snapshots job covers it via the main push.

5. **RS-6-5 (tag/push)** — annotated tag `v1.9.1` created on the release
   commit (tagger identity matching v1.9.0: kimpearce888). **Push pending the
   credential re-supply**: the sandbox rebuild (2026-10-30 00:35) lost
   `~/.git-credentials` (searched: dotfiles, shell history, /tmp, project
   dirs, env, ssh — no copy). `git push` fails with "could not read Username"
   until the owner re-supplies the token; rs5 `0ad0bf1`, the G4 closure
   `157f0ef` and this release commit + tag all go out in the same push, and
   the tag push triggers the Release workflow (build → gates → .deb verify on
   ubuntu 22.04/24.04 + debian 12 → AppImage verify → publish with the [1.9.1]
   notes + checksums).

## Batch RF-1 — recovery baseline — COMPLETE 2026-10-31

Reconstructed program, decisions.md D7. The sandbox reset destroyed the
original RF-1..RF-5 local commits and ai records; recovery re-cloned the
v1.9.1 tree (`acd557a`) with the owner's re-supplied credentials and
re-verified the full baseline gate table on it: fmt clean; check clean
(2m59s cold); clippy -D warnings 0; gate_traceability OK (23 routes / 20
DC tags / 27 ids); gate_no_simulation clean; core lib 495 passed / 4
ignored / 0 failed; app-shell lib 237 / 0; xtask 11 / 0; catalog lib 0/0;
doc-tests 0/0; verify-icons clean; kittest 24 suites re-confirmed as the
CI lane (standing disclosed gap in this sandbox). Baseline locked as the
pre-refactor gate table; plan.md RF section reconstructed with fresh
file:line evidence; pushes owner-authorized.

## Batch RF-2 — core dead code, wave 1 — COMPLETE 2026-10-31

Evidence: symbol census over all four crates (identifier frequency incl.
string dispatch, doc mentions, tests; 163 zero-reference pub items in core,
0 in catalog) + per-item spot verification before deletion + clippy-driven
orphan sweep after (never-used privates, never-constructed structs, orphaned
impl blocks, dead imports). Deletions (line counts before→after):
RF-2-1 intelligence_features 1761→1116 (21 dead fns: the vestigial
radar/incident/issue API layer — the live pages use only
add_incident_note / create_manual_incident / link_incident_conversation /
refresh_known_issue_counts / count_stale_docs / list_knowledge_gaps /
record_knowledge_gap / count_sla_breaches / get_sla_config / set_sla_config
/ sweep_unmonitored_sla / sweep / update_doc_freshness / apply_m*);
RF-2-2 outreach 2935→2919 (16: dead segment/campaign API — live send path
lives in workers); RF-2-3 side_threads 985→181 (14 + orphans: dead
side-thread API); RF-2-4 automation 2366→1595 (12 + orphans); RF-2-5
conversation_ops (9 + orphans); RF-2-6 ticket_states 688→349 (8 + orphaned
OrderedF64 impls); RF-2-7 saved_views 2618→2376 (7); RF-2-8 embeddings
(5); RF-2-9 search 1813→1659 (5 index_*_fts) — escape_like retained for
RF-4; RF-2-10 jobs (5). NO-TOUCH respected: response_state_sql.rs,
ai_center.rs untouched. Verification: fmt clean; clippy --workspace
--all-targets -D warnings 0; core lib 495 passed / 4 ignored / 0 failed
(--test-threads=1); app-shell lib 237/0; gate_traceability OK (23 routes /
20 DC tags / 27 ids); gate_no_simulation clean; verify-icons clean.
Kittest 24 suites: standing CI-lane gap (disclosed). Net: 11 files,
-3,761/+25 lines.

## Batch RF-3 — core dead code wave 2 + app-shell — COMPLETE 2026-10-31

The second-order orphan wave (RF-2's root deletions cascaded 70 more
zero-reference pub items) plus the census tail, processed in ten items per
the progress.md selection: custom_objects 5, quality 4 (+ the DerivedOutcome
field trim its deletion orphaned — segment::derive_outcome consumer kept, six
never-read fields dropped), support_graph 4 (+2 cascaded), eventing 4,
knowledge_store 4, ai_attributes/interaction_engine/people_store 3+3+3,
webhook/segment/ticket_states 3+3+3 (ticket_states' new orphans incl.
get_state_history group), saved_views/notification_prefs/encrypted_sync
2+2+2, the singles wave (20 one-item files: ai_pipeline process_new_ticket,
audit, automation trigger_matches, business_hours, coaching
ensure_coaching_schema, config ensure_dir, copilot delete_session,
customer_events, customers, embeddings decode_f32_le, incident_workspace,
intelligence_features incident_row_json, interaction_current, issue_impact,
notifications, operations, outreach, richtext, translation, workload
capacity_metrics), and app-shell chart.rs bar_chart. NO-TOUCH held:
response_state_sql ×3, ai_center ×1, db open_with_migrations — the only
census items allowed to remain (convergence proof). Incident + recovery: two
deletion spans overran into adjacent #[cfg(test)] modules (saved_views
batch31_tests, segment batch30_tests — the batch 30/31 regression tests);
caught by the #[test]-count census (507→504), both modules restored
byte-identical from acd557a, test count re-verified at the baseline.
Verification: fmt clean; clippy --workspace --all-targets -D warnings 0;
core lib 495 passed / 4 ignored / 0 failed; app-shell lib 237/0;
gate_traceability OK (23/20/27); gate_no_simulation clean; verify-icons
clean. Working diff vs pre-batch: 36 files, −4,062/+30 lines (+ the restored
test modules). Kittest: standing CI-lane gap.

## Batch RF-4 — unifications — COMPLETE 2026-10-31

Ten items per progress.md: RF-4-1 escape_like 5→1 (canonical search.rs,
byte-equivalent bodies, all external callers already canonical);
RF-4-2 add_column_if_missing 9→2 (canonical migrations.rs pub(crate) +
maintenance.rs NO-TOUCH local; outreach's nested copy removed; semantics:
missing table = no-op, missing column on existing table = ALTER with error
propagation — replacing the two silent `let _ =` copies); RF-4-3
table_exists 5→2 (canonical sync_schema::table_exists; search's
tables-or-views variant intentional, documented); RF-4-4 now_iso 7→4 (the
two byte-identical-output idioms collapse onto business_hours::now_iso;
outreach + sync_engine semantic variants documented in place); RF-4-5 hex
3→1 (crypto::hex); RF-4-6 is_closing_acknowledgment 2→1 (quality's
OnceLock-cached version); RF-4-7 is_valid_timezone → pub use re-export;
RF-4-8 round2 2→1; RF-4-9 five families dispositioned as INTENTIONAL
VARIANTS with the semantic differences recorded (decode_entity,
chunk_text, truncate_chars, is_word_char, table_columns); RF-4-10
verification. Incident: the first propagation change failed 294 tests
(BootStep 8 → ensure_main_rule_columns → apply_m035 widens `incidents`
before its creating step runs) — fixed by the missing-table no-op
semantic; the primitive is now strictly stronger than either historical
copy. Verification: fmt clean; clippy --workspace --all-targets -D
warnings 0; core lib 495 passed / 4 ignored / 0 failed; app-shell lib
237/0; gate_traceability OK (23/20/27); gate_no_simulation clean;
verify-icons clean. Diff: 27 files, +72/−252 (net −180). Kittest:
standing CI-lane gap.

## Batch RF-5 — migration module relocation (R100 pattern) — COMPLETE 2026-10-31

Census-derived selection: exactly 8 modules whose ENTIRE pub surface is
migration fns — ai_analysis, ai_features, data_tools, db_breadth,
intelligence (NO-TOUCH module: relocated byte-identically, zero internal
edits, per the standing constraint), mirror_parity, mirror_tables,
side_threads. Execution: `git mv` each into `crates/core/src/migrations/`
(R100: byte-identical, rename detection intact), migrations.rs gains the 8
`pub mod` declarations, lib.rs replaces the 8 `pub mod` lines with one
`pub use migrations::{…}` block. Zero call-site edits proven by grep counts
(154 crate::X::apply_ / 82 spp_core::bootstrap::apply_all callers / 20
spp_core moved-module refs — before = after). Verified: fmt clean; clippy
--workspace --all-targets -D warnings 0; core lib 495 passed / 4 ignored /
0 failed (the it_sync install_master_key quartet included — 10/10 it_sync
tests); app-shell lib 237/0; gate_traceability OK (23/20/27);
gate_no_simulation clean; verify-icons clean. Kittest: standing CI-lane gap.

## Batch RF-6 — god-file split, part 1 — COMPLETE 2026-11-01

Re-census selection on the post-RF-5 tree (the plan's evidence census
predates RF-2..RF-5): saved_views.rs measured 1568 ln after the RF-2/RF-3
deletions and no longer qualifies as a god-file — its split item dissolved
with the size (dispositioned at open, not skipped); the tenth slot went to
interaction_engine.rs (2142 ln). app-shell pages stay reserved for RF-7 per
plan.md. Execution: ten cohesive regions (9,089 lines) extracted
byte-identically into ten sibling modules under directory-converted parents
(`git mv X.rs X/mod.rs` per file; child files `use super::*;` and inherit
the parent's items and import bindings; moved impl slices keep their
`impl` headers — multi-impl blocks per type across child modules;
`pub use`/`pub(crate) use` re-exports at every old home keep the public
paths resolving; lib.rs untouched; tests stayed in place; zero call-site
edits outside the ten split modules — proven by the full-workspace
check/clippy compiling with only the split-module diffs). Items:
RF-6-1 workers.rs 5276→2639 (`workers/execute.rs` 2641: execute_job
dispatcher, on_after_initial_sync, on_conversation_changed, run_engine, the
21 run_*_job runners, run_attachment_downloads + the family-internal AiJob
enum); RF-6-2 ai_pipeline.rs 3761→3052 (`ai_pipeline/interaction.rs` 715:
build_interaction_strategy_block, extract_provided_facts,
has_version_number, ProcessNewTicketResult, AnalyzeInteractionOutcome,
ObservationOutput, parse_observation, parse_recommendation,
analyze_interaction); RF-6-3 helpscout_real.rs 3696→2991
(`helpscout_real/http.rs` 710: docs_request_inner, docs_request_send,
access_token, client_credentials_login_if_configured, request_inner — the
slice also carried the small mappers sitting inside the request span
(map_user/mailbox/folder/field/tag, doc_article_from_value);
map_conversation/customer/thread stayed); RF-6-4 sync_engine.rs
3115→1973 (`sync_engine/loops.rs` 1151: finish_resource, reconcile,
sync_resource, sync_customers, sync_conversations, ingest_conversation);
RF-6-5 outreach.rs 2861→2316 (`outreach/send.rs` 549: pick_batch,
classify_error, record_outcome, datetime_now, ensure_recipient_updated_at,
send_batch, BatchCounts, count_remaining_for_send, finalize_if_drained);
RF-6-6 design.rs 2814→2646 (`design/controls.rs` 177: chip_impl, card,
card_titled, section_title, hint_text, pager — the DC-tag-free span between
the tag clusters; the 20 DC doc-tags never moved and the gate figure is
unchanged); RF-6-7 segment.rs 2697→1312 (`segment/eval.rs` 1390:
tickets_for_customer_matching + the eval_* tree through eval_customer_event,
incl. the sub-40-ln eval_history_tag/eval_incident_exposure); RF-6-8
reports.rs 2640→2175 (`reports/config.rs` 467, two slices: config_to_json +
parse_report_config/enum_error/metric_variants/dimension_variants/
parse_report_config_inner/parse_report_filters; the saved-report CRUD
stayed between them); RF-6-9 quality.rs 2536→2195 (`quality/qa_ai.rs` 345:
coerce_ai_json, compute_qa_ai_layer, finish_qa_ai_layer, round1,
DerivedOutcome, derive_outcome); RF-6-10 interaction_engine.rs 2142→1205
(`interaction_engine/baseline.rs` 944: closed_conversation_ids,
rebuild_baseline, comparison_baseline, compute_changes, ThreadRow,
outcome_summary, heuristic_recommendation, detect_repeat_issue,
ensure_history_backfill, build_timeline, build_profile, build_playbook,
compute_outcome). Visibility: 37 compiler-demanded `pub(super)` bumps on
moved private items called from retained parent code (7 workers, 9
helpscout, 5 sync_engine, 14 segment, 1 outreach, 1 design) — bodies
untouched; plus two name-binding imports for unqualified retained calls
(outreach→send::ensure_recipient_updated_at,
design→controls::chip_impl). Disclosures: (a) gate_traceability.py's
DESIGN path constant followed the rename (design.rs → design/mod.rs) — a
one-line verification-infra fix; gate output figures unchanged (23 routes /
20 DC tags / 27 ids); (b) two zero-reference pub items found by the split
and preserved byte-identically, NOT deleted in a split batch:
ai_pipeline::ProcessNewTicketResult and
interaction_engine::AiEnrichment (re-exported to keep the pub path —
candidates for a future dead-code census); (c) incident at RF-6-2: the
first cut swallowed the `#[cfg(test)]` line (census end-line off by a
blank line) — caught by the compiler, restored, and every subsequent
boundary was read literally before cutting. Verification: fmt clean;
clippy --workspace --all-targets -D warnings 0; gate_traceability OK
(23/20/27); gate_no_simulation clean; verify-icons clean; core lib 495
passed / 4 ignored / 0 failed (--test-threads=1, 351.92s); app-shell lib
237/0; xtask 11/0; doc-tests 0/0. Diff: 21 files (+9,126/−9,071 net +55 —
the split is structural, not a deletion wave). Kittest: standing CI-lane
gap.

RF-6 CI hotfix (2026-11-01): the RF-6 push failed CI (run 41260762329,
both the workspace-test and kittest jobs) on `style_lint`
`no_raw_colors_and_no_off_scale_spacing` — the token-factory exemption was
keyed to the `design.rs` filename and RF-6's directory conversion
(design.rs → design/mod.rs + controls.rs) broke it, flagging the factory's
own raw colors in the split files. Fixed as a verification-infra
path-follow (filename suffix → `src/design/` directory prefix,
separator-normalized, doc comment updated); zero Rust source bytes
changed. Disclosed: RF-6's local gate table omitted `--test style_lint`
(process miss — the gate that would have caught this). Disclosed (good
news): all 24 kittest snapshot suites PASSED in CI on this run — the
standing CI-lane gap is closed green. Verified: style_lint 2/0, fmt clean,
clippy --workspace --all-targets -D warnings 0, gate_traceability OK
(23/20/27), gate_no_simulation clean, verify-icons clean, xtask tests 0/0.
Diff: 1 file (crates/app-shell/tests/style_lint.rs, +6/−3).

## Batch RF-7 — god-file split, part 2 + pages/queries.rs (2026-11-01)

Ten items completed per progress.md selection: the second census wave plus
the app-shell relocation, all as byte-identical contiguous-region
extractions under the RF-6 pattern (git mv X.rs X/mod.rs, child opens with
`use super::*`, re-exports at the old home, compiler-demanded visibility
bumps only, zero call-site edits outside the split modules):

1. **RF-7-1 ai_attributes.rs 2069→1148** — the AI extraction engine (929
   ln: prompt version/system, CustomerMessage, prepare/finish_computation,
   run_extraction/compute/compute_with_backend — three `pub async fn`s the
   census outline regex initially missed, disclosed — deterministic
   records, the marker vocab tables) → `ai_attributes/extraction.rs`; pub
   surface re-exported (workers' compute_with_backend caller untouched).
2. **RF-7-2 helpscout.rs 1810→1063** — the inert provider (755 ln:
   EmptyHelpScoutProvider, InertWorld, the full `impl
   HelpScoutProvider for EmptyHelpScoutProvider`, cursor codec,
   no_account/not_found_remote, the region-only `use
   std::sync::Mutex;` travelling with its consumer) → `helpscout/inert.rs`;
   `pub use inert::{EmptyHelpScoutProvider, InertWorld}` keeps
   workers_host/sync_engine/workers + provider_handle constructing it.
3. **RF-7-3 app.rs 1700→1402** — the shell chrome (313 ln:
   human_outbound_kind, sync_completed_toast, NAV_GROUPS, nav_row,
   palette_trigger, theme_row) → `app/chrome.rs`; six `pub(super)` bumps +
   the parent's `use chrome::{…}` import (app-internal surface, no
   external callers).
4. **RF-7-4 people_store.rs 1685→1166** — the support-health report family
   (529 ln: EVIDENCE_LIMIT, evidence_ids, health_metric, health_flag,
   build_support_health_report) → `people_store/health.rs`; one
   `pub(super)` bump (build_support_health_report, the only parent
   caller).
5. **RF-7-5 support_graph.rs 1671→927** — the derived-edge engine (754 ln:
   DerivedEdge + derived_edges_touching) → `support_graph/derived.rs`;
   nine `pub(super)` bumps (the fn + all eight struct fields —
   neighbors_json reads relation/origin/kind/id/note/at directly);
   `use derived::derived_edges_touching` import.
6. **RF-7-6 search.rs 1641→1254** — the per-type lanes (397 ln:
   search_conversations/customers/knowledge_raw/knowledge/known_issues/
   saved_replies/ai_analyses + their three private helpers) →
   `search/lanes.rs`; seven-fn `pub use` re-export; escape_like and the
   FTS migration stay put (RF-4 canonical untouched).
7. **RF-7-7 ai_tools.rs 1624→483+483** — the tool-execution engine (1144
   ln: execute + clamp_limit/arg_i64/red/red_chars/conv_by_number — usage
   census confirmed execute-only — + map_cluster_row/collect_rows) →
   `ai_tools/execute.rs`; `pub use execute::execute`.
8. **RF-7-8 saved_views.rs 1568→820** — the compile engine (`impl
   ViewEngine`, 756 ln) → `saved_views/engine.rs`; no re-export needed
   (inherent impl on the parent-resident ViewEngine struct resolves
   crate-wide).
9. **RF-7-9 coaching.rs 1565→831** — the check family (742 ln, nine
   check_* fns) → `coaching/checks.rs`; nine `pub(super)` bumps + the
   parent import (get() calls all nine).
10. **RF-7-10 pages/queries.rs** — the invalidation map + field helpers
    (101 ln: invalidated_pages with its doc, str_field/i64_field/
    opt_str_field) out of pages/mod.rs (618→522); `pub use
    queries::invalidated_pages` + `pub(crate) use queries::{…}`; the
    PageKey enum/labels/all()/tests stay — gate_traceability's
    routes() source intact, gate re-proven at 23/20/27.

Two compiler-demanded cfg(test) follow-ons, disclosed: the ai_tools
`batch29_audit98_99_tests` mod (103 ln, three tests of the moved
arg_i64/execute paths) relocated into execute.rs — tests of moved code
live with the moved code; `lock_world` bumped `pub(super)` for the
helpscout parent tests.

Incident (disclosed, recovered): the first style_lint rerun hit "No space
left on device" — a shell missing the `CARGO_INCREMENTAL=0` contract let
incremental sessions regrow; incremental dir cleaned (1.2 GB freed), the
run repeated under the contract and passed.

Verification (all green): fmt clean; clippy `--workspace --all-targets -D
warnings` 0; gate_traceability OK (23 routes / 20 DC tags / 27 ids);
gate_no_simulation clean; verify-icons clean; style_lint 2/0; core lib 495
passed / 0 failed / 4 ignored (--test-threads=1, 351.72s); app-shell lib
237/0; xtask 11/0; doc-tests 0/0. Churn proof: only the ten split modules
+ this record changed. Diff: 21 files, +5850/−5690 (net +160 — the splits
are structural). Kittest 24 suites: CI lane (green in CI run 41284592762,
re-confirmed on this push).

## Batch RF-8 — renames (opened and completed 2026-11-01)

The plan.md §RF-8 census at open (word-boundary reference census per
candidate across all four crates + tests + the two live gates +
README/USER_MANUAL) selected ten naming debts surfaced by RF-2..RF-7
execution. All renames are zero-semantic-change: `git mv` (rename
detection intact) + reference re-pointing, compiler + full suite prove
behavior unchanged.

1. **RF-8-1 eventing.rs → event_bus.rs (core file)** — the module is the
   EventBus home (broadcast bus + notify_* helpers); the `events` (type
   definitions) vs `eventing` (the bus) minimal pair was the confusable
   debt. lib.rs decl + all 53 path refs follow across core src, app-shell
   src, it_sync/it_helpscout and the two kittest suites.
2. **RF-8-2 hybrid_search.rs → rrf.rs (core file)** — since batch 34 the
   module carries only `RRF_K` + its characterization test; the name
   described the deleted fusion stack. lib.rs decl + 3 `crate::rrf::RRF_K`
   refs in search/mod.rs + two comment mentions updated truthful (the
   module doc's history line intentionally still names the deleted
   `hybrid_search` entry point).
3. **RF-8-3 sync.rs → mirror.rs (core file)** — the Hs* mirror-upsert
   layer; debt proven by the codebase's own alias `use crate::sync as
   mirror;` (sync_engine/mod.rs:12) — the alias dropped, the 12 `mirror::`
   sites resolve directly, 5 `crate::sync::` sites re-pointed (bench×3,
   workers/execute×2); `sync_engine`/`sync_schema` untouched
   (word-boundary proof: zero `crate::sync\b` refs remain).
4. **RF-8-4 interaction_current.rs → interaction_signals.rs (core
   file)** — word-salad name next to `interaction_engine/`; the module
   holds the interaction-signal infrastructure (client_current_signals +
   interaction_evidence tables, signal vocabulary, needs_de_escalation,
   MessageStats). ~35 refs across 10 files follow. Disclosed path-follows:
   migrations/db_breadth.rs:360 (import path only — zero SQL semantics
   touched, diff-verified one line), the bench timeit label, one
   baseline.rs comment; intelligence.rs:46 comment stays byte-identical
   (NO-TOUCH; dies with all comments at RF-11).
5. **RF-8-5 mention.rs → mention_popup.rs (app-shell file)** — the
   composer mention-autocomplete popup logic; disambiguates from the core
   `mentions` parser it consumes. inbox.rs import + lib.rs decl follow.
6. **RF-8-6 bridge.rs → task_bridge.rs (app-shell file)** — module name =
   the `TaskBridge` type it holds. app/mod.rs import + lib.rs decl follow.
7. **RF-8-7 `red` → `redact` (item, ai_tools/execute.rs)** — the fn
   applies the redaction gate (`redaction_enabled` setting +
   `security::redact_text`); "red" read as a color. Private, file-local.
8. **RF-8-8 `red_chars` → `redact_chars` (item, same file)** — same debt;
   all call sites file-local.
9. **RF-8-9 `conv_by_number` → `conversation_by_number` (item, same
   file)** — abbreviation debt surfaced by the RF-7 census that moved it;
   private, file-local.
10. **RF-8-10 `upsert_customer_basic` → `upsert_customer_without_channels`
    (item, mirror.rs + its one caller sync_engine/loops.rs:762)** —
    "basic" never said what it does; the name now carries the audit-C4
    contract (upsert WITHOUT touching the contact-channel tables).

Census exclusions recorded (with reasons): `helpscout_real` (established
real-vs-inert pairing), `webhook.rs`/`webhook_handler.rs` (distinct
roles), `vectorstore.rs` (holds the `VectorStore` trait — name is right),
`search::search` (the documented `/api/search` port; the stutter lint is
explicitly allowed off), `it_*`/`mockserver`/`bench` (RF-10 territory),
migration modules (wiring stays), audit-artifact test names
(`batch29_audit98_99_tests`, `auditNNN_*` — the audit-trail traceability
convention), `ProviderHandle`/`ProviderKind` (moderate value, high UI
churn — dropped).

**Census FINDING disclosed for the owner's decision:**
`mirror_readouts.rs` (298 ln, five pub fns: inbox_fields/workflows/
user_statuses/webhook_configs/saved_replies + ensure_tables) has ZERO
references anywhere — no path refs, no fn callers, not in tests or gates.
It is dead code, but deletion is outside RF-8's rename scope, so it is
preserved byte-identical and parked like the RF-6 zero-ref disclosures
(`ai_pipeline::ProcessNewTicketResult`, `interaction_engine::AiEnrichment`).
Recommend: delete in a follow-up owner decision (or at FINISH if the
owner approves).

Verification (all green): fmt clean (applied AFTER the last content edit
— the rename-driven re-sort of module decls/import order was applied and
re-checked; RF-7 hotfix lesson honored); clippy `--workspace
--all-targets -D warnings` 0; gate_traceability OK (23 routes / 20 DC
tags / 27 ids); gate_no_simulation clean; verify-icons clean; style_lint
2 passed / 0 failed (explicit — RF-6 lesson); core lib 495 passed / 0
failed / 4 ignored (--test-threads=1, 350.79s); app-shell lib 237/0;
xtask 11/0; doc-tests 0/0. Churn proof: 6 git-mv renames (0-line deltas
on moved files) + 31 ref-site files (+233/−139 across 37 files incl. the
two ai records). Kittest 24 suites: CI lane (runs on this push). Docs:
README/USER_MANUAL/docs reference none of the renamed names — zero doc
debt created for RF-9. Dead `scripts/archive/*` verifiers still name old
paths — dead code, historical records, untouched.

## Batch RF-9 — docs sync (opened and completed 2026-11-01)

Ten items selected at open by the docs census (grep of the four live
surfaces — README.md, USER_MANUAL.md, .gitignore, ai/decisions.md +
ai/rule.md — for every pre-RF-5..RF-8 module basename/path, plus
claim-by-claim verification against source ground truth: Cargo.toml
members, the xtask Cmd enum, PageKey variants, ci.yml lanes,
.cargo/config.toml, core env readers, xtask hygiene tests). The three
content edits touched docs only — zero code, test or gate bytes changed;
every gate number below therefore re-proves the pre-existing behavior on
the post-edit tree.

RF-9-1 (census) — the old→new module map of the whole program (RF-5
migrations relocation; RF-6 splits workers/ai_pipeline/helpscout_real/
sync_engine/outreach/design/segment/quality/interaction_engine; RF-7
splits ai_attributes/helpscout/app/people_store/support_graph/search/
ai_tools/saved_views/coaching + pages/queries.rs; RF-8 renames
eventing→event_bus, hybrid_search→rrf, sync→mirror,
interaction_current→interaction_signals, mention→mention_popup,
bridge→task_bridge) grepped over the four live surfaces: 2 genuine doc
hits (README Verification section drift — item 2; ai/decisions.md D6
`design.rs` — item 6) + 3 dead .gitignore entries (item 5) + 0 hits in
USER_MANUAL.md and ai/rule.md. Historical batch ledgers (plan.md
RF-sections, progress.md, completed.md) excluded by design — their
old-path mentions ARE the audit trail.
RF-9-2 (README fix) — the "Verification" CI bullet re-pointed to
.github/workflows/ci.yml ground truth: now names the two machine gates
(scripts/gate_no_simulation.py + scripts/gate_traceability.py, ci.yml:35/
41), icon verification (ci.yml:66), clippy across the workspace AND the
sandbox feature (ci.yml:69/79 — the README said only sandbox tests),
cargo-deny advisories/licenses/bans (ci.yml:95 — the README dropped
`bans`), and the kittest snapshot locks (ci.yml:181, the reskin drift
gate on lavapipe — added by RS-21, confirmed by the RF-6-era kittest CI
gap closure; the README never mentioned it). The AppImage/.deb/smoke
claims re-verified line-by-line unchanged (job 2).
RF-9-3 (README verify, no-op) — "What's inside" proven 4/4 crates =
Cargo.toml members; 10/10 xtask subcommands = crates/xtask/src/bin/
xtask.rs Cmd enum (dev/lint/verify-icons/package/deb + migrate/backup/
restore/healthcheck/vector-count); 23/23 pages = PageKey variants incl.
display spellings (AI Center, Live Chats, Issue Radar, Support Graph,
Sync Health, Custom Objects).
RF-9-4 (README verify, no-op) — .cargo/config.toml alias = `run --locked
-p supportos-plus-xtask --` (README claim verbatim); rust-toolchain.toml
present; package output paths = xtask defaults (target/appimage,
target/deb); bundled-libs claim = ci.yml:122 (libxkbcommon0
libxkbcommon-x11-0 libxcb-xkb1).
RF-9-5 (.gitignore fix) — three dead crate-path entries removed:
`/crates/ui/dist`, `/crates/app/src-tauri/target`, `/crates/app/
src-tauri/gen` (crates/ui and crates/app ceased at the Phase 4 cutover;
workspace = app-shell, catalog, core, xtask — no tooling references the
dead paths, grep-verified over .github/ + scripts/). Every remaining
entry kept with justification: /target /dist (build output), /data
(xtask backup default ./data/backups), *.sosync (crypto sync bundles,
README-claimed), *.supportos (workspace data), .env/!.env.example
(secret overrides), .vscode/.idea/.DS_Store/Thumbs.db/*.log/*.dmp
(editor/OS/diagnostic noise), /reference /supportos (scratch),
*.db/*.db-shm/*.db-wal (SQLite sidecars), secrets.key (FIND-003 —
pinned by the spp_xtask hygiene test, which passes post-prune:
gitignore_keeps_the_secrets_key_never_track_rule +
no_secret_key_artifacts_anywhere_in_the_repo, 2/0).
RF-9-6 (ai/decisions.md fix) — D6's mapping re-pointed: "every `--ds-*`
token → one token-factory constant in `crates/app-shell/src/design/`"
with the RF-6 history named inline; the 1:1 contract preserved
truthfully. decisions.md otherwise clean: line 21's `ui/src/pages` is
the UPSTREAM original's tree (correct as written); D7 anchors are
history.
RF-9-7 (USER_MANUAL verify, no-op) — zero old-basename/`src/`/`crates/`
hits; §13 env names verified name-by-name against source readers:
SPP_DATA_DIR (config.rs:35), SPP_LOOPBACK_PORT (app-shell main.rs),
SPP_LOG_FORMAT (core logging.rs:41), SPP_RENDERER (app-shell lib.rs:211
+ fatal.rs:60), HELPSCOUT_CLIENT_ID/SECRET/WEBHOOK_SECRET (config.rs +
loopback.rs:313), HELPSCOUT_API_BASE/DOCS_API_BASE/DOCS_API_KEY/
REDIRECT_URI (config.rs), LMSTUDIO_BASE_URL/CHAT_MODEL/EMBEDDING_MODEL/
TIMEOUT_MS/CONCURRENCY (config.rs:245..), QDRANT_URL/QDRANT_ENABLED
(config.rs), LOG_LEVEL (logging.rs:23), SYNC_INTERVAL_MINUTES/
SYNC_API_CONCURRENCY (config.rs); §8 page walkthrough = 23/23 PageKey
coverage.
RF-9-8 (frozen/dated artifacts disposition, NOT edited) — docs/audit/
design-contract-v1.md: frozen DC v1.0, a gate input; its two `design.rs`
refs (lines 11/36) describe the v1.0 freeze-time state and stay — the
gate's own DESIGN constant already follows design/mod.rs since RF-6;
mockups/design-lab/README.md + mismatch-register.md: dated reskin
artifacts whose line refs predate the reskin they document (app.rs
NAV_GROUPS since moved to app/chrome.rs — RF-7); repo-audit-2026-10-24/,
qa-audit/20261019T050847Z/, qa-redesign/, docs/audit/acceptance/: dated
audit artifacts. Zero live claims depend on their stale paths.
RF-9-9 (CHANGELOG disposition, NOT edited) — the three `design.rs`
mentions (lines 66/837/842) are immutable release-history records
describing v1.x states at release time; history is not rewritten;
disclosed so the post-batch grep expectations are explicit.
RF-9-10 (closing gate) — post-fix census re-run: the 28-basename
old-path grep over the four live surfaces returns exactly one hit — the
D6 historical framing itself (old name stated as PAST); every other old
basename and every `crates/*/src` path returns zero hits (the only
`crates/` path now named in the live docs is the new, correct
`crates/app-shell/src/design/`).

Verification (all green): fmt clean (run after the last content edit;
docs-only edits, zero Rust bytes touched); clippy `--workspace
--all-targets -D warnings` 0; gate_traceability OK (23 routes / 20 DC
tags / 27 ids, alias declared True); gate_no_simulation clean;
verify-icons clean; style_lint 2 passed / 0 failed (explicit — RF-6
lesson); core lib 495 passed / 0 failed / 4 ignored (--test-threads=1,
352.28s); app-shell lib 237/0 (213.39s); xtask 11/0 (lib) + 0 bin + 0
doc; doc-tests 0/0 (core + app-shell); hygiene 2/0 post-prune. Kittest
24 suites: CI lane (runs on this push). Churn proof: 3 content files
(README.md, .gitignore, ai/decisions.md) + the two ai records; diff is
docs-only (zero .rs bytes, zero CI/workflow bytes, zero script bytes).

## Batch RF-10 — test paths follow the new structure (opened and completed 2026-11-01)

Ten items selected at open by the test-surface census: every test vehicle
inventoried (core in-lib: it_sync/it_helpscout/it_loopback/it_lm_studio/
bench.rs + the `#[cfg(test)]` mods of every split parent; app-shell: 24
external suites + in-src test mods), every RF-8 old name grepped, every
split parent's child re-export lines extracted and cross-checked against
its own test-mod ranges. Finding: the external vehicles and the
overwhelming majority of parent test mods already follow the new layout —
RF-8 re-pointed every renamed path (zero old-name hits in test code), the
kittest suites import current homes (`spp_shell::design` = design/ mod.rs,
`pages::<page>`, `spp_core::bootstrap::apply_all` = the intended public
API), the it_*/bench imports name unaffected or current homes, and the
search/quality/outreach/design-controls/interaction-engine/segment/
sync_engine/workers child items are used only by parent production code or
via inherent impls (methods need no imports). Exactly FIVE files carried
parent-resident test mods resolving child-module items through `use
super::*` → the parent's compatibility re-exports: nine import-path-only
re-points, zero semantic test change.

RF-10-1 (census) — the evidence ledger recorded in progress.md: RF-8 old
names zero hits; kittest 1-2 current-home refs each; it_*/bench clean;
child-item probes across all parents isolate the five files.
RF-10-2 (ai_attributes a2_wiring_tests) — explicit
`use super::extraction::compute_with_backend;` (was: `use super::*` →
parent's `pub use extraction::{…}`).
RF-10-3 (ai_attributes batch22_audit_tests) — explicit `use
super::extraction::{ExtractionOutput, finish_computation,
prepare_computation, run_extraction};`.
RF-10-4 (ai_attributes batch23_audit_tests) — explicit `use
super::extraction::{ATTRIBUTE_EXTRACTION_PROMPT_VERSION, ExtractionOutput,
customer_messages, finish_computation, prepare_computation};`.
RF-10-5 (ai_tools batch23_audit_tests) — explicit `use
super::execute::execute;` (module and item share the name — valid path;
was: parent's `pub use execute::execute;`).
RF-10-6 (helpscout batch20_audit25_tests) — explicit `use
super::inert::EmptyHelpScoutProvider;` (InertWorld unused by the mod —
verified; was: parent's `pub use inert::{…}`).
RF-10-7 (app-shell app/mod.rs tests) — explicit `use
super::chrome::{NAV_GROUPS, sync_completed_toast};` (was: `use super::*`
→ the parent's production `use chrome::{…}`). Disclosed keep: the mod's
`use crate::pages::invalidated_pages;` stays — `mod queries` is private
to pages, so the parent re-export IS the load-bearing cross-module test
surface; widening it would be a production-visibility change (out of the
import-path-only scope).
RF-10-8 (app-shell pages/mod.rs tests) — explicit `use
super::queries::{i64_field, invalidated_pages, opt_str_field, str_field};`
(was: `use super::*` → parent's `pub use` + `pub(crate) use`).
RF-10-9 (external vehicles verify, no-op) — it_sync / it_helpscout /
it_loopback / it_lm_studio / bench.rs + 23 kittest suites + style_lint:
every import names an unaffected or current home; zero old names;
style_lint's token-factory exemption already follows `src/design/` (RF-6
hotfix). Verified, nothing to change.
RF-10-10 (closing gate) — post-edit re-census: all nine test mods name
the RF-6/RF-7 child homes explicitly (7 use statements, grep-verified);
parent re-exports untouched and still load-bearing for cross-module test
imports and production callers (sync_engine/mod.rs:1777, workers_host.rs:
173 → helpscout::EmptyHelpScoutProvider).

Verification (all green): fmt clean (applied + re-checked after the last
content edit; the inserted use lines were already rustfmt-conformant);
cargo check --workspace --all-targets 0 errors; clippy `--workspace
--all-targets -D warnings` 0; gate_traceability OK (23 routes / 20 DC
tags / 27 ids, alias declared True); gate_no_simulation clean;
verify-icons clean; style_lint 2 passed / 0 failed (explicit — RF-6
lesson); core lib 495 passed / 0 failed / 4 ignored (--test-threads=1,
351.83s — the re-pointed core mods execute); app-shell lib 237/0
(206.24s — the re-pointed shell mods execute); xtask 11/0; doc-tests
0/0. Kittest 24 suites: CI lane (runs on this push). Churn proof: +12
lines across exactly the 5 target files (imports only) + the ai records;
`git diff` review = zero test-body bytes, zero production bytes, zero
gate/workflow bytes.

## Batch RF-11 — comment deletion (LAST batch of the program order) — completed 2026-11-01

All comments deleted in one final wave (module docs, item docs, inline) —
11,929 comment lines censused at open, 11,871 deleted, **58 gate-locked
lines kept**; rustfmt + full gates green after. The ten items:

RF-11-1 (census + protection contract) — Rust-lexer census of all four
crates: 11,929 comment lines (11,927 line comments + 2 block comments —
pages/ai_center.rs:742, pages/knowledge.rs:1133 `/* DC sp-1 */`; zero
`/**`/`/*!` doc blocks) ≈ 693 KB. Protections recorded at open and held
throughout: (a) the stripper is a real lexer (strings, raw strings, byte
strings, chars, lifetimes, nested block comments) — comment-like
sequences inside string literals (SQL `--`, `https://`, security.rs's
CSS `/*…*/` prose) structurally unreachable; (b) every DS-id-bearing
comment line in design/mod.rs (58) kept byte-identical → gate output
unchanged; (c) `missing_docs` probe: core/catalog's later
`#![allow(..., missing_docs)]` overrides the earlier `#![warn(...,
missing_docs)]` (rustc -D warnings probe) → doc deletion fires zero
missing_docs warnings, **zero lib.rs attribute edits**; (d) zero
`style-lint:allow` opt-outs existed; (e) `#[doc(hidden)]`
(pages/operations.rs:242) is an attribute — stays.

RF-11-2..RF-11-9 (the eight deletion waves, each stripped + compiled +
gated) — core root wave 1 (41 files, activity→inbox) and wave 2 (42
files) incl. the NO-TOUCH root members ai_center.rs, db.rs,
maintenance.rs, response_state_sql.rs; core subdirs wave 1 (workers/,
helpscout_real/, ai_pipeline/, sync_engine/) and wave 2 (all remaining,
incl. migrations/ and its NO-TOUCH intelligence.rs — MIGRATIONS SQL
string contents byte-identical incl. the `--` sequences inside the
strings); catalog/src + xtask/src incl. bin/; app-shell root + app/;
app-shell pages/ (the two DC sp-1 block comments die here —
gate_traceability does not read pages/); design/ (mod.rs with the 58
DS-id lines kept, controls.rs) + crates/app-shell/tests/ (24 kittest
suites + style_lint.rs). NO-TOUCH interpretation recorded: NO-TOUCH =
zero semantic/code edits — comments die with the batch per the RF-8
record ("intelligence.rs:46 comment … dies at RF-11"); the five modules'
non-comment bytes stayed byte-identical (0 insertions in their diffs).

RF-11-10 (closing gate) — the full battery, all green (numbers in the
verification table below). Structural proofs beyond the battery:
string-literal multiset identity vs HEAD for all 179 changed .rs files
(0 failures); residual census = exactly 58 comment lines, all DS-id
lines in design/mod.rs; DS lines 58→58 with zero deleted.

**Disclosed deviation — four compiler-suggested collapses**: clippy's
`collapsible_if` is comment-aware; deleting the comments exposed four
latently-collapsible `if`s the intervening comments had suppressed:
people_store/mod.rs:706 (Clean-audit item 47 comment), overlays.rs:119
("Window close button / Escape cancel" comment), pages/operations.rs:
337, pages/settings.rs:810. Fixed per the machine suggestions (one by
hand, three via `cargo clippy --fix`, then fmt-reflowed) — zero
behavior change, proven by the full suite. These four sites are the
batch's only non-comment code bytes.

Scope: .rs files only — scripts/*.py, ci.yml, Cargo.toml, README/USER_
MANUAL and the ai/*.md records keep their comments (harness and records
are not program source).

Verification (all green): fmt clean (applied + re-checked after the
last content edit); cargo check --workspace --all-targets 0 errors;
clippy `--workspace --all-targets -D warnings` 0 (after the four
disclosed collapses); gate_traceability OK — identical line: 23 routes
/ 20 DC tags / 27 ids, alias declared True; gate_no_simulation clean;
verify-icons clean; style_lint 2 passed / 0 failed (explicit — RF-6
lesson); core lib 495 passed / 0 failed / 4 ignored (--test-threads=1,
353.10s); app-shell lib 237/0 (204.41s); xtask 11/0; doc-tests 0/0
(four crates). Kittest 24 suites: CI lane (runs on this push). Churn
proof: +268/−12,099 across 181 files (179 .rs + ai/plan.md +
ai/progress.md); the +268 = comment-trimmed code lines, the four
collapses, fmt reflows, and the ai records.

## FINISH — program close-out (2026-11-01)

The dead-code/refactor program RF-1..RF-11 is complete. Every batch was
executed, verified and pushed under rule.md; plan.md has no open items.

**Program ledger (pushed commits, oldest → newest):** 454b6f0 (rf1),
62a2277 (rf2), 29215a1 (rf3), 63ee353 (rf4), 1f48b0d (rf5), a60904c
(rf6), 984f83a (rf6 hotfix), 7d15bbe (rf7), 5a33641 (rf7 hotfix),
95c5cc9 (rf8), f70b97c (rf9), c65dba3 (rf10), 672fef7 (rf11). Final
state: main = 672fef7, comment-free tree (58 gate-locked DS-id lines in
design/mod.rs are the only comments in any .rs file), working tree
clean, all gates green at RF-11 (the table in the RF-11 record above).

**Rollback instruction (ships at FINISH per plan.md):**

    git reset --hard pre-refactor

- The named anchor `pre-refactor` now exists: annotated tag pushed at
  FINISH on commit `acd557afb3105b34bec99b7aa82a6aebb85ca93e` — the
  v1.9.1 release commit ("the G4-accepted state"), verified an ancestor
  of main. Equivalently: `git reset --hard v1.9.1`.
- Disclosed record fact: no `pre-refactor` tag existed before FINISH
  (verified local + `git ls-remote`); the plan's parenthetical "(tag
  `pre-refactor` equivalent = v1.9.1)" was literal — v1.9.1 was the
  only anchor. FINISH created the tag so the instruction above is
  self-contained. The tag adds nothing to the code; it names the
  commit the plan already designated.
- Effect: discards the entire program (13 commits, rf1..rf11) and
  restores the v1.9.1 tree byte-for-byte. No other steps required.

**Known-bugs list (carried verbatim from the plan's standing
constraints — LIST-ONLY, never fixed inside this program, still open on
main):**

  4× empty-list per-frame refetch; Settings LM Studio dirty-clobber

Current-code evidence (FINISH re-census, so the owner can act on the
list without re-deriving it):

- Empty-list per-frame refetch — the exact `if state.is_empty() {
  reload(db) }` shape batch 34 fixed elsewhere (item 147, with
  `loaded` latches) is still live, latch-free, in three pages (7
  gates): pages/custom_objects.rs:182 (`reload_types`) and :184
  (`reload_objects`); pages/inbox.rs:847 (`reload_refs`) and :850
  (`reload_list`) in `ensure_loaded`, plus the same mailboxes gate
  inside `reload_list` at :402; pages/knowledge.rs:687 and :764 (both
  `reload_all` — an empty documents/sources mirror re-runs the full
  reload twice per rendered frame, once per tab's ui fn). A genuinely
  empty mirror — the steady state of a fresh install — re-runs the
  full reload query on every rendered frame at each gate.
- Settings LM Studio dirty-clobber — pages/settings.rs:538-557: the
  lmstudio_* form load is gated on `self.lmstudio_base_url.is_empty()`
  with no dirty guard (the General section's `general_persisted`
  compare at :507-511 is the in-file counterexample): clearing the
  base-url field — or any persisted-empty config — re-pulls the stored
  lmstudio_base_url/chat_model/embedding_model over unsaved edits on
  every subsequent frame.

**Kittest CI-lane disposition (the standing gap, closed at CI):** the
24 suites / 175 tests / 171 goldens never ran in the sandbox (dev-dep
chain ~600 MB vs 3 GB sandbox RAM — RF-1-9, disclosed at every STOP).
They ran in CI on every program push; the final proof on the last
program commit 672fef7 (GitHub check runs, all completed/success):
"kittest snapshot locks (reskin drift gate)", "fmt, clippy, test,
deny", "AppImage + .deb build, smoke". The comment-free tree —
including the four disclosed collapsible_if collapses — shipped with
zero golden drift and a green build+smoke.

**FINISH environment disclosure:** the sandbox was reset again between
RF-11 and FINISH (local tree and stored credentials lost); this record
was written from a fresh clone of origin/main (recovery precedent,
RF-1). FINISH changed zero code bytes: two ai records, the
pre-refactor tag, this commit.

## Batch KB-1 — known-bugs fix wave + kittest in-sandbox resolution — completed 2026-11-01

Owner-directed post-FINISH batch (the FINISH program's LIST-ONLY scope
decision on these two bugs superseded by the owner's explicit "fix this"
+ "resolve this"). The two known bugs are FIXED on main; the kittest
CI-lane gap is RESOLVED in-sandbox.

KB-1-1 (custom_objects) — pages/custom_objects.rs: batch-34 latch pattern
applied to both buggy gates: `types_loaded` (reload_types sets it,
invalidate clears it) and selection-scoped `objects_loaded` (reload_objects
sets it; cleared on invalidate, select_type, delete_type's selection-clear,
and reload_types' selection-vanished branch — selection changes still
refetch). ensure_loaded fires once per latch; a genuinely empty mirror no
longer re-runs reload_types/reload_objects every rendered frame.

KB-1-2 (inbox) — pages/inbox.rs: `refs_loaded` + `list_loaded` latches
(set in reload_refs/reload_list; reload_list's latch sits before the
refresh_view_counts tail so the page-clamp recursive path also lands on
it); invalidate clears both; the inline ui() gates extracted into
`ensure_loaded` (the batch-34 shape); the filters-prerequisite gate inside
reload_list now keys on the latch, and the transient detail gate is
unchanged. An empty mirror no longer re-runs reload_refs + reload_list
per frame.

KB-1-3 (knowledge) — pages/knowledge.rs: one `loaded` latch over
reload_all, extracted into `ensure_loaded`, gating both documents_ui and
sources_ui (pre-fix each tab fn re-ran the full reload_all every frame on
an empty mirror — twice per frame across the two). The empty-state
rendering after the gate is unchanged.

KB-1-4 (Settings LM Studio dirty-clobber) — pages/settings.rs: the
`lmstudio_base_url.is_empty()` gate replaced with General's
`general_persisted` dirty-guard shape (the in-file counterexample): a
`lmstudio_persisted: (String, String, String)` snapshot; reload() assigns
to the form only when the form matches the snapshot; save_lmstudio
re-bases the snapshot on the saved trimmed values. Clearing the URL field
— or a persisted-empty config — no longer re-pulls stored values over
unsaved edits; external changes still propagate while the form is clean.

KB-1-5 (kittest in-sandbox) — the RF-1-9 standing gap RESOLVED: sandbox
capacity re-verified (7.7 GB free / 2.8 GB RAM available — the ~600 MB /
3 GB constraint no longer binds); Mesa lavapipe installed without root
(apt-get download + dpkg -x of mesa-vulkan-drivers 25.0.7 + libllvm19
into /home/z/mesa-lvp; VK_DRIVER_FILES/VK_ICD_FILENAMES at the extracted
lvp_icd.json, LD_LIBRARY_PATH at the extracted libs, WGPU_BACKEND=vulkan
— ci.yml's own env block ci.yml:198-220); the CI command
`cargo test --locked -p supportos-plus-app-shell --features kittest-ui`
run per suite. **23 suites / 173 tests passed / 0 failed; all 171
committed goldens diff-locked with zero drift; snapshots/ = exactly 171
.png baselines (census reconciled: 24 files = 23 suites + style_lint,
175 tests = 173 + 2, 171 goldens).** style_lint under the same feature
build: 2/0. First in-sandbox kittest run in the program's history.

KB-1-6 (closing gate) — all green: fmt clean (applied + re-checked);
check --workspace --all-targets clean; clippy --workspace --all-targets
-D warnings 0; gate_traceability OK (23 routes / 20 DC tags / 27 ids,
alias True); gate_no_simulation clean; verify-icons clean; style_lint
2/0; core lib 495/0/4ig (--test-threads=1, 348.55s); app-shell lib
242/0 (235.32s — 237 baseline + exactly the 5 new tests); xtask 11/0;
doc-tests 0/0; kittest 175/175 with 171/171 goldens (in-sandbox AND CI
lane). Churn: +339/−38 across 4 source files (4 fixes + 5 regression
tests) + the ai records.

## Batch AUDIT-1 — full-repo dead-surface audit — completed 2026-11-02

Read-only whole-repo audit @ `0f02e62` (kb1): 205/205 .rs reachable (0
orphaned files); ~25 verified-dead pub API clusters inventoried with
locations (largest: the saved-views engine cluster, core/saved_views/
engine.rs + mod.rs:536-595); ~80 EXPORTED-UNUSED de-pub candidates
(disposition: de-pub later, not delete); 2 allow(dead_code) stashes;
5 unused deps/features (subtle; xtask serde+serde_json; root anyhow;
rusqlite blob+limits; chrono/serde); 4 placebo settings switches
(UI+persisted, zero readers); 11 read-never-written knobs; 1 dead event
(CampaignUpdated + 3 dead helpers); 4 stale live-doc paths; 2 orphaned
scripts + verify_batch35.py archive gap + ~48 MB unreferenced history
dirs (qa-audit/, repo-audit-2026-10-24/, qa-redesign/); clean on
TODO/FIXME, assets, schema drift, routes, CI paths. Method: 6 parallel
scripted scans + 3 manual confirmations; zero cargo runs (audit sandbox
at 99% disk — disclosed; kb2 re-proves every removal against the full
gate battery). Environment: sandbox reset after the kb1 push — repo +
credentials + toolchain re-provisioned; records written from a fresh
clone of origin/main @ `0f02e62` (tree byte-identical to the audited
one). Fix disposition: kb2 takes the deletions (A1-2 minus the
response_state_sql.rs NO-TOUCH exception, A1-4, A1-8), dep pruning
(A1-5), placebo wire-or-drop (A1-6), doc path fixes (A1-9), script/dir
disposition (A1-10); A1-3 de-pub sweep + A1-7 knob census recorded as
open items.

## Batch KB-2 — dead-surface deletion + dep pruning + placebo wire-or-drop — completed 2026-11-02

Implements AUDIT-1's disposition; every removal re-proved by the
compiler + full gate battery; two audit over-reaches corrected during
the re-proof (compute_friction family LIVE via recompute_recent_friction
→ kept; BatchRow live via outreach/send.rs → only its `email` stash
field removed; delete_segment live → restored on the compile catch).

Deleted: the saved-views engine cluster (engine.rs + mod.rs dead
surface, mod.rs 800→51, live trio kept); universal_search/rewrite_draft/
validate_ssrf_full-sync/scan_for_mentions/redacted_secret/
InteractionCard/CardProvenance/AiEnrichment/LinkError; reports
ReportRow/ReportResult/build_report/metric_info/friction_findings + 2
tests; outreach SavedSegment/create_segment/Campaign/CampaignRecipient/
CampaignStats/campaign_reconcile/count_remaining + batch30_tests;
interaction_signals' interaction_profile island + INTERACTION_DIMENSIONS;
22 dead consts (incl. 6 never-executed M0XX_SQL duplicates — registry
untouched; M009/M014 NO-TOUCH exceptions held); 9 dead design tokens
(gate output unchanged: 20 DC tags / 27 ids); 9 dead catalog re-exports
(2 initially mis-cut via a braced-import census hole — restored on
compile, disclosed); the CampaignUpdated event family + channel/
payload_json/subscriber_count (SyncCompleted now also invalidates
Outreach — the D19 test caught the uncovered page at 241/1, re-wiring
→ 242/0); the automatic_reply_sending placebo end-to-end (UI row
dropped) + the write-only fts_version marker; 2 orphan scripts; the 3
unreferenced history dirs (528 tracked files, reversible via git
history); 4 stale doc paths re-pointed (incl. the gate docstring).
demo_mode/display_timezone verified already-inert — recorded, no
change. Deps pruned: subtle, root anyhow, xtask serde+serde_json,
rusqlite blob+limits, chrono/serde; Cargo.lock = 4 dep-reference
removals, zero version churn.

Verification, all green: fmt clean; clippy --workspace --all-targets
-D warnings 0; gate_traceability OK (23 routes / 20 DC tags / 27 ids,
identical to kb1); gate_no_simulation clean; verify-icons clean;
style_lint 2/0; core lib 489/0/4ig (--test-threads=1, 345.18s — 495
minus exactly the 6 deleted cluster tests); app-shell lib 242/0; xtask
11/0; doc-tests 0/0; kittest 23 suites / 173 passed / 0 failed —
exactly the 6 settings goldens regenerated for the intended checkbox
drop, the other 165 zero-drift. Churn: +139/−2011 across 50 tracked
files + 528 history-dir deletions + ai records. Environment: 4th
sandbox reset — repo/toolchain/Mesa re-provisioned from a fresh clone
@ `0f02e62`; one disk-full event during the kittest build (bus-error
links) reclaimed and rebuilt with DEBUG=0 + -j 2 (disclosed).

## KB-3 — A1-3 de-pub sweep + A1-7 knob census + response_state_sql disposition (2026-11-02)

The A1-3/A1-7 open items closed at `59f981d`, plus one kb2 record
correction and the owner-delegated response_state_sql decision. Owner
directives: response_state_sql.rs's 3 dead fns = **KEEP** (NO-TOUCH
module; ~30 dead lines are not worth a gate re-proof on the most
sensitive SQL surface — logical minimum-risk choice, module never
touched, diff-verified); A1-3 + A1-7 approved.

A1-3: the census (conservative name-based, collisions only suppress)
found 1,182 zero-external-consumer pub items = 44 same-file-only +
1,134 cross-file crate-internal (recorded as open **A1-3b** — its own
reviewable batch, not executed). The 44 − 7 NO-TOUCH skips
(response_state_sql ×3 per owner, db.rs open_with_migrations,
ai_center M009_SQL + copilot_tool_allowlist, migrations/intelligence
M014_SQL) − 1 universal_search = 36 de-pubs + 4 sibling cfg(test) lib
mods = 40 visibility edits. **kb2 correction disclosed: the kb2 record
claimed universal_search deleted but the 59f981d diff never contained
it — kb3 completed the disposition** (universal_search + cap_query +
MAX_SEARCH_QUERY_LENGTH + SearchResult, 39 lines — the return-struct
orphan the original audit also missed). The compiler re-proof then
showed every de-pub'd item except l2_norm is FULLY dead: **34 items
deleted** (incl. the l2_norm cascade and CapacityMetrics) and
**mirror_readouts deleted whole** (291 lines — RF-8's parked decision,
now compiler-proven; its exactly-2 own tests died with it, reconciling
core lib 489 → 487). M007_SQL (an unregistered never-executed
duplicate kb2's list missed) deleted — apply_m007's inline SQL and all
migration semantics untouched. The 6 cfg(test) lib mods are
alive-and-now-private. No allow() stamps anywhere.

A1-7: census delivered, zero deletions. 14 concrete keys, all with
live readers and zero production writers: KEEP documented
(lmstudio_timeout_ms/lmstudio_concurrency — README + USER_MANUAL
"Request budget"; ai_evaluation_mode — USER_MANUAL:871 + test
coverage), KEEP tested (redaction_enabled — two tests exercise the
disabled path), KEEP hidden-lever (ops_waiting_threshold_minutes,
freshness ×3, sweep ×3, backup_interval_hours, notifications.{type}
.enabled), DEFER (retention_days — NO-TOUCH maintenance.rs coupling).
The settings table is the app's documented operator interface; these
are running knobs, not dead code. A1-6 residue re-verified inert.

Incident disclosed: the region-deleter's ()/{} conflation left 6
orphaned fn bodies — caught by the immediate check, repaired by hand,
re-proven. The `#ust_use]` scare was a display-pipeline artifact
(files byte-correct).

Verification, all green: fmt clean; clippy --workspace --all-targets
-D warnings 0; gate_traceability OK (23 routes / 20 DC tags / 27 ids,
identical); gate_no_simulation clean; verify-icons clean; core lib
487/0/4ig (--test-threads=1, 346.02s — 489 minus exactly the 2 dead
mirror_readouts tests); app-shell lib 242/0; xtask 11/0; doc-tests
0/0; kittest 24 suites / 185 passed / 0 failed — 171 goldens ZERO
drift (no UI changes). Churn: +7/−670 across 18 files. Environment:
5th sandbox reset — repo/toolchain/Mesa re-provisioned from a fresh
clone @ `59f981d`.

---

## kb4 — A1-3b visibility tranche + A1-7 documentation (2026-11-02)

Owner-delegated "best course of action" batch: BOTH recorded open
items closed. (1) **A1-3b**: 447 pub→pub(crate) visibility edits
across 99 source files (68 core + 31 app-shell) — the one-reviewable-
tranche slice of kb3's 1,134-item census; zero deletions, zero
signature changes, migrations/SQL semantics untouched, NO-TOUCH
modules diff-verified absent. Diff purity proven line-by-line (the
only non-visibility lines are the manual section + rustfmt reflows of
widened signatures). Residual census RE-DERIVED at the post-tranche
tree: 651 candidates remain (472 core + 170 app-shell + 9 catalog —
the latter exactly the enums orphaned by kb2's re-export deletions) +
41 NO-TOUCH sites out of scope; 447 + 651 ≈ kb3's 1,134 within
name-based noise. (2) **A1-7**: USER_MANUAL §13.1 "Settings-table
levers" documents all 9 hidden-lever keys + retention_days (DEFER
resolved as documented) with defaults, ranges, read-timing — every
claim verified against the readers first; verification fixed two
imprecisions (Inbox also reads ops_waiting_threshold_minutes;
enforce_retention also prunes notifications). Verification, all
green: fmt clean; check 0 warnings; clippy -D warnings 0;
gate_traceability identical (23 routes / 20 DC / 27 ids);
gate_no_simulation clean; verify-icons clean; core lib 487/0/4ig
(345.74s); app-shell lib 242/0; xtask 11/0; doc-tests 0/0; kittest 24
unique targets / 175 unique tests / 0 failed (batch A 13/93 + batch B
12/89 with the live_chats overlap) — 171 goldens ZERO drift. RECORD
CORRECTION: kb3's "185 tests" double-counted the live_chats overlap;
canonical is 175 = 173 + 2, re-proven. Churn: 100 files, +541/−447.
Open: A1-3b remainder (651, re-censused), response_state_sql.rs 3 fns
(KEEP). ai records synced.

---

## kb5 — A1-3b completion: 649 narrows + compiler-certified deletions (2026-11-02)

The A1-3b remainder executed to completion per the kb3 playbook: 649
pub→pub(crate) narrows (incl. 17 `pub async fn` census blind-spot
items), then the compiler re-proof at scale — confirming kb4's
finding that most census "consumers" were mentions inside other dead
code. Deleted as compiler-certified: the entire encrypted_sync bundle
surface (1,047 lines → single live fn), the security HTML-sanitizer
surface, 9 catalog enums orphaned by kb2 (workspace.rs died whole),
notification_prefs, 3 side_threads row structs, vectorstore dead
trait methods + 4 test-only-consumer tests, the crypto scrypt wrapper
(its 4 tests rewritten to fixed keys — they cover LIVE encrypt/
decrypt), and ~20 further dead items + 7 write-never-read in-memory
fields. Net: 112 files, +542/−2,816. Three census blind spots closed
(pub async fn; mixed-name re-export lines; variant-payload types).
THREE self-inflicted NO-TOUCH violations (response_state_sql mod
re-narrowed by the applier + one fn region cut once) self-caught and
restored byte-identical, disclosed in full. Verification all green:
check/clippy 0, gates identical, core lib 467/0/3 (census reconciles
exactly: −20/−1 = 21 deleted tests), app-shell 241/0, xtask 11/0,
kittest 24 targets / 175 unique / 0 failed, 171 goldens ZERO drift.
Residual for kb6: never-read DB-adjacent struct fields (field surgery
per kb2 precedent) + re-narrowing their reverted struct headers. ai
records synced.

---

## kb6 — never-read DB-adjacent field surgery; the engagement's last open item closed (2026-11-03)

The KB-5-5 residual executed: the census re-derived at HEAD with the
compiler made authoritative (7 candidate headers narrowed to
pub(crate) — all verified zero external consumers / zero serde paths;
OverrideRow and SqliteVecStore excluded with reasons — then lib-only
cargo check fired exactly 6 dead_code warnings). Deleted:
RuleRecord.name/priority/last_run_at/run_count (SELECT re-shifted to
6 columns), RunRecord whole (all 6 fields never read — struct +
read + RUN_SELECT + readback died; fire_trigger_for_conversation now
returns Result<()>; its production caller and tests check is_ok
only), ClaimedJob.attempts/max_attempts (claim SELECTs re-shifted,
attempt kept for the claim UPDATE), WebhookEventRecord
.event_id/.processing_state (pending-events SELECT re-shifted to
3 columns), StoredRecommendation.prompt_version/model,
StoredCurrent.analysis_version (SELECT re-indexed); plus
ValidationIssue whole (zero uses anywhere) and two empty impl
blocks. Two KB-5-5 labels — "ConversationChunk" and "AiRunLog" —
proven phantom census display names with no code counterpart at HEAD
(strings entered history only through kb5's own record diff); closed
as artifacts. Runtime SELECT column lists re-shifted per the kb2
precedent; migrations SQL semantics untouched; NO-TOUCH modules
never in the diff; one test-only read (RuleRecord.name lookup)
rewritten preserving its live coverage (parse survival + legacy
trigger mapping). The six surviving headers now pub(crate) — the
A1-3b direction completed for the last held-back set. Verified all
green: fmt clean, check --workspace --all-targets 0/0, clippy
-D warnings 0, gate_traceability identical (23/20/27),
no_simulation clean, verify-icons clean, core lib 467/0/3
(--test-threads=1, 318.82s — identical baseline, zero test deaths,
census reconciles), app-shell 241/0, xtask 11/0, doc-tests 0/0,
kittest batch A 13/93 + batch B 11/82 = 24 unique targets / 175
unique tests / 0 failed — 171 goldens ZERO drift (alphabetical
13+11 split, no overlap run). Churn: 4 files, +27/−144 (+2 ai
records). Engagement state: NO open items remain anywhere —
response_state_sql 3 fns KEEP by owner disposition; plan.md's
programs closed; kb2..kb6 residuals all consumed. ai records synced.

## kb7 — independent closeout audit: zero live candidates survive (2026-11-03)

Fresh census at HEAD (e5baba2) with the corpus corrected to include
integration tests and the bin target as external consumers: 229 candidates,
2 excluded by standing disposition (response_state_sql mod decl, M014_SQL),
227 applied, 138 reverted during the private_interfaces convergence
(3 rounds: 91 → 41 → 5 — the kb5 layered-constraint mechanism: signature
constraints hold a pub face exactly as wide as the compiler demands), then
the dead_code cascade (rounds 4–13) executed the compiler-enumerated
deletions until silence. Shipped: 43 files, +226/−1980 — 42 surviving
pub→pub(crate) narrows, ~127 whole-item deletions (4 of them whole test
mods) incl. custom_objects patch/read surface, people_store
create/update-customer surface, customers search/get, jobs QueueJob/
list_jobs/get_job/status, connectors redacted_auth/redacted/validate_config,
ai_attributes MatchOp, support_graph subgraph_json/HumanEdge, security
validate_config, settings secret_or_error, translation detect, mentions
Mention, inbox ThreadType, copilot ToolResult, automation SqlValue,
intelligence_features IncidentPatch, digest digest_by_id, helpscout
revoke/has_stored_tokens, ai_pipeline dead-field surgery (CachedRun,
LatestAnalysis + its ai_sources readback, AnalyzeOutcome), conversation_ops
ConvRef → conv_exists, and the kb6-style dead-field SELECT re-shifts
(AttachmentRow, CustomerConversation.thread_count, OutboundJob.max_attempts,
ClusterRow/KnownIssueRow ids, ApprovalRow.status); test-only helpers
(build_sync_engine, compute_signature, pending_event_count, the workers_host
method) gated #[cfg(test)] instead of deleted — their coverage stays.
16 dead-subject tests died with their surfaces (every subject verified
compiler-certified dead, every live sibling retains coverage); readback
probes rewritten as direct SQL or local cfg(test) helpers (digest_row),
so no live assertion was lost — the dropped !outcome.cached pair is
redundant with the surviving audit96 non-caching tests. NO-TOUCH modules
never in the diff; no allow() stamps; no schema/migration SQL anywhere.
Verified all green: fmt clean; check --workspace --all-targets 0/0
(zero diagnostics); clippy -D warnings 0; gate_traceability identical
(23/20/27); gate_no_simulation clean; verify-icons clean; core lib
452/0/3 (−15 = exactly the enumerated dead-subject tests), app-shell
240/0 (−1 health_badge_colors_map), xtask 11/0, doc-tests 0/0; kittest
24 unique targets / 175 unique tests / 0 failed — 171 goldens ZERO drift,
zero snapshot artifacts. Environment incidents disclosed: two disk-full
events (one poisoned a core test run — all 35 failures ENOSPC artifacts,
clean re-run 452/0/3; one broke the first kittest build — linker
bus-error, the kb2 signature); this sandbox lacked the Mesa env (first
kittest attempts: "No adapter found" ×56 — fixed by sourcing
/home/z/mesa-env.sh); direct bin execution exposed two recipe gaps,
both self-caught with zero tree impact (kittest snapshot dir is
CWD-relative — root-level .new.png pollution deleted, re-runs from the
package cwd green; CARGO_INCREMENTAL=0 rehashes local crates — reverted).
Engagement state: A1-3b complete end-to-end (kb3 census → kb4 tranche →
kb5 completion → kb6 field surgery → kb7 independent re-derivation finds
zero surviving live candidates); no open items remain anywhere in the
ledger. ai records synced.

### Batch REL-1 — 2026-11-03 (release commit `a53273d`, tag `v1.9.2`, Release run `41731702400`: success)

**Owner-directed Build & Release — SupportOS+ 1.9.2 published.** The repo was declared
production-ready on 2026-11-03 (honest answer: yes, with two owner-accepted reservations
disclosed at the time — G4 blind-test 5/10 against the ≥9/10 preregistered line, owner
exemption on record; SUS 85.0 n=1; no external security audit). The owner then issued the
eight-point Build & Release work order; REL-1 executed it against the terminal source
`88830e4` (kb7's zero-survivor state), the release content differing only by the version
bump, CHANGELOG section and ledger entries (commit `a53273d` = tag `v1.9.2` target = main
HEAD).

**REL-1-1 — release prep (commit `a53273d`)**: workspace version 1.9.1 → 1.9.2
(Cargo.toml + Cargo.lock, all four members); CHANGELOG `## [1.9.2] — 2026-11-03`
(Fixed: the two KB-1 user-visible production defects — the empty-state per-frame re-query
on Inbox/Customers/Knowledge/Custom Objects, and the Settings→LM Studio dirty guard;
Changed: the internal hardening program summarized); plan.md REL-1 opener, progress.md
REL-1-1..6, decisions.md D8.

**REL-1-2 — full gate set on the exact release content (local)**: fmt clean;
`check --workspace --all-targets` zero diagnostics; `clippy --locked -D warnings` clean
(workspace + core/sandbox feature); tests `--locked`: core 452/0/3 (--test-threads=1,
byte-identical to the kb7 baseline), app-shell 240/0, xtask 11/0, core+sandbox 460/0/3,
doc-tests 0/0; gate_no_simulation clean; gate_traceability OK (23 routes / 20 DC tags /
27 ids); verify-icons clean.

**REL-1-3/4 — rehearsal packaging + local verification (disclosed scope)**: the
release-profile LTO link OOMs in this 4 GB no-swap sandbox, so the rehearsal built at
`CARGO_PROFILE_RELEASE_LTO=off` from the locked tree with the unmodified xtask — both
packages produced and staged (AppImage 15,292,920 B; .deb 10,917,076 B; versioned names +
SHA256SUMS.txt + BUILD-INFO.txt). No-root lanes all green: dpkg -I metadata correct;
`apt-get install -s` resolves the full dependency set; payload = usr/bin + .desktop +
4 hicolor icons; ldd fully resolved; SPP_SMOKE=1 EXIT=0 from the extracted deb tree
(53 migrations, boot services, workers, automatic backup, loopback listener with
OS-assigned-port fallback, first-run init DB + 0600 secrets.key + instance.lock, 24/24
pages = 4,220 shapes); Xvfb+lavapipe GUI rc=124 (window created, render loop alive at the
25 s TERM — the CI success criterion); AppImage APPIMAGE_EXTRACT_AND_RUN smoke EXIT=0 +
Xvfb GUI rc=124. The root-requiring lanes (real APT install with dependency fetch,
desktop-file-validate, reinstall, uninstall data-retention, purge, 0.2.x upgrade
migration) ran only in CI's verify-deb containers — sudo needs a password in this
sandbox; disclosed, not skipped.

**REL-1-5 — publish (Release run `41731702400`, 32.1 min, 6/6 jobs success)**: the owner
re-supplied the token (D7/D8 credential gate closed); pushed `main` (88830e4→a53273d) and
the annotated tag `v1.9.2` (0b572c9). The unmodified release.yml executed end-to-end:
build+test+package on ubuntu:22.04 (glibc 2.35 baseline, pinned profile, full workspace
suite incl. the sandbox feature); verify .deb on ubuntu:22.04, ubuntu:24.04, debian:12
(APT dependency resolution + real install, desktop entry + icons, boot smoke, 0.2.x→1.x
upgrade migration with data preserved, GUI under Xvfb with only declared dependencies,
reinstall, uninstall, purge); verify AppImage (smoke + Xvfb GUI); publish. Companion:
CI #212 on `a53273d` (main) — success.

**Published release — https://github.com/kimpearce888/SupportOS-Plus/releases/tag/v1.9.2**
(draft=false, prerelease=false, published 2026-11-03T23:04:22Z):

| Asset | Size | SHA-256 (shipped bytes) |
|---|---|---|
| `SupportOS-Plus-1.9.2-x86_64.AppImage` | 12,777,976 B | `a4dc96a1382dbea5e5808ace584bf7012174d56ae9b10ac8b3d2edaa67726a79` |
| `supportos-plus_1.9.2_amd64.deb` | 10,165,298 B | `7ee1e689f4678f39b0d2ca915693038c25a92cc1670f392a811c3cd9bb08923d` |
| `SHA256SUMS.txt` | 200 B | covers both packages |
| `BUILD-INFO.txt` | 187 B | version 1.9.2, commit `a53273d`, ubuntu:22.04 container, glibc 2.35 baseline |

**Release integrity, independently re-verified (2026-11-03)**: all four assets downloaded
live from the published URLs; `sha256sum -c SHA256SUMS.txt` → both packages OK (the
hashes above are the shipped bytes, not rehearsal bytes); BUILD-INFO commit = `a53273d` =
tag target = main HEAD (the publish job refuses to ship on mismatch); the CI-built .deb
lints correct (Package supportos-plus, Version 1.9.2, amd64, full Depends stack,
Homepage); release notes carry the 1.9.2 changelog, install instructions (APT one-liner +
AppImage chmod +x), and system requirements (x86_64; glibc 2.35+ = Ubuntu 22.04 LTS+ /
Debian 12+; X11/Wayland desktop session; ~200 MB; no compilation, dev tools or manual
setup).

**Engagement state:** REL-1 complete — the owner's Build & Release spec is satisfied
end-to-end. No open items remain in the ledger. Rule 8 STOP awaits owner direction.

### Batch AU-1 — 2026-11-03 (code-truth audit @ `49247d5`; items AU-1-1..AU-1-10 verified)

**Owner directive:** program code only is the source of truth; tests and documentation are
not evidence and not consumers; find every issue, bug, blocker, dead code, orphan, broken/
unwired feature, everything not done and due; repo files only (GitHub); follow rule.md; no
answers from memory. Evidence policy recorded as decisions.md D9.

**Method.** (1) Corpus pinned: 166 non-test program files in crates/{core,app-shell,catalog}/src
(bench/it_*/mockserver/testsupport verified #[cfg(test)]-gated; xtask excluded as tooling).
(2) Mechanical sweeps: incompleteness-marker scan (zero hard markers; 4 unreachable! all
guarded; no dead_code stamps); pub-face census — 1,874 pub items vs non-test references
after stripping cfg(test)/tooling-feature blocks (scripted, deterministic; 21 dead items;
4 import-only hits discarded as census artifacts after adjudication; 1 name-collision catch:
migrations::find inflated by Iterator::find). (3) Four independent deep-read passes (UI
wiring, services wiring, core logic, app-shell logic). (4) Adjudication: every agent finding
re-verified by me at file:line; conflicts resolved by reading the tree — two material
corrections recorded below.

**Adjudication highlights (honesty record).** (i) My census-led reading of
`ensure_translation_schema` (zero callers) as a missing-creation blocker was OVERRULED:
migrations/db_breadth.rs:327 creates `translation_cache` at boot (m040) — the function is a
dead redundant creator, not a missing table; translation works on fresh databases.
(ii) The automation_approvals "no program INSERT" claim was challenged by two textual INSERT
sites (app-shell pages/automation.rs:769/832) — both verified #[test]-gated, so the
approval-loop severance stands.

**Confirmed findings (all at `49247d5`; classes: FD=feature-dead, BF=broken-feature/bug,
UW=unwired/not-done, D=dead-in-program, O=orphaned-creation, LH=low-confidence hygiene):**

| # | Class | Finding (file:line) |
|---|---|---|
| F1 | FD | **Automation rules can never fire/act/approve end-to-end**: trigger vocabulary mismatch (core fires new_conversation/customer_reply only, automation.rs:222-231 + execute.rs:630-642; UI creates status_changed/tag_added/sla_risk only, app-shell automation.rs:9-26; compare at automation.rs:808); action_json object-vs-array (create_rule automation.rs:92-98 vs engine as_array gate :829-831); approval loop severed (parked jobs never requeued with approved:true — automation.rs:329-349 + execute.rs:380-419; automation_approvals has no program INSERT; approve()/reject() execute nothing, automation.rs:156-176) |
| F2 | BF | digest top_tags always empty — digest.rs:266-271 selects nonexistent t.tag; error swallowed by unwrap_or_default |
| F3 | BF | Search page button renders only during the one-frame focus-loss (&&-short-circuit), search.rs:129-133 — click-to-search impossible |
| F4 | BF | Outreach campaign report renders all zeros — UI reads top-level keys, core nests under totals (outreach.rs:537-570 vs outreach/mod.rs:945/1025, send.rs:494); self-admitted in UI |
| F5 | BF | knowledge mark_reviewed/mark_verified run UPDATEs on with_read lanes (knowledge.rs:471/481; readers are full connections, db.rs:102-131) — bypasses single-writer serialization |
| F6 | BF | knowledge actions clear vectors but keep loaded=true → table blanks until an unrelated bus event (no events emitted on those paths), knowledge.rs:396-405/470-508 |
| F7 | BF | inbox select() never resets assign_pick (inbox.rs:693-720 vs combo :1515-1517 / submit :1529) → stale assignee submitted to Help Scout after switching conversations |
| F8 | BF | deleted selected conversation → detail=None + selected=Some forever → reload_detail every frame (inbox.rs:396-399/447-467) |
| F9 | BF | sync_note "syncing…" set on click, never cleared/updated on completion (dashboard.rs:523, customers.rs:336, organizations.rs:151, reports.rs:367) |
| F10 | BF | palette saved-reply href "/inbox" unmapped → navigates nowhere (palette.rs:45-62; core emits it, search/lanes.rs:358) |
| F11 | BF | palette mapped branches discard record ids — deep links land on previous selection (palette.rs:47-61; NavRequest has no record-selection variant, app/mod.rs:203-222) |
| F12 | BF | Search-page hit cards are display-only; hit.href never read (search.rs:211-248) |
| F13 | BF | ai_center "(unset)" role row writes a throwaway temp — roles can never be unrouted (:1325); probe runtime failure leaves "probing…" forever (:1561-1564); hub role edits lost on tab switch (:1366-1391) |
| F14 | BF | mention_caret stale byte offset after out-of-band composer body replacement → char-boundary panic risk in the render path (mention_popup.rs:51/95, inbox.rs:2338/2307/2389) |
| F15 | BF | connectors::refresh emits no event and the page latch stays loaded → rows stale until restart without HS connection (connectors.rs:328 area, app-shell connectors.rs:207-224) |
| F16 | BF | settings message/error set in ~15 places, never cleared (settings.rs whole file) |
| F17 | BF | disconnect_helpscout leaves the in-session provider handle live → 401 loops after explicit disconnect (settings.rs:654-668 vs :854-882) |
| F18 | BF | workspace reset leaves workers running; auto-sync re-pulls within a minute while the message claims next-start (settings.rs:884-913 vs workers/mod.rs:213-228) |
| F19 | UW | webhook-health job error path emits no event → settings wh_outcome stale after failed "check now" (execute.rs:2305-2321); success/healed paths do emit |
| F20 | UW | version_check is manual-only (sole caller settings.rs:1646); html_url/published_at parsed, never displayed |
| F21 | UW | encrypted_sync is a name-only stub: table created at boot (mirror_parity m044), zero program readers/writers (encrypted_sync.rs whole file) |
| F22 | UW | 5 execute_job arms with no program producer: initial_sync/incremental_sync/reconciliation/rebuild_embeddings/sync.conversations (execute.rs:19/28/36/203/423; enqueue inventory exhaustive) |
| F23 | UW | api_queue snapshot() stats have no production consumer — queue pressure unobservable (api_queue.rs:28-38/102-104); queue itself fully wired |
| F24 | BF | Esc consumed by both palette and confirm dialog; ⌘K active over a live destructive confirm; confirm dialog non-modal, second Confirm request silently overwrites (palette.rs:112-123, overlays.rs:119-123, app/mod.rs:216-220) |
| D1-D21 | D | Dead-in-program pub items (0 non-test refs, census): db.rs open_with_migrations/reader_pool_size/is_shared_with; audit.rs with_before_state/with_conversation_id (before-state + conversation correlation never recorded); ai_center.rs M009_SQL + copilot_tool_allowlist; migrations/intelligence.rs M014_SQL; migrations.rs find; response_state_sql.rs count_by_response_state/count_conversations_with_state/count_conversations_with_state_in_mailbox; translation.rs ensure_translation_schema; vectorstore_sqlitevec.rs count_by_entity (cascades count_by_entity_type to test-only); outreach/mod.rs is_on_dnc (DNC itself honored via inline SQL, outreach/mod.rs:415/1289); app-shell pages/ai_center.rs attribute_catalog; design/mod.rs message_card/quoted_history/file_chip/cop_similar (DC-tagged components never rendered — the composer consequently has no attachment UI); pages/support_graph.rs kittest_seed_search (ungated test-seed helper) |
| O1 | O | 16 created-never-used tables: state_transitions, side_thread_messages, side_thread_mentions (+side_threads never written → support_graph reads permanently empty), verified_drafts, interaction_signals (table; live module writes client_current_signals), campaigns, campaign_recipients (superseded by outreach_*), support_cases, known_issue_refs, thread_participants, notification_prefs, knowledge_doc_usage, report_snapshots, daily_metrics, release_events, routing_configurations, encrypted_sync_log |
| LH1-LH6 | LH | Low-confidence, verify-then-fix: quality day-window mixed timestamp formats (quality/mod.rs:203-212); SLA string-MAX over mixed-precision timestamps (sla.rs:512-519/668-676); unknown automation action kinds report success (automation.rs:415/869-877); webhook redelivery dedupe ignores stored state (webhook.rs:81-90); jobs claim UPDATE lacks status guard (jobs.rs:106-114; unreachable under single writer); maintenance ?-chain places backups after trend/retention — future failures there silently disable backups (workers/mod.rs:897-1035) |

Also recorded (cosmetic/hygiene, not itemized above): boot log misstates the timer set
("webhook, embedding timers" — main.rs:68); workers_host restart() drops the JoinHandle;
Shell-page stale self-declared hint text (app/mod.rs:358-364); dead bindings app/mod.rs:100/654;
inbox "Review draft" chip silent no-op when composer empty (:2236-2249); draft-saved label
asserts a persistence that does not exist (composer drafts are in-memory only, inbox.rs:2358-2364);
"knowledge_doc_usage" style minor reads; ai_evidence table read-guarded but never created
(knowledge_store.rs:341-350 — freshness "used_in" always 0); settings restore/reset run
blocking IO on the render thread.

**Verification (Rule 5).** Every finding above carries file:line evidence re-read in the tree
at `49247d5` during adjudication (not accepted from agent prose alone); the census is
deterministic and re-runnable; the four deep-read passes covered all 24 pages + shell/palette/
overlays + the full boot/service matrix (bootstrap → BOOT_STEPS v3-v53 + runtime guards →
workers → event bus symmetry, all WIRED except the items listed) — per-file coverage tables
and event→invalidation maps are in the batch working notes; positive space confirmed along
the way: settings persistence verified saved AND consumed key-by-key, all 7 ServerEvent
variants published AND consumed, all schema tables wired except the 16 orphans + ai_evidence,
SQL arity sweep found no placeholder/column-index mismatches.

**Remaining risk (scope honesty).** ~125K LOC: mechanical sweeps complete; semantic passes
broad but not a line-by-line absence proof; spot-check-only areas: sync_engine/loops.rs thread
backfill, people_store/health aggregates, segment/eval.rs internals, embeddings write-back,
notification_sweep beyond approvals. These carry forward as standing risk, not open items.

**Batch AU-2 (items AU2-1..AU2-15) written to plan.md** — the confirmed findings queued for
fix batches (ten at a time per rule.md, on the owner's `continue`).

**Engagement state:** Batch AU-1 complete; ledger synced; Rule 8 STOP.
## Batch AU-2 — confirmed AU-1 findings, fix wave 1 (items AU2-1..AU2-10 completed and verified, 2026-11-04)

Source: Batch AU-1 confirmed findings. Rule 2 move recorded in progress.md (2026-11-04,
owner `continue`). The AU2-1 owner-decision gate was taken in-code per the standing fix
directive and recorded as **D10** (decisions.md, 2026-11-04): trigger vocabulary =
the engine's fire vocabulary (`new_conversation`, `customer_reply`); actions limited to
engine-executable kinds (SetPriority dropped, `send_note` wired through the outbound
`create_note` lane at higher_risk); engine canonicalizes both stored action shapes;
approval model = parked job + `automation_approvals` row, approve() requeues the job
with `approved:true`, reject() deletes it. Program code only as evidence (D9).

1. **AU2-1 (automation dead end-to-end)** — FIXED at all three breaks.
   (a) triggers: core `Trigger` gains `NewConversation`/`CustomerReply`
   (automation.rs; parse_trigger_json maps both), creation form offers exactly those
   two (pages/automation.rs TriggerKind::ALL); legacy triggers stay parseable, are no
   longer offered (no emitter exists). (b) action shape: `canonical_actions`
   (automation.rs) accepts the M007 single object (lifts top-level fields into
   `params`, normalizes `change_status`→`set_status`) and the engine-native array;
   fire loop dispatches canonical actions; unknown kinds record an honest
   "no engine handler — skipped" run instead of a fake "completed".
   (c) approval loop: `enqueue_awaiting_approval` now ALSO inserts a pending
   `automation_approvals` row (new `job_id` column via ensure_main_rule_columns);
   `approve()` flips status and requeues the parked job with `approved:true`
   (`requeue_awaiting_job`); `reject()` flips status and deletes the parked job; the
   existing worker branch then executes via `execute_approved_action` and records the
   approved run. `send_note` executes through `create_outbound_job_keyed("create_note")`.
2. **AU2-2 (digest top_tags always empty)** — digest.rs topic_facts joins
   `tags t ON t.id = ct.tag_id` and selects `t.name` (shape proven at
   issue_impact.rs:150-162); `top_tags` now populated in weekly digests.
3. **AU2-3 (Search page button unclickable)** — search.rs renders the search button
   unconditionally (`search_clicked || (lost_focus && enter)`); the old
   `lost_focus() && button(...)` short-circuit existed only on the one focus-loss frame.
4. **AU2-4 (mention caret panic risk)** — mention_popup.rs gains
   `floor_char_boundary`; `caret_query`/`apply_completion` snap stale byte offsets to
   char boundaries (no more mid-char slice panics); inbox.rs resets
   `mention_caret`/`mention_active` on every out-of-band body replacement (select
   stash/clear, copilot insert, LM-draft insert, insert_ai_draft both arms, AI
   transform, saved-reply insert, Send) and after each @-completion.
5. **AU2-5 (knowledge read-lane writes + latch blanking)** — mark_reviewed /
   mark_verified moved to `db.with_write` (serialized writer); mark_reviewed /
   mark_verified / delete_document / both import paths refetch via `reload_all`
   instead of clearing vectors under a true `loaded` latch.
6. **AU2-6 (inbox stale assignee + deleted-selection frame loop)** — select() resets
   `assign_pick` on switch; reload_detail Ok(None) is terminal (clears selected +
   assign_pick, sets an explanatory status message); the ensure_loaded refetch gate
   also honors `detail_error` so persistent read errors cannot spin per-frame.
7. **AU2-7 (deep links broken three ways)** — NavRequest gains
   OpenConversation/OpenCustomer/OpenKnowledgeDoc/OpenHelpArticle/OpenKnownIssues;
   href_to_nav maps bare `/inbox` (saved replies), carries record ids for
   `/inbox/conversation/{id}`, `/customers/{id}`, `/knowledge?doc={id}`,
   `/knowledge/help-center/{id}`, `/issues?tab=known`; apply_nav routes each to the
   page's record view (inbox select, customer open_profile, knowledge
   go_to_documents/open_help_article, issue radar Known tab); Search hit cards are
   clickable (interact on the card rect, pointing-hand cursor) and route through the
   same href_to_nav mapping. Disclosed limitation: `/knowledge?doc={id}` lands on the
   Documents tab — knowledge_documents have no per-document reader in the program.
8. **AU2-8 (ai_center role-routing defects)** — the "(unset)" combo row now really
   clears the role (`roles.set(role, None)`; the old row wrote into a throwaway
   `None::<usize>`); hub model edits commit per change (lost_focus-only commits were
   dropped by tab/page switches) with the golden_eval job coalesced via new
   `jobs::has_pending`; a failed probe runtime posts "probe failed — runtime
   unavailable" into the slot instead of panicking the thread (mutex-poison
   tolerant delivery) so "probing…" can no longer stick forever.
9. **AU2-9 (sync_note stuck + operations spinner loop)** — SyncCompleted updates the
   note on all four pages (dashboard/customers/organizations/reports `sync_finished`)
   with the outcome; operations reload arms a 10 s backoff on snapshot failure
   (`failed_reload_at`), cleared on invalidate/success — no more per-frame re-query.
10. **AU2-10 (outreach report all zeros)** — report_ui reads `report["totals"]`
    (recipients/sent/failed/skipped/cancelled/replied/unknown + reply_rate, the
    producer's real keys) and shows the producer's own data-source note; the
    self-admitting bug callout is gone.

**Verification (Rule 5)** — all gates re-run at the working tree, all green, all
matching the v1.9.2 release baseline: `cargo check --workspace --all-targets` clean;
`cargo fmt --all` applied / `--check` clean; `cargo clippy --workspace --all-targets
-- -D warnings` zero diagnostics; core lib **452 passed / 0 failed / 3 ignored**;
app-shell **240 passed / 0 failed**; xtask **11 passed / 0 failed**; catalog no tests;
core doc-tests **0 / 0**. Local-environment disclosures: 4 GB no-swap sandbox (the
known LTO link constraint, lto=off user-config for local runs only — repo files
untouched), one doc-test rerun after /tmp StorageFull (environmental, passed after
freeing space). No new test code written (owner standing rule); behavior-level
confidence for the AU2-1 approval loop rests on the existing worker branch
(execute_approved_action + record_approved_run, covered by core batch19 tests) plus
the schema/SQL review of the new requeue path recorded here.

Rule 8 STOP — remaining Batch AU-2 items AU2-11..AU2-15 stay queued in plan.md.

## Batch AU-2 — confirmed AU-1 findings, fix wave 2 (items AU2-11..AU2-15 completed and verified, 2026-11-04)

The final five AU-2 items, verified at the wave-2 tree with program-code-only
evidence (D9 policy). Owner decisions carried inside AU2-13/AU2-15 were taken
in-code per the standing fix directive and recorded as decisions.md D11.

1. **AU2-11 (settings page feedback lifecycle)** — tab switches now clear
   `message`/`error` (feedback belongs to the tab that produced it) and a
   `dismiss` button clears them in place, so stale feedback can no longer glue
   above every tab until restart; `disconnect_helpscout` now deactivates the
   in-session provider handle (same `on_provider_kind_transition` +
   `provider_kind=empty` transition as `clear_helpscout_credentials`) — no more
   queued calls firing 401s after an explicit disconnect;
   `reset_workspace` pauses the WorkersHost BEFORE the wipe (new synchronous
   `WorkersHost::pause`), and the message now says background sync is paused
   until restart instead of falsely promising the re-pull happens on next
   start; the webhook-health job's failure path now emits `SyncCompleted`
   (same refresh signal as the non-healed success path) so Settings
   `wh_outcome` / SyncHealth reload the fresh outcome after a failed
   "check now".
2. **AU2-12 (palette/overlay input arbitration)** — a live confirm dialog now
   owns the keyboard: `palette.ui` takes `input_locked` and closes itself
   without consuming anything when a destructive confirm is armed; arming a
   confirm closes the palette and the ⌘K palette-trigger + Search buttons are
   gated while the dialog is up; a second `NavRequest::Confirm` while one is
   armed is rejected with a warn log instead of silently overwriting the
   pending action. One Esc can no longer close the palette AND cancel the
   armed confirm (the palette is closed and locked before the dialog can see
   a key).
3. **AU2-13 (AU-1 dead-surface census, disposition per group — D11)** —
   (a) dead items deleted: db.rs `open_with_migrations`/`reader_pool_size`/
   `is_shared_with`; audit.rs `with_before_state`/`with_conversation_id`
   (their unit/fixture seeds moved to the program's own `jobs::audit`
   conversation-scoped API); ai_center.rs `M009_SQL` +
   `copilot_tool_allowlist`; migrations/intelligence.rs `M014_SQL` const
   (the real `apply_m014` stays); migrations.rs `find`;
   response_state_sql.rs count trio; translation.rs
   `ensure_translation_schema` (m040 owns creation); outreach `is_on_dnc`;
   app-shell `attribute_catalog`; support_graph `kittest_seed_search`
   (gated to `kittest-ui` per the census note — release builds carry no test
   surface); design/mod.rs `message_card`/`quoted_history`/`file_chip`/
   `cop_similar` deleted (`message_card_with` made pub — the kittest identity
   fixture now drives the exact form inbox/live_chats use, body rendering
   byte-identical, so only the removed dead widgets change the frame);
   vectorstore `count_by_entity` + cascade `count_by_entity_type` deleted —
   the per-entity count now lives where it is actually used: xtask
   `vector-count` reads `vec_payload` directly (a plain table) and the store
   tests assert against the store file itself; (b) the five producer-less
   execute_job arms (`initial_sync`/`incremental_sync`/`reconciliation`/
   `rebuild_embeddings`/`sync.conversations`) deleted; the post-initial-sync
   follow-ups now run where the program actually pulls the account first time
   (auto-sync NEW branch calls `on_after_initial_sync`); (c) the 16
   created-never-used tables (plus the never-written side_threads family,
   whose permanently-empty support_graph reads were removed with it) are
   dropped by new boot step m063 `apply_m063_drop_orphan_tables`
   (schema_version 54) so upgraded DBs and fresh installs converge;
   `data_reset` KEEP_TABLES no longer names dropped tables; (d)
   encrypted_sync.rs name-only stub deleted (module removed; m044 stops
   creating its log table); (e) api_queue write-only stats counters +
   consumer-less `snapshot()` deleted; (f) boot log now names the real
   WorkerManager timer set; (g) `workers_host::restart` retains its task
   JoinHandle (panics surfaced, aborted on shutdown) and serializes
   overlapping restarts through a gate.
4. **AU2-14 (low-confidence hygiene, verified-then-fixed)** — quality
   day-windows compare on the same format (`strftime('%Y-%m-%dT%H:%M:%fZ')`
   bounds, three sites); SLA picks the last customer message with
   `MAX(strftime(...))` / `ORDER BY strftime(...)` instead of string MAX over
   mixed precision (two sites); `jobs::claim_next` guards its UPDATE on
   `status = 'queued'` (invariant made local; unreachable under the
   single-writer lane); webhook redelivery of an event whose retry budget is
   exhausted (state=failed, attempts>=5) resets the budget and requeues
   instead of being swallowed as a duplicate; the 6-hourly maintenance chain
   now runs the backup block FIRST and demoted `compute_trends` to a
   non-fatal warn — no future failure up the chain can silently disable
   backups again.
5. **AU2-15 (version_check manual-only — owner call, D11)** — manual-only is
   the intended design: the module documents it in code (the sole program
   caller is the Settings "check now" thread; no boot/periodic check, so an
   offline machine never phones GitHub unprompted) and `ReleaseInfo` now
   parses only `tag_name` — the never-read `name`/`published_at`/`html_url`
   deserialization surface is gone.

**Verification (Rule 5)** — all gates re-run at the wave-2 tree, all green:
`cargo check --workspace --all-targets` clean; `cargo fmt --all` applied /
`--check` clean; `cargo clippy --workspace --all-targets -- -D warnings` zero
diagnostics; `cargo clippy -p supportos-plus-core --all-targets --features
sandbox -- -D warnings` zero diagnostics; core **452/0/3**; core+sandbox
**460/0/3**; app-shell **239/0**; xtask **11/0** (+2 elsewhere, all green);
doc-tests **0/0**. Environment disclosures: 4 GB no-swap sandbox (lto=off
user-config for local runs only — repo files untouched); one disk-full event
(`No space left on device`) resolved by clearing the local incremental
compilation cache, tests re-run green. Kittest note: the identity-frame
change from the four deleted design widgets shifts exactly two diff-locked
goldens (`r2c_inbox_identity_light/dark`); this sandbox has no Vulkan ICD and
no root, so the baselines are regenerated CI-side (lavapipe, the same
rasterizer family that produced them) via a one-shot UPDATE_SNAPSHOTS run
recorded in D11; the final HEAD's kittest diff-lock job is the proof gate.
(Disclosure, full regen path: the first regen dispatch failed compiling the
fixture — `scale::text::LEAD` is a deliberately crate-private design token;
the fixture now pins the value locally with the diff-lock flagging any token
drift, and `check -p supportos-plus-app-shell --features kittest-ui --tests`
compiles the suites locally without a GPU. The first baselines commit then
missed the PNGs — `git add` was omitted, so 7285e8e carried only the workflow
deletion. The CI kittest job surfaced the bigger truth: it had been failing
at the automation suite since wave 1 (f526f5b red with the same seven tests —
AU2-1's form changes were never mirrored into the baselines) and cargo's
fail-fast stopped it there, so the later suites were never diff-locked at
all. A full-suite UPDATE_SNAPSHOTS=1 run re-rendered every golden: exactly
22 changed — identity x2 (the deleted design widgets, diff audited),
automation x7 (AU2-1 trigger/action vocabulary + the seeded rule's canonical
action JSON, diff audited), outreach x6 (AU2-10: the all-zeros report with
its self-admitting callout replaced by the real producer totals, diff
audited), search x7 (the per-frame search button fix, diff audited) — and
the other ~149 goldens came back byte-identical, confirming the rendering is
deterministic across runner instances and nothing else drifted.)

Rule 8 STOP — Batch AU-2 is drained; next queued work waits on the owner's
`continue`.
### Batch REL-2 — 2026-11-04 (release commit `5b3a8a8`, tag `v1.9.3`, Release run `41883619992`: success)

**Owner-directed Build & Release — SupportOS+ 1.9.3 published from the final
source.** The owner's work order (build, test, publish install-ready Linux
`.deb` + `.AppImage` from the final repository source; GitHub Actions; clean-
environment verification; release integrity; no unverified claims) is closed
with the following evidence.

**REL-2-1 — release prep (commit `5b3a8a8`)**: workspace version 1.9.2 → 1.9.3
(Cargo.toml + Cargo.lock; lock verified: 578 packages, only the four member
versions changed, zero dependency drift); CHANGELOG `## [1.9.3] — 2026-11-04`
in product language (the fifteen AU-2 fixes, dead-surface removal, manual-only
version check); decisions.md D12 (version policy); progress.md Rule-2 opener.
Content rationale: final source `4a8db7e` carries Batch AU-1 + AU-2 waves 1–2
and the relocked kittest baselines — none of it was in any published release
(v1.9.2 shipped `a53273d`, 11 commits back).

**REL-2-2 — gates at the exact tagged content (local, 4 GB no-swap recipe)**:
fmt --all --check clean; check --locked --workspace --all-targets zero
diagnostics (2m31s); clippy --locked -D warnings zero (workspace 31s; core +
sandbox feature 41s); tests --locked --test-threads=1: app-shell **239/0**
(161s), catalog 0/0, core **452/0/3** (311s), core+sandbox **460/0/3**
(311s), xtask **11/0**, doc-tests **0/0**; gate_no_simulation clean;
gate_traceability OK (23 routes / 20 DC tags / 27 ids); verify-icons clean.
Disclosures: the disk-full remedy from the wave-2 record repeated (Phase-A
target cleanup → full test-profile rebuild); one environment SIGKILL of a
detached full-suite run (silent, mid-suite — the same watchdog class as the
no-root sandbox has shown before) → resumed per-crate in foreground, every
phase green; CI run #225 on `5b3a8a8` (incl. the kittest lavapipe diff-lock
lane) independently **success**.

**REL-2-3 — publish (Release run `41883619992`, run 32: success, 33.5 min,
6/6 jobs)** — the unmodified release.yml on tag push: build + full gate set +
package in an ubuntu:22.04 container (glibc 2.35 baseline); verify-deb matrix
green on **ubuntu:22.04, ubuntu:24.04, debian:12** (APT dependency
resolution, package metadata, desktop entry + icons, desktop-file-validate,
SPP_SMOKE boot chain incl. first-run initialization, 0.2.x → 1.x data-
directory upgrade migration, Xvfb GUI launch on declared dependencies only,
reinstall, uninstall with user-data retention, purge); verify-appimage green
(smoke + Xvfb GUI launch, self-contained runtime); publish-after-green with
in-workflow live link + checksum re-verification.

**REL-2-4 — post-publish verification (independent, from this sandbox)**:
published release https://github.com/kimpearce888/SupportOS-Plus/releases/tag/v1.9.3
— tag `v1.9.3`, "SupportOS+ 1.9.3", draft=false, prerelease=false,
target = `5b3a8a81abcd15c85ffc7f94fa368d5b411106ed` (== final source commit,
verified byte-for-byte against the local checkout HEAD). All four assets
downloaded live from their browser_download_url:

| Asset | Size | SHA-256 |
|---|---|---|
| `SupportOS-Plus-1.9.3-x86_64.AppImage` | 12,798,456 B | `5a97da170a570e1d7f07dea560d7ec73e4f68d708817249ebf6b322bde272cd3` |
| `supportos-plus_1.9.3_amd64.deb` | 10,184,462 B | `b9e77a04968e2ec6df74b6c61c8c691ee7ad94a6854b67274a3b23831e4e2fed` |
| `SHA256SUMS.txt` | 200 B | (checksum list itself, verified against both packages) |
| `BUILD-INFO.txt` | 187 B | version 1.9.3, commit `5b3a8a8…`, ubuntu:22.04 container (glibc 2.35 baseline) |

`sha256sum -c SHA256SUMS.txt` → both packages **OK** (checksums recomputed
from the downloaded bytes match the published list). `.deb` metadata re-read
from the downloaded artifact: Package `supportos-plus`, Version 1.9.3,
Section utils, Architecture amd64, Installed-Size 30,217 KB, full Depends
stack (libc6 >= 2.35, X11/xkb/GL/EGL/GLES/Wayland client set), Homepage,
description; payload carries `/usr/bin/supportos-plus`,
`/usr/share/applications/supportos-plus.desktop`, hicolor icons
(1024/256/128/32). AppImage re-read: ELF 64-bit x86-64 (type-2 runtime).
Release notes carry the `[1.9.3]` changelog section, install instructions
(APT one-liner / chmod+x AppImage), system requirements (x86_64, glibc 2.35+,
Ubuntu 22.04 LTS+ / Debian 12+, desktop session, ~200 MB) and the
verified-in-CI statement.

**Compatibility declaration (shipped in the release notes)**: x86_64 only;
glibc >= 2.35 (Ubuntu 22.04 LTS / 24.04 LTS, Debian 12+); X11 or Wayland
desktop; GL driver (Mesa or vendor) — the glow renderer is the default, wgpu
opt-in via `SPP_RENDERER=wgpu`; no compilation, no dev tools, no Docker, no
external services; first run initializes the SQLite store under
`$XDG_DATA_HOME/supportos-plus` by itself.

**Engagement state:** Batch REL-2 complete; ledger synced (this record +
progress.md); RS-4 remains the active queued batch (held, not started, Rule 6)
and resumes on the owner's `continue`. Rule 8 STOP.

## Batch REC-1 — ledger reconciliation: plan/progress/completed vs repo facts (facts-vs-fictions audit) — 2026-11-04

Owner directive: check plan.md, progress.md, completed.md and the current repo
state; determine what remains unfinished; separate facts from fictions (the
owner suspected the plan carried stale or outright wrong pending items).
Read-only audit: no program code touched. Evidence re-gathered from the pushed
files, git state and the live GitHub API at HEAD `c9a2289` — nothing from
memory.

**Method.** All three ledgers read in full (plan.md 611 lines; progress.md
1,940; completed.md 6,863); every active/pending/parked/held marker grepped;
every completion claim cross-checked against the completion records, the tags,
the workflow runs and the release via the GitHub API.

### Facts (verified)

1. **The work queue is empty — zero pending items anywhere.** Every program in
   the ledger is consumed and carries a completion record in completed.md:
   the v2-blueprint batches 1–15 (released v1.4.0→v1.7.0); the v1.7.0
   clean-audit items 1–165 (batches 16–36, incl. 56/150/151 wired per the
   owner's batch-36 decision); the Design-System Reskin RS-01..RS-21 + the
   RS-4 rejection wave + RS-4-11 + Gate G4 (owner verdict: accepted,
   2026-10-30) + RS-6 (v1.9.1); the refactor program RF-1..RF-11 + FINISH;
   KB-1 (both known bugs fixed), AUDIT-1, KB-2..KB-7 ("zero live candidates
   survive"); REL-1 (v1.9.2); AU-1 (10/10); AU-2 (15/15, both waves, "Batch
   AU-2 is drained"); REL-2 (v1.9.3 published and live-verified). The AU-2
   wave-2 record closes with "next queued work waits on the owner's
   `continue`" — and there is no queued work behind it.
2. **Repo state matches the records.** main = `c9a2289` = origin/main (0/0);
   workspace version 1.9.3 (Cargo.toml:11); CHANGELOG `[1.9.3] — 2026-11-04`;
   tags v1.9.0–v1.9.3 on origin (v1.9.2→`a53273d`, v1.9.3→`5b3a8a8`); latest
   release v1.9.3 live (draft=false, prerelease=false; 4 assets: AppImage,
   deb, SHA256SUMS.txt, BUILD-INFO.txt); GitHub API: Release run
   `41883619992` success, CI success on `5b3a8a8`, `4a8db7e` and the ledger
   commit `c9a2289`; decisions D1–D12 all present.
3. **AU-2 closed with full disclosure** (the kittest inherited-drift story, 22
   regenerated goldens diff-audited, ~149 byte-identical) and REL-2 closed
   with live-verified assets (`sha256sum -c` OK from downloaded bytes).

### Fictions (stale/false ledger text — the source of the "too many pending items" impression)

- **F1 — progress.md "## Active items": "Batch RS-4 (items RS-4-1..RS-4-10) —
  active".** FALSE. RS-4 completed and verified 2026-10-30 (completed.md,
  "Batch RS-4 completion record (Rule 4)" — all ten items), RS-4-11 followed
  (Batch RS-5), G4 closed accepted, and plan.md's G4-closure note states the
  reskin program fully consumed, "Nothing is open." The stale line
  propagated: the Batch REL-2 record's closing sentence ("RS-4 remains the
  active queued batch (held, not started, Rule 6)") and the `c9a2289` commit
  message repeat it. **That sentence is hereby superseded by this record:**
  no batch is active or held; nothing resumes on `continue` because nothing
  is queued. The Active-items block in progress.md is corrected in this
  commit (the only ledger edit of this batch).
- **F2 — progress.md is an archive mislabeled as an active queue.** 23 batch
  sections (RS-4, RS-6, RF-1..RF-11, KB-1, AUDIT-1, KB-2..KB-7, REL-1, AU-1;
  ~1,930 lines) sit under the title "active items", yet every one has a
  completion record in completed.md. Rules 4/7 say completed items leave
  progress.md; only KB-2..KB-6 carry header-level completion markers, so on
  a skim the file reads as a huge pending backlog. This file — not plan.md —
  is where the "too many pending items" impression comes from. The sections
  are retained for now (several batches' full item texts — e.g. the
  RF-6..RF-11 census lists — exist nowhere else); their disposition
  (explicit archive labelling vs relocation into completed.md appendices)
  awaits the owner's direction.
- **F3 — plan.md is titled "TODO" / "## Items" but holds zero open items.**
  611 lines of historical narrative; the queue-closed statements live in the
  Notes (v1.7.0 queue fully consumed), the G4 closure ("Nothing is open")
  and FINISH ("plan.md has no open items"). Misleading packaging, not false
  content.
- **F4 — plan.md's three "stays parked" notes (items 56/150/151).** Stale:
  resolved 2026-10-25 (batch 36, the owner's "wire it" decision; the Notes
  confirm). Chronologically layered text contradicts itself within one file.
- **F5 — plan.md RF section: "Known production bugs stay LIST-ONLY … never
  fixed inside this program."** True only as an RF-program scope rule; both
  bugs were fixed by KB-1 on the owner's explicit post-FINISH directive
  (2026-11-01). A skimmer would read two open bugs; there are none.
- **F6 — cosmetic trail gaps.** AU-1-9's record says the confirmed findings
  were "written into plan.md as Batch AU-2 items 1..15", but plan.md holds
  only the one-line drain header (the item texts lived transiently in
  progress.md per Rule 2 and survive in the AU-2 completion records);
  plan.md has no REL-2 section (the batch was opened directly via
  progress.md). No open work behind either gap.

### Open items

- **Work: none.** The only recorded, owner-accepted disclosures (not work
  items): the G4 blind test 5/10 (pre-registered bar not met, owner-accepted),
  SUS n=1, no external audit; AU2-7's disclosed `/knowledge?doc=` limitation
  (no per-document reader exists in the program).
- **Ledger hygiene (the sole remediation candidate, awaiting owner
  direction):** F1 is corrected in this commit; F2–F5 await the owner's
  disposition choice (archive-label or slim progress.md; annotate or
  restructure plan.md's historical narrative). No code, batch, or feature
  work is implied by any of it.

Rule 8 STOP — nothing to continue to; the next work item requires a new owner
directive.

## Batch HYG-1 — ledger hygiene: the three-file discipline restored (owner-directed, 2026-11-04)

Owner direction on the REC-1 report: **"do what is best to keep things in
order."** Scope: ledger files only (rule.md's own file contract); no program
code, no tests, no product files touched.

1. **HYG-1-1 (`ai/archive-plan.md`)** — the full historical narrative of
   plan.md (as of `6fcd8b9`, 610 lines) moved byte-identically behind a
   provenance banner with known-stale-spot notes (the three "stays parked"
   notes resolved by batch 36; the RF "LIST-ONLY" constraint superseded by
   KB-1; every "moved to progress.md" line long since completed).
2. **HYG-1-2 (`ai/archive-progress.md`)** — the 23 completed-batch
   transcripts that had accumulated in progress.md from 2026-10-30 to
   2026-11-03 (RS-4, RS-6, RF-1..RF-11, KB-1, AUDIT-1, KB-2..KB-7, REL-1,
   AU-1; 1,928 lines) moved byte-identically behind a provenance banner
   naming each section's completion record in completed.md.
3. **HYG-1-3 (`ai/plan.md`)** — rewritten as the live TODO queue: the
   authoritative queue-state section (EMPTY, all programs consumed, nothing
   pending/parked/held) + pointers to the archives, completed.md and
   decisions.md + the Rule-2/4/5 workflow note for future work.
4. **HYG-1-4 (`ai/progress.md`)** — slimmed to the active-items file only:
   the "None" state (kept from REC-1's correction), the REC-1 historical
   note, and the HYG-1 relocation pointer. 1,945 → 17 lines.
5. **HYG-1-5 (records)** — decisions.md **D13** (the restored structure +
   the standing rule that completed batches leave progress.md at the Rule 4
   sync) and this record. Rule 2 formality: opened and executed as a single
   ledger pass in one commit, per the REC-1 precedent.

**Verification (Rule 5)** — all mechanical, re-run at the working tree:
- Byte preservation: `diff` of the moved portions against their `6fcd8b9`
  sources is empty (plan.md full-file; progress.md from the "## Batch RS-4"
  header to EOF) — zero content lost, zero altered.
- Section integrity: exactly 23 `## Batch`-level section headers in
  archive-progress.md; each of the 23 cross-checked to its completion
  record in completed.md (RS-4 → "Batch RS-4 completion record (Rule 4)",
  RS-6 → Batch RS-6, RF-1..RF-11 → their Batch RF-* records + FINISH, KB-1
  → Batch KB-1, AUDIT-1 → Batch AUDIT-1, KB-2..KB-7 → their kb records,
  REL-1 → Batch REL-1, AU-1 → Batch AU-1). 23/23 present.
- Discipline restored: plan.md (20 lines) and progress.md (17 lines) contain
  no completed-batch content; the only "active" declaration in the ledger
  now reads None; completed.md remains append-only (the REL-2/REC-1 records
  untouched in place, the stale RS-4 sentence superseded by REC-1, not
  edited).
- No code touched: `git diff --stat` shows only `ai/*.md` files.

Rule 8 STOP — the ledger is in order; the next work item requires a new
owner directive.

## Batch GA-1 — api_queue decoupled from helpscout_real (the graphify-audit cycle) — completed and verified 2026-11-04

Owner directive: "yes, continue" on the graphify knowledge-graph audit report
(run on this repo at `681a1d3` with Graphify-Labs/graphify v0.9.102, code-only
AST extraction, zero API cost, outputs outside the repo). The audit surfaced
one real structural defect: a 2-file module cycle — `api_queue.rs:7` imported
`crate::helpscout_real::HsRateLimiter` while `helpscout_real/mod.rs:7` imported
`ApiQueue` back — coupling the generic priority queue (PRIORITY_*) to one
provider. Two further graph findings cleared against source before any edit
(error.rs self-loop = extractor artifact, zero self-references in source;
app/mod.rs <-> lib.rs = idiomatic `pub mod` + re-export pattern) and one minor
smell was noted and deliberately left untouched (the shared JSON field helpers
defined inside the pages/queries.rs page module, re-exported via mod.rs:352 —
an intentional, consistently-imported pattern).

**GA-1-1 (`crates/core/src/api_queue.rs` + `crates/core/src/helpscout_real/mod.rs`).**
`pub trait QueueRateLimiter: Send + Sync { fn wait_time_ms(&self, is_write: bool) -> i64 }`
defined consumer-side in api_queue.rs; the queue field and constructor now take
`Arc<dyn QueueRateLimiter>`; `HsRateLimiter` implements the trait in
helpscout_real — the inherent `wait_time_ms` body moved into the impl (the
inherent had no callers outside the queue, so a delegating inherent would have
been dead code under `-D warnings`); the `RealHelpScoutProvider::new`
construction site coerces explicitly (`Arc::clone(&limiter) as
Arc<dyn QueueRateLimiter>` — `Arc::clone`'s generic unifies with the expected
trait-object type and blocks the implicit unsize coercion; the sibling
`Arc::new(HsRateLimiter::new(None))` docs-queue call coerces implicitly).
Dependency now points one way: helpscout_real -> api_queue. Zero caller
changes elsewhere: the queue's only limiter interaction was the single
`wait_time_ms(is_write)` call in pump(), and api_queue's only importer was
helpscout_real.

**Verification (Rule 5)** — all mechanical, re-run at the working tree:
- `cargo fmt --all -- --check`: clean.
- `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- Tests, foreground per-crate, baselines exact: core 452 passed / 0 failed /
  3 ignored (incl. the full it_helpscout integration suite through the trait);
  app-shell 239 / 0 (kittest lanes are CI-only, as in REL-2); catalog 0 (no
  tests exist); xtask 11 / 0.
- Dependency check: `rg "helpscout_real" crates/core/src/api_queue.rs` hits
  doc comments only; the use-block imports std + error exclusively.
- Graphify re-verification: the knowledge graph rebuilt from the fixed tree
  (6,639 nodes / 17,813 edges / 244 communities) — the report's Import Cycles
  section no longer lists `api_queue <-> helpscout_real`; the two remaining
  entries are the pre-verified benign artifacts. Outputs in
  /tmp/spp-graph-after/ (outside the repo).
- Scope: `git diff --stat` = the two .rs files + ai/plan.md + ai/progress.md
  (+ this record). No other code, no version bump (releases remain REL's job).
- Environment note: one build attempt hit ENOSPC (the sandbox's 4 GB disk —
  target/debug/incremental at 1.1 GB); the cache was deleted, no output lost,
  gates re-run green with CARGO_INCREMENTAL=0 for the remaining suites.

Rule 8 STOP — the queue is empty; the next work item requires a new owner
directive.

**Addendum (GA-1, CI outcome, 2026-11-04):** the first push (`8b6ed28`)
failed CI's Format-check step — a process miss, disclosed: `cargo fmt
--check` had been run before the coercion edit but was not re-run after it,
so the pushed tree carried a hand-split line rustfmt rejects
(helpscout_real/mod.rs:605; CI wanted the multi-line `ApiQueue::new` arg form
after the `as Arc<dyn QueueRateLimiter>` cast). Fix commit `fa8236c` applied
`cargo fmt --all` (6 lines), re-verified fmt + clippy locally, and CI went
**green 3/3** on it (fmt/clippy/test/deny ✓ · AppImage + .deb build smoke ✓ ·
kittest snapshot locks ✓). The two test-bearing checks on `8b6ed28` had
already passed (AppImage/.deb ✓, kittest ✓) — only the format step failed.
Lesson carried into practice: every post-edit tree gets `cargo fmt --all --
--check` as the final local step before push, not an early one.

## Batch GA-2 — neutral home for the shared JSON field helpers (the graphify-audit smell) — completed and verified 2026-11-04

Owner directive: "continue" on the GA-1 closeout (the one remaining candidate
from the graphify audit). The audit's "surprising connection" flagged
`notifications.rs` tests reaching `i64_field()` in the sibling page module
`queries.rs`; source verification showed it was an intentional, consistently
imported pattern — but the helpers' *definition* lived inside a page module
while 9+ sibling pages consume them through the `pages` re-export.

**GA-2-1 (`crates/app-shell/src/pages/{json_fields.rs (new), mod.rs, queries.rs}`).**
`str_field`/`i64_field`/`opt_str_field` (17 lines, verbatim bodies) moved out
of `pages/queries.rs` (was lines 67–83, the file's tail) into the new neutral
module `pages/json_fields.rs` with module + item docs referencing GA-2.
`mod.rs` now declares `mod json_fields;` and the re-export line changed from
`pub(crate) use queries::{i64_field, opt_str_field, str_field}` to
`pub(crate) use json_fields::{...}` — **the import contract is unchanged**:
every consumer's `use super::{i64_field, opt_str_field, str_field}` (9+ pages
at top level, `use super::super::{...}` inside nested test modules) resolves
through the same re-export path and needed zero edits. The one direct-path
import (`mod.rs` tests: `use super::queries::{i64_field, invalidated_pages,
opt_str_field, str_field}`) split into `use super::json_fields::{...}` +
`use super::queries::invalidated_pages`. queries.rs itself needed no import
change (its `use super::*;` glob picks the re-exports up; it never calls the
helpers). The `value_field_helpers` test stays in `mod.rs` — it exercises
exactly the re-export consumption path. No core/catalog/xtask surface
touched (helpers are app-shell-only, verified by repo-wide rg).

**Verification (Rule 5)** — all mechanical, re-run at the working tree:
- `cargo clippy --workspace --all-targets -- -D warnings`: clean (4.92s,
  core cached — unchanged since GA-1).
- Tests, foreground per-crate, baselines exact: core 452 / 0 / 3; app-shell
  239 / 0; catalog 0 (no tests); xtask 11 / 0.
- **fmt --check as the FINAL pre-push local step** (the GA-1 standing
  practice): clean.
- Graphify re-verification: graph rebuilt from the fixed tree (6,640 nodes /
  17,813 edges / 250 communities) — all three helpers resolve to
  `crates/app-shell/src/pages/json_fields.rs` (node ids
  `crates_app_shell_src_pages_json_fields_*`), Import Cycles unchanged (the
  two pre-verified benign artifacts only, no new cycles). One disclosed
  hiccup: the first extract attempt died on a cache write with the sandbox
  disk at 98% (ENOSPC family); the incremental second pass completed cleanly
  (241 files cached, 0 re-extracted). Outputs in /tmp/spp-graph-ga2/.
- Scope: `git diff --stat` = pages/mod.rs (6 lines), pages/queries.rs (-18),
  pages/json_fields.rs (new, 29) + ai/plan.md + ai/progress.md (+ this
  record). No behavior change anywhere — a definition relocation only.

Rule 8 STOP — the queue is empty; the next work item requires a new owner
directive.

**Addendum (GA-2, CI outcome, 2026-11-04):** CI on the implementation commit
(`f535e77`) went **green 3/3 on the first push** — no GA-1-style process miss
this time: the GA-1 standing practice (`cargo fmt --all -- --check` as the
final pre-push local step, not an early one) held. Checks: fmt/clippy/test/
deny ✓ · AppImage + .deb build smoke ✓ · kittest snapshot locks ✓. Ledger
synced per Rule 4 with this addendum; the engagement returns to a clean
Rule 8 STOP — the queue is empty, the next work item requires a new owner
directive.

## Batch RW-1 — connectors: no more blocking I/O on the async runtime (the repowise-audit defect) — completed and verified 2026-11-05

Owner directive: "continue" on the repowise-audit closeout (repowise,
repowise-dev/repowise v0.60.1, installed from PyPI and run locally against
3845e10 — zero-LLM lanes: health/performance/dead-code/doc-drift/security/
change-risk). The audit's performance lane surfaced exactly one real defect,
verified against source before any edit: `fetch_rows`
(`crates/core/src/connectors.rs`) is an `async fn` awaited from the
background worker loop (`workers/mod.rs:1130` → `refresh` → `refresh_inner`
→ `fetch_rows`) yet made blocking calls on the shared tokio runtime —
`std::fs::metadata` + `std::fs::read_to_string` in the `local_json` and
`csv` branches, and synchronous rusqlite (`open_with_flags` READ_ONLY →
COUNT → prepare → query_map) in the `sqlite` branch. Every other audit
lane verified clean or benign: `hot_path_sync_io` (9) all legitimate sync
contexts (xtask packaging, db open, config migration, backup restore,
fatal handler); `nested_loop_with_io` production hit
(`sync_engine::persist_thread_details`) small-N inside the sync
transaction; the 315 error_handling findings all low severity with
spot-checked false positives; the 4 dead-code findings false positives
(`richtext.rs` test-module SpanStyle helpers); doc-drift (25) confined to
the frozen ledger archives (expected under D13); security clean bill
(0 findings, 293 commits / 4,550 blobs, tree + full history).

**RW-1-1 (`Cargo.toml` (workspace), `crates/core/src/connectors.rs`).**
- tokio workspace features: added `"fs"` (additive; `tokio::fs` is the
  idiomatic non-blocking fs surface).
- New private helper `read_capped(data_dir, file)`: `jail_resolve` →
  `tokio::fs::metadata` → 10MB cap → `tokio::fs::read_to_string`, awaited
  end to end; error messages byte-identical to the pre-RW-1 blocking
  version ("file error: {e}" / "file too large ({}MB > 10MB cap)" /
  "read error: {e}"). Also deduplicates the stat→cap→read prologue the
  `local_json` and `csv` branches had carried verbatim (the audit's DRY
  note).
- `local_json` + `csv` branches: one `read_capped(...).await?` line each;
  parse/normalize logic untouched.
- `sqlite` branch: the whole rusqlite read moved into
  `tokio::task::spawn_blocking` — the codebase's established pattern
  (workers/execute.rs uses it 8×) — with owned `abs`/`table` moved in,
  JoinError mapped exactly like workers/execute.rs
  (`.map_err(|e| Error::Other(e.to_string().into()))??`), the pure
  validation (jail_resolve + table-name charset check) still sync before
  the hop, and error strings preserved ("sqlite open: {e}" /
  "sqlite read: {e}" / "invalid table name").
- `http` branch untouched (already fully async: SSRF validation, reqwest,
  chunked stream with mid-stream cap).
- Public surface unchanged: `fetch_rows`/`refresh`/`refresh_inner`
  signatures identical; the only behavior delta is that the blocking hops
  now yield to the executor.

**Verification (Rule 5)** — all mechanical, re-run at the working tree
(fresh build: the sandbox `target/` had been deleted for disk headroom
after the audit pushed the root volume to 100%; disclosed):
- `cargo clippy --workspace --all-targets -- -D warnings`: clean (3m08s,
  from scratch).
- Tests, foreground per-crate, baselines exact: core 452 / 0 / 3;
  app-shell 239 / 0 (+2 integration tests green); catalog 0 (no tests);
  xtask 11 / 0.
- `cargo fmt --all` then `cargo fmt --all -- --check` as the FINAL
  pre-push local step: clean (GA-1 standing practice).
- Repowise re-verification: `repowise health --recompute` over the fixed
  working tree — **blocking_sync_in_async: 4 → 0 across the whole repo**;
  the remaining `connectors.rs` markers are the pre-existing structural/
  low-severity ones (large_method/complex_method on the already-large
  function, io_in_loop ×2 on refresh_inner's transactional INSERT loop,
  low-severity error_handling) — none introduced by this batch; the
  prologue dedup removed duplication, added none.
- Scope: `git diff --stat` = Cargo.toml (1 line), connectors.rs
  (69+/59−) + ai/plan.md + ai/progress.md + this record. No API change,
  no behavior change beyond the blocking hops now awaiting.

Rule 8 STOP — the queue is empty; the next work item requires a new owner
directive.

**Addendum (RW-1, CI outcome, 2026-11-05):** CI on the implementation commit
(`7cf30be`) went **green 3/3 on the first push** — the GA-1 standing practice
(`cargo fmt --all -- --check` as the final pre-push local step) held for the
second consecutive batch. Checks: fmt/clippy/test/deny ✓ · AppImage + .deb
build smoke ✓ · kittest snapshot locks ✓. Ledger synced per Rule 4 with this
addendum; the engagement returns to a clean Rule 8 STOP — the queue is empty,
the next work item requires a new owner directive.

## Batch DENY-1 — deny lane: zero warnings (the reviewdog-battery findings) — completed and verified 2026-11-05

**Owner:** "continue" on the reviewdog verification battery closeout.
Context recorded for the instrument chain: the owner had directed a
Tauri-WebDriver "check everything again" pass; the source audit showed
tauri-driver inapplicable (the Tauri app was deleted at the Phase 4 cutover —
zero tauri references in any manifest or source file, README:193 documents
the eframe/egui replacement), so the battery ran as a full local re-run of
every CI lane at 1c50e4f (toolchain 1.99.1 pinned) plus the SPP_SMOKE
boot-chain E2E (rc=0). The `cargo deny` lane, run locally with a prebuilt
cargo-deny 0.20.2 (current latest; CI's taiki-e install resolves latest at
runtime), passed but emitted three warnings — the findings this batch
consumes.

**Findings (all three tool-flagged, each verified against the graph before
touching anything):**
1. `warning[yanked]` — async-executor 1.14.0 is yanked on crates.io;
   chain: accesskit_unix ← egui-winit ← eframe (the Linux accessibility
   stack). cargo-deny's own suggestion: `cargo update -p async-executor`.
2. `warning[unnecessary-skip]` — the `bitflags` bans skip ("1.x legacy
   alongside 2.x") is dead: `cargo tree -i bitflags@1.3.2` prints nothing
   for the checked graph (x86_64-unknown-linux-gnu), and the version does
   not exist in the sandbox-feature graph either; bitflags 1.3.2 sits in
   Cargo.lock only behind a target/feature edge no checked configuration
   activates (cargo tree itself points at non-checked targets).
3. `warning[license-not-encountered]` — the `CDLA-Permissive-2.0` allowance
   matches nothing (zero CDLA mentions in Cargo.lock).

**Items (Rule 2: opened in plan.md, moved through progress.md):**
- DENY-1-1: drop the dead `bitflags` skip line — the [bans] block's own
  drop-policy ("when an upstream dedupes one of them, drop the line; CI
  stays green — the skip is then simply unused"), applied on the tool's
  explicit flag rather than an upstream dedupe.
- DENY-1-2: drop the unused `CDLA-Permissive-2.0` allowance.
- DENY-1-3: `cargo update -p async-executor` 1.14.0 → 1.14.1 — the exact
  suggested patch bump; dry-run verified a single-package move (no
  collateral lock churn).

**Verification (fresh container after the mid-session sandbox recycle —
rustup + pinned toolchain 1.99.1 reinstalled, repo re-cloned at 1c50e4f;
disclosed):**
- `cargo deny --locked check advisories licenses bans`: **rc=0 with zero
  warnings** — "advisories ok, bans ok, licenses ok"; no yanked, skip, or
  license notes remain.
- `cargo clippy --locked --workspace --all-targets -- -D warnings`: clean
  (14.2s warm — the bump recompiled only the a11y-stack crates).
- Tests, foreground per-crate, baselines exact: core 452 / 0 / 3 (406s);
  app-shell 239 / 0 lib (216s) + style_lint 2 / 0 + the 24 kittest suites
  0/0 (feature-gated, as designed); catalog 0; xtask 11 / 0; doc-tests
  0 / 0 everywhere.
- Gates: no-simulation clean; traceability OK (23 routes / 20 DC tags /
  27 ids).
- `cargo fmt --all -- --check` as the FINAL pre-push local step: clean
  (GA-1 standing practice).
- Scope: `git diff --stat` = deny.toml (2 lines), Cargo.lock (one package),
  ai/plan.md, ai/progress.md, this record. Zero source files touched; no
  API change, no behavior change.
- Known-lane limit (unchanged by this batch): the kittest snapshot job is
  not locally runnable in this sandbox (zero Vulkan ICDs, no libEGL, no
  sudo); it remains CI-covered and green at the base commit.

Rule 8 STOP — the queue is empty; the next work item requires a new owner
directive. (CI addendum to follow at the push.)

CI outcome addendum (appended to the DENY-1 record, append-only): the
implementation commit 5aa4c23 went green 3/3 on the first push
(fmt/clippy/test/deny ✓ · AppImage + .deb build smoke ✓ · kittest snapshot
locks ✓) — the GA-1 standing practice (cargo fmt --all -- --check as the
final pre-push local step) held for the third consecutive batch, and the
deny lane is now warning-free end to end (no yanked-advisory note, no dead
skip, no unmatched allowance). Rule 8 STOP.

---

## Batch REL-3 — Build & Release v1.9.4 from the final source (2026-11-05)

**Owner directive:** build, test, and publish install-ready Linux `.deb` and
`.AppImage` packages from the final repository source; versioned GitHub
Release with both packages attached, checksums, compatibility, and verified
download links; packages must be self-contained and launch-ready with no
dev tools, manual dependency handling, or first-run setup.

**Items:**
- REL-3-1: workspace version 1.9.3 → 1.9.4 (Cargo.toml + Cargo.lock).
- REL-3-2: CHANGELOG `[1.9.4]` section in product language.
- REL-3-3: annotated tag `v1.9.4` on the release commit.
- REL-3-4: publish + live post-publish verification of the GitHub Release.

**Release commit:** `f35c463` (main tip at push; RW-1 + DENY-1 included —
neither was in any published release before this one).

**Version bump verification (no Rust toolchain in the recycled sandbox,
~1.7 GB free disk — disclosed; all build/test gates therefore ran in
release.yml on the exact tagged content, as REL-2 established):**
- Cargo.lock hand-bumped via a name-keyed script, tomllib re-parse: exactly
  the four member package versions changed (app-shell, catalog, core,
  xtask → 1.9.4), 620 packages total, zero stray `1.9.3` strings.
- release.yml's own extraction commands simulated locally: the version awk
  prints `1.9.4`; the changelog section awk returns the full `[1.9.4]`
  section (17 lines).

**CI Release run 42006886538 (tag push, ~29 min, 6/6 jobs green):**
- build + test + package (ubuntu:22.04): no-simulation + traceability
  gates, fmt, clippy ×2 `--locked -D warnings`, workspace tests, release
  build with the Landlock/seccomp parse sandbox, xtask AppImage + .deb
  packaging, versioned dist staging with SHA256SUMS.txt + BUILD-INFO.txt.
- verify .deb on clean ubuntu:22.04, ubuntu:24.04 and debian:12 containers:
  checksum integrity, APT install with automatic dependency resolution,
  package metadata + desktop entry (`desktop-file-validate`) + all four
  icon sizes, SPP_SMOKE boot chain, 0.2.x→1.x data-directory upgrade
  migration (in-place, data preserved), GUI launch under Xvfb with only
  declared dependencies (rc=124 = alive at timeout), reinstall mechanics,
  uninstall with user-data retention, purge record cleared.
- verify AppImage: checksum integrity, APPIMAGE_EXTRACT_AND_RUN boot-chain
  smoke, GUI launch under Xvfb.
- publish-after-green: release created, then every asset URL re-downloaded
  in-workflow and checksum-verified against the uploaded bytes.

**Live post-publish verification (this sandbox, independent of CI):**
- Release `v1.9.4` = "SupportOS+ 1.9.4", draft=false, prerelease=false,
  target_commitish = `f35c463…` (== final source HEAD).
- All four assets downloaded live (AppImage 12,806,648 B; .deb 10,175,014 B;
  SHA256SUMS.txt; BUILD-INFO.txt); `sha256sum -c SHA256SUMS.txt`: both
  packages OK (AppImage `a0395aea…e443`, .deb `a5c9cd64…742a`).
- BUILD-INFO.txt: version 1.9.4, commit `f35c463578f9…` == tag target ==
  main tip; built on ubuntu:22.04 (glibc 2.35 baseline) at
  2026-11-05T10:27:04Z.
- `.deb` inspection: Package supportos-plus, Version 1.9.4, arch amd64,
  19 declared Depends led by `libc6 (>= 2.35)` (APT resolves all of them —
  CI proved it on three clean distro containers); contents = /usr/bin
  binary + applications/supportos-plus.desktop + hicolor icons (all four
  sizes, CI-verified) + copyright.
- `.deb` extracted (dpkg-deb -x, no install) and smoke-run in this sandbox:
  SPP_SMOKE rc=0 — all 54 migrations, boot services, background workers,
  loopback listener (graceful OS-assigned port fallback), automatic backup,
  24 pages / 4222 shapes rendered headlessly, clean exit; first-run
  initialization created the database on its own.
- AppImage `APPIMAGE_EXTRACT_AND_RUN=1 SPP_SMOKE=1`: rc=0, identical boot
  chain (54 migrations, 24 pages / 4222 shapes); structure contains AppRun
  + supportos-plus.desktop + icon for desktop-menu integration.
- Release notes: 9/9 content checks — `[1.9.4]` changelog section, both
  exact SHA-256 hashes, commit binding, both install commands, system
  requirements (x86_64, glibc 2.35+, Ubuntu 22.04 LTS+/Debian 12+,
  desktop session + GPU driver, ~200 MB), CI-verified list, no-manual-setup
  statement.

**Environmental limits disclosed (honest scope):** no sudo in this sandbox
→ APT-install/uninstall/upgrade lanes ran only in CI's clean containers
(green ×3 distros); no Vulkan ICD → local GUI launch impossible, covered by
CI's four Xvfb GUI lanes on the exact artifacts (green). Everything else
was verified live in this sandbox from the published bytes.

Rule 8 STOP — the queue is empty; the next work item requires a new owner
directive.

---

## Batch HYG-2 — CHANGELOG link-reference completeness (2026-11-05/06)

**Owner directive:** "continue" with the queue empty post-REL-3. The
continue ran an honest read-only post-release consistency sweep; the single
finding became this batch.

**Sweep results (all clean except one finding):**
- Zero stale `1.9.3` strings outside ledger/CHANGELOG history (product,
  packaging, and docs surfaces are version-clean at 1.9.4).
- README install section is version-parameterized (`<version>` filenames)
  and already matches the published asset naming exactly.
- GitHub: 0 open issues/PRs; last 8 workflow runs all success.
- CHANGELOG header/tag triangle: 23 version headers, 23 origin tags —
  exact 1:1 (`git ls-remote` cross-check).
- **Finding:** the file declares Keep-a-Changelog format but only 3 of 23
  version headers carried a link reference (`[1.1.0]`, `[1.0.0]`,
  `[0.2.0]`) — 20 missing, every one backed by an existing tag (zero 404
  risk).

**Fix (commit `118378e`):** added the 20 missing link references in
descending order above `[1.1.0]` (1.9.4 → 1.1.1), keeping one sorted block.
Docs-only: CHANGELOG +20 lines, ledger +19; no source, no lock, no
packaging surface.

**Verification:**
- Header/ref parity 23/23, zero orphans in either direction (mechanical
  regex census).
- All 23 reference URLs live-checked: HTTP 200 each.
- CI run `42134855363` on `118378e`: 3/3 green (fmt/clippy/test/deny ✓ ·
  AppImage + .deb build smoke ✓ · kittest snapshot locks ✓).
- (Context note: the recycled sandbox wiped the clone mid-sweep; re-cloned
  at `d306caa` — zero state lost, everything lives on GitHub per the
  ledger principle. The sweep re-ran from the fresh clone.)

Rule 8 STOP — the queue is empty; the next work item requires a new owner
directive.

---

## Batch UXA-1 — UX audit fix wave 1 (items UX-1..UX-9 + UX-25 completed and verified, 2026-11-06)

**Owner:** the autonomous UX/UI audit directive — audit the SupportOS+
repo, find and fix every frontend UX/UI/layout/design defect with
before/after evidence per item; frontend-only per the scope contract
(`ux-audit/reports/scope.md`); work directly on `main`, small
issue-tagged commits, `git pull --rebase` before pushing; batches of
exactly ten per `ai/rule.md` with a STOP after each batch.

**Program context:** Phase 0–2 of the audit ran first (worklog:
`ux-audit/reports/worklog.md`): scope contract, journey map
(`ux-audit/reports/journey-map.md` — every page + shell journeys driven
under Xvfb 1920×1080 with synthetic input + VLM verification; OAuth and
webhook relay verified live against the Help Scout test account), and the
issue census (`ux-audit/reports/issues.md` — 35 findings incl. 3
backend-blocked logged UX-26B/34/35, plus 3 investigated false
positives). Batch UXA-1 consumed the ten highest-severity fixable items.

**Items (Rule 2: opened in plan.md, moved through progress.md):**
- UX-1 (blocker): internal dev-note footnotes (`design::quirk`) shipped in
  production UI — all page-level call sites removed (connectors, ai_center,
  issue_radar, notifications, operations, dashboard, reports); the widget
  itself stays in the design system (kittest_widgets coverage).
- UX-2 (blocker): Automation page debug note + header-only rules table →
  note removed, real empty state ("No automation rules yet" + what rules
  do + where review actions park).
- UX-3 (major): Shell page stale ported-pages checklist → runtime-info
  presentation ("app runtime information" + hint, middle-truncated DB
  path at 72 chars, "Where things live" orientation card).
- UX-4 (major): Onboarding raw key–value dump → three-step checklist with
  per-step todo/done copy from the same status data.
- UX-5 (major): metric cards hugging 150px → `metric_card_w` +
  row-width grids on Dashboard (12 cards) and Reports.
- UX-6 (major): tables at 25–40% width → `table_stretched` +
  `data_table` width distribution (Customers, Organizations, Incidents,
  Connectors, Automation, customer-conversations).
- UX-7 (major): Settings narrow ~600–900px column → `card_titled_wide`
  on every settings surface, all tabs.
- UX-8 (major): Inbox fixed-330px list → proportional 30% (330–520px),
  user-resizable (300–640px).
- UX-9 (major): charts with no axes → `line_chart_axes`: y min/max
  labels, x endpoint dates, gridlines, hover readout (guide line +
  flipping value bubble); wired on Dashboard + Reports.
- UX-25 (major): browser OAuth flow unreachable (disclosure gated on
  `!configured` while `begin_oauth` needs the active provider that exists
  only when connected — verified by racing an in-flight connect) →
  disclosure renders whenever credentials exist; verified live in the
  fixed build while CONNECTED (green badge + expanded "advanced: browser
  authorization flow" with the begin button); misleading copy replaced.
  Side-fix en route [UX-27 partial]: inbox search box takes the free row
  width (was a fixed 140px truncating its hint).

**Implementation commits (11, one per page/design-system group):**
57c9a03 (design-system variants) · 18d6852 (shell) · 36e48fd (onboarding)
· cf91880 (dev-note removals) · b09b258 (automation) · 72b5977
(connectors) · c83e646 (tables) · 0561a46 (dashboard/reports/chart) ·
0ba3100 (settings wide) · b990f0c (settings OAuth disclosure) · cc71c84
(inbox) — format `fix(ui): <what> on <where> [ISSUE-ID]` per the owner's
directive; settings.rs deliberately split across 0ba3100/b990f0c so UX-7
and UX-25 are separately revertable (intermediate state compile-checked).

**Verification (this sandbox, toolchain 1.99.1 pinned):**
- `cargo fmt --all -- --check`: clean (GA-1 standing practice, final
  pre-push step).
- `cargo clippy --locked --workspace --all-targets -- -D warnings`: clean.
- Tests: app-shell lib 239/239; xtask 11/11 incl.
  `no_secret_key_artifacts_anywhere_in_the_repo` (audit secrets live only
  in gitignored `ux-audit/scripts/`).
- **kittest snapshot locks — made locally runnable this batch**: DENY-1's
  standing disclosure ("no Vulkan ICD, no sudo") was worked around by
  downloading the `mesa-vulkan-drivers` .deb and extracting lavapipe
  locally (`VK_DRIVER_FILES` + `LD_LIBRARY_PATH`); all 24 suites +
  style_lint ran diff-locked against the committed baselines. 89 snapshot
  baselines drifted; the drift list was audited one-by-one — every drift
  maps to an intended fix (r2a widget harness ×4 via data_table;
  automation ×7; connectors ×7; customers/organizations/incidents ×7
  each; dashboard ×7 + reports ×6; inbox ×6 + copilot ×3; issue_radar ×6
  + notifications ×7 + operations workload ×1 via the dev-note removals;
  onboarding ×7; settings ×7). Baselines regenerated with
  UPDATE_SNAPSHOTS=1, then the full suite re-ran diff-locked GREEN —
  the diff-locked-twice discipline.
- `SPP_SMOKE=1`: rc=0 (boot chain + 24 pages rendered headlessly).
- Per-item before/after screenshot evidence at 1920×1080 (light; +
  Dashboard dark): 13 after-shots, each verified for page identity
  (header read-back) and theme (luma scan); UX-25's shot verified for
  connected-state + expanded disclosure. Evidence dirs are gitignored by
  design; referenced by path from `ux-audit/reports/issues.md`.
- Evidence-integrity disclosure: a theme-toggle miss mid-run darkened 11
  after-shots; caught by the luma verification, all re-shot light before
  any item was marked fixed. The palette-driven navigator proved
  unreliable on this dataset (conversation subjects outmatched page
  names for queries like "reports"); sidebar-row navigation with
  verification was used instead.
- Scope: 14 source files under `crates/app-shell/src/` + 89 kittest
  baselines + ledger/gitignore. Zero backend, API, schema, CI, workflow,
  packaging or Cargo.lock changes (`--locked` throughout).

**Backend-blocked findings logged, not queued** (per the scope contract):
UX-26B (webhook secret hard cap in core), UX-34 (Operations tiles have
no nav target from core data), UX-35 (raw job-error strings from core).

Rule 8 STOP — batch UXA-1 of ten is complete; the queue holds UXA-2
(items UX-10 onward). Resume on the owner's "continue".

CI outcome addendum (appended to the UXA-1 record, append-only): the
12-commit push (57c9a03..ec4632d) went green 3/3 on the first push —
fmt/clippy/test/deny ✓ · AppImage + .deb build smoke ✓ · **kittest
snapshot locks ✓** (the locally-regenerated baselines, rendered with the
sandbox-extracted lavapipe 25.0.7, match CI's lavapipe rendering — the
cross-Mesa risk closed) — CI run 42148385123, completed success. The GA-1
standing practice (cargo fmt --all -- --check as the final pre-push local
step) held. Rule 8 STOP.

## Batch UXA-2 — the second ten UX-audit fixes (UX-10..UX-19)

Opened 2026-11-06 on the owner's "continue" after the UXA-1 Rule 8 STOP;
moved to `progress.md` per Rule 2; implemented, verified, and closed the
same session (Rule 4). Scope contract and per-item evidence:
`ux-audit/reports/` (scope.md · issues.md statuses · worklog.md items
15–20).

**Items (10):** UX-10 (global Title-Case normalization of buttons, chips,
tabs, pagers, and table headers across every page), UX-11 (Settings
subtitle + Vector-tab hint rewritten in user language; "1.4.0 R8:" /
"Clean-audit item 138:" hover-note prefixes stripped), UX-12 (dark-theme
contrast: muted_foreground 161→186, border 39→48, border_strong 63→66,
sidebar 12→16, selected chips invert to primary fill + strong label),
UX-13 (notifications grouped by (type,title,body) with ×N counters, group
mark read/unread, humanized type badges, monospace truncated error
bodies), UX-14 (Failed jobs → Diagnostics, Sync problems → Sync Health,
Automation approvals → Automation, each with a named drill line; truthful
subtitle; relative "updated …" stamp), UX-15 (example-query chips replace
the "+ Try 'export'" CTA; Scope: label + visible selected state), UX-16
(empty-state CTAs wired to real destinations on Live chats/Custom
Objects/Outreach/Support Graph + contradiction and pluralization fixes),
UX-17 (labeled Status/Severity filters; filter-aware empty states with
Clear Filters and New Incident CTAs), UX-18 (import disclosure
auto-opens on the first-run empty state; "Import Documents" CTA),
UX-19 ("Never" / "No domains yet" fallbacks + Title-Cased headers on
Customers/Organizations).

**Implementation commits (11, code + per-suite kittest assertion updates):**
7bb2c89 (design system: chips, dark tokens, shared relative_time)
· e5f27a9 (notifications) · 25d6126 (operations) · a0c2d21 (search)
· 32b80bb (live chats/custom objects/outreach/support graph) · 1882f52
(incidents) · c7212e8 (knowledge) · b355991 (customers/organizations)
· 66b389e (settings copy) · 7921b69 (title-case sweep) · d66122b (157
regenerated kittest baselines, one atomic commit because the design-token
change drifts every page's snapshots — per-commit splitting would be
lossy). Format `fix(ui): <what> on <where> [ISSUE-ID]` per the owner's
directive.

**Verification (this sandbox, toolchain 1.99.1 pinned):**
- `cargo fmt --all -- --check`: clean.
- `cargo clippy --locked --workspace --all-targets -- -D warnings`: clean.
- Tests: app-shell lib 242/242 (3 new UX-13 grouping/toggle/type-label
  tests; the dc_v1 token-freeze test updated for the intended UX-12
  sidebar value with an in-code note); xtask 11/11 incl. the secrets
  hygiene check.
- kittest snapshot locks: all 24 suites + style_lint diff-locked GREEN
  end-to-end (exit 0). Drift discipline: the first full run's failure
  list was bogus — the sandbox-reaped /tmp lavapipe extraction made every
  render test panic with "No adapter found"; caught by auditing one
  failure's panic text before regenerating anything, re-extracted
  lavapipe to a persistent home location, re-ran: 157 real drifts, every
  one mapped to an intended UXA-2 change (three VLM-audited diff images:
  dashboard scope-chip = chip selected state; notifications = Mark
  Read/Unread/All Read + grouping; hero dark = token brightening).
  Baselines regenerated with UPDATE_SNAPSHOTS=1, 5 label-dependent
  kittest assertions updated to the renamed buttons, full suite re-ran
  diff-locked GREEN — the diff-locked-twice discipline.
- `SPP_SMOKE=1`: rc=0 (boot chain + 24 pages / 5131 shapes rendered
  headlessly).
- After-evidence: 15 deterministic per-page screenshots at 1920×1080 via
  the app's own `ui-screenshot` feature (SPP_SHOT_PAGE/SPP_SHOT_THEME) —
  dashboard light+dark, notifications, operations, search, incidents,
  customers, organizations, knowledge, custom objects, outreach, live
  chats, support graph, settings default + vector tab. Evidence dirs are
  gitignored by design; referenced by path from
  `ux-audit/reports/issues.md`.
- Scope: 27 source files under `crates/app-shell/src/` + 15 kittest test
  files + 157 baselines + ledger/reports. Zero backend, API, schema, CI,
  workflow, packaging or Cargo.lock changes (`--locked` throughout).

**Backend-blocked findings stand** (logged, not queued): UX-26B (webhook
secret cap), UX-34 (no nav target in core tile data), UX-35 (raw
job-error strings from core).

Rule 8 STOP — batch UXA-2 of ten is complete; the queue holds UXA-3
(items UX-20 onward). Resume on the owner's "continue".

CI outcome addendum (appended to the UXA-2 record, append-only): the
13-commit push (7bb2c89..70b2da2) went green 3/3 on the first push —
fmt/clippy/test/deny ✓ · AppImage + .deb build smoke ✓ · kittest snapshot
locks ✓ (the 157 regenerated baselines match CI's lavapipe rendering) —
CI run 42166530366, completed success. Rule 8 STOP.

## Batch UXA-3 — the third ten UX-audit fixes (UX-20..UX-30)

Opened 2026-11-06 on the owner's "continue" after the UXA-2 Rule 8 STOP;
moved to `progress.md` per Rule 2; implemented, verified, and closed the
same session (Rule 4). Scope contract and per-item evidence:
`ux-audit/reports/` (scope.md · issues.md statuses · worklog.md items
21–28). UX-25 was already fixed in UXA-1; UX-26's backend half stays
blocked as UX-26B.

**Items (10):** UX-20 (Sync Health: `table_stretched` + bounded scroll on
both tables, Title-Case headers, user-language hero with relative time —
the "never shows relative time — forensic value" dev note gone), UX-21
(Diagnostics: per-level counts ON the filter chips — "Errors 36" is the
path to the 36 errors — from one unfiltered fetch with a new unit test;
proper pluralization; stretched + scroll-bounded runs/outbound tables),
UX-22 (Backups: Destructive-variant restore with "confirmation step
follows" hover and file-naming confirmation; human_bytes + relative-time
metadata with exact values on hover), UX-23 (LM Studio: "Open AI Center"
button — Settings now returns Option<NavRequest>; purpose subtitle; field
hints/hovers; base-URL validation on save), UX-24 (Vector: labelled
semantic-search toggle wired on change; truncating + copyable store
path), UX-26 (webhook copy tells the truth before AND after save: Help
Scout signs with the FULL secret, SupportOS+ verifies only the first 40
chars — no more "the platform caps…" misattribution), UX-27 (Inbox: Cc/Bcc
above the composer body in a scroll-in-place pane; SP1 row breathing;
"Unassigned" as plain italic text — dashed pill removed from the shared
design::assignee widget), UX-28 (Dashboard: full-width filter card with
one wrapped chip row; breakdown cards capped at 8 bars + "and N more…";
named chart caption inside the frame — shared line_chart_axes, Reports
inherit), UX-29 (Hours: purpose subtitle, Warning badge on "not
configured", spelled-out fr/res abbreviations, self-explaining empty
state), UX-30 (Sync & AI: uniform checkbox list, 110×26 numeric inputs
with hovers, dirty-state save row — "Save Changes" + unsaved note vs
"All changes saved" badge — backed by general_persisted).

**Implementation commits (6, code + baselines):** b917382 (sync_health
[UX-20]) · 118fa9d (diagnostics [UX-21]) · 3c55867 (inbox + design
assignee [UX-27]) · d156e7a (dashboard + chart [UX-28]) · 766a1a5
(settings tabs: backups/lmstudio/vector/helpscout/hours/syncai
[UX-22][UX-23][UX-24][UX-26][UX-29][UX-30] + the feature-gated
SPP_SHOT_TAB evidence hook) · 90b2c73 (46 regenerated kittest baselines,
one atomic commit — per-commit splitting would be lossy). Format
`fix(ui): <what> on <where> [ISSUE-ID]` per the owner's directive.

**Verification (this sandbox, toolchain 1.99.1 pinned; the sandbox was
recycled between sessions — lavapipe + libxkbcommon-x11/libxcb-xkb
re-extracted to /home/z/mesa-lvp/extracted from .debs, no sudo):**
- `cargo fmt --all -- --check`: clean.
- `cargo clippy --locked --workspace --all-targets -- -D warnings`: clean
  (re-verified after every in-session code change).
- Tests: app-shell lib 244/244 (2 new tests: human_bytes user-facing
  sizes, level-counts-cover-the-full-log); xtask 11/11 incl. the secrets
  hygiene check.
- kittest snapshot locks: all 24 suites + style_lint diff-locked GREEN
  (exit 0, re-run a third time on the final code state after the
  SPP_SHOT_TAB hook — it is `#[cfg(feature = "ui-screenshot")]`, compiled
  out of kittest builds, and the re-run proves it). Drift discipline:
  first full run --no-fail-fast produced 46 snapshot drifts across 8
  suites + 1 REAL style-lint failure — UX-27's row breathing used
  off-scale add_space(1.0) (DC §2.4); fixed in code to
  design::scale::space::SP1 before any baseline regeneration, style_lint
  2/2 after. All 46 drifts audited one-by-one, every one mapped to an
  intended UXA-3 change (dashboard ×7, diagnostics ×7, sync_health ×7,
  settings ×6 — the helpscout-tab golden byte-identical because UX-26 is
  hover/save-note copy; reports ×6 = shared chart caption; inbox ×6 +
  copilot ×3; widgets ×4 = assignee). Zero unexpected suites. Baselines
  regenerated with UPDATE_SNAPSHOTS=1, full suite re-ran diff-locked
  GREEN; exactly 46 goldens changed in git, one-for-one with the audited
  list.
- `SPP_SMOKE=1`: rc=0 (boot chain + 24 pages / 4259 shapes rendered
  headlessly; the shape count differs from UXA-2's 5131 because the
  recycled sandbox boots a fresh DB — the criterion is per-page
  non-empty render, unchanged).
- After-evidence: 11 deterministic shots at 1920×1080 via ui-screenshot
  + the new SPP_SHOT_TAB (sync_health, diagnostics, settings
  backups/lmstudio/vector/helpscout/hours/syncai, inbox, dashboard
  light+dark), all rc=0, all luma-verified (10 light 244–250, 1 dark
  26.6). The Backups shot is seeded with two real SQLite backup files
  (random bytes fail the integrity check the restore flow runs).
  Disclosure: VLM read-back spot-checks were 429-rate-limited all session
  (three attempts spread over the batch — the same limit the UXA-1/UXA-2
  sessions hit); page identity is instead verified structurally: same
  binary, SPP_SHOT_PAGE label match, per-shot rc=0, luma theme scan,
  and the diff-locked kittest goldens that encode the exact fixed states.
- Scope: 11 source files under `crates/app-shell/src/` + 46 kittest
  baselines + ledger/reports. Zero backend, API, schema, CI, workflow,
  packaging or Cargo.lock changes (`--locked` throughout). The
  SPP_SHOT_TAB hook is dev-harness only (feature-gated off in every
  normal/CI build, same class as the existing SPP_SHOT_* hooks).

**Backend-blocked findings stand** (logged, not queued): UX-26B (webhook
secret cap), UX-34 (no nav target in core tile data), UX-35 (raw
job-error strings from core).

Rule 8 STOP — batch UXA-3 of ten is complete; the queue holds the final
UX-audit batch UXA-4 (items UX-31..UX-33). Resume on the owner's
"continue".

CI outcome addendum (appended to the UXA-3 record, append-only): the
7-commit push (b917382..540092c) went green 3/3 on the first push —
fmt/clippy/test/deny ✓ · AppImage + .deb build smoke ✓ · **kittest
snapshot locks ✓** (the locally-regenerated 46 baselines, rendered with
the re-extracted sandbox lavapipe 25.0.7, match CI's lavapipe rendering —
the cross-Mesa risk stays closed) — CI run 42190717843, completed
success. The GA-1 standing practice (cargo fmt --all -- --check as the
final pre-push local step) held. Rule 8 STOP.

## Batch UXA-4 — the final three UX-audit fixes (UX-31..UX-33)

Opened 2026-11-06 on the owner's "continue" after the UXA-3 Rule 8 STOP
(second sandbox recycle of the engagement: repo re-cloned at 5bde367,
Rust 1.99.1 + lavapipe/xkb re-extracted to a persistent home prefix
first); moved to `progress.md` per Rule 2; implemented, verified, and
closed the same session (Rule 4). Per-item evidence:
`ux-audit/reports/` (issues.md statuses · worklog.md items 29–36).

**Items (3):** UX-31 (palette hit rows re-banded at 46px — the old 40px
row genuinely overlapped title and subtitle galleys by ~7px, the audit's
"bleed-through"; selected-row subtitle now uses the selection-foreground
pair instead of ~4.4:1 muted gray; guard test
`palette_row_bands_never_overlap`), UX-32 (Backups tab: purposeful
"Back up or export your data" card with hover-explained, clearly-enabled
Outline exports + "Create Backup" Title Case, and a separate "Restore a
backup" card carrying the destructive rows and armed confirm — the Help
Scout danger-zone pattern; no card title repeats the tab name),
UX-33 (header: the audit's "bell" was the icon-only theme toggle — now a
labelled Outline button "Dark"/"Light"; the freshness pill names its
subject on its face, "Help Scout · synced Ns ago"/"syncing…"/"never
synced", hover explains the local mirror and opens Sync Health).

**Verification:** fmt clean · clippy --locked --workspace --all-targets
-D warnings clean · app-shell lib 245/245 (1 new) · kittest suites +
style_lint diff-locked GREEN twice on the final code (420/420 per full
run, zero golden drift — nothing golden-locked changed this batch) ·
xtask 11/11 · SPP_SMOKE rc=0 (24 pages / 4331 shapes) · after-evidence:
3 luma-verified 1920×1080 shots (palette__search, settings__backups,
shell__default) with GREEN VLM read-backs (no rate limits this session)
— the palette shot seeded mid-search via the new feature-gated
SPP_SHOT_PALETTE_QUERY hook, the backups shot seeded with two real
SQLite backup files.

- Scope: 3 source files under `crates/app-shell/src/` (palette.rs,
  app/mod.rs, pages/settings.rs) + Cargo.toml harness comment + the
  feature-gated SPP_SHOT_PALETTE_QUERY evidence hook (dev-only, off in
  every normal/CI build) + ledger/reports. Zero backend, schema, CI,
  workflow, packaging or Cargo.lock changes (`--locked` throughout).

**With UXA-4 the UX-audit program is fully consumed**: UXA-1 (UX-1..9 +
UX-25), UXA-2 (UX-10..19), UXA-3 (UX-20..30), UXA-4 (UX-31..33) — every
frontend-fixable finding closed with evidence. The three
backend-blocked findings stand logged, not queued: UX-26B (webhook
secret cap), UX-34 (no nav target in core tile data), UX-35 (raw
job-error strings from core).

Rule 8 STOP — final batch of the program; the queue is empty. Resume
only on a new owner directive.

CI outcome addendum (appended to the UXA-4 record, append-only): the
4-commit push (1ad1376..7b2399c) went green 3/3 on the first push —
fmt/clippy/test/deny ✓ · AppImage + .deb build smoke ✓ · **kittest
snapshot locks ✓** (zero golden drift confirmed on CI's lavapipe — the
local zero-drift prediction held: nothing golden-locked changed this
batch) — CI run 42294959798, completed success. The GA-1 standing
practice (cargo fmt --all -- --check as the final pre-push local step)
held. The UX-audit program is closed; Rule 8 STOP.

## Batch UXB-1 — the backend-blocked UX-audit fixes (UX-26B/UX-34/UX-35)

Opened 2026-11-07 on the owner's "continue … if no pending item is there,
resolve ignored items first" directive after the UXA-4 program close
(third sandbox recycle of the engagement: repo re-cloned at 771ed80, Rust
1.99.1 + lavapipe/xkb re-extracted to the persistent home prefix, owner's
GitHub credentials stored in the git credential helper for the batch
push); the three items pulled from `ux-audit/reports/issues.md` per Rule 2
with the batch scope extended to `crates/core/src/` by owner directive;
implemented, verified, and closed the same session (Rule 4). Per-item
evidence: `ux-audit/reports/` (issues.md backend-blocked statuses ·
worklog.md items 37–46).

**Items (3):** UX-26B (webhook secret cap — `verify_delivery` verifies
with the FULL stored secret and keeps a legacy fallback that accepts a
signature over the first 40 characters, so hooks registered by cap-era
builds — whose platform-side secret IS the capped prefix — do not regress;
`register_hook` now registers the full secret; the settings copy retired
the 40-char warnings), UX-34 (operations drill data — core's FailedJobs
drill corrected to `diagnostics` where the failed-jobs table lives, and
the frontend now derives tile navigation + drill lines from core's drill
payload instead of a hardcoded duplicate map, making the three tiles core
already marked inbox-drillable (Repeated issue, Known issue, AI
escalation) clickable like their siblings; unknown page names degrade to
inert tiles), UX-35 (job-failure notifications — friendly job-type title
+ classified friendly body (unreachable/auth/rate_limited/bad_response/
misconfigured/unknown, host extracted, LM Studio hint for localhost), the
verbatim error + code + queue + job_type preserved in the notifications
`payload` JSON (column existed, unused); the notifications page reveals
the raw detail on hover).

**Verification:** fmt clean · clippy --locked --workspace --all-targets
-D warnings clean · core 458/458 (9 new/updated UX tests) · app-shell lib
246/246 (2 new UX-34 tests) · non-kittest integration 2/2 · sandbox
feature lane clippy + 466/466 · kittest first diff-locked run
--no-fail-fast: exactly the two intended suites drifted (operations 6,
notifications 7 — audited one-for-one against the batch changes; diff
regions pixel-located: 3 tiles' new "open →" lines, the new fixture card)
· baselines regenerated, full suite diff-locked GREEN twice (exit 0, 27
suites) — 13 intended golden regens plus 7 disclosed sync-health regens
from the UXB-T1 date-tick (see below) · SPP_SMOKE rc=0 (24 pages / 4338
shapes) · after-evidence: 3 luma-verified 1920×1080 shots (operations,
notifications, settings Help Scout) with GREEN VLM read-backs: all 16
operations tiles now carry named drill affordances incl. "diagnostics →"
and the three newly wired "open →" tiles; the notifications card reads
"Background job failed: AI task" with a friendly body (no raw URL or
error string visible; badge "Job failure"); the Help Scout tab renders
the webhook-secret field with no 40-character warning text.

- Scope: `crates/core/src/` (webhook.rs, webhook_handler.rs, loopback.rs,
  webhook_health.rs, operations.rs, notifications.rs, notification_sweep.rs)
  + `crates/app-shell/src/` (pages/operations.rs, pages/notifications.rs,
  pages/settings.rs, app/mod.rs) + the kittest notifications fixture and
  the feature-gated screenshot seed (dev-only, off in every normal/CI
  build) + 20 kittest goldens + ledger/reports. No CI, workflow,
  packaging or Cargo.lock changes (`--locked` throughout).

**New finding disclosed (UXB-T1, test-infra, logged not queued):** the
sync-health kittest goldens tick "Nd ago" at every UTC midnight (fixed
2026-10-27 fixture stamps vs day-granularity `relative_time`); it fired
mid-batch when UTC midnight crossed between the first diff-locked run and
the baseline regen (10d → 11d, a single 6×8px glyph per golden,
pixel-verified), and any CI run on a later calendar day than the last
regen would fail those 7 goldens. Re-baselined with the batch; durable
fix (deterministic clock for fixtures) proposed in
`ux-audit/reports/issues.md`.

With UXB-1 the ux-audit backlog is fully consumed: UXA-1..UXA-4 (33
frontend findings) + UXB-1 (3 backend findings). Rule 8 STOP — the queue
is empty; resume only on a new owner directive.

CI outcome addendum (appended to the UXB-1 record, append-only): the
5-commit push (83290a0..41a62b9) went green 3/3 on the first push —
fmt/clippy/test/deny ✓ · AppImage + .deb build smoke ✓ · **kittest
snapshot locks ✓** (the regenerated baselines passed on CI's lavapipe,
including the 13 intended regens and the 7 disclosed UXB-T1 date-tick
regens — CI ran on the same UTC day as the regen, so the "11d ago"
goldens held) — CI run 42319227840, completed success. The GA-1 standing
practice (cargo fmt --all -- --check as the final pre-push local step)
held. The UX-audit backlog is fully consumed; Rule 8 STOP.

---

## Batch UXB-T1 — deterministic clock for the kittest shot fixtures (closed 2026-11-07)

Single-item batch (item UXB-T1), opened on the owner's "handle UXB-T1"
directive ~1h after the UXB-1 push (2026-11-07, 01:37Z) — the test-infra
finding disclosed during UXB-1's verification, logged-not-queued until that
directive pulled it back out of `ux-audit/reports/issues.md` per Rule 2
(fourth sandbox recycle: repo re-cloned at 7178153, Rust 1.99.1 +
lavapipe/xkb re-extracted to the persistent home prefix first); implemented,
verified, and closed the same session (Rule 4). Per-item evidence:
`ux-audit/reports/` (issues.md UXB-T1 status · worklog.md items 47–48).

**Item (1):** UXB-T1 (kittest golden date-tick) — the goldens rendered
"Nd ago"/"Yesterday" from the wall clock (`relative_time` →
`business_hours::now_iso_millis`) against fixed fixture stamps, ticking one
day further at every UTC midnight; the disclosed sync-health 7 (fixed
2026-10-27 seeds) plus, on the same clock, the operations 6 (fixture-frozen
`generated_at` = 2026-10-27T09:59:59.123Z — the freeze pinned the seed, not
the clock; regenerated inside UXB-1's intended set, ticking with it) and the
notifications 8 (wall-relative `iso_days_ago` seeds — stable, but exposed to
the same seeding/rendering midnight race that fired mid-UXB-1). Fix: a
`kittest-ui`-gated pinned `now` in core `business_hours`
(`KITTEST_PINNED_NOW_ISO` + `pin_now_for_kittest`, first-pin-wins,
unparseable rejected) consulted by every clock read on the golden render
path (`now_iso_millis`, `iso_days_ago`, `now_iso` → `effective_now()` —
inlines to `Utc::now()` in every normal build, feature off); the three
fixtures pin the anchor (noon UTC on 2026-11-07, the UXB-1 regen's calendar
day — day-granular rendering pixel-identical to the current baselines, so
pinning froze the tick with ZERO golden churn) before seeding; a new
`kittest_clock_pin` suite proves the mechanism (anchor drives both clock
reads; seed helper and render clock share the pinned instant; idempotent
re-pin accepted, conflicting and unparseable anchors rejected). The suite
earned its keep immediately: it caught that the first pin implementation
compared `OnceLock::set`'s Err payload (the REJECTED value) against the
candidate — a conflicting anchor would have falsely reported success while
the clock kept the first pin (rendering still correct, the API lied) —
fixed by comparing against `get()`.

**Verification:** fmt clean (final `--check` after the last edit) · clippy
--locked --workspace --all-targets -D warnings clean (re-run on the final
code) · core 458/458 (re-run on the final code) · app-shell lib 246/246 ·
non-kittest integration green · sandbox lane clippy + 466/466 (the pin
lives behind cfg(kittest-ui), not compiled in that lane — the lane's core
compilation is identical pre/post fix) · xtask 11/11 · feature-on clippy
lanes clean (core --features kittest-ui; app-shell --features kittest-ui
with the two PRE-EXISTING `too_many_arguments` lints in
kittest_list_pages.rs:135/430 exempted — a lane CI never clippy-gates, both
outside this diff, disclosed not fixed) · kittest lane diff-locked GREEN
twice on the final code (all 28 suites incl. the new clock suite; 171
goldens, ZERO .diff.png, ZERO golden churn — pinned rendering matched the
Nov-7-regenerated baselines byte-for-byte) · clock suite 4/4 in BOTH
parallel and --test-threads=1 modes (order-independent, self-sufficient
tests) · SPP_SMOKE=1 rc=0 (24 pages / 4338 shapes — identical to the UXB-1
baseline; the production binary compiles the feature off, so the only
production-file change, business_hours.rs, is semantically unchanged there:
`effective_now()` inlines to `Utc::now()`).

Zero-churn proof chain (why the freeze holds on ANY calendar day, not just
today): the clock suite proves every render-path read returns the pinned
anchor exclusively (the wall clock never reaches the screenshots); the
diff-locked runs prove pinned rendering equals the committed baselines;
therefore on any future day the rendering is bit-identical to today's. The
wall clock would have diverged at the next UTC midnight (the regen was
2026-11-07 00:35:58Z; the disclosed 10d→11d glyph tick fired at the
previous midnight under the same mechanics).

- Scope: `crates/core/src/business_hours.rs` (feature-gated pin +
  `effective_now`) + the `kittest-ui` feature wiring (core `Cargo.toml`
  gains the feature; app-shell forwards to it — no `Cargo.lock` delta,
  verified) + `crates/app-shell/tests/` (kittest_sync_health_page,
  kittest_operations_page, kittest_notifications_page pin in
  `seeded_page()`; new kittest_clock_pin.rs) + ledger/reports. No golden,
  CI, workflow, packaging or production-code changes (`--locked`
  throughout; reports pages' wall-relative breakdown defaults left alone —
  empirically not rendered in any golden, CI-green across regen-day
  boundaries, and pinning them would churn untouched baselines).

With UXB-T1 the last open finding of the UX-audit program (including its
test-infra tail) is closed. Rule 8 STOP — the queue is empty; resume only
on a new owner directive.

CI outcome addendum (appended to the UXB-T1 record, append-only): the
2-commit push (aa23d21..0051a42) went green 3/3 on the first push —
fmt/clippy/test/deny ✓ · AppImage + .deb build smoke ✓ · **kittest snapshot
locks ✓** (all suites incl. the new `kittest_clock_pin` passed on CI's own
lavapipe runner against the UNCHANGED committed baselines — the pinned
rendering matched byte-for-byte on a different machine/rasterizer, which is
the freeze holding cross-environment; notably CI's lane now survives any
calendar day, the very property this batch was opened for) — CI run
42331436125, completed success. The GA-1 standing practice (cargo fmt --all
-- --check as the final pre-push local step) held. The UX-audit program,
including its test-infra tail, is fully closed; Rule 8 STOP.

---

## Batch REL-4 — Build & Release v1.9.5 from the final source (2026-11-07)

**Owner directive:** build, test, and publish install-ready Linux `.deb` and
`.AppImage` packages from the final repository source; versioned GitHub
Release with both packages attached, checksums, compatibility, and verified
download links; packages must be self-contained and launch-ready with no
dev tools, manual dependency handling, or first-run setup (same program as
REL-1/2/3).

**Items:**
- REL-4-1: workspace version 1.9.4 → 1.9.5 (Cargo.toml + Cargo.lock).
- REL-4-2: CHANGELOG `[1.9.5]` section in product language + link reference.
- REL-4-3: annotated tag `v1.9.5` on the release commit (after CI green).
- REL-4-4: publish via release.yml + live post-publish verification.

**Release commit:** `9afef58` (main tip at push; carries UXB-1
UX-26B/34/35 + UXB-T1 — none of it in any published release; v1.9.4
shipped `f35c463`).

**Local verification battery (FULL, unlike the REL-3 recycle — this
sandbox had Rust 1.99.1 and enough disk; fifth sandbox recycle, repo
re-cloned at fa663b3):**
- Version bump: Cargo.toml edit + `cargo metadata` regeneration; text diff
  shows exactly the four member-package version lines (app-shell, catalog,
  core, xtask → 1.9.5), zero stray changes; zero stale `1.9.4` strings
  outside ledger/CHANGELOG history.
- Gates: no-simulation clean, traceability OK (23 routes, 20 design.rs DC
  tags, 27 DC ids).
- fmt `--all --check` clean; clippy `--locked --workspace --all-targets
  -D warnings` clean (3m02s); sandbox-feature clippy lane clean (40.8s).
- Tests: core 458/458 (+3 ignored), app-shell lib 246/246, style_lint
  integration 2/2, xtask 11/11, sandbox lane 466/466 (+3 ignored) —
  matching the UXB-1/UXB-T1 baselines exactly.
- Release build on the EXACT release profile (fat LTO, codegen-units=1,
  opt-level=s, strip; sandbox feature on): the link OOM-killed once with
  the sandbox's co-resident Next.js dev server (~800 MB RSS); stopped it
  (unrelated standing environment process, restartable) and the link went
  green in 3m03s — 30.9 MB binary.
- SPP_SMOKE rc=0 on the release binary: 54 migrations, boot services,
  background workers, loopback listener, automatic backup, 24 pages /
  4338 shapes (identical to the UXB-1/T1 baseline).
- `.deb` via `xtask deb --skip-build`: 9,354,888 B local build;
  dpkg-deb metadata (Package supportos-plus, Version 1.9.5, amd64, the
  19-dep Depends set led by `libc6 (>= 2.35)`), contents (binary +
  desktop entry + copyright + hicolor icons ×4), extract + SPP_SMOKE
  rc=0 from the extracted tree.
- AppImage preflight: `xtask package --skip-build` staged the AppDir and
  stopped exactly at `ensure_host_libs_bundled` (the sandbox host lacks
  libxkbcommon-x11/libxcb-xkb and has no root — disclosed); the three
  BUNDLED_HOST_LIBS were placed from the ubuntu:22.04 archive
  (libxkbcommon 1.4.0-1, libxcb-xkb 1.14-3ubuntu3) exactly as the step
  copies them; appimagetool 1.9.1 downloaded at xtask's pinned URL and
  SHA256-verified; AppImage built (12,818,936 B local) and
  APPIMAGE_EXTRACT_AND_RUN + SPP_SMOKE rc=0.
- release.yml extraction simulation: version awk → 1.9.5; changelog
  section awk → the full 35-line `[1.9.5]` section.

**CI on the release commit:** run `42359689689` — 3/3 green (fmt/clippy/
test/deny, AppImage + .deb build smoke, kittest snapshot locks on the
pinned clock) before tagging.

**CI Release run `42361423228` (tag push, 6/6 jobs green):**
- build + test + package (ubuntu:22.04, glibc 2.35 baseline): gates, fmt,
  clippy ×2 `--locked -D warnings`, workspace tests ×2, release build
  with the Landlock/seccomp parse sandbox, xtask AppImage + .deb
  packaging, dist staging with SHA256SUMS.txt + BUILD-INFO.txt.
- verify .deb on clean ubuntu:22.04, ubuntu:24.04, debian:12 containers:
  checksum integrity, APT install with automatic dependency resolution,
  package metadata + desktop-file-validate + all four icons, SPP_SMOKE
  boot chain, 0.2.x→1.x data-directory upgrade migration (in-place, data
  preserved), GUI launch under Xvfb with only declared dependencies,
  reinstall, uninstall (user data preserved), purge.
- verify AppImage (ubuntu-latest): checksum integrity,
  APPIMAGE_EXTRACT_AND_RUN boot smoke, GUI launch under Xvfb.
- publish-after-green: release created, then every asset URL
  re-downloaded in-workflow and checksum-verified against the uploaded
  bytes.

**Live post-publish verification (this sandbox, from the published
bytes):**
- Release `v1.9.5` = "SupportOS+ 1.9.5", draft=false, prerelease=false,
  target_commitish `9afef580fb4d…` == release commit == main tip; tag
  peels to the same commit (`git rev-parse v1.9.5^{commit}`).
- All four assets downloaded live (AppImage 12,806,648 B; .deb
  10,208,482 B; SHA256SUMS.txt; BUILD-INFO.txt); `sha256sum -c`: both
  packages OK (AppImage `85c319f8…cafed`, .deb `6dc0915b…b731`).
- BUILD-INFO.txt: version 1.9.5, commit `9afef580fb4d…`, built on
  ubuntu:22.04 at 2026-11-07T10:14:49Z.
- Published `.deb` re-inspected (Package/Version/Architecture, 22
  usr-tree files incl. binary + desktop + 4 icons) and extracted;
  SPP_SMOKE rc=0 from the published bytes (54 migrations, 24 pages /
  4338 shapes, first-run DB self-initialization).
- Published AppImage: APPIMAGE_EXTRACT_AND_RUN + SPP_SMOKE rc=0 —
  identical boot chain from the self-extracted runtime (AppRun path
  resolution, bundled libs, first-run init).
- Release notes: 11/11 content checks — `[1.9.5]` changelog section
  (prepended), both exact SHA-256 hashes, commit binding, both install
  command blocks, system requirements (x86_64, glibc 2.35+, Ubuntu
  22.04 LTS+/Debian 12+), APT auto-dependency statement, CI-verified
  list, UX-26B/UX-35 product lines.

**Environmental limits disclosed (honest scope):** no sudo/root in this
sandbox → the APT install/upgrade/uninstall lanes, desktop-file-validate
and the Xvfb GUI launches ran only in CI's clean containers (green ×3
distros + ubuntu-latest, on the exact published artifacts); the AppImage
host-libs bundling step ran locally with ubuntu:22.04-archive libs (CI
performs it natively in the ubuntu:22.04 build container — the published
bytes are CI's, not the local preflight's); the fat-LTO link required
stopping the sandbox's standing Next.js dev server (memory, not
correctness); debug build artifacts were removed after all test lanes
passed (disk pressure; release artifacts and evidence unaffected).

Rule 8 STOP — the queue is empty; the next work item requires a new owner
directive.

CI outcome addendum (appended to the REL-4 record, append-only): the
ledger-close push (9afef58..37eb33a) went green 3/3 on the first push —
fmt/clippy/test/deny ✓ · AppImage + .deb build smoke ✓ · kittest snapshot
locks ✓ (the pinned clock held again, now two calendar days of CI runs
past the Nov-7 regen) — CI run `42363630837`, completed success. Rule 8
STOP — the queue is empty; the next work item requires a new owner
directive.

## Batch UXD-1 — 2026-11-07 — Dashboard layout repair (owner screenshot report)

**UXD-1-1 — design-system width discipline.**
- Work: `metric_card_w` (and the `metric_card` 150px default) now treat their
  width as the card's OUTER width — the 14px inner margin AND the 1px stroke
  on each side (egui frame sizing: `content + inner_margin + 2·stroke.width`)
  are subtracted inside; the old inner-width semantics made every card 30px
  wider than the callers' column math, so a 4-column KPI row overflowed by
  90px and clipped its last column at every width. `data_table` now computes
  INTEGER exact-fit column widths (`min_col_width` = `max_col_width` =
  `floor(exact)`), hosts its grids in a Ui pinned to the card's inner width,
  and lays the header strip on the same bounded grid (headers sit over their
  columns; long cell values wrap instead of widening columns; the old
  unconditional 90px floor let a 9-column table exceed its card below
  ~950px, and fractional column rects each rounded up ~0.5px — a measured
  +4px page-bounding-box drift that fed the scroll-runaway). New
  `card_exact_wh`/`card_titled_exact` helpers (controls.rs) land a card on
  an exact outer width (and optional exact min height) with the inner Ui
  CAPPED (`set_max_width`) so width-filling children (bar rows, charts) see
  the card's width, never the unbounded remaining panel width egui hands
  out inside Grid cells and horizontal rows.
- Root cause (from the owner's screenshot + the checked-in goldens, which
  had baked the broken layout in): `bar_row`/`data_table`/`metric_card_w`
  read `ui.available_width()` inside `egui::Grid` cells, where egui 0.37
  hands cells the unbounded remaining panel width (last column: to the
  grid's right edge; other columns: the previous frame's grown column width
  — a positive feedback loop), so breakdown bars and KPI cards inflated
  frame over frame until columns left the viewport; combined with the page
  `ScrollArea::auto_shrink([false; 2])` persisting the inflated content
  width, the whole page (chart caption included) ran off-screen — exactly
  the owner's "cards enlarging or moving to the right side, few cards out
  of screen".

**UXD-1-2 — dashboard restructure (research applied).**
- Research: 2024–25 helpdesk/analytics dashboard pattern (Help Scout,
  Zendesk, Linear, Stripe, Vercel — LLM design-spec pass): KPI stat row →
  trend chart + insight side panel (65/35-class split) → equal-width
  breakdown grid → full-width data tables; equal widths, left alignment,
  consistent gutters, capped bar lists; responsive tiers for desktop
  800–2560px.
- Work (`crates/app-shell/src/pages/dashboard.rs`): responsive KPI grid
  (2/3/4/6 columns by content width; 12 tiles regrouped volume → workload
  → responsiveness → happiness; the unit folded into the label so every
  tile is two lines and rows align; 12 divides by 2/3/4/6 so no tier leaves
  a ragged row); 62/38 hero row (New-conversations chart + Issue-radar
  panel side-by-side from 1000px, the radar card stretched to the chart
  card's measured height; stacked full-width below); breakdowns on an
  equal-width top-aligned 1/2/3-column grid (`horizontal_top` rows — the
  old `egui::Grid` vertically centered shorter cells mid-row) with the
  UX-28 eight-bar cap kept; channel-metrics + mailbox-comparison tables
  full-width below (headers Title-Case); the by-channel card's dev-facing
  `dashboard.rs:337-345` note reworded to product language ("every channel
  is listed — the Channel filter scopes the other cards, not this one");
  all width math floored to integers (egui rounds fractional widget rects
  per column and the drift accumulates). `reports.rs` KPI grid re-floored
  to the new outer-width semantics.

**UXD-1-3 — verification (full battery, all green).**
- Gates: no-simulation ✓, traceability ✓ (23 routes, 20 DC tags).
- fmt `--all --check` ✓; clippy ×2 `--locked -D warnings` (workspace
  `--all-targets` 51s; sandbox lane 39s) ✓.
- Tests: core 458/458 (3 ignored, baseline match) ✓; app-shell lib 246/246
  ✓; style lint 2/2 ✓; xtask 11/11 ✓; sandbox lane 466/466 ✓;
  `verify-icons` ✓.
- kittest: 173 goldens (171 + two NEW 1920 light/dark dashboard locks —
  the owner's report was a wide window). Regen (UPDATE_SNAPSHOTS=1) for the
  helper-affected suites (dashboard, reports, knowledge, incidents,
  list_pages=customers+organizations, widgets) then **diff-locked ×2, zero
  drift across all 24 suites**.
- Objective bounds proof (pixel probes on every changed golden, alpha
  channel): dashboard goldens at 800/1024/1440/1920 × light/dark + scope
  chip + empty-DB probes at 700/1440/2000 — opaque content bounds
  left=8/right=8 EXACTLY (the kittest central-panel frame fill strip that
  betrayed the old bounding-box overflow is gone); KPI tiles measure 227px
  ×6 uniform, hero cards 876+536, breakdown cards 466×3 uniform (1440
  dark). VLM read-backs: structural reads confirm the 6-col KPI grid,
  chart+side-panel hero, equal-width breakdown rows, product-language
  note, clean 10px gutters ("PASS" on the 800 light read); the model's
  right-edge-clipping claims on 1440/1920 are disproven by the pixel
  probes above (known VLM edge hallucination; the caption it flagged sits
  8px inside the chart rect by construction) — pixel measurement is the
  primary evidence, disclosed here as such.
- SPP_SMOKE rc=0 on the debug binary (54 migrations, 24 pages, 4332
  shapes — was 4338; the restructure paints 6 fewer shapes, the count is
  informational, no assertion).
- **Newly-diagnosed, NOT fixed in this batch (pre-existing, disclosed):**
  pixel probes show pre-existing right-edge overflow (identical bounds at
  HEAD) on Reports 800/1024 (its own `table_stretched` helper keeps the
  unconditional 90px floor + fractional columns — same defect class as the
  fixed `data_table`), Incidents 800 + incidents/customers/organizations
  detail goldens, and customers_detail (left=0). All bounds match HEAD
  exactly — zero regression from this batch; candidates for a follow-up
  batch on owner direction.
- Environment notes: fifth sandbox recycle — Mesa lavapipe re-extracted
  to /home/z/mesa-lvp (apt-get download + dpkg -x, no root, env script
  committed to nothing); disk pressure twice (kittest link bus-error at
  100% full) — cleared by deleting target/release + target/debug/
  incremental (1.9–2.8GB), CARGO_INCREMENTAL=0 for the test lanes; the
  600s workspace-test single run was split per-crate instead.

Rule 8 STOP — the queue is empty; the next work item requires a new owner
directive.

CI outcome addendum (appended to the UXD-1 record, append-only): the
batch push (b9149ac..1e15797) went green on the first push — CI run
`42375985087`, completed success (fmt/clippy/test/deny ✓ · AppImage +
.deb build smoke ✓ · kittest snapshot locks ✓ — the 171 regenerated + 2
new goldens held cross-runner through lavapipe, and the pinned shot clock
held for a third calendar day of CI runs past the regen). Rule 8 STOP —
the queue is empty; the next work item requires a new owner directive.

## Batch UXE-1 — full-app design modernization (closed 2026-11-07/08)

Opened on the owner's directive (screenshot report + "check the sidebar,
the reply editor at the bottom, basically most of the layouts are not
correctly designed, gap, padding, label alignments too many issues …
should not look like different pages are designed differently … make it
professional and modern, it is too much old school like an ancient build
… should work fine on every monitor/display … make sure to fix every
design issues"). Diagnosis (VLM audits of the owner screenshot + the
current goldens, converged): mixed casing/label conventions per page
(100+ lowercase `label:` field labels, lowercase buttons), three page-
header patterns, native egui ComboBox/checkbox controls reading
Win95-era, saturated full-fill callout cards, a copilot panel and
composer that clipped on 1024-class displays (the composer defect
PRESENT IN THE PRE-BATCH GOLDENS: the auto-sized bottom panel converged
to a sliver — UX-27's scroll bound never actually contained it), fixed-
width inbox columns (views 172 + list 330..640 + copilot 320), and no
window minimum size.

Items (all ten closed):

- **UXE-1-1 — tokens & cards**: TITLE 22→18 / SECTION_BIG 18→16 (frozen-
  scale test updated with intent), one radius scale (CARD 10 / CONTROL 8
  / BADGE 6, widget visuals on CONTROL), borderless elevated cards in
  light mode (soft shadow; dark keeps the hairline, drops the shadow),
  page_header rhythm (SP3 after the header on every page), card diet in
  message cards (16/12/13 margins → 14/11/12).
- **UXE-1-2 — shell chrome**: brand row with inline version pill (was a
  two-line block), one micro-label convention (11px muted sentence case
  for sidebar groups AND the inbox views rail — "INBOX VIEWS" → "Views"),
  30px nav rows with strong selected labels, search trigger on the nav
  grid at 32px, theme row pinned without the orphan hairline, top bar
  52→48px.
- **UXE-1-3 — control set**: `design::select` (styled 32px dropdown,
  hairline border, muted chevron, card-styled `Popup::menu`, accessible
  name — bare `WidgetInfo::new(ComboBox)` failed the kittest
  accessibility check), `design::switch` (deterministic pill-and-knob
  boolean), `design::segmented` (muted track + raised active segment);
  controls radius via widget visuals; Title Case sweep ("apply"→"Apply",
  "filters"→"Filters", "next ›"→"Next ›", "ask"→"Ask", "thinking…"→
  "Thinking…", checkbox labels).
- **UXE-1-4 — responsive architecture**: window `min_inner_size` 980×600;
  width-aware inbox columns — the views rail folds into chips below 900px
  of content width (and folds to keep a docked copilot viable), the list
  is proportional 280–560 resizable, the copilot docks at 300px while
  the reader keeps a 480px floor and otherwise SWAPS IN over the reader
  (narrow mode) instead of crushing it; the composer steps aside with
  the reader in narrow mode. New goldens pin all three states:
  r5_inbox_copilot_docked_1920_light, r5_inbox_copilot_narrow_1024_light,
  r5_inbox_folded_views_800_light.
- **UXE-1-5 — inbox list & views**: folded-views chips, styled selects
  for mailbox/channel/tag, the search row reserves the `#` input and
  Apply widths so the primary row composes on one line at every legal
  list width, row anatomy with a 3px selection accent bar, "Reset
  Filters"→"Reset", 10px row padding, en-dash ranges.
- **UXE-1-6 — reader**: `Sides::shrink_left` header (cluster anchored
  right, bounded truncated subject/meta — the old left-to-right row let
  a long subject push the cluster off the edge at 800px), the toolbar as
  two explicit rows (Status segmented control over Assignee select +
  Assign; egui cannot wrap opaque nested groups — the wrapped-group
  attempt clipped the Assign button at 800px), meta line de-logged
  ("mailbox: X · priority: none" → "X · Priority: none"), note/AI
  message cards swap full colored borders for 3px left accent bars.
- **UXE-1-7 — composer**: one card — segmented Reply|Note header +
  Saved-Replies dropdown (real `Popup::menu`, not the inline frame) +
  AI Draft; To/Cc/Bcc recipient rows on aligned 28px label columns
  before the body; AI tools (Review Draft/Shorten/Expand/Warmer/
  Translate + language select) after the body; footer hint + primary
  Send. ROOT-CAUSE FIX: the composer bottom panel now tracks the card's
  measured height (`composer_panel_h`, exact_size, window-capped) — the
  old auto-sized panel converged to a sliver and clipped the composer
  at the window bottom on EVERY width (pre-existing in the goldens;
  exactly the owner's screenshot complaint).
- **UXE-1-8 — copilot**: "Local" badge in the header, the "(s)" dev
  string pluralized properly ("N past conversation(s)" → conditional
  plural), `cop_suggest` on the accent-bar anatomy (no full colored
  border).
- **UXE-1-9 — cross-page pass**: reports drops the floating-label tab
  groups for ONE wrapped underline tab row (long tab sets wrap instead
  of clipping — `tabs_row` now `horizontal_wrapped`); settings AI
  behaviour/Help Scout/vector-index rows become switch rows; 100-label
  sentence-case sweep across 13 pages (`name:`→"Name", `base url:`→
  "Base URL", `api key:`→"API key", …); "(s)" plurals fixed in
  automation/diagnostics/incidents/issue_radar/ai_center (+ the
  incidents test assertion updated).
- **UXE-1-10 — verification** (below).

Verification (full battery, 2026-11-07/08):

- Gates: fmt `--check` ✓; clippy workspace `--all-targets` 0 warnings ✓;
  clippy kittest lane: only the two PRE-EXISTING `too many arguments`
  warnings in `kittest_list_pages.rs` (verified present on HEAD via
  stash; the file is `kittest-ui`-gated so CI's deny-warnings lane never
  compiles it — not from this batch); clippy sandbox lane `-D warnings`
  ✓.
- Tests: core 458 ✓ · sandbox feature 466 ✓ · app-shell lib 246 ✓ ·
  style lint 2 ✓ (label sweep is on-scale by construction) · xtask 11 ✓.
- kittest: full golden regeneration (173 modified + 3 new responsive
  goldens = 176 snapshots across 24 suites) then **diff-locked ×2, zero
  drift**.
- SPP_SMOKE rc=0 on the debug binary (54 migrations, boot services,
  workers, 24 pages rendered headlessly · 4326 shapes — was 4332; the
  restructure paints 6 fewer shapes, count is informational, no
  assertion).
- VLM read-backs (kittest goldens): inbox 800 folded-views **PASS**,
  1024 narrow-copilot **PASS**, 1440 light **PASS** + dark **PASS**,
  1920 docked — no clipping/overlap (the reviewer's "garbled summary"
  reads are the fixture's summary+questions lists and the quoted
  customer text, not layout); settings 1440 **PASS** (switch rows read
  cleanly); widgets composer golden **PASS** (full card: header,
  divider, To row, textarea, tools, footer, Send). The reviewer's
  repeated "Reports title clipped at the left edge" claim is DISPROVEN
  by pixel probes: title leftmost text pixel x=10, subtitle x=9, zero
  dark pixels in columns 0–6 — the identical hallucination the UXD-1
  record documented on the same page; pixel measurement is the primary
  evidence, disclosed here as such.
- Debug-loop disclosures (all fixed in-batch): the first composer
  rewrite's `with_layout` reader-head consumed the reader's height
  (caught by VLM read-back + pixel probes, rewritten on `Sides`);
  the wrapped-toolbar attempt clipped the Assign button at 800px
  (egui cannot wrap opaque nested groups — rewritten as two explicit
  rows); the first `select` widget_info lacked an accessible name
  (kittest accessibility check caught it).
- Known-remaining (pre-existing, NOT from this batch, disclosed): the
  reader's scroll-hosted cards ride ~22px wider than the composer card
  at the right edge (reader card right ≈ x=791 vs composer ≈ x=769 at
  800px; identical bounds in the pre-batch goldens — an egui
  ScrollArea/scrollbar-gutter behavior, nothing clips the window);
  candidate for a follow-up batch on owner direction.

Rule 8 STOP — the queue is empty; the next work item requires a new
owner directive.

CI outcome addendum (appended to the UXE-1 record, append-only): the
batch push (9acb6fd..e53d680) went green on the first push — CI run
`42389418657`, completed success (fmt/clippy/test/deny ✓ · AppImage +
.deb build smoke ✓ · kittest snapshot locks ✓ — the 173 regenerated + 3
new responsive goldens held cross-runner through lavapipe, and the
pinned shot clock held). Rule 8 STOP — the queue is empty; the next
work item requires a new owner directive.

## Batch UXE-2 — post-recycle verify-and-fix (closed 2026-11-08)

Opened on the owner's post-recycle directive ("you have to fix not
change. you have to fix what we have to make sure that is working as
expected, introducing new things are totally prohibited" — issued after
a proposed tech-stack change was rejected; the fix stayed inside the
existing Rust/egui codebase and design system, zero new dependencies,
zero new components). Fifth sandbox recycle: repo re-cloned at 8a89ebd
(main tip = the UXE-1 close), Rust 1.99.1 + lavapipe/xkb prefix rebuilt
first (~/.local/opt/vulkan-libs, the established recycle procedure).

Items (all four closed):

- **UXE-2-1 — post-recycle HEAD verification**: the full battery,
  unchanged, on HEAD before any edit — no-simulation + traceability
  gates ✓; fmt `--check` ✓; verify-icons ✓; clippy workspace
  `--all-targets -D warnings` 0 ✓; clippy sandbox lane `-D warnings` 0 ✓;
  clippy kittest lane: only the two pre-existing `kittest_list_pages.rs`
  `too many arguments` warnings (the known state; CI's kittest lane runs
  tests, not clippy); workspace tests 717 ✓ (core 458 · xtask 11 · style
  lint 2 · app-shell lib 246); sandbox-feature tests 466 ✓; kittest
  diff-locked ×2 — 28 suites, 430 tests, ZERO drift against the committed
  UXE-1 baselines through the rebuilt lavapipe (cross-recycle
  reproducibility of the design-modernization goldens proven on this
  machine); SPP_SMOKE rc=0 (boot chain + shell/renderer). The UXE-1
  state works as expected after the recycle.
- **UXE-2-2 — golden read-backs** (VLM + pixel probes as primary
  evidence): r3 inbox 800 light — cards aligned, no edge clipping; the
  reviewer's search-row "overlap" claim DISPROVEN by zoomed re-read
  (placeholder ellipsis inside its own field); the "odd yellow strip"
  at the fold pixel-proven to be the next note card's rounded top cut
  by the scroll viewport (corner-arc geometry + edge fade, zero text
  pixels). Unchanged goldens (1024/1440 light+dark, 1920 docked,
  narrow-copilot, folded-views) are pixel-identical to the UXE-1-verified
  baselines by the zero-drift lock — stronger than re-reading them.
  r2c composer golden: the reviewer's "stray tools / sent_text= strings"
  and "missing Cc/Bcc" are TEST-FIXTURE artifacts, verified against the
  fixture source (placeholder closures + the send-behavior readout in a
  720×520 harness; the real composer's To/Cc/Bcc + AI-tools rows render
  in the r3 goldens).
- **UXE-2-3 — the disclosed follow-up candidate, resolved
  evidence-first**: pixel probes DISPROVE UXE-1's "known-remaining"
  reader/composer right-edge misalignment on HEAD — reader cards and
  the composer card land on the same right edge at every width (769/770
  at 800, 993/994 at 1024, 1409/1410 at 1440); the disclosed 791-vs-769
  bounds exist only in the PRE-UXE-1 goldens (verified from git history
  at 9acb6fd) — the disclosure was a stale pre-batch measurement, and
  egui 0.37's default floating ScrollStyle (`floating_allocated_width:
  0`) means no content reflow when the reader's bar appears either. The
  REAL defect the read-backs found in the same component: the Note
  message card's avatar circle was INVISIBLE (`t.warning_soft` circle on
  a `t.warning_soft` card fill — bare floating initials, the one role
  breaking the avatar anatomy; zoomed VLM read + pixel probes). Fixed in
  `design::message_card_with`: the Note avatar is now the card-surface
  chip `(t.card, t.warning_foreground)` — clearly visible in light
  (white circle, amber initials), subtly visible in dark exactly like
  the Customer role's quiet chip, no new tokens. Plus ONE new regression
  golden `r5_inbox_reader_overflow_800_light` (16-entry thread; the
  fold falls mid-card) pinning the previously un-exercised
  reader-overflow state: cards keep the composer card's exact width at
  the fold (pixel-proven 769/770 both regions), the cut is a clean
  rounded-corner scroll clip (the reviewer's "squeezed" claim disproven
  by the corner-arc pixels). Golden drift: exactly 7 baselines
  regenerated (the note-card-bearing goldens: r2c_inbox_identity
  light+dark, r3_inbox 800 light / 1024 light+dark / 1440 light+dark;
  r3_inbox 800_dark unchanged — its note card rides below the fold) + 1
  new baseline.
- **UXE-2-4 — post-fix battery**: gates ✓; fmt ✓; verify-icons ✓;
  clippy workspace `-D warnings` 0 ✓; clippy sandbox 0 ✓; clippy kittest
  lane: the two pre-existing warnings only ✓; workspace tests 717 ✓;
  sandbox tests 466 ✓; kittest diff-locked ×2 on the regenerated
  baselines — 28 suites, 431 tests (430 + the new overflow lock), zero
  drift both runs ✓; SPP_SMOKE rc=0 on the rebuilt binary ✓.

Rule 8 STOP — the queue is empty; the next work item requires a new
owner directive.

CI outcome addendum (appended to the UXE-2 record, append-only): the
batch push (8a89ebd..dd9ea29) went green on the first push — CI run
`42413290650`, completed success (fmt/clippy/test/deny ✓ · AppImage +
.deb build smoke ✓ · kittest snapshot locks ✓ — the 7 regenerated
note-card baselines + the new r5_inbox_reader_overflow_800_light lock
held cross-runner through lavapipe). Rule 8 STOP — the queue is empty;
the next work item requires a new owner directive.

## Batch UXF-1 — AI Center layout repair + golden-eval functional fix (closed 2026-11-08)

Opened on the owner's "Fix these issues" directive (two fresh AI Center
screenshots: the Jobs tab — a narrow "Recent AI runs" card hugging ~35% of
the content column; the Usage tab — raw uncased sections and a 27-line
golden-set error dump). Sixth sandbox recycle: repo re-cloned at 1a60476
(UXE-2 close, CI run 42413290650 green), Rust 1.99.1 + lavapipe/xkb prefix
rebuilt, HEAD verified rendering-clean before any edit (ai_center suite
7/7, zero golden drift). Mid-batch the owner's scope correction ("it is
not the design issue, read the failed statements there") added UXF-2-1
(the functional root cause) and deferred UXF-1-4/UXF-1-5 back to the
queue. Items (all closed):

- **UXF-1-1 — Jobs tab**: the "Recent AI runs" card is now
  `card_titled_wide` (full content-column width) hosting a `data_table`
  (exact integer columns, header strip, 90px-floor-gated); the unbounded
  `show_rows` ScrollArea is gone; card header lands on one aligned
  baseline. Golden drift across the responsive proofs: the card spans
  [9,w-2] = 99.7–99.9% of the page area at 800/1024/1440 light+dark
  (pre-batch golden pixel-proven to carry the owner's defect — a
  630px card hugging left at 1440, span 0.44).
- **UXF-1-2 — Usage tab**: the three sections (Per-feature usage / Draft
  feedback / Golden-set evals) are full-width titled cards on one SP
  rhythm (pixel-proven 18px card gaps); the ledger is a `data_table`;
  empty states use the shared `empty_state_slim`; golden runs render as
  contained `golden_run_block`s — pass badge + backend + run meta on one
  row, per-case errors grouped into a mono list inside the block (the
  owner-reported 27-line dump state, collapsed).
- **UXF-1-3 — Health/Analytics/Attributes/Copilot tabs**: same
  `card_titled_wide` width discipline; all six tabs share one anatomy
  (the six regenerated page goldens lock it at 800/1024/1440 light+dark).
- **UXF-2-1 — the REAL failure in the owner's screenshots** (owner scope
  correction): every golden_eval / ticket_analysis run died with
  `config error: LM Studio chat request failed (HTTP 400 Bad Request):
  {"error":"response_format.type' must be 'json_schema' or 'text'"}` —
  `chat_with_opts` sent OpenAI's legacy `{"type":"json_object"}`, which
  LM Studio rejects. json_mode now sends the portable `json_schema` form
  (`{"type":"json_schema","json_schema":{"name":"structured_output",
  "schema":{"type":"object"}}}`, `strict` omitted); wire format locked by
  the updated it_lm_studio roundtrip test + a new end-to-end regression
  lock (`golden_set_passes_against_an_lm_studio_strict_response_format_server`
  — a mock server that 400s like LM Studio unless the schema form is
  used). Affects every json_mode caller: chat_json, chat_structured,
  chat_qa json_mode lanes.
- **UXF-1-6 — responsive proof**: pixel probes at 800/1024/1440 light+dark
  (card span ratios, margin symmetry, table-spans-card: the Jobs header
  strip = card width − 2×14px padding, exact at both widths) + VLM
  read-backs (Jobs seeded 1440+800 full-table audits, incidents/customers
  trio, diagnostics, copilot docked 1920, usage failing-run). Adjudicated
  read-back flags: ID right-alignment under a left header = GitHub
  standard (focused re-read: ACCEPTABLE); "page title clipped at top" =
  harness artifact (ink starts y=13, the real app carries the 48px chrome
  above); the inbox fold "yellow fragment" = the UXE-2-2 pre-adjudicated
  scroll-viewport corner-arc cut; the "void below content" = a page
  shorter than the 900px viewport.
- **UXF-1-7 — new golden locks**: seeded Jobs-tab goldens (12 failed
  golden_eval rows in the table) at 1440+800 and Usage-tab failing-run
  goldens (the 0/9 LM-Studio-400 run) at 1440+800 — both fixed states
  diff-locked forever.
- **UXF-1-8 — full battery**: gates (no-simulation, traceability) ✓; fmt
  ✓; verify-icons ✓; clippy workspace `-D warnings` 0 ✓; clippy sandbox
  lane 0 ✓; workspace tests 718 ✓ (core 459 incl. the new regression
  lock · app-shell lib 246 · xtask 11 · style lint 2); sandbox tests 467
  ✓; kittest diff-locked ×2 on the final baselines — 25 suites + lib,
  435 tests, ZERO drift both runs, zero artifacts ✓; SPP_SMOKE rc=0 ✓.

**Two shared-system root-cause fixes surfaced by this batch's
verification** (the golden-eval work migrated the Jobs/Usage tables onto
`data_table`, and the read-backs caught what the old natural-width
rendering had hidden for every table in the app):

1. **`design::badge` / `chip_impl` consumed a phantom second Grid cell** —
   both called `allocate_space` AND `advance_cursor_after_rect`; linear
   layouts set the cursor absolutely from the rect (the second call is an
   idempotent no-op — which is why every non-grid golden stayed
   pixel-identical), but egui Grids advance INCREMENTALLY per allocation,
   so a badge/chip in a table cell ate the next column: every following
   cell shifted right and the last column clipped at the window edge (the
   Jobs `created` column showed one character at 1440; the same latent
   defect existed in the customers conversations table, the connectors
   chip column, and three diagnostics tables). Fix: drop the redundant
   advance (one widget, one cell). 61 goldens regenerated across the
   affected suites (ai_center, customers/organizations/incidents,
   connectors, diagnostics, knowledge help-center tab, dashboard 800,
   inbox 800 + reader-overflow + copilot states, widgets demo) — each
   verified by pixel probe or VLM read-back as the intended realignment.
2. **`data_table` header/body grids started flush at the table stroke**
   — labels ~2px from the border, last column butting the right edge.
   Fix: CELL_PAD 10px inner margins on the header strip and the body
   frame (one shared pad so the exact integer columns still line up; the
   column math sizes against the padded cell area). Pixel-proven: "ID"
   label now 14px inside the strip, header and body column origins agree
   within glyph-bearing noise, created text ends 65px inside the padded
   cell edge at 1440.

Also fixed en route: the new UXF-1-7 fixture seeded `golden_test_runs`
without `started_at`, letting the column DEFAULT (wall-clock now) drift
the rendered raw stamp run-to-run — the exact nondeterminism class
UXB-T1 closed for the other suites; the fixture now pins the stamp
explicitly (2026-11-05T23:52:07.412Z) and the suite is diff-locked ×2.

**Queue candidates for the owner (pre-existing, out of this batch's
scope, verified present in the pre-batch goldens): customers "Last
Contact" raw-ISO timestamps ellipsize in their 140px column; the
incidents page header shows the incident count twice (subtitle +
page_counter). Alongside the deferred UXF-1-4 (chrome header balance)
and UXF-1-5 (legacy `design::table` width sweep).**

Rule 8 STOP — the queue is empty; the next work item requires a new
owner directive.

CI outcome addendum (appended to the UXF-1 record, append-only): the
batch push (1a60476..e40dd63) went green on the first push — CI run
`42447950175`, completed success 3/3 (fmt/clippy/test/deny ✓ · AppImage +
.deb build smoke ✓ · kittest snapshot locks ✓ — the 65 regenerated
baselines + the 4 new UXF-1-7 locks held cross-runner through lavapipe).
Rule 8 STOP — the queue is empty; the next work item requires a new
owner directive.

## Batch UXG-1 — 2026-11-08 — All-display design discipline + cross-page consistency sweep (items UXG-1-1..UXG-1-10)

Opened on the owner's all-display directive ("work on all size of monitors
i use a 2k one and it should also work on 4k or 8k, and all the pages
should have been designed with same design principle, all the pages, all
the items. please do that make everything correct"); the batch also
consumed the four deferred queue candidates (UXF-1-4 chrome header,
UXF-1-5 table width-defect sweep, customers "Last Contact" raw-ISO,
incidents header count duplication). Everything inside the existing
Rust/egui codebase and design system — no new dependencies.

**UXG-1-1 — one content-column width discipline (the 2K/4K/8K fix).**
`design::scale::CONTENT_MAX` (1800) + `design::centered_column` /
`design::content_column` helpers (exact-`max_rect` child scope via
`scope_builder`: full width to the cap, a centered exact-width column
above it; below the cap the rect IS the available rect — pixel-identical
to an uncapped render). The shell's non-inbox page funnel wraps every
standard page in it, so all 23 standard pages share one width anatomy
from a 980px window to an 8K display: full-width background, centered
1800px column above ~2060px of window width. 1800 fits the dashboard's
6-column KPI tier (~295px tiles) and 7–9 column data tables.

**UXG-1-2 — inbox reader reading measure.** `design::scale::
READER_MEASURE` (860) — the reader pane's content (head, action bar,
thread, triage/QA cards) and the composer panel ride
`centered_column(READER_MEASURE)`, centered in the pane: the Gmail/Help
Scout wide-window register. Every ≤1440 golden state sits under the cap
and renders unchanged; 1625+ states cap (5 inbox goldens regenerated).

**UXG-1-3/4/5 — table width-defect sweep (UXF-1-5).** All 17 remaining
natural-width `design::table` / `table_stretched` page-level lists
migrated to `design::data_table` (exact integer columns, header strip,
gated 90px floor, virtualized rows — the redundant outer ScrollAreas
dropped): reports ×5 (SLA tab in a new full-width "SLA by mailbox"
titled card, builder outcome rows, insights friction + graph nodes,
happiness list), knowledge ×3 (documents, sources in a new titled card,
articles), operations (team workload), diagnostics ×3 (audit trail, sync
runs, outbound failures — the connectors list also gained the empty
state the other lists already had), automation (rules), connectors,
sync_health ×2 (checkpoints, runs), customers detail (conversations).
The legacy `table`/`table_stretched`/`table_impl` helpers are deleted —
zero callers remain; the design-smoke lib test renders `data_table`.

**UXG-1-6 — customers "Last Contact".** The raw-ISO `last_activity_at`
column renders through the shared `relative_time` helper ("270d ago",
"Yesterday", "Never") — no more ellipsized ISO strings in the 140px
column; deterministic under the kittest clock pin.

**UXG-1-7 — incidents header count dedup.** The subtitle no longer
repeats the count the `page_counter` pill already carries: the pill is
the one count register; the subtitle describes scope ("Track outages and
recurring problems across conversations") and only breaks into numbers
for the "Showing the first X of Y" truncation case.

**UXG-1-8 — chrome header balance (UXF-1-4).** The 48px top bar carries
the active page's label on the left (`PageKey::label`, LEAD strong) with
the same anatomy as `page_header` — the right-only toolbar read
unbalanced, worse the wider the display.

**UXG-1-9 — all-display locks + two verification-surfaced fixes.**
New locks: `uxg_dashboard_capped_2560_light/dark`, `uxg_3840_light`
(dashboard through the shell funnel composition: CentralPanel background
→ content_column → margin frame), `uxg_reports_capped_3840_light` (SLA
tab — the swept seven-column table at 4K), `uxg_reports_capped_
2560_dark`, `uxg_reports_sla_tab_1440_light`, `uxg_reports_insights_
tab_1440_light`, `uxg_inbox_reader_2560_light`; the 8K (7680) tier is
proven by exact pixel probes in the new `kittest_wide_displays` suite
(no multi-megabyte golden): the content column holds at CONTENT_MAX
centered, the reading measure at READER_MEASURE centered, and the real
inbox at 7680 keeps its reader subject inside the 860px measure and
centered (x > 3000). Verification surfaced and fixed two real defects:
(a) the reports SLA stats read the unpinned wall clock ("waiting oldest
(min)" ticked every real minute — the first SLA golden drifted +31
minutes between two runs 31 minutes apart); the reports fixture now
pins the kittest clock (UXB-T1 mechanism) to its own anchor, and the
default-tab goldens held unchanged; (b) the trend chart's 6px plot
inset let the line's endpoints land inside the corner-label bands
whenever the series' max sat at its first/last day — `plot_inner`
reserves the label bands (8/20/18), gridlines stay inside the plot, and
the chart unit tests pin the new geometry. The old
`r3_diagnostics_errors_filter` test was clicking a non-interactive
table-header label (its golden silently captured the UNfiltered log);
the contains-query now clicks the real "Errors N" chip.

**UXG-1-10 — full battery on the final code.** Gates (no-simulation,
traceability) ✓; fmt `--all --check` ✓; verify-icons ✓; clippy workspace
`--all-targets -D warnings` 0 ✓; clippy sandbox `-D warnings` 0 ✓; clippy
kittest lane: only the two pre-existing `kittest_list_pages.rs` warnings
(the documented known state) ✓; tests core 459 + app-shell 249 (incl. the
new chart-geometry tests) + catalog/xtask 11 = 719 passed 0 failed ✓;
sandbox-feature tests 467 ✓; kittest diff-locked ×2 on the final
baselines — 29 suites, 447 tests, 0 failed (61 regenerated + 8 new
goldens) ✓; SPP_SMOKE 24 pages · 4348 shapes ✓. Acceptance evidence:
pixel probes primary — the capped column centers within 1px at 2560
(gutters 402/403) and 3840 (1042/1043); the SLA data table's seven
values end exactly on their seven columns' right edges (right-aligned
register); the chart labels clear the line by 4px; VLM read-backs
confirm the chart fix, the clean composer boundary, and the 2K dark
dashboard (PASS) — the VLM's 4K centering/table-alignment claims were
contradicted by the exact pixel measurements (the documented VLM
measurement unreliability at large scales; UXE-2 precedent: pixel probes
are primary where a read-back is ambiguous).

