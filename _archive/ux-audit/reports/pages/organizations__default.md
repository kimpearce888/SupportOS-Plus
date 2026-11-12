# SupportOS+ — Organizations Page Audit (Main Content Area Only)

## 1. CONTENT INVENTORY
- Page title "Organizations" with subtitle "customer organizations, their domains, and their members"
- Right-aligned count "6 organizations" and a "⟳ Sync now" outlined button
- "Filter by name…" text input (top-left of content)
- Black "filter" button (top-right of content)
- Table/card with column headers: ORGANIZATION / DOMAINS / CUSTOMERS / CONVERSATIONS (uppercase)
- 6 rows: Acme Analytics, Globex Media, Help Scout, Initech Solutions, Internet Assigned Numbers Authority… (truncated), Northwind Traders
- Each row: an "open" button; some rows show a domain value in monospace (helpscout.com, example.com); a "0" value under/near CONVERSATIONS; Domains/Customers columns appear empty for most rows
- "New organization" form card: lowercase labels "name" / "domains", inputs "Name" and placeholder "acme.com, acme.io", black "create organization" button
- Large empty region below the form (~40% of viewport height)

## 2. LAYOUT DEFECTS
- **Table is compressed to ~40% of available width** while a vast empty area sits to its right and below the page — the whole layout is left-packed with a huge dead zone bottom-right (from ~y700 to 1080).
- **Domains column inconsistent**: only 2 of 6 rows show a domain; rows without a value have a large blank gap where the domain should be — no "—" or placeholder, and column positions for the monospace domains don't align to a stable column (helpscout.com and example.com start at different x than empty rows imply).
- **"0" conversation counts appear as bare floating numerals** near the right edge of the table, not aligned under the CONVERSATIONS header; DOMAINS and CUSTOMERS header columns appear to have no data beneath them at all.
- **Filter input is narrow (~260px) while a "filter" button floats far away on the right edge** — no visual association; unclear what the button does distinct from the input.
- "Internet Assigned Numbers Authority…" is truncated with "…" but the full name is nowhere (no tooltip evident), and truncation is applied to only one row — a symptom of the table not reserving proper column width.
- The "New organization" card's inputs are right-aligned in the far corner while labels sit far left — label/field distance ~1300px, severe F-pattern break.
- Filter bar, table, and form have inconsistent horizontal extents (filter row full-width, table ~500px, form ~1660px).

## 3. VISUAL / TYPOGRAPHY DEFECTS
- Case inconsistency: "Open"? — header "New organization" is title case, button "create organization" is lowercase, "Sync now" title case, "filter" lowercase, "open" lowercase. Three casing styles on one screen.
- Column headers in tiny light-gray uppercase are nearly illegible and don't align with actual data columns.
- Domain values in monospace vs. sans everywhere else — acceptable as data styling but applied inconsistently (missing for 4 rows).
- Count "6 organizations" in small gray text floats detached from the title; hierarchy between it and "Sync now" is muddy.
- Form labels ("name", "domains") lowercase, gray, same size as placeholder text — labels and placeholders are visually confusable.

## 4. COMPONENT DEFECTS
- Six identical "open" buttons per row is redundant — the row itself should be clickable; tiny pill buttons are weak affordances for opening an entity.
- "filter" button appears to be a dark filled (primary-looking) button while "Sync now" is outlined and "create organization" is dark filled — filled/black styling used for both primary action (create) and a secondary toggle (filter), inconsistent.
- No visible hover/selected/disabled states on rows; no sort indicators on column headers.
- Filter input has no clear ("×") affordance, no result-count feedback when filtering.
- Domains input accepts comma-separated values ("acme.com, acme.io") — no chips/tag affordance or validation hint.
- No visible error/validation affordance on the create form.

## 5. UX DEFECTS
- Duplicate filter mechanisms (input + "filter" button) with no explanation of the button's purpose — likely a dead or mystery control.
- Empty DOMAINS/CUSTOMERS columns: either dead columns or data failing to render — either way users can't tell.
- Conversation counts all "0" — no empty-state guidance for what makes a count nonzero.
- No row-level actions beyond "open": no edit, no delete, no menu; "open" destination unknown from this view.
- Create form: no feedback about required fields, name uniqueness, or domain format; success/error feedback not visible.
- The lower half of the page is entirely dead space — content could be centered or the table could expand; currently reads as a rendering bug rather than intentional layout.
- "Sync now" next to "6 organizations" — no indication of last-sync time or what sync affects.

## 6. ACCESSIBILITY
- Column headers extremely low contrast (light gray on white, small caps) — likely fails 4.5:1.
- "6 organizations" gray-on-gray text is low contrast.
- Bare "0" numbers are meaningless without column context for screen readers (header/data association broken by misalignment).
- Truncated org name exposes no accessible full-text alternative.
- "open" buttons repeated 6× with identical labels — no distinguishing accessible names (e.g., "Open Acme Analytics").
- Large empty layout may disrupt logical reading order (filter → table → form → vast gap).

## 7. VERDICT

| # | Severity | Defect |
|---|----------|--------|
| 1 | **[major]** | Table occupies <40% of width; huge dead zone below and right of content |
| 2 | **[major]** | DOMAINS/CUSTOMERS columns empty for 4/6 rows — dead columns or data not rendering |
| 3 | **[major]** | "0" counts and domain values misaligned with their column headers; header/data association broken |
| 4 | **[major]** | Redundant/confusing filter UI: input on left, mystery "filter" button on far right |
| 5 | **[major]** | Create-form inputs right-aligned ~1300px from their labels — severe form layout failure |
| 6 | **[major]** | Identical "open" buttons without distinct accessible names or row click affordance |
| 7 | **[major]** | Inconsistent casing: "Sync now" / "create organization" / "filter" / "open" mix title, sentence, lowercase |
| 8 | **[minor]** | "Internet Assigned Numbers Authority…" truncated with no tooltip or accessible full text |
| 9 | **[minor]** | Missing "—" placeholders for empty table cells creates ragged, gappy rows |
| 10 | **[minor]** | Filter styling (dark filled) competes with true primary action "create organization" |
| 11 | **[minor]** | Low-contrast uppercase column headers likely fail WCAG AA |
| 12 | **[minor]** | No sort, hover, or selection affordances on table rows/headers |
| 13 | **[minor]** | Domains field: comma-separated input with no chip UI, validation hint, or error state |
| 14 | **[polish]** | Labels ("name", "domains") lowercase and visually indistinguishable from placeholders |
| 15 | **[polish]** | "6 organizations" count floats detached; add sync timestamp for context |
| 16 | **[polish]** | No empty-state guidance for zero conversation counts |

**Summary:** Not an empty state — a populated page rendered with broken structure. Top priorities: fix table column/data alignment, expand layout to use available width, move form inputs adjacent to labels, resolve the filter control confusion, and give rows proper click affordances with distinct accessible names.