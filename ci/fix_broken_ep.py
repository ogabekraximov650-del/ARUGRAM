#!/usr/bin/env python3
# ci/fix_broken_ep.py — BIR MARTA: kanaldan o'chib ketgan asl videoli (Laputa 2,9 GB, anime 4 / 1-qism) buzuq qism yozuvlarini bazadan tozalaydi
# (qayta qo'shish mumkin bo'lsin, raqam to'qnashmasin). Faqat `ci/FIX_BROKEN_EP` dagi `anime:bo'lim:qism` va u asl nomga bog'liq yozuvlar.
import json, os, sys, urllib.request

url = os.environ["TURSO_URL"].strip().rstrip("/")
tok = os.environ["TURSO_TOKEN"].strip()
a, s, n = [int(x) for x in open("ci/FIX_BROKEN_EP").read().split()[0].split(":")]


def run(sql, args=()):
    body = {"requests": [{"type": "execute", "stmt": {"sql": sql, "args": [{"type": "text", "value": str(x)} for x in args]}}, {"type": "close"}]}
    req = urllib.request.Request(url + "/v2/pipeline", data=json.dumps(body).encode(), headers={"Authorization": "Bearer " + tok, "Content-Type": "application/json"})
    r = json.load(urllib.request.urlopen(req, timeout=60))["results"][0]
    if r["type"] == "error":
        raise RuntimeError(r["error"]["message"])
    res = r["response"]["result"]
    return [[c.get("value") for c in row] for row in res["rows"]]


rows = run("SELECT epizod_id, origin_video FROM epizod_db WHERE anime_id=? AND season_id=? AND epizod_number=?", (a, s, n))
if len(rows) != 1:
    sys.exit(f"{a}/{s}/{n}-qism topilmadi yoki bir nechta: {len(rows)}")
eid, origin = rows[0]
print("qism:", eid, "asl nom:", origin)
print("hozirgi: encode_jobs", run("SELECT COUNT(*) FROM encode_jobs WHERE anime_id=? AND season_id=? AND epizod_id=?", (a, s, eid))[0][0],
      "| encode_q", run("SELECT COUNT(*) FROM encode_q WHERE anime_id=? AND season_id=? AND epizod_id=?", (a, s, eid))[0][0])
for t in ("encode_jobs", "encode_q", "epizod_db"):
    run(f"DELETE FROM {t} WHERE anime_id=? AND season_id=? AND epizod_id=?", (a, s, eid))
if origin:
    esc = origin.replace("\\", "\\\\").replace("%", "\\%").replace("_", "\\_")
    run("DELETE FROM tg_files WHERE file_name=? OR file_name LIKE ? ESCAPE '\\'", (origin, esc + ".p%"))
run("UPDATE season_db SET epizod_count=(SELECT COUNT(*) FROM epizod_db WHERE anime_id=? AND season_id=?) WHERE anime_id=? AND season_id=?", (a, s, a, s))
print("✅ tozalandi:", a, s, n, "(epizod_id", eid, ")")
