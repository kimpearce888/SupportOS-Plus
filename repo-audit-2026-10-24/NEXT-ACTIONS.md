# Next Actions

## Immediate next action
1. **Action:** W-007 theme round-trip (no data deps): click the header theme
   button (top-right ~(1130-1160, 19-41), measured) and/or the sidebar theme
   row (respect FIND-004 sliver rules), verify the theme flips (light↔dark,
   big content diff), restart the app and verify persistence via the
   settings table, then flip back. Record vs theme.rs / F-026 claims.
   - Relevant IDs/files: W-007; C-APP-006/007; theme.rs; F-026
   - Prerequisites: Xvfb tooling proven
   - Records: WORKFLOW-COVERAGE.md (W-007), manifest, SESSION-LOG.md

## Remaining actions (dependency order)
2. W-010 backups (export/list; avoid restore-replace until data exists) →
   W-006 automation rule (create + approve/reject; exercises C-AUTO forms) →
   W-005 knowledge ingest (paste-a-document path from the import expander —
   no credentials needed) → **W-002 sync lifecycle (consumes the never-synced
   precondition: initial sync attempt fails without credentials — record as
   designed behavior + FIND-001 webhook-drain check)** → W-003/W-004 as data
   allows → W-009.
3. FIND-001 queued runtime checks: Settings → Help Scout tab listener-health
   message; webhook drain at worker start (degraded listener path).
4. Edge/failure/recovery + cross-feature tests (QA-004/005) incl.
   second-instance UI check (FIND-002 reclassification decision).
5. Formal QA ledger (QA-001..007) with per-step screenshots (EVID-001..006);
   exceptions to evidence/exceptions.csv as they arise.
6. ARCHITECTURE.md deep-dive (workers/catalog/ai paths) — ARCH-001 completion.
7. Coverage reconciliation (COV-001/002) + evidence accounting (EVID-006)
   across F-/C-/W-/E-/EXC- ledgers.
8. REPORT.md consolidation (REPORT-001) and completion gate (04 §5).
9. Git publication of audit folder per 05 §8–9 (GIT-001..003).
4. FIND-001 queued runtime checks: Settings → Help Scout tab listener-health
   message; webhook drain at worker start (degraded listener path).
5. Edge/failure/recovery + cross-feature tests (QA-004/005) incl.
   second-instance UI check (FIND-002 reclassification decision).
6. Formal QA ledger (QA-001..007) with per-step screenshots (EVID-001..006);
   exceptions to evidence/exceptions.csv as they arise.
7. ARCHITECTURE.md deep-dive (workers/catalog/ai paths) — ARCH-001 completion.
8. Coverage reconciliation (COV-001/002) + evidence accounting (EVID-006)
   across F-/C-/W-/E-/EXC- ledgers.
9. REPORT.md consolidation (REPORT-001) and completion gate (04 §5).
10. Git publication of audit folder per 05 §8–9 (GIT-001..003): stage only
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
  ledger + 46 runtime control exercises (E-032..E-080), and the W-008
  palette check (E-081..E-086, FIND-011) — all recorded.
