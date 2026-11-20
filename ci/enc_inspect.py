#!/usr/bin/env python3
# ci/enc_inspect.py — FAQAT O'QISH: kodlashda "davomiylik mos emas" bergan qismning ASL videosini kanaldan yuklab, butunligini tekshiradi.
# `ci/ENC_INSPECT`: `anime:bo'lim:qism` (masalan `3:1:35 9:1:6`). Har biri: epizod_db -> asl video nomi -> tg_files xabari -> Pyrogram yuklab ->
# ffprobe (konteyner/oqim davomiyligi), paketlar vaqti (bo'shliq/sakrash), ffmpeg bilan to'liq dekod (birinchi xato vaqti).
import asyncio, json, os, sqlite3, subprocess, sys, tempfile, urllib.request
from pathlib import Path
import pyrogram.utils
from pyrogram import Client

pyrogram.utils.MIN_CHANNEL_ID = -1009999999999
url = os.environ["TURSO_URL"].strip()
token = os.environ["TURSO_TOKEN"].strip()
if url.startswith("libsql://"):
    url = "https://" + url[len("libsql://"):]
SESSION = os.environ["SESSION_PATH"]
chan = int(open("ci/TG_CHANNEL_ID").read().strip())
targets = [tuple(int(x) for x in t.split(":")) for t in open("ci/ENC_INSPECT").read().split() if t[0].isdigit()]


def q(sql, args=()):
    body = {"requests": [{"type": "execute", "stmt": {"sql": sql, "args": [{"type": "text", "value": str(x)} for x in args]}}, {"type": "close"}]}
    req = urllib.request.Request(url + "/v2/pipeline", data=json.dumps(body).encode(), headers={"Authorization": "Bearer " + token, "Content-Type": "application/json"})
    with urllib.request.urlopen(req, timeout=60) as r:
        res = json.load(r)["results"][0]
    if res["type"] != "ok":
        raise RuntimeError(res)
    r = res["response"]["result"]
    return [dict(zip([c["name"] for c in r["cols"]], [c.get("value") for c in row])) for row in r["rows"]]


def api_id():
    try:
        c = sqlite3.connect(f"file:{SESSION}.session?mode=ro", uri=True)
        return int(c.execute("SELECT api_id FROM sessions").fetchone()[0] or 0)
    except Exception:
        return 0


def run(cmd, timeout=3000):
    p = subprocess.run(cmd, capture_output=True, text=True, timeout=timeout)
    return (p.stdout or "") + (p.stderr or "")


def packets(path, kind):
    out = run(["ffprobe", "-v", "error", "-select_streams", kind, "-show_entries", "packet=pts_time,dts_time,duration_time,size", "-of", "csv=p=0", str(path)])
    pts = []
    for l in out.splitlines():
        a = l.split(",")
        try:
            pts.append(float(a[0]))
        except (ValueError, IndexError):
            pass
    return pts


def report(path, label):
    print(f"--- {label}: {path.stat().st_size} bayt")
    print("konteyner/oqimlar:", run(["ffprobe", "-v", "error", "-show_entries", "format=duration,bit_rate:stream=index,codec_type,codec_name,start_time,duration,nb_frames,time_base", "-of", "json", str(path)]).replace("\n", " ")[:1200])
    for kind in ("v:0", "a:0"):
        p = packets(path, kind)
        if not p:
            print(f"  [{kind}] paket yo'q")
            continue
        ps = sorted(p)
        gaps = [(round(ps[i], 1), round(ps[i + 1] - ps[i], 1)) for i in range(len(ps) - 1) if ps[i + 1] - ps[i] > 2.0]
        back = sum(1 for i in range(len(p) - 1) if p[i + 1] < p[i] - 1.0)
        print(f"  [{kind}] paketlar {len(p)}, birinchi {ps[0]:.1f}s, OXIRGI {ps[-1]:.1f}s, 2 s dan katta bo'shliqlar: {gaps[:10]}, orqaga sakrashlar: {back}")
    out = run(["ffmpeg", "-v", "error", "-nostdin", "-i", str(path), "-map", "0:v:0", "-f", "null", "-progress", "pipe:1", "-"])
    errs = [l for l in out.splitlines() if l and "=" not in l.split(" ")[0]]
    t = [l for l in out.splitlines() if l.startswith("out_time_us=")]
    last = int(t[-1].split("=")[1]) / 1e6 if t and t[-1].split("=")[1].lstrip("-").isdigit() else 0
    print(f"  video dekod: oxirgi vaqt {last:.1f}s; xato qatorlari {len(errs)}: {[e[:140] for e in errs[:8]]}")


async def main():
    app = Client(SESSION, api_id=api_id() or int(os.environ["TG_API_ID"]), api_hash=os.environ["TG_API_HASH"], no_updates=True)
    async with app:
        async for _ in app.get_dialogs():
            pass
        for a, s, n in targets:
            print(f"\n===== anime {a}, bo'lim {s}, {n}-qism =====")
            rows = q("SELECT epizod_id, origin_video, origin_size, origin_height FROM epizod_db WHERE anime_id=? AND season_id=? AND epizod_number=?", (a, s, n))
            if not rows:
                print("epizod_db da yo'q")
                continue
            r = rows[0]
            print("epizod_db:", {k: (str(v)[:48] if k != "origin_video" else "nom") for k, v in r.items()})
            name = r["origin_video"]
            tg = q("SELECT file_name, msg_id FROM tg_files WHERE file_name=? OR file_name LIKE ? ESCAPE '\\' ORDER BY file_name",
                   (name, name.replace("\\", "\\\\").replace("%", "\\%").replace("_", "\\_") + ".p%")) if name else []
            print("tg_files:", [(x["file_name"][-24:], x["msg_id"]) for x in tg])
            job = q("SELECT attempts, error, state, done FROM encode_jobs WHERE anime_id=? AND season_id=? AND epizod_id=?", (a, s, r["epizod_id"]))
            print("encode_jobs:", job)
            main_row = next((x for x in tg if x["file_name"] == name), None)
            if not main_row:
                print("asl video tg_files da yo'q")
                continue
            m = await app.get_messages(chan, int(main_row["msg_id"]))
            d = (m.document or m.video) if m and not m.empty else None
            print("kanal xabari:", None if not d else (getattr(d, "file_name", ""), getattr(d, "file_size", 0), getattr(d, "duration", None)))
            if not d:
                continue
            tmp = Path(tempfile.mkdtemp())
            got = await app.download_media(m, file_name=str(tmp / "src.mp4"))
            print("yuklandi:", got, Path(got).stat().st_size if got else 0)
            if got:
                report(Path(got), f"{n}-qism asl video (Telegramdan)")
                Path(got).unlink()


asyncio.run(main())
