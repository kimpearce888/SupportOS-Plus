# SupportOS+ — Reports Page Audit (Main Content Area Only)

## 1. CONTENT INVENTORY

- Page header: "Reports" (H1-style) with subtitle "Local analytics over the selected range"
- Top-right controls: "Sync now" button (with icon), and a date-range selector: "1 year / 90 days / 30 days / 7 days" (30 days appears active)
- Three floating group labels above the tab row: "Volume & quality", "AI & digests", "Platform & custom"
- Tab bar: Overview (active, underlined), SLA, Why contacting, Top questions, Quality, Issue radar, Digest, Happiness, Help Scout, Builder, Insights
- Row 1 of KPI cards (4): New 18, Closed 2, Replies sent 0, Backlog 15
- Row 2 of KPI cards (4): Great ratings 0, Okay ratings 0, Not-good ratings 0, Unassigned 14
- Explanatory footnote: "Ratings come from the satisfaction webhook; Help Scout's own happiness reports need a Plus/Pro plan and are not fetched."
- Chart card: "New conversations per day" with a purple line/area chart, and a raw-looking legend caption "new conversations · 16 → 2"
- Large empty region below the chart card (~40% of viewport height is dead space)

## 2. LAYOUT DEFECTS

- KPI cards occupy only the left ~55% of the content width (4 cards end around x=985 of 1895 usable). Rows of 4 cards that don't fill or reflow — right side is blank. Uneven, unbalanced.
- The 8 KPI cards are split into two rigid rows of 4 instead of one responsive grid (e.g., 4×2 full-width or 8-up), reinforcing the dead space on the right.
- The chart line is clipped at the left edge of the plot area — the line and its start point run into/behind the plot border at the top-left; the first data point marker is half cut off.
- The chart area's bottom-right endpoint also touches the plot border exactly; no padding around the line's extremes.
- Uneven vertical rhythm: header → tabs → cards → footnote → chart, then a very large empty band (~350–400px) between the chart card and the bottom of the window. Content is top-loaded with no anchor below.
- Footnote sits directly under the cards with tight spacing, visually reads as card body text rather than a page-level note.
- Chart card height is disproportionate (~250px of chart for 2 data points), exaggerating the diagonal.
- Date-range selector has no visual container — four bare text labels floating right of "Sync now"; the active "30 days" is distinguished only by slightly darker weight, easy to miss.

## 3. VISUAL / TYPOGRAPHY DEFECTS

- Group labels ("Volume & quality", "AI & digests", "Platform & custom") float ambiguously above tabs; their positions don't visually bind to their tab groups (e.g., "AI & digests" sits between "Quality" and "Issue radar" but covers tabs Issue radar–Happiness; alignment is approximate, not exact over each group).
- The chart legend "new conversations · 16 → 2" is lowercase, tiny, and unstyled — inconsistent with the sentence-case labels everywhere else ("New conversations per day").
- "Not-good ratings" uses a hyphenated compound while siblings are plain ("Great ratings", "Okay ratings") — awkward terminology; "Okay" is informal next to "Great"/"Not-good".
- Title-case vs sentence-case is inconsistent across tabs: "Why contacting", "Top questions" vs "Overview", "SLA", "Digest" — acceptable but "Why contacting" reads as truncated ("Why customers are contacting"?).
- All-zero KPI cards (Great/Okay/Not-good ratings = 0, Replies sent = 0) get identical visual weight to meaningful values; no de-emphasis or differentiation for empty metrics.
- Footnote text is low-emphasis gray and single-run; the plan limitation ("need a Plus/Pro plan") is important but buried.

## 4. COMPONENT DEFECTS

- Date-range selector: click targets are bare text labels with no hover/active affordance, no underline/pill container; unclear whether "1 year" etc. are buttons.
- "Sync now" button: no visible success feedback loop on the page other than the tiny top-bar "synced 9s ago" chip (which is outside the audited area but the connection is weak); no in-place spinner/state.
- Tabs: no indication of which tabs have content vs. are empty; no disabled state; no counts.
- KPI cards are not interactive but look identical to potentially-clickable cards — no affordance signal either way (no hover cue visible, no "view details" link).
- Chart has no axis labels, no gridline values, no date ticks — the x-axis is entirely unlabeled; the y-axis likewise. Only the legend line "16 → 2" conveys values.
- No chart interaction affordances (tooltip on hover? zoom? unclear).

## 5. UX DEFECTS

- No data granularity: with "30 days" selected, a 2-point line (start 16 → end 2) is a poor representation; no daily bars/points, no tooltip values, no axis context. The user can't answer "when did volume drop?"
- "Replies sent: 0" alongside "New: 18" is contradictory-looking with no explanation (is syncing incomplete? is this metric broken?). Zero-valued metrics need a "no data" vs. "genuinely zero" distinction.
- Footnote is the only explanation for the zeros in ratings — it's easy to miss and doesn't state that ratings are currently unavailable (vs. actually 0).
- Dead end: nothing below the chart; no "view more", no secondary sections, no link to other report tabs. The bottom 40% of the page is empty.
- Group labels above tabs imply grouping but tabs are one flat row — the grouping system is half-implemented and confusing.
- "Sync now" vs. automatic sync vs. the top-bar "synced 9s ago" chip: relationship unclear.

## 6. ACCESSIBILITY

- Chart conveys data purely through a line shape with no text alternative, no table fallback, no ARIA description; screen-reader users get only the terse legend.
- Date-range "selected" state (30 days) appears to rely on font weight alone — likely insufficient contrast/non-visual state; no aria-selected semantics verifiable.
- Active tab indicated by underline only; color/underline contrast of the indicator vs. border is low.
- Footnote gray-on-white text is small and light — likely below WCAG AA contrast.
- "new conversations · 16 → 2" legend uses an arrow glyph without text semantics.
- The KPI zeros are indistinguishable from missing data non-visually.

## 7. VERDICT

- **[BLOCKER]** Chart has no axes, ticks, or hover values — 2-point line is unreadable and unverifiable
- **[MAJOR]** Chart line clipped at plot edges (left start point cut off, right end touching border)
- **[MAJOR] ~40% of the page below the chart is dead space; content ends abruptly with no follow-on sections
- **[MAJOR]** KPI card rows use only half the content width; right half blank, layout unbalanced
- **[MAJOR]** Zero-valued metrics (Replies sent, ratings) not distinguished between "no data" and "true zero"; misleading
- **[MAJOR]** Date-range selector has no container/affordance; active state (30 days) barely distinguishable
- **[MINOR]** Floating tab group labels don't align with or visually bind to their tab groups
- **[MINOR]** Chart legend "new conversations · 16 → 2" is lowercase, tiny, stylistically inconsistent
- **[MINOR]** "Not-good ratings" / "Okay ratings" naming inconsistent with sibling card terminology
- **[MINOR]** Footnote (webhook/Plus-Pro limitation) too subtle for its informational importance
- **[MINOR]** Tabs lack disabled/empty-state indication; users can't tell which reports have data
- **[POLISH]** "Sync now" lacks in-context feedback on the Reports page itself
- **[POLISH]** Chart height disproportionate for the data volume; consider bars for sparse daily data