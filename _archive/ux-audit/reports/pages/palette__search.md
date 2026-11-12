# Command Palette: "webhook"

## Palette Contents

**Input row:**
- Magnifier icon, input field containing query **"webhook"** with visible text cursor; input box has dark focus border.

**Result list (2 items, no section headers visible — results appear flat):**
1. Badge **"tickets"** — title **"#24 Webhook live probe: convo.created delivery"** — subtitle (partially obscured by highlight): "Created via API so the webhook handler can be tested with a real conversation id." — row is highlighted/selected (light gray background).
2. Badge **"tickets"** — title **"#3 API integration not working"** — subtitle: "Hi, your API docs recommend using /clear the webhook setup working. Can you send cleare..." (truncated with ellipsis).

**Bottom hint bar:**
- `↑ ↓` to move · `Enter` to open · `Esc` to close

## Visual Defects

| # | Defect | Severity |
|---|--------|----------|
| 1 | **Text bleed-through on selected row:** result 1's subtitle shows ghosted/overlapping text ("...API so the webhook handler...") colliding with the row title line — a rendering/transparency artifact. | High |
| 2 | **No section labels/grouping** (e.g., "Tickets"); both results are bare rows with small "tickets" badges, inconsistent with typical palette sections. | Low |
| 3 | **Vertical crowding between results** — result 2's badge/title sits very close under result 1's subtitle; row padding uneven. | Medium |
| 4 | Result subtitles are light gray on light gray highlight — **low contrast** on the selected row. | Medium |
| 5 | Badge-to-title alignment: "tickets" badge is small and top-aligned rather than vertically centered with the title. | Low |
| 6 | Hint bar spacing is fine but the ↑↓ arrows use a small glyph baseline that looks slightly misaligned with "to move". | Low |

## Centering & Size

- **Horizontally:** near-centered (roughly x 680–1238 in a 1920 viewport — center ≈ 959, essentially centered). ✅
- **Vertically:** NOT vertically centered — it sits near the top (~82–265 px), consistent with top-anchored palette convention, which is acceptable.
- **Width (~560 px) and height** look appropriate; results area is compact but not cramped. Overall: **well-sized, properly positioned**.
