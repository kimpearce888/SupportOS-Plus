# Complete Repository Audit — SupportOS-Plus

**Status: FINAL — reconciled 2026-10-25 (see §17 coverage accounting and the completion-gate record in HANDOVER.md)**
Repository: `https://github.com/kimpearce888/SupportOS-Plus`
Application revision audited: `main @ 3affe7e` (audited revision; full sha in HANDOVER.md)
Audit branch: `repo-audit/2026-10-24` — 16 incremental audit checkpoints `e15ffa8`…`5a19003`, push verified after every checkpoint (GIT-003)
Audit date/timezone: 2026-10-24 → 2026-10-25 (UTC)
Audit folder: `repo-audit-2026-10-24/` (the ONLY content published by this audit — no app code was modified, no tests weakened, no force-push)

## 1. Scope and methodology

Full instruction-driven audit per `repo-audit-2026-10-24/instructions/` (15 files; loading gate SATISFIED — 15/15 completely read, 14/14 SHA-256 match against PACKAGE-MANIFEST.md; two instruction-package inconsistencies resolved and recorded in INSTRUCTION-LOADING-RECORD.md).

**Source of truth = the actual build/runtime-relevant implementation + observed runtime behavior.** Method:

1. Instruction gate → build → launch → automated test run (TEST-001) before any UI claim.
2. Architecture discovery (source-first, then runtime cross-checked — ARCHITECTURE.md §1–§11).
3. **Pass 1 — feature walk:** every sidebar page opened in the live GUI by XTEST-injected, pill-verified mouse clicks with per-page screenshots (23/23 pages + 3 shell-level features).
4. **Pass 2 — control/option ledger:** 535 source-first `C-<PAGE>-<nn>` rows (identity, expected effect, conditions, validation, dependencies, line refs) reconciled by 46 real-input runtime control exercises across 21 pages.
5. **Workflow execution:** the 10 identified user journeys, each at a real entry point with per-step screenshots, branch coverage, DB-level assertions, and honest blocker registration.
6. **Edge/failure/recovery matrix (QA-004/005)** and cross-feature persistence tests.
7. Continuous evidence manifesting, exception registration, incremental checkpoint commits, and coverage reconciliation (COV-001/002).

Every UI interaction used the REAL interface (XTEST mouse on Xvfb :99); no screenshot was ever constructed, altered, or substituted (EVID-004). Tooling defects and corrections are retained in the events JSONLs and SESSION-LOG (e.g. `w005-correction`, `w002-reconciliation`, the EVID-003 video-claim correction) — corrections supersede, never rewrite.

## 2. Environment and reproducibility

Details in BUILD-AND-INSTALL.md. Summary: Debian sandbox (2 vCPU / 4 GB), Rust 1.99.1 (matches rust-toolchain.toml pin), SQLite via rusqlite (bundled), Xvfb `:99` (1600×1000×24) + ffmpeg x11grab capture + python-xlib XTEST injection (mouse). Environment remediation (recorded, environment-only): `libxkbcommon-x11`/`libxcb-xkb` installed under `/home/z/.local/audit-libs` and wired through `run-app.sh` LD_LIBRARY_PATH (the app panics without them on this sandbox — see fatal.log bring-up rows). Build: `cargo build` (debug) — **0 warnings**; launch via `./run-app.sh`; app data dir `~/.local/share/supportos-plus/` (QA-owned).

## 3. Key findings overview

Full register with repro/evidence/root-cause in FINDINGS.md; summary:

