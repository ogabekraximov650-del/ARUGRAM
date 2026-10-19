"""Mavjud (pyrogram) sessiyadan hisob raqamini oladi: python get_phone.py <sessiya> <chiqish_fayli>."""
import asyncio, os, sqlite3, sys
from pyrogram import Client


def api_id_of(session):
    try:
        c = sqlite3.connect(f"file:{session}.session?mode=ro", uri=True)
        return int(c.execute("SELECT api_id FROM sessions").fetchone()[0] or 0)
    except Exception:
        return 0


async def main(session, out):
    app = Client(session, api_id=api_id_of(session) or int(os.environ["TG_API_ID"]), api_hash=os.environ["TG_API_HASH"], no_updates=True)
    async with app:
        me = await app.get_me()
        open(out, "w").write(me.phone_number if me.phone_number.startswith("+") else "+" + me.phone_number)
    print("raqam olindi")

asyncio.run(main(sys.argv[1], sys.argv[2]))
