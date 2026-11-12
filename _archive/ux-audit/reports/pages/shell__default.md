PAGE: Shell

1. CONTENT INVENTORY
- Page header: "Shell" (large title, top-left of content area)
- Subtitle: "native shell diagnostics (Phase 2 surface)" in small gray text
- "switch to dark" button, top-right of content area (secondary/outline style)
- Card "Runtime" with 5 key–value rows: database (long file path in monospace), schema v54, background events 1439, last event sync, sync engine idle
- Caption/footnote line beneath the card: "all pages ported: dashboard, inbox, operations, customers, organizations, knowledge/docs, reports, issue radar, incidents, custom objects, automation, AI center, outreach, notifications, search, support graph, connectors, sync health, settings, onboarding — stale checklist — omits live chats"
- Top app-bar items within content area scope: "synced 25s ago" chip (green pill), "Search ⌘K" input/button, bell icon button
- Remaining ~65% of content area: empty

2. LAYOUT DEFECTS
- Runtime card occupies only ~30% of available width (~610px of ~1700px); vast dead space to its right.
- Whole page content occupies top ~400px of a 1080px viewport; ~600px of vertical dead space below the footnote line.
- Footnote line runs nearly the full content width as a single unwrapped run-on line — hard to scan; no wrapping/bullets.
- Card top edge nearly touches the subtitle with no section spacing rhythm; subtitle-to-card gap is tight relative to the large empty area below.
- Footnote sits flush-left with card but has no container/separator — floats ambiguously between card and nothing.

3. VISUAL/TYPOGRAPHY DEFECTS
- Subtitle is all-lowercase "(Phase 2 surface)" — developer jargon leaking into UI; inconsistent casing vs. title-case "Runtime".
- Footnote lowercase throughout, uses em-dashes inconsistently ("— stale checklist —") with unclear meaning.
- Label column ("database", "schema", etc.) lowercase gray; values monospace but mixed alignment baseline looks slightly off for "background events" row (long label shifts value column).
- The key–value column widths appear fixed so the long label "background events" pushes close to its value; values are not column-aligned consistently (1439 vs v54 vs sync alignment fine but label column width ad hoc).
- Weak hierarchy: title "Shell" is the only strong element; card header "Runtime" is nearly same size as body text.
- Footnote gray-on-white small text likely below AA contrast at that size.

4. COMPONENT DEFECTS
- "switch to dark" button: lowercase label, inconsistent with typical capitalized button labels; also duplicated by "switch to dark theme" in sidebar (out of scope but redundant affordance).
- The long database path in monospace has no copy-to-clipboard affordance — the primary interaction users want on a path.
- "sync engine: idle" and "last event: sync" are plain text with no status dot/badge — status affordance missing, though a "synced 25s ago" chip exists elsewhere (inconsistent status presentation).
- "synced 25s ago" chip is very small click/target-less; unclear if refreshable.
- No refresh action visible on the Runtime card — stale-looking diagnostics with no way to re-fetch.

5. UX DEFECTS
- Page has no clear purpose from the user's perspective: a card of 5 runtime fields + a dev-style checklist footnote. No actions, no navigation to the mentioned pages, no links.
- Footnote "stale checklist — omits live chats" is an unresolved internal TODO shipped to the UI; confusing and undermines trust.
- "background events: 1439" — raw number with no context (is that good? clickable?).
- No empty state, loading state, or error state for the page; nothing below the card explains the emptiness.
- "Phase 2 surface" subtitle tells users the page is unfinished.

6. ACCESSIBILITY
- Footnote gray text: likely low contrast (light gray on white at small size).
- Bell icon top-right is icon-only with no visible tooltip/label.
- ⌘K hint only; no visible alternative for keyboard-less discovery (minor).
- Status conveyed by text color/absence ("idle") without icon — okay but color-only status conventions elsewhere create inconsistency.
- Reading order: "switch to dark" appears before the page title in DOM/visual scan (top-right vs top-left), minor.

7. VERDICT
- [MAJOR] Page is ~70% empty; no content, actions, or explanation fills the space.
- [MAJOR] Internal TODO/stale-checklist text exposed to end users.
- [MAJOR] No copy affordance or action on the database path — the page's only concrete datum is inert.
- [MAJOR] No refresh/update mechanism on runtime diagnostics.
- [MINOR] Runtime card fixed narrow width, not using available layout width.
- [MINOR] Developer subtitle "native shell diagnostics (Phase 2 surface)" with jargon and casing inconsistency.
- [MINOR] Run-on single-line footnote, unscannable, inconsistent em-dash usage.
- [MINOR] Redundant/conflicting theme toggle ("switch to dark" button vs sidebar item) with lowercase button label.
- [MINOR] Status values ("idle", "sync") lack badge/dot affordance; inconsistent with "synced 25s ago" chip.
- [MINOR] Low-contrast footnote text.
- [MINOR] Icon-only bell button without accessible label.
- [POLISH] Weak heading hierarchy — "Runtime" card header barely larger than body.
- [POLISH] "background events" raw number lacks context or drill-down.