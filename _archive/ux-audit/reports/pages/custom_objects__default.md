# UX/UI Audit — Custom Objects (Main Content Area)

## 1. Content Inventory
- **Page header row:** "Custom Objects" title (left) with subtitle "0 type(s) · 0 object(s) · 0 link(s)"; right side: "0 types" count label + black primary button "+ New Type".
- **Section label:** "Types" (small heading, left, below a hairline divider).
- **Left column empty state:** objects icon, "No types yet", caption "Create a type to start storing objects."
- **Right column empty state (detail pane):** objects icon, "Select a type", caption "Pick a type on the left to see its objects."
- Massive empty vertical space below (~600px) across both columns.

## 2. Layout Defects
- **Two competing empty states side-by-side** — master list and detail pane both render empty states, doubling the message and wasting the layout.
- **Detail-pane empty state is orphaned**: "Pick a type on the left" is nonsensical when the list is empty; it sits far right (~1470px) visually disconnected from its partner.
- **No visible column/card boundary** between the list and detail panes — both empty states float in one undivided grey field.
- **Dead space**: entire lower 60% of content area is blank; empty states are top-anchored rather than vertically centered in their panes.
- **Header misalignment**: "0 types" count and "+ New Type" button sit noticeably higher than the "Custom Objects" title baseline (button near top edge, title lower) — the header row reads as two vertically misaligned clusters.
- **Subtitle "0 type(s) · 0 object(s) · 0 link(s)"** is redundant with the "0 types" label on the right; duplicate counts in one screen.
- **"Types" section label** floats alone with no container or border under it; relationship to the empty states is unclear.

## 3. Visual / Typography Defects
- Subtitle uses developer-style pluralization "type(s) · object(s) · link(s)" — inconsistent casing/format vs. clean "0 types" label on the right.
- "0 types" count is very low-contrast light grey and tiny, yet is the only live data in the header.
- Empty-state headings ("No types yet", "Select a type") same small size (~14px) as body text — no hierarchy between heading and caption.
- Both captions are light grey (~#8a8a8a on #f5f5f5) — near-invisible.
- "New Type" button says "New Type" while empty state says "Create a type" — terminology mismatch.

## 4. Component Defects
- **"Types" label is not a click target or header of anything** — unclear affordance.
- **Primary CTA buried in header**: in an empty state, "+ New Type" should also be an inline button within the "No types yet" empty state; currently the empty state is a dead end with text only.
- The objects icon in both empty states is identical — no differentiation between "list" and "detail" states.
- No visible hover/focus affordance anywhere (nothing to interact with besides one button).
- Icon-only ghost button in top-right corner (sidebar area — ignored) but the main area's only control is one button; no secondary action (e.g., "Import", "Docs", "Learn about custom objects").

## 5. UX Defects
- **Dead-end empty state**: "No types yet" instructs "Create a type" but offers no clickable action — user must scan up to the header button.
- **Contradictory guidance**: "Pick a type on the left to see its objects" is impossible to satisfy (list is empty); confusing and signals the detail pane rendered before checking list state.
- Redundant counts (subtitle + "0 types") add noise without information.
- No sample/template types, import option, or documentation link to help a first-time user understand what a "custom object" is.
- No feedback loop: nothing indicates what "New Type" will do (form? modal?).

## 6. Accessibility
- Empty-state captions and "0 types" label: very low contrast grey-on-grey, likely below 4.5:1.
- "(s)" parenthetical pluralization is read awkwardly by screen readers ("zero type s").
- Two adjacent empty states create confusing reading order — a screen reader announces "Select a type / pick a type on the left" immediately after "No types yet".
- No visible focus indication on the "+ New Type" button (egui default).

## 7. Verdict

| Severity | Defect |
|---|---|
| **major** | Empty state is a dead end — no inline "Create type" CTA where the instruction says "Create a type" |
| **major** | Detail pane renders "Pick a type on the left" even when the list is empty — contradictory guidance |
| **major** | Two simultaneous empty states with no pane separation — confusing, wasted layout |
| **minor** | Header row vertical misalignment: count/button sit above title baseline |
| **minor** | Terminology mismatch: "New Type" vs "Create a type"; "type(s) · object(s) · link(s)" vs "0 types" |
| **minor** | Low-contrast grey captions and count label fail WCAG contrast |
| **minor** | No onboarding affordances: no templates, import, or "learn more" for first-run users |
| **polish** | Empty-state icons and headings lack hierarchy/size differentiation |
| **polish** | Empty states top-anchored; ~60% of content area is dead space |
| **polish** | Screen-reader-unfriendly "(s)" pluralization in subtitle |