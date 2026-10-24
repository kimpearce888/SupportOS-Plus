#!/usr/bin/env python3
"""W-010 backups/export runtime verification (audit tooling, NOT app code).

Same proven real-input stack as pass-1/pass-2/W-007 (XTEST clicks on Xvfb :99,
ffmpeg x11grab frames, pointer parked off the analyzed content box, pill-
verified sidebar navigation, 4-pass OCR label targeting). Self-contained.

What it exercises (WORKFLOW-COVERAGE W-010; NEXT-ACTIONS #1):
  * Settings -> "Backups & export" tab (C-SET-06; tab triggers reload_backups)
  * "create backup" (C-SET-61): backup_service::backup = VACUUM INTO a new
    supportos-backup-<stamp>.db (+ .settings.json sidecar); toast "Backup
    created (X MB)."; list refreshes (newest first per list_backups sort)
  * "export JSON" (C-SET-62): table dump JSON into the backups dir
  * "export conversations CSV" (C-SET-63)
  * "restore" ARM step (C-SET-64): first click must ONLY arm the two-step
    confirmation (red "REPLACED" warning appears; nothing is swapped)
  * "cancel" (C-SET-65): disarms

DELIBERATE NON-EXECUTION (recorded, not silent):
  * "really restore" (C-SET-66) is NOT clicked — NEXT-ACTIONS explicitly
    defers restore-replace until QA data exists (W-005/W-006/W-002 will
    create it); the armed-state warning text IS captured as runtime evidence
    of the two-step gate (audit item 11 claim), and backup_service::restore
    swap mechanics are source-verified only at this point.
  * never-synced precondition untouched (no sync buttons anywhere near this
    flow); theme untouched.

Tooling history (recorded, both runs retained in the events JSONL):
  * run 1: the S004 tab click at OCR window center (606,93) produced a
    pixel-identical no-op (diff 0.0); a live probe immediately afterwards at
    the SAME coordinates switched the tab (diff 4.096) — transient XTEST
    event-delivery fluke, not an app defect.
  * run 2: find_label v1 mis-located the tab at (456,82) — its loose crop
    included the 'Settings' page title on a merged OCR line, duplicate words
    from the 4-pass merge joined title→tab, and the (mis)ranking preferred
    the WIDEST window. Fixed in find_label v2: same-visual-row constraint
    (dy <= 5 px) + tightest-pixel-window ranking (the v1 comment claimed
    "tightest" but the sort did the opposite).
  * all click steps now VERIFY their effect and retry with varied positions
    (every attempt recorded in the events JSONL).

Run: bash start-xvfb.sh && python3 w010-backups.py
"""
import glob
import hashlib
import json
import os
import shutil
import sqlite3
import subprocess
import sys
import time

from PIL import Image, ImageChops, ImageOps
from Xlib import X, display
from Xlib.ext import xtest

REPO = "/home/z/my-project/audit/SupportOS-Plus"
AUDIT = os.path.join(REPO, "repo-audit-2026-10-24")
OUT = os.path.join(AUDIT, "evidence/screenshots/WORKFLOW")
APP_LOG = "/tmp/w10-app.log"
LOG_DST = os.path.join(AUDIT, "evidence/logs/W10-app.log")
EVENTS = os.path.join(AUDIT, "evidence/artifacts/control-verify-events.jsonl")
DB = "/home/z/.local/share/supportos-plus/supportos-plus.db"
BACKUPS_DIR = "/home/z/.local/share/supportos-plus/backups"

SHOT_W, SHOT_H = 1600, 1000
PARK = (785, 491)
CONTENT_BOX = (230, 60, 1150, 700)
PAGES = ["01-dashboard", "02-inbox", "03-live-chats", "04-operations",
         "05-customers", "06-organizations", "07-knowledge", "08-reports",
         "09-issue-radar", "10-incidents", "11-custom-objects", "12-automation",
         "13-ai-center", "14-outreach", "15-notifications", "16-search",
         "17-support-graph", "18-connectors", "19-diagnostics", "20-sync-health",
         "21-settings", "22-onboarding", "23-shell"]
