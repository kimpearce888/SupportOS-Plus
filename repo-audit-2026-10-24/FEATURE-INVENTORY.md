# FEATURE-INVENTORY.md — SupportOS-Plus (incremental)

Status: IN PROGRESS — pass 1 (page walk) not yet started. Structure and ID scheme established here.

ID scheme: features `F-<nnn>`; findings are `FIND-xxx` (see FINDINGS.md — F-001/F-002 are findings, not features); workflows `W-<nnn>`; steps `W-<nnn>-S<nnn>`; evidence `E-<nnn>`.

| Feature ID | Name / purpose | Entry point | Source paths | Status | Evidence |
|---|---|---|---|---|---|
| F-PAGES | 23 pages enumerated from pages/mod.rs PageKey enum (Dashboard, Inbox, Live chats, Operations, Customers, Organizations, Knowledge, Reports, Issue Radar, Incidents, AI Center, Automation, Connectors, Custom Objects, Diagnostics, Notifications, Onboarding, Outreach, Search, Settings, Support Graph, Sync Health, Shell) | sidebar nav / command palette / ServerEvent routing | crates/app-shell/src/pages/mod.rs:212–268 | DISCOVERED (runtime walk queued) | BUILD-002-launch-initial.png shows Dashboard |
