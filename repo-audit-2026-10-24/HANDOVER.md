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
  W-008), `a5e47ef` (checkpoint 5, W-007 + log-publication fix), `8074dd7`
  (checkpoint 6, W-010)
- Remote: `https://github.com/kimpearce888/SupportOS-Plus` (credential store,
  kimpearce888); push of `8074dd7` verified 2026-10-24 ~23:00 UTC
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
(C-SET-66) deliberately deferred until QA data exists. **W-006 executed
DONE-PARTIAL** (E-105..E-118, mouse-only): New Rule form toggle, trigger
chips 3/3 + conditional field swaps, action chips 5/5 + conditional fields,
empty-create validation ('rule name required', zero records); rule CREATION
is tooling-blocked (text input limitation) so rule-row/approval-queue
controls are unreachable — recorded as tooling-limited, not exceptions;
minor observation: validation error persists after form close. Findings now
FIND-001 (Medium) + FIND-002..FIND-011 (Low). Next workflow: W-005.

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
- Workflows: WORKFLOW-COVERAGE.md (W-006 DONE-PARTIAL; W-007 DONE; W-010
  DONE-PARTIAL, C-SET-66 deferred; W-008 PARTIAL; W-001..W-005, W-009 queued)
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
- W-006 positions (real-input verified): New Rule toggle (1117,91; dark
  Primary+icon, OCR-blind); trigger chips Status changed (392,281) / Tag
  added (495,280) / SLA (590,280); action chips Assign (357,368) / Add tag
  (443,368) / Send note (515,368) / Change status (622,368) / Set priority
  (733,368); Create Rule (300,450; dark rect). Conditional-row layout shifts
  with trigger kind — position-aware OCR predicates required ('tag' in the
  trigger band is ambiguous with the action row's 'Add tag' chip). 'new
  status' hint OCRs as 'new statu'.
- Sandbox reaps background processes between tool calls: keep each app run
  self-contained in one call (the scripts already pkill orphans and verify
  the instance lock).
- Do NOT use the `ui-screenshot` cargo feature for evidence (non-default
  build); its existence is recorded as a discovered QA affordance instead.

## Coverage snapshot (2026-10-24 ~23:20 UTC)
- Pages inventoried w/ evidence: 23/23; shell-level features: 3
- Control/option ledger rows: 535; controls runtime-exercised: 46 pass-2
  steps + W-007 (both theme controls, both directions, 2 restarts) + W-010
  (6 backup/export controls incl. the two-step restore gate) + W-006
  (toggle, 3 trigger chips, 5 action chips, empty-create validation)
- Formal control testing (QA-001..007 ledger): NOT STARTED (pass-2 exercises
  are inventory verification, not the formal ledger)
- Workflows identified / executed: 10 identified; W-006 DONE-PARTIAL
  (creation tooling-limited), W-007 DONE, W-010 DONE-PARTIAL (C-SET-66
  deferred), W-008 PARTIAL; 6 queued (W-005 next)
- Evidence: E-001..E-118, all validated; every manifest path exists AND is
  git-tracked (log-evidence gap closed at checkpoint 5)
- Accepted exceptions: none · Unresolved gaps: none new (W-001/W-003 external
  data blockers unchanged; C-SET-66 + W-006 creation deferred by tooling)
- Findings: FIND-001 Medium; FIND-002..FIND-011 Low
- DB state after W-006 (read-only check): theme=light; automation_rules=0,
  approvals=0, runs=0 (never-synced precondition intact); integrity ok
- Coverage snapshot time: 2026-10-24 ~23:20 UTC

## Outstanding tasks (ordered)
1. Workflow execution order per NEXT-ACTIONS.md #2 (W-005 knowledge ingest
   [paste-a-document path — but note the paste field is a TextEdit; the
   ingest may be tooling-limited the same way; verify the expander/controls
   reachable by mouse] → W-002 sync lifecycle [consumes never-synced state]
   → W-003/W-004 as data allows → W-009; then deferred C-SET-66 + W-006
   creation, both awaiting a text-capable run or QA-fixture decision;
   W-008 remaining steps keyboard-blocked, record as tooling-limited).
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
- Timestamp/timezone: 2026-10-24 ~23:20 UTC
- Validator/check: W-006 8/8 steps PASS (events JSONL w006-* rows; creation
  tooling-limited + stale-error observation recorded); W-006 PNGs (12) +
  W-10 (7) + W-07 (7) PIL+visually validated; manifest E-001..E-118 zero
  missing paths; DB integrity ok, theme=light, never-synced intact; app log
  read (only the known FIND-001 WARN)
