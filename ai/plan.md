# SupportOs-Plus — TODO

> Live TODO queue for remaining work items, executed ten at a time under
> `rule.md` (batch size raised from five by the owner, 2026-10-23).
> Status flow: `plan.md` → `progress.md` → implementation → verification →
> `completed.md`. Planning context and decision ledger: `decisions.md`.

## Queue state

**Batch REL-5 (release v1.9.6 from the final source) OPENED 2026-11-08
on the owner's standing Build-and-Release directive ("continue" after
the CI-1 close): v1.9.5 was cut at `9afef58`, but main has since landed
five owner-directed design batches and the packaging hardening — UXD-1
(dashboard layout repair), UXE-1 (full-app design modernization), UXE-2
(post-recycle verify + the note-card avatar fix), UXF-1 + UXF-2-1 (AI
Center layout repair + the LM Studio json_schema wire fix), UXG-1 (the
all-display design discipline: CONTENT_MAX 1800 centered column,
READER_MEASURE 860, the 17-table data_table sweep, 2560/3840 goldens +
8K probes — the owner's "work on all size of monitors… 2k… 4k or 8k…
same design principle, all the pages, all the items" mandate), and CI-1
(retry-hardened pinned downloads incl. the type-2 runtime via
--runtime-file). None of it is in any published release. Items:**

- **REL-5-1** — workspace version 1.9.5 → 1.9.6 (root Cargo.toml, the
  single source of truth — members are `version.workspace = true`; +
  the four member entries in Cargo.lock).
- **REL-5-2** — CHANGELOG `[1.9.6]` section in product language + the
  `[1.9.6]` link reference (five fixed groups: dashboard + full-app
  modernization; AI Center layout repair; the LM Studio json_schema
  fix; the all-display width discipline for 2K/4K/8K + one design
  principle across all pages; note-card avatar fix) + internal note
  for the packaging-lane hardening.
- **REL-5-3** — annotated tag `v1.9.6` on the release commit (only
  after CI green on the bump push).
- **REL-5-4** — publish via release.yml (gates + package + 3-distro
  .deb verification + AppImage verification + publish-after-green) +
  live post-publish verification of every asset.

**Batch CI-1 (packaging lane must survive transient CDN failures)
CONSUMED 2026-11-08: items CI-1-1 and CI-1-2 moved to `progress.md` per
Rule 2, implemented, verified, and closed the same session; record in
`completed.md`. CI run 42473244597 on the batch push `de2c13c` green
3/3 — the previously failing AppImage job passed with both downloads
(appimagetool + type-2 runtime) going through the pinned retry-hardened
path and the pack succeeding via --runtime-file. The batch reopened once mid-flight: the verification CI
push (run 42472196649 on ca95343) proved CI-1-1 works — the download
survived three consecutive transient failures (503, transport error,
504) and succeeded on attempt 4 — but then surfaced the second
single-shot download in the lane: appimagetool's own type-2 runtime
fetch, which cannot be retried from outside the tool. CI-1-2 therefore
pins the runtime (frozen type2-runtime tag 20251108, sha256 per arch,
retry-hardened, cached under target/tools) and passes it via
--runtime-file so appimagetool never touches the network. The queue is
EMPTY again. New work opens only on an owner directive.**

**Batch UXG-1 (all-display design discipline + cross-page consistency
sweep) CONSUMED 2026-11-08: items UXG-1-1..UXG-1-10 moved to
`progress.md` per Rule 2, implemented, verified (full battery incl. two
verification-surfaced fixes — the reports SLA clock pin and the chart
label-band reservation — with 61 goldens regenerated + 8 new locks; see
`completed.md`), and closed the same session. The queue is EMPTY again.
New work opens only on an owner directive.** The batch consumed the four
deferred queue candidates (UXF-1-4 chrome header balance, UXF-1-5 table
width-defect sweep, customers "Last Contact" raw-ISO, incidents header
count duplication) plus the new all-display width discipline
(CONTENT_MAX 1800 centered column through the shell funnel; READER_
MEASURE 860 for the inbox reader/composer; 2560/3840 goldens + 8K
pixel probes).

**Deferred candidates (queue): NONE remaining — UXF-1-4 (chrome header
balance), UXF-1-5 (table width-defect class sweep — the remaining
natural-width `design::table`/`table_stretched` page-level lists on
reports/knowledge/operations/diagnostics/automation/connectors/sync_
health; the UXD-1-disclosed known-remaining right-edge overflow on
Reports 800/1024 and the detail goldens), customers "Last Contact"
raw-ISO timestamps ellipsizing in their 140px column, and the incidents
page header showing the incident count twice (subtitle + page_counter)
were all CONSUMED by Batch UXG-1 above on the owner's all-display
directive (2026-11-08).**

### UX-fix queue (EMPTY — Batch UXG-1 closed 2026-11-08)

Opened on the owner's all-display directive ("work on all size of
monitors i use a 2k one and it should also work on 4k or 8k, and all the
pages should have been designed with same design principle, all the
pages, all the items"); all ten items moved to `progress.md` per Rule 2,
implemented, verified, and closed the same session (record in
`completed.md`). Diagnosis: the non-inbox pages render at the
full window width with no content-column cap — on a 2560-wide 2K display
the content column is ~2300px, at 4K (3840 logical) ~3600px, at 8K
(7680 logical) ~7400px: every card, table, chart and form stretches to
absurd widths (the "broken on my monitor" class), and the inbox reader
thread stretches the same way past ~1900px of pane width. Fix scope —
one width discipline and one consistency pass over all 24 pages, inside
the existing design system:

- **UXG-1-1** — One content-column width discipline (the 2K/4K/8K fix):
  a `design::scale::CONTENT_MAX` token (1800 — fits the dashboard's
  6-column KPI tier and 7–9 column tables at professional column
  widths) + a `design::content_column(ui, …)` helper (full width up to
  the cap; a centered exact-width column above it, via an
  exact-`max_rect` child scope). The shell's non-inbox page funnel wraps
  every standard page in it — one principle, every page, from 720 to
  7680 logical px; below the cap (windows ≤ ~2060px) the layout is
  pixel-identical to today.
- **UXG-1-2** — Inbox reader reading measure: the reader pane's content
  (head, action bar, thread, triage/QA cards) and the composer panel cap
  at a `READER_MEASURE` (860px) reading column, centered in the pane —
  the Gmail/Help Scout wide-window register; unchanged below the cap
  (every existing ≤1440 golden state), capped at 1920+ (the docked-1920
  golden regenerates).
- **UXG-1-3** — Table width-defect sweep A (reports): the five remaining
  natural-width `design::table` lists on the reports page migrate to
  `design::data_table` (header strip, exact integer columns, gated 90px
  floor) so no reports list can overflow or hug at any width — closes
  the UXD-1-disclosed known-remaining Reports right-edge overflow.
- **UXG-1-4** — Table width-defect sweep B: knowledge ×3, operations ×1
  migrate to `design::data_table`.
- **UXG-1-5** — Table width-defect sweep C: diagnostics ×3, automation
  ×1, connectors ×1, sync_health ×2, customers detail ×1 migrate to
  `design::data_table` (the unconditional-90px-floor `table_stretched`
  callers included) — with this, no page-level list uses the legacy
  natural-width helpers anywhere.
- **UXG-1-6** — Customers "Last Contact" column: the raw-ISO
  `last_activity_at` stamps render through the shared `relative_time`
  helper ("3d ago", "Yesterday") — no more ellipsized
  `2026-02-10T08:00…` strings in the 140px column; deterministic under
  the kittest clock pin.
- **UXG-1-7** — Incidents page header count dedup: the subtitle and the
  `page_counter` pill both state the incident count today — one count
  register (the pill), the subtitle describes scope (and the
  "showing the first X of Y" truncation case) instead of repeating it.
- **UXG-1-8** — Chrome header balance (UXF-1-4): the 48px top bar
  carries only right-aligned controls today — the active page's label
  lands on the left (window-level context, the modern helpdesk
  breadcrumb register; same anatomy as `page_header`: left title +
  baseline-aligned right actions).
- **UXG-1-9** — All-display proof + locks: new kittest goldens at 2560
  (2K) and 3840 (4K logical) — dashboard + reports (a swept table page)
  through `content_column`, and the inbox reader at 2560 — plus a 7680
  (8K) pixel-probe test asserting the content column == CONTENT_MAX and
  the reader thread == READER_MEASURE exactly (no giant golden); every
  affected baseline regenerates (expected: reports/knowledge/operations/
  diagnostics/automation/connectors/sync_health/customers table goldens
  + the inbox 1920 docked golden).
- **UXG-1-10** — Full battery: fmt/clippy (both lanes)/tests (core,
  app-shell lib, style lint, xtask, sandbox)/kittest diff-locked ×2 on
  the regenerated baselines/SPP_SMOKE, VLM read-backs at 2560/3840 both
  themes, ledger close per Rules 4–5, push, CI green (Rule 8).

**Batch UXE-2 (post-recycle verify-and-fix) CONSUMED 2026-11-08: items
UXE-2-1..UXE-2-4 moved to `progress.md` per Rule 2, implemented,
verified (full battery, see `completed.md`), and closed the same session.
The queue is EMPTY again. New work opens only on an owner directive.**
The batch was opened on the owner's post-recycle directive ("you have to
fix not change. you have to fix what we have to make sure that is
working as expected, introducing new things are totally prohibited").
Outcome for the record: HEAD verified working as expected after the
fifth recycle (full battery + zero-drift golden locks); UXE-1's disclosed
"known-remaining" reader/composer misalignment DISPROVEN on HEAD (stale
pre-batch measurement — pixel-proven aligned at every width); the real
defect found and fixed instead was the invisible Note-card avatar
(one token pair, no new dependencies); one new overflow regression
golden added. Details in `completed.md`.

### UX-fix queue (EMPTY — Batch UXF-1 closed 2026-11-08)

Opened on the owner's "Fix these issues" directive (two fresh
screenshots: the AI Center Jobs tab — a narrow "Recent AI runs" card
hugging ~35% of the content column with the rest dead; the AI Center
Usage tab — sections as raw uncased text hugging the left, a 27-line
golden-set error dump, inconsistent empty states). All items moved to
`progress.md` per Rule 2, implemented, verified, and closed the same
session (record in `completed.md`); UXF-1-4 and UXF-1-5 were
scope-deferred by the owner's mid-batch correction and stay queue
candidates above:

