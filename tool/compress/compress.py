#!/usr/bin/env python3
"""Siqish — kodlash botining "🗜 Siqish" bo'limi (`worker/src/cmpbot.rs`).

ALOHIDA workflow (`avtoencode` repoda `.github/workflows/compress.yml`, shablon — `tool/compress/compress.workflow.yml`).
Worker navbatda ish bo'lsa uni o'zi ishga tushiradi. Bitta run — bitta video.

Bitta ish:
  1. video yopiq kanaldan yuklab olinadi (bot ko'chirgan nusxa), asosiy akkaunt (pyrogram sessiyasi) bilan;
  2. ffmpeg: H.265 (libx265), CRF HAR DOIM 30 (bitrate cheklanmaydi). Hajm TARGET_MB (5 MB) dan oshsa, faqat o'lcham
     kamaytiriladi: 1080p -> 1080, ... juft o'lchamlar bo'yicha hajm sig'guncha qayta-qayta kodlanadi (taxmin + qisqartirish);
     eng katta sig'adigan o'lcham topiladi. OVOZSIZ (GIF), `hvc1`. Butun ish ko'pi bilan JOB_MAX_MIN (60) daqiqa;
  3. tayyor fayl GIF (animatsiya, `send_animation`) qilib asosiy akkauntning "Saqlangan xabarlar"iga (Saved Messages, `me`) yuboriladi;
  4. `/api/cmp/finish` — worker kanal nusxasini va ish yozuvini o'chiradi.
Jarayon botdagi bitta xabarda jonli ko'rinadi (`/api/cmp/progress`).
"""

import asyncio
import collections
import json
import os
import re
import shutil
import signal
import subprocess
import sys
import time
import urllib.error
import urllib.request
from pathlib import Path

HERE = Path(__file__).resolve().parent
WORK = Path(os.environ.get("WORK_ROOT", os.environ.get("RUNNER_TEMP", "/tmp"))) / "arumediatv_compress"
CRF = os.environ.get("H265_CRF", "30")
PRESET = os.environ.get("H265_PRESET", "medium")
MAX_SHORT_SIDE = int(os.environ.get("MAX_SHORT_SIDE", "1080"))   # qidiruv shu (yoki video o'zining) qisqa tomonidan boshlanadi
MIN_SHORT_SIDE = int(os.environ.get("MIN_SHORT_SIDE", "144"))
# GIF hajmi chegarasi (MB): CRF HAR DOIM bir xil (30), bitrate cheklanmaydi — faqat o'lcham (qisqa tomon) kamaytiriladi.
TARGET_MB = float(os.environ.get("TARGET_MB", "5"))
# Butun ish (yuklash + qayta-qayta kodlash + yuborish) uchun maksimal vaqt.
JOB_MAX_MIN = float(os.environ.get("JOB_MAX_MIN", "60"))
SEND_RESERVE = 150   # yuborish uchun zaxira soniya


class Lost(Exception):
    """Ish o'chirildi yoki boshqa run'ga o'tdi."""


class Fatal(Exception):
    """Qayta urinishdan foyda yo'q."""


def log(*a):
    print(time.strftime("%H:%M:%S"), *a, flush=True)


# ── Worker bilan aloqa ───────────────────────────────────────

def _post(path, body, timeout=60, tries=5):
    base = os.environ["API_BASE"].rstrip("/")
    data = json.dumps(body).encode()
    err = None
    for attempt in range(tries):
        req = urllib.request.Request(
            f"{base}{path}", data=data, method="POST",
            headers={"X-Encode-Token": os.environ["ENCODE_TOKEN"],
                     "Content-Type": "application/json", "User-Agent": "arumediatv-compress"})
        try:
            with urllib.request.urlopen(req, timeout=timeout) as r:
                return json.loads(r.read() or b"{}")
        except urllib.error.HTTPError as e:
            if e.code == 409:
                raise Lost()
            if e.code in (400, 401, 403, 404):
                raise RuntimeError(f"{path}: HTTP {e.code}")
            err = e
        except Exception as e:  # tarmoq
            err = e
        time.sleep(3 * (attempt + 1))
    raise RuntimeError(f"{path}: {err}")


