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
