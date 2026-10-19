#!/usr/bin/env python3
"""F1: post-audit fix verification (DEFECT-01, DEFECT-03) on the fixed build.

Pre-fix state (audit evidence, qa-audit/20261019T050847Z):
  - DEFECT-01: Inbox/Reports/Search/Shell rows (the four shortest labels)
    ignored clicks at x >= ~20 while hover painted the label; only the
    far-left sliver (x <= ~20) navigated (nav-sweep-A x=30, hitbox-scan
    x=40, K3 dead-row xsweep x=45 all inert).
  - DEFECT-03: the palette opened without keyboard focus; typing produced
    a 0-pixel change until the input was clicked (K1/K3 evidence).

Fixed build: sidebar rows are hand-allocated full-width (the sensed rect
is exactly the painted row), and the palette focuses the query field on
open. Probes:

  A. Ctrl+K then TYPE "inbox" with NO click into the field.
     PASS = typed glyphs render (diff > 0 with the bbox inside the
     palette input area) — pre-fix this produced a 0-pixel change.
  B. Escape closes the palette (K1 regression check).
  C. Click every DEFECT-01 row at x=75 — deep inside the old dead zone,
     far beyond every label text. Projected new row centers are
     65 + 25*(row-1) (22px row + 3px item spacing); retried +/-8px.
     PASS = navigation diff >= 0.9% (the audit's threshold).
  D. Click the Inbox row at x=130 (panel far right; never clickable
     pre-fix, far beyond the old label-hugging rects).
  E. Regression: three always-working rows at x=75 must still navigate.
"""
import json
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from harness import Session
import Xlib.X
import Xlib.display

TARGETS = [("Inbox", 2), ("Reports", 7), ("Search", 15), ("Shell", 21)]
CONTROLS = [("Organizations", 5), ("Knowledge", 6), ("Settings", 19)]
NAV_THRESHOLD = 0.009          # the audit's navigation-diff threshold

# palette input rect (K3): x310-597 y115-146; hint line renders below it
BBOX_IN = (280, 620, 95, 260)  # (x0_max, x1_max, y0_max, y1_max) bounds


def row_center(index: int) -> int:
    """Projected center of a nav row in the fixed layout."""
    return 65 + 25 * (index - 1)


s = Session("F1-fix-verify", record=True)
verdict = {"palette_focus": None, "escape_close": None, "rows": {}, "x130": None}
try:
    s.start()
    cur = s.screenshot("F1-00-base")
    s.note(step="boot", window=str(s.win_geo))

    # --- keyboard channel (bare Xvfb has no WM) -------------------------
    d = Xlib.display.Display(s.display)
    root = d.screen().root
    for w in root.query_tree().children:
        try:
            if "SupportOs-Plus" in (w.get_wm_name() or ""):
                w.set_input_focus(Xlib.X.RevertToParent, Xlib.X.CurrentTime)
                d.sync()
                s.note(step="x-focus-set")
                break
        except Exception:
            continue
    d.close()

    # --- A. DEFECT-03: palette opens focused; typing lands with no click
    s.hotkey("ctrl", "k")
    time.sleep(0.8)
    p_open = s.screenshot("F1-10-palette-open")
    s.note(step="palette-open", diff=round(s.diff(cur, p_open), 4))

    s.type("inbox")
    time.sleep(0.6)
    p_typed = s.screenshot("F1-20-palette-typed-noclick")
    typed_diff = s.diff(p_open, p_typed, tol=0)
    bbox = s.diff_bbox(p_open, p_typed, tol=0)
    s.note(step="palette-typed-noclick",
           diff=round(typed_diff, 4), bbox=str(bbox))
    ok = (typed_diff > 0.0 and bbox is not None
          and bbox[0] >= BBOX_IN[0] and bbox[2] <= BBOX_IN[1]
          and bbox[1] >= BBOX_IN[2] and bbox[3] <= BBOX_IN[3])
    verdict["palette_focus"] = bool(ok)
    s.note(step="defect03-verdict", passed=bool(ok))

    # --- B. Escape must close (K1 regression) ---------------------------
    s.press("escape")
    time.sleep(0.5)
    p_esc = s.screenshot("F1-30-palette-escape")
    esc_diff = s.diff(p_typed, p_esc)
    verdict["escape_close"] = esc_diff > 0.01
    s.note(step="escape-close", diff=round(esc_diff, 4))
    cur = p_esc

    # --- C/E. row clicks at x=75 (old dead zone) ------------------------
    for name, index in TARGETS + CONTROLS:
        y0 = row_center(index)
        hit = None
        for attempt, y in enumerate((y0, y0 - 8, y0 + 8)):
            s.click(75, y)
            time.sleep(0.45)
            shot = s.screenshot(f"F1-nav-{name}-x75-{attempt}")
            dnav = s.diff(cur, shot)
            s.note(step="nav", row=name, x=75, y=y,
                   diff=round(dnav, 4))
            cur = shot
            if dnav >= NAV_THRESHOLD:
                hit = (y, round(dnav, 4))
                break
        verdict["rows"][name] = hit
        s.note(step="nav-verdict", row=name, hit=str(hit))

    # --- D. far-right probe on the Inbox row -----------------------------
    y_inbox = (verdict["rows"]["Inbox"][0] if verdict["rows"]["Inbox"]
               else row_center(2))
    for attempt, y in enumerate((y_inbox, y_inbox - 8, y_inbox + 8)):
        s.click(130, y)
        time.sleep(0.45)
        shot = s.screenshot(f"F1-nav-Inbox-x130-{attempt}")
        dnav = s.diff(cur, shot)
        s.note(step="nav", row="Inbox", x=130, y=y, diff=round(dnav, 4))
        cur = shot
        if dnav >= NAV_THRESHOLD:
            verdict["x130"] = (y, round(dnav, 4))
            break

    # --- verdict ----------------------------------------------------------
    verdict["pass"] = (
        verdict["palette_focus"]
        and verdict["escape_close"]
        and verdict["x130"] is not None
        and all(verdict["rows"][name] for name, _ in TARGETS)
        and all(verdict["rows"][name] for name, _ in CONTROLS)
    )
finally:
    s.stop()
    s.save_results()
    vpath = Path(__file__).resolve().parents[1] / "F1-fix-verify-verdict.json"
    vpath.write_text(json.dumps(verdict, indent=1))
    print("VERDICT", json.dumps(verdict))
print("F1 done")
