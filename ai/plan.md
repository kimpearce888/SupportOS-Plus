# SupportOs-Plus — TODO

> Remaining work items, executed five at a time under `rule.md`.
> Status flow: `plan.md` → `progress.md` → implementation → verification → `completed.md`.
> Planning context and decision ledger: `decisions.md`.

## Items

1. Create minimal loopback listener (webhook + OAuth callback only)
2. AppImage packaging via `xtask package`; drop deb/msi/nsis/dmg/app targets
3. Pin final toolchain; drop wasm32 target (row 15 complete)
4. DB pool inside `DbHandle`: writer thread + 2 WAL readers; migrate remaining lock sites
5. sqlite-vec swap behind `VectorStore` (per D4 gate outcome); remove qdrant-edge if GO
6. tokio feature slim + 2-thread runtime (row 12)
7. Landlock + seccomp sandbox for pdf/zip parsing (Linux, feature-flagged)
8. CI: AppImage build + smoke-test job
