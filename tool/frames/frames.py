#!/usr/bin/env python3
"""UZ: Qism kadrlari (post afishasi albomi) — ALOHIDA workflow (`frames.yml`). Navbat bo'shaguncha `/api/frames/claim` qiladi:
kadrsiz qismning eng yuqori sifatli videosini video omboridan (`vdlib.download`) yuklaydi, `n` ta to'liq o'lchamli kadr oladi va
`/api/encode/collage` ga SHIFRLAB yuboradi (worker saqlaydi). Xato bo'lsa `/api/frames/fail` (keyinroq qayta uriniladi).
EN: Separate workflow that takes the frames of episodes that have none: claim -> download best quality from the video store ->
    pick frames -> post them (encrypted) to the existing collage endpoint."""
import json
import os
import secrets
import shutil
import subprocess
import sys
import tempfile
import time
import urllib.request
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "vd"))
import vdlib  # noqa: E402
from cryptography.hazmat.primitives.ciphers import Cipher, algorithms, modes  # noqa: E402

API = vdlib.API
TOKEN = vdlib.TOKEN
FRAMES_N = 10          # Telegram albomi — eng ko'pi bilan 10 ta
FRAMES_FROM, FRAMES_TO = 0.20, 0.80   # intro va titrlar tushib qolmasin
BUDGET_S = int(os.environ.get("START_BUDGET_MIN", "300")) * 60
RUNNER = os.environ.get("GITHUB_RUN_ID", "local") + "-frames"


def log(*a):
    print(time.strftime("%H:%M:%S"), *a, flush=True)


def probe_duration(path: Path) -> float:
    out = subprocess.run(["ffprobe", "-v", "error", "-show_entries", "format=duration", "-of", "json", str(path)],
                         capture_output=True, text=True, timeout=120, check=True).stdout
    d = float(json.loads(out)["format"]["duration"])
    if d <= 0:
        raise RuntimeError("video davomiyligi noto'g'ri")
    return d


def make_frames(video: Path, dur: float, n: int = FRAMES_N) -> list:
    """`n` ta SIFATLI kadr (to'liq o'lcham, 16:9 JPEG): videoning 20%..80% oralig'i `n` bo'lakka bo'linadi; har bo'lakdan 6 nomzod
    (kichik nusxada) olinib ENG BATAFSILI (JPEG hajmi eng katta) tanlanadi va aynan shu vaqt to'liq o'lchamda qayta olinadi."""
    tmp = Path(tempfile.mkdtemp())
    try:
        t0, t1 = dur * FRAMES_FROM, dur * FRAMES_TO
        seg = (t1 - t0) / n
        frames = []
        for i in range(n):
            cands = []
            for j in range(6):
                t = t0 + seg * i + seg * (j + 0.5) / 6
                f = tmp / f"c{i}_{j}.jpg"
                r = subprocess.run(["ffmpeg", "-v", "error", "-nostdin", "-y", "-ss", f"{t:.2f}", "-i", str(video), "-frames:v", "1",
                                    "-vf", "scale=640:360:flags=lanczos", "-q:v", "4", str(f)], capture_output=True, timeout=120)
                if r.returncode == 0 and f.exists() and f.stat().st_size > 0:
                    cands.append((f.stat().st_size, t))
            if not cands:
                raise RuntimeError(f"kadrlar: {i + 1}-bo'lakdan kadr olinmadi")
            _, bt = max(cands, key=lambda c: c[0])
            out = tmp / f"f{i}.jpg"
            r = subprocess.run(["ffmpeg", "-v", "error", "-nostdin", "-y", "-ss", f"{bt:.2f}", "-i", str(video), "-frames:v", "1",
                                "-vf", "scale='min(1920,iw)':-2:flags=lanczos", "-q:v", "2", str(out)], capture_output=True, timeout=120)
            if r.returncode != 0 or not out.exists() or out.stat().st_size < 2000:
                raise RuntimeError(f"{i + 1}-kadr olinmadi: " + (r.stderr or b"")[-200:].decode(errors="replace"))
            frames.append(out.read_bytes())
        return frames
    finally:
        shutil.rmtree(tmp, ignore_errors=True)


def pack_frames(frames: list) -> bytes:
    """Har kadr oldidan 4 bayt (big-endian) uzunlik — worker (`announce::split_frames`) shunday ajratadi."""
    return b"".join(len(f).to_bytes(4, "big") + f for f in frames)


def post_frames(job: dict, frames: list) -> dict:
    """Kadrlarni SHIFRLAB (AES-128-CTR, tasodifiy kalit, IV nol) `/api/encode/collage` ga yuboradi."""
    k = secrets.token_bytes(16)
    enc = Cipher(algorithms.AES(k), modes.CTR(b"\0" * 16)).encryptor()
    sealed = enc.update(pack_frames(frames)) + enc.finalize()
    q = f"a={job['anime_id']}&s={job['season_id']}&e={job['epizod_id']}&n={len(frames)}"
    req = urllib.request.Request(f"{API}/api/encode/collage?{q}", data=sealed, method="POST",
                                 headers={"X-Encode-Token": TOKEN, "X-Collage-Key": k.hex(), "Content-Type": "application/octet-stream",
                                          "User-Agent": "arumediatv-frames"})
    with urllib.request.urlopen(req, timeout=180) as r:
        return json.loads(r.read() or b"{}")


def one(job: dict) -> None:
    work = Path(tempfile.mkdtemp())
    try:
        src = work / "video.mp4"
        log(f"  {job['name']} ({job['quality']}) yuklanmoqda...")
        size = vdlib.download(job["name"], src)
        log(f"  yuklandi: {size / 1048576:.0f} MB")
        dur = probe_duration(src)
        frames = make_frames(src, dur)
        log(f"  {len(frames)} ta kadr tayyor ({sum(len(x) for x in frames) // 1024} KB)")
        r = post_frames(job, frames)
        log(f"  kadrlar saqlandi: {r}")
    finally:
        shutil.rmtree(work, ignore_errors=True)


def main() -> int:
    t0 = time.time()
    done = 0
    while time.time() - t0 < BUDGET_S:
        r = vdlib.api("/api/frames/claim", {"runner": RUNNER})
        job = r.get("job")
        if not job:
            log("navbat bo'sh" if not done else f"tayyor: {done} ta qism")
            return 0
        ident = f"{job['anime_id']}/{job['season_id']}/{job['epizod_id']} ({job['epizod_number']}-qism), urinish {job['attempt']}"
        log(f"qism {ident}")
        try:
            one(job)
            done += 1
        except Exception as ex:  # noqa: BLE001
            msg = f"{type(ex).__name__}: {str(ex)[:200]}"
            log(f"  XATO: {msg}")
            try:
                vdlib.api("/api/frames/fail", {"anime_id": job["anime_id"], "season_id": job["season_id"],
                                               "epizod_id": job["epizod_id"], "error": msg})
            except Exception as ex2:  # noqa: BLE001
                log(f"  xato haqida xabar berilmadi: {ex2}")
    log("vaqt byudjeti tugadi — qolganlari keyingi run'da")
    return 0


if __name__ == "__main__":
    sys.exit(main())
