# Changelog

All notable changes to **SupportOS+** (SupportOS-Plus) are documented here.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/);
versions follow [Semantic Versioning](https://semver.org/).

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
