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
# Yuklash 100% ga yetgach Telegram tasdiqni uzoq kechiktirishi mumkin (akkaunt yuklashi cheklangan): bu vaqtda bekor qilinmaydi,
# faqat shuncha kutiladi (bot faylni allaqachon olgan bo'lsa — baza yozuvidan bilinadi va kutish tugaydi).
STALL_100_S = int(os.environ.get("UPLOAD_STALL_100_SEC", "900"))
SEND_TRIES = int(os.environ.get("UPLOAD_TRIES", "5"))
COMMIT_WAIT = int(os.environ.get("COMMIT_WAIT_SEC", "300"))
SIBLING_WAIT = int(os.environ.get("SIBLING_WAIT_SEC", "300"))
T0 = time.time()


# ── Sessiyalar havzasi: asosiy + qo'shimcha (botdagi "🗂 Yuklovchi sessiyalar"). FloodWait chiqqan sessiya
# `COOL` bo'yicha dam oladi, yuklash keyingi tayyor sessiyaga o'tadi.
POOL: list = []
COOL: dict = {}
_rr = [0]


def pick_app(default=None, main_only=False):
    """Yuklovchi ASOSIY akkauntni ishlatmaydi: faqat qo'shimcha sessiyalar (POOL). FloodWait'dagilar dam oladi."""
    if not POOL:
        return default
    now = time.time()
    ready = [a for a in POOL if COOL.get(id(a), 0) <= now]
    if ready:
        a = ready[_rr[0] % len(ready)]
        _rr[0] += 1
        return a
    return min(POOL, key=lambda a: COOL.get(id(a), 0))


def flood(app, secs):
    COOL[id(app)] = max(COOL.get(id(app), 0), time.time() + int(secs) + 1)


def flood_sleep(app, secs):
    """FloodWait: bu sessiya dam oladi; boshqa tayyor sessiya bo'lsa kutilmaydi, bo'lmasa shuncha kutiladi."""
    flood(app, secs)
    alt = [a for a in POOL if a is not app and COOL.get(id(a), 0) <= time.time()]
    return 0 if alt else int(secs or 10)


class Status:
    """Botdagi "Holat" xabari uchun: yuklovchi holatini daqiqada bir marta worker'ga yuboradi (`/api/encode/push`, `X-Encode-Quality: upload`)."""

    def __init__(self):
        self.lines, self.data, self.progress, self.file = [], {}, "idle", ""
        self.last_push = 0.0

    def line(self, text):
        self.lines.append(text)
        del self.lines[:-14]

    def set(self, file="", progress="idle", **data):
        self.file, self.progress = file, progress
        self.data = {"file": file, **data}

    def xfer(self, kind, cur, total, t0):
        el = max(time.time() - t0, 0.1)
        sp = cur / 1048576 / el
        self.data["xfer"] = {"kind": kind, "pct": round(cur * 100 / max(total, 1), 1), "cur_mb": round(cur / 1048576, 1),
                             "total_mb": round(total / 1048576, 1), "mbps": round(sp, 2), "elapsed": int(el),
                             "eta": int((total - cur) / 1048576 / sp) if sp > 0 else -1}

    def push(self, force=False):
        if not force and time.time() - self.last_push < 55:
            return
        self.last_push = time.time()
        body = "\n".join(["#arustatus", f"run: {RUNNER}", "job: ", "num: ", f"progress: {self.progress}",
                          f"updated: {int(time.time())}", "data: " + json.dumps(self.data, separators=(",", ":"), ensure_ascii=False),
                          "---"] + self.lines)
        try:
            req = urllib.request.Request(
                f"{API}/api/encode/push", data=body.encode(), method="POST",
                headers={"X-Encode-Token": TOKEN, "X-Encode-Quality": "upload", "Content-Type": "text/plain; charset=utf-8",
                         "User-Agent": "arugram-upload"})
            urllib.request.urlopen(req, timeout=20).read()
        except Exception as ex:  # noqa: BLE001
            print(f"holat yuborilmadi: {ex}", flush=True)


ST = Status()


def log(*a):
    line = time.strftime("%H:%M:%S ") + " ".join(str(x) for x in a)
    print(line, flush=True)
    ST.line(line)
    ST.push()


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


