#!/usr/bin/env python3
"""Control-ledger pass-2 runtime verification v2 (audit tooling, NOT app code).

Same proven real-input stack as the pass-1 walk (XTEST clicks on Xvfb :99,
pointer parked off the controls for every analyzed frame, settled frames
before targeting, selection-pill-verified navigation), extended with
tesseract OCR label targeting for in-page controls.

v2 fixes after the v1 calibration run (all recorded in the events JSONL):
  * v1's page-switch diff threshold (mean > 1.0) false-negatived control-
    level changes (a selected chip affects only a few hundred pixels);
    v2 uses a per-step threshold (default 0.12). Pixel checks + visual
    strip reads proved D01/I01/I02/S01..S03/F01 actually worked in v1.
  * three-pass OCR per frame: psm6, then inverted psm6 (white-on-dark
    buttons like "New Connector"/"New Incident"/"New Type"/"New Rule"
    were invisible to v1), then 2x-upscaled psm11 (small chip rows on
    Diagnostics/Sync Health were dropped entirely).
  * needle matching: exact word first, then minimal sliding-window over
    the OCR line whose join contains the needle; the click aims at the
    WINDOW CENTER — v1 aimed at the first word contained in the needle,
    which hit "open chats" for needle "show ended chats" and message
    text for "new incident".
  * crops tightened from measured TSV boxes (subtitles at y~107-110
    contain the words "documents"/"sources"/"unread"/"alerts" and stole
    v1 clicks aimed at tab rows; "mark all read" stole the All-tab
    click on Notifications).
  * type-steps pick the RIGHTMOST exact-word "search" right of the field
    (v1 clicked the field's own placeholder word).

Deliberate sequencing decisions (recorded, not silent):
  * No clicks on Sync Health "Run initial/incremental sync" or Onboarding
    "Run initial sync"/"Mark complete": the never-synced precondition is
    one-shot state that workflow W-002 must consume first. Sync Health's
    idle "Cancel running sync" (a no-op guard path) IS clicked instead.
  * No clicks on the "Sync now" buttons and no theme toggle (W-007).
  * Diagnostics "clear log" IS executed through its confirmation dialog
    (QA-owned data dir; NEXT-ACTIONS pass-2 target).

Run: bash start-xvfb.sh && python3 control-verify.py [--from STEP-ID]
"""
import json
import os
import shutil
import subprocess
import sys
import time

from PIL import Image, ImageChops, ImageOps
from Xlib import X, display
from Xlib.ext import xtest

REPO = "/home/z/my-project/audit/SupportOS-Plus"
AUDIT = os.path.join(REPO, "repo-audit-2026-10-24")
OUT = os.path.join(AUDIT, "evidence/screenshots/CONTROL")
APP_LOG = "/tmp/ctrl-app.log"
EVENTS = os.path.join(AUDIT, "evidence/artifacts/control-verify-events.jsonl")
LOG_DST = os.path.join(AUDIT, "evidence/logs/CONTROL-app.log")

SHOT_W, SHOT_H = 1600, 1000
CLICK_MAX_Y = 700
SCAN_Y1 = 745
PILL_X = 185
PILL_LUM = 247
BAND_THR = 100
BAND_X0, BAND_X1 = 45, 150
BAND_Y0 = 125
PARK = (785, 491)
TOL = 4
CONTENT_BOX = (230, 60, 1150, 700)
# v1/v2 calibration: a selected chip/tab flip measures mean diff 0.06-0.1;
# no-op clicks measure 0.0-0.01; full view switches 0.6-5.5.
DEFAULT_THRESH = 0.04

PAGES = [  # sidebar order == PageKey::all() (asserted app.rs:1523)
    "01-dashboard", "02-inbox", "03-live-chats", "04-operations",
    "05-customers", "06-organizations", "07-knowledge", "08-reports",
    "09-issue-radar", "10-incidents", "11-custom-objects", "12-automation",
    "13-ai-center", "14-outreach", "15-notifications", "16-search",
    "17-support-graph", "18-connectors", "19-diagnostics", "20-sync-health",
    "21-settings", "22-onboarding", "23-shell",
]

