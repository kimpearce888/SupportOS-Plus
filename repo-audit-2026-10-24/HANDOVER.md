# HANDOVER.md — authoritative breakpoint record

> New session: read this file first, then PROGRESS.md → NEXT-ACTIONS.md →
> SESSION-LOG.md (tail) → FINDINGS.md. Work only from the recorded state;
> do not redo completed items (see NEXT-ACTIONS.md §Do not repeat).

## Repository and Git state
- Working tree: clean at last checkpoint (verify with `git -C /home/z/my-project/audit/SupportOS-Plus status --short`)
- Branch: `repo-audit/2026-10-24` (up to date with origin at checkpoint time)
- Pre-existing HEAD (audited revision): `3affe7e`
- Audit commits (audit artifacts only, folder `repo-audit-2026-10-24/`):
  `e15ffa8` (checkpoint 1), `eec1eed` (checkpoint 1a), `fe7101a` (checkpoint 2,
  feature walk), pass-2 control-ledger checkpoint, `006d047` (checkpoint 4,
  W-008), `a5e47ef` (checkpoint 5, W-007 + log-publication fix)
- Remote: `https://github.com/kimpearce888/SupportOS-Plus` (credential store,
  kimpearce888); push of `a5e47ef` verified 2026-10-24 ~22:40 UTC
- PUBLICATION NOTE (checkpoint 5): the repo-root `.gitignore` `*.log` rule had
  silently excluded ALL audit log evidence from checkpoints 1–4; every
  manifest-referenced log is now force-added and tracked; the missing E-002
  log was regenerated from the exact FIND-001 scenario (launch-only, no state
  consumed)
- Publish rule (05 §8–9): only `repo-audit-2026-10-24/` may be committed/pushed;
  never app code, never force-push

## Where the audit stands (one paragraph)
Instruction gate SATISFIED. Build (0 warnings), launch, TEST-001 (703/0/4)
recorded. Architecture v1 written. Pass 1 (23/23 page walk, E-006..E-031)
done. **Pass 2 (control/option ledger, FEAT-002/003) is COMPLETE:** a 535-row
source-first ledger (C-<PAGE>-<nn>) lives in FEATURE-INVENTORY.md, reconciled
by 46 real-input runtime control exercises (control-verify.py v3; OCR label
targeting + dark-rect/bbox detectors; E-032..E-080). The NEXT-ACTIONS targeted
controls were all exercised, including the executed Diagnostics clear-log
confirmation flow (log now empty — the button is absent with 0 events). Two
recorded notes: XTEST keyboard injection does not reach egui TextEdits in this
sandbox (now also verified for chords — Ctrl+K is a no-op) and the
Onboarding/Shell sync-controls are deliberately deferred to W-002/W-007 to
preserve the never-synced precondition. **W-008 executed PARTIAL** (E-081..
E-086): palette opens via the header button with the correct empty state;
the click-outside backdrop is unreliable → FIND-011. **W-007 executed DONE**
(E-087..E-095): header icon button (C-APP-006) and sidebar theme row
(C-APP-005) flip light↔dark in both directions; `application_settings.theme`
follows every click synchronously; dark AND light persistence across restarts
verified (two relaunches, rendered frames structurally identical, diff 0.0);
original light precondition restored. **W-010 executed DONE-PARTIAL** (E-096..
E-104): Backups & export tab, create backup (new pair on disk), export JSON
+ CSV, restore ARMS-ONLY two-step gate runtime-verified (red REPLACED
warning, DB stable, no swap files), cancel disarms; "really restore"
(C-SET-66) deliberately deferred until QA data exists. Findings now FIND-001
(Medium) + FIND-002..FIND-011 (Low). Next workflow: W-006 (creates first QA
data rows).

## What is verified and where the evidence lives
- Instruction loading: INSTRUCTION-LOADING-RECORD.md; instructions/ (15 files)
- Traceability: REQUIREMENT-TRACEABILITY.csv (50/50 rows; BOOT/ENV/BUILD/TEST/
  FEAT-001/002/003 SATISFIED; FEAT-004 IN PROGRESS)
