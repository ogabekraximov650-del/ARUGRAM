#!/usr/bin/env python3
"""ARUDRIVE qabul boti: botga forward qilingan video/fayl -> R2 -> Telegram kanali (worker saqlaydi).

Ikkinchi akkauntdagi repoda `tgbot.yml` (cron har 5 daqiqada, bir vaqtda BITTA run) ishga tushiradi; run 30 daqiqa o'chmay turadi
(xabar bo'lsa ham, bo'lmasa ham) va o'zi to'xtaydi (Actions to'xtatilsa — SIGTERM — ham tinchgina chiqadi).

Qanday ishlaydi:
  * Foydalanuvchi videoni bot chatiga (`@arudrivebot`) forward qiladi. Sessiya akkaunti (MTProto, pyrogram) shu chatni tinglaydi va
    faylni to'g'ridan-to'g'ri yuklab oladi — Bot API ning 20 MB chegarasi YO'Q (2 GB gacha).
  * Fayl worker bergan vaqtinchalik havola bilan R2 ga BUTUN holda yuboriladi (`/api/bd/presign`; 5 GiB dan kattasi 1 GiB qismlar),
    kichik rasm qo'shiladi (`/api/bd/thumb`), so'ng `/api/bd/complete` (yoki `/api/bd/finish`) — Telegram'ga saqlashni worker o'zi qiladi.
  * Natija bot nomidan chatga yozib turiladi. Oxirgi xabarda `⟦<xabar raqami>⟧` belgisi bo'ladi: run yangidan boshlanganda
    chat tarixidan belgisiz (ya'ni hali qayta ishlanmagan) fayllar topilib ishlanadi — bo'sh vaqtda forward qilinganlar yo'qolmaydi.
"""
import asyncio
import json
import os
import re
import shutil
import signal
import sqlite3
import subprocess
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path

from pyrogram import Client, filters
from pyrogram.handlers import MessageHandler

API = os.environ.get("BD_API", "https://arudrive.uzcom.workers.dev").rstrip("/")
ENCODE_TOKEN = os.environ.get("ENCODE_TOKEN", "").strip()
BOT_TOKEN = os.environ.get("BOT_TOKEN", "").strip()
RUN_MIN = int(os.environ.get("RUN_MIN", "30"))
HISTORY_HOURS = int(os.environ.get("HISTORY_HOURS", "48"))
WORK = Path(os.environ.get("WORK_DIR", "/tmp/tgbot"))
UA = "arudrive-tgbot"
T0 = time.time()
DEADLINE = T0 + RUN_MIN * 60
MARK = "⟦{}⟧"
MARK_RE = re.compile(r"⟦(\d+)⟧")

stop = False


def log(*a):
    print(time.strftime("%H:%M:%S"), *a, flush=True)


def human(n: int) -> str:
    return f"{n / 1048576:.1f} MB" if n < 1 << 30 else f"{n / (1 << 30):.2f} GB"


# ── HTTP (worker, Bot API, R2) ─────────────────────────────────────────────

def worker(path, body=None, raw=None, ctype="application/json", timeout=120):
    data = raw if raw is not None else (json.dumps(body).encode() if body is not None else None)
    req = urllib.request.Request(
        f"{API}{path}", data=data, method="POST",
        headers={"X-Encode-Token": ENCODE_TOKEN, "Content-Type": ctype, "User-Agent": UA})
    try:
        with urllib.request.urlopen(req, timeout=timeout) as r:
            return json.loads(r.read() or b"{}")
    except urllib.error.HTTPError as e:
        raise RuntimeError(f"worker {path}: HTTP {e.code} {e.read()[:160].decode(errors='replace')}")


def bot_api(method, **params):
    req = urllib.request.Request(
        f"https://api.telegram.org/bot{BOT_TOKEN}/{method}", data=json.dumps(params).encode(), method="POST",
        headers={"Content-Type": "application/json", "User-Agent": UA})
    try:
        with urllib.request.urlopen(req, timeout=60) as r:
            return json.loads(r.read())
    except urllib.error.HTTPError as e:
        return {"ok": False, "description": e.read()[:200].decode(errors="replace")}
    except Exception as e:  # noqa: BLE001
        return {"ok": False, "description": str(e)}


