# Existing Test Coverage — QA Audit 20261019T050847Z

Mapping of the repository's own test suites (at commit 19dfb35) to the
behaviors discovered by the audit. The suite is credential-free and
cloud-free by design: external services are replaced by an in-process mock
HTTP server and loopback TCP.

## Suite totals

- `cargo test --workspace`: **304 passed** — unit + integration, including
  the shell suite; plus **169** from the TODO-45 integration batch
  (`it_loopback`, `it_helpscout`, `it_lm_studio`, `it_sync`), 473 total,
  0 failures. 3 tests ignored by design (2 bench seeds, 1 LM Studio live
  round-trip gated behind `SPP_LM_STUDIO_URL`).
- CI runs both jobs on every push: AppImage build + smoke; fmt/clippy
  (`-D warnings`) / test / cargo-deny.

## Behavior-to-test mapping (behaviors the GUI audit surfaced)

| Discovered behavior | Covering tests |
|---|---|
| Webhook ingress: HMAC-SHA1 verification, persistence, dedup, job enqueue, status mapping, concurrent burst | `it_loopback` over real TCP against the live `LoopbackServer` |
| OAuth authorization-code flow: single-use state, browser-style `GET /oauth/callback?code=…`, real `RealHelpScoutProvider` token exchange vs mock token endpoint, connected-user fetch, tokens + audit rows persisted (encrypted envelope) | `it_loopback` |
| Help Scout provider HTTP stack: reqwest + `ApiQueue` + rate limiter + token persistence/refresh (incl. the `id=1` single-row OAuth store) | `it_helpscout` against `mockserver` |
| Sync engine: initial/incremental/single-conversation sync, fixture dataset (2 mailboxes, 3 customers, 3 conversations + threads, paged tags/teams/workflows), row-by-row mirror assertions | `it_sync` |
| LM Studio / OpenAI-compatible client: wire format, model listing, availability probe, timeouts, HTTP error surfaces, recovery; live inference `#[ignore]`d without a real server | `it_lm_studio` |
| Boot pipeline: 49 migrations, FTS5 reference tables, guards | `bootstrap`/`migrations` unit tests + observed on every GUI boot |
| Background workers (job/webhook/embedding/maintenance, auto-sync timer, backup, ratings refresh) | `workers`/`backup_service` unit tests + runtime observation |
| Search (palette/`/api/search` parity) | `search` unit tests (conversation/customer/knowledge scopes, href shapes) |
| Encryption envelope (`settings::set_secret`, per-profile master key) | `crypto`/`settings` unit tests |
| Navigation/palette href resolution (`href_to_nav` → Inbox/Customers/Knowledge/IssueRadar) | `palette` unit tests in app-shell |
| Page rendering/state per page | app-shell unit tests (pages), 21 pages verified rendering by GUI screenshots |
| Non-2xx error surfacing from LM Studio, OAuth re-save after refresh (single-row store) | production fixes covered by regression tests in `ai_lm_studio.rs` / `helpscout_real.rs` suites |

## What the existing suite does NOT cover (mirrors `coverage-gaps.md`)

- Real-UI interaction (egui hit-testing, widget focus, sidebar geometry) —
  the suite tests pages as state machines and business logic, not the
  rendered widget tree. This is exactly where DEFECT-01 lives: no existing
  test could have caught a click-hitbox compression, because hit-testing is
  exercised only with a real window system.
- Live third-party behavior (Help Scout production API responses, real LM
  Studio inference) — replaced by mocks by design; documented as
  out-of-scope without credentials.
- Multi-window/responsive/DPI behavior — single fixed-size window product.
