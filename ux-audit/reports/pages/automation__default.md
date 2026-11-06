# SupportOS+ — Automation Page Audit (Main Content Area)

## 1. CONTENT INVENTORY
- Page header: "Automation" (h1) with sub-caption "Rules trigger actions when conversations match certain conditions. Actions that need review park in the approvals queue."
- Primary button, top-right: "＋ New Rule" (black pill).
- Card 1 "Rules" (narrow, ~170px wide): heading, count "0 rule(s)", table header row with columns `name  when  then` — no rows, no empty state.
- Card 2 "Pending approvals" (full-width): heading, count "0 pending approval(s)", empty state with clock icon, "No approvals waiting" and helper text "Actions that need review will park here."
- Stray developer/debug text line above a large bordered box: "Reskin fix (presentation): approval rows led with the raw proposed_action_json. Human-readable action first, raw JSON collapsible."
- Large empty bordered container (~1550×530px) with no content at all.
- Top bar (right): "synced 5s ago" chip, "Search ⌘K" pill, bell icon button.

## 2. LAYOUT DEFECTS
- The "Rules" card is drastically undersized (~9% of content width) while the card below spans full width — inconsistent card sizing, massive dead space to its right (region x≈400–1880, y≈145–255).
- Rules table columns `name / when / then` have no rows; the card is header-only with no empty-state message or CTA.
- A huge empty bordered box (~1.55M px² of dead space) occupies the lower half of the page with zero content — reads as a rendering failure, not intentional.
- Internal dev note text sits outside/above that box, overlapping its top border edge (y≈520) — looks like accidental DOM placement.
- Page background is uniform gray; the empty lower box is indistinguishable in tone from the cards — weak sectioning.
- No spacing rhythm: gap between Rules card and Pending approvals (~30px) differs from gap to the dev-note box.

## 3. VISUAL / TYPOGRAPHY DEFECTS
- The developer note is user-facing: raw engineering jargon ("proposed_action_json", "Reskin fix") in a production UI — clearly leaked debug content.
- Table headers `name / when / then` are lowercase while every other heading on the page is sentence case ("Pending approvals", "New Rule") — casing inconsistency.
- Counts use raw developer pluralization: "0 rule(s)", "0 pending approval(s)" — "(s)" pattern is unpolished; should be "No rules yet" / "No approvals".
- Weak hierarchy: card headings, counts, and helper text are close in size/weight; the page title is the only clear anchor.
- Helper text under the title is small and low-emphasis gray — acceptable, but the same gray is reused for counts and column headers, flattening the hierarchy.

## 4. COMPONENT DEFECTS
- `name / when / then` column headers are plain text — no sorting affordance, no alignment definition, and headers without a table body are meaningless.
- The large bordered box has a border but no header, no label, no affordance — unclear what it is or whether it's interactive.
- "New Rule" is the only actionable control on the page; nothing else has hover/active/disabled states visible or implied.
- Empty Rules card has no CTA — the obvious fix is a "Create your first rule" button linking to the same action as "New Rule".
- No loading/skeleton states anywhere despite a "synced 5s ago" chip implying async data.

## 5. UX DEFECTS
- Dead end: user lands on Automation with zero rules and the Rules card offers no guidance — the empty state fails to onboard.
- The giant empty box is a trust-killer; users will assume the app is broken.
- The dev-note suggests the approvals row presentation is unfinished — feature shipped in a non-production state.
- "New Rule" button is far from the Rules card it populates (top-right vs. small card at top-left); when rules exist, the causal link is weak.
- No feedback loop: page says "synced 5s ago" but no refresh control or indication of what syncing does here.

## 6. ACCESSIBILITY
- Bell icon in the top bar is icon-only with no visible tooltip/label — needs an accessible name.
- "synced 5s ago" chip: very small, green-on-light text likely fails 4.5:1 contrast.
- Lowercase table headers with 2–3 character words ("when", "then") give screen readers near-meaningless context with no rows.
- The dev-note text is small gray on gray — low contrast and would be read aloud as page content.
- The unlabeled empty bordered box is announced as nothing — noise for assistive tech.

## 7. VERDICT

| Severity | Defect |
|---|---|
| **[BLOCKER]** | Developer debug note ("Reskin fix (presentation): … proposed_action_json …") shipped in the user-facing UI |
| **[BLOCKER]** | Large empty unexplained container occupies half the page — looks like a broken/unrendered component |
| **[MAJOR]** | Rules card has no empty state or CTA — dead end for first-time users |
| **[MAJOR]** | Rules card is ~9% width with vast dead space beside it — layout inconsistency vs. full-width card below |
| **[MAJOR]** | Header-only table (`name / when / then`) with no body and unclear purpose |
| **[MINOR]** | "(s)" pluralization ("0 rule(s)", "0 pending approval(s)") — raw string formatting leaked into UI |
| **[MINOR]** | Lowercase column headers inconsistent with sentence-case headings elsewhere |
| **[MINOR]** | "synced 5s ago" chip likely low-contrast and tiny |
| **[MINOR]** | Icon-only bell button lacks accessible name/tooltip |
| **[POLISH]** | Weak typographic hierarchy among card headings, counts, and helper text |
| **[POLISH]** | "New Rule" button visually disconnected from the Rules card it creates content for |

**Summary:** The Pending-approvals empty state is the only production-quality element on this page. The page reads as a mid-development build: leaked dev notes, a placeholder container, and an empty-state-less Rules table. Fix the two blockers before any user exposure.