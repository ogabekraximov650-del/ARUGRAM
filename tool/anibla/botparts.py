"""Anibla videosini BOT API orqali kanalga ~18 MiB li OCHILADIGAN mp4 qismlarga (vaqt bo'yicha) bo'lib yuklash va qaytarib olish.

UZ: Actions ishchisi worker'siz, o'zi (polling: navbatdan `claim`) videoni yuklab, `ffmpeg -c copy` bilan vaqt bo'yicha ~18 MiB li mp4 qismlarga bo'ladi va
    har birini Bot API `sendVideo` (supports_streaming) bilan kanalga yuboradi. Bot `getFile` 20 MB gacha beradi, shuning uchun qism <= 19 MiB —
    kodlovchi ularni yana Bot API bilan (akkaunt/FloodWait'siz) olib `concat` qiladi. Bot tokeni va kanal — `botup.enc` (kalit — ENCODE_TOKEN).
EN: the Actions runner (polling the queue itself, no worker in the data path) splits the video by TIME into ~18 MiB playable mp4 parts (`-c copy`) and sends each
    via Bot API `sendVideo`; the encoder later fetches them with `getFile` (<= 20 MB) and concatenates. Bot token + channel live in `botup.enc` (key: ENCODE_TOKEN).
RU: ranner Actions sam (polling) rezhet video po VREMENI na ~18 MiB mp4-chasti i shlet kazhduyu cherez Bot API sendVideo; kodirovshchik skachivaet ikh getFile i skleivaet.
"""
import json
import os
import subprocess
import time
import urllib.error
import urllib.parse
import urllib.request
import uuid
from pathlib import Path

import stage

HERE = Path(__file__).resolve().parent
# Bot `getFile` 20 MB gacha beradi: qism 18 MiB atrofida, hech qachon 19 MiB dan oshmaydi.
TARGET = 18 * 1048576
LIMIT = 19 * 1048576
# `tg_files` da bot qismlarining xabar raqami shu qadar siljitiladi: asosiy kanal xabarlari bilan to'qnashmasin (worker `BOT_OFFSET` bilan bir xil).
OFFSET = 10_000_000_000
API = "https://api.telegram.org"


def creds(key: str, path=None):
    """`botup.enc` dan (token, kanal) — AES-256-CBC + PBKDF2, kalit ENCODE_TOKEN. Fayl yo'q yoki ochilmasa `None`."""
    p = Path(path) if path else HERE / "botup.enc"
    if not p.exists():
        return None
    k = "".join(str(key).split())
    r = subprocess.run(["openssl", "enc", "-d", "-aes-256-cbc", "-pbkdf2", "-iter", "200000", "-pass", "env:K", "-in", str(p)],
                       env={**os.environ, "K": k}, capture_output=True)
    if r.returncode != 0:
        return None
    lines = r.stdout.decode().split("\n")
    tok, ch = lines[0].strip(), (lines[1].strip() if len(lines) > 1 else "")
    return (tok, int(ch)) if tok and ch.lstrip("-").isdigit() else None


def split(src, outdir, duration: float, size: int):
    """Vaqt bo'yicha >=2 ta qism (har biri <= LIMIT)."""
    return stage.split_playable(src, outdir, duration, size, target=TARGET, limit=LIMIT)


def _multipart(fields: dict, file_field: str, path: Path, filename: str):
    b = "----aru" + uuid.uuid4().hex
    out = bytearray()
    for k, v in fields.items():
        out += f'--{b}\r\nContent-Disposition: form-data; name="{k}"\r\n\r\n{v}\r\n'.encode()
    out += (f'--{b}\r\nContent-Disposition: form-data; name="{file_field}"; filename="{filename}"\r\n'
            f"Content-Type: application/octet-stream\r\n\r\n").encode()
    out += Path(path).read_bytes()
    out += f"\r\n--{b}--\r\n".encode()
    return bytes(out), f"multipart/form-data; boundary={b}"


