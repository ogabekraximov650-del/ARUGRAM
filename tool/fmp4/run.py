#!/usr/bin/env python3
"""Eski qismlarning fMP4 nusxasini yasaydi (Telegram Mini App pleyeri uchun).

ALOHIDA workflow `fmp4.yml` (avtoencode repo, shablon `tool/fmp4/fmp4.workflow.yml`).
Worker (`worker/src/fmp4.rs`) Mini App fMP4'i yo'q qismni so'raganda tayyor
sifatlarni `fmp4_jobs` navbatiga qo'yadi va shu workflow'ni ishga tushiradi.

Har ish: kanaldagi shifrlangan MP4 -> ochiladi (AES-128-CTR) -> `ffmpeg -c copy`
bilan fMP4 (qayta siqilmaydi, boshida `sidx`) -> YANGI kalit bilan shifrlanadi
-> kanalga yuklanadi -> `/api/fmp4/done`. Navbat bo'shaguncha davom etadi.
"""
import json
import os
import secrets
import shutil
import subprocess
import sys
import time
import urllib.parse
import urllib.request
from pathlib import Path

from cryptography.hazmat.primitives.ciphers import Cipher, algorithms, modes
from pyrogram import Client

API = os.environ.get("API_BASE", "https://arumediatv.uzcom.workers.dev").rstrip("/")
TOKEN = "".join(os.environ["ENCODE_TOKEN"].split())
RUNNER = f"{os.environ.get('GITHUB_RUN_ID', 'local')}-{os.environ.get('GITHUB_RUN_ATTEMPT', '1')}"
BUDGET = int(os.environ.get("START_BUDGET_MIN", "300")) * 60
HERE = Path(__file__).parent
SESSION = str(HERE / "pyro_session")
WORK = Path(os.environ.get("RUNNER_TEMP", "/tmp")) / "fmp4"
CHUNK = 4 << 20
T0 = time.time()


def log(*a):
    print(time.strftime("%H:%M:%S"), *a, flush=True)


def api(path, body):
    data = json.dumps(body).encode()
    for attempt in range(5):
        req = urllib.request.Request(
            f"{API}/api/fmp4/{path}", data=data, method="POST",
            headers={"X-Encode-Token": TOKEN, "Content-Type": "application/json",
                     "User-Agent": "arumediatv-fmp4"})
        try:
            with urllib.request.urlopen(req, timeout=60) as r:
                return json.loads(r.read() or b"{}")
        except Exception as ex:  # noqa: BLE001
            log(f"api {path}: {ex}")
            time.sleep(2 + attempt * 3)
    raise RuntimeError(f"api {path} ishlamadi")


def ctr(src: Path, dst: Path, key: bytes):
    """AES-128-CTR (IV nol) — shifrlash ham, ochish ham bir xil."""
    c = Cipher(algorithms.AES(key), modes.CTR(b"\0" * 16)).encryptor()
    with open(src, "rb") as fi, open(dst, "wb") as fo:
        while True:
            b = fi.read(CHUNK)
            if not b:
                break
            fo.write(c.update(b))
        fo.write(c.finalize())


def make_fmp4(src: Path, dst: Path):
    subprocess.run(
        ["ffmpeg", "-hide_banner", "-loglevel", "error", "-y", "-i", str(src),
         "-map", "0", "-c", "copy",
         "-movflags", "+frag_keyframe+empty_moov+default_base_moof+global_sidx",
         str(dst)], check=True, timeout=1800)


def session_api_id():
    """Sessiya qaysi api_id bilan yaratilgan bo'lsa — o'sha (encode/run.py dagidek)."""
    try:
        import sqlite3
        con = sqlite3.connect(SESSION + ".session")
        row = con.execute("SELECT api_id FROM sessions").fetchone()
        con.close()
        return int(row[0]) if row and row[0] else 0
    except Exception:  # noqa: BLE001
        return 0


class _Slice:
    """Fayldan `length` bayt o'qiydi (oqim, xotiraga yig'ilmaydi)."""

    def __init__(self, f, length: int):
        self.f, self.left = f, length

    def read(self, n=-1):
        if self.left <= 0:
            return b""
        n = self.left if n is None or n < 0 else min(n, self.left)
        b = self.f.read(n)
        self.left -= len(b)
        return b


def put_direct(url: str, path: Path):
    """Imzolangan havola bilan R2'ga TO'G'RIDAN-TO'G'RI bitta PUT (worker'dan o'tmaydi)."""
    size = path.stat().st_size
    with open(path, "rb") as f:
        req = urllib.request.Request(url, data=_Slice(f, size), method="PUT",
                                     headers={"Content-Length": str(size), "User-Agent": "arumediatv-fmp4"})
        with urllib.request.urlopen(req, timeout=1800) as r:
            r.read()


