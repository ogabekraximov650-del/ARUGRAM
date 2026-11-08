#!/usr/bin/env python3
"""Dublyaj — kodlash botining "🗣 Dublyaj" bo'limi (`worker/src/dubbot.rs`).

ALOHIDA workflow (`avtoencode` repoda `.github/workflows/dub.yml`, shablon — `tool/dub/dub.workflow.yml`).
Worker navbatda ish bo'lsa uni o'zi ishga tushiradi. Bitta run — bitta video.

Bitta ish:
  1. video yopiq kanaldan yuklab olinadi (bot ko'chirgan nusxa), asosiy akkaunt (pyrogram sessiyasi) bilan;
  2. VoiceStudio (CPU) bilan dublyaj — `tool/voicedub/run.py` (ARUGRAM repodan): Whisper -> tarjima -> OmniVoice (asl ovozni klonlab) -> ffmpeg;
  3. tayyor video yopiq kanalga yuklanadi, `/api/dub/finish` — worker uni admin chatiga ko'chiradi.
FOIZ ACTIONS'NING O'ZIDAN: har bosqich uchun umumiy foiz (yuklash, ovoz ajratish, transkripsiya, tarjima, har bir gap ovozlanishi,
eksport, yuborish) botdagi bitta xabarda jonli yangilanib turadi (`/api/dub/progress`), arudrivebot kabi.
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
WORK = Path(os.environ.get("WORK_ROOT", os.environ.get("RUNNER_TEMP", "/tmp"))) / "arumediatv_dub"
# ARUGRAM repodan olingan `tool/voicedub` (driver `run.py` shu yerda).
VD = Path(os.environ.get("VD_DIR", HERE.parent / "voicedub"))
# Actions job chegarasi 6 soat: run boshlanishi (JOB_T0, workflow yozadi) + byudjet; oxirgi 8 daqiqa yuborish uchun zaxira.
JOB_T0 = float(os.environ.get("JOB_T0") or time.time())
JOB_BUDGET_S = float(os.environ.get("JOB_BUDGET_MIN", "335")) * 60
SEND_RESERVE_S = 8 * 60


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
                     "Content-Type": "application/json", "User-Agent": "arumediatv-dub"})
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
    return _post(f"/api/dub/{path}", body)


def claim(runner):
    try:
        r = api("claim", {"runner": runner})
    except Exception as e:  # noqa: BLE001
        print("dub/claim:", e, flush=True)
        return None
    return r if r.get("job") and r.get("channel") else None


# ── Jonli holat (foiz Actions'dan) ───────────────────────────

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
    """Botdagi jonli holat xabari (`/api/dub/progress`): umumiy foiz, bosqich, o'tgan va taxminiy qolgan vaqt.
    Har 8 soniyada bittadan ko'p yuborilmaydi (bosqich almashganda darhol)."""

    def __init__(self, r):
        job = r["job"]
        self.base = {"id": int(job["id"]), "name": str(job.get("name") or ""), "chat": int(job["chat"]),
                     "attempt": int(job.get("attempt") or 1), "status_msg": int(r.get("status_msg") or 0)}
        self.steps = []
        self.last = 0.0
        self.last_stage = ""
        self.t0 = time.time()
        self.pipe_t0 = None

    def done(self, line):
        self.steps.append("✅ " + line)

    def send(self, line, force=False):
        now = time.time()
        if not self.base["status_msg"] or (not force and now - self.last < 8):
            return
        self.last = now
        text = "\n".join(self.steps + [line, "", f"⏱ jami: {hms(now - self.t0)}"])
        try:
            api("progress", {**self.base, "text": text})
        except Exception as e:  # noqa: BLE001
            print("dub/progress:", e, flush=True)

    def pipeline(self, pct, stage):
        """Umumiy foiz (0..100) va bosqich matni."""
        if self.pipe_t0 is None:
            self.pipe_t0 = time.time()
        el = time.time() - self.pipe_t0
        eta = el * (100 - pct) / pct if pct >= 12 else 0
        eta_txt = f" · qoldi ~{hms(eta)}" if eta else ""
        changed = stage.split(":")[0] != self.last_stage
        self.last_stage = stage.split(":")[0]
        self.send(f"{bar(pct)} {pct:.0f}%\n{stage}\no'tdi {hms(el)}{eta_txt}", force=changed)


# ── ffprobe ──────────────────────────────────────────────────

def probe(path):
    try:
        out = subprocess.check_output(
            ["ffprobe", "-v", "error", "-select_streams", "v:0", "-show_entries", "stream=width,height:format=duration",
             "-of", "json", str(path)], stderr=subprocess.DEVNULL).decode()
        j = json.loads(out)
        st = (j.get("streams") or [{}])[0]
        return int(st.get("width") or 0), int(st.get("height") or 0), float((j.get("format") or {}).get("duration") or 0)
    except Exception:  # noqa: BLE001
        return 0, 0, 0.0


# ── VoiceStudio (driver) ─────────────────────────────────────

