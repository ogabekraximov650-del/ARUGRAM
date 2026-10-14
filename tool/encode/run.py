#!/usr/bin/env python3
"""ARUGRAM avto-kodlash (GitHub Actions, `.github/workflows/encode.yml`).

Navbat worker'da (`worker/src/lib.rs` -> `encode_route`). Bu skript:

  1. `claim` — navbatdagi ENG ESKI ishni oladi (bir vaqtda faqat bittasi);
  2. asl videoni yopiq kanaldan yuklab oladi va kaliti bilan ochadi
     (AES-128-CTR, IV nol — ilovadagi `rust/src/telegram.rs` bilan bir xil);
  3. manba sifatiga qarab (upscale YO'Q) har sifatni ALOHIDA H.265 bilan
     kodlaydi — `anime` repodagi `encode_h265.sh` sozlamalari:
     CRF (1080p BASE, 720p BASE-1, 480p BASE-2, 360p BASE-3), `hvc1`,
     `+faststart`, AAC stereo;
  4. har sifatni yangi tasodifiy kalit bilan shifrlab kanalga yuklaydi
     (izoh: `<fayl nomi>\\nkey:<hex>` — bot uni `tg_files` ga o'zi yozadi)
     va `quality` bilan jurnalga (`epizod_db`) yozadi;
  5. `finish`.

SIFAT-WORKFLOW REJIMI (`ENCODE_QUALITY=360p|480p|720p|1080p`, 2026-10): 4 ta alohida workflow
(`encode_360.yml` ...) BIR VAQTDA ishlaydi, har biri FAQAT o'z sifatini tayyorlaydi va navbatdagi
barcha qismlarni to'xtamasdan yuradi. Har run asl videoni O'ZI yuklab, haqiqiy balandligini
o'lchaydi; sifat qismga to'g'ri kelmasa (upscale yo'q) worker'ga `skip` deb xabar beradi — worker
buni bazaga yozib, keyingi qismni beradi. 4 sifatning hammasi hal bo'lgach (tayyor yoki `skip`)
worker asl videoni o'chiradi. `ENCODE_QUALITY` berilmasa — eski usul (hamma sifat ketma-ket).

XAVFSIZLIK:
  * tayyor sifat (`done`) QAYTA kodlanmaydi; yuklangan-u jurnalga yozilmay
    qolgani (`uploaded`) faqat jurnalga yoziladi;
  * kodlangan faylning davomiyligi manbaga mos kelmasa — yuklanmaydi;
  * `heartbeat` YO'Q (2026-10): ish olinganda ijara butun run'ga beriladi
    (`no_heartbeat`), o'lgan run'ni worker GitHub'dan tekshirib bo'shatadi.
    Ish boshqa run'ga o'tgan bo'lsa (`quality`/`finish` 409) — to'xtaydi;
  * jonli holat bazaga YOZILMAYDI: log kanalidagi bitta QADALGAN xabar
    (`#arustatus`) har ~15 soniyada tahrirlanadi, worker uni faqat admin
    so'raganda o'qiydi (`StatusPin`);
  * vaqt limiti yaqin bo'lsa yangi ish olinmaydi (qolgani keyingi run'da).
"""

import asyncio
import collections
import html
import json
import signal
import os
import secrets
import shutil
import subprocess
import sys
import threading
import time
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path

import pyrogram.utils
from cryptography.hazmat.primitives.ciphers import Cipher, algorithms, modes
from pyrogram import Client, enums

# Pyrogram 2.0.106: yangi kanallarning raqami eski chegaradan kichik —
# aks holda "Peer id invalid" (ma'lum xato, shu yamoq bilan tuzaladi).
pyrogram.utils.MIN_CHANNEL_ID = -1009999999999

API = os.environ["API_BASE"].rstrip("/")
TOKEN = os.environ["ENCODE_TOKEN"]
RUNNER = f"{os.environ.get('GITHUB_RUN_ID', 'local')}-{os.environ.get('GITHUB_RUN_ATTEMPT', '1')}"
CRF_BASE = int(os.environ.get("H265_CRF", "30"))
PRESET = os.environ.get("H265_PRESET", "medium")
# Qo'shimcha x265 sozlamalari (ixtiyoriy, `:` bilan, masalan `frame-threads=2`).
# Preset o'zgarmaydi.
X265_EXTRA = os.environ.get("H265_X265_EXTRA", "").strip(":")
# Shu vaqtdan keyin YANGI ish olinmaydi (Actions limiti 6 soat).
START_BUDGET = int(os.environ.get("START_BUDGET_MIN", "240")) * 60
# Har run BITTA sifat tayyorlaydi (1080p, keyin 720p...): sifat yuklangach ish navbatga
# qaytariladi va workflow keyingi sifat uchun YANGI run ochadi (foydalanuvchi talabi).
# "0" — eski tartib (bitta run hamma sifatni ketma-ket kodlaydi).
ONE_QUALITY_PER_RUN = os.environ.get("ONE_QUALITY_PER_RUN", "1") != "0"
# Sifat-workflow rejimi: shu run faqat shu sifatni tayyorlaydi (bo'sh — eski usul).
QUALITY = os.environ.get("ENCODE_QUALITY", "").strip()
if QUALITY not in ("", "360p", "480p", "720p", "1080p"):
    raise SystemExit(f"ENCODE_QUALITY noto'g'ri: {QUALITY!r}")
STOP_RUN = False  # process() keyingi sifatni yangi run'ga qoldirdi — main sikli to'xtaydi
# Actions log'i shu YOPIQ kanalga yoziladi (yangi xabarlar, har 10 soniyada).
# 0 yoki bo'sh — o'chiq.
LOG_CHANNEL = int(os.environ.get("LOG_CHANNEL_ID", "0") or 0)
# (2026-10, yangi talab) Sifat-run Telegram'ga log YUBORMAYDI: holat daqiqada bir marta worker'ga ketadi
# (`StatusPin.send_worker`), bot "Holat" xabari shundan yangilanadi. Shu sabab kanal ID'lari ishlatilmaydi.
LOG_INTERVAL = max(1.0, float(os.environ.get("LOG_INTERVAL_SEC", "10") or 10))
# Qadalgan holat xabari shu oraliqda tahrirlanadi (`StatusPin`).
# 3 soniya — admin paneli uni ~2 soniyada o'qiydi (deyarli real vaqt);
# Telegram bundan tez tahrirlashga ruxsat bermaydi (FloodWait).
# Sifat-run o'z log kanaliga har 5 soniyada YANGI xabar yuboradi (foydalanuvchi talabi).
STATUS_INTERVAL = max(2.0, float(os.environ.get("STATUS_INTERVAL_SEC", "5" if QUALITY else "3") or 3))
# To'liq holat worker'ga (`EncodeLive`) shu oraliqda yuboriladi.
# Sifat-run daqiqada bir marta (worker so'rovlari va Durable Object limiti tejaladi); eski rejim ~8 s.
WORKER_PUSH_SEC = max(3.0, float(os.environ.get("WORKER_PUSH_SEC", "60" if QUALITY else "8") or 8))
SESSION = str(Path(__file__).with_name("pyro_session"))
WORK = Path(os.environ.get("RUNNER_TEMP", "/tmp")) / "arugram_encode"


def session_api_id() -> int:
    """Sessiya faylidagi api_id (o'qib bo'lmasa 0)."""
    try:
        import sqlite3
        c = sqlite3.connect(f"file:{SESSION}.session?mode=ro", uri=True)
        return int(c.execute("SELECT api_id FROM sessions").fetchone()[0] or 0)
    except Exception:
        return 0

# (sifat, balandlik, CRF farqi) — kattadan kichikka.
LADDER = [("1080p", 1080, 0), ("720p", 720, 1), ("480p", 480, 2), ("360p", 360, 3)]
CHUNK = 4 * 1024 * 1024
T0 = time.time()


CURRENT = None  # hozir ishlanayotgan ish (bekor qilinganda qaytarish uchun)


def on_cancel(signum, frame):
    """Run bekor qilindi (GitHub SIGINT/SIGTERM yuboradi): ishni darhol
    navbatga qaytaramiz — keyingi run 6 daqiqa kutib o'tirmasin."""
    if CURRENT:
        try:
            req = urllib.request.Request(
                f"{API}/api/encode/finish",
                data=json.dumps({**CURRENT, "ok": False, "cancelled": True}).encode(),
                method="POST",
                headers={"X-Encode-Token": TOKEN, "Content-Type": "application/json",
                         "User-Agent": "arugram-encoder"})
            urllib.request.urlopen(req, timeout=10).read()
            print("Run bekor qilindi — ish navbatga qaytarildi", flush=True)
        except Exception as e:
            print("Bekor qilishda qaytarib bo'lmadi:", e, flush=True)
    os._exit(0)


