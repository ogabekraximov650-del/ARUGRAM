#!/usr/bin/env python3
# ci/send_apk.py — APK ni (GitHub Release havolasidan) ASOSIY HISOB sessiyasi bilan "Saqlangan xabarlar"ga yuboradi (Pyrogram; 2 GB gacha).
# `ci/SEND_APK`: 1-qator — yuklab olish havolasi, 2-qator — izoh. Fayl o'zgarganda `send-apk.yml` ishga tushadi (BIR MARTA).
import asyncio, os, sqlite3, sys, urllib.request
from pathlib import Path

from pyrogram import Client

lines = [l.rstrip("\n") for l in open("ci/SEND_APK").read().splitlines() if l.strip()]
if not lines or not lines[0].startswith("https://"):
    sys.exit("ci/SEND_APK: 1-qator https:// havola bo'lishi kerak")
URL = lines[0].strip()
CAPTION = lines[1] if len(lines) > 1 else ""
SESSION = os.environ["SESSION_PATH"]
OUT = Path(os.environ.get("RUNNER_TEMP", "/tmp")) / "app.apk"


def session_api_id():
    try:
        c = sqlite3.connect(f"file:{SESSION}.session?mode=ro", uri=True)
        return int(c.execute("SELECT api_id FROM sessions").fetchone()[0] or 0)
    except Exception:
        return 0


def download():
    req = urllib.request.Request(URL, headers={"User-Agent": "arugram-ci"})
    with urllib.request.urlopen(req, timeout=300) as r, open(OUT, "wb") as f:
        while True:
            b = r.read(1 << 20)
            if not b:
                break
            f.write(b)
    size = OUT.stat().st_size
    if size < 1_000_000:
        sys.exit(f"yuklab olingan fayl juda kichik ({size} bayt) — havola noto'g'ri?")
    print(f"yuklab olindi: {size / 1048576:.1f} MiB")


async def main():
    download()
    app = Client(SESSION, api_id=session_api_id() or int(os.environ["TG_API_ID"]), api_hash=os.environ["TG_API_HASH"], no_updates=True)
    last = [0]

    async def prog(cur, total):
        pct = int(cur * 100 / total) if total else 0
        if pct >= last[0] + 10:
            last[0] = pct
            print(f"yuborilmoqda: {pct}%")

    async with app:
        m = await app.send_document("me", str(OUT), caption=CAPTION, file_name="telegram-glass.apk", progress=prog)
        print(f"yuborildi: Saqlangan xabarlar, xabar #{m.id}")


asyncio.run(main())