CLICK_MAX_Y = 700
SCAN_Y1 = 745
PILL_X = 185
PILL_LUM = 247
BAND_THR = 100
BAND_X0, BAND_X1 = 45, 150
BAND_Y0 = 125
TOL = 4
SETTINGS_IDX = PAGES.index("21-settings")

events = open(EVENTS, "a")


def ev(kind, **kw):
    rec = {"t": round(time.time(), 3), "kind": kind, **kw}
    events.write(json.dumps(rec) + "\n")
    events.flush()
    print(json.dumps(rec), flush=True)


def die(msg):
    ev("w10-fatal", error=msg)
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
    tmp = path or "/tmp/w10-shot.png"
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


def diff_mean(a, b, box=CONTENT_BOX):
    diff = ImageChops.difference(
        a.convert("L").crop(box), b.convert("L").crop(box))
    hist = diff.histogram()
    total = sum(hist)
    mean = sum(i * c for i, c in enumerate(hist)) / max(total, 1)
    return round(mean, 3)


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
    base = img.crop(crop)
    words = []

    def run_ocr(psm, invert=False, scale=1):
        im = base
        if scale != 1:
            im = im.resize((im.width * scale, im.height * scale))
        if invert:
            im = ImageOps.invert(im.convert("L")).convert("RGB")
        im.save("/tmp/w10-ocr.png")
        subprocess.run(
            ["tesseract", "/tmp/w10-ocr.png", "/tmp/w10-ocr-out",
             "--psm", str(psm), "tsv"],
            check=True, capture_output=True)
        return _tsv_to_words(crop, "/tmp/w10-ocr-out.tsv", scale=scale)

    for psm, inv, sc in [(6, False, 1), (6, True, 1), (6, False, 3),
                         (11, False, 2)]:
        try:
            words += run_ocr(psm, inv, sc)
        except (subprocess.CalledProcessError, FileNotFoundError):
            pass
    return words


