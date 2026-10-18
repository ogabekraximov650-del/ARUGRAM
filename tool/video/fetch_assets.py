#!/usr/bin/env python3
"""Video uchun haqiqiy poster va bitta anime qismini yuklab oladi (GitHub Actions, `video.yml`).

1. Worker'dan (`GET /api/video/assets`, `X-Encode-Token`) eng ko'p ko'rilgan bo'limlar posterlari va bitta kichik sifatli
   qism haqida ma'lumot (kanaldagi xabar raqami + AES-128-CTR kaliti) olinadi.
2. Fayllar Telegram kanalidan Pyrogram bilan olinadi (kodlash/shrift workflow'lari bilan BIR XIL sessiya) va ochiladi
   (AES-128-CTR, IV nol — ilovadagi `ctr_apply` bilan bir xil). Fayl baytlari worker orqali O'TMAYDI.
3. Qismdan ikkita qisqa lavha kadrlarga ajratiladi (`ffmpeg`) — pleyer oynasida shu ko'rinadi.

Natija: `assets/` papkasi (`assets.json`, `posters/`, `episode/`). Xato bo'lsa skript 0 bilan chiqadi va video
o'rnini bosuvchi rasmlar bilan tayyorlanadi (ilova ham rasm yo'q bo'lsa shunday ko'rsatadi).
"""
import asyncio
import json
import os
import shutil
import subprocess
import sys
import time
import urllib.request
from pathlib import Path

from cryptography.hazmat.primitives.ciphers import Cipher, algorithms, modes

HERE = Path(__file__).resolve().parent
OUT = HERE / "assets"
SESSION = str(HERE / "pyro_session")
API = os.environ.get("API_BASE", "").rstrip("/") or "https://arumediatv.uzcom.workers.dev"
TOKEN = os.environ.get("ENCODE_TOKEN", "").strip()
FPS = 12            # pleyer oynasi kichik — 12 kadr/soniya yetadi
CLIP_A = (26, 48)   # (boshlanish soniya, davomiylik) — ilovadagi "0:26 dan davom etasiz"
CLIP_B_LEN = 48


def log(*a):
    print(time.strftime("%H:%M:%S"), *a, flush=True)


def api_get(path):
    last = None
    for i in range(40):  # worker deploy bo'layotgan bo'lishi mumkin — kutamiz
        req = urllib.request.Request(API + path, headers={"X-Encode-Token": TOKEN, "User-Agent": "arumediatv-video"})
        try:
            with urllib.request.urlopen(req, timeout=60) as r:
                return json.loads(r.read())
        except Exception as e:  # noqa: BLE001
            last = e
            time.sleep(15)
    raise RuntimeError(f"{path}: {last}")


def ctr(key_hex: str):
    return Cipher(algorithms.AES(bytes.fromhex(key_hex)), modes.CTR(b"\0" * 16)).decryptor()


def decrypt_file(src: Path, dst: Path, key_hex: str):
    d = ctr(key_hex)
    with src.open("rb") as f, dst.open("wb") as o:
        while True:
            b = f.read(1 << 22)
            if not b:
                break
            o.write(d.update(b))
        o.write(d.finalize())


def sniff_ext(p: Path) -> str:
    h = p.read_bytes()[:16]
    if h[:3] == b"\xff\xd8\xff":
        return ".jpg"
    if h[:8] == b"\x89PNG\r\n\x1a\n":
        return ".png"
    if h[:4] == b"RIFF" and h[8:12] == b"WEBP":
        return ".webp"
    return ".bin"


def ffprobe_duration(p: Path) -> float:
    try:
        r = subprocess.run(["ffprobe", "-v", "error", "-show_entries", "format=duration", "-of", "csv=p=0", str(p)],
                           capture_output=True, text=True, timeout=120)
        return float(r.stdout.strip())
    except Exception:  # noqa: BLE001
        return 0.0


def extract(video: Path, start: float, length: float, outdir: Path):
    outdir.mkdir(parents=True, exist_ok=True)
    subprocess.run(["ffmpeg", "-v", "error", "-ss", f"{start:.2f}", "-i", str(video), "-t", f"{length:.2f}",
                    "-vf", f"fps={FPS},scale=656:-2", "-q:v", "4", str(outdir / "%05d.jpg")], check=True)
    return len(list(outdir.glob("*.jpg")))


