PAGE: Connectors

**1. CONTENT INVENTORY**
- Page title "Connectors" with one-line description ("Attach external data sources (HTTP endpoints, CSV files inside the data directory) as read-only tables the AI tools can query them when allowed.").
- Header-right counter "0 connectors".
- Primary button "+ New Connector" (dark).
- Table header row: columns "name", "kind", "health", "refresh", "last sync" (lowercase).
- Large table panel (bordered box) that is empty except for a dev/placeholder note at the top-left: "A-5 (D5-0): the rows/refresh/delete triple collapsed to refresh + delete (danger + confirm); the connector chip opens the rows viewer below."
- No rows, no empty state, no charts, no forms.
- Top bar (main area): "synced 25s ago" chip, "Search ⌘K" button, a bell/theme icon button.

**2. LAYOUT DEFECTS**
- The table panel is ~850px tall and completely empty — massive dead space below the column headers; the box height appears fixed rather than content-sized.
- Column headers are left-aligned at x≈238 but there is no visible column structure/grid alignment across the empty area — can't verify alignment of name/kind/health/refresh/last sync widths.
- The dev note text sits inside the table box, jammed against the top border with no padding (y≈197, border at y≈192).
- No empty-state content centered in the panel; the entire available width is a blank canvas.
- Header row spacing: counter "0 connectors" and "New Connector" are vertically aligned with the title block, fine, but the description text runs very wide (~750px) with no max-width, hurting readability.

**3. VISUAL/TYPOGRAPHY DEFECTS**
- Column headers are lowercase ("name", "kind", "health", "refresh", "last sync") — inconsistent with title-case page headings and the "New Connector" button; reads as raw field names.
- The placeholder/dev note is user-visible ("A-5 (D5-0): the rows/refresh/delete triple collapsed…") — internal ticket notation leaking into the UI; sentence is also grammatically odd for the description ("as read-only tables the AI tools can query them" — missing "which/that").
- Weak hierarchy: header labels and dev note are similar small gray sizes; nothing signals the table is intentionally empty.

**4. COMPONENT DEFECTS**
- No empty state component at all — no icon, message, CTA, or "Create your first connector" action inside the panel; the only CTA is the header button, far from where the eye lands.
- Column headers have no sort affordances or hover states visible.
- The bell-ish icon button top-right is icon-only with unclear purpose.

**5. UX DEFECTS**
- With 0 connectors, the empty table should be the conversion moment; instead users see a blank 850px void plus a confusing developer annotation — a dead end.
- Counter "0 connectors" and empty table are redundant without guidance.
- Dev note text is meaningless to end users and may block comprehension of what the table would contain.

**6. ACCESSIBILITY**
- Dev note appears to be low-contrast small gray text; if the table headers share that gray, contrast may be borderline.
- Icon-only button top-right has no visible label/tooltip.
- Empty table region has no accessible text describing state (no "no connectors" message for screen readers).
- Table headers not marked as column labels for any content.

**7. VERDICT**
- [SEVERITY: blocker] Internal dev/placeholder note ("A-5 (D5-0):…") rendered in production UI.
- [SEVERITY: major] No empty state in the table — blank 850px panel with zero guidance or CTA.
- [SEVERITY: major] Massive dead space; table panel height not sized to content.
- [SEVERITY: minor] Lowercase column headers inconsistent with app casing.
- [SEVERITY: minor] Grammatical error in page description ("as read-only tables the AI tools can query them").
- [SEVERITY: minor] Icon-only top-right button with no label.
- [SEVERITY: minor] No accessible empty-table announcement for screen readers.
- [SEVERITY: polish] Description line unbounded width; add max-width (~70ch) for readability.
- [SEVERITY: polish] Dev note has no padding against panel border.