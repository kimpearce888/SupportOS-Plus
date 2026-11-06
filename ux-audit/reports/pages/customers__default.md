# SupportOS+ — 'Customers' Page Audit (main content area only, 1920×1080)

## 1. CONTENT INVENTORY
- Page title "Customers" (top-left of content area)
- Right-aligned metadata text "7 customers (of 7)" + "⟳ Sync now" button (top-right)
- Search input with placeholder "Search customers by name or email…" (left, above table)
- "search" button (small dark pill, far right of search row)
- Table with columns: NAME, EMAIL, CONVERSATIONS, LAST CONTACT (+ unlabeled action column)
- 7 customer rows: Maya Rodriguez, Practice Customer, Help Scout, Liam Carter, Ethan Walsh, Priya Sharma, Tom Becker
- Every row has "—" in both CONVERSATIONS and LAST CONTACT columns
- Every row has a small "open" button in the trailing column
- No pagination, no filters, no bulk actions, no empty state (table is populated)

## 2. LAYOUT DEFECTS
- Table card is ~440px wide in a ~1750px content area; ~75% horizontal dead space to the right of the table.
- Search input is fixed-width (~285px) and stranded far left, while its "search" button is pushed to the far right edge of the window — they read as unrelated elements; the button is not visually attached to the input it triggers.
- "Sync now" and "7 customers (of 7)" sit at the extreme right edge with only ~8–16px margin; "Sync now" nearly touches the window edge.
- CONVERSATIONS and LAST CONTACT columns are extremely narrow and both render only "—" — the column widths don't match the (empty) content; trailing "open" button is crammed against the card's right border.
- Header row background highlight (light grey band) only extends across the first three columns, stopping before LAST CONTACT — the header band is visually truncated.
- Header column labels don't align with their data (e.g., "EMAIL" header sits left of the email text which is indented to a different x-position).

## 3. VISUAL / TYPOGRAPHY DEFECTS
- "search" button label is lowercase while "Sync now" and all headers are capitalized — inconsistent casing; also lowercase "open" in every row.
- Header labels are tiny uppercase with wide letter-spacing at very low contrast vs the page title's large serif-ish style — hierarchy jump is abrupt.
- The "7 customers (of 7)" count is small, low-contrast grey, and easy to miss.
- "open" repeated 7 times adds visual noise; a row-level affordance would be cleaner.

## 4. COMPONENT DEFECTS
- "open" buttons are ~50×26px — below comfortable click-target size, and there are 7 of them with no hover/selected row indication visible.
- Search button styling (dark filled pill) differs from "Sync now" (outlined) with no clear primary/secondary logic; a table filter search rarely warrants the strongest button on the page.
- No visible table row hover/selected states, no sorting affordances on column headers, no pagination controls.
- "—" placeholders in two columns for all rows give no affordance or explanation (no tooltip, no "no data" styling).
- The top-right icon-only button (bell/bell-slash?) next to "Search ⌘K" is in the audited strip's corner — icon-only, no tooltip.

## 5. UX DEFECTS
- Two competing search mechanisms: a dedicated customer search field plus a global "Search ⌘K" — relationship unclear; the dedicated one requires clicking "search" instead of filtering as-you-type.
- Every CONVERSATIONS and LAST CONTACT value is empty: either the data pipeline is broken or columns are useless — but no feedback/explanation is offered. Showing "0" and "never" would be more informative than "—".
- "7 customers (of 7)" implies a filtered total vs. a full total, but nothing is filtered — the "(of 7)" phrasing is confusing when they're equal.
- Row action is "open" but it's unclear what opens (detail pane? new view?) — no preview affordance.
- No way to add a customer, no visible sort, no way to jump to a customer other than scrolling a 7-row list (fine now, won't scale).
- Large empty region under the table suggests missing content (stats, recent activity) rather than intentional design.

## 6. ACCESSIBILITY
- Header labels and "—" placeholders are very light grey on white — likely failing WCAG AA (≈3:1 or worse for small text).
- "7 customers (of 7)" and "synced 7s ago"-style micro-text are low contrast.
- "open" ×7 are repeated identical button labels with no accessible name referencing the customer (screen reader hears "open, open, open…").
- Click targets ~26px tall < 24×24px minimum, and well under 44px recommended.
- Table semantics unclear: header band truncation suggests the header row isn't properly associated with all columns.

## 7. VERDICT
- [MAJOR] Table occupies ~25% of available width; massive dead space, layout looks broken at 1920px.
- [MAJOR] All CONVERSATIONS and LAST CONTACT values are empty "—" with no explanation or fallback content.
- [MAJOR] Search input and its "search" button are separated by ~1200px of empty space; button ownership is ambiguous.
- [MAJOR] Low-contrast table headers and placeholder text likely fail WCAG AA.
- [MINOR] Repeated "open" buttons lack customer-specific accessible names and are undersized click targets.
- [MINOR] Inconsistent casing: "search"/"open" lowercase vs "Sync now"/"Customers".
- [MINOR] Header background band stops before LAST CONTACT; header labels misaligned with column data.
- [MINOR] No sorting, filtering, pagination, or add-customer affordance on a customer list view.
- [MINOR] "7 customers (of 7)" phrasing confusing when no filter is applied.
- [POLISH] "Sync now" and count crammed against right window edge; uneven margins vs. left side.
- [POLISH] Inconsistent button styles (dark pill vs outlined) without clear primary/secondary hierarchy.