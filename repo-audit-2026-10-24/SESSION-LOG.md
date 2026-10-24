# Session Log

Use chronological, factual entries. Do not put secrets in this file. Each entry should identify the session/agent if known, timestamp/timezone, revision, action, actual result, evidence paths, files updated, and next action.

## Entries

### `2026-10-24 14:50–15:05 UTC` — Z.ai Code (session web-7a117f68-2ddf-4d17-85d9-048c274a5b02)
- Revision/branch: main @ 3affe7e (fresh clone; working tree clean)
- Action performed: Located and downloaded all 15 files of the audit instruction package from Google Drive folder 16b9H9cR3iYdnS47ngnrlFgpik0X6hf5b; verified all 14 manifest SHA-256 hashes (all match); read README.md, 00-BOOTSTRAP.md, files 01–05, traceability CSV (50 rows), PACKAGE-MANIFEST.md, and all 6 templates completely; reconciled 50/50 requirements; cloned repository; checked environment; installed Rust 1.99.1 (matches rust-toolchain.toml pin); created audit folder with instruction copies, provenance record, loading record, reconciled checklist, and live records
- Actual result: Instruction-loading gate SATISFIED; repo cloned clean; toolchain ready; workspace build started in background (outcome pending)
- Evidence/output paths: repo-audit-2026-10-24/INSTRUCTION-LOADING-RECORD.md, repo-audit-2026-10-24/instructions/FILE-PROVENANCE.md, repo-audit-2026-10-24/REQUIREMENT-TRACEABILITY.csv
- Records/files updated: PROGRESS.md, NEXT-ACTIONS.md, HANDOVER.md, BUILD-AND-INSTALL.md, SESSION-LOG.md (this file)
- Status changes or decisions: (1) Previous qa-audit/20261019T050847Z treated as completed prior art under a different spec — this is a NEW audit, not a resume. (2) ENV-001 environment content recorded in BUILD-AND-INSTALL.md (documented CSV/source filename discrepancy). (3) Drive file named 00-BOOTSTRAP.md (ID 16wp…) is actually SESSION-LOG.template.md per manifest hash — stored under manifest name.
- Next action: Await/verify `cargo build` result; then architecture discovery (crates/core, crates/app-shell, crates/catalog, crates/xtask) into ARCHITECTURE.md; then feature inventory.

### `2026-10-24 15:40–15:50 UTC` — Z.ai Code (same session)
- Revision/branch: main @ 3affe7e
- Action performed: Verified background jobs killed by sandbox between calls (empty logs); installed python-xlib (import verified); ran `cargo build` foreground — SUCCESS 3m57s, 0 warnings, binary target/debug/supportos-plus; launched app on Xvfb :99
- Actual result: Two launch crashes from missing sandbox libs (libxkbcommon-x11.so.0, libxcb-xkb.so.1) — environment root cause; remediated via apt-get download + dpkg -x to /home/z/.local/audit-libs + LD_LIBRARY_PATH (no app/code changes); relaunch SUCCESS: 52 DB migrations applied, workers started, window SupportOS+ 1180x760 mapped; foreground run alive full 20s (timeout exit 124); dashboard UI rendered and screenshot validated
- Evidence/output paths: repo-audit-2026-10-24/evidence/screenshots/BUILD/BUILD-002-launch-initial.png (validated 1600x1000 PNG); /tmp/app-run*.log
- Records/files updated: BUILD-AND-INSTALL.md (build + launch sections), PROGRESS.md, run-app.sh + start-xvfb.sh helpers added
- Status changes or decisions: BUILD-001/002 → PASS with evidence; sandbox background-process reaping documented (self-contained QA batches required); candidate finding F-CAND-001 opened (loopback listener "Address already in use" WARN at main.rs:129)
- Next action: Architecture discovery into ARCHITECTURE.md (app-shell pages/, core modules, catalog, xtask, bridge, workers); investigate F-CAND-001 port/impact; then feature inventory

