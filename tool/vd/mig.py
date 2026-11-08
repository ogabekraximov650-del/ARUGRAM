#!/usr/bin/env python3
"""UZ: Migratsiya (alohida workflow `vdmig.yml`, ikkinchi akkauntdagi repoda): bazadagi HAR BIR eski sifat faylini (`tg_files`, kanalda,
    eski kalit bilan shifrlangan) kanaldan yuklab oladi, eski kalit bilan ochadi, YANGI tasodifiy kalit bilan (bitta fayl = bitta kalit)
    18 MiB bo'laklarga bo'lib shifrlaydi va bo'laklarni R2 ga yozadi. Bo'laklarni Telegram'ga saqlashni ALOHIDA yuklovchi (`vdstore.yml`) qiladi.
    Eski kanal posti va eski kalit O'CHIRILMAYDI — eski ilova va Mini App ishlashda davom etadi.
EN: Migration: for every legacy quality file download it from the channel, decrypt with the old key, re-encrypt with a NEW random key
    in 18 MiB chunks and stage the chunks in R2. A separate uploader workflow stores them in Telegram. The old post/key stay untouched.
RU: Migratsiya: kazhdyj staryj fajl kachestva skachivaetsya iz kanala, rasshifrovyvaetsya starym klyuchom, shifruetsya NOVYM klyuchom kuskami po
    18 MiB i kladetsya v R2. Sohranenie v Telegram delaet otdelnyj workflow. Staryj post i klyuch ne udalyayutsya.
"""
import asyncio
import os
import shutil
import sqlite3
import sys
import time
from pathlib import Path

from pyrogram import Client

import vdlib
from vdlib import api, log

SESSION = str(Path(__file__).with_name("pyro_session"))
WORK = Path(os.environ.get("RUNNER_TEMP", "/tmp")) / "vd_mig"
BUDGET = int(float(os.environ.get("START_BUDGET_MIN", "330")) * 60)
WAIT_MAX = int(os.environ.get("WAIT_MAX_MIN", "40")) * 60
T0 = time.time()


def session_api_id() -> int:
    try:
        c = sqlite3.connect(f"file:{SESSION}.session?mode=ro", uri=True)
        return int(c.execute("SELECT api_id FROM sessions").fetchone()[0] or 0)
    except Exception:  # noqa: BLE001
        return 0


async def one(app, job: dict) -> bool:
    name, msg_id, old_key, channel = job["name"], int(job["msg_id"]), job["key"], int(job["channel"])
    log(f"{name}: kanal posti #{msg_id}")
    shutil.rmtree(WORK, ignore_errors=True)
    WORK.mkdir(parents=True)
    dst = WORK / "old.bin"
    try:
        m = await app.get_messages(channel, msg_id)
        media = getattr(m, "document", None) or getattr(m, "video", None)
        if not m or m.empty or not media:
            raise RuntimeError("kanalda fayl topilmadi")
        want = int(media.file_size or 0)
        t0, last = time.time(), [0.0]

        def prog(cur, total):
            if time.time() - last[0] > 10:
                last[0] = time.time()
                log(f"  yuklab olinmoqda {cur * 100 // max(total, 1)}% ({cur / 1048576:.0f}/{total / 1048576:.0f} MB, "
                    f"{cur / 1048576 / max(time.time() - t0, 0.1):.1f} MB/s)")

        got = await app.download_media(m, file_name=str(dst), progress=prog)
        if not got or Path(got).stat().st_size != want or want <= 0:
            raise RuntimeError("yuklab olingan hajm mos emas")
        if Path(got) != dst:
            shutil.move(str(got), str(dst))
        new_key = os.urandom(16).hex()
        log(f"  {want / 1048576:.1f} MB — yangi kalit bilan {vdlib.chunks_of(want)} bo'lakka bo'linmoqda")
        waited = 0
        while True:
            st = await asyncio.to_thread(
                vdlib.stage_file, dst, name, new_key, old_key, True,
                lambda c, t: log(f"  R2 ga: bo'lak {c}/{t}") if c % 10 == 0 or c == t else None)
            if st != "wait":
                break
            waited += 60
            if waited > WAIT_MAX or time.time() - T0 > BUDGET:
                raise RuntimeError("R2 da joy ochilmadi")
            if waited == 60:
                log("  R2 da joy yo'q — yuklovchi bo'shatishini kutilmoqda...")
            await asyncio.sleep(60)
        log(f"{name}: tayyor ({st})")
        return True
    finally:
        shutil.rmtree(WORK, ignore_errors=True)


async def main():
    app = Client(SESSION, api_id=session_api_id() or int(os.environ["TG_API_ID"]),
                 api_hash=os.environ["TG_API_HASH"], no_updates=True)
    worked = 0
    async with app:
        async for _ in app.get_dialogs():
            pass
        while time.time() - T0 < BUDGET:
            job = api("/api/vd/mig/claim", {"runner": os.environ.get("GITHUB_RUN_ID", "local")})
            if job.get("wait"):
                log("R2 to'la — yuklovchi bo'shatishini kutilmoqda")
                await asyncio.sleep(60)
                if time.time() - T0 > WAIT_MAX and not worked:
                    break
                continue
            if job.get("none") or not job.get("name"):
                log("Navbat bo'sh.")
                break
            try:
                await one(app, job)
                worked += 1
            except Exception as e:  # noqa: BLE001
                log(f"XATO {job.get('name')}: {e}")
                try:
                    api("/api/vd/mig/fail", {"name": job["name"], "error": str(e)[:200]})
                except Exception:  # noqa: BLE001
                    pass
                await asyncio.sleep(10)
    log(f"Tugadi. O'tkazildi: {worked}")
    out = os.environ.get("GITHUB_OUTPUT")
    if out:
        with open(out, "a") as fh:
            fh.write(f"worked={'1' if worked else '0'}\n")


if __name__ == "__main__":
    if not vdlib.TOKEN:
        sys.exit("ENCODE_TOKEN yo'q")
    asyncio.run(main())