- Build/launch/tests: BUILD-AND-INSTALL.md; evidence/screenshots/BUILD/;
  evidence/logs/F-CAND-001*.log; single-instance-second-run.log
- Page walk: FEATURE-INVENTORY.md (F-001..F-026); evidence/screenshots/FEATURE/;
  evidence/artifacts/feature-walk.py + events JSONL
- Control ledger: FEATURE-INVENTORY.md §Runtime verification + §ledger
  (groups a–h merged); evidence/screenshots/CONTROL/ (66 PNGs);
  evidence/artifacts/control-verify.py + control-verify-events.jsonl (incl.
  retained v1/v2 tool-calibration rows); evidence/logs/CONTROL-app.log
- Findings: FINDINGS.md (FIND-001..FIND-010)
- Workflows: WORKFLOW-COVERAGE.md (W-007 DONE; W-010 DONE-PARTIAL, C-SET-66
  deferred; W-008 PARTIAL; W-001..W-006, W-009 queued)
- Report: REPORT.md (skeleton, IN PROGRESS)

## Tooling facts a new session must know
- Environment: Rust 1.99.1 pinned; Xvfb :99 helper `./start-xvfb.sh`; app
  launcher `./run-app.sh`; app data dir ~/.local/share/supportos-plus
  (QA-owned). DB state after pass 2 (read-only checks): organizations=0,
  conversations=0, sync_runs=0, sync_cursors=0, sync_checkpoints=0, jobs=0,
  application_errors=0; theme=light; last page ≈ Dashboard.
- Input injection: python-xlib XTEST works for MOUSE only — XTEST KeyPress
  does NOT reach egui TextEdits in this environment (verified twice; xdotool
  unavailable). Mouse-only control testing is proven end-to-end.
- control-verify.py v3 patterns: OCR 4-pass label targeting (plain/inverted/
  3×/sparse-2×, conf≥15), sliding-window needle matching, dark-rect and bbox
  detectors for `+icon` buttons, per-step diff thresholds (chip 0.06–0.10,
  no-op 0.0–0.01, checkmark 0.007, view switch 0.6–5.5, theme flip 237.2),
  dialog danger button is RIGHT of Cancel, incidents checkbox responds only
  to the box (word center −43 px), Diagnostics clear-log button absent when
  0 events.
- W-007 control positions (real-input verified both directions): header
  theme icon button (1145,30) — icon-only, tooltip label; sidebar theme row
  (100,728) inside FIND-004 rect [714,743]. Theme key lives in the
  `application_settings` table (NOT `settings`); a missing key = Light
  default (theme.rs::from_setting_value); toggles store synchronously.
- W-010 positions (real-input verified): Settings nav row; "Backups &
  export" tab chip (606,91); "create backup" dark Primary+icon button
  (307,198; OCR-blind — dark-rect detector); "export JSON" (423,221);
  "export conversations CSV" (574,221); per-row "restore" (880,269);
  armed-state "cancel" (283,448-ish). The Settings tab row merges with the
  page title in OCR — use same-row-constrained window matching (find_label
  v2 in w010-backups.py). "really restore" red Destructive button is
  OCR-blind → red-pixel detector. Backups dir QA artifacts now include 3
  backup pairs + 2 CSVs + 2 JSONs (all audit-created, harmless).
- Sandbox reaps background processes between tool calls: keep each app run
  self-contained in one call (the scripts already pkill orphans and verify
  the instance lock).
- Do NOT use the `ui-screenshot` cargo feature for evidence (non-default
  build); its existence is recorded as a discovered QA affordance instead.

## Coverage snapshot (2026-10-24 ~22:55 UTC)
- Pages inventoried w/ evidence: 23/23; shell-level features: 3
- Control/option ledger rows: 535; controls runtime-exercised: 46 pass-2
  steps + W-007 (both theme controls, both directions, 2 restarts) + W-010
  (6 backup/export controls incl. the two-step restore gate)