def api(path, body):
    return _post(f"/api/cmp/{path}", body)


def claim(runner):
    try:
        r = api("claim", {"runner": runner})
    except Exception as e:  # noqa: BLE001
        print("cmp/claim:", e, flush=True)
        return None
    return r if r.get("job") and r.get("channel") else None


# ── Jonli holat ──────────────────────────────────────────────

def hms(sec):
    sec = max(0, int(sec))
    h, m, s = sec // 3600, sec % 3600 // 60, sec % 60
    return f"{h}:{m:02}:{s:02}" if h else f"{m:02}:{s:02}"


def bar(pct, width=12):
    full = int(round(max(0.0, min(100.0, pct)) / 100 * width))
    return "▓" * full + "░" * (width - full)


def mb(n):
    return f"{n / 1048576:.1f} MB"


class Live:
    """Botdagi jonli holat xabari (`/api/cmp/progress`). Har 10 soniyada bittadan ko'p yuborilmaydi."""

    def __init__(self, r):
        job = r["job"]
        self.base = {"id": int(job["id"]), "name": str(job.get("name") or ""), "chat": int(job["chat"]),
                     "attempt": int(job.get("attempt") or 1), "status_msg": int(r.get("status_msg") or 0)}
        self.steps = []
        self.last = 0.0
        self.t0 = time.time()

    def done(self, line):
        self.steps.append("✅ " + line)

    def send(self, line, force=False):
        now = time.time()
        if not self.base["status_msg"] or (not force and now - self.last < 10):
            return
        self.last = now
        text = "\n".join(self.steps + [line, "", f"⏱ jami: {hms(now - self.t0)}"])
        try:
            api("progress", {**self.base, "text": text})
        except Exception as e:  # noqa: BLE001
            print("cmp/progress:", e, flush=True)


# ── ffmpeg ───────────────────────────────────────────────────

def probe(path):
    out = subprocess.run(
        ["ffprobe", "-v", "error", "-select_streams", "v:0", "-show_entries", "stream=width,height:format=duration",
         "-of", "json", str(path)], capture_output=True, text=True)
    try:
        j = json.loads(out.stdout)
        st = j["streams"][0]
        return int(st["width"]), int(st["height"]), float(j["format"].get("duration") or 0)
    except Exception as e:  # noqa: BLE001
        raise Fatal(f"fayl video emas yoki o'qib bo'lmadi: {out.stderr.strip()[:150] or e}")


