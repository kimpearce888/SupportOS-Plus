# SupportOS+

> **Independent Rust/egui desktop reimplementation of [supportos](https://github.com/kimpearce888/supportos).**
> Local-first. No telemetry. No cloud AI.

SupportOS+ is a ground-up rewrite of the SupportOS support-desk platform in pure
Rust, trading the original TypeScript/React stack for a single-language workspace
that compiles to a fast, self-contained desktop binary. Everything the original
does — unified inbox, real-time conversation sync, semantic docs search, SLA
reporting — is rebuilt on an embedded, zero-service architecture that never
leaves your machine.

Repository: <https://github.com/kimpearce888/SupportOS-Plus> ·
Releases: <https://github.com/kimpearce888/SupportOS-Plus/releases>

**Naming layers** (applied where each is allowed): `SupportOS+` is the display
name — window title, desktop menu entry, release title. `SupportOS-Plus` is the
product identifier — repository, window/app ID, artifact base name.
`supportos-plus` is the Debian package name and, since 1.0.0, also the
installed command and internal crate identifier. (Pre-1.0.0 builds shipped
the command `supportos-plusplus`; see the upgrade note below.)

---

## Install

Install-ready Linux packages are published on the
[Releases](https://github.com/kimpearce888/SupportOS-Plus/releases) page, built
and verified by CI from the tagged source commit. Both packages carry the same
release binary, including the Landlock + seccomp parse sandbox.

| Package | File | Target |
|---|---|---|
| `.deb` | `supportos-plus_<version>_amd64.deb` | Ubuntu 22.04 LTS / 24.04 LTS, Debian 12+ |
| `.AppImage` | `SupportOS-Plus-<version>-x86_64.AppImage` | any modern x86_64 desktop (glibc >= 2.35) |

**System requirements:** x86_64 (amd64) Linux with glibc 2.35 or newer, a desktop
session (X11 or Wayland) and a GL or Vulkan driver (Mesa software rendering
works on GPU-less machines). ~200 MB free disk space. No compilation, no
development tools, no Docker, no external services, no manual configuration.

### .deb

```sh
sudo apt install ./supportos-plus_<version>_amd64.deb
```

APT downloads and resolves every declared dependency automatically — the
package's `Depends` set covers the full X11/GL/Wayland client-library surface
the GUI loads at runtime. Launch **SupportOS+** from the desktop application
menu, or run `supportos-plus`.

### AppImage

```sh
chmod +x SupportOS-Plus-<version>-x86_64.AppImage
./SupportOS-Plus-<version>-x86_64.AppImage
```

Runs without installation; the type-2 AppImage bundles the binary, icons and
desktop entry (AppImageLauncher integrates it into the menu). Only the host
display server and GL/Vulkan driver are used from the system.

**First run** initializes itself: the SQLite database is created (all
migrations applied) under `~/.local/share/supportos-plus/` (override with
`SPP_DATA_DIR`), background workers start, a loopback listener serves Help
Scout webhooks and the OAuth callback on `127.0.0.1`, and the shell boots into
demo mode unless Help Scout credentials are configured. No setup steps.

**Upgrading from 0.2.x:** the first 1.0.0 launch moves your existing data from
the old `~/.local/share/supportos-plusplus/` directory to the new location
(and renames the database inside it) automatically — one atomic directory
rename, nothing lost, no manual steps. Downgrading back to 0.2.x afterwards
is not supported.

**Checksums:** every release ships `SHA256SUMS.txt` covering both packages;
`sha256sum -c SHA256SUMS.txt` after download.

## What's inside (from the source)

A single Cargo workspace, split by concern:

| Crate | Role |
|---|---|
| `crates/catalog` | Shared type catalog re-exported by every other crate |
| `crates/core` | Pure-Rust business core — SQLite persistence, sync engine, Help Scout provider (real + demo), AI providers, sqlite-vec vector store, loopback listener. No UI deps. |
| `crates/app-shell` | Native desktop shell (eframe/egui) — 21 pages render in-process against `core`, no HTTP hop |
| `crates/xtask` | Developer entry point — `dev` / `lint` / `verify-icons` / `package` / `deb` / db ops (`migrate`/`backup`/`restore`/`seed`/`healthcheck`/`vector-count`) |

The 21 shell pages: Shell, Dashboard, Inbox, Customers, Knowledge, Reports,
Operations, AI Center, Automation, Connectors, Custom Objects, Incidents,
Issue Radar, Notifications, Onboarding, Organizations, Outreach, Search,
Settings, Support Graph, Sync Health.

## Key design decisions

- **Native egui shell.** The Phase 4 cutover replaced the Leptos/WASM frontend
  and the Tauri 2 webview shell with a single eframe/egui desktop app — no
  browser engine, no CSP plumbing, no wasm target. Pages query the core
  directly through the `DbHandle` facade (writer lane + 2 WAL reader
  connections behind it); live updates flow over the in-process `EventBus`
  and wake the render loop on demand.
- **sqlite-vec vector index (decision D4).** The chunk count sits far below the
  100k gate, so dense vectors live in a `vec0` SQLite table behind the
  `VectorStore` trait — the store stays swappable, and there is no embedded
  qdrant engine to compile or babysit.
- **Loopback listener.** A minimal `127.0.0.1` HTTP listener serves exactly two
  routes: `POST /webhook` (Help Scout event ingress, HMAC-SHA1 verified) and
  `GET /oauth/callback` (single-use state, code exchange) — the only inbound
  network surface of the desktop app.
- **SQLite everywhere.** `rusqlite` (bundled) for tickets, conversations and docs;
  business-minutes SLA math with full IANA timezone parity (`chrono-tz`).
- **One crypto module.** All encrypted artifacts (`.sosync` sync bundles, settings
  exports) share `core::crypto`: scrypt key derivation + AES-256-GCM sealing,
  written once and tested.
- **Local AI adapters.** LM Studio, Ollama and generic OpenAI-compatible endpoints
  via `reqwest` with SSE streaming — pointed at `localhost` services only.
- **Hardened input handling.** `ammonia` HTML sanitization, bounded NFA regex
  (no ReDoS surface), SSRF-guarded URL parsing for connectors, and a
  Landlock + seccomp sandbox around pdf/docx parsing — compiled into the
  shipped packages (kernels without Landlock degrade to seccomp-only,
  logged).
- **Small release binaries.** LTO + `opt-level = "s"` + stripped symbols.

## Building from source

Requires the pinned Rust toolchain (see `rust-toolchain.toml`) and no build-time
system packages — the egui shell needs no webkit and no wasm target. At runtime
a machine needs the usual Linux desktop graphics stack: an X11 (or Wayland)
session, `libxkbcommon-x11`, and a GL or Vulkan implementation (Mesa;
software rendering via llvmpipe/lavapipe works fine on GPU-less and headless
setups).

```sh
cargo xtask dev
cargo xtask lint
cargo xtask package   # AppImage → target/appimage/SupportOS-Plus-x86_64.AppImage
cargo xtask deb       # .deb     → target/deb/supportos-plus_<version>_amd64.deb
cargo test            # unit tests (crypto round-trips, HMAC vectors, SLA math, page models)
```

`xtask package` stages the AppDir (release binary, `.desktop` entry, hicolor
icons, `AppRun`) and runs `appimagetool` — located via `APPIMAGETOOL`, cached
under `target/tools/`, or downloaded from the official continuous release.
The AppImage bundles the X11 keyboard libraries (`libxkbcommon`,
`libxkbcommon-x11`, `libxcb-xkb`) from the build host so it also opens windows
on GUI-less systems, so the build host must have `libxkbcommon0
libxkbcommon-x11-0 libxcb-xkb1` installed (every desktop machine has them; CI
installs them explicitly). `xtask deb` stages the same payload for dpkg and
packs it with `dpkg-deb --root-owner-group`. The Tauri-era msi/nsis/dmg/app
targets are gone; the two Linux packages are the complete packaging surface.

## Verification

- **CI (every push to main):** fmt, clippy (`-D warnings`), the full workspace
  test suite, the sandbox-feature tests, `cargo-deny` advisories/licenses, plus
  an AppImage build, a `.deb` build, and boot-chain smoke runs of both
  packages (`SPP_SMOKE=1` verifies logging, single-instance lock, migrations,
  boot services, background workers and the loopback listener).
- **Release pipeline (every tag):** builds both packages on the ubuntu:22.04
  glibc 2.35 baseline, installs the `.deb` through APT on clean
  ubuntu:22.04 / ubuntu:24.04 / debian:12 containers (dependency resolution,
  desktop entry + icons, boot smoke, GUI launch under Xvfb using only the
  declared dependencies, reinstall, uninstall with user data preserved,
  purge), launches the AppImage under Xvfb, then publishes the release with
  SHA-256 checksums and live-verified download links. Nothing is published
  unless every step passes.

## License

MIT
