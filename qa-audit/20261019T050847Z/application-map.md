# Application Map — QA Audit 20261019T050847Z

Discovered structure of SupportOs-Plus at commit
`19dfb35a714822ed94b6b5144eabb222997cc2ca`, as mapped by the audit from
source, runtime logs, and GUI sessions.

## What the product is

A native desktop application for customer-support operations (Help Scout
mirror + AI assistance), built in Rust as a workspace:

- **`crates/core` (`spp_core`)** — the application core, UI-independent:
  SQLite persistence (49 migrations, FTS5 search), background workers,
  Help Scout provider + sync engine, LM Studio / OpenAI-compatible AI
  client, encrypted secret envelope, search, automation rules, analytics.
- **`crates/app-shell`** — the egui/eframe desktop UI (`supportos-plusplus-app-shell`),
  21 pages plus a command palette and modal overlays.
- **`crates/xtask`** — build tooling (AppImage packaging path).
- One deliverable binary + `SupportOs-Plus-x86_64.AppImage`.

## User-facing interfaces

1. **Native GUI window** (1180×760, `ViewportBuilder::with_inner_size`):
   left sidebar with 21 page rows + a `⌘K search` button; content area;
   command-palette overlay (Ctrl/Cmd+K or button); modal dialogs
   (`overlays.rs`).
2. **Loopback HTTP listener** (ephemeral port, observed 13535/13754/…):
   `POST /webhook` (Help Scout webhook ingress, HMAC-SHA1 verified) and
   `GET /oauth/callback` (OAuth authorization-code redirect target).
   Bound to 127.0.0.1 only.

## Pages (21) and their control counts (from `control-inventory.csv`)

| Page | Controls | Page | Controls |
|---|---|---|---|
| Inbox | 42 | Notifications | 4 |
| Incidents | 31 | Operations | 4 |
| Knowledge | 27 | Reports | 4 |
| Settings | 23 | Support Graph | 4 |
| Outreach | 21 | App shell chrome | 4 |
| Custom Objects | 20 | AI Center | 3 |
| Automation | 19 | Customers | 3 |
| Connectors | 16 | Onboarding | 3 |
| Issue Radar | 11 | Command palette | 3 |
| Dashboard | 8 | Sync Health | 2 |
| Organizations | 6 | Overlays | 1 |
| Search | 5 | | |

Total discovered actionable controls: **264**.

## Background machinery (observed live on every boot)

- Job queue + webhook + embedding + maintenance workers with timers;
  auto-sync timer armed at 5-minute intervals.
- Ratings refresh worker (pushed 7 ratings on each fresh boot).
- Automatic backup worker (created a backup within the first second).
- 49 sequential migrations applied on first boot of a fresh data dir
  (~0.5 s cold), then `database ready`.

## Data boundaries

- **SQLite database** at `${SPP_DATA_DIR}/supportos-plusplus.db` —
  per-profile, created/migrated on boot (observed in every session log).
- **`secrets.key`** — fresh per-profile master key for the encrypted
  secret envelope; `settings::set_secret` wraps stored tokens.
- **Backups** written under the data dir (`backups/…settings.json` observed).
- No telemetry, no crash reporting, no outbound network except:
  Help Scout API (on user-configured sync), LM Studio (user-configured
  endpoint, default `127.0.0.1:1234`).

## External dependencies

- Help Scout (api.helpscout.net): OAuth2 authorization-code flow, mailboxes,
  conversations, customers sync — all credential-gated.
- LM Studio or any OpenAI-compatible local server (optional, for AI
  features).
- Neither is required to boot or use the UI (fresh profiles run fully
  offline; the audit ran everything without credentials or external
  network).

## Feature inventory (behavioral, cross-checked with tests)

Conversations/inbox mirror · customers/organizations · knowledge base with
FTS5 search + import · reports · issue radar (known issues) · incidents ·
custom objects · automation rules · AI center (analysis, prompts,
evidence, memory guard, tools) · outreach · notifications · global search ·
support graph · connectors · sync health (engine state, cursors) ·
settings (theme persisted to DB, integrations) · onboarding · shell
diagnostics (theme toggle, schema version, worker/event counters) ·
command palette (Ctrl/Cmd+K overlay searching tickets/customers/knowledge/
issues, resolves web-style hrefs into shell navigation) · webhook ingress ·
OAuth connect flow · automatic backups · demo/simulation actions on the
Operations page (simulate incoming message, knowledge import, known-issue
seeding — each runs through the real job pipeline).
