#!/usr/bin/env python3
"""Telegram hisobidagi animatsion (.tgs) stikerlarni sinov uchun yuklab oladi (faqat O'QISH).

Manba: o'rnatilgan stiker to'plamlari, maxsus emoji to'plamlari va tavsiya etilgan (featured) to'plamlar.
Har to'plamdan ko'pi bilan PER_SET ta fayl olinadi (xilma-xillik uchun). Hech narsa o'zgartirilmaydi / yuborilmaydi.

Ishlatish: fetch.py <chiqish_papka> <jami_limit> [PER_SET]
Sessiya: `tool/tgs/pyro_session.session` (workflow tiklaydi), TG_API_ID / TG_API_HASH muhitdan.
"""

import asyncio
import os
import sqlite3
import sys
from pathlib import Path

import pyrogram.utils
from pyrogram import Client
from pyrogram.errors import FloodWait
from pyrogram.file_id import FileId, FileType
from pyrogram.raw import functions, types

SESSION = str(Path(__file__).with_name("pyro_session"))
TGS = "application/x-tgsticker"


def _session_api_id() -> int:
    try:
        c = sqlite3.connect(f"file:{SESSION}.session?mode=ro", uri=True)
        return int(c.execute("SELECT api_id FROM sessions").fetchone()[0] or 0)
    except Exception:
        return 0


async def call(app, req, tries=4):
    for _ in range(tries):
        try:
            return await app.invoke(req)
        except FloodWait as e:
            await asyncio.sleep(min(int(e.value) + 1, 60))
    return None


async def main():
    out = Path(sys.argv[1])
    limit = int(sys.argv[2])
    per_set = int(sys.argv[3]) if len(sys.argv) > 3 else 8
    out.mkdir(parents=True, exist_ok=True)
    pyrogram.utils.MIN_CHANNEL_ID = -1009999999999
    api_id = _session_api_id() or int(os.environ["TG_API_ID"])
    app = Client(SESSION, api_id=api_id, api_hash=os.environ["TG_API_HASH"], no_updates=True)
    got = 0
    seen = set()
    async with app:
        sources = []
        for kind, req in (
            ("st", functions.messages.GetAllStickers(hash=0)),
            ("em", functions.messages.GetEmojiStickers(hash=0)),
            ("ft", functions.messages.GetFeaturedStickers(hash=0)),
            ("fe", functions.messages.GetFeaturedEmojiStickers(hash=0)),
        ):
            r = await call(app, req)
            if r is None:
                print(f"{kind}: so'rov bajarilmadi")
                continue
            sets = []
            for s in getattr(r, "sets", []) or []:
                info = getattr(s, "set", s)  # StickerSetCovered -> .set
                sets.append(info)
            print(f"{kind}: {len(sets)} ta to'plam")
            sources += [(kind, s) for s in sets]
        for kind, s in sources:
            if got >= limit:
                break
            if s.id in seen or getattr(s, "masks", False):
                continue
            seen.add(s.id)
            full = await call(app, functions.messages.GetStickerSet(
                stickerset=types.InputStickerSetID(id=s.id, access_hash=s.access_hash), hash=0))
            if full is None:
                continue
            n = 0
            for d in full.documents:
                if n >= per_set or got >= limit:
                    break
                if d.mime_type != TGS:
                    continue
                fid = FileId(major=FileId.MAJOR, minor=FileId.MINOR, file_type=FileType.STICKER, dc_id=d.dc_id,
                             media_id=d.id, access_hash=d.access_hash, file_reference=d.file_reference)
                path = out / f"{kind}_{s.short_name}_{d.id}.tgs"
                try:
                    await app.download_media(fid.encode(), file_name=str(path))
                except Exception as e:  # bitta fayl yiqilsa, qolganlari davom etadi
                    print(f"yuklanmadi {path.name}: {type(e).__name__}")
                    continue
                if path.exists() and path.stat().st_size > 50:
                    n += 1
                    got += 1
            print(f"{kind} {s.short_name}: {n} ta")
    print(f"JAMI: {got} ta .tgs")


if __name__ == "__main__":
    asyncio.run(main())
