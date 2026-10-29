# R4 Blind Test — 5-second AI-reply identification (RS-21, Gate G4)

> DC basis: **DS-C-22** — "5-second identification pre-check passes on the card
> alone": a viewer must be able to tell AI-authored content from human-authored
> content within five seconds, by the visual alone. G-2 makes this structural —
> **AI is always violet, in every surface** (`--ds-ai*`), and no non-AI element may
> take the violet tone ("violet for non-AI things" is a DS-C-22 *don't*).

## What is being tested

The reskin's AI-identity guarantee, end to end: any AI-authored reply, suggestion,
summary or chat bubble in v1.9.0 is identifiable **as AI** within a 5-second look
at its card alone — and nothing else wears the AI identity.

## Mechanism pre-check (always-on, automated — not the human test)

The human test below exercises a guarantee the code already enforces structurally:

1. **Typed widget path** — AI content can only render through the DS-C-22 widget
   set (`design::cop_suggest`, `cop_chat`, `cop_similar`) or the `ai` role arm of
   `design::message_card_with`; the AI badge (`badge` with the `ai` tone) is
   hard-wired to `--ds-ai*`. There is no free-form card an AI row could take
   (`crates/app-shell/src/design.rs`, R2c widget set, RS-15).
2. **Single tone map** — §5 DS.TONE is the only status→color site (`design::tone`
   + `tone_colors`); the `ai` tone resolves to violet in both themes. The style
   lint (`crates/app-shell/tests/style_lint.rs`, CI) bans raw color sites, so no
   page can hand-tint a surface violet outside the tone map.
3. **Golden locks** — every AI surface is diff-locked in the kittest goldens
   (e.g. the Inbox copilot panel and AI-badge cards in
   `crates/app-shell/tests/snapshots/r3_inbox_page_*`); a tone regression fails CI
   (RS-21 `snapshots` job).

## Human test protocol (administered at G4; results recorded in §Result)

1. **Stimuli.** Ten single-card images, shown one at a time: five AI-authored,
   five non-AI (agent replies, customer messages, notes). Crops are taken from the
   checked-in kittest goldens (real app renders, not lab mocks):

   | # | Source golden (crates/app-shell/tests/snapshots/) | Truth |
   |---|---|---|
   | 1 | `r3_inbox_page_1440_light.png` — copilot suggest card | AI |
   | 2 | `r3_inbox_page_1440_dark.png` — copilot chat bubble (assistant) | AI |
   | 3 | `r3_inbox_page_1024_light.png` — AI-drafted reply card in the reader | AI |
   | 4 | `r3_live_chats_page_1440_light.png` — AI/agent message card | AI |
   | 5 | `r3_ai_center_page_1440_light.png` — AI status/badge surface | AI |
   | 6 | `r3_inbox_page_1440_light.png` — agent reply card | not AI |
   | 7 | `r3_inbox_page_1440_dark.png` — customer message card | not AI |
   | 8 | `r3_live_chats_page_1440_light.png` — agent message card | not AI |
   | 9 | `r3_inbox_page_1024_dark.png` — internal note card | not AI |
   | 10 | `r3_customers_page_1440_light.png` — detail-panel text block | not AI |

   Crop rule: single card, role surface fully visible, no page context that
   answers the question by location (the copilot panel position must not give it
   away — crop to the card). Shuffle before presenting; keep the truth key above.

2. **Presentation.** Each card flashes for **5 seconds** (slides or a timed
   viewer), then the participant answers one question: *"Was this written by AI?"*
   (yes/no, no revision). No training round on the app itself beyond normal use.

3. **Pass bar.** ≥ 9/10 correct overall **and** zero false-AI calls on the five
   non-AI stimuli (a violet mis-read of human content is the failure the DC
   forbids). With multiple participants, record per-participant rows and the
   aggregate; the bar applies to every participant and to the aggregate.

4. **Failure action.** Any miss is a reskin defect: fix the DS-C-22 surface (badge
   placement/role surface), re-lock the goldens, re-run the protocol.

## Result (filled at Gate G4)

| Participant | Correct /10 | False-AI (of 5) | Time per card | Pass |
|---|---|---|---|---|
| _awaiting G4 administration_ | — | — | — | — |

**Aggregate: — /10 · verdict: pending G4.**
