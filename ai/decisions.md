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
- 2026-10-19 — Post-audit remediation batch (maintainer: "fix everything
  and keep only the main branch"):
  - **DEFECT-01 fix:** sidebar rows are hand-allocated full-width widgets
    (`nav_row`, app.rs): `allocate_space` + `interact(Sense::click)` +
    `Style::interact_selectable` paint, a11y button role and interact
    cursor kept. The sensed rect IS the painted row — immune to egui
    0.37's atom hit-geometry anomaly on short galleys by construction,
    instead of depending on toolkit internals. GUI retest F1-fix-verify:
    all four dead rows navigate at x=75 and x=130.
  - **DEFECT-02 correction:** not a defect. egui 0.37 single-line
    TextEdit surrenders focus on Enter (builder.rs:119/:1331);
    `lost_focus` is live-computed (memory/mod.rs:872) and `key_pressed`
    is not consumption-filtered (input_state/mod.rs:767), so the audited
    web-port idiom works as written. Headless E2E
    `palette_enter_confirms_the_selected_hit` proves it against the
    original code. Audit record corrected; code unchanged.
  - **DEFECT-03 fix:** palette `open()` sets `wants_focus`; the first
    pass calls `request_focus()` on the query field (both open paths
    route through `open()`). Headless E2E proves focus + keystroke
    delivery with no click (pre-fix K1/K3: 0-pixel change). The GUI
    re-verification leg is BLOCKED-ENV: the sandbox janitor severed fresh
    sessions' X sockets within ~8 s of boot, four attempts (F2–F5),
    frozen-frame forensics committed with the evidence.
  - **Headless egui testing seam:** the palette E2E tests drive the real
    event loop (`Context::run_pass` with `RawInput` events against a
    seeded fixture DB, `FullOutput::drop_without_applying_deltas()`) —
    a reusable pattern for focus/keyboard-path proofs without a display.
  - **Branch policy:** audit branch pushed, merged fast-forward into
    main and deleted (local + remote) per maintainer instruction;
    single-branch main restored. Workspace after remediation: 306
    tests green, clippy/fmt clean, AppImage rebuilt.
- 2026-10-19 — Release engineering batch (maintainer: audit the repo with
  build-relevant code as the only source of truth; rename to SupportOS-Plus /
  SupportOS+ where allowed; build, test and publish install-ready .deb +
  .AppImage via GitHub Actions):
  - **Audit:** README claims re-verified against source (crates, pages,
    data dir, boot chain, packaging). Naming layers defined: SupportOS+
    display name (window title, desktop Name, release title), SupportOS-Plus
    identifier (repo, app_id/StartupWMClass, artifact base), supportos-plus
    Debian package, supportos-plusplus internal crate/binary identifier
    (Cargo names cannot carry +). GitHub repo renamed
    SupportOs-Plus -> SupportOS-Plus (API PATCH; old URL redirects).
  - **Packaging:** `xtask deb` added (stage_deb + package_deb, unit-tested
    like stage_appdir): dpkg-deb --root-owner-group, policy-format control
    (Package/Version/Arch/Installed-Size/Depends/Homepage/Description), no
    maintainer scripts (system trigger providers own the caches; user data
    under XDG_DATA_HOME is never package-owned), standard modes (0755/0644)
    stamped independent of build umask. Depends declares the GUI toolkit's
    runtime dlopen set (libxkbcommon/x11/GL/EGL/wayland client libs) since
    ldd cannot see them — APT resolves everything on minimal systems; the
    GL driver stays host-provided. Bug found by local dry-run: Rust `\n\`
    string continuation eats the leading spaces Debian requires on
    Description continuation lines — dpkg-deb rejected the control;
    description is now built explicitly (deb_description()).
  - **CI:** AppImage path renamed SupportOS-Plus-x86_64.AppImage; the job now
    also builds the .deb (dpkg-deb -I + payload extraction + SPP_SMOKE) and
    uploads both as artifacts.
  - **Release workflow** (.github/workflows/release.yml, tag v* or manual):
    build+full tests on ubuntu:22.04 (glibc 2.35 baseline, broadest compat);
    SHA256SUMS + BUILD-INFO with the source commit; clean-container
    verification of the .deb on ubuntu:22.04/24.04 and debian:12 (APT
    dependency resolution, desktop-file-validate, boot smoke, GUI launch
    under Xvfb with only declared deps, reinstall, uninstall keeps user
    data, purge); AppImage smoke + GUI launch; release job refuses to
    publish unless BUILD-INFO matches the tag commit and version, then
    creates the versioned GitHub Release with both packages + checksums and
    re-downloads every asset to verify links and SHA-256.
  - **Verification protocol:** nothing claimed ready unless built and tested;
    first-release upgrade coverage is via reinstall (file-replacement
    mechanics) — a true prior-version upgrade activates from the next
    release onward.

### D6 — Reskin program: the Design System is the single source (2026-10-28)

Owner verdict: UX 0/10 (inbox worst) → Intercom-grade reskin while everything else
stays frozen ("everything is working fine, we need to reskin only according to the
new design system").

- **Scope contract: presentation only.** No feature, data, workflow, shortcut or
  page-set change. The v1.8.x program keeps working exactly as today; every visual
  element must trace to Design System v1.0 (`mockups/design-lab/ds.css|ds.js`,
  DS.TONE, DS-C-xx) or be a registered exception — nothing invented during
  implementation.
- **HTML/CSS/JS stays in `mockups/design-lab/` only** (spec/preview artifact, never
  shipped, no webview in the program). The reskin itself is pure Rust/egui —
  consistent with D1 (no UI DSL, one language).
- **Mapping is 1:1:** every `--ds-*` token → one token-factory constant in
  `crates/app-shell/src/design/` (the frozen-in `design.rs` became
  `design/mod.rs` + extracted components in RF-6 — same constants, one home);
  every DS-C-xx → one egui widget; DS.TONE → one match arm. Pages are
  recomposed from those widgets only.
- **Fidelity loop** (Task 20 plan, now binding): F0 Design Contract v1.0 freeze →
  R1 token port + style lint → R2 widget sets (kittest snapshots) → R3 page waves
  (Inbox pilot first, goldens at 1440/1024/800 vs lab goldens) → R4 traceability +
  blind test (5s AI identification) + SUS ≥ 68.
- **Gates:** G1 = owner approves the conformed design lab (blocks all Rust work);
  G2 = token sign-off; G3 = Inbox pilot acceptance; G4 = final acceptance. Gates
  align with rule.md batch STOPs (plan.md batches RS-1..RS-3).
- **Execution queue:** `ai/plan.md` RS-01..RS-21 under `rule.md` (ten at a time).
  Data-path defects (per-frame re-queries, async track) stay out of the reskin scope
  and remain separately tracked.

## Change log (appended)

- 2026-10-28 — D6 added: Design-System Reskin program approved; queue reopened in
  plan.md as RS-01..RS-21 (Stage 1 lab conformance → Gate G1 → Stage 2 egui reskin).
- 2026-10-29 — RS-21 executed (Batch RS-3): the R4 acceptance package
  (machine-gated traceability matrix, CI snapshot locks on lavapipe with the
  settings 32px registered budget, blind-test + SUS instruments, the
  acceptance report folded into the printable PDF series) and release v1.9.0
  tagged. GATE G4 presented with the STOP — the reskin program RS-01..RS-21
  is fully queued-through; only the owner's final acceptance remains.

## D7 — Refactor program recovery (2026-10-31)

The sandbox reset between sessions destroyed the refactor program's local
state: batches RF-1..RF-5 existed only as unpushed commits (`b7a5fcb`,
`4128f53`, `9179e60`) and their ai/* batch records. The remote (owner
re-confirmed via API with the re-supplied credentials) is exactly the
v1.9.1 release commit `acd557a`. Decision (owner-directed, "continue and
push the updates to git"): recover by re-cloning v1.9.1 and re-executing
the program under its original batch numbers from a reconstructed plan.md
section — same scope, order, gates and patterns, with every re-derived
item carrying fresh file:line evidence from the surviving tree. The
reconstruction is disclosed here rather than silently re-enacted. Pushes
to remote are authorized for this program (credentials stored at
`~/.git-credentials`, credential.helper=store, per the owner's explicit
instruction; the owner will revoke when required — no warnings to be
issued).

## D8 — Release v1.9.2 from the final source (2026-11-03)

- **Trigger (owner):** "is this repo production ready? … follow the next
  steps: Build and Release". Verdict delivered honestly: ready, with the two
  standing disclosed caveats (G4 blind test 5/10 owner-accepted; SUS n=1;
  no external third-party audit). Direction accepted as work order REL-1.
- **Version:** 1.9.2 (patch). Changes since v1.9.1: RF-1..11 (internal),
  KB-1 known-bug fixes (the user-facing deltas), A1-3b closeout kb2..7
  (internal). No features; no behavior change beyond the two fixes.
- **Verification split (disclosed):** the sandbox has no passwordless sudo,
  so the root-requiring .deb lanes (real APT install with dependency fetch,
  desktop-file-validate, upgrade/reinstall/uninstall/purge) run in
  release.yml's verify-deb matrix on clean ubuntu:22.04/24.04 + debian:12
  containers — exactly where they ran for v1.9.0/v1.9.1. Locally verified:
  the full gate set at the tagged content, both packages built from the
  locked tree, payload smoke + Xvfb/lavapipe GUI launch (deb-extracted tree
  and AppImage), SHA-256 checksums. The Release job additionally refuses to
  ship unless BUILD-INFO's embedded commit matches HEAD, and live-checks
  every published asset URL against SHA256SUMS.
- **Credential gate:** ~/.git-credentials was lost in the environment reset
  (see D7). No push (branch or tag) is possible until the owner re-supplies
  the token; all pre-push work completes first and the exact push commands
  are staged for immediate execution.
- **Artifacts match the source commit** by construction: BUILD-INFO.txt
  embeds `git rev-parse HEAD` at build time; versioned artifact names carry
  the workspace version; SHA256SUMS.txt covers every shipped file.
- **Closure (2026-11-03, same day):** the owner re-supplied the token;
  `main` (88830e4→a53273d) and the annotated tag `v1.9.2` (0b572c9) pushed;
  Release run 41731702400 success (32.1 min, 6/6 jobs); SupportOS+ 1.9.2
  published with both packages + SHA256SUMS.txt + BUILD-INFO.txt (commit
  a53273d); all assets downloaded live from the published URLs and
  `sha256sum -c` verified OK. The credential gate is closed.

## D9 — AU-1 audit evidence policy (owner-directed, 2026-11-03)

- **Program code is the sole source of truth.** Tests and all documentation (README,
  docs/, CHANGELOG, ai/*) are not behavioral evidence and are not consumers. Anything
  referenced only by test code is dead in the program. Census strips #[cfg(test)] mods
  and the ui-screenshot/kittest-ui tooling features before counting references.
- **Corpus:** 166 non-test files in crates/{core,app-shell,catalog}/src; xtask excluded
  (dev tooling). Known census blind spots (conservative bias): derive/serde-generated
  field consumption, macro-internal references, comment mentions counting as refs.
- **Adjudication protocol:** every deep-read agent finding was re-verified at file:line
  in the actual tree before acceptance; conflicting evidence wins over narrative —
  recorded example: the translation_cache "missing creation" reading was overruled by
  db_breadth.rs:327 (m040 boot creation), and two app-shell automation.rs INSERT sites
  were proven #[test]-gated, confirming the approval-loop severance instead.
- **Display artifact:** lines rendering as `#ust_use]` are `#[must_use]` (terminal ANSI
  `[m` swallow); bytes verified with od; not an anomaly.
