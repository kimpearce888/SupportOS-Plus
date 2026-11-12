PAGE: Support Graph

## 1. CONTENT INVENTORY
- Page header: "Support Graph" (large, top-left of content area)
- Subtitle/metrics line: "0 conversation links · 0 customer links · 0 human edges" (small gray text)
- Right-aligned counter: "0 nodes" (top-right corner)
- Search input: "Search the graph…" (top-left, below header, ~320px wide)
- Primary button: "search" (black pill, immediately right of input)
- Empty-state icon: share/graph glyph (centered)
- Empty-state title: "Search the graph"
- Empty-state body: "Search customers, conversations, organizations, users, or incidents by name, subject, or number."
- (Top bar: "syncing…" pill, global "Search ⌘K" field, help icon — app chrome, noted only in passing)

## 2. LAYOUT DEFECTS
- Search bar is left-aligned in a ~1700px-wide row; the empty state is centered — the two centers conflict; search row should be centered or full-width with a visible boundary.
- Search input is fixed ~320px wide; with only 1–2 words typed there is ample room, but the row leaves ~1200px of dead space to its right with no other action (no filters, no "browse all nodes").
- Large dead zone: between the search row (ends ~y=170) and the empty-state block (starts ~y=190 at icon, text ends ~y=300) — everything below y=310 to y=1080 is empty canvas; vertically the empty state is NOT centered in remaining space, it hugs the top, leaving ~700px of dead area below.
- "0 nodes" counter floats top-right with no visual anchor (no border, card, or label like "Graph stats"); it can read as orphaned UI noise.
- Metrics line ("0 conversation links · 0 customer links · 0 human edges") uses middot separators at very small size; sits close under the H1 with tighter spacing than the gap to the search row — spacing rhythm is uneven (header→subtitle ~8px, subtitle→search ~30px).

## 3. VISUAL/TYPOGRAPHY DEFECTS
- Button label "search" is lowercase while every other label in the UI ("Search", "Support Graph", empty-state text) is sentence/title case — casing inconsistency.
- Empty-state body text is low-contrast gray and small; it is the most informative text on the page but reads as tertiary.
- Metrics line vs "0 nodes": two different places express node/link counts, with slightly different formats ("0 conversation links" vs "0 nodes") — inconsistent terminology for the same data.
- Weak hierarchy: nothing visually groups the search controls with the empty state they drive; input has a visible border but the button is a filled black pill — mismatched visual weight in one row.
- Placeholder "Search the graph…" duplicates the empty-state title "Search the graph" verbatim — redundant copy.

## 4. COMPONENT DEFECTS
- Button label "search" lowercase, appears to be an unstyled/default egui button — inconsistent with the app's styled pill/chip components ("syncing…" pill).
- No clear/escape affordance in the search field (no ✕ to clear), no visible shortcut hint (global search has ⌘K hint, this one doesn't).
- No visible focus/disabled/loading states defined for the search control; when "syncing…" is active it's unclear whether search is even usable, and nothing disables or communicates that.
- "0 nodes" is plain text, not a chip/badge — inconsistent with how counts are typically shown elsewhere.
- Search input click target is fine, but the whole top-right ("syncing…", help icon) and bottom-right ✕ have tiny targets and no labels.

## 5. UX DEFECTS
- Circular/tautological empty state: title says "Search the graph" directly above an already-visible search field — it instructs the user to do the thing they're looking at.
- No path forward when the graph is empty: no "Import data", "Connect a source", or "Open connectors" CTA, even though a Connectors page exists in the sidebar. The empty state is a dead end.
- Zero-state metrics ("0 conversation links · 0 customer links · 0 human edges") provide no value before data exists; they read as debug output.
- No feedback loop between the search field and empty state: unclear whether typing filters the graph below or shows a results list; the page shows only "Search the graph" regardless.
- Relationship between this page's search and the global "Search ⌘K" is unexplained — two searches on one screen.

## 6. ACCESSIBILITY
- Empty-state body text and metrics line appear to be light gray on near-white — likely below 4.5:1 contrast.
- Help icon (top-right) and bottom-right ✕ are icon-only with no visible labels or tooltips captured.
- "syncing…" status is conveyed by color/animation on a small pill; screen readers may not announce it; no non-visual status alternative.
- Empty-state icon is purely decorative but has no `aria-hidden` equivalent indication (egui default); reading order starts with header→subtitle→search→icon→title→body, which is acceptable, but the "0 nodes" counter at far right may be read last despite being a headline stat.

## 7. VERDICT
- [major] Empty state is a dead end — no CTA to import/connect data (e.g., Connectors) when graph is empty.
- [major] Large dead space below empty state; block not vertically centered in available canvas (~700px unused).
- [minor] Button casing inconsistent ("search" vs sentence case elsewhere).
- [minor] Tautological empty-state copy "Search the graph" duplicates placeholder and the visible input.
- [minor] Conflicting/duplicate count displays ("0 conversation links…" vs "0 nodes") with inconsistent terminology.
- [minor] Search row left-aligned while empty state is centered — misaligned composition.
- [minor] Low-contrast gray body/metrics text likely fails WCAG AA.
- [minor] No clear affordance, shortcut hint, or syncing-state feedback in the search field.
- [polish] "0 nodes" floats as unstyled orphan text; should be a labeled chip/stat.
- [polish] Redundant placeholder/empty-state title wording.