#!/usr/bin/env python3
# ci/enc_inspect.py — FAQAT O'QISH: kodlash navbatidagi qismlarning asl videosini (vd ombori) yuklab, butunligini tekshiradi
# ("davomiylik mos emas" tashxisi). `ci/ENC_INSPECT`: `epizod_id` lar (masalan `40 49 41`; 41 — salomat qism, solishtirish uchun).
# Har biri: encode_jobs.origin (vd fayl nomi) -> vdlib.download -> hajm, ffprobe, ffmpeg bilan to'liq o'qish (birinchi xato vaqti), bo'lak chegaralari.
import json, os, re, subprocess, sys, tempfile, urllib.request
from pathlib import Path

sys.path.insert(0, "tool/vd")
import vdlib  # noqa: E402

url = os.environ["TURSO_URL"].strip()
token = os.environ["TURSO_TOKEN"].strip()
if url.startswith("libsql://"):
    url = "https://" + url[len("libsql://"):]
ids = [int(t) for t in open("ci/ENC_INSPECT").read().split() if t[0].isdigit()]
vdlib.API, vdlib.TOKEN = vdlib.API, "".join(os.environ.get("ENCODE_TOKEN", "").split())


def q(sql, args=()):
    body = {"requests": [{"type": "execute", "stmt": {"sql": sql, "args": [{"type": "integer", "value": str(x)} for x in args]}}, {"type": "close"}]}
    req = urllib.request.Request(url + "/v2/pipeline", data=json.dumps(body).encode(), headers={"Authorization": "Bearer " + token, "Content-Type": "application/json"})
    with urllib.request.urlopen(req, timeout=60) as r:
        res = json.load(r)["results"][0]
    if res["type"] != "ok":
        raise RuntimeError(res)
    r = res["response"]["result"]
    return [dict(zip([c["name"] for c in r["cols"]], [c.get("value") for c in row])) for row in r["rows"]]


def run(cmd, timeout=1800):
    p = subprocess.run(cmd, capture_output=True, text=True, timeout=timeout)
    return (p.stdout or "") + (p.stderr or "")


for eid in ids:
    print(f"\n===== epizod_id {eid} =====")
    jobs = q("SELECT * FROM encode_jobs WHERE anime_id=3 AND season_id=1 AND epizod_id=?", (eid,))
    ep = q("SELECT epizod_number,origin_size,origin_height FROM epizod_db WHERE epizod_id=?", (eid,)) if False else []
    if not jobs:
        print("encode_jobs da yozuv yo'q")
        continue
    j = jobs[0]
    print("ustunlar:", {k: (str(v)[:50] if k not in ("origin_key", "origin") else "...") for k, v in j.items() if k not in ("origin_key",)})
    name = j.get("origin") or ""
    if not name:
        print("origin bo'sh")
        continue
    esc = name.replace("\\", "\\\\").replace("%", "\\%").replace("_", "\\_")
    rows = q("SELECT file_name, msg_id, length(file_key) AS kl FROM tg_files WHERE file_name=? OR file_name LIKE ? ESCAPE '\\' ORDER BY file_name", (name, esc + ".p%")) if False else []
    import urllib.parse
    def q2(sql, a):
        body = {"requests": [{"type": "execute", "stmt": {"sql": sql, "args": [{"type": "text", "value": x} for x in a]}}, {"type": "close"}]}
        req = urllib.request.Request(url + "/v2/pipeline", data=json.dumps(body).encode(), headers={"Authorization": "Bearer " + token, "Content-Type": "application/json"})
        with urllib.request.urlopen(req, timeout=60) as r:
            res = json.load(r)["results"][0]["response"]["result"]
        return [dict(zip([c["name"] for c in res["cols"]], [c.get("value") for c in row])) for row in res["rows"]]
    for r in q2("SELECT file_name, msg_id, length(file_key) AS kl FROM tg_files WHERE file_name=? OR file_name LIKE ? ESCAPE '\\' ORDER BY file_name", (name, esc + ".p%")):
        print("tg_files:", r)
    try:
      for r in q2("SELECT epizod_number, origin_size, origin_height, CASE WHEN origin_key<>'' THEN 'kalit bor' ELSE 'kalit yoq' END AS k FROM epizod_db WHERE epizod_id=?", (str(eid),)):
        print("epizod_db:", r)
    except Exception as ex:  # noqa: BLE001
        print("epizod_db xato:", str(ex)[:150])

    continue
