#!/usr/bin/env python3
"""Diagnose what the continuous ~936px sidebar diff actually is."""
import sys, time
from pathlib import Path
sys.path.insert(0, str(Path(__file__).parent))
import numpy as np
from PIL import Image
from harness import Session, EVID

s = Session("B1-diag-hover")
try:
    s.start()
    s.move(700, 750)
    rest = s.grab()
    Image.fromarray(rest).save(EVID / "screenshots" / "B1-diag-rest.png")
    for y in (60, 100, 200, 300, 400, 520):
        s.move(40, y)
        shot = s.grab()
        Image.fromarray(shot).save(EVID / "screenshots" / f"B1-diag-y{y}.png")
        d = (np.abs(shot.astype(np.int16) - rest.astype(np.int16)) > 8).any(axis=2)
        sub = d[28:640, 0:200].copy()
        sub[:, 26:56] = False          # mask probe cursor column
        n = int(sub.sum())
        if n:
            ys, xs = np.where(sub)
            # x-distribution in 20px buckets
            buckets = np.histogram(xs, bins=10, range=(0, 200))[0]
            print(f"y={y:4d} n={n:5d} bbox=({xs.min()},{ys.min()+28})-({xs.max()},{ys.max()+28}) "
                  f"xbuckets(20px)={buckets.tolist()}")
        else:
            print(f"y={y:4d} n=0")
finally:
    s.stop()
    s.save_results()
