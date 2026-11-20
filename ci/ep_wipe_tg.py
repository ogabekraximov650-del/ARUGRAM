#!/usr/bin/env python3
# ci/ep_wipe_tg.py — QISM VIDEOLARINI kanaldan ASOSIY HISOB sessiyasi (kanal egasi, Pyrogram) bilan o'chiradi, so'ng bazadan tozalaydi.
# Bot "message can't be deleted" deganda ishlatiladi (botda o'chirish huquqi yo'q). Rasm, avatar, foydalanuvchi, to'lov, anime/bo'lim QOLADI.
#   1) epizod_db: q360/q480/q720/q1080_vd_msgs dagi xabarlar -> kanal (VD_CHANNEL_ID); tg_files `orig%` va vd_queue `ep_/orig_` xabarlari.
#   2) Xabarlar o'chgach: epizod_db, encode_jobs, encode_q bo'shatiladi, season_db hisoblagichlari 0, belgi yoziladi (BIR MARTA).
import asyncio, json, os, sqlite3, sys, urllib.request
from pathlib import Path

import pyrogram.utils
from pyrogram import Client

pyrogram.utils.MIN_CHANNEL_ID = -1009999999999

url = os.environ.get("TURSO_URL", "").strip()
token = os.environ.get("TURSO_TOKEN", "").strip()
mark = open("ci/EP_WIPE_TG_ONCE").read().strip()
chan = int(open("ci/VD_CHANNEL_ID").read().strip())
tgchan = int(open("ci/TG_CHANNEL_ID").read().strip())
SESSION = os.environ["SESSION_PATH"]
if not url or not token or not mark:
    sys.exit("TURSO_URL / TURSO_TOKEN / belgi yo'q")
if url.startswith("libsql://"):
    url = "https://" + url[len("libsql://"):]


def run(*stmts):
    reqs = [{"type": "execute", "stmt": {"sql": s}} for s in stmts] + [{"type": "close"}]
    req = urllib.request.Request(url + "/v2/pipeline", data=json.dumps({"requests": reqs}).encode(),
                                 headers={"Authorization": "Bearer " + token, "Content-Type": "application/json"})
    with urllib.request.urlopen(req, timeout=120) as r:
        out = json.load(r)["results"]
    for x in out[:-1]:
        if x["type"] == "error":
            raise RuntimeError(x["error"]["message"])
    return [x["response"]["result"] for x in out[:-1]]


def rows(res):
    return [[c.get("value") for c in r] for r in res["rows"]]


def ids(s):
    return [int(x) for x in str(s or "").replace(" ", "").split(",") if x.strip().isdigit()]


tabs = [r[0] for r in rows(run("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'")[0])]
if "app_config" in tabs:
    d = rows(run("SELECT cfg_value FROM app_config WHERE cfg_key='ep_wipe_done'")[0])
    if d and d[0][0] == mark:
        print(f"'{mark}' belgisi bilan allaqachon bajarilgan — o'tkazib yuborildi.")
        sys.exit(0)
if "epizod_db" not in tabs or int(rows(run("SELECT COUNT(*) FROM anime_db")[0])[0][0]) == 0:
    sys.exit("anime_db bo'sh yoki epizod_db yo'q — noto'g'ri baza? To'xtatildi.")

vd_msgs, tg_msgs = [], []
for q in ("360", "480", "720", "1080"):
    for (m,) in rows(run(f"SELECT q{q}_vd_msgs FROM epizod_db WHERE COALESCE(q{q}_vd_msgs,'')<>''")[0]):
        vd_msgs += ids(m)
if "vd_queue" in tabs:
    for (m,) in rows(run("SELECT msg_ids FROM vd_queue WHERE name LIKE 'ep\\_%' ESCAPE '\\' OR name LIKE 'orig\\_%' ESCAPE '\\'")[0]):
        vd_msgs += ids(m)
for (m,) in rows(run("SELECT msg_id FROM tg_files WHERE file_name LIKE 'orig%'")[0]):
    tg_msgs += ids(m)
vd_msgs, tg_msgs = sorted(set(vd_msgs)), sorted(set(tg_msgs))
print(f"O'chiriladigan xabarlar: saqlash kanali {len(vd_msgs)}, asl video {len(tg_msgs)}")


def session_api_id():
    try:
        c = sqlite3.connect(f"file:{SESSION}.session?mode=ro", uri=True)
        return int(c.execute("SELECT api_id FROM sessions").fetchone()[0] or 0)
    except Exception:
        return 0


async def main():
    app = Client(SESSION, api_id=session_api_id() or int(os.environ["TG_API_ID"]), api_hash=os.environ["TG_API_HASH"], no_updates=True)
    total = 0
    async with app:
        async for _ in app.get_dialogs():
            pass
        for ch, lst in ((chan, vd_msgs), (tgchan, tg_msgs)):
            if not lst:
                continue
            await app.get_chat(ch)
            for i in range(0, len(lst), 100):
                part = lst[i:i + 100]
                n = await app.delete_messages(ch, part)
                total += int(n or 0)
                print(f"  kanal {ch}: {i + len(part)}/{len(lst)} (shu bosqichda o'chdi: {n})")
                await asyncio.sleep(1)
    print("Jami o'chirildi:", total)


asyncio.run(main())

print("2) Jadvallarni bo'shatish")
for t in ("epizod_db", "encode_jobs", "encode_q"):
    if t in tabs:
        run(f'DELETE FROM "{t}"')
        print("   tozalandi:", t)
for t in ("vd_queue",):
    if t in tabs:
        run("DELETE FROM vd_queue WHERE name LIKE 'ep\\_%' ESCAPE '\\' OR name LIKE 'orig\\_%' ESCAPE '\\'")
run("DELETE FROM tg_files WHERE file_name LIKE 'orig%'")
# 2026-11-20: faqat VIDEOLAR tozalanadi — tomosha tarixi, ko'rishlar soni va statistika TEGILMAYDI (foydalanuvchi: "barcha videolarni tozalab tashla"). Faqat qism soni 0.
if "season_db" in tabs:
    run("UPDATE season_db SET epizod_count=0")
    print("   season_db.epizod_count = 0")
run("CREATE TABLE IF NOT EXISTS app_config (cfg_key TEXT PRIMARY KEY, cfg_value TEXT)",
    f"INSERT OR REPLACE INTO app_config (cfg_key, cfg_value) VALUES ('ep_wipe_done', '{mark}')")
print(f"✅ Qism videolari kanaldan va bazadan tozalandi. Belgi: {mark}")
