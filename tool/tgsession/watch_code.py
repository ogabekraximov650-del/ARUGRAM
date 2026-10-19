"""Telegram xizmat chati (777000) dan yangi kirish kodini o'qib, faylga yozadi.

python watch_code.py <sessiya> <SENT_FLAG> <CODE_FILE>  — SENT_FLAG paydo bo'lgach, undan keyin kelgan xabardan 5 xonali kod olinadi.
Kod chop etilmaydi.
"""
import asyncio, os, re, sqlite3, sys, time
from pyrogram import Client


def api_id_of(session):
    try:
        c = sqlite3.connect(f"file:{session}.session?mode=ro", uri=True)
        return int(c.execute("SELECT api_id FROM sessions").fetchone()[0] or 0)
    except Exception:
        return 0


async def main(session, flag, out):
    app = Client(session, api_id=api_id_of(session) or int(os.environ["TG_API_ID"]), api_hash=os.environ["TG_API_HASH"], no_updates=True)
    while not os.path.exists(flag):
        await asyncio.sleep(1)
    t0 = time.time() - 30
    async with app:
        for _ in range(150):
            async for m in app.get_chat_history(777000, limit=5):
                if m.date.timestamp() >= t0 and m.text:
                    mm = re.search(r"\b(\d{5})\b", m.text)
                    if mm:
                        open(out, "w").write(mm.group(1))
                        print("kod olindi")
                        return
            await asyncio.sleep(2)
    print("kod topilmadi")
    sys.exit(1)

asyncio.run(main(*sys.argv[1:4]))
