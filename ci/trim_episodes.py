#!/usr/bin/env python3
# ci/trim_episodes.py — bitta anime bo'limining N-qismidan KEYINGI qismlarini TO'LIQ o'chiradi (foydalanuvchi talabi, 2026-11: Bleach 36+).
# UZ: `ci/TRIM_EPISODES` ichidagi buyruq: `dry` (faqat ko'rsatadi) yoki `delete <anime_id> <season_id> <qoldiriladigan_oxirgi_qism_raqami>`.
#     Qism qatori, fayllari (worker `/api/vd/forget`: R2, kanal xabarlari), kodlash navbati va tomosha tarixi o'chadi; bo'lim qismlar soni yangilanadi.
# EN: Deletes every episode numbered above N of one season: row, files (via worker), encode queue, watch history. `dry` only prints.
# RU: Удаляет все серии с номером больше N одного сезона: строку, файлы (через воркер), очередь кодирования, историю. `dry` только показывает.
import json
import os
import sys
import time
import urllib.error
import urllib.request

url = os.environ.get("TURSO_URL", "").strip()
token = os.environ.get("TURSO_TOKEN", "").strip()
api = os.environ.get("API_BASE", "https://arumediatv.uzcom.workers.dev").rstrip("/")
enc = "".join(os.environ.get("ENCODE_TOKEN", "").split())
cmd = open("ci/TRIM_EPISODES").read().split()
if not url or not token or not enc or not cmd:
    sys.exit("TURSO_URL / TURSO_TOKEN / ENCODE_TOKEN / buyruq yo'q")
if url.startswith("libsql://"):
    url = "https://" + url[len("libsql://"):]


def arg(v):
    return {"type": "integer", "value": str(v)} if isinstance(v, int) else {"type": "text", "value": str(v)}


def run(*stmts):
    reqs = [{"type": "execute", "stmt": ({"sql": s} if isinstance(s, str) else {"sql": s[0], "args": [arg(a) for a in s[1]]})} for s in stmts]
    reqs.append({"type": "close"})
    req = urllib.request.Request(url + "/v2/pipeline", data=json.dumps({"requests": reqs}).encode(),
                                 headers={"Authorization": "Bearer " + token, "Content-Type": "application/json"})
    with urllib.request.urlopen(req, timeout=120) as r:
        out = json.load(r)["results"]
    for x in out[:-1]:
        if x["type"] == "error":
            raise RuntimeError(x["error"]["message"])
    return [x["response"]["result"] for x in out[:-1]]


def rows(res):
    return [[c.get("value") for c in r] for r in res["rows"]]


def forget(name):
    last = None
    for i in range(4):
        try:
            req = urllib.request.Request(api + "/api/vd/forget", data=json.dumps({"name": name}).encode(), method="POST",
                                         headers={"X-Encode-Token": enc, "Content-Type": "application/json", "User-Agent": "arumediatv-trim"})
            with urllib.request.urlopen(req, timeout=120) as r:
                return r.status
        except urllib.error.HTTPError as e:
            last = f"HTTP {e.code}"
            time.sleep(3 * (i + 1))
        except Exception as e:  # noqa: BLE001
            last = str(e)
            time.sleep(3 * (i + 1))
    return last


def file_name(v):
    v = (v or "").strip()
    if not v:
        return ""
    if "/api/image/" in v:
        v = v.split("/api/image/", 1)[1]
    v = v.split("?", 1)[0]
    return v if v and "/" not in v and " " not in v else ""


if cmd[0] == "dry":
    keep = int(cmd[1]) if len(cmd) > 1 else 35
    print(f"== DRY: 'bleach' nomli animelar, {keep}-qismdan keyingilar ==")
    for aid, an in rows(run("SELECT id, name FROM anime_db WHERE lower(name) LIKE '%bleach%'")[0]):
        print(f"\nanime_id={aid}  nomi={an!r}")
        for sid, bid, nomi, cnt in rows(run(("SELECT season_id, bolim_id, nomi, epizod_count FROM season_db WHERE anime_id=? ORDER BY season_id", [int(aid)]))[0]):
            n_all, mx = rows(run(("SELECT COUNT(*), COALESCE(MAX(epizod_number),0) FROM epizod_db WHERE anime_id=? AND season_id=?", [int(aid), int(sid)]))[0])[0]
            n_del = rows(run(("SELECT COUNT(*) FROM epizod_db WHERE anime_id=? AND season_id=? AND epizod_number>?", [int(aid), int(sid), keep]))[0])[0][0]
            n_q = rows(run(("SELECT COUNT(*) FROM encode_jobs WHERE anime_id=? AND season_id=?", [int(aid), int(sid)]))[0])[0][0]
            print(f"  season_id={sid} bolim={bid} nomi={nomi!r}: bazada {n_all} qism (eng katta raqam {mx}), o'chiriladigan ({keep}+) {n_del}, kodlash navbatida {n_q}")
    sys.exit(0)

