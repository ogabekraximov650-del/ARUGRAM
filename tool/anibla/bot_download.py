#!/usr/bin/env python3
"""aniraxuzbot: anibla.uz videosini (BITTA sifat — BITTA run) yuklab, kerak bo'lsa qismlarga bo'lib, botga yuboradi.

Bot (`aniraxuzbot15/bot`, "➕ Yangi qism qo'shish" -> "🌐 Anibla orqali") har bir sifat uchun alohida `workflow_dispatch`
qiladi (`anibla_bot_<sifat>.yml`). Bu skript:
  1. HLS ni qayta kodlamasdan mp4 ga yig'adi (`download.py` dagi tez parallel yuklash);
  2. fayl 2 GB dan katta bo'lsa — QAYTA KODLAMASDAN ANIQ O'LCHANIB `LIMIT` dan oshmaydigan qismlarga bo'linadi (`split_file`):
     har qism alohida ochiladigan mp4, `moov` atomi boshida (faststart), kalit kadrdan kesiladi;
  3. Telegram akkaunti (sessiya) orqali videoni BOTGA yuboradi; izoh: `ANIBLA:<ish>:<token>` (qismlarga bo'lingan bo'lsa
     `ANIBLA:<ish>:<token>:<i>/<n>`). Bot uni yopiq kanalga saqlaydi, kanaldagi `file_id` ni (qismlar bo'lsa vergul bilan) bazaga yozadi.
Xato bo'lsa botga `ANIBLA_FAIL:<ish>:<token>:<sabab>` xabari yuboriladi (admin chatida ko'rinadi).

Muhit: JOB, TOKEN, BOT, QUALITY, VIDEO, AUDIO, NAME; TG_API_ID, TG_API_HASH; sessiya `tool/anibla/pyro_session.session`.
"""
import asyncio
import json
import os
import shutil
import subprocess
import sys
import threading
import time
import types
import urllib.request
from collections import deque
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import download as dl  # noqa: E402  (tez yuklash va Telegram'ga yuklash funksiyalari)

# Telegram oddiy akkaunt chegarasi 2000 MiB; zaxra bilan 2 000 000 000 bayt.
LIMIT = int(os.environ.get("MAX_BYTES", "2000000000"))

JOB = os.environ["JOB"].strip()
TOKEN = os.environ["TOKEN"].strip()
BOT = os.environ["BOT"].strip().lstrip("@")
QUALITY = os.environ.get("QUALITY", "").strip()
VIDEO = os.environ["VIDEO"].strip()
AUDIO = os.environ.get("AUDIO", "").strip()
NAME = os.environ.get("NAME", "").strip() or f"video_{JOB}"
# Botdagi "📊 Holat" tugmasi uchun jonli holat shu manzilga yuboriladi (`/anibla/progress`).
API = os.environ.get("API", "").strip().rstrip("/")
LIVE_SEC = float(os.environ.get("LIVE_SEC", "5"))
LOG_LINES = int(os.environ.get("LOG_LINES", "5"))

log = dl.log


class Fatal(Exception):
    """Qayta urinishdan foyda yo'q."""


