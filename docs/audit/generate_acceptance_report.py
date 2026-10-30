#!/usr/bin/env python3
# SupportOS-Plus Reskin Acceptance Report (R4) - body PDF generator (ReportLab, Report route)
# Series-consistent with the Deep Audit Report and the UX Recovery & Reskin Plan
# (docs/audit/generate_audit_report.py, generate_ux_report.py — Template 07 Crystal
# Blue body palette, FreeSerif/NotoSerifSC typography). RS-21 (Batch RS-3): the
# R4 acceptance records folded into the printable report series (the deferred PDF
# task) — traceability, blind test, SUS, CI snapshot locks, release v1.9.0, G4.
import os, re, sys
from reportlab.lib.pagesizes import A4
from reportlab.lib.units import inch
from reportlab.lib import colors
from reportlab.lib.styles import ParagraphStyle
from reportlab.lib.enums import TA_LEFT, TA_CENTER
from reportlab.platypus import (SimpleDocTemplate, Paragraph, Spacer, Table, TableStyle,
                                PageBreak, KeepTogether, CondPageBreak, HRFlowable)
from reportlab.platypus.tableofcontents import TableOfContents
from reportlab.pdfbase import pdfmetrics
from reportlab.pdfbase.ttfonts import TTFont
from reportlab.pdfbase.pdfmetrics import registerFontFamily

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
OUT = os.path.join(ROOT, "docs/audit/SupportOS-Plus-Reskin-Acceptance-Report.pdf")

FONT_DIR = "/usr/share/fonts"
pdfmetrics.registerFont(TTFont("NotoSerifSC", f"{FONT_DIR}/truetype/noto-serif-sc/NotoSerifSC-Regular.ttf"))
pdfmetrics.registerFont(TTFont("NotoSerifSC-Bold", f"{FONT_DIR}/truetype/noto-serif-sc/NotoSerifSC-Bold.ttf"))
pdfmetrics.registerFont(TTFont("FreeSerif", f"{FONT_DIR}/truetype/freefont/FreeSerif.ttf"))
pdfmetrics.registerFont(TTFont("FreeSerif-Bold", f"{FONT_DIR}/truetype/freefont/FreeSerifBold.ttf"))
pdfmetrics.registerFont(TTFont("FreeSerif-Italic", f"{FONT_DIR}/truetype/freefont/FreeSerifItalic.ttf"))
pdfmetrics.registerFont(TTFont("FreeSerif-BoldItalic", f"{FONT_DIR}/truetype/freefont/FreeSerifBoldItalic.ttf"))
pdfmetrics.registerFont(TTFont("DejaVuSans", f"{FONT_DIR}/truetype/dejavu/DejaVuSansMono.ttf"))
registerFontFamily("NotoSerifSC", normal="NotoSerifSC", bold="NotoSerifSC-Bold")
registerFontFamily("FreeSerif", normal="FreeSerif", bold="FreeSerif-Bold",
                   italic="FreeSerif-Italic", boldItalic="FreeSerif-BoldItalic")
registerFontFamily("DejaVuSans", normal="DejaVuSans", bold="DejaVuSans")

try:  # the series' font-fallback helper (present in this lab; optional elsewhere)
    PDF_SKILL_DIR = "/home/z/my-project/skills/pdf"
    sys.path.insert(0, os.path.join(PDF_SKILL_DIR, "scripts"))
    from pdf import install_font_fallback
    install_font_fallback()
except Exception:
    pass

# --- Template 07 Crystal Blue fixed body palette (series-consistent)
PAGE_BG      = colors.HexColor("#f5f8fc")
SECTION_BG   = colors.HexColor("#edf2f9")
CARD_BG      = colors.HexColor("#e4ecf5")
TABLE_STRIPE = colors.HexColor("#eef3fa")
HEADER_FILL  = colors.HexColor("#1a4a7a")
BORDER       = colors.HexColor("#c0d0e2")
ACCENT       = colors.HexColor("#2d7ab3")
TEXT_PRIMARY = colors.HexColor("#142840")
TEXT_MUTED   = colors.HexColor("#5a7a96")

