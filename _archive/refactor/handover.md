# Refactor Handover — Operations Manual

Read this every session before touching anything. Pairs with `memory.md`
(durable decisions) and `progress.md` (live state).

**PROGRAM STATE: PROGRAM 4 OPEN — audit-fix closure (R9.1–R9.3)**
(session 26, 2026-11-16; owner directive "fix everything" after the
independent post-v1.10.1 audit). Programs 1–3 stay closed; v1.10.1
remains the published release until R9.3 tags 1.10.2
(session 25, 2026-11-16). Program 1 (20 sessions / CI runs
#301–#359), Program 2 (Q-01..Q-06, sessions 21–24 / CI runs
#360–#383), and Program 3 (R8.1–R8.5, session 25 / CI runs
#386–#389 + Release run#39) are all terminal; their closing
reports live in `complete.md`. **v1.10.1 is published** (tag
`v1.10.1`, target `d4a19f5`, Release run#39 all-6-jobs green,
post-publish live-verified: BUILD-INFO version/commit match, both
binary assets HTTP 200, SHA256SUMS present; v1.10.0 also stands at
its tag). The residue audit is fully resolved — nothing recorded
as deferred/if-ordered remains anywhere in the ledger. The queue
is empty — new work opens only on an owner directive. Steps 1–3
below still apply at every session start.

## 1. What this project is (and is not)

- The repo: **SupportOS-Plus**, a Rust workspace at `/home/z/SupportOS-Plus`
  (clone location). Program crates: `core`, `app-shell`, `catalog`, `xtask`.
- It is NOT the Next.js workspace at `/home/z/my-project` — that is a
  different project in this sandbox. `dev.log`, `bun`, and port 3000 belong
  to it, not to us. Ignore them.
- Build system: cargo + pinned toolchain (1.99.1, from `rust-toolchain.toml`)
  + two GitHub workflows (CI, Release). No Node, no Python in the repo
  (R7.5 ported the no-simulation gate into the xtask `gate`
  subcommand; the release container carries no python3).

## 2. Fresh-sandbox re-provisioning (expect this often)

Session 1 found the clone AND `~/.cargo` wiped. Full recovery:

```bash
# 1. re-clone (credentials are persistent, owner-approved):
TOKEN=$(rg '^GITHUB_TOKEN=' /home/z/my-project/tool-results/github-credentials.txt | cut -d= -f2)
git clone https://kimpearce888:${TOKEN}@github.com/kimpearce888/SupportOS-Plus.git /home/z/SupportOS-Plus
cd /home/z/SupportOS-Plus

# 2. git identity (fresh sandboxes default to the wrong one):
git config user.name kimpearce888
git config user.email kimpearce888@users.noreply.github.com

# 3. toolchain (rust-lang.org is reachable; ~10 min):
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
  | sh -s -- -y --profile default --default-toolchain none
. "$HOME/.cargo/env"
rustup toolchain install        # reads rust-toolchain.toml → 1.99.1
rustc --version                 # must print 1.99.1
```

First build is cold (no `target/`); expect ~10–20 min for the first full
battery, incremental afterwards.

## 3. Tooling facts

- `cargo` needs `export PATH="$HOME/.cargo/bin:$PATH"` in every new shell.
  Partial-reset case (session 11; path updated sessions 13+14 — the
  persistent dir MOVES between resets): if `~/.cargo` is gone, look for
  the surviving toolchain under BOTH `/home/z/my-project/` and
  `/tmp/my-project/` (session 14 found it at `/tmp/my-project/
  {rustup,cargo-home}`) —
  `export RUSTUP_HOME=<that-dir>/rustup CARGO_HOME=<that-dir>/cargo-home
  PATH=<that-dir>/cargo-home/bin:$PATH` (1.99.1 toolchain + crate
  registry cache ride along; saves the §2 toolchain download). NOTE:
  exports do NOT persist across bash tool calls in this sandbox — export
  in EVERY command that runs cargo.
- `python3` (3.12) is present in the sandbox — used only for ad-hoc
  CI monitoring (§5 JSON parsing); the repo itself needs none of it
  (the no-simulation gate is the xtask `gate` subcommand).
- No Vulkan/lavapipe locally → the kittest snapshot suites are **CI-only**;
  never try to run them in the sandbox.
- `cargo machete` is not installed; install ad hoc for R2.3
  (`cargo install cargo-machete`) or skip if crates.io install fails.

## 4. Local verification battery (before every commit)

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cd /home/z/SupportOS-Plus
cargo run --locked -p supportos-plus-xtask -- gate        # gate
cargo run --locked -p supportos-plus-xtask -- unwrap-policy   # unwrap policy
cargo fmt --all --check                                # fmt
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo clippy --locked -p supportos-plus-core --all-targets --features sandbox -- -D warnings
cargo test --locked --workspace
cargo test --locked -p supportos-plus-core --features sandbox
cargo run --locked -p supportos-plus-xtask -- verify-icons
# from R1.0 on, additionally — during R1.x use SUBSET paths (the stripped
# batches so far); the whole-tree check becomes mandatory at R1.10:
cargo run --locked -p supportos-plus-xtask -- strip-comments --check crates/catalog crates/xtask
# strip batch procedure (R1.x): strip --write [paths] -> cargo fmt --all -> battery
```

## 5. CI monitor (GitHub API)

```bash
TOKEN=$(rg '^GITHUB_TOKEN=' /home/z/my-project/tool-results/github-credentials.txt | cut -d= -f2)
curl -s -m 30 -H "Authorization: token $TOKEN" \
  "https://api.github.com/repos/kimpearce888/SupportOS-Plus/actions/runs?per_page=3" \
| python3 -c "
import json,sys
for r in json.load(sys.stdin).get('workflow_runs',[]):
    print(r['name'],'|',r['head_branch'],'|',r['head_sha'][:7],'|',r['status'],'|',r.get('conclusion'),'| run#',r['run_number'],'| id',r['id'])
"
# per-job detail: .../actions/runs/<id>/jobs?per_page=10
```

CI reality: ~25–35 min total; the arm64 packages lane usually finishes
first, the check lane last. A session that pushes at the end may hand CI
verification to the next session's step 3 — that is acceptable; the next
session must not start a batch until the previous head is green.

## 6. Commit / push conventions

- One batch = one commit: `refactor(R<id>): <what changed>`.
- Push once per session (all commits together); the pushed head must be
  battery-green locally.
- Never force-push; never rewrite published history.

## 7. Per-session checklist

1. Read `memory.md` → this file → `progress.md`.
2. §2 re-provision if the sandbox was reset.
3. §5 confirm last head CI-green; if red: diagnose via the run's job log,
   fix, push, stop — do not start new batches on a red base.
4. Execute the CURRENT POINTER batch from `progress.md`.
5. Battery (§4) → commit → update `progress.md` (+`complete.md` if
   terminal, +`memory.md` if decisions/gotchas emerged).
6. Push (§6). Monitor CI if time allows; record the run id in
   `progress.md`.

## 8. Known gotchas (short list — details in `memory.md`)

- Shell `&&` chains silently stop when `rg` finds nothing (exit 1). Use `;`
  between exploratory commands.
- Touching `crates/core/src/audit.rs` → gate allowlist sync duty
  (plan invariant 7; the allowlist lives in
  `crates/xtask/src/gate.rs`).
- Kittest suite file names / test fn names must stay stable (golden keys).
- New code is comment-free (owner constraint 3) — including code written
  for the refactor itself.