def download(name: str, dst: Path, size: int, url: str | None = None):
    """R2'dan BITTA oqim (stream) bilan yuklab oladi; uzilsa qolgan joyidan davom ettiradi. `url` (imzolangan havola) bo'lsa
    R2'dan TO'G'RIDAN-TO'G'RI (worker'dan o'tmasdan, `Range`), aks holda worker orqali (`offset`)."""
    t0, last_log, got, err = time.time(), 0.0, 0, None
    with open(dst, "wb") as f:
        for attempt in range(1, 9):
            try:
                if url:
                    req = urllib.request.Request(url, headers={"Range": f"bytes={got}-", "User-Agent": "arugram-upload"})
                else:
                    req = urllib.request.Request(
                        f"{API}/api/r2/get?file={urllib.parse.quote(name)}&offset={got}",
                        headers={"X-Encode-Token": TOKEN, "User-Agent": "arugram-upload"})
                with urllib.request.urlopen(req, timeout=120) as r:
                    while True:
                        chunk = r.read(1 << 20)
                        if not chunk:
                            break
                        f.write(chunk)
                        got += len(chunk)
                        ST.xfer("R2'dan olish", got, size, t0)
                        if time.time() - last_log >= 5:
                            last_log = time.time()
                            log(f"    R2'dan yuklab olinmoqda {got * 100 // max(size, 1)}% "
                                f"({got / 1048576:.0f}/{size / 1048576:.0f} MB, {got / 1048576 / max(time.time() - t0, 0.1):.1f} MB/s)")
                if got >= size:
                    break
                err = RuntimeError(f"oqim erta tugadi ({got}/{size})")
            except Exception as ex:  # noqa: BLE001
                err = ex
            log(f"  R2'dan olish {attempt}/8 uzildi ({got // 1048576} MB da): {err} — davom ettiriladi")
            time.sleep(3 * attempt)
    if got != size:
        raise RuntimeError(f"R2'dan olib bo'lmadi: {err}")


# TELEGRAM'GA TEZ YUKLASH (foydalanuvchi talabi). Pyrogram katta faylni BITTA
# ulanishda (4 ta so'rov navbatda) yuboradi. Bu yerda bo'laklar (512 KB) `UP_CONN`
# ta alohida ulanish x `UP_WORKERS` ta so'rov bilan parallel yuboriladi. Har
# bo'lak xato/FloodWait bo'lsa qayta yuboriladi (Pyrogram'niki xatoni yutib
# yuborardi). Biror sabab bilan ishlamasa — Pyrogram'ning o'z usuli.
UP_CONN = int(os.environ.get("UP_CONN", "4"))
UP_WORKERS = int(os.environ.get("UP_WORKERS", "4"))
PART = 512 * 1024


