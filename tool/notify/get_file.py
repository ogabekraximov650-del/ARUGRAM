"""Telegram "Saqlangan xabarlar"dan fayl yuklab oladi (eng yangisi, fayl nomi bo'yicha).

DrKLO Telegram'ning emulyator (x86_64) APK'si bir marta yig'ilib "Saqlangan xabarlar"da saqlanadi; skrinshot job'lari uni shu
yerdan oladi (qayta yig'ish shart emas).

Ishlatish: python get_file.py <sessiya_yo'li_(.session'siz)> <fayl_nomi> <chiqish_papkasi>
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


async def main(session: str, name: str, out: str) -> None:
    api_id = api_id_of(session) or int(os.environ["TG_API_ID"])
    app = Client(session, api_id=api_id, api_hash=os.environ["TG_API_HASH"], no_updates=True)
    os.makedirs(out, exist_ok=True)
    async with app:
        async for m in app.get_chat_history("me", limit=300):
            doc = m.document
            if doc and doc.file_name == name:
                path = await app.download_media(m, file_name=os.path.join(out, name))
                print("Yuklandi:", path, os.path.getsize(path))
                return
    print("Topilmadi:", name)
    sys.exit(1)


if __name__ == "__main__":
    asyncio.run(main(sys.argv[1], sys.argv[2], sys.argv[3]))
