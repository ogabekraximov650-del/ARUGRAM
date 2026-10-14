"""Telegram'ga yuklanadigan fayllarni R2'ga qo'yish (Post, Anibla, To'plam workflow'lari uchun umumiy).

Fayl Telegram'ga bu run'ning o'zi yuklamaydi: R2'ga yoziladi (`worker/src/r2.rs`), `upload.yml` yuklovchisi uni `dest` tavsifi bo'yicha
yuboradi va `callback` ni chaqiradi. Kalitlar bo'lsa R2'ga to'g'ridan-to'g'ri (imzolangan havola), bo'lmasa worker orqali bo'laklab.
`deliver()` `False` qaytarsa (R2 yo'q yoki ishlamadi) — chaqiruvchi eski usulda (to'g'ridan-to'g'ri Telegram'ga) yuboradi.
"""
import json
import time
import urllib.parse
import urllib.request
from pathlib import Path


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


def _post(api_base, token, path, body, ua):
    req = urllib.request.Request(
        f"{api_base}{path}", data=json.dumps(body).encode(), method="POST",
        headers={"X-Encode-Token": token, "Content-Type": "application/json", "User-Agent": ua})
    with urllib.request.urlopen(req, timeout=60) as r:
        return json.loads(r.read() or b"{}")


def _put_direct(url, path: Path, ua):
    size = path.stat().st_size
    with open(path, "rb") as f:
        req = urllib.request.Request(url, data=_Slice(f, size), method="PUT",
                                     headers={"Content-Length": str(size), "User-Agent": ua})
        with urllib.request.urlopen(req, timeout=1800) as r:
            r.read()


def _put_parts(api_base, token, path: Path, name, part_size, ua):
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
                        f"{api_base}/api/r2/part?file={urllib.parse.quote(name)}&n={n}", data=_Slice(f, length), method="PUT",
                        headers={"X-Encode-Token": token, "Content-Type": "application/octet-stream",
                                 "Content-Length": str(length), "User-Agent": ua})
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


def deliver(api_base: str, token: str, path, name: str, dest: dict, log=print, ua: str = "arugram-r2") -> bool:
    """`path` faylini R2'ga yozadi va yuklovchiga `dest` (qayerga/qanday yuborish, `callback`, `lease`) bilan topshiradi.

    `name` — noyob, xavfsiz fayl nomi (`[A-Za-z0-9._-]`). `True` — R2'ga yozildi (yuklovchi yetkazadi); `False` — R2 yo'q/ishlamadi."""
    path, api_base = Path(path), api_base.rstrip("/")
    size = path.stat().st_size
    waited = 0
    for attempt in range(1, 4):
        try:
            while True:
                r = _post(api_base, token, "/api/r2/put", {"file": name, "size": size, "dest": dest}, ua)
                if not r.get("wait"):
                    break
                if not waited:
                    log("  R2'da joy yo'q — bo'shashi kutilmoqda...")
                waited += 30
                if waited > 8 * 3600:
                    raise RuntimeError("R2 8 soat bo'shamadi")
                time.sleep(30)
            if r.get("url"):
                _put_direct(r["url"], path, ua)
                parts = []
            else:
                parts = _put_parts(api_base, token, path, name, int(r.get("part_size") or 94371840), ua)
            _post(api_base, token, "/api/r2/ready", {"file": name, "size": size, "parts": parts}, ua)
            log(f"  {name} R2'ga yozildi ({size / 1048576:.1f} MB) — Telegram'ga yuklovchi workflow yetkazadi")
            return True
        except Exception as ex:  # noqa: BLE001
            if "503" in str(ex):
                return False
            log(f"  R2'ga yozish {attempt}/3 muvaffaqiyatsiz: {ex}")
            time.sleep(4 * attempt)
    return False