class JobLost(Exception):
    """Ish boshqa run'ga o'tdi yoki qayta navbatga qo'yildi."""


class Fatal(Exception):
    """Qayta urinishning foydasi yo'q (buzuq manba va h.k.)."""


class ChannelLog:
    """Actions log'ini yopiq kanalga YANGI xabarlar bilan yuboradi.

    Xabar TAHRIRLANMAYDI (foydalanuvchi talabi): har `LOG_INTERVAL` soniyada
    (odatda 10) shu orada to'plangan qatorlar bitta yangi xabar bo'lib
    ketadi. Har qism o'z sarlavhasi bilan boshlanadi. Telegram FloodWait
    bersa — qatorlar to'planib turadi va ruxsat berilgach bitta xabar bo'lib
    ketadi (log yo'qolmaydi). Xato bo'lsa kodlashga TEGMAYDI — 5 marta
    ketma-ket xatodan keyin kanalga yozish o'chadi.
    """
    LIMIT = 3600  # bitta xabar 4096 belgidan oshmasin

    def __init__(self):
        self.lock = threading.Lock()
        self.pending = collections.deque()
        self.active = False
        self.fails = 0
        self.hold_until = 0.0
        # Sifat-run'da kanalga alohida log oqimi yuborilmaydi — faqat run'ning o'z (tahrirlanadigan) xabari bor.
        self.off = LOG_CHANNEL == 0 or bool(QUALITY)

    def start(self, title):
        with self.lock:
            self.pending.clear()
            self.active = True
            self.pending.append(("title", title))

    def heading(self, title):
        """Yangi qism sarlavhasi (xabar oqimi to'xtamaydi)."""
        with self.lock:
            if self.active:
                self.pending.append(("title", title))

    def stop(self):
        with self.lock:
            self.active = False

    def add(self, text, progress=False):
        with self.lock:
            if self.active:
                self.pending.append(("line", text))

    def _take(self):
        """Yuboriladigan qatorlarni oladi (navbatdan hali OLMAYDI)."""
        with self.lock:
            items = list(self.pending)
        out, size, n = [], 0, 0
        for kind, text in items:
            piece = f"<b>{html.escape(text)}</b>" if kind == "title" else html.escape(text)
            if out and size + len(piece) + 1 > self.LIMIT:
                break
            out.append(piece)
            size += len(piece) + 1
            n += 1
        return n, "\n".join(out)

    async def flush(self, app):
        if self.off or time.time() < self.hold_until:
            return
        n, text = self._take()
        if not n:
            return
        try:
            await app.send_message(LOG_CHANNEL, text, parse_mode=enums.ParseMode.HTML,
                                   disable_notification=True)
            with self.lock:
                for _ in range(n):
                    self.pending.popleft()
            self.fails = 0
        except Exception as e:
            if type(e).__name__ == "FloodWait":
                # Qatorlar to'planib turadi, ruxsat berilgach bittada ketadi.
                self.hold_until = time.time() + int(getattr(e, "value", 10) or 10)
                return
            self.fails += 1
            print(f"Kanalga log yozilmadi ({self.fails}/5): {e}", flush=True)
            if self.fails >= 5:
                self.off = True
                print("Kanalga log yozish o'chirildi.", flush=True)

    async def loop(self, app):
        while True:
            await asyncio.sleep(LOG_INTERVAL)
            await self.flush(app)


CHLOG = ChannelLog()


def transfer_progress(kind: str):
    """Telegram yuklab olish/yuklash jarayoni: har soniyada bitta qator."""
    t0, last = time.time(), [0.0]

    def cb(cur, total):
        now = time.time()
        if now - last[0] < 1.0 and cur < total:
            return
        last[0] = now
        pct = cur * 100 / total if total else 0
        sp = cur / max(now - t0, 0.1) / 1048576
        log(f"    {kind} {pct:5.1f}% | {cur / 1048576:.1f}/{total / 1048576:.1f} MB | {sp:.2f} MB/s")
        STATUS.update(xfer={
            "kind": kind, "pct": round(pct, 1), "cur_mb": round(cur / 1048576, 1),
            "total_mb": round(total / 1048576, 1), "mbps": round(sp, 2),
            "elapsed": int(now - t0),
            "eta": int((total - cur) / 1048576 / sp) if sp > 0 and total else -1,
        })
    return cb


# ── Telegram'ga yuklash: NAVBAT va qotib qolsa QAYTA URINISH ─────────────────────────────
# Bir vaqtda faqat BITTA sifat yuklaydi (worker'dagi qulf: `/api/encode/ulock`), qolganlari kutadi — 4 ta run bir
# Telegram sessiyasidan birdan yuklasa, yuklash oxirida (100%) qotib qolardi. Yuklash to'xtab qolsa (progress
# `STALL_S` soniya o'zgarmasa) bekor qilinadi va TAYYOR (shifrlangan) fayl QAYTA kodlanmasdan qayta yuboriladi;
# oldingi urinish aslida kanalga o'tib ulgurgan bo'lsa (javob yo'qolgan) — o'sha xabar ishlatiladi.
STALL_S = int(os.environ.get("UPLOAD_STALL_SEC", "120"))
SEND_TRIES = int(os.environ.get("UPLOAD_TRIES", "5"))
LOCK_TTL_S = 900


def ulock(op: str) -> bool:
    """Yuklash navbati: `acquire` — navbat sizniki bo'lsa True; `release` — qaytaradi. Xato bo'lsa — kutmaymiz."""
    try:
        r = api("ulock", {"runner": RUNNER, "wq": QUALITY or "all", "op": op, "ttl_s": LOCK_TTL_S})
        return bool(r.get("ok"))
    except JobLost:
        return True
    except Exception as ex:  # noqa: BLE001
        print(f"yuklash navbati xato ({ex}) — kutilmaydi", flush=True)
        return True


async def upload_turn(label: str):
    """Telegram'ga yuklash navbatini kutadi (boshqa sifat yuklayotgan bo'lsa)."""
    waited = 0
    while not await asyncio.to_thread(ulock, "acquire"):
        if waited == 0:
            log(f"  {label}: boshqa sifat Telegram'ga yuklayapti — navbat kutilmoqda...")
        await asyncio.sleep(3)
        waited += 3
        if waited >= 40 * 60:
            log(f"  {label}: navbat 40 daqiqa kutildi — baribir yuklanadi")
            return


async def find_sent(app, channel: int, name: str, size: int):
    """Kanalning oxirgi xabarlarida shu nomli fayl bormi (oldingi urinish aslida o'tgan bo'lishi mumkin)."""
    try:
        async for m in app.get_chat_history(channel, limit=20):
            d = getattr(m, "document", None)
            if d and getattr(d, "file_name", "") == name and (not size or getattr(d, "file_size", 0) == size):
                return m
    except Exception as ex:  # noqa: BLE001
        log(f"    kanal tarixi o'qilmadi: {ex}")
    return None


BOT = ""  # kodlash botining username'i (worker `claim` javobida beradi); bo'sh — eski usul (kanalga to'g'ridan-to'g'ri)


R2 = False  # worker `claim` javobida (`r2`): R2 sozlangan bo'lsa tayyor fayllar R2'ga yoziladi (yuklovchi workflow Telegram'ga olib boradi)


class _Slice:
    """Fayldan `length` bayt (bo'lak) o'qiydi — xotiraga yig'ilmaydi (oqim); o'qilgan sari foiz yangilanadi."""

    def __init__(self, f, length: int, cb):
        self.f, self.left, self.cb = f, length, cb

    def read(self, n=-1):
        if self.left <= 0:
            return b""
        n = self.left if n is None or n < 0 else min(n, self.left)
        b = self.f.read(n)
        self.left -= len(b)
        self.cb(len(b))
        return b


