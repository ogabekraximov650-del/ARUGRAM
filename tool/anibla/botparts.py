"""Anibla videosini BOT API orqali kanalga BAYT bo'yicha ANIQ 20 MiB li FAYL qismlarida yuklash va qaytarib olish.

UZ: Actions ishchisi worker'siz, o'zi (polling: navbatdan `claim`) videoni yuklab, bayt bo'yicha 20 MiB (20 971 520 bayt) li qismlarga bo'ladi (oxirgisi qisqaroq)
    va har birini Bot API `sendDocument` bilan kanalga fayl qilib yuboradi. Bot `getFile` aynan 20 MiB gacha beradi (20 971 521 bayt — "file is too big"; sinalgan),
    shuning uchun qism 20 MiB dan kam ham, ko'p ham emas. Kodlovchi qismlarni Bot API bilan (akkaunt/FloodWait'siz) olib bayt bo'yicha ulaydi.
    Bot tokeni va kanal — `botup.enc` (kalit — ENCODE_TOKEN).
EN: the Actions runner (polling the queue itself) splits the video by BYTES into exact 20 MiB file parts and sends each via Bot API `sendDocument`;
    the encoder fetches them with `getFile` (limit is exactly 20 MiB) and joins them byte-wise. Bot token + channel live in `botup.enc`.
RU: ranner Actions rezhet video po BAJTAM rovno na 20 MiB i shlet kazhduyu chast cherez sendDocument; kodirovshchik skachivaet getFile i skleivaet po bajtam.
"""
import json
import os
import shutil
import subprocess
import time
import urllib.error
import urllib.parse
import urllib.request
import uuid
from pathlib import Path

import stage

HERE = Path(__file__).resolve().parent
# Bot `getFile` AYNAN 20 MiB (20 971 520 bayt) gacha beradi (sinalgan: 20 971 521 -> "file is too big"). Qism shu hajmda, oxirgisi qisqaroq.
PART = 20 * 1048576
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


def iter_parts(src, outdir, part: int = PART):
    """Faylni bayt bo'yicha `part` li qismlarga bo'ladi (bittadan yaratadi: `(raqam, jami, yo'l)`; yuborilgach chaqiruvchi o'chiradi — disk ikki baravar band bo'lmaydi).
    Fayl bitta qismga sig'sa ham worker >=2 qism kutadi — yarmidan bo'linadi."""
    src = Path(src)
    size = src.stat().st_size
    if size <= 0:
        raise RuntimeError("fayl bo'sh")
    if size <= part:
        part = (size + 1) // 2
    n = -(-size // part)
    outdir = Path(outdir)
    outdir.mkdir(parents=True, exist_ok=True)
    with open(src, "rb") as f:
        for i in range(n):
            p = outdir / f"part{i:04d}.bin"
            left = min(part, size - i * part)
            with open(p, "wb") as o:
                while left > 0:
                    chunk = f.read(min(8 * 1048576, left))
                    if not chunk:
                        raise RuntimeError("fayl o'qishda uzildi")
                    o.write(chunk)
                    left -= len(chunk)
            yield i, n, p


def _multipart(fields: dict, files):
    """`files` — [(maydon, yo'l, fayl nomi)]."""
    b = "----aru" + uuid.uuid4().hex
    out = bytearray()
    for k, v in fields.items():
        out += f'--{b}\r\nContent-Disposition: form-data; name="{k}"\r\n\r\n{v}\r\n'.encode()
    for field, path, filename in files:
        out += (f'--{b}\r\nContent-Disposition: form-data; name="{field}"; filename="{filename}"\r\n'
                f"Content-Type: application/octet-stream\r\n\r\n").encode()
        out += Path(path).read_bytes()
        out += b"\r\n"
    out += f"--{b}--\r\n".encode()
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


def send_document(token: str, chat: int, path, filename: str, caption: str = ""):
    """Qismni kanalga FAYL qilib yuboradi. Qaytadi: (xabar raqami, file_id)."""
    f = {"chat_id": chat, "disable_notification": "true", "disable_content_type_detection": "true"}
    if caption:
        f["caption"] = caption[:1024]
    body, ctype = _multipart(f, [("document", path, filename)])
    m = _call(token, "sendDocument", body, ctype)
    doc = m.get("document") or {}
    if not doc.get("file_id"):
        raise RuntimeError("Telegram file_id bermadi")
    return int(m["message_id"]), str(doc["file_id"])


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


def _is_mp4(p) -> bool:
    with open(p, "rb") as f:
        return f.read(12)[4:8] == b"ftyp"


def fetch_origin(key: str, fids, dst, workdir, log=print):
    """Qismlarni (`file_id` lar tartib bilan) Bot API bilan olib bitta fayl (`dst`) ga ulaydi: bayt bo'yicha qismlar — to'g'ridan-to'g'ri ulanadi;
    har biri mustaqil mp4 bo'lgan eski qismlar (oldingi usul) — ffmpeg `concat -c copy` bilan."""
    c = creds(key)
    if not c:
        raise RuntimeError("botup.enc ochilmadi (ENCODE_TOKEN noto'g'ri yoki fayl yo'q)")
    token, _chat = c
    wd = Path(workdir)
    wd.mkdir(parents=True, exist_ok=True)
    paths = []
    for i, fid in enumerate(fids):
        p = wd / f"bpart{i:04d}.bin"
        n = get_file(token, fid, p)
        log(f"  asl video qismi {i + 1}/{len(fids)} yuklandi ({n / 1048576:.1f} MB)")
        paths.append(p)
    if len(paths) > 1 and all(_is_mp4(p) for p in paths):
        stage.concat_parts(paths, dst)
    else:
        with open(dst, "wb") as out:
            for p in paths:
                with open(p, "rb") as f:
                    shutil.copyfileobj(f, out, 8 * 1048576)
    for p in paths:
        p.unlink(missing_ok=True)
