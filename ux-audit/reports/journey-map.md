# UX Audit Journey Map — SupportOS+ v1.9.4 (d306caa)

Legend: tested = ✔ (driven in the live app under Xvfb 1920×1080 with synthetic
input + VLM verification), partial = ◐ (reached but not every sub-state),
blocked/not-run = ✖ with reason. All journeys on `main` build of d306caa,
data dir seeded by the live HelpScout test account (env client credentials,
mailbox "Webinars Today", 18 conversations).

## Shell / global

| # | Journey / element | States exercised | Tested |
|---|---|---|---|
| S1 | App boot → first-run (fresh data dir) | default window 1180×760 → maximized 1920×1080 | ✔ |
| S2 | Sidebar navigation — all 23 pages | click-through with verification loop | ✔ (all pages reached; retry loop proved some rows need scroll at 1080p) |
| S3 | Sidebar scroll | wheel scroll up/down, scrollbar visible | ✔ |
| S4 | Theme toggle (top bar moon icon) | light → dark → light; persisted across restart | ✔ |
| S5 | Command palette (Ctrl+K) | open, empty state, type "webhook" query, results, Esc close | ✔ (required window focus — no-WM sandbox artifact, not an app bug) |
| S6 | Top-bar search field ("Search ⌘K" button) | click → palette opens | ✔ (same as S5) |
| S7 | Sync status pill ("synced Ns ago" / "never synced") | live ticking, both states seen | ✔ |
| S8 | Window resize 1920×1080 → default → 2560×1440/1440×900 resilience | resize + spot screenshots | ◐ (1920×1080 full pass; 1180×760 default; other sizes deferred to kittest 1440/1024/800 coverage) |
| S9 | Bell icon (top bar) | click → Notifications page | ✔ (icon-only button — a11y note) |
| S10 | Toasts (disconnect/connect/sync) | disconnect toast "Disconnected. Local data is fully preserved." seen with dismiss | ✔ |

## Pages (content audit at 1920×1080, light + selective dark)

| Page | States exercised | Tested |
|---|---|---|
| Dashboard | synced (18 convos), filters (Range/Channel/Mailboxes chips seen), dark theme, first-run | ✔ |
| Inbox | list (18 convos), conversation open (#24 detail pane: thread, composer, Reply/Note tabs, Cc/Bcc), search box, view chips (All/Active/Pending), operations drill-down row | ✔ |
| Live chats | empty state (0 open / 0 ended), view toggle links | ✔ |
| Operations | default tiles view (sync problems 55, failed jobs 18, etc.) | ✔ |
| Customers | default list, empty CONVERSATIONS/LAST CONTACT columns | ✔ |
| Organizations | default list (6 rows), empty DOMAINS/CUSTOMERS columns | ✔ |
| Knowledge | default documents view, empty-state CTA | ✔ |
| Reports | default (2-point chart, no axes) | ✔ |
| Issue Radar | default (chart without labels) | ✔ |
| Incidents | empty state with filters "Any/Any" | ✔ |
| Custom Objects | empty state + "Pick a type on the left" pane | ✔ |
| Automation | default (debug note card + empty container) | ✔ |
| AI Center | default (provider cards, LM Studio state) | ✔ |
| Outreach | Campaigns empty state (+ New Campaign button), tabs | ✔ |
| Notifications | 18 job_failure notifications list, mark-read buttons | ✔ |
| Search | empty state + "+ Try 'export'" CTA + scope selector | ✔ |
| Support Graph | empty state | ✔ |
| Connectors | table (dev note "A-5 (D5-0)", no empty state) | ✔ |
| Diagnostics | log view (36 errors/1 warning counts) | ✔ |
| Sync Health | checkpoints + recent runs (clipped) | ✔ |
| Settings — Sync & AI tab | full form audit | ✔ |
| Settings — Help Scout tab | connected / disconnected / credentials-saved states, browser-flow disclosure (collapsed+expanded), authorize URL flow, Connection Doctor (5 green checks), credentials form (fill/save), webhook secret save, disconnect w/ toast | ✔ |
| Settings — LM Studio tab | form audit | ✔ |
| Settings — Vector index tab | status card | ✔ |
| Settings — Business hours tab | mailbox card ("Webinars Today" not configured) | ✔ |
| Settings — Backups & export tab | backup row, restore/export buttons | ✔ |
| Onboarding | "Welcome to SupportOS+" debug-dump page | ✔ |
| Shell | internal page (TODO text, ~70% empty) | ✔ |

## HelpScout end-to-end journeys (external, test account)

| # | Journey | Result | Tested |
|---|---|---|---|
| H1 | Help Scout web login (the test account) | success (browser session kept for OAuth) | ✔ |
| H2 | Locate OAuth app "SupportOS Plus E2E" | App ID matches; **registered redirect is `http://localhost:3001/oauth/callback`** (not 3000) | ✔ |
| H3 | Create webhook (Apps → Webhooks → Install) | webhook id 354070, 17 events checked (convo.*, customer.*, beacon.chat.*), secret generated (48-hex), callback `http://localhost:3000/webhook` initially | ✔ |
| H4 | Update webhook (secret → 40 chars, callback → `http://localhost:3001/webhook`) | saved, verified via re-read | ✔ |
| H5 | Save webhook secret into app Settings (UI) | chip "webhook secret: not set" → "app settings"; save message incl. 40-char platform-cap note | ✔ |
| H6 | Signed webhook delivery probe (HMAC-SHA1) | first with 48-char secret → 401 (app caps at 40); with 40-char secret → **200 `{"received":true}`** | ✔ |
| H7 | OAuth browser flow | disconnect → save creds (provider activates) → during-connect window expand "advanced: browser authorization flow" → "open the authorization page in the browser" → hyperlink appears → open authorize URL in browser → "Authorize" → redirect to `localhost:3001/oauth/callback` → success page "Connected as Sayeed Nabin" → app chip "connected", oauth_tokens=1, state consumed | ✔ |
| H8 | Webhook relay & health "check now" | NOT driven — requires a public relay URL; loopback-only by design (documented in app copy) | ✖ (by design, no relay available) |

## Not exercised (with reasons)

- Real inbound webhook delivery from HelpScout cloud → the app is
  loopback-only by design; HelpScout's servers cannot reach localhost. The
  signature-verification path was instead proven with a correctly signed
  probe (H6). A real relay is out of scope for a local-first audit sandbox.
- LM Studio / local AI features — no LM Studio server in the sandbox; the
  AI failure notifications (18 job_failure entries) were observed and audited
  as-is.
- Beacon live chats (zero chats in the test account; empty state audited).
- Payment/account/security settings on HelpScout — prohibited by audit rules.
- Destructive actions (delete flows in-app): driven only for clearly-marked
  audit test data where present; otherwise verified to the confirmation step
  and cancelled (see issues).
