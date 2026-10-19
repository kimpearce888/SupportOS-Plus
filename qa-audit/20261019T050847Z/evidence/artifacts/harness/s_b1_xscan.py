#!/usr/bin/env python3
"""S-B1x: map the horizontal (x) extent of each sidebar row's hover senser.

For a set of representative rows (short and long labels), sweep the pointer
across the sidebar at the row's band center and record whether the row
highlights. Session chunked: 2-3 rows per session (sandbox kills sustained
input after ~50s).
"""
import sys, time
from pathlib import Path
sys.path.insert(0, str(Path(__file__).parent))
import numpy as np
from PIL import Image
from harness import Session, EVID

NEUTRAL = (700, 750)
REGION = (0, 28, 200, 640)
CURSOR_COL = (18, 72)      # WIDE mask: probe cursor sits at the swept x —
                           # mask the whole swept band from detection
TOL = 8

# row -> band y-center (from the B1 hover band map)
ROWS = {
    "Dashboard":     60,
    "Inbox":         86,
    "Operations":   108,
    "Reports":      200,
    "Search":       386,
    "Support Graph": 408,
    "Shell":        522,
}
# chunk id -> rows
CHUNKS = [["Dashboard", "Inbox", "Reports"],
          ["Search", "Support Graph", "Shell"]]
CI = int(sys.argv[1]) if len(sys.argv) > 1 else 0


def hover_count(rest, shot):
    x0, y0, x1, y1 = REGION
    cm0, cm1 = CURSOR_COL
    d = (np.abs(shot.astype(np.int16) - rest.astype(np.int16)) > TOL).any(axis=2)
    sub = d[y0:y1, x0:x1].copy()
    sub[:, cm0 - x0:cm1 - x0] = False      # mask the swept cursor column
    return int(sub.sum())


def main():
    rows = CHUNKS[CI]
    s = Session(f"B1-xscan-chunk{CI}")
    try:
        s.start()
        results = {}
        for row in rows:
            yc = ROWS[row]
            s.move(*NEUTRAL)
            rest = s.grab()
            probes = []
            for x in range(10, 148, 6):
                s.move(x, yc)
                shot = s.grab()
                n = hover_count(rest, shot)
                probes.append({"x": x, "hp": n})
            results[row] = probes
            s.results["xscan"] = results
            lit = [p["x"] for p in probes if p["hp"] > 30]
            print(f"{row}: lit_x={lit}", flush=True)
        s.note(step="xscan-done", rows=rows)
    finally:
        s.stop()
        s.save_results()
    print("XSCAN CHUNK", CI, "DONE")


if __name__ == "__main__":
    main()
