# progress.md — active items

> Exactly ten active items at any time, per `rule.md` (batch size raised from five by the owner, 2026-10-23). Items move here from `plan.md` (Rule 2)
> and leave for `completed.md` only after verification (Rule 5).

## Active items

**Batch RS-3 — opened 2026-10-29** on the owner's `continue` after the Batch
RS-2 STOP (gate receipt recorded in `completed.md`).

---

**RS-21 — R4: final acceptance + release.** Traceability matrix (screen region →
DS-C id → DC row); blind test (5-second AI reply identification); SUS
questionnaire, target ≥68; CI snapshot locks every rebuilt page against drift;
v1.9.0 tag + release notes; acceptance records folded into the printable report
series (the deferred PDF task). **GATE G4 — owner final acceptance.**

Execution notes (opened 2026-10-29):

- Traceability matrix → `docs/audit/acceptance/traceability-matrix.md`, backed by
  a machine gate (`scripts/gate_traceability.py`, wired into CI) that validates
  every DS-C/DS-A id against `docs/audit/design-contract-v1.md` and every page
  module under `crates/app-shell/src/pages/` against the matrix.
- Blind test + SUS → instruments and materials under `docs/audit/acceptance/`;
  the human-scored administrations are executed at Gate G4 (the DC's DS-C-22
  5-second card pre-check is already enforced by the typed widget set).
- CI snapshot locks → a dedicated `snapshots` job running the 23 kittest suites
  (152 tests) with the goldens diff-locked on the runner (lavapipe software
  Vulkan — the rasterizer family the baselines were produced with).
- Release → version 1.9.0 (workspace bump + CHANGELOG section; the release
  workflow composes the GitHub release notes from that section), tag `v1.9.0`.
- Acceptance records → folded into the printable report series as
  `docs/audit/SupportOS-Plus-Reskin-Acceptance-Report.pdf` via a
  `generate_acceptance_report.py` in the series' established style.