- **FIND-001 (Medium, CONFIRMED at runtime + by implementation analysis):** the webhook/OAuth loopback listener binds the port baked into the Help Scout redirect URI — **:3000 by default** (config.rs:209) — and `SPP_LOOPBACK_PORT` is only consulted when the redirect URI has no port (main.rs:103–113), so the override is ineffective in the default config. With :3000 occupied (Next.js/React/Angular dev servers), the app silently degrades to no-inbound-HTTP (webhooks + OAuth callback dead) with a WARN log only; no UI signal. Positive control: override URI port 3999 → listener ready (F-CAND-001 logs). Credential-gated remediation surfaces (listener-health message, OAuth round-trip, webhook drain) are exception-registered (EXC-001..004) — the finding stands on the runtime-proven root cause.
- **FIND-002 (Low → CLOSED by W-009):** second-instance flock guard — exit 1 + stderr + fatal.log + (on notifier-equipped desktops) a zenity modal; no focus-existing-window IPC by design; instance-1 UI offers no affordance (pixel-identical frames). Stays observation/informational.
- **FIND-003 (Low, pre-existing, observation):** the PRIOR audit (`qa-audit/20261019T050847Z/`) committed app-local secret-envelope key material as repo artifacts — maintainer decision required; untouched by this audit.
- **FIND-004 (Low):** pinned theme row overlaps the bottom sidebar rows; Shell row clickable only through a ~10 px sliver at the default window size.
- **FIND-005 (Low):** destructive actions without confirmation on several pages while equivalent deletes elsewhere are shell-confirm-gated (inconsistent destructive UX).
- **FIND-006 (Low):** Sync Health "Cancel running sync" on an idle engine toasts "(started in background)" — misleading feedback; DB-verified no state change.
- **FIND-007 (Low):** zero-hit help-center searches silently persist knowledge-gap records (hidden write from a read-looking control).
- **FIND-008 (Low, runtime-confirmed in W-005):** Issue Radar "Detect gaps now"/"Cluster now" fire-and-forget enqueues — no completion signal; manual reload required.
- **FIND-009 (Low):** silent input coercion (Connectors refresh-seconds → default 3600; Operations capacity clamp 1..=500; Builder dates length-10 only).
- **FIND-010 (Low):** Customers "← back to search" clears results without re-running the applied query.
- **FIND-011 (Low, runtime-confirmed + VIDEO):** command-palette click-outside dismissal unreliable and position-dependent (W-008 matrix: 5/6 outside clicks failed; two fell through to the UI beneath — one spawned a REAL sync action, attributed consequence); supplemental video E-158 (11.7 s, 6/6 timestamped actions) shows the overlay staying open under outside clicks and closing only in the working region.

## 4. Program purpose and problem coverage

SupportOS+ is a local-first native desktop support-desk application (v1.7.0): a Rust workspace with an egui/eframe GUI (`app-shell`, 23 pages + shell), a core domain library (`core`, SQLite, 52 migrations), a declarative domain catalog (`catalog`), and `xtask` automation. It syncs with Help Scout (mailboxes/conversations/webhooks/OAuth), runs AI pipelines against local LM Studio (OpenAI-compatible) with an `ai_runs` response cache, maintains FTS + sqlite-vec vector indexes, and provides automation rules, Issue Radar, reporting, backups/export/restore, and a command palette.

Distinct problems addressed/attempted by the audit (all with IDs): build/launch/lifecycle verification (BUILD-001/002, TEST-001), full page/control/workflow discovery and testing (FEAT-001..004, QA-001..007, FLOW-001), sync lifecycle without credentials (W-002 + EXC-005), second-instance UX (W-009, FIND-002 closed), theme persistence round-trip (W-007), backups/export/restore incl. the two-step destructive gate and its confirm leg (W-010 + QA-45 S005), knowledge ingest gates (W-005), automation form conditional logic (W-006), palette dismissal (W-008 + FIND-011 + video), edge/recovery paths (QA-004/005: repetition, re-arm, clear-log round trip, SIGKILL recovery, restore-replace swap, cross-restart persistence), loopback-port collision degradation (FIND-001 + exceptions), and repository-hygiene observation (FIND-003).

## 5. Architecture and construction

See ARCHITECTURE.md (§1–§8 map + §9–§11 deep-dive). Highlights, each cross-checked against runtime:

