# R4 SUS Questionnaire — RS-21 (Gate G4)

> Target: **SUS ≥ 68** (decisions.md D6 fidelity loop: "R4 traceability + blind
> test (5s AI identification) + SUS ≥ 68").
> Instrument: the standard **System Usability Scale** (Brooke 1996) — the ten
> canonical statements, 5-point Likert (1 = Strongly disagree … 5 = Strongly
> agree), applied to SupportOS+ v1.9.0. Only the system name is substituted; the
> statements are verbatim so the score stays comparable to the published
> benchmarks (68 ≈ industry average; the reskin's bar is *at least* average with
> the owner's UX-0/10 verdict as the baseline it must clear).

## Administration

- **When:** after at least 30 minutes of real work in v1.9.0 (the same sessions
  the G4 review already requires) — not immediately after reading the release
  notes.
- **Who:** the program's user(s). Record every participant as a row; the score is
  the mean of per-participant SUS values (standard practice). With a single
  participant the row is the score, and the n=1 caveat is recorded verbatim.
- **Before scoring:** ensure the participant has touched the reskinned surfaces
  relevant to the original complaint (the Inbox first — the UX-0/10 page — plus
  at least one list page, one console page and Settings).

## The ten statements

1. I think that I would like to use this system frequently.
2. I found the system unnecessarily complex. *(reversed)*
3. I thought the system was easy to use.
4. I think that I would need the support of a technical person to be able to use
   this system. *(reversed)*
5. I found the various functions in this system were well integrated.
6. I thought there was too much inconsistency in this system. *(reversed)*
7. I would imagine that most people would learn to use this system very quickly.
8. I found the system very cumbersome to use. *(reversed)*
9. I felt very confident using the system.
10. I needed to learn a lot of things before I could get going with this system.
    *(reversed)*

## Scoring (standard SUS)

- Odd items: score − 1.
- Even items: 5 − score.
- Sum the ten adjusted values, multiply by 2.5 → 0–100 per participant.
- **Pass bar: ≥ 68** (per participant and for the mean; a mean across n).

## Score sheet (filled at Gate G4 — 2026-10-30)

Administration: **chat fallback** (2026-10-30) — the answer sheet's SUS column
transcribed verbatim: `1=4 2=2 3=5 4=1 5=4 6=1 7=4 8=1 9=4 10=2`. The owner had
been working in v1.9.0 live (the Inbox copilot sessions the RS-4 rejection wave
was reported from) before scoring.

| Participant | Q1 | Q2 | Q3 | Q4 | Q5 | Q6 | Q7 | Q8 | Q9 | Q10 | SUS |
|---|---|---|---|---|---|---|---|---|---|---|---|
| owner (n=1) | 4 | 2 | 5 | 1 | 4 | 1 | 4 | 1 | 4 | 2 | **85.0** |

Scoring: odd (4−1)+(5−1)+(4−1)+(4−1)+(4−1) = 16 · even (5−2)+(5−1)+(5−1)+(5−1)+(5−2) = 18 ·
sum 34 × 2.5 = **85.0**.

**Mean SUS: 85.0 · n: 1 · pass bar (≥ 68): met · caveat: n=1 — the owner is the
sole participant (the program's only user); the score is recorded verbatim and
is not a population estimate.**

## Interpretation notes (record with the result)

- SUS measures *usability*, not visual fidelity — it complements (does not
  replace) the traceability matrix and the golden locks.
- A failing score is not silently re-run: record the score, fix the named
  friction, and re-administer the full instrument as a new row.
- The completed sheet is folded into the printable acceptance report
  (`docs/audit/SupportOS-Plus-Reskin-Acceptance-Report.pdf`, regenerate via
  `docs/audit/generate_acceptance_report.py`).
