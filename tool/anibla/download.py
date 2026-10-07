#!/usr/bin/env python3
"""anibla.uz videolarini yuklab olish — kodlash botining "Anibla yuklash" bo'limi
(`worker/src/anibla.rs`).

ALOHIDA workflow (`avtoencode` repoda `.github/workflows/anibla.yml`, shablon —
`tool/anibla/anibla.workflow.yml`). Sifat tugmasi bosilganda video bazadagi
navbatga (`anibla_jobs`) tushadi va worker shu workflow'ni ishga tushiradi.
Run navbat bo'shaguncha videolarni KETMA-KET oladi (`/api/anibla/claim`).

Bitta video:
  1. ffmpeg HLS bo'laklarini qayta kodlamasdan (`-c copy`) bitta mp4 ga yig'adi;
  2. Telegram sessiyasi bilan yopiq kanalga video bo'lib yuklanadi;
  3. `/api/anibla/done` — kodlash boti videoni BOT CHATIGA ko'chiradi va
     kanal postini o'chiradi, navbat yozuvi o'chadi.
Jarayon botdagi holat xabarida jonli ko'rinadi (`/api/anibla/progress`).
"""

import asyncio
import json
import os
import re
import shutil
import subprocess
import sys
import time
import urllib.error
import urllib.request
from pathlib import Path

HERE = Path(__file__).resolve().parent
WORK = Path(os.environ.get("RUNNER_TEMP", "/tmp")) / "anibla"
SESSION = str(HERE / "pyro_session")
UA = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0 Safari/537.36"
# Telegram oddiy akkaunt uchun fayl chegarasi — 2 GB.
MAX_BYTES = 2000 * 1048576
RUNNER = f"{os.environ.get('GITHUB_RUN_ID', 'local')}-{os.environ.get('GITHUB_RUN_ATTEMPT', '1')}"
# Shu vaqtdan keyin yangi video olinmaydi (Actions limiti 6 soat).
START_BUDGET = int(os.environ.get("START_BUDGET_MIN", "280")) * 60
T0 = time.time()
CURRENT = None


class Lost(Exception):
    """Video navbatdan olib tashlandi yoki boshqa run'ga o'tdi."""


class Fatal(Exception):
    """Qayta urinishdan foyda yo'q."""


class Job:
    def __init__(self, r):
        j = r["job"]
        self.id = int(j["id"])
        self.url = str(j.get("url") or "").strip()
        self.caption = str(j.get("caption") or "").strip()
        name = re.sub(r'[\\/:*?"<>|]', "", str(j.get("file_name") or "")).strip()[:90]
        self.file_name = name or f"video_{self.id}"
        self.status = int(j.get("status_msg") or 0)
        self.chat = int(j.get("chat") or 0)
        self.attempt = int(j.get("attempt") or 1)
        self.channel = int(r.get("channel") or 0)

    def ident(self):
        return {"runner": RUNNER, "id": self.id, "status_msg": self.status, "chat": self.chat}


def log(*a):
    print(time.strftime("%H:%M:%S"), *a, flush=True)


def api(path, body):
    base = os.environ["API_BASE"].rstrip("/")
    data = json.dumps(body).encode()
    err = None
    for attempt in range(4):
        req = urllib.request.Request(
            f"{base}/api/anibla/{path}", data=data, method="POST",
            headers={"X-Encode-Token": os.environ["ENCODE_TOKEN"],
                     "Content-Type": "application/json", "User-Agent": "arugram-anibla"})
        try:
            with urllib.request.urlopen(req, timeout=60) as r:
                return json.loads(r.read() or b"{}")
        except urllib.error.HTTPError as e:
            if e.code == 409:
                raise Lost()
            if e.code in (400, 401, 403, 404):
                raise RuntimeError(f"anibla/{path}: HTTP {e.code}")
            err = e
        except Exception as e:  # tarmoq
            err = e
        time.sleep(3 * (attempt + 1))
    raise RuntimeError(f"anibla/{path}: {err}")


def hms(sec):
    sec = max(0, int(sec))
    h, m, s = sec // 3600, sec % 3600 // 60, sec % 60
    return f"{h}:{m:02}:{s:02}" if h else f"{m:02}:{s:02}"


def bar(pct, width=12):
    full = int(round(max(0.0, min(100.0, pct)) / 100 * width))
    return "▓" * full + "░" * (width - full)


