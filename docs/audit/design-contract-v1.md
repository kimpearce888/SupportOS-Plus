# SupportOS+ Design Contract v1.0 (DC v1.0) — F0 freeze

> **Status: FROZEN 2026-10-28** (RS-11, Batch RS-2). This document is the binding
> contract for the egui reskin (R1–R4). It is extracted verbatim from the conformed
> design lab (`mockups/design-lab/` at commit `af5bc2a` `design-lab-v4`) — the owner-
> approved reference (Gate G1). Nothing here is invented; every value already renders
> in the lab's 23 pages.
>
> **Change control (binding):** any change to this file is a versioned DC release
> (v1.1, v2.0 …) recorded in `ai/decisions.md` **before** code changes. Code never
> leads the DC. The Rust reskin tracks this file — one DC token = one `design.rs`
> constant, one DS-C = one typed widget, one DS-A = one page composition recipe.
>
> **Visual baseline:** `mockups/design-lab/goldens/v4/` — 23 pages × light/dark ×
> 1440/1024/800 = 138 PNGs + `audit.json` (structural record: one page-head, one
> filter, one table, one detail pattern per page). R3 goldens are diffed against
> this set. The living spec is `mockups/design-lab/ds.html` (registry + register).

---

## 1. Governance rules

| # | Rule |
|---|------|
| G-1 | **Composition** — a page = archetypes (DS-A) + components (DS-C). No bespoke styling; zero per-page variants outside this contract. |
| G-2 | **Tone** — status → color **only** via DS.TONE (§5). One meaning = one token, both themes. AI is always violet; notes always amber. |
| G-3 | **Scales** — spacing 4pt scale (§2.4), type roles (§2.5), radius/elevation/motion (§2.6). Off-scale = bug. |
| G-4 | **Change control** — DC bump before code (this file + `ai/decisions.md`), then pages update. |
| G-5 | **Honesty contract** — receipt-cited quirks (§7 Class C) stay visible; data-layer bugs are footnoted, never faked fixed; mocked data-track items are labeled (C-2). |
| G-6 | **Scope** (decisions.md D6) — presentation only. No feature, data, workflow, shortcut or page-set change. Lab HTML/CSS/JS is spec/preview only, never shipped. |
| G-7 | **Exceptions** — anything on screen that cannot trace to §3–§6 must be a registered exception (§7) — never silently approximated. |

## 2. Token sheet (DS-T) — `--ds-*` values, both themes

Source: `mockups/design-lab/ds.css` (lines 33–66). Ported 1:1 into
`crates/app-shell/src/design.rs` LIGHT/DARK tables (RS-12).

### 2.1 Surfaces

| Token | Light | Dark | Use |
|-------|-------|------|-----|
| `--ds-bg` | `#f4f4f5` | `#09090b` | app canvas behind all cards |
| `--ds-surface` | `#ffffff` | `#18181b` | cards, raised content, menus |
| `--ds-surface-2` | `#fafafa` | `#0c0c0e` | chrome rails, faint rows, table head |
| `--ds-input` | `#ffffff` | `#0c0c0e` | text inputs, textareas |
| `--ds-border` | `#e4e4e7` | `#27272a` | hairlines, card edges |
| `--ds-border-strong` | `#d4d4d8` | `#3f3f46` | hovered edges, dashed dividers |

### 2.2 Text & interaction

| Token | Light | Dark | Use |
|-------|-------|------|-----|
| `--ds-text` | `#09090b` | `#fafafa` | primary content |
| `--ds-text-2` | `#71717a` | `#a1a1aa` | secondary text, labels, meta |
| `--ds-primary` | `#18181b` | `#fafafa` | primary buttons, active-tab underline, selected-row inset |
| `--ds-on-primary` | `#fafafa` | `#18181b` | text on primary |
| `--ds-hover` | `rgba(9,9,11,0.045)` | `rgba(250,250,250,0.06)` | hover wash on rows/items |

### 2.3 Status tones · role surfaces