DOC_TITLE  = "SupportOS-Plus — Design-System Reskin Acceptance Report (R4)"
DOC_AUTHOR = "kimpearce888 / SupportOS-Plus · RS-21 (Batch RS-3) · 2026-10-29"
MARGIN = 0.9 * inch
PAGE_W, PAGE_H = A4

S = {
    "cover_title": ParagraphStyle("cover_title", fontName="FreeSerif-Bold", fontSize=27, leading=34,
                                  textColor=colors.white, alignment=TA_LEFT),
    "cover_sub": ParagraphStyle("cover_sub", fontName="FreeSerif", fontSize=12.5, leading=19,
                                textColor=colors.HexColor("#dbe7f3")),
    "toc_title": ParagraphStyle("toc_title", fontName="FreeSerif-Bold", fontSize=19, leading=24,
                                textColor=TEXT_PRIMARY),
    "h1": ParagraphStyle("h1", fontName="FreeSerif-Bold", fontSize=16.5, leading=21,
                         textColor=HEADER_FILL, spaceBefore=14, spaceAfter=7),
    "h2": ParagraphStyle("h2", fontName="FreeSerif-Bold", fontSize=12.5, leading=17,
                         textColor=TEXT_PRIMARY, spaceBefore=10, spaceAfter=4),
    "body": ParagraphStyle("body", fontName="FreeSerif", fontSize=10.2, leading=15.3,
                           textColor=TEXT_PRIMARY, alignment=TA_LEFT, spaceAfter=6),
    "quote": ParagraphStyle("quote", fontName="FreeSerif-Italic", fontSize=10, leading=15,
                            textColor=TEXT_MUTED, leftIndent=14, spaceAfter=6),
    "code": ParagraphStyle("code", fontName="DejaVuSans", fontSize=8.2, leading=12,
                           textColor=TEXT_PRIMARY, backColor=SECTION_BG,
                           borderPadding=6, leftIndent=6, spaceAfter=6),
    "cell": ParagraphStyle("cell", fontName="FreeSerif", fontSize=8.6, leading=12,
                           textColor=TEXT_PRIMARY),
    "cell_b": ParagraphStyle("cell_b", fontName="FreeSerif-Bold", fontSize=8.6, leading=12,
                             textColor=TEXT_PRIMARY),
    "cell_h": ParagraphStyle("cell_h", fontName="FreeSerif-Bold", fontSize=8.8, leading=12,
                             textColor=colors.white),
    "caption": ParagraphStyle("caption", fontName="FreeSerif-Italic", fontSize=8.2, leading=11.5,
                              textColor=TEXT_MUTED, spaceBefore=2, spaceAfter=8),
}

def esc(t):
    return t.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")

story = []

def h1(num, text):
    full = f"{num}. {text}"
    key = f"key-h1-{num}"
    flow = Paragraph(f'<a name="{key}"/><b>{esc(full)}</b>', S["h1"])
    flow.bookmark_name = f"h1-{num}"
    flow.bookmark_level = 0
    flow.bookmark_text = full
    flow.bookmark_key = key
    story.append(CondPageBreak(1.4 * inch))
    story.append(flow)
    story.append(HRFlowable(width="100%", color=ACCENT, thickness=1.1, spaceAfter=8))

def h2(text):
    key = f"key-h2-{abs(hash(text)) % 100000}"
    flow = Paragraph(f'<a name="{key}"/><b>{esc(text)}</b>', S["h2"])
    flow.bookmark_name = f"h2-{abs(hash(text)) % 100000}"
    flow.bookmark_level = 1
    flow.bookmark_text = text
    flow.bookmark_key = key
    story.append(flow)

def p(text):
    story.append(Paragraph(text, S["body"]))

def quote(text):
    story.append(Paragraph(text, S["quote"]))

def code(text):
    story.append(Paragraph(esc(text).replace("\n", "<br/>"), S["code"]))

def bullets(items):
    for it in items:
        story.append(Paragraph(f"•&nbsp;&nbsp;{it}", S["body"]))

