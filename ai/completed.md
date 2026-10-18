# completed.md — completed and verified items

> Items land here from `progress.md` only after Rule 5 verification. Each entry records the
> work done and the verification evidence (test run, CI run, build result).

## Completed

### Batch 1 — 2026-10-18 (commit `560a5ce`, CI run `39112162370`: success)

**1. Add `LICENSE` (MIT, matching manifests)**
- Work: `LICENSE` file added at repo root with the standard MIT text (copyright 2026 kimpearce888).
- Verification: `cargo deny check licenses` passes with MIT in the allow list (local + CI);
  the workspace `license = "MIT"` declaration and the file now agree.

**2. Add CI: fmt + clippy + test + cargo-deny**
- Work: `.github/workflows/ci.yml` — runs on every push/PR: `cargo fmt --all --check`,
  `cargo clippy` (catalog/core/xtask, `--all-targets -D warnings`), `cargo test -p
  supportos-plusplus-core`, `cargo deny check advisories licenses` via taiki-e/install-action.
  `deny.toml`: permissive-license allow list (MIT, Apache-2.0, BSD, ISC, Zlib, MPL-2.0,
  CC0-1.0, BSL-1.0, …); 8 unmaintained-crate advisories from the leptos/tauri/xtask trees
  ignored with documented reasons (most disappear at the Phase 4 cutover). Clippy/test are
  scoped to catalog/core/xtask until the Phase 4 cutover (ui needs wasm32, app needs WebKitGTK).
- Verification: CI run `39112162370` on commit `560a5ce` completed with conclusion `success`
  (all four steps green). Also reproduced locally with cargo-deny 0.20.2: advisories ok,
  licenses ok.

**3. Characterization tests: `webhook_handler`**
- Work: 7 tests in `crates/core/src/webhook_handler.rs` — accept + persist + job enqueue
  (convo.created → sync_conversation), empty body → BadRequest, non-JSON body → BadRequest,
  wrong signature → SignatureInvalid, missing signature with secret set → SignatureInvalid,
  empty secret skips verification (characterizes CURRENT behavior; flips with plan item 6 /
  map row 1), duplicate event → same row id, drain_pending processes stored events.
- Verification: `cargo test -p supportos-plusplus-core` — 32/32 pass locally and in CI.

**4. Characterization tests: `backup_service`**
- Work: 4 tests in `crates/core/src/backup_service.rs` — backup creates a verified DB (VACUUM
  INTO, integrity_check ok) plus a `.settings.json` sidecar that redacts token/secret/password
  keys; verify_backup rejects non-database files; restore swaps the database file, removes
  stale `-wal`/`-shm`, and preserves data; restore reports missing backups; prune keeps the
  newest N and removes pruned sidecars.
- Verification: `cargo test -p supportos-plusplus-core` — 32/32 pass locally and in CI.

**5. Characterization tests: `encrypted_sync`**
- Work: 4 tests in `crates/core/src/encrypted_sync.rs` — export rejects passphrases < 8 chars;
  export→verify round-trip (SOSYNC magic present, no plaintext SQLite header in the bundle,
  right passphrase verifies, wrong passphrase rejected); tampered ciphertext fails GCM
  authentication; full import round-trip restores data with a `.pre-import-` safety backup.
- Verification: `cargo test -p supportos-plusplus-core` — 32/32 pass locally and in CI.

**Batch verification summary:** `cargo fmt --all --check` clean · `cargo clippy -D warnings`
clean (catalog/core/xtask, all targets) · `cargo test` 32/32 (15 new characterization tests +
17 pre-existing) · `cargo deny check advisories licenses` ok · GitHub Actions run
`39112162370` → success.