- **UXF-1-1** — Jobs tab: the "Recent AI runs" card gets width
  discipline (full content-column width, the `card_titled_wide` register
  UX-7 established) and the natural-width `design::table` Grid migrates
  to `design::data_table` (exact integer columns, header strip,
  90px-floor-gated) so the table spans the card; kill the unbounded
  `show_rows` ScrollArea that leaves dead card space; the card header
  (icon + title + badge) lands on one aligned baseline.
- **UXF-1-2** — Usage tab: the three sections (Per-feature usage /
  Draft feedback / Golden-set evals) become full-width titled cards with
  one vertical rhythm (SP-scale, no off-scale gaps); the usage ledger
  Grid migrates to `data_table`; empty states use the shared
  `empty_state_slim` component (no bare hint_text sections); the
  golden-set runs render as contained run blocks — pass badge + backend
  + run meta on one row, per-case errors collapsed into a mono error
  list inside the block (no 27 raw lines floating on the page).
- **UXF-1-3** — Health / Analytics / Attributes / Copilot tabs: same
  width discipline (full-width cards) so all six tabs share one anatomy;
  the Attributes catalog nested-card list gets SP rhythm; the Analytics
  metric form aligns with the Jobs/Usage register.
- **UXF-1-4** — App chrome header (both screenshots flag it): the 48px
  top bar carries only right-aligned controls today — add the active
  page's label on the left (window-level context, the modern helpdesk
  breadcrumb register) so the bar reads balanced; no new components,
  label + tokens only. **SCOPE-DEFERRED 2026-11-08: the owner's
  correction ("it is not the design issue, read the failed statements
  there") redirected the batch to the functional failure (UXF-2-1,
  done+verified); this chrome item moves back to the queue below as a
  candidate on owner direction — adding unrequested chrome is not part
  of a fix mandate.**