class Slice:
    """Fayldan `length` bayt o'qiydi (oqim, xotiraga yig'ilmaydi); `on_bytes(n)` — har o'qilgan bayt uchun."""

    def __init__(self, f, length, on_bytes=None):
        self.f, self.left, self.on = f, length, on_bytes

    def read(self, n=-1):
        if self.left <= 0:
            return b""
        n = self.left if n is None or n < 0 else min(n, self.left)
        b = self.f.read(n)
        self.left -= len(b)
        if self.on and b:
            self.on(len(b))
        return b


def put_direct(url, path, offset, length, on_bytes=None):
    """Imzolangan R2 havolasiga fayl qismini PUT qiladi (8 marta urinadi); qaytadi: ETag. `on_bytes(n)` — yuborilgan bayt."""
    last = None
    for attempt in range(8):
        sent = [0]

        def cb(n):
            sent[0] += n
            if on_bytes:
                on_bytes(n)

        try:
            with open(path, "rb") as f:
                f.seek(offset)
                req = urllib.request.Request(
                    url, data=Slice(f, length, cb), method="PUT",
                    headers={"Content-Length": str(length), "User-Agent": UA})
                with urllib.request.urlopen(req, timeout=3600) as r:
                    return r.headers.get("ETag", "")
        except Exception as e:  # noqa: BLE001
            last = e
            log(f"R2 PUT xato ({attempt + 1}/8): {e}")
            if on_bytes and sent[0]:
                on_bytes(-sent[0])  # urinish qaytadan boshlanadi — foiz ham orqaga
            time.sleep(min(2 * (attempt + 1), 15))
    raise RuntimeError(f"R2 ga yuborilmadi: {last}")


def upload(path, name, kind, mime, w, h, dur_ms, thumb, on_bytes=None):
    """Faylni worker orqali R2 ga yuboradi va Telegram'ga saqlashni boshlatadi. Qaytadi: fayl id."""
    size = os.path.getsize(path)
    pre = worker("/api/bd/presign", {"name": name, "size": size, "kind": kind, "mime": mime, "w": w, "h": h, "dur": dur_ms})
    fid = pre["id"]
    parts = []
    if pre["mode"] == "single":
        put_direct(pre["urls"][0], path, 0, size, on_bytes)
    else:
        ps = int(pre["part_size"])
        for i, url in enumerate(pre["urls"]):
            off = i * ps
            etag = put_direct(url, path, off, min(ps, size - off), on_bytes)
            parts.append({"n": i + 1, "etag": etag})
    if thumb:
        worker(f"/api/bd/thumb?id={urllib.parse.quote(fid)}", raw=thumb, ctype="image/jpeg")
    if pre["mode"] == "single":
        worker("/api/bd/complete", {"id": fid})
    else:
        worker("/api/bd/finish", {"id": fid, "upload_id": pre["upload_id"], "parts": parts}, timeout=300)
    return fid


# ── Telegram (sessiya) ─────────────────────────────────────────────────────

def api_id_of(session: str) -> int:
    try:
        c = sqlite3.connect(f"file:{session}.session?mode=ro", uri=True)
        return int(c.execute("SELECT api_id FROM sessions").fetchone()[0] or 0)
    except Exception:  # noqa: BLE001
        return 0


GREETING = ("👋 ARUDRIVE qabul boti ishlayapti.\n"
            "Video yoki faylni shu chatga forward qiling — yuklab olib R2 ga yuboraman, so'ng Telegram kanaliga saqlanadi.\n"
            "Holat: /holat")


def is_cmd(m, *names):
    t = (m.text or "").strip().lower().split("@")[0]
    return bool(m.outgoing and t in names)


def media_of(m):
    return m.video or m.animation or m.video_note or m.document or m.audio


def make_thumb(path):
    """Telegram kichik rasmi bo'lmasa — ffmpeg bilan 1-soniyadagi kadr (bo'lmasa None)."""
    if not shutil.which("ffmpeg"):
        return None
    out = Path(path).with_suffix(".thumb.jpg")
    try:
        subprocess.run(["ffmpeg", "-y", "-loglevel", "error", "-ss", "1", "-i", str(path), "-frames:v", "1",
                        "-vf", "scale='min(720,iw)':-2", "-q:v", "4", str(out)], timeout=120, check=True)
        data = out.read_bytes()
        return data if 0 < len(data) <= 1024 * 1024 else None
    except Exception as e:  # noqa: BLE001
        log("ffmpeg kichik rasm olinmadi:", e)
        return None
    finally:
        out.unlink(missing_ok=True)


