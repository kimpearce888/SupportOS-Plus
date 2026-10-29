# Changelog

All notable changes to **SupportOS+** (SupportOS-Plus) are documented here.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/);
versions follow [Semantic Versioning](https://semver.org/).

## [1.9.0] — 2026-10-29

### Changed — Design-System reskin (the presentation layer, rebuilt on DC v1.0)

The owner-approved reskin (decisions.md D6: "everything is working fine, we need
to reskin only according to the new design system") ships in full. Every one of
the app's 23 routes now renders through the Design Contract v1.0 widget set —
the contract was extracted from the owner-approved design lab (Gate G1, commit
`af5bc2a`), and the Rust port never leads it. **No feature, data, workflow,
shortcut or page-set change**: every query, write, dialog and key binding is
byte-identical to 1.8.x; data-layer defects surfaced during the rebuild are
footnoted as receipt-cited quirks (DC C-1) and stay visible.

- **R1 — tokens (RS-12):** every `--ds-*` design token ported 1:1 into
  `design.rs` (LIGHT + DARK); DS.TONE is the single status→color map; the
  style lint (CI) bans raw color sites and off-scale spacing.
- **R2 — widget sets (RS-13/14/15):** the 17 canonical DS-C components (cards,
  tables, tabs, chips, buttons, forms, empty states, callout roles, toasts,
  detail panel, avatars, role-typed message cards, composer, AI identity set,
  console hero) as typed egui widgets — one DC sheet, one widget.
- **R3 — page waves (RS-16..RS-20):** all 23 routes rebuilt onto the six DS-A
  archetype recipes, with the archetype's presentation fixes applied (counter
  pills, filter bars, bar lists, danger zones, hero band) and every interactive
  control carrying its visible label for accessibility (`Response::labelled_by`
  / `widget_info`) — zero visual change from the lab.
- **Locked against drift:** 23 kittest suites / 152 tests / 168 goldens lock the
  real pages (not mocks) at 1440/1024/800 × 900, light + dark, diff-locked
  twice; CI now runs them on every push (the new `snapshots` job, lavapipe
  software Vulkan) — a visual regression on any page fails the build.
- **Traceability gate (RS-21):** the R4 matrix
  (`docs/audit/acceptance/traceability-matrix.md`, screen region → DS-C id →
  DC row) is machine-checked in CI against the frozen contract and the route
  list; the acceptance records are folded into the printable report series
  (`docs/audit/SupportOS-Plus-Reskin-Acceptance-Report.pdf`).
- **Acceptance instruments (RS-21):** the 5-second AI-identification blind-test
  protocol and the SUS questionnaire (target ≥ 68) ship under
  `docs/audit/acceptance/` for the G4 final-acceptance administration.

## [1.8.3] — 2026-10-28

### Fixed — Diagnostics event log
- **The event log could be permanently empty in perfect silence** — a
  database that reached the current version with the pre-m050 table
  shape (a skipped or renumbered boot step, a restored backup) made
  every diagnostics write die with "no column named level" — an error
  every call site swallows — while every read returned an honest-looking
  empty list. "The event log reports nothing" was the visible result.
  `ensure_tables` now guarantees the full schema on **every** write,
  read and clear: new installs get the `level` column from the first
  CREATE, older shapes are self-healed in place (ALTER), and the m050
  migration shares the same one canonical guarantee. A write that still
  fails is now audible in the stdout log instead of vanishing.
- **Webhook-health failures never reached the event log** — the Diagnostics
  page promises "webhook rejections", but the health check only ever
  wrote its outcome to the Sync Health snapshot. Failed checks (error)
  and healed hooks (info) are now persisted to the event log too.
- **Seventeen zombie sync runs** — runs killed mid-flight (app closed
  during a sync) kept their `sync_runs` row in INITIALIZING forever, so
  Sync reports read as if nothing ever finished. At worker start, before
  any sync can be running, orphaned INITIALIZING/BACKFILLING rows are
  closed out as CANCELLED with a finished timestamp, and the cleanup is
  announced in the event log.

## [1.8.2] — 2026-10-28

### Fixed — Help Scout integration
- **Webhook registration rejected with 400 (`path: "url" — must be a
  valid URL.`)** — the platform's url validator refused the relay URL the
  health check sent (the live account hit this on 2026-10-27). The check
  now **normalizes the relay URL before anything is sent**: surrounding
  whitespace is trimmed, a missing scheme becomes https (the classic
  "funnel address pasted without the prefix"), and plain http is upgraded
  to https as in 1.8.1.
- **Undeliverable relay URLs are refused with the fix, not echoed as an
  opaque 400** — loopback, private, link-local and CGNAT hosts (127.0.0.1,
  192.168.x, 10.x, 100.64/10 — including Tailscale node IPs) never reach
  the API; the outcome instead names the stored value and says exactly
  what to set in Settings (a public https relay address such as
  `https://your-host.ts.net/webhook`).
- **Registration failures name the url they sent** — if the platform
  still rejects a registration, the error now carries `(the url we sent:
  …)` so any future rejection can be diagnosed from the Sync Health
  outcome alone.

## [1.8.1] — 2026-10-28

### Fixed — Help Scout integration
- **Webhook registration rejected with 400 ("Expected array of values
  from this list")** — Help Scout retired four event names the app
  subscribed to (`convo.updated`, `beacon.chat.agent.replied`,
  `beacon.chat.assigned`, `beacon.chat.ended`), and the platform rejects
  the whole registration when even one name is stale, leaving the webhook
  health check stuck "failed" forever. The shipped list now matches the
  platform's 2026-10 catalogue (and re-subscribes `convo.tags` /
  `convo.custom-fields`, documented valid again), and registration
  **self-heals**: if the platform ever retires a name again, the accepted
  values are read straight out of the 400 body and the subscription
  retries with the intersection — no release needed. The outcome names
  what was dropped.
- **Plain-http relay URLs are upgraded to https:// before registering**
  — Tailscale Funnel (and every other TLS relay) redirects http, and a
  hook registered against a redirecting URL would be disabled by Help
  Scout after ten bounced deliveries. Loopback URLs are exempt.

## [1.8.0] — 2026-10-27

The complete audit-fix release: every finding from a full source audit plus
live end-to-end testing against a real Help Scout account — 4 Critical,
8 High and 25 Medium issues — fixed and re-verified against the live API.

### Fixed — Help Scout integration
- **Conversation creation never worked against the real API** — the wrong
  API version was used (`/v3` instead of `/v2`), the new conversation's id
  is actually delivered in the `Resource-Id` response header (the app read
  a body field that does not exist), and Help Scout requires top-level
  `type` and `status` fields. Outreach campaigns and every other outbound
  create now deliver end to end.
- **Help-Center sync silently synced nothing** — the Docs API wraps every
  list response in a resource-named envelope (`collections` /
  `categories` / `articles`) the app did not unwrap, and ids are hex
  strings. Collections, categories and articles now sync, and the
  Knowledge page no longer shows a false "Docs API key missing" banner
  while sync works.
- **Customer contact channels were never fetched** — emails, phones,
  websites, chats and social profiles live behind per-customer
  sub-resources. The Customers page and Outreach recipient lists now
  populate from real data.
- **Upgrading from 1.7.3 now delivers the customer chat-channel mirror** —
  the table had been introduced by editing an already-applied migration in
  place, so upgraded databases (recorded version 52) never received it and
  the first chat-channel sync would fail. A new idempotent migration
  (version 53) creates it on upgrade and is a no-op on fresh installs.
- **Search was permanently empty** — nothing ever enqueued the index
  rebuild. The full-text index is now built at ingest and refreshed after
  every sync pass.
- The incremental-sync date filter used a timestamp format the real API
  rejects.
- Thread authorship is parsed from Help Scout's polymorphic `createdBy`
  object — attribution, automations and SLA reporting see the real author
  — and thread recipients accept the real plain-string `to`/`cc` arrays.
- SLA first-response math keys on real thread types and excludes customer
  replies, so "first response" measures agent responses only.
- Sync robustness: one poisoned record can no longer abort a whole sync
  stage; every crawl has page caps and in-loop cancel checks; the Docs
  fetch frontier rotates past the 500-article cap; Docs requests retry
  429/5xx with backoff; rate-limit `Retry-After` is honored up to 300 s;
  merged-conversation 301s are surfaced instead of silently followed;
  the webhook relay refuses to register with an empty secret; duplicate
  webhook events no longer overwrite the original event's state.

### Fixed — data safety
- Encrypted-bundle restore now stages and fsyncs the imported database and
  moves the live DB + WAL aside as one unit — a crash can no longer pair
  the fresh database with a stale WAL.
- Plaintext export snapshots, import temps, backups and PII exports are
  created with 0600 permissions, and stale temp files are swept.
- Workspace reset uses `secure_delete` + VACUUM — freed pages no longer
  carry recoverable customer data.
- Crafted `.sosync` bundles cannot force out-of-range scrypt work factors;
  requeuing stale running jobs is executor-aware (no double sends); the
  maintenance pass no longer holds the writer lane across `VACUUM INTO`;
  the backup settings sidecar survives data directories containing
  ".db" in the path.

### Security
- The parser sandbox is tightened: `read(2)` is pinned to stdio file
  descriptors, `mmap(2)` is anonymous-only, and `close(2)` is denied.
- OAuth failure pages answer HTTP 400 (previously 200); OAuth state
  consumption is a single atomic UPDATE (TOCTOU removed).
- Credential-bearing configuration structs redact their secrets in `Debug`
  output, and corrupt secrets-envelope reads warn instead of silently
  reading as "not set".

### Fixed — UI
- The "check for updates" result now actually displays.
- Search hit cards and palette deep links open the record they point to.
- The Search button no longer disappears while its field has focus.
- Legitimately-empty lists no longer re-query SQLite every rendered frame.
- The sidebar responds to mouse-wheel scrolling; the stray ScrollArea
  overlay on Diagnostics is gone; timestamps render in a human form;
  stale status messages clear when the condition clears; list caps are
  honest; the Help Scout credential form validates before claiming
  success; live-chat threads render their real messages instead of
  "Other".

### Packaging / docs / CI
- The `.deb` ships a machine-readable copyright file (Debian policy).
- CI hardening: concurrency groups, per-job timeouts, no persisted GitHub
  tokens on checkout; the release pipeline stays serialized.
- README corrected to 23 pages with every environment knob documented;
  the documented `cargo xtask` invocation now actually works; xtask
  database-operation defaults match the app's real data directory;
  orphaned one-shot scripts archived; an unused dependency dropped.

## [1.7.3] — 2026-10-26

### Added
- **Live terminal progress for the long sync stages** — the conversation
  walk and other slow passes report per-resource progress as they run
  instead of looking dead, and the "Cancel running sync" button is
  consulted inside the long page loops (not only between resources).

### Fixed
- HAL-style pagination caps for incremental passes so a looping `next`
  link can no longer spin forever.

## [1.7.2] — 2026-10-26

### Fixed
- **Page-1 pagination for the v2 list endpoints** — a malformed page
  cursor fell back to page 1 instead of erroring the whole pass, and a
  missing `organization-properties` payload is tolerated rather than
  fatal.
- Test mocks moved to the real 1-based Help Scout page convention.

## [1.7.1] — 2026-10-25

The first audit-fix release: every finding from the pre-release audit
(FIND-001..011) that needed a code change.

### Fixed
- **FIND-001** — the loopback listener honors `SPP_LOOPBACK_PORT`,
  falls back to an OS port with an explicit warning when the bind
  fails, and the actually-bound port is surfaced in Settings (with the
  "register this EXACTLY" redirect-URI guidance).
- **FIND-004** — the sidebar layout reserves the theme strip so the
  bottom navigation rows are reachable again.
- **FIND-005** — every destructive action (backup overwrite/restore,
  data reset, log clear, outreach cancel/DNC, AI job deletes) arms the
  shell confirm dialog before it runs.
- **FIND-006/007/008/009/010** — honest idle-sync message, knowledge
  zero-hit search surfaces a gap suggestion, issue-radar enqueue
  feedback, connector refresh-seconds validation, and the customers
  back-to-search flow re-runs the applied query.
- **FIND-011** — command-palette dismissal is layer-independent.
- **FIND-003 (hygiene)** — `secrets.key` is forward-guarded against
  being committed, and the audit record is in the docs.

## [1.7.0] — 2026-10-21

The "AI becomes real, learns, and the long tail ships honestly" release:
everything since 1.4.0 — Phases A ("Lights On"), K ("Learn & Advise") and
P ("Power & Polish") of the v2 blueprint, complete.

### Added — live chat (P2)

- **A Live chats page answers Beacon visitors while the chat is still
  open.** Chats arrive through the five `beacon.chat.*` webhook events
  (Help Scout has no chat list endpoint — the webhook IS the data path)
  and land in a new local mirror (m059) that keeps ended chats, because
  the platform stops serving them (`GET /v2/chats/{id}` answers 404 once
  a chat collapses into a conversation). Sending uses the documented
  `PUT /v2/chats/{id}/messages/{uuid}` contract: the message id is
  caller-generated, so a retry after a timeout can never double-send
  (the platform's 422 "id value is already in use" answer is treated as
  delivered). The send box enforces the platform's **assigned-agent
  rule** in plain language — unassigned chats note that sending accepts
  them, chats held by another agent are blocked with the 400 reason,
  ended chats point at the conversation they became.

### Added — the library surfaces (P3)

- **Saved replies become manageable from Operations** — create, edit
  and delete go straight to the platform (`POST`/`PUT`/`DELETE`
  `/v2/mailboxes/{id}/saved-replies`, the platform's required `name`
  enforced), and the table re-reads the platform's post-write list,
  never an optimistic local edit. Workflows, synced all along but
  invisible, surface with their type/status and a run action
  (`POST /v2/workflows/{id}/run`) that resolves the conversation number
  from the local mirror.

### Added — the webhook relay guard (P4, closes D21)

- **A silently deactivated Help Scout webhook now heals itself.** The
  new health check lists the account's hooks, verifies ours is
  registered, enabled and subscribed to the events the app processes,
  and re-registers it (with the stored secret, capped at the platform's
  40 characters) when the platform dropped, disabled or pruned it —
  then removes the stale twin. It runs every ~6 hours and on demand
  (Settings → "check now"), and its outcome surfaces in Settings and
  Sync Health. The user-run tunnel guide lives next to it: the listener
  stays loopback-only by design, the card documents cloudflared/ngrok/
  ssh relays, the probe that proves the chain (`curl -X POST …/webhook
  -d '{}'` must answer 401), and the honest limit that the API never
  reveals a hook's stored secret, so a rotated local secret needs a
  re-register.

### Added — the AI layer (Phase A, since 1.4.0)

- **A provider hub** (AI Center): local LM Studio / Ollama plus
  user-added OpenAI-compatible endpoints (BYOK, keys in the encrypted
  envelope), five model roles (Embed/Classify/Summarize/Draft/Agent)
  routed per role with a legacy fallback, per-endpoint health probes,
  a feature registry under one kill switch, and a cost ledger that
  prices token usage per feature per day.
- **The wire batch**: 14-key attribute extraction with human overrides,
  inbox triage chips, real issue clustering (daily cadence + manual),
  customer memory ("know before you reply") with a quarantine guard,
  post-resolution QA, coaching insights, and the golden-set eval that
  re-runs whenever the prompts or routing change.

### Added — the knowledge loop (Phase K, since 1.4.0)

- **AI article drafts with a human-only review queue**: gap candidates
  become cited draft articles (`[^N]` citations, honest refusals
  before any run row), review approves/rejects, and approved drafts
  publish through the real Docs API with a verify re-sync.
- **Weekly AI digests**: deterministic anomaly detection (2-sigma), fact
  tables from SQL, narrative from the Summarize role when routed —
  archived either way, searchable on the Reports Digest tab.
- **The Reports API behind honest capability detection**: 32 registry
  endpoints, one generic fetch job, a 403 → "your plan does not include
  the Reports API" banner instead of empty charts, and a cache keyed by
  report/range.
- **Happiness from webhook truth (P1, closes D11)**: the satisfaction
  webhook ingests over TCP end to end (signed, verified, persisted,
  fetch-by-id), and the Happiness tab renders CSAT from those rows —
  absent honestly means "none received", never "zero happened".

### Fixed

- `create_webhook` now reads the documented `Resource-Id` header on the
  201 answer (previously returned 0 for every created hook).
- Terminal job failures in the chat/report/workflow jobs carry the real
  explanation to the jobs table (the `job_id` threading defect class
  found and fixed while building the chat jobs).

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