class BotLive:
    """Botdagi "📊 Holat" uchun jonli holat: bosqich, foiz, tezlik, oxirgi log qatorlari. Alohida oqim har `LIVE_SEC` soniyada
    `/anibla/progress` ga yuboradi (`download.py` dagi `Live` kabi, lekin ARUMEDIATV workeriga emas, botga). Xatosi ishga tegmaydi."""

    def __init__(self):
        self.steps = []
        self.logs = deque(maxlen=LOG_LINES)
        self.line = "⏳ boshlanmoqda..."
        self.t0 = time.time()
        self.sent = ""
        self.pause_until = 0.0
        self.lock = threading.Lock()
        self.stop = threading.Event()
        dl.LIVE = self  # `download.py` log() shu yerga ham yozadi (add_log)
        if API:
            threading.Thread(target=self._loop, daemon=True).start()

    def text(self):
        with self.lock:
            parts = self.steps + [self.line]
            if self.logs:
                parts += ["", "📜 Log:"] + list(self.logs)
        parts += ["", f"⏱ jami: {dl.hms(time.time() - self.t0)}"]
        return "\n".join(parts)[-3000:]

    def add_log(self, line):
        line = line.strip()
        if line:
            with self.lock:
                self.logs.append(f"{time.strftime('%H:%M:%S')} {line[:140]}")

    def send(self, line, force=False):
        with self.lock:
            self.line = line
        if force:
            self._push()

    def done(self, line):
        with self.lock:
            self.steps.append("✅ " + line)
        self.add_log(line)

    def _push(self):
        if not API or time.time() < self.pause_until:
            return
        text = self.text()
        if text == self.sent:
            return
        try:
            req = urllib.request.Request(
                f"{API}/anibla/progress", method="POST",
                data=json.dumps({"job": int(JOB), "token": TOKEN, "text": text}).encode(),
                headers={"Content-Type": "application/json", "User-Agent": "aniraxuzbot-anibla"})
            r = json.loads(urllib.request.urlopen(req, timeout=20).read() or b"{}")
            wait = int(r.get("retry_after") or 0)  # Telegram cheklasa — bot aytgan soniya kutiladi (panel botdagidek)
            if wait:
                self.pause_until = time.time() + wait
            else:
                self.sent = text
        except Exception as e:  # noqa: BLE001
            print("progress:", e, flush=True)

    def _loop(self):
        while not self.stop.wait(LIVE_SEC):
            self._push()

    def close(self):
        self.stop.set()
        self._push()
        if dl.LIVE is self:
            dl.LIVE = None

    def transfer(self, kind):
        t0 = time.time()
        state = {"log": 0.0}

        def cb(cur, total):
            now = time.time()
            pct = cur * 100 / total if total else 0
            sp = cur / max(now - t0, 0.1) / 1048576
            eta = (total - cur) / 1048576 / sp if sp > 0 and total else 0
            self.send(f"{kind}\n{dl.bar(pct)} {pct:.1f}%\ntezlik {sp:.2f} MB/s\n"
                      f"hajm {cur / 1048576:.0f} / {total / 1048576:.0f} MB · qoldi ~{dl.hms(eta)}")
            if now - state["log"] >= 15 or cur >= total:
                state["log"] = now
                log(f"Telegram'ga {pct:.1f}% · {cur / 1048576:.0f}/{total / 1048576:.0f} MB · {sp:.2f} MB/s")
        return cb


def mb(n):
    return f"{n / 1048576:.0f} MB"


def keyframes(src):
    """Videodagi kalit kadrlar: [(vaqt_matn, vaqt, fayldagi_bayt_o'rni)], bayt o'rni bo'yicha tartiblangan."""
    out = subprocess.check_output(
        ["ffprobe", "-v", "error", "-select_streams", "v:0", "-show_entries", "packet=pts_time,pos,flags",
         "-of", "csv=p=0", str(src)]).decode()
    res = []
    for line in out.splitlines():
        f = line.strip().split(",")
        if len(f) >= 3 and f[2].startswith("K") and f[0] not in ("", "N/A") and f[1].isdigit():
            res.append((f[0], float(f[0]), int(f[1])))
    res.sort(key=lambda k: k[2])
    return res


def cut_points(kfs, total, limit):
    """Ochko'z bo'lish: har qism `limit` ga imkon qadar to'lib boradi, oxirgi qism — qoldiq
    (masalan 2.5 GB -> 2 GB + 0.5 GB). Kesish faqat kalit kadrda."""
    cuts, base = [], 0
    while total - base > limit:
        cand = [k for k in kfs if base < k[2] <= base + limit]
        if not cand:
            raise Fatal("kalit kadrlar topilmadi — bo'lib bo'lmaydi")
        cuts.append(cand[-1])
        base = cand[-1][2]
        if len(cuts) > 30:
            raise Fatal("30 dan ortiq qism — bo'lib bo'lmaydi")
    return cuts


def split_file(src, work, live=None):
    """`src` ni qayta kodlamasdan qismlarga bo'ladi: birinchi qism `LIMIT` ga imkon qadar yaqin (2.5 GB bo'lsa 2 GB + 0.5 GB),
    qoldiq oxirgi qismga tushadi. Har qism alohida ochiladigan mp4: `moov` atomi fayl boshida (`+faststart`), vaqt 0 dan.
    Hajm ANIQ o'lchanadi: biror qism `LIMIT` dan oshsa — zaxira kamaytirilib qayta bo'linadi."""
    size = src.stat().st_size
    kfs = keyframes(src)
    if not kfs:
        raise Fatal("kalit kadrlar topilmadi — bo'lib bo'lmaydi")
    for margin in (0.995, 0.985, 0.97, 0.94, 0.90, 0.85):
        cuts = cut_points(kfs, size, int(LIMIT * margin))
        for old in work.glob("part_*.mp4"):
            old.unlink()
        if live:
            live.send(f"✂️ {len(cuts) + 1} qismga bo'linmoqda (har biri ≤ {mb(LIMIT)})...", force=True)
        r = subprocess.run(
            ["ffmpeg", "-nostdin", "-y", "-hide_banner", "-loglevel", "error", "-i", str(src),
             "-map", "0:v:0", "-map", "0:a?", "-c", "copy", "-f", "segment",
             "-segment_times", ",".join(c[0] for c in cuts),
             "-reset_timestamps", "1", "-segment_format", "mp4",
             "-segment_format_options", "movflags=+faststart", str(work / "part_%03d.mp4")],
            capture_output=True, text=True)
        parts = sorted(work.glob("part_*.mp4"))
        if r.returncode != 0 or not parts:
            raise RuntimeError("ffmpeg (bo'lish) xatosi: " + (r.stderr.strip().splitlines() or ["?"])[-1][:200])
        sizes = [p.stat().st_size for p in parts]
        log(f"bo'lish (zaxira {margin}): {len(parts)} qism · " + ", ".join(mb(x) for x in sizes))
        if max(sizes) <= LIMIT:
            return parts
    raise Fatal(f"{mb(LIMIT)} dan oshmaydigan qismlarga bo'lib bo'lmadi")


