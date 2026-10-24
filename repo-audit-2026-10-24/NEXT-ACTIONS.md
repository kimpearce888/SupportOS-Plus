# Next Actions

## Immediate next action
1. **Action:** Control/option inventory pass 2 (FEAT-002/003): enumerate every control, field, option, constraint and validation per page into FEATURE-INVENTORY.md §control ledger — source-first (pages/*.rs), then targeted runtime confirmation of non-obvious controls (Settings Help Scout & LM Studio tabs, Knowledge import expander, Incidents filters, Reports tabs incl. Builder, Connectors form, Diagnostics clear-log confirm, AI Center Copilot/Usage/Jobs tabs).
   - Relevant IDs/files: FEAT-002, FEAT-003, FEAT-004; FEATURE-INVENTORY.md; run-app.sh/start-xvfb.sh helpers
   - Prerequisites: page walk done (F-001..F-026 evidenced); Xvfb tooling proven
   - Exact steps: for each page, read its source file → list controls with stable IDs C-<page>-<nn> → runtime-verify a sample per page with XTEST clicks + screenshots (same verified-input method) → record
   - Evidence: per-control screenshots only where runtime behavior is claimed
   - Records: FEATURE-INVENTORY.md (ledger section), manifest, PROGRESS.md, SESSION-LOG.md
2. Then: command-palette runtime check (W-008) — Ctrl+K via XTEST key injection; empty-query Enter; record vs palette.rs claims.

## Remaining actions (dependency order)
3. Workflow map execution (FLOW-001): W-007 theme round-trip (no data deps) → W-010 backups → W-006 automation rule → W-005 knowledge ingest → W-002 sync lifecycle (never-synced → initial sync attempt will fail without credentials: record as designed behavior + FIND-001 drain check) → W-003/W-004 as data allows → W-008 → W-009.
4. FIND-001 queued runtime checks: Settings → Help Scout tab listener-health message; webhook drain at worker start (degraded listener path).
5. Edge/failure/recovery + cross-feature tests (QA-004/005) incl. second-instance UI check (FIND-002 reclassification decision).
6. Formal QA ledger (QA-001..007) with per-step screenshots (EVID-001..006); exceptions to evidence/exceptions.csv as they arise.
7. ARCHITECTURE.md deep-dive (workers/catalog/ai paths) — ARCH-001 completion.
8. Coverage reconciliation (COV-001/002) + evidence accounting (EVID-006) across F-/C-/W-/E-/EXC- ledgers.
9. REPORT.md consolidation (REPORT-001) and completion gate (04 §5).
10. Git publication of audit folder per 05 §8–9 (GIT-001..003): stage only `repo-audit-2026-10-24/`, descriptive commits, verify push, record hashes.

## Blocked actions
- W-001 full OAuth round-trip: requires live Help Scout credentials/endpoint (external) — degraded-path checks remain possible; register exception if the round-trip cannot be executed.
- W-003 conversation lifecycle deep-steps: require synced/seeded conversation data (depends on W-002 outcome with no credentials).

## Do not repeat without a concrete reason
- Instruction download/read/verification (INSTRUCTION-LOADING-RECORD.md — hashes verified).
- Repository clone + initial state capture (HANDOVER.md §Repository).
- Rust toolchain installation (1.99.1 present at /home/z/.cargo).
- Traceability CSV reconciliation (50/50 preserved in REQUIREMENT-TRACEABILITY.csv).
- Build (BUILD-001), launch verification (BUILD-002), TEST-001 suite run (703/0/4), loopback port matrix + second-instance runtime checks (E-002..E-005), and the 23-page walk (E-006..E-031) — all recorded.
