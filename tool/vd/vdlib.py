"""UZ: Video ombori (`vd`) uchun umumiy kutubxona: shifrlash (AES-128-CTR, bitta kalit, hisoblagich fayl bo'ylab davom etadi),
18 MiB bo'laklarga bo'lish va worker bilan gaplashish. Migratsiya (`mig.py`), saqlovchi (`store.py`) va kodlovchi (`tool/encode/run.py`) shuni ishlatadi.
EN: Shared library for the video store (`vd`): AES-128-CTR with ONE key (the counter continues across the file, so any 16-byte aligned
    offset is decryptable on its own), 18 MiB chunking and the worker API client.
RU: Obshchaya biblioteka hranilishcha video (`vd`): AES-128-CTR odnim klyuchom, schetchik prodolzhaetsya po vsemu faylu, kuski po 18 MiB.
"""
import json
import os
import time
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path

from cryptography.hazmat.primitives.ciphers import Cipher, algorithms, modes

CHUNK = 18 * 1024 * 1024
API = (os.environ.get("API_BASE") or os.environ.get("VD_API") or "https://arumediatv.uzcom.workers.dev").rstrip("/")
TOKEN = "".join(os.environ.get("ENCODE_TOKEN", "").split())
UA = "arumediatv-vd"
# MP4 fayl boshidagi qutilar: kalit to'g'ri ochilganini tekshirish uchun (migratsiyada).
MP4_ATOMS = (b"ftyp", b"moov", b"mdat", b"free", b"wide", b"skip")


def log(*a):
    print(time.strftime("%H:%M:%S"), *a, flush=True)


def ctr_at(key: bytes, offset: int):
    """AES-128-CTR shifrlovchisi, fayldagi `offset` (16 ga karrali) baytdan boshlab. Eski fayllar (IV nol, 128-bit big-endian
    hisoblagich) bilan bir xil: bo'lak boshi = `offset // 16`-blok."""
    if offset % 16:
        raise ValueError("offset 16 ga karrali bo'lishi shart")
    return Cipher(algorithms.AES(key), modes.CTR((offset // 16).to_bytes(16, "big"))).encryptor()


def apply(key: bytes, offset: int, data: bytes) -> bytes:
    """CTR shifrlash == shifrni ochish (bir xil amal)."""
    return ctr_at(key, offset).update(data)


def chunks_of(size: int) -> int:
    return (size + CHUNK - 1) // CHUNK


def iter_chunks(path, new_key: bytes, old_key: bytes | None = None, check_mp4: bool = False, plain: bool = False):
    """Fayldan ketma-ket `(n, bo'lak_baytlari)`. `old_key` berilsa eski kalit bilan ochilib YANGI kalit bilan shifrlanadi (migratsiya);
    berilmasa bo'lak allaqachon `new_key` bilan shifrlangan deb olinadi (kodlovchi) va shunchaki kesib beriladi."""
    with open(path, "rb") as f:
        n = 0
        while True:
            raw = f.read(CHUNK)
            if not raw:
                break
            if plain:
                raw = apply(new_key, n * CHUNK, raw)          # fayl ochiq (kalitsiz): faqat YANGI kalit bilan shifrlanadi
            elif old_key is not None:
                dec = apply(old_key, n * CHUNK, raw)
                if check_mp4 and n == 0 and dec[4:8] not in MP4_ATOMS:
                    raise RuntimeError("eski kalit bilan ochilgan fayl MP4 emas (kalit mos kelmadi?)")
                raw = apply(new_key, n * CHUNK, dec)
            yield n, raw
            n += 1


def _call(path, body=None, method="POST", data=None, ctype=None, raw=False, timeout=300, tries=6):
    url = API + path
    h = {"X-Encode-Token": TOKEN, "User-Agent": UA}
    if body is not None:
        data = json.dumps(body).encode()
        ctype = "application/json"
    if ctype:
        h["Content-Type"] = ctype
    last = None
    for i in range(tries):
        try:
            req = urllib.request.Request(url, data=data, headers=h, method=method)
            with urllib.request.urlopen(req, timeout=timeout) as r:
                out = r.read()
            return out if raw else json.loads(out or b"{}")
        except urllib.error.HTTPError as e:
            msg = e.read()[:300]
            if e.code < 500 and e.code != 429:
                raise RuntimeError(f"worker {path}: HTTP {e.code} {msg!r}")
            last = f"HTTP {e.code}"
        except (urllib.error.URLError, TimeoutError, ConnectionError, OSError) as e:
            last = str(e)
        time.sleep(3 * (i + 1))
    raise RuntimeError(f"worker {path}: {last}")


def api(path, body=None):
    return _call(path, body if body is not None else {})


def stage_file(path, name: str, key_hex: str, old_key_hex: str | None = None, check_mp4: bool = False, progress=None, plain: bool = False) -> str:
    """Faylni 18 MiB bo'laklarga bo'lib R2 ga yozadi (`init` -> `put`... -> `finish`). Qaytadi: `ok` (R2 da, saqlash navbatida),
    `ready` (allaqachon yangi tizimda) yoki `wait` (R2 da joy yo'q — keyinroq urinib ko'ring). Xato bo'lsa istisno."""
    size = Path(path).stat().st_size
    r = api("/api/vd/init", {"name": name, "size": size, "key": key_hex})
    if r.get("ready"):
        return "ready"
    if r.get("wait"):
        return "wait"
    new_key = bytes.fromhex(key_hex)
    old_key = bytes.fromhex(old_key_hex) if old_key_hex else None
    total = chunks_of(size)
    for n, data in iter_chunks(path, new_key, old_key, check_mp4, plain):
        _call(f"/api/vd/put?name={urllib.parse.quote(name)}&n={n}", method="PUT", data=data, ctype="application/octet-stream", timeout=600)
        if progress:
            progress(n + 1, total)
    api("/api/vd/finish", {"name": name})
    return "ok"


def download(name: str, dst, progress=None) -> int:
    """Yangi tizimdagi (`vd`) tayyor faylni yuklab, shifrini ochib `dst` ga yozadi (kodlash uchun asl video). Bo'laklar Telegram'dan Bot API
    (`getFile`, har biri <= 18 MiB) bilan olinadi; kalit worker'dan (ENCODE_TOKEN). Qaytadi: yozilgan bayt."""
    m = api("/api/vd/dl", {"name": name})
    token = m["bot_token"]
    print("::add-mask::" + token)
    key = bytes.fromhex(m["key"])
    ids = m["chunk_ids"]
    size = int(m["size"])
    got = 0
    with open(dst, "wb") as f:
        for n, fid in enumerate(ids):
            data = None
            for attempt in range(6):
                try:
                    with urllib.request.urlopen(f"https://api.telegram.org/bot{token}/getFile?file_id={urllib.parse.quote(fid)}", timeout=60) as r:
                        path = json.loads(r.read())["result"]["file_path"]
                    with urllib.request.urlopen(f"https://api.telegram.org/file/bot{token}/{path}", timeout=300) as r:
                        data = r.read()
                    if len(data) == min(CHUNK, size - n * CHUNK):
                        break
                    data = None
                except (urllib.error.URLError, TimeoutError, ConnectionError, KeyError, ValueError, OSError):
                    data = None
                time.sleep(3 * (attempt + 1))
            if data is None:
                raise RuntimeError(f"bo'lak {n} Telegram'dan olinmadi")
            f.write(apply(key, n * CHUNK, data))
            got += len(data)
            if progress:
                progress(got, size)
    if got != size:
        raise RuntimeError(f"hajm mos emas ({got} != {size})")
    return got
