# Next Actions

## Immediate next action
1. **Action:** Formal QA ledger consolidation (QA-001..007): assign final
   statuses from the already-executed work — QA-001 (operate the real UI:
   pass-1 walk + all workflows), QA-002 (entry points: pass 1), QA-003
   (controls: pass-2 535-row ledger + 46 exercises + workflow exercises),
   QA-006 (accessibility/usability: disabled gates, dialogs, focus/
   keyboard limitation observations), QA-007 (continuity: HANDOVER/
   NEXT-ACTIONS) — and EVID-001..006 evidence accounting (screenshot
   coverage math per 03 §9). QA-004/005 are DONE (E-143..E-157).
   - Relevant IDs/files: REQUIREMENT-TRACEABILITY.csv; PROGRESS.md §counts;
     evidence/manifest.csv; exceptions.csv; instruction 03 §9 math
   - Prerequisites: none
   - Records: traceability statuses, PROGRESS counts, REPORT.md inputs
   - Then: ARCHITECTURE.md deep-dive (ARCH-001) → COV-001/002 → REPORT-001
     → completion gate → final publication.
## Remaining actions (dependency order)
2. W-003/W-004 remain data-blocked (register as exceptions if still
   unexecutable at report time); deferred (tooling): C-SET-66 "really
   restore" + W-006 rule creation + W-005 ingest text path + W-008
   keyboard steps (await a text-capable run or an explicit QA-fixture
   decision).
3. FIND-001 queued runtime checks — RESOLVED BY #1: listener-health message
   + OAuth round-trip register as exceptions (E-135 already captures the
   unconfigured tab); webhook drain stays source-verified (a signed event
   cannot be forged outside the app).
4. Edge/failure/recovery + cross-feature tests (QA-004/005) — the
   second-instance UI check is DONE (W-009, FIND-002 closed).
5. Formal QA ledger (QA-001..007) with per-step screenshots (EVID-001..006);
   exceptions to evidence/exceptions.csv as they arise.
6. ARCHITECTURE.md deep-dive (workers/catalog/ai paths) — ARCH-001 completion.
7. Coverage reconciliation (COV-001/002) + evidence accounting (EVID-006)
   across F-/C-/W-/E-/EXC- ledgers.
8. REPORT.md consolidation (REPORT-001) and completion gate (04 §5).
9. Git publication of audit folder per 05 §8–9 (GIT-001..003): stage only
    `repo-audit-2026-10-24/`, descriptive commits, verify push, record hashes.

## Blocked actions
- W-001 full OAuth round-trip: requires live Help Scout credentials/endpoint
  (external) — degraded-path checks remain possible; register exception if
  the round-trip cannot be executed.
- W-003 conversation lifecycle deep-steps: require synced/seeded conversation
  data (depends on W-002 outcome with no credentials).

## Do not repeat without a concrete reason
- Instruction download/read/verification (INSTRUCTION-LOADING-RECORD.md —
  hashes verified).
- Repository clone + initial state capture (HANDOVER.md §Repository).
- Rust toolchain installation (1.99.1 present at /home/z/.cargo).
- Traceability CSV reconciliation (50/50 preserved).
- Build (BUILD-001), launch verification (BUILD-002), TEST-001 suite run
  (703/0/4), loopback port matrix + second-instance runtime checks
  (E-002..E-005), the 23-page walk (E-006..E-031), the pass-2 control
  ledger + 46 runtime control exercises (E-032..E-080), the W-008
  palette check (E-081..E-086, FIND-011), the W-007 theme round-trip
  (E-087..E-095; theme restored to light, never-synced precondition intact),
  the W-010 backups/export pass (E-096..E-104; "really restore" deferred),
  the W-006 automation pass (E-105..E-118; creation tooling-limited), and
  the W-005 knowledge/radar pass (E-119..E-130; FIND-008 runtime-confirmed)
  — all recorded.