STEPS = [
    dict(id="D01", page=0, kind="click", label="7 days",
         crop=(230, 130, 1179, 220), expect="change",
         note="range chip (v1 click verified by pixel+strip read; rerun restores)"),
    dict(id="I01", page=1, kind="click", label="pending",
         crop=(230, 125, 1179, 200), expect="change", note="status tab"),
    dict(id="I02", page=1, kind="click", label="all",
         crop=(230, 125, 1179, 190), expect="change", note="restore All tab"),
    dict(id="L01", page=2, kind="click", label="showendedchats",
         crop=(230, 125, 1179, 200), expect="change"),
    dict(id="L02", page=2, kind="click", label="openchats",
         crop=(230, 125, 1179, 200), expect="change", note="restore"),
    dict(id="O01", page=3, kind="click", label="workload",
         crop=(230, 125, 1179, 200), expect="change",
         note="v1 clicked the subtitle word; crop now starts below it"),
    dict(id="O02", page=3, kind="click", label="library",
         crop=(230, 125, 1179, 200), expect="change"),
    dict(id="O03", page=3, kind="click", label="tiles",
         crop=(230, 125, 1179, 200), expect="change", note="restore"),
    dict(id="CU1", page=4, kind="type", placeholder="searchcustomers",
         text="", button="search", button_crop=(480, 100, 700, 165),
         crop=(230, 60, 1179, 320), expect=None,
         note="empty-query path: field clicked (focus) + Search button "
              "clicked; TEXT INJECTION NOT EXERCISABLE (XTEST KeyPress does "
              "not reach egui TextEdit in this env; xdotool unavailable; "
              "verified twice by field OCR showing placeholder) — text-input "
              "behavior is source-verified only"),
    dict(id="OR1", page=5, kind="dark_button",
         crop=(230, 300, 900, 500), expect=None,
         note="wide dark 'create organization' button; empty-field "
              "validation path; diff recorded"),
    dict(id="K01", page=6, kind="click", label="importdocuments",
         crop=(230, 150, 1179, 560), expect="change",
         note="expander open (NEXT-ACTIONS target); verified open in v1"),
    dict(id="K02", page=6, kind="click", label="sources",
         crop=(230, 150, 1179, 200), expect="change",
         note="tab row only; v1 clicked the subtitle 'sources'"),
    dict(id="K03", page=6, kind="click", label="documents",
         crop=(230, 150, 1179, 200), expect="change", note="restore tab"),
    dict(id="R01", page=7, kind="click", label="builder",
         crop=(230, 60, 1179, 300), expect="change",
         note="Builder tab (NEXT-ACTIONS target); verified in v1"),
    dict(id="R02", page=7, kind="click", label="overview",
         crop=(230, 60, 1179, 300), expect="change", note="restore"),
    dict(id="RA1", page=8, kind="click", label="clusters",
         crop=(230, 60, 1179, 190), expect=None,
         note="topmost 'clusters' = Reload clusters button; diff recorded"),
    dict(id="RA2", page=8, kind="click", label="alerts",
         crop=(230, 135, 1179, 200), expect=None,
         note="tab (default Alerts; v1 clicked subtitle)"),
    dict(id="IN1", page=9, kind="dark_button",
         crop=(600, 60, 1179, 140), expect="change",
         note="dark +icon button unreadable by OCR (icon merges into 'N'); "
              "rect detector (v1 clicked empty-state message text)"),
    dict(id="IN2", page=9, kind="click", label="openonly",
         crop=(230, 140, 1179, 215), expect="change", left_shift=43,
         thresh=0.005,
         note="checkbox BOX only responds; checkmark toggling measured at "
              "diff 0.007 (pixel check: +12 dark px in box region), so a "
              "checkmark-sized threshold is required"),
    dict(id="IN3", page=9, kind="click", label="openonly",
         crop=(230, 140, 1179, 215), expect="change", left_shift=43,
         thresh=0.005, note="checkbox off again"),
    dict(id="CO1", page=10, kind="dark_button",
         crop=(600, 60, 1179, 140), expect="change",
         note="dark +icon top-right button (rect detector)"),
    dict(id="AU1", page=11, kind="dark_button",
         crop=(600, 60, 1179, 140), expect="change",
         note="dark +icon top-right button (rect detector)"),
    dict(id="A01", page=12, kind="click", label="copilot",
         crop=(230, 90, 1179, 165), expect="change",
         note="tab row; v1 ocr-miss on garbled conf"),
    dict(id="A02", page=12, kind="click", label="usage",
         crop=(230, 90, 1179, 165), expect="change"),
    dict(id="A03", page=12, kind="click", label="jobs",
         crop=(230, 90, 1179, 165), expect="change"),
    dict(id="A04", page=12, kind="click", label="health",
         crop=(230, 90, 1179, 165), expect="change", note="restore"),
    dict(id="OU1", page=13, kind="click", label="segments",
         crop=(230, 95, 1179, 165), expect="change",
         note="v1 diff 0.603 < 1.0 false negative"),
    dict(id="OU2", page=13, kind="click", label="contact",
         crop=(230, 115, 1179, 145), expect="change",
         note="exact word 'contact' in the tab row (window match hit a gap)"),
    dict(id="OU3", page=13, kind="click", label="campaigns",
         crop=(230, 95, 1179, 165), expect="change", note="restore"),
    dict(id="N01", page=14, kind="click", label="mentions",
         crop=(230, 125, 1179, 185), expect="change",
         note="v1 verified by pixel; threshold false negative"),
    dict(id="N02", page=14, kind="click", label="unread",
         crop=(230, 125, 1179, 185), expect="change",
         note="v1 clicked subtitle 'unread'; crop now tabs row only"),
    dict(id="N03", page=14, kind="click", label="all",
         crop=(230, 125, 1179, 185), expect="change",
         note="v1 clicked 'mark all read'; crop now tabs row only"),
    dict(id="S01", page=15, kind="click", label="tickets",
         crop=(230, 150, 1179, 240), expect="change", note="scope chip"),
    dict(id="S02", page=15, kind="click", label="knowledge",
         crop=(230, 150, 1179, 240), expect="change"),
    dict(id="S03", page=15, kind="click", label="all",
         crop=(230, 150, 1179, 240), expect="change", note="restore"),
    dict(id="G01", page=16, kind="type", placeholder="searchthegraph",
         text="", button="search", button_crop=(560, 120, 800, 175),
         crop=(230, 60, 1179, 350), expect=None,
         note="empty-query path: field + search button clicked; same XTEST "
              "keyboard-injection limitation as CU1; Enter does NOT submit "
              "(v3 type_enter diff 0.0)"),
    dict(id="CN1", page=17, kind="dark_button",
         crop=(600, 60, 1179, 140), expect="change",
         note="dark +icon top-right button (rect detector)"),
    dict(id="DI1", page=18, kind="dark_button",
         crop=(230, 190, 900, 250), expect="change", rightmost=True,
         note="red clear-log button, rightmost solid cluster in the filter "
              "row (NEXT-ACTIONS target)"),
    dict(id="DI2", page=18, kind="dialog", expect="change",
         note="click dialog danger button (Clear log) left of Cancel"),
    dict(id="SY1", page=19, kind="click", label="cancelrunningsync",
         crop=(230, 60, 1179, 560), expect=None,
         note="idle no-op guard path; sync-state precondition preserved"),
    dict(id="SE1", page=20, kind="click", label="backups",
         crop=(230, 60, 1179, 200), expect="change",
         note="Help Scout & LM Studio tabs are NEXT-ACTIONS targets"),
    dict(id="SE2", page=20, kind="click", label="lmstudio",
         crop=(230, 60, 1179, 200), expect="change", fallback_xy=(975, 92),
         note="tab text conf < 8 even upscaled; geometric fallback between "
              "Vector index and Help Scout chips (visual read arbitrates)"),
    dict(id="SE3", page=20, kind="click", label="scout",
         crop=(230, 60, 1179, 200), expect="change"),
    dict(id="SE4", page=20, kind="click", label="sync",
         crop=(230, 60, 1179, 200), expect="change", note="restore"),
    dict(id="F01", page=None, kind="home", label="30days",
         crop=(230, 130, 1179, 220), expect=None,
         note="restore 30-day chip (already holds after earlier verified "
              "restore; no-op recorded honestly)"),
]

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