- **UXF-1-5** — Table width-defect class sweep (the UXD-1-disclosed
  known-remaining): migrate the remaining natural-width `design::table`
  page-level lists (reports ×5, knowledge ×3, operations, diagnostics)
  and the `table_stretched` callers with the unconditional 90px floor
  (automation, connectors, customers detail, sync_health ×2) to
  `design::data_table` so no page-level list can overflow or hug at any
  width. **SCOPE-DEFERRED 2026-11-08 with UXF-1-4 (same owner
  correction — the functional fix took the batch; this sweep stays a
  queue candidate below).**
- **UXF-1-6** — Responsive proof: pixel-probe the changed goldens at
  800/1024/1440 (opaque bounds left=right within the content column),
  VLM read-backs of the AI Center tabs (Jobs seeded with rows, Usage
  with a failing golden run) at 1440 light + 800 light against the
  owner's acceptance ("professional and modern … works on every
  display").
- **UXF-1-7** — New golden locks: a seeded Jobs-tab golden (rows in the
  table, not the empty state) and a Usage-tab golden with a failing
  golden-set run (the error-block render) so both fixed states are
  diff-locked forever.
- **UXF-1-8** — Full battery: fmt/clippy (both lanes)/tests (core,
  app-shell lib, style lint, xtask, sandbox)/kittest diff-locked ×2 on
  regenerated baselines/SPP_SMOKE, ledger close per Rules 4–5, push, CI
  green (Rule 8).

