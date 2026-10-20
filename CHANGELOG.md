# Changelog

All notable changes to **SupportOS+** (SupportOS-Plus) are documented here.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/);
versions follow [Semantic Versioning](https://semver.org/).

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
