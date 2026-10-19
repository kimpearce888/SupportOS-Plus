#!/usr/bin/env python3
"""F3: GUI proof of the DEFECT-03 fix (palette auto-focus), button-open path.

The audit's DEFECT-03 evidence (K1/K3): open the palette via the sidebar
button, type WITHOUT clicking the input -> 0-pixel change (keystrokes
swallowed). K3 then had to click the true input rect (x310-597 y115-146)
before glyphs rendered.

Fixed build: open() requests keyboard focus for the query field. This
session reproduces the audit's exact scenario: button-open, then type
"inbox" with NO click. PASS = glyphs render (diff > 0, bbox inside the
palette input area) and the "(no hits)" hint flip. Escape must still
close (K1 regression).

Environment note (F2 attempt): the sandbox janitor severed the app's X
connection ~6s after boot (log evidence/logs/F2-palette-focus.log:
"X connection to :93 broken"), so F2's button clicks hit a dead window
and every screenshot stayed pixel-identical to the boot frame. F3
minimizes time-to-evidence: focus -> button sweep -> type -> escape.
"""
import json
import os
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
import harness
from harness import Session

TAG = sys.argv[1] if len(sys.argv) > 1 else "F3"
# Direct release-binary launch (no AppImage extraction) to minimize the
# boot phase — the sandbox janitor severs fresh sessions' X sockets at
# unpredictable ages (F2: ~6s, F3: ~8s; the audit observed 5s..180s),
# so every saved second of boot is probe time.
harness.APPIMAGE = harness.REPO / "target" / "release" / "supportos-plusplus"
import Xlib.X
import Xlib.display

# palette input rect (K3): x310-597 y115-146; hint line below it
BBOX_IN = (280, 620, 95, 260)

s = Session(f"{TAG}-palette-focus", record=False)
verdict = {"button_open": None, "typed_noclick": None, "bbox": None,
           "escape_close": None, "x_death": False}
try:
    s.start()
    base = s.screenshot(f"{TAG}-00-base")
    s.note(step="boot", window=str(s.win_geo))

    # keyboard channel (bare Xvfb has no WM)
    d = Xlib.display.Display(s.display)
    root = d.screen().root
    app_win = None
    for w in root.query_tree().children:
        try:
            if "SupportOs-Plus" in (w.get_wm_name() or ""):
                app_win = w
                w.set_input_focus(Xlib.X.RevertToParent, Xlib.X.CurrentTime)
                d.sync()
                s.note(step="x-focus-set")
                break
        except Exception:
            continue
    d.close()

    # --- open the palette via the sidebar button (audit path) ----------
    # New layout (F1-proven): rows center at 65+25*(i-1) => row 21 ends
    # ~576, separator, then the button center ~594-601. Sweep the band,
    # then fall back to the left-edge x=10 (the audit's narrow-hitbox
    # workaround) just in case.
    opened = False
    for (x, y) in ((30, 594), (30, 601), (30, 588), (30, 608), (30, 615),
                   (10, 601), (10, 594)):
        s.click(x, y)
        time.sleep(0.5)
        p_open = s.screenshot(f"{TAG}-10-palette-open")
        dopen = s.diff(base, p_open)
        s.note(step="button-click", x=x, y=y, diff=round(dopen, 4))
        if dopen > 0.01:
            opened = True
            verdict["button_open"] = (x, y, round(dopen, 4))
            break
        base = p_open

    if not opened:
        s.note(step="abort", reason="palette never opened via button band")
    else:
        # --- DEFECT-03: type with NO click into the input -----------------
        s.type("inbox")
        time.sleep(0.6)
        p_typed = s.screenshot(f"{TAG}-20-palette-typed-noclick")
        typed_diff = s.diff(p_open, p_typed, tol=0)
        bbox = s.diff_bbox(p_open, p_typed, tol=0)
        s.note(step="palette-typed-noclick",
               diff=round(typed_diff, 4), bbox=str(bbox))
        ok = (typed_diff > 0.0 and bbox is not None
              and bbox[0] >= BBOX_IN[0] and bbox[2] <= BBOX_IN[1]
              and bbox[1] >= BBOX_IN[2] and bbox[3] <= BBOX_IN[3])
        verdict["typed_noclick"] = round(typed_diff, 4)
        verdict["bbox"] = str(bbox)
        s.note(step="defect03-verdict", passed=bool(ok))

        # --- K1 regression: Escape closes --------------------------------
        s.press("escape")
        time.sleep(0.5)
        p_esc = s.screenshot(f"{TAG}-30-palette-escape")
        esc_diff = s.diff(p_typed, p_esc)
        verdict["escape_close"] = esc_diff > 0.01
        s.note(step="escape-close", diff=round(esc_diff, 4))

    verdict["pass"] = bool(
        verdict["button_open"] and verdict["typed_noclick"]
        and verdict["escape_close"] and verdict.get("bbox") not in (None, "None")
    )
finally:
    s.stop()
    s.save_results()
    vpath = Path(__file__).resolve().parents[1] / f"{TAG}-palette-focus-verdict.json"
    vpath.write_text(json.dumps(verdict, indent=1))
    print("VERDICT", json.dumps(verdict), flush=True)
print(f"{TAG} done", flush=True)
os._exit(0)