class Status:
    """Bitta fayl uchun BITTA xabar: foiz o'sib borgan sari o'sha xabar tahrirlanadi (har ~3 soniyada, Telegram chegarasi uchun)."""

    def __init__(self, chat_id):
        self.chat, self.mid, self.last, self.text = chat_id, None, 0.0, ""

    def set(self, text, force=False):
        now = time.time()
        if text == self.text or (not force and now - self.last < 3):
            return
        if self.mid is None:
            r = bot_api("sendMessage", chat_id=self.chat, text=text, disable_notification=True)
            if r.get("ok"):
                self.mid = r["result"]["message_id"]
        else:
            r = bot_api("editMessageText", chat_id=self.chat, message_id=self.mid, text=text)
            if not r.get("ok") and "not modified" not in str(r.get("description", "")):
                log("xabar tahrirlanmadi:", r.get("description"))
        self.text, self.last = text, now


def bar(pct: int) -> str:
    n = max(0, min(10, pct // 10))
    return "▰" * n + "▱" * (10 - n)


class Prog:
    """Bosqich (yuklab olish / R2 ga yuborish) foizi va tezligi."""

    def __init__(self, status, title, name, total):
        self.st, self.title, self.name, self.total = status, title, name, max(total, 1)
        self.done, self.t0 = 0, time.time()

    def set(self, done, force=False):
        self.done = max(0, done)
        pct = min(100, int(self.done * 100 / self.total))
        sp = self.done / max(time.time() - self.t0, 0.001)
        self.st.set(f"{self.title}\n{self.name}\n{bar(pct)} {pct}%  ({human(self.done)} / {human(self.total)}, {human(int(sp))}/s)", force)

    def add(self, n):
        self.set(self.done + n)


async def handle(app, m, chat_id):
    media = media_of(m)
    safe = re.sub(r"[^\w.\- ]", "_", getattr(media, "file_name", None) or f"video_{m.id}.mp4")[:120]
    size = int(getattr(media, "file_size", 0) or 0)
    mime = getattr(media, "mime_type", "") or ""
    is_video = bool(m.video or m.animation or m.video_note or mime.startswith("video/"))
    w, h = int(getattr(media, "width", 0) or 0), int(getattr(media, "height", 0) or 0)
    dur_ms = int(getattr(media, "duration", 0) or 0) * 1000
    status = Status(chat_id)
    dl = Prog(status, "⬇️ Telegram'dan yuklab olinmoqda", safe, size)
    dl.set(0, force=True)
    path = WORK / f"{m.id}_{safe}"
    try:
        got = await app.download_media(m, file_name=str(path), progress=lambda cur, tot: dl.set(cur))
        if not got:
            raise RuntimeError("Telegram'dan yuklab bo'lmadi")
        path = Path(got)
        thumb = None
        if is_video:
            for t in sorted(getattr(media, "thumbs", None) or [], key=lambda t: getattr(t, "file_size", 0) or 0, reverse=True):
                tp = await app.download_media(t.file_id, file_name=str(WORK / f"{m.id}_t.jpg"))
                if tp:
                    b = Path(tp).read_bytes()
                    Path(tp).unlink(missing_ok=True)
                    if 0 < len(b) <= 1024 * 1024:
                        thumb = b
                        break
            if thumb is None:
                thumb = await asyncio.to_thread(make_thumb, path)
        real = path.stat().st_size
        up = Prog(status, "⬆️ R2 ga yuborilmoqda", safe, real)
        up.set(0, force=True)
        fid = await asyncio.to_thread(upload, path, safe, "video" if is_video else "file", mime or "application/octet-stream", w, h, dur_ms, thumb, up.add)
        status.set(f"✅ Qabul qilindi: {safe} ({human(real)})\nTelegram kanaliga saqlanmoqda (id {fid}). {MARK.format(m.id)}", force=True)
    except Exception as e:  # noqa: BLE001
        log("XATO:", e)
        status.set(f"❌ {safe}: {str(e)[:300]}\nQayta forward qiling. {MARK.format(m.id)}", force=True)
    finally:
        path.unlink(missing_ok=True)


async def main():
    global stop
    if not (ENCODE_TOKEN and BOT_TOKEN):
        sys.exit("ENCODE_TOKEN yoki BOT_TOKEN yo'q")
    WORK.mkdir(parents=True, exist_ok=True)
    session = os.environ["SESSION"]
    api_id = api_id_of(session) or int(os.environ["TG_API_ID"])
    app = Client(session, api_id=api_id, api_hash=os.environ["TG_API_HASH"])
    loop = asyncio.get_running_loop()
    for sig in (signal.SIGTERM, signal.SIGINT):
        loop.add_signal_handler(sig, lambda: globals().__setitem__("stop", True))

    me_bot = bot_api("getMe")
    if not me_bot.get("ok"):
        sys.exit(f"bot tokeni ishlamadi: {me_bot.get('description')}")
    bot_id, bot_user = me_bot["result"]["id"], me_bot["result"]["username"]

    await app.start()
    me = await app.get_me()
    log(f"Sessiya: {me.id} ({me.first_name}); bot: @{bot_user}; ish vaqti {RUN_MIN} daqiqa")
    await app.get_users(bot_user)  # bot chatini (peer) aniqlaydi

    def notify(text):
        r = bot_api("sendMessage", chat_id=me.id, text=text, disable_notification=True)
        if not r.get("ok"):
            log("bot xabar yubormadi:", r.get("description"))

    queue: asyncio.Queue = asyncio.Queue()
    queued = set()

    def want(m):
        return bool(m.outgoing and media_of(m) and m.id not in queued)

    # Run yangidan boshlangan: bo'sh vaqtda forward qilinib hali ishlanmaganlar (oxirgi xabarda `⟦id⟧` belgisi yo'q).
    done, pending = set(), []
    last_bot_ts = last_start_ts = 0.0
    cutoff = time.time() - HISTORY_HOURS * 3600
    async for m in app.get_chat_history(bot_id, limit=200):
        ts = m.date.timestamp() if m.date else 0.0
        if ts < cutoff:
            break
        if m.outgoing is False and m.text:
            done.update(int(x) for x in MARK_RE.findall(m.text))
            last_bot_ts = max(last_bot_ts, ts)
        elif is_cmd(m, "/start", "/holat"):
            last_start_ts = max(last_start_ts, ts)
        elif want(m):
            pending.append(m)
    if last_start_ts > last_bot_ts:  # bot o'chiq paytda yozilgan /start javobsiz qolgan
        notify(GREETING)
    for m in reversed(pending):
        if m.id not in done:
            queued.add(m.id)
            await queue.put(m)
    log(f"Navbatda (o'tgan vaqtdan): {queue.qsize()}")

    async def on_message(_, m):
        if is_cmd(m, "/start"):
            notify(GREETING)
        elif is_cmd(m, "/holat"):
            left = max(0, int((DEADLINE - time.time()) / 60))
            notify(f"✅ Bot ishlayapti. Navbatda: {queue.qsize()} ta fayl. Bu run yana ~{left} daqiqa o'chmay turadi.")
        elif want(m):
            queued.add(m.id)
            await queue.put(m)
            log("Yangi fayl:", m.id)

    app.add_handler(MessageHandler(on_message, filters.chat(bot_id)))

    async def worker_loop():
        while True:
            m = await queue.get()
            try:
                if time.time() > DEADLINE:
                    continue  # muddat tugadi: yangi fayl olinmaydi, belgisiz qoladi — keyingi run boshida chat tarixidan olinadi
                await handle(app, m, me.id)  # BIR VAQTDA BITTA fayl (navbat ketma-ket)
            finally:
                queue.task_done()

    task = asyncio.create_task(worker_loop())
    while not stop and time.time() < DEADLINE:
        await asyncio.sleep(1)
    log("To'xtatilmoqda: yangi ish olinmaydi, joriy ish tugashi kutiladi")
    grace = time.time() + 1500  # boshlangan fayl tugashiga ruxsat (yangisi olinmaydi)
    while (queue.qsize() or not queue.empty() or getattr(queue, "_unfinished_tasks", 0)) and time.time() < grace and not stop:
        await asyncio.sleep(1)
    task.cancel()
    await app.stop()
    log("Tugadi")


if __name__ == "__main__":
    asyncio.run(main())
