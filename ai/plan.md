# SupportOs-Plus — TODO

> Remaining work items, executed five at a time under `rule.md`.
> Status flow: `plan.md` → `progress.md` → implementation → verification → `completed.md`.
> Planning context and decision ledger: `decisions.md`.

## Items

1. Port: dashboard page
2. Port: inbox page (mention widget, richtext, live updates)
3. Port: customers page
4. Port: knowledge/docs pages
5. Port: reports page (egui_plot)
6. Port: operations page
7. Port: remaining 17 pages
8. Port: command palette, overlays, toasts
9. Event bus → repaint wiring across all ported pages
10. Cutover: delete axum router + Leptos + Tauri + wasm32 target + CSP plumbing
11. Create minimal loopback listener (webhook + OAuth callback only)
12. AppImage packaging via `xtask package`; drop deb/msi/nsis/dmg/app targets
13. Pin final toolchain; drop wasm32 target (row 15 complete)
14. DB pool inside `DbHandle`: writer thread + 2 WAL readers; migrate remaining lock sites
15. sqlite-vec swap behind `VectorStore` (per D4 gate outcome); remove qdrant-edge if GO
16. tokio feature slim + 2-thread runtime (row 12)
17. Landlock + seccomp sandbox for pdf/zip parsing (Linux, feature-flagged)
18. CI: AppImage build + smoke-test job
