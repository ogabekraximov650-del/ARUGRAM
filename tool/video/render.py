#!/usr/bin/env python3
"""scenes.html dan kadrlarni yozib oladi (Chromium, deterministik vaqt) va ffmpeg bilan MP4 qiladi.

Ishlatish:
  python3 render.py still <soniya> <fayl.png>             — bitta kadr (tekshirish uchun)
  python3 render.py segment <boshlanish> <tugash> <out.mp4>  — kadrlar oralig'i (kadr raqamlari) MP4 bo'lagiga
"""
import json
import os
import subprocess
import sys
from pathlib import Path

from playwright.sync_api import sync_playwright

HERE = Path(__file__).resolve().parent
TL = json.loads((HERE / "timeline.json").read_text(encoding="utf-8"))
FPS = TL["fps"]
CHROME = os.environ.get("CHROME", "/opt/pw-browsers/chromium-1194/chrome-linux/chrome")
ASSETS_JSON = HERE / "assets" / "assets.json"


def open_page(p):
    kw = {"args": ["--no-sandbox", "--disable-gpu", "--hide-scrollbars", "--force-color-profile=srgb"]}
    if Path(CHROME).exists():
        kw["executable_path"] = CHROME
    b = p.chromium.launch(**kw)
    pg = b.new_page(viewport={"width": 1280, "height": 720}, device_scale_factor=1)
    assets = {"posters": [], "episode": None}
    if ASSETS_JSON.exists():
        assets = json.loads(ASSETS_JSON.read_text(encoding="utf-8"))
    pg.add_init_script("window.TL = %s; window.ASSETS = %s;" % (json.dumps(TL, ensure_ascii=False), json.dumps(assets, ensure_ascii=False)))
    pg.on("pageerror", lambda e: print("JS xato:", e, file=sys.stderr))
    pg.goto((HERE / "scenes.html").as_uri())
    # rasmlar to'liq yuklanmaguncha kadr olinmaydi
    pg.evaluate("Promise.all(window.__imgs.map(i => i.decode().catch(() => 0))).then(() => 1)")
    pg.wait_for_timeout(300)
    return b, pg


def main():
    mode = sys.argv[1]
    with sync_playwright() as p:
        b, pg = open_page(p)
        if mode == "still":
            pg.evaluate("t => window.setTime(t)", float(sys.argv[2]))
            pg.screenshot(path=sys.argv[3])
        elif mode == "segment":
            a, z, out = int(sys.argv[2]), int(sys.argv[3]), sys.argv[4]
            ff = subprocess.Popen(
                ["ffmpeg", "-v", "error", "-y", "-f", "image2pipe", "-framerate", str(FPS), "-c:v", "mjpeg", "-i", "-",
                 "-c:v", "libx264", "-preset", "veryfast", "-crf", "19", "-pix_fmt", "yuv420p", "-r", str(FPS),
                 "-g", str(FPS), "-keyint_min", str(FPS), "-sc_threshold", "0", "-movflags", "+faststart", out],
                stdin=subprocess.PIPE)
            for i in range(a, z):
                pg.evaluate("t => window.setTime(t)", i / FPS)
                ff.stdin.write(pg.screenshot(type="jpeg", quality=93))
            ff.stdin.close()
            if ff.wait() != 0:
                sys.exit("ffmpeg xatosi")
        b.close()


if __name__ == "__main__":
    main()
