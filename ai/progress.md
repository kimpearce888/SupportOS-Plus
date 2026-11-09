## Active items

(none — Batch AUD-P1-1 (fix the P1 backlog — the eight dead-end/workflow
items of the repo audit §5) closed 2026-11-10, record in
`completed.md`; Rule 8 STOP, the queue is empty, new work opens only
on an owner directive. The audit's P2 backlog (§5 items 15–18) remains
NOT queued.)

Batch AUD-1 (the full product audit — gaps, bugs, blockers, features,
workflows, options questioned on practical scenarios; report only, no
code changes) opened on the owner's audit directive 2026-11-09 and
closed the same session: the deliverable `docs/audit/repo-audit-v1.9.6.md`
(what is built and why, 9 practical-scenario walkthroughs, 44 verified
findings in 5 classes, P0/P1/P2 remediation backlog — not queued), with
the verification battery re-run on the audited HEAD as evidence (gates,
fmt, verify-icons, clippy ×2, tests 464/472/251/2/14, cargo-deny, CI
run 42542458251 green 3/3). Record in `completed.md`.

Batch AIE-1 (items AIE-1-1..AIE-1-4: the AI reads the help center and
the customers' own words, and reply drafting is triggerable from the
Inbox) moved through this file per Rules 2 and 4 on 2026-11-09 — opened
on the owner's AI-pipeline directive, implemented (help center articles
in the reply-draft evidence + their own citation id-space + the leakage
auditor corpus; the customer's question in article citations + prompt;
the "Draft reply with LM Studio" trigger in the copilot pane; the
AIE-1-4 verification-surfaced customers-golden clock pin), verified
(gates, fmt, clippy ×2 lanes + the kittest lane for the touched target,
tests 464+472+251, cargo-deny, kittest diff-locked on all 24 suites +
style lint with 7 intended golden changes + 1 new golden and a stash
re-run proving the customers drift pre-existing, SPP_SMOKE, VLM
read-backs), and closed the same session (record in `completed.md`).

Batch UXH-1 (item UXH-1-1: the help center article reader must render
the synced Help Scout HTML body as a readable article, not the raw
source) moved through this file per Rules 2 and 4 on 2026-11-09 —
opened on the owner's screenshot report ("it should show help center
articles in a correct way, it is showing them with html syntax"),
implemented in `crates/app-shell/src/pages/knowledge.rs` only
(`article_body_ui`/`article_blocks`/`article_block_ui` on the
`spp_core::richtext::html_to_spans` pipeline, the reader riding
`READER_MEASURE`), verified (gates, fmt, clippy ×2 lanes, tests
251+459+467+14+2, cargo-deny, kittest diff-locked ×2 on all 24 suites +
style lint with zero drift on the 189 existing goldens + 3 new reader
goldens, SPP_SMOKE, VLM read-back), and closed the same session (record
in `completed.md`).

Batch UXG-1 (all-display design discipline + cross-page consistency
sweep, items UXG-1-1..UXG-1-10) moved through this file per Rules 2 and
4 on 2026-11-08 — opened on the owner's all-display directive ("work on
all size of monitors i use a 2k one and it should also work on 4k or 8k,
and all the pages should have been designed with same design principle,
all the pages, all the items"), implemented (CONTENT_MAX 1800 +
content_column in the shell funnel; READER_MEASURE 860 for the inbox
reader/composer; the 17-table data_table sweep with the legacy helpers
deleted; customers relative-time; incidents count dedup; the chrome
header page label; 2560/3840 goldens + 8K pixel probes; two
verification-surfaced fixes — the reports SLA clock pin and the chart
label-band reservation), verified (gates, fmt, clippy ×3 lanes, tests
719+467-sandbox, kittest diff-locked ×2 on 447 tests with 61 regenerated
+ 8 new goldens, SPP_SMOKE 24 pages · 4348 shapes, pixel probes +
VLM read-backs), and closed the same session (record in `completed.md`).

Batch UXF-1 (AI Center layout repair + golden-eval functional fix, items
UXF-1-1..UXF-1-8 + the owner-correction item UXF-2-1) moved through this
file per Rules 2 and 4 on 2026-11-08 — opened on the owner's "Fix these
issues" directive (two AI Center screenshots), implemented (Jobs/Usage/
Health/Analytics/Attributes/Copilot width discipline via
`card_titled_wide` + `data_table`; the contained golden-run blocks; the
UXF-2-1 LM Studio json_schema wire fix with its regression lock), and —
driven by the UXF-1-6 read-backs — two shared-system root-cause fixes
(the badge/chip phantom Grid cell; the data_table CELL_PAD), verified
(gates, fmt, clippy ×2, tests 459+246+11+2 / 467-sandbox, kittest
diff-locked ×2 on 435 tests with 65 regenerated + 4 new goldens,
SPP_SMOKE, pixel probes + VLM read-backs), and closed the same session
(record in `completed.md`).

Batch UXD-1 (dashboard layout repair, items UXD-1-1..UXD-1-3:
design-system width discipline, dashboard restructure per the 2024–25
helpdesk-dashboard pattern, full verification battery) opened on the owner's
screenshot report — moved through this file per Rules 2 and 4, was
implemented, verified (gates, fmt, clippy ×2, tests 458+246+2+11+466, kittest
diff-locked ×2 on 173 goldens incl. two new 1920 dashboard locks, pixel-probe
bounds proof, SPP_SMOKE 24 pages · 4332 shapes), and closed the same session
(record in `completed.md`). The engagement sits at a Rule 8 STOP; new work
opens only on an owner directive.

Batch REL-4 (items REL-4-1..REL-4-4) is CLOSED (moved through this file
per Rules 2 and 4, implemented, verified — full local battery + CI run
42359689689 green + Release run 42361423228 green 6/6 + live post-publish
verification — record in `completed.md`). Release v1.9.5 is published at
tag `v1.9.5` (target `9afef58`).

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
