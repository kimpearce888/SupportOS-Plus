# FEATURE-INVENTORY.md — SupportOS-Plus (incremental)

Status: **pass 1 (page walk) COMPLETE** — all 23 sidebar pages opened in the real
running app via XTEST-injected clicks, each navigation machine-verified
(selection-pill tracking, ±1 px), each page screenshot captured and visually
validated. Pass 2 (control/option ledger per page, FEAT-002) not started.

ID scheme: features `F-<nnn>`; findings `FIND-xxx` (FINDINGS.md); workflows
`W-<nnn>`; steps `W-<nnn>-S<nnn>`; evidence `E-<nnn>`; exceptions `EXC-<nnn>`.

## Method (audited-revision binary, no app-code changes)

- Binary: `target/debug/supportos-plus` @ main `3affe7e` (default build, glow renderer).
- Environment: Xvfb :99 (1600x1000x24), LIBGL_ALWAYS_SOFTWARE=1, default window
  1180x760 (hardcoded `lib.rs:241`; the `SPP_SHOT_W/H` override only exists under
  the non-default `ui-screenshot` cargo feature, so it was NOT used — the audited
  artifact is the default build).
- Input: python-xlib XTEST — real mouse clicks on sidebar rows and real wheel
  scrolling of the sidebar nav area. Every frame analyzed was captured with the
  pointer parked outside the sidebar (no hover states in evidence frames) and
  after the egui scroll animation settled (two consecutive identical frames).
- Navigation self-verification: the sidebar highlights the active page with a
  pill background (luminance 244 vs 250 panel). After every click the script
  re-shot and required the pill to sit on the clicked row (tolerance ±4 px;
  observed drift ±1 px). Scroll position was anchored to the live pill, so no
  hardcoded geometry is load-bearing.
- Tool: `evidence/artifacts/feature-walk.py`; per-step machine log:
  `evidence/artifacts/feature-walk-events.jsonl`; app log of the clean run:
  `evidence/logs/FEATURE-walk-app.log` (fresh process pid 18958, lock acquired,
  loopback degraded-to-WARN consistent with FIND-001 in this sandbox).
- History kept for honesty: v1 of the tool drifted off-by-one on scrolled rows
  (frames aliased to one reused PNG file + hover contamination); v3/v4 briefly
  drove an orphaned app instance left by a v2 crash (provenance defect — their
  outputs were discarded); the published evidence is the v5 clean
  single-process run. Details in SESSION-LOG.md.

## Page inventory (23/23; nav order = `PageKey::all()`, asserted app.rs:1523)

