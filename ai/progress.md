# progress.md — active items

> Exactly ten active items at any time, per `rule.md` (batch size raised from five by the owner, 2026-10-23). Items move here from `plan.md` (Rule 2)
> and leave for `completed.md` only after verification (Rule 5).

## Active items

**Batch RS-4 (items RS-4-1..RS-4-10)** — active, moved from plan.md per Rule 2
(2026-10-30). Source: the owner's G4 rejection; root-cause reproduction and
disposition in the G4-verdict addendum (completed.md, 2026-10-30).

## Batch RS-4 — G4-rejection fix wave (owner-directed, 2026-10-30)


> Source: the owner's G4 rejection ("you need to fix the reskin…") + the
> root-cause reproduction recorded in the G4-verdict addendum (completed.md,
> 2026-10-30). Every item carries the evidence that opened it.

1. **RS-4-1 (core/richtext)** — add `pub fn strip_html(html:&str)->String`
   (block tags → spaces, entities decoded) and
   `pub fn break_long_tokens(s:&str,max:usize)->String` (U+200B into tokens
   longer than `max`, so egui can wrap unbreakable URLs); unit tests for
   both (empty, no-tags, nested tags, attribute-bearing tags, multi-break).
2. **RS-4-2 (core/ai_local)** — the copilot analyzer must consume
   `strip_html`'d thread text (summary, questions, sentiment/intent inputs);
   unit test: a `<br><br>`-laden customer body yields clean summary lines
   with zero markup tokens (the live-data leak the owner screenshot shows).
3. **RS-4-3 (app-shell/inbox)** — `render_rich_spans`: span text (incl. link
   labels) passes through `break_long_tokens`, so no thread-body token can
   exceed the reader viewport (root cause of the cards painting over the
   copilot panel; evidence: probe zz_probe_inbox_1625_copilot_open).
4. **RS-4-4 (app-shell/inbox)** — `copilot_pane_ui`: summary/question lines
   render wrapped + token-broken inside `card_titled`; the pane's ScrollArea
   clamps to the panel width so card content cannot overflow the card or the
   panel (owner screenshot: Summary text spilling past the card bounds).
5. **RS-4-5 (app-shell/inbox)** — composer pane: no fixed-width children may
   exceed the pane; Cc/Bcc inputs span the composer width (owner screenshot
   + r3 goldens show them ~400 px against a ~870 px composer).
6. **RS-4-6 (app-shell/inbox)** — move "Ctrl+Enter sends the composer" out of
   the views rail (inbox.rs:1252) into the composer footer by the send
   button where the shortcut acts; the rail keeps the counts only.
7. **RS-4-7 (app-shell/inbox)** — conversation list rows: a blank customer
   name falls back to the email local-part (Judy-row evidence: headerless
   row in the live list); reader head keeps its existing fallback.
8. **RS-4-8 (app-shell/tests)** — promote the temporary probe to the permanent
   `kittest_inbox_copilot.rs` suite: copilot-open 1625×828 light+dark +
   copilot-closed 1625 light, diff-locked (the state class no golden ever
   covered); delete `zz_g4_reskin_probe.rs` and its snapshots; regenerate the
   r3_inbox_page_* goldens the RS-4-5/6 changes legitimately alter, plus any
   widget goldens the fixes touch; register the suite in the traceability
   matrix and CI counts (24 suites / 175 tests / 171 goldens target).
9. **RS-4-9 (verify)** — full local verification: fmt, clippy -D warnings,
   core suite, app-shell lib suite, all kittest suites over lavapipe+Xvfb
   (a11y ON), SPP_SMOKE, gate_traceability, gate_no_simulation; the
   copilot-open probe must show zero overlap (reader cards inside their
   viewport, copilot panel clean) before anything is recorded.
10. **RS-4-10 (docs)** — sync every living count/state touched by the new
    suite and goldens (traceability matrix, CHANGELOG drift bullet, ci.yml
    comment, acceptance report regeneration), rebuild the G4 kit if its
    stimulus-source goldens changed, record the batch in completed.md, and
    re-present GATE G4 with the STOP.

None — Batch RS-5 (RS-4-11) completed and moved to `completed.md`
(2026-10-30). `plan.md` holds no further items; the G4 verdict (blind test /
SUS / accepted / waive) remains with the owner.
