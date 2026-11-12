#!/usr/bin/env python3
"""G4 administration kit generator (RS-21, Gate G4).

Builds the two printable instruments from the checked-in kittest goldens — the
real app renders, not mocks — so the human administration is fifteen minutes:

  g4-blind-test-slides.pdf   presentation deck: instructions, ten stimulus pages
                             (one card per page, shuffled, no truth markers),
                             then the administrator truth key after a marked
                             page break.
  g4-answer-sheet.pdf        the participant sheet: ten yes/no answers + the
                             standard SUS ten statements (Brooke 1996) with
                             5-point Likert circles.

Deterministic: fixed presentation order (seeded shuffle), fixed crops. Re-run
regenerates byte-stable content (PDF timestamps aside). Sources are the widget
suite + page goldens; the stimulus table below was verified visually crop by
crop (single card, role surface fully visible, no location giveaway — the
copilot panel position and page chrome never appear).

Usage: python3 docs/audit/acceptance/make_g4_kit.py
Output: docs/audit/acceptance/g4-kit/
"""
import os
import random

from PIL import Image
from reportlab.lib.pagesizes import A4, landscape
from reportlab.lib.units import mm
from reportlab.pdfgen import canvas

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(HERE, "..", "..", ".."))
SNAP = os.path.join(REPO, "crates", "app-shell", "tests", "snapshots")
OUT = os.path.join(HERE, "g4-kit")

# (source golden, crop box (l, t, r, b), truth) — verified single-card crops.
# Presentation order is the table order shuffled with the fixed seed below.
STIMULI = [
    ("r2c_inbox_identity_light.png", (12, 432, 708, 566), "AI"),
    ("r2c_inbox_identity_dark.png", (12, 432, 708, 566), "AI"),
    ("r2c_inbox_identity_light.png", (12, 616, 708, 812), "AI"),
    ("r2c_inbox_identity_dark.png", (12, 616, 708, 812), "AI"),
    ("r3_diagnostics_page_1440_dark.png", (14, 672, 668, 706), "AI"),
    ("r2c_inbox_identity_dark.png", (12, 200, 708, 300), "not AI"),
    ("r2c_inbox_identity_light.png", (12, 318, 708, 414), "not AI"),
    ("r3_inbox_page_1440_light.png", (533, 347, 1410, 455), "not AI"),
    ("r3_inbox_page_1024_light.png", (543, 228, 995, 345), "not AI"),
    ("r3_customers_detail_light.png", (243, 142, 442, 398), "not AI"),
]
SEED = 20261029

SUS_STATEMENTS = [
    "I think that I would like to use this system frequently.",
    "I found the system unnecessarily complex. (reversed)",
    "I thought the system was easy to use.",
    "I think that I would need the support of a technical person to be able to use this system. (reversed)",
    "I found the various functions in this system were well integrated.",
    "I thought there was too much inconsistency in this system. (reversed)",
    "I would imagine that most people would learn to use this system very quickly.",
    "I found the system very cumbersome to use. (reversed)",
    "I felt very confident using the system.",
    "I needed to learn a lot of things before I could get going with this system. (reversed)",
]


