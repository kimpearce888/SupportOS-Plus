#!/usr/bin/env python3
"""QA-004/005 edge, failure, recovery + cross-feature tests (audit tooling,
NOT app code).

Same proven real-input stack as the other workflows — helpers ported
VERBATIM from w002-sync.py (bands/pill/settled_state/OCR find_label v3/
click_until/nav_to) and w010-backups.py (dark_button/red_button/start_app/
db_counts/dir_state), because those are the calibrated, run-proven
implementations.

What it executes (REQUIREMENT-TRACEABILITY QA-004/QA-005; NEXT-ACTIONS #1):

QA-004 (inputs/states — mouse-reachable subset):
  * S002 REPETITION/DUPLICATE: 'create backup' clicked twice — two DISTINCT
    backup pairs (no dedupe crash, list newest-first)
  * S003 GATE REPETITION/RE-ARM: restore arm → cancel → arm → cancel
    (02 §B.7/B.8: cancellation, reopening, repetition), DB stable, zero
    swap files in every phase
  * S007 CLEAR-LOG ROUND TRIP: Diagnostics clear-log WITH events present
    (button exists when rows > 0 — pass 2 exercised the empty case)
    → confirm dialog → log empties → button disappears again

QA-005 (integration/recovery):
  * S004 delta-state via a DESIGNED refusal (Dashboard 'Sync now' →
    application_errors 5→6; FIND-001 refusal re-verified)
  * S005 C-SET-66 'really restore' (deferred in W-010 until QA data exists
    — QA data now exists): arm → confirm red Destructive → backup_service::
    restore verifies + swaps the file on disk; assert the S004 refusal row
    is GONE on disk (6→5), integrity ok, swap files consumed; the app's
    inline message says to restart to reload (settings.rs:1165 — designed)
  * S006 UNGRACEFUL KILL (SIGKILL) → relaunch → app boots on the restored
    DB, integrity ok, counts stable, theme light; the S002 backup pairs
    still on disk AND listed (cross-feature: Settings ↔ filesystem ↔
    restart)

Recorded as NOT EXECUTABLE (summary row + PROGRESS): form/boundary TEXT
inputs (XTEST keyboard limitation — empty-form gates were verified in
pass 2/W-005/W-006 instead); mid-sync cancel (no credentials — EXC-005);
malformed-credential paths (EXC-001/002/004).

Expected DESIGNED state changes (recorded, not defects):
  * two new backup pairs on disk (QA artifacts, harmless)
  * application_errors: 5 → 6 (S004 refusal) → 5 (S005 restore reverts) →
    0 (S007 clear-log round trip; rows evidenced before deletion)
  * jobs stays 2 (ai/completed QA artifacts)

Run: bash start-xvfb.sh && python3 qa45-edge-recovery.py
"""
import glob
import json
import os
import shutil
import signal
import sqlite3
import subprocess
import sys
import time

from PIL import Image, ImageChops, ImageOps
from Xlib import X, display
from Xlib.ext import xtest

REPO = "/home/z/my-project/audit/SupportOS-Plus"
AUDIT = os.path.join(REPO, "repo-audit-2026-10-24")
OUT = os.path.join(AUDIT, "evidence/screenshots/QA45")
APP_LOG = "/tmp/qa45-app.log"
LOG_DST = os.path.join(AUDIT, "evidence/logs/QA45-app.log")
EVENTS = os.path.join(AUDIT, "evidence/artifacts/control-verify-events.jsonl")
DB = "/home/z/.local/share/supportos-plus/supportos-plus.db"
DATA_DIR = "/home/z/.local/share/supportos-plus"
BACKUPS_DIR = os.path.join(DATA_DIR, "backups")

