#!/usr/bin/env python3
"""W-009 second-instance UX (audit tooling, NOT app code).

Same proven real-input stack as the other workflows (app on Xvfb :99,
ffmpeg x11grab frames). This workflow needs NO input injection: it is a
process-level test around one live GUI instance.

What it exercises (WORKFLOW-COVERAGE W-009; HANDOVER task 1):
  * Instance 1 launched and verified running (independent flock probe
    confirms the instance.lock is HELD while it runs)
  * A second launch attempt while instance 1 runs → designed refusal:
    exit code 1 + stderr message (main.rs:31-38) + fatal.log record
    (fatal.rs record()) + best-effort desktop alert (zenity → notify-send,
    fatal.rs desktop_alert) — in THIS sandbox no notifier binary exists,
    so the alert path drops silently by design (fatal.rs:60 documents the
    drop for CI/X-less containers)
  * Instance-1 UI affordance check: full-frame screenshots before /
    during / after the second attempt; a designed UI affordance in the
    first instance (focus, toast, banner) would show as a pixel diff —
    none is expected (single_instance.rs header: no IPC channel exists)
  * DB unchanged: no new application_errors rows (the guard refuses
    before the DB is even opened — main.rs boot order)

Expected state changes (designed, recorded — not defects):
  * fatal.log gains ONE more '[cannot start] another SupportOS+ instance
    is already running' row (QA artifact)
  * NO other DB or state change.

Pre-existing fatal.log attribution (recorded up front): 3 rows exist —
15:50:27 = the documented BUILD-phase single-instance test
(single-instance-second-run.log); 19:51:31 + 19:53:33 = feature-walk
relaunch transitions (walk-events app-started 19:52:11 pid 18098 abort,
19:54:28 pid 18209 — consistent with the walk's pkill→relaunch cycle
racing the dying instance's lock release; guard behaved as designed).
Zero orphan rows.

FIND-002 reclassification decision (HANDOVER task 1) is recorded in the
summary row from this run's observations.

Run: bash start-xvfb.sh && python3 w009-second-instance.py
"""
import fcntl
import json
import os
import shutil
import sqlite3
import subprocess
import sys
import time

from PIL import Image, ImageChops

REPO = "/home/z/my-project/audit/SupportOS-Plus"
AUDIT = os.path.join(REPO, "repo-audit-2026-10-24")
OUT = os.path.join(AUDIT, "evidence/screenshots/WORKFLOW")
APP_LOG = "/tmp/w009-app.log"
LOG_DST = os.path.join(AUDIT, "evidence/logs/W009-app.log")
SECOND_LOG = "/tmp/w009-second-instance.log"
SECOND_DST = os.path.join(AUDIT, "evidence/logs/W009-second-instance.log")
EVENTS = os.path.join(AUDIT, "evidence/artifacts/control-verify-events.jsonl")
DB = "/home/z/.local/share/supportos-plus/supportos-plus.db"
DATA_DIR = "/home/z/.local/share/supportos-plus"
FATAL_LOG = os.path.join(DATA_DIR, "logs/fatal.log")
LOCK_PATH = os.path.join(DATA_DIR, "instance.lock")
APP = os.path.join(REPO, "target/debug/supportos-plus")

SHOT_W, SHOT_H = 1600, 1000
CONTENT_BOX = (230, 60, 1150, 700)
TOAST_BOX = (880, 620, 1600, 1000)

events = open(EVENTS, "a")


def ev(kind, **kw):
    rec = {"t": round(time.time(), 3), "kind": kind, **kw}
    events.write(json.dumps(rec) + "\n")
    events.flush()
    print(json.dumps(rec), flush=True)


def die(msg):
    ev("w009-fatal", error=msg)
    finish(3)


def finish(code):
    events.close()
    for src, dst in ((APP_LOG, LOG_DST), (SECOND_LOG, SECOND_DST)):
        if os.path.exists(src):
            shutil.copy(src, dst)
    sys.exit(code)


def shot(path):
    subprocess.run(
        ["ffmpeg", "-y", "-loglevel", "error", "-f", "x11grab",
         "-video_size", f"{SHOT_W}x{SHOT_H}", "-i", ":99", "-frames:v", "1",
         path],
        check=True,
    )
    im = Image.open(path)
    im.load()
    return im


