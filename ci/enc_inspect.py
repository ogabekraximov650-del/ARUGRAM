#!/usr/bin/env python3
# ci/enc_inspect.py — FAQAT O'QISH: kodlash navbatidagi "Anibla orqali" qismlarining HLS bo'laklarini tekshiradi ("davomiylik mos emas" xatosi tashxisi).
# `ci/ENC_INSPECT`: `epizod_id:soniya` juftliklari (masalan `40:323.4 49:883.2`). Hamma bo'lakni yuklab (saqlamasdan) tekshiradi: TS belgisi (0x47, 188 bayt),
# hajm, EXT-X-DISCONTINUITY/KEY/MAP joylari; muammoli joy va uning qo'shnilari ffprobe bilan ko'riladi. URL'lar logga CHIQMAYDI (maskalanadi).
import json, os, re, subprocess, sys, tempfile, urllib.parse
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

sys.path.insert(0, "tool/anibla")
import download as dl  # noqa: E402

url = os.environ["TURSO_URL"].strip()
token = os.environ["TURSO_TOKEN"].strip()
if url.startswith("libsql://"):
    url = "https://" + url[len("libsql://"):]
ANIME, SEASON = 3, 1
pairs = []
for tok in [t for t in open("ci/ENC_INSPECT").read().split() if t[0].isdigit()]:
    a, _, b = tok.partition(":")
    pairs.append((int(a), float(b or 0)))


def q(sql, args=()):
    import urllib.request
    body = {"requests": [{"type": "execute", "stmt": {"sql": sql, "args": [{"type": "integer", "value": str(x)} for x in args]}}, {"type": "close"}]}
    req = urllib.request.Request(url + "/v2/pipeline", data=json.dumps(body).encode(), headers={"Authorization": "Bearer " + token, "Content-Type": "application/json"})
    with urllib.request.urlopen(req, timeout=60) as r:
        res = json.load(r)["results"][0]
    if res["type"] != "ok":
        raise RuntimeError(res)
    r = res["response"]["result"]
    return [[c.get("value") for c in row] for row in r["rows"]]


def ffprobe(path):
    try:
        o = subprocess.run(["ffprobe", "-v", "error", "-show_entries", "stream=index,codec_type,codec_name,width,height,start_time,duration:format=duration,size", "-of", "json", str(path)],
                           capture_output=True, text=True, timeout=60).stdout
        return json.loads(o or "{}")
    except Exception as e:  # noqa: BLE001
        return {"xato": str(e)}


def ts_ok(b: bytes):
    if not b:
        return "bo'sh"
    if len(b) % 188 != 0:
        return f"hajm 188 ga karrali emas ({len(b)})"
    if any(b[i] != 0x47 for i in range(0, len(b), 188)):
        return "TS sinxron belgisi (0x47) buzilgan"
    return ""


