#!/usr/bin/env python3
"""Siqish — kodlash botining "🗜 Siqish" bo'limi (`worker/src/cmpbot.rs`).

ALOHIDA workflow (`avtoencode` repoda `.github/workflows/compress.yml`, shablon — `tool/compress/compress.workflow.yml`).
Worker navbatda ish bo'lsa uni o'zi ishga tushiradi. Bitta run — bitta video.

Bitta ish:
  1. video yopiq kanaldan yuklab olinadi (bot ko'chirgan nusxa), asosiy akkaunt (pyrogram sessiyasi) bilan;
  2. ffmpeg: H.265 (libx265), CRF 30, qisqa tomoni 720 dan katta bo'lsa 720 gacha kichraytiriladi
     (kichik video kattalashtirilmaydi), OVOZSIZ (GIF), `hvc1` belgisi, faststart;
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
MAX_SHORT_SIDE = int(os.environ.get("MAX_SHORT_SIDE", "720"))


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


# Qisqa tomon MAX_SHORT_SIDE dan katta bo'lsa shunga tushiriladi (gorizontal -> balandlik 720, vertikal -> kenglik 720);
# kichik video kattalashtirilmaydi; o'lchamlar juft (x265 uchun).
VF = (f"scale=w='if(lt(iw,ih),min({MAX_SHORT_SIDE},iw),-2)':h='if(lt(iw,ih),-2,min({MAX_SHORT_SIDE},ih))',"
      "scale=trunc(iw/2)*2:trunc(ih/2)*2")


def encode(src: Path, dst: Path, duration: float, live: Live):
    cmd = ["ffmpeg", "-y", "-hide_banner", "-loglevel", "error", "-nostats", "-progress", "pipe:1",
           "-i", str(src), "-map", "0:v:0", "-vf", VF,
           "-c:v", "libx265", "-preset", PRESET, "-crf", CRF, "-tag:v", "hvc1", "-pix_fmt", "yuv420p",
           "-x265-params", "log-level=error",
           "-an", "-sn", "-dn",   # GIF (animatsiya) ovozsiz bo'ladi
           "-movflags", "+faststart", str(dst)]
    p = subprocess.Popen(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, bufsize=1)
    t0 = time.time()
    done_s = 0.0
    for line in p.stdout:
        m = re.match(r"out_time_(?:us|ms)=(\d+)", line)
        if m:
            # out_time_us (mikrosoniya); ba'zi versiyalarda out_time_ms ham mikrosoniya.
            done_s = int(m.group(1)) / 1_000_000
            pct = done_s * 100 / duration if duration else 0
            el = time.time() - t0
            eta = (el * (duration - done_s) / done_s) if done_s > 1 else 0
            live.send(f"🎞 Siqilmoqda\n{bar(pct)} {pct:.0f}%\no'tdi {hms(el)}" + (f" · qoldi ~{hms(eta)}" if eta else ""))
    err = p.stderr.read()
    if p.wait() != 0 or not dst.exists() or dst.stat().st_size == 0:
        raise RuntimeError(f"ffmpeg xato: {err.strip()[-300:]}")


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
        out = WORK / f"{stem[:80]}_720p_h265.mp4"
        t1 = time.time()
        live.send("🎞 Siqilmoqda...", force=True)
        await asyncio.to_thread(encode, src, out, dur, live)
        size = out.stat().st_size
        ow, oh, _ = probe(out)
        live.done(f"Siqildi: {mb(size)} · {ow}×{oh} ({hms(time.time() - t1)})")
        log(f"  tayyor: {mb(size)} {ow}x{oh} (manbaning {size * 100 / max(src_size, 1):.0f}%)")
        src.unlink(missing_ok=True)
        if size > 2_000_000_000:
            raise Fatal(f"siqilgan fayl {mb(size)} — Telegram 2 GB dan kattani yubormaydi (CRF ni oshirish kerak)")

        api("check", ident)   # admin shu orada o'chirgan bo'lsa — yuborilmaydi
        live.send("⬆️ GIF qilib Saqlangan xabarlarga yuborilmoqda...", force=True)
        th = thumb(out, WORK / "thumb.jpg", min(3.0, dur / 2 if dur else 0))

        def up_cb(cur, total):
            live.send(f"⬆️ Saqlangan xabarlarga yuborilmoqda\n{bar(cur * 100 / max(total, 1))} {mb(cur)} / {mb(total)}")

        await app.send_animation("me", str(out), caption=f"🗜 {name}"[:1024], duration=int(dur), width=ow, height=oh,
                                 thumb=str(th) if th else None, file_name=out.name, progress=up_cb)
        api("finish", {**ident, "ok": True, "size": size, "src_size": src_size})
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
