# SupportOs-Plus — TODO

> Remaining work items, executed five at a time under `rule.md`.
> Status flow: `plan.md` → `progress.md` → implementation → verification → `completed.md`.
> Planning context and decision ledger: `decisions.md`.

## Items

1. Add `LICENSE` (MIT, matching manifests)
2. Add CI: fmt + clippy + test + cargo-deny (Linux; core/catalog/xtask scope)
3. Characterization tests: `webhook_handler` (accept / duplicate / signature-reject / bad-request)
4. Characterization tests: `backup_service` (backup, verify, restore, prune, secret-filtered sidecar)
5. Characterization tests: `encrypted_sync` (export/verify round-trip, wrong passphrase, tamper, import)
6. Characterization tests: `hybrid_search` (RRF fusion vectors)
7. Characterization tests: SLA edge cases (extend the existing suite)
8. Add `xtask vector-count` command; record chunk count (D4 gate)
9. Pin toolchain to exact version; pin egui `=0.37` in workspace (keep wasm32 until Phase 4)
10. Row 2: replace hand-rolled HMAC/SHA-1/base64/hex with `hmac` + `sha1` crates + `Mac::verify_slice`
11. Row 1: empty webhook secret → hard reject outside demo mode
12. Row 5: replace `query_row(...).ok()` with `.optional()?` (3 sites in webhook.rs)
13. Row 6: move webhook DDL to a numbered migration; stop ensuring schema per-insert
14. Row 13: encrypted secrets envelope (`set_secret`/`get_secret` via `core::crypto`) + `webhook_secret` boot migration
15. Row 4a: byte caps — file import cap, zip declared-size check + `Read::take`, PDF input cap
16. Row 16 remainder: workspace reqwest 0.12 → 0.13 (dedupe lockfile)
17. `crates/app-shell`: eframe shell, settings-table theme, flock single-instance (add `libc`)
18. `DbHandle` facade over `Arc<Mutex<Connection>>` (D3 seam)
19. `core::richtext`: HTML → styled spans, fixture tests
20. Background task bridge → `request_repaint`
21. Port: dashboard page
22. Port: inbox page (mention widget, richtext, live updates)
23. Port: customers page
24. Port: knowledge/docs pages
25. Port: reports page (egui_plot)
26. Port: operations page
27. Port: remaining 17 pages
28. Port: command palette, overlays, toasts
29. Event bus → repaint wiring across all ported pages
30. Cutover: delete axum router + Leptos + Tauri + wasm32 target + CSP plumbing
31. Create minimal loopback listener (webhook + OAuth callback only)
32. AppImage packaging via `xtask package`; drop deb/msi/nsis/dmg/app targets
33. Pin final toolchain; drop wasm32 target (row 15 complete)
34. DB pool inside `DbHandle`: writer thread + 2 WAL readers; migrate remaining lock sites
35. sqlite-vec swap behind `VectorStore` (per D4 gate outcome); remove qdrant-edge if GO
36. tokio feature slim + 2-thread runtime (row 12)
37. Landlock + seccomp sandbox for pdf/zip parsing (Linux, feature-flagged)
38. CI: AppImage build + smoke-test job
