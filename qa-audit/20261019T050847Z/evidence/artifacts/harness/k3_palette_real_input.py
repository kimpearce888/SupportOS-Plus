#!/usr/bin/env python3
"""K3: palette input — REAL box geometry (x310-597, y115-146) + dead-row x-sweep.

K1/K2 clicked (640,*): 43px RIGHT of the input box's true right edge (597)
— egui's text_edit_singleline sizes to ~half the window width, not full.
This session clicks the true center (450, 130) with X input focus set:

  1. XSetInputFocus on the app window (keyboard channel — K1 proven)
  2. open palette via the sidebar button (30, 558)
  3. click input center (450, 130) -> caret should appear
  4. type "inbox" -> glyphs + search hint flip to "(no hits)" on empty DB
  5. press Enter   -> per source, Enter needs lost_focus() (web-port artifact)
     — expected NOT to navigate; evidence for the ported-defect note
  6. press Escape  -> palette must close
  7. dead-row x-sweep: click Inbox/Reports/Search/Shell rows at x=10 and
     x=45, y=row centers (87/201/386/523) — narrow-hitbox hypothesis
"""
import sys, time
from pathlib import Path
sys.path.insert(0, str(Path(__file__).parent))
from harness import Session
import Xlib.X
import Xlib.display

DEAD_ROWS = [("Inbox", 87), ("Reports", 201), ("Search", 386), ("Shell", 523)]

s = Session("K3-palette-real-input", record=True)
try:
    s.start()
    base = s.screenshot("K3-00-base")
    s.note(step="boot", window=str(s.win_geo))

    # keyboard channel
    d = Xlib.display.Display(s.display)
    root = d.screen().root
    for w in root.query_tree().children:
        try:
            if "SupportOs-Plus" in (w.get_wm_name() or ""):
                w.set_input_focus(Xlib.X.RevertToParent, Xlib.X.CurrentTime)
                d.sync()
                s.note(step="focus-set")
                break
        except Exception:
            continue
    d.close()

    # open palette
    s.click(30, 558)
    time.sleep(0.5)
    p0 = s.screenshot("K3-10-palette-open")
    s.note(step="palette-open", diff=s.diff(base, p0))

    # click the TRUE input center
    s.click(450, 130)
    time.sleep(0.3)
    p1 = s.screenshot("K3-20-input-clicked")
    s.note(step="input-click", focus_diff=s.diff(p0, p1, tol=0))

    # type
    s.type("inbox")
    time.sleep(0.5)
    p2 = s.screenshot("K3-30-typed")
    s.note(step="typed", typed_diff=s.diff(p1, p2, tol=0))

    # Enter (web-port: requires lost_focus -> expected no-op)
    s.press("enter")
    time.sleep(0.5)
    p3 = s.screenshot("K3-40-enter")
    s.note(step="enter", enter_diff=s.diff(p2, p3, tol=0))

    # Escape closes
    s.press("escape")
    time.sleep(0.4)
    p4 = s.screenshot("K3-50-escape")
    s.note(step="escape", close_diff=s.diff(p3, p4, tol=0))

    # dead-row x-sweep (page must be Dashboard; nav success = big content diff)
    cur = p4
    results = []
    for name, y in DEAD_ROWS:
        for x in (10, 45):
            s.click(x, y)
            time.sleep(0.45)
            shot = s.screenshot(f"K3-60-dead-{name}-{x}")
            dnav = s.diff(cur, shot)
            results.append({"row": name, "y": y, "x": x, "nav_diff": round(dnav, 4)})
            cur = shot
    s.note(step="dead-row-xsweep", probes=results)
finally:
    s.stop()
    s.save_results()
print("K3 done")
