#!/usr/bin/env python3
"""W-007 theme round-trip runtime verification (audit tooling, NOT app code).

Same proven real-input stack as the pass-1 walk / pass-2 control verification
(XTEST clicks on Xvfb :99, ffmpeg x11grab frames, pointer parked off controls
for every analyzed frame). Self-contained (no import of control-verify.py so
no __pycache__ is created in the published folder).

What it exercises (per NEXT-ACTIONS #1, WORKFLOW-COVERAGE W-007):
  * header theme icon button (C-APP-006, app.rs:471-485) — icon-only 30x30
    ghost button, rightmost in the header; NEXT-ACTIONS measured rect
    (1130-1160, 19-41) -> click (1145, 30)
  * sidebar bottom theme row (C-APP-005, app.rs:455-465 / theme_row
    app.rs:1052-1105) — full-sidebar-width 30px row; FIND-004 measured rect
    y [714,743] -> click (100, 728) (row paints OVER the Shell nav row)
  * theme.rs claims: click -> toggle + store(&db) + apply(ctx); value lives
    in the application_settings table under key 'theme' ("light"/"dark");
    unknown/missing value falls back to Light; persists across restarts
    (F-026 persistence claim).

Deliberate sequencing (recorded):
  * light -> dark via header button; dark -> light via sidebar row (both
    directions, both controls); dark again via sidebar row (restart prep);
    restart proves dark persisted (settings table + rendered UI);
    header button flips back to light; second restart proves light
    persisted and restores the original precondition (theme unset->light).
  * NO other controls are touched; the never-synced precondition is
    untouched (no sync buttons); palette is not opened.

Run: bash start-xvfb.sh && python3 w07-theme.py
"""
import json
import os
import shutil
import sqlite3
import subprocess
import sys
import time

from PIL import Image, ImageChops
from Xlib import X, display
from Xlib.ext import xtest

REPO = "/home/z/my-project/audit/SupportOS-Plus"
AUDIT = os.path.join(REPO, "repo-audit-2026-10-24")
OUT = os.path.join(AUDIT, "evidence/screenshots/WORKFLOW")
APP_LOG = "/tmp/w07-app.log"
LOG_DST = os.path.join(AUDIT, "evidence/logs/W07-app.log")
EVENTS = os.path.join(AUDIT, "evidence/artifacts/control-verify-events.jsonl")
DB = "/home/z/.local/share/supportos-plus/supportos-plus.db"

SHOT_W, SHOT_H = 1600, 1000
PARK = (785, 491)
CONTENT_BOX = (230, 60, 1150, 700)          # same content region as pass-2
LUM_BOX = (600, 300, 900, 500)              # plain content area luminance
HEADER_BTN = (1145, 30)                     # C-APP-006 (measured 1130-1160,19-41)
HEADER_ICON_CROP = (1115, 8, 1180, 55)      # moon icon presence pre-check
SIDEBAR_ROW = (100, 728)                    # C-APP-005 (FIND-004 rect y 714-743)
FLIP_THRESH = 0.5                           # full-UI theme flip diff (mean/255)
LUM_LIGHT_MIN = 150.0
LUM_DARK_MAX = 90.0
SAME_THEME_MAX = 0.35                       # restart same-theme structural diff

events = open(EVENTS, "a")


def ev(kind, **kw):
    rec = {"t": round(time.time(), 3), "kind": kind, **kw}
    events.write(json.dumps(rec) + "\n")
    events.flush()
    print(json.dumps(rec), flush=True)


def die(msg):
    ev("w07-fatal", error=msg)
    finish(3)


def finish(code):
    events.close()
    if os.path.exists(APP_LOG):
        shutil.copy(APP_LOG, LOG_DST)
    sys.exit(code)


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
    tmp = path or "/tmp/w07-shot.png"
    subprocess.run(
        ["ffmpeg", "-y", "-loglevel", "error", "-f", "x11grab",
         "-video_size", f"{SHOT_W}x{SHOT_H}", "-i", ":99", "-frames:v", "1", tmp],
        check=True,
    )
    import io
    with open(tmp, "rb") as f:
        data = f.read()
    im = Image.open(io.BytesIO(data))
    im.load()
    return im


