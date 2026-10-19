# SupportOS+ shadcn/ui Redesign — Visual Verification Record

Branch: `redesign/shadcn-ui` · Scope: presentation layer of
`crates/app-shell` only (no core/catalog/xtask/workflow changes).

## What was built

- **`design.rs`** — centralized shadcn/ui-inspired design system:
  zinc light + dark semantic tokens (WCAG-AA contrast verified by unit
  test), full `egui::Visuals` for **both** themes installed per context,
  modern type scale (Heading 22 / Body·Button 14 / Mono 13 / Small 12),
  and reusable components: buttons (Primary/Secondary/Outline/Ghost/
  Destructive, uniform 32 px control height, hover/press tonal overlay,
  keyboard focus ring), icon buttons, badges, status tones, filter chips,
  cards, titled cards, metric cards, page headers, striped tables,
  aligned form grids, empty states, hint/success/error text.
- **`icons.rs`** — a Lucide-style stroke icon family (42 glyphs) painted
  with the egui painter on a 24-grid; no icon font or new dependency.
  `Icon::for_page` maps every page.
- **Redesigned shell** — full-height grouped sidebar (brand, ⌘K palette
  trigger, sectioned nav with icons, sun/moon theme switch), application
  header, page-level vertical scrolling for every page except the Inbox
  (whose list/detail panes scroll independently), restyled command
  palette, confirm dialogs and toasts.
- **All 21 pages** restyled through the same components; Inbox, Dashboard,
  Reports and Settings fully re-laid-out.

## Library evaluation (why no `egui-shadcn`)

`egui-shadcn` 0.3.0 (MIT) targets **egui 0.33**; this workspace pins
**egui/eframe 0.37** and downgrading is out of scope. The design system
is implemented natively instead, with **zero new dependencies**.

## Quality gates

| Gate | Result |
| --- | --- |
| `cargo fmt --all --check` | clean |
| `cargo clippy --workspace --all-targets -- -D warnings` | clean |
| `cargo test --workspace` | 320 passed, 0 failed (app-shell 146 · core 168 + 4 ignored · catalog 6), incl. new contrast/design smoke tests |

## Visual verification (screenshots/)

The real shell (dev binary, real SQLite store, seeded demo data) was run
headless under **Xvfb + llvmpipe software GL** with the off-by-default
`ui-screenshot` feature (eframe `glow` + `__screenshot`); 29 PNGs were
captured across pages, themes and window sizes, then reviewed by a
vision model with pixel-level zoom checks, and the findings were fixed
and re-verified:

- all 21 pages in light theme
- Dashboard/Inbox/Reports/Settings/AI Center in dark theme
- 900×620 narrow variants (Dashboard, Inbox) — pages now scroll
- command palette overlay
- each screenshot ~1180×760

Findings fixed during review: palette/dialog fade-in ghosting on open
(disabled window fade), chart annotations clipped at the frame edge,
long ISO range strings overflowing the chart card hint, inbox list-row
preview colliding with the timestamp, filter-row control misalignment
(uniform 32 px control height everywhere), operations tile cramping,
side-by-side duplicate hero empty states on Incidents, page content
unreachable below the fold (added whole-page scrolling).

## Honest limitations

- Verified headless (Xvfb/software GL) + vision-model review, not by a
  human on a GPU desktop; no live mouse/keyboard flows were recorded —
  interaction behavior is covered by the existing headless egui tests
  (palette focus/Enter, sidebar full-width hitboxes, model round-trips).
- The screenshots use seeded demo rows; real production data renders
  through the same widgets.
- Text rendering uses egui's embedded Ubuntu-Light/Hack fonts (no system
  font is assumed), matching previous releases.
