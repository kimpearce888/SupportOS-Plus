# SupportOS-Plus v1.9.6 — Full Product Audit Report

**Date:** 2026-11-09 · **HEAD audited:** `11273bc` (main tip) · **Version:** 1.9.6
**Trigger:** owner directive — *"audit the repo, check for gaps, bugs, blockers, errors, issues… check features, workflows, options, everything the program offers, and question them on practical scenarios… what is built and why they are built, what purposes they serve, what is just built but not complete. create the report first."*
**Method:** full-code read of all 4 crates (204 Rust files, ~128k LOC: 22 app-shell pages + shell, ~90 core modules, xtask, catalog), backend↔frontend call-tracing, marker sweeps, verification battery re-run on the audited HEAD, and spot verification of every headline finding against the source (all file:line claims below were re-read in-source, not taken on faith).
**Read-only audit — no code was changed.**

---

## 0. Executive summary

The program is **real, coherent, and in substantially better shape than its size predicts**: the local-first Help Scout mirror works end-to-end (connect → sync → triage → reply → QA → report), the AI stack is genuinely wired (LM Studio OpenAI-compatible client, evidence-grounded drafting with citations and a verification pass, 22-tool read-only copilot, golden-set evals, per-feature kill switches, cost ledger), the design system is tokenized and drift-locked (style lint + 24 kittest suites + CI), and the release pipeline (3-distro .deb verification + AppImage + SHA-256 + post-publish asset checks) is unusually rigorous.

The marker discipline is exceptionally clean — **zero `TODO`/`FIXME`/`unimplemented!()`/`todo!()` in the codebase**; every gap is structural, not annotated. That cuts both ways: the gaps below are invisible to routine maintenance.

**The audit's core verdict: nothing is broken in the "crashes or loses data" sense (the battery is green, CI is 3/3 green on this HEAD), but there is a consistent pattern of features built to 80–90% and then left without their last mile** — a dead automation trigger set here, an unclickable list there, a phantom table silently zeroing a metric, a lifecycle that can never be closed. In one sentence: **the read path is excellent, the write/edit/manage path repeatedly dead-ends.**

Counts (all verified in-source):

| Class | Count |
|---|---|
| A. Confirmed code bugs (wrong/empty data, fake success) | 8 |
| B. Silent no-ops & dead engine paths | 7 |
| C. UI dead ends (built-but-not-completable from the UI) | 14 |
| D. Operational risks & expectation traps | 8 |
| E. Documentation / repo drift | 7 |