- Formal control testing (QA-001..007 ledger): NOT STARTED (pass-2 exercises
  are inventory verification, not the formal ledger)
- Workflows identified / executed: 10 identified; W-007 DONE, W-010
  DONE-PARTIAL (C-SET-66 deferred until data exists), W-008 PARTIAL;
  7 queued (W-006 next)
- Evidence: E-001..E-104, all validated; every manifest path exists AND is
  git-tracked (log-evidence gap closed at checkpoint 5)
- Accepted exceptions: none · Unresolved gaps: none new (W-001/W-003 external
  data blockers unchanged; C-SET-66 deferred by plan)
- Findings: FIND-001 Medium; FIND-002..FIND-011 Low
- DB state after W-010 (read-only check): theme=light; all sampled counters 0
  (never-synced precondition intact); PRAGMA integrity_check ok
- Coverage snapshot time: 2026-10-24 ~22:55 UTC

## Outstanding tasks (ordered)
1. Workflow execution order per NEXT-ACTIONS.md #2 (W-006 automation rule
   [creates first QA data rows] → W-005 knowledge ingest → W-002 sync
   lifecycle [consumes never-synced state] → W-003/W-004 as data allows →
   W-009; then the deferred C-SET-66 "really restore"; W-008 PARTIAL done —
   remaining W-008 steps are keyboard-blocked, record as tooling-limited,
   not exceptions).
3. FIND-001 UI-signal + webhook-drain checks.
4. Edge/failure/recovery + cross-feature tests (QA-004/005) incl.
   second-instance UI check (FIND-002 reclassification decision).
5. Formal QA ledger (QA-001..007) with per-step screenshots (EVID-001..006);
   exceptions to evidence/exceptions.csv as they arise.
6. ARCHITECTURE.md deep-dive (workers/catalog/ai paths) — ARCH-001 completion.
7. COV-001/002 reconciliation → REPORT-001 → completion gate (04 §5).
8. Final publication pass (GIT-001..003).

## Blockers and known problems
- W-001 OAuth round-trip needs external Help Scout credentials (likely
  exception registration); W-003 deep steps need data.
- XTEST keyboard limitation (see tooling facts) — verified for text AND
  chords; remaining palette steps (query typing, Escape) are tooling-limited.
- FIND-011: palette dismissal unreliable — if a workflow needs the palette
  closed by mouse, use the (300,300)-region click or restart the app run.
- None environmental: Xvfb/XTEST mouse tooling proven end-to-end.

## Decisions and constraints to preserve
- Source of truth = actual implementation + observed runtime behavior; prior
  audit `qa-audit/20261019T050847Z` is background only.
- Do not modify app code; do not weaken tests; only the audit folder is
  published; incremental commits; no overclaims (failures recorded as such).
- ID namespaces: features F-xxx, controls C-xxx, workflows W-xxx (steps
  W-xxx-Snnn), evidence E-xxx, exceptions EXC-xxx, findings FIND-xxx.
- The walk's v1–v4 tool defects and the orphan-process incident, and the
  pass-2 v1/v2 calibration history, are retained in the events JSONLs and
  SESSION-LOG.md for honesty; the final runs are the published evidence.
- State preservation: never consume the never-synced precondition before
  W-002 (no "Sync now"/"Run initial sync"/"Mark complete" clicks); theme
  toggle reserved for W-007.

## Publication status
- Local: audit commits on `repo-audit/2026-10-24` (see §Repository); remote
  push verified at checkpoint time
- Publication blocker: none

## Last validated
- Timestamp/timezone: 2026-10-24 ~22:55 UTC
- Validator/check: W-010 10/10 steps PASS (events JSONL w10-* rows; C-SET-66
  deferred by plan); 7 W-10 PNGs + 7 W-07 PNGs PIL+visually validated;
  manifest E-001..E-104 zero missing paths; DB integrity ok, theme=light,
  never-synced intact; app log read (only the known FIND-001 WARN)
