#!/usr/bin/env python3
"""K1: keyboard-channel discrimination with explicit X input focus.

Prior evidence: A23 == A24 == A25 pixel-identical — XTEST keystrokes never
reached the app (bare Xvfb, no WM to establish keyboard focus). This session
explicitly sets input focus via python-xlib, then:

  1. open the command palette via the sidebar "⌘K search" button (mouse)
  2. click into the palette text input
  3. type "inbox"  — expect the typed text to render and the palette hint to
     switch from "Type to search..." to "(no hits)" (fresh DB: no data)
  4. press Escape  — expect the palette to close (toggle_shortcut)

If step 3/4 produce visible changes, the keyboard channel works and the
earlier A-series palette result is reclassified from "possible defect" to
"environment focus artifact".
"""
import sys, time
from pathlib import Path
sys.path.insert(0, str(Path(__file__).parent))
from harness import Session
import Xlib.X
import Xlib.display

s = Session("K1-keyboard-focus", record=True)
try:
    s.start()

    # ---- explicit input focus on the app window (no WM to do it) ----
    d = Xlib.display.Display(s.display)
    root = d.screen().root
    win = None
    for w in root.query_tree().children:
        try:
            name = w.get_wm_name()
            if name and "SupportOs-Plus" in name:
                win = w
                break
        except Exception:
            continue
    if win is None:
        s.note(step="focus-failed", error="window not found")
    else:
        win.set_input_focus(Xlib.X.RevertToParent, Xlib.X.CurrentTime)
        d.sync()
        s.note(step="focus-set", win_name=win.get_wm_name())
    d.close()
    base = s.screenshot("K1-00-base")

    # ---- open the palette via the sidebar button ----
    # nav rows end ~y535 (Shell center 523), separator, then the button.
    opened = False
    for attempt, y in enumerate((558, 575, 545)):
        s.click(30, y)
        time.sleep(0.5)
        shot = s.screenshot(f"K1-10-palette-click-{y}")
        diff = s.diff(base, shot)
        s.note(step="palette-click", y=y, diff=round(diff, 4), attempt=attempt)
        if diff > 0.02:          # palette overlay dims the screen: expect ~3%
            opened = True
            break
    if not opened:
        s.note(step="palette-open-failed", hint="button position not found")

    # ---- click into the text input (palette window x360-920, input y~105-135) ----
    s.click(640, 120)
    time.sleep(0.3)
    clicked = s.screenshot("K1-20-input-clicked")

    # ---- type the query ----
    s.type("inbox")
    time.sleep(0.5)
    typed = s.screenshot("K1-30-typed")
    d_typed = s.diff(clicked, typed)
    s.note(step="typed", diff=round(d_typed, 4))

    # ---- Escape closes the palette (toggle_shortcut) ----
    s.press("escape")
    time.sleep(0.4)
    esc = s.screenshot("K1-40-escape")
    d_esc = s.diff(typed, esc)
    s.note(step="escape", diff=round(d_esc, 4))
finally:
    s.stop()
    s.save_results()
print("K1 done")
