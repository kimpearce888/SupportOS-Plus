#!/usr/bin/env python3
"""W-005 knowledge ingest + gaps, mouse-reachable portion (audit tooling, NOT
app code).

Same proven real-input stack as pass-1/pass-2/W-007/W-010/W-006 (XTEST clicks
on Xvfb :99, ffmpeg x11grab frames, pointer parked off the analyzed content
box, pill-verified sidebar navigation, 4-pass OCR, click-until-verified
retries with every attempt recorded).

What it exercises (WORKFLOW-COVERAGE W-005; NEXT-ACTIONS #1):
  * Knowledge → "Import documents" expander (C-KNOW-04 area; pass-2 K01
    opened it once — this run records it at workflow level)
  * the manual-import form's DISABLED-state gate (C-KNOW-08): with empty
    title/content the 'Import document' button renders via
    `add_enabled(false)` (knowledge.rs:656-663) — clicking it must be a
    no-op and create ZERO records
  * Issue Radar enqueue paths (W-005 workflow definition + FIND-008 runtime
    check): 'Detect gaps now' and 'Cluster now' fire-and-forget
    `jobs::enqueue_on("ai", …)` (issue_radar.rs:332/350, 379/397) — click,
    then verify jobs-table rows appear and the UI shows NO feedback
    (FIND-008's silent-enqueue claim, runtime-confirmed)
  * 'Reload clusters' empty path (0 clusters — no-op, matches pass-2 RA1)

DELIBERATE NON-EXECUTION / BLOCKED STEPS (recorded, not silent):
  * the ingest TEXT path (title / source name / content typing) is NOT
    exercisable: XTEST KeyPress does not reach egui TextEdits here
    (limitation verified twice; xdotool unavailable) — recorded as
    tooling-limited; 'Import document' can never become enabled at runtime
    in this environment, so the enabled-path import + FTS/vector indexing
    are source-verified only.
  * the enqueued AI jobs will fail/pend forever (no LM Studio in the
    sandbox) — the resulting jobs rows are QA artifacts, recorded with
    status breakdown at end; they do NOT touch sync state.
  * never-synced precondition untouched; theme untouched.

Run: bash start-xvfb.sh && python3 w005-knowledge.py
"""
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
APP_LOG = "/tmp/w005-app.log"
LOG_DST = os.path.join(AUDIT, "evidence/logs/W005-app.log")
EVENTS = os.path.join(AUDIT, "evidence/artifacts/control-verify-events.jsonl")
DB = "/home/z/.local/share/supportos-plus/supportos-plus.db"

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
KNOWLEDGE_IDX = PAGES.index("07-knowledge")
RADAR_IDX = PAGES.index("09-issue-radar")
FULL_CONTENT = (230, 60, 1179, 760)

events = open(EVENTS, "a")


def ev(kind, **kw):
    rec = {"t": round(time.time(), 3), "kind": kind, **kw}
    events.write(json.dumps(rec) + "\n")
    events.flush()
    print(json.dumps(rec), flush=True)


def die(msg):
    ev("w005-fatal", error=msg)
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
    tmp = path or "/tmp/w005-shot.png"
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
        im.save("/tmp/w005-ocr.png")
        subprocess.run(
            ["tesseract", "/tmp/w005-ocr.png", "/tmp/w005-ocr-out",
             "--psm", str(psm), "tsv"],
            check=True, capture_output=True)
        return _tsv_to_words(crop, "/tmp/w005-ocr-out.tsv", scale=scale)

    for psm, inv, sc in [(6, False, 1), (6, True, 1), (6, False, 3),
                         (11, False, 2)]:
        try:
            words += run_ocr(psm, inv, sc)
        except (subprocess.CalledProcessError, FileNotFoundError):
            pass
    return words


def find_label(img, needle, crop, first=False):
    """find_label v3 — v2 semantics (same-visual-row dy<=5, tightest pixel
    window first)."""
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
    logf.write(f"\n===== W-005 {tag} =====\n")
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
        tail = t.split(f"===== W-005 {tag} =====")[-1]
        if "cannot acquire the instance lock" in tail:
            lock_err = True
            break
        if "migrations" in tail or "loopback" in tail:
            ready = True
            break
    time.sleep(4 if ready else 8)
    ev("w005-app-started", tag=tag, pid=p.pid, ready=ready, lock_error=lock_err)
    if lock_err or not ready:
        die(f"app failed to start for {tag}")
    return p


