# SupportOS+ Inbox — Main Content Area Audit

## 1. CONTENT INVENTORY

- Top bar: "synced 14s ago" status pill, global "Search ⌘K" button, notification bell icon (far right)
- Column header "All" with subtitle "18 of 18 conversations"
- Search input (placeholder "search conversat…" — truncated) and a small "#" chip/input (tag filter?)
- Dark "apply" button
- Text link "Filters", text link "reset filters", outlined "Sync now" button with icon
- INBOX VIEWS list: All (18), Active (13), Pending (2), Closed (2) — with counts, "All" selected
- Conversation list: 7 rows (Priya Sharma #24, Liam Carter #23, Priya Sharma #22, Priya Sharma #21, Tom Becker #14, Maya Rodriguez #16, Priya Sharma #15), each with name, timestamp, subject line, "unassigned" chip, status chip (Active/Pending/Closed)
- Row #21 shows an avatar ("SN") + "Sayeed Nabin" assignee; others show none
- Right pane: empty state with image placeholder icon, "Select a conversation" heading, helper text "Pick a conversation from the list to read the thread and reply."

## 2. LAYOUT DEFECTS

- **Massive dead space**: the list + detail pane occupy only ~740px of a 1920px viewport; ~1150px of the right side is blank white. The conversation list is pinned to the left-center with no list-to-detail split, and the "detail" empty state floats arbitrarily at x≈1320 with no panel boundary.
- No visible divider or panel edge between the conversation list and the empty-state area — the two regions blur into one undefined canvas.
- The list ends at y≈1067 but its background panel ends at y≈1080 with a ragged white gap to the right of the list (list panel ~410px wide, floating).
- The "apply" button is vertically misaligned with the search input and "#" chip (sits slightly higher, y≈147 vs input y≈146 center offset).
- Row #21: the assignee avatar+name ("SN Sayeed Nabin") appears on the *status chip row* instead of the meta row, breaking the consistent 3-line rhythm of every other row (some rows are 3 lines, this is 4).
- "Filters" / "reset filters" / "Sync now" toolbar row is left-clustered with a large empty gap to the right — unbalanced.
- Conversation rows have generous internal padding but inconsistent vertical spacing between the subject and chip rows vs. between rows.

## 3. VISUAL / TYPOGRAPHY DEFECTS

- Casing inconsistency: "apply" (lowercase) vs "Sync now" (Title Case) vs "reset filters" (lowercase) vs "Select a conversation" (Title Case) — three casing conventions in one screen.
- Search placeholder "search conversat…" is truncated with no clear ellipsis intent — the input is too narrow for its own placeholder.
- Status chips use three different fills (blue Active, amber Pending, gray Closed) but "unassigned" chips are italic outlined — a fourth chip style for what is also metadata; visual weight competes with status chips.
- Subject lines ("#24 Webhook live probe: convo created delivery") are near-identical weight/size to customer names — weak scan hierarchy; the name should dominate or the subject should.
- Timestamps in light gray at ~11px are borderline legibility.
- The empty-state helper text is centered but the icon above it is small and generic (image-placeholder glyph) — reads as a broken-image icon rather than an intentional illustration.
- "18 of 18 conversations" — redundant phrasing; if a filter is active the "of 18" matters, but here it's noise and in a smaller, low-contrast size.

## 4. COMPONENT DEFECTS

- "apply" button: unclear what it applies to (search? the # field? both?); no visible text-input↔button grouping. No "clear" affordance for search.
- "#" chip is ambiguous — is it a tag filter, a conversation-number jump, or a prefix label? No tooltip, no label.
- "Filters" looks like a plain text link — tiny click target, unclear whether it expands a panel (nothing visibly toggles).
- "reset filters" is always shown even when no filters are applied; unclear disabled/enabled state.
- "Sync now" vs the passive "synced 14s ago" pill: no connection between them; the pill isn't clickable and doesn't indicate progress states.
- List rows: no visible hover/selected state demonstrated; the selected view ("All") has a highlight but conversation rows have no selection affordance shown despite the empty state instructing selection.
- Status chips and "unassigned" chips don't appear interactive but have chip styling that suggests clickability.

## 5. UX DEFECTS

- Empty-state dead end: "Pick a conversation from the list" but no CTA button, no shortcut hint (e.g., ↑/↓ navigation), and the empty state is spatially far from the list it references.
- No sorting control or sort indicator on the list (rows appear chronological but #14/#16/#15 ordering is non-monotonic — #14 between #23 and #16 — with no explanation, suggesting mixed sort or data ordering bug).
- Timestamps show date+time but no relative time or "today/yesterday" grouping; "2026-10-27 05:24" forces mental math.
- No unread indicators, preview snippets, or priority markers on rows — rows look identical regardless of urgency.
- Notification bell (top right) is outside the audited sidebar and has no badge/state; unclear whether it duplicates sidebar "Notifications."
- Search in list + global Search ⌘K + "Filters" = three overlapping find mechanisms with no role distinction.

## 6. ACCESSIBILITY

- Timestamps and "18 of 18 conversations" gray text: low contrast (~3:1 estimated), below WCAG AA for small text.
- "unassigned" chips: light gray italic text on light outline — likely sub-AA.
- "Filters" and "reset filters" text links are small, low-contrast, and likely lack focus affordances.
- "#" control has no accessible name or label; icon-only bell button has no visible tooltip/aria-label evidence.
- Reading order: the empty-state instruction ("pick from the list") appears in DOM/visual flow to the *right* of the list — screen-reader users reach the instruction after the entire list, and it may not be announced as related to it.
- Selected-state ("All" view) appears communicated only by background tint — verify an `aria-selected`/`aria-current` equivalent exists in egui accessibility layer.

## 7. VERDICT

- **[MAJOR]** ~60% of the 1920px viewport is dead space; no list/detail panel split or divider defined.
- **[MAJOR]** Row #21 breaks the list row pattern (assignee on chip row, 4-line row vs 3-line everywhere else).
- **[MAJOR]** Conversation ordering is non-chronological with no sort control or explanation (#14 between #23 and #16).
- **[MAJOR]** "apply" button's target is ambiguous; search input, "#" field, and apply lack grouping/labels.
- **[MAJOR]** "#" filter control is unlabeled and incomprehensible without a tooltip.
- **[MINOR]** Search placeholder truncated ("search conversat…") — input too narrow for its own text.
- **[MINOR]** Casing inconsistency across buttons ("apply" vs "Sync now" vs "reset filters").
- **[MINOR]** Empty state has no CTA, no keyboard hint, and is spatially detached from the list it references.
- **[MINOR]** Four visually distinct chip styles (Active/Pending/Closed/unassigned) compete for attention; unassigned reads as interactive.
- **[MINOR]** Timestamps lack relative formatting and unread/priority signals; rows are visually homogeneous.
- **[MINOR]** Low-contrast timestamps, subtitle text, and "unassigned" chips likely fail WCAG AA.
- **[MINOR]** "reset filters" shown with nothing to reset; no disabled state logic evident.
- **[MINOR]** "Sync now" and "synced 14s ago" pill are disconnected; pill offers no affordance or progress state.
- **[MINOR]** "Filters" is a tiny text-link target with no evidence of what it toggles.
- **[POLISH]** Empty-state icon resembles a broken-image placeholder; needs a real illustration.
- **[POLISH]** "18 of 18 conversations" is redundant phrasing when no filter narrows the set.