def callout_row(items):
    cells = [[Paragraph(f"<b>{esc(k)}</b>", S["cell_b"]), Paragraph(esc(v), S["cell"])] for k, v in items]
    t = Table(cells, colWidths=[(PAGE_W - 2 * MARGIN) / len(items) - 6] * len(items))
    t.setStyle(TableStyle([
        ("BACKGROUND", (0, 0), (-1, -1), CARD_BG),
        ("BOX", (0, 0), (-1, -1), 0.8, BORDER),
        ("INNERGRID", (0, 0), (-1, -1), 0.5, BORDER),
        ("VALIGN", (0, 0), (-1, -1), "TOP"),
        ("TOPPADDING", (0, 0), (-1, -1), 6),
        ("BOTTOMPADDING", (0, 0), (-1, -1), 6),
        ("LEFTPADDING", (0, 0), (-1, -1), 6),
        ("RIGHTPADDING", (0, 0), (-1, -1), 6),
    ]))
    story.append(t)
    story.append(Spacer(1, 8))

def table(headers, rows, ratios, caption=None):
    data = [[Paragraph(f"<b>{esc(hh)}</b>", S["cell_h"]) for hh in headers]]
    for r in rows:
        data.append([Paragraph(esc(c), S["cell"]) for c in r])
    total = PAGE_W - 2 * MARGIN
    t = Table(data, colWidths=[total * x for x in ratios], repeatRows=1)
    style = [
        ("BACKGROUND", (0, 0), (-1, 0), HEADER_FILL),
        ("VALIGN", (0, 0), (-1, -1), "TOP"),
        ("BOX", (0, 0), (-1, -1), 0.8, BORDER),
        ("INNERGRID", (0, 0), (-1, -1), 0.4, BORDER),
        ("TOPPADDING", (0, 0), (-1, -1), 4),
        ("BOTTOMPADDING", (0, 0), (-1, -1), 4),
        ("LEFTPADDING", (0, 0), (-1, -1), 5),
        ("RIGHTPADDING", (0, 0), (-1, -1), 5),
    ]
    for i in range(1, len(data)):
        style.append(("BACKGROUND", (0, i), (-1, i), TABLE_STRIPE if i % 2 == 0 else colors.white))
    t.setStyle(TableStyle(style))
    story.append(t)
    if caption:
        story.append(Paragraph(caption, S["caption"]))
    else:
        story.append(Spacer(1, 8))

# ============================ COVER ============================
cover_meta = Table(
    [[Paragraph("<b>SupportOS+ · Design-System Reskin</b>", S["cover_title"])],
     [Paragraph("R4 Final Acceptance + Release v1.9.1 — the acceptance record of the "
                "presentation-layer reskin: traceability to the frozen Design Contract, "
                "the 5-second AI-identification blind test, the SUS instrument, the CI "
                "snapshot locks, and the release record. Report 4 of the printable "
                "series (Deep Audit · UX Recovery &amp; Reskin Plan · Review Analysis "
                "&amp; Industry Benchmark · Reskin Acceptance).", S["cover_sub"])]],
    colWidths=[PAGE_W - 2.4 * inch])
cover_meta.setStyle(TableStyle([
    ("BACKGROUND", (0, 0), (-1, -1), HEADER_FILL),
    ("VALIGN", (0, 0), (-1, -1), "MIDDLE"),
    ("TOPPADDING", (0, 0), (-1, 0), 26), ("BOTTOMPADDING", (0, -1), (-1, -1), 26),
    ("LEFTPADDING", (0, 0), (-1, -1), 22), ("RIGHTPADDING", (0, 0), (-1, -1), 22),
]))
cover_rows = [
    ["Document", "SupportOS-Plus-Reskin-Acceptance-Report.pdf"],
    ["Series", "4 of 4 — the printable report series (docs/audit/)"],
    ["Program", "Design-System Reskin RS-01..RS-21 (decisions.md D6)"],
    ["Item", "RS-21 — R4: final acceptance + release (Batch RS-3)"],
    ["Contract", "Design Contract v1.0 (FROZEN 2026-10-28) — docs/audit/design-contract-v1.md"],
    ["Status", "Closed — GATE G4 owner verdict: accepted (2026-10-30); blind-test miss disclosed in §4.2, not waived; the accepted state releases as v1.9.1 (v1.9.0, tagged at the RS-3 STOP, predates the fix wave)"],
    ["Date", "2026-10-30"],
]
meta_t = Table([[Paragraph(f"<b>{esc(k)}</b>", S["cell_b"]), Paragraph(esc(v), S["cell"])] for k, v in cover_rows],
               colWidths=[1.5 * inch, PAGE_W - 2.4 * inch - 1.5 * inch])