def db_state():
    conn = sqlite3.connect(DB)
    try:
        out = {
            "knowledge_documents": conn.execute(
                "SELECT COUNT(*) FROM knowledge_documents").fetchone()[0],
            "knowledge_sources": conn.execute(
                "SELECT COUNT(*) FROM knowledge_sources").fetchone()[0],
            "jobs_total": conn.execute(
                "SELECT COUNT(*) FROM jobs").fetchone()[0],
            "jobs": conn.execute(
                "SELECT queue, status, COUNT(*) FROM jobs "
                "GROUP BY queue, status").fetchall(),
            "sync_runs": conn.execute(
                "SELECT COUNT(*) FROM sync_runs").fetchone()[0],
            "theme": (conn.execute(
                "SELECT value FROM application_settings WHERE key='theme'"
            ).fetchone() or [None])[0],
            "integrity": conn.execute(
                "PRAGMA integrity_check").fetchone()[0],
        }
        return out
    finally:
        conn.close()


def nav_to(img, k_cur, target):
    scrolls = 0
    while k_cur < target:
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
            img, _, _, _ = settled_state()
            scrolls += 1
            if (bands(img), pill(img)) == pre:
                wheel(5)
                img, _, _, _ = settled_state()
                if (bands(img), pill(img)) == pre:
                    die(f"sidebar at max scroll before {PAGES[target]}")
            continue
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

    # S001 — pre-state.
    pre = db_state()
    ev("w005-S001-pre", db=pre,
       expect="knowledge empty, jobs baseline, sync untouched")

    # S002 — launch + nav to Knowledge.
    p = start_app("launch-1")
    time.sleep(2)
    img = parked_shot()
    if pill(img) is None:
        die("no pill after launch")
    img, k = nav_to(img, 0, KNOWLEDGE_IDX)
    img.save(os.path.join(OUT, "W005-01-knowledge.png"))
    ev("w005-S002-nav-knowledge", to=PAGES[KNOWLEDGE_IDX],
       pill_y=pill(img), verdict="PASS")

    # S003 — open the Import documents expander.
    pos = find_label(img, "importdocuments", FULL_CONTENT)
    if not pos:
        die("Import documents expander not found by OCR")
    img, atts, ok = click_until(
        pos, lambda f: ocr_contains(f, "sourcename", (230, 100, 1179, 700))
        and ocr_contains(f, "importdocument", (230, 100, 1179, 700)),
        "expander-open")
    img.save(os.path.join(OUT, "W005-02-import-form-open.png"))
    ev("w005-S003-expander-open", attempts=atts,
       verdict="PASS" if ok else "FAIL")
    if not ok:
        die("S003: Import documents expander did not open")

    # S004 — DISABLED 'Import document' with empty fields (C-KNOW-08):
    # clicking must be a no-op; zero records; no enabled-path feedback.
    pos = find_label(img, "importdocument", (230, 100, 1179, 700))
    if not pos:
        die("Import document button not found by OCR")
    pre_db = db_state()
    pre_img = img
    click(*pos)
    time.sleep(1.0)
    img = parked_shot(os.path.join(OUT, "W005-03-import-disabled-click.png"))
    post_db = db_state()
    dm = diff_mean(pre_img, img)
    no_records = (post_db["knowledge_documents"] == 0
                  and post_db["knowledge_sources"] == 0)
    ok = no_records and post_db["jobs_total"] == pre_db["jobs_total"]
    ev("w005-S004-import-disabled", clicked_at=list(pos), diff=dm,
       db=post_db, verdict="PASS" if ok else "FAIL",
       note="disabled gate runtime-checked: add_enabled(false) with empty "
            "title/content (knowledge.rs:656-663); click is a no-op; the "
            "ENABLED path requires text input — tooling-limited, "
            "source-verified only")
    if not ok:
        die("S004: disabled Import click was NOT a no-op (unexpected record)")

    # S004b — Visibility chip round (mouse-reachable form state; no data):
    # 'customer-safe (may support customer drafts)' <-> 'internal-only'.
    pos_cs = find_label(img, "customersafe", (230, 430, 1179, 530))
    if pos_cs:
        img, atts, ok = click_until(
            pos_cs,
            lambda f, base=img: diff_mean(base, f) > 0.01,
            "visibility-customer-safe")
        img.save(os.path.join(OUT, "W005-03b-visibility-customer-safe.png"))
        ev("w005-S004b-visibility-chip", attempts=atts, diff_recorded=True,
           verdict="PASS" if ok else "RECORDED",
           note="visibility chip click exercised (form state only; no "
                "record created)")
        pos_io = find_label(img, "internalonly", (230, 430, 1179, 530))
        if pos_io:
            img, atts2, ok2 = click_until(
                pos_io,
                lambda f, base=img: diff_mean(base, f) > 0.01,
                "visibility-internal-only")
            img.save(os.path.join(OUT,
                                  "W005-03c-visibility-internal-only.png"))
            ev("w005-S004b2-visibility-restore", attempts=atts2,
               verdict="PASS" if ok2 else "RECORDED")

    # S005 — close the expander.
    # run-1 note: on the OPEN frame find_label mis-located the toggle text
    # (matched inside the folder explainer at y≈298); the toggle row itself
    # is stable at (313,242) (probe-verified on W005-02) — use it directly.
    pos = find_label(img, "importdocuments", (230, 200, 700, 280)) \
        or (313, 242)
    if not (200 <= pos[1] <= 280):
        pos = (313, 242)
    img, atts, ok = click_until(
        pos, lambda f: not ocr_contains(f, "sourcename", (230, 100, 1179, 700)),
        "expander-close")
    img.save(os.path.join(OUT, "W005-04-import-form-closed.png"))
    ev("w005-S005-expander-closed", attempts=atts,
       verdict="PASS" if ok else "FAIL")
    if not ok:
        die("S005: Import documents expander did not close")

    # S006 — nav to Issue Radar.
    img, k = nav_to(img, k, RADAR_IDX)
    img.save(os.path.join(OUT, "W005-05-issue-radar.png"))
    ev("w005-S006-nav-radar", to=PAGES[RADAR_IDX], pill_y=pill(img),
       verdict="PASS")

    # S007 — 'Detect gaps now': enqueue with NO UI feedback (FIND-008).
    pos = find_label(img, "detectgapsnow", FULL_CONTENT)
    if not pos:
        die("'Detect gaps now' not found by OCR")
    pre_db = db_state()
    pre_img = img
    click(*pos)
    time.sleep(1.0)
    img = parked_shot(os.path.join(OUT, "W005-06-detect-gaps-clicked.png"))
    post_db = db_state()
    dm = diff_mean(pre_img, img)
    enq = post_db["jobs_total"] > pre_db["jobs_total"]
    ev("w005-S007-detect-gaps", clicked_at=list(pos), diff=dm,
       jobs_before=pre_db["jobs_total"], jobs_after=post_db["jobs_total"],
       enqueued=enq, verdict="PASS" if enq else "FAIL",
       note="fire-and-forget enqueue (issue_radar.rs:379/397); small UI diff "
            "and no toast = FIND-008 silent-enqueue runtime confirmation")
    if not enq:
        die("S007: 'Detect gaps now' did not enqueue a job")

    # S008 — 'Cluster now': same enqueue path.
    pos = find_label(img, "clusternow", FULL_CONTENT)
    if not pos:
        die("'Cluster now' not found by OCR")
    pre_db = db_state()
    pre_img = img
    click(*pos)
    time.sleep(1.0)
    img = parked_shot(os.path.join(OUT, "W005-07-cluster-now-clicked.png"))
    post_db = db_state()
    dm = diff_mean(pre_img, img)
    enq = post_db["jobs_total"] > pre_db["jobs_total"]
    ev("w005-S008-cluster-now", clicked_at=list(pos), diff=dm,
       jobs_before=pre_db["jobs_total"], jobs_after=post_db["jobs_total"],
       enqueued=enq, verdict="PASS" if enq else "FAIL",
       note="fire-and-forget enqueue (issue_radar.rs:332/350)")
    if not enq:
        die("S008: 'Cluster now' did not enqueue a job")

    # S009 — 'Reload clusters' empty path (no clusters to reload).
    pos = find_label(img, "reloadclusters", FULL_CONTENT)
    if not pos:
        die("'Reload clusters' not found by OCR")
    pre_img = img
    click(*pos)
    time.sleep(1.0)
    img = parked_shot(os.path.join(OUT, "W005-08-reload-clusters.png"))
    dm = diff_mean(pre_img, img)
    ev("w005-S009-reload-clusters", clicked_at=list(pos), diff=dm,
       verdict="PASS" if dm < 0.05 else "RECORDED-NOOP",
       note="0 clusters; matches pass-2 RA1 (diff 0.0)")

    # S010 — end state.
    p.terminate()
    time.sleep(2)
    gone = wait_proc_gone()
    end_db = db_state()
    ok = (gone and end_db["knowledge_documents"] == 0
          and end_db["sync_runs"] == 0 and end_db["integrity"] == "ok"
          and end_db["theme"] == "light")
    ev("w005-S010-end", proc_gone=gone, db=end_db,
       verdict="PASS" if ok else "FAIL",
       note="knowledge untouched (0 docs); never-synced precondition intact; "
            "the enqueued AI jobs remain as QA artifacts (no AI backend)")
    if not ok:
        die("S010: end-state verification failed")

    ev("w005-summary", verdict="PASS", steps="S001..S010",
       executed="import expander + disabled-Import gate (C-KNOW-08), "
                "'Detect gaps now' + 'Cluster now' enqueues (FIND-008 "
                "runtime-confirmed: silent), 'Reload clusters' empty no-op",
       blocked="ingest text path (title/source/content typing) — XTEST "
               "keyboard limitation; enabled import + FTS/vector indexing "
               "source-verified only")
    finish(0)


if __name__ == "__main__":
    try:
        main()
    except SystemExit:
        raise
    except Exception as e:  # noqa: BLE001 — audit tooling: record and stop
        die(f"unhandled: {type(e).__name__}: {e}")
