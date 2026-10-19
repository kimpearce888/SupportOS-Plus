# QA Audit 20261019T050847Z — SupportOs-Plus

Universal Human-Behavior QA Audit per the audit specification supplied by the user
(Google Doc "Universal Human-Behavior QA Audit — Any Application or Repository").

- **Target repository**: https://github.com/kimpearce888/SupportOs-Plus
- **Audit target commit**: `19dfb35a714822ed94b6b5144eabb222997cc2ca` (main HEAD at audit start; CI run 39201357186 both jobs success)
- **Audit branch**: `qa-audit/20261019T050847Z` (created from main @ 19dfb35)
- **Audit type**: Independent QA / real-user / accessibility / software audit. **No product code changes** — product code on this branch is byte-identical to the audit target.
- **Started (UTC)**: 2026-10-19T05:08:47Z

## Index

| Artifact | Purpose |
|---|---|
| `executive-summary.md` | Target, environment, overall results, major findings, limitations |
| `application-map.md` | Discovered interfaces, architecture, features, dependencies, data boundaries |
| `control-inventory.csv` | Every discovered actionable control: context, expected behavior, status |
| `feature-workflow-matrix.csv` | Features × complete workflows × results × evidence |
| `state-transition-matrix.csv` | States, transitions, outcomes, durable-state verification |
| `cross-feature-coverage.csv` | Tested feature pairs/combos and exclusions |
| `scenario-catalog.md` | Scenarios: preconditions, steps, expectations, results, evidence |
| `test-command-results.md` | Commands, exit codes, test counts, build results, blockers |
| `defects.md` | Reproducible defects with evidence and qualified analysis |
| `coverage-gaps.md` | Everything blocked / not run / inconclusive, with reasons |
| `existing-test-coverage.md` | Mapping of existing tests to discovered behavior |
| `audit-metadata.json` | Machine-readable metadata: commits, counts, inventory, limitations |
| `evidence/` | screenshots / videos / logs / traces / artifacts (verified) |

## Status

COMPLETE (2026-10-19). Artifacts were committed and pushed incrementally
per Section 2 of the specification; final counts and statuses are in
`audit-metadata.json` and `executive-summary.md`. Headline: 473 declared
tests green, all 21 pages reachable, DEFECT-01 (sidebar hitbox compression
on the four shortest labels) fully characterized — see `defects.md`.
The final push of this branch was blocked mid-audit by the sandbox
janitor removing the git credentials (see `coverage-gaps.md` §6);
resolved post-audit: credentials were re-supplied, the branch was pushed,
then merged fast-forward into `main` and deleted per maintainer
instruction (single-branch policy).

## Post-audit remediation (2026-10-19, same day)

All three defects are resolved — see `defects.md` "Remediation" blocks,
`fix-report.md`, and the updated `control-inventory.csv` (C-APP-002):

- **DEFECT-01 (MAJOR) — FIXED:** sidebar rows are now hand-allocated
  full-width widgets (`nav_row`, `crates/app-shell/src/app.rs`), so the
  sensed rect is exactly the painted row. GUI retest `F1-fix-verify`:
  all four dead rows navigate at x=75 (diffs 1.62–3.87%) and Inbox also
  at the panel's far right x=130 (2.49%); no regression on control rows.
- **DEFECT-02 (MINOR) — INVALIDATED:** egui 0.37 single-line TextEdits
  surrender focus on Enter, so the audited "web-port" idiom works as
  written; a headless E2E test proves Enter confirms the selected hit
  against the original code. Audit record corrected; no code change.
- **DEFECT-03 (MINOR) — FIXED:** the palette now focuses the query field
  on open (`wants_focus` + `request_focus()`); headless E2E proof that
  keystrokes land with no click. The on-screen re-verification is
  BLOCKED-ENV (janitor severed fresh sessions' X sockets within ~8 s,
  four attempts F2–F5, forensic logs committed).

Workspace after remediation: 306 tests green (140 shell incl. the two new
headless palette E2E tests / 164 core / 2 xtask; 4 env-gated ignored),
clippy `-D warnings` clean, `cargo fmt --check` clean, AppImage rebuilt
from the fixed binary via `xtask package`.