meta_t.setStyle(TableStyle([
    ("ROWBACKGROUNDS", (0, 0), (-1, -1), [colors.white, TABLE_STRIPE]),
    ("BOX", (0, 0), (-1, -1), 0.8, BORDER),
    ("INNERGRID", (0, 0), (-1, -1), 0.4, BORDER),
    ("VALIGN", (0, 0), (-1, -1), "TOP"),
    ("TOPPADDING", (0, 0), (-1, -1), 5), ("BOTTOMPADDING", (0, 0), (-1, -1), 5),
    ("LEFTPADDING", (0, 0), (-1, -1), 6), ("RIGHTPADDING", (0, 0), (-1, -1), 6),
]))
story.append(Spacer(1, 1.5 * inch))
story.append(cover_meta)
story.append(Spacer(1, 0.45 * inch))
story.append(meta_t)
story.append(PageBreak())

# ============================ TOC ============================
class TocDocTemplate(SimpleDocTemplate):
    def afterFlowable(self, flowable):
        if hasattr(flowable, "bookmark_name"):
            level = getattr(flowable, "bookmark_level", 0)
            text = getattr(flowable, "bookmark_text", "")
            key = getattr(flowable, "bookmark_key", "")
            self.notify("TOCEntry", (level, text, self.page, key))

toc = TableOfContents()
toc.levelStyles = [
    ParagraphStyle("TOC1", fontName="FreeSerif", fontSize=11.5, leading=17, leftIndent=14,
                   firstLineIndent=-14, textColor=TEXT_PRIMARY, spaceBefore=4),
    ParagraphStyle("TOC2", fontName="FreeSerif", fontSize=9.5, leading=14, leftIndent=34,
                   firstLineIndent=-14, textColor=TEXT_MUTED),
]
story.append(Paragraph("<b>Table of Contents</b>", S["toc_title"]))
story.append(HRFlowable(width="100%", color=ACCENT, thickness=1.2, spaceAfter=12))
story.append(toc)
story.append(PageBreak())

# ================= 1. EXECUTIVE SUMMARY =================
h1(1, "Executive Summary")
p("On 28 October 2026 the owner set the program's goal: \x27everything is working fine, we need "
  "to reskin only according to the new design system\x27 (decisions.md D6) — after the UX verdict "
  "of 0/10 with the Inbox named as the worst page. The reskin was executed as a presentation-only "
  "program under a frozen contract: Design Contract v1.0 (DC v1.0) extracted from the "
  "owner-approved design lab (Gate G1), ported 1:1 into the egui codebase across three waves — "
  "R1 tokens, R2 widget sets, R3 page rebuilds — with every behavior, query, write, shortcut and "
  "workflow byte-identical. This report is the R4 acceptance record: it closes the program by "
  "tracing every screen element back to the contract, packaging the human acceptance instruments "
  "(the 5-second AI-identification blind test and the SUS questionnaire), locking every rebuilt "
  "page against visual drift in CI, and cutting release v1.9.0.")
callout_row([
    ("23 routes", "every page re-rendered through the DC widget set (R3 complete)"),
    ("17 + 6", "canonical DS-C component sheets + DS-A archetype recipes, all traced"),
    ("24 / 175 / 171", "kittest suites / tests (173 kittest + 2 style lint) / goldens diff-locking every page"),
    ("v1.9.0", "the reskin release, tagged from the verified acceptance state"),
])
p("The scope contract never moved: no feature, data, workflow, shortcut or page-set change "
  "(D6; DC G-6). Data-layer defects surfaced during the rebuild are footnoted as receipt-cited "
  "quirks (DC C-1) and stay visible — they were documented, never faked fixed. The gate "
  "sequence followed rule.md: G1 (lab conformance, commit af5bc2a) → G2/G3 evidence packs at "
  "the Batch RS-2 STOP (token pins + the 23-suite golden set) → this R4 package → GATE G4, "
  "the owner's final acceptance.")