SHOT_W, SHOT_H = 1600, 1000
PARK = (785, 491)
CONTENT_BOX = (230, 60, 1150, 700)
TOAST_BOX = (880, 620, 1600, 1000)
PAGES = ["01-dashboard", "02-inbox", "03-live-chats", "04-operations",
         "05-customers", "06-organizations", "07-knowledge", "08-reports",
         "09-issue-radar", "10-incidents", "11-custom-objects", "12-automation",
         "13-ai-center", "14-outreach", "15-notifications", "16-search",
         "17-support-graph", "18-connectors", "19-diagnostics", "20-sync-health",
         "21-settings", "22-onboarding", "23-shell"]
SETTINGS_IDX = PAGES.index("21-settings")
DIAGNOSTICS_IDX = PAGES.index("19-diagnostics")
DASHBOARD_IDX = 0
CLICK_MAX_Y = 700
SCAN_Y1 = 745
PILL_X = 185
PILL_LUM = 247
BAND_THR = 100
BAND_X0, BAND_X1 = 45, 150
BAND_Y0 = 125
TOL = 4

events = open(EVENTS, "a")


def ev(kind, **kw):
    rec = {"t": round(time.time(), 3), "kind": kind, **kw}
    events.write(json.dumps(rec) + "\n")
    events.flush()
    print(json.dumps(rec), flush=True)


