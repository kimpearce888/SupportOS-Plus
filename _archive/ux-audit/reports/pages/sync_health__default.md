PAGE: Sync Health

1. CONTENT INVENTORY
- Page header "Sync Health" with subtitle "engine state, checkpoints, and recent runs"
- Secondary status line: "progress: threads · resource 16/16 · 42 record(s) so far"
- Wide status card: large "LIVE" label, "last success: 2026-11-05T23:21:57Z · never shows relative time — forensic value" text, and a green "LIVE" chip at far right
- "Engine" card with two dense mono lines: webhook queue stats and webhook health ("not_configured · checked 2026-11-05T22:45:22.710…+00:00 · no relay URL configured — the listener stays loopback-only until you run a tunnel")
- Button row: "Run initial sync" (filled dark), "Run incremental sync" (outline), "Run reconcile" (outline), plus a detached red "Cancel running sync" at far right
- "Checkpoints" card containing a mono table (resource, status, processed, failed, retries, last success) with 18 resource rows, all "ok"
- "Recent runs" card with a table header (id, kind, state, started, resources, records, errors) whose body is cut off by the viewport

2. LAYOUT DEFECTS
- Checkpoints table: massive horizontal gap between the "retries" column (ends ~x=735) and "last success" values pinned to the far right edge (~x=1880). The last-success column header, however, sits at ~x=740 — header and values are in completely different places.
- Engine card ends at ~x=1150, leaving ~750px of dead white space to its right while the LIVE card above spans full width — inconsistent card widths.
- Engine card text runs nearly edge-to-edge inside the card with no right padding buffer; the long webhook-health line almost touches the card border.
- "Cancel running sync" floats alone on the far right of the button row with no grouping rationale; ~1100px of empty space separates it from the run buttons.
- "Recent runs" table is clipped at the viewport bottom — only headers visible; no scroll affordance indicated.
- Checkpoints table right edge: values flush against the card border with essentially zero right padding.
- Uneven vertical rhythm: LIVE card → Engine card gap (~45px) vs Engine card → button row (~40px) vs button row → Checkpoints (~25px).

3. VISUAL/TYPOGRAPHY DEFECTS
- Debug/developer text shipped to UI: "never shows relative time — forensic value" is meaningless to end users.
- Raw machine timestamps ("2026-11-05T23:21:57Z", "2026-11-05T22:45:22.710428884+00:00" with 9-digit nanosecond precision) shown everywhere; no human-readable formatting.
- The progress line ("progress: threads · resource 16/16 · 42 record(s) so far") is very low-contrast small gray text — it's the live status yet is visually the weakest element.
- "LIVE" appears twice in the same card (large text label + chip) — redundant.
- Terminology inconsistency: "record(s) so far" vs table column "processed" vs "records" in Recent runs.
- Lowercase-first sentences mixed with Title Case card titles ("Engine", "Checkpoints", "Recent runs") — acceptable in a design system, but "not_configured" raw enum value leaks into UI text.

4. COMPONENT DEFECTS
- No enabled/visible pagination, sort, or filter on the checkpoints table.
- Buttons: "Run initial sync" is filled/dark (primary), but on a page where a sync is already running, three run buttons and a cancel button all appear enabled — no disabled/loading states visible.
- "Cancel running sync" styled as a solid destructive red — unclear whether it cancels the current run or is always active; no confirmation affordance implied.
- Table rows have no hover state visible; no row-level drill-in affordance for failures (all "ok", so unverifiable, but no error styling pattern is demonstrated).
- Green LIVE chip is a status chip with no tooltip or click affordance explaining what "live" means.

5. UX DEFECTS
- Running-sync state is communicated only by a tiny gray progress line and a red cancel button; no progress bar, percentage, or ETA despite "42 record(s) so far" implying live counts.
- "never shows relative time — forensic value" reads like a developer note; user can't tell if data is fresh.
- If a sync is running, offering "Run initial/incremental sync" simultaneously is a confusing/conflicting flow with no explanation of differences between the three run types.
- Recent runs table clipped with no visible scrollbar or "view all" — user can't tell how many runs exist.
- Checkpoints table with 18 identical "ok" rows gives no summary (e.g., "18/18 healthy"); users must scan all rows.
- Engine card webhook message mixes status, timestamp, and remediation advice in one run-on sentence; hard to parse.

6. ACCESSIBILITY
- "Cancel running sync" is the only red-on-white text on the page; red #c33-on-white is borderline contrast for small text (~4.0:1).
- Status conveyed by color alone (green LIVE chip, "ok" text) without icons — okay for text, but the chip pairs color+text so acceptable; the tiny gray progress line likely fails 4.5:1.
- Very long mono timestamp strings are hostile to screen readers (read as digit soup).
- Huge header/column misalignment in checkpoints table will confuse screen-reader column association.
- Icon-only element: none in main area (good), but the LIVE chip has no accessible name beyond "LIVE".

7. VERDICT
- [MAJOR] Checkpoints "last success" header/values misaligned — header at one x-position, values at the far screen edge
- [MAJOR] Recent runs table clipped at viewport with no scroll indication
- [MAJOR] Three "run sync" actions enabled concurrently with a running sync; no states or explanation
- [MAJOR] Developer/debug strings exposed in UI ("never shows relative time — forensic value", "not_configured", nanosecond timestamps)
- [MINOR] Engine card width inconsistent with full-width cards; large dead space to its right
- [MINOR] Cancel button stranded ~1100px from the run buttons it relates to
- [MINOR] Running-sync progress is a tiny low-contrast text line; no progress bar or ETA
- [MINOR] Duplicate "LIVE" indicator (large label + chip) in the same card
- [MINOR] Raw ISO timestamps with nanosecond precision throughout; no relative time
- [MINOR] Terminology inconsistency: record(s)/processed/records
- [POLISH] No checkpoints summary (e.g., "18/18 ok") forcing full-row scan
- [POLISH] Webhook health message is a run-on sentence mixing status, timestamp, and remediation
- [POLISH] Table values flush to card border; insufficient cell padding on right edge
- [POLISH] Cancel button red text contrast borderline at small size