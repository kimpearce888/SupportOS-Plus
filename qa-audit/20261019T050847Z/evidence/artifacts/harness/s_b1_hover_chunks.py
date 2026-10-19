#!/usr/bin/env python3
"""S-B1 hover scan, CHUNKED: the sandbox kills Xvfb during sustained input
injection (~50s observed), so the y-range is split across short sessions.

Each chunk: fresh boot -> neutral rest frame -> 2px-step pointer sweep with
cursor-masked sidebar-region diffing -> per-chunk JSON. Bands are merged
post-hoc by the analysis step.

Detection: egui paints a hover tint over the hovered row (calibrated from
prior-session evidence: band x 8..88, ~19px tall). The probe pointer's X
cursor sprite (x 34..52 at probe x=40) is masked out; the rest cursor sits
outside the scan region.
"""
import sys, time
from pathlib import Path
sys.path.insert(0, str(Path(__file__).parent))
import numpy as np
from PIL import Image
from harness import Session, EVID

SIDEBAR_X = 40
NEUTRAL = (700, 750)
REGION = (0, 28, 200, 640)     # x0, y0, x1, y1 — sidebar scan region
CURSOR_MASK = (26, 56)         # x-range masked around the probe pointer
TOL = 8

CHUNKS = [(28, 232), (232, 432), (432, 624)]
# which chunk to run: argv[1]
CI = int(sys.argv[1]) if len(sys.argv) > 1 else 0
YA, YB = CHUNKS[CI]


def hover_count(rest, shot):
    """(pixel_count, bbox) of hover tint in the sidebar, cursor column masked."""
    x0, y0, x1, y1 = REGION
    cm0, cm1 = CURSOR_MASK
    d = (np.abs(shot.astype(np.int16) - rest.astype(np.int16)) > TOL).any(axis=2)
    sub = d[y0:y1, x0:x1].copy()
    sub[:, cm0 - x0:cm1 - x0] = False
    n = int(sub.sum())
    bbox = None
    if n:
        ys, xs = np.where(sub)
        bbox = [int(xs.min()) + x0, int(ys.min()) + y0,
                int(xs.max()) + x0, int(ys.max()) + y0]
    return n, bbox


def main():
    s = Session(f"B1-hover-chunk{CI}")
    try:
        s.start()
        s.move(*NEUTRAL)
        rest = s.grab()
        Image.fromarray(rest).save(
            EVID / "screenshots" / f"B1-hover-chunk{CI}-rest.png")
        probes = []
        t_start = time.time()
        cur_band_key = None
        for y in range(YA, YB, 2):
            s.move(SIDEBAR_X, y)
            shot = s.grab()
            n, bbox = hover_count(rest, shot)
            probes.append({"y": y, "hp": n, "bbox": bbox})
            s.results["probes"] = probes      # incremental: watchdog flushes this
            # on entering a new hover band, persist one visual-evidence frame
            band_key = None if n < 30 else (bbox[1] // 20)
            if band_key is not None and band_key != cur_band_key:
                Image.fromarray(shot).save(
                    EVID / "screenshots" / f"B1-hover-chunk{CI}-y{y}.png")
                cur_band_key = band_key
        s.results["chunk"] = [YA, YB]
        s.note(step="chunk-done", probes=len(probes),
               elapsed=round(time.time() - t_start, 1))
    finally:
        s.stop()
        s.save_results()
    print(f"CHUNK{CI} DONE: {len(probes)} probes, "
          f"hover>30 at {[p['y'] for p in probes if p['hp'] > 30][:40]}")


if __name__ == "__main__":
    main()