def inspect(eid, target):
    rows = q("SELECT origin FROM encode_jobs WHERE anime_id=? AND season_id=? AND epizod_id=?", (ANIME, SEASON, eid))
    print(f"\n===== epizod_id {eid} (maqsad {target} s) =====")
    if not rows or not rows[0][0]:
        print("encode_jobs da yozuv yo'q (qism tugagan va navbatdan olib tashlangan bo'lishi mumkin)")
        return
    origin = rows[0][0]
    raw = origin[len("anibla:"):] if origin.startswith("anibla:") else origin
    j = None
    try:
        j = json.loads(raw)
        saved = str(j.get("u") or "")
        print("manba:", j.get("t"), "|", j.get("l"), "| h =", j.get("h"), "| n =", j.get("n"))
    except Exception:  # noqa: BLE001
        saved = raw
    vurl, _, aurl = saved.partition("\n")
    shape = lambda x: re.sub(r"[A-Za-z]", "a", re.sub(r"\d", "9", x))[:70]
    print("kalitlar:", sorted(j.keys()) if isinstance(j, dict) else type(j), "| origin boshi:", shape(origin[:40]), "| u shakli:", shape(vurl.strip()), "| u uzunligi:", len(vurl.strip()))
    print("u sxemasi:", vurl.strip().split(":")[0][:10] if ":" in vurl else "yo'q")
    for u in (vurl, aurl):
        if u.strip():
            print("::add-mask::" + u.strip())
    print("saqlangan manzil bor:", bool(vurl.strip()), "| alohida ovoz:", bool(aurl.strip()))
    for name, u in (("VIDEO", vurl.strip()), ("OVOZ", aurl.strip())):
        if not u:
            continue
        print(f"--- {name} playlist ---")
        try:
            text = dl.fetch_retry(u).decode(errors="replace")
        except Exception as e:  # noqa: BLE001
            print("playlist olinmadi:", type(e).__name__, str(e)[:120])
            continue
        tags = {t: text.count(t) for t in ("#EXT-X-DISCONTINUITY", "#EXT-X-KEY", "#EXT-X-MAP", "#EXT-X-BYTERANGE", "#EXT-X-STREAM-INF", "#EXT-X-ENDLIST", "#EXTINF")}
        print("teglar:", tags, "| TARGETDURATION:", (re.search(r"#EXT-X-TARGETDURATION:(\d+)", text) or [None, "?"])[1])
        segs = dl.segments(u)
        if not segs:
            print("segments() None qaytardi (shifrlangan/fMP4/master) — ffmpeg o'zi yuklaydi")
            continue
        total = sum(d for _, d in segs)
        print(f"bo'laklar: {len(segs)} ta, jami {total:.1f} s")
        cum, marks, disc = 0.0, [], []
        lines = [l.strip() for l in text.splitlines()]
        idx = -1
        for l in lines:
            if l.startswith("#EXT-X-DISCONTINUITY"):
                disc.append(idx + 1)
            elif l and not l.startswith("#"):
                idx += 1
        for k, (_, d) in enumerate(segs):
            marks.append(cum)
            cum += d
        if disc:
            print("DISCONTINUITY bo'lak raqamlari:", disc[:20], "vaqtlari:", [round(marks[i], 1) for i in disc[:20] if i < len(marks)])
        # Maqsad vaqtdagi bo'lak
        tgt = next((i for i, m in enumerate(marks) if m + segs[i][1] >= target), None) if target else None
        if tgt is not None:
            print(f"maqsad {target} s -> bo'lak #{tgt} ({marks[tgt]:.1f}..{marks[tgt] + segs[tgt][1]:.1f} s)")
        # Hamma bo'lakni tekshirish (saqlamasdan)
        bad, sizes = [], {}

        def chk(i):
            try:
                b = dl.fetch(segs[i][0], timeout=90)
            except Exception as e:  # noqa: BLE001
                return i, None, f"yuklanmadi: {type(e).__name__}: {str(e)[:80]}"
            return i, len(b), ts_ok(b)
        with ThreadPoolExecutor(max_workers=16) as ex:
            for i, n, why in ex.map(chk, range(len(segs))):
                sizes[i] = n
                if why:
                    bad.append((i, n, why))
        print(f"tekshirildi: {len(segs)} bo'lak; muammoli: {len(bad)}")
        for i, n, why in bad[:25]:
            print(f"  #{i} ({marks[i]:.1f} s, {segs[i][1]:.2f} s, {n} bayt): {why}")
        vals = [v for v in sizes.values() if v]
        if vals:
            avg = sum(vals) / len(vals)
            small = [(i, v) for i, v in sizes.items() if v and v < avg * 0.2]
            print(f"o'rtacha bo'lak hajmi {avg / 1024:.0f} KB; juda kichik (<20%): {[(i, round(marks[i], 1), v) for i, v in small[:15]]}")
        # Maqsad atrofidagi bo'laklarni ffprobe
        around = sorted(set([tgt - 1, tgt, tgt + 1] if tgt is not None else []) | {i for i, _, _ in bad[:3]})
        tmp = Path(tempfile.mkdtemp())
        for i in around:
            if not (0 <= i < len(segs)):
                continue
            p = tmp / f"s{i}.ts"
            try:
                p.write_bytes(dl.fetch(segs[i][0], timeout=90))
            except Exception as e:  # noqa: BLE001
                print(f"  #{i}: yuklanmadi {e}")
                continue
            print(f"  ffprobe #{i}:", json.dumps(ffprobe(p), ensure_ascii=False)[:600])


for eid, target in pairs:
    try:
        inspect(eid, target)
    except Exception as e:  # noqa: BLE001
        print(f"epizod {eid}: xato {type(e).__name__}: {str(e)[:200]}")