**Verification battery on the audited HEAD (all green):** gates (no-simulation, traceability 23/23 routes) · `cargo fmt --check` · `xtask verify-icons` · clippy workspace `--all-targets -D warnings` · clippy sandbox lane `-D warnings` · tests core 464 ✓ / core+sandbox 472 ✓ / app-shell lib 251 ✓ / style lint 2 ✓ / xtask+catalog 14 ✓ · cargo-deny (advisories/licenses/bans) ok · CI run **42542458251 green 3/3** on `11273bc` (incl. kittest drift gate + packaged smoke). *(An initial sandbox-lane run showed 35 failures — all `StorageFull` from the recycled sandbox's full disk, not code; after freeing space the lane passed 472/472.)*

---

## 1. What is built and why — the product map

**Purpose (why it exists):** a **local-first Linux desktop helpdesk client for Help Scout**. All account data is mirrored into a local SQLite DB; every read (inbox, dashboards, search, AI evidence) is served locally; writes go through a durable outbound job queue back to Help Scout. Privacy/resilience is the design thesis: secrets AES-256-GCM-encrypted under a file master key, no telemetry, sandboxed parsing of untrusted knowledge files (Landlock+seccomp), SSRF-jailed connectors, self-contained .deb/AppImage releases.

**The four crates:**

| Crate | Role | Verdict |
|---|---|---|
| `core` (~90 modules) | Everything: SQLite mirror of 19 Help Scout resource types, sync engine (initial/incremental/reconcile, resumable cursors, rate limiter, 401 refresh), job/outbound queues with crash recovery, AI stack (LM Studio client, evidence pipeline, copilot agent, evals, hub), knowledge store + FTS + sqlite-vec semantic search, reports/SLA/automation/outreach/incidents, backups/reset, loopback webhook+OAuth server | Load-bearing, real, tested (464+472 tests) |
| `app-shell` (egui/eframe, 22 pages) | The whole desktop UI: tokenized design system (~50 tokens, WCAG-tested), 24 kittest golden suites, style lint, smoke harness | Load-bearing, real, drift-locked |
| `catalog` | Serde-only closed vocabularies (14 AI attribute keys, 22 copilot tools, 15 notification types, 21 report metrics…) shared by core+shell | Real, deliberately tiny, consumed by ~20 modules |
| `xtask` | Build/packaging: verify-icons, AppImage (pinned appimagetool 1.9.1 + pinned type-2 runtime, retry-hardened), .deb, healthcheck, backup/restore, vector-count | Real; invoked by CI |

**Feature inventory — what each surface offers and what purpose it serves:**

- **Inbox (core workspace):** adaptive 3-pane layout; views/filters (status, mailbox, channel, tag, search, `#number`); reply/note composer (To/Cc/Bcc, saved replies, @mentions with notifications, Ctrl+Enter); status change + assignment; LM Studio draft card (Insert→composer→Send, human-approved); deterministic Review Draft (9 checks + AI layer); AI Shorten/Expand/Warmer/Translate; copilot panel (signals, summary, similar past issues, relevant help articles incl. synced Help Center since AIE-1, suggested reply, "Ask the assistant" tool-chat); editable triage chips (intent/urgency/frustration/escalation/cluster); QA card; per-conversation refresh; duplicate-send guard; eval-mode and not-connected send guards.
- **Dashboard / Reports:** 12-KPI responsive dashboard, trend chart, issue radar; 11 report tabs incl. SLA (per-mailbox business-hours editor), why-contacting, top questions, QA overview, weekly digests, CSAT, **Report Builder** (metric×dimension×filters, saved reports, comparison), Insights (health facts, friction, support-graph nodes), and capability-gated Help Scout remote reports (32 specs, plan-gated with honest banners).
- **Knowledge (2 lanes):** local documents (import from `knowledge-import/`: md/txt/csv/json/html/pdf/docx, paste, visibility, review/verify/delete, FTS+semantic chunks) and **synced Help Center** (stats, gap detection, sanitized rich-text article reader — the UXH-1 fix, publish pipeline: AI-draft-from-gap → review queue → approve/reject → publish to Help Scout → verify).
- **AI Center:** Health (endpoints hub with keys/probe/prices, 5-role model routing, 16 feature kill switches, cost ledger), Analytics, Attributes distributions, Copilot sessions, Usage ledger + golden-set evals, Jobs table. Local no-model fallback: heuristics (urgency/frustration/intent, extractive summary, template suggested reply) — the app degrades honestly, never lies about AI absence.
- **Automation:** rules (New conversation / Customer reply → Assign/Add tag/Send note/Change status), approval queue, runs logged. **Plus the findings in §3B — this is the least complete subsystem.**
- **Connectors:** local_json/csv/sqlite/http data sources (SSRF-jailed, 10 MB caps, schema inference, 5k rows), manual/interval refresh, AI-visibility flag.
- **Outreach:** segments (JSON DSL), campaigns (queue/pause/resume/retry/cancel, `{{var}}` templates, reply-rate report), DNC list.
- **Customers/Organizations/Custom Objects:** customer 360 (health, event-mix, milestones, AI interaction profile with provenance, conversations, timeline), orgs, typed custom-object system.
- **Incidents / Issue Radar / Operations / Workload:** incident workspace (link conversations, notes, affected customers), issue radar (alerts, clusters, knowledge gaps, known issues), operations tiles with drill-down, agent workload with capacity model + suggested assignees, saved-replies library, manual workflow runner.
- **Live Chats:** beacon chat sessions (webhook-ingested), guarded agent replies.
- **Search:** FTS + semantic (RRF fusion) across tickets/knowledge/customers, clickable hits; command palette (Ctrl+K).
- **Notifications:** grouped, mention-aware, mark-read, preferences.
- **Sync Health / Diagnostics / Settings:** forensic sync state + checkpoints + runs, event log + audit trail, full settings (sync/API/behavior, OAuth browser flow, connection doctor, webhook relay health, LM Studio, vector index, business hours, backups/restore/exports, data reset with double-confirm, version check).

---

## 2. Practical scenario walkthroughs (the "does it actually resolve the task?" tests)

Each scenario was traced code-end-to-end (data flow + UI affordance + failure path), not assumed.

### S1. First run → connect Help Scout → initial sync → answer a ticket ✅ WORKS
Empty DB boots clean (provider refuses syncs with legible errors until credentials exist; the empty→real transition resets poisoned watermarks). OAuth browser flow via loopback callback; client-credentials also supported. Initial sync: 19 resources, resumable per-page cursors (a crash resumes; 5,000-page safety cap), rate-limiter honors `x-ratelimit-*`, 401s refresh proactively (120 s margin) and reactively. Reply: composer → durable `outbound_jobs` (idempotency key, duplicate-send guard) → Help Scout → single-conversation re-sync → delivery toast. **The golden path is genuinely solid.**

*Caveats encountered:* a 100k-conversation account needs one threads-request per conversation — rate-limited to ~11+ hours, app must stay running (honest in-code disclosure; mitigated by cursors/heartbeats/cancel). Webhooks are **loopback-only** — real-time needs an external https relay (tunnel); without one the app is polling-only (adaptive ≥1 min). Clock skew >10 min silently misses incremental changes until a manual Reconcile (reconcile is **not** scheduled).

### S2. Configure AI → draft a reply → insert → send ✅ WORKS (with traps)
Settings→LM Studio or the hub endpoints (probe, keys, prices), route the 5 model roles, per-feature kill switches. "Draft reply with LM Studio" (AIE-1 trigger) → job → evidence build (thread with authorship, customer history, similar conversations FTS+semantic, known issues, knowledge docs, **help center articles**, saved replies, quarantine-filtered memory, interaction strategy) → draft card with citations → Insert → Send. A second verification pass flags unsupported claims/leakage.

*Traps:* **fresh install with no LM Studio running = notification spam** — AI defaults on, 14/16 features default on, no roles routed → every synced conversation spawns analyze jobs that fail twice each with "check that LM Studio… is running" notifications; no circuit breaker for "no backend configured" (D-A1). A rejected/discarded draft **re-appears** in the card (D-A7). Endpoint prices can't be edited after creation (D-C7). Config lives in three places with implicit precedence (D-A9).

### S3. Find and reuse knowledge ✅ WORKS (with a dead metric)
Search page fuses BM25+semantic with honest mode notes; palette deep-links; the Help Center reader now renders sanitized rich text (UXH-1 verified green in CI). **But** the Knowledge Freshness tab's usage metric is dead — see B2: `usage_count` is always 0 because it queries a table that is never created, so the "low usage" flag fires on age alone and "recurring questions" can never fire. Local documents have **no reader** (a palette hit on a document lands on the list tab — acknowledged dead end in code) and **no re-ingest** (delete+re-import is the only refresh).

### S4. Automate triage ⚠️ HALF WORKS — the weakest subsystem
Create rule → trigger on New conversation/Customer reply → action Assign/AddTag/SendNote/ChangeStatus → fires on the next customer thread → write actions park for approval → Approve executes locally **and** pushes to Help Scout with an idempotency key. This narrow happy path works.

*But see §3B: three of the five trigger types never fire at all, SetPriority fake-completes, "read" actions record completions without doing anything, rules can't be edited, the assignee field demands a raw numeric user id, and the status field is unvalidated free text (a typo encodes a rule that sets a bogus status). A user who builds a StatusChanged rule gets no error, no run, and no explanation — ever.*

### S5. Outreach campaign ⚠️ WORKS but is power-user-only
Segment JSON DSL → campaign create (recipient snapshot, DNC excluded) → queue/pause/resume/retry/cancel → outcome classification + reply detection → report. Solid engine. *But:* segments are a raw JSON textarea (no builder, no recipient-count preview before save — the preview exists in the engine but is only used as a create guard), targeting needs raw mailbox/segment ids typed from a hint line, no campaign edit/delete, DNC add needs a raw customer id.

### S6. Manage an incident ⚠️ STARTS, NEVER FINISHES
Create incident (title/status/severity/description/linked conversations) → notes, affected customers, releases, related — all read fine, and the AI/intelligence sweep can auto-populate customer-safe explanations/workarounds. *But the manual path dead-ends: status/severity/owner/resolution/workaround can never be changed or set from the UI (9 create-form fields are permanently `None`), no delete, no unlink, no resolve transition. An incident opened manually can only ever be "open".*

### S7. SLA monitoring ✅ WORKS
Per-mailbox business hours (IANA tz, days, window) + first-response/resolution targets → business-minutes math → report + at-risk/breached alerts with per-conversation drill and daily-deduped notifications. Honest about unconfigured mailboxes (listed, not guessed). *No holidays model; at-risk threshold fixed at 0.8.*

### S8. Backups / restore / migration / reset ✅ WORKS (one operational trap)
VACUUM-INTO backup + redacted settings sidecar (secrets excluded), keep-20 prune, **verified restore with staged swap + rollback**, JSON/CSV exports, transactional secure-delete workspace reset, 0.2.x→1.9.x data-dir migration (verified in 3 distros in CI). *Trap: `secrets.key` (the file master key) is not part of the DB backup and not in the sidecar — a restored backup without the key file has unrecoverable credentials; nothing enforces or prominently documents copying it (D-D4).*

### S9. Big-account / multi-agent operation ⚠️ MIXED
Single-writer SQLite + WAL is structurally safe (double-claim guards, panic containment at both boundaries, crash recovery of interrupted sync/stuck outbound/orphaned runs). *Risks:* `me_user_local_id` falls back to **the first user by id** when `me_remote_id` is unresolved — on multi-user mirrors, mentions/notifications can route to the wrong agent (B14); webhook bursts contend on the single writer during long sync transactions; retention (when enabled) deletes audit history (default off, disclosed).

---

## 3. Findings (all verified in-source; severity-tagged)

### A. Confirmed code bugs — produce wrong/empty data or fake success

| # | Finding | Evidence | Impact |
|---|---|---|---|
| **A1** | **Weekly digest's `top_issue_clusters` is always empty.** `topic_facts` selects `label` from `issue_clusters`, but the table's column is `name` (schema at `intelligence_features.rs:27-34`); the prepare fails and `.unwrap_or_default()` swallows it. | `digest.rs:253` vs `intelligence_features.rs:29` | Every digest ships without its top-clusters topic block — silently wrong output. |
| **A2** | **Knowledge Freshness usage metrics are dead.** `freshness_report` builds a usage subquery against an `ai_evidence` table that is **never created anywhere** (guarded by an `sqlite_master` existence check that is always false; the real citations table is `ai_sources`). | `knowledge_store.rs:341-353`; repo-wide grep: no `CREATE TABLE ai_evidence` | `usage_count` always 0 → "low usage" fires on age alone; "recurring questions" (`usage_count > 5`) can never fire. The Freshness tab under-reports real usage. |
| **A3** | **The entire priority feature chain is broken on real accounts.** (a) The mirror hardcodes `priority: None` when mapping conversations (`helpscout_real/mod.rs:732`) — the mirrored `conversations.priority` is never populated; (b) the inbox priority filter matches that column (`inbox.rs:135-138`), so it can never match; (c) local priorities live in `supportos_priority` but the UI never edits them (reader displays read-only "Priority: none", `inbox.rs:1580`; UI filters default `priority: None`, `inbox.rs:88`); (d) automation `SetPriority` no-ops (B3). | all cited lines | A Help Scout-native concept is displayed, filterable in the backend, actionable in automation — and none of the three connect. Built, not complete. |
| **A4** | **Rejected AI drafts are re-offered.** `latest_ai_draft_row` takes the newest draft with **no state filter**; after "Discard" (state `rejected`) the same draft re-renders in the draft card and can be re-inserted. | `ai_local.rs:500-511` vs `inbox.rs:2069-2089` | The discard action appears not to work; feedback data gets polluted by re-inserts. |
| **A5** | **`sync_conversation`/`delete_conversation`/`sync_customer` jobs with malformed payloads "complete" silently** — `if let Some(remote) = …` with no else: a producer bug yields a green job that did nothing. | `workers/execute.rs:19-48` | Silent data-loss class; failures are invisible in the Jobs/Diagnostics surfaces. |
| **A6** | **Automation `SetPriority` fake-completes after approval.** The fire path records "skipped: no engine handler" (`automation.rs:1028-1036` — `risk_tier` has no mapping, `:241-251`), but the **approved** path's `execute_non_destructive` catch-all `_ => Ok(true)` records a **successful** run without doing anything (`:601`, `:622-705`). | cited lines | The audit trail lies: an approved priority action shows "completed" and never executes. |
| **A7** | **The worker failure handler ignores `LmStudioError.retryable:false`** — `let retryable = true;` in the catch-all, so deterministic failures (e.g. unparseable model output) burn all attempts and spam notifications; conversely copilot failures are hard-marked non-retryable. | `workers/execute.rs:415-424, 1422-1431` | Wrong retry semantics on both sides. |
| **A8** | **`build_http_client` falls back to `reqwest::Client::default()`** (`.unwrap_or_default()`) — a builder failure silently yields a client with **no timeout**, undermining the configured 120 s. | `ai_lm_studio.rs:341-346` | Rare but real: a hung LM Studio hangs a job indefinitely (job-layer stale sweep only after 30 min). |

### B. Silent no-ops & dead engine paths (built, wired, never reachable)

| # | Finding | Evidence |
|---|---|---|
| **B1** | **Automation triggers `StatusChanged`, `TagAdded`, `SlaRisk` are never fired.** The only production call path is `on_conversation_changed` → `customer_thread_trigger_decision`, which can only return `"new_conversation"` or `"customer_reply"` (`automation.rs:285-295`, called at `workers/execute.rs:591-599`). The UI does not offer these triggers (deliberately, AU2-1/D10) but the engine vocabulary, display descriptions, and docs still carry them. Rules using them silently never run — and record nothing. | cited lines |
| **B2** | **Automation "read" actions `search_similar` / `check_known_issues` record `"completed" / "Executed read action…"` without executing anything** (only `analyze_ticket` is really enqueued). | `automation.rs:1040-1075` |
| **B3** | `TicketOperation::AddNote` **discards its `_body`** — inserts a bodiless note activity event; the real note path is the outbound lane. Dead variant kept in the enum. | `ticket_ops.rs:254-283` |
| **B4** | `TicketOperation::SetTicketState` has **no UI caller at all** (the six local workflow states and transition audit exist in core; nothing surfaces them). | `ticket_ops.rs:28-33`; UI grep |
| **B5** | **`update_tags` outbound rejects an empty tag list** — clearing all tags remotely is impossible through that lane. | `workers/mod.rs:697-703` |
| **B6** | **`AiStatus.provider_available` is always `None`** (vestigial column; the health record never probes). | `ai_center.rs:114` |
| **B7** | Webhook **unknown event types are accepted, deduped, persisted — then ignored** (`_ => {}`). Harmless but silent; a new Help Scout event type vanishes without a trace beyond raw storage. | `webhook_handler.rs:174` |

### C. UI dead ends — built but not completable end-to-end from the UI

| # | Surface | The dead end | Evidence |
|---|---|---|---|
| **C1** | **Notifications** | Cards carry full deep-link fields (`conversation_id/number`, `customer_local_id`, `issue_id`, `campaign_id`, `job_id`) but the page returns **no navigation** — "mentioned you on #501" cannot be clicked. No unread badge in sidebar/header chrome either. | `core/src/notifications.rs:109-129` vs `app-shell/pages/notifications.rs:208` |
| **C2** | **Connectors** | Credentials can **never** be configured — `auth` is hard-coded `json!({})` at creation; no edit, no probe/test (unlike AI hub endpoints), and manual refresh gives **no completion feedback** (core `refresh` emits no bus event; the page doesn't poll — health/last-sync/rows stay stale until an unrelated sync). | `app-shell/pages/connectors.rs:166, 207-224` vs `core/src/connectors.rs:328-358` |
| **C3** | **Custom objects** | Links can never be created (`links: Vec::new()` hard-coded) while the page counts and advertises them ("N links"); no edit for types/objects; no pager — self-admitted: *"showing first N of M — the list has no pager"*. | `custom_objects.rs:321, 475` |
| **C4** | **Incidents** | Lifecycle dead after creation: no status/severity/owner/resolution/workaround edits (9 create-form fields permanently `None` from the UI), no delete, no unlink; linked conversations render as inert text. | `incidents.rs:209-228, 593-633` |
| **C5** | **Inbox** | No tag add/remove, no mailbox move, no snooze/merge/forward/delete (provider supports snooze/schedule — core wired, UI absent); customers' conversation rows in the profile panel are not clickable; outbox/failed-send retry view absent (failures surface only via status line + Diagnostics). | `inbox.rs:1576-1581`; `customers.rs:847-881` |
| **C6** | **Automation** | No rule **editing** (delete+recreate only); assignee requires typing a numeric user id (an assignable-users list exists and is used 30 lines away in the Inbox); status is unvalidated free text; no approval history (pending-only queue). | `automation.rs:105-122, 526-560` |
| **C7** | **AI Center** | Endpoint $/mtok prices not editable after creation (hint text sits next to nothing); endpoint enable/disable not surfaced; copilot sessions can't be renamed/deleted; golden-set is 9 seeded cases with no user management (create/edit/deactivate). | `ai_center.rs:1255-1258, 1624-1625` |
| **C8** | **Issue Radar** | Alerts tab is read-only with no drill-down (radar alerts, SLA callouts, clusters, gap evidence examples — all inert text, in contrast to Operations tiles which drill); known issues are create-only (no edit/status/delete); queued cluster/gap jobs need a manual reload. | `issue_radar.rs:584, 650, 689-695` |
| **C9** | **Outreach** | Segments are raw-JSON textareas with no recipient-count preview before save; campaign targeting needs raw mailbox/segment ids from a hint line; no campaign edit; DNC add needs a raw customer id. | `outreach.rs:617-681` |
| **C10** | **Reports Builder** | Filters are raw CSV id fields ("mailbox ids (csv):" etc.) — no pickers, unlike the inbox; happiness table's conversation numbers are inert; support-graph nodes have no delete. | `reports.rs:901-912, 1753-1757` |
| **C11** | **Knowledge documents** | No document reader (palette hits land on the list — acknowledged in code); no per-document re-ingest; local docs can't be edited. | `knowledge.rs:642-649` |
| **C12** | **Organizations** | Create-only: no edit/delete; members not clickable to customer profiles; capped at 200. | `organizations.rs` |
| **C13** | **Operations** | "Suggested assignees" is display-only text (`#42 → Grace (lowest load)`) — no Assign button; workflows can be run but not created/edited. | `operations.rs:525-554` |
| **C14** | **Live chats** | The chat→conversation link (`conversation_remote_id/number`) exists in the model but is never displayed/clickable; no agent "end chat"; no navigation to the customer. | `chats.rs:57-58` vs `live_chats.rs` |

### D. Operational risks & expectation traps

| # | Risk | Evidence |
|---|---|---|
| **D1** | **Fresh-install AI noise:** AI on by default + no roles routed → every synced conversation spawns analyze/attribute jobs that fail twice with "check that LM Studio… is running" notifications. No circuit breaker keyed on "no backend configured". | `ai_hub.rs:219-241`; `notification_sweep.rs:382-449` |
| **D2** | **Embedding dead-ends:** 60-chunk pass cap with no self-requeue (a large first sync starves semantic indexing); 5 failed attempts **permanently** exclude a chunk; no user-facing re-index/reset action anywhere (Settings/palette only rebuild FTS). One transient LM Studio outage at the wrong moment degrades semantic search indefinitely and invisibly. | `embeddings.rs:32, 501-519`; `workers/execute.rs:139-165` |
| **D3** | **Conversation re-embedding waste:** `chunk_conversation` DELETEs+re-INSERTs all chunks on every sync touch (no content-hash dedup, unlike knowledge docs) — every status-change sync re-embeds the whole conversation through LM Studio. | `embeddings.rs:189-217` |
| **D4** | **Backup/restore does not cover `secrets.key`:** credentials in the restored DB are AES-GCM-ciphertext without the file key — unrecoverable; nothing enforces or prominently documents backing up the key file. | `crypto.rs`/`settings.rs:10-33`; backup sidecar redacts secrets by design |
| **D5** | **Multi-user identity fallback:** `me_user_local_id` falls back to the **first user by id** when `me_remote_id` is unresolved — mentions/notifications route to the wrong agent on multi-user mirrors. | `notifications.rs:176-191` |
| **D6** | **Sync throughput & clock skew:** one threads-request per conversation (≈11+ h for 100k conversations, app must stay running); incremental overlap is only 10 minutes — local clock >10 min fast silently misses changes until a **manual** Reconcile (not scheduled); token expiry trusts the local clock. | `loops.rs:901-907`; `mod.rs:54`; `http.rs:141-199` |
| **D7** | **Webhook reality gap:** loopback-only listener; real-time requires an external https relay (tunnel). The health system self-heals registration and instructs the user, but cannot create the relay — without one the product is polling-only. | `webhook_health.rs:148-201` |
| **D8** | **`retention_days` deletes audit history when enabled** (default off, disclosed in Settings) — a compliance-relevant default to be aware of. | `maintenance.rs` |

### E. Documentation / repo drift (built-then-drifted)

1. **USER_MANUAL.md and all of `manual/` are stamped "Version 1.8.0"** — five releases stale; 1.9.0 was a full design reskin, so the manual's UI descriptions predate the current UI. `manual/` (7.7 MB, incl. the 4.2 MB Developer Handover PDF — the repo's largest blob) is unreferenced.
2. **README:242-243** says appimagetool is "downloaded from the official continuous release" — stale: it's pinned to 1.9.1 with sha256, and a test *explicitly forbids* `continuous`.
3. **README:110** still documents `QDRANT_URL`/`QDRANT_ENABLED` as "Vector store endpoint" — the qdrant engine was removed (D4, sqlite-vec); the env vars are legacy-named and the framing is misleading.
4. **README:105** says `SPP_LOOPBACK_PORT` default is "OS-assigned" — actual default is the redirect-URI port (3000), OS-assigned only as last fallback (USER_MANUAL:909 has it right).
5. **ci.yml:172-175** comment: "24 suites (175 tests… over 171 goldens)" — reality: 25 suites / 204 tests / 193 goldens.
6. **Root Cargo.toml comments**: "fs/process/signal have zero call sites" (fs is used); dev `debug = 0` justified by "the embedded qdrant engine" (gone).
7. **Vestigial inventory:** `icon.ico`/`icon.icns` are CI-verified but never shipped (Linux-only packaging); aarch64 packaging support (arch map, pinned runtime, "both CI arches" test claims) has **no CI lane** — the test asserts arches CI doesn't build; `deny.toml` targets x86_64 only; the release **tag path skips cargo-deny and the kittest drift gate** (ci.yml doesn't trigger on tags — in practice tags come from main, but nothing enforces it); dead design component `design::quirk` (snapshot-only); `search.rs` vestigial `status_filter`; palette hint promises "issues" scope that doesn't exist; catalog's "WASM-safe" description predates the WASM deletion.

