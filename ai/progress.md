# progress.md — active items

> Exactly five active items at any time, per `rule.md`. Items move here from `plan.md` (Rule 2)
> and leave for `completed.md` only after verification (Rule 5).

## Active items

1. Row 1: empty webhook secret → hard reject outside demo mode
2. Row 5: replace `query_row(...).ok()` with `.optional()?` (3 sites in webhook.rs)
3. Row 6: move webhook DDL to a numbered migration; stop ensuring schema per-insert
4. Row 13: encrypted secrets envelope (`set_secret`/`get_secret` via `core::crypto`) + `webhook_secret` boot migration
5. Row 4a: byte caps — file import cap, zip declared-size check + `Read::take`, PDF input cap