async def fast_save_file(app, orig, path, progress=None, progress_args=(), file_id=None, file_part=0):
    import math
    from pyrogram import raw
    from pyrogram.errors import FloodWait
    from pyrogram.session import Session
    size = os.path.getsize(path) if isinstance(path, (str, Path)) else 0
    # Kichik fayl (rasm), qayta yuborish (`file_id`) yoki fayl yo'li emas — asl usul.
    if file_id is not None or size <= 10 * 1048576 or UP_CONN <= 1:
        return await orig(path, progress=progress, progress_args=progress_args, file_id=file_id, file_part=file_part)
    total = int(math.ceil(size / PART))
    fid = app.rnd_id()
    dc, key, test = await app.storage.dc_id(), await app.storage.auth_key(), await app.storage.test_mode()
    sessions = [Session(app, dc, key, test, is_media=True) for _ in range(UP_CONN)]
    queue = asyncio.Queue()
    for i in range(total):
        queue.put_nowait(i)
    sent = [0]
    gate = [0.0]  # FloodWait: hamma ishchi birga kutadi (har biri alohida urilmasin)

    async def worker(sess):
        with open(path, "rb") as f:
            while True:
                try:
                    i = queue.get_nowait()
                except asyncio.QueueEmpty:
                    return
                f.seek(i * PART)
                chunk = f.read(PART)
                for attempt in range(12):
                    wait = gate[0] - time.time()
                    if wait > 0:
                        await asyncio.sleep(wait)
                    try:
                        await sess.invoke(raw.functions.upload.SaveBigFilePart(
                            file_id=fid, file_part=i, file_total_parts=total, bytes=chunk))
                        break
                    except FloodWait as e:
                        flood(app, e.value)
                        until = time.time() + int(e.value) + 1
                        if until > gate[0] + 1:
                            log(f"Telegram FloodWait {e.value} s — hamma ulanish kutadi")
                        gate[0] = max(gate[0], until)
                    except Exception as e:  # noqa: BLE001
                        if attempt == 11:
                            raise RuntimeError(f"{i}-bo'lak Telegram'ga yuborilmadi: {e}")
                        await asyncio.sleep(2 * (attempt + 1))
                sent[0] += len(chunk)
                if progress:
                    progress(min(sent[0], size), size, *progress_args)

    started = []
    try:
        for sess in sessions:
            await sess.start()
            started.append(sess)
        log(f"Telegram'ga {UP_CONN} ulanish x {UP_WORKERS} so'rov bilan yuklanmoqda ({total} bo'lak)")
        await asyncio.gather(*[worker(sess) for sess in started for _ in range(UP_WORKERS)])
    finally:
        for sess in started:
            try:
                await sess.stop()
            except Exception:  # noqa: BLE001
                pass
    return raw.types.InputFileBig(id=fid, parts=total, name=os.path.basename(str(path)))


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
    base_app = app
    for attempt in range(1, SEND_TRIES + 1):
        app = pick_app(base_app)
        if len(POOL) > 1:
            log(f"  sessiya: {getattr(app, 'my_uid', '?')} ({POOL.index(app) + 1}/{len(POOL)})")
        prog = {"p": -1, "t": time.time(), "log": 0.0, "t0": time.time()}

        def cb(cur, total, prog=prog):
            if cur != prog["p"]:
                prog["p"], prog["t"] = cur, time.time()
            ST.xfer("Telegram'ga yuklash", cur, total, prog["t0"])
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
            at_end = prog["p"] >= size
            if at_end and await asyncio.to_thread(is_committed, name):
                # Fayl 100% yuborilgan, bot uni allaqachon saqlab bazaga yozgan — Telegram javobi kechikkan, xolos.
                log("  bot faylni saqladi (Telegram javobi kechikdi) — kutish tugadi")
                task.cancel()
                try:
                    await task
                except BaseException:  # noqa: BLE001
                    pass
                return
            if time.time() - prog["t"] > (STALL_100_S if at_end else STALL_S):
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
                    await asyncio.sleep(flood_sleep(app, getattr(ex, "value", 10)))
        else:
            if prog["p"] >= size:
                flood(app, 120)     # 100% da qotdi — shu sessiya bir oz dam olsin
            pct = prog["p"] * 100 // max(size, 1)
            last_err = f"{STALL_100_S if prog['p'] >= size else STALL_S} soniya qimirlamadi ({pct}%)"
        log(f"  yuklash {attempt}/{SEND_TRIES} muvaffaqiyatsiz — {last_err}")
        if await asyncio.to_thread(is_committed, name):
            log("  fayl allaqachon saqlangan — qayta yuklanmaydi")
            return
        await asyncio.sleep(5 * attempt)
    raise RuntimeError(f"Telegram'ga yuklab bo'lmadi ({SEND_TRIES} urinish): {last_err}")


def _subst(v, msg_id: int):
    """Qaytarish tanasidagi "$MSG_ID" qiymatlarini yuborilgan xabar raqami bilan almashtiradi."""
    if isinstance(v, dict):
        return {k: _subst(x, msg_id) for k, x in v.items()}
    if isinstance(v, list):
        return [_subst(x, msg_id) for x in v]
    return msg_id if v == "$MSG_ID" else v


async def send_via_panel_bot(job: dict, dest: dict, path: Path, name: str):
    """Zaxira (Post): panel bot foydalanuvchiga va adminga yuboradi (MTProto bot sessiyasi — 50 MB cheklovsiz)."""
    tok = job.get("bot_token") or ""
    if not tok:
        raise RuntimeError("panel bot tokeni berilmadi")
    api_id = session_api_id() or int(os.environ["TG_API_ID"])
    bot = Client("panel_fallback", api_id=api_id, api_hash=os.environ["TG_API_HASH"], bot_token=tok,
                 in_memory=True, no_updates=True)
    to, admin = int(dest["to"]), int(job.get("admin") or 0)
    kw = {k: dest[k] for k in ("duration", "width", "height") if dest.get(k)}
    msg = None
    async with bot:
        for chat in dict.fromkeys([to] + ([admin] if admin else [])):
            try:
                m = await bot.send_video(chat, str(path), caption=dest.get("caption", ""),
                                         file_name=dest.get("file_name") or name,
                                         supports_streaming=bool(dest.get("supports_streaming")), **kw)
                if chat == to:
                    msg = m
                log(f"  panel bot: {chat} ga yuborildi")
            except Exception as ex:  # noqa: BLE001
                log(f"  panel bot {chat} ga yubora olmadi: {type(ex).__name__}: {ex}")
                if chat == to:
                    raise
    return msg