---

## 4. Test & CI audit

- **Battery on HEAD: fully green** (§0), and CI run 42542458251 confirms the same 3/3 on `11273bc` — including the kittest drift gate (all 25 suites diff-locked) and packaged smoke tests. The suite counts above (25/204/193) are current.
- **Strengths:** transactional migrations with FK gates; crash-recovery tests (interrupted sync, stuck outbound, orphaned runs); integration tests against a mock Help Scout server; golden-locked UI at 3 widths × 2 themes; style lint banning raw colors; supply-chain pinning everywhere (actions by SHA, appimagetool/runtime by sha256, exact toolchain); release pipeline verifies byte-identical artifacts, 3-distro install/upgrade/uninstall, GUI launch, and live post-publish asset checksums.
- **Coverage gaps:** no interaction tests for the missing deep links (they don't exist to test); connector refresh flow untested end-to-end; palette/toasts have no kittest; aarch64 packaging never built in CI; tag-path gap (deny+kittest not run on release tags); no nightly/scheduled runs; `xtask lint` is a subset of CI (no `--locked`, no gates) — local green ≠ CI green.

---

## 5. Prioritized remediation backlog (recommendations only — nothing fixed in this audit; every item is inside the existing stack, no new dependencies)

**P0 — wrong data / lying audit trail / user-visible malfunction (highest value):**
1. A1 digest `label`→`name` (one-line SQL fix + regression test).
2. A2 freshness usage → wire to the real `ai_sources` table (or remove the dead metric).
3. A4 rejected-draft state filter (one WHERE clause + test).
4. A5 malformed-payload jobs must fail loudly (else branch → job error).
5. A6+B1+B2+B3 — the automation cluster: fire or remove the three dead triggers; make SetPriority real or delete it; make read-actions real or record them as unsupported; store the note body or drop `AddNote`.
6. A3 priority chain: populate the mirror column (or point the filter/reader at `supportos_priority`) and surface one edit control.

**P1 — dead ends that block real workflows:**
7. C1 notification click-through + unread badge (data already in the record).
8. C2 connector credentials/edit/probe + refresh completion event.
9. C4 incident lifecycle (status/severity/resolution edits).
10. C5 inbox: tags, snooze, priority edit, clickable customer conversations.
11. C6 automation rule editing + validated assignee/status inputs.
12. C7 editable endpoint prices.
13. D1 "no backend configured" circuit breaker (stop job spam on fresh installs).
14. D2 embedding re-index action + self-requeue past the 60-chunk cap.

**P2 — consistency, polish, docs:**
15. C3/C8–C14 remaining dead ends (custom-object links, radar drill-downs, outreach pickers/preview, report pickers, knowledge reader, org edit, operations one-click assign, chat links).
16. E: re-stamp USER_MANUAL + `manual/` (or delete `manual/`), fix README rows (appimagetool, QDRANT, loopback port), refresh ci.yml counts, Cargo comments.
17. Infra: run deny + kittest on the tag path; decide aarch64 (lane or remove claims); add `[sources]` to deny.toml; `secrets.key` backup guidance in the manual + restore flow warning (D4).
18. D5 me-user fallback guard; D3 conversation chunk content-hash; D6 scheduled reconcile; A7 retryable-flag honor; A8 timeout fallback fix.

---

## 6. Ledger note

This audit was opened and closed as a single report-only batch on the owner's directive (no code changes; battery re-run for evidence). The remediation backlog above is **not** queued — per Rule 8, fixes start only on an owner directive, e.g. "fix P0".