- **Findings land in plan.md as Batch AU-2 items**; fixes are subsequent batches per
  rule.md (ten at a time, owner says continue).

## D10 — AU2-1 automation vocabulary + approval model (owner-decision gate taken in-code, 2026-11-04)

The AU2-1 item carries an owner decision (intended trigger vocabulary + approval
model). Per the owner's standing fix directive ("fix every broken feature") and the
batch opener, the decision is taken in-code this batch and recorded here.

- **Trigger vocabulary = the engine's fire vocabulary: `new_conversation` and
  `customer_reply`.** These are the only automation triggers the program actually
  emits (workers/execute.rs customer-thread detector → fire_trigger_for_conversation).
  The three legacy M007 triggers (`status_changed` / `tag_added` / `sla_risk`) have no
  emitter anywhere in program code, so rules with those triggers could never fire —
  that mismatch was break (a). They remain parseable (stored rules keep loading) but
  are removed from the creation form so no new never-firing rule can be created.
- **Action vocabulary = the engine's executable kinds.** UI creation keeps Assign
  (→ engine `assign`, `params.userId`), AddTag (→ `add_tag`, `params.tag`), Change
  Status (→ `set_status`, `params.status`), Send Note (→ new `send_note` engine
  handler posting an internal Help Scout note via the outbound `create_note` lane,
  tiered higher_risk). SetPriority is removed from the form: no engine handler exists
  (it silently recorded "completed" while doing nothing).