class Live:
    """Botdagi holat xabari: bosqich, foiz, tezlik, hajm, qolgan vaqt.
    Har 10 soniyada bittadan ko'p yuborilmaydi; xatosi ishga tegmaydi."""

    def __init__(self, job):
        self.job = job
        self.head = "⬇️ " + job.caption + (f"\n(urinish {job.attempt})" if job.attempt > 1 else "")
        self.steps = []
        self.last = 0.0
        self.t0 = time.time()

    def text(self, line):
        return "\n".join([self.head, ""] + self.steps + [line, "", f"⏱ jami: {hms(time.time() - self.t0)}"])

    def done(self, line):
        self.steps.append("✅ " + line)

    def send(self, line, force=False):
        now = time.time()
        if not self.job.status or (not force and now - self.last < 10):
            return
        self.last = now
        try:
            api("progress", {**self.job.ident(), "text": self.text(line)})
        except Exception as e:  # noqa: BLE001
            log("progress:", e)

    def transfer(self, kind):
        t0 = time.time()

        def cb(cur, total):
            now = time.time()
            pct = cur * 100 / total if total else 0
            sp = cur / max(now - t0, 0.1) / 1048576
            eta = (total - cur) / 1048576 / sp if sp > 0 and total else 0
            self.send(f"{kind}\n{bar(pct)} {pct:.1f}%\n"
                      f"{cur / 1048576:.1f} / {total / 1048576:.1f} MB · {sp:.2f} MB/s · "
                      f"qoldi ~{hms(eta)}", force=cur >= total)
        return cb


def playlist_duration(url):
    """Variant playlist'idagi bo'laklar uzunligi (soniya)."""
    try:
        req = urllib.request.Request(url, headers={"User-Agent": UA})
        with urllib.request.urlopen(req, timeout=60) as r:
            text = r.read().decode(errors="replace")
        return sum(float(m) for m in re.findall(r"#EXTINF:([\d.]+)", text))
    except Exception as e:  # noqa: BLE001
        log("playlist:", e)
        return 0.0


def ffprobe(path, entry, stream=None):
    cmd = ["ffprobe", "-v", "error"]
    if stream:
        cmd += ["-select_streams", stream, "-show_entries", f"stream={entry}"]
    else:
        cmd += ["-show_entries", f"format={entry}"]
    cmd += ["-of", "csv=p=0", str(path)]
    try:
        v = subprocess.check_output(cmd, stderr=subprocess.DEVNULL).decode().split("\n")[0].strip()
        n = int(float(v))
        return n if n > 0 else None
    except Exception:
        return None


def download(url, out, live):
    """HLS -> mp4 (qayta kodlamasdan). ffmpeg `-progress` dan foiz."""
    total = playlist_duration(url)
    log(f"  yuklanmoqda: {url} ({hms(total)})")
    cmd = ["ffmpeg", "-nostdin", "-y", "-loglevel", "error", "-progress", "pipe:1",
           "-user_agent", UA, "-reconnect", "1", "-reconnect_streamed", "1", "-reconnect_delay_max", "10",
           "-i", url, "-map", "0:v:0?", "-map", "0:a?", "-c", "copy",
           "-bsf:a", "aac_adtstoasc", "-movflags", "+faststart", str(out)]
    p = subprocess.Popen(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, bufsize=1)
    t0 = time.time()
    cur_t = size = 0
    for line in p.stdout:
        k, _, v = line.strip().partition("=")
        if k == "out_time_us" and v.isdigit():
            cur_t = int(v) / 1e6
        elif k == "total_size" and v.isdigit():
            size = int(v)
        elif k == "progress":
            el = time.time() - t0
            pct = cur_t * 100 / total if total else 0
            eta = el * (100 - pct) / pct if pct > 0.5 else 0
            sp = size / max(el, 0.1) / 1048576
            est = f" (~{size / 1048576 * 100 / pct:.0f} MB bo'ladi)" if pct > 3 else ""
            live.send(f"⬇️ Saytdan yuklab olinmoqda\n{bar(pct)} {pct:.1f}%\n"
                      f"{size / 1048576:.1f} MB{est} · {sp:.2f} MB/s\n"
                      f"o'tdi {hms(el)} · qoldi ~{hms(eta)}")
    err = p.stderr.read()
    code = p.wait()
    if code != 0 or not out.exists() or out.stat().st_size == 0:
        raise RuntimeError("ffmpeg xatosi: " + (err.strip().splitlines() or ["?"])[-1][:200])
    return time.time() - t0


async def upload(app, job, out, thumb, live):
    extra = {}
    for k, v in (("duration", ffprobe(out, "duration")),
                 ("width", ffprobe(out, "width", "v:0")),
                 ("height", ffprobe(out, "height", "v:0"))):
        if v:
            extra[k] = v
    if thumb.exists() and thumb.stat().st_size > 0:
        extra["thumb"] = str(thumb)
    m = await app.send_video(
        job.channel, str(out), caption=job.caption[:1024], file_name=f"{job.file_name}.mp4",
        supports_streaming=True, disable_notification=True,
        progress=live.transfer("⬆️ Telegram'ga yuklanmoqda"), **extra)
    return m.id


