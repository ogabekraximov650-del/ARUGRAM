#!/usr/bin/env python3
"""aniraxuzbot: Anibla — BITTA run = BITTA to'liq qism (hamma sifat), sifatlar 360p dan boshlab ketma-ket.

Bot (`aniraxuzbot15/bot`) navbatga qo'yilgan ishlarni saqlaydi. Bu run (`anibla_bot.yml`, bir vaqtda faqat bitta) ishga tushganda:
  1. botdan `POST /anibla/claim` bilan navbatdagi ENG ESKI qismning barcha sifatlarini oladi (360p, 480p, 720p, 1080p tartibida);
  2. har sifatni birin-ketin: HLS ni qayta kodlamasdan mp4 ga yig'adi (`download.py`); 2 GB dan katta bo'lsa — QAYTA KODLAMASDAN aniq o'lchab
     qismlarga bo'ladi (`split_file`: har qism alohida ochiladi, `moov` boshida);
  3. tayyor faylni (qismni) R2'ga yozadi (`tool/upload/r2client.py`); R2 yuklovchisi (`upload.yml`, qo'shimcha akkauntlar, asosiy akkauntsiz)
     uni R2'dan olib BOTGA yuboradi (izoh: `ANIBLA:<ish>:<token>` yoki `ANIBLA:<ish>:<token>:<i>/<n>`); bot kanalga saqlab bazaga yozadi;
  4. sifat R2'ga to'liq yozilgach botga `POST /anibla/r2done` — keyingi sifatga o'tadi. Run hamma sifatdan keyin tugaydi.
Xato bo'lgan sifat `POST /anibla/fail` bilan botga xabar qilinadi va run keyingi sifatga o'tadi.

Muhit: API (bot worker manzili), RUN_KEY (= ENCODE_TOKEN), API_BASE (R2 worker), ENCODE_TOKEN.
"""
import base64
import json
import os
import shutil
import subprocess
import sys
import threading
import time
import urllib.request
from collections import deque
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent / "upload"))
import download as dl  # noqa: E402  (tez yuklash)
import r2client  # noqa: E402  (R2'ga yozish)

# Telegram oddiy akkaunt chegarasi 2000 MiB; zaxra bilan 2 000 000 000 bayt.
LIMIT = int(os.environ.get("MAX_BYTES", "2000000000"))
API = os.environ.get("API", "").strip().rstrip("/")
RUN_KEY = os.environ.get("RUN_KEY", "").strip()
API_BASE = os.environ.get("API_BASE", "").strip().rstrip("/")
ENCODE_TOKEN = "".join(os.environ.get("ENCODE_TOKEN", "").split())
LIVE_SEC = float(os.environ.get("LIVE_SEC", "5"))
LOG_LINES = int(os.environ.get("LOG_LINES", "5"))
RUNNER = f"anibla-{os.environ.get('GITHUB_RUN_ID', 'local')}"

log = dl.log


class Fatal(Exception):
    """Qayta urinishdan foyda yo'q."""


def bot_api(path, body, key=False):
    """Bot workeriga POST (JSON). `key` — claim uchun RUN_KEY sarlavhasi."""
    h = {"Content-Type": "application/json", "User-Agent": "aniraxuzbot-anibla"}
    if key:
        h["X-Run-Key"] = RUN_KEY
    req = urllib.request.Request(f"{API}{path}", method="POST", data=json.dumps(body).encode(), headers=h)
    with urllib.request.urlopen(req, timeout=30) as r:
        return json.loads(r.read() or b"{}")


class BotLive:
    """Botdagi "📊 Holat" uchun jonli holat (bitta sifat): bosqich, foiz, tezlik, oxirgi log qatorlari. Alohida oqim har `LIVE_SEC`
    soniyada `/anibla/progress` ga yuboradi (Telegram cheklasa — `retry_after`). Xatosi ishga tegmaydi."""

    def __init__(self, job, token):
        self.job, self.token = int(job), token
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
            r = bot_api("/anibla/progress", {"job": self.job, "token": self.token, "text": text})
            wait = int(r.get("retry_after") or 0)  # Telegram cheklasa — bot aytgan soniya kutiladi
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


def safe_name(job, token, i, n):
    """R2 fayl nomi: faqat [A-Za-z0-9._-], noyob."""
    return f"anibla_{job}_{token[:8]}_{i}of{n}_{int(time.time())}.mp4"


def make_thumb(f, thumb):
    for ss in ("10", "0"):  # qism 10 soniyadan qisqa bo'lsa
        subprocess.run(["ffmpeg", "-y", "-loglevel", "error", "-ss", ss, "-i", str(f), "-frames:v", "1",
                        "-vf", "scale='min(320,iw)':-2", "-q:v", "5", str(thumb)])
        if thumb.exists() and thumb.stat().st_size > 0:
            return


