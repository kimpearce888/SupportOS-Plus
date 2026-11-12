# SupportOS-Plus — Visual Defect Register v1 (Phase 0 baseline)

**Target:** v1.9.7 @ cb064e5 (post-remediation tree) · **Date:** 2026-11-12
**Method:** adversarial VLM pass over the 37 committed CI-verified goldens (13 inbox shots across 800/1024/1440/1625/1920/2560 + 24 page/tab/hero shots at 1440), then code-verified triage of every headline finding. Artifact families (golden-context-only symptoms) were separated BEFORE counting — see end.

**Purpose:** the v1.9.6/v1.9.7 audits gated function, not look. This register is the first counted visual baseline. Every finding carries a checkable acceptance criterion. IDs are stable (class + number) for the remediation ledger.

---

## Class V — verification-system findings (the root enablers)

| ID | Sev | Finding | Evidence | Acceptance criterion |
|----|-----|---------|----------|----------------------|
| V1 | HIGH | Golden UI suites render pages **outside the production host**: tests call `page.ui(ui, db)` on a bare ui, while production hosts every standard page inside `CentralPanel → ScrollArea → content_column(1800) → Frame(inner_margin symmetric(22,14))` (`app/mod.rs:904-924`). "Zero golden drift" certifies a synthetic composition; the real app's visual state was never gated. This is the structural reason past sessions could report "all fine" while the product looks wrong. | kittest_*.rs `build_ui_state` vs `app/mod.rs` | Goldens render through the exact production host wrapper (shared helper); a fidelity assertion pins the golden title x-offset to the production 22px inset. |
| V2 | HIGH | Spacing is not enforced end-to-end. A token scale exists (`design::scale::space` SP1–4 = 4/8/12/16) but pages bypass it with hand-rolled Margin literals (inbox: 6 distinct literals 10/12/8/22; host 22/14; cards same(10)). Consistency exists only by coincidence. | `design/mod.rs:192-195`, literal census | All layout insets/margins reference the token scale; `tests/style_lint.rs` gains a rule failing CI on literal `Margin` values in `pages/`. |
| V3 | MED | Inbox composition is five stacked generations of magic-number patches with no spec: comment trail UX-8 → UX-27 → UXE-1-4 → UXE-1-7 → UXG-1-2; the composer height is a feedback hack (`composer_panel_h` = last-frame measure + `18.0` guess, `348.0` fallback, clamp `120..win-12`); column geometry is bare constants (168/300/480/0.30/280..520). | `inbox.rs:1152-1299` | Panel geometry derives from the Phase-1 composition spec with named constants; invariant tests (panel-rect non-intersection, reader floor, composer alignment) replace the height-tracking hack. |

## Class L — layout defects

