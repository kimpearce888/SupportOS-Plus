# SupportOs-Plus — TODO

> Remaining work items, executed five at a time under `rule.md`.
> Status flow: `plan.md` → `progress.md` → implementation → verification → `completed.md`.
> Planning context and decision ledger: `decisions.md`.

## Items

1. Row 1: empty webhook secret → hard reject outside demo mode
2. Row 5: replace `query_row(...).ok()` with `.optional()?` (3 sites in webhook.rs)
3. Row 6: move webhook DDL to a numbered migration; stop ensuring schema per-insert
4. Row 13: encrypted secrets envelope (`set_secret`/`get_secret` via `core::crypto`) + `webhook_secret` boot migration
5. Row 4a: byte caps — file import cap, zip declared-size check + `Read::take`, PDF input cap
6. Row 16 remainder: workspace reqwest 0.12 → 0.13 (dedupe lockfile)
7. `crates/app-shell`: eframe shell, settings-table theme, flock single-instance (add `libc`)
8. `DbHandle` facade over `Arc<Mutex<Connection>>` (D3 seam)
9. `core::richtext`: HTML → styled spans, fixture tests
10. Background task bridge → `request_repaint`
11. Port: dashboard page
12. Port: inbox page (mention widget, richtext, live updates)
13. Port: customers page
14. Port: knowledge/docs pages
15. Port: reports page (egui_plot)
16. Port: operations page
17. Port: remaining 17 pages
18. Port: command palette, overlays, toasts
19. Event bus → repaint wiring across all ported pages
20. Cutover: delete axum router + Leptos + Tauri + wasm32 target + CSP plumbing
21. Create minimal loopback listener (webhook + OAuth callback only)
22. AppImage packaging via `xtask package`; drop deb/msi/nsis/dmg/app targets
23. Pin final toolchain; drop wasm32 target (row 15 complete)
24. DB pool inside `DbHandle`: writer thread + 2 WAL readers; migrate remaining lock sites
25. sqlite-vec swap behind `VectorStore` (per D4 gate outcome); remove qdrant-edge if GO
26. tokio feature slim + 2-thread runtime (row 12)
27. Landlock + seccomp sandbox for pdf/zip parsing (Linux, feature-flagged)
28. CI: AppImage build + smoke-test job