# ================= 2. PROGRAM RECORD & GATES =================
h1(2, "Program Record and Gates")
h2("2.1 Gate receipts")
table(
    ["Gate", "Scope", "Receipt"],
    [
        ["G1", "Design-lab conformance (RS-01..RS-10, HTML/CSS/JS only)",
         "Owner approved the conformed lab as the final design — commit af5bc2a design-lab-v4; receipt: owner 'continue' after the RS-1 STOP."],
        ["G2", "Token sign-off (R1)",
         "RS-12's 1:1 --ds-* → design.rs LIGHT/DARK port + the DC pin tests; style lint enforces no-raw-color / on-scale in CI."],
        ["G3", "Inbox pilot acceptance (R3 first wave)",
         "RS-16 rebuilt the Inbox onto DS-A-05; the six r3_inbox_page_* goldens at 1440/1024/800 × 900, light + dark, a11y check ON."],
        ["G4", "Final acceptance (R4)",
         "This report — presented to the owner with the traceability matrix, the blind test, the SUS instrument and release v1.9.0."],
    ],
    [0.08, 0.32, 0.60],
    caption="Table 2.1 — gate receipts. G2/G3 evidence packs were presented at the Batch RS-2 STOP (completed.md, 2026-10-29).")
h2("2.2 Commit chain (Batch RS-2, R1–R3)")
code("af5bc2a (RS-1 lab) → 8b83daf (G1) → f84d43c (RS-11) → fbf106c (RS-12) → 8bb994f (RS-13)\n"
     "→ b5e613d (RS-14) → 9718cd7 (RS-15) → d172cfc (RS-15) → 8490eee (CI fix) → 7001cf8..c4b7315\n"
     "(RS-19 waves) → 5ddb782..feee18a (RS-20 waves) → 5c12fd1 (batch record) → this batch (RS-21)")
p("Batch RS-2 closed 2026-10-29 with: fmt clean; clippy -D warnings 0 (workspace + core sandbox); "
  "app-shell 236 + core 485 + core sandbox 493 + xtask 11 + style-lint 2, all green; kittest 170 "
  "tests across the 22 suites green (every page golden diff-locked twice, a11y check ON); SPP_SMOKE exit 0 "
  "(24 frames painted on a fresh binary).")

# ================= 3. TRACEABILITY MATRIX =================
h1(3, "R4 Traceability Matrix — screen region → DS-C id → DC row")
p("The full matrix lives at <b>docs/audit/acceptance/traceability-matrix.md</b> and is "
  "machine-checked: <b>scripts/gate_traceability.py</b> (a CI step in both ci.yml and release.yml) "
  "fails the build when (a) any DS-C/DS-A id cited in the matrix does not exist in the frozen DC, "
  "(b) any app route (PageKey) has no matrix section, (c) any cited golden-lock file is missing, "
  "or (d) any design.rs DC doc-tag stops resolving to the DC. One registered alias exists "
  "(G-7): DS-C-04 — the code's label for the MetricTile half of the DC's DS-C-03 sheet. The gate "
  "accepts the alias only while the matrix declares it.")
h2("3.1 Archetype coverage (DC §4)")
table(
    ["DS-A recipe", "Pages (routes)", "Core components"],
    [
        ["DS-A-01 List page", "customers · organizations · incidents",
         "DS-C-01 header + counter pill · DS-C-10 FilterBar · DS-C-05 DataTable · DS-C-17 DetailPanel over DS-A-03 · DS-C-14 EmptyState"],
        ["DS-A-02 Dashboard", "dashboard · reports · issue-radar · custom-objects · outreach · automation · support-graph · knowledge*",
         "DS-C-01 · DS-C-09 tabs/grouped · DS-C-03 Stat/MetricTile · DS-C-23 bar rows · DS-C-02 cards · DS-C-06 chips/badges · DS-C-15 callout roles"],
        ["DS-A-03 Detail record", "(composition recipe) — DetailPanel content everywhere",
         "record-head + two-col blocks / cards (DS-C-19 avatars, DS-C-18 kv)"],
        ["DS-A-04 Settings", "settings · connectors · notifications · onboarding · live-chats",
         "DS-C-09 rail · DS-C-12/13 forms · DS-C-02 + danger zones · DS-C-16 armed confirm"],
        ["DS-A-05 Inbox workspace", "inbox",
         "views rail · list rows + DS-C-06 unread dot · reader with DS-C-20 role-typed cards · DS-C-21 Composer · DS-C-22 copilot"],
        ["DS-A-06 Console / log", "operations · ai-center · diagnostics · sync-health · search",
         "DS-C-24 hero (sync-health; B-2 closure) · event rows + level dots · chips · tables"],
    ],
    [0.16, 0.34, 0.50],
    caption="Table 3.1 — the six archetype recipes over the 23 routes. *Knowledge rides the DS-A-02 "
            "block stack over the DS-C-09 rail (RS-18 wave B). Shell chrome traces to DC §6.")
