# SupportOS+ — Incidents Page Audit (Main Content Area Only)

## 1. Content Inventory
- Page title "Incidents" (top-left of content area)
- Sub-count "0 incident(s)" directly under title
- Secondary count "0 incidents" floating top-right, next to primary CTA
- Primary button "+ New Incident" (top-right, dark)
- Filter row: dropdown "Any", dropdown "Any", checkbox "open only", text input "search incidents..."
- "apply" button (dark, far right of filter row)
- Empty-state icon (triangle warning glyph)
- Empty-state heading "No incidents match"
- Empty-state helper text "Adjust the filters or create a new incident."
- Top bar elements: "synced 4s ago" chip, global "Search ⌘K" input, bell icon (treated as context)

## 2. Layout Defects
- "apply" button is separated from the rest of the filter row by ~1000px of dead space — it appears orphaned at the far right edge rather than grouped with the filters it applies. (x≈1839–1897, y≈150)
- Duplicate/competing counts: "0 incident(s)" under the title (left) and "0 incidents" right-aligned next to New Incident — redundant and inconsistently phrased, both crammed around the CTA.
- "0 incidents" text sits immediately left of "+ New Incident" with almost no gap — reads as a label attached to the button.
- Filter row controls are left-clustered (ends at x≈798) with no visual grouping/labels; the row occupies ~40% width, leaving the middle of the row empty before the orphaned apply button.
- Empty state is vertically centered but content area is mostly whitespace below y≈330 — acceptable for an empty state, but the warning icon, heading, and helper text feel slightly sparse/floaty with no card or container.

## 3. Visual / Typography Defects
- Inconsistent casing: "search incidents..." placeholder is lowercase, "open only" lowercase, while "No incidents match", "New Incident", "Incidents" are title case. No consistent convention.
- "apply" button lowercase while "+ New Incident" is title case — inconsistent button labeling on the same screen.
- "(s)" pluralization hack: "0 incident(s)" is developer string leaking into UI; should be "0 incidents" (which is already used elsewhere on the same page — inconsistent).
- Empty-state heading appears in a lighter gray than the helper text contrast allows — weak hierarchy between icon, heading, and body.
- Filter dropdowns both read "Any" with no field labels — the two identical dropdowns are indistinguishable (presumably status and priority/severity, but the UI doesn't say).

## 4. Component Defects
- Unlabeled dropdowns ("Any" / "Any") — no visible or accessible labels; users can't tell what each filters.
- "open only" checkbox is a bare checkbox+label with no group/label context and a small click target (~14px checkbox).
- No visible "clear filters" affordance — the empty state says "Adjust the filters" but the filter row offers no reset button, and the applied filters aren't shown as active (dropdowns just say "Any", so what filter is active?).
- "apply" is a manual-apply pattern while the search input implies live filtering — mixed filter models without feedback.
- Dark primary buttons ("apply", "+ New Incident") use lowercase black pills that don't visually match the outlined/ghost controls elsewhere — two different button systems on one screen.
- No loading/disabled/pending state visible for "apply" — no feedback that a filter query ran.

## 5. UX Defects
- Contradiction: header says "0 incidents" and dropdowns say "Any" (no filters), yet empty state says "No incidents match" and "Adjust the filters" — if zero filters are set, this is not a filter-mismatch state, it's a truly empty workspace. The empty state is telling the user to fix filters that aren't set. (Or the "open only"/search defaults are hidden — either way the state explanation is wrong/unclear.)
- Empty state offers no "Clear filters" action button — only the passive text "Adjust the filters"; user must hunt for the apply button 1000px away.
- No CTA in the empty state itself; the "New Incident" button is ~70px up and to the right — an inline empty-state CTA is standard and missing.
- Manual "apply" pattern without any indication that filters are pending/unapplied — users will change a dropdown and expect instant results.
- Duplicate counts ("0 incident(s)" vs "0 incidents") give no additional information and add noise.

## 6. Accessibility
- Icon-only bell button in top bar with no visible label/tooltip — unknown purpose.
- Unlabeled form controls: two dropdowns and the search input rely on placeholder text only; screen readers get "Any", "Any".
- Placeholder "search incidents..." is likely low-contrast gray on white (appears ~#9x gray) — placeholder-as-label antipattern.
- Empty-state heading appears light gray — likely below 4.5:1 contrast for text of its size.
- "synced 4s ago" chip: green text on light background — borderline low contrast.
- Checkbox "open only" click target below recommended 24×24px minimum.
- Reading order: "apply" button is 1000px from its filter group; visual and logical grouping diverge.

## 7. Verdict

| # | Severity | Defect |
|---|----------|--------|
| 1 | **[MAJOR]** | Empty-state message contradicts visible state: "No incidents match / adjust the filters" shown with zero filters applied and count "0 incidents" |
| 2 | **[MAJOR]** | Two unlabeled "Any" dropdowns — indistinguishable filter controls |
| 3 | **[MAJOR]** | "apply" button orphaned ~1000px from its filter group at far-right edge |
| 4 | **[MAJOR]** | No "clear filters" action anywhere despite empty state instructing users to adjust filters |
| 5 | **[MINOR]** | Duplicate inconsistent counts: "0 incident(s)" (left) vs "0 incidents" (right) |
| 6 | **[MINOR]** | "(s)" pluralization hack leaking developer strings into UI |
| 7 | **[MINOR]** | Mixed casing conventions: lowercase "apply"/"open only"/placeholder vs title-case buttons/headings |
| 8 | **[MINOR]** | Empty state lacks inline primary CTA ("New Incident" not repeated in empty state) |
| 9 | **[MINOR]** | Manual-apply filter model mixed with live search, with no pending/unapplied-feedback state |
| 10 | **[MINOR]** | Unlabeled form controls (dropdowns, search) — placeholder-only labeling harms screen-reader users |
| 11 | **[POLISH]** | "open only" checkbox click target below 24px minimum |
| 12 | **[POLISH]** | Empty-state heading/gray text likely below 4.5:1 contrast; "synced" chip borderline |
| 13 | **[POLISH]** | Icon-only bell button without visible label |
| 14 | **[POLISH]** | Two competing button styles (dark pill vs ghost/outline) on one screen |

**Overall:** The page is a clean empty state but functionally confused — the biggest issue is the mismatch between what the UI claims (filters set) and what it shows (no filters), plus unlabeled filters and a detached apply button. Fix the empty-state logic, label the filters, and group the apply control before ship.