def put_parts(path: Path, name: str, part_size: int) -> list:
    """Worker orqali multipart (R2 kaliti yo'q rejimi): bo'laklar OQIM bilan."""
    size, parts = path.stat().st_size, []
    with open(path, "rb") as f:
        n, off = 0, 0
        while off < size:
            n += 1
            length = min(part_size, size - off)
            err = None
            for attempt in range(1, 5):
                try:
                    f.seek(off)
                    req = urllib.request.Request(
                        f"{API}/api/r2/part?file={urllib.parse.quote(name)}&n={n}", data=_Slice(f, length), method="PUT",
                        headers={"X-Encode-Token": TOKEN, "Content-Type": "application/octet-stream",
                                 "Content-Length": str(length), "User-Agent": "arumediatv-fmp4"})
                    with urllib.request.urlopen(req, timeout=900) as r:
                        parts.append({"n": n, "etag": json.loads(r.read())["etag"]})
                    err = None
                    break
                except Exception as ex:  # noqa: BLE001
                    err = ex
                    time.sleep(3 * attempt)
            if err is not None:
                raise RuntimeError(f"bo'lak {n}: {err}")
            off += length
    return parts


def upload_r2(ident: dict, path: Path, name: str, key: str) -> bool:
    """fMP4 ni R2'ga yozadi (yuklovchi workflow Telegram'ga olib boradi). R2 yo'q bo'lsa `False` (eski usul: to'g'ridan-to'g'ri kanalga)."""
    size = path.stat().st_size
    waited = 0
    for attempt in range(1, 4):
        try:
            while True:
                r = api("r2put", {**ident, "file": name, "key": key, "size": size})
                if not r.get("wait"):
                    break
                if not waited:
                    log("  R2'da joy yo'q — bo'shashi kutilmoqda...")
                waited += 30
                if waited > 8 * 3600:
                    raise RuntimeError("R2 8 soat bo'shamadi")
                time.sleep(30)
            if r.get("url"):
                put_direct(r["url"], path)
                parts = []
            else:
                parts = put_parts(path, name, int(r.get("part_size") or 94371840))
            api("r2ready", {**ident, "file": name, "size": size, "parts": parts})
            return True
        except Exception as ex:  # noqa: BLE001
            if "503" in str(ex) or "api r2put ishlamadi" in str(ex):
                return False
            log(f"  R2'ga yozish {attempt}/3 muvaffaqiyatsiz: {ex}")
            time.sleep(4 * attempt)
    return False


def one(app, job):
    a, s, e, q = job["anime_id"], job["season_id"], job["epizod_id"], job["quality"]
    ident = {"anime_id": a, "season_id": s, "epizod_id": e, "quality": q}
    shutil.rmtree(WORK, ignore_errors=True)
    WORK.mkdir(parents=True, exist_ok=True)
    try:
        log(f"#{a}/{s}/{e} {q}: {job['src_name']} yuklanmoqda...")
        msg = app.get_messages(int(job["channel"]), int(job["src_msg_id"]))
        if not msg or not (msg.document or msg.video):
            raise RuntimeError("kanalda fayl topilmadi")
        enc = Path(app.download_media(msg, file_name=str(WORK / "src.enc")))
        plain = WORK / "src.mp4"
        key = job.get("src_key") or ""
        if len(key) == 32:
            ctr(enc, plain, bytes.fromhex(key))
            enc.unlink()
        else:
            enc.rename(plain)
        frag = WORK / "frag.mp4"
        make_fmp4(plain, frag)
        plain.unlink()
        size = frag.stat().st_size
        fk = secrets.token_bytes(16)
        sealed = WORK / job["name"]
        ctr(frag, sealed, fk)
        frag.unlink()
        # Avval R2'ga (yuklovchi workflow Telegram'ga olib boradi, bot bazaga yozadi); R2 bo'lmasa — to'g'ridan-to'g'ri kanalga.
        if upload_r2(ident, sealed, job["name"], fk.hex()):
            log(f"  fMP4 {size / 1048576:.1f} MB R2'ga yozildi — Telegram'ga yuklovchi workflow olib boradi")
            return
        log(f"  fMP4 {size / 1048576:.1f} MB — kanalga yuklanmoqda...")
        sent = app.send_document(int(job["channel"]), str(sealed), file_name=job["name"],
                                 force_document=True, caption=job["name"], disable_notification=True)
        api("done", {**ident, "ok": True, "file": job["name"], "size": size,
                     "key": fk.hex(), "msg_id": sent.id})
        log("  tayyor")
    except Exception as ex:  # noqa: BLE001
        log("  XATO:", ex)
        try:
            api("done", {**ident, "ok": False, "error": str(ex)[:300]})
        except Exception:  # noqa: BLE001
            pass
    finally:
        shutil.rmtree(WORK, ignore_errors=True)


def main():
    api_id = session_api_id() or int(os.environ["TG_API_ID"])
    with Client(SESSION, api_id=api_id, api_hash=os.environ["TG_API_HASH"], no_updates=True) as app:
        for _ in app.get_dialogs():
            pass
        done = 0
        while time.time() - T0 < BUDGET:
            r = api("claim", {"runner": RUNNER})
            job = r.get("job")
            if not job:
                if r.get("retry"):
                    continue
                break
            one(app, job)
            done += 1
        log(f"Navbat tugadi: {done} ta ish")


if __name__ == "__main__":
    main()
    sys.exit(0)
