PAGE: Dashboard

1. CONTENT INVENTORY
- Page title "Dashboard" with subtitle "Local metrics · sources labeled Help Scout / local / AI-derived everywhere".
- "Sync now" button (top right of content area).
- Filter card with three chip rows: Range (7 days / 30 days* / 90 days / 1 year), Channel (All channels* / Email / Chat (Beacon)), Mailboxes (All* / Webinars Today).
- KPI card grid: New 18, Active 13, Pending 2, Closed 2 / Unassigned 14, Backlog (7d stale) 15, Replies sent 0, First response avg 0 min / Great ratings 0, Okay ratings 0, Not-good ratings 0, Resolution avg 6 min.
- Line/area chart card "New conversations per day" with range "2026-10-07 → 2026-11-06 · 2 points", single purple area, x-label "2026-10-07", caption "new conversations · 16 → 2".
- "By mailbox" horizontal bar card (Webinars Today — 18).
- "By tag" bar card (welcome, practice, import bars) — clipped at right edge and bottom.

2. LAYOUT DEFECTS
- The last KPI column (Closed / First response avg / Resolution avg) overflows the right viewport edge — the 4th-column cards are cut off and First-response/Resolution cards visibly extend past the scrollbar area. [see verdict]
- "By tag" card is cut off on the right and clipped at the bottom of the viewport with no scroll affordance visible; last tag row is half-visible.
- KPI row 2 and 3 have misaligned heights: "First response avg" and "Resolution avg" cards are taller (extra "min" unit line) than their row siblings, breaking the grid baseline.
- Large dead space to the right of the filter card (card is fixed-width ~360px; the rest of the header row is empty).
- Chart x-axis shows only one label ("2026-10-07") despite spanning a month; a white band overlaps the bottom of the chart near the axis (label strip overlaps the plot area).
- Chart caption "new conversations · 16 → 2" sits outside the card, orphaned below it.
- Chart is only "2 points" for a 30-day range — the area chart renders as one giant meaningless diagonal.
- "By mailbox" bar card ends ~x=1550 leaving a wide empty gutter before the clipped "By tag" card; widths of the two cards don't sum to available width.

3. VISUAL/TYPOGRAPHY DEFECTS
- Inconsistent casing in chart card: title is Title Case ("New conversations per day") but the caption is lowercase ("new conversations · 16 → 2").
- "Backlog (7d stale)" uses parenthetical jargon inconsistent with plain labels elsewhere ("Not-good ratings" is hyphenated, awkward).
- KPI value and unit ("min") are visually detached — unit appears as a floating second line on two cards only, not on others that could carry units.
- Subtitle jargon ("sources labeled Help Scout / local / AI-derived everywhere") is developer-facing, not user-facing copy.
- Filter labels (Range/Channel/Mailboxes) are plain gray text at same weight as chip text — weak label/control distinction.

4. COMPONENT DEFECTS
- "Sync now" button and chips have no visible hover/focus/disabled indication in the static shot; chips' selected state is subtle (light gray fill) — low affordance.
- "Syncing…" pill in top bar is plain text, not a progress component; no progress feedback tied to "Sync now".
- KPI cards are not obviously clickable (if they link to filtered views, no affordance; if not, they're dead ends).
- Chart has no tooltip/interaction affordances visible; no y-axis gridline labels except clipped "16".

5. UX DEFECTS
- All-zero KPIs (Replies sent 0, ratings 0,0,0) presented identically to real data — no empty-state explanation or "no data in range" messaging.
- "First response avg 0 min" is ambiguous: is 0 real, or missing data?
- Filter defaults (30 days) contradict the data shown ("2 points" over a month); no feedback that data is sparse.
- "Sync now" vs global "syncing…" state relationship unclear — user can't tell if sync already ran.
- No drill-down from any KPI/chart; dashboard is a read-only dead end.

6. ACCESSIBILITY
- Gray chip/label text on white is low contrast; subtitle gray is likely below 4.5:1.
- "min" units and the "16" axis label are tiny, low-contrast gray.
- Chart conveys data by color/area alone with no text alternative (only the caption "16 → 2" partially compensates).
- Click targets for chips are small (~24px tall).

7. VERDICT
- [SEVERITY: blocker] 4th KPI column and "By tag" card overflow/clipped at right and bottom edges — content unreachable.
- [SEVERITY: major] 30-day chart rendered from 2 points is misleading/meaningless.
- [SEVERITY: major] Zero-value KPIs shown without empty-data explanation.
- [SEVERITY: major] Misaligned KPI card heights break grid alignment (unit rows on 2 cards only).
- [SEVERITY: minor] Orphaned chart caption outside its card; casing inconsistency.
- [SEVERITY: minor] Chart axis: single x label, overlapping label strip, clipped y label.
- [SEVERITY: minor] Filter card leaves large dead space; "By mailbox"/"By tag" widths don't fill row.
- [SEVERITY: minor] Developer jargon in subtitle and "(7d stale)" label.
- [SEVERITY: minor] No drill-down/interaction from KPIs or chart (dashboard dead end).
- [SEVERITY: polish] Low-contrast gray text, small chip targets, weak selected-state affordance.
- [SEVERITY: polish] "First response avg 0 min" ambiguity (real zero vs no data).