#!/usr/bin/env python3
"""Feature-inventory pass 1 page walk, v2 (audit tooling, NOT app code).

v1 defect (recorded in feature-walk-events.jsonl and the session log): the
first walk captured sidebar band frames while the pointer hovered the
sidebar and while egui's animated wheel scroll was still gliding, so the
scrolled-pass click targets drifted up to ~14 px and pages 17-23 were
selected off-by-one; the ±8 px pill tolerance masked it.

v2 fixes:
  * every analyzed frame is taken with the pointer parked OFF the sidebar
    (no hover backgrounds in any frame);
  * after each wheel batch the scroll must SETTLE: two consecutive frames
    with identical bands+pill, 0.5 s apart, before any targeting;
  * click verification tolerance tightened to ±4 px (observed noise ±1);
  * a click candidate that fails verification falls through to the next
    band below (group labels can sneak into the band list at some
    subpixel scroll phases — a label click is a harmless no-op);
  * Shell handling: band detection extended to y<745; the theme row rect
    is measured by hover and clicks are placed inside the Shell row but
    outside the theme rect (top sliver or bottom sliver); any accidental
    theme toggle is reverted immediately (net no-op).

Launches the default binary (main @ 3affe7e) on Xvfb :99, injects real
X11 input (XTEST clicks + wheel), saves one screenshot per page into
evidence/screenshots/FEATURE/, plus an event log and the app log.
"""
import json
import os
import shutil
import subprocess
import sys
import time

from PIL import Image, ImageChops
from Xlib import X, display
from Xlib.ext import xtest

REPO = "/home/z/my-project/audit/SupportOS-Plus"
AUDIT = os.path.join(REPO, "repo-audit-2026-10-24")
OUT = os.path.join(AUDIT, "evidence/screenshots/FEATURE")
APP_LOG = "/tmp/walk-app.log"
EVENTS = os.path.join(AUDIT, "evidence/artifacts/feature-walk-events.jsonl")
LOG_DST = os.path.join(AUDIT, "evidence/logs/FEATURE-walk-app.log")

PAGES = [  # sidebar order == PageKey::all() (asserted in app.rs:1523)
    "01-dashboard", "02-inbox", "03-live-chats", "04-operations",
    "05-customers", "06-organizations", "07-knowledge", "08-reports",
    "09-issue-radar", "10-incidents", "11-custom-objects", "12-automation",
    "13-ai-center", "14-outreach", "15-notifications", "16-search",
    "17-support-graph", "18-connectors", "19-diagnostics", "20-sync-health",
    "21-settings", "22-onboarding", "23-shell",
]
SHOT_W, SHOT_H = 1600, 1000
CLICK_MAX_Y = 700              # never click the theme-row overlay zone
SCAN_Y1 = 745                  # detect (not click) bands down to here
PILL_X = 185                   # right of all labels, inside row backgrounds
PILL_LUM = 247                 # selected bg 244 vs panel 250
BAND_THR = 100                 # text/icons < 100; muted group labels ~114
BAND_X0, BAND_X1 = 45, 150
BAND_Y0 = 125
PARK = (785, 491)
TOL = 4                        # click verification tolerance (px)

events = open(EVENTS, "a")


def ev(kind, **kw):
    rec = {"t": round(time.time(), 3), "kind": kind, **kw}
    events.write(json.dumps(rec) + "\n")
    events.flush()
    print(json.dumps(rec), flush=True)


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


def wheel(notches, x=100, y=400):
    move(x, y)
    btn = 5 if notches > 0 else 4
    for _ in range(abs(notches)):
        xtest.fake_input(d, X.ButtonPress, btn)
        d.sync()
        time.sleep(0.02)
        xtest.fake_input(d, X.ButtonRelease, btn)
        d.sync()
        time.sleep(0.04)


def shot(path=None):
    tmp = path or "/tmp/walk-shot.png"
    subprocess.run(
        ["ffmpeg", "-y", "-loglevel", "error", "-f", "x11grab",
         "-video_size", f"{SHOT_W}x{SHOT_H}", "-i", ":99", "-frames:v", "1", tmp],
        check=True,
    )
    # Return an independent in-memory snapshot: PIL is lazy, and every
    # call overwrites the same tmp file — an un-loaded handle would read
    # whichever frame was written last (the v1 drift bug).
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