def put_parts(path: Path, name: str, part_size: int, label: str) -> list:
    """Faylni worker orqali R2'ga yozadi: har bo'lak (standart 90 MB) OQIM bilan, xotiraga yig'ilmasdan; hech qanday R2/S3 kaliti
    kerak emas — worker o'z binding'i bilan yozadi. Qaytadi: `[{"n": 1, "etag": "..."}, ...]` (worker `r2ready` da yig'adi)."""
    size = path.stat().st_size
    t0, state = time.time(), {"done": 0, "last": 0.0}

    def progress(k):
        state["done"] += k
        now = time.time()
        if now - state["last"] >= 1.0 or state["done"] >= size:
            state["last"] = now
            d = state["done"]
            sp = d / max(now - t0, 0.1) / 1048576
            STATUS.update(xfer={
                "kind": "R2'ga yuklash", "pct": round(d * 100 / size, 1) if size else 0,
                "cur_mb": round(d / 1048576, 1), "total_mb": round(size / 1048576, 1), "mbps": round(sp, 2),
                "elapsed": int(now - t0), "eta": int((size - d) / 1048576 / sp) if sp > 0 else -1,
            })

    parts = []
    with open(path, "rb") as f:
        n, off = 0, 0
        while off < size:
            n += 1
            length = min(part_size, size - off)
            err = None
            for attempt in range(1, 5):
                try:
                    f.seek(off)
                    state["done"] = off  # qayta urinishda foiz orqaga qaytadi
                    req = urllib.request.Request(
                        f"{API}/api/r2/part?file={urllib.parse.quote(name)}&n={n}", data=_Slice(f, length, progress), method="PUT",
                        headers={"X-Encode-Token": TOKEN, "Content-Type": "application/octet-stream",
                                 "Content-Length": str(length), "User-Agent": "arugram-encoder"})
                    with urllib.request.urlopen(req, timeout=900) as r:
                        parts.append({"n": n, "etag": json.loads(r.read())["etag"]})
                    err = None
                    break
                except Exception as ex:  # noqa: BLE001
                    err = ex
                    time.sleep(3 * attempt)
            if err is not None:
                raise RuntimeError(f"bo'lak {n}: {err}")
            off += length
    return parts


async def upload_r2(ident: dict, path: Path, name: str, kind: str, key: str, label: str) -> bool:
    """Tayyor faylni R2'ga yozadi (`kind` — mp4|fmp4). Worker R2 sozlanmagan desa `False` (eski usulga qaytiladi)."""
    size = path.stat().st_size
    last = ""
    waited = 0
    attempt = 0
    while True:
        if waited == 0:
            attempt += 1
        try:
            r = await asyncio.to_thread(api, "r2put", {**ident, "kind": kind, "file": name, "key": key,
                                                       "size": size, "quality": label})
            if r.get("wait"):
                # Worker: "R2'da joy yo'q" — bo'shaguncha kutiladi (yuklovchi workflow fayllarni Telegram'ga olib borib o'chiradi).
                if not waited:
                    log(f"  {label}: R2'da {size / 1048576:.0f} MB uchun joy yo'q — bo'shashi kutilmoqda...")
                waited += 30
                if waited > 8 * 3600:
                    raise RuntimeError("R2 8 soat bo'shamadi")
                await asyncio.sleep(30)
                continue
            if waited:
                log(f"  {label}: R2'da joy ochildi ({waited // 60} daqiqa kutildi)")
                waited = 0
            parts = await asyncio.to_thread(put_parts, path, name, int(r.get("part_size") or 94371840), label)
            await asyncio.to_thread(api, "r2ready", {**ident, "kind": kind, "file": name, "size": size, "parts": parts})
            STATUS.update(xfer=None)
            log(f"  {label}: {name} R2'ga yozildi ({size / 1048576:.1f} MB) — Telegram'ga yuklovchi workflow olib boradi")
            return True
        except JobLost:
            raise
        except Exception as ex:  # noqa: BLE001
            last = str(ex)
            if "503" in last and attempt == 1:
                log(f"  {label}: R2 sozlanmagan (yoki fayl R2'ga umuman sig'maydi) — to'g'ridan-to'g'ri bot chatiga yuklanadi")
                return False
            log(f"  {label}: R2'ga yozish {attempt}/3 muvaffaqiyatsiz: {last}")
            if attempt >= 3:
                break
            await asyncio.sleep(4 * attempt)
    # R2 ishlamasa (to'lgan, uzilgan) kodlash yo'qolmasin: eski usulda to'g'ridan-to'g'ri bot chatiga yuklanadi.
    log(f"  {label}: R2 ishlamadi — to'g'ridan-to'g'ri bot chatiga yuklanadi")
    return False


def is_committed(name: str) -> bool:
    """Bot faylni yopiq kanalga saqlab, bazaga yozdimi (`/api/encode/committed`)."""
    try:
        req = urllib.request.Request(
            f"{API}/api/encode/committed?file={urllib.parse.quote(name)}",
            headers={"X-Encode-Token": TOKEN, "User-Agent": "arugram-encoder"})
        with urllib.request.urlopen(req, timeout=30) as r:
            return bool(json.loads(r.read() or b"{}").get("ok"))
    except Exception as ex:  # noqa: BLE001
        print(f"committed tekshiruvi xato: {ex}", flush=True)
        return False


async def wait_committed(name: str, label: str, timeout: int = 300):
    """Bot faylni saqlab bazaga yozguncha kutadi; shundan keyingina run davom etadi/o'chadi."""
    end = time.time() + timeout
    while time.time() < end:
        if await asyncio.to_thread(is_committed, name):
            log(f"  {label}: bot faylni yopiq kanalga saqladi va bazaga yozdi")
            return
        await asyncio.sleep(3)
    raise RuntimeError(f"{label}: bot {timeout} soniyada faylni saqlamadi (bot kanalda adminmi? sessiya ro'yxatdan o'tganmi?)")


async def fetch_origin_via_bot(app, ident: dict):
    """Worker orqali bot asl videoni shu sessiya chatiga yuboradi; chat tarixidan topib qaytaradi."""
    r = await asyncio.to_thread(api, "origin", ident)
    tag = r.get("tag", "")
    end = time.time() + 180
    while time.time() < end:
        try:
            async for m in app.get_chat_history(BOT, limit=15):
                if (m.caption or "") == tag and (m.document or m.video):
                    return m
        except Exception as ex:  # noqa: BLE001
            log(f"    bot chati o'qilmadi: {ex}")
        await asyncio.sleep(3)
    raise RuntimeError("bot asl videoni yubormadi (sessiya botni /start qilganmi?)")


async def send_file(app, channel, path: Path, name: str, label: str, caption: str = ""):
    """Faylni yuboradi (qotib qolsa bekor qilib, o'sha faylni qayta yuboradi). `channel` — kanal ID yoki bot username;
    `caption` berilsa (bot chatiga) — shu izoh, aks holda fayl nomi."""
    size = path.stat().st_size
    last_err = ""
    for attempt in range(1, SEND_TRIES + 1):
        prog = {"p": -1, "t": time.time()}
        base = transfer_progress(f"{label} Telegram'ga yuklanmoqda")

        def cb(cur, total, base=base, prog=prog):
            if cur != prog["p"]:
                prog["p"], prog["t"] = cur, time.time()
            return base(cur, total)

        task = asyncio.create_task(app.send_document(
            channel, str(path), file_name=name, force_document=True, caption=caption or name,
            disable_notification=True, progress=cb))
        renew = time.time()
        stalled = False
        while True:
            done, _ = await asyncio.wait({task}, timeout=10)
            if done:
                break
            now = time.time()
            if now - renew > 60:  # navbat ijarasi uzaytiriladi
                await asyncio.to_thread(ulock, "acquire")
                renew = now
            if now - prog["t"] > STALL_S:
                stalled = True
                task.cancel()
                try:
                    await task
                except BaseException:  # noqa: BLE001 — CancelledError ham
                    pass
                break
        if not stalled:
            try:
                return task.result()
            except Exception as ex:  # noqa: BLE001
                last_err = f"{type(ex).__name__}: {ex}"
                if type(ex).__name__ == "FloodWait":
                    await asyncio.sleep(int(getattr(ex, "value", 10) or 10))
        else:
            last_err = f"{STALL_S} soniya qimirlamadi ({prog['p'] * 100 // max(size, 1)}%)"
        log(f"  {label}: yuklash {attempt}/{SEND_TRIES} muvaffaqiyatsiz — {last_err}")
        # Bot chatiga yuborilgan bo'lsa bot uni allaqachon saqlab, chatdan o'chirgan bo'lishi mumkin.
        if caption and await asyncio.to_thread(is_committed, name):
            log(f"  {label}: fayl allaqachon saqlangan — qayta yuklanmaydi")
            return None
        found = await find_sent(app, channel, name, size)
        if found:
            log(f"  {label}: fayl topildi — qayta yuklanmaydi")
            return found
        await asyncio.sleep(5 * attempt)
    raise RuntimeError(f"{label}: Telegram'ga yuklab bo'lmadi ({SEND_TRIES} urinish): {last_err}")


def log(*a):
    print(time.strftime("%H:%M:%S"), *a, flush=True)
    line = time.strftime("%H:%M:%S ") + " ".join(str(x) for x in a)
    CHLOG.add(line)
    STATUS.line(line)