async def send_dest(app, dest: dict, path: Path, name: str):
    """Post/Anibla/To'plam faylini `dest` bo'yicha yuboradi (qotib qolsa bekor qilib qayta yuboradi). Qaytadi: xabar."""
    import base64
    to = int(dest["to"])
    size = path.stat().st_size
    thumb = None
    if dest.get("thumb_b64"):
        thumb = WORK / "thumb.jpg"
        thumb.write_bytes(base64.b64decode(dest["thumb_b64"]))
    last_err = ""
    base_app = app
    for attempt in range(1, SEND_TRIES + 1):
        app = pick_app(base_app)
        prog = {"p": -1, "t": time.time(), "log": 0.0, "t0": time.time()}

        def cb(cur, total, prog=prog):
            if cur != prog["p"]:
                prog["p"], prog["t"] = cur, time.time()
            ST.xfer("Telegram'ga yuklash", cur, total, prog["t0"])
            if time.time() - prog["log"] >= 5:
                prog["log"] = time.time()
                log(f"    Telegram'ga yuklanmoqda {cur * 100 // max(total, 1)}% ({cur / 1048576:.0f}/{total / 1048576:.0f} MB)")

        if dest.get("mode") == "video":
            kw = {k: dest[k] for k in ("duration", "width", "height") if dest.get(k)}
            coro = app.send_video(to, str(path), caption=dest.get("caption", ""), file_name=dest.get("file_name") or name,
                                  supports_streaming=bool(dest.get("supports_streaming")),
                                  disable_notification=bool(dest.get("disable_notification")),
                                  thumb=str(thumb) if thumb else None, progress=cb, **kw)
        else:
            coro = app.send_document(to, str(path), file_name=dest.get("file_name") or name, force_document=True,
                                     caption=dest.get("caption", ""), disable_notification=bool(dest.get("disable_notification")),
                                     progress=cb)
        task = asyncio.create_task(coro)
        stalled = False
        while True:
            done, _ = await asyncio.wait({task}, timeout=10)
            if done:
                break
            if time.time() - prog["t"] > (STALL_100_S if prog["p"] >= size else STALL_S):
                stalled = True
                task.cancel()
                try:
                    await task
                except BaseException:  # noqa: BLE001
                    pass
                break
        if not stalled:
            try:
                return task.result()
            except Exception as ex:  # noqa: BLE001
                last_err = f"{type(ex).__name__}: {ex}"
                if type(ex).__name__ == "FloodWait":
                    await asyncio.sleep(flood_sleep(app, getattr(ex, "value", 10)))
        else:
            last_err = f"{STALL_S} soniya qimirlamadi ({prog['p'] * 100 // max(size, 1)}%)"
        log(f"  yuklash {attempt}/{SEND_TRIES} muvaffaqiyatsiz — {last_err}")
        await asyncio.sleep(5 * attempt)
    raise RuntimeError(f"Telegram'ga yuklab bo'lmadi ({SEND_TRIES} urinish): {last_err}")


def callback(cb: dict, msg_id: int):
    """Yuborilgach egasi (Post/Anibla/To'plam) yo'lini chaqiradi. `False` — ish yo'qolgan (409)."""
    data = json.dumps(_subst(cb.get("body") or {}, msg_id)).encode()
    req = urllib.request.Request(
        f"{API}{cb['path']}", data=data, method="POST",
        headers={"X-Encode-Token": TOKEN, "Content-Type": "application/json", "User-Agent": "arugram-upload"})
    for attempt in range(4):
        try:
            with urllib.request.urlopen(req, timeout=60) as r:
                r.read()
            return True
        except urllib.error.HTTPError as e:
            if e.code == 409:
                return False
            if e.code in (400, 401, 403, 404):
                raise RuntimeError(f"{cb['path']}: HTTP {e.code} {e.read()[:200]!r}")
        except Exception as ex:  # noqa: BLE001
            log(f"  qaytarish xato: {ex}")
        time.sleep(3 * (attempt + 1))
    raise RuntimeError(f"{cb['path']} ishlamadi")


