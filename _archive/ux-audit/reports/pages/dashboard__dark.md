# Dark Theme Audit — SupportOS+ Dashboard (1920×1080)

## Overall Verdict
The dark theme is **mostly coherent** — near-black background (#0a0a0a–#111), dark cards, purple accent. No white flash boxes detected. But there are several legibility and hierarchy issues.

---

## 1) Contrast Issues

- **[MEDIUM] Sidebar section labels** — "Workspace", "Customers & knowledge", "Insights", "Automation & AI", "System" are very dim gray on black. At 1080p they're borderline; on dimmer panels they'd disappear. Needs ≥ #8a8a8a.
- **[MEDIUM] Subtitle text** — "Local metrics · sources labeled Help Scout / local / AI-derived everywhere" under the page title is low-contrast gray. Small font + dim gray = straining.
- **[LOW] Chart caption** — "new conversations · 16 → 2" sits below the chart in dim gray against the page background; nearly invisible at a glance.
- **[LOW] "Webinars Today" chip (filter row, Mailboxes)** — inactive chip text is dim against a dark chip fill; distinguishable but weak.
- **[OK] Sidebar nav items** (Dashboard, Inbox, etc.), stat card labels, and the "synced 11s ago" green badge all have adequate contrast.

## 2) Card / Background Separation

- **[MEDIUM] Filter panel ("Range / Channel / Mailboxes") vs stat cards vs page background** — all three surfaces are nearly identical dark grays. The filter panel reads as a floating blob; the stat cards' borders are barely visible. The whole middle of the page feels flat.
- **[MEDIUM] Right side of the content area is a huge empty black expanse** — stat cards occupy only the left ~half, leaving ~1000px of raw background. Reads as broken/unbalanced layout rather than intentional.
- **[LOW] "By mailbox" and "By tag" cards** have slightly lighter fill than the page, which helps — but their borders are faint; edge definition is inconsistent with the chart card above.

## 3) Chart Colors on Dark

- **[MEDIUM] Area chart fill opacity** — the purple gradient fill under the line is quite muted/gray-purple; combined with a single 2-point line it looks washed out and slightly "dead". Line itself (light purple) has good contrast — that's fine.
- **[LOW] No gridline/axis label visibility** — axis area appears empty; if gridlines exist they're too faint. Chart lacks any anchor values on dark.
- **[OK] Bar charts (By mailbox / By tag)** — bright purple bars on dark cards: high contrast, works well.

## 4) Chip / Button Legibility

- **[MEDIUM] Active filter chips ("30 days", "All channels", "All")** — dark-gray pill with light text; the active state is too subtle. It doesn't clearly read as "selected" vs. the inactive dim-gray chips. Needs a stronger fill or accent border.
- **[LOW] "Sync now" button (top right)** — dark outline button with gray text; low affordance, looks disabled at first glance.
- **[LOW] "Search ⌘K" topbar pill** — fine, but icon+text contrast is on the dim side.
- **[OK] "synced 11s ago"** — green-on-dark badge is legible and a good status cue.

## 5) Broken / Wrong-looking Elements in Dark Mode

- **[HIGH] Layout looks broken, not just dim** — the KPI card grid stops mid-page leaving a large empty black region; combined with the floating filter panel it reads as a rendering failure. Also, the "First response avg" card is visually taller (has a "min" unit row) than its row-mates ("Unassigned", "Backlog", "Replies sent"), which misaligns the grid rows.
- **[MEDIUM] "min" unit labels** — tiny gray "min" under "First response avg" and "Resolution avg" values but the values show **0** and **6** with a jarring gap; unit placement looks orphaned.
- **[LOW] Chart clipped at bottom** — "By tag" card is cut off at the viewport bottom (bottom row of bars/tags truncated), so the page appears to end abruptly.
- **[LOW] "switch to light theme"** at sidebar bottom — dim text; functional but easy to miss.

---

## Priority Fixes
1. [HIGH] Fix stat-card grid alignment/height and the half-empty right region (or widen cards full-width).
2. [MEDIUM] Strengthen active-chip styling (accent fill/border) and raise contrast of section labels, subtitle, and chart caption.
3. [MEDIUM] Add visible chart gridlines/axis values and bump area-fill opacity for a livelier chart.
4. [LOW] Unify card border treatment across filter panel, KPI cards, and bottom cards.
