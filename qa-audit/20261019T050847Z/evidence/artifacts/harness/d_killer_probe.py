#!/usr/bin/env python3
"""Diagnose the 49.25s killer: boot, then heartbeat grabs + child health."""
import sys, time, subprocess
from pathlib import Path
sys.path.insert(0, str(Path(__file__).parent))
from harness import Session

s = Session("D-killer-probe", record=True)
t0 = time.time()
try:
    s.start()
    print(f"[{time.time()-t0:6.1f}s] booted, window={s.win_geo}", flush=True)
    for i in range(60):
        time.sleep(2)
        alive_x = s._xvfb.poll() is None
        alive_a = s._app.poll() is None
        alive_r = (s._rec.poll() is None) if s._rec else None
        try:
            arr = s.grab()
            gl = f"grab ok mean={arr.mean():.0f}"
        except Exception as e:
            gl = f"GRAB FAILED: {type(e).__name__} {e}"
        print(f"[{time.time()-t0:6.1f}s] hb{i:02d} xvfb={alive_x} app={alive_a} "
              f"rec={alive_r} {gl}", flush=True)
        if not (alive_x and alive_a):
            print("A CHILD DIED - stopping experiment", flush=True)
            break
        if "FAILED" in gl:
            print("GRAB BROKEN - stopping experiment", flush=True)
            break
finally:
    s.stop()
    s.save_results()
print("probe done", flush=True)