async def one(app, job: dict):
    if job.get("kind") == "deliver":
        return await one_deliver(app, job)
    name, size = job["file"], int(job["size"])
    path = WORK / name
    shutil.rmtree(WORK, ignore_errors=True)
    WORK.mkdir(parents=True)
    beat = asyncio.create_task(beat_loop(name))
    try:
        ST.set(name, "download", kind=job.get("kind"), attempt=job.get("attempt"), job_started=int(time.time()))
        log(f"{name}: R2'dan olinmoqda ({size / 1048576:.1f} MB), urinish {job.get('attempt')}")
        ST.push(True)
        await asyncio.to_thread(download, name, path, size, job.get("url"))
        ST.progress = "upload"
        ST.data.pop("xfer", None)
        log(f"{name}: botga yuborilmoqda")
        ST.push(True)
        await send_to_bot(app, job["bot"], path, name, job["caption"])
        end = time.time() + COMMIT_WAIT
        while time.time() < end and not await asyncio.to_thread(is_committed, name):
            await asyncio.sleep(3)
        if not await asyncio.to_thread(is_committed, name):
            raise RuntimeError(f"bot {COMMIT_WAIT} soniyada faylni saqlamadi (bot kanalda adminmi? sessiya ro'yxatdan o'tganmi?)")
        r = await asyncio.to_thread(api, "done", {"file": name})
        ST.data.pop("xfer", None)
        log(f"{name}: bazaga yozildi, R2'dan o'chirildi ({r.get('deleted')})")
    finally:
        beat.cancel()
        shutil.rmtree(WORK, ignore_errors=True)


async def one_deliver(app, job: dict):
    """Post/Anibla/To'plam fayli: R2'dan olinadi, `dest` bo'yicha yuboriladi, qaytarish yo'li chaqiriladi, R2'dan o'chadi."""
    name, size, dest = job["file"], int(job["size"]), job.get("dest") or {}
    path = WORK / name
    shutil.rmtree(WORK, ignore_errors=True)
    WORK.mkdir(parents=True)
    beat = asyncio.create_task(beat_loop(name))
    try:
        ST.set(name, "download", kind="deliver", attempt=job.get("attempt"), job_started=int(time.time()))
        log(f"{name}: R2'dan olinmoqda ({size / 1048576:.1f} MB), urinish {job.get('attempt')}")
        ST.push(True)
        await asyncio.to_thread(download, name, path, size, job.get("url"))
        ST.progress = "upload"
        ST.data.pop("xfer", None)
        log(f"{name}: {dest.get('to')} ga yuborilmoqda")
        ST.push(True)
        if dest.get("fallback_bot"):
            # Post: asosiy akkaunt yubora olmagan. Avval panel bot (userga va adminga), u ham bo'lmasa qo'shimcha sessiyalar.
            try:
                msg = await send_via_panel_bot(job, dest, path, name)
            except Exception as ex:  # noqa: BLE001
                log(f"  panel bot yubora olmadi ({type(ex).__name__}: {ex}) — qo'shimcha sessiya bilan urinadi")
                msg = await send_dest(app, dest, path, name)
        else:
            msg = await send_dest(app, dest, path, name)
        cb = dest.get("callback")
        if cb:
            if not await asyncio.to_thread(callback, cb, int(msg.id)):
                log(f"  {name}: ish yo'qolgan (409) — yuborilgan xabar o'chiriladi" if cb.get("on_lost_delete") else f"  {name}: ish yo'qolgan (409)")
                if cb.get("on_lost_delete"):
                    try:
                        await app.delete_messages(int(dest["to"]), msg.id)
                    except Exception as ex:  # noqa: BLE001
                        log(f"  xabar o'chirilmadi: {ex}")
        await asyncio.to_thread(api, "done", {"file": name})
        ST.data.pop("xfer", None)
        log(f"{name}: yetkazildi, R2'dan o'chirildi")
    finally:
        beat.cancel()
        shutil.rmtree(WORK, ignore_errors=True)


def _patch(app):
    orig_save = app.save_file

    async def patched_save(path, progress=None, progress_args=(), file_id=None, file_part=0):
        return await fast_save_file(app, orig_save, path, progress, progress_args, file_id, file_part)

    app.save_file = patched_save