async def run(app):
    work = Path(os.environ.get("WORK_ROOT") or os.environ.get("RUNNER_TEMP", "/tmp")) / f"anibla_bot_{JOB}"
    shutil.rmtree(work, ignore_errors=True)
    work.mkdir(parents=True)
    dl.WORK = work
    out = work / "video.mp4"
    thumb = work / "thumb.jpg"
    live = BotLive()
    try:
        log(f"#{JOB}: {NAME} ({QUALITY})")
        url = VIDEO + (("\n" + AUDIO) if AUDIO else "")
        el = await asyncio.to_thread(dl.download, url, out, live)
        size = out.stat().st_size
        w, h, dur = dl.ffprobe(out, "width", "v:0"), dl.ffprobe(out, "height", "v:0"), dl.ffprobe(out, "duration")
        log(f"yuklab olindi: {mb(size)} · {w or '?'}x{h or '?'} · {dl.hms(dur or 0)} · {dl.hms(el)} da")
        live.done(f"Yuklab olindi: {mb(size)} · {w or '?'}x{h or '?'} · {dl.hms(dur or 0)} · {dl.hms(el)} da")
        files = await asyncio.to_thread(split_file, out, work, live) if size > LIMIT else [out]
        total = len(files)
        if total > 1:
            out.unlink(missing_ok=True)  # joy bo'shatiladi: faqat qismlar qoladi
            live.done(f"{total} qismga bo'lindi: " + " + ".join(mb(f.stat().st_size) for f in files))
        for i, f in enumerate(files, 1):
            subprocess.run(["ffmpeg", "-y", "-loglevel", "error", "-ss", "10", "-i", str(f), "-frames:v", "1",
                            "-vf", "scale='min(320,iw)':-2", "-q:v", "5", str(thumb)])
            if not thumb.exists() or thumb.stat().st_size == 0:  # qism 10 soniyadan qisqa bo'lsa
                subprocess.run(["ffmpeg", "-y", "-loglevel", "error", "-i", str(f), "-frames:v", "1",
                                "-vf", "scale='min(320,iw)':-2", "-q:v", "5", str(thumb)])
            tag = f":{i}/{total}" if total > 1 else ""
            fname = f"{NAME} ({i}-{total})" if total > 1 else NAME
            job = types.SimpleNamespace(channel=BOT, caption=f"ANIBLA:{JOB}:{TOKEN}{tag}", file_name=fname[:90] or f"video_{JOB}")
            log(f"botga yuboriladi: @{BOT} ({mb(f.stat().st_size)}){f' · qism {i}/{total}' if total > 1 else ''}")
            t_up = time.time()
            await dl.upload(app, job, f, thumb, live)
            live.done(f"Botga yuborildi{f' · qism {i}/{total}' if total > 1 else ''} · {dl.hms(time.time() - t_up)}")
            thumb.unlink(missing_ok=True)
        live.send("✅ Tayyor — bot kanalga saqlaydi", force=True)
        live.close()
        shutil.rmtree(work, ignore_errors=True)
    except Exception as e:  # noqa: BLE001
        live.send(f"❌ Xato: {str(e)[:200]}", force=True)
        live.close()
        raise


async def main():
    import pyrogram.utils
    from pyrogram import Client
    pyrogram.utils.MIN_CHANNEL_ID = -1009999999999
    api_id = dl.session_api_id() or int(os.environ["TG_API_ID"])
    app = Client(dl.SESSION, api_id=api_id, api_hash=os.environ["TG_API_HASH"], no_updates=True)
    async with app:
        try:
            await run(app)
        except Exception as e:  # noqa: BLE001
            log(f"XATO: {e}")
            try:
                reason = str(e).replace("\n", " ")[:250]
                await app.send_message(BOT, f"ANIBLA_FAIL:{JOB}:{TOKEN}:{reason}")
            except Exception as e2:  # noqa: BLE001
                log(f"botga xabar yuborilmadi: {e2}")
            raise


if __name__ == "__main__":
    try:
        asyncio.run(main())
    except Exception:
        sys.exit(1)
