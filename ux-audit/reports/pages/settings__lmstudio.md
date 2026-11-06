**Tab:** Settings → "LM Studio" (active)

**Cards/controls:** One card — "LM Studio (local AI gateway)" with base url, chat model, embedding model inputs and a "save" button.

**Defects:**
1. [blocker] Sidebar top clipped: first nav item ("...edge") cut off under the search box — overlapping/clipped content.
2. [major] Massive width waste: card occupies ~25% of screen; rest of 1920px content area is empty.
3. [major] No save feedback/states: button gives no success confirmation, no dirty/disabled state, no error handling.
4. [major] Confusing copy: helper text says "Test the connection from the AI Center page" but offers no link/button — dead-end instruction.
5. [major] Missing validation/labels: empty chat/embedding fields have no required indicators, placeholders, or error states.
6. [minor] Labels lowercase ("base url:", "save") — inconsistent with Title Case used elsewhere in the UI.
7. [minor] Label/input vertical misalignment: labels sit slightly higher than field text.
8. [minor] Colons on labels are unnecessary; inconsistent form styling.
9. [minor] Subtitle "local configuration, persisted in the settings table" exposes internal jargon to users.
10. [minor] Card max-width fixed and small; no responsive use of space; single lonely card looks unfinished.
11. [polish] Inconsistent spacing: large gap between card and header vs cramped label rows.
12. [polish] "synced 18s ago" pill tiny/low-contrast; unclear what it syncs.
13. [polish] Dark-theme toggle isolated at very bottom with no section grouping.
