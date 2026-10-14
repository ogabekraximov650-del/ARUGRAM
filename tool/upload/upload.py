#!/usr/bin/env python3
"""R2 -> Telegram yuklovchi (alohida workflow `upload.yml`).

Kodlovchi workflow'lar tayyor faylni (shifrlangan MP4 / fMP4) Cloudflare R2'ga yozadi. Bu run ularni navbat bilan
R2'dan oladi (worker imzolangan havola beradi), ASOSIY botning chatiga yuboradi; bot faylni yopiq kanalga saqlab
bazaga yozadi va chatdan o'chiradi; worker R2 obyektini ham o'chiradi (`worker/src/r2.rs`, `encupload.rs`).
Telegram yuklash qotib qolsa (STALL_S) bekor qilinib, o'sha fayl (R2'dan qayta yuklamasdan) qayta yuboriladi.
Bir vaqtda faqat BITTA fayl yuklanadi — qotib qolish shundan kamayadi.
"""
import asyncio
import json
import os
import shutil
import sqlite3
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path

from pyrogram import Client

API = os.environ["API_BASE"].rstrip("/")
TOKEN = "".join(os.environ["ENCODE_TOKEN"].split())
SESSION = str(Path(__file__).with_name("pyro_session"))
WORK = Path(os.environ.get("RUNNER_TEMP", "/tmp")) / "arugram_upload"
RUNNER = f"up-{os.environ.get('GITHUB_RUN_ID', 'local')}-{os.environ.get('GITHUB_RUN_ATTEMPT', '1')}"
START_BUDGET = int(float(os.environ.get("START_BUDGET_MIN", "300")) * 60)
STALL_S = int(os.environ.get("UPLOAD_STALL_SEC", "120"))
SEND_TRIES = int(os.environ.get("UPLOAD_TRIES", "5"))
COMMIT_WAIT = int(os.environ.get("COMMIT_WAIT_SEC", "300"))
T0 = time.time()


def log(*a):
    print(time.strftime("%H:%M:%S"), *a, flush=True)


def session_api_id() -> int:
    try:
        c = sqlite3.connect(f"file:{SESSION}.session?mode=ro", uri=True)
        return int(c.execute("SELECT api_id FROM sessions").fetchone()[0] or 0)
    except Exception:  # noqa: BLE001
        return 0


def api(path: str, body: dict | None = None) -> dict:
    data = json.dumps(body or {}).encode()
    err = None
    for attempt in range(5):
        req = urllib.request.Request(
            f"{API}/api/r2/{path}", data=data, method="POST",
            headers={"X-Encode-Token": TOKEN, "Content-Type": "application/json", "User-Agent": "arugram-upload"})
        try:
            with urllib.request.urlopen(req, timeout=60) as r:
                return json.loads(r.read() or b"{}")
        except urllib.error.HTTPError as e:
            if e.code in (400, 401, 403, 404):
                raise RuntimeError(f"r2/{path}: HTTP {e.code}")
            err = e
        except Exception as e:  # noqa: BLE001
            err = e
        time.sleep(2 + attempt * 3)
    raise RuntimeError(f"r2/{path}: {err}")


def is_committed(name: str) -> bool:
    try:
        req = urllib.request.Request(
            f"{API}/api/encode/committed?file={urllib.parse.quote(name)}",
            headers={"X-Encode-Token": TOKEN, "User-Agent": "arugram-upload"})
        with urllib.request.urlopen(req, timeout=30) as r:
            return bool(json.loads(r.read() or b"{}").get("ok"))
    except Exception as ex:  # noqa: BLE001
        log(f"committed tekshiruvi xato: {ex}")
        return False


def download(url: str, dst: Path, size: int):
    """R2'dan oqim bilan yuklab oladi (xato bo'lsa qayta urinadi)."""
    last = None
    for attempt in range(1, 6):
        try:
            got, t0, last_log = 0, time.time(), 0.0
            with urllib.request.urlopen(urllib.request.Request(url, headers={"User-Agent": "arugram-upload"}), timeout=120) as r, \
                    open(dst, "wb") as f:
                while True:
                    chunk = r.read(1 << 20)
                    if not chunk:
                        break
                    f.write(chunk)
                    got += len(chunk)
                    if time.time() - last_log >= 5:
                        last_log = time.time()
                        log(f"    R2'dan yuklab olinmoqda {got * 100 // max(size, 1)}% "
                            f"({got / 1048576:.0f}/{size / 1048576:.0f} MB, {got / 1048576 / max(time.time() - t0, 0.1):.1f} MB/s)")
            if dst.stat().st_size == size:
                return
            last = f"hajm mos emas ({dst.stat().st_size} != {size})"
        except Exception as ex:  # noqa: BLE001
            last = ex
        log(f"  R2'dan olish {attempt}/5 muvaffaqiyatsiz: {last}")
        time.sleep(3 * attempt)
    raise RuntimeError(f"R2'dan olib bo'lmadi: {last}")