- **Lifecycle (main.rs):** logging → single-instance flock (`instance.lock`; exit 1 + fatal.log + best-effort desktop alert on refusal — W-009 verified held/released via independent flock probes) → DB with all boot migrations → core event bus → boot services (sync engine + provider, real or empty, hot-swappable) → background worker host (dedicated 2-thread runtime) → minimal loopback listener (webhook ingress + OAuth callback) → eframe window.
- **Worker host (workers.rs §9):** 2 s job drain (outbound dispatch first), adaptive auto-sync (30 s busy / doubling quiet backoff to a settings ceiling / woken by `nudge_sync()` after every send), 6 h maintenance, 10 min stale-job requeue (running > 30 min), notification/customer-events/intelligence/connectors sweeps; RAII TickGuards + contained panics; boot recovery requeues crash-stranded sends and recovers sync_state stranded in INITIALIZING/BACKFILLING; `webhook_handler::drain_pending` at worker start.
- **AI subsystem (§10):** `LocalAiProvider` trait; production impl `LmStudioProvider` (OpenAI-compatible, 127.0.0.1:1234); versioned prompts + structured serde outputs + evidence assembly; `ai_runs` cache keyed (input_hash, prompt_version, model); each pipeline call drives its future on a PRIVATE current-thread runtime (documented write-guard deadlock avoidance); memory guard + golden-eval jobs.
- **Catalog + eventing (§11):** declarative enums/entries drive the UI surfaces (metrics/dimensions for the report builder, tiles, graph kinds); in-process tokio broadcast `EventBus` → native-shell repaint bridge (why background completions surface without polling).
- **Data:** single SQLite file, 52 migrations observed at runtime; secret envelope (m049); backups dir with verify-first restore swap (`restore-tmp`/`restore-prev`, runtime-proven in QA45 S005).

## 6. Complete feature/control inventory

Detailed sources: FEATURE-INVENTORY.md (authoritative) + REQUIREMENT-TRACEABILITY.csv.

- **Features:** 26 — 23 sidebar pages (F-001..F-023, every one opened in the live app with screenshots) + 3 shell-level capabilities (F-024..F-026: single-instance/lock, theme persistence, command palette).
- **Controls/options:** 535 ledger rows (`C-<PAGE>-<nn>`), each with identity, expected effect, conditions, validation/constraints, effect paths, dependencies, and source line refs. Runtime-exercised: 46 pass-2 exercises + every workflow/QA step (chips, tabs, toggles, dialogs, two-step gates, destructive confirms, disabled states, theme flips, palette). Not runtime-exercised WITH reasons: credential-gated surfaces (EXC-001..004), text-entry paths (XTEST keyboard limitation — mouse-reachable empty-form gates verified instead: C-KNOW-08, C-AUTO-12), rule-row controls (no rule can exist — tooling-limited, W-006), destructive deletes beyond verified gates (deliberate).

## 7. Distinctive implementation behavior and trade-offs

- The native shell deliberately replaces Tauri's single-instance plugin with flock + "clear message and non-zero exit" — no IPC channel, so no focus-existing-window (documented in single_instance.rs; W-009 verified the full feedback inventory incl. the environment-dependent desktop-alert leg).
- `fatal()` writes `logs/fatal.log` AND attempts a zenity/notify-send dialog because "the desktop entry discards stderr" — window-less failures are findable by design (fatal.rs:52–60); the dialog leg drops silently on notifier-less systems (sandbox: all notifier binaries absent — recorded environment-dependence, not absence).
- Sync refuses BEFORE creating any run row when the provider is not connected (guard_provider_connected, sync_engine.rs:1012-1022) — the never-synced DB state survived every refusal attempt (W-002, QA45).
- The restore flow swaps the file on disk while running and tells the user to restart ("restart the app to reload it", settings.rs:1165) with verify-first refusal of junk files — QA45 proved the swap end-to-end (refusal row reverted 6→5 on disk; post-restart the restored content was live).
- Adaptive auto-sync trades poll cost for freshness (30 s busy / doubling quiet backoff / immediate post-send nudge).
- The app restores last page + sidebar scroll across relaunches (discovered by the feature walk; encoded into all tooling as `normalize_to_dashboard`).

