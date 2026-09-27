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

XAVFSIZLIK:
  * tayyor sifat (`done`) QAYTA kodlanmaydi; yuklangan-u jurnalga yozilmay
    qolgani (`uploaded`) faqat jurnalga yoziladi;
  * kodlangan faylning davomiyligi manbaga mos kelmasa — yuklanmaydi;
  * har 4 daqiqada `heartbeat`; ish boshqa run'ga o'tgan bo'lsa (409) —
    darhol to'xtaydi, hech narsa yozmaydi;
  * vaqt limiti yaqin bo'lsa yangi ish olinmaydi (qolgani keyingi run'da).
"""

import asyncio
import json
import os
import secrets
import shutil
import subprocess
import sys
import threading
import time
import urllib.error
import urllib.request
from pathlib import Path

import pyrogram.utils
from cryptography.hazmat.primitives.ciphers import Cipher, algorithms, modes
from pyrogram import Client

# Pyrogram 2.0.106: yangi kanallarning raqami eski chegaradan kichik —
# aks holda "Peer id invalid" (ma'lum xato, shu yamoq bilan tuzaladi).
pyrogram.utils.MIN_CHANNEL_ID = -1009999999999

API = os.environ["API_BASE"].rstrip("/")
TOKEN = os.environ["ENCODE_TOKEN"]
RUNNER = f"{os.environ.get('GITHUB_RUN_ID', 'local')}-{os.environ.get('GITHUB_RUN_ATTEMPT', '1')}"
CRF_BASE = int(os.environ.get("H265_CRF", "30"))
PRESET = os.environ.get("H265_PRESET", "medium")
# Shu vaqtdan keyin YANGI ish olinmaydi (Actions limiti 6 soat).
START_BUDGET = int(os.environ.get("START_BUDGET_MIN", "240")) * 60
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


class JobLost(Exception):
    """Ish boshqa run'ga o'tdi yoki qayta navbatga qo'yildi."""


class Fatal(Exception):
    """Qayta urinishning foydasi yo'q (buzuq manba va h.k.)."""


def log(*a):
    print(time.strftime("%H:%M:%S"), *a, flush=True)


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


def encode(src: Path, dst: Path, src_h: int, target: int, dcrf: int):
    vf = [] if target >= src_h else ["-vf", f"scale=-2:{target}:flags=lanczos"]
    abr = "128k" if target >= 720 else "96k"
    cmd = ["ffmpeg", "-hide_banner", "-loglevel", "error", "-y", "-i", str(src),
           "-map", "0:v:0", "-map", "0:a:0?", "-map_metadata", "-1", "-sn", *vf,
           "-c:v", "libx265", "-preset", PRESET, "-crf", str(CRF_BASE - dcrf),
           "-x265-params", "log-level=error", "-pix_fmt", "yuv420p", "-tag:v", "hvc1",
           "-c:a", "aac", "-ac", "2", "-b:a", abr, "-ar", "44100",
           "-movflags", "+faststart", str(dst)]
    subprocess.run(cmd, check=True)


class Heartbeat:
    def __init__(self, ident):
        self.ident = ident
        self.stop = threading.Event()
        self.lost = False
        self.t = threading.Thread(target=self.run, daemon=True)
        self.t.start()

    def run(self):
        while not self.stop.wait(240):
            try:
                api("heartbeat", self.ident)
            except JobLost:
                self.lost = True
                return
            except Exception as e:
                log("heartbeat xato:", e)

    def check(self):
        if self.lost:
            raise JobLost()


async def process(app: Client, channel: int, job: dict):
    a, s, e, qa = job["anime_id"], job["season_id"], job["epizod_id"], job["queued_at"]
    ident = {"runner": RUNNER, "anime_id": a, "season_id": s, "epizod_id": e, "queued_at": qa}
    hb = Heartbeat(ident)
    shutil.rmtree(WORK, ignore_errors=True)
    WORK.mkdir(parents=True)
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
        m = await app.get_messages(channel, int(job["origin_msg"]))
        if not m or m.empty or not (m.document or m.video):
            raise Fatal("asl video kanalda topilmadi")
        log("  asl video yuklab olinmoqda...")
        got = await app.download_media(m, file_name=str(enc))
        if not got or Path(got).stat().st_size == 0:
            raise RuntimeError("asl video yuklab olinmadi")
        key = (job.get("origin_key") or "").strip()
        if key:
            await asyncio.to_thread(ctr_file, Path(got), src, bytes.fromhex(key))
            Path(got).unlink()
        else:
            Path(got).rename(src)
        hb.check()
        src_h, src_d = await asyncio.to_thread(probe, src)
        steps = plan(src_h)
        log(f"  manba {src_h}p, {src_d:.0f} s -> {', '.join(x[0] for x in steps)}")

        for label, target, dcrf in steps:
            hb.check()
            if label in done:
                log(f"  {label}: tayyor — o'tkazib yuborildi")
                continue
            name = f"ep_{a}_{s}_{e}_{label}_{qa}.mp4"
            out = WORK / f"{label}.mp4"
            log(f"  {label}: kodlanmoqda...")
            t = time.time()
            # Alohida oqimda — Telegram ulanishi (ping) uzilib qolmasin.
            await asyncio.to_thread(encode, src, out, src_h, target, dcrf)
            _, d = await asyncio.to_thread(probe, out)
            if abs(d - src_d) > 2.0:
                raise RuntimeError(f"{label}: davomiylik mos emas ({d:.1f} / {src_d:.1f} s)")
            hb.check()
            k = secrets.token_bytes(16)
            sealed = WORK / name
            await asyncio.to_thread(ctr_file, out, sealed, k)
            size = out.stat().st_size
            out.unlink()
            log(f"  {label}: {size / 1048576:.1f} MB, {time.time() - t:.0f} s — yuklanmoqda...")
            await app.send_document(
                channel, str(sealed), file_name=name, force_document=True,
                caption=f"{name}\nkey:{k.hex()}", disable_notification=True)
            sealed.unlink()
            api("quality", {**ident, "quality": label, "file": name, "size": size, "key": k.hex()})
            done.add(label)
            log(f"  {label}: jurnalga yozildi")

        api("finish", {**ident, "ok": True})
        log("  tayyor")
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
        hb.stop.set()
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
        while True:
            if time.time() - T0 > START_BUDGET:
                log("Vaqt limiti yaqin — qolgan ishlar keyingi run'da.")
                break
            r = api("claim", {"runner": RUNNER})
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


if __name__ == "__main__":
    asyncio.run(main())
    sys.exit(0)
