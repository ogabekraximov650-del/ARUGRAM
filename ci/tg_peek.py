#!/usr/bin/env python3
# ci/tg_peek.py — FAQAT O'QISH: saqlash kanalidagi xabarlar (raqam, tur, hajm, nom, sana) va sessiya egasi. Kodlash xatosi ("asl video kanalda topilmadi") tashxisi uchun.
import asyncio, os, sqlite3
import pyrogram.utils
from pyrogram import Client

pyrogram.utils.MIN_CHANNEL_ID = -1009999999999
SESSION = os.environ["SESSION_PATH"]
chan = int(open("ci/VD_CHANNEL_ID").read().strip())
ids = []
for tok in open("ci/TG_PEEK").read().split():
    if "-" in tok.strip("-"):
        a, b = tok.split("-", 1)
        ids += list(range(int(a), int(b) + 1))
    elif tok.lstrip("-").isdigit():
        ids.append(int(tok))


def api_id():
    try:
        c = sqlite3.connect(f"file:{SESSION}.session?mode=ro", uri=True)
        return int(c.execute("SELECT api_id FROM sessions").fetchone()[0] or 0)
    except Exception:
        return 0


def brief(m):
    if not m or m.empty:
        return "BO'SH (xabar yo'q yoki ko'rinmaydi)"
    d = m.document or m.video
    nm = getattr(d, "file_name", "") if d else ""
    sz = getattr(d, "file_size", 0) if d else 0
    cap = (m.caption or m.text or "")[:60].replace("\n", " ")
    return f"{'hujjat/video' if d else 'boshqa'} nom={nm!r} hajm={sz} sana={m.date} izoh={cap!r} kimdan={getattr(m.from_user, 'id', None) or getattr(m.sender_chat, 'id', None)}"


async def main():
    app = Client(SESSION, api_id=api_id() or int(os.environ["TG_API_ID"]), api_hash=os.environ["TG_API_HASH"], no_updates=True)
    async with app:
        async for _ in app.get_dialogs():
            pass
        print("sessiya hisobi:", app.me.id, app.me.username)
        chat = await app.get_chat(chan)
        print("kanal:", chat.id, chat.title, "a'zolar:", getattr(chat, "members_count", None))
        for i in ids:
            m = await app.get_messages(chan, i)
            print(f"  xabar {i}: {brief(m)}")
        # Kim o'chirdi: kanal harakatlar jurnali (o'chirilgan xabarlar), faqat egasi/admin ko'radi.
        try:
            from pyrogram.types import ChatEventFilter
            print("--- kanal jurnali: o'chirilgan xabarlar (oxirgi 40) ---")
            n = 0
            async for ev in app.get_chat_event_log(chan, filters=ChatEventFilter(deleted_messages=True), limit=40):
                dm = ev.deleted_message
                d = (dm.document or dm.video) if dm else None
                print(f"  {ev.date} kim={getattr(ev.user, 'id', None)} xabar={getattr(dm, 'id', None)} nom={getattr(d, 'file_name', None)!r} hajm={getattr(d, 'file_size', None)}")
                n += 1
            print(f"  jami {n} ta")
        except Exception as ex:  # noqa: BLE001
            print("kanal jurnali o'qilmadi:", type(ex).__name__, str(ex)[:200])
        print("--- oxirgi 5 xabar ---")
        async for m in app.get_chat_history(chan, limit=5):
            print(f"  #{m.id}: {brief(m)}")
        print("--- 'ARUSTAGE' (anibla bosqichi) xabarlari va .p0/.p1 hujjatlar (oxirgi 1500 xabar) ---")
        n = 0
        async for m in app.get_chat_history(chan, limit=1500):
            d = m.document or m.video
            nm = (getattr(d, "file_name", "") or "") if d else ""
            if (m.caption or "").startswith("ARUSTAGE") or ".p0" in nm or ".p1" in nm:
                print(f"  #{m.id}: {brief(m)}")
                n += 1
        print("topilgan:", n)


asyncio.run(main())