def run_driver(src, lang, outdir, live):
    """`tool/voicedub/run.py` — `PROGRESS <foiz>\\t<matn>` qatorlari holat xabariga, qolgani Actions log'iga."""
    env = {**os.environ, "VS_API": os.environ.get("VS_API", "http://127.0.0.1:8000")}
    p = subprocess.Popen([sys.executable, str(VD / "run.py"), str(src), lang, str(outdir)],
                         stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, bufsize=1, env=env)
    tail = collections.deque(maxlen=30)
    gen0 = None   # ovozlash (30..95%) boshlangan payt
    for line in p.stdout:
        line = line.rstrip()
        if not line:
            continue
        tail.append(line)
        if line.startswith("PROGRESS "):
            pct, _, txt = line[9:].partition("\t")
            try:
                pv = float(pct)
                live.pipeline(pv, txt)
                # Ovozlash tezligidan 6 soatlik chegaraga sig'ish-sig'masligi oldindan aniqlanadi (juda uzun video).
                if pv >= 31 and gen0 is None:
                    gen0 = (time.time(), pv)
                if gen0 and pv >= 35 and pv > gen0[1]:
                    rate = (pv - gen0[1]) / max(time.time() - gen0[0], 1)
                    finish_at = time.time() + (96 - pv) / rate
                    if finish_at > JOB_T0 + JOB_BUDGET_S - SEND_RESERVE_S:
                        p.terminate()
                        over = (finish_at - (JOB_T0 + JOB_BUDGET_S - SEND_RESERVE_S)) / 60
                        raise Fatal(f"video juda uzun: ovozlash Actions'ning 6 soatlik chegarasiga sig'maydi "
                                    f"(taxminan {over:.0f} daqiqa yetmaydi). Videoni qisqaroq bo'laklarga bo'lib yuboring")
            except ValueError:
                pass
            log("  " + txt)
        else:
            log("  " + line)
    return p.wait(), list(tail)


# ── Ish ──────────────────────────────────────────────────────

async def process(app, r, runner):
    channel = int(r["channel"])
    job = r["job"]
    jid, name, lang = int(job["id"]), str(job.get("name") or "video"), str(job.get("lang") or "uz")
    live = Live(r)
    ident = {"runner": runner, "id": jid, "status_msg": live.base["status_msg"]}
    shutil.rmtree(WORK, ignore_errors=True)
    WORK.mkdir(parents=True)
    try:
        log(f"🗣 Dublyaj #{jid}: {name} -> {lang} (urinish {job.get('attempt')})")
        live.send("⬇️ Video yuklab olinmoqda...", force=True)
        msg = await app.get_messages(channel, int(job["ch_msg"]))
        if not msg or msg.empty or not (msg.video or msg.document):
            raise Fatal("video kanalda topilmadi")
        t0 = time.time()

        def dl_cb(cur, total):
            live.send(f"⬇️ Video yuklab olinmoqda\n{bar(cur * 100 / max(total, 1))} {cur * 100 / max(total, 1):.0f}% · {mb(cur)} / {mb(total)}")

        src = await app.download_media(msg, file_name=str(WORK / "src.bin"), progress=dl_cb)
        if not src or Path(src).stat().st_size == 0:
            raise RuntimeError("video yuklab olinmadi")
        src = Path(src)
        w, h, dur = probe(src)
        live.done(f"Yuklab olindi: {mb(src.stat().st_size)} · {w}×{h} · {hms(dur)} ({hms(time.time() - t0)})")

        # VoiceStudio ASR/diarizatsiya uchun oddiy mp4 kutadi.
        clip = WORK / "clip.mp4"
        subprocess.run(["ffmpeg", "-hide_banner", "-loglevel", "error", "-y", "-i", str(src), "-c:v", "libx264", "-preset", "veryfast",
                        "-crf", "26", "-vf", "scale='min(854,iw)':-2", "-c:a", "aac", "-ar", "44100", str(clip)], check=True)
        src.unlink(missing_ok=True)

        out_dir = WORK / "out"
        t1 = time.time()
        live.pipeline(1, "VoiceStudio ishga tushmoqda")
        code, tail = await asyncio.to_thread(run_driver, clip, lang, out_dir, live)
        out = out_dir / f"dubbed_{lang}.mp4"
        if code != 0 or not out.exists():
            raise RuntimeError("dublyaj bajarilmadi: " + (tail[-1] if tail else f"kod {code}")[:200])
        live.done(f"Dublyaj tayyor ({hms(time.time() - t1)})")
        size = out.stat().st_size
        ow, oh, od = probe(out)

        api("check", ident)   # admin shu orada o'chirgan bo'lsa — yuborilmaydi

        def up_cb(cur, total):
            live.send(f"⬆️ Tayyor video yuborilmoqda\n{bar(cur * 100 / max(total, 1))} {cur * 100 / max(total, 1):.0f}% · {mb(cur)} / {mb(total)}")

        live.send("⬆️ Tayyor video yuborilmoqda...", force=True)
        sent = await app.send_video(channel, str(out), caption=f"dub_{jid}_{lang}", supports_streaming=True,
                                    duration=int(od), width=ow, height=oh, file_name=f"dub_{jid}_{lang}.mp4", progress=up_cb)
        summary = f"{lang} · {hms(od)} · {mb(size)} · jami {hms(time.time() - live.t0)}"
        api("finish", {**ident, "ok": True, "res_msg": int(sent.id), "size": size, "summary": summary})
        log(f"  ✅ dublyaj #{jid} yuborildi ({summary})")
    except Lost:
        log(f"  dublyaj #{jid} o'chirildi yoki boshqa run'ga o'tdi — yuborilmadi")
    except Fatal as ex:
        log(f"  dublyaj #{jid} XATO (qayta urinilmaydi): {ex}")
        try:
            api("finish", {**ident, "ok": False, "fatal": True, "error": str(ex)})
        except Exception:  # noqa: BLE001
            pass
    except Exception as ex:  # noqa: BLE001
        log(f"  dublyaj #{jid} XATO: {type(ex).__name__}: {ex}")
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
