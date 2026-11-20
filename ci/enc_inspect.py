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
    d = Path(tempfile.mkdtemp())
    f = d / "src.mp4"
    try:
        n = vdlib.download(name, f)
        print("yuklandi:", n, "bayt =", round(n / 1048576, 1), "MiB, bo'laklar:", vdlib.chunks_of(n))
    except Exception as e:  # noqa: BLE001
        print("YUKLASH XATOSI:", type(e).__name__, str(e)[:300])
        continue
    print("ffprobe:", run(["ffprobe", "-v", "error", "-show_entries", "format=duration,bit_rate,size:stream=index,codec_type,codec_name,width,height,duration,nb_frames", "-of", "json", str(f)]).replace("\n", " ")[:900])
    # moov/mdat tuzilmasi
    print("atomlar:", run(["ffprobe", "-v", "trace", "-i", str(f)], 120).count("type:'moov'"), "ta moov belgisi")
    out = run(["ffmpeg", "-v", "error", "-nostdin", "-i", str(f), "-map", "0:v:0", "-f", "null", "-progress", "pipe:1", "-"], 1800)
    errs = [l for l in out.splitlines() if l and not l.startswith(("frame=", "fps=", "stream_", "bitrate=", "total_size=", "out_time", "dup_frames", "drop_frames", "speed=", "progress=")) ]
    times = [l for l in out.splitlines() if l.startswith("out_time_us=")]
    last = int(times[-1].split("=")[1]) / 1e6 if times else 0
    print(f"ffmpeg video-dekod: oxirgi vaqt {last:.1f} s; xato qatorlari: {len(errs)}")
    for l in errs[:15]:
        print("   ", l[:200])
    # 18 MiB chegaralari bilan solishtirish: dekod to'xtagan joyga eng yaqin bayt
    if last and n:
        print(f"to'xtagan vaqt ≈ fayl ulushi {last / max(1.0, float(j.get('duration') or 1478)):.2%} (taxmin)")
    f.unlink()