def api(path, body=None, method="POST"):
    data = None if body is None else json.dumps(body).encode()
    for attempt in range(5):
        req = urllib.request.Request(
            f"{API}/api/encode/{path}",
            data=data,
            method=method,
            headers={"X-Encode-Token": TOKEN, "Content-Type": "application/json",
                     "User-Agent": "arugram-encoder"},
        )
        try:
            with urllib.request.urlopen(req, timeout=60) as r:
                return json.loads(r.read() or b"{}")
        except urllib.error.HTTPError as e:
            if e.code == 409:
                raise JobLost()
            if e.code in (400, 401, 403, 404):
                raise RuntimeError(f"{path}: HTTP {e.code} {e.read()[:200]!r}")
            err = e
        except Exception as e:  # tarmoq
            err = e
        time.sleep(3 * (attempt + 1))
    raise RuntimeError(f"{path}: {err}")


def api_fmp4(path, body):
    """`/api/fmp4/<path>` (Mini App uchun fMP4 nusxa) — xato bo'lsa jim."""
    req = urllib.request.Request(
        f"{API}/api/fmp4/{path}", data=json.dumps(body).encode(), method="POST",
        headers={"X-Encode-Token": TOKEN, "Content-Type": "application/json",
                 "User-Agent": "arugram-encoder"})
    for attempt in range(3):
        try:
            with urllib.request.urlopen(req, timeout=60) as r:
                return json.loads(r.read() or b"{}")
        except Exception as ex:  # noqa: BLE001
            log(f"  fmp4/{path}: {ex}")
            time.sleep(2 + attempt * 3)
    return {}


def make_fmp4(src: Path, dst: Path):
    """MP4 -> fMP4 (bo'laklangan, boshida `sidx` indeksi). QAYTA SIQILMAYDI
    (`-c copy`) — bir necha soniya. Mini App pleyeri (MSE) uchun
    (`worker/src/fmp4.rs`)."""
    subprocess.run(
        ["ffmpeg", "-hide_banner", "-loglevel", "error", "-y", "-i", str(src),
         "-map", "0", "-c", "copy",
         "-movflags", "+frag_keyframe+empty_moov+default_base_moof+global_sidx",
         str(dst)], check=True, timeout=1800)


def ctr_file(src: Path, dst: Path, key: bytes):
    """AES-128-CTR (IV nol, 128-bit big-endian hisoblagich)."""
    enc = Cipher(algorithms.AES(key), modes.CTR(b"\0" * 16)).encryptor()
    with open(src, "rb") as fi, open(dst, "wb") as fo:
        while True:
            b = fi.read(CHUNK)
            if not b:
                break
            fo.write(enc.update(b))
        fo.write(enc.finalize())


def probe(path: Path):
    """(balandlik, davomiylik soniyada) — o'qib bo'lmasa Fatal."""
    try:
        out = subprocess.run(
            ["ffprobe", "-v", "error", "-select_streams", "v:0",
             "-show_entries", "stream=height:format=duration", "-of", "json", str(path)],
            capture_output=True, text=True, timeout=120, check=True).stdout
        j = json.loads(out)
        h = int(j["streams"][0]["height"])
        d = float(j["format"]["duration"])
        if h <= 0 or d <= 0:
            raise ValueError("bo'sh")
        return h, d
    except Exception as e:
        raise Fatal(f"video o'qilmadi: {e}")


def src_info(path: Path) -> dict:
    """Manba haqida qo'shimcha (kadr tezligi, kodek, eni) — panel uchun."""
    try:
        out = subprocess.run(
            ["ffprobe", "-v", "error", "-select_streams", "v:0",
             "-show_entries", "stream=codec_name,width,avg_frame_rate,nb_frames",
             "-of", "json", str(path)],
            capture_output=True, text=True, timeout=60, check=True).stdout
        st = json.loads(out)["streams"][0]
        num, _, den = str(st.get("avg_frame_rate", "0/1")).partition("/")
        fps = float(num) / float(den or 1) if float(den or 1) else 0.0
        return {"codec": st.get("codec_name", ""), "w": int(st.get("width") or 0),
                "fps": round(fps, 3)}
    except Exception:
        return {}


def plan(height):
    """Upscale YO'Q: manbadan baland sifat qilinmaydi. Nostandart
    balandlik (masalan 1070p) eng yaqin sifat sifatida o'lchamsiz."""
    h = min(height, 1080)
    out = []
    for label, target, dcrf in LADDER:
        if h >= target:
            out.append((label, target, dcrf))
        elif h >= target * 0.9 and not out:
            out.append((label, h - h % 2, dcrf))
    if not out:
        out.append(("360p", h - h % 2, 3))
    return out


def cpu_model() -> str:
    """CPU modeli va yadrolar (mantiqiy / jismoniy) — tezlik farqini tushunish uchun."""
    model, phys = "CPU noma'lum", ""
    try:
        for line in open("/proc/cpuinfo"):
            if line.startswith("model name") and model == "CPU noma'lum":
                model = line.split(":", 1)[1].strip()
            elif line.startswith("cpu cores") and not phys:
                phys = line.split(":", 1)[1].strip()
    except Exception:
        pass
    logical = os.cpu_count()
    return f"{model}, {logical} mantiqiy" + (f" / {phys} jismoniy yadro" if phys else " yadro")


def hms(sec: float) -> str:
    sec = max(0, int(sec))
    h, r = divmod(sec, 3600)
    m, s_ = divmod(r, 60)
    return f"{h}:{m:02d}:{s_:02d}" if h else f"{m:02d}:{s_:02d}"


def progress_line(label: str, f: dict, dur: float, started: float):
    """ffmpeg `-progress` bloki -> (foiz, log qatori) yoki None."""
    us = f.get("out_time_us") or f.get("out_time_ms") or ""
    if not us.isdigit() or dur <= 0:
        return None
    t = int(us) / 1e6
    pct = min(99, int(t / dur * 100))
    speed = f.get("speed", "").strip()
    try:
        sp = float(speed.rstrip("x"))
    except ValueError:
        sp = 0.0
    eta = hms((dur - t) / sp) if sp > 0 else "--:--"
    br = f.get("bitrate", "").strip()
    try:
        brt = f"{float(br.replace('kbits/s', '')):.0f} kb/s"
    except ValueError:
        brt = "-"
    size = f.get("total_size", "")
    if size.isdigit() and int(size) > 0:
        # Hozirgi hajm va shu sur'atda yakuniy taxminiy hajm.
        mb = f"{int(size) / 1048576:.1f} MB"
        if t > 5:
            mb += f" (~{int(size) / 1048576 * dur / t:.0f} MB bo'ladi)"
    else:
        mb = "-"
    line = (f"    {label} {pct:3d}% | video {hms(t)}/{hms(dur)} | "
            f"tezlik {speed or '-'} | {f.get('fps', '-')} kadr/s | "
            f"bitreyt {brt} | {mb} | o'tdi {hms(time.time() - started)} | qoldi ~{eta}")
    # Botning "Holat" xabari uchun ixcham ko'rinish (`|` bilan, bo'shliqsiz):
    # tezlik|kadr/s|bitreyt|hajm MB|taxminiy MB|o'tdi s|qoldi s
    def num(x, fmt="{:.1f}"):
        try:
            return fmt.format(float(x))
        except ValueError:
            return "-"
    est = f"{int(size) / 1048576 * dur / t:.0f}" if size.isdigit() and int(size) > 0 and t > 5 else "-"
    # Admin paneli uchun TO'LIQ ma'lumot (`StatusPin`, `cur`).
    def fnum(x):
        try:
            return float(str(x).replace("kbits/s", "").rstrip("x").strip())
        except ValueError:
            return None
    frame = int(f["frame"]) if f.get("frame", "").isdigit() else None
    detail = {
        "q": label,
        "pct": round(min(t / dur * 100, 100.0), 2),
        "out_s": round(t, 1),
        "dur_s": round(dur, 1),
        "frame": frame,
        "fps": fnum(f.get("fps", "")),
        "speed": fnum(speed) if speed else None,
        "bitrate_kbps": fnum(br),
        "size_mb": round(int(size) / 1048576, 2) if size.isdigit() else None,
        "est_mb": round(int(size) / 1048576 * dur / t, 1) if size.isdigit() and int(size) > 0 and t > 5 else None,
        "elapsed": int(time.time() - started),
        "eta": int((dur - t) / sp) if sp > 0 else -1,
        "drop": int(f["drop_frames"]) if f.get("drop_frames", "").isdigit() else None,
        "dup": int(f["dup_frames"]) if f.get("dup_frames", "").isdigit() else None,
        "qp": fnum(f.get("stream_0_0_q", "")),
    }
    tail = "|".join([
        (speed or "-").replace(" ", ""), num(f.get("fps", ""), "{:.1f}"),
        num(br.replace("kbits/s", ""), "{:.0f}"),
        f"{int(size) / 1048576:.1f}" if size.isdigit() else "-", est,
        str(int(time.time() - started)),
        str(int((dur - t) / sp)) if sp > 0 else "-",
    ])
    return pct, line, tail, detail


