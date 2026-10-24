"""Tayyor videoni Telegram hisobining "Saqlangan xabarlar"iga VIDEO sifatida yuboradi (ichida o'ynaydi).

Sessiya `tool/encode/session.enc` dan tiklangan (CI qadami). Xato bo'lsa
workflow buzilmaydi (qadam `continue-on-error`).

Ishlatish: python send_video.py <sessiya_yo'li_(.session'siz)> <video.mp4> <izoh> [davomiylik_s] [kenglik] [balandlik]
"""
import asyncio
import os
import sqlite3
import sys

from pyrogram import Client


def api_id_of(session: str) -> int:
    try:
        c = sqlite3.connect(f"file:{session}.session?mode=ro", uri=True)
        return int(c.execute("SELECT api_id FROM sessions").fetchone()[0] or 0)
    except Exception:
        return 0


async def main(session: str, path: str, caption: str, dur: int, w: int, h: int) -> None:
    api_id = api_id_of(session) or int(os.environ["TG_API_ID"])
    app = Client(session, api_id=api_id, api_hash=os.environ["TG_API_HASH"], no_updates=True)
    async with app:
        await app.send_video("me", path, caption=caption, duration=dur, width=w, height=h,
                             supports_streaming=True, file_name=os.path.basename(path))
    print("Saqlangan xabarlarga yuborildi:", os.path.basename(path))


if __name__ == "__main__":
    a = sys.argv
    asyncio.run(main(a[1], a[2], a[3], int(a[4]) if len(a) > 4 else 0, int(a[5]) if len(a) > 5 else 1280, int(a[6]) if len(a) > 6 else 720))