| Token | Light bg | Light fg | Dark bg | Dark fg | + border (L/D) |
|-------|----------|----------|---------|---------|----------------|
| `--ds-info` | `#eff6ff` | `#1d4ed8` | `#172554` | `#60a5fa` | solid: `#2563eb` / `#3b82f6` |
| `--ds-success` | `#f0fdf4` | `#15803d` | `#0a2c1a` | `#4ade80` | solid: `#16a34a` / `#22c55e` |
| `--ds-warning` | `#fffbeb` | `#b45309` | `#2b2205` | `#fbbf24` | `#fde68a` / `#57431a` (solid `#d97706`/`#f59e0b`) |
| `--ds-danger` | `#fee2e2` | `#b91c1c` | `#451a1a` | `#f87171` | solid: `#dc2626` / `#b91c1c` |
| `--ds-neutral` | `#f4f4f5` | `#3f3f46` | `#27272a` | `#d4d4d8` | — |
| `--ds-ai` | `#f5f3ff` | `#6d28d9` | `#241d3b` | `#c4b5fd` | `#ddd6fe` / `#463a70` (solid `#7c3aed`/`#a78bfa`) |
| `--ds-agent` | `#fafafa` | — | `#1d1d21` | — | `#e4e4e7` / `#2c2c31` |

Role rule: **AI identity is always violet** (`--ds-ai*`); **agent-authored messages**
use the neutral agent surface; **internal notes reuse the warning surface** (amber,
Intercom convention). Danger buttons render white-on-danger.

### 2.4 Spacing scale (4pt)

`sp-1` 4px (micro gaps inside controls) · `sp-2` 8px (control gaps, chip rows) ·
`sp-3` 12px (card padding, list rhythm) · `sp-4` 16px (page & msg-card padding) ·
`sp-5` 20px (reader padding) · `sp-6` 24px (section rhythm) · `sp-7` 32px (page
bottom). 6px is allowed **only** inside control clusters (e.g. badge gaps). Off-scale
values (5/7/9/11) are bugs (register B-3, closed).

### 2.5 Type scale

| Role | Size | Use |
|------|------|-----|
| `meta` | 11 / 11.5px | timestamps, badges, kbd |
| `small` | 12 / 12.5px | hints, secondary cells, controls |
| `body` | 13.5px | default text, nav, buttons |
| `lead` | 14px | message body, card titles |
| `section` | 15 / 18px | section titles |
| `stat` | 20 / 26px | metric values (tabular-nums) |
| `title` | 22px, −0.01em | page h1 |

Font: Ubuntu stack, weight 300 base / 500 strong. egui mapping: body = 13.5px
rounds to the egui `body`TextStyle sized 13.5 → the Rust port keeps px→pt 1:1 and
sets strong = Medium weight (see §3 egui notes).

### 2.6 Radius · elevation · motion

Radius: `r-sm` 6 (badges, text buttons) · `r-md` 8 (buttons, inputs, nav items) ·
`r-lg` 10 (tables, menus, lists) · `r-xl` 12 (cards, message cards, panels) ·
`r-pill` 999 (chips, counters, dots).
Elevation: `e1` `0 1px 2px rgba(0,0,0,0.04)` (cards) · `e2` `0 12px 32px
rgba(0,0,0,0.14)` (drawers/menus). egui: e1 ≈ 1px border shadow — the port keeps
the hairline border and drops the blur (registered exception E-1).
Motion: `t-fast` 120ms (hovers, tile lift) · `t-slow` 200ms (drawers, toast,
quoted expand). egui: animations are driver-level; the port keeps durations only
where egui animates (Toast/DetailPanel) — else static (E-1).

## 3. Component sheets (DS-C) — 17 canonical

Each sheet: anatomy → variants → do/don't → **egui mapping note** (R2 widget).
Doc-comment tag in Rust: `DC: DS-C-xx`.

### DS-C-01 PageHeader
Anatomy: 22px title + optional 12.5px subtitle (max 640px) + right action cluster +
optional counter pill (`.ds-counter`: 12.5px on neutral, r-pill, 3×10px pad).
Variants: with/without counter, with sync control (A-8 composite).
Do: counts live in the counter pill, never in card titles. Don't: second h1,
icon-only header actions.
**egui:** one `page_header(ui, title, sub, counter, actions)` widget; counter =
pilled label; actions right-aligned via `ui.with_layout(CommentRight)`.