async def beat_loop(name: str):
    while True:
        await asyncio.sleep(60)
        try:
            await asyncio.to_thread(api, "beat", {"file": name})
        except Exception as ex:  # noqa: BLE001
            log(f"beat xato: {ex}")


async def send_to_bot(app, bot: str, path: Path, name: str, caption: str):
    """Faylni botga yuboradi; qotib qolsa (STALL_S) bekor qilib qayta yuboradi. Bot allaqachon saqlagan bo'lsa — tamom."""
    size = path.stat().st_size
    last_err = ""
    for attempt in range(1, SEND_TRIES + 1):
        prog = {"p": -1, "t": time.time(), "log": 0.0}

        def cb(cur, total, prog=prog):
            if cur != prog["p"]:
                prog["p"], prog["t"] = cur, time.time()
            if time.time() - prog["log"] >= 5:
                prog["log"] = time.time()
                log(f"    Telegram'ga yuklanmoqda {cur * 100 // max(total, 1)}% ({cur / 1048576:.0f}/{total / 1048576:.0f} MB)")

        task = asyncio.create_task(app.send_document(
            bot, str(path), file_name=name, force_document=True, caption=caption,
            disable_notification=True, progress=cb))
        stalled = False
        while True:
            done, _ = await asyncio.wait({task}, timeout=10)
            if done:
                break
            if time.time() - prog["t"] > STALL_S:
                stalled = True
                task.cancel()
                try:
                    await task
                except BaseException:  # noqa: BLE001
                    pass
                break
        if not stalled:
            try:
                task.result()
                return
            except Exception as ex:  # noqa: BLE001
                last_err = f"{type(ex).__name__}: {ex}"
                if type(ex).__name__ == "FloodWait":
                    await asyncio.sleep(int(getattr(ex, "value", 10) or 10))
        else:
            last_err = f"{STALL_S} soniya qimirlamadi ({prog['p'] * 100 // max(size, 1)}%)"
        log(f"  yuklash {attempt}/{SEND_TRIES} muvaffaqiyatsiz — {last_err}")
        if await asyncio.to_thread(is_committed, name):
            log("  fayl allaqachon saqlangan — qayta yuklanmaydi")
            return
        await asyncio.sleep(5 * attempt)
    raise RuntimeError(f"Telegram'ga yuklab bo'lmadi ({SEND_TRIES} urinish): {last_err}")


async def one(app, job: dict):
    name, size = job["file"], int(job["size"])
    path = WORK / name
    shutil.rmtree(WORK, ignore_errors=True)
    WORK.mkdir(parents=True)
    beat = asyncio.create_task(beat_loop(name))
    try:
        log(f"{name}: R2'dan olinmoqda ({size / 1048576:.1f} MB), urinish {job.get('attempt')}")
        await asyncio.to_thread(download, job["url"], path, size)
        log(f"{name}: botga yuborilmoqda")
        await send_to_bot(app, job["bot"], path, name, job["caption"])
        end = time.time() + COMMIT_WAIT
        while time.time() < end and not await asyncio.to_thread(is_committed, name):
            await asyncio.sleep(3)
        if not await asyncio.to_thread(is_committed, name):
            raise RuntimeError(f"bot {COMMIT_WAIT} soniyada faylni saqlamadi (bot kanalda adminmi? sessiya ro'yxatdan o'tganmi?)")
        r = await asyncio.to_thread(api, "done", {"file": name})
        log(f"{name}: bazaga yozildi, R2'dan o'chirildi ({r.get('deleted')})")
    finally:
        beat.cancel()
        shutil.rmtree(WORK, ignore_errors=True)


async def main():
    api_id = session_api_id() or int(os.environ["TG_API_ID"])
    app = Client(SESSION, api_id=api_id, api_hash=os.environ["TG_API_HASH"], no_updates=True)
    async with app:
        async for _ in app.get_dialogs():
            pass
        worked = 0
        while time.time() - T0 < START_BUDGET:
            job = api("claim", {"runner": RUNNER})
            if job.get("nosession"):
                log("Yuklovchi sessiyasi ulanmagan (botda 'Yangi sessiya' -> upload).")
                break
            if job.get("none") or not job.get("file"):
                log("Navbat bo'sh.")
                break
            try:
                await one(app, job)
                worked += 1
            except Exception as ex:  # noqa: BLE001
                log(f"XATO: {job.get('file')}: {ex}")
                try:
                    api("fail", {"file": job["file"], "error": str(ex)[:250]})
                except Exception as e2:  # noqa: BLE001
                    log(f"fail yozilmadi: {e2}")
        log(f"Run tugadi. Yuklandi: {worked}")


if __name__ == "__main__":
    try:
        asyncio.run(main())
    except Exception as e:  # noqa: BLE001
        log("XATO:", e)
        sys.exit(1)
