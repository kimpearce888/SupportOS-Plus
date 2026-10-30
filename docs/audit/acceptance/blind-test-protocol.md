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
   five non-AI (agent replies, customer messages, notes). Crops are taken from
   the checked-in kittest goldens (real app renders, not lab mocks). The table
   below is the verified crop set (2026-10-30, owner-directed kit build): the
   first-draft sources were corrected where the named page golden does not
   visually contain the described card — the AI-authored surfaces live in the
   widget-suite identity goldens (the role-typed cards in isolation), and the
   AI-badge status surface lives in the Diagnostics audit trail (the ai_center
   goldens carry no violet AI card). Every crop was verified visually: single
   card, role surface fully visible, no page context that answers the question
   by location.

   | # | Source golden (crates/app-shell/tests/snapshots/) | Truth |
   |---|---|---|
   | 1 | `r2c_inbox_identity_light.png` — AI draft card (Support Ai, violet, insert-as-reply) | AI |
   | 2 | `r2c_inbox_identity_dark.png` — AI draft card (dark) | AI |
   | 3 | `r2c_inbox_identity_light.png` — copilot Suggested-reply panel (AI badge) | AI |
   | 4 | `r2c_inbox_identity_dark.png` — copilot Suggested-reply panel (dark) | AI |
   | 5 | `r3_diagnostics_page_1440_dark.png` — audit row "reply drafted" with the AI actor + ai-column badges | AI |
   | 6 | `r2c_inbox_identity_dark.png` — agent reply card (Kim Pearce) | not AI |
   | 7 | `r2c_inbox_identity_light.png` — internal note card (amber) | not AI |
   | 8 | `r3_inbox_page_1440_light.png` — agent reply card (Ada Lovelace) | not AI |
   | 9 | `r3_inbox_page_1024_light.png` — customer message card (Kim Pearce) | not AI |
   | 10 | `r3_customers_detail_light.png` — Support health stats block (no AI badge; the panel's Interaction-profile card IS AI-badged and must not be used as a not-AI stimulus) | not AI |

   Crop rule: single card, role surface fully visible, no page context that
   answers the question by location (the copilot panel position must not give it
   away — crop to the card). Shuffle before presenting; keep the truth key above.
   The committed kit generator `make_g4_kit.py` (same directory) crops this table
   verbatim, shuffles with a fixed seed, and emits `g4-kit/g4-blind-test-slides.pdf`
   (instructions + ten stimulus pages + marked end + administrator key) and
   `g4-kit/g4-answer-sheet.pdf` (blind-test answers + the SUS form).

2. **Presentation.** Each card flashes for **5 seconds** (slides or a timed
   viewer), then the participant answers one question: *"Was this written by AI?"*
   (yes/no, no revision). No training round on the app itself beyond normal use.

3. **Pass bar.** ≥ 9/10 correct overall **and** zero false-AI calls on the five
   non-AI stimuli (a violet mis-read of human content is the failure the DC
   forbids). With multiple participants, record per-participant rows and the
   aggregate; the bar applies to every participant and to the aggregate.

4. **Failure action.** Any miss is a reskin defect: fix the DS-C-22 surface (badge
   placement/role surface), re-lock the goldens, re-run the protocol.

## Result (filled at Gate G4 — 2026-10-30)

**Administration: chat fallback.** The owner viewed
`g4-kit/g4-blind-test-slides.pdf` and answered the ten cards in chat; the answer
sheet's yes/no column is transcribed verbatim:
`1Y 2N 3N 4Y 5N 6Y 7N 8N 9Y 10N`. Deviation recorded, not silently dropped: the
5-second per-card flash is **not enforced** in this administration mode. Scored
against the administrator key (seeded shuffle, deterministic):

| Participant | Correct /10 | False-AI (of 5) | Time per card | Pass |
|---|---|---|---|---|
| owner (n=1) | **5 /10** | **2 of 5** | not enforced (chat administration) | **no** — the pre-registered bar (≥ 9/10 and 0 false-AI) was **not** met |

Per-card detail (presentation order per the administrator key): correct — cards
1, 5, 6, 7, 10 · misses on AI stimuli — cards 2, 3, 8 (the AI draft cards
light + dark and the dark copilot suggested-reply panel were read as human) ·
**false-AI** — cards 4, 9 (the amber internal note card and the Ada Lovelace
agent-reply card in page context were read as AI).

**Aggregate: 5 /10 · false-AI 2 · pre-registered bar NOT met — recorded, not
spun.** Per §Failure action these two false-AI surfaces are named as reskin
defects: the amber note card (a warm tone the participant read as the AI
identity) and the agent-reply-in-page-context card.

**Gate G4 verdict (owner, 2026-10-30): accepted.** The owner's final acceptance
— given in the same message as the verdict `blind test + SUS / accepted` — closes
the gate with the score above in view. The owner's acceptance supersedes the
protocol's fix-and-rerun loop for this program; the two false-AI surfaces stand
as disclosed, owner-accepted limitations. No record above is altered by the
verdict.
