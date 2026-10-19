#!/usr/bin/env python3
"""QA-audit GUI harness for SupportOs-Plus (AppImage under Xvfb).

One Session = one foreground tool-call lifetime (the sandbox janitor kills
long-lived background processes, so every scenario is a self-contained
foreground run that starts Xvfb + the app, executes scripted steps, captures
evidence, and tears everything down).

Input injection: python-xlib XTEST (real pointer/keyboard events, the same
channel a physical user uses).  Capture: ffmpeg x11grab PNG/MP4.
"""
from __future__ import annotations

import json
import os
import random
import shutil
import subprocess
import time
from pathlib import Path

REPO = Path(__file__).resolve().parents[5]          # .../app-clone
AUDIT = REPO / "qa-audit" / "20261019T050847Z"
EVID = AUDIT / "evidence"
APPIMAGE = REPO / "target" / "appimage" / "SupportOs-Plus-x86_64.AppImage"
XKB_LIBDIR = "/home/z/.local/lib/xkb/usr/lib/x86_64-linux-gnu"
XKB_ROOT = "/home/z/.local/lib/xkb"
SCREEN_W, SCREEN_H = 1280, 800


def _ensure_dirs() -> None:
    for d in ("screenshots", "videos", "logs", "artifacts", "traces"):
        (EVID / d).mkdir(parents=True, exist_ok=True)