h2("3.2 Shell chrome (DC §6)")
table(
    ["Region", "DC trace", "Lock"],
    [
        ["Sidebar nav (NAV_GROUPS verbatim; active = neutral fill + 500)", "§6", "SPP_SMOKE paint gate"],
        ["Global search (⌘K) stays in the sidebar", "§6", "SPP_SMOKE paint gate"],
        ["Theme switch light/dark (persisted; header outline act)", "§6 + §2 + DS-C-11", "SPP_SMOKE paint gate"],
        ["Shell page: Runtime card + ported-pages hint (stale-checklist tag)", "DS-C-02 + DS-C-15 hint", "SPP_SMOKE paint gate (harness exception, RS-20h)"],
    ],
    [0.52, 0.18, 0.30])
h2("3.3 Registered exceptions and contractual keeps (DC §7)")
bullets([
    "<b>E-1</b> — blur shadows render as hairline strokes; motion only where egui animates natively (painter model).",
    "<b>C-1</b> — receipt-cited data-layer quirks stay visible as quirk footnotes (knowledge freshness void, "
    "Load/Pressure swap, endpoint-name combo, approvals raw-JSON collapse, notifications raw-UTC hover, "
    "campaign report top-level-key read, outreach raw severity tokens, radar severity tokens).",
    "<b>C-2</b> — data-track items labeled mocked (unread counts, assignee, draft persistence) until the async track lands.",
])

# ================= 4. BLIND TEST =================
h1(4, "Blind Test — 5-second AI-reply identification")
p("DC basis DS-C-22: \x27AI is always violet, in every surface\x27 (G-2) and the 5-second "
  "identification pre-check passes on the card alone. The full protocol lives at "
  "<b>docs/audit/acceptance/blind-test-protocol.md</b>. Summary: ten single-card stimuli (five "
  "AI-authored, five non-AI) cropped from the checked-in kittest goldens flash for five seconds "
  "each; the participant answers \x27was this written by AI?\x27. Pass bar: ≥ 9/10 correct and zero "
  "false-AI calls on the non-AI set (a violet mis-read of human content is precisely the failure "
  "the DC forbids).")
h2("4.1 Mechanism pre-check (always-on, automated)")
bullets([
    "AI content can only render through the typed DS-C-22 widget set (cop_suggest / cop_chat / "
    "cop_similar) or the ai role arm of message_card_with — there is no free-form card for AI rows.",
    "§5 DS.TONE is the single status→color site; the style lint (CI) bans raw color sites, so no "
    "page can hand-tint a surface violet outside the tone map.",
    "Every AI surface is diff-locked in the kittest goldens — a tone regression fails the CI snapshots job.",
])
h2("4.2 Result (recorded at G4 — 2026-10-30, chat administration)")
table(
    ["Participant", "Correct /10", "False-AI (of 5)", "Time per card", "Pass"],
    [["owner (n=1)", "5 /10", "2 of 5", "not enforced (chat)", "no — bar not met"]],
    [0.34, 0.16, 0.18, 0.16, 0.16],
    caption="Pre-registered bar (≥ 9/10 and 0 false-AI) NOT met — recorded, not spun. Misses on the AI draft cards (light + dark) and the dark copilot panel; false-AI on the amber internal-note card and the Ada Lovelace agent reply in page context. Gate G4 verdict: the owner accepted with this score in view — the two false-AI surfaces stand as disclosed, owner-accepted limitations.")