def api_id_of(session_base: str) -> int:
    try:
        c = sqlite3.connect(f"file:{session_base}.session?mode=ro", uri=True)
        return int(c.execute("SELECT api_id FROM sessions").fetchone()[0] or 0)
    except Exception:  # noqa: BLE001
        return 0


async def main():
    # Yuklovchi ASOSIY akkauntni (session.enc) ishlatmaydi — faqat botdagi "🗂 Yuklovchi sessiyalar" (tool/encode/pool/*.enc).
    files = sorted(Path(SESSION).parent.glob("pool_*.session"))
    clients = []
    for f in files:
        base = str(f.with_suffix(""))
        c = Client(base, api_id=api_id_of(base) or int(os.environ["TG_API_ID"]),
                   api_hash=os.environ["TG_API_HASH"], no_updates=True)
        _patch(c)
        clients.append(c)
    if not clients:
        log("XATO: qo'shimcha sessiya yo'q. Yuklovchi asosiy akkauntni ishlatmaydi — panel botda "
            "\u00AB\U0001F5C2 Yuklovchi sessiyalar\u00BB orqali kamida bitta akkaunt qo'shing.")
        ST.set("", "idle")
        ST.push(True)
        return
    started = []
    try:
        for c in clients:
            try:
                await c.start()
                c.my_uid = (await c.get_me()).id
                async for _ in c.get_dialogs():
                    pass
                POOL.append(c)
                started.append(c)
            except Exception as ex:  # noqa: BLE001
                log(f"sessiya ochilmadi ({c.name}): {ex}")
        if not POOL:
            log("XATO: birorta ham sessiya ochilmadi")
            return
        log(f"Sessiyalar: {len(POOL)} ta (asosiy akkaunt ishlatilmaydi)")
        await _main(POOL[0])
    finally:
        for c in started:
            try:
                await c.stop()
            except Exception:  # noqa: BLE001
                pass


async def _main(app):
    if True:
        me = await app.get_me()
        started_bot = False
        worked = 0
        group = ""      # `a/s/e/queued_at/sifat` — bitta sifatning MP4 va fMP4 fayllari
        seen_kinds = set()
        sib_wait = 0
        while time.time() - T0 < START_BUDGET:
            job = api("claim", {"runner": RUNNER, "uid": me.id, "uids": [c.my_uid for c in POOL],
                                **({"group": group} if group else {})})
            if group and (job.get("none") or not job.get("file")):
                # Shu sifatning ikkinchi fayli (MP4 <-> fMP4) hali R2'ga yetib kelmagan bo'lishi mumkin — qisqa kutiladi.
                if len(seen_kinds) < 2 and sib_wait < SIBLING_WAIT:
                    await asyncio.sleep(20)
                    sib_wait += 20
                    continue
                log(f"Sifat tugadi ({group}) — run o'chadi, keyingisi yangi run'da ochiladi.")
                break
            if job.get("none") or not job.get("file"):
                log("Navbat bo'sh.")
                break
            if job.get("kind") == "deliver":
                group, seen_kinds = "", set()      # Post/Anibla/To'plam: har fayl alohida, juftlik kutilmaydi
            else:
                group = job.get("group") or group
                seen_kinds.add(job.get("kind") or "")
            if not started_bot and job.get("bot"):
                # Bot shu akkauntdan fayl qabul qilishi uchun akkaunt botni bir marta ochgan bo'lsin.
                started_bot = True
                for c in POOL:
                    try:
                        await c.send_message(str(job["bot"]).lstrip("@"), "/start")
                    except Exception as ex:  # noqa: BLE001
                        log(f"botga /start yuborilmadi ({getattr(c, 'my_uid', '?')}): {ex}")
            try:
                await one(app, job)
                worked += 1
            except Exception as ex:  # noqa: BLE001
                log(f"XATO: {job.get('file')}: {ex}")
                try:
                    api("fail", {"file": job["file"], "error": str(ex)[:250]})
                except Exception as e2:  # noqa: BLE001
                    log(f"fail yozilmadi: {e2}")
                await asyncio.sleep(15)
        log(f"Run tugadi. Yuklandi: {worked}")
        ST.set("", "idle")
        ST.push(True)
        out = os.environ.get("GITHUB_OUTPUT")
        if out:
            with open(out, "a") as fh:
                fh.write(f"worked={'1' if worked else '0'}\n")


if __name__ == "__main__":
    try:
        asyncio.run(main())
    except Exception as e:  # noqa: BLE001
        log("XATO:", e)
        sys.exit(1)