### DS-C-02 Card
Anatomy: 1px border, r-xl, surface bg, e1; header row (14px/500, bottom hairline,
11×14px pad) + body (12×14px pad); `sub` slot (12px/300 text-2).
Variants: default · **danger** (danger border + 4px inset + inner borderless body) ·
flush. Helpers: `section-title` (15/18px), `two-col` grid (1fr/1fr, 14px gap;
wide-left 340px/1fr).
Do: one Card per content block. Don't: nested cards, card titles with counts.
**egui:** `card()` frame via `egui::Frame` + stroke/hairline; header/body split
function; danger = red stroke + inner frame.

### DS-C-03 StatTile · MetricTile (banded)
StatTile: border r-xl card, 12px label / 26px tabular value / 12px hint.
MetricTile: horizontal, r-lg, 20px count + 13px label, min-width 196; banding =
3px left border in tone color (critical→danger, warning, info); drill = hover
border-strong + 1px lift, click emits page act.
Do: tabular numbers; bands only from TONE. Don't: more than 3 band levels.
**egui:** `stat_tile()` / `metric_tile()` widgets; band = 3px left rect fill;
drill = `Response::clicked` → act.

### DS-C-05 DataTable
Anatomy: 1px r-lg wrap; head = 11px uppercase 0.04em text-2 on surface-2, 8×12 pad;
cells 9×12 pad, hairline bottom (surface-2), row hover = hover wash; `num` cells
right-aligned mono 12.5 tabular; `strong` = 500.
Variants: with row actions (≤2 inline: one outline/text primary, destructive =
danger + armed confirm — A-5 rule).
Do: one table per page region; counts via header pill. Don't: per-row button
soup, raw status text without tone.
**egui:** virtualized `data_table()` (ScrollArea + rows, no per-frame alloc);
mono numerals via `Monospace` family; row hover via `response.hovered()`.

### DS-C-06 Badge · Chip · StatusDot
Badge: 18px tinted pill (r-sm, 0×8 pad, 11px) in tone bg/fg; `ai` variant adds
ai-border. Chip: 26px r-pill bordered button, 12.5px, `on` = neutral fill +
border-strong + 500; `full` width. StatusDot: 8px (11px big) circle from TONE
(map: ok→success, info→info, warn→warning, err→danger, ai→violet, else border-strong).
Do: badge text from TONE labels; chips for filters/selections; dot bound to the
same tone as its badge. Don't: new colors, dot without a tone key.
**egui:** `badge(tone, text)` / `chip(on, text, act)` / `status_dot(tone)` —
all resolve through the single `tone` match arm (§5).

### DS-C-09 Tabs
Anatomy: underline style — 34px rows, 13px text-2 → text+500 on active, 2px
primary underline inset 10px, bottom hairline rail. Grouped variant: labeled
groups (11px uppercase text-2) with 22px group gap (AI Center reports pattern).
Variants: flat · grouped · composer Reply|Note (DS-C-21).
Do: active = exactly one. Don't: pills as tabs, mixed tab styles per page.
**egui:** `tabs(ids, active, act)` with painter underline; grouped = label + row
sequence in one widget.

### DS-C-10 FilterBar / Toolbar
Anatomy: one row, 8px gaps, wraps; filters left (inputs/combos), context +
**exactly one primary apply** right; `split` = space-between.
Do: one FilterBar per page (A-2); global ⌘K search stays in the sidebar.
Don't: stacked search rows, two primary buttons on a row.
**egui:** `filter_bar(left, apply)`; combos = `ComboBox` styled r-md; apply
button = primary variant.

### DS-C-11 Buttons
Anatomy: 32px, r-md, 13px, 0×14 pad, 6px icon gap, 13px icons.
Variants: `primary` (primary/on-primary, hover 0.88 opacity) · `outline`
(border, surface, hover border-strong+hover wash) · `outline.ai` (ai-fg/ai-border,
hover ai-bg) · `danger` (solid danger, white text) · `text` (13px, text-2, hover
wash+text) · `ai` (28px solid ai, white, 500) · `sm` (24px, 11.5px).
Do: one primary per view region; destructive always danger + armed confirm.
Don't: custom heights, ghost-but-not-text buttons.
**egui:** `btn(variant, icon, label, act)` — one widget, enum variants; armed
two-step uses ConfirmDialog (DS-C-16 set).

