# SupportOS+ — Outreach Page Audit (Main Content Only)

## 1. CONTENT INVENTORY
- Page title: "Outreach" (top-left of content area)
- Horizontal tab bar: "Campaigns" (active, underlined), "Segments", "Do not contact"
- Primary button: "＋ New Campaign" (dark filled, below tab bar, left-aligned)
- Empty-state icon: megaphone/send icon, centered
- Empty-state heading: "No campaigns" (centered, gray)
- Empty-state helper text: "Create a campaign to reach a saved segment." (centered, smaller gray)
- No table, list, cards, chips, forms, charts, or secondary empty-state CTA present.

## 2. LAYOUT DEFECTS
- Empty state is vertically centered in the *entire* remaining viewport (~700px of dead space between "New Campaign" button and empty state, and below it), making the page feel hollow rather than intentionally composed.
- Empty state block is horizontally centered on the full content width while the "New Campaign" button is left-aligned — two competing alignment anchors with no visual connection.
- The helper text says "Create a campaign…" but the nearest actionable control ("New Campaign") is ~250px above-left of it, in the opposite corner; no CTA inside the empty state itself.
- Tabs sit close to the page title (fine) but the divider line under the tabs spans full width while the "New Campaign" row has a second full-width divider below it — two stacked rules create a sandwiched band with inconsistent inner padding.
- Right side of the header band is completely empty; no contextual actions (import, docs, filter) — dead space at top-right of the content area.

## 3. VISUAL / TYPOGRAPHY DEFECTS
- Empty-state heading and body are both muted gray with nearly identical tone; weak hierarchy — the heading doesn't read as a heading.
- "New Campaign" uses sentence case with "+" prefix; tab labels are sentence case — consistent — but "Do not contact" reads as a feature label while "Campaigns"/"Segments" are object labels; mixed taxonomy in one tab row.
- Helper text is small (~13px) and low-contrast gray on white; borderline legibility.
- Icon (megaphone) is thin-stroke and large but the same gray as text — no accent color to signal the empty state affordance.

## 4. COMPONENT DEFECTS
- "New Campaign" is the only actionable control; it's dark-filled so affordance is fine, but the empty state itself has **no button/link** — the helper text is not clickable and looks inert.
- Tabs: active tab indicated only by a thin underline; inactive tabs have weak contrast — hover/focus states unverifiable but underline-only active state is subtle at this scale.
- No visible loading state or skeleton; if campaigns are being fetched, the empty state could flash prematurely (no way to distinguish "empty" from "loading").

## 5. UX DEFECTS
- Empty state is a dead end: it describes the action ("Create a campaign to reach a saved segment") but doesn't perform it. The primary CTA should be *in* the empty state.
- Copy references "a saved segment" — a dependency on the Segments tab — but there's no link to Segments or hint of what to do if no segments exist yet. Likely a nested dead end.
- No secondary guidance: no "Learn about campaigns," sample/template offer, or documentation link for first-time users.
- No feedback that data is synced/local; on a local-first app, the "synced 11s ago" chip exists (top bar) but the empty state doesn't clarify whether campaigns failed to load vs. genuinely none exist.

## 6. ACCESSIBILITY
- "No campaigns" heading appears to be styled text; if not a real heading element, screen-reader users get no structure for the empty state.
- Gray-on-gray empty state text likely fails WCAG AA (≈ #9x gray on white).
- Tab focus indicators: underline-only active state offers no focus ring evidence — keyboard users may lose tab position.
- Empty-state icon is decorative but has no aria-hidden evidence; risk of it being announced with no label.
- Reading order: "New Campaign" button before empty-state text is acceptable, but the disconnect means SR users may hear helper text referencing an action they've already passed.

## 7. VERDICT — DEFECT LIST

| # | Severity | Defect |
|---|----------|--------|
| 1 | **[MAJOR]** | Empty state has no CTA — helper text says "Create a campaign" but the button is 250px away in the header; dead-end for first-run users |
| 2 | **[MAJOR]** | Copy references "a saved segment" with no link to Segments or guidance if none exist — nested dead end |
| 3 | **[MINOR]** | Weak hierarchy in empty state: heading and body nearly same gray/size, icon lacks accent |
| 4 | **[MINOR]** | ~700px of dead vertical space; empty state centered on full viewport instead of composed near content top |
| 5 | **[MINOR]** | Low-contrast helper text (~13px gray on white), borderline WCAG AA |
| 6 | **[MINOR]** | No loading vs. empty distinction — empty state may flash during data load |
| 7 | **[MINOR]** | No secondary affordances: no docs link, template, or import option in header or empty state |
| 8 | **[POLISH]** | Mixed taxonomy in tab row: "Do not contact" is a policy/feature label alongside object labels "Campaigns"/"Segments" |
| 9 | **[POLISH]** | Underline-only tab active state is subtle; unclear focus indicators for keyboard users |
| 10 | **[POLISH]** | Empty-state heading may not be a semantic heading; icon may not be aria-hidden |

**Overall:** Clean, functional skeleton but a weak first-run experience. The single biggest fix: move the CTA into the empty state (e.g., "Create your first campaign" button + "no segments yet? Start in Segments" link), tighten vertical centering, and strengthen text contrast/hierarchy. No blockers — the page renders correctly and its one control works — but the empty state underdelivers for a feature-dependent workflow.