# UX Audit Scope — SupportOS+ (SupportOS-Plus)

## What the app is

SupportOS+ is a local-first desktop support desk (single-binary Rust app,
eframe/egui 0.37 GUI, glow renderer by default / wgpu opt-in). It mirrors a
Help Scout account (conversations, customers, docs/knowledge, teams,
mailboxes) into a local SQLite store, adds local AI (LM Studio), automation,
SLA reporting, semantic search and a webhook/OAuth loopback server. No web
frontend, no server-rendered templates — the "frontend" is the egui
immediate-mode UI.

## Stack summary

| Layer | Technology |
|---|---|
| GUI framework | eframe/egui 0.37 (immediate mode), glow renderer default |
| Design system | in-repo: `crates/app-shell/src/design/` (tokens, controls), `theme.rs`, `palette.rs`, `icons.rs`, `chart.rs` |
| Pages | `crates/app-shell/src/pages/` — 23 pages (Dashboard, Inbox, Live chats, Operations, Customers, Organizations, Knowledge, Reports, Issue Radar, Incidents, Custom Objects, Automation, AI Center, Outreach, Notifications, Search, Support Graph, Connectors, Diagnostics, Sync Health, Settings, Onboarding, Shell) |
| App shell | `crates/app-shell/src/app/` (chrome, nav, toasts, overlays, mention popup) |
| Backend crate | `crates/core` — DB (rusqlite, 54 migrations), Help Scout client, sync engine, OAuth, loopback HTTP server, webhook handler, AI services, workers |
| Tests | 889 `#[test]`s; kittest snapshot UI tests per page (`crates/app-shell/tests/`, feature `kittest-ui`, snapshots in `tests/snapshots/`), `style_lint.rs` |
| Build | cargo workspace, pinned toolchain 1.99.1, `cargo fmt`/`clippy`/`deny` gates via CI |

## Frontend/backend boundary (HARD CONSTRAINT 1-2)

### ALLOWED (frontend — presentational, safe to change)

- `crates/app-shell/src/pages/**` — all page UI code (layout, widgets, copy shown to users, empty states, hover/focus/disabled states)
- `crates/app-shell/src/design/**` — shared design tokens, buttons, inputs, cards, section headers (the "component library")
- `crates/app-shell/src/app/**` — shell chrome: sidebar/nav, top bar, toasts, overlays
- `crates/app-shell/src/theme.rs`, `palette.rs`, `icons.rs`, `chart.rs`, `mention_popup.rs`, `overlays.rs`, `toasts.rs` — theme and visual primitives
- `crates/app-shell/tests/**` — kittest UI tests + snapshots (regression evidence; adding/updated snapshots for verified fixes)
- `ux-audit/reports/**` — audit deliverables (markdown only)
- `ai/plan.md`, `ai/progress.md`, `ai/completed.md` — rule.md workflow ledgers (batch bookkeeping only)

Rationale: these files only decide what the user SEES — layout, spacing,
colors, typography, labels, icons, interaction states, responsive behavior
at window sizes. They read from the backend; they never define business
logic.

### FORBIDDEN (backend — do not touch)

- `crates/core/**` — all services: DB schema/migrations, Help Scout API
  client, sync engine, OAuth state machine, loopback/webhook servers, AI
  providers, workers, crypto, backup, business logic
- `crates/catalog/**` — seeded reference catalog data
- `crates/xtask/**` — packaging tooling
- `crates/*/Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`, `deny.toml` — build/supply-chain config
- `.github/**` — CI/release workflows
- `scripts/**`, `docs/**`, `manual/**`, `mockups/**` — ops/docs material
- `.gitignore` beyond the already-added ux-audit lines

### Boundary judgment calls

- Page code in `app-shell` that calls `spp_core` APIs: the CALL is
  untouchable behavior, but how results are PRESENTED is in scope.
- UI copy (labels, hints, error text shown from backend errors) is in scope
  when it is rendered by the frontend; re-wording a frontend label is a UI
  fix, changing an error string produced by `core` is NOT.
- If a defect's root cause is in `core` (e.g. a malformed date string
  produced by the backend), the issue is logged **Blocked by backend** with
  evidence and a suggested change — no exception.

## How the app is run (audit environment)

- Binary: `cargo build -p supportos-plus-app-shell --bin supportos-plus`
  (debug build; UI identical to release)
- Data dir override: `SPP_DATA_DIR` (fresh dir = clean first-run state)
- Headless GUI: Xvfb (1920x1080 primary; 2560x1440 and 1440x900 spot
  checks) + Mesa llvmpipe software GL; screenshots via `ffmpeg -f x11grab`
- Help Scout connection: env vars (`HELPSCOUT_CLIENT_ID`, `CLIENT_SECRET`,
  `REDIRECT_URI`, `DOCS_API_KEY`) from gitignored `ux-audit/scripts/app.env`;
  OAuth loopback binds the redirect port; webhook secret is generated in the
  Help Scout test account and pasted into Settings (journey-driven)
- Alternate UI instrument: kittest harness renders any page with seeded
  data in both themes at 1440/1024/800 widths — used for deterministic
  before/after evidence and regression snapshots
- Baseline gates: `cargo fmt --all -- --check`, `cargo clippy --workspace
  --all-targets -- -D warnings`, `cargo test --workspace`, kittest suite
  with `--features kittest-ui`

## Out of scope (won't fix by policy)

- Real email sending, real payments, destructive actions against real Help
  Scout data (test data only; destructive UI verified to confirmation step
  then cancelled)
- Backend behavior changes of any kind (see FORBIDDEN)
