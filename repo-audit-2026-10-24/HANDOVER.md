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
  (checkpoint 6, W-010), `9e91910` (checkpoint 7, W-006), `9db4045`
  (checkpoint 8, W-005)
- Remote: `https://github.com/kimpearce888/SupportOS-Plus` (credential store,
  kimpearce888); push of `9db4045` verified 2026-10-24 ~23:50 UTC
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
minor observation: validation error persists after form close. **W-005
executed DONE-PARTIAL** (E-119..E-130, mouse-only): import expander +
disabled-Import gate no-op runtime-verified (C-KNOW-08), visibility chips
clicked, Issue Radar enqueues runtime-verified ('gap detection queued —
reload in a few seconds' inline line; jobs 0→2 ai/completed; NO completion
signal — FIND-008 upgraded to runtime-confirmed); ingest TEXT path
tooling-blocked. **W-002 executed DONE-PARTIAL** (E-131..E-137, no-
credentials leg — the sanctioned consumer of the never-synced
precondition): 'Run initial sync' + Dashboard 'Sync now' both refused by
the DESIGNED guard (error toast captured visually, application_errors +1
each, sync tables stay 0, freshness chip unchanged); Settings→Help Scout
tab shows 'not configured' + redirect uri http://localhost:3000/oauth
callback (FIND-001 visual); listener-health surface needs credentials
(external). FORENSIC: a pre-existing application_errors row (21:31:15)
was attributed to the W-008 FIND-011 fall-through click at (1100,90)
pressing the Dashboard 'Sync now' button — FIND-011 impact upgraded.
Findings now FIND-001 (Medium) + FIND-002..FIND-011 (Low; FIND-008
runtime-confirmed; FIND-011 consequence attributed). Next workflow: W-009
(second-instance UX); W-003/W-004 data-blocked.

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
- Workflows: WORKFLOW-COVERAGE.md (W-002/W-005/W-006/W-010 DONE-PARTIAL;
  W-007 DONE; W-008 PARTIAL; W-001/W-003/W-004 blocked, W-009 next)
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
- W-005 positions (real-input verified): 'Import documents' toggle row
  STABLE at (313,242) when open or closed (find_label on the open frame can
  mis-match inside the folder explainer — constrain the search row);
  disabled 'Import document' (317,650); visibility chips customer-safe
  (702,481) / internal-only (440,482); radar header buttons Detect gaps now
  (856,93) / Reload clusters (992,91) / Cluster now (1112,92). The radar
  enqueue renders an inline 'gap detection queued — reload in a few
  seconds' line that clears on reload (FIND-008 behavior).
- W-005 DB note: two `ai` jobs (completed) remain in the jobs table as QA
  artifacts; sync tables untouched.
- W-002 positions/facts: Sync Health 'Run initial sync' (277,288; dark
  button — inverted-pass OCR only, crop tight to the row); header chip
  'never synced' at (860-1020, 8-48) — OCRs as 'neversynce' (use prefix
  'neversync'); Dashboard 'Sync now' (1100,90; Outline+Refresh icon,
  OCR-blind — the position is evidence-backed via FIND-011's fall-through
  consequence). Sync refusals write to application_errors (NOT the tracing
  log); the guard refuses BEFORE creating run rows. Backward sidebar nav:
  scroll-top + click first band + pill-verify (scroll alone leaves the old
  page selected).
- Sandbox reaps background processes between tool calls: keep each app run
  self-contained in one call (the scripts already pkill orphans and verify
  the instance lock).
- Do NOT use the `ui-screenshot` cargo feature for evidence (non-default
  build); its existence is recorded as a discovered QA affordance instead.

## Coverage snapshot (2026-10-25 ~00:10 UTC)
- Pages inventoried w/ evidence: 23/23; shell-level features: 3
- Control/option ledger rows: 535; controls runtime-exercised: 46 pass-2
  steps + W-007 + W-010 + W-006 + W-005 + W-002 (both sync entry points'
  designed refusals, freshness chip, Help Scout tab state)
- Formal control testing (QA-001..007 ledger): NOT STARTED (pass-2 exercises
  are inventory verification, not the formal ledger)
- Workflows identified / executed: 10 identified; W-002/W-005/W-006/W-010
  DONE-PARTIAL, W-007 DONE, W-008 PARTIAL; W-001/W-003/W-004 blocked
  (external/data); W-009 next
- Evidence: E-001..E-137, all validated; every manifest path exists AND is
  git-tracked (log-evidence gap closed at checkpoint 5)
- Accepted exceptions: none · Unresolved gaps: W-001 OAuth round-trip +
  W-003 deep steps blocked on external credentials/data; C-SET-66, W-006
  creation, W-005 text path, W-008 keyboard steps deferred by tooling
- Findings: FIND-001 Medium; FIND-002..FIND-011 Low (FIND-008
  runtime-confirmed; FIND-011 consequence attributed)
- DB state after W-002 (read-only check): theme=light; sync_runs/cursors/
  checkpoints=0 (never-synced DB state survives); application_errors=5
  (1 pre-existing attributed to FIND-011 + run-2 partial 1 + run-3 partial
  1 + final-run 2 — DB-verified; the initial '4' derivation missed run-3's
  row, corrected after a read-only DB reconciliation; see the
  w002-reconciliation JSONL row);
  jobs=2 (ai/completed, QA artifacts); integrity ok
- Coverage snapshot time: 2026-10-25 ~00:10 UTC

## Outstanding tasks (ordered)
1. W-009 second-instance UX (launch a second instance while one runs —
   exit 1 + stderr message already recorded as FIND-002; check for any UI
   affordance and make the FIND-002 reclassification decision).
2. FIND-001 remaining sub-checks: listener-health message needs saved
   credentials (external) — register the exception per 05 if the round-trip
   cannot be executed; webhook drain stays source-verified.
3. Edge/failure/recovery + cross-feature tests (QA-004/005) incl. the
   FIND-002 reclassification.
4. Formal QA ledger (QA-001..007) with per-step screenshots (EVID-001..006);
   exceptions to evidence/exceptions.csv as they arise (W-001 credential
   blocker likely becomes one).
5. ARCHITECTURE.md deep-dive (workers/catalog/ai paths) — ARCH-001 completion.
6. Coverage reconciliation (COV-001/002) + evidence accounting (EVID-006).
7. REPORT.md consolidation (REPORT-001) and completion gate (04 §5).
8. Git publication of audit folder per 05 §8–9 (GIT-001..003).

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
- Timestamp/timezone: 2026-10-25 ~00:10 UTC
- Validator/check: W-002 9/9 steps PASS (events JSONL w002-* rows; toast
  captured visually, OCR miss noted); W-002 PNGs (6) + all earlier WORKFLOW
  PNGs PIL+visually validated; manifest E-001..E-137 zero missing paths; DB
  integrity ok, theme=light, sync tables 0, application_errors=5 (all
  attributed); app log read (only the known FIND-001 WARN)