def build():
    os.makedirs(OUT, exist_ok=True)
    order = list(range(len(STIMULI)))
    random.Random(SEED).shuffle(order)

    crops = []
    tmp = os.path.join(OUT, "_stim_tmp")
    os.makedirs(tmp, exist_ok=True)
    for i, (src, box, truth) in enumerate(STIMULI):
        im = Image.open(os.path.join(SNAP, src)).convert("RGB")
        crop = im.crop(box)
        crops.append(crop)
        crop.save(os.path.join(tmp, f"s{i + 1:02d}.png"))

    # ---------------- slides ----------------
    path = os.path.join(OUT, "g4-blind-test-slides.pdf")
    c = canvas.Canvas(path, pagesize=landscape(A4))
    w, h = landscape(A4)

    def line(x, y, text, size=11, bold=False):
        c.setFont("Helvetica-Bold" if bold else "Helvetica", size)
        c.drawString(x, y, text)

    # instructions page
    y = h - 30 * mm
    line(25 * mm, y, "R4 Blind Test — 5-second AI-reply identification", 18, True)
    y -= 12 * mm
    for t in [
        "Protocol: docs/audit/acceptance/blind-test-protocol.md (DC basis DS-C-22, Gate G4).",
        "1. Show each of the following ten cards for exactly 5 seconds (a phone timer or metronome works),",
        "    then ask once: \u201cWas this written by AI?\u201d — yes/no, no revision, no discussion.",
        "2. Record the answers on g4-answer-sheet.pdf (row numbers match the slide page numbers).",
        "3. Pass bar: \u2265 9/10 correct overall AND zero false-AI calls on the five non-AI cards.",
        "4. The cards were cropped from the checked-in kittest goldens (real app renders). No page",
        "    context appears — the answer must come from the card alone.",
        "",
        "Presenter: advance one page per card. The truth key is printed AFTER the end marker —",
        "do not show pages past the end marker to the participant.",
    ]:
        line(25 * mm, y, t, 11)
        y -= 7 * mm
    c.showPage()

    # stimulus pages
    for slide_no, stim_idx in enumerate(order, 1):
        img = crops[stim_idx]
        iw, ih = img.size
        max_w, max_h = w - 60 * mm, h - 80 * mm
        scale = min(max_w / iw, max_h / ih, 2.2)
        dw, dh = iw * scale, ih * scale
        c.drawImage(os.path.join(OUT, "_stim_tmp", f"s{stim_idx + 1:02d}.png"),
                    (w - dw) / 2, (h - dh) / 2 + 8 * mm, dw, dh)
        line(w / 2 - 12 * mm, 18 * mm, f"card {slide_no} / 10", 12, True)
        c.showPage()

    # end marker
    y = h / 2
    line(25 * mm, y, "END OF STIMULI — stop here.", 16, True)
    y -= 10 * mm
    line(25 * mm, y, "Everything after this page is the administrator key.", 11)
    c.showPage()

    # administrator truth key
    y = h - 30 * mm
    line(25 * mm, y, "ADMINISTRATOR KEY — not for the participant", 16, True)
    y -= 10 * mm
    line(25 * mm, y, "Presentation order (seeded shuffle, deterministic):", 11, True)
    y -= 8 * mm
    for slide_no, stim_idx in enumerate(order, 1):
        src, box, truth = STIMULI[stim_idx]
        line(28 * mm, y, f"card {slide_no:2d}  ->  {truth:<6}  source: {src}  crop {box}", 10)
        y -= 6 * mm
    y -= 4 * mm
    line(25 * mm, y, "Scoring: 1 point per correct call; pass = >= 9/10 AND zero false-AI (an 'AI' answer on a non-AI card).", 11)
    c.showPage()
    c.save()

    # ---------------- answer sheet + SUS form ----------------
    path = os.path.join(OUT, "g4-answer-sheet.pdf")
    c = canvas.Canvas(path, pagesize=A4)
    w, h = A4
    y = h - 25 * mm
    line(20 * mm, y, "G4 answer sheet — blind test + SUS (SupportOS+ v1.9.0)", 15, True)
    y -= 10 * mm
    line(20 * mm, y, "Participant: ____________________    Date: ____________", 11)
    y -= 10 * mm

    line(20 * mm, y, "Part 1 — blind test (one answer per card, in presentation order)", 12, True)
    y -= 8 * mm
    for i in range(1, 11):
        line(22 * mm, y, f"card {i:2d}:   Was this written by AI?      YES  [ ]        NO  [ ]", 11)
        y -= 7 * mm

    y -= 6 * mm
    line(20 * mm, y, "Part 2 — SUS (1 = strongly disagree … 5 = strongly agree, one circle per row)", 12, True)
    y -= 9 * mm
    col_x = [22 * mm, 132 * mm, 142 * mm, 152 * mm, 162 * mm, 172 * mm, 182 * mm]
    for x, lab in zip(col_x[1:], ["1", "2", "3", "4", "5"]):
        line(x, y, lab, 10, True)
    y -= 4 * mm
    for i, s in enumerate(SUS_STATEMENTS, 1):
        line(col_x[0], y, f"{i:2d}. {s}", 9)
        for x in col_x[1:]:
            c.circle(x + 1.2 * mm, y + 1.2 * mm, 1.6 * mm)
        y -= 9 * mm

    y -= 4 * mm
    line(20 * mm, y, "Scoring (administrator): odd items score-1, even items 5-score, sum x 2.5 -> 0-100. Pass bar >= 68.", 9)
    line(20 * mm, y - 6 * mm, "Record the results in docs/audit/acceptance/sus-questionnaire.md and completed.md at Gate G4.", 9)
    c.showPage()
    c.save()

    # cleanup temp crops (the slides PDF embeds them)
    tmp = os.path.join(OUT, "_stim_tmp")
    for f in os.listdir(tmp):
        os.remove(os.path.join(tmp, f))
    os.rmdir(tmp)

    print("G4_KIT_OK", OUT)
    print("presentation order:", [f"s{i+1:02d}({STIMULI[i][2]})" for i in order])


if __name__ == "__main__":
    build()
