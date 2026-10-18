# progress.md — active items

> Exactly five active items at any time, per `rule.md`. Items move here from `plan.md` (Rule 2)
> and leave for `completed.md` only after verification (Rule 5).

## Active items

1. Create minimal loopback listener (webhook + OAuth callback only)
2. AppImage packaging via `xtask package`; drop deb/msi/nsis/dmg/app targets
3. Pin final toolchain; drop wasm32 target (row 15 complete)
4. DB pool inside `DbHandle`: writer thread + 2 WAL readers; migrate remaining lock sites
5. sqlite-vec swap behind `VectorStore` (per D4 gate outcome); remove qdrant-edge if GO