def parked_shot(path=None):
    move(*PARK)
    time.sleep(0.3)
    return shot(path)


def diff_mean(a, b, box=CONTENT_BOX):
    diff = ImageChops.difference(
        a.convert("L").crop(box), b.convert("L").crop(box))
    hist = diff.histogram()
    total = sum(hist)
    mean = sum(i * c for i, c in enumerate(hist)) / max(total, 1)
    return round(mean, 3)


def lum(img, box=LUM_BOX):
    g = img.convert("L").crop(box)
    hist = g.histogram()
    total = sum(hist)
    return round(sum(i * c for i, c in enumerate(hist)) / max(total, 1), 1)


def theme_in_db():
    conn = sqlite3.connect(DB)
    try:
        row = conn.execute(
            "SELECT value FROM application_settings WHERE key = 'theme'"
        ).fetchone()
        return row[0] if row else None
    finally:
        conn.close()


def wait_proc_gone(timeout=20):
    deadline = time.time() + timeout
    while time.time() < deadline:
        r = subprocess.run(
            ["pgrep", "-f", "target/debug/supportos-plus"],
            capture_output=True)
        if r.returncode != 0:
            return True
        time.sleep(1)
    return False


def start_app(tag):
    subprocess.run(["pkill", "-f", "target/debug/supportos-plus"], check=False)
    if not wait_proc_gone():
        die(f"prior app instance would not die before {tag}")
    time.sleep(1)
    logf = open(APP_LOG, "a")
    logf.write(f"\n===== W-007 {tag} =====\n")
    p = subprocess.Popen(["./run-app.sh"], stdout=logf,
                         stderr=subprocess.STDOUT, cwd=AUDIT,
                         start_new_session=True)
    deadline = time.time() + 45
    ready = False
    lock_err = False
    while time.time() < deadline:
        time.sleep(1)
        try:
            t = open(APP_LOG, errors="replace").read()
        except FileNotFoundError:
            continue
        tail = t.split(f"===== W-007 {tag} =====")[-1]
        if "cannot acquire the instance lock" in tail:
            lock_err = True
            break
        if "migrations" in tail or "loopback" in tail:
            ready = True
            break
    time.sleep(4 if ready else 8)
    ev("w07-app-started", tag=tag, pid=p.pid, ready=ready, lock_error=lock_err)
    if lock_err or not ready:
        die(f"app failed to start for {tag} (ready={ready} lock_err={lock_err})")
    return p


def icon_present(img):
    """Dark pixels of the moon glyph near the header icon rect (light theme)."""
    g = img.convert("L").crop(HEADER_ICON_CROP)
    px = g.load()
    w, h = g.size
    pts = [(x, y) for y in range(h) for x in range(w) if px[x, y] < 120]
    if not pts:
        return None
    xs = [p[0] for p in pts]
    ys = [p[1] for p in pts]
    return (HEADER_ICON_CROP[0] + min(xs), HEADER_ICON_CROP[1] + min(ys),
            HEADER_ICON_CROP[0] + max(xs), HEADER_ICON_CROP[1] + max(ys),
            len(pts))


def expect_theme(state, tag, pre_img=None, post_img=None, want=None,
                 min_flip=None, reference=None, ref_kind=None):
    """Shared verification for a theme state; returns the screenshot."""
    img = parked_shot(os.path.join(OUT, f"W07-{tag}.png"))
    l = lum(img)
    dbv = theme_in_db()
    checks = {}
    if want == "light":
        checks["lum_is_light"] = l >= LUM_LIGHT_MIN
    elif want == "dark":
        checks["lum_is_dark"] = l <= LUM_DARK_MAX
    if dbv is not None or want is not None:
        # theme.rs semantics: a MISSING key falls back to Light; after any
        # toggle the key is always written, so 'dark' must be explicit.
        checks["db_matches"] = (
            (dbv == want) if want == "dark" else (dbv in (None, "light")))
    if pre_img is not None and min_flip is not None:
        dm = diff_mean(pre_img, img)
        checks["flip_diff"] = dm
        checks["flip_ok"] = dm >= min_flip
    if reference is not None and ref_kind == "same":
        dm = diff_mean(reference, img)
        checks["same_ref_diff"] = dm
        checks["same_ok"] = dm <= SAME_THEME_MAX
    bools = {k: v for k, v in checks.items()
             if k in ("lum_is_light", "lum_is_dark", "db_matches",
                      "flip_ok", "same_ok")}
    ok = all(bools.values())
    ev(f"w07-{state}", tag=tag, lum=l, db_theme=dbv, want=want,
       checks=checks, verdict="PASS" if ok else "FAIL")
    if not ok:
        die(f"{state} ({tag}) verification failed: {checks}")
    return img