def key(code):
    xtest.fake_input(d, X.KeyPress, code)
    d.sync()
    time.sleep(0.03)
    xtest.fake_input(d, X.KeyRelease, code)
    d.sync()
    time.sleep(0.05)


def shot(path=None):
    tmp = path or "/tmp/ctrl-shot.png"
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


def content_changed(a, b):
    diff = ImageChops.difference(
        a.convert("L").crop(CONTENT_BOX), b.convert("L").crop(CONTENT_BOX))
    hist = diff.histogram()
    total = sum(hist)
    mean = sum(i * c for i, c in enumerate(hist)) / max(total, 1)
    return mean, round(mean, 3)


def settled_state(min_settle=2, tries=10):
    prev = None
    stable = 0
    cur = None
    img = None
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
    return img, cur[0], cur[1], tries


def norm(s):
    return "".join(c for c in s.lower() if c.isalnum())


def _tsv_to_words(crop, tsv_path, conf_min=15, scale=1):
    words = []
    for row in open(tsv_path, errors="replace").read().splitlines()[1:]:
        col = row.split("\t")
        if len(col) < 12:
            continue
        try:
            conf = float(col[10])
        except ValueError:
            continue
        text = col[11].strip()
        if conf < conf_min or not text:
            continue
        left, top, w, h = (int(col[6]), int(col[7]), int(col[8]), int(col[9]))
        cx = crop[0] + (left + w // 2) / scale
        cy = crop[1] + (top + h // 2) / scale
        line_key = (col[2], col[3], col[4])
        words.append((norm(text), int(cx), int(cy), line_key))
    return words


def ocr_words(img, crop):
    """OCR a crop with a 4-pass strategy; merge word boxes.
    Passes: psm6 plain, psm6 inverted (white-on-dark buttons), psm6 3x
    (small chip/button rows), psm11 2x (sparse)."""
    base = img.crop(crop)
    words = []

    def run_ocr(psm, invert=False, scale=1):
        im = base
        if scale != 1:
            im = im.resize((im.width * scale, im.height * scale))
        if invert:
            im = ImageOps.invert(im.convert("L")).convert("RGB")
        im.save("/tmp/ctrl-ocr.png")
        subprocess.run(
            ["tesseract", "/tmp/ctrl-ocr.png", "/tmp/ctrl-ocr-out",
             "--psm", str(psm), "tsv"],
            check=True, capture_output=True)
        return _tsv_to_words(crop, "/tmp/ctrl-ocr-out.tsv", scale=scale)

    for psm, inv, sc in [(6, False, 1), (6, True, 1), (6, False, 3),
                         (11, False, 2)]:
        try:
            words += run_ocr(psm, inv, sc)
        except (subprocess.CalledProcessError, FileNotFoundError):
            pass
    return words


def dark_button(img, crop, min_row_dark=40, min_h=22, min_w=50,
                rightmost=False):
    """Find a solid dark button rectangle in the crop (audit-tooling
    detector; the app's +icon buttons defeat OCR by merging the icon
    into the first glyph). Returns the cluster center or None."""
    g = img.crop(crop).convert("L")
    px = g.load()
    w, h = g.size
    dark_rows = []
    for y in range(h):
        cnt = sum(1 for x in range(w) if px[x, y] < 140)
        dark_rows.append(cnt)
    bands = []
    start = None
    for y in range(h + 1):
        v = y < h and dark_rows[y] >= min_row_dark
        if v and start is None:
            start = y
        elif not v and start is not None:
            if y - start >= min_h:
                bands.append((start, y - 1))
            start = None
    best = None
    for (y0, y1) in bands:
        cols = []
        for x in range(w):
            cnt = sum(1 for y in range(y0, y1 + 1) if px[x, y] < 140)
            cols.append(cnt)
        run_start = None
        for x in range(w + 1):
            v = x < w and cols[x] >= (y1 - y0 + 1) * 0.6
            if v and run_start is None:
                run_start = x
            elif not v and run_start is not None:
                if x - run_start >= min_w:
                    cand = (run_start, x - 1, y0, y1)
                    if best is None or ((cand[1] - cand[0]) > (best[1] - best[0])):
                        if not rightmost:
                            best = cand
                    if rightmost:
                        if best is None or cand[0] > best[0]:
                            best = cand
                run_start = None
    if best is None:
        return None
    x0, x1, y0, y1 = best
    return (crop[0] + (x0 + x1) // 2, crop[1] + (y0 + y1) // 2)


def dark_bbox(img, crop, min_px=3, min_w=40, min_h=18):
    """Bounding box of ALL dark pixels in the crop (for +icon buttons whose
    white text breaks the solid-rect detector). Returns center or None."""
    g = img.crop(crop).convert("L")
    px = g.load()
    w, h = g.size
    rows = [y for y in range(h)
            if sum(1 for x in range(w) if px[x, y] < 140) >= min_px]
    if not rows:
        return None
    y0, y1 = rows[0], rows[-1]
    cols = [x for x in range(w)
            if sum(1 for y in range(y0, y1 + 1) if px[x, y] < 140) >= min_px]
    if not cols:
        return None
    x0, x1 = cols[0], cols[-1]
    if x1 - x0 < min_w or y1 - y0 < min_h:
        return None
    return (crop[0] + (x0 + x1) // 2, crop[1] + (y0 + y1) // 2)


def find_label(img, needle, crop, first=False):
    """Exact word first; else minimal sliding window whose join contains
    needle; click aims at the window center. Returns (x, y) or None."""
    needle = norm(needle)
    words = ocr_words(img, crop)
    exact = [(w[1], w[2]) for w in words if w[0] == needle]
    if exact:
        exact.sort(key=lambda p: (p[1], p[0]))
        return exact[0]
    lines = {}
    for w in words:
        lines.setdefault(w[3], []).append(w)
    wins = []
    for lk, ws in lines.items():
        ws.sort(key=lambda t: t[1])
        n = len(ws)
        for i in range(n):
            join = ""
            for j in range(i, n):
                join += ws[j][0]
                if needle in join and len(join) < len(needle) * 3 + 12:
                    x0 = ws[i][1]
                    x1 = ws[j][1]
                    wins.append(((x0 + x1) // 2, ws[i][2], i, j))
                    break
    if not wins:
        return None
    # prefer the tightest window, then topmost
    wins.sort(key=lambda p: (p[2] - p[3], p[1], p[0]))
    return wins[0][0], wins[0][1]


def start_app():
    subprocess.run(["pkill", "-f", "target/debug/supportos-plus"], check=False)
    time.sleep(2)
    logf = open(APP_LOG, "w")
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


def normalize_home(die):
    """Scroll sidebar to top and select Dashboard."""
    wheel(-12)
    img, bs, py, _ = settled_state()
    if not bs or not (170 <= bs[0] <= 190):
        die(f"sidebar did not return to top (first band {bs[:1] if bs else None})")
    click(100, bs[0])
    time.sleep(0.3)
    img = parked_shot()
    py = pill(img)
    if py is None or abs(py - bs[0]) > TOL:
        die("could not select Dashboard at the top of the sidebar")
    ev("home", pill_y=py)
    return img, bs, py


def nav_forward(app, die, img, k_cur, k_target):
    """Walk the sidebar forward to page k_target (proven walk algorithm)."""
    scrolls = 0
    while k_cur < k_target:
        bs = bands(img)
        py = pill(img)
        if py is None:
            die(f"pill lost while navigating to {PAGES[k_target]}")
        candidates = [y for y in bs if py + 24 < y <= CLICK_MAX_Y]
        if not candidates:
            if scrolls > 40:
                die("scroll budget exhausted")
            pre = (bs, py)
            wheel(3)
            img, bs2, py2, _ = settled_state()
            scrolls += 1
            if (bs2, py2) == pre:
                wheel(5)
                img, bs2, py2, _ = settled_state()
                if (bs2, py2) == pre:
                    die(f"sidebar at max scroll before {PAGES[k_target]}")
            continue
        target = candidates[0]
        ok = False
        for ty in candidates[:3]:
            click(100, ty)
            time.sleep(0.3)
            img2 = parked_shot()
            py2 = pill(img2)
            ok = py2 is not None and abs(py2 - ty) <= TOL
            if ok:
                break
        if not ok:
            die(f"nav click for {PAGES[k_cur + 1]} not verified")
        img = img2
        k_cur += 1
    return img, k_cur


def finish_click(sid, step, pre, post, extra):
    post.save(os.path.join(OUT, sid + ".png"))
    mean, rounded = content_changed(pre, post)
    thresh = step.get("thresh", DEFAULT_THRESH)
    changed = mean > thresh
    expect_change = step.get("expect") == "change"
    ok = (changed == expect_change) if step.get("expect") else True
    ev("step", id=sid, diff=rounded, changed=changed,
       expect=step.get("expect"), result="ok" if ok else "verify-failed",
       **extra)
    return ok


def do_step(app, die, step, img, k):
    sid = step["id"]
    crop = step.get("crop")
    pre = parked_shot()
    if step["kind"] == "click":
        hit = find_label(pre, step["label"], crop)
        method = "ocr"
        if hit is None and step.get("fallback_xy"):
            hit = step["fallback_xy"]
            method = "geometric-fallback"
        if hit is None:
            pre.save(os.path.join(OUT, sid + "-ocr-miss.png"))
            ev("step", id=sid, result="ocr-miss", label=step["label"])
            return img, k, False
        hx, hy = hit
        if not (CLICK_MAX_Y >= hy >= 62):
            ev("step", id=sid, result="reject", reason=f"y={hy} outside band")
            return img, k, False
        if step.get("left_shift"):
            hx -= step["left_shift"]
        click(hx, hy)
        time.sleep(0.35)
        post = parked_shot()
        ok = finish_click(sid, step, pre, post,
                          dict(action="click", label=step["label"],
                               at=[hx, hy], method=method))
        return post, k, ok
    if step["kind"] == "dark_button":
        hit = dark_button(pre, crop, rightmost=step.get("rightmost", False))
        if hit is None:
            pre.save(os.path.join(OUT, sid + "-rect-miss.png"))
            ev("step", id=sid, result="rect-miss")
            return img, k, False
        click(*hit)
        time.sleep(0.35)
        post = parked_shot()
        ok = finish_click(sid, step, pre, post,
                          dict(action="dark_button", at=[hit[0], hit[1]]))
        return post, k, ok
    if step["kind"] in ("type", "type_enter"):
        phit = find_label(pre, step["placeholder"], crop)
        if phit is None:
            pre.save(os.path.join(OUT, sid + "-ocr-miss.png"))
            ev("step", id=sid, result="ocr-miss", label=step["placeholder"])
            return img, k, False
        click(*phit)  # focus the field (recorded even when text is empty)
        time.sleep(0.25)
        if step["text"]:
            for ch in step["text"]:
                kc = d.keysym_to_keycode(ord(ch))
                if kc:
                    key(kc)
        time.sleep(0.2)
        pre2 = parked_shot()
        if step["kind"] == "type_enter":
            ret = d.keysym_to_keycode(0xFF0D)  # Return
            if not ret:
                ev("step", id=sid, result="no-return-key")
                return img, k, False
            key(ret)
            time.sleep(0.4)
            post = parked_shot()
            ok = finish_click(sid, step, pre2, post,
                              dict(action="type_enter", field_at=list(phit),
                                   typed=step["text"]))
            return post, k, ok
        # button click: prefer a per-step measured button_crop (dark bbox);
        # else exact OCR word on the FIELD's row, right of it
        if step.get("button_crop"):
            hit = dark_bbox(pre2, step["button_crop"])
            if hit is None:
                pre2.save(os.path.join(OUT, sid + "-button-miss.png"))
                ev("step", id=sid, result="button-miss", field_at=list(phit),
                   button_crop=list(step["button_crop"]))
                return img, k, False
            bx, by, how = hit[0], hit[1], "bbox"
        else:
            words = ocr_words(pre2, crop)
            cands = [w for w in words if w[0] == step["button"]
                     and w[1] > phit[0] + 40 and abs(w[2] - phit[1]) <= 25]
            if cands:
                cands.sort(key=lambda w: -w[1])  # rightmost = the button
                bx, by = cands[0][1], cands[0][2]
                how = "ocr"
            else:
                hit = dark_button(pre2, (phit[0] + 40, phit[1] - 30,
                                         crop[2], phit[1] + 30))
                if hit is None:
                    pre2.save(os.path.join(OUT, sid + "-button-miss.png"))
                    ev("step", id=sid, result="button-miss", field_at=list(phit))
                    return img, k, False
                bx, by, how = hit[0], hit[1], "rect"
        pre3 = parked_shot()
        click(bx, by)
        time.sleep(0.4)
        post = parked_shot()
        ok = finish_click(sid, step, pre3, post,
                          dict(action="type", field_at=list(phit),
                               button_at=[bx, by], how=how,
                               typed=step["text"] or "(none - keyboard limitation)",
                               field_clicked=True))
        return post, k, ok
    if step["kind"] == "dialog":
        words = ocr_words(pre, (230, 60, 1179, 900))
        cancel = [w for w in words if w[0] == "cancel"]
        if not cancel:
            pre.save(os.path.join(OUT, sid + "-dialog-miss.png"))
            ev("step", id=sid, result="dialog-miss", note="no Cancel word")
            return img, k, False
        cx, cy = cancel[0][1], cancel[0][2]
        # dialog layout (measured in DI1.png): Cancel outline LEFT, danger
        # red "Clear log" RIGHT of it
        bx = by = None
        action = None
        hit = dark_bbox(pre, (cx + 30, cy - 28, cx + 240, cy + 28))
        if hit is not None:
            bx, by, action = hit[0], hit[1], "red-bbox-right-of-cancel"
        else:
            bx, by, action = cx + 75, cy, "geometric-right-of-cancel"
        click(bx, by)
        time.sleep(0.5)
        post = parked_shot()
        ok = finish_click(sid, step, pre, post,
                          dict(action="dialog", button=action, at=[bx, by]))
        return post, k, ok
    if step["kind"] == "home":
        img, _bs, _py = normalize_home(die)
        k = 0
        pre2 = parked_shot()
        hit = find_label(pre2, step["label"], step["crop"])
        if hit is None:
            pre2.save(os.path.join(OUT, sid + "-ocr-miss.png"))
            ev("step", id=sid, result="ocr-miss", label=step["label"])
            return img, k, False
        click(*hit)
        time.sleep(0.35)
        post = parked_shot()
        ok = finish_click(sid, step, pre2, post,
                          dict(action="home+click", label=step["label"],
                               at=list(hit)))
        return post, k, ok
    die(f"unknown step kind {step['kind']}")
    return img, k, False


def main():
    os.makedirs(OUT, exist_ok=True)
    start_from = ""
    if "--from" in sys.argv:
        start_from = sys.argv[sys.argv.index("--from") + 1]

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

    img, bs, py, _ = settled_state()
    if pill(img) is None:
        die("no pill after launch")
    ev("launch", pill_y=py, bands=bs)

    img, _bs, _py = normalize_home(die)
    k = 0
    ev("normalized", page=PAGES[0])

    started = start_from == ""
    results = {}
    for step in STEPS:
        if not started:
            if step["id"] == start_from:
                started = True
            else:
                ev("skip", id=step["id"])
                continue
        if step.get("page") is not None and step["page"] > k:
            img, k = nav_forward(app, die, img, k, step["page"])
            ev("nav", page=PAGES[k], pill_y=pill(img))
        img, k, ok = do_step(app, die, step, img, k)
        results[step["id"]] = ok

    parked_shot(os.path.join(OUT, "ZZ-end-state.png"))
    app.terminate()
    try:
        app.wait(timeout=5)
    except Exception:
        app.kill()
    shutil.copy(APP_LOG, LOG_DST)
    failed = [i for i, ok in results.items() if not ok]
    ev("done", steps=len(results), failed=failed)
    print(f"CTRL-VERIFY COMPLETE: {len(results)} steps, failed={failed}")
    sys.exit(0 if not failed else 3)


if __name__ == "__main__":
    try:
        main()
    finally:
        # never leave an orphan holding the instance lock
        subprocess.run(["pkill", "-f", "target/debug/supportos-plus"],
                       check=False)
