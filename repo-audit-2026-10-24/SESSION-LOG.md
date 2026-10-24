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

## 2026-10-24 ~22:25 UTC — W-007 theme round-trip executed (DONE, all steps PASS)

- Full light↔dark round-trip under real input (evidence/artifacts/w07-theme.py,
  self-contained XTEST stack; 3 app launches + 1 precondition-restore launch):
  * S002 initial light (lum 252.3); moon icon bbox measured [1138,19,1179,54]
    on the icon-only header button (C-APP-006 renders tooltip only).
  * S003 header button (1145,30) light→dark: whole-UI diff 237.2, lum 19.7,
    application_settings.theme='dark' synchronously.
  * S004 sidebar row (100,728 — FIND-004 rect) dark→light: diff 237.2; frame
    identical to initial (0.0).
  * S005 sidebar row light→dark again (restart prep).
  * S006 process exit with theme='dark' already stored.
  * S007 RELAUNCH BOOTS DARK — frame structurally identical to pre-restart
    dark (diff 0.0); persistence (F-026 / theme.rs boot-load) confirmed.
  * S008 header button dark→light. S009 second RELAUNCH BOOTS LIGHT (0.0).
  * S010 end: theme='light', original precondition restored; never-synced
    precondition untouched.
- Tooling honesty (run-1 defects, both fixed and retained in the events
  JSONL): (1) db_matches check wrongly demanded an explicit 'light' value for
  the unset-key default state — corrected to theme.rs semantics (missing key
  → Light); (2) verdict all() treated the numeric same_ref_diff 0.0 as
  falsy, producing one false FAIL at S007 despite all boolean checks true —
  verdict now computed from the boolean subset. Between runs the theme was
  restored to light THROUGH THE APP's own sidebar control (restore run in
  W07-app.log) — no out-of-band DB writes.
- Evidence: WORKFLOW/W07-01..07.png (7 frames, PIL+visual), E-087..E-095;
  WORKFLOW-COVERAGE W-007 → DONE; FEATURE-INVENTORY carried-bullet updated.
- Next workflow: W-010 (backups/export), then W-006 → W-005 → W-002 →
  W-003/W-004 → W-009 (NEXT-ACTIONS order).

## 2026-10-24 ~22:35 UTC — publication-completeness fix: audit log evidence was git-ignored; E-002 log regenerated

- DISCOVERED: the repo-root `.gitignore` (`*.log`) silently excluded every
  file under repo-audit-2026-10-24/evidence/logs/ from all audit commits —
  `git status` reported clean because ignored files are invisible to it, so
  the "push verified" checkpoints shipped WITHOUT the log evidence. Only the
  logs/ subtree is affected (full ignored-scan of the audit folder = logs
  only; screenshots/artifacts/JSONLs are tracked).
- ALSO FOUND: manifest E-002 referenced evidence/logs/F-CAND-001-port-free.log
  which no longer existed on disk (never committed; locally lost).
- FIX (no app code, no user config touched): (1) regenerated the E-002 log by
  re-running the exact FIND-001 scenario (default config launch while
  next-server holds :3000; launch-only capture, no state consumed) — the log
  again shows `WARN supportos_plus: loopback listener could not be started,
  error: Address already in use (os error 98)` at main.rs:129 (42 lines,
  evidence/logs/F-CAND-001-port-free.log); (2) all audit logs force-added
  (git add -f) so every manifest-referenced path is now tracked; .gitignore
  left untouched.
- Checkpoint 5 commit includes W-007 (E-087..E-095) + this fix; manifest
  re-verified: zero missing paths.

## 2026-10-24 ~22:55 UTC — W-010 backups/export executed (DONE-PARTIAL: C-SET-66 deferred by plan)

