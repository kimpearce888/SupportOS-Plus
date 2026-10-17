# SupportOS++

> **Independent Rust/Tauri 2 desktop reimplementation of [supportos](https://github.com/kimpearce888/supportos).**
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
| `crates/core` | Pure-Rust business core — SQLite persistence, sync engine, AI providers, embedded vector store. **No UI, no Tauri deps.** |
| `crates/ui` | Leptos/WASM frontend; depends on `core` types only |
| `crates/app/src-tauri` | Tauri 2 desktop shell; serves the WASM bundle and bridges to `core` |
| `crates/xtask` | Developer entry point — `dev` / `lint` / `verify-icons` / `package` / db ops (`migrate`/`backup`/`restore`/`seed`/`healthcheck`) |

## Key design decisions

- **Embedded vector engine.** [`qdrant-edge`](https://crates.io/crates/qdrant-edge) runs in-process
  for semantic docs search — no separate Qdrant server to install or babysit.
- **SQLite everywhere.** `rusqlite` (bundled) for tickets, conversations and docs;
  business-minutes SLA math with full IANA timezone parity (`chrono-tz`).
- **Encrypted backups.** AES-GCM + scrypt key derivation; constant-time comparison via `subtle`.
- **Loopback-only HTTP.** An `axum` listener bound to `127.0.0.1` bridges the webview
  and the Rust core — no network exposure, ever.
- **Local AI adapters.** LM Studio, Ollama and generic OpenAI-compatible endpoints
  via `reqwest` with SSE streaming — pointed at `localhost` services only.
- **Hardened input handling.** `ammonia` HTML sanitization, bounded NFA regex
  (no ReDoS surface), SSRF-guarded URL parsing for connectors.
- **Small release binaries.** LTO + `opt-level = "s"` + stripped symbols.

## Building

Requires a stable Rust toolchain (1.80+) with the `wasm32-unknown-unknown` target
and the Tauri 2 prerequisites for your platform.

```sh
rustup target add wasm32-unknown-unknown

cargo xtask dev
cargo xtask lint
cargo xtask package
```

## License

MIT
