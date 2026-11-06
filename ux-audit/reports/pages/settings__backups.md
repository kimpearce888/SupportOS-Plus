**Tab:** Settings → "Backups & export" sub-tab.
**Card/controls:** "Backups & export" card with buttons: create backup, export JSON, export conversations CSV; one backup row (filename, size, timestamp) with a "restore" button.

**Defects:**
1. [major] Sidebar first section label is clipped ("Source…"/"Knowledge base" cut off under search box).
2. [major] Huge width waste — content max ~670px on 1920px screen; card ~35% of viewport, ~1200px dead space to the right.
3. [major] Massive vertical empty space below the card; page feels broken/unfinished.
4. [major] No empty/multi-backup handling visible; only one backup row, no list header or count, no description of what restore does.
5. [major] Destructive "restore" button has no visual differentiation, no confirmation state, no icon — dangerous affordance.
6. [minor] No loading/success feedback shown for backup/export actions; unclear if buttons are disabled during operations.
7. [minor] Backup metadata cramped: filename, raw byte size, and raw ISO timestamp jammed together with no labels or formatting (bytes not human-readable; timestamp should be localized).
8. [minor] Sub-tab row lacks container/underline for inactive tabs; active indicator is a lone thin line, low affordance.
9. [minor] "synced 26s ago" pill floats top-right with no context of what synced.
10. [minor] Subtitle "local configuration, persisted in the settings table" is dev-speak; confusing to end users.
11. [polish] Card header duplicates the sub-tab name ("Backups & export" twice).
12. [polish] Low contrast on gray secondary text (metadata, subtitle).
13. [polish] Dark "create backup" vs gray export buttons — primary/hierarchy okay but export buttons look disabled (inconsistent enabled styling).
14. [polish] No danger zone styling or spacing separating restore from export actions.
