#!/usr/bin/env python3
"""Finalize the SupportOS+ Developer Handover Book PDF (two-pass TOC + page-number stamping).
Chapter pages are located by title text at chapter-title font size (>18pt),
which cannot collide with TOC entries (9.9pt) or body prose (11.6pt)."""
import re, subprocess, sys, os
import fitz  # pymupdf
from pypdf import PdfReader, PdfWriter
from reportlab.pdfgen import canvas
from reportlab.lib.colors import HexColor
from io import BytesIO

HERE = os.path.dirname(os.path.abspath(__file__))
SRC = os.path.join(HERE, "SupportOS-Plus-Developer-Handover.html")
PDF_SKILL = "/home/z/my-project/skills/pdf"
W_PT, H_PT = 540.0, 765.0  # 720x1020 px at 0.75 pt/px

TITLES = {
    1: "What this book is",
    2: "The project in one sitting",
    3: "Four crates, one-way ladder",
    4: "The boot sequence",
    5: "Threads and the event bus",
    6: "The database layer",
    7: "Migrations and schema law",
    8: "Errors, panics and toasts",
    9: "Storage, config, single instance",
    10: "The sandbox",
    11: "Secrets and the envelope",
    12: "Backups and restore",
    13: "Bundles, resets, secure delete",
    14: "Webhooks and OAuth",
    15: "Untrusted content",
    16: "Local-first AI",
    17: "Five roles and the golden set",
    18: "Guardrails",
    19: "The read-only agent",
    20: "Vectors and hybrid search",
    21: "The ledger and the prompts",
    22: "Automation",
    23: "SLA and workload",
    24: "Reports and segments",
    25: "Outreach and incidents",
    26: "The daily build",
    27: "Packaging",
    28: "The release pipeline",
    29: "The history rewrite",
    30: "The laws of the codebase",
    31: "Sharp edges A–Z",
    32: "Known trade-offs, open ends",
}

def render(html_path, out_pdf):
    cmd = ["node", os.path.join(PDF_SKILL, "scripts", "html2pdf-next.js"),
           html_path, "--output", out_pdf, "--width", "720px", "--height", "1020px",
           "--title", "SupportOS+ The Handover Book — Why the code is the way it is (v1.8.0)"]
    r = subprocess.run(cmd, cwd=HERE, capture_output=True, text=True, timeout=420)
    tail = "\n".join((r.stdout + r.stderr).splitlines()[-6:])
    if not os.path.exists(out_pdf):
        print(tail); sys.exit(f"render failed for {html_path}")
    print(f"  rendered {os.path.basename(out_pdf)}")

def chapter_pages(pdf_path):
    """Map chapter n -> 1-based pdf page: title line rendered at chapter-title size."""
    pages = {}
    with fitz.open(pdf_path) as doc:
        for i, page in enumerate(doc, start=1):
            d = page.get_text("dict")
            for block in d.get("blocks", []):
                block_lines = []
                for line in block.get("lines", []):
                    text = "".join(s["text"] for s in line["spans"]).strip()
                    maxsize = max((s["size"] for s in line["spans"]), default=0)
                    if maxsize >= 18 and text:
                        block_lines.append(text)
                if not block_lines:
                    continue
                joined = " ".join(block_lines)
                for n, title in TITLES.items():
                    if n not in pages and (joined == title or block_lines[0] == title):
                        pages[n] = i
    return pages

def fill_toc(html, mapping):
    def repl(m):
        n = int(m.group(1))
        num = mapping.get(n)
        return f'<span class="toc-pg" id="pg-{n}">{num if num is not None else "&middot;"}</span>'
    return re.sub(r'<span class="toc-pg" id="pg-(\d+)">.*?</span>', repl, html)

def roman(n):
    vals = [(10, 'x'), (9, 'ix'), (5, 'v'), (4, 'iv'), (1, 'i')]
    out = ""
    while n:
        for v, s in vals:
            if n >= v:
                out += s; n -= v; break
    return out

def stamp(src_pdf, dst_pdf, body_start):
    reader = PdfReader(src_pdf)
    writer = PdfWriter()
    n_total = len(reader.pages)
    for idx, page in enumerate(reader.pages, start=1):
        label = None
        if idx == 1 or idx == n_total:
            label = None                      # cover & back cover: unnumbered
        elif idx < body_start:
            label = roman(idx - 1)            # front matter: i, ii, ...
        else:
            label = str(idx - body_start + 1) # body: arabic, resets to 1
        if label:
            buf = BytesIO()
            c = canvas.Canvas(buf, pagesize=(W_PT, H_PT))
            c.setFont("Helvetica", 8.5)
            c.setFillColor(HexColor("#6f665a"))
            c.drawCentredString(W_PT / 2, 18, label)
            c.save(); buf.seek(0)
            page.merge_page(PdfReader(buf).pages[0])
        writer.add_page(page)
    writer.add_metadata({
        "/Title": "SupportOS+ The Handover Book — Why the code is the way it is (v1.8.0)",
        "/Author": "Z.ai",
        "/Subject": "Developer handover for SupportOS+ v1.8.0 — every architectural decision, rejected alternative and sharp edge, with source evidence",
        "/Creator": "Z.ai",
    })
    with open(dst_pdf, "wb") as f:
        writer.write(f)

def main():
    html = open(SRC, encoding="utf-8").read()

    print("Pass 1: rendering with placeholder TOC ...")
    render(SRC, os.path.join(HERE, "handover-pass1.pdf"))
    pages1 = chapter_pages(os.path.join(HERE, "handover-pass1.pdf"))
    if len(pages1) != 32:
        sys.exit(f"chapter detection incomplete: found {sorted(pages1)}")
    body_start = pages1[1]
    mapping = {n: p - (body_start - 1) for n, p in pages1.items()}
    print(f"  body starts at pdf page {body_start}; chapter->printed page: {mapping}")

    print("Pass 2: rendering with real TOC numbers ...")
    final_path = os.path.join(HERE, "handover-final.html")
    open(final_path, "w", encoding="utf-8").write(fill_toc(html, mapping))
    render(final_path, os.path.join(HERE, "handover-nostamp.pdf"))
    pages2 = chapter_pages(os.path.join(HERE, "handover-nostamp.pdf"))
    if pages2 != pages1:
        sys.exit(f"layout drifted between passes: {pages1} vs {pages2}")
    print("  verified: chapter pages identical in both passes")

    print("Stamping page numbers + metadata ...")
    stamp(os.path.join(HERE, "handover-nostamp.pdf"),
          os.path.join(HERE, "SupportOS-Plus-Developer-Handover.pdf"), body_start)
    print("DONE.")

if __name__ == "__main__":
    main()
