# progress.md — active items

> Exactly five active items at any time, per `rule.md`. Items move here from `plan.md` (Rule 2)
> and leave for `completed.md` only after verification (Rule 5).

## Active items

1. Characterization tests: `hybrid_search` (RRF fusion vectors)
2. Characterization tests: SLA edge cases (extend the existing suite)
3. Add `xtask vector-count` command; record chunk count (D4 gate)
4. Pin toolchain to exact version; pin egui `=0.37` in workspace (keep wasm32 until Phase 4)
5. Row 2: replace hand-rolled HMAC/SHA-1/base64/hex with `hmac` + `sha1` crates + `Mac::verify_slice`