| ID | Sev | Finding | Acceptance criterion |
|----|-----|---------|----------------------|
| L1 | HIGH | **Inbox composer/copilot composition** (the owner's complaint): the right copilot panel spans the full window height; the composer strip is confined to the reader column — the sidebar column runs beneath/beside the composer instead of the composer spanning beneath both columns (HelpScout reference composition). `inbox.rs:1217-1251` panel allocation order; visible in `r5_inbox_copilot_docked_1920`. | At 1920 + copilot open: copilot.bottom == composer.top; composer spans reader+copilot width; kittest rect assertion enforces it. |
| L2 | HIGH | Copilot panel header collision: title overlaps the Local button and the X close button (1625 + 1920 goldens). | No intersecting widget rects in the copilot header region (kittest assertion). |
| L3 | HIGH | Copilot suggested-reply card overflows its panel; the closing footnote is clipped mid-sentence at the panel bottom (1625/1920). | Copilot content fits or scrolls; zero text clipped at panel bounds. |
| L4 | HIGH | Knowledge page: row action buttons wrap to a second line and collide with the row divider; the Delete button is clipped mid-height. | Action cluster stays single-line (overflow collapses to menu); no rect intersects the divider. |
| L5 | HIGH | Live chats: customer message bubble clipped mid-line; transcript shows one cut-off message and reads broken. | Transcript items fully visible inside the scroll; no mid-line clip. |
| L6 | MED | Notifications: body text truncated mid-sentence with no ellipsis or expand affordance; last card clipped at the fold. | Truncation shows ellipsis + expandable detail; fold never cuts mid-card. |
| L7 | MED | Reports insights: the "escalated" cell renders two colliding values in one cell (`reports.rs:1399-1433`). | One value per cell, or an explicit dual-metric layout with divider. |
| L8 | MED | Dashboard: By agent / By team cards cut mid-row at the window fold. | No card ever cut mid-row at any window height (cards size to content / grid reflows). |
| L9 | MED | Settings: last toggle row cut at the window bottom. | Same invariant as L8 (visible last row + bottom padding). |
| L10 | MED | Diagnostics: AI column crushed against the table's right border, values nearly touching the edge. | Min SP2 cell padding on both edges of every column. |
| L11 | MED | Issue radar: last "open →" link flush against / clipped by the card bottom edge. | Card content inset ≥ SP2 from card bounds. |

## Class S — spacing / alignment

| ID | Sev | Finding | Acceptance criterion |
|----|-----|---------|----------------------|
| S1 | HIGH | Inbox composer field grid misaligned: To label/value baselines differ from Cc/Bcc bordered inputs; inputs extend past the reply textarea edge; three rows, three treatments (1440/1920). | Composer fields share one column grid (equal label column, equal input x-extent) — kittest rect-grid assertion. |
| S2 | HIGH | Inbox properties card: inconsistent row rhythm; State row uses a different pill style than Status/Priority; Snooze row flush against the card bottom. | Uniform SP2 inter-row rhythm; one pill treatment per card; bottom inset ≥ SP2. |
| S3 | HIGH | Inbox list toolbar scatter: Filters/Reset/Sync-now float with dead gaps; search row mixes control widths and a heavy black Apply beside ghost controls. | Grouped control clusters with consistent gaps; search-row controls share height + weight tiering. |
| S4 | MED | Copilot Signals card: three different treatments for peer metadata (filled chip vs plain colored text vs chip). | One chip style per metadata class. |
| S5 | MED | Sentiment rendered twice with different treatments (floating inline + Signals card). | Single placement for per-conversation sentiment. |
| S6 | MED | Reading pane: conversation title block x-offset differs from the card below (no shared grid); properties card spans ~2/3 width leaving a ragged right edge at 2560. | One column grid for the reading pane; width rules consistent. |
| S7 | MED | Reports SLA: rightmost column values hard against the table edge. | Min SP2 inset from table edges. |
| S8 | MED | Inbox status card: pill-group gaps vary; the State row stretches near full-width while others hug left. | One gap rhythm + alignment rule for property rows. |

## Class H — hierarchy / typography

| ID | Sev | Finding | Acceptance criterion |
|----|-----|---------|----------------------|
| H1 | HIGH | Inbox conversation-list rows: sender, timestamp, subject, assignee, status all near-equal weight — the subject doesn't dominate. This is the core "unprofessional inbox" read (all viewports). | Three-tier row hierarchy (subject primary / sender+time metadata / status+assignee tertiary), locked by kittest style assertions. |
| H2 | MED | Inbox state control: eight lowercase segmented buttons across four rows with no grouping headers — reads as a debug panel. | Grouped under labeled sections or one consolidated control. |
| H3 | MED | Composer footer: tiny grey helper text beside a heavy black Send button — competing weights at row ends. | Quiet-caption tier + primary action tier with consistent sizes. |
| H4 | HIGH | AI Center renders a raw debug record into the product UI: "stored record: provider_kind=none chat_model=(unset)…" (`ai_center.rs:1089`). | No raw diagnostic strings in product UI (moved to diagnostics). |
| H5 | LOW | Timeline date separators: near-invisible hairline + tiny text competing with message content. | Muted centered caption with adequate contrast/size. |
| H6 | LOW | Onboarding status lines flush against the bottom region, ungrouped. | Status block in a card with SP2 padding. |

## Class C — composition / density

| ID | Sev | Finding | Acceptance criterion |
|----|-----|---------|----------------------|
| C1 | HIGH | Inbox reading pane: ~250px dead zone between content and composer — crushed-vs-empty imbalance on one screen (1920). | Thread column stretches or shows a centered empty-state; no raw dead gap. |
| C2 | MED | Inbox list column half-empty below the rows with no empty-state. | Empty region shows an empty-state treatment. |
| C3 | MED | Views rail: tall empty column below the last item. | Bounded rail box / empty-state, not an open-ended strip. |
| C4 | MED | Inbox header: Copilot button + status dot orphaned near the window edge with nothing beneath. | Header controls form a grouped cluster. |
| C5 | MED | Seven pages render content in a top strip and leave the bottom half unmanaged (automation, connectors, customers, incidents, organizations, live_chats, hero) — in production this manifests as a background void with no empty-state. | Each page defines its short-content/empty state. |
| C6 | MED | Inbox empty transcript: lone date separator, zero messages — reads unfinished (seeded state + first-run real state). | Empty-transcript placeholder treatment. |
| C7 | LOW | Inbox header: Closed chip + spinner cluster cramped/misaligned with the title baseline. | Baseline-aligned cluster with SP2 gaps. |

---

## Counts

| Class | High | Med | Low | Total |
|-------|------|-----|-----|-------|
| V (system) | 2 | 1 | 0 | 3 |
| L (layout) | 6 | 5 | 0 | 11 |
| S (spacing) | 3 | 5 | 0 | 8 |
| H (hierarchy) | 2 | 2 | 2 | 6 |
| C (composition) | 1 | 5 | 1 | 7 |
| **Total** | **14** | **18** | **3** | **35** |

## Artifact families — excluded from product counts (honesty ledger)

These appear in the goldens but are **golden-context symptoms of V1**, not product defects; they were excluded before counting:
- **AF1** Titles clipped at window edges (8 pages): goldens render pages without the production 22px/14px host inset — production does inset. Feed V1.
- **AF2** Black bottom-half voids (7 pages): unpainted harness canvas beyond content — production paints the panel background. The real-app symptom (unmanaged empty region) is registered once as C5.
- **AF3** Mouse-cursor overlaps in a few goldens (outreach, SLA tab): harness cursor rendering; excluded.

## Next step

Phase 1 (owner gate): the one-page layout spec — spacing scale + panel inset rules + inbox composition wireframe — for sign-off BEFORE any fix code. Then L-class fixes with invariant tests, then tokenization (V2) with the style-lint gate, then page passes tier by tier.