**Batch UXE-1 (full-app design modernization) CONSUMED 2026-11-07/08:
items UXE-1-1..UXE-1-10 moved to `progress.md` per Rule 2, implemented,
verified (full battery, see `completed.md`), and closed the same session.
The queue is EMPTY again. New work opens only on an owner directive.**

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

### UX-fix queue (EMPTY — Batch UXE-2 closed 2026-11-08)

Opened on the owner's post-recycle directive ("you have to fix not
change. you have to fix what we have to make sure that is working as
expected, introducing new things are totally prohibited" — rejecting a
proposed tech-stack change; the fix stayed inside the existing Rust/egui
codebase and design system). Scope: prove the current HEAD (8a89ebd, the
UXE-1 design-modernized state with green CI run 42389418657) still works
as expected after the fifth sandbox recycle, then close the one
known-remaining defect UXE-1 disclosed. CONSUMED 2026-11-08: all four
items moved to `progress.md` per Rule 2, implemented, verified, and
closed the same session (record in `completed.md`):

- **UXE-2-1** — post-recycle HEAD verification: dev prefix rebuilt (Rust
  1.99.1 per rust-toolchain.toml, lavapipe + env at
  `~/.local/opt/vulkan-libs`), then the full battery unchanged on HEAD:
  fmt `--check`; clippy workspace `--all-targets` (known: the two
  pre-existing `kittest_list_pages.rs` warnings on the kittest lane
  only); clippy sandbox lane `-D warnings`; tests core/sandbox/app-shell
  lib/style lint/xtask; kittest diff-locked ×2 (no UPDATE_SNAPSHOTS —
  the committed baselines must hold); SPP_SMOKE.
- **UXE-2-2** — golden read-backs of the design-modernization states
  (inbox 800 folded-views / 1024 narrow-copilot / 1440 light + dark /
  1920 docked, widgets composer) against the UXE-1 acceptance criteria:
  no clipping/overlap, aligned cards, modern anatomy — with pixel probes
  as primary evidence where a reviewer read-back is ambiguous.