def process_job(j, bot, root):
    """Bitta sifat: yuklash -> (kerak bo'lsa) bo'lish -> har qismni R2'ga yozish -> botga `r2done`."""
    job, token, quality, name = int(j["job"]), j["token"], j["quality"], j["name"]
    live = BotLive(job, token)
    work = root / f"job_{job}"
    shutil.rmtree(work, ignore_errors=True)
    work.mkdir(parents=True)
    dl.WORK = work
    out, thumb = work / "video.mp4", work / "thumb.jpg"
    try:
        log(f"#{job}: {name} ({quality})")
        url = j["video"] + (("\n" + j["audio"]) if j.get("audio") else "")
        el = dl.download(url, out, live)
        size = out.stat().st_size
        w, h, dur = dl.ffprobe(out, "width", "v:0"), dl.ffprobe(out, "height", "v:0"), dl.ffprobe(out, "duration")
        log(f"yuklab olindi: {mb(size)} · {w or '?'}x{h or '?'} · {dl.hms(dur or 0)} · {dl.hms(el)} da")
        live.done(f"Yuklab olindi: {mb(size)} · {w or '?'}x{h or '?'} · {dl.hms(dur or 0)} · {dl.hms(el)} da")
        files = split_file(out, work, live) if size > LIMIT else [out]
        total = len(files)
        if total > 1:
            out.unlink(missing_ok=True)  # joy bo'shatiladi: faqat qismlar qoladi
            live.done(f"{total} qismga bo'lindi: " + " + ".join(mb(f.stat().st_size) for f in files))
        for i, f in enumerate(files, 1):
            tag = f":{i}/{total}" if total > 1 else ""
            make_thumb(f, thumb)
            fw, fh, fd = dl.ffprobe(f, "width", "v:0"), dl.ffprobe(f, "height", "v:0"), dl.ffprobe(f, "duration")
            fname = f"{name} ({i}-{total})" if total > 1 else name
            # `to` — R2 worker talabi (butun son); haqiqiy qabul qiluvchi `raw_bot` (yuklovchi qo'shimcha akkauntlar bilan yuboradi).
            dest = {"mode": "video", "to": 1, "raw_bot": bot, "caption": f"ANIBLA:{job}:{token}{tag}",
                    "file_name": f"{fname[:90]}.mp4", "supports_streaming": True, "disable_notification": True,
                    **{k: v for k, v in (("duration", fd), ("width", fw), ("height", fh)) if v}}
            if thumb.exists() and 0 < thumb.stat().st_size < 200_000:
                dest["thumb_b64"] = base64.b64encode(thumb.read_bytes()).decode()
            live.send(f"📦 R2'ga yozilmoqda{f' (qism {i}/{total})' if total > 1 else ''}: {mb(f.stat().st_size)}", force=True)
            t_up = time.time()
            ok = r2client.deliver(API_BASE, ENCODE_TOKEN, f, safe_name(job, token, i, total), dest, log, "aniraxuzbot-anibla")
            if not ok:
                raise RuntimeError("R2 ishlamadi (R2'ga yozib bo'lmadi)")
            live.done(f"R2'ga yozildi{f' · qism {i}/{total}' if total > 1 else ''} · {dl.hms(time.time() - t_up)}")
            f.unlink(missing_ok=True)
            thumb.unlink(missing_ok=True)
        bot_api("/anibla/r2done", {"job": job, "token": token, "parts": total})
        live.send("📤 R2'da — yuklovchi botga yuboradi", force=True)
        live.close()
    except Exception as e:  # noqa: BLE001
        log(f"#{job} XATO: {e}")
        live.send(f"❌ Xato: {str(e)[:200]}", force=True)
        live.close()
        try:
            bot_api("/anibla/fail", {"job": job, "token": token, "error": str(e).replace("\n", " ")[:250]})
        except Exception as e2:  # noqa: BLE001
            log(f"botga xato yuborilmadi: {e2}")
    finally:
        shutil.rmtree(work, ignore_errors=True)


def main():
    if not (API and RUN_KEY and API_BASE and ENCODE_TOKEN):
        sys.exit("API, RUN_KEY, API_BASE, ENCODE_TOKEN muhit o'zgaruvchilari kerak")
    r = bot_api("/anibla/claim", {"runner": RUNNER}, key=True)
    jobs = r.get("jobs") or []
    if not jobs:
        log("Navbat bo'sh (yoki boshqa run ishlayapti) — run tugadi.")
        return
    bot = str(r.get("bot") or "").lstrip("@")
    log(f"Qism: {jobs[0].get('name')} · sifatlar: " + ", ".join(x["quality"] for x in jobs))
    root = Path(os.environ.get("WORK_ROOT") or os.environ.get("RUNNER_TEMP", "/tmp")) / f"anibla_run_{os.environ.get('GITHUB_RUN_ID', 'x')}"
    shutil.rmtree(root, ignore_errors=True)
    root.mkdir(parents=True)
    for j in jobs:  # bot tartibi: 360p, 480p, 720p, 1080p
        process_job(j, bot, root)
    shutil.rmtree(root, ignore_errors=True)
    log("Run tugadi.")


if __name__ == "__main__":
    try:
        main()
    except Exception as e:  # noqa: BLE001
        log(f"XATO: {e}")
        sys.exit(1)
