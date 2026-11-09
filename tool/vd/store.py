#!/usr/bin/env python3
"""UZ: Saqlovchi (alohida workflow `vdstore.yml`): R2 dagi tayyor bo'laklarni (`vd/c/<nom>/<n>` yoki ilova yuklagan butun fayl) Bot API
    `sendDocument` bilan Telegram kanaliga saqlaydi va `file_id` larni worker'ga yozadi. Kodlash/shifrlash YO'Q — bo'laklar tayyor keladi.
    ARUDRIVE `tool/bdrive/chunk.py` bilan bir xil usul. Run o'lsa ijara tugagach ish qayta navbatga qaytadi, tayyor bo'laklar QAYTA yuklanmaydi.
EN: Uploader: stores staged chunks from R2 into the Telegram channel via Bot API and records the file_ids (same method as ARUDRIVE chunk.py).
RU: Zagruzchik: sohranyaet gotovye kuski iz R2 v kanal Telegram cherez Bot API i zapisyvaet file_id (kak ARUDRIVE chunk.py).
"""
import hashlib
import json
import os
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
import uuid

import vdlib
from vdlib import api, log

BUDGET = int(float(os.environ.get("START_BUDGET_MIN", "330")) * 60)
T0 = time.time()


def multipart(fields, file_field, filename, data):
    b = uuid.uuid4().hex
    parts = []
    for k, v in fields.items():
        parts.append(f'--{b}\r\nContent-Disposition: form-data; name="{k}"\r\n\r\n{v}\r\n'.encode())
    parts.append(f'--{b}\r\nContent-Disposition: form-data; name="{file_field}"; filename="{filename}"\r\n'
                 f"Content-Type: application/octet-stream\r\n\r\n".encode())
    parts.append(data)
    parts.append(f"\r\n--{b}--\r\n".encode())
    return b"".join(parts), f"multipart/form-data; boundary={b}"


def tg_send(token, channel, filename, data, caption=""):
    """Bot API `sendDocument`: (file_id, message_id). Telegram 429 bersa `retry_after` kutiladi; hajm Telegram javobi bilan solishtiriladi."""
    fields = {"chat_id": channel, "disable_notification": "true"}
    if caption:
        fields["caption"] = caption
    body, ctype = multipart(fields, "document", filename, data)
    url = f"https://api.telegram.org/bot{token}/sendDocument"
    last = None
    for i in range(6):
        try:
            req = urllib.request.Request(url, data=body, headers={"Content-Type": ctype}, method="POST")
            with urllib.request.urlopen(req, timeout=600) as r:
                d = json.loads(r.read())
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
    name, chunks = job["name"], int(job["chunks"])
    token, channel = job["bot_token"], job["channel"]
    if not token or not channel:
        raise RuntimeError("worker bot tokeni yoki kanalni bermadi")
    print("::add-mask::" + token)
    done = {int(x) for x in job.get("done", [])}
    vid = int(job.get("vid") or 0)          # fayl RAQAMI (bazadagi ketma-ket raqam): hujjat nomi `vid_<raqam>_<bo'lak>.bin`
    label = job.get("label") or name
    log(f"{name}: {chunks} bo'lak, tayyor {len(done)}")
    pending = []   # bazaga yozilmagan bo'laklar: har 5 tasida bitta so'rov (Turso yozuvlari kamayadi); uzilsa ular qayta yuboriladi

    def flush():
        if pending:
            api("/api/vd/chunkdone", {"name": name, "items": list(pending)})
            pending.clear()

    for n in range(chunks):
        if n in done:
            continue
        data = vdlib._call(f"/api/vd/read?name={urllib.parse.quote(name)}&n={n}", method="GET", raw=True)
        fid, mid = tg_send(token, channel, f"vid_{vid}_{n + 1}.bin", data)
        pending.append({"n": n, "tg_file_id": fid, "msg_id": mid})
        if len(pending) >= 5 or n == chunks - 1:
            flush()
        if n % 5 == 0 or n == chunks - 1:
            log(f"  bo'lak {n + 1}/{chunks} yuborildi ({len(data) // 1024} KB)")
    flush()
    api("/api/vd/done", {"name": name})
    log(f"{name}: tayyor")


def main():
    if not vdlib.TOKEN:
        sys.exit("ENCODE_TOKEN yo'q")
    worked = 0
    while time.time() - T0 < BUDGET:
        job = api("/api/vd/claim", {})
        if job.get("none"):
            log("Navbat bo'sh.")
            break
        try:
            process(job)
            worked += 1
        except Exception as e:  # noqa: BLE001
            log(f"XATO {job.get('name')}: {e}")
            try:
                api("/api/vd/fail", {"name": job["name"], "error": str(e)[:200]})
            except Exception:  # noqa: BLE001
                pass
            time.sleep(5)
    out = os.environ.get("GITHUB_OUTPUT")
    if out:
        with open(out, "a") as fh:
            fh.write(f"worked={'1' if worked else '0'}\n")


if __name__ == "__main__":
    main()