## 8. Partial/unreachable/disconnected implementations

No no-op buttons found in the main flows — every enumerated control has a wired handler. Behavioral gaps are recorded as findings (FIND-004..011). Unreachable AT RUNTIME in this sandbox (with registered reasons, never silently skipped):

- Help Scout OAuth round-trip + configured-state listener-health message + webhook drain (EXC-001..003 — no external credentials/endpoint; signed webhook events cannot be forged: HMAC secret app-internal).
- Sync happy path (EXC-005 — the executed branch is the DESIGNED no-credentials refusal).
- W-003 conversation lifecycle deep steps and W-004 report-builder runs (need synced/seeded data; zero-data states were captured in pass 1 and exports were exercised header-only in W-010).
- Text-input form paths (keyboard limitation; mouse-reachable gates verified).

## 9. Workflow map and branch coverage

Authoritative: WORKFLOW-COVERAGE.md (W-001..W-010 with entry points, steps, branches, outcomes, evidence). Status summary:

| Workflow | Status | Evidence |
|---|---|---|
| W-001 Help Scout OAuth connect | BLOCKED (external) — steps S001..S005 enumerated; S001 executed within W-002-S008 | E-135; EXC-001..004 |
| W-002 Sync lifecycle | DONE-PARTIAL (no-credentials leg: both entry points refused by the DESIGNED guard; freshness chip unchanged; never-synced state survives) | E-131..E-137; EXC-005 |
| W-003 Conversation lifecycle | BLOCKED (needs synced data) | pass-1 zero-data states |
| W-004 Report builder | BLOCKED (needs data; builder controls inventoried) | pass-2 ledger |
| W-005 Knowledge ingest + gaps | DONE-PARTIAL (disabled-import gate runtime-verified; visibility chips; radar enqueues 0→2 ai/completed; inline queued-line; NO completion signal → FIND-008; text path tooling-blocked) | E-119..E-130 |
| W-006 Automation rule | DONE-PARTIAL (form toggle, trigger chips 3/3 with conditional field swaps, action chips 5/5, empty-create validation 'rule name required' with zero records; creation tooling-blocked) | E-105..E-118 |
| W-007 Theme switch round-trip | DONE (both directions, synchronous settings write, persistence across two restarts, frames diff 0.0) | E-087..E-095 |
| W-008 Command palette | PARTIAL (opens via header button, correct empty state, repeatable; Ctrl+K no-op — keyboard; backdrop unreliable → FIND-011 + supplemental video) | E-081..E-086; E-158..E-160 |
| W-009 Second-instance UX | DONE (flock probes both directions; exit 1 + stderr + fatal.log; no instance-1 affordance — pixel-identical; DB unchanged) | E-138..E-142 |
| W-010 Backups/export/restore | DONE-PARTIAL (backup/export JSON/CSV; two-step restore gate; "really restore" C-SET-66 later EXECUTED in QA45 — deferral closed) | E-096..E-104; E-152 |

Branches exercised: designed refusals, empty-form validations, arm/cancel/re-arm ×2, confirm-leg destructive restore, clear-log confirm dialog, palette fall-through matrix (stills + video), second-attempt refusals (repetition). Return-to-valid-state verified after each destructive-path test.

## 10. Manual test results (controls, features, journeys, edge cases)