def _call(token: str, method: str, data: bytes, ctype: str, tries: int = 8, timeout: int = 300):
    """Bot API chaqiruvi: 429 (`retry_after`) va vaqtincha xatolarda qayta uriniladi. Qaytadi: `result`."""
    err = ""
    for t in range(tries):
        req = urllib.request.Request(f"{API}/bot{token}/{method}", data=data, headers={"Content-Type": ctype})
        try:
            with urllib.request.urlopen(req, timeout=timeout) as r:
                d = json.loads(r.read() or b"{}")
            if d.get("ok"):
                return d["result"]
            err = str(d.get("description"))
        except urllib.error.HTTPError as e:
            try:
                d = json.loads(e.read() or b"{}")
            except Exception:  # noqa: BLE001
                d = {}
            err = f"HTTP {e.code} {d.get('description', '')}"
            if e.code == 429:
                time.sleep(min(int((d.get("parameters") or {}).get("retry_after", 5)) + 1, 120))
                continue
            if 400 <= e.code < 500:
                raise RuntimeError(f"Telegram: {err}")
        except Exception as e:  # noqa: BLE001 — tarmoq uzilishi
            err = f"{type(e).__name__}: {e}"
        time.sleep(min(2 ** t, 30))
    raise RuntimeError(f"Telegram: {err}")


def send_video(token: str, chat: int, path, duration=0, width=0, height=0, caption=""):
    """Qismni kanalga OCHILADIGAN video qilib yuboradi. Qaytadi: (xabar raqami, file_id)."""
    path = Path(path)
    f = {"chat_id": chat, "supports_streaming": "true", "disable_notification": "true"}
    if duration:
        f["duration"] = int(duration)
    if width and height:
        f["width"], f["height"] = int(width), int(height)
    if caption:
        f["caption"] = caption[:1024]
    body, ctype = _multipart(f, "video", path, path.name)
    m = _call(token, "sendVideo", body, ctype)
    media = m.get("video") or m.get("document") or {}
    if not media.get("file_id"):
        raise RuntimeError("Telegram file_id bermadi")
    return int(m["message_id"]), str(media["file_id"])


def delete_message(token: str, chat: int, msg: int):
    try:
        _call(token, "deleteMessage", json.dumps({"chat_id": chat, "message_id": msg}).encode(), "application/json", tries=3, timeout=30)
    except Exception:  # noqa: BLE001 — o'chmasa ham jarayon to'xtamaydi
        pass


def get_file(token: str, fid: str, dst) -> int:
    """`file_id` bo'yicha faylni `dst` ga yuklaydi (<= 20 MB). Qaytadi: bayt."""
    r = _call(token, "getFile", json.dumps({"file_id": fid}).encode(), "application/json", tries=6, timeout=60)
    url = f"{API}/file/bot{token}/{r['file_path']}"
    err = ""
    for t in range(6):
        try:
            with urllib.request.urlopen(url, timeout=120) as resp:
                data = resp.read()
            if data:
                Path(dst).write_bytes(data)
                return len(data)
            err = "bo'sh javob"
        except Exception as e:  # noqa: BLE001
            err = f"{type(e).__name__}: {e}"
        time.sleep(min(2 ** t, 20))
    raise RuntimeError(f"Telegram fayl yuklanmadi: {err}")


def fetch_origin(key: str, fids, dst, workdir, log=print):
    """Qismlarni (`file_id` lar tartib bilan) Bot API bilan olib, `-c copy` bilan bitta mp4 (`dst`) ga ulaydi."""
    c = creds(key)
    if not c:
        raise RuntimeError("botup.enc ochilmadi (ENCODE_TOKEN noto'g'ri yoki fayl yo'q)")
    token, _chat = c
    wd = Path(workdir)
    wd.mkdir(parents=True, exist_ok=True)
    paths = []
    for i, fid in enumerate(fids):
        p = wd / f"bpart{i:03d}.mp4"
        n = get_file(token, fid, p)
        log(f"  asl video qismi {i + 1}/{len(fids)} yuklandi ({n / 1048576:.1f} MB)")
        paths.append(p)
    stage.concat_parts(paths, dst)
    for p in paths:
        p.unlink(missing_ok=True)