def encode(src: Path, dst: Path, src_h: int, target: int, dcrf: int,
           dur: float = 0.0, on_progress=None, label: str = ""):
    vf = [] if target >= src_h else ["-vf", f"scale=-2:{target}:flags=lanczos"]
    abr = "128k" if target >= 720 else "96k"
    cmd = ["ffmpeg", "-hide_banner", "-loglevel", "error", "-nostats",
           "-progress", "pipe:1", "-y", "-i", str(src),
           "-map", "0:v:0", "-map", "0:a:0?", "-map_metadata", "-1", "-sn", *vf,
           "-c:v", "libx265", "-preset", PRESET, "-crf", str(CRF_BASE - dcrf),
           "-x265-params", "log-level=error" + (":" + X265_EXTRA if X265_EXTRA else ""), "-pix_fmt", "yuv420p", "-tag:v", "hvc1",
           "-c:a", "aac", "-ac", "2", "-b:a", abr, "-ar", "44100",
           "-movflags", "+faststart", str(dst)]
    # `-progress pipe:1` — ffmpeg har soniyada `out_time_us=...` yozadi;
    # foiz = shu vaqt / manba davomiyligi (bot "Holat" xabari uchun).
    # Har soniyada bitta to'liq qator log'ga chiqadi (foiz, tezlik, ETA).
    p = subprocess.Popen(cmd, stdout=subprocess.PIPE, text=True)
    fields, last, last_log, started = {}, -1, 0.0, time.time()
    for line in p.stdout:
        k, _, v = line.strip().partition("=")
        if k != "progress":
            fields[k] = v
            continue
        r = progress_line(label, fields, dur, started)
        fields = {}
        if not r:
            continue
        pct, text, tail, detail = r
        if on_progress:
            on_progress(pct, tail)
        STATUS.update(cur=detail)
        if time.time() - last_log >= 1.0:
            last_log = time.time()
            print(time.strftime("%H:%M:%S"), text, flush=True)
            CHLOG.add(time.strftime("%H:%M:%S ") + text.strip(), progress=True)
            STATUS.line(time.strftime("%H:%M:%S ") + text.strip(), progress=True)
    if p.wait() != 0:
        raise subprocess.CalledProcessError(p.returncode, "ffmpeg")