### DS-C-12 Inputs & forms (with DS-C-13)
Inputs: 32px, r-md, 1px border, input bg, 13px; focus = border-strong (composer
textarea focus = ai-border + 3px ai-tint ring); textarea min-64 r-lg; combo
32px r-md surface with chevron; checkbox row 13px.
Form grid: 220px label column / 1fr controls, 9×12 gap (`four` = 2×2);
field-label = 11.5px uppercase 0.04em text-2, 10×4 margins.
Do: labels via field-label, help via `hint` (B-1 role). Don't: placeholders as
labels, ad-hoc grid columns.
**egui:** `form_row(label, control)`; `text_input` styled via WidgetVisuals
override in the theme painter; focus ring painted on `response.has_focus()`.

### DS-C-14 EmptyState
Anatomy: 300px centered column (slim = 120px), 38px icon @0.7, 15px/500 title,
12.5px sub (max 480px), optional CTA button (`.ds-empty-cta`, 12px top margin).
Every empty state states **why + next step** (A-7).
Do: always sub + CTA where an action exists; slim in inline panes.
Don't: bare "No data", silent gaps.
**egui:** `empty_state(icon, title, sub, slim, cta)` centered in available rect.

### DS-C-15 Callout set (fixed roles, B-1)
Five fixed roles — never interchangeable: `banner` (page context, 12.5 text-2) ·
`hint` (field help) · `callout` (in-card context: neutral r-md 8×10; `.warn` =
warning bg/fg + border; `.err` = danger bg/fg) · `status-line` (action result,
12.5px: ok=success-fg, err=danger-fg) · `quirk` (receipt-cited quirk: dashed
border-strong r-md, 12px text-2, 55% neutral bg, 12px icon).
Do: pick by role, not by look. Don't: status-line for static context, callout
for page banners.
**egui:** five typed functions; quirk = dashed stroke frame.

### DS-C-16 Toast · armed ConfirmDialog · DetailPanel (DS-C-17)
Toast: fixed bottom-center, primary/on-primary 12.5 r-9, 8×15 pad, e2 shadow,
200ms rise+fade. DetailPanel: right sheet 420px (94vw max), surface, left
hairline, e2; header 13×16 pad + 14px/500 title; body scroll 14×16 pad; scrim
rgba(0,0,0,0.28); opens on row `open` act (A-1); Esc closes.
Do: one panel open at a time; panel content = DS-A-03 composition.
Don't: modal-style center dialogs for detail views.
**egui:** Toast = timed overlay painter (top layer); DetailPanel = right-anchored
Area + transparent scrim rect capturing clicks; Esc via input state.

### DS-C-19 Avatar & Assignee
Avatar: r-pill, 500; `lg` 34px neutral-bg/text-2 hairline · `md` 28px · `sm` 16px
primary/on-primary. Role-typed colors inside message cards (DS-C-20).
Assignee: 11.5px text-2 inline (avatar+name); `dashed` variant = dashed r-pill
(= unassigned).
Do: initials only, role colors from the map. Don't: images, per-page palettes.
**egui:** `avatar(size, initials, role)` painted circle + centered text;
`assignee()` = avatar + label row.

### DS-C-20 Message Card (role-typed)
Anatomy: r-xl card 12×16/13 pad; head = avatar + 13.5px/500 name + 11.5px role
(with icon) + right 11.5px time; body 14px/1.62 pre-wrap; footer actions 7px gap.
Role surfaces: `customer` = surface; `agent` = agent-bg/-border, avatar primary;
`note` = warning bg/border, avatar warning; `ai` = ai-bg/border, avatar solid
violet + white; ai/note role text inherits card fg @0.75. `unread` accent = 3px
info left border (card variant).
Do: role = one of the four; timestamps right-aligned. Don't: role colors outside
the map, mixed role styles in one thread.
**egui:** `message_card(role, name, time, body, actions)`; role → surface/stroke/
avatar lookup table.