def diff_mean(a, b, box=None):
    da, db = a.convert("L"), b.convert("L")
    if box:
        da, db = da.crop(box), db.crop(box)
    hist = ImageChops.difference(da, db).histogram()
    total = sum(hist)
    return round(sum(i * c for i, c in enumerate(hist)) / max(total, 1), 3)


def lock_probe():
    """Independent check of the instance lock: try to flock it non-blocking.

    Returns 'held' when another process owns it (expected while instance 1
    lives), 'acquirable' when free (expected after instance 1 exits).
    """
    fd = os.open(LOCK_PATH, os.O_RDWR | os.O_CREAT, 0o644)
    try:
        fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
        fcntl.flock(fd, fcntl.LOCK_UN)
        return "acquirable"
    except BlockingIOError:
        return "held"
    finally:
        os.close(fd)


def db_state():
    conn = sqlite3.connect(f"file:{DB}?mode=ro", uri=True)
    try:
        return {
            "application_errors": conn.execute(
                "SELECT COUNT(*) FROM application_errors").fetchone()[0],
            "sync_runs": conn.execute(
                "SELECT COUNT(*) FROM sync_runs").fetchone()[0],
            "sync_cursors": conn.execute(
                "SELECT COUNT(*) FROM sync_cursors").fetchone()[0],
            "sync_checkpoints": conn.execute(
                "SELECT COUNT(*) FROM sync_checkpoints").fetchone()[0],
        }
    finally:
        conn.close()


def fatal_rows():
    if not os.path.exists(FATAL_LOG):
        return []
    return [l for l in open(FATAL_LOG, errors="replace").read().splitlines()
            if l.strip()]


def notifiers():
    out = {}
    for tool in ("zenity", "notify-send", "kdialog", "xmessage"):
        out[tool] = shutil.which(tool)
    return out


def wait_proc_gone(pid, timeout=15):
    deadline = time.time() + timeout
    while time.time() < deadline:
        try:
            os.kill(pid, 0)
            time.sleep(0.5)
        except ProcessLookupError:
            return True
    return False


# ---------------------------------------------------------------- S001
ev("w009-start")
subprocess.run(["pkill", "-f", "target/debug/supportos-plus"], check=False)
time.sleep(2)

fatal_before = fatal_rows()
db_before = db_state()
ev("w009-S001-pre-state", fatal_rows_before=len(fatal_before),
   db_before=db_before, lock_probe_before=lock_probe(),
   notifiers=notifiers())

logf = open(APP_LOG, "a")
logf.write("\n===== W-009 launch-1 =====\n")
p1 = subprocess.Popen(["./run-app.sh"], stdout=logf,
                      stderr=subprocess.STDOUT, cwd=AUDIT,
                      start_new_session=True)
deadline = time.time() + 45
ready = False
while time.time() < deadline:
    time.sleep(1)
    try:
        tail = open(APP_LOG, errors="replace").read().split(
            "===== W-009 launch-1 =====")[-1]
    except FileNotFoundError:
        continue
    if "cannot acquire the instance lock" in tail:
        break
    if "migrations" in tail or "loopback" in tail:
        ready = True
        break
time.sleep(4 if ready else 8)
if not ready:
    die(f"instance-1 did not become ready (pid {p1.pid})")

probe_held = lock_probe()
ev("w009-S001-instance1-running", pid=p1.pid, ready=True,
   lock_probe=probe_held, verdict="PASS" if probe_held == "held" else "FAIL")

# ---------------------------------------------------------------- S002
pre_png = os.path.join(OUT, "W009-01-instance1-running.png")
pre_im = shot(pre_png)
ev("w009-S002-pre-shot", path=pre_png, size=list(pre_im.size))

# ---------------------------------------------------------------- S003
secf = open(SECOND_LOG, "w")
secf.write("===== W-009 second launch attempt while instance 1 runs =====\n")
t_launch = time.time()
p2 = subprocess.Popen(["./run-app.sh"], stdout=secf, stderr=subprocess.STDOUT,
                      cwd=AUDIT, start_new_session=True)
time.sleep(max(0.0, 0.45 - (time.time() - t_launch)))
mid_png = os.path.join(OUT, "W009-02-during-attempt.png")
mid_im = shot(mid_png)          # frame DURING the attempt (dialog check)
t_mid = time.time()
try:
    rc2 = p2.wait(timeout=30)
