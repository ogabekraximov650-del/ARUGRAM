"""Post kodlash — kodlash botining "Post kodlash" bo'limi (`worker/src/postbot.rs`).

`tool/encode/run.py` har qismdan OLDIN `claim()` ni chaqiradi: navbatda
post bo'lsa, u shu run'da, shu Telegram sessiyasi (Pyrogram `app`) bilan
ishlanadi — bitta sessiyani ikki run bir vaqtda ishlatsa Telegram uni
bekor qiladi.

Bitta post:
  1. rasm va video yopiq kanaldan yuklab olinadi (bot ko'chirgan postlar);
  2. `encode.sh` — `anime` repodagi oddiy "Encode" bilan bir xil (H.264,
     boshida 3 soniya rasm, burchakda logotip);
  3. tayyor video `<ID>_logo.png` fayl nomidagi ID'ga Telegram'da
     ochiladigan VIDEO bo'lib (fayl emas), tagida post nomi bilan yuboriladi;
  4. `/api/post/finish` — worker kanal postlarini va navbat yozuvini o'chiradi.
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
WORK = Path(os.environ.get("RUNNER_TEMP", "/tmp")) / "arugram_post"


class Lost(Exception):
    """Post o'chirildi yoki boshqa run'ga o'tdi."""


class Fatal(Exception):
    """Qayta urinishdan foyda yo'q."""


def api(path, body):
    base = os.environ["API_BASE"].rstrip("/")
    data = json.dumps(body).encode()
    err = None
    for attempt in range(5):
        req = urllib.request.Request(
            f"{base}/api/post/{path}", data=data, method="POST",
            headers={"X-Encode-Token": os.environ["ENCODE_TOKEN"],
                     "Content-Type": "application/json", "User-Agent": "arugram-encoder"})
        try:
            with urllib.request.urlopen(req, timeout=60) as r:
                return json.loads(r.read() or b"{}")
        except urllib.error.HTTPError as e:
            if e.code == 409:
                raise Lost()
            if e.code in (400, 401, 403, 404):
                raise RuntimeError(f"post/{path}: HTTP {e.code}")
            err = e
        except Exception as e:  # tarmoq
            err = e
        time.sleep(3 * (attempt + 1))
    raise RuntimeError(f"post/{path}: {err}")


def claim(runner):
    """Navbatdagi post (yo'q bo'lsa yoki worker eski bo'lsa — None)."""
    try:
        r = api("claim", {"runner": runner})
    except Exception as e:
        print("post/claim:", e, flush=True)
        return None
    if r.get("job") and r.get("channel"):
        return r
    return None


def target():
    """Video yuboriladigan ID — `<ID>_logo.png` fayl nomidan."""
    for p in sorted(HERE.glob("*_logo.png")):
        uid = p.name[: -len("_logo.png")]
        if re.fullmatch(r"-?\d+", uid):
            return int(uid), p
    raise Fatal("tool/post/<ID>_logo.png topilmadi")


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


def run_encode(args, log):
    """encode.sh — har qator Actions log'iga, har 30 soniyada bittasi kanal log'iga."""
    p = subprocess.Popen(["bash", str(HERE / "encode.sh"), *map(str, args)],
                         stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, bufsize=1)
    last = 0.0
    for line in p.stdout:
        line = line.rstrip()
        if not line:
            continue
        now = time.time()
        if line.startswith("\U0001F3AC") and now - last < 30:
            print(line, flush=True)
            continue
        last = now
        log("  " + line)
    return p.wait()


