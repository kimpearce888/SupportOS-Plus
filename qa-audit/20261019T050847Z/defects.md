# Defects — QA Audit 20261019T050847Z

Reproducible defects found by the audit, each with evidence references and
qualified analysis. Evidence paths are relative to `evidence/`.

---

## DEFECT-01 [MAJOR] — Sidebar click hitboxes compressed on the four shortest-label rows

**Summary.** Four of the 21 sidebar navigation rows — **Inbox, Reports, Search,
Shell**, exactly the four labels with ≤7 characters — ignore mouse clicks
placed on the visible label text (the region any real user clicks). Clicks
only register in a narrow strip at the far-left edge of the row (x ≤ ~20 px).
Hover feedback paints normally across the whole label area on the same rows,
so the paint/hover rect and the click-acceptance rect are decoupled.

**Reproduction (all in 1280×800 Xvfb sessions, window 1180×760 at (0,0)).**

1. `artifacts/nav-sweep-A.json` + `screenshots/A01..A21`: sequential clicks at
   row centers (x≈30, y = 63…523) navigated **17/21** pages (page change
   verified by pixel diff >0.9%). The four dead rows produced screenshots
   **pixel-identical** to the previous page (diff 0.05% ≈ clock animation):
   - `A02-nav-Inbox` ≡ `A01-nav-Dashboard` (stayed on Dashboard)
   - `A07-nav-Reports` ≡ `A06-nav-Knowledge` (stayed on Knowledge)
   - `A15-nav-Search` ≡ `A14-nav-Notifications` (stayed on Notifications)
   - `A21-nav-Shell` ≡ `A20-nav-Onboarding` (stayed on Onboarding)
2. `artifacts/hitbox-scan.json` (139-probe scan, x=40): no y position in or
   near the four rows' visual spans ever selects them, while every other row
   selects at its center.
3. `artifacts/B1-xscan-chunk*.json`: hovering the **same dead rows** at
   x=10…58 produces the normal row-highlight paint (hover sensing is alive
   there) — the highlight rect and the click rect disagree.
4. `artifacts/K3-palette-real-input.json` `dead-row-xsweep` + screenshots
   `K3-60-dead-*-{10,45}.png`: at **x=10** all four rows navigate correctly
   (content diff 1.3–3.6%, sidebar selection moves to the clicked row —
   verified row-by-row in the follow-up analysis); at **x=45** all four are
   ignored (diff 0.0).

**Impact.** A user clicking the visible label of Inbox / Reports / Search /
Shell gets no navigation and no error — the app appears to ignore input.
Those four pages look unreachable by mouse. **Workaround:** click the far
left edge of the row (x ≤ ~20); with that, all 21 pages are reachable (the
audit reached every page this way). Label lengths of the working set are all
≥8 characters, dead set all ≤7 — the split is exact (4 shortest of 21),
which is hard to explain by chance (1/5985) and points at text-layout-driven
hit-geometry.

**Qualified analysis.** All rows are rendered by one uniform loop,
`if ui.selectable_label(self.page == key, key.label()).clicked()`
(`crates/app-shell/src/app.rs`, `shell_ui`, ~line 278; inventory `C-APP-002`).
There is no per-row code difference. Hover highlight, paint, and selection
state all work on the dead rows; only click acceptance is restricted to a
left sliver. The click-vs-hover rect decoupling, confined to the shortest
labels, suggests an egui 0.37 hit-testing / text-galley geometry anomaly
for short strings rather than an application-logic error. The audit did not
root-cause the toolkit internals (out of scope for a black-box audit); the
behavior is precisely characterized and independently reproduced across
three sessions (nav-sweep A, B1 scans, K3).

**Severity.** MAJOR (navigation defect on 4 of 21 pages for typical mouse
use), not blocking (workaround exists; all pages reachable).

---

## DEFECT-02 [MINOR] — Command palette: Enter never confirms a hit (web-port artifact)

**Summary.** In `crates/app-shell/src/palette.rs` (`ui`, line ~210):

```rust
let take = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
```

`Enter` only resolves the selected hit when the query text edit reports
`lost_focus()` in the same frame — a pattern ported from the web (blur-then-
Enter). In egui, a single-line `TextEdit` does not blur on Enter, so the
keyboard path to confirm a palette hit appears to never fire. The mouse path
(clicking a hit row, line ~224) is unaffected.

**Evidence.** Code read on the audit target (verified in the working tree at
commit 19dfb35). GUI confirmation was attempted (K3: typed a query, pressed
Enter with the input focused — `enter_diff` 0.06% = caret blink only, no
navigation), but a decisive GUI reproduction requires at least one palette
**hit** to exist; fresh profiles have no seeded conversations/customers/knowledge,
so every query returns `(no hits)` and there is nothing to confirm. Classified
MINOR: functional mouse path exists; the keyboard path is dead code as read.

**Impact.** Keyboard-only users cannot confirm palette selections.

---

## DEFECT-03 [MINOR] — Command palette input does not take focus on open

**Summary.** `palette.open()` (palette.rs:136) opens the overlay but never
requests keyboard focus for the query field. Until the user clicks inside the
input, typed keys go nowhere. The web counterpart auto-focuses the field
(standard palette UX).

**Evidence.** K1 session: palette opened via button; screenshot immediately
after shows no caret; typing "inbox" produced a 0-pixel change until the
input was explicitly clicked (K3 with the true input rect, x310–597 ×
y115–146: caret appeared, typed glyphs rendered — `K3-20-input-clicked`,
`K3-30-typed`).

**Impact.** Two-step interaction where one step would do; combined with
DEFECT-01's sidebar issues it makes keyboard navigation feel broken.

---

## Observations (not defects)

- **Palette input renders half-width** (x310–597 inside a 576-px window):
  `text_edit_singleline` sizes to content, not available width — egui
  default behavior, cosmetic.
- **Keyboard input requires explicit X focus in this audit environment**
  (bare Xvfb, no window manager sets input focus). After `XSetInputFocus`
  the app receives keys normally (K1 Escape; K3 typing). Environment
  characteristic, not an app defect — real desktops always have a WM.
- **`Sandbox janitor` session kills**: long-running foreground tool calls
  are terminated at unpredictable ages (5 s…180 s observed; `D-killer-probe`,
  `D-rec-rename-test`, `D-xa-rename-test` evidence). This constrained GUI
  automation to short sessions; see `coverage-gaps.md`.