def even(n):
    return max(2, int(n) // 2 * 2)


def encode_once(src: Path, dst: Path, duration: float, short: int, portrait: bool, live: Live, label: str):
    """H.265, CRF o'zgarmas; faqat qisqa tomon `short` px (juft), ikkinchi tomon nisbat bo'yicha (-2). Ovozsiz (GIF)."""
    vf = f"scale={short}:-2" if portrait else f"scale=-2:{short}"
    cmd = ["ffmpeg", "-y", "-hide_banner", "-loglevel", "error", "-nostats", "-progress", "pipe:1",
           "-i", str(src), "-map", "0:v:0", "-vf", vf,
           "-c:v", "libx265", "-preset", PRESET, "-crf", CRF, "-tag:v", "hvc1", "-pix_fmt", "yuv420p",
           "-x265-params", "log-level=error", "-an", "-sn", "-dn", "-movflags", "+faststart", str(dst)]
    p = subprocess.Popen(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, bufsize=1)
    t0 = time.time()
    for line in p.stdout:
        m = re.match(r"out_time_us=(\d+)", line)
        if m:
            done_s = int(m.group(1)) / 1_000_000
            pct = done_s * 100 / duration if duration else 0
            el = time.time() - t0
            eta = (el * (duration - done_s) / done_s) if done_s > 1 else 0
            live.send(f"🎞 {label}\n{bar(pct)} {pct:.0f}%\no'tdi {hms(el)}" + (f" · qoldi ~{hms(eta)}" if eta else ""))
    err = p.stderr.read()
    if p.wait() != 0 or not dst.exists() or dst.stat().st_size == 0:
        raise RuntimeError(f"ffmpeg xato: {err.strip()[-300:]}")
    return dst.stat().st_size, time.time() - t0


def search_size(src: Path, workdir: Path, w: int, h: int, duration: float, live: Live, deadline: float):
    """CRF doim bir xil; hajm TARGET_MB dan kichik bo'lguncha qisqa tomon kamaytirib qayta kodlanadi.

    1080p -> 1080, so'ng kichikroq... (x265 uchun o'lcham juft bo'lishi kerak, shuning uchun qadam kamida 2 px).
    Har urinishdan keyin keyingi o'lcham hajm ~ o'lcham^n qonuni bo'yicha taxmin qilinadi (n o'lchangan natijalardan),
    oxirida sig'adigan eng katta o'lcham topiladi (qo'shni 2 px gacha). Natija: dict.
    """
    import math
    limit = TARGET_MB * 1024 * 1024 * 0.995 if TARGET_MB > 0 else 0
    portrait = h > w
    s0 = even(min(min(w, h), MAX_SHORT_SIDE))
    tries = []          # (short, size, sec)
    best = None         # sig'gan eng katta o'lchamli natija: (short, size, path)
    hi = None           # sig'magan eng kichik o'lcham: (short, size)
    s = s0
    t_last = None       # oxirgi kodlash vaqti va o'lchami (vaqt bashorati uchun)
    while True:
        # Vaqt bashorati: kodlash vaqti ~ o'lcham^2.
        if t_last:
            est = t_last[1] * (s / t_last[0]) ** 2
            if time.time() + est > deadline:
                live.send(f"⏱ Vaqt chegarasi: keyingi urinish ({s}px, ~{hms(est)}) sig'maydi — to'xtatildi", force=True)
                break
        out = workdir / f"enc_{s}.mp4"
        n = len(tries) + 1
        label = f"Siqilmoqda — {n}-urinish: qisqa tomon {s}px, CRF {CRF}"
        live.send(label, force=True)
        size, sec = encode_once(src, out, duration, s, portrait, live, label)
        tries.append((s, size, round(sec)))
        t_last = (s, sec)
        log(f"  urinish {n}: {s}px → {mb(size)} ({hms(sec)})" + ("" if not limit else f" [chegara {mb(limit)}]"))
        fits = (not limit) or size <= limit
        if fits:
            if best is None or s > best[0]:
                if best:
                    best[2].unlink(missing_ok=True)
                best = (s, size, out)
            else:
                out.unlink(missing_ok=True)
            if hi is None:
                break                      # birinchi urinishdayoq sig'di — to'liq o'lcham
        else:
            if hi is None or s < hi[0]:
                hi = (s, size)
            out.unlink(missing_ok=True)
        lo_s = best[0] if best else None
        hi_s = hi[0]
        if lo_s is not None and hi_s - lo_s <= 2:
            break                          # eng katta sig'adigan o'lcham topildi
        # Keyingi o'lcham.
        if lo_s is not None:
            lo_size = best[1]
            # Ikki chegara ma'lum: hajm bo'yicha chiziqli interpolyatsiya (regula falsi), oraliqning o'rta qismida cheklanadi —
            # hajm o'lchamga silliq bog'liq emas (x265 bloklari), shuning uchun himoya: oraliq har urinishda ~yarmiga qisqaradi.
            span = hi_s - lo_s
            frac = (limit * 0.99 - lo_size) / (hi[1] - lo_size) if hi[1] > lo_size else 0.5
            frac = max(0.3, min(frac, 0.7))
            nxt = even(lo_s + frac * span)
            nxt = max(lo_s + 2, min(nxt, hi_s - 2))
            if nxt <= lo_s or nxt >= hi_s:
                break
        else:
            n_exp = 2.0
            if len(tries) >= 2:
                a_, b_ = sorted([(t[0], t[1]) for t in tries], key=lambda x: x[0])[:2]
                if b_[1] > a_[1] and b_[0] > a_[0]:
                    n_exp = max(1.0, min(math.log(b_[1] / a_[1]) / math.log(b_[0] / a_[0]), 4.0))
            guess = hi_s * (limit * 0.97 / hi[1]) ** (1 / n_exp)
            nxt = even(min(guess, hi_s - 2))
            if nxt < MIN_SHORT_SIDE:
                if hi_s <= MIN_SHORT_SIDE:
                    break                  # eng kichik o'lchamda ham sig'madi
                nxt = even(MIN_SHORT_SIDE)
        s = nxt
    return {"best": best, "tries": tries, "hi": hi, "short0": s0, "portrait": portrait}


def thumb(src: Path, dst: Path, at: float):
    subprocess.run(["ffmpeg", "-y", "-loglevel", "error", "-ss", f"{at:.1f}", "-i", str(src), "-frames:v", "1",
                    "-vf", "scale=320:-2", "-q:v", "4", str(dst)], capture_output=True)
    return dst if dst.exists() and dst.stat().st_size > 0 else None


# ── Ish ──────────────────────────────────────────────────────

async def process(app, r, runner):
    channel = int(r["channel"])
    job = r["job"]
    jid, name = int(job["id"]), str(job.get("name") or "video")
    live = Live(r)
    ident = {"runner": runner, "id": jid, "status_msg": live.base["status_msg"]}
    shutil.rmtree(WORK, ignore_errors=True)
    WORK.mkdir(parents=True)
    try:
        log(f"🗜 Siqish #{jid}: {name} (urinish {job.get('attempt')})")
        live.send("⬇️ Video yuklab olinmoqda...", force=True)
        msg = await app.get_messages(channel, int(job["ch_msg"]))
        if not msg or msg.empty or not (msg.video or msg.document):
            raise Fatal("video kanalda topilmadi")
        t0 = time.time()
        t_job0 = t0

        def dl_cb(cur, total):
            live.send(f"⬇️ Video yuklab olinmoqda\n{bar(cur * 100 / max(total, 1))} {mb(cur)} / {mb(total)}")

        src = await app.download_media(msg, file_name=str(WORK / "src.bin"), progress=dl_cb)
        if not src or Path(src).stat().st_size == 0:
            raise RuntimeError("video yuklab olinmadi")
        src = Path(src)
        src_size = src.stat().st_size
        w, h, dur = probe(src)
        live.done(f"Yuklab olindi: {mb(src_size)} · {w}×{h} · {hms(dur)} ({hms(time.time() - t0)})")
        log(f"  manba: {mb(src_size)} {w}x{h} {dur:.0f}s")

        stem = re.sub(r'[\\/:*?"<>|]+', "_", Path(name).stem).strip() or f"video_{jid}"
        t1 = time.time()
        deadline = t_job0 + JOB_MAX_MIN * 60 - SEND_RESERVE
        res = await asyncio.to_thread(search_size, src, WORK, w, h, dur, live, deadline)
        tries = res["tries"]
        if not res["best"]:
            if res["hi"]:
                raise Fatal(f"{TARGET_MB:g} MB ga sig'madi: eng kichik natija {mb(res['hi'][1])} ({res['hi'][0]}px, CRF {CRF}); "
                            f"{len(tries)} urinish, {hms(time.time() - t1)}. Video juda uzun yoki vaqt (1 soat) tugadi")
            raise Fatal("kodlab bo'lmadi (urinish bo'lmadi)")
        short, size, enc_path = res["best"]
        out = WORK / f"{stem[:80]}_{short}p_h265.mp4"
        enc_path.rename(out)
        ow, oh, _ = probe(out)
        secs = int(time.time() - t1)
        tries_txt = ", ".join(f"{t[0]}px→{t[1] / 1048576:.1f}MB" for t in tries)
        summary = (f"{w}×{h} → {ow}×{oh} · CRF {CRF} · {mb(size)} · {len(tries)} marta kodlandi ({hms(secs)})")
        live.done(f"Siqildi: {summary}")
        log(f"  tayyor: {summary}\n  urinishlar: {tries_txt}")
        src.unlink(missing_ok=True)
        if size > 2_000_000_000:
            raise Fatal(f"siqilgan fayl {mb(size)} — Telegram 2 GB dan kattani yubormaydi")

        api("check", ident)   # admin shu orada o'chirgan bo'lsa — yuborilmaydi
        live.send("⬆️ GIF qilib Saqlangan xabarlarga yuborilmoqda...", force=True)
        th = thumb(out, WORK / "thumb.jpg", min(3.0, dur / 2 if dur else 0))

        def up_cb(cur, total):
            live.send(f"⬆️ Saqlangan xabarlarga yuborilmoqda\n{bar(cur * 100 / max(total, 1))} {mb(cur)} / {mb(total)}")

        await app.send_animation("me", str(out), caption=f"🗜 {name}\n{summary}"[:1024], duration=int(dur), width=ow, height=oh,
                                 thumb=str(th) if th else None, file_name=out.name, progress=up_cb)
        api("finish", {**ident, "ok": True, "size": size, "src_size": src_size, "summary": summary, "tries": tries_txt})
        log(f"  ✅ siqish #{jid} yuborildi")
    except Lost:
        log(f"  siqish #{jid} o'chirildi yoki boshqa run'ga o'tdi — yuborilmadi")
    except Fatal as ex:
        log(f"  siqish #{jid} XATO (qayta urinilmaydi): {ex}")
        try:
            api("finish", {**ident, "ok": False, "fatal": True, "error": str(ex)})
        except Exception:  # noqa: BLE001
            pass
    except Exception as ex:  # noqa: BLE001
        log(f"  siqish #{jid} XATO: {type(ex).__name__}: {ex}")
        try:
            api("finish", {**ident, "ok": False, "error": f"{type(ex).__name__}: {ex}"[:300]})
        except Exception:  # noqa: BLE001
            pass
    finally:
        shutil.rmtree(WORK, ignore_errors=True)


SESSION = str(HERE / "pyro_session")
RUNNER = f"{os.environ.get('GITHUB_RUN_ID', 'local')}-{os.environ.get('GITHUB_RUN_ATTEMPT', '1')}"
CURRENT = None


def session_api_id():
    try:
        import sqlite3
        c = sqlite3.connect(f"file:{SESSION}.session?mode=ro", uri=True)
        return int(c.execute("SELECT api_id FROM sessions").fetchone()[0] or 0)
    except Exception:  # noqa: BLE001
        return 0


def on_cancel(signum, frame):
    """Run bekor qilindi — ish navbatga qaytadi."""
    if CURRENT:
        try:
            api("finish", {"runner": RUNNER, "id": CURRENT, "ok": False, "cancelled": True})
        except Exception:  # noqa: BLE001
            pass
        print("Run bekor qilindi — ish navbatga qaytarildi", flush=True)
    os._exit(0)


async def main():
    global CURRENT
    import pyrogram.utils
    from pyrogram import Client
    pyrogram.utils.MIN_CHANNEL_ID = -1009999999999
    api_id = session_api_id() or int(os.environ["TG_API_ID"])
    app = Client(SESSION, api_id=api_id, api_hash=os.environ["TG_API_HASH"], no_updates=True)
    done = 0
    async with app:
        async for _ in app.get_dialogs():  # kanal peer keshida bo'lsin
            pass
        r = claim(RUNNER)
        if r:
            CURRENT = int(r["job"]["id"])
            try:
                await process(app, r, RUNNER)
                done += 1
            finally:
                CURRENT = None
    log(f"Run tugadi: {done} ta ish.")
    out = os.environ.get("GITHUB_OUTPUT")
    if out and done:
        with open(out, "a") as f:
            f.write("worked=1\n")


if __name__ == "__main__":
    signal.signal(signal.SIGINT, on_cancel)
    signal.signal(signal.SIGTERM, on_cancel)
    asyncio.run(main())
    sys.exit(0)
