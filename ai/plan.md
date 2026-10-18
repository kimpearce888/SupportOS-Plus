# SupportOs-Plus — TODO

> Remaining work items, executed five at a time under `rule.md`.
> Status flow: `plan.md` → `progress.md` → implementation → verification → `completed.md`.
> Planning context and decision ledger: `decisions.md`.

## Items

1. Characterization tests: `hybrid_search` (RRF fusion vectors)
2. Characterization tests: SLA edge cases (extend the existing suite)
3. Add `xtask vector-count` command; record chunk count (D4 gate)
4. Pin toolchain to exact version; pin egui `=0.37` in workspace (keep wasm32 until Phase 4)
5. Row 2: replace hand-rolled HMAC/SHA-1/base64/hex with `hmac` + `sha1` crates + `Mac::verify_slice`
6. Row 1: empty webhook secret → hard reject outside demo mode
7. Row 5: replace `query_row(...).ok()` with `.optional()?` (3 sites in webhook.rs)
8. Row 6: move webhook DDL to a numbered migration; stop ensuring schema per-insert
9. Row 13: encrypted secrets envelope (`set_secret`/`get_secret` via `core::crypto`) + `webhook_secret` boot migration
10. Row 4a: byte caps — file import cap, zip declared-size check + `Read::take`, PDF input cap
11. Row 16 remainder: workspace reqwest 0.12 → 0.13 (dedupe lockfile)
12. `crates/app-shell`: eframe shell, settings-table theme, flock single-instance (add `libc`)
13. `DbHandle` facade over `Arc<Mutex<Connection>>` (D3 seam)
14. `core::richtext`: HTML → styled spans, fixture tests
15. Background task bridge → `request_repaint`
16. Port: dashboard page
17. Port: inbox page (mention widget, richtext, live updates)
18. Port: customers page
19. Port: knowledge/docs pages
20. Port: reports page (egui_plot)
21. Port: operations page
22. Port: remaining 17 pages
23. Port: command palette, overlays, toasts
24. Event bus → repaint wiring across all ported pages
25. Cutover: delete axum router + Leptos + Tauri + wasm32 target + CSP plumbing
26. Create minimal loopback listener (webhook + OAuth callback only)
27. AppImage packaging via `xtask package`; drop deb/msi/nsis/dmg/app targets
28. Pin final toolchain; drop wasm32 target (row 15 complete)
29. DB pool inside `DbHandle`: writer thread + 2 WAL readers; migrate remaining lock sites
30. sqlite-vec swap behind `VectorStore` (per D4 gate outcome); remove qdrant-edge if GO
31. tokio feature slim + 2-thread runtime (row 12)
32. Landlock + seccomp sandbox for pdf/zip parsing (Linux, feature-flagged)
33. CI: AppImage build + smoke-test job