class StatusPin:
    """Log kanalidagi QADALGAN holat xabari (`#arustatus`).

    TALAB (foydalanuvchi): "yangilash tugmasini bosganda worker oxirgi logni
    so'rab olsin ... shunda har 10 daqiqada bazaga log yozish shart
    bo'lmasdi". Runner'ga tashqaridan murojaat qilib bo'lmaydi, shu sabab
    holat bitta xabarda turadi va ~15 soniyada tahrirlanadi (bepul, bazaga
    hech narsa yozilmaydi). Worker'ning boti (kanal admini) uni faqat admin
    so'raganda `getChat` -> `pinned_message` bilan o'qiydi.

    Birinchi qatorlar — mashina o'qiydigan `kalit: qiymat`, `---` dan keyin
    oxirgi log qatorlari. Xato bo'lsa kodlashga TEGMAYDI.
    """
    TAG = "#arustatus"
    LINES = 14

    def __init__(self):
        self.lock = threading.Lock()
        self.lines = collections.deque(maxlen=self.LINES)
        self.job = ""
        self.num = ""
        self.progress = "idle"
        self.msg_id = 0
        self.sent = ""
        self.off = LOG_CHANNEL == 0 or bool(QUALITY)  # sifat-run Telegram'da xabar yuborib/tahrirlamaydi
        self.bots = []
        # Batafsil statistika (admin paneli uchun, `data:` qatorida JSON).
        self.data = {}
        self._cpu = None
        self.interval = STATUS_INTERVAL

    def update(self, **kv):
        with self.lock:
            for k, v in kv.items():
                if v is None:
                    self.data.pop(k, None)
                else:
                    self.data[k] = v

    def quality(self, q, **kv):
        """Sifatlar zinasidagi bitta sifat holatini yangilaydi."""
        with self.lock:
            for row in self.data.get("ladder", []):
                if row.get("q") == q:
                    row.update(kv)

    def html_text(self) -> str:
        """Sifat-run xabari — botdagi "Holat" ko'rinishida: sarlavha, manba, bosqich (foiz-chiziq), oxirgi loglar."""
        esc = html.escape

        def bar(p):
            n = round(max(0.0, min(100.0, p)) / 100 * 12)
            return "\u2593" * n + "\u2591" * (12 - n)

        with self.lock:
            d = dict(self.data)
            num, progress = self.num, self.progress
            lines = [t for _, t in self.lines][-3:]
        head = f"\U0001F539 <b>{QUALITY}</b>"
        if progress == "idle":
            return head + " \u00B7 run tugadi, yangisi ochiladi\u2026"
        title = ", ".join(x for x in (d.get("anime_title"), d.get("season_title")) if x)
        head += f" \u00B7 {esc(title)}, {esc(num or '-')}-qism" if title else f" \u00B7 {esc(num or '-')}-qism"
        if (d.get("attempt") or 1) > 1:
            head += f" (urinish {d['attempt']})"
        t = head
        src = d.get("src")
        if isinstance(src, dict):
            t += (f"\n\U0001F39E Manba: {src.get('h')}p \u00B7 {hms(src.get('dur_s') or 0)} \u00B7 "
                  f"{src.get('mb')} MB")
        cur, x = d.get("cur"), d.get("xfer")
        if progress.startswith("enc") and isinstance(cur, dict):
            pct = float(cur.get("pct") or 0)
            est = f" (~{cur['est_mb']} MB)" if cur.get("est_mb") else ""
            t += (f"\n\u2699\uFE0F Kodlanmoqda {bar(pct)} {pct:.0f}%"
                  f"\n    tezlik {cur.get('speed')}x \u00B7 {cur.get('fps')} kadr/s \u00B7 {cur.get('bitrate_kbps')} kb/s"
                  f" \u00B7 hajm {cur.get('size_mb')} MB{est}"
                  f"\n    o'tdi {hms(cur.get('elapsed') or 0)} \u00B7 qoldi ~{hms(cur.get('eta') or 0)}")
        elif isinstance(x, dict):
            up = "Telegram" in str(x.get("kind"))
            pct = float(x.get("pct") or 0)
            what = "\u2B06\uFE0F Telegram'ga yuklanmoqda" if up else "\u2B07\uFE0F Asl video yuklab olinmoqda"
            eta_s = hms(x["eta"]) if (x.get("eta") or -1) >= 0 else "--:--"
            t += (f"\n{what} {bar(pct)} {pct:.1f}%"
                  f"\n    tezlik {x.get('mbps')} MB/s \u00B7 {x.get('cur_mb')} / {x.get('total_mb')} MB"
                  f"\n    o'tdi {hms(x.get('elapsed') or 0)} \u00B7 qoldi ~{eta_s}")
        elif progress.startswith("upload"):
            t += "\n\u2B06\uFE0F Telegram'ga yuklanmoqda"
        elif progress.startswith("download"):
            t += "\n\u2B07\uFE0F Asl video yuklab olinmoqda"
        else:
            t += "\n\u23F3 boshlanmoqda..."
        for ln in lines:
            ln = ln.strip()
            if ln:
                t += f"\n\U0001F4DC {esc(ln[:90])}"
        if d.get("job_started"):
            t += f"\n\u23F1 jami: {hms(time.time() - d['job_started'])}"
        return t[:4000]

    def _sys(self):
        """CPU, RAM, disk — Linux runner'da /proc dan (xato bo'lsa bo'sh)."""
        out = {}
        try:
            with open("/proc/stat") as f:
                v = [int(x) for x in f.readline().split()[1:]]
            idle, total = v[3] + (v[4] if len(v) > 4 else 0), sum(v)
            if self._cpu:
                di, dt = idle - self._cpu[0], total - self._cpu[1]
                if dt > 0:
                    out["cpu"] = round(100 * (1 - di / dt), 1)
            self._cpu = (idle, total)
        except Exception:
            pass
        try:
            mem = {}
            with open("/proc/meminfo") as f:
                for ln in f:
                    k, _, v = ln.partition(":")
                    mem[k] = int(v.split()[0])
            out["ram_total_mb"] = mem["MemTotal"] // 1024
            out["ram_used_mb"] = (mem["MemTotal"] - mem.get("MemAvailable", 0)) // 1024
        except Exception:
            pass
        try:
            du = shutil.disk_usage(WORK.parent)
            out["disk_free_gb"] = round(du.free / 1e9, 1)
        except Exception:
            pass
        try:
            out["load"] = round(os.getloadavg()[0], 2)
        except Exception:
            pass
        out["cores"] = os.cpu_count() or 0
        return out

    def line(self, text, progress=False):
        with self.lock:
            # ffmpeg har soniyada qator beradi — oxirgisi almashtiriladi.
            if progress and self.lines and self.lines[-1][0]:
                self.lines[-1] = (True, text)
            else:
                self.lines.append((progress, text))

    def set_job(self, job: str, num: str):
        with self.lock:
            self.job, self.num = job, num

    def set_progress(self, progress: str):
        with self.lock:
            self.progress = progress or "idle"

    def text(self) -> str:
        sysinfo = self._sys()
        with self.lock:
            data = dict(self.data)
            data["sys"] = sysinfo
            data["run_s"] = int(time.time() - T0)
            head = [self.TAG, f"run: {RUNNER}", f"job: {self.job}", f"num: {self.num}",
                    f"progress: {self.progress}", f"updated: {int(time.time())}",
                    "data: " + json.dumps(data, separators=(",", ":"), ensure_ascii=False),
                    "---"]
            body = [t for _, t in self.lines]
        # Telegram xabari 4096 belgidan oshmasin — eski qatorlar tashlanadi.
        while body and len("\n".join(head + body)) > 4000:
            body.pop(0)
        return "\n".join(head + body)[:4000]

    async def _promote(self, app):
        # Worker'ning botlari kanalda ADMIN bo'lmasa qadalgan xabarni o'qiy
        # olmaydi — kanal egasi (shu sessiya) ularni o'zi qo'shadi.
        from pyrogram.types import ChatPrivileges
        for b in self.bots:
            try:
                await app.promote_chat_member(
                    LOG_CHANNEL, b, ChatPrivileges(can_manage_chat=True, can_post_messages=True))
            except Exception as e:
                print(f"@{b} log kanaliga admin qilinmadi: {e}", flush=True)

    async def start(self, app):
        if self.off:
            return
        if QUALITY:
            # Har sifat-run ishga tushganda kanalga O'Z xabarini yuboradi va faqat shuni tahrirlaydi.
            try:
                t = self.html_text()
                m = await app.send_message(LOG_CHANNEL, t, disable_notification=True,
                                           parse_mode=enums.ParseMode.HTML, disable_web_page_preview=True)
                self.msg_id, self.sent = m.id, t
            except Exception as e:
                print(f"Holat xabari ochilmadi: {e}", flush=True)
                self.off = True
            return
        try:
            await self._promote(app)
            chat = await app.get_chat(LOG_CHANNEL)
            pm = getattr(chat, "pinned_message", None)
            if pm and (pm.text or "").startswith(self.TAG):
                self.msg_id = pm.id
            else:
                m = await app.send_message(LOG_CHANNEL, self.text(), disable_notification=True,
                                           parse_mode=enums.ParseMode.DISABLED)
                self.msg_id = m.id
                await app.pin_chat_message(LOG_CHANNEL, m.id, disable_notification=True)
        except Exception as e:
            print(f"Holat xabari ochilmadi: {e}", flush=True)
            self.off = True

    async def push(self, app):
        if self.off or not self.msg_id:
            return
        t = self.html_text() if QUALITY else self.text()
        if t == self.sent:
            return
        try:
            if QUALITY:
                # Sifat-run: har safar YANGI xabar (tahrir emas) — foydalanuvchi talabi, har 5 soniyada.
                m = await app.send_message(LOG_CHANNEL, t, disable_notification=True,
                                           parse_mode=enums.ParseMode.HTML, disable_web_page_preview=True)
                self.msg_id, self.sent = m.id, t
                return
            # TOPILGAN XATO: Pyrogram matnni sukut bo'yicha Markdown deb
            # o'qiydi — `---` ajratgich va log'dagi `--:--` "tagiga chizish"
            # belgisi bo'lib yo'qolardi, ilova esa log va statistikani
            # topa olmasdi ("Log hali bo'sh"). Endi matn O'Z HOLICHA ketadi.
            await app.edit_message_text(LOG_CHANNEL, self.msg_id, t,
                                        disable_web_page_preview=True,
                                        parse_mode=enums.ParseMode.HTML if QUALITY else enums.ParseMode.DISABLED)
            self.sent = t
        except Exception as e:
            if type(e).__name__ == "FloodWait":
                # Telegram sekinlashtirishni so'radi — oraliq uzayadi.
                self.interval = min(self.interval + 1.0, 15.0)
                await asyncio.sleep(int(getattr(e, "value", 10) or 10))
            elif type(e).__name__ != "MessageNotModified":
                print(f"Holat xabari yangilanmadi: {e}", flush=True)

    def send_worker(self):
        """To'liq holat va log'ni worker'ga (`/api/encode/push` -> `EncodeLive`).

        TALAB (foydalanuvchi): "har daqiqada oxirgi to'liq log keshga
        yozilsin va to'g'ridan-to'g'ri kesh orqali ko'rsatilsin". Bazaga
        (Turso) yozilmaydi. Xato bo'lsa kodlashga tegmaydi.

        Sifat-run (`ENCODE_QUALITY`) ham shu yo'l bilan yuboradi, lekin daqiqada bir marta (`WORKER_PUSH_SEC`);
        worker uni `X-Encode-Quality` bo'yicha xotirada saqlaydi va bot "Holat" xabarini yangilaydi.
        """
        try:
            req = urllib.request.Request(
                f"{API}/api/encode/push", data=self.text().encode(), method="POST",
                headers={"X-Encode-Token": TOKEN, "Content-Type": "text/plain; charset=utf-8",
                         "User-Agent": "arugram-encoder", **({"X-Encode-Quality": QUALITY} if QUALITY else {})})
            urllib.request.urlopen(req, timeout=20).read()
        except Exception as e:
            print(f"Holat worker'ga yuborilmadi: {e}", flush=True)

    async def loop(self, app):
        """Telegram'dagi QADALGAN xabarni tahrirlaydi. FloodWait bo'lsa oraliq
        o'sadi — lekin bu worker'ga yuborishga (bot/ilova logi) TEGMAYDI."""
        while True:
            await asyncio.sleep(self.interval)
            await self.push(app)

    async def worker_loop(self):
        """To'liq holatni worker'ga MUSTAQIL, qat'iy `WORKER_PUSH_SEC` oralig'ida
        yuboradi (bot "Ilova uchun" va ilova admin paneli shundan yangilanadi).
        TOPILGAN MUAMMO: avval bu Telegram pin-tahririga bog'langan edi —
        Telegram cheklaganda (FloodWait) bot/ilova logi ham 15 s gacha
        sekinlashar, ba'zida muzlab qolganday ko'rinardi. Endi ajratildi:
        anibla/post'dagidek doim ~5 soniyada yangilanadi."""
        while True:
            await asyncio.sleep(WORKER_PUSH_SEC)
            await asyncio.to_thread(self.send_worker)


STATUS = StatusPin()


def anibla_download(url: str, dst: Path):
    """anibla.uz HLS (`video` yoki `video\naudio`) -> mp4: `tool/anibla/download.py`
    dagi parallel yuklovchi (avtoencode reposida ham shu yo'lda turadi)."""
    sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "anibla"))
    import download as anibla  # noqa: E402

    class _Live:  # anibla holat xabari yo'q: yuklovchi o'zi har 10 s log yozadi
        def __getattr__(self, _):
            return lambda *a, **k: ""

    anibla.download(url, dst, _Live())


class Heartbeat:
    def __init__(self, ident):
        self.ident = ident
        self.stop = threading.Event()
        self.lost = False
        # Botning "Holat" xabari uchun: `download`, `enc|1080p|37|1|4`,
        # `upload|720p|2|4`.
        self.progress = ""
        # Davriy ping YO'Q (bazaga yozuv kamaysin): ijara run uchun
        # beriladi (`no_heartbeat`), holat esa `STATUS` da.

    def ping(self):
        """Ijarani uzaytiradi va jarayonni yuboradi. False — ish boshqaga o'tgan."""
        try:
            api("heartbeat", {**self.ident, "progress": self.progress})
            return True
        except JobLost:
            self.lost = True
            return False
        except Exception as e:
            log("heartbeat xato:", e)
            return True

    def set(self, progress, now=False):
        self.progress = progress
        STATUS.set_progress(progress)

    def check(self):
        if self.lost:
            raise JobLost()


