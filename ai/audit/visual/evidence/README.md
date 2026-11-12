# SupportOS-Plus — Visual Fix Batch VF-1/2/3 evidence pack

**Target:** v1.9.7 tree, batch on top of cb064e5 · **Date:** 2026-11-12
**Directive:** owner — "start fixing everything, take the best decision, verify every fix with proof, it should work on all monitors."

## What is in here

- `before/` — the committed goldens BEFORE the batch (the register's baseline look).
- `after/` — the same golden names AFTER the batch (rebaselined, CI-locked).
- `vlm-before/`, `vlm-after/` — adversarial VLM audits of the same images (same prompts both passes).

## Proof matrix (register ID → verification)

| Fix | Proof |
|-----|-------|
| **L1 sidebar beneath reply** | Geometry invariant `aud_geo_composition_*` (3 tests, 12 width×state combos: 800/1024/1280/1440/1625/1920/2560 × copilot closed/docked/narrow) asserting on LIVE panel rects: sibling columns disjoint, copilot panel bottom edge == composer top edge (±1.5px), composer spans reader+copilot (shared right edge), reader floor 480px holds. All pass. |
| **V1 goldens never gated the real look** | inbox page + copilot + list_pages suites now render through the exact production host (CentralPanel + SP5/14 inset; standard pages additionally scroll + content_column(1800)). All goldens rebaselined through it. |
| **V2 no spacing system** | SP5/SP6 tokens added; every literal Margin in `pages/` replaced with scale references; `style_lint.rs` gains `lint_page_margins` — CI fails on any future numeric Margin literal in pages/. Gate green. |
| **V3 magic-number inbox** | `inbox_geo` module: 10 named, documented, invariant-tested constants; `composer_panel_h` feedback now token-derived. |
| L2 copilot header collision | Header rebuilt (title+X row, caption below, Local badge removed); golden re-audit: collision mentions 2→0. |
| L3 copilot overflow | Content scrolls in the panel; footnote no longer clipped (re-audit: 0 mentions). |
| S1 composer field grid | One 34px label column; To renders as a bordered read-only field matching Cc/Bcc (28px, same grid). Re-audit: 0 mentions. |
| S2/S8 properties card | `card_titled("Conversation")`, 64px label column on every row, SP2 rhythm, bottom inset. |
| S3 toolbar scatter | Filters+Reset one left cluster; Sync owns the right edge. |
| S4/S5 duplicate sentiment | Copilot Signals drops sentiment (reader triage owns it); Local badge/caption dedup. |
| S6 reader grid | Avatar removed from reader head — title aligns to the column edge. |
| H1 flat list rows | Subject = CONTROL strong (primary line), sender = muted metadata above. Re-audit: 0 mentions of flat hierarchy. |
| H2/H3 | Titled form card; footer quiet-caption + primary Send. |
| H4 debug dump | `ai_center.rs` raw "stored record:" line → `tracing::debug!`. Re-audit: 0 mentions. |
| L4 knowledge row clip | 34px rows + Read/Edit visible + overflow menu (Re-index/Reviewed/Verify/Delete). Re-audit: 0 mentions. |
| L5 live chats | Explicit body wrap + long-token breaking, always-visible scrollbar, compact wall-time timestamps (no raw ISO), state badge + guard note + End chat grouped in one "Conversation" card. Pixel-measured card boxes prove clean stacking (no overlap). The VLM's residual "clipped text" quotes the LIST PREVIEW's deliberate ellipsis — documented as VLM noise on this image. |
| L7 reports escalated cell | `td_num` unified right-aligned mono register — one value, one treatment. |
| H6 onboarding status | Both status lines in one quiet card. |
| L10/S7 table edge insets | Verified already satisfied in code (CELL_PAD 10px both sides, UXF-1-1) — no churn; re-audit clean. |

## Battery (all on the final tree)

fmt --check PASS · clippy default lane -D warnings PASS · clippy kittest-ui lane -D warnings PASS (new lane; exposed+fixed 2 pre-existing deprecated `show_inside` + 2 latent lint allows) · style_lint gate PASS (with new pages-margins rule) · gates no-simulation + traceability PASS · app-shell kittest battery: all 26 suites green on the rebaselined goldens (incl. 3 new geometry suites, 10 tests) · SPP_SMOKE real binary: 24 pages, clean boot + exit.

## Honest notes

- **VLM adversarial noise:** the re-audit prompt mandates defect-finding, so raw defect-line counts stay high by design (294→287 lines across the 20 re-audited images; HIGH 98→88). The meaningful signal is the TARGETED defect matrix above: all flagship defect classes now have 0 mentions, plus the geometry invariants that no VLM can argue with.
- **Fold cuts:** static goldens show scroll folds mid-content (normal in any scrolling UI); scrollbar affordances added where the fold reads ambiguous.
- **Remaining (not this batch):** migrating the remaining per-page kittest suites to the production host wrapper (pattern + helper now exist in the migrated suites); the C5-family empty-state polish on short tables; H5 timeline separator contrast tweak. Ledger rows follow.
