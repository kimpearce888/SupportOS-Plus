# Next Actions

## Immediate next action
1. **Action:** Feature-inventory pass 1: walk every page in the running app under Xvfb using XTEST clicks on sidebar nav, capture per-page screenshots, and reconcile against crates/app-shell/src/pages/*.rs (23 PageKeys) into FEATURE-INVENTORY.md with stable IDs F-001.. (feature namespace; finding IDs are FIND-xxx now to avoid collision).
   - Relevant IDs/files: FEAT-001..004; FEATURE-INVENTORY.md; ARCHITECTURE.md §4; helpers run-app.sh/start-xvfb.sh
   - Prerequisites/current state: app verified launching (BUILD-002); XTEST verified importable; DB state: fresh/empty + auto-created
   - Exact steps: one self-contained call per 3–5 pages (sandbox reaps background processes): start Xvfb → run app → XTEST click nav item → ffmpeg screenshot per page → save to evidence/screenshots/F-xxx/
   - Evidence to capture: per-page PNGs (validated) + nav-click screenshots
   - Records to update: FEATURE-INVENTORY.md, evidence/manifest.csv, PROGRESS.md, SESSION-LOG.md
   - Expected outcome: inventoried pages with source + runtime cross-check; list of controls per page for pass 2

## Remaining actions (dependency order)
2. Architecture discovery → `ARCHITECTURE.md` (ARCH-001): entry points, crates/app-shell (egui GUI), crates/core, crates/catalog, crates/xtask, data/persistence paths.
3. Feature/control inventory → `FEATURE-INVENTORY.md` (FEAT-001..004): every page/control/option; use source + runtime UI discovery; stable IDs F-xxx/C-xxx.
4. Workflow map → `WORKFLOW-COVERAGE.md` (FLOW-001): connected journeys with branches and step IDs W-xxx-Snnn.
5. Manual UI QA under Xvfb with real input injection (QA-001..007): python-xlib XTEST (install pending) or xdotool alternative; screenshots via ffmpeg x11grab per step (EVID-001..006, EXC-001..003).
6. Existing test suite run (TEST-001): `cargo test` output recorded separately from manual QA.
7. Findings + fix recommendations (FIND-001/002, FIX-001) → `FINDINGS.md`.
8. Edge/failure/recovery and cross-feature tests (QA-004/005).
9. Coverage reconciliation (COV-001/002) and evidence accounting (EVID-006).
10. REPORT.md consolidation (REPORT-001) and final completion gate (04 §5).
11. Git publication of audit folder per 05 §8–9 (GIT-001..003): stage only `repo-audit-2026-10-24/`, descriptive commits, verify push, record hashes.

## Blocked actions
- None yet. Watch items: python-xlib not installed (pip install attempt pending); workspace build resource-heaviness (2 vCPU/4 GB) — monitor build.

## Do not repeat without a concrete reason
- Instruction download/read/verification (INSTRUCTION-LOADING-RECORD.md — hashes verified).
- Repository clone + initial state capture (HANDOVER.md §Repository).
- Rust toolchain installation (1.99.1 present at /home/z/.cargo).
- Traceability CSV reconciliation (50/50 preserved in REQUIREMENT-TRACEABILITY.csv).
