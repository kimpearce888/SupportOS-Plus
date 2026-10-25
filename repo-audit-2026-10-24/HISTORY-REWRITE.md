# FIND-003 history purge — authorized operation record (2026-10-25)

## Authorization

Maintainer instruction, this session: **"for FIND-003 you have authorization, do it."**

This lifted the audit-time constraint *"no history rewrite / no force-push"* for
this single operation only, executing option (b) of the FIND-003 row in
FIX-RECOMMENDATIONS.md (*"purge the artifacts"*). No other finding's fix
authorization changed.

## What was purged

The prior QA session's key material — 8 files, 32-byte binary secret-envelope
keys — removed from **every commit of every ref**:

```
qa-audit/20261019T050847Z/evidence/artifacts/qa-profile-A/secrets.key
qa-audit/20261019T050847Z/evidence/artifacts/qa-profile-smoke/secrets.key
qa-audit/20261019T050847Z/evidence/artifacts/qa-profile-t1/secrets.key
qa-audit/20261019T050847Z/evidence/artifacts/qa-profile-t2/secrets.key
qa-audit/20261019T050847Z/evidence/artifacts/qa-profile-t3/secrets.key
qa-audit/20261019T050847Z/evidence/artifacts/qa-profile-t4/secrets.key
qa-audit/20261019T050847Z/evidence/artifacts/qa-profile-t6/secrets.key
qa-audit/20261019T050847Z/evidence/artifacts/smoke-datadir/secrets.key
```

Reachability fact that forced the rewrite scope: `git tag --contains` proved
**all 11 release tags** (v0.2.0 … v1.7.0) descend from the secret-introducing
commits (`7a0a095`, `47c752d`, `6bd7f9f`, `b345c8d`), so the tags were rewritten
together with `main`. (Note: commit `6bd7f9f` had once removed this material,
but it was re-added later — it was still tracked at the pre-rewrite head.)

## Method

- `git filter-repo` (version `a40bce548d2c`), `--force --invert-paths` with the
  8 exact paths plus catch-all `--path-glob '*secrets.key'`;
  `--replace-refs delete-no-add`.
- 164 commits parsed, history rewritten in 0.05 s; repo repacked (2710 objects,
  20.7 MB pack). `git reflog expire --expire=now --all` + `git gc --prune=now`
  removed every pre-rewrite object locally.
- `filter-repo` removes the `origin` remote by design; it was re-added verbatim
  (`https://github.com/kimpearce888/SupportOS-Plus.git`).
- Scope kept tight per the published recommendation: `instance.lock` artifacts
  (runtime lock state, non-secret) were deliberately **not** included — see
  §Residual risks.

## Ref mapping (old → new)

| Ref | Pre-rewrite | Rewritten |
|---|---|---|
| `refs/heads/main` | `07777a1` (07777a12f850…) | `00eb4fa` (00eb4fa611f0…) — head is `ae46cf3` after the guard commit below |
| `refs/tags/v0.2.0` | `02ba9a8` | `5870e73` |
| `refs/tags/v1.0.0` | `0d56c1f` | `ae0ca54` |
| `refs/tags/v1.1.0` | `a166ab8` | `5f02261` |
| `refs/tags/v1.1.1` | `cb46de5` | `4e3d815` |
| `refs/tags/v1.1.2` | `d3fd986` | `af85244` |
| `refs/tags/v1.1.3` | `34d40bc` | `bd80c47` |
| `refs/tags/v1.1.4` | `b2cc03d` | `ce6c3c7` |
| `refs/tags/v1.2.0` | `92770d8` | `ddca519` |
| `refs/tags/v1.3.0` | `c6f2e95` | `787d3d9` |
| `refs/tags/v1.4.0` | `f867003` | `bf2dbec` |
| `refs/tags/v1.7.0` | `c5e2296` | `4d4e3da` |

