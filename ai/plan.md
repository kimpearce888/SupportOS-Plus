# SupportOs-Plus — TODO

> Remaining work items, executed five at a time under `rule.md`.
> Status flow: `plan.md` → `progress.md` → implementation → verification → `completed.md`.
> Planning context and decision ledger: `decisions.md`.

## Items

1. Port: operations page
2. Port: remaining 17 pages
3. Port: command palette, overlays, toasts
4. Event bus → repaint wiring across all ported pages
5. Cutover: delete axum router + Leptos + Tauri + wasm32 target + CSP plumbing
6. Create minimal loopback listener (webhook + OAuth callback only)
7. AppImage packaging via `xtask package`; drop deb/msi/nsis/dmg/app targets
8. Pin final toolchain; drop wasm32 target (row 15 complete)
9. DB pool inside `DbHandle`: writer thread + 2 WAL readers; migrate remaining lock sites
10. sqlite-vec swap behind `VectorStore` (per D4 gate outcome); remove qdrant-edge if GO
11. tokio feature slim + 2-thread runtime (row 12)
12. Landlock + seccomp sandbox for pdf/zip parsing (Linux, feature-flagged)
13. CI: AppImage build + smoke-test job
