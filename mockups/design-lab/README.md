# SupportOS+ UX Design Lab

A high-fidelity **HTML/CSS/JS replica** of the SupportOS+ desktop shell, built so the
before/after of the approved reskin plan can be *seen and clicked* before any Rust code
changes. The Rust program, its schema, and its sync behaviour are untouched — everything
here is a static design reference (reskin plan phases R0-R4 made tangible).

**v2 — ALL 23 pages.** Every page in `app.rs:926-971 NAV_GROUPS` now has a faithful
Before replica and a token-v3 After proposal, built from forensic specs that cite
file:line for every label, column, button, empty state, and quirk.

## Open it

- `index.html` — lab shell: **Before / After / Side-by-side** toggle. Page navigation is
  synced across both replicas (click a sidebar entry in one, the other follows).
- `current.html` — pixel-faithful replica of the **current v1.8.0 shell** (zinc tokens from
  `design.rs:104-179`): 66px list rows, 300px thread keyhole, flat thread entries, and the
  real layouts of all 22 other pages — including their honest quirks (raw UTC stamps, the
  Operations Load/Pressure value swap, the seven-zero Outreach report, the empty Freshness
  tab, `(unset)`/`—` placeholders).
- `reskin.html` — the **reskin proposal**: token v3 semantic surfaces, role-typed message
  cards, AI identity (violet), collapsed quoted history, Reply|Note composer with saved
  replies + AI-draft insert, typed Copilot cards, unread/assignee triage, `j`/`k` keyboard
  navigation — plus the v3 treatment of all 22 other pages (underline tabs, severity bands,
  status dots, load bars, isolated danger zones, per-page **Reskin notes**).
- `data.js` — shared synthetic fixtures mirroring `ThreadEntry` semantics
  (`core/inbox.rs:326-341`) and every page's real section structure.
- `pages-current.js` / `pages-reskin.js` — the 22 non-Inbox page renderers for each mode.

## Page coverage (23/23)

| Group | Pages |
|---|---|
| Workspace | Dashboard · Inbox (fully interactive) · Live chats · Operations Center |
| Customers & knowledge | Customers · Organizations · Knowledge |
| Insights | Reports (11 tabs) · Issue Radar · Incidents |
| Automation & AI | Custom Objects · Automation · AI Center (6 tabs) · Outreach |
| System | Notifications · Search · Support Graph · Connectors · Diagnostics · Sync Health · Settings (6 tabs) · Onboarding · Shell |

## Design notes (the receipt layer)

**Inbox:** toggle **Design notes** in the views rail — 10 numbered pins, each citing the
forensic receipt it fixes:

| # | Change | Was | Receipt |
|---|--------|-----|---------|
| 1 | Role-typed message cards | flat badge+text+divider entries | `inbox.rs:2858-2930` |
| 2 | AI gets its own violet identity | AI badge identical to Agent (both Info blue) | `inbox.rs:2871 vs 2875` |
| 3 | Full-height reader | 300px ScrollArea keyhole | `inbox.rs:2306-2314` |
| 4 | Quoted history collapsed | inline weak text | `inbox.rs:2916-2918` |
| 5 | Attachments surfaced | downloaded but never shown | Task 15 forensics |
| 6 | Saved replies in composer | Operations-page only | `operations.rs:746` |
| 7 | Copilot typed cards | one dense mixed-weight card | `inbox.rs:1473-1589` |
| 8 | Human time + date dividers | raw UTC stamps (`compact_time`) | `inbox.rs:2797-2801` |
| 9 | Unread / assignee / views / keys | no read flag, assignee hardcoded `None` | `inbox.rs:121` |
| 10 | Scope contract | presentation only; data-track items mocked | UX plan PDF Ch. 7-9 |

**All other pages:** each page header in the After view carries a **Reskin notes** button
opening the same receipt-style drawer (FIX / WAS / file:line) — 2-3 notes per page, e.g.:

| Page | Sample note | Receipt |
|---|---|---|
| Dashboard | "By channel" ignores the channel chip — kept, footnoted | `dashboard.rs:337-345` |
| Operations | Load/Pressure column swap un-swapped (presentation fix) | `operations.rs:425-426` |
| Knowledge | Freshness tab reads `report["flags"]` which never exists — void named, not hidden | `knowledge.rs:1249` vs `knowledge_store.rs:516-530` |
| Reports | radar badge always "medium"/Warning (engine never emits "high") | `reports.rs:1997-2001` |
| AI Center | routing combo showed endpoint *index*, now shows endpoint *name* | `ai_center.rs:1444-1448` |
| Outreach | campaign report renders seven zeros (top-level vs `totals` keys) — kept, footnoted | `outreach.rs:568-593` vs `core/outreach.rs:1336` |

## Scope contract

- No DB/schema/sync/feature changes are proposed by the reskin itself.
- Items that need the async data track (real unread flags, assignment persistence, draft
  persistence, attachment data) are labelled as mocked (U2-I1/I2), exactly as in the plan.
- Data-layer bugs are **documented, not fixed**: where presentation cannot repair them
  (Freshness tab, Outreach report zeros, channel-filter quirk) the reskin adds a visible
  footnote instead of fake content.
- Two quirks are presentation-fixable and the After view fixes them with receipts:
  the Model-role routing combo label (`ai_center.rs:1444-1448`) and the automation
  approvals raw-JSON-first rows (`automation.rs:492-501`).

## Verification

Fresh-browser session, all 23 pages walked in both modes (light + dark): zero console
errors; Inbox golden path intact in both replicas; cross-iframe page sync verified in
Side-by-side mode; 800px narrow layout stacks to a single column.
