#!/usr/bin/env python3
# ci/tg_peek.py — FAQAT O'QISH: saqlash kanalidagi xabarlar (raqam, tur, hajm, nom, sana) va sessiya egasi. Kodlash xatosi ("asl video kanalda topilmadi") tashxisi uchun.
import asyncio, os, sqlite3
import pyrogram.utils
from pyrogram import Client

pyrogram.utils.MIN_CHANNEL_ID = -1009999999999
SESSION = os.environ["SESSION_PATH"]
chan = int(open("ci/VD_CHANNEL_ID").read().strip())
ids = []
deep_ids = []   # "d3177" — xabarni TO'LIQ tekshirish: xom atributlar, yuklab olish va ffprobe (videolarni solishtirish uchun)
thumb_ids = []  # "t3177" — yuklab olmasdan: xom atributlar va kichik rasmlar (thumbs) o'lchami
for tok in open("ci/TG_PEEK").read().split():
    if tok.startswith("d") and tok[1:].isdigit():
        deep_ids.append(int(tok[1:]))
        continue
    if tok.startswith("t") and tok[1:].isdigit():
        thumb_ids.append(int(tok[1:]))
        continue
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


async def deep(app, chat, i):
    import subprocess
    from pyrogram.raw import functions, types
    print(f"\n===== xabar {i}: TO'LIQ TEKSHIRUV =====")
    peer = await app.resolve_peer(chat)
    r = await app.invoke(functions.channels.GetMessages(channel=peer, id=[types.InputMessageID(id=i)]))
    raw = r.messages[0] if r.messages else None
    doc = getattr(getattr(raw, "media", None), "document", None)
    if doc is None:
        print("  hujjat yo'q:", type(getattr(raw, "media", None)).__name__)
        return
    print(f"  xom: mime={doc.mime_type} hajm={doc.size} dc={doc.dc_id} thumbs={len(doc.thumbs or [])} video_thumbs={len(getattr(doc, 'video_thumbs', None) or [])}")
    print(f"  media bayroqlari: spoiler={getattr(raw.media, 'spoiler', None)} video={getattr(raw.media, 'video', None)} round={getattr(raw.media, 'round', None)}")
    for a in doc.attributes:
        print("  atribut:", type(a).__name__, " ".join(str(a).split()))
    print("  xabar matni:", repr((raw.message or "")[:120]), "| sana:", raw.date, "| kimdan:", getattr(raw.from_id, "user_id", None) or getattr(raw.from_id, "channel_id", None))
    for th in (doc.thumbs or []):
        print("  kichik rasm:", type(th).__name__, " ".join(str(th).split())[:200])
    if i in thumb_ids:
        return
    m = await app.get_messages(chat, i)
    if doc.size > 600 * 1024 * 1024:
        print("  juda katta — yuklab olinmadi")
        return
    path = await app.download_media(m, file_name=f"/tmp/deep_{i}.bin")
    print("  yuklandi:", path, os.path.getsize(path))
    head = open(path, "rb").read(4096)
    print("  boshi (hex):", head[:32].hex(), "| ftyp:", b"ftyp" in head[:16])
    with open(path, "rb") as f:
        d = f.read(8 * 1024 * 1024)
    mo, md = d.find(b"moov"), d.find(b"mdat")
    print(f"  moov o'rni: {mo}, mdat o'rni: {md} (moov fayl boshida: {0 <= mo < (md if md >= 0 else 10**12)})")
    out = subprocess.run(["ffprobe", "-v", "error", "-show_entries",
                          "stream=index,codec_type,codec_name,codec_tag_string,profile,width,height,sample_aspect_ratio,display_aspect_ratio,r_frame_rate,avg_frame_rate,pix_fmt,color_range,color_space,time_base,nb_frames:stream_side_data:stream_tags:format=format_name,duration,bit_rate:format_tags",
                          "-of", "default=nw=1", path], capture_output=True, text=True)
    print("  ffprobe:\n" + "\n".join("    " + x for x in (out.stdout or out.stderr).splitlines()[:80]))
    os.remove(path)


async def main():
    app = Client(SESSION, api_id=api_id() or int(os.environ["TG_API_ID"]), api_hash=os.environ["TG_API_HASH"], no_updates=True)
    async with app:
        async for _ in app.get_dialogs():
            pass
        print("sessiya hisobi:", app.me.id, app.me.username)
        chat = await app.get_chat(chan)
        if deep_ids or thumb_ids:
            for i in deep_ids + thumb_ids:
                try:
                    await deep(app, chan, i)
                except Exception as ex:  # noqa: BLE001
                    print(f"  xabar {i} tekshiruvi xato: {type(ex).__name__}: {str(ex)[:200]}")
            return
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
