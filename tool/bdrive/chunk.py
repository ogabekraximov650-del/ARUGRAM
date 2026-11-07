#!/usr/bin/env python3
"""ARUDRIVE: R2'dagi faylni 18 MB bo'laklarga bo'lib Bot API orqali Telegram kanaliga saqlaydi (KODLASH YO'Q).

Ikkinchi akkauntdagi repoda (`avtoencode`) `bdrive.yml` orqali ishlaydi (worker `arudrive` ishga tushiradi). Faqat worker bilan
gaplashadi (`ENCODE_TOKEN`): navbatdagi ishni `claim` qiladi (bot tokeni va kanal ham shu javobda keladi), har bo'lakni
`/api/bd/read` bilan R2'dan oladi, Bot API `sendDocument` bilan kanalga yuboradi va `chunkdone` ga yozadi; oxirida `done`.
Run o'lib qolsa ish ijarasi tugagach qayta navbatga qaytadi va tayyor bo'laklar QAYTA yuklanmaydi (`done` ro'yxati).
"""
import json
import os
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
import uuid

API = os.environ.get("BD_API", "https://arudrive.uzcom.workers.dev").rstrip("/")
TOKEN = os.environ.get("ENCODE_TOKEN", "").strip()
BUDGET = int(os.environ.get("START_BUDGET_MIN", "100")) * 60
T0 = time.time()


def log(*a):
    print(time.strftime("%H:%M:%S"), *a, flush=True)


def http(url, data=None, headers=None, method=None, timeout=300):
    req = urllib.request.Request(url, data=data, headers=headers or {}, method=method)
    with urllib.request.urlopen(req, timeout=timeout) as r:
        return r.status, r.read()


def worker(path, body=None, method="POST", raw=False):
    """Worker so'rovi (qayta urinish bilan). `raw=True` — baytlar qaytadi."""
    url = API + path
    h = {"X-Encode-Token": TOKEN}
    data = None
    if body is not None:
        data = json.dumps(body).encode()
        h["Content-Type"] = "application/json"
    last = None
    for i in range(5):
        try:
            st, out = http(url, data, h, method)
            return out if raw else json.loads(out or b"{}")
        except urllib.error.HTTPError as e:
            msg = e.read()[:200]
            if e.code < 500 and e.code != 429:
                raise RuntimeError(f"worker {path}: HTTP {e.code} {msg!r}")
            last = f"HTTP {e.code}"
        except (urllib.error.URLError, TimeoutError, ConnectionError) as e:
            last = str(e)
        time.sleep(3 * (i + 1))
    raise RuntimeError(f"worker {path}: {last}")


def multipart(fields, file_field, filename, data):
    """multipart/form-data tanasi: (tana, Content-Type)."""
    b = uuid.uuid4().hex
    parts = []
    for k, v in fields.items():
        parts.append(f'--{b}\r\nContent-Disposition: form-data; name="{k}"\r\n\r\n{v}\r\n'.encode())
    parts.append(
        f'--{b}\r\nContent-Disposition: form-data; name="{file_field}"; filename="{filename}"\r\n'
        f"Content-Type: application/octet-stream\r\n\r\n".encode()
    )
    parts.append(data)
    parts.append(f"\r\n--{b}--\r\n".encode())
    return b"".join(parts), f"multipart/form-data; boundary={b}"


def tg_send(token, channel, name, data):
    """Bot API `sendDocument`: (file_id, message_id). Telegram 429 bersa `retry_after` kutiladi."""
    body, ctype = multipart({"chat_id": channel, "disable_notification": "true"}, "document", name, data)
    url = f"https://api.telegram.org/bot{token}/sendDocument"
    last = None
    for i in range(6):
        try:
            st, out = http(url, body, {"Content-Type": ctype}, "POST", timeout=600)
            d = json.loads(out)
            doc = d["result"]["document"]
            if int(doc.get("file_size", len(data))) != len(data):
                raise RuntimeError("Telegram hajmi mos emas")
            return doc["file_id"], d["result"]["message_id"]
        except urllib.error.HTTPError as e:
            try:
                d = json.loads(e.read())
            except Exception:  # noqa: BLE001
                d = {}
            if e.code == 429:
                time.sleep(int(d.get("parameters", {}).get("retry_after", 5)) + 1)
                last = "429"
                continue
            if e.code in (400, 401, 403):
                raise RuntimeError(f"Telegram rad etdi: {d.get('description', e.code)}")
            last = f"HTTP {e.code}"
        except (urllib.error.URLError, TimeoutError, ConnectionError, KeyError, ValueError) as e:
            last = str(e)
        time.sleep(4 * (i + 1))
    raise RuntimeError(f"Telegram yuborilmadi: {last}")


def process(job):
    jid, chunks = job["id"], int(job["chunks"])
    token, channel = job["bot_token"], job["channel"]
    if not token or not channel:
        raise RuntimeError("worker bot tokeni yoki kanalni bermadi (BD_BOT_TOKEN / TG_CHANNEL_ID)")
    print("::add-mask::" + token)
    done = {int(x) for x in job.get("done", [])}
    log(f"{jid}: {job['name']} — {chunks} bo'lak, tayyor {len(done)}")
    for n in range(chunks):
        if n in done:
            continue
        data = worker(f"/api/bd/read?id={jid}&n={n}", method="GET", raw=True)
        fid, mid = tg_send(token, channel, f"bd_{jid}_{n}", data)
        worker("/api/bd/chunkdone", {"id": jid, "n": n, "tg_file_id": fid, "size": len(data), "msg_id": mid})
        log(f"  bo'lak {n + 1}/{chunks} yuborildi ({len(data) // 1024} KB)")
    worker("/api/bd/done", {"id": jid})
    log(f"{jid}: tayyor")


def main():
    if not TOKEN:
        sys.exit("ENCODE_TOKEN yo'q")
    while time.time() - T0 < BUDGET:
        job = worker("/api/bd/claim", {})
        if job.get("none"):
            log("navbat bo'sh")
            return
        try:
            process(job)
        except Exception as e:  # noqa: BLE001
            log(f"XATO {job.get('id')}: {e}")
            try:
                worker("/api/bd/fail", {"id": job["id"], "why": str(e)[:200]})
            except Exception:  # noqa: BLE001
                pass
            time.sleep(5)


if __name__ == "__main__":
    main()