async def process(app: Client, channel: int, job: dict):
    a, s, e, qa = job["anime_id"], job["season_id"], job["epizod_id"], job["queued_at"]
    ident = {"runner": RUNNER, "anime_id": a, "season_id": s, "epizod_id": e, "queued_at": qa}
    if QUALITY:
        ident["wq"] = QUALITY
    global CURRENT
    CURRENT = ident
    CHLOG.heading(f"\U0001F3AC Qism {a}/{s}/{e} (#{job.get('epizod_number')}) — run {RUNNER}"
                  + (f" [{QUALITY}]" if QUALITY else ""))
    hb = Heartbeat(ident)
    STATUS.set_job(f"{a}/{s}/{e}", str(job.get("epizod_number") or ""))
    STATUS.set_progress("start")
    STATUS.update(job_started=int(time.time()), attempt=job.get("attempt"),
                  anime_title=job.get("anime_title"), season_title=job.get("season_title"),
                  src=None, ladder=None, cur=None, xfer=None)
    await asyncio.to_thread(STATUS.send_worker)
    await STATUS.push(app)
    shutil.rmtree(WORK, ignore_errors=True)
    WORK.mkdir(parents=True)
    global STOP_RUN
    requeue = False
    try:
        done = set(job.get("done") or [])
        # Oldingi run yuklagan, jurnalga yozilmay qolgan sifatlar —
        # QAYTA kodlanmaydi, faqat jurnalga yoziladi.
        for u in job.get("uploaded") or []:
            m = await app.get_messages(channel, int(u["msg_id"]))
            size = getattr(getattr(m, "document", None), "file_size", 0) or 0
            if size > 0:
                api("quality", {**ident, "quality": u["quality"], "file": u["file"],
                                "size": size, "key": u["key"]})
                done.add(u["quality"])
                log(f"  {u['quality']}: avval yuklangan — jurnalga yozildi")

        log(f"Qism {a}/{s}/{e} (#{job.get('epizod_number')}), urinish {job.get('attempt')}")
        enc = WORK / "origin.enc"
        src = WORK / "origin.mp4"
        if job.get("origin_url"):
            # "Ilova uchun -> Anibla orqali": asl video Telegram'da emas — saytdan
            # (HLS, eng yuqori sifat) to'g'ridan-to'g'ri yuklab, shu zahoti kodlanadi.
            # Telegram'ning 2 GB chegarasi asl videoga tegmaydi.
            log(f"  asl video anibla.uz dan yuklab olinmoqda: {job.get('origin_label') or ''}")
            await asyncio.to_thread(hb.set, "download", True)
            await asyncio.to_thread(anibla_download, job["origin_url"], src)
            if not src.exists() or src.stat().st_size == 0:
                raise RuntimeError("anibla.uz dan yuklab bo'lmadi")
        else:
            bot_msg = None
            if QUALITY and BOT:
                # Xavfsizlik: asl videoni kanaldan emas, BOT chatidan olamiz — bot uni shu sessiya chatiga nusxalaydi.
                bot_msg = await fetch_origin_via_bot(app, ident)
                m = bot_msg
            else:
                m = await app.get_messages(channel, int(job["origin_msg"]))
            if not m or m.empty or not (m.document or m.video):
                raise Fatal("asl video kanalda topilmadi")
            log("  asl video yuklab olinmoqda...")
            await asyncio.to_thread(hb.set, "download", True)
            got = await app.download_media(m, file_name=str(enc),
                                           progress=transfer_progress("yuklab olinmoqda"))
            if not got or Path(got).stat().st_size == 0:
                raise RuntimeError("asl video yuklab olinmadi")
            if bot_msg is not None:
                try:
                    await app.delete_messages(BOT, bot_msg.id)
                except Exception as ex:  # noqa: BLE001
                    log(f"  bot chatidagi nusxa o'chirilmadi: {ex}")
            key = (job.get("origin_key") or "").strip()
            if key:
                await asyncio.to_thread(ctr_file, Path(got), src, bytes.fromhex(key))
                Path(got).unlink()
            else:
                Path(got).rename(src)
        hb.check()
        src_h, src_d = await asyncio.to_thread(probe, src)
        steps = plan(src_h)
        if QUALITY:
            # Haqiqiy balandlik worker'ga yoziladi: boshqa sifatlar uchun to'g'ri kelmaydiganlari
            # (upscale yo'q) "skip" bo'ladi, asl video bekorga yuklanmaydi.
            if src_h > 0 and src_h != int(job.get("src_h") or 0):
                try:
                    api("meta", {**ident, "h": src_h})
                except JobLost:
                    raise
                except Exception as ex:  # noqa: BLE001
                    log(f"  meta yozilmadi ({ex}) — davom etiladi")
            steps = [x for x in steps if x[0] == QUALITY]
            if not steps:
                log(f"  manba {src_h}p — {QUALITY} to'g'ri kelmaydi (upscale yo'q) — o'tkazib yuborildi")
                api("finish", {**ident, "ok": False, "skip": True})
                return
        src_mb = src.stat().st_size / 1048576
        extra = await asyncio.to_thread(src_info, src)
        STATUS.update(xfer=None, src={
            "h": src_h, "dur_s": round(src_d, 1), "mb": round(src_mb, 1),
            "kbps": round(src.stat().st_size * 8 / src_d / 1000), **extra,
            "frames": round(src_d * extra["fps"]) if extra.get("fps") else None,
        }, ladder=[{"q": l, "h": tgt, "crf": CRF_BASE - dc,
                    "state": "done" if l in done else "wait"} for l, tgt, dc in steps])
        log(f"  manba {src_h}p, {src_d:.0f} s, {src_mb:.1f} MB, "
            f"bitreyt ~{src.stat().st_size * 8 / src_d / 1000:.0f} kb/s -> "
            f"{', '.join(x[0] for x in steps)}")

        for idx, (label, target, dcrf) in enumerate(steps, 1):
            hb.check()
            if label in done:
                log(f"  {label}: tayyor — o'tkazib yuborildi")
                continue
            name = f"ep_{a}_{s}_{e}_{label}_{qa}.mp4"
            out = WORK / f"{label}.mp4"
            log(f"  {label}: kodlanmoqda...")
            t = time.time()
            STATUS.quality(label, state="enc", started=int(t))
            STATUS.update(cur=None, xfer=None)
            # Alohida oqimda — Telegram ulanishi (ping) uzilib qolmasin.
            await asyncio.to_thread(hb.set, f"enc|{label}|0|{idx}|{len(steps)}", True)
            await asyncio.to_thread(
                encode, src, out, src_h, target, dcrf, src_d,
                lambda pct, tail, l=label, i=idx, n=len(steps): hb.set(f"enc|{l}|{pct}|{i}|{n}|{tail}"),
                label)
            _, d = await asyncio.to_thread(probe, out)
            if abs(d - src_d) > 2.0:
                raise RuntimeError(f"{label}: davomiylik mos emas ({d:.1f} / {src_d:.1f} s)")
            hb.check()
            k = secrets.token_bytes(16)
            sealed = WORK / name
            await asyncio.to_thread(ctr_file, out, sealed, k)
            size = out.stat().st_size
            # Mini App uchun fMP4 nusxa (ilovaga tegishli emas) — MP4 dan
            # darhol, qayta siqmasdan. Xato bo'lsa qism baribir tayyor:
            # Mini App so'raganda worker uni alohida navbatga qo'yadi.
            frag = None
            try:
                frag_plain = WORK / f"{label}_f_plain.mp4"
                await asyncio.to_thread(make_fmp4, out, frag_plain)
                fk = secrets.token_bytes(16)
                frag = (WORK / name.replace(".mp4", "_f.mp4"), fk, frag_plain.stat().st_size)
                await asyncio.to_thread(ctr_file, frag_plain, frag[0], fk)
                frag_plain.unlink()
            except Exception as ex:  # noqa: BLE001
                log(f"  {label}: fMP4 tayyorlanmadi ({ex}) — keyin navbat orqali")
                frag = None
            out.unlink()
            STATUS.quality(label, state="upload", size_mb=round(size / 1048576, 1),
                           enc_s=int(time.time() - t),
                           kbps=round(size * 8 / src_d / 1000))
            STATUS.update(cur=None)
            log(f"  {label}: tayyor — {size / 1048576:.1f} MB, o'rtacha bitreyt "
                f"{size * 8 / src_d / 1000:.0f} kb/s, kodlash {hms(time.time() - t)} "
                f"— yuklanmoqda...")
            await asyncio.to_thread(hb.set, f"upload|{label}|{idx}|{len(steps)}", True)
            # Bir vaqtda faqat bitta sifat yuklaydi: navbat kutiladi (kodlash esa parallel davom etadi).
            r2_used = False
            if QUALITY and R2:
                qa = ident["queued_at"]
                r2_used = await upload_r2(ident, sealed, name, "mp4", k.hex(), label)
                if r2_used:
                    sealed.unlink(missing_ok=True)
                    if frag is not None:
                        fpath, fk, fsize = frag
                        try:
                            await upload_r2(ident, fpath, fpath.name, "fmp4", fk.hex(), f"{label} fMP4")
                        except Exception as ex:  # noqa: BLE001
                            log(f"  {label}: fMP4 R2'ga yozilmadi ({ex}) — keyin navbat orqali")
                        finally:
                            fpath.unlink(missing_ok=True)
            if not r2_used:
                await upload_turn(label)
            try:
                if r2_used:
                    pass
                elif QUALITY and BOT:
                    # Sifat-workflow: fayl KANALGA emas, kodlash BOTI chatiga ketadi; bot uni yopiq kanalga saqlab
                    # bazaga yozadi (`worker/src/encupload.rs`). Run tasdiqni kutadi va shundan keyin tugaydi.
                    qa = ident["queued_at"]
                    cap = f"ARUENC|mp4|{a}|{s}|{e}|{qa}|{label}|{k.hex()}|{size}"
                    await send_file(app, BOT, sealed, name, label, caption=cap)
                    sealed.unlink(missing_ok=True)
                    await wait_committed(name, label)
                    if frag is not None:
                        fpath, fk, fsize = frag
                        try:
                            fcap = f"ARUENC|fmp4|{a}|{s}|{e}|{qa}|{label}|{fk.hex()}|{fsize}"
                            await send_file(app, BOT, fpath, fpath.name, f"{label} fMP4", caption=fcap)
                            await wait_committed(fpath.name, f"{label} fMP4")
                            log(f"  {label}: fMP4 nusxa ham tayyor ({fsize / 1048576:.1f} MB)")
                        except Exception as ex:  # noqa: BLE001
                            log(f"  {label}: fMP4 saqlanmadi ({ex}) — keyin navbat orqali")
                        finally:
                            fpath.unlink(missing_ok=True)
                else:
                    # Kalit kanal postiga YOZILMAYDI (xavfsizlik) — faqat worker'ga.
                    sent = await send_file(app, channel, sealed, name, label)
                    sealed.unlink()
                    api("quality", {**ident, "quality": label, "file": name, "size": size,
                                    "key": k.hex(), "msg_id": sent.id})
                    if frag is not None:
                        fpath, fk, fsize = frag
                        try:
                            fsent = await send_file(app, channel, fpath, fpath.name, f"{label} fMP4")
                            api_fmp4("done", {"anime_id": ident.get("anime_id"), "season_id": ident.get("season_id"),
                                              "epizod_id": ident.get("epizod_id"), "quality": label, "ok": True,
                                              "file": fpath.name, "size": fsize, "key": fk.hex(),
                                              "msg_id": fsent.id})
                            log(f"  {label}: fMP4 nusxa ham tayyor ({fsize / 1048576:.1f} MB)")
                        except Exception as ex:  # noqa: BLE001
                            log(f"  {label}: fMP4 yuklanmadi ({ex}) — keyin navbat orqali")
                        finally:
                            fpath.unlink(missing_ok=True)
            finally:
                await asyncio.to_thread(ulock, "release")
            done.add(label)
            STATUS.quality(label, state="done")
            STATUS.update(xfer=None)
            log(f"  {label}: jurnalga yozildi")
            nxt = [l for l, _, _ in steps if l not in done]
            if ONE_QUALITY_PER_RUN and nxt:
                log(f"  keyingi sifat ({nxt[0]}) yangi run'da — bu run tugaydi")
                requeue = True
                break

        if requeue:
            # Ish xatosiz, urinish sanalmasdan navbatga qaytadi (tayyor sifatlar saqlanadi).
            api("finish", {**ident, "ok": False, "cancelled": True})
            STOP_RUN = True
        else:
            api("finish", {**ident, "ok": True})
            log("  tayyor")
            if QUALITY:
                # Sifat-workflow: har run BITTA sifatni tayyorlab o'chadi — navbat qolgan bo'lsa workflow
                # ("Davom ettirish" qadami) va worker YANGI run ochadi (toza mashina, disk va xotira).
                log("  bitta sifat tayyor — run tugaydi, keyingisi yangi run'da")
                STOP_RUN = True
    except JobLost:
        log("  ish boshqa run'ga o'tdi — to'xtatildi")
    except Fatal as ex:
        log("  XATO (qayta urinilmaydi):", ex)
        try:
            api("finish", {**ident, "ok": False, "fatal": True, "error": str(ex)})
        except Exception:
            pass
    except Exception as ex:
        log("  XATO:", ex)
        try:
            api("finish", {**ident, "ok": False, "error": str(ex)})
        except Exception:
            pass
    finally:
        CURRENT = None
        for _ in range(6):  # qolgan qatorlar to'liq ketsin
            await CHLOG.flush(app)
        hb.stop.set()
        STATUS.set_job("", "")
        STATUS.set_progress("idle")
        STATUS.update(job_started=None, attempt=None, src=None, ladder=None,
                      cur=None, xfer=None)
        await STATUS.push(app)
        await asyncio.to_thread(STATUS.send_worker)
        shutil.rmtree(WORK, ignore_errors=True)


