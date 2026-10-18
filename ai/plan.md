# SupportOs-Plus — TODO

> Remaining work items, executed five at a time under `rule.md`.
> Status flow: `plan.md` → `progress.md` → implementation → verification → `completed.md`.
> Planning context and decision ledger: `decisions.md`.

## Items

1. Row 16 remainder: workspace reqwest 0.12 → 0.13 (dedupe lockfile)
2. `crates/app-shell`: eframe shell, settings-table theme, flock single-instance (add `libc`)
3. `DbHandle` facade over `Arc<Mutex<Connection>>` (D3 seam)
4. `core::richtext`: HTML → styled spans, fixture tests
5. Background task bridge → `request_repaint`
6. Port: dashboard page
7. Port: inbox page (mention widget, richtext, live updates)
8. Port: customers page
9. Port: knowledge/docs pages
10. Port: reports page (egui_plot)
11. Port: operations page
12. Port: remaining 17 pages
13. Port: command palette, overlays, toasts
14. Event bus → repaint wiring across all ported pages
15. Cutover: delete axum router + Leptos + Tauri + wasm32 target + CSP plumbing
16. Create minimal loopback listener (webhook + OAuth callback only)
17. AppImage packaging via `xtask package`; drop deb/msi/nsis/dmg/app targets
18. Pin final toolchain; drop wasm32 target (row 15 complete)
19. DB pool inside `DbHandle`: writer thread + 2 WAL readers; migrate remaining lock sites
20. sqlite-vec swap behind `VectorStore` (per D4 gate outcome); remove qdrant-edge if GO
21. tokio feature slim + 2-thread runtime (row 12)
22. Landlock + seccomp sandbox for pdf/zip parsing (Linux, feature-flagged)
23. CI: AppImage build + smoke-test job
