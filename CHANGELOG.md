# Changelog

All notable changes to **SupportOS+** (SupportOS-Plus) are documented here.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/);
versions follow [Semantic Versioning](https://semver.org/).

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

[1.0.0]: https://github.com/kimpearce888/SupportOS-Plus/releases/tag/v1.0.0
[0.2.0]: https://github.com/kimpearce888/SupportOS-Plus/releases/tag/v0.2.0