def session_api_id():
    """Sessiya faylidagi api_id (sessiya qaysi ilova bilan yaratilgan bo'lsa)."""
    try:
        import sqlite3
        c = sqlite3.connect(f"file:{SESSION}.session?mode=ro", uri=True)
        return int(c.execute("SELECT api_id FROM sessions").fetchone()[0] or 0)
    except Exception:
        return 0


def claim():
    """Navbatdagi video (yo'q bo'lsa — None)."""
    try:
        r = api("claim", {"runner": RUNNER})
    except Exception as e:  # noqa: BLE001
        log("claim:", e)
        return None
    return Job(r) if r.get("job") else None


async def process(app, job):
    live = Live(job)
    shutil.rmtree(WORK, ignore_errors=True)
    WORK.mkdir(parents=True)
    out = WORK / "video.mp4"
    thumb = WORK / "thumb.jpg"
    log(f"#{job.id}: {job.caption.replace(chr(10), ' | ')} (urinish {job.attempt})")
    try:
        if not job.channel:
            raise Fatal("yopiq kanal (TG_CHANNEL_ID) worker'da sozlanmagan")
        if not job.url:
            raise Fatal("video manzili yo'q")
        live.send("⏳ boshlanmoqda...", force=True)
        el = await asyncio.to_thread(download, job.url, out, live)
        size = out.stat().st_size
        live.done(f"Yuklab olindi: {size / 1048576:.1f} MB · {hms(el)}")
        log(f"  yuklab olindi: {size / 1048576:.1f} MB")
        if size > MAX_BYTES:
            raise Fatal(f"fayl {size / 1048576:.0f} MB — Telegram chegarasi 2 GB. Pastroq sifatni tanlang.")
        subprocess.run(["ffmpeg", "-y", "-loglevel", "error", "-ss", "10", "-i", str(out), "-frames:v", "1",
                        "-vf", "scale='min(320,iw)':-2", "-q:v", "5", str(thumb)])
        live.send("⬆️ Telegram'ga yuklanmoqda...", force=True)
        msg = await upload(app, job, out, thumb, live)
        log(f"  kanalga yuklandi: xabar {msg}")
        try:
            api("done", {**job.ident(), "ok": True, "channel_msg": msg,
                         "text": live.text(f"✅ Tayyor ({size / 1048576:.1f} MB)")})
        except Lost:
            # Shu orada navbatdan olib tashlangan — kanaldagi nusxa ham kerak emas.
            await app.delete_messages(job.channel, msg)
            raise
        log(f"  ✅ #{job.id} yuborildi")
    except Lost:
        log(f"  #{job.id} navbatdan olib tashlandi yoki boshqa run'ga o'tdi")
    except Exception as ex:  # noqa: BLE001
        log(f"  #{job.id} XATO: {ex}")
        try:
            api("done", {**job.ident(), "ok": False, "fatal": isinstance(ex, Fatal),
                         "error": str(ex)[:300], "text": live.text("")})
        except Exception:
            pass
    finally:
        shutil.rmtree(WORK, ignore_errors=True)


def on_cancel(signum, frame):
    """Run bekor qilindi — yuklanayotgan video navbatga qaytadi."""
    if CURRENT:
        try:
            api("done", {**CURRENT.ident(), "ok": False, "cancelled": True})
        except Exception:
            pass
        print("Run bekor qilindi — video navbatga qaytarildi", flush=True)
    os._exit(0)


async def main():
    global CURRENT
    import pyrogram.utils
    from pyrogram import Client
    # Pyrogram 2.0.106: yangi kanallar raqami eski chegaradan kichik ("Peer id invalid").
    pyrogram.utils.MIN_CHANNEL_ID = -1009999999999
    api_id = session_api_id() or int(os.environ["TG_API_ID"])
    app = Client(SESSION, api_id=api_id, api_hash=os.environ["TG_API_HASH"], no_updates=True)
    done = 0
    async with app:
        # Kanal Pyrogram peer keshida bo'lsin.
        async for _ in app.get_dialogs():
            pass
        while time.time() - T0 < START_BUDGET:
            job = claim()
            if not job:
                break
            CURRENT = job
            try:
                await process(app, job)
                done += 1
            finally:
                CURRENT = None
    log(f"Run tugadi: {done} ta video.")


if __name__ == "__main__":
    import signal
    signal.signal(signal.SIGINT, on_cancel)
    signal.signal(signal.SIGTERM, on_cancel)
    asyncio.run(main())
    sys.exit(0)
