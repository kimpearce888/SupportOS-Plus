# Build and Install Record — SupportOS-Plus

Status: BUILD COMPLETE; LAUNCH VERIFIED (screenshots captured). QA phase ongoing.
Audited revision: `main` @ `3affe7e` (HEAD at clone time, 2026-10-24 UTC)

## Environment (ENV-001 record; note: CSV verification method names ENVIRONMENT.md — resolved here per loading record)

- Repository URL: https://github.com/kimpearce888/SupportOS-Plus
- Local path: /home/z/my-project/audit/SupportOS-Plus (fresh clone, 2026-10-24)
- Branch: `main`; HEAD `3affe7e`; remote main in sync (cloned from origin)
- Working-tree state at start: clean (no uncommitted user changes); only new untracked `repo-audit-2026-10-24/` created by this audit
- OS: Debian GNU/Linux (sandbox); kernel 5.10.134-013.15.kangaroo.al8.x86_64; 2 vCPU; 4159 MB RAM
- Disk at audit start: 9.9G root fs, 16% used (7.9G free)
- Toolchain: rustc/cargo **not preinstalled** → installed rustup default 1.99.1 minimal + rustfmt + clippy at /home/z/.cargo (matches `rust-toolchain.toml` pin `1.99.1`); verified `cargo 1.99.1 (5f94df478 2026-08-27)`
- Display: Xvfb + xvfb-run available (`/usr/bin/Xvfb`, `/usr/bin/xvfb-run`); no real display server
- Capture: ffmpeg present (`/usr/bin/ffmpeg`) → x11grab screenshots/video **verified working** (see evidence/screenshots/BUILD/); python-xlib installed and import-verified (XTEST input injection available)
- Missing system libraries on this sandbox (no root): `libxkbcommon-x11.so.0`, `libxcb-xkb.so.1` — required by the app's winit/xkbcommon-dl GUI stack; remediated **environment-only** via `apt-get download` + local `dpkg -x` extraction to `/home/z/.local/audit-libs/` with `LD_LIBRARY_PATH` (app binary and code untouched); unversioned `.so` symlinks created (normally provided by `-dev` packages)
- Network: outbound HTTPS available (GitHub API + Drive verified earlier)
- Clock: system date 2026-10-24 UTC (consistent with repo commit timeline)
- NOTE (sandbox behavior): background processes are reaped between tool calls; every QA batch therefore runs as a self-contained call (Xvfb → app → interactions → screenshots). Helpers: `run-app.sh`, `start-xvfb.sh` in this audit folder

## Build commands and outcome (BUILD-001)

- Workspace: 4 members — `crates/app-shell`, `crates/catalog`, `crates/core`, `crates/xtask` (Cargo.toml, resolver 3, edition 2024, version 1.7.0)
- Command: `cargo build` with `RUSTUP_HOME=/home/z/.rustup CARGO_HOME=/home/z/.cargo CARGO_INCREMENTAL=0` (incremental disabled per prior-art ENOSPC note and to save disk)
- Outcome: **SUCCESS** — "Finished `dev` profile target(s) in 3m 57s", **0 warnings**; artifacts `target/debug/supportos-plus` (170 MB debug binary) and `target/debug/xtask`
- Sandbox note: an earlier nohup'd background build was killed by the sandbox between tool calls (empty log); the successful build ran in foreground and was verified by a second no-op `cargo build` invocation ("Finished" with no recompiles)

## Launch and first-run (BUILD-002) — VERIFIED

- Attempt 1 & 2: **crash** — `xkbcommon-dl` panic: `libxkbcommon-x11.so could not be loaded` (missing system lib on sandbox; log /tmp/app-run1.log, /tmp/app-run2.log). Root cause: environment, not app code. Remediated via local deb extraction (see Environment).
- Attempt 3 (after lib fix): **SUCCESS** — app logs `SupportOS-Plus native shell starting`, DB at `/home/z/.local/share/supportos-plus/supportos-plus.db`, **52 migrations applied** (initial_schema → m061_knowledge_chunk_attempts), background workers started, glow renderer, window `SupportOS+` 1180x760 mapped on Xvfb :99
- Foreground run held 20s until `timeout` killed it (exit 124 = alive the whole time); UI rendered (verified visually)
- Evidence: `evidence/screenshots/BUILD/BUILD-002-launch-initial.png` (validated: 1600x1000 PNG, dashboard content visible: sidebar nav Workspace/Customers & knowledge/Insights/Automation & AI, header "never synced" badge + Search ⌘K + theme toggle, Range/Channel/Mailboxes filter chips, 12 zero-value metric cards, "New conversations per day" chart, "Sync now" button, v1.7.0 label)
- First-run observations: fresh DB initializes empty (all metrics 0); log WARN `loopback listener could not be started, error: Address already in use (os error 98)` at main.rs:129 — **candidate finding F-CAND-001**: need to identify port, cause, and user impact (likely a dev single-instance port; see single_instance.rs); app continues without the listener
- App data persists across launches in `~/.local/share/supportos-plus/` (supports restart-persistence testing)

## Existing test-suite results (TEST-001 — supplementary evidence, separate from manual QA)

Command: `cargo test --workspace` (dev profile, CARGO_INCREMENTAL=0), 2026-10-24, revision 3affe7e:

| Suite | Result |
|---|---|
| spp_shell (app-shell) | 228 passed / 0 failed / 0 ignored (208.9 s) |
| spp_core | 466 passed / 0 failed / 4 ignored (409.4 s) |
| spp_xtask | 9 passed / 0 failed |
| spp_catalog | 0 tests |
| bin targets (main.rs/xtask.rs) | 0 tests |
| **Total** | **703 passed / 0 failed / 4 ignored** |

Notes: two earlier runs hit the 560 s tool-call window during compilation/long suites; completion achieved via per-package runs (`-p supportos-plus-core`, `-p supportos-plus-xtask -p supportos-plus-catalog`) after binaries were compiled. Exit codes 0. Per the source-of-truth rules this does NOT establish product correctness; manual QA proceeds independently.

## Prior-art hints (non-authoritative)

Previous audit (qa-audit/20261019T050847Z/audit-metadata.json) reports the same
environment family ran this workspace successfully with Xvfb + llvmpipe + XTEST +
ffmpeg, rustc 1.99.1, and a debug build — treated as a hint only; current build
result will be established by direct observation.
