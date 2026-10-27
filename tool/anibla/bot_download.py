#!/usr/bin/env python3
"""aniraxuzbot: anibla.uz videosini (BITTA sifat — BITTA run) yuklab, kerak bo'lsa H.265 bilan siqib, botga yuboradi.

Bot (`aniraxuzbot15/bot`, "➕ Yangi qism qo'shish" -> "🌐 Anibla orqali") har bir sifat uchun alohida `workflow_dispatch`
qiladi (`anibla_bot.yml`). Bu skript:
  1. HLS ni qayta kodlamasdan mp4 ga yig'adi (`download.py` dagi tez parallel yuklash);
  2. fayl 2 GB dan katta bo'lsa — H.265 (libx265) CRF 30 bilan kodlaydi; sig'masa CRF 31, 32, 33, ... ketma-ket (har safar
     ASL fayldan) — hajm chegaraga tushguncha. CRF ham, vaqt ham yetmasa — eng kichik natija ANIQ O'LCHANIB `LIMIT` dan oshmaydigan
     qismlarga bo'linadi (`split_file`): har qism alohida ochiladigan mp4, `moov` atomi boshida (faststart), kalit kadrdan kesiladi;
  3. Telegram akkaunti (sessiya) orqali videoni BOTGA yuboradi; izoh: `ANIBLA:<ish>:<token>` (qismlarga bo'lingan bo'lsa
     `ANIBLA:<ish>:<token>:<i>/<n>`). Bot uni yopiq kanalga saqlaydi, kanaldagi `file_id` ni (qismlar bo'lsa vergul bilan) bazaga yozadi.
Xato bo'lsa botga `ANIBLA_FAIL:<ish>:<token>:<sabab>` xabari yuboriladi (admin chatida ko'rinadi).

Muhit: JOB, TOKEN, BOT, QUALITY, VIDEO, AUDIO, NAME; TG_API_ID, TG_API_HASH; sessiya `tool/anibla/pyro_session.session`.
"""
import asyncio
import json
import math
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
CRF_START = int(os.environ.get("CRF_START", "30"))
CRF_MAX = int(os.environ.get("CRF_MAX", "45"))
PRESET = os.environ.get("X265_PRESET", "medium")
# Actions 6 soatlik chegarasi: shundan keyin yangi kodlash urinishi boshlanmaydi.
BUDGET = int(os.environ.get("BUDGET_MIN", "240")) * 60  # qolgan vaqt (6 soatgacha) qismlarga bo'lish va yuklashga ketadi
T0 = time.time()

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


def encode(src, dst, crf, dur, live=None):
    """H.265 (libx265), ovoz o'zgarishsiz. Foiz har 30 soniyada logga chiqadi."""
    cmd = ["ffmpeg", "-nostdin", "-y", "-hide_banner", "-loglevel", "error", "-progress", "pipe:1", "-stats_period", "30",
           "-i", str(src), "-map", "0:v:0", "-map", "0:a?", "-c:v", "libx265", "-preset", PRESET, "-crf", str(crf),
           "-pix_fmt", "yuv420p", "-tag:v", "hvc1", "-x265-params", "log-level=error", "-c:a", "copy",
           "-movflags", "+faststart", str(dst)]
    p = subprocess.Popen(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, bufsize=1)
    t0 = time.time()
    for line in p.stdout:
        k, _, v = line.strip().partition("=")
        if k == "out_time_us" and v.isdigit() and dur:
            pct = int(v) / 1e6 * 100 / dur
            el = time.time() - t0
            eta = el * (100 - pct) / pct if pct > 0.5 else 0
            msg = f"H.265 CRF {crf}: {pct:.1f}% · o'tdi {dl.hms(el)} · qoldi ~{dl.hms(eta)}"
            if live:
                live.send(f"🎞 {msg}\n{dl.bar(pct)}")
            log("  " + msg)
    err = p.stderr.read()
    if p.wait() != 0 or not Path(dst).exists() or Path(dst).stat().st_size == 0:
        raise RuntimeError("ffmpeg (H.265) xatosi: " + (err.strip().splitlines() or ["?"])[-1][:200])


