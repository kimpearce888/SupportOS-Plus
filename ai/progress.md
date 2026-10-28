# progress.md — active items

> Exactly ten active items at any time, per `rule.md` (batch size raised from five by the owner, 2026-10-23). Items move here from `plan.md` (Rule 2)
> and leave for `completed.md` only after verification (Rule 5).

## Active items — Batch RS-2 (Design-System Reskin, Stage 2: egui reskin R1–R3) — opened 2026-10-28

Scope: **Rust** (`crates/app-shell/` + workspace tooling) — pure egui reskin per decisions.md
D6/D1: presentation only, no feature/data/workflow/shortcut/page-set change, no HTML/CSS/JS
shipped. Design source of truth: Design System v1.0, frozen by Gate G1 (owner `continue`,
2026-10-28, after Batch RS-1 STOP; lab state `af5bc2a` = approved reference). Gate evidence
packs (G2 tokens, G3 Inbox pilot) are presented at the batch STOP; the owner may split this
batch at either gate on request.

**RS-11 — F0: Design Contract v1.0 freeze.** Extract `docs/audit/design-contract-v1.md`
from the conformed lab: token sheet (--ds-* values, both themes), 17 DS-C component
sheets (anatomy/variants/do-don't + egui mapping note each), 6 archetype sheets,
DS.TONE table, exception register, change-control rule (DC bump before code). Verify:
DC committed; every register A/B row closed in DC; RS-10 goldens referenced as the
visual baseline.

**RS-12 — R1: token port + style lint.** Rebase on latest origin/main; re-scan stray
hardcodes (raw `Color32::from_rgb` sites). `crates/app-shell/src/design.rs` LIGHT/DARK
token tables → DC v1.0 tokens 1:1; spacing/type/radius scales; DS.TONE → one match arm.
Style lint (CI test): forbid raw Color32 outside design.rs and off-scale spacing.
Verify: cargo fmt/clippy/test green; SPP_SMOKE per-page paint gate green; page
structure unchanged (colors/typography now DS — this IS the G2 token sign-off);
fresh goldens lock the token state against drift; G2 evidence pack (token diff table)
presented at batch STOP.

**RS-13 — R2a: structure widget set.** Add egui_kittest (pinned to the egui 0.37 line)
as dev-dependency + harness. Widgets 1:1 with `DC: DS-C-xx` doc-comments: PageHeader
(counter pill), DataTable (virtualized rows), StatTile/MetricTile, FilterBar, Tabs
(underline), Badge/Chip/StatusDot via DS.TONE, button set. Verify: kittest snapshots
per widget/state, light+dark; fmt/clippy/tests green.

**RS-14 — R2b: feedback widget set.** EmptyState (+slim variant), Callout set (five
fixed roles, B-1), Toast, armed ConfirmDialog, FormField/inputs, DetailPanel,
Avatar/Assignee. Verify: same harness + snapshots + DC mapping.

**RS-15 — R2c: inbox identity widget set.** Role-typed Message Card
(agent/customer/note/AI), day divider, quoted-history collapse, attachment chips,
Composer (Reply|Note, saved-replies, AI insert, draft-saved flag), AI badge set, typed
copilot cards. Verify: harness snapshots; 5-second AI-identification pre-check on the
isolated card.

**RS-16 — R3 pilot: Inbox rebuild.** Rebuild the Inbox UI strictly from the widget set
(list, reader, composer, copilot panel; `crates/app-shell/src/pages/inbox.rs`); zero
bespoke styling. Functional regression checklist: every existing interaction identical
(pagination past the 50-row cap, draft retention on row click, sync trigger, triage
loads). No new per-frame DB re-queries introduced by the rebuild (existing data-path
issues stay out of scope — data track, decisions.md D6). Verify: checklist recorded;
kittest goldens at 1440/1024/800 vs the RS-10 lab goldens; G3 evidence pack
(before/after + checklist) presented at batch STOP.

**RS-17 — R3 wave A: Dashboard, Customers, Organizations, LiveChats.**

**RS-18 — R3 wave B: Knowledge, Incidents, IssueRadar, Reports (+ Report Builder).**

**RS-19 — R3 wave C: Operations, AiCenter, Automation, CustomObjects, Outreach.**

**RS-20 — R3 wave D: Settings, Notifications, Search, SupportGraph, Connectors,
Diagnostics, SyncHealth, Onboarding, Shell.**

Each wave (RS-17..RS-20): rebuild from the widget set only; per-page functional
regression checklist; kittest goldens vs lab goldens; any egui-cannot-reproduce gap
becomes a registered exception in the DC — never silently approximated. Verify per
wave: checklist + goldens + fmt/clippy/tests + SPP_SMOKE green.

### Batch RS-2 status — checkpoint 2026-10-28 (RS-11..RS-15 verified; RS-16..RS-20 next)

- **RS-11 — F0 DC freeze:** `docs/audit/design-contract-v1.md` committed (f84d43c): token sheet both
  themes (exact hex), 17 DS-C sheets with egui mapping notes, 6 DS-A recipes, DS.TONE 25 rows,
  register A/B closed + C keeps + E-1, change-control rule, RS-10 goldens as visual baseline.
- **RS-12 — R1 token port:** design.rs LIGHT/DARK = DC tokens 1:1 (fbf106c): canvas #f4f4f5, ai/agent/
  warning-border/neutral-fg/on-ai/shadow/scrim added, hyperlink+charts → AI violet, dark faint #0c0c0e;
  type roles 22/13.5/13/12.5/12 wired into install(); DS.TONE = one 25-row match arm (BadgeTone::Ai,
  tone_colors_of shared core — inbox row-painter collapsed onto it); design::scale (space/type/radius +
  DC pads); **style lint CI test** (no raw Color32 outside design.rs, no off-scale spacing, B-3
  impossible); doctor colors + dialog/toast/palette shadows + scrim → tokens; 3px gaps normalized;
  DC pin tests. Contrast note: text-2 on canvas = DC-frozen 4.11:1 (4.7:1 on surface).
- **RS-13 — R2a structure set:** egui_kittest 0.37 dev-dep (kittest-ui feature, like ui-screenshot);
  page_counter (DS-C-01), status_dot/tone_solid (C-06), tabs/tab_groups underline (C-09),
  filter_bar one-primary-apply (C-10/A-2), metric_tile banded+drill (C-04), data_table virtualized
  pinned-header (C-05), metric_card→26px (C-03), ButtonVariant Text/Ai + manual DS-C-11 anatomy
  (fixes the real-shell icon/text overlap); 4 snapshots diff-locked; headless via Xvfb+lavapipe.
- **RS-14 — R2b feedback set:** empty_state_slim/cta (C-14/A-7), callout/status_line/quirk
  (C-15/B-1 roles), avatar/assignee (C-19), armed_danger_button with frame-guard vs auto-Id
  collision (C-16/A-5), detail_panel right-sheet + scrim + Esc (C-16/A-1), form_row (C-13);
  dashed strokes unavailable in egui → registered E-2; 8/8 snapshots diff-locked.
- **RS-15 — R2c inbox identity set:** message_card role-typed customer/agent/note/AI (C-20, G-2),
  day_divider, quoted_history, file_chip, composer (C-21 pure-UI act enum: send/saved-reply/AI-insert,
  Ctrl+Enter, draft-saved flag, amber note mode), cop_suggest/cop_similar/cop_chat (C-22); style-lint
  allowlist extended with DC-frozen sheet pads 5/7/11/13 (DC §2.4 note); 14/14 diff-locked incl. the
  5-second AI-identification pre-check + send/Ctrl+Enter interaction tests.

**Verification per item:** cargo fmt clean; clippy workspace -D warnings green (both feature configs);
cargo test: app-shell 233 (incl. DC pin tests) + core 485 + xtask 11 + style lint 2; kittest 14/14
(diff-locked twice: baseline write + clean diff); SPP_SMOKE exit 0 (fresh binary each time). Commits:
f84d43c, fbf106c, 8bb994f, b5e613d, 9718cd7 — all pushed to origin/main.

**Next in batch:** RS-16 (Inbox pilot rebuild from the widget set + regression checklist + G3 goldens),
then RS-17..RS-20 waves, then the G2/G3 evidence packs at the batch STOP.

**RS-16 — R3 pilot, Inbox rebuild (verified 2026-10-28):** `pages/inbox.rs` rebuilt on the
widget set (4004 lines): views rail via `design::view_item` (DS-A-05, counts beside the view,
A-6), list rows + reader via `message_card`/`message_card_with` (DS-C-20, rich-body closure —
`render_rich_spans` plain spans stay at HEAD sizing), composer/copilot panels unchanged in
behavior; a11y pass — the assign ComboBox now carries a programmatic label
(`widget_info` "assign: …", repo pattern) so kittest's accessibility check holds ON for the
real-page suite. Regression checklist all green in the 233: pagination past the 50-row cap
(page_one_reaches_conversations_past_the_cap, offset_compiles_from_the_page_index,
a_page_past_the_end_falls_back_to_the_last_page), draft retention on row click
(row_click_stashes_and_restores_the_typed_draft, reselecting_the_same_row_keeps_the_typing_
untouched), sync trigger (submit_reply_creates_durable_job_and_blocks_duplicates, r4_d8/r4_d9),
triage loads (ops_tile_filter_drills…, change_status_writes_locally_and_queues_remote_job,
invalidation_refreshes_the_filter_refs); no new per-frame DB re-queries (lazy empty-gate
reloads + event invalidation, same data path as HEAD). Zero bespoke styling — style lint 2/2
over the rebuilt file. **Page goldens:** new tests/kittest_inbox_page.rs renders the REAL
InboxPage (seeded in-memory DB, fixed timestamps for determinism) at 1440/1024/800 × 900,
light+dark = 6 goldens, diff-locked twice; lab reference for the G3 side-by-side is
mockups/design-lab/goldens/v4/{light,dark}/inbox-*.png (egui render ≠ HTML pixels by design;
side-by-side review = G3 evidence pack at batch STOP). Verification: fmt clean; clippy -D
warnings green (app-shell all-targets + core sandbox); cargo test: 233 + 485 + 11 + lint 2
+ core sandbox 493, workspace exit 0; kittest 20/20 (14 widgets + 6 page) with a11y check ON;
SPP_SMOKE exit 0. **CI note:** pushes since RS-13 (8bb994f..d172cfc) failed in CI — the
regenerated Cargo.lock (egui_kittest dev-dep tree) had never been committed; fixed by 8490eee,
both CI jobs fully green on it. **Next in batch:** RS-17 wave A (Dashboard, Customers,
Organizations, LiveChats).