- Full pass with real input (evidence/artifacts/w010-backups.py; 4 runs, all
  retained in the events JSONL):
  * Settings reached via the pill-verified walk (S003); "Backups & export"
    tab opened on first click (S004, (606,91)).
  * "create backup" (S005): new pair on disk
    supportos-backup-2026-10-24T22-25-55-627Z.db (1847296 bytes) +
    .settings.json sidecar; inline green success message
    "backup /home/z/.../supportos-backup-….db"; list refreshed newest-first.
    Note: the confirmation is an INLINE success_text (settings.rs
    self.message), not a floating toast — ledger wording "toast" reconciled.
  * "export JSON" (S006): supportos-export-1792880769998.json (252 B).
  * "export conversations CSV" (S007): supportos-conversations-….csv
    (65 B, header-only — zero conversations).
  * "restore" (S008): FIRST CLICK ARMS ONLY — red warning "Restore …? The
    current database will be REPLACED by this backup (verified first)…" +
    cancel + red "really restore" Destructive button (1706 red px detected);
    DB counters stable; NO .restore-tmp/.restore-prev swap files — the
    audit-item-11 two-step gate is runtime-verified.
  * "cancel" (S009) disarms. S010: DB integrity ok, counters unchanged,
    theme=light, never-synced precondition intact.
  * C-SET-66 "really restore" deliberately NOT executed (NEXT-ACTIONS
    defers restore-replace until QA data exists; W-006 will create the
    first data rows). Swap mechanics source-verified only.
- Tooling history (retained): run-1 transient XTEST no-op at correct coords
  (live probe reproduced the click working); run-2 find_label v1 wide-window
  defect (Settings title merged into the OCR line; ranking picked the widest
  window → click 150px left) fixed in v2 (same-visual-row dy≤5 +
  tightest-window ranking — v1's comment claimed "tightest" but sorted the
  opposite); run-3 white-on-red "really restore" OCR miss → red-pixel
  detector. All click steps now verify-and-retry with every attempt logged.
- Entry-point correction recorded: the Shell runtime card has no backup
  surface; backups live in Settings only.
- Evidence: WORKFLOW/W10-01..07.png (PIL+visual), E-096..E-104;
  WORKFLOW-COVERAGE W-010 → DONE-PARTIAL. Next workflow: W-006.

## 2026-10-24 ~23:20 UTC — W-006 automation rule executed (DONE-PARTIAL: creation tooling-limited)

- Mouse-only pass with real input (evidence/artifacts/w006-automation.py;
  4 runs, all retained):
  * Automation page via pill-verified walk; empty states confirmed
    ('0 rule(s)', 'No approvals waiting').
  * 'New Rule' toggle opens/closes the form (C-AUTO-01; button is a dark
    Primary+icon — OCR-blind, dark-rect detector).
  * Trigger chips 3/3 with conditional swaps (C-AUTO-03): Tag added replaces
    from→to with a tag field; SLA at risk removes the row entirely (action
    row pulls up 41px — structural evidence); Status changed restores.
  * Action chips 5/5 with conditional fields (C-AUTO-06): assignee id /
    note body / priority / new status / tag.
  * 'Create Rule' on an EMPTY form → 'error: rule name required' (name-first
    validation) with ZERO records (C-AUTO-12 empty path; DB read back 0).
- BLOCKED (recorded as tooling-limited, NOT exceptions): rule CREATION needs
  text input (XTEST keyboard limitation, verified twice) — consequently the
  rule-row controls (enabled/disabled, delete confirm) and the approvals
  queue (approve/reject) are unreachable with no rule in existence. The
  C-SET-66 'really restore' deferral extends (W-006 cannot produce QA data
  rows as planned; only a text-capable run or an explicit QA-fixture
  decision can unblock it).
- RUNTIME OBSERVATION (minor, recorded not filed): the validation error
  'error: rule name required' persists after the form is closed (toggle
  does not clear self.error).
- Tooling history (retained): run-1 S004b predicate false-negative (the
  action row's 'Add tag' chip slid into the trigger OCR band after the SLA
  layout shift) → action-row-position-aware predicates; run-2 'new status'
  hint OCR truncation ('new statu') → needle fix; run-3 stale-error
  close-check fix.
- Evidence: WORKFLOW/W006-01..08.png (12 frames, PIL+visual), E-105..E-118;
  WORKFLOW-COVERAGE W-006 → DONE-PARTIAL. Next workflow: W-005.
