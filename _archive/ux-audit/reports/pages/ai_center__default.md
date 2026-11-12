# SupportOS+ — AI Center (Main Content Area) Audit

## 1. CONTENT INVENTORY

1. Page title: "AI Center" heading, top-left of content area
2. Tab bar: Health / Analytics / Attributes / Copilot / Usage / Jobs (Health active, underlined)
3. Card "LM Studio settings": key/value list — AI enabled (green "enabled" chip), provider: `lmstudio`, base url: `http://127.0.0.1:1234`, chat model: `(unset)`, embedding model: `(unset)`, timeout: `120000 ms`, concurrency: `2`
4. Helper text: "Start LM Studio locally… Attributes stay local and never sync back."
5. Button: "test connection" (dark primary)
6. Card "AI provider record (ai_settings)" with two-line explanatory paragraph
7. Segmented/selector row: provider: "None (no AI provider)" (selected, pill) / "LM Studio" / "not configured" (gray chip)
8. Form fields: chat model (placeholder "(empty — unset)"), embedding model (truncated placeholder), embedding dim (placeholder "e.g. 768"), base url (placeholder value)
9. Button: "Save provider record" (dark primary)
10. Footer status line: "stored record: provider_kind=none chat_model=(unset) embedding_model=(unset) dim=(unset) base_url=(unset)"
11. Card "Provider hub — endpoints" with subtitle; row: "on" chip, "LM Studio (local)", URL `http://127.0.0.1:1234/v1`, "built-in" chip, buttons "probe" and "set key" (right-aligned)
12. A resize/more handle glyph clipped at the bottom-right corner of the last card

## 2. LAYOUT DEFECTS

- **Massive dead space in "LM Studio settings" card**: the key/value table occupies ~15% of a card that spans the full ~1630px width; values end at x≈540 and the rest is empty. Card should be width-constrained or two-column.
- **Second card ("AI provider record") is half-width while cards above and below are full-width** — inconsistent card sizing, ragged right edge against the full-width cards; dead space to its right.
- **Form column is cramped and left-heavy**: inputs are ~260px wide in a ~620px card; labels and fields hug the left edge with large empty right side.
- **"Provider hub — endpoints" card appears clipped at the viewport bottom**: the row and buttons sit at the very bottom edge with no bottom padding, and a stray glyph ("X"-like resize handle) is cut off at bottom-right — content below the fold is jammed/cut.
- **Trailing edge of the "stored record" line touches the card's right padding** (line ends at "base_url=(unset)" nearly flush with the card edge).
- Uneven vertical rhythm: card 1 has generous line spacing (~41px between rows); card 2's form rows are ~41px but tighter overall; no consistent 8pt grid.

## 3. VISUAL / TYPOGRAPHY DEFECTS

- **Inconsistent key-label casing**: card 1 mixes "AI enabled:" (capitalized) with "provider:", "base url:", "chat model:" (lowercase); card 2 uses "chat model:" / "embedding model:"; hub card uses title case headings. No case system.
- **Inconsistent terminology for the same field**: "embedding model" (card 1) vs "embedding model:" (card 2) vs stored-record key `embedding_model`; "base url" vs `base_url`.
- **Monospace values at same visual weight as labels** — weak hierarchy; `(unset)` placeholders in card 1 look like actual data.
- **Truncated placeholder with no ellipsis affordance**: "(empty — embedding columns stay u…" is clipped mid-word — appears to be a rendering clip, not a deliberate ellipsis.
- Tiny helper text (~12px, light gray) below card-2 title is very low contrast and nearly illegible.
- "not configured" gray chip sits in the same row as selectable provider options but is a status, not an option — mixed semantics in one control row.
- Lowercase buttons ("test connection", "probe", "set key") vs Title Case button ("Save provider record") — inconsistent button labeling.

## 4. COMPONENT DEFECTS

- **Selector row is ambiguous**: "None (no AI provider)" looks selected (bordered pill), "LM Studio" looks like a button, "not configured" looks like a disabled chip — three different visual treatments in one radio-group-like row; unclear affordance.
- **Both primary actions are dark-filled, same weight** ("test connection", "Save provider record") — on a settings page, only one should be primary per context; "test connection" also lacks a loading/success state spec (nothing visible indicating feedback).
- Inputs have no visible focus/required indicators; no units or validation hints (e.g., embedding dim accepts what range?).
- "probe" and "set key" buttons are small (~26px tall) — small click targets.
- "enabled" chip is non-interactive-looking but reads like a toggle — is AI enabled mutable here? No affordance.

## 5. UX DEFECTS

- **Redundant/conflicting configuration surfaces**: LM Studio settings card shows provider `lmstudio`, but the provider record card says provider_kind=none and the selector says "None (no AI provider)" — a user cannot tell which is authoritative.
- **"(unset)" states with no resolution path**: chat model and embedding model are unset, but no inline CTA ("Configure in Settings → LM Studio") except buried in prose; dead end for first-run users.
- "test connection" result is not shown anywhere on the page — no success/error feedback region.
- Stored-record line dumps raw debug keys (`provider_kind=…`) at the user — developer-facing, not user-facing.
- "not configured" chip unexplained — configured where? No link.
- Page doesn't use available width; a 3-column layout (settings / record / hub) would eliminate scrolling and clarify relationships.

## 6. ACCESSIBILITY

- **Low-contrast gray helper text** on both card 2 and hub card (light gray on white, likely <4.5:1).
- **"(empty — embedding columns stay u…" clipped text** — content inaccessible to any user.
- Labels are plain text adjacent to inputs, not programmatically associated (no visible `for`/id pattern) — assumed from egui rendering.
- "on" chip in hub row conveys state by color/text only at ~11px — tiny target and low legibility.
- Key/value rows have no semantic grouping (dl/table) — screen readers will read "provider:" and "lmstudio" as unrelated lines.

## 7. VERDICT

- **[MAJOR]** Conflicting provider state between LM Studio settings card and provider record card — user can't tell which config is live.
- **[MAJOR]** "AI provider record" card half-width vs full-width siblings — inconsistent layout and wasted right-hand space.
- **[MAJOR]** Massive dead space in "LM Studio settings" card — value column ends at ~1/3 of card width.
- **[MAJOR]** "Provider hub — endpoints" card clipped at viewport bottom; stray resize glyph cut off at corner.
- **[MAJOR]** Truncated placeholder "(empty — embedding columns stay u…" without ellipsis — clipped content.
- **[MAJOR]** No visible feedback region for "test connection" / "probe" results.
- **[MINOR]** Ambiguous selector row mixing pill/button/chip semantics for one choice.
- **[MINOR]** Inconsistent label casing ("AI enabled" vs "provider") and button casing ("test connection" vs "Save provider record").
- **[MINOR]** Inconsistent terminology: "embedding model" vs `embedding_model`, "base url" vs `base_url`.
- **[MINOR]** Low-contrast ~12px helper text in cards 2 and 3.
- **[MINOR]** Raw debug string "stored record: provider_kind=…" shown to end users.
- **[MINOR]** Small click targets on "probe" / "set key" (~26px height).
- **[POLISH]** "(unset)" styling indistinguishable from real values — should be muted/italic.
- **[POLISH]** "enabled" chip looks like a non-interactive status but reads like a toggle — clarify or make it a switch.
- **[POLISH]** No first-run CTA when models are unset besides prose mention of Settings path.

**Overall**: functional but developer-scaffold-like; the three cards need a unified width system, a single source of truth for provider config, and explicit feedback for connection actions before this is end-user ready.