async def main():
    if not TOKEN:
        log("ENCODE_TOKEN yo'q — rasm va qism yuklanmaydi")
        return
    shutil.rmtree(OUT, ignore_errors=True)
    (OUT / "posters").mkdir(parents=True)
    info = api_get("/api/video/assets")
    channel = int(info.get("channel") or 0)
    posters, ep = info.get("posters") or [], info.get("episode") or None
    log(f"worker: {len(posters)} poster, qism: {'bor' if ep else 'yo`q'}")
    if not channel:
        raise RuntimeError("kanal noma'lum")

    import pyrogram.utils
    from pyrogram import Client

    pyrogram.utils.MIN_CHANNEL_ID = -1009999999999  # Pyrogram 2.0.106: yangi kanallar uchun yamoq (kodlashdagidek)
    api_id = int(os.environ["TG_API_ID"])
    app = Client(SESSION, api_id=api_id, api_hash=os.environ["TG_API_HASH"], no_updates=True)
    result = {"posters": [], "episode": None}
    tmp = HERE / "tmp"
    tmp.mkdir(exist_ok=True)
    async with app:
        async for _ in app.get_dialogs():
            pass
        for i, p in enumerate(posters):
            try:
                msg = await app.get_messages(channel, int(p["msg_id"]))
                raw = Path(await app.download_media(msg, file_name=str(tmp / f"p{i}.enc")))
                plain = tmp / f"p{i}.plain"
                decrypt_file(raw, plain, p["key"])
                ext = sniff_ext(plain)
                if ext == ".bin":
                    log(f"poster {i}: rasm emas, o'tkazildi")
                    continue
                final = OUT / "posters" / f"p{i}{ext}"
                shutil.move(str(plain), final)
                raw.unlink(missing_ok=True)
                rc = int(p.get("rating_count") or 0)
                result["posters"].append({
                    "file": f"assets/posters/{final.name}", "name": p["name"], "section": p.get("section") or 1,
                    "episodes": p.get("episodes") or 0, "views": p.get("views") or 0,
                    "rating": round((int(p.get("rating_sum") or 0) / rc), 1) if rc else 0,
                })
                log(f"poster {i}: {p['name']} ({final.stat().st_size // 1024} KB)")
            except Exception as e:  # noqa: BLE001
                log(f"poster {i} yuklanmadi: {e}")
        if ep:
            try:
                size = int(ep.get("size") or 0)
                log(f"qism: {ep['anime']} {ep['number']}-qism {ep['quality']} ({size / 1048576:.0f} MB) yuklanmoqda")
                msg = await app.get_messages(channel, int(ep["msg_id"]))
                raw = Path(await app.download_media(msg, file_name=str(tmp / "ep.enc")))
                video = tmp / "ep.mp4"
                decrypt_file(raw, video, ep["key"])
                raw.unlink(missing_ok=True)
                dur = ffprobe_duration(video)
                log(f"qism davomiyligi: {dur:.0f} s")
                if dur > 90:
                    jump = min(600.0, max(120.0, dur * 0.45))
                    na = extract(video, CLIP_A[0], CLIP_A[1], OUT / "episode" / "a")
                    nb = extract(video, jump, CLIP_B_LEN, OUT / "episode" / "b")
                    result["episode"] = {
                        "anime": ep["anime"], "number": ep["number"], "title": ep.get("title") or "",
                        "duration": round(dur, 1), "fps": FPS, "jump": round(jump - CLIP_A[0], 1),
                        "a": {"dir": "assets/episode/a", "start": CLIP_A[0], "count": na},
                        "b": {"dir": "assets/episode/b", "start": round(jump, 1), "count": nb},
                    }
                    log(f"lavhalar: {na} + {nb} kadr")
            except Exception as e:  # noqa: BLE001
                log(f"qism yuklanmadi: {e}")
    shutil.rmtree(tmp, ignore_errors=True)
    (OUT / "assets.json").write_text(json.dumps(result, ensure_ascii=False, indent=1), encoding="utf-8")
    log("tayyor:", len(result["posters"]), "poster,", "qism bor" if result["episode"] else "qism yo'q")


if __name__ == "__main__":
    try:
        asyncio.run(main())
    except Exception as e:  # noqa: BLE001
        log("XATO (video rasmlarsiz tayyorlanadi):", e)
    sys.exit(0)