| Feature ID | Page (sidebar label) | Source | Observed at runtime (empty DB, default config) | Entry points | Evidence |
|---|---|---|---|---|---|
| F-001 | Dashboard | crates/app-shell/src/pages/dashboard.rs | Title + "Local metrics" subtitle; range chips 7/30/90 days/1 year (30 days active); channel chips All/Email/Chat (Beacon); mailboxes chip All; 12 zero-value metric cards (New, Active, Pending, Closed, Unassigned, Backlog 7d stale, Replies sent, First response avg min, Great ratings, Okay ratings, Not-good ratings, Resolution avg min); "New conversations per day" chart card; header "Sync now" | sidebar row, command palette (untested), ServerEvent routing | E-006 |
| F-002 | Inbox | pages/inbox.rs | "0 of 0 conversations"; status tabs All/Active/Pending/Closed; dropdowns All mailboxes / All channels / Any tag + apply; search field, `#` tag field, Search button; "reset filters"; banner "Help Scout is not connected — nothing can sync yet…"; two-pane layout, list empty, detail pane "Select a conversation" | sidebar, operations tiles (NavRequest::InboxTile), palette | E-007 |
| F-003 | Live chats | pages/live_chats.rs | "0 open · 0 ended"; tabs open chats / show ended chats; explainer "Beacon chats arrive through the five beacon.chat.* webhook events — Help Scout has no list endpoint, the webhook IS the data path."; empty state | sidebar, palette | E-008 |
| F-004 | Operations | pages/operations.rs | "Operations Center"; tabs Tiles/Workload/Library; "16 tiles cover the team's current workload, SL risk, and system health. Click a tile to filter the inbox."; generated timestamp + "waiting threshold 240 min"; Critical (SLA breached), Warning (Needs first response, Waiting over threshold, Urgent, SLA at risk), Info (Unassigned, Customer waiting, High effort, Repeated issue) — all 0, "open in inbox →" drill links | sidebar, palette | E-009 |
| F-005 | Customers | pages/customers.rs | Search customers by name or email + Search; "No customers loaded yet — Search by name or email and press Enter."; header Sync now | sidebar, palette | E-010 |
| F-006 | Organizations | pages/organizations.rs | "0 organization(s)"; filter by name + filter; New organization form (Name, domains placeholder acme.com, acme.io) + create organization; detail pane "Select an organization"; header Sync now | sidebar, palette | E-011 |
| F-007 | Knowledge | pages/knowledge.rs | Tabs Documents/Sources/Freshness/Help center/Article drafts; "Sync Help Center now" (disabled until Docs API key; hint text present); "Import documents" expander; "(no documents)" empty state | sidebar, palette | E-012 |
| F-008 | Reports | pages/reports.rs | 11 tabs: Overview/SLA/Why contacting/Top questions/Quality/Issue radar/Digest/Happiness/Help Scout/Builder/Insights; range 1 year/90/30/7 days (30 days active); 8 metric cards (0 values); note "Ratings come from the satisfaction webhook; Help Scout's own happiness reports need a Plus/Pro plan and are not fetched."; "New conversations per day" chart ("no data in range") | sidebar, palette | E-013 |
| F-009 | Issue Radar | pages/issue_radar.rs | Buttons Detect gaps now / Reload clusters / Cluster now; tabs Alerts/Clusters/Gaps/Known issues; "0 radar alert(s) …(no alerts)" card; "SLA: 0 breached · 0 at risk …(no SLA alerts)" card | sidebar, palette | E-014 |
| F-010 | Incidents | pages/incidents.rs | "New Incident" button; filters status Any / severity Any / open only checkbox / search + apply; "0 incident(s)"; empty state + "Select an incident" detail pane | sidebar, palette | E-015 |
| F-011 | Custom Objects | pages/custom_objects.rs | "New Type"; "0 type(s) · 0 object(s) · 0 link(s)"; Types pane "No types yet"; "Select a type" detail pane | sidebar, palette | E-016 |
| F-012 | Automation | pages/automation.rs | "New Rule"; Rules card (0 rule(s), columns name/when/then); Pending approvals card "No approvals waiting" | sidebar, palette | E-017 |
| F-013 | AI Center | pages/ai_center.rs | Tabs Health/Analytics/Attributes/Copilot/Usage/Jobs; LM Studio settings card: AI enabled=enabled badge, provider lmstudio, base url http://127.0.0.1:1234, chat/embedding model (unset), timeout 120000 ms, concurrency 2, "test connection"; "AI provider record (ai_settings)" card (None/no AI provider; LM Studio; not configured; chat model "(empty — unset)") | sidebar, palette | E-018 |
| F-014 | Outreach | pages/outreach.rs | Tabs Campaigns/Segments/Do not contact; "New Campaign"; "No campaigns" empty state | sidebar, palette | E-019 |
| F-015 | Notifications | pages/notifications.rs | "0 total · 0 unread"; "mark all read"; tabs All/Mentions/Unread; "(no notifications)" | sidebar, palette | E-020 |
| F-016 | Search | pages/search.rs | Query field + clear; scope All/Tickets/Knowledge/Customers; "Search across your workspace" empty state | sidebar, palette, header ⌘K | E-021 |
| F-017 | Support Graph | pages/support_graph.rs | "0 conversation links · 0 customer links · 0 human edges"; "Search the graph…" + search; empty state | sidebar, palette | E-022 |
| F-018 | Connectors | pages/connectors.rs | "New Connector"; explainer "Attach external data sources (HTTP endpoints, CSV files inside the data directory) as read-only tables; the AI tools can query them when allowed."; table headers name/kind/health/refresh/last sync (empty) | sidebar, palette | E-023 |
| F-019 | Diagnostics | pages/diagnostics.rs | Event log card: filter tabs all/errors/warnings/info, refresh, red "clear log"; "1 event(s) shown — 0 error(s), 0 warning(s)"; row `info 2026-10-24 15:42:38 sync "provider transition →empty"`; Audit trail card (0 audited changes) | sidebar, palette | E-024 |
| F-020 | Sync Health | pages/sync_health.rs | Engine card: state NEW, last success (never), webhook queue 0 total/0 pending/0 processed/0 failed/0 duplicates; buttons Run initial sync / Run incremental sync / Run reconcile / red Cancel running sync; Checkpoints card "(no checkpoints yet — run an initial sync)"; Recent runs card "(no runs recorded)" | sidebar, palette, header freshness chip | E-025 |
| F-021 | Settings | pages/settings.rs | Tabs Backups & export / Business hours / Vector index / LM Studio / Help Scout / Sync & AI (Sync & AI active by default); Synchronization card: max quiet sync interval (minutes)=1, api concurrency=2, log level=info, agent language=en, status after reply=pending; AI behaviour card checkboxes: AI enabled ✓, Automatic analysis ✓, Automatic internal notes ☐, Automatic draft replies ☐, Automatic reply sending ☐, Automation rules ☐, Automation write actions ☐, Attachment auto download ✓ (list continues below fold) | sidebar, palette | E-026 |
| F-022 | Onboarding | pages/onboarding.rs | "Welcome to SupportOS+"; "The app starts empty: no sample content, no stand-ins — only your real history."; Setup status card: step=welcome, onboarding complete=false, conversations=0, Help Scout credentials=false, sync state=new; buttons Run initial sync / Mark complete; hints "Onboarding is not marked complete yet." / "No Help Scout credentials yet — add them in Settings → Help Scout first." | sidebar, palette | E-027 |
| F-023 | Shell | pages/mod.rs (shell_ui in app.rs:779) | "Shell — native shell diagnostics (Phase 2 surface)"; Runtime card: database=/home/z/.local/share/supportos-plus/supportos-plus.db, schema=v52, background events=0, last event —, sync engine=idle; note "all pages ported: dashboard, inbox, operations, customers, organizations, knowledge/docs, reports, issue radar, incidents, custom objects, automation, AI center, outreach, notifications, search, support graph, connectors, sync health, settings, onboarding"; header "switch to dark" | sidebar row ONLY (no cross-links, no CLI arg — verified in source) | E-028 (see FIND-004) |

