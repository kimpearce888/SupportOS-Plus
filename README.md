# SupportOS++

> **Independent Rust/egui desktop reimplementation of [supportos](https://github.com/kimpearce888/supportos).**
> Local-first. No telemetry. No cloud AI.

SupportOS++ is a ground-up rewrite of the SupportOS support-desk platform in pure
Rust, trading the original TypeScript/React stack for a single-language workspace
that compiles to a fast, self-contained desktop binary. Everything the original
does — unified inbox, real-time conversation sync, semantic docs search, SLA
reporting — is rebuilt on an embedded, zero-service architecture that never
leaves your machine.

---

## Architecture

A single Cargo workspace, split by concern:

| Crate | Role |
|---|---|
| `crates/catalog` | Shared type catalog re-exported by every other crate |
| `crates/core` | Pure-Rust business core — SQLite persistence, sync engine, AI providers, sqlite-vec vector store, loopback listener. No UI deps. |
| `crates/app-shell` | Native desktop shell (eframe/egui) — every page renders in-process against `core`, no HTTP hop |
| `crates/xtask` | Developer entry point — `dev` / `lint` / `verify-icons` / `package` / db ops (`migrate`/`backup`/`restore`/`seed`/`healthcheck`) |

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
  shipped AppImage (kernels without Landlock degrade to seccomp-only,
  logged).
- **Small release binaries.** LTO + `opt-level = "s"` + stripped symbols.

## Building

Requires the pinned Rust toolchain (see `rust-toolchain.toml`) and no build-time
system packages — the egui shell needs no webkit and no wasm target. At runtime
a machine needs the usual Linux desktop graphics stack: an X11 (or Wayland)
session, `libxkbcommon-x11`, and a GL or Vulkan implementation (Mesa;
software rendering via llvmpipe/lavapipe works fine on GPU-less and headless
setups).

```sh
cargo xtask dev
cargo xtask lint
cargo xtask package   # AppImage → target/appimage/SupportOs-Plus-x86_64.AppImage
cargo test          # unit tests (crypto round-trips, HMAC vectors, SLA math, page models)
```

`xtask package` stages the AppDir (release binary, `.desktop` entry, hicolor
icons, `AppRun`) and runs `appimagetool` — located via `APPIMAGETOOL`, cached
under `target/tools/`, or downloaded from the official continuous release. It
is the only desktop packaging target; the Tauri-era deb/msi/nsis/dmg/app
targets are gone.

## License

MIT