### DS-C-21 Composer
Anatomy: tabs (Reply|Note underline-on-primary; note-mode = warning-tinted
textarea + warning send), toolbar chips (27px r-md: saved-reply, AI insert) with
menu (340px sheet, e2, 5px pad, items r-md hover), textarea (min-92, 13.5px/1.55,
10×12 pad), foot = context left (11.5 text-2) + send right (30px r-md primary;
note = warning bg; Ctrl+Enter shortcut), draft-saved flag (11px text-2, fades in).
Do: Reply default; Note explicit; shortcuts preserved (Ctrl+Enter send, saved-
reply insert). Don't: hiding the mode, auto-send.
**egui:** `composer(state)` widget group; menu = popup Area; flag via fade alpha.

### DS-C-22 AI identity set
Copilot cards: `cop-card` (r-lg card) · `suggest` (ai-bg/ai-border, ai-fg 11.5/500
title with icon) · bullets (4px dot, 12.5/1.5) · similar-question rows (top
hairline, 12.5/500 + 11.5 result) · chat bubbles (r-xl; assistant = ai-bg/ai-border,
user = neutral; user row right-aligned) · cited line (11.5 text-2).
AI badge: solid-violet tint pill "AI" (from TONE `ai`).
Rule: **AI is always violet, in every surface** (G-2).
Do: cite sources line; 5-second identification pre-check passes on the card alone.
Don't: AI content in neutral cards, violet for non-AI things.
**egui:** `cop_suggest/cop_summary/cop_similar/cop_chat` widgets; ai_badge().

### DS-C-24 Hero & event rows (console)
Hero: r-xl surface card, 14×16 pad, mono 17px/500 state + description + right
actions. Event rows: 5px pad, hairline bottom; level inset = 2px left shadow
(error=danger, warn=warning) + 6px pad; line = ts + dot + text wrap.
Do: hero only on console pages (DS-A-06) — never on list/dashboard pages (A-4).
Don't: level colors outside TONE.
**egui:** `hero(state, desc, actions)`; `ev_row(level, ts, text)` with left inset
rect fill.

*Folded helpers (no separate sheet, part of the above):* KeyValue rows (DS-C-18
kv/timeline), bar rows (DS-C-23), selection list (master-detail pane), file chips
(part of DS-C-20 attachments), pill icon-button (reader head), freshness pill,
keyboard `kbd` chip.

## 4. Archetype sheets (DS-A) — 6 page recipes

| id | Recipe | Pages |
|----|--------|-------|
| **DS-A-01 List page** | PageHeader (+counter pill, +sync) → FilterBar → DataTable → row `open` act opens DetailPanel (DS-C-17) whose content is a DS-A-03 composition. | customers, organizations, incidents |
| **DS-A-02 Dashboard** | PageHeader → optional tabs → block stack (statGrid, tiles, charts, cards, kv, bars). | dashboard, reports, support-graph, issue-radar, custom-objects, outreach, automation |
| **DS-A-03 Detail record** | record-head (avatar + name + meta + badges + actions) → two-col blocks / cards. Used standalone and as DetailPanel content. | (composition recipe) |
| **DS-A-04 Settings** | PageHeader → tabs → form sections (Card + FormGrid + hint) → isolated danger zones (danger Card, armed confirm). | settings, connectors, notifications, onboarding, live-chats |
| **DS-A-05 Inbox workspace** | 4 panes: views rail (172px, surface-2, count per item) → conversation list (330px: head + filter + rows: unread dot, name, 2-line preview, time) → reader (head avatar row + scroll + composer DS-C-21) → copilot panel (320px surface-2, DS-C-22). | inbox |
| **DS-A-06 Console / log** | PageHeader → hero (DS-C-24) → chips → event log / kv cards / blocks. | operations, ai-center, diagnostics, sync-health, search |

Rule: a page = PAGE-DATA + **one** archetype call; pages never restyle archetype
anatomy (DS-4 engine rule, carried into R3).

## 5. DS.TONE — the only status → color map

`toneOf(status)` → tone → `--ds-*` pair. Adding a status = adding a map row first.
Colors resolve per theme from §2.3.

| status | tone | label | status | tone | label |
|--------|------|-------|--------|------|-------|
| open | info | Open | ok | success | OK |
| pending | warning | Pending | healthy | success | Healthy |
| waiting | warning | Waiting | live | success | Live |
| resolved | success | Resolved | stale | warning | Stale |
| closed | neutral | Closed | error | danger | Error |
| spam | danger | Spam | syncing | info | Syncing |
| active | info | Active | catching_up | info | Catching up |
| published | success | Published | ai | ai | AI |
| draft | neutral | Draft | note | warning | Internal note |
| internal | info | Internal | customer | neutral | Customer |
| critical | danger | Critical | agent | neutral | Agent |
| high | warning | High | medium | info | Medium |
| low | neutral | Low | | | |

