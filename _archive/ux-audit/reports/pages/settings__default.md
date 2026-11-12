PAGE: Settings

1. CONTENT INVENTORY
- Page header "Settings" with subtitle "local configuration, persisted in the settings table".
- Top-right header row: "synced 3s ago" status chip, "Search ⌘K" button, help icon button.
- Horizontal tab bar: Sync & AI (active, underlined), Help Scout, LM Studio, Vector index, Business hours, Backups & export.
- Card "Synchronization" with five form rows: numeric inputs "max quiet sync interval (minutes):" = 1, "api concurrency:" = 2; dropdowns "log level" = info, "status after reply" = pending; text input "agent language" = en.
- Card "AI behaviour" with helper text "safe defaults — review before enabling write actions" and a list of checkboxes: AI enabled ✓, Automatic analysis ✓, Automatic internal notes ☐, Automatic draft replies ☐, Automation rules ☐, Automation write actions ☐, Attachment auto download ✓ (several rows have light gray hover/highlight backgrounds).
- Card "About & updates": "Running version: v1.9.4", "check for updates" button, footnote "The check reads the public GitHub release feed once, on click. The app never sends telemetry."
- Primary dark button "save settings" below all cards.

2. LAYOUT DEFECTS
- All three cards are stacked in a narrow left column (~460px max width); the Sync card is wider (~460) than AI behaviour (~285) and About (~530) — three different card widths in one column, no grid system. Massive dead space across the right two-thirds of the 1920px canvas.
- Card widths are inconsistent: Synchronization ends ~x=696, AI behaviour ~x=548, About ~x=765; left edges align but right edges don't — ragged, ungridded.
- Cards are jammed together vertically with no consistent rhythm (Synchronization ends ~458, AI behaviour starts ~478, gap ~20; About card starts ~845 with a smaller gap).
- Synchronization card: labels and inputs are on one cramped row but inputs are pushed to varying x positions (colons at differing distances from inputs); input column not aligned to a shared gutter — "1" and "2" inputs sit at ~462 while dropdowns/text sit at ~462 but with different widths (dropdown 160px, text 220px, stepper 36px) — ragged right edges inside the card.
- Numeric inputs ("1", "2") are ~36px wide — far too narrow, no visible stepper or unit hint, and they're not vertically centered against their labels.
- Checkbox rows in AI behaviour have alternating background tints (rows 2, 5, 6, 7 appear highlighted) — looks like stale hover states baked in, visually noisy and unexplained.
- Content doesn't use available width: at 1920px, ~65% of the main area is empty gray.
- "check for updates" button overlaps/sits tight against the footnote text; spacing between button and footnote is tight (~10px) and the footnote nearly touches the card's bottom edge.
- Tab bar active underline is thin and close to the label; inactive tab labels sit slightly higher than the active label baseline (active tab appears bold/larger, causing vertical jitter).

3. VISUAL/TYPOGRAPHY DEFECTS
- Inconsistent casing throughout: "Settings" (title case) vs "save settings", "check for updates" (lowercase) vs card titles "Synchronization", "AI behaviour" — and form labels all lowercase ("max quiet sync interval (minutes):").
- Trailing colons on labels ("api concurrency:") is a non-idiomatic pattern; mixed with no-colon helper text.
- Subtitle "local configuration, persisted in the settings table" exposes internal implementation ("settings table") to end users.
- Card helper "safe defaults — review before enabling write actions" is small, low-contrast, and uses an em dash construction inconsistently with other microcopy.
- Weak hierarchy: card titles, labels, and helper text are all close in size/weight; only the page title stands out.
- Checkbox label capitalization mixed: "AI enabled" / "Attachment auto download" (sentence case) vs "Automatic analysis" (title case) — actually all are inconsistent: "Automatic internal notes" vs "api concurrency" style.
- British vs American spelling unverified but "behaviour" vs typical product English — fine but note consistent terminology elsewhere ("Automatic" prefix vs "Automation" prefix rows could confuse).
- "synced 3s ago" chip is green-on-light-green — likely fails contrast at that size.

4. COMPONENT DEFECTS
- Checkboxes rendered as text-glyph checkmarks (✓) next to labels rather than consistent checkbox components; checked/unchecked visual difference is subtle for unchecked rows.
- Rows in AI behaviour appear hoverable (gray backgrounds) but it's unclear if the whole row is the click target or only the tiny checkbox — inconsistent affordance.
- "check for updates" button styled as a secondary/outline button while "save settings" is primary dark — but "check for updates" is the only action in its card and its hierarchy is unclear.
- Numeric inputs have no spinners, no min/max, no validation hints; "api concurrency: 2" gives no range guidance.
- Dropdown chevrons are solid filled triangles (▲ style) — heavy, inconsistent with the otherwise light iconography.
- No visible disabled/loading states on "save settings"; unclear whether changes apply instantly or require save (checkboxes suggest instant, but a Save button exists — contradictory model).
- "save settings" button is small; no confirmation/feedback shown after saving.
- Help icon top-right is icon-only with no tooltip visible.

5. UX DEFECTS
- No per-field help text for technical settings (api concurrency, max quiet sync interval) — users can't know safe ranges.
- "agent language" as free-text input ("en") instead of a dropdown — typo-prone, no validation shown.
- Ambiguous save model: some controls look immediately active while a global "save settings" sits far below the fold of the card stack — risk of losing changes.
- No unsaved-changes indicator or dirty-state feedback.
- "status after reply: pending" — no explanation of what values mean.
- "check for updates" result state unknown — no last-checked timestamp.
- Six tabs but no indication of which contain unsaved changes or errors.
- Version info duplicated (sidebar "v1.9.4" and card "Running version: v1.9.4") — minor redundancy, fine, but no changelog link.

6. ACCESSIBILITY
- "synced 3s ago" chip: light green text on light green background — probable WCAG AA contrast failure.
- Helper/footnote texts (subtitle, "safe defaults…", telemetry footnote) are small, gray, likely below 4.5:1.
- Checkbox hit targets are tiny (~16px) with no enlarged row target guaranteed; ✓ glyph may not be announced as checked state by screen readers if it's a text glyph, not a real checkbox role.
- Icon-only help button (top right) has no accessible label.
- Lowercase-only button labels ("save settings") read awkwardly with screen readers and reduce scannability.
- Tab bar must be keyboard-operable with proper tablist roles; underline-only active indication is a low-contrast non-color-dependent cue (good) but thin (~2px).

7. VERDICT
- [MAJOR] Massive unused width / ungridded card layout with three inconsistent card widths.
- [MAJOR] Ambiguous save model — inline-looking controls plus a distant global Save, no dirty-state feedback.
- [MAJOR] Checkboxes rendered inconsistently (glyph + hover-tinted rows) with unclear click targets.
- [MAJOR] Low-contrast status chip and helper texts (a11y risk).
- [MINOR] Inconsistent text casing across titles, labels, and buttons.
- [MINOR] Narrow numeric inputs without range guidance or steppers.
- [MINOR] "agent language" free-text field should be a select with validation.
- [MINOR] Implementation-leaking subtitle ("settings table").
- [MINOR] Card vertical spacing rhythm uneven; footnote tight against card edge.
- [MINOR] Icon-only help button without label/tooltip.
- [POLISH] Filled-triangle dropdown chevrons inconsistent with light icon style.
- [POLISH] Active-tab label size change causes slight vertical jitter.
- [POLISH] Duplicate version display with no changelog link.