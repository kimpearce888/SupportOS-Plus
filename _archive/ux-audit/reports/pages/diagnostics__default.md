PAGE: Diagnostics

1. CONTENT INVENTORY
- Page header "Diagnostics" with two-line subtitle describing what the page records.
- Card: "Event log" with count line "88 event(s) shown — 36 error(s), 1 warning(s)".
- Filter tab row: all / errors / warnings / info (pill style, "all" active).
- Right-aligned buttons: "refresh" (secondary) and "clear log" (red destructive).
- Event log list: 9 visible rows, each = timestamp, blue status dot, "sync" chip, message "Incremental sync finished: 42 records processed, 0 resource(s) failed".
- Card: "Audit trail" — "0 audited change(s) shown — newest first" plus an empty-state sentence.
- Card: "Sync reports" — subtitle, table with columns run / kind / state / records / errors / started, rows 51–43 visible; state chips INITIALIZING / LIVE; a trailing numeric column (0) hugging the right edge; bottom row clipped by viewport.
- Top bar (shared): "synced 20s ago" chip, Search field, help icon.
- Scrollbar on far right.

2. LAYOUT DEFECTS
- Sync reports table's last numeric column (0) is jammed against the card's right edge with no padding, visually detached from headers ("errors" values appear both mid-table and far-right — unclear column mapping).
- Sync reports card runs off the bottom of the viewport with a row half-clipped (row 42) — no visual affordance that the card continues.
- Audit trail card is ~60% width while Event log and Sync reports cards are full width — inconsistent card widths, large dead space to its right.
- Event log list rows end at ~840px; the card is ~1660px wide, leaving a large empty right half of every row — content does not use available width.
- Large vertical gap between subtitle and Event log card vs tighter gaps elsewhere; uneven vertical rhythm between the three cards.
- Filter tabs sit on the left, refresh/clear on the far right of a very wide card — action cluster feels disconnected from the list it acts on.

3. VISUAL/TYPOGRAPHY DEFECTS
- Button labels lowercase ("refresh", "clear log") vs Title Case headings ("Event log") — inconsistent casing; also vs sidebar "switch to dark theme" lowercase.
- Event-log dot colors: rows shown are all blue/success-looking, but the header claims 36 errors and 1 warning — no visible error/warning rows or red/amber coloring in view; status semantics unclear.
- The "sync" chip is repeated identically 9 times with no informational value; kind is also a table column in Sync reports — redundant terminology.
- Count strings use "(s)" programmer notation ("event(s)", "error(s)") — should be pluralized properly.
- Timestamps use ISO format in the log and mixed ISO-8601 with "T" in the INITIALIZING row ("2026-11-05T23:17:27Z") vs space-separated in others — inconsistent date formatting.
- Weak hierarchy: subtitle text nearly same visual weight as card count lines.

4. COMPONENT DEFECTS
- Filter tabs (all/errors/warnings/info) have small hit areas and no visible count badges, though counts are known (36/1).
- "clear log" is destructive but placed directly adjacent to "refresh" — no separation or confirmation affordance visible.
- Event log rows show no hover state, no expand/copy affordance, and no indication of clickability (or confirmation they're inert).
- The trailing "0" column in Sync reports has no header visible/aligned, tiny click target if it's meant to be interactive.
- No pagination, "load more", or row-limit indicator on the Event log (88 shown, 9 visible).

5. UX DEFECTS
- Header claims 36 errors / 1 warning, yet the default "all" view shows only uniform success sync messages — either the filter is wrong, the counts are wrong, or errors are below the fold with no sort control; user cannot reach the errors they were told about.
- No way to jump to errors (no "36 errors" link) — dead-end count text.
- Audit trail empty state is a single flat sentence with no illustration, no "what triggers audit entries" CTA, no action to generate/verify.
- "INITIALIZING" run 51 has no progress indicator or elapsed time; user can't tell if it's stuck.
- No export/copy of log content despite it being a diagnostics page.
- "refresh" gives no feedback about last refresh (the "synced 20s ago" chip is in a different, unconnected location).

6. ACCESSIBILITY
- Status conveyed by colored dots alone (color-only, no icon/text/aria-label visible).
- Small gray count/subtitle text appears low contrast against white.
- "clear log" red-on-white may fall below 4.5:1 for its small text.
- Chip "sync" carries meaning only via color/context; screen reader gets "sync sync sync" per row.
- The detached right-edge "0" column will be read without association to its row meaning.

7. VERDICT
- [SEVERITY: major] Error/warning counts (36/1) not reflected in the visible log — no path to the errors.
- [SEVERITY: major] Sync reports trailing column detached at card edge with no header alignment.
- [SEVERITY: major] Sync reports bottom row clipped by viewport with no scroll affordance.
- [SEVERITY: major] No pagination/load-more on an 88-event log showing 9 rows.
- [SEVERITY: minor] Audit trail card width inconsistent with siblings; dead space beside it.
- [SEVERITY: minor] Event log rows use <50% of card width; right half dead.
- [SEVERITY: minor] Color-only status dots; no accessible status text.
- [SEVERITY: minor] Destructive "clear log" adjacent to "refresh" without separation/confirmation.
- [SEVERITY: minor] Inconsistent casing (lowercase buttons vs Title Case headings) and mixed timestamp formats (space vs "T…Z").
- [SEVERITY: minor] "(s)" pluralization in count strings.
- [SEVERITY: minor] INITIALIZING row lacks progress/elapsed feedback.
- [SEVERITY: polish] Filter tabs lack count badges and have small hit targets.
- [SEVERITY: polish] Redundant "sync" chip on every row adds no information.
- [SEVERITY: polish] Audit trail empty state lacks illustration/CTA.