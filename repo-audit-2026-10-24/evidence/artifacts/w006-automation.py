#!/usr/bin/env python3
"""W-006 automation-rule runtime verification (audit tooling, NOT app code).

Same proven real-input stack as pass-1/pass-2/W-007/W-010 (XTEST clicks on
Xvfb :99, ffmpeg x11grab frames, pointer parked off the analyzed content box,
pill-verified sidebar navigation, 4-pass OCR + dark-rect detectors,
click-until-verified retries with every attempt recorded).

What it exercises (WORKFLOW-COVERAGE W-006; NEXT-ACTIONS #1):
  * "New Rule" toggle (C-AUTO-01) — form open/close
  * trigger chip group (C-AUTO-03): Status changed / Tag added / SLA at risk —
    including the CONDITIONAL FIELD SWAP (from→to fields vs tag field vs none)
  * action chip group (C-AUTO-06): Assign / Add tag / Send note /
    Change status / Set priority — conditional field swap per kind
  * "Create Rule" on an EMPTY form (C-AUTO-12 empty path): validation error
    "error: rule name required" (create_rule automation.rs:286-289) and ZERO
    records created (DB automation_rules=0) — same pattern as the pass-2
    Organizations OR1 exercise
  * rules/approvals empty states visible ("0 rule(s)", "No approvals waiting")

DELIBERATE NON-EXECUTION / BLOCKED STEPS (recorded, not silent):
  * RULE CREATION is NOT possible at runtime in this environment: the form
    requires text input (name; plus one required text field per action kind)
    and XTEST KeyPress does not reach egui TextEdits here (limitation
    verified twice in earlier passes; xdotool unavailable). Typing-dependent
    steps — creating a rule, the enabled/disabled row toggle, the delete
    confirm dialog, and approve/reject in the approvals queue — are therefore
    recorded as TOOLING-LIMITED (not app defects, not E1-E6 exceptions), and
    the C-SET-66 "really restore" deferral extends (W-006 cannot produce the
    first QA data rows as planned; only a maintainer-side text-capable run
    or an explicit QA-fixture decision can).
  * never-synced precondition untouched; theme untouched.

Run: bash start-xvfb.sh && python3 w006-automation.py
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
APP_LOG = "/tmp/w006-app.log"
LOG_DST = os.path.join(AUDIT, "evidence/logs/W006-app.log")
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
AUTO_IDX = PAGES.index("12-automation")

# Measured form geometry (probe 2026-10-24, /tmp probe + OCR dump)
NEW_RULE_BTN = (1117, 91)                 # dark Primary + Plus icon (OCR-blind)
TRIGGER_STATUS = (392, 281)
TRIGGER_TAG = (495, 280)
TRIGGER_SLA = (590, 280)
ACTION_ASSIGN = (357, 368)
ACTION_ADDTAG = (443, 368)
ACTION_SENDNOTE = (515, 368)
ACTION_CHSTATUS = (622, 368)
ACTION_SETPRIO = (733, 368)
CREATE_RULE_REGION = (240, 420, 600, 480)
TRIGGER_BAND = (250, 300, 640, 360)       # conditional trigger fields row
ACTION_BAND = (250, 385, 760, 445)        # conditional action fields row
ERROR_BAND = (230, 100, 1179, 200)        # error_text renders under header

events = open(EVENTS, "a")


def ev(kind, **kw):
    rec = {"t": round(time.time(), 3), "kind": kind, **kw}
    events.write(json.dumps(rec) + "\n")
    events.flush()
    print(json.dumps(rec), flush=True)


def die(msg):
    ev("w006-fatal", error=msg)
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
    tmp = path or "/tmp/w006-shot.png"
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
        im.save("/tmp/w006-ocr.png")
        subprocess.run(
            ["tesseract", "/tmp/w006-ocr.png", "/tmp/w006-ocr-out",
             "--psm", str(psm), "tsv"],
            check=True, capture_output=True)
        return _tsv_to_words(crop, "/tmp/w006-ocr-out.tsv", scale=scale)

    for psm, inv, sc in [(6, False, 1), (6, True, 1), (6, False, 3),
                         (11, False, 2)]:
        try:
            words += run_ocr(psm, inv, sc)
        except (subprocess.CalledProcessError, FileNotFoundError):
            pass
    return words


def ocr_contains(img, needle, crop):
    needle = norm(needle)
    words = ocr_words(img, crop)
    joined = "".join(w[0] for w in words)
    return needle in joined


def dark_rect(img, crop, min_row_dark=30, min_h=20, min_w=40):
    """Bounding-rect detector for solid dark buttons (OCR-blind
    Primary buttons with a leading icon)."""
    g = img.crop(crop).convert("L")
    px = g.load()
    w, h = g.size
    rows = [y for y in range(h)
            if sum(1 for x in range(w) if px[x, y] < 140) >= min_row_dark]
    if not rows:
        return None
    y0, y1 = rows[0], rows[-1]
    if y1 - y0 < min_h:
        return None
    cols = [x for x in range(w)
            if sum(1 for y in range(y0, y1 + 1) if px[x, y] < 140) >= min_w // 2]
    if not cols or cols[-1] - cols[0] < min_w:
        return None
    return (crop[0] + (cols[0] + cols[-1]) // 2,
            crop[1] + (y0 + y1) // 2)


def click_until(pos, predicate, tag, tries=3, settle=0.9):
    """Click, verify predicate, retry with slight position variation on a
    no-op (transient XTEST event-loss guard; attempts recorded)."""
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
    logf.write(f"\n===== W-006 {tag} =====\n")
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
        tail = t.split(f"===== W-006 {tag} =====")[-1]
        if "cannot acquire the instance lock" in tail:
            lock_err = True
            break
        if "migrations" in tail or "loopback" in tail:
            ready = True
            break
    time.sleep(4 if ready else 8)
    ev("w006-app-started", tag=tag, pid=p.pid, ready=ready, lock_error=lock_err)
    if lock_err or not ready:
        die(f"app failed to start for {tag}")
    return p


def db_counts():
    conn = sqlite3.connect(DB)
    try:
        out = {
            "automation_rules": conn.execute(
                "SELECT COUNT(*) FROM automation_rules").fetchone()[0],
            "automation_approvals": conn.execute(
                "SELECT COUNT(*) FROM automation_approvals").fetchone()[0],
            "automation_runs": conn.execute(
                "SELECT COUNT(*) FROM automation_runs").fetchone()[0],
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
    pre = db_counts()
    ev("w006-S001-pre", db=pre,
       expect="automation_rules=0, approvals=0 (empty; never-synced intact)")

    # S002 — launch + nav to Automation.
    p = start_app("launch-1")
    time.sleep(2)
    img = parked_shot()
    if pill(img) is None:
        die("no pill after launch")
    img, k = nav_to(img, 0, AUTO_IDX)
    img.save(os.path.join(OUT, "W006-01-automation.png"))
    ev("w006-S002-nav", to=PAGES[AUTO_IDX], pill_y=pill(img), verdict="PASS")

    # S003 — open the New Rule form.
    pos = dark_rect(img, (1020, 60, 1180, 120)) or NEW_RULE_BTN
    img, atts, ok = click_until(
        pos, lambda f: ocr_contains(f, "rulename", (230, 130, 1179, 300)),
        "open-form")
    img.save(os.path.join(OUT, "W006-02-form-open.png"))
    ev("w006-S003-form-open", attempts=atts, verdict="PASS" if ok else "FAIL")
    if not ok:
        die("S003: New Rule form did not open")

    # S004 — trigger chips: conditional field swap.
    # Band geometry: with a conditional trigger row present, the action row
    # sits at y≈368 (outside the trigger band); with NO conditional row
    # (SLA), the action row pulls UP into y≈323. The action row's "Add tag"
    # chip contains the word 'tag', so 'tag'-presence checks must be
    # qualified by the action-row position (recorded after run-1 S004b).
    # 4a: Tag added -> tag field replaces from/to; action row stays down.
    def tag_added(f):
        return (ocr_contains(f, "tag", (250, 300, 400, 355))
                and not ocr_contains(f, "fromstatus", (250, 300, 640, 360))
                and not ocr_contains(f, "action", (250, 300, 340, 360)))
    img, atts, ok = click_until(TRIGGER_TAG, tag_added, "trigger-tag-added")
    img.save(os.path.join(OUT, "W006-03-trigger-tag-added.png"))
    ev("w006-S004a-trigger-tag", attempts=atts,
       expect="trigger tag field replaces from→to; action row unchanged",
       verdict="PASS" if ok else "FAIL")
    if not ok:
        die("S004a: trigger 'Tag added' conditional swap not verified")
    # 4b: SLA at risk -> NO conditional trigger row: the action row pulls up
    # into the former conditional slot (structural evidence).
    def sla(f):
        return (ocr_contains(f, "action", (250, 300, 340, 360))
                and not ocr_contains(f, "fromstatus", (250, 300, 640, 360)))
    img, atts, ok = click_until(TRIGGER_SLA, sla, "trigger-sla")
    img.save(os.path.join(OUT, "W006-04-trigger-sla.png"))
    ev("w006-S004b-trigger-sla", attempts=atts,
       expect="no conditional trigger fields (action row pulled up)",
       verdict="PASS" if ok else "FAIL")
    if not ok:
        die("S004b: trigger 'SLA at risk' conditional swap not verified")
    # 4c: back to Status changed -> from/to fields return.
    img, atts, ok = click_until(
        TRIGGER_STATUS,
        lambda f: ocr_contains(f, "fromstatus", TRIGGER_BAND),
        "trigger-status")
    img.save(os.path.join(OUT, "W006-05-trigger-status.png"))
    ev("w006-S004c-trigger-status", attempts=atts,
       expect="from→to fields restored", verdict="PASS" if ok else "FAIL")
    if not ok:
        die("S004c: trigger 'Status changed' restore not verified")

    # S005 — action chips: conditional field swap per kind.
    seq = [("assign", ACTION_ASSIGN, "assignee", "assignee id field"),
           ("sendnote", ACTION_SENDNOTE, "notebody", "note body field"),
           ("setpriority", ACTION_SETPRIO, "priority", "priority field"),
           # run-2 note: the 'new status' hint OCRs as 'new statu' (trailing
           # s truncated by tesseract at this size); needle matches that.
           ("changestatus", ACTION_CHSTATUS, "newstatu", "new status field"),
           ("addtag", ACTION_ADDTAG, "tag", "tag field")]
    for i, (tag_id, pos, needle, label) in enumerate(seq):
        img, atts, ok = click_until(
            pos, lambda f, n=needle: ocr_contains(f, n, ACTION_BAND),
            f"action-{tag_id}")
        img.save(os.path.join(OUT, f"W006-06-{i}-action-{tag_id}.png"))
        ev(f"w006-S005-action-{tag_id}", attempts=atts, expect=label,
           verdict="PASS" if ok else "FAIL")
        if not ok:
            die(f"S005: action '{tag_id}' conditional field not verified")

    # S006 — Create Rule on an EMPTY form: validation error, zero records.
    pre_db = db_counts()
    pos = dark_rect(img, CREATE_RULE_REGION)
    if not pos:
        die("Create Rule button not found (dark-rect detector)")
    img, atts, ok = click_until(
        pos, lambda f: ocr_contains(f, "rulenamerequired", ERROR_BAND),
        "create-empty")
    img.save(os.path.join(OUT, "W006-07-create-empty-error.png"))
    post_db = db_counts()
    no_records = post_db["automation_rules"] == 0
    ev("w006-S006-create-empty", attempts=atts, error_shown=ok,
       db_automation_rules=post_db["automation_rules"],
       verdict="PASS" if (ok and no_records) else "FAIL",
       note="validation order: name first (automation.rs:286-289)")
    if not (ok and no_records):
        die("S006: empty-create validation path not verified")

    # S007 — close the form (toggle).
    # run-3 note: the S006 validation error ('error: rule name required')
    # PERSISTS after the toggle closes the form (the toggle does not clear
    # self.error — recorded as a runtime observation), so the closed-check
    # uses the form-specific 'trigger:' label instead of 'rule name'.
    def form_closed(f):
        return not ocr_contains(f, "trigger", (230, 240, 1179, 470))
    img, atts, ok = click_until(NEW_RULE_BTN, form_closed, "close-form")
    img.save(os.path.join(OUT, "W006-08-form-closed.png"))
    err_persists = ocr_contains(img, "rulenamerequired", (230, 100, 1179, 220))
    ev("w006-S007-form-closed", attempts=atts, verdict="PASS" if ok else "FAIL",
       stale_error_after_close=err_persists,
       note="observation: validation error text persists after the form is "
            "closed (toggle does not clear self.error) — recorded, minor")
    if not ok:
        die("S007: New Rule toggle did not close the form")

    # S008 — end state.
    p.terminate()
    time.sleep(2)
    gone = wait_proc_gone()
    end_db = db_counts()
    ok = gone and end_db["automation_rules"] == 0 \
        and end_db["automation_approvals"] == 0 \
        and end_db["integrity"] == "ok" and end_db["theme"] == "light"
    ev("w006-S008-end", proc_gone=gone, db=end_db,
       verdict="PASS" if ok else "FAIL",
       note="zero records; never-synced precondition intact; theme unchanged")
    if not ok:
        die("S008: end-state verification failed")

    ev("w006-summary", verdict="PASS", steps="S001..S008",
       executed="C-AUTO-01 toggle (open+close), C-AUTO-03 trigger chips "
                "(3/3 + conditional swaps), C-AUTO-06 action chips (5/5 + "
                "conditional swaps), C-AUTO-12 empty-create validation "
                "('rule name required'), zero records",
       blocked="rule creation + enabled/disabled toggle + delete dialog + "
               "approve/reject: require TEXT INPUT (XTEST keyboard limitation, "
               "verified twice) or existing data — recorded as "
               "tooling-limited, NOT exceptions; C-SET-66 deferral extends")
    finish(0)


if __name__ == "__main__":
    try:
        main()
    except SystemExit:
        raise
    except Exception as e:  # noqa: BLE001 — audit tooling: record and stop
        die(f"unhandled: {type(e).__name__}: {e}")