class Session:
    def __init__(self, name: str, fresh_profile: bool = True,
                 extra_env: dict | None = None, port: int | None = None,
                 record: bool = False, appimage: bool = True,
                 profile_dir: str | None = None,
                 xvfb_binary: str = "/home/z/.local/bin/xa-helper"):
        _ensure_dirs()
        self.name = name
        self.display = f":{random.randint(88, 99)}"
        self.port = port or random.randint(13100, 13900)
        self.profile = Path(profile_dir or f"/tmp/spp-qa-profiles/{name}")
        if fresh_profile and self.profile.exists():
            shutil.rmtree(self.profile)
        self.profile.mkdir(parents=True, exist_ok=True)
        self.log_path = EVID / "logs" / f"{name}.log"
        self.procs: list[subprocess.Popen] = []
        self.win_geo = None          # (x, y, w, h) of the app window
        self.extra_env = extra_env or {}
        self.record = record
        self.video_path = EVID / "videos" / f"{name}.mp4"
        self.appimage = appimage
        self._xvfb = None
        self._app = None
        self._rec = None
        self._logf = None
        self._xvfb_err = None
        self._stopping = True
        self.t0 = 0.0
        self.input_events = 0
        self.xvfb_binary = xvfb_binary
        self.results: dict = {"session": name, "steps": []}

    # ---------------------------------------------------------------- env
    def _env(self) -> dict:
        env = os.environ.copy()
        env["DISPLAY"] = self.display
        env["SPP_DATA_DIR"] = str(self.profile)
        env["LD_LIBRARY_PATH"] = XKB_LIBDIR + ":" + env.get("LD_LIBRARY_PATH", "")
        env["HELPSCOUT_REDIRECT_URI"] = f"http://localhost:{self.port}/oauth/callback"
        env["LOG_LEVEL"] = env.get("LOG_LEVEL", "info")
        # eframe 0.37 defaults to the WGPU renderer → needs a Vulkan ICD.
        # lavapipe (software Vulkan) extracted into XKB_ROOT by the audit.
        env["VK_DRIVER_FILES"] = XKB_ROOT + "/vulkan/lvp_icd.json"
        env["VK_ICD_FILENAMES"] = XKB_ROOT + "/vulkan/lvp_icd.json"
        env["LIBGL_ALWAYS_SOFTWARE"] = "1"
        env.update(self.extra_env)
        return env

    # ------------------------------------------------------------- startup
    def start(self, wait_window: float = 40.0) -> None:
        import threading
        self.t0 = time.time()
        self._stopping = False
        self.input_events = 0
        self._xvfb_err = open(EVID / "logs" / f"{self.name}.xvfb.log", "ab")
        self._xvfb = subprocess.Popen(
            [self.xvfb_binary, self.display, "-screen", "0",
             f"{SCREEN_W}x{SCREEN_H}x24", "-dpi", "96", "-nolisten", "tcp"],
            stdout=subprocess.DEVNULL, stderr=self._xvfb_err)
        self.procs.append(self._xvfb)
        deadline = time.time() + 10
        while time.time() < deadline:
            if Path(f"/tmp/.X11-unix/X{self.display[1:]}").exists():
                break
            time.sleep(0.2)
        else:
            raise RuntimeError("Xvfb did not create its socket")
        time.sleep(0.5)
        # pyautogui/XTEST runs inside THIS process: it needs DISPLAY too.
        os.environ["DISPLAY"] = self.display

        if self.record:
            self._rec = subprocess.Popen(
                ["/home/z/.local/bin/fr-helper", "-y", "-loglevel", "error", "-f", "x11grab",
                 "-framerate", "8", "-video_size", f"{SCREEN_W}x{SCREEN_H}",
                 "-i", self.display, "-pix_fmt", "yuv420p", str(self.video_path)],
                stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            self.procs.append(self._rec)

        self._logf = open(self.log_path, "ab")
        if self.appimage:
            cmd = ["env", "APPIMAGE_EXTRACT_AND_RUN=1", str(APPIMAGE)]
        else:
            cmd = [str(REPO / "target" / "release" / "supportos-plusplus-app-shell")]
        self._app = subprocess.Popen(cmd, env=self._env(),
                                     stdout=self._logf, stderr=subprocess.STDOUT,
                                     start_new_session=True)
        self.procs.append(self._app)

        if not self.wait_for_window(wait_window):
            raise RuntimeError("app window never appeared")
        time.sleep(1.5)   # let first frames settle
        # Watchdog: if a child dies mid-session (sandbox killer observed to
        # terminate Xvfb during sustained input injection), flush partial
        # results and hard-exit so the tool call returns instead of hanging.
        threading.Thread(target=self._watchdog, daemon=True).start()

    def _watchdog(self) -> None:
        while True:
            time.sleep(0.5)
            if self._stopping:
                return
            dead = []
            if self._xvfb and self._xvfb.poll() is not None:
                dead.append(["xvfb", self._xvfb.poll()])
            if self._app and self._app.poll() is not None:
                dead.append(["app", self._app.poll()])
            if dead:
                self.results["death"] = {
                    "age_s": round(time.time() - self.t0, 2),
                    "input_events": self.input_events,
                    "dead": dead}
                try:
                    self.save_results()
                except Exception:
                    pass
                os._exit(77)

    def wait_for_window(self, timeout: float) -> bool:
        import Xlib.X
        import Xlib.display
        deadline = time.time() + timeout
        while time.time() < deadline:
            try:
                d = Xlib.display.Display(self.display)
                root = d.screen().root
                # Bare Xvfb has no WM, so _NET_CLIENT_LIST is never set —
                # walk the root's children instead.
                tree = root.query_tree()
                for w in tree.children:
                    try:
                        name = w.get_wm_name()
                        if name and "SupportOs-Plus" in name:
                            geo = w.get_geometry()
                            # top-level windows' parent IS the root here
                            self.win_geo = (geo.x, geo.y, geo.width, geo.height)
                            d.close()
                            return True
                    except Exception:
                        continue
                d.close()
            except Exception:
                pass
            time.sleep(0.3)
        return False

    # ------------------------------------------------------------- capture
    def grab(self):
        """Fast screen capture as an RGB ndarray via PIL/XCB (5-20ms)."""
        from PIL import ImageGrab
        import numpy as np
        return np.asarray(ImageGrab.grab(xdisplay=self.display).convert("RGB"))

    def screenshot(self, name: str) -> Path:
        """Persist a named PNG screenshot (PIL save; ffmpeg only for video)."""
        from PIL import Image
        out = EVID / "screenshots" / f"{name}.png"
        Image.fromarray(self.grab()).save(out)
        return out

    @staticmethod
    def _pair(a, b):
        import numpy as np
        from PIL import Image
        pa = np.asarray(Image.open(a).convert("RGB"), dtype=int) \
            if isinstance(a, (str, Path)) else a
        pb = np.asarray(Image.open(b).convert("RGB"), dtype=int) \
            if isinstance(b, (str, Path)) else b
        return pa, pb

    @staticmethod
    def diff(a, b, tol: int = 12) -> float:
        """Fraction of pixels differing beyond tol (0..1)."""
        import numpy as np
        ia, ib = Session._pair(a, b)
        return float(((ia - ib).abs() > tol).any(axis=2).mean())

    @staticmethod
    def diff_bbox(a, b, tol: int = 12):
        """Bounding box (x0,y0,x1,y1) of differing pixels, or None."""
        import numpy as np
        ia, ib = Session._pair(a, b)
        m = ((ia - ib).abs() > tol).any(axis=2)
        if not m.any():
            return None
        ys, xs = np.where(m)
        return int(xs.min()), int(ys.min()), int(xs.max()), int(ys.max())

    # --------------------------------------------------------------- input
    def _pa(self):
        import pyautogui
        pyautogui.FAILSAFE = False
        pyautogui.PAUSE = 0.03
        return pyautogui

    def click(self, x: int, y: int, button: str = "left") -> None:
        self.input_events += 1
        pa = self._pa()
        pa.moveTo(x, y, _pause=False)
        time.sleep(0.06)
        pa.mouseDown(button=button); time.sleep(0.05); pa.mouseUp(button=button)
        time.sleep(0.12)

    def dclick(self, x: int, y: int) -> None:
        pa = self._pa()
        pa.moveTo(x, y, _pause=False); time.sleep(0.06)
        pa.doubleClick()

    def move(self, x: int, y: int) -> None:
        self.input_events += 1
        self._pa().moveTo(x, y, _pause=False)
        time.sleep(0.25)             # let egui paint the hover state

    def press(self, *keys: str) -> None:
        self.input_events += len(keys)
        self._pa().press(list(keys))
        time.sleep(0.15)

    def hotkey(self, *keys: str) -> None:
        self.input_events += len(keys)
        self._pa().hotkey(*keys)
        time.sleep(0.2)

    def type(self, text: str, interval: float = 0.03) -> None:
        self.input_events += len(text)
        self._pa().typewrite(text, interval=interval)
        time.sleep(0.25)

    # -------------------------------------------------------------- output
    def note(self, **kv) -> None:
        self.results["steps"].append(kv)
        print("STEP", json.dumps(kv, default=str)[:400])

    def log_tail(self, lines: int = 5) -> str:
        try:
            out = subprocess.run(["tail", f"-{lines}", str(self.log_path)],
                                 capture_output=True, text=True, timeout=10)
            return out.stdout
        except Exception:
            return ""

    def save_results(self) -> Path:
        p = EVID / "artifacts" / f"{self.name}.json"
        p.write_text(json.dumps(self.results, indent=1, default=str))
        return p

    # ------------------------------------------------------------- teardown
    def stop(self) -> None:
        self._stopping = True
        for p in reversed(self.procs):
            try:
                p.terminate()
                p.wait(timeout=5)
            except Exception:
                try:
                    p.kill()
                except Exception:
                    pass
        if self._logf:
            self._logf.close()
        time.sleep(0.3)


if __name__ == "__main__":
    # self-test: boot the app, screenshot, teardown
    s = Session("harness-selftest", record=False)
    try:
        s.start()
        s.screenshot("H0-harness-selftest")
        s.note(step="boot", window=str(s.win_geo),
               log=s.log_tail(3).replace("\n", " | ")[:300])
        print("WINDOW", s.win_geo)
    finally:
        s.stop()
        s.save_results()
    print("SELFTEST OK")