def fit(src, work, live=None):
    """Fayl `LIMIT` dan katta bo'lsa H.265 CRF 30 -> 31 -> 32 ... bilan sig'dirishga urinadi.
    Qaytaradi: (yo'l, bo'lish_kerak). CRF ham, vaqt ham yetmasa — eng kichik natija va bo'lish_kerak=True."""
    size = src.stat().st_size
    if size <= LIMIT:
        return src, False
    dur = dl.ffprobe(src, "duration") or 0
    log(f"fayl {mb(size)} > {mb(LIMIT)} — H.265 bilan siqiladi (CRF {CRF_START} dan boshlab, sig'masa +1)")
    crf = CRF_START
    best = src
    while crf <= CRF_MAX:
        if time.time() - T0 > BUDGET:
            log(f"vaqt chegarasi: CRF {crf} urinilmadi — natija qismlarga bo'linadi")
            return best, True
        dst = work / f"enc_crf{crf}.mp4"
        t1 = time.time()
        if live:
            live.send(f"🎞 H.265 CRF {crf} kodlanmoqda...", force=True)
        encode(src, dst, crf, dur, live)
        s = dst.stat().st_size
        log(f"  CRF {crf}: {mb(s)} ({dl.hms(time.time() - t1)} da)")
        if s <= LIMIT:
            return dst, False
        if best != src:
            best.unlink(missing_ok=True)
        best = dst
        crf += 1
    log(f"CRF {CRF_MAX} da ham {mb(LIMIT)} dan kichik bo'lmadi — natija qismlarga bo'linadi")
    return best, True


def split_file(src, work, live=None):
    """`src` ni har biri `LIMIT` dan oshmaydigan qismlarga bo'ladi (qayta kodlamasdan, kalit kadrlardan).
    Har qism alohida ochiladigan mp4: `moov` atomi fayl boshida (`+faststart`), vaqt 0 dan boshlanadi.
    Hajm ANIQ o'lchanadi: biror qism `LIMIT` dan oshsa — qism soni oshirilib qayta bo'linadi."""
    size = src.stat().st_size
    dur = dl.ffprobe(src, "duration") or 0
    if not dur:
        raise Fatal("video davomiyligi aniqlanmadi — bo'lib bo'lmaydi")
    safe = LIMIT * 0.97  # kalit kadrlar tufayli qismlar hajmi biroz farq qiladi
    n = max(2, math.ceil(size / safe))
    for _ in range(25):
        for old in work.glob("part_*.mp4"):
            old.unlink()
        seg = dur / n
        if live:
            live.send(f"✂️ {n} qismga bo'linmoqda (har biri ≤ {mb(LIMIT)})...", force=True)
        r = subprocess.run(
            ["ffmpeg", "-nostdin", "-y", "-hide_banner", "-loglevel", "error", "-i", str(src),
             "-map", "0:v:0", "-map", "0:a?", "-c", "copy", "-f", "segment", "-segment_time", f"{seg:.3f}",
             "-reset_timestamps", "1", "-segment_format", "mp4",
             "-segment_format_options", "movflags=+faststart", str(work / "part_%03d.mp4")],
            capture_output=True, text=True)
        parts = sorted(work.glob("part_*.mp4"))
        if r.returncode != 0 or not parts:
            raise RuntimeError("ffmpeg (bo'lish) xatosi: " + (r.stderr.strip().splitlines() or ["?"])[-1][:200])
        sizes = [p.stat().st_size for p in parts]
        log(f"bo'lish: {len(parts)} qism · " + ", ".join(mb(x) for x in sizes))
        biggest = max(sizes)
        if biggest <= LIMIT:
            return parts
        n = max(len(parts) + 1, math.ceil(len(parts) * biggest / safe))
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
        final, need_split = await asyncio.to_thread(fit, out, work, live)
        files = await asyncio.to_thread(split_file, final, work, live) if need_split else [final]
        total = len(files)
        if total > 1:
            for big in {out, final}:  # joy bo'shatiladi: faqat qismlar qoladi
                big.unlink(missing_ok=True)
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
