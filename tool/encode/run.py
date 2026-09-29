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
import signal
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
    return pct, line


def encode(src: Path, dst: Path, src_h: int, target: int, dcrf: int,
           dur: float = 0.0, on_progress=None, label: str = ""):
    vf = [] if target >= src_h else ["-vf", f"scale=-2:{target}:flags=lanczos"]
    abr = "128k" if target >= 720 else "96k"
    cmd = ["ffmpeg", "-hide_banner", "-loglevel", "error", "-nostats",
           "-progress", "pipe:1", "-y", "-i", str(src),
           "-map", "0:v:0", "-map", "0:a:0?", "-map_metadata", "-1", "-sn", *vf,
           "-c:v", "libx265", "-preset", PRESET, "-crf", str(CRF_BASE - dcrf),
           "-x265-params", "log-level=error", "-pix_fmt", "yuv420p", "-tag:v", "hvc1",
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
        pct, text = r
        if pct != last and on_progress:
            last = pct
            on_progress(pct)
        if time.time() - last_log >= 1.0:
            last_log = time.time()
            print(time.strftime("%H:%M:%S"), text, flush=True)
    if p.wait() != 0:
        raise subprocess.CalledProcessError(p.returncode, "ffmpeg")


class Heartbeat:
    def __init__(self, ident):
        self.ident = ident
        self.stop = threading.Event()
        self.lost = False
        # Botning "Holat" xabari uchun: `download`, `enc|1080p|37|1|4`,
        # `upload|720p|2|4`.
        self.progress = ""
        self.t = threading.Thread(target=self.run, daemon=True)
        self.t.start()

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

    def run(self):
        while not self.stop.wait(120):
            if not self.ping():
                return

    def set(self, progress, now=False):
        self.progress = progress
        if now:
            self.ping()

    def check(self):
        if self.lost:
            raise JobLost()


async def process(app: Client, channel: int, job: dict):
    a, s, e, qa = job["anime_id"], job["season_id"], job["epizod_id"], job["queued_at"]
    ident = {"runner": RUNNER, "anime_id": a, "season_id": s, "epizod_id": e, "queued_at": qa}
    global CURRENT
    CURRENT = ident
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
        await asyncio.to_thread(hb.set, "download", True)
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
        src_mb = src.stat().st_size / 1048576
        log(f"  manba {src_h}p, {src_d:.0f} s, {src_mb:.1f} MB, "
            f"bitreyt ~{src.stat().st_size * 8 / src_d / 1000:.0f} kb/s -> "
            f"{', '.join(x[0] for x in steps)}")
        log(f"  kompyuter: {os.cpu_count()} yadro, preset {PRESET}, CRF {CRF_BASE}")

        for idx, (label, target, dcrf) in enumerate(steps, 1):
            hb.check()
            if label in done:
                log(f"  {label}: tayyor — o'tkazib yuborildi")
                continue
            name = f"ep_{a}_{s}_{e}_{label}_{qa}.mp4"
            out = WORK / f"{label}.mp4"
            log(f"  {label}: kodlanmoqda...")
            t = time.time()
            # Alohida oqimda — Telegram ulanishi (ping) uzilib qolmasin.
            await asyncio.to_thread(hb.set, f"enc|{label}|0|{idx}|{len(steps)}", True)
            await asyncio.to_thread(
                encode, src, out, src_h, target, dcrf, src_d,
                lambda pct, l=label, i=idx, n=len(steps): hb.set(f"enc|{l}|{pct}|{i}|{n}"),
                label)
            _, d = await asyncio.to_thread(probe, out)
            if abs(d - src_d) > 2.0:
                raise RuntimeError(f"{label}: davomiylik mos emas ({d:.1f} / {src_d:.1f} s)")
            hb.check()
            k = secrets.token_bytes(16)
            sealed = WORK / name
            await asyncio.to_thread(ctr_file, out, sealed, k)
            size = out.stat().st_size
            out.unlink()
            log(f"  {label}: tayyor — {size / 1048576:.1f} MB, o'rtacha bitreyt "
                f"{size * 8 / src_d / 1000:.0f} kb/s, kodlash {hms(time.time() - t)} "
                f"— yuklanmoqda...")
            await asyncio.to_thread(hb.set, f"upload|{label}|{idx}|{len(steps)}", True)
            # Kalit kanal postiga YOZILMAYDI (xavfsizlik) — faqat worker'ga.
            sent = await app.send_document(
                channel, str(sealed), file_name=name, force_document=True,
                caption=name, disable_notification=True)
            sealed.unlink()
            api("quality", {**ident, "quality": label, "file": name, "size": size,
                            "key": k.hex(), "msg_id": sent.id})
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
        CURRENT = None
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
        worked = False
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
            worked = True
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