egui mapping (RS-12): one `fn tone(status) -> Tone` match arm + one
`fn tone_colors(tone, theme) -> (bg, fg, solid, border)` — no other status-color
site in the crate.

## 6. Layout & chrome (shell)

Sidebar nav (surface-2, 218–260px): groups verbatim from NAV_GROUPS
(`crates/app-shell/src/app.rs:926-971`); active item = neutral fill + 500.
Global search (⌘K) stays in the sidebar. Theme switch: light/dark from the
settings table (unchanged). Notes drawer is a **lab-only** artifact — not part
of the app shell. Shortcuts (`j`/`k` list nav, `/` filter focus, Ctrl+Enter send,
Esc close) preserved unchanged (D6).

## 7. Exception & mismatch register (DS-0 → DC)

### Class A — CLOSED in DS-4 (design-lab-v4, receipts in mismatch-register.md)

| id | Finding | DC resolution |
|----|---------|---------------|
| A-1 | Master–detail flow differed per page | One list archetype: DS-A-01 → DS-C-17 DetailPanel, content = DS-A-03. |
| A-2 | Search/apply button styles differed | DS-C-10 FilterBar: filters left, exactly one primary apply; ⌘K stays in sidebar. |
| A-3 | Status→color in ≥4 parallel systems | DS.TONE only (§5); page maps merely alias data tokens to TONE keys. |
| A-4 | Detail record header invented per page | DS-A-03 record-head everywhere; DS-C-24 hero only on console pages. |
| A-5 | Row-action buttons inconsistent | DS-C-05 rule: ≤2 inline actions; destructive = danger + armed confirm. |
| A-6 | Counts formatted differently | DS-C-01 counter pill; card titles carry no counts. |
| A-7 | Empty states ad-hoc | DS-C-14 everywhere; why + next step; slim for inline panes. |
| A-8 | Sync feedback only on some pages | Sync = button + status-line composite on every syncing page. |

### Class B — CLOSED in DS-4

| id | Finding | DC resolution |
|----|---------|---------------|
| B-1 | Five overlapping info-text patterns | DS-C-15 fixed roles: banner / hint / callout / status-line / quirk. |
| B-2 | One-off composites | engine-hero → DS-C-24; approval/when-chip → chip + status-line; gap/endpoint blocks → Card + kv. |
| B-3 | Off-scale spacing (5/7/9/11px) | 4pt scale (§2.4); 6px only inside control clusters. |

### Class C — intentional, stays by contract

| id | Item | Contract |
|----|------|----------|
| C-1 | Receipt-cited quirk footnotes + per-page Reskin notes | Data-layer bugs are footnoted (quirk role), never faked fixed: knowledge freshness void, Load/Pressure swap, endpoint-name combo, approvals human-first with collapsible raw JSON, notifications raw-UTC title. |
| C-2 | Data-track items labeled mocked | unread counts, assignee, draft persistence — presentation replicas until the async data track (U2) lands. |

### Registered egui exceptions (R-port)

| id | Exception | Reason |
|----|-----------|--------|
| E-1 | Blur shadows (`e1`/`e2`) render as hairline strokes; motion limited to surfaces egui animates natively | egui painter model — visual weight kept via borders; never silently approximated beyond this registered note. |

## 8. Verification checklist (F0 acceptance — this RS-11 item)

- [x] DC committed as `docs/audit/design-contract-v1.md`.
- [x] Every register A/B row closed in DC (§7 — 8 A + 3 B, receipts at `af5bc2a`).
- [x] RS-10 goldens referenced as the visual baseline (§0 header: 138 PNGs + audit.json).
- [x] Token sheet = both themes, exact hex (§2, source ds.css 33–66).
- [x] 17 DS-C sheets with egui mapping notes (§3).
- [x] 6 DS-A sheets (§4).
- [x] DS.TONE table complete (§5 — 25 rows).
- [x] Change-control rule stated (header + G-4).
