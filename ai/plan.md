# SupportOs-Plus — TODO

> Live TODO queue for remaining work items, executed ten at a time under
> `rule.md` (batch size raised from five by the owner, 2026-10-23).
> Status flow: `plan.md` → `progress.md` → implementation → verification →
> `completed.md`. Planning context and decision ledger: `decisions.md`.

## Queue state

**Batch UXD-1 (dashboard layout repair) CONSUMED 2026-11-07: items
UXD-1-1..UXD-1-3 moved to `progress.md` per Rule 2, implemented, verified
(full battery, see `completed.md`), and closed the same session. The queue
is EMPTY again. Root cause for the record: the width-unbounded widgets
inside `egui::Grid` cells (see the UX-dashboard queue section below) plus
the 30px frame padding the old `metric_card_w` semantics never accounted
for. New work opens only on an owner directive.**

**UX-audit program — CONSUMED 2026-11-06/07: the final batch UXA-4 (items
UX-31..UX-33) moved to `progress.md` per Rule 2, implemented, verified,
and closed the same session (record in `completed.md`). Every
frontend-fixable finding is done (UXA-1: UX-1..UX-9 + UX-25, UXA-2:
UX-10..UX-19, UXA-3: UX-20..UX-30, UXA-4: UX-31..UX-33). The three
backend-blocked findings UX-26B/UX-34/UX-35 followed as batch UXB-1 on the
owner's "resolve ignored items" directive (2026-11-07, per the queue
section below); with that the ux-audit backlog is fully consumed and the
queue is EMPTY. New work opens only on an owner directive.**

Every prior program is consumed with completion records in `completed.md`
(v2-blueprint 1–15; clean-audit 1–165; RS-01..21 + RS-6; RF-1..11 + FINISH;
KB/AUDIT batches; REL-1/2/3; AU-1/AU-2; GA-1/GA-2; RW-1; DENY-1). The UX
program opened on the owner's UX-audit directive (session 2026-11-05):
discover every journey, audit every page/state at 1920×1080 (+dark theme),
fix frontend-only defects in ten-item batches per rule.md, evidence per
`ux-audit/reports/` (scope contract: `ux-audit/reports/scope.md`; issue
detail: `ux-audit/reports/issues.md`). Backend-blocked findings are logged
there, not queued here.

### UX-dashboard queue (EMPTY — Batch UXD-1 closed 2026-11-07)

Opened on the owner's dashboard screenshot report (cards enlarging / moving
right, cards out of screen, cards not well aligned; "fix the dashboard
design and make it modern, do a research how should it look and apply it,
it should not break on any screen size for desktop"). Diagnosis: the
width-unbounded widgets inside `egui::Grid` cells (see the queue-state
preamble) plus the 28px frame padding that `metric_card_w`'s callers never
accounted for (a 4-column KPI row overflows by exactly 4×28−30 ≈ 82px, so
the last column clips at every width). Fix scope:

- **UXD-1-1** — design-system width discipline: `metric_card_w` treats its
  width as the card's OUTER width (frame padding subtracted inside);
  `data_table` gets adaptive bounded columns (`min_col_width`/`max_col_width`
  pairs so the table can never exceed its card at any width, header strip
  laid out on the same column grid so headers align with columns); new
  `card_exact_w`/`card_titled_exact_w` helpers land cards on an exact outer
  width (+ optional exact min height for side panels).
- **UXD-1-2** — dashboard restructure per the 2024–25 helpdesk-dashboard
  pattern (Help Scout/Zendesk/Linear/Stripe/Vercel: KPI tiles → trend chart
  + insight side panel → equal-width breakdown grid → full-width tables):
  responsive KPI grid (2/3/4/6 columns by content width, unit folded into
  labels so every tile is two lines and rows align), 62/38 hero row (trend
  chart + Issue-radar panel stretched to the chart's height, stacking below
  1000px), breakdown cards on an equal-width 1/2/3-column grid with
  top-aligned rows, channel-metrics + mailbox-comparison tables full width,
  and the by-channel card's dev-facing `dashboard.rs:337-345` note reworded
  into product language.
- **UXD-1-3** — verification: fmt/clippy (both lanes)/tests (core, app-shell
  lib, style lint, xtask, sandbox), kittest golden regen for every
  design-helper-affected suite + two new wide (1920) dashboard goldens,
  diff-locked re-run ×2, SPP_SMOKE, and VLM alignment read-backs of the
  regenerated dashboard goldens at 800/1024/1440/1920.