def find_label(img, needle, crop, first=False):
    """Exact word first; else minimal sliding window whose join contains
    needle; click aims at the window center. Returns (x, y) or None.

    v2 (run-2 fix, recorded): candidates are restricted to words on the
    SAME visual row (|dy| <= 5 px within the window) and windows are
    ranked by pixel width (tightest first) — run 1's loose crop included
    the 'Settings' page title on a merged OCR line, and the widest-window
    ranking picked a window that spanned from the title into the tab row
    (click center 456 px left of the real chip)."""
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
                if ws[j][2] - ws[i][2] > 5:   # same visual row only
                    break
                join += ws[j][0]
                if needle in join and len(join) < len(needle) * 3 + 12:
                    width = ws[j][1] - ws[i][1]
                    wins.append((width, ws[i][1], ws[j][1],
                                 (ws[i][2] + ws[j][2]) // 2))
                    break
    if not wins:
        return None
    wins.sort()                      # tightest pixel window, then leftmost
    return (wins[0][1] + wins[0][2]) // 2, wins[0][3]


def dark_button(img, crop, min_row_dark=40, min_h=22, min_w=50,
                rightmost=False):
    """Find a solid dark button rectangle in the crop (audit-tooling
    detector; Primary buttons with a leading icon defeat OCR by merging
    the icon into the first glyph). Returns the cluster center or None."""
    g = img.crop(crop).convert("L")
    px = g.load()
    w, h = g.size
    dark_rows = []
    for y in range(h):
        cnt = sum(1 for x in range(w) if px[x, y] < 140)
        dark_rows.append(cnt)
    bands_ = []
    start = None
    for y in range(h + 1):
        v = y < h and dark_rows[y] >= min_row_dark
        if v and start is None:
            start = y
        elif not v and start is not None:
            if y - start >= min_h:
                bands_.append((start, y - 1))
            start = None
    best = None
    for (y0, y1) in bands_:
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
                    if not rightmost:
                        if best is None or (cand[1] - cand[0]) > (best[1] - best[0]):
                            best = cand
                    else:
                        if best is None or cand[0] > best[0]:
                            best = cand
                run_start = None
    if best is None:
        return None
    x0, x1, y0, y1 = best
    return (crop[0] + (x0 + x1) // 2, crop[1] + (y0 + y1) // 2)


def red_button(img, crop, min_px=50):
    """Count saturated-red pixels in the crop (audit-tooling detector for
    Destructive buttons; their small white-on-red label defeats the OCR
    passes in a large crop). Returns the pixel count and bbox center."""
    im = img.crop(crop).convert("RGB")
    px = im.load()
    w, h = im.size
    pts = [(x, y) for y in range(h) for x in range(w)
           if px[x, y][0] > 150 and px[x, y][1] < 110 and px[x, y][2] < 110]
    if len(pts) < min_px:
        return 0, None
    xs = [p[0] for p in pts]
    ys = [p[1] for p in pts]
    return len(pts), (crop[0] + (min(xs) + max(xs)) // 2,
                      crop[1] + (min(ys) + max(ys)) // 2)


def ocr_contains(img, needle, crop):
    needle = norm(needle)
    words = ocr_words(img, crop)
    joined = "".join(w[0] for w in words)
    return needle in joined


def click_until(img, pos, predicate, tries=3, settle=1.0):
    """Click, verify `predicate(frame)`, retry with slight position
    variation on a no-op (transient XTEST event-loss guard; every attempt
    is recorded by the caller via the returned attempt list)."""
    attempts = []
    variations = [pos, (pos[0] - 29, pos[1]), (pos[0] + 29, pos[1] - 1),
                  (pos[0], pos[1] - 5)]
    frame = img
    for i, p in enumerate(variations[:tries] if tries <= 4 else variations):
        click(*p)
        time.sleep(settle)
        frame = parked_shot()
        ok = predicate(frame)
        attempts.append({"pos": p, "ok": ok})
        if ok:
            return frame, attempts, True
        time.sleep(0.4)
    return frame, attempts, False


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
    logf.write(f"\n===== W-010 {tag} =====\n")
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
        tail = t.split(f"===== W-010 {tag} =====")[-1]
        if "cannot acquire the instance lock" in tail:
            lock_err = True
            break
        if "migrations" in tail or "loopback" in tail:
            ready = True
            break
    time.sleep(4 if ready else 8)
    ev("w10-app-started", tag=tag, pid=p.pid, ready=ready, lock_error=lock_err)
    if lock_err or not ready:
        die(f"app failed to start for {tag}")
    return p


def dir_state():
    out = {}
    for f in sorted(glob.glob(BACKUPS_DIR + "/*")):
        st = os.stat(f)
        out[os.path.basename(f)] = st.st_size
    return out


def db_counts():
    conn = sqlite3.connect(DB)
    try:
        keys = ["organizations", "conversations", "sync_runs", "jobs",
                "knowledge_documents", "automation_rules", "customers"]
        counts = {k: conn.execute(
            f"SELECT COUNT(*) FROM {k}").fetchone()[0] for k in keys}
        theme = conn.execute(
            "SELECT value FROM application_settings WHERE key='theme'"
        ).fetchone()
        counts["theme"] = theme[0] if theme else None
        counts["integrity"] = conn.execute(
            "PRAGMA integrity_check").fetchone()[0]
        return counts
    finally:
        conn.close()


def nav_to_settings(img, k_cur):
    """Proven walk algorithm (control-verify.py nav_forward) to Settings."""
    scrolls = 0
    while k_cur < SETTINGS_IDX:
        bs = bands(img)
        py = pill(img)
        if py is None:
            die(f"pill lost while navigating to {PAGES[SETTINGS_IDX]}")
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
                    die("sidebar at max scroll before Settings")
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


def main():
    os.makedirs(OUT, exist_ok=True)

    # S001 — pre-state: backups dir + DB counters.
    pre_dir = dir_state()
    pre_counts = db_counts()
    ev("w10-S001-pre", backups_dir=pre_dir, db=pre_counts)

    # S002 — launch.
    p = start_app("launch-1")
    time.sleep(2)
    img = parked_shot()
    bs = bands(img)
    py = pill(img)
    if not bs or py is None or abs(py - bs[0]) > TOL:
        die(f"app did not open on Dashboard at top of sidebar (pill={py})")
    k_cur = 0

    # S003 — walk to Settings (pill-verified each step).
    img, k_cur = nav_to_settings(img, k_cur)
    img.save(os.path.join(OUT, "W10-01-settings.png"))
    ev("w10-S003-settings", nav_to=PAGES[SETTINGS_IDX],
       pill_y=pill(img), verdict="PASS")

    # S004 — open "Backups & export" tab; list shows the boot backup pair.
    pos = find_label(img, "backupsexport", (230, 55, 1179, 125))
    if not pos:
        die("Backups & export tab not found by OCR")
    pre = img

    def tab_open(frame):
        # NOTE: 'create backup' is a Primary button WITH a leading icon —
        # the icon merges into the first glyph and defeats OCR (pass-2
        # known limitation), so the open-check uses the icon-free
        # 'export JSON' button + the big view diff instead.
        return diff_mean(pre, frame) > 0.1 and \
            ocr_contains(frame, "exportjson", (230, 60, 1179, 400))

    img, atts, ok = click_until(img, pos, tab_open)
    img.save(os.path.join(OUT, "W10-02-backups-tab.png"))
    has_row = ocr_contains(img, "supportosbackup", (230, 60, 1179, 760))
    ev("w10-S004-tab", attempts=atts, list_shows_boot_backup=has_row,
       verdict="PASS" if (ok and has_row) else "FAIL")
    if not (ok and has_row):
        die("S004: Backups tab open verification failed")

    # S005 — create backup: new pair on disk + toast + list row.
    pre_dir = dir_state()
    pos = dark_button(img, (230, 130, 700, 260))
    if not pos:
        pos = find_label(img, "createbackup", (230, 130, 700, 260))
    if not pos:
        die("create backup button not found (dark-rect detector + OCR)")

    def made(frame):
        cur = dir_state()
        return any(f.endswith(".db") and f not in pre_dir for f in cur)

    img, atts, ok = click_until(img, pos, made)
    toast = shot()          # immediate, unparked: catch the toast
    toast_hit = ocr_contains(toast, "backupcreated", (230, 560, 1179, 1000))
    time.sleep(1.5)
    img = parked_shot(os.path.join(OUT, "W10-03-after-create.png"))
    post_dir = dir_state()
    new_db = [f for f in post_dir if f.endswith(".db") and f not in pre_dir]
    new_json = [f for f in post_dir if f.endswith(".settings.json")
                and f not in pre_dir]
    ok = bool(new_db) and bool(new_json) and \
        post_dir[new_db[0]] > 1000 and \
        ocr_contains(img, "supportosbackup", (230, 60, 1179, 760))
    ev("w10-S005-create-backup", attempts=atts, new_db=new_db,
       new_settings_json=new_json,
       new_db_bytes=post_dir.get(new_db[0]) if new_db else None,
       toast_captured=toast_hit, verdict="PASS" if ok else "FAIL")
    if not ok:
        die("S005: create backup verification failed")

    # S006 — export JSON.
    pre_dir = dir_state()
    pos = find_label(img, "exportjson", (230, 60, 1179, 400))
    if not pos:
        die("export JSON button not found by OCR")

    def exported(frame):
        return any(f not in pre_dir for f in dir_state())

    img, atts, ok = click_until(img, pos, exported)
    img.save(os.path.join(OUT, "W10-04-after-export-json.png"))
    post_dir = dir_state()
    new = [f for f in post_dir if f not in pre_dir]
    ev("w10-S006-export-json", attempts=atts, new_files=new,
       verdict="PASS" if ok else "FAIL")
    if not ok:
        die("S006: export JSON produced no file")

    # S007 — export conversations CSV.
    pre_dir = dir_state()
    pos = find_label(img, "exportconversationscsv", (230, 60, 1179, 400))
    if not pos:
        die("export conversations CSV button not found by OCR")

    def exported2(frame):
        return any(f not in pre_dir for f in dir_state())

    img, atts, ok = click_until(img, pos, exported2)
    img.save(os.path.join(OUT, "W10-05-after-export-csv.png"))
    post_dir = dir_state()
    new = [f for f in post_dir if f not in pre_dir]
    ev("w10-S007-export-csv", attempts=atts, new_files=new,
       verdict="PASS" if ok else "FAIL")
    if not ok:
        die("S007: export CSV produced no file")

    # S008 — ARM the restore on the newest backup row; NOTHING may be
    # swapped; red REPLACED warning must appear.
    pre_counts = db_counts()
    pos = find_label(img, "restore", (230, 200, 1179, 760), first=True)
    if not pos:
        die("restore row button not found by OCR")

    def armed_fn(frame):
        return ocr_contains(frame, "replaced", (230, 60, 1179, 760))

    img, atts, ok_arm = click_until(img, pos, armed_fn)
    img.save(os.path.join(OUT, "W10-06-restore-armed.png"))
    has_cancel = ocr_contains(img, "cancel", (230, 360, 700, 470))
    red_count, red_center = red_button(img, (240, 370, 600, 440))
    has_really = red_count >= 50
    post_counts = db_counts()
    stable_db = all(post_counts[k] == pre_counts[k] for k in pre_counts
                    if k not in ("integrity",))
    tmp_files = glob.glob(DB + ".restore-tmp*") + glob.glob(DB + ".restore-prev*")
    ok = ok_arm and has_cancel and has_really and stable_db and not tmp_files
    ev("w10-S008-restore-armed", attempts=atts,
       armed_warning=ok_arm, cancel_visible=has_cancel,
       really_restore_visible=has_really, red_px=red_count,
       red_center=red_center, db_stable=stable_db,
       swap_files=tmp_files, verdict="PASS" if ok else "FAIL")
    if not ok:
        die(f"S008: arm verification failed (armed={ok_arm} "
            f"cancel={has_cancel} really={has_really} stable={stable_db})")

    # S009 — cancel disarms; still nothing swapped.
    pos = find_label(img, "cancel", (230, 360, 700, 470))
    if not pos:
        die("cancel button not found by OCR")

    def disarmed_fn(frame):
        no_text = not ocr_contains(frame, "replaced", (230, 60, 1179, 760))
        red_count, _ = red_button(frame, (240, 370, 600, 440))
        return no_text and red_count < 50

    img, atts, ok_dis = click_until(img, pos, disarmed_fn)
    img.save(os.path.join(OUT, "W10-07-after-cancel.png"))
    ev("w10-S009-cancel", attempts=atts, disarmed=ok_dis,
       verdict="PASS" if ok_dis else "FAIL",
       note="'really restore' deliberately NOT clicked (deferred until QA "
            "data exists per NEXT-ACTIONS); two-step gate runtime-verified "
            "up to the cancel branch")
    if not ok_dis:
        die("S009: cancel did not disarm the restore")

    # S010 — end state: DB integrity + counters + theme unchanged.
    p.terminate()
    time.sleep(2)
    gone = wait_proc_gone()
    end_counts = db_counts()
    end_dir = dir_state()
    same_counts = all(end_counts[k] == pre_counts[k]
                      for k in pre_counts if k != "integrity")
    ok = gone and end_counts["integrity"] == "ok" and same_counts
    ev("w10-S010-end", proc_gone=gone, db=end_counts, backups_dir=end_dir,
       counts_unchanged=same_counts,
       verdict="PASS" if ok else "FAIL",
       note="never-synced precondition intact; theme unchanged")
    if not ok:
        die("S010: end-state verification failed")

    ev("w10-summary", verdict="PASS", steps="S001..S010",
       executed="C-SET-06 tab, C-SET-61 create backup, C-SET-62 export JSON, "
                "C-SET-63 export CSV, C-SET-64 arm, C-SET-65 cancel",
       deferred="C-SET-66 'really restore' (per NEXT-ACTIONS: restore-replace "
                "deferred until QA data exists; two-step gate verified)")
    finish(0)


if __name__ == "__main__":
    try:
        main()
    except SystemExit:
        raise
    except Exception as e:  # noqa: BLE001 — audit tooling: record and stop
        die(f"unhandled: {type(e).__name__}: {e}")
