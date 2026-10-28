# SupportOS+ Design System v1.0 — Mismatch Register (DS-0)

> Evidence base for the Design System (Task 22). Audit of the 23-page design lab
> (`mockups/design-lab/`, commit a8492d0): what mismatched across pages even though
> the theme was consistent, and the DS resolution each item receives.
> **Class A** = same role, different look/flow (must unify) · **Class B** = one-off
> patterns needing rationalization · **Class C** = intentional, stays by contract.
> Live status also rendered on `ds.html#register`.

## Class A — unify (burn to zero in DS-4)

| id | finding | evidence (page → pattern) | DS resolution |
|----|---------|---------------------------|---------------|
| A-1 | Master–detail flow differs per page | **Customers**: search → table → `open` → full-page profile replace + `← back to search` (pages-reskin.js:207-227). **Organizations**: left `sel-list` + right detail column (pages-reskin.js:231-244). **Knowledge/Incidents/others**: table only, no detail flow. | One **List-page archetype (DS-A-01)**: toolbar → DataTable → row action opens **DetailPanel (DS-C-17)**. Detail content = DS-A-03 composition. |
| A-2 | Search/apply uses different button styles per page | customers: `input + primaryBtn("search")` (208). organizations: `input + outlineBtn("filter")` (243). knowledge/help-center: two combos + outline `apply` **and** a second stacked `input + primary "search"` row (265-266). | **FilterBar (DS-C-10)**: filters left (input/combo), exactly one primary apply on the right. In-page list search only; ⌘K global search stays in the sidebar. |
| A-3 | Status→color decided in ≥4 parallel systems | central `statusTone()` map (pages-reskin.js:21-24) **plus** inline ternaries for knowledge articles (270) **plus** `.state-dot.*` CSS classes **plus** `.health-dot.*` (reskin.css:656-659) **plus** `.tile` level classes (607-609). | Single **DS.TONE** map (ds.js). Badges, dots, tiles, bands and severities all resolve through it; adding a status = adding a map row first. |
| A-4 | Detail record header invented per page | customers profile: `section-title big` + hint meta line (216). organizations: same recipe (240). inbox: `reader-head` avatar row. diagnostics: `engine-hero` (685). | **DS-A-03 Detail record** header: avatar + name + meta pills + status/AI badges. `engine-hero` only on console pages (DS-C-24). |
| A-5 | Row-action buttons inconsistent | knowledge documents: outline `reviewed` + outline `verify` + **danger** `delete` triple inline (256). customers: single outline `open` (211). notifications: none. | Rule in **DS-C-05**: ≤2 inline actions, one text/outline primary action, destructive always danger + confirm dialog. |
| A-6 | Counts & record labels formatted differently | "4 customers (of 4)" inside card title (209) vs "N organization(s)" as section title (236) vs inbox view counts `.view-count`. | **DS-C-01** counter pill in the PageHeader; card titles never carry counts. |
| A-7 | Empty states ad-hoc | `emptyState()` component (organizations 242, knowledge freshness 262) vs plain `hint("(no milestones recorded)")` (224) vs silent gaps elsewhere. | **DS-C-14** everywhere; slim variant for inline panes; every empty state states why + next step. |
| A-8 | Sync feedback only on some pages | `sync-note` span wired on dashboard/customers/organizations headers (114, 206, 234); knowledge "Sync Help Center now" (250) has no feedback affordance. | Sync = one composite: button + inline **status-line** result ("syncing… → up to date / error"). |

## Class B — rationalize

| id | finding | evidence | DS resolution |
|----|---------|----------|---------------|
| B-1 | Five overlapping info-text patterns | `banner`/`hint-banner`, `hint`, `callout`, `status-line`, `quirk-note` used interchangeably (reskin.css:523-535, pages-reskin.js:33,53-56). | Fixed roles in **DS-C-15**: banner=page context · hint=field help · callout=in-card context · status-line=action result · quirk=receipt-cited quirk (stays). |
| B-2 | One-off composites | `engine-hero` (685), `approval-line`/`when-chip` (650-651), `gap-cand` (647), `ep-block` (653). | engine-hero → **DS-C-24**; approval/when-chip → chip + status-line composition; gap/endpoint blocks fold into Card + kv/timeline rows. |
| B-3 | Off-scale spacing | micro-gaps of 5/7/9/11px across conv rows, tool rows, msg head (reskin.css pass). | **DS-4** normalizes to the 4pt scale (2/4/6/8/12/16/20/24/32); 6px allowed only inside control clusters. |

## Class C — intentional, stays by contract

| id | item | contract |
|----|------|----------|
| C-1 | Receipt-cited quirk footnotes + per-page Reskin notes | Data-layer bugs are footnoted, never faked fixed (knowledge freshness void, Load/Pressure swap, endpoint-index combo, approvals raw-JSON-first…). |
| C-2 | Data-track items labeled mocked | unread counts, assignee, draft persistence — presentation replicas until the async data track (U2) lands. |

## Conformance gate (DS-4 acceptance)

1. Every A/B row above resolved in the lab rebuild; register goes **0 open A/B**.
2. Every page ≥90% composed of DS renderers (grep gate: pages import `DS.*`, no one-off CSS).
3. One header, one table, one filter, one detail pattern across all 23 pages.
4. Tone audit: no status renders outside DS.TONE, both themes.
5. Blind walkthrough: same-type pages feel muscle-memory identical (open / filter / act / feedback).
