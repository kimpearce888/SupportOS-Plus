# Next Actions

## Immediate next action
1. **Action:** FIND-001 closing decision: register the W-001 OAuth
   round-trip + listener-health message sub-checks as an evidence
   exception (type E2 external dependency) in evidence/exceptions.csv,
   per instruction 05 — the sandbox has no live Help Scout endpoint or
   credentials, and W-002 already captured the unconfigured-tab state
   (E-135). Webhook drain stays source-verified (cannot forge a signed
   event).
   - Relevant IDs/files: FIND-001; W-001; evidence/exceptions.csv (create);
     INSTRUCTION-LOADING-RECORD.md §05 reference
   - Prerequisites: none
   - Records: exceptions.csv, FINDINGS.md FIND-001, manifest, SESSION-LOG
   - Then: edge/failure/recovery + cross-feature tests (QA-004/005) — the
     FIND-002 reclassification is DONE (W-009), carry its result in.

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
