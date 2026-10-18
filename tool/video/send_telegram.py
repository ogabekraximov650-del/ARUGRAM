#!/usr/bin/env python3
"""Tayyor videoni Telegram'ga yuboradi (GitHub Actions, `video.yml` -> `telegram`).

Sessiya — kodlash workflow'lari ishlatadigan kanal egasining Pyrogram sessiyasi (`tool/encode/session.enc`).
Video shu hisobning "Saqlangan xabarlar"iga ("me") yuboriladi: boshqa hech kimga ko'rinmaydi.

Ishlatish:  python3 send_telegram.py <video.mp4>
"""
import asyncio
import os
import subprocess
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
SESSION = str(HERE / "pyro_session")


def log(*a):
    print(time.strftime("%H:%M:%S"), *a, flush=True)


async def main():
    video = Path(sys.argv[1])
    if not video.exists():
        sys.exit(f"{video} topilmadi")
    thumb = HERE / "tmp_thumb.jpg"
    subprocess.run(["ffmpeg", "-v", "error", "-y", "-ss", "3", "-i", str(video), "-frames:v", "1",
                    "-vf", "scale=320:-2", str(thumb)], check=False)

    import pyrogram.utils
    from pyrogram import Client

    pyrogram.utils.MIN_CHANNEL_ID = -1009999999999  # Pyrogram 2.0.106 yamog'i (kodlashdagidek)
    app = Client(SESSION, api_id=int(os.environ["TG_API_ID"]), api_hash=os.environ["TG_API_HASH"], no_updates=True)
    last = [0.0]

    def progress(cur, total):
        if time.time() - last[0] > 5:
            last[0] = time.time()
            log(f"yuborilmoqda: {cur * 100 // total}%")

    caption = ("ARUMEDIATV — ilova qanday ishlaydi (5 daqiqa)\n"
               "Akkauntga kirish, video pleyer, shifrlash va foydalanuvchi xavfsizligi.")
    async with app:
        msg = await app.send_video(
            "me", str(video), caption=caption, duration=300, width=1280, height=720,
            thumb=str(thumb) if thumb.exists() else None, supports_streaming=True, progress=progress)
    log(f"Yuborildi: Saqlangan xabarlar, xabar #{msg.id}")
    thumb.unlink(missing_ok=True)


if __name__ == "__main__":
    asyncio.run(main())
