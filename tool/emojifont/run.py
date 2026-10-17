"""Emoji shriftini (`fonts/TgEmoji.ttf`) kanalga yuklash (GitHub Actions, `emojifont.yml`).

Ilova shriftni APK ichida saqlamaydi: u yopiq kanalda AES-128-CTR bilan shifrlangan
`sys_emojifont_<versiya>.bin` fayli bo'lib turadi, ilova emoji oynasi ochilganda uni yuklab oladi
(`worker/src/emojifont.rs`). Worker shriftni 5 urinishda ochib bo'lmasa shu workflow'ni uyg'otadi —
u faylni YANGI kalit va YANGI versiya bilan qayta yuklaydi, eskisi kanaldan o'chadi.

Telegram sessiyasi kodlash/to'plamlar bilan BIR XIL (`tool/encode/session.enc`). Fayl baytlari
worker orqali o'tmaydi: kanalga sessiya to'g'ridan-to'g'ri yuboradi, worker'ga faqat natija (`commit`) boradi.
"""
import asyncio
import json
import os
import secrets
import sys
import tempfile
import time
import urllib.error
import urllib.request
from pathlib import Path

from cryptography.hazmat.primitives.ciphers import Cipher, algorithms, modes

API = os.environ.get("API_BASE", "").rstrip("/")
TOKEN = os.environ.get("ENCODE_TOKEN", "")
ROOT = Path(__file__).resolve().parent.parent.parent
FONT = ROOT / "fonts" / "TgEmoji.ttf"
SESSION = str(Path(__file__).with_name("pyro_session"))


def log(*a):
    print(time.strftime("%H:%M:%S"), *a, flush=True)


def api(path: str, body: dict) -> dict:
    data = json.dumps(body).encode()
    err = None
    for _ in range(5):
        req = urllib.request.Request(
            f"{API}/api/emojifont/job/{path}", data=data, method="POST",
            headers={"X-Encode-Token": TOKEN, "Content-Type": "application/json",
                     "User-Agent": "arumediatv-encoder"})
        try:
            with urllib.request.urlopen(req, timeout=60) as r:
                return json.loads(r.read() or b"{}")
        except urllib.error.HTTPError as e:
            if e.code in (400, 401, 403, 404):
                raise RuntimeError(f"emojifont/{path}: HTTP {e.code} {e.read()[:200]!r}")
            err = e
        except Exception as e:  # tarmoq
            err = e
        time.sleep(3)
    raise RuntimeError(f"emojifont/{path}: {err}")


def seal(plain: bytes, key: bytes) -> bytes:
    """AES-128-CTR (IV nol, 128-bit big-endian hisoblagich) — ilovadagi `ctr_apply` bilan bir xil."""
    return Cipher(algorithms.AES(key), modes.CTR(b"\0" * 16)).encryptor().update(plain)


def _session_api_id() -> int:
    try:
        import sqlite3
        c = sqlite3.connect(f"file:{SESSION}.session?mode=ro", uri=True)
        return int(c.execute("SELECT api_id FROM sessions").fetchone()[0] or 0)
    except Exception:
        return 0


async def main():
    if not API or not TOKEN:
        sys.exit("API_BASE / ENCODE_TOKEN yo'q")
    r = api("peek", {})
    if not r.get("need") and os.environ.get("FORCE") != "true":
        log("Shrift yangilash kerak emas")
        return
    channel = int(r.get("channel") or 0)
    if not channel:
        sys.exit("kanal noma'lum")
    plain = FONT.read_bytes()
    if len(plain) < 100_000 or plain[:4] not in (b"\x00\x01\x00\x00", b"true", b"OTTO", b"ttcf"):
        sys.exit("shrift fayli yaroqsiz")
    ver = int(time.time())
    name = f"sys_emojifont_{ver}.bin"
    key = secrets.token_bytes(16)
    sealed = seal(plain, key)
    log(f"{name}: {len(sealed) / 1048576:.1f} MB — kanalga yuklanmoqda")

    import pyrogram.utils
    from pyrogram import Client

    # Pyrogram 2.0.106: yangi kanallar uchun ma'lum xato (kodlashdagi yamoq).
    pyrogram.utils.MIN_CHANNEL_ID = -1009999999999
    api_id = _session_api_id() or int(os.environ["TG_API_ID"])
    app = Client(SESSION, api_id=api_id, api_hash=os.environ["TG_API_HASH"], no_updates=True)
    async with app:
        async for _ in app.get_dialogs():
            pass
        with tempfile.TemporaryDirectory() as d:
            p = Path(d) / name
            p.write_bytes(sealed)
            sent = await app.send_document(
                channel, str(p), file_name=name, force_document=True,
                caption=name, disable_notification=True)
    api("commit", {"name": name, "ver": ver, "key": key.hex(), "msg_id": sent.id, "size": len(sealed)})
    log("Tayyor: shrift kanalga yuklandi va worker'ga yozildi")


if __name__ == "__main__":
    asyncio.run(main())