# ================= 5. SUS =================
h1(5, "SUS Questionnaire — target ≥ 68")
p("The standard System Usability Scale (Brooke 1996), applied to v1.9.0 after at least 30 minutes "
  "of real work — the Inbox first (the UX-0/10 page), plus a list page, a console page and "
  "Settings. The full instrument and scoring sheet live at "
  "<b>docs/audit/acceptance/sus-questionnaire.md</b>: ten canonical statements, 5-point Likert, "
  "scored per participant (odd −1, even 5−x, sum × 2.5 → 0–100). Pass bar: ≥ 68 for every "
  "participant and for the mean. SUS measures usability — it complements, never replaces, the "
  "traceability matrix and the golden locks. A failing score is recorded, the named friction is "
  "fixed, and the full instrument is re-administered as a new row.")
table(
    ["Participant", "Q1..Q10", "SUS", "Pass ≥ 68"],
    [["owner (n=1)", "4 · 2 · 5 · 1 · 4 · 1 · 4 · 1 · 4 · 2", "85.0", "yes"]],
    [0.34, 0.30, 0.18, 0.18],
    caption="Mean SUS: 85.0 · n: 1 · caveat (n=1): the sole participant — recorded verbatim, not a population estimate. Chat administration 2026-10-30.")

# ================= 6. CI SNAPSHOT LOCKS =================
h1(6, "CI Snapshot Locks — the reskin drift gate")
p("Before RS-21 the 23 kittest suites ran only in this lab: the goldens were diff-locked twice "
  "locally, but CI never rendered them (the suites are kittest-ui-gated and need a Vulkan "
  "rasterizer). RS-21 adds the <b>snapshots</b> job to ci.yml: the runner installs Mesa's "
  "lavapipe — the software-rasterizer family the baselines were produced with — and runs the "
  "whole set with UPDATE_SNAPSHOTS deliberately unset: every one of the 171 goldens diffs "
  "against the committed baselines and any pixel drift fails the build. A release tag is cut "
  "on a main commit, so the tagged commit's goldens are diffed by the snapshots job; the "
  "Release workflow re-runs fmt, clippy and the workspace tests before publishing.")
h2("6.1 What is locked")
bullets([
    "<b>Real pages, not mocks</b> — every suite renders the production page struct over a seeded "
    "in-memory mirror (fixed-timestamp fixtures), 1440/1024/800 × 900, light + dark, a11y check ON.",
    "<b>Interaction goldens</b> — the heaviest states per page are locked too (Workload tab, Usage tab, "
    "New-Rule/New-Type/New-Campaign forms, Try-export, Neighborhood, errors filter, Mark-complete, "
    "Help-Scout tab, Mentions tab, detail panels).",
    "<b>The widget set itself</b> — kittest_widgets.rs locks the DS-C primitives (16+2 goldens after "
    "the DS-C-24 hero pair joined in RS-20f); style_lint.rs enforces the no-raw-color / on-scale rules.",
    "<b>Traceability</b> — the matrix's golden-lock column is gate-checked: a suite file that "
    "disappears fails the traceability gate before any snapshot can silently rot.",
])
h2("6.2 The discipline")
p("Regeneration is a deliberate, reviewed act: UPDATE_SNAPSHOTS=1 produces the new baselines, a "
  "clean re-run must then pass byte-identical (the repo's diff-locked-twice rule), and the diff "
  "ships in the commit that justifies it. In CI the rule is stricter — there is no regeneration; "
  "drift is failure.")

# ================= 7. RELEASE v1.9.1 =================
h1(7, "Release v1.9.1")
p("The accepted state ships as <b>v1.9.1</b> — a patch bump: v1.9.0 was tagged and pushed at "
  "the R4 STOP, before the owner's G4 rejection produced the fix wave (RS-4-1..11), so the "
  "fixes live first in v1.9.1; the presentation layer changed, the feature surface did not. "
  "The workspace version is bumped, the CHANGELOG carries the release notes (the section "
  "below is read from CHANGELOG.md at report-build time), and the annotated tag v1.9.1 is "
  "cut from the G4-accepted state. The release workflow builds the AppImage and the .deb on "
  "ubuntu:22.04 (glibc 2.35 baseline), verifies them in clean containers, and publishes with "
  "checksums — composing the GitHub release notes from this same CHANGELOG section.")
