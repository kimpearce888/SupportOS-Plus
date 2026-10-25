#!/usr/bin/env python3
"""FIND-011 supplemental VIDEO evidence (audit tooling, NOT app code).

Instruction 03 §6 warrants screen video for intermittent behavior —
FIND-011 (palette click-outside dismissal unreliable, position-dependent,
with fall-through to the UI underneath) is exactly that case. This script
records the live palette dismissal matrix as an mp4 with per-action
timestamps, so the intermittency and the fall-through are visible as a
sequence, complementing the W-008 still matrix (E-081..E-086).

Sequence recorded (each action logged with its video timestamp):
  open palette (header 'Search ⌘K' at (1059,26) — the W-008 position)
  → outside click (600,500)  [fall-through zone]  → STILL OPEN
  → outside click (950,120)  [header zone]        → STILL OPEN
  → click (300,300)          [working region]     → CLOSED
  → reopen → outside click (450,620) [fall-through, diff 0.081 in W-008]
  → STILL OPEN → click (300,300) → CLOSED
Run: bash start-xvfb.sh && python3 w011-palette-video.py
"""
import json
import os
import shutil
import subprocess
import sys
import time

from PIL import Image

REPO = "/home/z/my-project/audit/SupportOS-Plus"
AUDIT = os.path.join(REPO, "repo-audit-2026-10-24")
VID_DIR = os.path.join(AUDIT, "evidence/videos")
APP_LOG = "/tmp/w011-app.log"
LOG_DST = os.path.join(AUDIT, "evidence/logs/W011-app.log")
EVENTS = os.path.join(AUDIT, "evidence/artifacts/control-verify-events.jsonl")
VIDEO = os.path.join(VID_DIR, "W-011-palette-dismissal.mp4")
SHOT_W, SHOT_H = 1600, 1000
OPEN_POS = (1059, 26)

events = open(EVENTS, "a")


def ev(kind, **kw):
    rec = {"t": round(time.time(), 3), "kind": kind, **kw}
    events.write(json.dumps(rec) + "\n")
    events.flush()
    print(json.dumps(rec), flush=True)


def finish(code):
    events.close()
    if os.path.exists(APP_LOG):
        shutil.copy(APP_LOG, LOG_DST)
    sys.exit(code)


def die(msg):
    ev("w011-fatal", error=msg)
    finish(3)


# --- lean XTEST + capture stack (verbatim patterns from the proven scripts)
from Xlib import X, display  # noqa: E402
from Xlib.ext import xtest  # noqa: E402

d = display.Display(":99")


def move(x, y):
    xtest.fake_input(d, X.MotionNotify, x=int(x), y=int(y))
    d.sync()
    time.sleep(0.06)


def click(x, y):
    move(x, y)
    xtest.fake_input(d, X.ButtonPress, 1)
    d.sync()
    time.sleep(0.04)
    xtest.fake_input(d, X.ButtonRelease, 1)
    d.sync()


def shot(path=None):
    tmp = path or "/tmp/w011-shot.png"
    subprocess.run(
        ["ffmpeg", "-y", "-loglevel", "error", "-f", "x11grab",
         "-video_size", f"{SHOT_W}x{SHOT_H}", "-i", ":99", "-frames:v", "1",
         tmp],
        check=True)
    im = Image.open(tmp)
    im.load()
    return im


def wait_proc_gone(timeout=20):
    deadline = time.time() + timeout
    while time.time() < deadline:
        r = subprocess.run(["pgrep", "-f", "target/debug/supportos-plus"],
                           capture_output=True)
        if r.returncode != 0:
            return True
        time.sleep(1)
    return False


def start_app(tag):
    subprocess.run(["pkill", "-f", "target/debug/supportos-plus"], check=False)
    if not wait_proc_gone():
        die(f"prior instance would not die before {tag}")
    time.sleep(1)
    logf = open(APP_LOG, "a")
    logf.write(f"\n===== W-011 video {tag} =====\n")
    p = subprocess.Popen(["./run-app.sh"], stdout=logf,
                         stderr=subprocess.STDOUT, cwd=AUDIT,
                         start_new_session=True)
    deadline = time.time() + 45
    ready = False
    while time.time() < deadline:
        time.sleep(1)
        try:
            tail = open(APP_LOG, errors="replace").read().split(
                f"===== W-011 video {tag} =====")[-1]
        except FileNotFoundError:
            continue
        if "cannot acquire the instance lock" in tail:
            die("instance lock error on launch")
        if "migrations" in tail or "loopback" in tail:
            ready = True
            break
    time.sleep(4 if ready else 8)
    if not ready:
        die(f"app failed to start for {tag}")
    ev("w011-app-started", tag=tag, pid=p.pid, ready=True)
    return p


