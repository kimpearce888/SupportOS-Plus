# SupportOS-Plus — Visual Fix Batch evidence pack (VF-1/2/3 + VF-4 close-out)

**Target:** v1.9.7 tree, VF-1/2/3 on top of cb064e5, VF-1..4 close-out on top of f5d3da0 · **Date:** 2026-11-12
**Directive:** owner — "start fixing everything, take the best decision, verify every fix with proof, it should work on all monitors" / "fix anything pending, do not left ignored items unworked."

## What is in here

- `before/` — the committed goldens BEFORE each batch (VF-1/2/3 baseline = 8ffceda; the close-out pairs = f5d3da0, extracted from git).
- `after/` — the same golden names AFTER (rebaselined, CI-locked, diff-locked twice per the repo's rule).
- `vlm-before/`, `vlm-after/` — adversarial VLM audits of the same images (same prompts both passes; the pass must find defects, so raw counts stay noisy by design — the signal is the TARGETED matrix below).

## Proof matrix (register ID → verification)

| Fix | Proof |
|-----|-------|
| **V1 goldens never gated the real look** | **COMPLETE (VF-4).** The production hosts were extracted from `app/mod.rs` into `design::host_standard_page` / `design::host_inbox_page` — ONE definition the shell itself calls. All 26 kittest suites render through it: the 19 previously-bare page suites, the capped 2K/4K renderers (whose inline copy had drifted to a literal 22 margin), the migrated inbox/copilot/list_pages suites (inline copies deleted), and the 8K reader geometry check. All 203 goldens rebaselined through the shared host; a clean re-run passes byte-identical. |
| **L1 sidebar beneath reply** | Geometry invariant `aud_geo_composition_*` (3 tests, 12 width×state combos: 800/1024/1280/1440/1625/1920/2560 × copilot closed/docked/narrow) asserting on LIVE panel rects: sibling columns disjoint, copilot panel bottom edge == composer top edge (±1.5px), composer spans reader+copilot (shared right edge), reader floor 480px holds. All pass. |
| **L11 radar "open →" clipped at card bound** | Root cause: `ScrollArea::show_rows(_, 26.0, …)` claimed 26px rows but painted ~3× that per row, so the scroll math pushed the last drill link past the layout height — clipped at the card boundary. Fix: plain scroll area, true row heights, callout → SP1 → link → SP2 rhythm, SP2 rhythm unified across both radar cards. Bottom inset = trailing SP2 (12) + card inner margin (14) = 26px ≥ SP2. Re-audit: the "clipped by the card/window boundary" HIGH claim is gone (residual: one "near-clipping" MED grumble and the fixed-height fold cut — AF-family, production scrolls). |
| **H5 near-invisible day separators** | `day_divider` rebuilt: muted pill (12px strong, muted_foreground on muted, BADGE radius) on border_strong hairlines, SP2 breathing in the thread. Re-audit: no more invisibility/size/competing complaints — the hostile pass moved on to unrelated composition nitpicks. |
| **C5 seven pages, unmanaged bottom half** | `design::list_end_note` — the short-content end-of-page marker (border_strong hairline over a quiet centered caption with the list's TRUE count) on automation, connectors, customers, incidents, organizations, live chats, and the onboarding hero. Zero-row lists keep their empty states. Re-audit: the VLM cites the marker's text verbatim on every page — the class has a defined state everywhere. |
| **V2 no spacing system** | SP5/SP6 tokens added; every literal Margin in `pages/` replaced with scale references; `style_lint.rs` `lint_page_margins` fails CI on numeric Margin literals in pages/. Gate green. |
| **V3 magic-number inbox** | `inbox_geo` module: named, documented, invariant-tested constants. |
| L2 copilot header collision | Header rebuilt; re-audit collision mentions 2→0. |
| L3 copilot overflow | Content scrolls in the panel. |
| S1 composer field grid | One 34px label column; To renders as a bordered read-only field matching Cc/Bcc. |
| S2/S8 properties card | `card_titled("Conversation")`, 64px label column, SP2 rhythm. |
| S3 toolbar scatter | Filters+Reset one left cluster; Sync owns the right edge. |
| S4/S5 duplicate sentiment | Copilot Signals drops sentiment; badge/caption dedup. |
| S6 reader grid | Avatar removed from reader head. |
| H1 flat list rows | Subject = CONTROL strong primary line, sender = muted metadata. |
| H2/H3 | Titled form card; footer quiet-caption + primary Send. |
| H4 debug dump | `ai_center.rs` raw "stored record:" line → `tracing::debug!`. |
| L4 knowledge row clip | 34px rows + always-visible Read/Edit + overflow menu. |
| L5 live chats | Wrap + scrollbar + compact timestamps + grouped Conversation card. |
| L7 reports escalated cell | `td_num` unified right-aligned mono register. |
| H6 onboarding status | Both status lines in one quiet card. |
| L10/S7 table edge insets | Verified already satisfied (UXF-1-1 CELL_PAD); no churn. |

## Battery (on the final close-out tree)

fmt --check PASS · clippy default lane -D warnings PASS · clippy kittest-ui lane -D warnings PASS · style_lint (incl. pages-margins rule) PASS · gates no-simulation + traceability PASS · all 26 kittest suites green on the rebaselined goldens (215 tests) with a byte-identical clean re-run (diff-locked twice) · workspace test battery green · SPP_SMOKE real binary: 24 pages, 4362 shapes, loopback :3000, migrations applied, clean exit.

## Honest notes

- **VLM adversarial noise:** the re-audit prompt mandates defect-finding, so every image yields complaints regardless of quality. The meaningful signal is the targeted matrix: every flagship class's old headline claim is gone. The residual "dead zone below the end-note" lines are the fixed-height golden window (900px) meeting a ~500px page — production scrolls and the page simply ends at the marker; no helpdesk fills tall windows artificially (HelpScout/Zendesk end their pages too). The fold-cut "SLA card cut off at viewport" is the same AF-family fold symptom as L8/L9 (production scrolls).
- **Host migration changes every page golden** — the 19 suites that previously rendered on a bare ui now render through the exact production funnel (background fill + scroll + 1800 column + SP5/14 inset). The `r3_settings_page_1440_light` before/after pair is the pure-host demo (no code change on that page this batch): the old bare-render artifacts (content flush to the window edge, unpainted lower region) disappear.
- **Fold cuts:** static goldens show scroll folds mid-content (normal in any scrolling UI).