async def main():
    # Sessiya qaysi ilova (api_id) bilan yaratilgan bo'lsa — o'sha bilan
    # ulanadi: ARUGRAM'ning `TG_API_ID` si boshqa bo'lishi mumkin
    # (masalan sessiya `anime` repodan olingan).
    api_id = session_api_id() or int(os.environ["TG_API_ID"])
    app = Client(SESSION, api_id=api_id,
                 api_hash=os.environ["TG_API_HASH"], no_updates=True)
    async with app:
        # Kanal ma'lum bo'lsin (Pyrogram peer keshida).
        async for _ in app.get_dialogs():
            pass
        # Kanalga log RUN BOSHLANISHI BILAN yoqiladi (qism kutilmaydi).
        CHLOG.start(f"\u25B6\uFE0F Run {RUNNER} boshlandi")
        log(f"kompyuter: {cpu_model()}, preset {PRESET}, CRF {CRF_BASE}")
        flusher = asyncio.create_task(CHLOG.loop(app))
        status_started = False
        status_task = None
        worker_task = None
        worked = False
        while True:
            if time.time() - T0 > START_BUDGET:
                log("Vaqt limiti yaqin — qolgan ishlar keyingi run'da.")
                break
            r = api("claim", {"runner": RUNNER, "no_heartbeat": True,
                              "log_chat": LOG_CHANNEL, **({"wq": QUALITY} if QUALITY else {})})
            if not status_started:
                status_started = True
                STATUS.bots = [b for b in (r.get("status_bots") or []) if isinstance(b, str) and b]
                await STATUS.start(app)
                status_task = asyncio.create_task(STATUS.loop(app))
                worker_task = asyncio.create_task(STATUS.worker_loop())
            if r.get("nosession"):
                log(f"{QUALITY} uchun Telegram sessiyasi ulanmagan — botda 'Yangi sessiya' orqali ulang. Run tugadi.")
                break
            global BOT, R2
            if QUALITY and r.get("bot"):
                BOT = str(r["bot"]).lstrip("@")
            R2 = bool(QUALITY and r.get("r2"))
            if r.get("busy"):
                log("Boshqa run ishlayapti — kutiladi.")
                break
            if r.get("wait"):
                log("Navbatdagi asl video hali kanalga ko'chirilmagan — keyinroq.")
                break
            job = r.get("job")
            if not job:
                log("Navbat bo'sh.")
                break
            await process(app, int(r["channel"]), job)
            worked = True
            if STOP_RUN:
                log("Keyingi sifat uchun yangi run ochiladi.")
                break
        log("Run tugadi.")
        STATUS.set_job("", "")
        STATUS.set_progress("idle")
        await STATUS.push(app)
        if status_task:
            status_task.cancel()
        if worker_task:
            worker_task.cancel()
        # Oxirgi holat ("Run tugadi", idle) darhol worker'ga ham.
        await asyncio.to_thread(STATUS.send_worker)
        for _ in range(6):
            await CHLOG.flush(app)
        CHLOG.stop()
        flusher.cancel()
        # Workflow'ning "Davom ettirish" qadami FAQAT ish bajarilgan bo'lsa
        # yangi run ochadi (aks holda "boshqa run ishlayapti" bilan tinmay
        # qisqa run'lar ochilaverardi).
        out = os.environ.get("GITHUB_OUTPUT")
        if out:
            with open(out, "a") as fh:
                fh.write(f"worked={'1' if worked else '0'}\n")


if __name__ == "__main__":
    signal.signal(signal.SIGINT, on_cancel)
    signal.signal(signal.SIGTERM, on_cancel)
    asyncio.run(main())
    sys.exit(0)