Full per-commit translation: `HISTORY-REWRITE-commit-map.txt` (164 commits) and
`HISTORY-REWRITE-ref-map.txt`, copied out of `.git/filter-repo/` before any gc.

Commit **messages** that quote old hashes (e.g. checkpoint messages such as
"checkpoint 17 … c2a1023") intentionally keep the pre-rewrite SHAs: messages are
historical narrative and were not rewritten. This file is the authoritative
old→new translation layer for every hash referenced anywhere in the audit docs.

## Forward guard (committed on the rewritten head as `ae46cf3`)

- `.gitignore`: `secrets.key` (matches at any depth) — must never be tracked again.
- `crates/xtask/src/lib.rs`: two `#[cfg(test)]` hygiene regression tests —
  (1) no file named `secrets.key` anywhere in the tree, (2) the `.gitignore`
  rule must stay in place.
- Full suite after the rewrite + guard: **706 passed / 0 failed / 4 ignored**
  (app-shell 229, core 466, xtask 11 = pre-rewrite baseline 704 + 2 new guards).

## Backups (retained locally, never pushed)

- `audit/spp-pre-rewrite-20261025T021728.bundle` (21 MB, `git bundle verify` passed;
  complete pre-rewrite history incl. all tags).
- `audit/spp-mirror-backup-20261025T021728.git` (24 MB bare mirror).
- ⚠️ Both contain the purged material by definition — keep local-only and destroy
  once the maintainer confirms the remote situation is settled.

## Verification evidence (all executed 2026-10-25)

1. **Local post-rewrite scans**: no `secrets.key` in any ref's history
   (`git log --all --name-only`), in `git ls-files`, or on disk (`rg --files`).
2. **Push**: `git push --force-with-lease=refs/heads/main:07777a12…` — the lease
   was pinned to the pre-rewrite remote head, so the push would have failed had
   the remote moved meanwhile; then `git push --force origin --tags` for the 11
   rewritten annotated tags. `git ls-remote` afterwards shows only rewritten
   SHAs; zero pre-rewrite SHAs remain in any remote ref; `main` is still the
   only branch.
3. **Independent fresh clone** of the GitHub remote into an empty directory:
   history scan clean, tree scan clean, 11 tags present, head `ae46cf3`.
4. **Byte-exact content-leak cross-check**: the 8 purged blobs were extracted
   from the backup mirror (10 nonempty key lines) and searched as raw bytes
   across the entire tracked tree (738 files, `.git`/`target` excluded):
   **no occurrence** — no purged key content appears in any doc or evidence
   file. (An earlier `rg -F -x` attempt was invalid for binary patterns —
   it aborted on UTF-8 validation and was replaced by this byte-exact scan.)

## Residual risks / follow-ups (honest disclosure)

- **GitHub-side retention is outside git control**: after a force-push the old
  commits become unreachable, but GitHub may retain unreachable objects (event
  caches, server-side GC lag). If server-side eviction is wanted, GitHub Support
  can run a GC; only refs are controllable from here. No forks/PRs existed
  (`ls-remote` shows no `refs/pull/*`), and no other clone is known.
- **`instance.lock` (8 files) remains tracked** under the same
  `qa-audit/20261019T050847Z/evidence/artifacts/` tree: runtime lock state, not
  secret material, and outside FIND-003's documented scope. Optional hygiene
  follow-up: `git rm --cached` + a `.gitignore` entry — deliberately not folded
  into this authorized rewrite.
- **Rotation discipline (option (b) second half)**: session records describe the
  qa-profiles as fully synthetic fixtures (32-byte generated keys, no embedded
  secrets), matching the finding's Low severity — but if any profile ever
  encrypted real data with these keys, rotate/re-encrypt those payloads now.
- Operation executed in session `web-7a117f68-2ddf-4d17-85d9-048c274a5b02`
  (IM gateway metadata), 2026-10-25, Asia/Dhaka.
