#!/usr/bin/env python3
"""K2: palette text-input focus attack — three paths to focus the query field.

K1 evidence: palette opens via button; click at (640,120) left no caret;
typing "inbox" changed 0 pixels; Escape (global key poll) DID work.
The input box visually spans y115-146, x365-915 (white box, 539 white px/row).

Paths tested here, each followed by typing and a pixel diff (tol=0):
  A. single click at the box center (640, 130)
  B. TAB focus traversal (egui moves widget focus on Tab)
  C. double-click at (640, 130)
Escape at the end re-verifies the keyboard channel and closes the palette.
"""
import sys, time
from pathlib import Path
sys.path.insert(0, str(Path(__file__).parent))
from harness import Session

s = Session("K2-palette-input", record=True)
try:
    s.start()
    base = s.screenshot("K2-00-base")
    s.note(step="boot", window=str(s.win_geo))

    # ---- open palette via the sidebar button (known working at y=558) ----
    s.click(30, 558)
    time.sleep(0.5)
    p0 = s.screenshot("K2-10-palette-open")
    s.note(step="palette-open", diff=s.diff(base, p0))

    # ---- path A: single click at input center ----
    s.click(640, 130)
    time.sleep(0.3)
    pa = s.screenshot("K2-20-clickA")
    s.type("a")
    time.sleep(0.3)
    pa2 = s.screenshot("K2-21-typedA")
    s.note(step="path-A-single-click",
           focus_diff=s.diff(p0, pa), typed_diff=s.diff(pa, pa2, tol=0))

    # ---- path B: TAB traversal ----
    s.press("tab")
    time.sleep(0.2)
    pb = s.screenshot("K2-30-tab")
    s.type("b")
    time.sleep(0.3)
    pb2 = s.screenshot("K2-31-typedB")
    s.note(step="path-B-tab",
           tab_diff=s.diff(pa2, pb, tol=0), typed_diff=s.diff(pb, pb2, tol=0))

    # ---- path C: double click ----
    s.dclick(640, 130)
    time.sleep(0.3)
    pc = s.screenshot("K2-40-dclick")
    s.type("c")
    time.sleep(0.3)
    pc2 = s.screenshot("K2-41-typedC")
    s.note(step="path-C-dclick", typed_diff=s.diff(pc, pc2, tol=0))

    # ---- keyboard sanity: Escape closes the palette ----
    s.press("escape")
    time.sleep(0.4)
    pe = s.screenshot("K2-50-escape")
    s.note(step="escape", close_diff=s.diff(pc2, pe))
finally:
    s.stop()
    s.save_results()
print("K2 done")