Shell-page navigation note (F-023 + FIND-004): at the default window the Shell
sidebar row is partly overlapped by the pinned theme row; the walk opened it via
a measured click in the free top sliver of the row (y=703; theme row hover rect
[714,743]), verified by selection pill (718) AND content diff (4.18 mean).

## Shell-level features (not pages)

| Feature ID | Feature | Source | Runtime observation | Evidence |
|---|---|---|---|---|
| F-024 | Sidebar navigation (5 groups: Workspace 4, Customers & knowledge 3, Insights 3, Automation & AI 4, System 9 = 23 rows; icons; selection pill; scrollable; command-palette trigger; theme row pinned bottom) | app.rs sidebar_ui 397–466, NAV_GROUPS 853–898 | All 23 rows clicked & pill-verified in one session (23 navigation steps, 4 wheel scrolls); group labels render muted; nav rows ~40 px pitch; bottom rows overlay defect → FIND-004 | E-006..E-029, E-030 |
| F-025 | Header chrome: freshness chip ("never synced", click → Sync Health per tooltip), "Search ⌘K" palette trigger, theme toggle button | app.rs header_ui 471–505 | Visible in every screenshot; freshness chip reads "never synced" throughout (no sync performed); tooltips present (header buttons) | E-006..E-029 |
| F-026 | Persistence & instance behavior | main.rs (flock), design/theme store | Single-instance flock: second launch exits 1 (E-005, FIND-002). Theme persisted in settings table (light theme retained across relaunches). Last page / sidebar scroll NOT restored: fresh process always opens Dashboard at nav scroll 0 (v5 launch-state observation; earlier "restore" appearance was an artifact of an orphaned process — corrected in SESSION-LOG) | E-005, E-030, E-031 |

## Counts (updated 2026-10-24, after pass 1)

- Pages inventoried with runtime evidence: 23/23 (F-001..F-023)
- Shell-level features inventoried: 3 (F-024..F-026)
- Controls/options enumerated so far: page-level primary controls listed above
  (pass 1 depth — every visible primary control named); full per-page control
  ledger (FEAT-002, incl. states/validation) NOT STARTED
- Navigation test steps executed: 23 click-steps + 1 normalize click + 4 wheel
  scrolls + 1 sliver click = 29 real-input steps; 29/29 verified
- Workflows: none executed yet (WORKFLOW-COVERAGE.md — all queued)
- Findings referenced: FIND-001 (loopback port), FIND-002 (single-instance
  message), FIND-004 (sidebar bottom-row overlay; opened during this walk)

## Open items feeding pass 2 (FEAT-002/003)

- Per-page control ledger with states, constraints, validation messages
  (Settings tabs — especially Help Scout & LM Studio — Knowledge import,
  Incidents filters, Reports tabs, Connectors form, Diagnostics clear-log).
- Command palette runtime behavior (Ctrl/Cmd+K) — entry point claimed in
  source/tests; not yet exercised (queued W-008).
- Theme toggle round-trip (queued W-007).
- Settings → Help Scout health-check message for the loopback listener
  (FIND-001's UI-visibility sub-check).
