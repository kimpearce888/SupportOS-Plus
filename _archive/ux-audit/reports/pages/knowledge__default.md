# UX/UI Audit — SupportOS+ Knowledge Page (main content area only)

## 1. CONTENT INVENTORY

- Top bar (within main area): green chip "synced 20s ago", global search field "Search ⌘K", icon-only bell/notification button (top-right corner of main region).
- Page header: "Knowledge" (H1), subtitle "local documents, sources, freshness, and the help center" (all lowercase).
- Primary button, top-right: "⟳ Sync Help Center now" (outlined pill).
- Tab bar: "Documents" (active, underlined), "Sources", "Freshness", "Help center", "Article drafts".
- Helper line: "Local knowledge documents used by search and AI answers" (small, low-contrast).
- Collapsible disclosure: "▶ Import documents" (chevron + label, no button styling).
- Empty-state block, horizontally centered: large book icon, text "(no documents)", subtext "Import a document to get started."
- No table, list, cards, chips, forms, or charts on this tab.

## 2. LAYOUT DEFECTS

- Empty state is vertically centered in remaining space but visually drifts into the lower-middle; the gap between the helper line/import disclosure (~y 229) and the icon (~y 300) is large dead space while everything above is compressed.
- "Sync Help Center now" button sits at ~x 1856, i.e., jammed against the right viewport edge with essentially no margin — no safe right gutter.
- Empty-state content is centered on the full pane width, but the header, helper line, and disclosure are all left-aligned at x≈237 — two competing alignment axes in one screen.
- The import disclosure and empty state are disconnected: ~60px of dead space and no visual relationship between the primary action and the empty state that asks for it.
- Massive unused vertical space below the empty state (~450px) with no anchor — content floats without a defined baseline.
- Helper line "Local knowledge documents…" is cramped directly against the tab underline (~10px gap) while the import disclosure gets ~30px — uneven vertical rhythm.

## 3. VISUAL/TYPOGRAPHY DEFECTS

- Subtitle casing: "local documents, sources, freshness, and the help center" is all-lowercase while every other label on the page is sentence/title case — inconsistent.
- "(no documents)" uses parentheses; parentheses in UI copy read as debug/placeholder text — should be "No documents yet."
- Helper line ("Local knowledge documents used by search and AI answers") is very light gray, small (~11px), and lower contrast than the empty-state subtext — hierarchy between two helper texts is inverted (the far one is more legible than the near one).
- Empty-state text is small relative to a 1080p viewport; the icon carries the composition but the copy at ~14px/12px is too timid for the page's primary moment.
- "Import documents" disclosure has no affordance styling — reads as plain body text, not interactive.

## 4. COMPONENT DEFECTS

- "Import documents": a disclosure-triangle control, not a button; tiny effective click target (label height only), no hover state visible, and no affordance signaling it opens a form.
- "Sync Help Center now" is an outlined button while the page's true primary CTA (import) is a weak disclosure — component weight is inverted relative to task importance.
- Icon-only button at top-right (bell/message) has no label or tooltip visible — unclear purpose.
- Tabs: active indicator is a thin underline only; inactive/active color contrast is subtle, and hover/focus states are not evidenced.
- Empty state offers no button — the CTA is plain text, not a clickable component.

## 5. UX DEFECTS

- Empty state says "Import a document to get started" but provides no action; the user must discover the unrelated-looking "Import documents" disclosure ~70px above and know that it expands.
- Empty state is generic: doesn't explain what documents do, what formats are supported (PDF/MD/HTML?), or offer secondary paths (connect a Help Center, open "Sources" tab).
- Two competing mental models on one screen: "documents" (local files) vs. "Help center sync" — nothing tells a new user which to do first or how they relate.
- "Synced 20s ago" chip: no target of the sync is stated (synced what?).
- Disclosure is collapsed by default even in a zero-content state — the one action available is hidden behind one extra click.
- Dead-end risk: if the user's intent is help-center content (tab exists), the empty Documents tab gives no cross-link to it.

## 6. ACCESSIBILITY

- Helper line gray-on-white is likely below 4.5:1 contrast (appears ~#9aa0a6 on white).
- "(no documents)" empty-state copy is low emphasis but that's minor; real risk is the icon-only top-right button with no accessible name (as visible).
- Disclosure "Import documents" appears to rely on chevron glyph alone for expand/collapse semantics; needs `aria-expanded`/role if egui exposes it.
- Empty-state message conveyed partly via icon (book) with no text alternative guarantee.
- Active tab differentiated only by underline + slightly darker text — insufficient for some low-vision users without `aria-selected` and stronger state styling.
- Large 450px empty region means keyboard focus order jumps from the disclosure straight into a large void — focus visibility in that region is unknown.

## 7. VERDICT

- **[MAJOR]** Empty-state CTA is plain text; primary action ("Import documents") is a hidden disclosure disconnected from the message that calls for it.
- **[MAJOR]** "Sync Help Center now" button flush against right viewport edge with no safe margin.
- **[MAJOR]** No guidance in empty state: no file formats, no link to Sources/Help-center alternatives, no first-run path.
- **[MINOR]** Inverted alignment: header left-aligned vs. empty-state center-aligned creates two visual axes.
- **[MINOR]** All-lowercase page subtitle inconsistent with sentence-case elsewhere.
- **[MINOR]** Helper line below contrast threshold and weaker than the subtext it precedes (inverted helper hierarchy).
- **[MINOR]** "(no documents)" parenthesized copy reads as placeholder/debug text.
- **[MINOR]** Primary/secondary action weight inverted: sync (secondary task) is a prominent button; import (primary) is bare text.
- **[MINOR]** Icon-only top-right button lacks visible label/tooltip; uncertain accessible name.
- **[MINOR]** Tab active state weak (thin underline only); hover/focus states not evident.
- **[MINOR]** Vertical rhythm uneven: helper line cramped under tabs, disclosure floating, ~450px unanchored dead space below empty state.
- **[POLISH]** Empty-state copy undersized for 1080p; enlarge type and add an actual "Import document…" button.
- **[POLISH]** "synced 20s ago" chip ambiguous — name the sync target or add tooltip.