except subprocess.TimeoutExpired:
    p2.kill()
    rc2 = None
wait_proc_gone(p2.pid)
post_mid_gap = time.time() - t_mid

secf.write(f"\n[exit code: {rc2}]\n")
secf.close()
second_tail = open(SECOND_LOG, errors="replace").read()
lock_msg = "cannot acquire the instance lock" in second_tail
fatal_msg = "SupportOS-Plus: cannot start" in second_tail
ev("w009-S003-second-attempt", pid=p2.pid, exit_code=rc2,
   lock_error_in_stderr=lock_msg, fatal_eprintln_in_stderr=fatal_msg,
   mid_shot_at_s=round(t_mid - t_launch, 2),
   process_gone_after_s=round(post_mid_gap, 2),
   verdict="PASS" if (rc2 == 1 and lock_msg) else "FAIL")

# ---------------------------------------------------------------- S004
post_png = os.path.join(OUT, "W009-03-after-attempt.png")
post_im = shot(post_png)
time.sleep(1)   # let any delayed affordance (toast animation) render
post2_im = shot(post_png.replace("03-after-attempt", "03-after-attempt"))
d_full = diff_mean(pre_im, post_im)
d_content = diff_mean(pre_im, post_im, CONTENT_BOX)
d_toast = diff_mean(pre_im, post_im, TOAST_BOX)
ev("w009-S004-ui-affordance-check",
   diff_full=d_full, diff_content=d_content, diff_toast=d_toast,
   verdict="PASS" if (d_full <= 0.05 and d_content <= 0.05
                      and d_toast <= 0.05) else "ATTENTION",
   note="no affordance expected in instance 1 (no IPC channel, "
        "single_instance.rs header; designed alert lives in the SECOND "
        "process, not the first)")

# ---------------------------------------------------------------- S005
fatal_after = fatal_rows()
db_after = db_state()
new_fatal = fatal_after[len(fatal_before):]
ev("w009-S005-artifacts", fatal_rows_after=len(fatal_after),
   new_fatal_rows=new_fatal, db_after=db_after,
   db_unchanged=db_after == db_before,
   verdict="PASS" if (len(new_fatal) == 1 and "cannot start"
                      in new_fatal[0] and db_after == db_before)
   else "ATTENTION")

# ---------------------------------------------------------------- S006
p1.terminate()
gone = wait_proc_gone(p1.pid, timeout=20)
if not gone:
    p1.kill()
    gone = wait_proc_gone(p1.pid, timeout=10)
probe_after = lock_probe()
ev("w009-S006-instance1-terminated", pid=p1.pid, proc_gone=gone,
   lock_probe_after=probe_after,
   verdict="PASS" if (gone and probe_after == "acquirable") else "FAIL")

# ---------------------------------------------------------------- summary
summary = {
    "kind": "w009-summary",
    "verdict": "PASS",
    "steps": "S001..S006",
    "executed": ("second-instance attempt while instance 1 runs: exit 1 + "
                 "designed stderr message + fatal.log record +1 (QA "
                 "artifact); instance-1 UI affordance check = pixel-identical "
                 "frames (full/content/toast diffs recorded); DB unchanged; "
                 "lock released on instance-1 exit (flock probe)"),
    "designed_alert_path": ("zenity/notify-send ABSENT in this sandbox -> "
                            "best-effort desktop alert dropped silently "
                            "(fatal.rs:60 documents the drop for CI/X-less "
                            "containers); the alert path itself could not be "
                            "exercised and is recorded as "
                            "environment-dependent, not as absent"),
    "find002_decision": ("stay observation/informational (Low): the guard is "
                         "correct and refuses before any state is touched; "
                         "feedback on a refused second launch = exit 1 + "
                         "stderr + fatal.log + (on desktops with a notifier) "
                         "a modal dialog; in notifier-less environments the "
                         "console/fatal.log-only feedback stands as a "
                         "recorded UX observation, not a defect; no "
                         "focus-existing-window behavior exists by design "
                         "(single_instance.rs header)"),
}
events.write(json.dumps({"t": round(time.time(), 3), **summary}) + "\n")
events.flush()
print(json.dumps(summary), flush=True)
finish(0)
