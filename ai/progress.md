# progress.md — active items

> Exactly ten active items at any time, per `rule.md` (batch size raised from
> five by the owner, 2026-10-23). Items move here from `plan.md` (Rule 2) and
> leave for `completed.md` only after verification (Rule 5).

## Active items

**None.** No batch is active and none is held (authoritative, 2026-11-07,
post-Batch UXB-T1). The test-infra batch — UXB-T1, the kittest golden
date-tick finding disclosed during UXB-1's verification, pulled from
`ux-audit/reports/issues.md` on the owner's "handle UXB-T1" directive —
moved through this file per Rules 2 and 4, was implemented, verified, and
closed the same session (record in `completed.md`; status updated in
`ux-audit/reports/issues.md`). With it the last open finding of the
UX-audit program is closed. The engagement sits at a Rule 8 STOP; new
work opens only on an owner directive.

Batch UXB-T1 (item UXB-T1) moved through this file per Rules 2 and 4 on
2026-11-07 — opened on the owner's "handle UXB-T1" directive ~1h after
the UXB-1 push (fourth sandbox recycle: repo re-cloned at 7178153, Rust
1.99.1 + lavapipe/xkb re-extracted to the persistent home prefix first),
implemented with the batch scope on the shot-fixture clock (core
`business_hours` pin behind the `kittest-ui` feature gate + the three
wall-clock-dependent kittest fixtures + the new clock-pin suite), verified
(fmt/clippy incl. both feature lanes/tests 458+246+466-sandbox+11/kittest
diff-locked ×2 with ZERO golden churn/clock suite 4/4 in both thread
modes/SPP_SMOKE 24 pages · 4338 shapes — identical to the UXB-1 baseline),
completed the same session; record in `completed.md`.

Batch UXB-1 (items UX-26B/UX-34/UX-35) moved through this file per Rules 2
and 4 on 2026-11-07 — opened on the owner's "resolve ignored items"
directive after the UXA-4 program close (third sandbox recycle: repo
re-cloned at 771ed80, Rust 1.99.1 + lavapipe/xkb re-extracted to the
persistent home prefix first), implemented with the batch scope extended
to `crates/core/src/` by owner directive, verified (fmt/clippy/tests
458+246+466-sandbox/kittest diff-locked ×2 on regenerated baselines/
SPP_SMOKE 24 pages · 4338 shapes + 3 luma-verified 1920×1080 after-shots
with GREEN VLM read-backs), completed the same session; record in
`completed.md`.

Batch UXA-4 (items UX-31..UX-33: the final three fixes of the
owner-directed UX/UI audit program) moved through this file per Rules 2
and 4 on 2026-11-06 — opened on the owner's "continue" after the UXA-3
Rule 8 STOP (second sandbox recycle: repo re-cloned, toolchain and
lavapipe/xkb prefix rebuilt first), implemented, verified (fmt/clippy/
tests 245+420/kittest diff-locked ×2 on final code/SPP_SMOKE + 3
luma-verified after-screenshots with GREEN VLM read-backs), completed
the same session; record in `completed.md`. With it the UX-audit queue
is fully consumed (backend-blocked UX-26B/34/35 stay logged in
`ux-audit/reports/issues.md`).

Historical batches (UXA-4, UXA-3, UXA-2, UXA-1, GA-1/2, RW-1, DENY-1,
HYG-2, REL-3) moved through this file per Rules 2 and 4 and are recorded
in `completed.md` — narratives preserved below.

Batch UXA-3 (items UX-20..UX-30: the third ten fixes of the owner-directed
UX/UI audit program) moved through this file per Rules 2 and 4 on
2026-11-06 — opened on the owner's "continue" after the UXA-2 Rule 8 STOP,
implemented, verified (fmt/clippy/tests/kittest diff-locked ×3 on the final
code/SPP_SMOKE + 11 luma-verified after-screenshot evidence per item),
completed the same session; record in `completed.md`.

Batch UXA-2 (items UX-10..UX-19: the second ten fixes of the owner-directed
UX/UI audit program) moved through this file per Rules 2 and 4 on
2026-11-06 — opened on the owner's "continue" after the UXA-1 STOP,
implemented, verified (fmt/clippy/tests/kittest diff-locked/SPP_SMOKE +
after-screenshot evidence per item), completed the same session; record in
`completed.md`.

Batch UXA-1 (items UX-1..UX-9 + UX-25: the first ten fixes of the
owner-directed UX/UI audit program) moved through this file per Rules 2 and
4 on 2026-11-06 — opened on the owner's audit directive, implemented,
verified (fmt/clippy/tests/kittest diff-locked/SPP_SMOKE + before/after
screenshot evidence per item), completed the same session; record in
`completed.md`.

Batch GA-1 (item GA-1-1: decouple `api_queue` from `helpscout_real`) moved
through this file per Rules 2 and 4 on 2026-11-04 — opened on the owner's
"yes, continue" after the graphify audit, completed and verified the same
session; completion record in `completed.md`.

Batch GA-2 (item GA-2-1: neutral home for the shared JSON field helpers)
moved through this file the same way on 2026-11-04 — opened on the owner's
"continue", completed and verified the same session; record in
`completed.md`.

Batch RW-1 (item RW-1-1: connectors must not block the async runtime) moved
through this file the same way on 2026-11-05 — opened on the owner's
"continue" after the repowise audit, completed and verified the same
session; record in `completed.md`.

Batch DENY-1 (items DENY-1-1..3: make the deny lane warning-free) moved
through this file the same way on 2026-11-05 — opened on the owner's
"continue" after the reviewdog verification battery, completed and verified
the same session; record in `completed.md`.

Batch HYG-2 (item HYG-2-1: CHANGELOG link-reference completeness) moved
through this file the same way on 2026-11-05 — opened on the owner's
"continue" after the post-release consistency sweep, completed and
verified the same session; record in `completed.md`.

Batch REL-3 (release v1.9.4 from the final source) moved through this file
the same way on 2026-11-05 — opened on the owner's Build-and-Release
directive, completed and verified the same session; record in
`completed.md`.

Historical note (REC-1): the "Batch RS-4 active" declaration that stood here
was stale fiction — RS-4 completed and was verified 2026-10-30 (completion
record in `completed.md`). The 23 batch transcripts that had accumulated in
this file from 2026-10-30 to 2026-11-03 (RS-4 through AU-1, all completed
and recorded in `completed.md`) were moved to `ai/archive-progress.md`
(Batch HYG-1, decision D13). This file holds active items only.