h2("7.1 CHANGELOG — the [1.9.1] section (verbatim)")
_changelog = os.path.join(ROOT, "CHANGELOG.md")
_section = []
if os.path.exists(_changelog):
    _txt = open(_changelog, encoding="utf-8").read()
    _m = re.search(r"^## \[1\.9\.1\].*?(?=^## \[|\Z)", _txt, re.M | re.S)
    if _m:
        _section = [ln for ln in _m.group(0).strip().splitlines()]
if _section:
    code("\n".join(_section))
else:
    quote("The [1.9.0] section was not found in CHANGELOG.md at build time — regenerate this "
          "report after the version bump lands.")

# ================= 8. G4 ACCEPTANCE RECORD =================
h1(8, "GATE G4 — Owner Final Acceptance")
p("Gate G4 is the owner's verdict on the reskin program. The package presented: this report; the "
  "traceability matrix (machine-gated in CI); the blind-test protocol and the SUS instrument "
  "(results recorded below); the CI snapshot locks (24 suites / 175 tests — 173 kittest + 2 "
  "style lint — over 171 goldens); and the accepted state released as v1.9.1 with checksums "
  "(v1.9.0, tagged at the RS-3 STOP, is superseded by the fix release).")
table(
    ["Record", "Value"],
    [
        ["Owner acceptance (accept / accept-with-notes / reject)", "accept — GATE G4 closed 2026-10-30"],
        ["Blind test aggregate (§4.2)", "5 /10 — pre-registered bar not met (2 false-AI); disclosed and owner-accepted"],
        ["SUS mean, n (§5)", "85.0 (n = 1)"],
        ["Notes / conditions", "Verdict given with the blind-test miss in view: the false-AI surfaces (amber internal-note card; Ada Lovelace agent reply in page context) stand as disclosed limitations; SUS 85.0 clears the ≥ 68 bar."],
        ["Date", "2026-10-30"],
    ],
    [0.52, 0.48],
    caption="The completed record is folded back into this report (regenerate via "
            "docs/audit/generate_acceptance_report.py) and the item closes in completed.md.")

# ============================ BUILD ============================
def _decor(canvas, doc):
    canvas.saveState()
    canvas.setFillColor(PAGE_BG)
    canvas.rect(0, 0, PAGE_W, PAGE_H, stroke=0, fill=1)
    canvas.restoreState()
    canvas.saveState()
    canvas.setFont("FreeSerif", 7.5)
    canvas.setFillColor(TEXT_MUTED)
    canvas.drawString(MARGIN, PAGE_H - 0.55 * inch, DOC_TITLE)
    canvas.setStrokeColor(ACCENT); canvas.setLineWidth(1.2)
    canvas.line(MARGIN, PAGE_H - 0.62 * inch, PAGE_W - MARGIN, PAGE_H - 0.62 * inch)
    canvas.setStrokeColor(BORDER); canvas.setLineWidth(0.5)
    canvas.line(MARGIN, 0.62 * inch, PAGE_W - MARGIN, 0.62 * inch)
    canvas.setFont("FreeSerif", 7.5)
    canvas.drawString(MARGIN, 0.45 * inch, DOC_AUTHOR)
    canvas.drawRightString(PAGE_W - MARGIN, 0.45 * inch, f"Page {doc.page}")
    canvas.restoreState()

doc = TocDocTemplate(OUT, pagesize=A4,
                     leftMargin=MARGIN, rightMargin=MARGIN,
                     topMargin=MARGIN, bottomMargin=MARGIN,
                     title=DOC_TITLE, author=DOC_AUTHOR, creator="Z.ai",
                     subject="R4 final acceptance record of the Design-System reskin (RS-21)")
doc.multiBuild(story, onFirstPage=_decor, onLaterPages=_decor)
print("ACCEPTANCE_REPORT_OK", OUT)
