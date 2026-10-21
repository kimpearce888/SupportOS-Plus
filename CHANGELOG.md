# Changelog

All notable changes to **SupportOS+** (SupportOS-Plus) are documented here.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/);
versions follow [Semantic Versioning](https://semver.org/).

## [1.4.0] — 2026-10-21

The "trust and truth" release: Phase R of the v2 blueprint completes
(every remaining defect in the audit register that gates trust is
closed), and the first two items of Phase A turn the AI from a stub
into a real, routed, measured system.

### Removed — honesty about the platform

- **The sync sweep no longer pretends Help Scout lists things it
  cannot.** The chats, attachments and ratings stages were silent
  no-ops or empty stubs: Help Scout exposes **no list endpoint** for
  Beacon chats (`GET /v2/chats/{id}` only) or ratings
  (`GET /v2/ratings/{id}` only) — both arrive through webhooks and are
  fetched by id. The dead sweep stages are gone (migration m052 drops
  their stale checkpoints), `list_ratings`/`list_beacon_chats` on the
  real provider now state that platform reality as errors instead of
  silently returning empty, and the misdesigned 30-second ratings
  poller is retired (ratings keep arriving through the satisfaction
  webhook path, which was always the real one). The Reports page says
  in one line where its rating counts come from and that Help Scout's
  own happiness reports need a Plus/Pro plan.

- **No simulated data anywhere, provably.** The `FakeAiProvider` and
  `NoopAiProvider` demo doubles (zero callers), the fabricated
  "Simulated attachment content…" download body, the dead simulated
  rating job handler, the legacy verified-draft and interaction-signal
  CRUD with no callers, and the stale boot comments are gone. A new
  **CI no-simulation gate** (`scripts/gate_no_simulation.py`, wired
  into both CI and the release workflow) fails the build if production
  code ever reintroduces simulated/fake/demo data markers — comments
  and test fixtures excluded, an explicit allowlist for the purge
  machinery itself.

### Security

- **OAuth tokens leave plaintext sqlite.** Access and refresh tokens
  now live in the AES-256-GCM encrypted envelope; the `oauth_tokens`
  row keeps metadata only (expiry, scope, grant times, revoked flag)
  and its token columns are permanently empty. Migration m053 seals
  pre-1.4.0 plaintext tokens on the first keyed boot and scrubs the
  columns; revocation and disconnect purge the envelope entirely. The
  connection status reads through the same envelope-backed reader, and
  a corrupt envelope surfaces as an error, never as "not connected".

### Added

- **Version transparency.** Settings → About & updates shows the
  running version against the latest published GitHub release, with a
  strictly user-initiated check (the app never phones home on its
  own), honest 404 messaging for a private/unreachable repository, and
  a semver-aware comparison.

- **Adaptive sync cadence + the freshness ring.** The fixed-interval
  poll is replaced: 30 s while the account is busy, backing off toward
  the configured ceiling when quiet, and an immediate poll after every
  send. `SYNC_INTERVAL_MINUTES` is finally consumed (as the ceiling
  fallback), the two divergent defaults are unified, and the header
  now carries a global freshness ring — "syncing…", "synced 12s ago",
  "never synced" — color-coded by age and clickable through to Sync
  Health. Every refresh control performs a real scoped fetch: the
  Dashboard, Reports, Customers and Organizations "Reload" buttons
  became real "Sync now" actions that report their outcome (a
  click during a running sync is answered, not swallowed).

- **The AI provider hub (Phase A begins).** Endpoints become a list —
  the default local LM Studio, an optional Ollama, and any number of
  OpenAI-compatible cloud endpoints (BYOK, keys in the encrypted
  envelope, off by default per the privacy decision). Every inference
  routes through one of five model roles (Embed / Classify / Summarize
  / Draft / Agent, blueprint Table 6); per-endpoint health probes
  replace guesswork; per-feature toggles plus a global kill switch
  live in AI Center; and a per-feature cost ledger aggregates tokens,
  latency and estimated spend per day from `ai_runs` (token usage now
  actually recorded from every chat call).

- **Triage-on-arrival goes live.** The 14-key attribute extraction now
  runs for real: deterministic signals always land (no model needed —
  urgency, question count, frustration, escalation, response style),
  the AI layer fills intent/product/feature/issue/customer goal when a
  Classify endpoint is routed, the AI Center Attributes tab and the
  `ai_attribute` automation conditions read live rows, and the inbox
  header shows intent/urgency/sentiment/topic chips with confidence —
  click to correct, and the override supersedes the machine row with
  source "human". Issue clustering generates real Issue Radar clusters
  (daily maintenance trigger plus a "Cluster now" button). Customer
  memory extraction runs on arrival, feeds a quarantine-filtered
  "know before you reply" briefing panel in the inbox, and rides the
  evidence pack into every analysis and draft prompt.

### Fixed

- The "sync interval" setting is now the honest "max quiet sync
  interval" with the adaptive behavior explained on hover.

## [1.3.0] — 2026-10-20

The "it actually does things" release. Replies, notes, status changes
and assignments now really reach Help Scout; the inbox streams
conversations in while the sync runs instead of freezing; every page
has working refresh; a zero-configuration Copilot panel reads the
conversation, finds the same issue in history and drafts an answer;
and the Help Center sync talks to the real Docs API (v1) instead of a
version that does not exist.

### Fixed

- **Replies, notes, status changes and assignments are now actually
  delivered to Help Scout.** Since the desktop port the composer was
  enqueue-only: rows landed in an `outbound_jobs` table that nothing
  ever dispatched, while the UI reported green success ("Reply queued
  as job #N"). A real dispatcher now claims each job on the 2-second
  worker tick, performs the provider call (with the `customer` object
  the Mailbox API requires, which was missing), records the attempt,
  retries with exponential backoff, pulls the remote truth back into
  the mirror, and reports `OutboundJobFinished` — success toast, or a
  red error toast plus a Diagnostics entry with the exact API error.
  Stuck "sending" rows recover at boot; migration m051 gives existing
  rows a 3-attempt budget.
- **Honest outcomes everywhere.** Rejections (eval mode, duplicates,
  "not connected") render in red instead of green success text, keep
  the typed composer text instead of wiping it, and the status line
  now shows the real delivery state per conversation: "sending to Help
  Scout…" → "delivered ✓" / "failed: <exact error>". Sending with no
  credentials is rejected up front with instructions instead of
  queuing into a provider that cannot deliver.
- **The open conversation survives sync events.** Every background
  event used to wipe the detail pane back to "Select a conversation"
  a frame after you sent something. Selection is preserved and the
  detail reloads.
- **"Sync in progress" no longer means a frozen screen.** The engine
  now emits per-resource and per-page `SyncProgress` events: Sync
  Health shows live "resource 3/22 · 1,204 records" and the inbox
  streams conversations in as they ingest. A periodic repaint
  heartbeat keeps status lines moving even without mouse input.
- **A crash can no longer strand the sync state machine.** Boot
  recovery resets a `sync_state` stranded in INITIALIZING/BACKFILLING
  by an unclean shutdown (which used to make the auto-sync timer skip
  itself forever), and the engine's running flag is now panic-safe
  (a panic mid-run used to answer every later trigger with "A sync is
  already running").
- **The initial sync no longer walks every conversation twice.** The
  "threads" resource previously repeated the full conversation walk
  (including thread fetches); it now only backfills conversations that
  came back thread-less. On a large account this halves the first
  sync.
- **Faster polling: the default sync interval drops from 5 minutes to
  1 minute.** Help Scout webhooks cannot reach a localhost desktop
  app (they need a public URL), so "real time" here means fast
  polling; existing installs on the old 5-minute default move to 1
  minute via migration m051.
- **Help Center sync now talks to the real Docs API.** The provider
  called `/v2/collections` — the Docs API only exists as v1, so every
  request 404'd and the KB never filled even with a valid key. The
  client now uses the v1 endpoints with the correct `{"items"}`
  envelopes, paginates collections/categories/articles, and fetches
  each article's full content (`GET /v1/articles/{id}` — v1 lists
  return refs without text). A missing Docs API key is surfaced as a
  banner with setup instructions instead of a silent zero.
- **AI jobs no longer look alive when they are dead.** A transport
  failure (LM Studio not running, wrong model name) used to leave
  `ai_runs` rows stuck at "running" — displayed with a green success
  badge. Runs are now marked failed, and "running" renders neutral.

### Added

- **Refresh everywhere.** "Sync now" on the Inbox header (a real
  incremental pull), per-conversation "refresh from Help Scout",
  "Sync Help Center now" on Knowledge, "Reload" on Dashboard,
  Reports, Customers and Organizations — all backed by the shared
  sync-action helper rather than re-reading a stale cache.
- **Reports auto-refresh.** The Reports page was missing from every
  invalidation map: after the first load it stayed frozen until a
  restart. It now refreshes on every sync/conversation event and has a
  Reload button.
- **Copilot panel in the conversation view — zero configuration.**
  Local, offline, deterministic: extractive summary of what the
  customer needs, open questions, intent/urgency/sentiment/escalation
  chips, the customer's history, **similar past issues** with
  resolution excerpts and match reasons, relevant Help Center
  articles, and an **evidence-grounded suggested reply** (built from
  the best past resolution and help articles — every line cites its
  source, nothing is hallucinated) that inserts into the composer with
  one click. With LM Studio configured, "Draft with LM Studio" runs
  the generative pass on top.
- **Live sync progress on Sync Health** (per-resource stage, running
  engine indicator) and a clearing action banner — the "started in
  background" message no longer lingers forever.
- **Outbound deliveries card on Diagnostics:** delivered / queued /
  sending / failed counts plus the exact error text of recent failed
  deliveries.

### Changed

- Migration m051 (schema v42): outbound retries get a budget, stuck
  "sending" rows re-queue, the 5-minute default poll moves to 1
  minute.
- The reply request now includes the `customer` object required by
  `POST /v2/conversations/{id}/reply`, and thread ids are read from
  the `Resource-Id` header (the 201 response body is empty).

## [1.2.0] — 2026-10-20

The "nothing fake" release. The demo dataset and its boot path are
gone entirely: a fresh install starts empty, credentials alone are
enough to connect and receive real data automatically, and every
error the app records is now visible on a dedicated Diagnostics page
instead of vanishing into stdout logs.

### Removed

- **The entire demo/placeholder dataset and every path that served
  it.** The simulated Help Scout account (fake customers,
  conversations, threads, ratings, knowledge, incidents — roughly
  1,500 lines of fixture content), the `new_demo()` provider, the
  demo boot mode, the `LOCAL_DEMO_MODE` override, the demo onboarding
  button, the simulated-activity demo tools on Sync Health, and the
  `xtask seed` command are all deleted. The app now boots in exactly
  two states: **Real** (credentials configured) or **Empty** (an
  inert provider that serves nothing and simulates nothing). Existing
  demo content in older databases is wiped by the new
  *Settings → Help Scout → Workspace data* reset (which also arms a
  fresh first sync), and migration m050 deletes the retired
  `demo_mode` / `demo_content_purged` settings rows.

### Fixed

- **Saved credentials never pulled any data.** Two compounding
  defects: (a) `access_token()` returned a bare 401 when no token
  row existed, so a user who saved their Client ID/Secret but never
  pressed a connect button got nothing — and no visible error; (b)
  the five-minute auto-sync timer treated `sync_state = NEW` (never
  synced) as "do nothing", and nothing in the production path ever
  started the initial sync — the only triggers were manual buttons
  on Onboarding/Sync Health. Now the client-credentials grant signs
  in automatically whenever credentials are stored (and re-logs in
  when a token expires without a refresh token), and a `NEW` state
  kicks off the initial sync immediately at boot: save credentials,
  restart, data arrives.
- **Errors were invisible.** Sync failures, rejected logins and
  webhook rejections only went to the terminal (or a DB table no
  page displayed). They are all recorded in the diagnostics event
  log now — see Added.

### Added

- **Diagnostics page** (System group): the persisted event log
  (`application_errors`, extended by migration m050 with an
  info/warn/error `level` column and a 500-row cap) with level
  filters, expandable details and a clear button, plus a sync-run
  report table (kind, state, records processed, errors). Writers:
  every failed sync resource (with the resource name and reason),
  run summaries, automatic client-credentials sign-ins and their
  failures, OAuth callback outcomes, unsigned-webhook rejections and
  automatic-sync failures.
- **Migration m050** (`m050_diagnostics_log`): the level column, a
  level-aware index, and removal of the retired demo settings rows.

### Changed

- *Settings → Help Scout → Demo data* became **Workspace data**: it
  now deletes all synced content (and any pre-1.2.0 demo leftovers),
  keeps credentials and configuration, and sets `sync_state = NEW`
  so the next start pulls a connected account fresh.
- Onboarding no longer offers demo mode; its status card counts
  saved credentials as "configured" (the 1.1.x check only saw
  stored tokens, missing the freshly-connected state).
- Unsigned webhook events with no configured secret are always
  rejected (the old demo-mode allowance died with demo mode).

## [1.1.4] — 2026-10-20

Hotfix for the two 1.1.3 features: user reports showed both the
demo-data purge and the Help Scout connect flow misbehaving on real
installs. A fresh end-to-end audit found four concrete defects — the
purge deleted rows but let the running session re-sync them minutes
later, and the connect flow dead-ended in two different ways before
the token exchange (which itself was verified correct against the
current Help Scout API).

### Fixed

- **Demo data reappeared after the purge.** Three re-seed paths
  survived the delete: (a) the purge kept `sync_state = LIVE` in the
  retained application settings while wiping the sync checkpoints, so
  the five-minute auto-sync timer saw "everything is new" and
  re-pulled the whole fake dataset from the still-running demo
  provider; (b) the 30-second ratings timer re-inserted fake ratings
  immediately (the upsert stored ratings whose conversation had just
  been deleted); (c) any restart rebuilt the demo provider whenever
  credentials were missing, so the purge message's own "restart
  SupportOS+" advice resurrected the data. All three are closed: the
  purge now exits demo mode atomically inside its transaction
  (`demo_mode` off, a `demo_content_purged` marker on, sync state back
  to `NEW` so even the still-running session stops auto-syncing), the
  ratings refresh skips ratings whose conversation is referenced but
  no longer mirrored, and boot picks the provider explicitly — real
  with credentials, **empty** (not demo) after a purge, demo
  otherwise. Re-enabling demo mode from the onboarding page
  deliberately clears the marker and brings the demo dataset back.
- **"Connect via OAuth" failed right after saving credentials.**
  Saving flipped the demo-mode setting immediately, so the connect
  button appeared in the same session — but the OAuth callback
  listener had started with the demo provider and answered every
  browser callback with "Demo mode is active - OAuth is not needed."
  The button now knows whether this session actually runs the real
  provider: before the restart it explains that a restart is required
  instead of offering a flow that cannot complete.
- **A busy callback port silently killed the OAuth flow.** Nothing
  verified the listener really bound its port (another program on
  3000 is the classic case) or that it is *our* listener. The connect
  button now probes `GET /healthz` and reports "listener not
  answering on port N" with concrete remedies; the settings page also
  shows the redirect URI that must be registered on the Help Scout
  custom app (`http://localhost:3000/oauth/callback` by default) —
  previously that requirement was documented nowhere in the app.
- **The callback fail page no longer lies.** With no real provider in
  the session it said "Demo mode is active" even when demo mode was
  off (empty post-purge session or credentials saved after boot); it
  now describes the actual situation.

### Added

- **Connect with client credentials** (Settings → Help Scout): no
  browser round-trip — verifies the stored client ID/secret against
  Help Scout and stores the token directly, reporting success or the
  friendly rejection message inside the app. Also the fastest way to
  test whether saved credentials work (the 1.1.3 loopback error page
  referenced this button; it did not exist until now).

## [1.1.3] — 2026-10-20

Two frequently-requested workflow features, both fully usable from the
interface — no environment variables, no shell, no data-directory
surgery.

### Added

- **Help Scout credentials can now be entered in the app**
  (Settings → Help Scout): client ID, client secret, webhook secret
  and Docs API key are stored in the encrypted secrets envelope
  (AES-GCM under the master key — the same envelope that already
  guards connection secrets; the plaintext never enters the
  database). Saving a configured client ID/secret pair turns demo
  mode off automatically, so the next launch boots the real provider.
  Per-field badges show where each value comes from — environment
  variables still win per field (CI, power users) — and "clear
  stored" removes the saved values without touching the connection.
  The "connect via OAuth" button now appears whenever credentials
  resolve from either source.
- **One-click demo-data purge** (Settings → Help Scout → Demo data):
  deletes every conversation, customer, thread, rating, knowledge
  document, incident, report and AI record — all demo-seeded or
  synced content — in one transaction, and resets the sync cursors
  and the vector store so the real account starts clean. Keeps
  credentials, application settings, business hours, automation
  rules, saved views and replies, connector configs and custom
  object types. The purge walks the live schema against a pinned
  KEEP allowlist (a test fails the build when a KEEP name goes
  stale), runs with foreign keys suspended like the boot migrations
  and commits only when `PRAGMA foreign_key_check` is clean — a
  failure rolls everything back. Two-step confirmation.

## [1.1.2] — 2026-10-20

Hotfix for the upgrade path from 0.2.x data. Migration 36
(`m045_threads_actor_model`, the `conversation_threads` table rebuild)
died with `UNIQUE constraint failed: ratings.id` on any database whose
foreign-key child tables already held rows — which is every real 0.2.x
database and none of the empty ones CI tests with. The database was
left untouched at schema version 35 (each migration is one
transaction), so installing 1.1.2 simply resumes and completes the
upgrade.

### Fixed

- **Migration 36 failed on real 0.2.x data** ("failed to open SQLite
  DB: migration 36 failed: UNIQUE constraint failed: ratings.id"):
  table-rebuild migrations captured the child rows of the rebuilt
  table in order to restore them afterwards, but the capture only
  *read* the rows — the restore then re-inserted rows that had never
  been removed, and the first non-empty child table (`ratings`)
  tripped its primary key.
- **Rebuild migrations no longer fight SQLite's live foreign-key
  machinery.** Enforcement is now suspended for the duration of each
  migration step: the drop-and-rename dance at the heart of every
  rebuild fires immediate cascade actions against rows that are about
  to come back under the rebuilt table, and SQLite's deferred-violation
  counter never forgets a violation that a rename has long since
  repaired. Integrity is enforced instead by an explicit
  `PRAGMA foreign_key_check` sweep inside each step's transaction — a
  step that would commit a dangling reference rolls back whole, which
  is a strictly stronger guarantee than before.
- **Dangling references are pruned explicitly.** Rebuild copies may
  drop rows (orphaned threads whose conversation no longer exists,
  merged conversations, ...); a breadth-first walk now deletes every
  row that references a missing row of the rebuilt table, recursively
  through the dependents, so no orphan survives to the check. The
  central fix covers every rebuild migration (`m045` threads, `m046`
  customers/attachments and friends, `m047` conversations).
- Regression coverage for the whole class: seeded 0.2.x-shaped
  fixtures (threads with `actor_id`, ratings and attachments as
  foreign-key children, customers without the `organization_id` FK)
  are driven through the `m045` rebuild and the `m046` customers
  rebuild, pinning id preservation, actor-column mapping, orphan
  cleanup and foreign-key-clean commits. Two interim fix designs were
  rejected by these tests: a capture-side delete (cascade-deleted
  every thread during the customers rebuild) and UPDATE-in-place
  restoration (the deferred-violation counter blocked the commit).

## [1.1.1] — 2026-10-20

Hotfix for 1.1.0's startup failure on real machines. The shipped binary
rendered with eframe's default `wgpu` backend, which needs a working
Vulkan driver or a GLES stack — neither is guaranteed outside CI, where
the GUI-launch tests install `mesa-vulkan-drivers` (software Vulkan).
On machines without them (VirtualBox/VMware VMs, NVIDIA with nouveau,
older Mesa, minimal desktops) the process died before opening a window,
and because the desktop entry discards stderr, it looked like "nothing
happened".

### Fixed

- **Startup failure on systems without Vulkan/GLES**: the shell now
  compiles in the GL-based `glow` renderer (already used by the CI
  screenshot harness) and makes it the default. `glow` opens wherever
  GLX or EGL desktop GL exists, including llvmpipe software GL on
  GPU-less machines. `SPP_RENDERER=wgpu` opts back into wgpu for
  systems where GL is the broken half instead; unrecognized values
  fall back to `glow` with a warning.
- **Startup failures are now findable**: fatal errors (instance lock,
  database, background workers, renderer/window setup) append to
  `logs/fatal.log` inside the data directory, and a panic hook records
  panics there too — including renderer and context-setup panics, the
  failure class that made 1.1.0 vanish silently. When `zenity` or
  `notify-send` is installed, the failure is also shown as a dialog.
- `.deb` now declares `libgles2` (wgpu's GL fallback `dlopen`s it);
  1.1.0 machines that had neither a Vulkan ICD nor `libGLESv2` lost
  their last fallback. The declared dependency set is otherwise
  unchanged.

## [1.1.0] — 2026-10-20

Interface refresh: the entire application now ships a shadcn/ui-inspired
design system. Presentation only — behavior, data model, on-disk layout
and the 0.2.x upgrade path are untouched.

### Changed

- **Complete visual redesign** on a new in-repo design system
  (`app-shell/src/design.rs`, zero new dependencies): zinc light and dark
  token sets (WCAG-AA contrast, unit-tested), a modern type scale, and a
  card / badge / chip / button / metric / table component family with
  status-tone color mapping replacing hard-coded colors.
- **Lucide-style icon set** — 42 stroke icons drawn with the egui painter
  (`app-shell/src/icons.rs`), mapped per page; no icon-font dependency.
- **Redesigned shell**: full-height grouped sidebar (Workspace /
  Customers & knowledge / Insights / Automation & AI / System) with brand
  block and command-palette trigger row; app header with search and theme
  toggle; sun/moon switch pinned at the sidebar bottom.
- **All 21 pages restyled**, plus the command palette, toasts and
  confirmation dialogs; dark mode is now a fully designed theme instead
  of the built-in palette swap.

### Fixed

- Page content no longer vanishes below the fold: every page scrolls
  vertically as a whole (the two-pane inbox scrolls its detail column).
- Uniform 32 px control heights for design-system buttons; palette and
  dialog open without fade-in ghosting; chart annotations no longer clip
  their card frame; long ISO timestamps no longer overflow chart cards;
  inbox preview/time collision fixed with compact time rendering;
  operations tiles re-spaced; duplicate hero empty states removed from
  the incidents page.

## [1.0.0] — 2026-10-20

First stable release. No functional or schema changes since 0.2.0 — this
release completes the product identity and locks in the upgrade path.

### Changed

- **Completed the SupportOS-Plus rename.** The installed command is now
  `supportos-plus` (was `supportos-plusplus`), matching the Debian package
  name: `apt install supportos-plus` now provides the `supportos-plus`
  command. Internal crates are renamed accordingly
  (`supportos-plus-{core,catalog,app-shell,xtask}`).
- **Data directory moved** to `$XDG_DATA_HOME/supportos-plus` (was
  `.../supportos-plusplus`); the database file inside is now
  `supportos-plus.db` (was `supportos-plusplus.db`).

### Added

- **Automatic, one-time data migration from the 0.2.x layout** on first
  launch: the whole legacy directory (database, `secrets.key`, vector
  store, backups) is moved with a single atomic `rename(2)` and the
  database file is renamed inside it. No-op on fresh installs, repeat
  runs, and `SPP_DATA_DIR` overrides; never overwrites existing data.
  Downgrading to 0.2.x afterwards is not supported.
- This changelog.

### Fixed

- CI and release-pipeline references now use the renamed crates and
  binary paths throughout, and the release verification suite gained a
  dedicated upgrade test: a seeded 0.2.x data directory must migrate
  in place (database, key material and vector store preserved, legacy
  directory consumed) before the package is publishable.

## [0.2.0] — 2026-10-19

First install-ready release: Linux `.deb` + `AppImage`, built and
verified end-to-end by the tag-triggered release pipeline.

### Added

- Full offline-first support-desk console: unified inbox, customers,
  knowledge base, docs, reports and settings pages over an embedded
  SQLite store with all boot migrations; zero external services.
- Local-only integrations: Help Scout sync/webhooks/OAuth via a
  loopback-only listener; LM Studio (OpenAI-compatible) chat and
  embeddings; encrypted secret envelope (`aes-gcm` + `scrypt`); semantic
  search over knowledge/docs/conversations via an embedded
  `sqlite-vec` vector store.
- Landlock + seccomp sandbox around untrusted PDF/ZIP parsing
  (release builds ship it enabled).
- Packaging: type-2 AppImage (self-contained incl. the X11 keyboard
  stack) and a `.deb` declaring the full X11/GL/Wayland client-library
  dependency set for APT auto-resolution; desktop entry, hicolor icons,
  first-run self-initialization, single-instance lock.
- CI: fmt + clippy (`-D warnings`) + tests + cargo-deny on every push,
  plus a package job that builds and smoke-boots both formats.

[1.1.0]: https://github.com/kimpearce888/SupportOS-Plus/releases/tag/v1.1.0
[1.0.0]: https://github.com/kimpearce888/SupportOS-Plus/releases/tag/v1.0.0
[0.2.0]: https://github.com/kimpearce888/SupportOS-Plus/releases/tag/v0.2.0
