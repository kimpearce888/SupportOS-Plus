# SupportOS+ UX Design Lab

A high-fidelity **HTML/CSS/JS replica** of the SupportOS+ desktop shell, built so the
before/after of the approved reskin plan can be *seen and clicked* before any Rust code
changes. The Rust program, its schema, and its sync behaviour are untouched — everything
here is a static design reference (reskin plan phases R0-R3 made tangible).

## Open it

- `index.html` — lab shell: **Before / After / Side-by-side** toggle.
- `current.html` — pixel-faithful replica of the **current v1.8.0 shell** (zinc tokens from
  `design.rs:104-179`, 66px list rows, 300px thread keyhole, flat thread entries).
- `reskin.html` — the **reskin proposal**: token v3 semantic surfaces, role-typed message
  cards, AI identity, collapsed quoted history, Reply|Note composer with saved replies and
  AI-draft insert, typed Copilot cards, unread/assignee triage, `j`/`k` keyboard navigation.
- `data.js` — shared synthetic fixture mirroring `ThreadEntry` semantics
  (`core/inbox.rs:326-341`) plus the async-track fields (read state, assignee).

## Design notes (the receipt layer)

In the **After** view, toggle **Design notes** at the bottom of the views rail. Every
change carries a numbered pin and cites the forensic receipt it fixes, e.g.:

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

## Scope contract

- No DB/schema/sync/feature changes are proposed by the reskin itself.
- Items that need the async data track (real unread flags, assignment persistence, draft
  persistence, attachment rendering) are **mocked here intentionally** and marked as
  `U2-I1/I2` — see the UX Recovery & Reskin Plan PDF, chapters 8-9.
- Fixture data is synthetic; any resemblance to real conversations is coincidental.

## Files

```
mockup/
├── index.html    lab shell (mode toggle, side-by-side)
├── lab.css       lab chrome styles
├── current.html  before-replica page
├── current.css   replica tokens + layout (matches design.rs)
├── current.js    replica behaviour (sync flash, draft loss, badge tones)
├── reskin.html   after-proposal page
├── reskin.css    token v3 + message cards + composer + copilot + notes
├── reskin.js     proposal behaviour (inserts, tabs, notes, j/k)
├── data.js       shared fixture data
└── README.md     this file
```
