# SupportOS+ Main Content Audit — Reports / Overview

**Note:** The task names this page "Issue Radar," but the screenshot shows the **Reports → Overview** tab (Issue Radar is an unselected tab). Auditing what's visible.

## 1. CONTENT INVENTORY

- Top bar (right): "synced 2s ago" status chip, Search ⌘K input, theme-toggle icon button.
- Page header: "Reports" (h1) with subtitle "local analytics over the selected range".
- Header controls: "Sync now" button; date-range controls "1 year / 90 days / 30 days / 7 days" (30 days selected, bold).
- Tab group with three floating group labels: "Volume & quality", "AI & digests", "Platform & custom".
- Tabs: Overview (active, underlined), SLA, Why contacting, Top questions, Quality, Issue radar, Digest, Happiness, Help Scout, Builder, Insights.
- KPI row 1: four stat cards — New 18, Closed 2, Replies sent 0, Backlog 15.
- KPI row 2: Great ratings 0, Okay ratings 0, Not-good ratings 0, Unassigned 14.
- Footnote: "Ratings come from the satisfaction webhook; Help Scout's own happiness reports need a Plus/Pro plan and are not fetched."
- Chart card: "New conversations per day" with a purple area/line chart (declining 16 → 2) and horizontal gridlines.
- Caption below chart: "new conversations · 16 → 2".
- Large empty whitespace in the lower ~40% of the content area.

## 2. LAYOUT DEFECTS

- KPI cards occupy only ~left half of the content width; the right half (from ~x=990 to 1897) is empty beside all eight cards.
- Second KPI row is visually jammed against row 1 (near-zero vertical gap between rows) while the gap above the footnote/chart is larger — uneven rhythm.
- Chart card extends full width but chart line starts at the extreme left edge of the plot and ends at the extreme right, with data points touching the card padding.
- Chart's bottom-right point nearly touches the card's bottom border; no bottom padding under the plot.
- Chart caption ("new conversations · 16 → 2") sits *outside* the card, floating ambiguously — it looks detached from the chart it describes.
- Massive dead space below the chart (~y=690 to 1080): content ends at ~62% of viewport height with nothing below — page feels unfinished.
- Tab group labels ("AI & digests", "Platform & custom") float with no visual connection (no divider, background, or indent) to the tabs they group; "AI & digests" hovers oddly mid-whitespace.
- Date-range controls vertically misaligned with "Sync now" button baseline (they read as text links, not buttons, on a different visual plane).

## 3. VISUAL / TYPOGRAPHY DEFECTS

- Casing inconsistency: page title "Reports" / card "New conversations per day" in Title Case, but caption "new conversations · 16 → 2" in lowercase, and subtitle "local analytics over the selected range" sentence case with no period hierarchy.
- "30 days" is shown bold as the selected range, but there's no container, underline, or pill — selection state is weight-only and nearly invisible.
- KPI labels and values are fine, but rating cards ("Great ratings 0") give zero visual differentiation from volume cards despite different meaning (ratings could be colored/sectioned).
- Tab labels use inconsistent casing concepts: "Help Scout" (proper noun) vs lowercase-ish feel elsewhere; "Issue radar" vs sidebar "Issue Radar" — terminology casing mismatch.
- Footnote is small gray text but carries an important limitation (paid-plan gating) — hierarchy too weak for its significance.
- Group labels "Volume & quality" etc. are tiny and gray, competing with the tabs themselves.

## 4. COMPONENT DEFECTS

- Date-range "1 year / 90 days / 30 days / 7 days" appear as plain text — unclear they're clickable; click targets are text-only, likely <32px tall.
- "Sync now" button has no visible loading/success state implied; combined with the "synced 2s ago" chip, two separate sync indicators with unclear relationship.
- Tabs: only the active tab has an underline; hover/focus states not distinguishable; group labels are not clickable affordances but look like they might be.
- Stat cards have no hover or click affordance (are they drill-downs?) — ambiguous interactivity.
- Icon-only theme button (top right, bell-like icon) has no tooltip/label.
- No chart legend, axis labels, or y-axis values on the chart — component is incomplete.

## 5. UX DEFECTS

- Chart has no x-axis dates and no y-axis scale — "per day" chart is unreadable beyond "it went down."
- Zero values everywhere (0 replies, 0 all ratings) with no empty-state explanation on cards; only the footnote explains ratings — replies sent = 0 is unexplained and may look like a data bug.
- No export, filter, or drill-down anywhere on the page — dead end for a Reports page.
- "Help Scout" and "Builder" tabs appear grouped under "Platform & custom" but mix third-party and internal concepts — grouping logic unclear.
- "Sync now" + "synced 2s ago": user can't tell what sync covers (all data? this report?).
- The "16 → 2" delta in the caption duplicates chart data but doesn't state the time period.

## 6. ACCESSIBILITY

- Date-range selection indicated only by font weight — likely fails non-color/weight-independent state requirements; keyboard focus state unknown.
- Chart conveys data via color/area only with no text alternative or table; screen-reader users get nothing.
- Group labels (gray on white) appear low-contrast at small size (~12px).
- Caption "new conversations · 16 → 2" is low-contrast gray and semantically disconnected from the chart.
- Tab bar has no visible ARIA tablist affordance/roving focus indicator in the static design.
- Footnote text is very small and light gray — contrast risk.

## 7. VERDICT

- **[major]** Chart lacks axis labels, scales, and any text alternative — data is not interpretable.
- **[major]** Lower 40% of the page is empty dead space; layout looks unfinished.
- **[major]** KPI grid uses only half the available width; right side of both rows is blank.
- **[major]** Date-range control looks like plain text; tiny click targets and near-invisible selection state.
- **[minor]** Chart caption sits outside the card, detached from the chart it describes.
- **[minor]** Tab group labels float without visual grouping or connection.
- **[minor]** Casing/terminology inconsistency ("Issue radar" vs "Issue Radar", caption lowercase vs Title Case headers).
- **[minor]** Zero-value KPIs have no inline explanation or empty-state treatment.
- **[minor]** Reports page has no export/filter/drill-down — functional dead end.
- **[minor]** Two sync indicators ("Sync now" vs "synced 2s ago") with unclear scope.
- **[polish]** Rating KPI cards not visually distinguished from volume KPI cards.
- **[polish]** Icon-only theme button lacks tooltip/label.
- **[polish]** Stat cards' interactivity (if any) is not signaled.
- **[polish]** Vertical spacing between KPI rows inconsistent with the rest of the rhythm.
- **[polish]** Page is titled "Issue Radar" in the task but displays Reports Overview — possible navigation/state bug worth verifying.