- **UXE-2-3** — fix the known-remaining reader/composer right-edge
  misalignment (UXE-1's disclosed follow-up candidate): the reader's
  scroll-hosted cards ride ~22px wider than the composer card at the
  right edge (reader card right ≈ x=791 vs composer ≈ x=769 at 800px;
  identical bounds in the pre-UXE-1 goldens — an egui ScrollArea
  scrollbar-gutter behavior, nothing clips the window). Fix inside the
  existing design system (no new components/deps): the reader scroll
  content and the composer card must land on the same right edge;
  regenerate the affected goldens.
- **UXE-2-4** — post-fix battery: fmt/clippy (both lanes)/tests (core,
  app-shell lib, style lint, xtask, sandbox), kittest diff-locked ×2 on
  the regenerated baselines, SPP_SMOKE, ledger close per Rules 4–5,
  push, CI green (Rule 8).

### UX-design queue (EMPTY — Batch UXE-1 closed 2026-11-07/08)

Opened on the owner's full-app design directive (screenshot report of the
v1.3.5-era build + "check the sidebar, the reply editor at the bottom,
basically most of the layouts are not correctly designed, gap, padding,
label alignments too many issues… should not look like different pages are
designed differently… make it professional and modern, it is too much old
school like an ancient build… should work fine on every monitor/display").
VLM audits of the owner screenshot + the current goldens (inbox/copilot/
reports/settings at 1440 light) converge on: mixed casing and label
conventions per page, ad-hoc spacing values (2/5/6/7/9/10/12/14/18/22)
with no rhythm, three different page-header patterns, native egui
ComboBox/checkbox controls that read Win95-era, saturated full-fill
callouts, a Copilot panel that clips on narrow windows, fixed-width inbox
columns (views 172 + list 330..640 + copilot 320) that break on
1024-class displays, and no window minimum size. Fix scope — one design
language across all 24 pages (Linear/Stripe/Zendesk-style helpdesk
pattern), responsive from 800 to 2560:

- **UXE-1-1** — token & card modernization: borderless elevated cards in
  light mode (hairline kept for dark), one radius scale (card 10 / control
  8 / badge 6 / chip pill), spacing-scale discipline replacing ad-hoc
  `add_space` values, one page-header rhythm (`page_header` on every page:
  title + subtitle + baseline-aligned right actions + consistent gap), one
  section-title/card-header anatomy, card padding diet.
- **UXE-1-2** — shell chrome: sidebar (brand row with inline version,
  search trigger on the nav grid, consistent group labels, 30px nav rows,
  active = filled selection, theme row pinned without orphan separator)
  and top bar (48px, aligned cluster, even gaps, one bottom hairline).
- **UXE-1-3** — modern control set: styled `select` dropdown (replaces
  every native egui ComboBox), `switch` for boolean settings, `segmented`
  control for ranges/statuses, uniform input heights/padding, Title Case
  button labels everywhere ("apply" → "Apply", "‹ Prev" → "Prev").
- **UXE-1-4** — responsive window & inbox architecture: window minimum
  size, width-aware inbox columns (views rail folds into the list header
  below the fold width; proportional list clamp; copilot docks as a
  right-side overlay when the reader would fall under its floor), composer
  scroll (UX-27) preserved; verified 800–2560.
- **UXE-1-5** — inbox views rail + conversation list redesign: consistent
  micro-header, view rows with count pills, uniform search row, compact
  filter chips, one list-row anatomy (name+time / subject / assignee+
  status) with hover and selected states on a fixed rhythm.
- **UXE-1-6** — inbox reader redesign: reader head (subject + meta +
  actions), action bar (status segmented + assign select), message cards
  with an accent left bar for note/AI roles instead of full saturated
  fills, aligned sender rows, day dividers.
- **UXE-1-7** — composer redesign: card frame, Reply/Note segmented +
  tool buttons header, compact recipient rows (To/Cc/Bcc), bounded body,
  footer row (context hint + primary Send), aligned to the reader width,
  never clipped.
- **UXE-1-8** — copilot panel redesign: header row with close, system card
  styling (AI accent on chips/actions only), aligned bullets, suggested
  reply with an Insert action, internal scrolling, width clamp, no
  horizontal clipping at any width.
- **UXE-1-9** — cross-page consistency & copy pass: reports (single tab
  row replacing the floating-label tab groups, KPI alignment, dead space),
  settings (aligned form grid, switches, input column), global casing
  sweep (Title Case buttons, sentence-case labels, no bare `label:`
  colons, no "(s)" dev strings, no leaked placeholders), every page
  through the same header/tabs/card components.
- **UXE-1-10** — verification: fmt, clippy (both lanes), tests (core,
  app-shell lib, style lint, xtask, sandbox), kittest golden regen for
  every affected suite (expected near-total churn) + new multi-width
  inbox goldens covering the folded-views and overlay-copilot states,
  diff-locked ×2, SPP_SMOKE, VLM read-backs at 800/1024/1440/1920 (+2560
  inbox) in both themes, pixel bounds probes on the responsive states.

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