- **Action shapes: the engine now accepts both stored generations.** A
  canonicalization step at fire time maps the M007 single object (`create_rule`'s
  shape, top-level fields) and the engine-native array of `{kind, params}` into one
  canonical form — break (b) was that only the array shape executed.
- **Approval model = human-in-the-loop over the parked-job machinery.** An action
  that needs approval parks its job (`awaiting_approval`) AND inserts an
  `automation_approvals` row (new `job_id` column) — break (c) was that the approvals
  card read a table nothing inserted into. `approve()` flips the status and requeues
  the parked job with `approved:true` (the existing worker branch then executes the
  action and records the run); `reject()` flips the status and deletes the parked job.
- Evidence base: program code only (D9 policy), HEAD daff746.

## D11 — AU2-13 dead-surface dispositions + AU2-15 manual-only (owner-decision gates taken in-code, 2026-11-04)

Per the owner's standing fix directive and the wave-2 opener, the decision
gates inside AU2-13/AU2-15 were taken in-code this batch and recorded here.

- **Design-system dead widgets (message_card, quoted_history, file_chip,
  cop_similar): DELETED, not gated.** The census grouped them with
  `attribute_catalog`, which wave 1 deleted outright; both test-only callers
  of `message_card` were migrated to `message_card_with` (made pub) with a
  body closure byte-identical to the deleted wrapper's rendering, so the
  inbox/live-chats rendering path is untouched and exactly two identity
  goldens shift. Baselines regenerate CI-side via a one-shot
  UPDATE_SNAPSHOTS kittest run (this sandbox has no Vulkan ICD and no root —
  local regen is impossible), and the final HEAD's diff-lock job remains the
  proof gate.
