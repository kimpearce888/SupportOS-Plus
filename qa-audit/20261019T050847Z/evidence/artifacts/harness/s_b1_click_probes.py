#!/usr/bin/env python3
"""S-B1c: click probes — verify navigation behavior at text positions vs
dead zones, for the DEFECT-01 correction record.

Positive controls  (x=20, over the label text): Inbox, Reports, Search,
  Shell (the four rows the prior session called "unclickable") + Dashboard
  (from a different page).
Dead-zone probes   (x=100 / x=130, empty area right of the label):
  Inbox, Reports, Search, Shell rows + x=130 on the LONGEST label
  (Support Graph) — proving even the widest row has a right-side dead zone.
Mechanism control  (x=40, inside senser, right of "Inbox" text end but
  within the +5px interaction radius... x=40 is inside the text rect) —
  covered by positives.

Navigation detection: central-panel diff (x>=200) pre vs post > 0.5%.
Page identification: min-distance match against the 21 palette-captured
page signatures (B1-sig-*.png, grayscale 4x downscale).
"""
import sys, time
from pathlib import Path
sys.path.insert(0, str(Path(__file__).parent))
import numpy as np
from PIL import Image
from harness import Session, EVID

PAGES = ["Dashboard", "Inbox", "Operations", "Customers", "Organizations",
         "Knowledge", "Reports", "Issue Radar", "Incidents", "Custom Objects",
         "Automation", "AI Center", "Outreach", "Notifications", "Search",
         "Support Graph", "Connectors", "Sync Health", "Settings",
         "Onboarding", "Shell"]

NEUTRAL = (700, 750)


def small(img):
    im = Image.fromarray(img).convert("L") if isinstance(img, np.ndarray) \
        else Image.open(img).convert("L")
    w, h = im.size
    return np.asarray(im.resize((w // 4, h // 4)), dtype=np.int16)


def central_diff(a, b):
    """Diff fraction restricted to the central panel (x>=200)."""
    aa = a[0:760, 200:1180].astype(int)
    bb = b[0:760, 200:1180].astype(int)
    return float((np.abs(aa - bb) > 12).any(axis=2).mean())


def main():
    # load signatures
    sigs = {}
    for i, page in enumerate(PAGES):
        p = EVID / "screenshots" / f"B1-sig-{i:02d}-{page.replace(' ', '-')}.png"
        if p.exists():
            sigs[page] = small(p)

    def classify(arr):
        a = small(arr)
        scored = sorted(
            ((float(np.abs(a.astype(int) - s.astype(int)).gt(10).mean()), pg)
             for pg, s in sigs.items()))
        return scored[0][1], round(scored[0][0], 4), round(scored[1][0], 4)

    s = Session("B1-click-probes", record=True)
    try:
        s.start()
        s.move(*NEUTRAL)
        probes = []

        def click_probe(tag, x, y, expect):
            pre = s.grab()
            s.click(x, y)
            time.sleep(0.45)          # navigation + repaint settle
            post = s.grab()
            Image.fromarray(post).save(
                EVID / "screenshots" / f"B1-click-{tag}.png")
            cdiff = central_diff(pre, post)
            page, d0, d1 = classify(post)
            navigated = cdiff > 0.005
            rec = {"tag": tag, "x": x, "y": y, "expect": expect,
                   "central_diff": round(cdiff, 5), "navigated": navigated,
                   "page_after": page, "sig_d0": d0, "sig_d1": d1,
                   "verdict": "PASS" if navigated == (expect == "nav") else "FAIL"}
            probes.append(rec)
            s.results["clicks"] = probes
            print("PROBE", rec, flush=True)

        # positives: over-text clicks on the 4 "dead" rows + return control
        click_probe("inbox-text", 20, 84, "nav")      # Inbox row
        click_probe("reports-text", 20, 200, "nav")   # Reports row
        click_probe("search-text", 20, 384, "nav")    # Search row
        click_probe("shell-text", 20, 520, "nav")     # Shell row
        # dead zones: empty area right of short labels
        click_probe("inbox-dead100", 100, 84, "nonav")
        click_probe("reports-dead100", 100, 200, "nonav")
        click_probe("search-dead100", 100, 384, "nonav")
        click_probe("shell-dead100", 100, 520, "nonav")
        # longest label still has a right dead zone
        click_probe("sgraph-dead130", 130, 408, "nonav")
        # positive return control proves Dashboard hitbox from another page
        click_probe("dashboard-text", 20, 62, "nav")

        s.note(step="click-probes-done", n=len(probes))
    finally:
        s.stop()
        s.save_results()
    npass = sum(1 for p in probes if p["verdict"] == "PASS")
    print(f"CLICK PROBES DONE: {npass}/{len(probes)} PASS")


if __name__ == "__main__":
    main()
