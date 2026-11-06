**1) Tab:** Settings → "Business hours" sub-tab. Card: "Business hours per mailbox" with row "Webinars Today" + "not configured" badge + "edit" button. Topbar: synced pill, search, dark-mode toggle.

**Defects:**
1. [major] Massive empty space — one small card in a huge empty canvas; content should be wider/grid or centered.
2. [major] Sidebar has clipped/garbled item above "Customers" (cut-off text "Data…Inbox"?) — clipping bug.
3. [major] "Webinars Today" reads as confusing copy — is it mailbox name "Webinars" + day "Today", or one label? Ambiguous.
4. [major] No explanation of what "Business hours per mailbox" does; empty state has no helper text or CTA.
5. [minor] "edit" button lowercase, inconsistent with typical title-case UI labels.
6. [minor] "not configured" badge lacks color/semantic state (no warning tone, no icon).
7. [minor] Card width fixed/narrow (~290px) — width waste even within available area.
8. [minor] Sub-tab row left-aligned but title block has different left rhythm; tab underline slightly cramped to bottom border.
9. [minor] No save/add control visible — unclear how to add a new mailbox.
10. [polish] Subtitle "local configuration, persisted in the settings table" is developer-speak; user-hostile copy.
11. [polish] Card corner radius/shadow slightly heavier than badge/button styling — inconsistent elevation.
12. [polish] Page background flat white both sides of divider — weak visual hierarchy vs sidebar.
13. [minor] No hover/focus states verifiable; edit button affordance is weak (text-only, light border).
14. [polish] "synced 23s ago" pill floats alone; no link/status tooltip implied.