# --- palette open/close verification (diff + OCR-free, matching W-008)
import io  # noqa: E402
from PIL import ImageChops  # noqa: E402


def diff_mean(a, b, box=(230, 60, 1150, 700)):
    hist = ImageChops.difference(
        a.convert("L").crop(box), b.convert("L").crop(box)).histogram()
    total = sum(hist)
    return round(sum(i * c for i, c in enumerate(hist)) / max(total, 1), 3)


def palette_open(frame, closed_ref):
    """The palette overlay changes the content box substantially when it
    opens (W-008 diff 1.69) and restores the baseline when it closes."""
    return diff_mean(closed_ref, frame) > 0.5


def main():
    os.makedirs(VID_DIR, exist_ok=True)
    p = start_app("launch-1")
    time.sleep(2)
    closed_ref = shot()
    click(*OPEN_POS)
    time.sleep(1.0)
    opened = shot()
    if not palette_open(opened, closed_ref):
        die(f"palette did not open via header button {OPEN_POS}")
    ev("w011-palette-opened", at=list(OPEN_POS),
       diff=diff_mean(closed_ref, opened))

    # ---- start the video
    if os.path.exists(VIDEO):
        os.remove(VIDEO)
    t0 = time.time()
    ff = subprocess.Popen(
        ["ffmpeg", "-y", "-loglevel", "error", "-f", "x11grab",
         "-video_size", f"{SHOT_W}x{SHOT_H}", "-i", ":99", "-r", "10",
         "-c:v", "libx264", "-preset", "veryfast", "-pix_fmt", "yuv420p",
         VIDEO],
        cwd=AUDIT)
    time.sleep(1.0)

    def act(label, pos, expect):
        ts = round(time.time() - t0, 2)
        click(*pos)
        time.sleep(1.3)
        frame = shot()
        is_open = palette_open(frame, closed_ref)
        ok = is_open == expect
        ev("w011-action", label=label, at=list(pos), video_ts_s=ts,
           palette_open_after=is_open, expected_open=expect,
           verdict="PASS" if ok else "FAIL")
        return ok, frame, ts

    results = []
    results.append(act("outside-click-fallthrough-zone", (600, 500), True))
    results.append(act("outside-click-header-zone", (950, 120), True))
    results.append(act("working-region-click-closes", (300, 300), False))
    ts = round(time.time() - t0, 2)
    click(*OPEN_POS)
    time.sleep(1.2)
    reopened = shot()
    ok_open = palette_open(reopened, closed_ref)
    ev("w011-action", label="reopen-via-header", at=list(OPEN_POS),
       video_ts_s=ts, palette_open_after=ok_open, expected_open=True,
       verdict="PASS" if ok_open else "FAIL")
    results.append((ok_open, reopened, ts))
    results.append(act("outside-click-metrics-zone", (450, 620), True))
    results.append(act("working-region-click-closes-2", (300, 300), False))

    # ---- stop the video
    time.sleep(1.0)
    ff.terminate()
    try:
        ff.wait(timeout=15)
    except subprocess.TimeoutExpired:
        ff.kill()
    duration = round(time.time() - t0, 2)

    all_ok = all(r[0] for r in results)
    ev("w011-video-stopped", path=VIDEO, duration_s=duration,
       actions=len(results), all_actions_pass=all_ok)

    # ---- validate the video + extract representative frames
    probe = subprocess.run(
        ["ffprobe", "-v", "error", "-show_entries",
         "format=duration,size:stream=nb_frames,codec_name",
         "-of", "json", VIDEO],
        capture_output=True, text=True)
    info = json.loads(probe.stdout) if probe.returncode == 0 else {}
    ev("w011-video-validated", ffprobe=info,
       verdict="PASS" if info.get("format", {}).get("duration") else "FAIL")

    for name, ss in (("frame-open-fallthrough", results[0][2] + 0.9),
                     ("frame-still-open-second-click", results[1][2] + 0.9)):
        fp = os.path.join(VID_DIR, f"W-011-{name}.png")
        subprocess.run(
            ["ffmpeg", "-y", "-loglevel", "error", "-ss", str(ss), "-i",
             VIDEO, "-frames:v", "1", fp],
            check=True)
        im = Image.open(fp)
        im.load()
        ev("w011-frame-extracted", path=fp, at_s=ss, size=list(im.size))

    p.terminate()
    time.sleep(2)
    gone = wait_proc_gone()
    ev("w011-summary", verdict="PASS" if (all_ok and gone) else "FAIL",
       note="FIND-011 intermittency + fall-through recorded as video with "
            "per-action timestamps; complements the W-008 still matrix")
    finish(0)


if __name__ == "__main__":
    try:
        main()
    except SystemExit:
        raise
    except Exception as e:  # noqa: BLE001
        die(f"unhandled: {type(e).__name__}: {e}")
