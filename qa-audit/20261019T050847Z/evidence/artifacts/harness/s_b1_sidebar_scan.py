#!/usr/bin/env python3
"""S-B1: sidebar deep scan — independent reproduction + characterization of
DEFECT-01 (dead/narrow sidebar hitboxes).

Phases:
  0. boot (fresh profile), base screenshot, window geometry
  1. palette-signature library: navigate to all 21 pages via Ctrl+K palette
     (authoritative navigation), screenshot each -> page signatures
  2. hover scan: pointer at x=40, y 28..620 step 2 -> detect hover-highlight
     bands (egui tints the widget's *senser rect*) -> map true hitboxes
  3. click probes: click detected band centers + text positions; classify the
     outcome page by signature matching; also probe the 4 previously-dead rows
Evidence: screenshots, video, sidebar-scan-B1.json
"""
import sys, time, json
from pathlib import Path
sys.path.insert(0, str(Path(__file__).parent))
from harness import Session, EVID

PAGES = ["Dashboard", "Inbox", "Operations", "Customers", "Organizations",
         "Knowledge", "Reports", "Issue Radar", "Incidents", "Custom Objects",
         "Automation", "AI Center", "Outreach", "Notifications", "Search",
         "Support Graph", "Connectors", "Sync Health", "Settings",
         "Onboarding", "Shell"]

SIDEBAR_X = 40
NEUTRAL = (700, 750)


def main():
    s = Session("B1-sidebar-scan", record=True)
    try:
        s.start()
        base = s.screenshot("B1-00-base")
        s.note(step="boot", window=str(s.win_geo))

        # ---- phase 1: signature library via palette ---------------------
        import numpy as np
        from PIL import Image as _PILImage
        SIG_SCALE = 4          # classify on 320x200 (small + fast)
        sigs = {}
        sig_arrs = {}
        def small(img_or_path):
            im = _PILImage.open(img_or_path).convert("L") \
                if isinstance(img_or_path, (str, Path)) else _PILImage.fromarray(img_or_path).convert("L")
            w, h = im.size
            return np.asarray(im.resize((w // SIG_SCALE, h // SIG_SCALE)), dtype=np.int16)
        missing = [pg for i, pg in enumerate(PAGES)
                   if not (EVID / "screenshots" / f"B1-sig-{i:02d}-{pg.replace(' ', '-')}").exists()]
        for i, page in enumerate(PAGES):
            shot = EVID / "screenshots" / f"B1-sig-{i:02d}-{page.replace(' ', '-')}"
            if page in missing:
                s.hotkey("ctrl", "k")
                time.sleep(0.25)
                s.type(page, interval=0.02)
                time.sleep(0.3)
                s.press("enter")
                time.sleep(0.7)
                shot = s.screenshot(f"B1-sig-{i:02d}-{page.replace(' ', '-')}")
            sigs[page] = shot
            sig_arrs[page] = small(shot)
        # pairwise sanity: min cross-page diff (calibrates classification)
        cross = []
        for a in PAGES:
            for b in PAGES:
                if a < b:
                    cross.append((round(float((small(sigs[a]) - small(sigs[b])).astype(int).__abs__().gt(10).mean()), 4), a, b))
        cross.sort()
        min_cross = cross[:3]
        s.note(step="signature-calibration", min_cross=min_cross)

        def classify(shot):
            arr = small(shot)
            best, bd = None, 1.0
            for page, sig in sig_arrs.items():
                d = float((arr - sig).astype(int).__abs__().gt(10).mean())
                if d < bd:
                    best, bd = page, d
            return best, round(bd, 4)

        # ---- phase 2: hover scan ----------------------------------------
        # move to neutral first and capture the resting state
        s.move(*NEUTRAL)
        from PIL import Image
        rest = s.grab()
        Image.fromarray(rest).save(EVID / "screenshots" / "B1-01-hover-rest.png")
        bands = []
        prev_hover = False
        band_start = None
        band_bbox = None
        hover_map = []
        rest8 = rest.astype(int)
        for y in range(28, 622, 2):
            s.move(SIDEBAR_X, y)
            shot = s.grab()
            m = ((shot.astype(int) - rest8).__abs__() > 12).any(axis=2)
            bbox = None
            if m.any():
                ys_, xs_ = np.where(m)
                bbox = (int(xs_.min()), int(ys_.min()), int(xs_.max()), int(ys_.max()))
            hovering = bbox is not None
            if y % 40 == 0:
                print("hover-progress", y, flush=True)
            hover_map.append({"y": y, "hover": hovering,
                              "bbox": list(bbox) if bbox else None})
            if hovering:
                if not prev_hover:
                    band_start = y
                    band_bbox = list(bbox)
                else:
                    band_bbox[0] = min(band_bbox[0], bbox[0])
                    band_bbox[1] = min(band_bbox[1], bbox[1])
                    band_bbox[2] = max(band_bbox[2], bbox[2])
                    band_bbox[3] = max(band_bbox[3], bbox[3])
            else:
                if prev_hover:
                    bands.append({"y0": band_start, "y1": y - 2,
                                  "bbox": band_bbox})
            prev_hover = hovering
            # persist only band-edge + sample frames
            if hovering != prev_hover or y % 50 == 0:
                Image.fromarray(shot).save(
                    EVID / "screenshots" / f"B1-hoverframe-{y}.png")
        if prev_hover:
            bands.append({"y0": band_start, "y1": 620, "bbox": band_bbox})
        s.note(step="hover-bands", bands=bands)

        # ---- phase 3: click probes --------------------------------------
        # 3a. click every detected hover band (center + edges)
        clicks = []
        for b in bands:
            yc = (b["y0"] + b["y1"]) // 2
            for label, yy in [("center", yc), ("top", b["y0"] + 1),
                              ("bottom", b["y1"] - 1)]:
                s.move(*NEUTRAL); time.sleep(0.2)
                pre = s.grab()
                s.click(SIDEBAR_X, yy)
                post = s.grab()
                Image.fromarray(post).save(
                    EVID / "screenshots" / f"B1-post-click-{label}-{yy}.png")
                page, dist = classify(post)
                changed = float((small(pre) - small(post)).astype(int).__abs__().gt(10).mean()) > 0.0005
                clicks.append({"y": yy, "pos": label, "band": [b["y0"], b["y1"]],
                               "band_bbox": b["bbox"], "result_page": page,
                               "sig_dist": dist, "changed": changed})
        # 3b. dead-zone probes: y positions inside rows that produced NO band
        dead_ys = []
        covered = set()
        for b in bands:
            for y in range(b["y0"], b["y1"] + 1, 2):
                covered.add(y)
        for y in range(60, 560, 22):          # visual row pitch estimate
            if not any(abs(y - c) <= 1 for c in covered):
                dead_ys.append(y)
        for y in dead_ys[:14]:
            s.move(*NEUTRAL); time.sleep(0.2)
            s.click(SIDEBAR_X, y)
            post = s.grab()
            Image.fromarray(post).save(
                EVID / "screenshots" / f"B1-deadprobe-{y}.png")
            page, dist = classify(post)
            clicks.append({"y": y, "pos": "dead-zone", "band": None,
                           "result_page": page, "sig_dist": dist,
                           "changed": None})
        s.note(step="clicks", clicks=clicks)

        s.results["window"] = s.win_geo
        s.results["hover_bands"] = bands
        s.results["clicks"] = clicks
        s.results["signatures"] = {p: str(v.name) for p, v in sigs.items()}
    finally:
        s.stop()
        out = s.save_results()
        print("RESULTS", out)


if __name__ == "__main__":
    main()