def main():
    os.makedirs(OUT, exist_ok=True)

    # S001 — pre-launch DB state (expect no theme key -> Light default).
    db0 = theme_in_db()
    ev("w07-S001-pre-db", theme_key=db0,
       expected="None (unset) -> Theme::Light per theme.rs::from_setting_value")

    # S002 — launch; light initial state.
    p = start_app("launch-1")
    time.sleep(2)
    img0 = expect_theme("S002-initial", "01-light-initial", want="light")
    ic = icon_present(img0)
    ev("w07-S002-icon-presence", moon_icon_bbox_px=ic,
       note="dark-pixel bbox of the icon-only header button (C-APP-006, "
            "tooltip 'switch to dark theme'; icon_button renders no text)")

    # S003 — header button: light -> dark (C-APP-006).
    click(*HEADER_BTN)
    time.sleep(1.2)
    img1 = expect_theme("S003-header-to-dark", "02-dark-after-header",
                        want="dark", pre_img=img0, min_flip=FLIP_THRESH)

    # S004 — sidebar row: dark -> light (C-APP-005).
    click(*SIDEBAR_ROW)
    time.sleep(1.2)
    img2 = expect_theme("S004-sidebar-to-light", "03-light-after-sidebar",
                        want="light", pre_img=img1, min_flip=FLIP_THRESH)

    # S005 — sidebar row again: light -> dark (restart prep).
    click(*SIDEBAR_ROW)
    time.sleep(1.2)
    img3 = expect_theme("S005-sidebar-to-dark", "04-dark-before-restart",
                        want="dark", pre_img=img2, min_flip=FLIP_THRESH)

    # S006 — terminate; theme must already be stored (store runs on click).
    p.terminate()
    time.sleep(2)
    gone = wait_proc_gone()
    db_after_exit = theme_in_db()
    ev("w07-S006-exit", proc_gone=gone, db_theme_after_exit=db_after_exit,
       verdict="PASS" if (gone and db_after_exit == "dark") else "FAIL")
    if not (gone and db_after_exit == "dark"):
        die("S006: process/DB state after exit unexpected")

    # S007 — relaunch: dark must persist (F-026 / theme.rs load at boot).
    p = start_app("launch-2")
    time.sleep(2)
    expect_theme("S007-dark-persisted", "05-dark-after-restart", want="dark",
                 reference=img3, ref_kind="same")

    # S008 — header button in the other direction: dark -> light.
    click(*HEADER_BTN)
    time.sleep(1.2)
    img5 = expect_theme("S008-header-to-light", "06-light-after-header",
                        want="light")

    # S009 — terminate; relaunch: light must persist; precondition restored.
    p.terminate()
    time.sleep(2)
    if not wait_proc_gone():
        die("S009: first terminate did not take effect")
    p = start_app("launch-3")
    time.sleep(2)
    expect_theme("S009-light-persisted", "07-light-after-restart", want="light",
                 reference=img5, ref_kind="same")

    # S010 — end state.
    p.terminate()
    time.sleep(2)
    gone = wait_proc_gone()
    db_end = theme_in_db()
    ev("w07-S010-end", proc_gone=gone, db_theme_end=db_end,
       note="round-trip closed: light->dark->light->dark->restart->light;"
            " original precondition (light) restored")

    ev("w07-summary", verdict="PASS", steps="S001..S010",
       controls="C-APP-006 header icon button (both directions), "
                "C-APP-005 sidebar row (both directions)",
       persistence="dark across restart (S007) AND light across restart (S009)")
    finish(0)


if __name__ == "__main__":
    try:
        main()
    except SystemExit:
        raise
    except Exception as e:  # noqa: BLE001 — audit tooling: record and stop
        die(f"unhandled: {type(e).__name__}: {e}")