### Release queue (EMPTY — Batch REL-4 closed 2026-11-07)

No queued items. The owner's Build-and-Release directive (2026-11-07, after
the UXB-T1 close) opened Batch REL-4 (items REL-4-1..REL-4-4: version bump
1.9.4 → 1.9.5, CHANGELOG `[1.9.5]`, annotated tag `v1.9.5`, publish +
verification); the items moved straight through Rule 2 to `progress.md`,
were implemented, verified (full local battery + CI run 42359689689 green
+ Release run 42361423228 green 6/6 + live post-publish verification), and
closed the same session (record in `completed.md`). Release **v1.9.5** is
published at tag `v1.9.5` (target `9afef58` = main tip) with the .deb +
AppImage, SHA-256 checksums, and per-asset link verification.

### UX-audit queue (EMPTY — test-infra batch UXB-T1 closed 2026-11-07)

No queued items. The owner's "resolve ignored items" directive (2026-11-07,
after the UXA-4 program close) pulled the three backend-blocked UX findings
out of `ux-audit/reports/issues.md` as batch UXB-1 and moved them straight
through Rule 2 to `progress.md` — implementation scope for that batch
extended to `crates/core/src/` by owner directive (the UXA scope contract's
frontend-only limit was lifted for exactly those three items). All three
are fixed and verified (record in `completed.md`; statuses updated in
`ux-audit/reports/issues.md`). The one test-infra finding that surfaced
during that batch's verification (UXB-T1: kittest goldens tick "Nd ago" at
every UTC midnight — fixed-stamp fixtures vs day-granularity relative
display) was logged-not-queued until the owner's "handle UXB-T1" directive
(2026-11-07, ~1h after the UXB-1 push) pulled it back out as its own
single-item batch, moved through Rule 2 to `progress.md`, and closed the
same session (record in `completed.md`; status updated in
`ux-audit/reports/issues.md`). With it the last open finding of the
UX-audit program is closed; the queue is empty.

Prior programs (v2-blueprint 1–15; clean-audit 1–165; RS-01..21 + RS-6;
RF-1..11 + FINISH; KB/AUDIT; REL-1..4; AU-1/AU-2; GA-1/GA-2; RW-1; DENY-1;
UXA-1..4 + UXB-1 + UXB-T1) are fully consumed with completion records in `completed.md`.

## Notes

- Historical batch narratives and the original program openers (the v1.7.0
  audit queue, the RS/RF/KB/REL/AU openers, scope contracts) live in
  `ai/archive-plan.md` (moved 2026-11-04, HYG-1); completed-batch item
  transcripts live in `ai/archive-progress.md`. Completion records + full
  verification evidence: `completed.md`. Decisions: `decisions.md` (D1–D13).
- New work opens only on an owner directive: it is written into this file as
  numbered items, moved ten at a time to `progress.md` (Rule 2), and leaves
  for `completed.md` only after verification (Rules 4–5).
