"""Tayyor APK'ni Telegram hisobining "Saqlangan xabarlar"iga yuboradi.

Sessiya `tool/encode/session.enc` dan tiklangan (CI qadami). Xato bo'lsa
build buzilmaydi (qadam `continue-on-error`).

Ishlatish: python send_apk.py <sessiya_yo'li_(.session'siz)> <apk> <izoh>
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


async def main(session: str, apk: str, caption: str) -> None:
    api_id = api_id_of(session) or int(os.environ["TG_API_ID"])
    app = Client(session, api_id=api_id, api_hash=os.environ["TG_API_HASH"],
                 no_updates=True)
    async with app:
        await app.send_document("me", apk, caption=caption,
                                file_name=os.path.basename(apk))
    print("Saqlangan xabarlarga yuborildi:", os.path.basename(apk))


if __name__ == "__main__":
    asyncio.run(main(sys.argv[1], sys.argv[2], sys.argv[3]))