### `2026-10-24 15:50–16:05 UTC` — Z.ai Code (same session)
- Revision/branch: main @ 3affe7e
- Action performed: Architecture source survey (pages/mod.rs 23 PageKeys; bridge.rs EventBus→repaint; config.rs loopback ports; workers.rs drain_pending; single_instance.rs flock). Runtime batches: loopback port matrix (default :3000 fail / HELPSCOUT_REDIRECT_URI=3999 ready / SPP_LOOPBACK_PORT ineffective in default config), second-instance flock test (exit 1 + ERROR log), `cargo test --workspace` completed per-package
- Actual result: TEST-001 = 703 passed / 0 failed / 4 ignored (shell 228, core 466+4 ign, xtask 9, catalog 0); F-CAND-001 promoted to FIND-001 with runtime evidence + positive control; FIND-002 recorded (single-instance OK, console-only message — UI check queued); ARCHITECTURE.md v1 written; FINDINGS.md opened; evidence manifest created (E-001..E-005, all validated)
- Evidence/output paths: evidence/logs/F-CAND-001-*.log, evidence/logs/single-instance-second-run.log, evidence/manifest.csv, ARCHITECTURE.md, FINDINGS.md, BUILD-AND-INSTALL.md (test section)
- Records/files updated: PROGRESS.md, SESSION-LOG.md, NEXT-ACTIONS.md, HANDOVER.md, REPORT.md §14
- Status changes or decisions: SPP_LOOPBACK_PORT found ineffective when redirect URI has a port (implementation-confirmed root cause, main.rs:103–113); loopback.rs settings health check (1.1.4) identified as existing mitigation — UI verification queued
- Next action: Feature inventory pass 1 (pages/ sources + runtime UI walk with XTEST injection), then workflow ledger

### `2026-10-24 16:10 UTC` — Z.ai Code (same session)
- Revision/branch: repo-audit/2026-10-24 @ e15ffa8 (checkpoint 1)
- Action performed: Created audit branch from main @ 3affe7e; staged ONLY repo-audit-2026-10-24/ (32 files); secret scan of staged set clean; commit + push + remote verification via ls-remote; recorded FIND-003 (pre-existing qa-audit secrets.key artifacts — observation only, no user content modified)
- Actual result: publication VERIFIED — origin/repo-audit/2026-10-24 @ e15ffa8
- Evidence/output paths: git ls-remote output recorded in HANDOVER.md §Git state
- Records/files updated: HANDOVER.md, FINDINGS.md (FIND-003), SESSION-LOG.md
- Status changes or decisions: findings renumbered: F-001/F-002 renamed FIND-001/FIND-002 (retired as finding IDs with SESSION-LOG references), new pre-existing-content observation = FIND-003; feature IDs will use F-<nnn> namespace (collision-free)
- Next action: feature-inventory pass 1 (page walk with XTEST + per-page screenshots)

---
## 2026-10-24 ~19:30–20:10 UTC — Feature-inventory pass 1: full 23-page sidebar walk (Task: FEAT-001 pass 1)

- Method: default binary (3affe7e) on Xvfb :99; python-xlib XTEST real input;
  tool `evidence/artifacts/feature-walk.py`; per-page screenshots to
  `evidence/screenshots/FEATURE/`; machine event log
  `evidence/artifacts/feature-walk-events.jsonl`.
- Tool history kept for honesty:
  - v1: drifted off-by-one on scrolled rows — root causes: (a) all PIL frames
    aliased one reused /tmp PNG (lazy loading read the newest file), (b) hover
    contamination of analyzed frames, (c) ±8 px verify tolerance masked the
    drift. DISCOVERED by visual validation: 17-support-graph.png showed the
    Search page. v1 PNGs discarded.
  - v2 crash (PIL read of overwritten frame) left an ORPHANED app process
    holding the instance lock.
  - v3/v4 then drove that orphan (provenance defect): their "launch-state"
    observations (Onboarding + scrolled sidebar, initially hypothesized as
    session/page restore) were the orphan's live state, NOT app behavior.
    Their outputs discarded; finding corrected below.
  - v5 (published): pre-launch pkill + lock verification, independent
    in-memory frame snapshots, pointer parked outside the sidebar for every
    analyzed frame, scroll-settle (2 identical frames), ±4 px pill
    verification, candidate-fallback clicks, theme-row rect measured by
    hover diff [714,743].
- Result (v5, fresh process pid 18958, lock acquired): 23/23 pages opened by
  sidebar clicks and pill-verified (observed drift ±1 px); 4 settled wheel
  scrolls; Shell (last row) opened via measured top-sliver click y=703
  (verified by selection pill at 718 AND content diff 4.18) → FIND-004
  records the underlying overlap defect. All 23 page PNGs + end state
  visually validated (page identity + selected-pill state checked per file).
- Launch-state correction: fresh processes ALWAYS open Dashboard with nav
  scroll 0 — the app does NOT restore last page / sidebar scroll. Theme
  (light) persists via the settings table.
- Artifacts: E-006..E-031 appended to evidence/manifest.csv (all validated).
- Status changes: FEAT-001 pass 1 COMPLETE (23 pages + 3 shell-level
  features F-001..F-026); FIND-004 opened (Low, confirmed); finding IDs
  F-001/F-002 renamed FIND-001/FIND-002 across docs+manifest (collision-free
  scheme: features F-xxx, findings FIND-xxx).