def bands(img):
    g = img.convert("L")
    px = g.load()
    raw = []
    start = None
    for y in range(BAND_Y0, SCAN_Y1):
        cnt = sum(1 for x in range(BAND_X0, BAND_X1) if px[x, y] < BAND_THR)
        v = cnt > 1
        if v and start is None:
            start = y
        elif not v and start is not None:
            raw.append((start, y - 1))
            start = None
    if start is not None:
        raw.append((start, SCAN_Y1 - 1))
    merged = []
    for b in raw:
        if merged and b[0] - merged[-1][1] < 10:
            merged[-1] = (merged[-1][0], b[1])
        else:
            merged.append(b)
    return [(a + b) // 2 for a, b in merged]


def pill(img, y0=125, y1=755):
    g = img.convert("L")
    px = g.load()
    run = 0
    for y in range(y0, y1):
        if px[PILL_X, y] <= PILL_LUM:
            run += 1
        else:
            run = 0
        if run >= 20:
            return y - run + 1 + 16
    return None


def light_theme(img):
    return img.convert("L").load()[PILL_X, 400] > 200


def content_changed(a, b):
    box = (230, 60, 1150, 700)
    diff = ImageChops.difference(a.convert("L").crop(box), b.convert("L").crop(box))
    hist = diff.histogram()
    total = sum(hist)
    mean = sum(i * c for i, c in enumerate(hist)) / max(total, 1)
    return mean > 1.0, round(mean, 3)


def settled_state(min_settle=2, tries=10):
    """Repeatedly (parked) shot until bands+pill are stable twice in a row."""
    prev = None
    stable = 0
    for i in range(tries):
        img = parked_shot()
        cur = (bands(img), pill(img))
        if prev is not None and cur == prev:
            stable += 1
            if stable >= min_settle - 1:
                return img, cur[0], cur[1], i + 1
        else:
            stable = 0
        prev = cur
        time.sleep(0.5)
    return img, cur[0], cur[1], tries  # not fully settled; caller decides


def start_app():
    # Hard provenance rule: exactly one fresh app process per walk. A
    # previous script crash can leave an orphan holding the instance lock
    # (observed after the v2 crash) — clean it up and verify the lock is
    # acquired, else the walk would drive a stale instance with a wrong
    # log trail.
    subprocess.run(["pkill", "-f", "target/debug/supportos-plus"],
                   check=False)
    time.sleep(2)
    logf = open(APP_LOG, "w")
    p = subprocess.Popen(["./run-app.sh"], stdout=logf, stderr=subprocess.STDOUT,
                         cwd=AUDIT, start_new_session=True)
    deadline = time.time() + 45
    ready = False
    lock_err = False
    while time.time() < deadline:
        time.sleep(1)
        try:
            t = open(APP_LOG, errors="replace").read()
        except FileNotFoundError:
            continue
        if "cannot acquire the instance lock" in t:
            lock_err = True
            break
        if "migrations" in t or "loopback" in t:
            ready = True
            break
    time.sleep(4 if ready else 8)
    ev("app-started", pid=p.pid, ready=ready, lock_error=lock_err)
    if lock_err or not ready:
        shutil.copy(APP_LOG, LOG_DST)
        p.terminate()
        sys.exit(4)
    return p


def theme_rect(img_a, img_b):
    """Rect of the theme row via hover-bg diff between two parked shots."""
    ga, gb = img_a.convert("L"), img_b.convert("L")
    pa, pb = ga.load(), gb.load()
    ys = [y for y in range(695, 758)
          if abs(pa[PILL_X, y] - pb[PILL_X, y]) >= 3]
    if not ys:
        return None
    return (min(ys), max(ys))


def main():
    os.makedirs(OUT, exist_ok=True)
    app = start_app()

    def die(msg):
        ev("abort", reason=msg)
        app.terminate()
        try:
            app.wait(timeout=5)
        except Exception:
            app.kill()
        shutil.copy(APP_LOG, LOG_DST)
        sys.exit(2)

    k = 0  # index of the currently selected page
    img, bs, py, _ = settled_state()
    if not light_theme(img):
        die("unexpected dark theme at startup (DB state changed?)")
    ev("launch-state", pill_y=py, bands=bs,
       note="app restores last page + sidebar scroll across restarts")

    # Prelude: normalize to a known state — the previous session's page
    # and nav scroll are restored on launch, so scroll the nav to the top
    # and explicitly select Dashboard before walking.
    wheel(-12)
    img, bs, py, _ = settled_state()
    if not bs or not (170 <= bs[0] <= 190):
        die(f"sidebar did not return to top (first band {bs[:1]})")
    click(100, bs[0])               # Dashboard row at offset 0
    time.sleep(0.3)
    img = parked_shot()
    py = pill(img)
    if py is None or abs(py - bs[0]) > TOL:
        die("could not select Dashboard at the top of the sidebar")
    img.save(os.path.join(OUT, PAGES[0] + ".png"))
    ev("page", page=PAGES[0], pill_y=py, method="launch+normalize", bands=bs)

    scrolls = 0
    shell_ok = None

    while k < len(PAGES) - 1:
        bs = bands(img)
        py = pill(img)
        if py is None:
            die(f"pill lost at page {PAGES[k]}")
        candidates = [y for y in bs if py + 24 < y <= CLICK_MAX_Y]
        if not candidates:
            if scrolls > 40:
                die("scroll budget exhausted while chasing the next row")
            pre = (bs, py)
            wheel(3)
            img, bs2, py2, settled_in = settled_state()
            scrolls += 1
            if (bs2, py2) == pre:
                wheel(5)
                img, bs2, py2, _ = settled_state()
                if (bs2, py2) == pre:
                    if k == len(PAGES) - 2 and PAGES[k + 1] == "23-shell":
                        img, shell_ok = shell_sliver(app, die, img)
                        k += 1
                        if not shell_ok:
                            ev("exception-candidate", id="EXC-CAND-001",
                               page="23-shell",
                               note="Shell row not reachable by real-input "
                                    "click; register exception")
                        break
                    die(f"sidebar at max scroll but next page is "
                        f"{PAGES[k+1]}, not Shell")
            ev("scroll", step=scrolls, settled_frames=settled_in,
               pill_y=py2, bands=bs2)
            continue

        target = candidates[0]
        k += 1
        slug = PAGES[k]
        ok = False
        attempt_targets = candidates[:3]
        for ti, ty in enumerate(attempt_targets):
            click(100, ty)
            time.sleep(0.3)
            img2 = parked_shot()
            py2 = pill(img2)
            ok = py2 is not None and abs(py2 - ty) <= TOL
            ev("click", page=slug, attempt=ti + 1, y=ty, pill_after=py2,
               verified=ok)
            if ok:
                break
        if not ok:
            die(f"click for {slug} did not move the pill onto any candidate "
                f"{attempt_targets}")
        img = img2
        py = py2
        img.save(os.path.join(OUT, slug + ".png"))

    parked_shot(os.path.join(OUT, "24-end-state.png"))
    app.terminate()
    try:
        app.wait(timeout=5)
    except Exception:
        app.kill()
    shutil.copy(APP_LOG, LOG_DST)
    ev("done", pages=PAGES[: k + 1], scroll_steps=scrolls,
       shell_verified=shell_ok)
    print(f"WALK COMPLETE: {k + 1}/23 pages, {scrolls} scroll steps, "
          f"shell_verified={shell_ok}")
    if shell_ok is False:
        sys.exit(3)


def shell_sliver(app, die, img):
    """Open the Shell page when its row is partly covered by the theme row.

    Strategy: measure the theme row rect from a hover-bg diff (hover shot
    keeps the pointer on the theme row), then click a point inside the
    Shell row but outside the theme rect (top sliver and/or bottom
    sliver). Verification: pill (anywhere 690..755) or content diff.
    A stray theme toggle is reverted immediately (net no-op).
    """
    move(*PARK)
    time.sleep(0.3)
    base = shot()                     # parked, no hover — reference state
    move(100, 718)
    time.sleep(0.35)
    img_hover = shot()                # pointer ON the theme row — keep it
    trect = theme_rect(base, img_hover)
    move(*PARK)
    time.sleep(0.3)
    img = shot()                      # parked state again (bands + pill)
    ev("shell-probe", theme_rect=trect)
    bs = bands(img)
    py = pill(img)
    cand = []
    if py is not None:
        below = [y for y in bs if y > py + 24]
        if below:
            sb = below[-1]            # last band below the pill = Shell row
            cand += [sb - 14, sb - 16, sb - 12, sb - 15, sb - 10,
                     sb - 13, sb - 8]
    if trect:
        t0, t1 = trect
        cand += [t1 + 3, t1 + 2, t0 - 2, t0 - 1]
    cand += [703, 702, 701, 700, 704, 730]
    seen = []
    im = img
    for y in cand:
        if y in seen or y > 750 or y < 655:
            continue
        seen.append(y)
        click(100, y)
        time.sleep(0.3)
        im = parked_shot()
        if not light_theme(im):
            ev("shell-attempt", y=y, note="theme toggled; reverting")
            click(100, 718)
            time.sleep(0.4)
            im = parked_shot()
            if not light_theme(im):
                die("could not revert an accidental theme toggle")
        changed, mean = content_changed(base, im)
        p = pill(im, y0=690, y1=755)
        verified = p is not None or changed
        ev("shell-attempt", y=y, content_diff=mean, changed=changed,
           pill_lower=p, verified=verified)
        if verified:
            im.save(os.path.join(OUT, "23-shell.png"))
            return im, True
    im.save(os.path.join(OUT, "23-shell-attempt-unverified.png"))
    return im, False


if __name__ == "__main__":
    _app = None
    try:
        main()
    finally:
        # Never leave an orphan holding the instance lock (the v2 crash
        # caused exactly that, and v3/v4 then drove the stale instance).
        subprocess.run(["pkill", "-f", "target/debug/supportos-plus"],
                       check=False)
