# progress.md — active items

> Exactly ten active items at any time, per `rule.md` (batch size raised from five by the owner, 2026-10-23). Items move here from `plan.md` (Rule 2)
> and leave for `completed.md` only after verification (Rule 5).

## Active items — Batch RS-1 (Design-System Reskin, Stage 1: design-lab conformance) — opened 2026-10-28

Scope: touches **only `mockups/design-lab/`** — zero Rust files. Design source of truth:
Design System v1.0 (commit f6c88b3: `ds.css`, `ds.js`, `ds.html`, `mismatch-register.md`).
Scope contract: decisions.md D6 (presentation only). The batch closes at **Gate G1**
(owner reviews the conformed lab; rule.md STOP).

**RS-01 — DS archetype render engine (foundation).** In `ds.js`, add the six archetype
page renderers (DS-A-01 list+detail workspace, DS-A-02 dashboard, DS-A-03 detail record,
DS-A-04 settings, DS-A-05 inbox workspace, DS-A-06 console) plus a page registry, so a
page = a PAGE_DATA object + one archetype call. Evidence: 17 bespoke per-page renderers
in `pages-reskin.js` (the a8492d0 build) are the composition drift the register logs
(mismatch-register.md Class A). Verify: `node --check`; every registry page resolves to
exactly one archetype; no page markup rendered outside `DS.*` calls.

**RS-02 — Inbox onto DS-A-05 (lab pilot).** Rebuild inbox as data + archetype: view
rail, list rows, DetailPanel reader (DS-C-17), role-typed message cards + day dividers +
quoted-history collapse + attachment chips, composer, typed copilot cards, sync
status-line (register A-8). Verify: golden-path interaction suite unchanged (open
conversation, reply/note send, saved-reply insert, AI insert, copilot actions); visual
equals the approved proposal; both themes.

**RS-03 — Dashboard onto DS-A-02.** Stat/metric tiles, chart, breakdowns through DS
renderers; counts as PageHeader counter pill (A-6); sync-note → status-line (A-8).
Verify: renderer-only composition; both themes; Reskin-notes drawer intact.

**RS-04 — Customers + Organizations onto DS-A-01/DS-A-03.** Customers abandons the
full-page profile replace (`pages-reskin.js:207-227`) for row → DetailPanel (A-1); one
toolbar + FilterBar for both (A-2); one detail record header (A-4); counts as header
pill (A-6). Verify: identical open/filter/act flow on both pages; blind walkthrough.

**RS-05 — Knowledge + Help Center.** One FilterBar (A-2: removes the stacked duplicate
search rows, `pages-reskin.js:265-266`); row-action rule (A-5: the reviewed/verify/
delete triple → ≤2 inline actions, destructive = danger + ConfirmDialog); DS-C-14 empty
states with why+next-step (A-7); article status via DS.TONE (A-3 site). Verify:
filter+apply parity; register rows A-5/A-7 closed for these pages.

**RS-06 — Incidents, Issue Radar, Reports (+ Report Builder).** Onto list/dashboard
archetypes; severity bands and tiles resolve through DS.TONE; approval/when-chip
composites per B-2. Verify: renderer-only; incident/report detail flows open DetailPanel.

**RS-07 — Operations + AI Center onto DS-A-06 (console).** engine-hero → DS-C-24 (B-2);
approval-line/when-chip → chip + status-line composition (B-2); gap-candidate and
endpoint blocks fold into Card + kv rows (B-2); receipt-cited quirks preserved (C-1:
endpoint name not index, approvals human-first with collapsible raw JSON). Verify:
console pages composed from DS parts only.

**RS-08 — Settings, Connectors, Notifications onto DS-A-04 (settings).** Tabs,
FormFields, isolated danger zones; notifications dot-as-state + relative time with
raw-UTC title kept as receipt (C-1); mark-all-read. Verify: settings tab parity;
danger-zone isolation visual check; both themes.

**RS-09 — All remaining NAV_GROUPS pages.** LiveChats, Search, SupportGraph, SyncHealth,
Onboarding, Shell, Outreach, Automation, CustomObjects, Diagnostics — final sweep onto
archetypes (page set = the 23 PageKeys, `crates/app-shell/src/pages/mod.rs:299-326`,
grouped per `app.rs:926-971`). Verify: every lab nav entry renders via the registry;
no stub left.

**RS-10 — Conformance burn-down + Gate G1.** Kill reskin.css one-off classes (the
235-class layer → DS layer only) and ~30 inline styles; off-scale 5/7/9/11px gaps →
4pt scale (B-3); five info-text patterns → five fixed roles (B-1). Tone audit: zero
status colors outside DS.TONE, both themes (A-3). Mismatch register: all A/B rows
closed; `ds.html` register section updated. Grep gate: pages ≥90% DS-composed, zero
one-off CSS. Goldens: headless screenshots of all 23 pages × both themes ×
1440/1024/800 — the reference set for R3. Blind walkthrough of same-type pages.
Release design-lab-v4 (commit + zip asset). **GATE G1 — owner reviews the conformed
lab and approves it as the final design; no Rust work before this (rule.md STOP).**

### Batch RS-1 status — all ten items verified 2026-10-28 (pending Gate G1 owner approval)

- **RS-01 — engine:** `node --check` clean on `ds.js`/`pages-ds.js`/`reskin.js`; vm smoke harness: 23/23 registry
  pages resolve to exactly one archetype (`stack`=DS-A-02/A-04, `console`=A-06, `list`=A-01, `record`=A-03,
  `inbox`=A-05), every `meta/tabs/blocks/list/detail/hero` renders strings without data errors; zero page markup
  outside `DS.*` (pages-ds.js: 0 inline styles, 0 non-DS classes — grep evidence).
- **RS-02 — Inbox:** DS-A-05 workspace live; golden-path suite re-verified in browser: open conversation, view
  filters (Mine→2/All→6), reply send (agent card + durable-job toast), note tab + send, saved-reply insert,
  AI message insert, copilot resolution insert, copilot toggle, Ctrl+Enter send, `j`/`k` + `/` shortcuts,
  notes drawer open/close. Both themes screenshot-verified.
- **RS-03..RS-09 — pages:** full 23-page × both-theme browser sweep with **zero console errors**; spot-verified
  interactions: reports grouped tabs (Builder tab), search Enter run, notifications mark-read/mark-all,
  knowledge help-center single FilterBar (A-2), automation New Rule state, customers/organizations/incidents
  DetailPanels (DS-C-17) with DS-A-03 record-head composition.
- **RS-10 — burn-down:** `pages-reskin.js` deleted (receipts at a8492d0); `reskin.css` 756 → ~190 lines,
  shell-only, consumes `--ds-*` tokens; tone audit clean (all status via DS.TONE; TONE rows added by the
  map-row-first process: live, catching_up, incident + campaign statuses). Register A-1..A-8 + B-1..B-3
  **CLOSED** with receipts in `mismatch-register.md` + live on `ds.html#register`. Goldens: 23 pages ×
  2 themes × 1440/1024/800 = **138 PNGs** in `mockups/design-lab/goldens/v4/` + `audit.json` structural
  record (one page-head everywhere; filter/detail/header parity).

**GATE G1 — STOP.** The conformed lab is the owner-review candidate for the final design. No Rust file is
touched (Stage 2 starts only after explicit owner approval; Batch RS-2 = RS-11..RS-20).