- Decisions: the `ui-screenshot` cargo feature (window-size env override +
  harness) was NOT used — evidence comes from the default build; feature
  existence noted for pass 2 as a discovered QA affordance.


## 2026-10-24 ~21:35 UTC — pass 2: control/option ledger (FEAT-002/003) COMPLETE

- Source-first enumeration by 8 read-only agents (worklog Task IDs 3-a..3-h):
  535 control/option rows across 23 pages + shell/platform (C-<PAGE>-<nn>),
  each with label, source lines, conditions, validation, effect path,
  dependencies and non-obvious notes; per-page UI-state lists and
  FEAT-003 background/integration/persistence lines included.
- Runtime verification via evidence/artifacts/control-verify.py (v1->v3):
  46 real-input steps over 21 pages + restore. Machine-verified by content
  diff with control-calibrated thresholds (chip 0.06-0.1, no-op 0.0-0.01,
  checkmark 0.007, view switch 0.6-5.5) + OCR label targeting (4-pass) +
  dark-rect/bbox detectors for +icon buttons + pill-verified navigation.
- Tool iteration retained for honesty: v1's page-switch threshold (1.0)
  false-negatived control-level changes (33 rows); calibration evidence
  (pixel checks, strip reads, TSV dumps) drove the v2/v3 fixes. v3->v4/v5
  residual fixes: dialog geometry (danger button is RIGHT of Cancel),
  checkbox box-only response (left_shift 43), type-button bbox crops,
  SE2 tab ambiguity resolved by targeted rerun SE2b (LM Studio verified).
- NEXT-ACTIONS targeted controls all exercised: Settings Help Scout & LM
  Studio tabs (LM Studio via SE2b), Knowledge import expander, Incidents
  filters (+New Incident form, checkbox on/off), Reports Builder, Connectors
  form, Diagnostics clear-log CONFIRMED EXECUTED (dialog opened, danger
  button clicked, log cleared; button then absent with 0 events), AI Center
  Copilot/Usage/Jobs tabs.
- Deliberate non-clicks (state preservation for W-002/W-007): all sync
  triggers, Mark complete, theme toggle. DB after pass 2 (read-only):
  organizations=0, conversations=0, sync_runs=0, cursors=0, checkpoints=0,
  jobs=0, application_errors=0 (the 1 synthetic boot event was consumed by
  the executed clear-log).
- Environment limitation recorded (NOT an app defect): XTEST KeyPress does
  not reach egui TextEdit here (Customers/Support Graph text entry
  source-verified only; xdotool unavailable).
- New findings FIND-005..FIND-010 (all Low; mostly source-verified):
  inconsistent destructive-action confirmation; idle Cancel-running-sync
  misleading toast; hidden knowledge-gap writes; radar fire-and-forget
  enqueues; silent input coercion (refresh-seconds default, capacity clamp,
  length-10 dates); Customers back-to-search query loss.
- Evidence: CONTROL/ 66 PNGs + control-verify-events.jsonl + CONTROL-app.log;
  manifest E-032..E-080 appended (all validated: PIL + visual reads incl. an
  8-frame sweep and chip/checkbox pixel checks).
- Status changes: FEAT-002/003 -> SATISFIED; FEAT-004 IN PROGRESS (columns
  complete for F- and C- ledgers; workflows pending).


## 2026-10-24 ~22:05 UTC — W-008 command-palette runtime check (partial; FIND-011 opened)

- Palette OPENS via the header "Search ⌘K" button (real click at 1059,26;
  OCR-located; diff 1.69; repeatable) — empty state "Type to search" +
  explainer verified visually and by OCR (E-081).
- Ctrl+K chord injected while open: no-op (diff 0.0) — the XTEST keyboard
  limitation now verified for chords, not just text.
- Backdrop close matrix: 5 of 6 outside clicks failed to close the palette
  and two interacted with the UI UNDERNEATH (contradicting the palette.rs
  item-133 catcher comment); one click at (300,300) did close it.
  Recorded as FIND-011 (Low, runtime-confirmed, root cause TBD).
- Header button + sidebar trigger are open()-only (not toggles) — with the
  palette open, mouse-only dismissal is unreliable (FIND-011).
- Query typing / hit-row navigation / Escape close blocked by the keyboard
  limitation (recorded, not an app defect); sidebar trigger row not clicked
  (FIND-004 overlap zone; same open() call source-verified).
- WORKFLOW-COVERAGE W-008 -> PARTIAL; manifest E-081..E-086.
