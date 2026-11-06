**Tab:** Settings → "Sync & AI" (active), with sub-tabs: Help Scout, LM Studio, Vector index, Business hours, Backups & export.

**Cards/controls:** Synchronization (sync interval, api concurrency, log level, agent language, status after reply), AI behaviour (checkbox list: AI enabled, automatic analysis, internal notes, draft replies, automation rules, write actions, attachment auto download), About & updates (version, check for updates), save settings button.

**Defects:**
1. [major] Massive width waste — cards occupy ~25% of a 1920px canvas; huge empty right area, no max-width container.
2. [major] First sidebar item clipped/truncated mid-render ("...nowledge" text cut by search box).
3. [major] Checkbox rows have inconsistent styling — some have grey filled backgrounds, others plain.
4. [major] "check for updates" button uses lowercase; mixes with Title Case elsewhere ("save settings" too) — inconsistent copy style.
5. [minor] Card widths inconsistent (Sync card ~460px, AI card ~290px, About ~510px) — ragged layout.
6. [minor] Numeric inputs (1, 2) extremely cramped/narrow; unclear if stepper or free text.
7. [minor] Labels lowercase with trailing colons ("max quiet sync interval (minutes):") — inconsistent casing, verbose.
8. [minor] "AI enabled" row has ✓ but no visible checkbox control unlike others — ambiguous affordance.
9. [minor] No visible Save feedback state, dirty-state indicator, or disabled state on save.
10. [minor] No save button per card — single floating "save settings" bottom-left, easy to miss.
11. [polish] Sub-tab active indicator underline thin; inactive tabs low contrast.
12. [polish] "synced 16s ago" pill far from relevant content; redundant with Sync Health nav item.
