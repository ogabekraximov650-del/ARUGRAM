#!/usr/bin/env python3
"""aniraxuzbot: anibla.uz videosini (BITTA sifat — BITTA run) yuklab, kerak bo'lsa H.265 bilan siqib, botga yuboradi.

Bot (`aniraxuzbot15/bot`, "➕ Yangi qism qo'shish" -> "🌐 Anibla orqali") har bir sifat uchun alohida `workflow_dispatch`
qiladi (`anibla_bot.yml`). Bu skript:
  1. HLS ni qayta kodlamasdan mp4 ga yig'adi (`download.py` dagi tez parallel yuklash);
  2. fayl 2 GB dan katta bo'lsa — H.265 (libx265) CRF 30 bilan kodlaydi; sig'masa CRF 31, 32, 33, ... ketma-ket (har safar
     ASL fayldan) — hajm chegaraga tushguncha;
  3. Telegram akkaunti (sessiya) orqali videoni BOTGA yuboradi; izoh: `ANIBLA:<ish>:<token>`. Bot uni yopiq kanalga saqlaydi,
     kanaldagi `file_id` ni olib bazaga yozadi.
Xato bo'lsa botga `ANIBLA_FAIL:<ish>:<token>:<sabab>` xabari yuboriladi (admin chatida ko'rinadi).

Muhit: JOB, TOKEN, BOT, QUALITY, VIDEO, AUDIO, NAME; TG_API_ID, TG_API_HASH; sessiya `tool/anibla/pyro_session.session`.
"""
import asyncio
import os
import shutil
import subprocess
import sys
import time
import types
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import download as dl  # noqa: E402  (tez yuklash va Telegram'ga yuklash funksiyalari)

# Telegram oddiy akkaunt chegarasi 2000 MiB; zaxra bilan 2 000 000 000 bayt.
LIMIT = int(os.environ.get("MAX_BYTES", "2000000000"))
CRF_START = int(os.environ.get("CRF_START", "30"))
CRF_MAX = int(os.environ.get("CRF_MAX", "45"))
PRESET = os.environ.get("X265_PRESET", "fast")
# Actions 6 soatlik chegarasi: shundan keyin yangi kodlash urinishi boshlanmaydi.
BUDGET = int(os.environ.get("BUDGET_MIN", "300")) * 60
T0 = time.time()

JOB = os.environ["JOB"].strip()
TOKEN = os.environ["TOKEN"].strip()
BOT = os.environ["BOT"].strip().lstrip("@")
QUALITY = os.environ.get("QUALITY", "").strip()
VIDEO = os.environ["VIDEO"].strip()
AUDIO = os.environ.get("AUDIO", "").strip()
NAME = os.environ.get("NAME", "").strip() or f"video_{JOB}"

log = dl.log


class Fatal(Exception):
    """Qayta urinishdan foyda yo'q."""


class Quiet:
    """`download.py` dagi `Live` o'rni: botdagi jonli xabar yo'q, faqat log."""

    def send(self, line, force=False):
        pass

    def done(self, line):
        log(line)

    def close(self):
        pass

    def transfer(self, kind):
        state = {"t": 0.0}

        def cb(cur, total):
            now = time.time()
            if now - state["t"] >= 15 or cur >= total:
                state["t"] = now
                log(f"Telegram'ga {cur * 100 / max(total, 1):.1f}% · {cur / 1048576:.0f}/{total / 1048576:.0f} MB")
        return cb


def mb(n):
    return f"{n / 1048576:.0f} MB"


def encode(src, dst, crf, dur):
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
            log(f"  H.265 CRF {crf}: {pct:.1f}% · o'tdi {dl.hms(el)} · qoldi ~{dl.hms(eta)}")
    err = p.stderr.read()
    if p.wait() != 0 or not Path(dst).exists() or Path(dst).stat().st_size == 0:
        raise RuntimeError("ffmpeg (H.265) xatosi: " + (err.strip().splitlines() or ["?"])[-1][:200])


def fit(src, work):
    """Fayl `LIMIT` dan katta bo'lsa H.265 CRF 30 -> 31 -> 32 ... bilan sig'dirib, yangi yo'lni qaytaradi."""
    size = src.stat().st_size
    if size <= LIMIT:
        return src
    dur = dl.ffprobe(src, "duration") or 0
    log(f"fayl {mb(size)} > {mb(LIMIT)} — H.265 bilan siqiladi (CRF {CRF_START} dan boshlab, sig'masa +1)")
    crf = CRF_START
    while crf <= CRF_MAX:
        if time.time() - T0 > BUDGET:
            raise Fatal(f"vaqt tugadi (Actions chegarasi): CRF {crf} urinilmadi")
        dst = work / f"enc_crf{crf}.mp4"
        t1 = time.time()
        encode(src, dst, crf, dur)
        s = dst.stat().st_size
        log(f"  CRF {crf}: {mb(s)} ({dl.hms(time.time() - t1)} da)")
        if s <= LIMIT:
            return dst
        dst.unlink(missing_ok=True)
        crf += 1
    raise Fatal(f"CRF {CRF_MAX} da ham {mb(LIMIT)} dan kichik bo'lmadi")


async def run(app):
    work = Path(os.environ.get("RUNNER_TEMP", "/tmp")) / f"anibla_bot_{JOB}"
    shutil.rmtree(work, ignore_errors=True)
    work.mkdir(parents=True)
    dl.WORK = work
    out = work / "video.mp4"
    thumb = work / "thumb.jpg"
    live = Quiet()
    log(f"#{JOB}: {NAME} ({QUALITY})")
    url = VIDEO + (("\n" + AUDIO) if AUDIO else "")
    el = await asyncio.to_thread(dl.download, url, out, live)
    size = out.stat().st_size
    w, h, dur = dl.ffprobe(out, "width", "v:0"), dl.ffprobe(out, "height", "v:0"), dl.ffprobe(out, "duration")
    log(f"yuklab olindi: {mb(size)} · {w or '?'}x{h or '?'} · {dl.hms(dur or 0)} · {dl.hms(el)} da")
    final = await asyncio.to_thread(fit, out, work)
    subprocess.run(["ffmpeg", "-y", "-loglevel", "error", "-ss", "10", "-i", str(final), "-frames:v", "1",
                    "-vf", "scale='min(320,iw)':-2", "-q:v", "5", str(thumb)])
    job = types.SimpleNamespace(channel=BOT, caption=f"ANIBLA:{JOB}:{TOKEN}", file_name=NAME[:90] or f"video_{JOB}")
    log(f"botga yuboriladi: @{BOT} ({mb(final.stat().st_size)})")
    t_up = time.time()
    await dl.upload(app, job, final, thumb, live)
    log(f"✅ yuborildi · {dl.hms(time.time() - t_up)}")
    shutil.rmtree(work, ignore_errors=True)


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