- Batch UXB-T1 (item UXB-T1: deterministic clock for the kittest shot
  fixtures) opened here 2026-11-07 on the owner's "handle UXB-T1" — the
  test-infra finding disclosed during UXB-1's verification: the sync-health
  goldens (and, on the same clock, the operations goldens' frozen
  `generated_at` and the notifications goldens' `iso_days_ago` seeds)
  re-render "Nd ago"/"Yesterday" one day further at every UTC midnight, so
  any CI run on a later calendar day than the last regen fails the drift
  gate. Fix scope: a feature-gated pinned `now` in core
  `business_hours` (the single clock every `relative_time` render and the
  notification seed helper read), pinned by the affected fixtures to a
  fixed anchor on the UXB-1 regen day (2026-11-07T12:00:00.000Z — day-
  granular rendering identical to the current baselines, so no golden
  churn), plus a clock-pin suite proving the mechanism. Moved to
  `progress.md` per Rule 2, completed and verified the same session, left
  for `completed.md` at the Rule 4 sync; queue empty again.
- Batch GA-1 (item GA-1-1: decouple `api_queue` from `helpscout_real`) was
  opened here 2026-11-04 on the owner's "yes, continue" after the graphify
  knowledge-graph audit surfaced the `api_queue.rs <-> helpscout_real/mod.rs`
  module cycle, moved to `progress.md` per Rule 2, completed and verified the
  same session, and left for `completed.md` at the Rule 4 sync. The queue is
  empty again; the graphify outputs live outside the repo (`/tmp/spp-graph/`).
- Batch GA-2 (item GA-2-1: give the shared JSON field helpers a neutral home)
  opened here 2026-11-04 on the owner's "continue" — the graphify audit's
  remaining candidate: `str_field`/`i64_field`/`opt_str_field` were defined
  inside the `pages/queries.rs` page module while 9+ sibling pages consume
  them through the `pages` re-export (mod.rs:352). Moved to
  `progress.md` per Rule 2, completed and verified the same session, left for
  `completed.md` at the Rule 4 sync; queue empty again.
- Batch RW-1 (item RW-1-1: connectors must not block the async runtime)
  opened here 2026-11-05 on the owner's "continue" after the repowise
  (repowise-dev/repowise v0.60.1) code-health audit of 3845e10 surfaced the
  one real defect: `fetch_rows` (async, awaited from the background worker
  loop at workers/mod.rs:1130) made blocking `std::fs` calls (local_json +
  csv branches) and synchronous rusqlite calls (sqlite branch) on the shared
  tokio runtime. Moved to `progress.md` per Rule 2, completed and verified
  the same session, left for `completed.md` at the Rule 4 sync; queue empty
  again. The repowise index stays outside the repo (`<worktree>/.repowise/`,
  untracked).
- Batch DENY-1 (items DENY-1-1..3: make the deny lane warning-free) opened
  here 2026-11-05 on the owner's "continue" after the reviewdog verification
  battery (reviewdog v0.21.2 check of 1c50e4f — tauri-driver inapplicable
  since the Phase 4 cutover, so the battery was a full local CI-lane re-run
  plus SPP_SMOKE) surfaced three cargo-deny warnings: yanked
  async-executor 1.14.0, a dead `bitflags` bans skip, and an unmatched
  `CDLA-Permissive-2.0` license allowance. Moved to `progress.md` per
  Rule 2, completed and verified the same session, left for `completed.md`
  at the Rule 4 sync; queue empty again.
- Batch REL-3 (release v1.9.4 from the final source) opened here 2026-11-05
  on the owner's Build-and-Release directive: workspace 1.9.3 → 1.9.4
  (Cargo.toml + Cargo.lock, the four member entries; the recycled sandbox
  has no Rust toolchain and ~1.7 GB free disk, so the bump was applied and
  verified mechanically — tomllib re-parse: exactly the four member package
  versions changed, 620 packages, zero stray 1.9.3 — and the Release
  workflow's `--locked` runs revalidate the lock against the manifest on
  the tagged content before packaging), CHANGELOG [1.9.4] in product
  language (RW-1: local connector reads moved off the async runtime;
  DENY-1: supply-chain gate warning-free), the release.yml extraction
  commands simulated locally (version awk → 1.9.4; changelog section awk →
  the full [1.9.4] section). Moved to `progress.md` per Rule 2; the
  annotated tag v1.9.4 triggers release.yml (gates + package + 3-distro
  .deb verification incl. APT dependency resolution / upgrade / uninstall
  + AppImage verification + publish-after-green with per-asset link and
  checksum re-verification); completed and verified the same session,
  record left for `completed.md` at the Rule 4 sync.
- Batch HYG-2 (item HYG-2-1: CHANGELOG link-reference completeness) opened
  here 2026-11-05 on the owner's "continue" (queue empty post-REL-3, so the
  continue ran an honest post-release consistency sweep, read-only): the
  sweep found the CHANGELOG header/tag/ref triangle intact everywhere else
  (zero stale 1.9.3 strings outside ledger history; README install section
  version-parameterized; 0 open issues; last 8 workflow runs green; all 23
  version headers have matching origin tags), but only 3 of 23 version
  headers carry a Keep-a-Changelog link reference ([1.1.0], [1.0.0],
  [0.2.0]) — 20 refs missing, every one backed by an existing tag, zero
  404 risk. Fix: add the 20 refs in descending order above [1.1.0], then
  mechanical verification (23/23 header→ref parity, every URL live-checked
  HTTP 200). Moved to `progress.md` per Rule 2, completed and verified the
  same session, record left for `completed.md` at the Rule 4 sync.