- **Vector per-entity count (count_by_entity + cascade count_by_entity_type):
  BOTH deleted.** The program never counts vectors per entity; the only
  non-test caller was xtask's `vector-count` diagnostic, which now reads the
  store's plain `vec_payload` table directly (no sqlite-vec extension
  needed), and the store tests assert against the store file itself —
  file-state assertions, stronger than the API round-trip they replace.
- **kittest_seed_search: gated to `kittest-ui`**, exactly as the census
  note prescribed ("gate"): it is a test hook; release builds carry no test
  surface, and the kittest suites keep working unchanged.
- **The 16 created-never-used tables (+ the never-written side_threads
  family): DROPPED by boot step m063** (schema_version 54) rather than by
  editing historical migrations — upgraded databases and fresh installs
  converge on the same final shape, and the drop is idempotent
  (`DROP TABLE IF EXISTS`).
- **AU2-15 version_check: manual-only is the intended design.** The sole
  program caller is the Settings "check now" thread; scheduling a boot-time
  or periodic check would add unprompted GitHub traffic for every machine,
  which the current UX does not ask for. The intent now lives in code, and
  `ReleaseInfo` parses only the field the program consumes (`tag_name`).
- Evidence base: program code only (D9 policy), wave-2 tree.

- **Addendum (2026-11-04, same batch closure): the kittest baseline path went
  through three dispatches and deserves the full record.** (1) The identity
  regen succeeded but its baselines commit missed the PNGs (git add omitted —
  7285e8e carried only the workflow deletion). (2) The CI kittest job then
  revealed inherited drift: it had been red since wave 1 at the automation
  suite (AU2-1's form changes were never mirrored into the goldens; the
  failure went unnoticed when that session ended) and cargo's fail-fast hid
  every suite behind it. (3) A full-suite regen re-rendered everything: 22
  goldens changed (identity x2, automation x7, outreach x6, search x7 — each
  group the exact pixel-print of a recorded wave-1/wave-2 fix, diff images
  audited individually), ~149 came back byte-identical. The wholesale-regen
  risk (blessing unintended drift) was mitigated by auditing every changed
  golden's diff image before committing; nothing unexplained surfaced.