- **Executed UI steps:** 143 — every one with a validated step-specific screenshot (100 % coverage; PROGRESS.md §Evidence accounting). Plus 2 process-level steps (W-009 flock probes) and a 6-action supplemental video run (W-011).
- **Pass 2 controls:** 46 exercises — 42 PASS, 2 PARTIAL (recorded environment limitation), 2 documented mis-target/no-ops superseded by better evidence (tooling history retained).
- **QA-004 (inputs/states):** repetition/duplicate (backup ×2 → two DISTINCT pairs, newest-first, no dedupe crash); gate re-arm ×2 (arm→cancel→arm→cancel clean, DB stable, zero swap files); clear-log round trip WITH events (button exists at 5 rows → confirm dialog → DB 5→0 → button disappears — round-trips the pass-2 empty-case observation); empty-form gates (C-KNOW-08, C-AUTO-12). NOT EXECUTABLE: text-input boundary matrix (keyboard), malformed credentials (external) — recorded, never claimed.
- **QA-005 (integration/recovery):** designed-refusal failure path (user communication verified); C-SET-66 "really restore" (verify-first swap runtime-proven; refusal row reverted 6→5 on disk; restart-to-reload semantics verified end-to-end after relaunch); SIGKILL (kill -9 mid-session → clean relaunch, integrity ok); cross-feature persistence (backups created in Settings survive kill + restart and render; theme persists across restarts — W-007).
- **Formal ledger statuses:** QA-001/002/003/007 SATISFIED; QA-004 EXECUTED (mouse-reachable subset); QA-005 EXECUTED (feasible subset); QA-006 PARTIAL (keyboard focus-order untestable in this sandbox — tooling limitation, recorded with the mouse-reachable observations: disabled gates, dialogs, validation feedback, scroll).

## 11. Defect register

