#!/usr/bin/env python3
# ci/collage_clean.py — BIR MARTA: saqlash kanalida qolib ketgan (qismi o'chirilgan) kollaj hujjatlarini o'chiradi.
# Kollaj hujjati nomi `collage_<anime>_<bo'lim>_<qism>.bin`. Bazada (`epizod_db.collage_msg`) hisobda bo'lganlari QOLADI, qolganlari o'chadi.
import asyncio, json, os, re, sqlite3, urllib.request
import pyrogram.utils
from pyrogram import Client

pyrogram.utils.MIN_CHANNEL_ID = -1009999999999
url = os.environ["TURSO_URL"].strip().rstrip("/")
tok = os.environ["TURSO_TOKEN"].strip()
SESSION = os.environ["SESSION_PATH"]
chan = int(open("ci/TG_CHANNEL_ID").read().strip())
PAT = re.compile(r"^collage_\d+_\d+_\d+\.bin$")


def q(sql):
    req = urllib.request.Request(url + "/v2/pipeline", data=json.dumps({"requests": [{"type": "execute", "stmt": {"sql": sql}}, {"type": "close"}]}).encode(),
                                 headers={"Authorization": "Bearer " + tok, "Content-Type": "application/json"})
    r = json.load(urllib.request.urlopen(req, timeout=60))["results"][0]
    if r["type"] == "error":
        raise RuntimeError(r["error"]["message"])
    return [[c.get("value") for c in row] for row in r["response"]["result"]["rows"]]


keep = {int(r[0]) for r in q("SELECT collage_msg FROM epizod_db WHERE COALESCE(collage_msg,0) > 0")}
print("bazada hisobdagi kollaj xabarlari:", sorted(keep))


def api_id():
    try:
        c = sqlite3.connect(f"file:{SESSION}.session?mode=ro", uri=True)
        return int(c.execute("SELECT api_id FROM sessions").fetchone()[0] or 0)
    except Exception:
        return 0


async def main():
    app = Client(SESSION, api_id=api_id() or int(os.environ["TG_API_ID"]), api_hash=os.environ["TG_API_HASH"], no_updates=True)
    async with app:
        async for _ in app.get_dialogs():
            pass
        found, drop = [], []
        async for m in app.get_chat_history(chan):
            d = m.document
            if d and d.file_name and PAT.match(d.file_name):
                found.append(m.id)
                if m.id not in keep:
                    drop.append(m.id)
                    print(f"  yetim: xabar {m.id} {d.file_name}")
        print(f"kanalda kollaj hujjatlari: {len(found)}; yetim: {len(drop)}")
        for i in range(0, len(drop), 100):
            n = await app.delete_messages(chan, drop[i:i + 100])
            print("  o'chirildi:", n)
    print("✅ tayyor")

asyncio.run(main())
