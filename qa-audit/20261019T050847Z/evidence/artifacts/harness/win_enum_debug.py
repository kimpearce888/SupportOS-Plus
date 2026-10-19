#!/usr/bin/env python3
"""Enumerate X11 windows + name properties on a live display."""
import subprocess, sys, time, os
sys.path.insert(0, os.path.dirname(__file__))
import harness
from harness import Session

s = Session("win-enum")
try:
    # start everything except the window wait:
    s.start.__wrapped__ if False else None
    import random
    s.display = f":{random.randint(88, 99)}"
    s._xvfb = subprocess.Popen(["Xvfb", s.display, "-screen", "0", "1280x800x24",
                                "-dpi", "96", "-nolisten", "tcp"],
                               stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    s.procs.append(s._xvfb)
    time.sleep(2)
    s._logf = open(s.log_path, "ab")
    s._app = subprocess.Popen(["env", "APPIMAGE_EXTRACT_AND_RUN=1",
                               str(harness.APPIMAGE)],
                              env=s._env(), stdout=s._logf,
                              stderr=subprocess.STDOUT, start_new_session=True)
    s.procs.append(s._app)
    time.sleep(8)

    import Xlib.X
    import Xlib.display
    d = Xlib.display.Display(s.display)
    root = d.screen().root
    print("ROOT children:")
    for w in root.query_tree().children:
        try:
            geo = w.get_geometry()
            wm_name = w.get_wm_name()
            net = w.get_full_property(d.intern_atom("_NET_WM_NAME"), Xlib.X.AnyPropertyType)
            net_val = net.value if net else None
            attrs = w.get_attributes()
            print(f"  id={w.id} geo=({geo.x},{geo.y},{geo.width},{geo.height}) "
                  f"mapped={attrs.map_state} wm_name={wm_name!r} net_wm_name={net_val!r}")
        except Exception as e:
            print(f"  id={wid} ERR {e}")
    d.close()
finally:
    s.stop()