FINDINGS.md (authoritative): FIND-001 Medium; FIND-003..FIND-011 Low (FIND-008 and FIND-011 runtime-confirmed; FIND-011 carries the attributed real-action consequence — a fall-through click spawned a sync action); FIND-002 CLOSED (observation/informational — W-009 decision). Each entry has classification, severity, confidence, repro, expected-vs-actual, evidence links, root cause where determined, fix direction, and regression risks. FIND-003 documents pre-existing repo content (prior audit's key material) — no app defect claimed.

## 12. Fix recommendations (with acceptance criteria)

Highlights (full rationale in FINDINGS.md; none weaken validation or hide errors):

- **FIND-001:** prefer `SPP_LOOPBACK_PORT` over the redirect-URI port (or make the chain explicit/fall back automatically) and surface listener state in the UI. Acceptance: with :3000 occupied and the override set, the listener binds the override; UI shows listener state; OAuth + webhook POST succeed on the bound port. Regression check: exact redirect-URI matching must keep working (both port sources tested).
- **FIND-011:** make the palette catcher a true full-screen top-order area (or close on click-outside-window-rect) and/or make the triggers toggle. Regression test: open palette, click each screen quadrant, assert closed.
- **FIND-004:** move/unpin the theme row; **FIND-005:** route all destructive actions through the shell confirm; **FIND-006:** idle-cancel toast should state the engine is idle; **FIND-007:** surface the knowledge-gap write; **FIND-008:** completion signal (or optimistic UI with job status); **FIND-009:** explicit validation instead of silent coercion; **FIND-010:** preserve/restore the applied query on back.

## 13. Build/install/runtime failures

None. Build 0 warnings; launch verified (E-001); shutdown incl. automatic backup observed; the sandbox's :3000 collision WARN is FIND-001 (product robustness, not an environment failure). Environment remediation (libxkbcommon-x11) was environment-only and is documented.

## 14. Existing test-suite results (separate from manual QA)

`cargo test --workspace` @ 3affe7e (TEST-001, 2026-10-24): spp_shell 228 passed / 0 failed; spp_core 466/0 (4 ignored); spp_xtask 9/0; spp_catalog 0 tests — **703 passed / 0 failed / 4 ignored**. Supplementary only — the audit does not treat them as proof of product correctness (per instruction 02).

## 15. Evidence index

`evidence/manifest.csv` — **160 rows (E-001..E-160), zero missing paths, every row validated** (PIL open + visual read for screenshots; ffprobe for the video; full reads for logs; machine events cross-checked). On disk: 156 step/evidence PNGs (BUILD 1 · CONTROL 72 — 66 step frames + 6 retained OCR-miss tooling frames · FEATURE 24 · QA45 14 · WORKFLOW 45) + 2 video-extracted frames + 1 ffprobe-validated mp4 (117 frames) + 14 logs + 11 tooling scripts/events JSONLs. Every row links Feature/Workflow/Step IDs, revision `3affe7e`, environment, and related findings/exceptions.

## 16. Evidence exceptions and unresolved gaps

- **exceptions.csv (EXC-001..EXC-005):** five E2 test-blocked exceptions, one per affected step (W-001-S002..S005, W-002-S004 happy-path branch) — all rooted in ONE external blocker (no live Help Scout credentials/endpoint; signed webhook events cannot be forged). Every blocker established by an executed attempt (E-135) or impossibility inspection — never speculation. All accepted-as-justified with next actions.
- **Accepted screenshot exceptions: 0.** Unresolved evidence gaps: 0.
- **Recorded limitations (not exceptions):** XTEST keyboard injection does not reach egui TextEdits/chords (verified twice) → text-input boundary matrix, palette typing, and rule-creation steps not executable; W-006 rule-row controls unreachable (no rule can exist); W-003/W-004 data-blocked. Each is visible in PROGRESS/HANDOVER/traceability as blocked/unattempted — never as covered.

## 17. Coverage accounting (definitions per instruction 03 §9)

- Executed manual UI steps requiring screenshots: **143** — with valid step-specific screenshots: **143 (100 %)**. A frame is shared only where it genuinely proves every referenced claim (manifest step mapping).
- Accepted screenshot exceptions: **0** (E1–E6 all zero). Non-visual/process steps: 2 (W-009 probes; DB reads are assertions). Unresolved evidence gaps: **0**.
- Blocked/not-attempted (explicitly NOT passes): W-001-S002..S005 (4 steps, exceptions), W-003/W-004 deep steps (data), QA-004 text-input subset (keyboard), mid-sync cancel (EXC-005).
- Workflows: 10 identified — 2 DONE, 4 DONE-PARTIAL, 1 PARTIAL, 3 blocked with registered reasons. Every executed workflow journey has entry, steps, branches, outcome, and evidence.
- Findings: 11 total — 1 Medium, 9 Low, 1 closed observation. Fix recommendations: 11 (directional; FIND-003 is a maintainer-decision item, not an app change).
- Reconciliation: features 26/26 · controls 535/535 statused · workflows 10/10 executed-or-blocked · manifest 160/160 · exceptions 5/5 reviewed · traceability 50/50 rows statused (REQUIREMENT-TRACEABILITY.csv).
- **No claim is made that all possible bugs were found or that every input combination was tested** (COV-002).

## 18. Continuity and publication state

- Audited revision: `main @ 3affe7e`. Audit branch: `repo-audit/2026-10-24`, head `5a19003` — 16 checkpoint commits `e15ffa8…5a19003`, each pushed and push-verified (remote: kimpearce888 credential store). Only `repo-audit-2026-10-24/` was ever committed; the working tree of the app is untouched; no force-push; no secrets committed (FIND-003's pre-existing keys were left in place untouched).
- Continuity records: HANDOVER.md (authoritative breakpoint/final state), PROGRESS.md (phase table + §9 accounting), SESSION-LOG.md (per-run records incl. all tooling corrections), NEXT-ACTIONS.md (remaining optional work: none blocking; future retest paths listed in exceptions.csv Next-Action column).
- Remaining work for future sessions: none required by the instructions; optional retests when external prerequisites become available (live Help Scout credentials/endpoint → EXC-001..005; a text-capable run → the deferred tooling-limited steps; W-003/W-004 after seeded data).