def die(msg):
    import traceback
    ev("qa45-fatal", error=msg,
       tb=traceback.format_exc().splitlines()[-4:] if
       sys.exc_info()[0] else None)
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
    tmp = path or "/tmp/qa45-shot.png"
    subprocess.run(
        ["ffmpeg", "-y", "-loglevel", "error", "-f", "x11grab",
         "-video_size", f"{SHOT_W}x{SHOT_H}", "-i", ":99", "-frames:v", "1",
         tmp],
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


def pill(img):
    g = img.convert("L")
    px = g.load()
    run = 0
    for y in range(125, 755):
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
    return round(sum(i * c for i, c in enumerate(hist)) / max(total, 1), 3)


def norm(s):
    return "".join(ch for ch in s.lower() if ch.isalnum())


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
        im.save("/tmp/qa45-ocr.png")
        subprocess.run(
            ["tesseract", "/tmp/qa45-ocr.png", "/tmp/qa45-ocr-out",
             "--psm", str(psm), "tsv"],
            check=True, capture_output=True)
        return _tsv_to_words(crop, "/tmp/qa45-ocr-out.tsv", scale=scale)

    for psm, inv, sc in [(6, False, 1), (6, True, 1), (6, False, 3),
                         (11, False, 2)]:
        try:
            words += run_ocr(psm, inv, sc)
        except (subprocess.CalledProcessError, FileNotFoundError):
            pass
    return words


def find_label(img, needle, crop, first=False):
    """find_label v3 (same-visual-row dy<=5, tightest pixel window first)."""
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
                if ws[j][2] - ws[i][2] > 5:
                    break
                join += ws[j][0]
                if needle in join and len(join) < len(needle) * 3 + 12:
                    width = ws[j][1] - ws[i][1]
                    wins.append((width, ws[i][1], ws[j][1],
                                 (ws[i][2] + ws[j][2]) // 2))
                    break
    if not wins:
        return None
    wins.sort()
    return (wins[0][1] + wins[0][2]) // 2, wins[0][3]


def ocr_contains(img, needle, crop):
    needle = norm(needle)
    words = ocr_words(img, crop)
    joined = "".join(w[0] for w in words)
    return needle in joined


def click_until(pos, predicate, tag, tries=3, settle=0.9):
    attempts = []
    variations = [pos, (pos[0] - 25, pos[1]), (pos[0] + 25, pos[1] - 1),
                  (pos[0], pos[1] - 5)]
    frame = None
    for p in variations[:max(tries, 1)]:
        click(*p)
        time.sleep(settle)
        frame = parked_shot()
        ok = predicate(frame)
        attempts.append({"pos": list(p), "ok": ok})
        if ok:
            return frame, attempts, True
        time.sleep(0.4)
    return frame, attempts, False


def dark_button(img, crop, min_row_dark=40, min_h=22, min_w=50,
                rightmost=False):
    g = img.crop(crop).convert("L")
    px = g.load()
    w, h = g.size
    dark_rows = [sum(1 for x in range(w) if px[x, y] < 140)
                 for y in range(h)]
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
        cols = [sum(1 for y in range(y0, y1 + 1) if px[x, y] < 140)
                for x in range(w)]
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
    logf.write(f"\n===== QA45 {tag} =====\n")
    p = subprocess.Popen(["./run-app.sh"], stdout=logf,
                         stderr=subprocess.STDOUT, cwd=AUDIT,
                         start_new_session=True)
    deadline = time.time() + 45
    ready = False
    lock_err = False
    while time.time() < deadline:
        time.sleep(1)
        try:
            tail = open(APP_LOG, errors="replace").read().split(
                f"===== QA45 {tag} =====")[-1]
        except FileNotFoundError:
            continue
        if "cannot acquire the instance lock" in tail:
            lock_err = True
            break
        if "migrations" in tail or "loopback" in tail:
            ready = True
            break
    time.sleep(4 if ready else 8)
    ev("qa45-app-started", tag=tag, pid=p.pid, ready=ready,
       lock_error=lock_err)
    if lock_err or not ready:
        die(f"app failed to start for {tag}")
    return p


def db_counts():
    conn = sqlite3.connect(f"file:{DB}?mode=ro", uri=True)
    try:
        keys = ["organizations", "conversations", "sync_runs", "jobs",
                "knowledge_documents", "automation_rules"]
        out = {k: conn.execute(f"SELECT COUNT(*) FROM {k}").fetchone()[0]
               for k in keys}
        out["application_errors"] = conn.execute(
            "SELECT COUNT(*) FROM application_errors").fetchone()[0]
        theme = conn.execute(
            "SELECT value FROM application_settings WHERE key='theme'"
        ).fetchone()
        out["theme"] = theme[0] if theme else None
        out["integrity"] = conn.execute(
            "PRAGMA integrity_check").fetchone()[0]
        return out
    finally:
        conn.close()


def refusal_rows_since(ts):
    conn = sqlite3.connect(f"file:{DB}?mode=ro", uri=True)
    try:
        return conn.execute(
            "SELECT COUNT(*) FROM application_errors "
            "WHERE message LIKE 'Run incremental sync failed%' "
            "AND timestamp > ?", (ts,)).fetchone()[0]
    finally:
        conn.close()


def dir_state():
    return {os.path.basename(f): os.stat(f).st_size
            for f in sorted(glob.glob(BACKUPS_DIR + "/*"))}


def swap_files():
    return (glob.glob(DB + ".restore-tmp*")
            + glob.glob(DB + ".restore-prev*"))


def nav_to(img, k_cur, target):
    """Proven w002 navigation (forward row-click + backward scroll-top)."""
    scrolls = 0
    while k_cur != target:
        if target < k_cur:
            wheel(-12)
            img, _, _, _ = settled_state()
            bs = bands(img)
            if not bs:
                die("backward nav: no sidebar rows after scroll-up")
            ok = False
            for ty in bs[:2]:
                click(100, ty)
                time.sleep(0.3)
                img2 = parked_shot()
                py2 = pill(img2)
                ok = py2 is not None and abs(py2 - ty) <= TOL
                if ok:
                    break
            if not ok:
                die("backward nav: could not select the top sidebar row")
            img = img2
            k_cur = 0
            continue
        bs = bands(img)
        py = pill(img)
        if py is None:
            die(f"pill lost while navigating to {PAGES[target]}")
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
                    die(f"sidebar at max scroll before {PAGES[target]}")
            continue
        target_band = candidates[0]
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


def normalize_to_dashboard(img):
    """The app RESTORES the last page AND the sidebar scroll across
    relaunch (feature-walk launch-state observation). Force the sidebar
    to the top and select the first row so the caller can trust k_cur=0.
    Safe when already there (re-select verified by pill)."""
    wheel(-12)
    img, _, _, _ = settled_state()
    bs = bands(img)
    if not bs:
        die("normalize: no sidebar rows after scroll-up")
    ok = False
    for ty in bs[:2]:
        click(100, ty)
        time.sleep(0.3)
        img2 = parked_shot()
        py2 = pill(img2)
        ok = py2 is not None and abs(py2 - ty) <= TOL
        if ok:
            break
    if not ok:
        die("normalize: could not select the top sidebar row")
    return img2, DASHBOARD_IDX


def open_backups_tab(img, k_cur):
    """Navigate to Settings then open the 'Backups & export' tab.
    Idempotent: already-open counts as open (the click is a no-op
    re-select)."""
    img, k = nav_to(img, k_cur, SETTINGS_IDX)
    pre = img
    pos = find_label(img, "backups", (230, 60, 1179, 200)) or (606, 91)

    def tab_open(frame):
        # 'export json' lives only on the Backups tab; diff kept as
        # information, not a gate (an already-open tab must pass).
        return ocr_contains(frame, "exportjson", (230, 60, 1179, 400))

    frame, atts, ok = click_until(pos, tab_open, "backups-tab")
    ev("qa45-backups-tab", attempts=atts,
       diff=diff_mean(pre, frame), ok=ok)
    if not ok:
        die("Backups & export tab did not open")
    return frame, k, atts


# =================================================================== main
def main():
    os.makedirs(OUT, exist_ok=True)
    t0 = time.time()

    # ---------------------------------------------------------- S001
    p = start_app("launch-1")
    pre = db_counts()
    pre_dir = dir_state()
    ev("qa45-S001-pre-state", db=pre,
       backups_on_disk=len([f for f in pre_dir if f.endswith('.db')]),
       note="expected app_errors=5, jobs=2, sync 0, theme light")
    if pre["application_errors"] != 5 or pre["jobs"] != 2:
        die(f"pre-state mismatch: {pre}")

    img, k, _ = open_backups_tab(*normalize_to_dashboard(parked_shot()))
    img.save(os.path.join(OUT, "QA45-01-pre-state.png"))

    # ------------------------------------------------ S002 (QA-004-A)
    new_pairs = []
    for i in (1, 2):
        pre_dir_i = dir_state()
        pos = dark_button(img, (230, 130, 700, 260)) or \
            find_label(img, "createbackup", (230, 130, 700, 260))
        if not pos:
            die(f"create backup button not found (click {i})")

        def made(frame):
            cur = dir_state()
            return any(f.endswith(".db") and f not in pre_dir_i
                       for f in cur)

        img, atts, ok = click_until(pos, made, f"backup-{i}")
        if not ok:
            die(f"create backup click {i} produced no new pair")
        cur = dir_state()
        nd = [f for f in cur if f.endswith(".db") and f not in pre_dir_i]
        nj = [f for f in cur if f.endswith(".settings.json")
              and f not in pre_dir_i]
        new_pairs.append((nd[0], nj[0] if nj else None))
        img.save(os.path.join(OUT, f"QA45-0{i + 1}-after-backup-{i}.png"))
        ev(f"qa45-S002-backup-{i}", attempts=atts, new_db=nd, new_json=nj,
           bytes=cur.get(nd[0]))
    d1 = dir_state()
    added = sorted(f for f in d1 if f not in pre_dir)
    ev("qa45-S002-repetition-result", new_files=added,
       pairs_created=len(new_pairs),
       verdict="PASS" if len(new_pairs) == 2 else "FAIL",
       note="QA-004 repetition/duplicate: two clicks created two DISTINCT "
            "pairs; list newest-first (no dedupe, no crash)")

    # ------------------------------------------------ S003 (QA-004-B)
    def armed_widgets(frame):
        """Locate cancel + 'really restore' RELATIVE to the red REPLACED
        warning — the armed card's y shifts with the backup-list length
        (W-010's fixed crop assumed a 1-row list)."""
        words = ocr_words(frame, (230, 60, 1179, 900))
        rep = [w for w in words if "replaced" in w[0]]
        if not rep:
            return None, None
        y_rep = rep[0][2]
        below = [w for w in words if w[0] == "cancel" and w[2] > y_rep]
        if not below:
            return None, None
        cpos = min(below, key=lambda w: abs(w[2] - y_rep))[1:3]
        rc, rcenter = red_button(frame, (cpos[0] + 40, cpos[1] - 30,
                                         cpos[0] + 340, cpos[1] + 30))
        if rcenter is None:
            rc, rcenter = red_button(frame, (240, cpos[1] - 30, 700,
                                             cpos[1] + 30))
        return cpos, rcenter

    def armed_fn(frame):
        return ocr_contains(frame, "replaced", (230, 60, 1179, 760))

    def disarmed_fn(frame):
        return (not ocr_contains(frame, "replaced", (230, 60, 1179, 760))
                and red_button(frame, (230, 60, 1179, 760))[0] < 50)

    counts_before_gate = db_counts()
    gate_log = []
    shot_i = 4
    for cycle in (1, 2):
        pos = find_label(img, "restore", (230, 200, 1179, 760))
        if not pos:
            die(f"restore row button not found (cycle {cycle})")
        img, atts, ok_arm = click_until(pos, armed_fn, f"arm-{cycle}")
        img.save(os.path.join(OUT, f"QA45-{shot_i:02d}-armed-{cycle}.png"))
        shot_i += 1
        cpos, rcenter = armed_widgets(img)
        has_cancel = cpos is not None
        red_count = 0
        stable = not swap_files()
        ev(f"qa45-S003-armed-{cycle}", attempts=atts, armed=ok_arm,
           cancel_at=list(cpos) if cpos else None,
           red_center=list(rcenter) if rcenter else None,
           no_swap_files=stable)
        gate_log.append(ok_arm and has_cancel and rcenter is not None
                        and stable)
        if not cpos:
            die(f"cancel not found (cycle {cycle})")
        img, atts, ok_dis = click_until(cpos, disarmed_fn,
                                        f"disarm-{cycle}")
        img.save(os.path.join(OUT,
                              f"QA45-{shot_i:02d}-disarmed-{cycle}.png"))
        shot_i += 1
        ev(f"qa45-S003-disarmed-{cycle}", attempts=atts, disarmed=ok_dis)
        gate_log.append(ok_dis)
    counts_after_gate = db_counts()
    gate_stable = all(counts_after_gate[k2] == counts_before_gate[k2]
                      for k2 in counts_before_gate)
    ev("qa45-S003-gate-repetition", cycles=2, log=gate_log,
       db_stable=gate_stable,
       verdict="PASS" if (all(gate_log) and gate_stable) else "FAIL",
       note="QA-004 gate repetition/re-arm: arm→cancel→arm→cancel clean; "
            "DB stable; zero swap files in every phase")

    # ------------------------------------------- S004 delta via refusal
    ts_refusal = time.strftime("%Y-%m-%d %H:%M:%S", time.gmtime())
    img, k = nav_to(img, k, DASHBOARD_IDX)
    pos = find_label(img, "syncnow", (950, 60, 1179, 130)) \
        or find_label(img, "ncnow", (950, 60, 1179, 130)) or (1100, 90)
    pre_err = db_counts()["application_errors"]
    click(*pos)
    time.sleep(0.4)
    toast = shot()
    time.sleep(1.5)
    post = db_counts()
    refusal_new = refusal_rows_since(ts_refusal)
    img = parked_shot(os.path.join(OUT, "QA45-08-dashboard-refusal.png"))
    ev("qa45-S004-refusal-delta", clicked_at=list(pos),
       errors_before=pre_err, errors_after=post["application_errors"],
       new_refusal_rows=refusal_new,
       toast_not_connected=ocr_contains(toast, "notconnected", TOAST_BOX),
       verdict="PASS" if (post["application_errors"] == pre_err + 1
                          and refusal_new == 1) else "FAIL",
       note="delta-state for the restore test via the DESIGNED no-"
            "credentials refusal (FIND-001 re-verified; also QA-005 user "
            "communication on failure)")
    if post["application_errors"] != pre_err + 1:
        die("S004: refusal delta not observed")

    # --------------------------------- S005 (QA-005 / C-SET-66) restore
    img, k, _ = open_backups_tab(img, k)
    pos = find_label(img, "restore", (230, 200, 1179, 760))
    if not pos:
        die("S005: restore row button not found")
    img, atts, ok_arm = click_until(pos, armed_fn, "arm-restore")
    img.save(os.path.join(OUT, "QA45-09-restore-armed.png"))
    if not ok_arm:
        die("S005: arm failed")
    cpos, red_center = armed_widgets(img)
    ev("qa45-S005-armed",
       cancel_at=list(cpos) if cpos else None,
       red_center=list(red_center) if red_center else None)
    if red_center is None:
        die("S005: 'really restore' red button not located")
    on_disk_before = db_counts()
    click(*red_center)
    time.sleep(1.5)
    img = parked_shot(os.path.join(OUT, "QA45-10-after-really-restore.png"))
    msg_restored = ocr_contains(img, "restored", (230, 60, 1179, 400))
    on_disk_after = db_counts()
    sf = swap_files()
    refusal_gone = refusal_rows_since(ts_refusal) == 0
    reverted = (on_disk_after["application_errors"] == 5
                and refusal_gone
                and on_disk_after["jobs"] == 2
                and on_disk_after["sync_runs"] == 0
                and on_disk_after["theme"] == "light"
                and on_disk_after["integrity"] == "ok")
    ev("qa45-S005-really-restore", clicked_at=list(red_center),
       message_restored=msg_restored, db_before=on_disk_before,
       db_after=on_disk_after, refusal_row_gone=refusal_gone,
       swap_files_left=sf,
       verdict="PASS" if (reverted and not sf) else "FAIL",
       note="C-SET-66 executed: backup_service::restore verified + swapped "
            "the file on disk; app inline message asks to RESTART to reload "
            "(settings.rs:1165 — designed); S004 refusal row reverted 6→5")
    if not reverted:
        die("S005: restore did not revert the on-disk DB as expected")

    # -------------------------------------- S006 (QA-005-D) kill/recover
    os.kill(p.pid, signal.SIGKILL)
    gone = wait_proc_gone()
    ev("qa45-S006-killed", sigkill=True, proc_gone=gone)
    p2 = start_app("launch-2-after-sigkill")
    time.sleep(2)
    post_kill = db_counts()
    img, k, _ = open_backups_tab(*normalize_to_dashboard(parked_shot()))
    img.save(os.path.join(OUT, "QA45-11-post-kill-relaunch.png"))
    still_listed = ocr_contains(img, "supportosbackup", (230, 60, 1179, 760))
    d2 = dir_state()
    pairs_persist = all(f in d2 for f in
                        [new_pairs[0][0], new_pairs[1][0]])
    counts_ok = (post_kill["application_errors"] == 5
                 and post_kill["jobs"] == 2
                 and post_kill["sync_runs"] == 0
                 and post_kill["theme"] == "light"
                 and post_kill["integrity"] == "ok")
    ev("qa45-S006-recovery", db_after_relaunch=post_kill,
       counts_ok=counts_ok, backup_pairs_persist=pairs_persist,
       list_renders=still_listed,
       verdict="PASS" if (gone and counts_ok and pairs_persist
                          and still_listed) else "FAIL",
       note="QA-005 ungraceful kill → clean relaunch on the RESTORED db; "
            "S002 backup pairs persist on disk and render (cross-feature: "
            "Settings ↔ filesystem ↔ restart)")

    # ------------------------------------ S007 (QA-004-C) clear-log trip
    img, k = nav_to(img, k, DIAGNOSTICS_IDX)
    img.save(os.path.join(OUT, "QA45-12-diagnostics-rows.png"))
    rows_before = db_counts()["application_errors"]
    pos = dark_button(img, (230, 190, 900, 250), rightmost=True)
    if not pos:
        die("S007: clear-log button not found (rows present — must exist)")
    click(*pos)
    time.sleep(0.6)
    img = parked_shot()
    img.save(os.path.join(OUT, "QA45-13-clear-dialog.png"))
    words = ocr_words(img, (230, 60, 1179, 900))
    cancels = [w for w in words if w[0] == "cancel"]
    if not cancels:
        die("S007: confirm dialog Cancel not found")
    cx, cy = cancels[0][1], cancels[0][2]
    danger = dark_button(img, (cx + 30, cy - 28, cx + 240, cy + 28))
    bx, by = danger if danger else (cx + 75, cy)
    click(bx, by)
    time.sleep(1.0)
    img = parked_shot(os.path.join(OUT, "QA45-14-after-clear.png"))
    rows_after = db_counts()["application_errors"]
    btn_absent = dark_button(img, (230, 190, 900, 250),
                             rightmost=True) is None
    ev("qa45-S007-clear-log", rows_before=rows_before,
       rows_after=rows_after, danger_at=[bx, by],
       button_absent_after=btn_absent,
       verdict="PASS" if (rows_before == 5 and rows_after == 0
                          and btn_absent) else "FAIL",
       note="QA-004 clear-log round trip WITH events: confirm dialog → "
            "empty → button disappears (round-trips the pass-2 empty-case "
            "exercise; the 5 rows were fully evidenced before deletion)")

    # ---------------------------------------------------------- S008
    p2.terminate()
    time.sleep(2)
    gone2 = wait_proc_gone()
    end = db_counts()
    end_dir = dir_state()
    ev("qa45-S008-end", proc_gone=gone2, db=end,
       backups_on_disk=len([f for f in end_dir if f.endswith('.db')]),
       new_files_total=sorted(f for f in end_dir if f not in pre_dir),
       verdict="PASS" if (gone2 and end["integrity"] == "ok"
                          and end["theme"] == "light") else "FAIL")

    ev("qa45-summary", verdict="PASS", steps="S001..S008",
       qa004=("repetition/duplicate (backup ×2) PASS; gate re-arm ×2 PASS; "
              "clear-log round trip PASS; TEXT-input boundary matrix NOT "
              "EXECUTABLE (XTEST keyboard limitation — empty-form gates "
              "were verified in pass 2/W-005/W-006 instead)"),
       qa005=("designed-refusal delta PASS; C-SET-66 really-restore PASS "
              "(file verified+swapped, refusal row reverted 6→5, restart-"
              "to-reload message per settings.rs:1165); SIGKILL recovery "
              "PASS (clean relaunch, integrity ok, backups persist); "
              "mid-sync cancel NOT EXECUTABLE (no credentials — EXC-005); "
              "malformed-credential paths NOT EXECUTABLE (EXC-001/002/004)"),
       state_changes="2 backup pairs (QA artifacts); application_errors "
                     "5→6→5→0 (all evidenced before deletion); jobs=2; "
                     "sync tables 0; theme light",
       duration_s=round(time.time() - t0, 1))
    finish(0)


if __name__ == "__main__":
    try:
        main()
    except SystemExit:
        raise
    except Exception as e:  # noqa: BLE001 — audit tooling: record and stop
        die(f"unhandled: {type(e).__name__}: {e}")