if cmd[0] != "delete" or len(cmd) != 4:
    sys.exit("Buyruq: dry [N] | delete <anime_id> <season_id> <N>")
aid, sid, keep = int(cmd[1]), int(cmd[2]), int(cmd[3])
nm = rows(run(("SELECT name FROM anime_db WHERE id=?", [aid]))[0])
if not nm or "bleach" not in (nm[0][0] or "").lower():
    sys.exit(f"To'xtadi: anime_id={aid} nomi 'bleach' emas ({nm}) — xavfsizlik tekshiruvi")
eps = rows(run(("SELECT epizod_id, epizod_number, url_360p, url_480p, url_720p, url_1080p, origin_video FROM epizod_db "
                "WHERE anime_id=? AND season_id=? AND epizod_number>? ORDER BY epizod_number", [aid, sid, keep]))[0])
print(f"anime={nm[0][0]!r} anime_id={aid} season_id={sid}: {keep}-qismdan keyin {len(eps)} ta qism o'chiriladi")
print("qism raqamlari:", ", ".join(str(e[1]) for e in eps) or "(yo'q)")
if not eps:
    print("O'chiriladigan qism yo'q.")
    sys.exit(0)
bad = 0
for e in eps:
    eid, num = int(e[0]), e[1]
    for v in e[2:]:
        n = file_name(v)
        if n:
            r = forget(n)
            if r != 200:
                bad += 1
                print(f"  ! fayl {n}: {r}")
    run(
        ("DELETE FROM epizod_db WHERE anime_id=? AND season_id=? AND epizod_id=?", [aid, sid, eid]),
        ("DELETE FROM encode_jobs WHERE anime_id=? AND season_id=? AND epizod_id=?", [aid, sid, eid]),
        ("DELETE FROM encode_q WHERE anime_id=? AND season_id=? AND epizod_id=?", [aid, sid, eid]),
        ("DELETE FROM watch_history_db WHERE anime_id=? AND season_id=? AND epizod_id=?", [aid, sid, eid]),
    )
    try:
        run(("DELETE FROM r2_files WHERE anime_id=? AND season_id=? AND epizod_id=?", [aid, sid, eid]))
    except RuntimeError:
        pass  # jadval yo'q bo'lishi mumkin
    print(f"  o'chirildi: {num}-qism (epizod_id={eid})")
run(("UPDATE season_db SET epizod_count=(SELECT COUNT(*) FROM epizod_db WHERE anime_id=? AND season_id=?) WHERE anime_id=? AND season_id=?", [aid, sid, aid, sid]))
left = rows(run(("SELECT COUNT(*), COALESCE(MAX(epizod_number),0) FROM epizod_db WHERE anime_id=? AND season_id=?", [aid, sid]))[0])[0]
over = rows(run(("SELECT COUNT(*) FROM epizod_db WHERE anime_id=? AND season_id=? AND epizod_number>?", [aid, sid, keep]))[0])[0][0]
q = rows(run(("SELECT COUNT(*) FROM encode_jobs WHERE anime_id=? AND season_id=? AND epizod_id NOT IN (SELECT epizod_id FROM epizod_db WHERE anime_id=? AND season_id=?)", [aid, sid, aid, sid]))[0])[0][0]
print(f"\nNATIJA: qoldi {left[0]} ta qism, eng katta raqam {left[1]}; {keep}+ qolgan: {over}; yetim navbat yozuvlari: {q}; fayl xatolari: {bad}")
if over:
    sys.exit(1)
