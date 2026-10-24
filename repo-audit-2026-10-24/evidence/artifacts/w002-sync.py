#!/usr/bin/env python3
"""W-002 sync lifecycle, no-credentials leg (audit tooling, NOT app code).

Same proven real-input stack as pass-1/pass-2/W-005..W-010 (XTEST clicks on
Xvfb :99, ffmpeg x11grab frames, pill-verified sidebar navigation, 4-pass
OCR, click-until-verified retries recorded per attempt).

What it exercises (WORKFLOW-COVERAGE W-002; NEXT-ACTIONS #1):
  * Sync Health pre-state: engine NEW — 'Run initial sync' visible, header
    freshness chip 'never synced' (the never-synced precondition the audit
    preserved until now; W-002 is its sanctioned consumer)
  * 'Run initial sync' with NO Help Scout credentials → the DESIGNED
    refusal: guard_provider_connected (sync_engine.rs:1012-1022) errors
    before any run is created; the failure is toasted live (v1.4.0 R5 D4:
    notify_error → error toast, bottom-right stack) AND logged to the
    application_errors table (jobs::log_error, sync service)
  * Dashboard 'Sync now' (Incremental action) → same refusal + the small
    'syncing…' note under the button (dashboard.rs:551)
  * freshness chip remains 'never synced' after both attempts
  * Settings → Help Scout tab: the listener-health message (FIND-001
    sub-check: loopback.rs offers/limits OAuth by listener state)

EXPECTED STATE CHANGES (designed behavior, recorded — not defects):
  * application_errors gains one 'Initial sync failed: …' and one
    'Incremental sync failed: …' row (the Diagnostics log will show them;
    the clear-log button returns — pass 2 had emptied the log)
  * sync_runs / sync_cursors / sync_checkpoints stay 0 (the guard refuses
    before creating anything) — the never-synced DB state survives the
    attempts; only the ATTEMPT is consumed.

NOT EXECUTABLE (recorded):
  * the webhook-drain runtime check (FIND-001) needs a signed webhook event
    in the DB — the HMAC secret is app-internal, so no event can be forged
    from outside; drain_pending stays source-verified (workers.rs:219).
  * the happy-path sync needs live Help Scout credentials (external).

Run: bash start-xvfb.sh && python3 w002-sync.py
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
APP_LOG = "/tmp/w002-app.log"
LOG_DST = os.path.join(AUDIT, "evidence/logs/W002-app.log")
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
SYNC_HEALTH_IDX = PAGES.index("20-sync-health")
SETTINGS_IDX = PAGES.index("21-settings")
DASHBOARD_IDX = 0
TOAST_BOX = (880, 620, 1600, 1000)     # bottom-right toast stack
FULL_CONTENT = (230, 60, 1179, 760)

events = open(EVENTS, "a")


def ev(kind, **kw):
    rec = {"t": round(time.time(), 3), "kind": kind, **kw}
    events.write(json.dumps(rec) + "\n")
    events.flush()
    print(json.dumps(rec), flush=True)


def die(msg):
    ev("w002-fatal", error=msg)
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
    tmp = path or "/tmp/w002-shot.png"
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
        im.save("/tmp/w002-ocr.png")
        subprocess.run(
            ["tesseract", "/tmp/w002-ocr.png", "/tmp/w002-ocr-out",
             "--psm", str(psm), "tsv"],
            check=True, capture_output=True)
        return _tsv_to_words(crop, "/tmp/w002-ocr-out.tsv", scale=scale)

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
    logf.write(f"\n===== W-002 {tag} =====\n")
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
        tail = t.split(f"===== W-002 {tag} =====")[-1]
        if "cannot acquire the instance lock" in tail:
            lock_err = True
            break
        if "migrations" in tail or "loopback" in tail:
            ready = True
            break
    time.sleep(4 if ready else 8)
    ev("w002-app-started", tag=tag, pid=p.pid, ready=ready, lock_error=lock_err)
    if lock_err or not ready:
        die(f"app failed to start for {tag}")
    return p


def db_state():
    conn = sqlite3.connect(DB)
    try:
        out = {
            "sync_runs": conn.execute(
                "SELECT COUNT(*) FROM sync_runs").fetchone()[0],
            "sync_cursors": conn.execute(
                "SELECT COUNT(*) FROM sync_cursors").fetchone()[0],
            "sync_checkpoints": conn.execute(
                "SELECT COUNT(*) FROM sync_checkpoints").fetchone()[0],
            "application_errors": conn.execute(
                "SELECT COUNT(*) FROM application_errors").fetchone()[0],
            "app_errors_tail": conn.execute(
                "SELECT service || ': ' || message FROM application_errors "
                "ORDER BY timestamp DESC, rowid DESC LIMIT 3"
            ).fetchall(),
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
    while k_cur != target:
        if target < k_cur:
            # backward: scroll the sidebar fully up, then click the FIRST
            # visible row (= Dashboard after a full scroll-up) and verify
            # the pill actually moved there (run-2 fix: the scroll alone
            # leaves the OLD page selected — pill None gave it away).
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
    ev("w002-S001-pre", db=pre,
       expect="sync tables 0; application_errors=1 PRE-EXISTING "
              "(timestamp 21:31:15, attributed: the W-008 FIND-011 "
              "backdrop fall-through click at (1100,90) hit the Dashboard "
              "'Sync now' button — Incremental action refused by the same "
              "guard; see FIND-011)")

    # S002 — launch + nav to Sync Health.
    p = start_app("launch-1")
    time.sleep(2)
    img = parked_shot()
    if pill(img) is None:
        die("no pill after launch")
    img, k = nav_to(img, 0, SYNC_HEALTH_IDX)
    img.save(os.path.join(OUT, "W002-01-sync-health.png"))
    ev("w002-S002-nav", to=PAGES[SYNC_HEALTH_IDX], pill_y=pill(img),
       verdict="PASS")

    # S003 — pre-state on the page: 'Run initial sync' + 'never synced'.
    # run-1 OCR notes: the dark button's white text is only read by the
    # inverted pass and reads reliably when the crop is tight to the button
    # row; the header chip's 'synced' misreads as 'synce' at this size —
    # use the 'neversync' prefix; the chip lives in the HEADER (y≈26), not
    # in the page content box.
    has_btn = ocr_contains(img, "runinitial", (230, 260, 780, 320))
    has_chip = ocr_contains(img, "neversync", (860, 8, 1020, 48))
    img2 = parked_shot(os.path.join(OUT, "W002-02-pre-state.png"))
    ev("w002-S003-pre-state", run_initial_visible=has_btn,
       never_synced_chip=has_chip,
       verdict="PASS" if (has_btn and has_chip) else "FAIL",
       note="engine NEW; the never-synced precondition visible in the UI")
    if not (has_btn and has_chip):
        die("S003: pre-state markers not found")

    # S004 — 'Run initial sync' with no credentials → designed refusal.
    pos = find_label(img2, "runinitial", (230, 260, 780, 320))
    if not pos:
        die("'Run initial sync' button not found by OCR")
    click(*pos)
    time.sleep(0.4)
    toast = shot()      # immediate: the error toast lives ~10 s bottom-right
    time.sleep(0.8)
    toast2 = shot()     # second chance (toast render latency guard)
    toast_hit = (ocr_contains(toast, "notconnected", TOAST_BOX)
                 or ocr_contains(toast2, "notconnected", TOAST_BOX))
    time.sleep(1.5)
    img = parked_shot(os.path.join(OUT, "W002-03-after-initial-sync.png"))
    post = db_state()
    err_logged = post["application_errors"] > pre["application_errors"]
    no_runs = post["sync_runs"] == 0 and post["sync_cursors"] == 0 \
        and post["sync_checkpoints"] == 0
    ev("w002-S004-initial-refusal", clicked_at=list(pos),
       toast_not_connected=toast_hit, app_error_logged=err_logged,
       app_errors_tail=post["app_errors_tail"], sync_tables_stay_zero=no_runs,
       verdict="PASS" if (err_logged and no_runs) else "FAIL",
       note="designed refusal: guard_provider_connected errors before any "
            "run is created; failure toasted live + logged to "
            "application_errors (jobs::log_error)")
    if not (err_logged and no_runs):
        die("S004: initial-sync refusal not verified")

    # S005 — back to Dashboard (backward nav: scroll top + climb down).
    img, k = nav_to(img, SYNC_HEALTH_IDX, DASHBOARD_IDX)
    img.save(os.path.join(OUT, "W002-04-dashboard.png"))
    ev("w002-S005-nav-dashboard", to=PAGES[DASHBOARD_IDX], pill_y=pill(img),
       verdict="PASS")

    # S006 — Dashboard 'Sync now' (Incremental) → same designed refusal.
    # run-3 note: the button is Outline WITH a leading Refresh icon — the
    # icon merges into the first glyph and defeats OCR (same known
    # limitation as 'New Rule'/'create backup'). Its center (1100,90) is
    # INDEPENDENTLY evidence-backed: the W-008 FIND-011 fall-through click
    # at exactly (1100,90) pressed this button (see FIND-011). Use OCR if
    # it happens to read, else the evidence-backed position.
    pos = find_label(img, "syncnow", (950, 60, 1179, 130)) \
        or find_label(img, "ncnow", (950, 60, 1179, 130)) or (1100, 90)
    pre_err = db_state()["application_errors"]
    click(*pos)
    time.sleep(0.4)
    toast = shot()
    time.sleep(0.8)
    toast2 = shot()
    toast_hit = (ocr_contains(toast, "notconnected", TOAST_BOX)
                 or ocr_contains(toast2, "notconnected", TOAST_BOX))
    time.sleep(1.5)
    img = parked_shot(os.path.join(OUT, "W002-05-after-sync-now.png"))
    post = db_state()
    err_logged = post["application_errors"] > pre_err
    no_runs = post["sync_runs"] == 0 and post["sync_cursors"] == 0 \
        and post["sync_checkpoints"] == 0
    ev("w002-S006-sync-now-refusal", clicked_at=list(pos),
       toast_not_connected=toast_hit, app_error_logged=err_logged,
       app_errors_total=post["application_errors"],
       sync_tables_stay_zero=no_runs,
       verdict="PASS" if (err_logged and no_runs) else "FAIL",
       note="Incremental action refused by the same guard; dashboard shows "
            "the transient 'syncing…' note (dashboard.rs:551)")
    if not (err_logged and no_runs):
        die("S006: sync-now refusal not verified")

    # S007 — freshness chip still 'never synced'.
    has_chip = ocr_contains(img, "neversync", (860, 8, 1020, 48))
    ev("w002-S007-freshness-chip", never_synced_chip=has_chip,
       verdict="PASS" if has_chip else "FAIL")
    if not has_chip:
        die("S007: freshness chip is no longer 'never synced'")

    # S008 — Settings → Help Scout tab: listener-health message (FIND-001).
    img, k = nav_to(img, DASHBOARD_IDX, SETTINGS_IDX)
    pos = find_label(img, "helpscout", (230, 55, 1179, 125))
    if not pos:
        die("Help Scout settings tab not found by OCR")
    img, atts, ok = click_until(
        pos, lambda f: diff_mean(img, f) > 0.1, "helpscout-tab")
    img.save(os.path.join(OUT, "W002-06-helpscout-tab.png"))
    listener_msg = ocr_contains(img, "listener", FULL_CONTENT)
    oauth_offer = ocr_contains(img, "connect", FULL_CONTENT)
    ev("w002-S008-helpscout-listener", attempts=atts, tab_open=ok,
       listener_message_visible=listener_msg, connect_offered=oauth_offer,
       verdict="PASS" if ok else "FAIL",
       note="FIND-001 sub-check: the tab's listener-health messaging is "
            "captured; text recorded for the FIND-001 verification plan")
    if not ok:
        die("S008: Help Scout tab did not open")

    # S009 — end state.
    p.terminate()
    time.sleep(2)
    gone = wait_proc_gone()
    end_db = db_state()
    ok = (gone and end_db["sync_runs"] == 0 and end_db["sync_cursors"] == 0
          and end_db["sync_checkpoints"] == 0
          and end_db["application_errors"] == pre["application_errors"] + 2
          and end_db["integrity"] == "ok" and end_db["theme"] == "light")
    ev("w002-S009-end", proc_gone=gone, db=end_db,
       verdict="PASS" if ok else "FAIL",
       note="designed end state: application_errors grew by exactly 2 (this "
            "run's two refusals; the run-2 partial attempt also contributed "
            "one and is retained); sync tables still 0 — never-synced DB "
            "state survives; Diagnostics shows the errors and the clear-log "
            "button returns (pass 2 had emptied it)")
    if not ok:
        die("S009: end-state verification failed")

    ev("w002-summary", verdict="PASS", steps="S001..S009",
       executed="Run initial sync refusal + Sync now refusal (designed "
                "no-credentials guard, toast + application_errors), "
                "freshness chip unchanged, Help Scout tab listener-health "
                "message captured (FIND-001 sub-check)",
       blocked="webhook-drain runtime check needs a signed webhook event "
               "(cannot be forged outside the app) — source-verified only; "
               "happy-path sync needs live credentials (external)")
    finish(0)


if __name__ == "__main__":
    try:
        main()
    except SystemExit:
        raise
    except Exception as e:  # noqa: BLE001 — audit tooling: record and stop
        die(f"unhandled: {type(e).__name__}: {e}")
