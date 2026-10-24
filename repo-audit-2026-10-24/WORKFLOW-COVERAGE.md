# WORKFLOW-COVERAGE.md — SupportOS-Plus (incremental)

Status: IN PROGRESS — no workflows executed yet. Entry points below are now
confirmed against the running app (feature-inventory pass 1) and source.

Ledger schema per workflow: Workflow ID, entry point, intended outcome, steps
(W-nnn-Snnn), branches, status, evidence, blocker.

| Workflow ID | Journey | Entry point (confirmed) | Intended outcome | Status | Evidence |
|---|---|---|---|---|---|
| W-001 | Help Scout OAuth connect | Settings → Help Scout tab → connect (loopback health check precedes offer) | Redirect to Help Scout, callback on loopback listener, provider swap without restart (ProviderHandle) | QUEUED (blocked on live OAuth endpoint; degraded-path message check possible) | — |
| W-002 | Sync lifecycle | Dashboard/Customers/Reports "Sync now"; Sync Health "Run initial sync" | Engine NEW → running → checkpoints + recent runs populate; header freshness chip changes | QUEUED | — |
| W-003 | Conversation lifecycle | Inbox (requires synced data) | List → select → reply → resolve; filters/tabs update | QUEUED (needs W-002 data or seeded fixture) | — |
| W-004 | Report builder | Reports → Builder tab | Configure → run → save; Insights tab reflects | QUEUED | — |
| W-005 | Knowledge ingest + gaps | Knowledge → Import documents; Issue Radar → Detect gaps now / Cluster now | Documents indexed (FTS+vector); radar/cluster outputs | QUEUED | — |
| W-006 | Automation rule | Automation → New Rule | Create rule → trigger → approval queue | QUEUED | — |
| W-007 | Theme switch round-trip | Sidebar theme row / Shell "switch to dark" | Toggle light↔dark, persisted across relaunch (settings table) | QUEUED | — |
| W-008 | Command palette navigation | Header "Search ⌘K" / sidebar palette trigger / Ctrl+K | Palette opens focused (DEFECT-03 fix claim), searches records, Enter navigates; Escape closes; backdrop click closes | PARTIAL (executed 2026-10-24: opens via header button ✓ incl. repeat, empty state "Type to search" ✓, Ctrl+K chord no-op — keyboard limitation, backdrop-close UNRELIABLE → FIND-011; query typing + hit-row navigation + Escape blocked by the keyboard limitation; sidebar trigger source-verified only — same open() call) | E-081..E-086; FIND-011 |
| W-009 | Second-instance UX | Launch second instance while one runs | Exit 1 + stderr message (FIND-002); check for any UI affordance | QUEUED | — |
| W-010 | Data reset/backup | Settings → Backups & export; Shell runtime card | Backup/export/reset flow incl. confirm dialogs (ConfirmRequest) | QUEUED | — |

Branches to exercise when executed: per-workflow happy path + at least one
failure/branch (e.g. W-001 with listener down = FIND-001 degraded path;
W-002 cancel mid-sync via "Cancel running sync"; W-008 empty-query Enter).
