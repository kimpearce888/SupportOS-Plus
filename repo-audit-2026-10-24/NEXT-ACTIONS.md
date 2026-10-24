# Next Actions

## Immediate next action
1. **Action:** Command-palette runtime check (W-008). Preferred entry: real
   CLICK on the sidebar "Search…" row or the header "Search ⌘K" button
   (C-APP rows) — XTEST Ctrl+K injection may not reach the app (keyboard
   limitation recorded in HANDOVER); attempt the chord once and record the
   result either way. Then: empty-query state, a query that matches seeded
   page titles (e.g. "inbox"), Escape close, outside-click close. Record vs
   palette.rs claims (C-PAL-01..04: live search overlay, MAX_HITS=12,
   href→page routing incl. the top-level collapse behavior).
   - Relevant IDs/files: W-008; C-APP-002/007/011; C-PAL-01..04;
     evidence/artifacts/control-verify.py (reuse harness)
   - Prerequisites: Xvfb tooling proven; no data prerequisites for page-title hits
   - Records: WORKFLOW-COVERAGE.md (W-008), manifest, SESSION-LOG.md
2. Then: workflow map execution (FLOW-001) in the order below.

## Remaining actions (dependency order)
3. W-007 theme round-trip (no data deps; header theme button + sidebar theme
   row; verify persistence across restart) → W-010 backups (export/list;
   avoid restore-replace until data exists) → W-006 automation rule
   (create + approve/reject; exercises C-AUTO forms) → W-005 knowledge ingest
   (paste-a-document path from the import expander — no credentials needed)
   → **W-002 sync lifecycle (consumes the never-synced precondition: initial
   sync attempt fails without credentials — record as designed behavior +
   FIND-001 webhook-drain check)** → W-003/W-004 as data allows → W-008
   (if not done in #1) → W-009.
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
  (E-002..E-005), the 23-page walk (E-006..E-031), and the pass-2 control
  ledger + 46 runtime control exercises (E-032..E-080) — all recorded.