async def process(app, r, runner, log, progress=None):
    channel = int(r["channel"])
    job = r["job"]
    pid, name = int(job["id"]), str(job.get("name") or "").strip()
    ident = {"runner": runner, "id": pid}
    shutil.rmtree(WORK, ignore_errors=True)
    WORK.mkdir(parents=True)
    try:
        uid, logo = target()
        log(f"\U0001F3AC Post #{pid}: {name.splitlines()[0] if name else ''} "
            f"(urinish {job.get('attempt')}) -> {uid}")

        pm = await app.get_messages(channel, int(job["photo_msg"]))
        vm = await app.get_messages(channel, int(job["video_msg"]))
        if not pm or pm.empty or not (pm.photo or pm.document):
            raise Fatal("rasm kanalda topilmadi")
        if not vm or vm.empty or not (vm.video or vm.document):
            raise Fatal("video kanalda topilmadi")

        raw = await app.download_media(pm, file_name=str(WORK / "cover.raw"))
        if not raw or Path(raw).stat().st_size == 0:
            raise RuntimeError("rasm yuklab olinmadi")
        log("  video yuklab olinmoqda...")
        src = await app.download_media(vm, file_name=str(WORK / "source.video"),
                                       progress=progress("yuklab olinmoqda") if progress else None)
        if not src or Path(src).stat().st_size == 0:
            raise RuntimeError("video yuklab olinmadi")
        src_mb = Path(src).stat().st_size / 1048576

        # encode.sh rasmni PNG deb oladi; thumbnail — Telegram talabi (JPEG, 320px).
        cover = WORK / "cover.png"
        thumb = WORK / "thumb.jpg"
        r1 = subprocess.run(["ffmpeg", "-y", "-loglevel", "error", "-i", raw, "-frames:v", "1", str(cover)])
        if r1.returncode != 0 or not cover.exists():
            raise Fatal("rasmni ochib bo'lmadi")
        subprocess.run(["ffmpeg", "-y", "-loglevel", "error", "-i", str(cover), "-vf",
                        "scale='min(320,iw)':-2", "-q:v", "5", str(thumb)])

        out = WORK / f"post_{pid}.mp4"
        log(f"  kodlanmoqda (H.264), manba {src_mb:.1f} MB...")
        t = time.time()
        code = await asyncio.to_thread(run_encode, [src, cover, logo, out], log)
        if code != 0 or not out.exists():
            raise RuntimeError("kodlashda xatolik (Actions log'iga qarang)")
        Path(src).unlink(missing_ok=True)
        size = out.stat().st_size
        log(f"  tayyor: {size / 1048576:.1f} MB, {int(time.time() - t)} s")

        # Admin shu orada o'chirgan bo'lsa — yuborilmaydi.
        api("check", ident)
        extra = {}
        for k, v in (("duration", ffprobe(out, "duration")),
                     ("width", ffprobe(out, "width", "v:0")),
                     ("height", ffprobe(out, "height", "v:0"))):
            if v:
                extra[k] = v
        if thumb.exists() and thumb.stat().st_size > 0:
            extra["thumb"] = str(thumb)
        log(f"  {uid} ga yuborilmoqda...")
        first = (name.splitlines()[0] if name else f"post_{pid}")
        fname = re.sub(r'[\\/:*?"<>|]', "", first).strip()[:80] or f"post_{pid}"
        await app.send_video(
            uid, str(out), caption=name[:1024], file_name=f"{fname}.mp4",
            supports_streaming=True,
            progress=progress("Telegram'ga yuklanmoqda") if progress else None, **extra)
        api("finish", {**ident, "ok": True, "size": size, "to": str(uid)})
        log(f"  ✅ post #{pid} yuborildi")
    except Lost:
        log(f"  post #{pid} o'chirildi yoki boshqa run'ga o'tdi — yuborilmadi")
    except Fatal as ex:
        log(f"  post #{pid} XATO (qayta urinilmaydi): {ex}")
        try:
            api("finish", {**ident, "ok": False, "fatal": True, "error": str(ex)})
        except Exception:
            pass
    except Exception as ex:
        log(f"  post #{pid} XATO: {ex}")
        try:
            api("finish", {**ident, "ok": False, "error": str(ex)[:300]})
        except Exception:
            pass
    finally:
        shutil.rmtree(WORK, ignore_errors=True)


def cancel(runner, pid):
    """Run bekor qilindi — post navbatga qaytadi."""
    try:
        api("finish", {"runner": runner, "id": pid, "ok": False, "cancelled": True})
    except Exception:
        pass


if __name__ == "__main__":
    print(__doc__)
    sys.exit(0)