## D12 — Release v1.9.3 from the final source (owner-directed Build & Release, 2026-11-04)

- **Trigger:** owner work order — build, test, and publish install-ready Linux
  `.deb` + `.AppImage` packages **from the final repository source**, automated
  through GitHub Actions, with clean-environment verification and release
  integrity (checksums, matching commit).
- **Version:** 1.9.3 (patch). Changes since v1.9.2 are Batch AU-1 (audit) +
  Batch AU-2 (fixes): automation end-to-end, digest tags, search button,
  mention caret, knowledge write-lane/latch, inbox selection/assignee, deep
  links, ai_center role routing, sync notes, outreach report totals, settings
  feedback lifecycle + provider disconnect + reset pause, palette/confirm
  arbitration, verified hygiene hardening (SQL time formats, jobs claim guard,
  webhook redelivery requeue, backups-first maintenance); dead-surface removal
  (21 dead items, producer-less job arms, encrypted_sync stub, queue stats,
  16 orphan tables dropped by m063); version check documented manual-only
  (D11). No API/DB-consumer behavior change beyond the recorded fixes.
- **Pipeline:** the unmodified `release.yml` (proven v1.9.0–v1.9.2) — ubuntu:22.04
  container build (glibc 2.35 baseline), full gate set on the tagged content,
  `xtask package`/`xtask deb`, verify-deb matrix (ubuntu:22.04/24.04,
  debian:12), verify-appimage, publish-after-green, live link + checksum
  re-verification in-workflow.
- **Execution:** single prep commit (version + CHANGELOG + ledger) on `main`,
  annotated tag `v1.9.3` on it; the release is scrapes-verified post-publish
  (assets, download URLs, sha256 from downloaded bytes, BUILD-INFO commit).

## D13 — Ledger structure: the three-file discipline restored (owner-directed, 2026-11-04)

- **Trigger:** Batch REC-1 (facts-vs-fictions reconciliation) found the ledger
  drifted from rule.md's file contract: progress.md held 23 completed-batch
  transcripts (~1,930 lines) under an "active items" title with a false
  "Batch RS-4 active" declaration, and plan.md held 611 lines of historical
  narrative under a "TODO" title with zero open items — the source of the
  owner's "too many pending items" impression. Owner direction on the REC-1
  report: "do what is best to keep things in order."
- **Decision:** restore rule.md Rule 7 literally — `plan.md` = remaining TODO
  items only; `progress.md` = active items only; `completed.md` = completion
  records (append-only, unchanged discipline). History is preserved, not
  deleted: the plan.md narrative moves to `ai/archive-plan.md` and the
  progress.md transcripts to `ai/archive-progress.md`, both byte-preserving
  moves with provenance banners and known-stale-spot notes (Rule 6: nothing
  discarded — every moved section has its completion record in completed.md,
  verified section-by-section at HYG-1).
- **Standing rule going forward:** completed batches leave progress.md at the
  Rule 4 sync (no in-place completion annotations); new programs open in
  plan.md as numbered items; the archive files receive nothing new — they
  are frozen snapshots of the 2026-11-04 restructuring.
