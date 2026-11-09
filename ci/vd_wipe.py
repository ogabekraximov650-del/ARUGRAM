#!/usr/bin/env python3
# ci/vd_wipe.py — Turso bazasini NOLDAN boshlash (foydalanuvchi talabi, 2026-11): faqat `anime_db`, `season_db`, `season_janr` qoladi;
# qolgan hamma jadval (foydalanuvchilar, to'lovlar, sozlamalar, fayllar, to'plamlar, izohlar...) bo'shatiladi; kanal postlari va R2 obyektlari o'chadi.
#
# Ikki bosqich (workflow `vd-wipe.yml`):
#   backup — hamma jadvalni JSONL ga yozadi (workflow uni ENCODE_TOKEN bilan shifrlab artifact qiladi). Xato bo'lsa hech narsa o'chmaydi.
#   wipe   — worker orqali kanal xabarlari/R2 o'chadi, so'ng jadvallar bo'shatiladi, belgi yoziladi (qayta ishga tushsa ham BIR MARTA).
import json
import os
import sys
import time
import urllib.request

KEEP = {"anime_db", "season_db", "season_janr"}
# Jadval sxemasini qayta yaratish kerak bo'lmasin: worker `init_db` o'zi yaratadi; bu kalitlar sxema ko'chirish belgilari — qoladi.
KEEP_CFG = ("mig\\_%", "wipe\\_done", "vd\\_wipe\\_done")

url = os.environ.get("TURSO_URL", "").strip()
token = os.environ.get("TURSO_TOKEN", "").strip()
api = os.environ.get("API_BASE", "https://arumediatv.uzcom.workers.dev").rstrip("/")
enc = "".join(os.environ.get("ENCODE_TOKEN", "").split())
mark = open("ci/VD_WIPE_ONCE").read().strip()
mode = sys.argv[1] if len(sys.argv) > 1 else ""
if not url or not token or not mark or not enc or mode not in ("backup", "wipe"):
    sys.exit("TURSO_URL / TURSO_TOKEN / ENCODE_TOKEN / belgi yo'q yoki rejim noto'g'ri (backup|wipe)")
if url.startswith("libsql://"):
    url = "https://" + url[len("libsql://"):]


def run(*stmts):
    reqs = [{"type": "execute", "stmt": ({"sql": s} if isinstance(s, str) else {"sql": s[0], "args": [{"type": "text", "value": a} for a in s[1]]})} for s in stmts]
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


def worker(body, tries=5):
    last = None
    for i in range(tries):
        try:
            req = urllib.request.Request(api + "/api/vd/wipe", data=json.dumps(body).encode(), method="POST",
                                         headers={"X-Encode-Token": enc, "Content-Type": "application/json", "User-Agent": "arumediatv-wipe"})
            with urllib.request.urlopen(req, timeout=120) as r:
                return json.loads(r.read() or b"{}")
        except Exception as e:  # noqa: BLE001
            last = e
            time.sleep(5 * (i + 1))
    raise RuntimeError(f"worker: {last}")


tables = [r[0] for r in rows(run("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'")[0])]
if "app_config" in tables:
    done = rows(run("SELECT cfg_value FROM app_config WHERE cfg_key='vd_wipe_done'")[0])
    if done and done[0][0] == mark:
        print(f"Baza '{mark}' belgisi bilan allaqachon tozalangan — o'tkazib yuborildi.")
        open(os.environ.get("GITHUB_OUTPUT", os.devnull), "a").write("skip=1\n")
        sys.exit(0)
n_anime = int(rows(run("SELECT COUNT(*) FROM anime_db")[0])[0][0])
if n_anime == 0 or "anime_db" not in tables or "season_db" not in tables:
    sys.exit("anime_db bo'sh yoki yo'q — noto'g'ri baza? To'xtatildi, hech narsa o'chirilmadi.")
print(f"Jadvallar: {len(tables)}; anime: {n_anime}")

if mode == "backup":
    out = os.path.join(os.environ.get("RUNNER_TEMP", "/tmp"), "turso_backup.jsonl")
    with open(out, "w") as f:
        for t in tables:
            off, total = 0, 0
            cols = None
            while True:
                res = run(f'SELECT * FROM "{t}" LIMIT 1000 OFFSET {off}')[0]
                cols = [c["name"] for c in res["cols"]]
                rs = rows(res)
                if not rs:
                    break
                for r in rs:
                    f.write(json.dumps({"t": t, "cols": cols, "row": r}, ensure_ascii=False) + "\n")
                total += len(rs)
                off += 1000
            print(f"  zaxira: {t} — {total} qator")
    print("Zaxira yozildi:", out)
    open(os.environ.get("GITHUB_OUTPUT", os.devnull), "a").write("ok=1\n")
    sys.exit(0)

# ── wipe ──
print("1) Saqlash/migratsiya run'larini bekor qilish:", worker({"stage": "cancel"}))
time.sleep(10)
for i in range(400):
    r = worker({"stage": "files"})
    print(f"   kanal/R2: o'chirildi {r.get('deleted')}, xato {r.get('failed')}, qoldi {r.get('left')}")
    if int(r.get("left", 1)) == 0:
        break
else:
    sys.exit("Fayllar to'liq o'chmadi (aylanish tugadi) — jadvallar tegilmadi.")

print("2) Jadvallarni bo'shatish")
for t in tables:
    if t in KEEP:
        continue
    if t == "app_config":
        cond = " AND ".join(f"cfg_key NOT LIKE '{k}' ESCAPE '\\'" for k in KEEP_CFG)
        run(f"DELETE FROM app_config WHERE {cond}")
    else:
        run(f'DELETE FROM "{t}"')
    print("   tozalandi:", t)
cols = {t: [r[1] for r in rows(run(f'PRAGMA table_info("{t}")')[0])] for t in ("anime_db", "season_db")}
run("UPDATE anime_db SET photo_url=''")
sets = ["photo_url=''"] + [f"{c}=0" for c in ("views_total", "watch_ms_total", "fav_count", "rating_sum", "rating_count", "epizod_count") if c in cols["season_db"]]
run("UPDATE season_db SET " + ", ".join(sets))
run("CREATE TABLE IF NOT EXISTS app_config (cfg_key TEXT PRIMARY KEY, cfg_value TEXT)",
    (f"INSERT OR REPLACE INTO app_config (cfg_key, cfg_value) VALUES ('vd_wipe_done', ?)", [mark]))
print("3) Yakuniy fayl tozalash:", worker({"stage": "files"}))
print("4) 'O'zgarish bormi?' belgilari:", worker({"stage": "marks", "tables": tables}))
left = {t: int(rows(run(f'SELECT COUNT(*) FROM "{t}"')[0])[0][0]) for t in tables}
print("Qoldi:", {k: v for k, v in left.items() if v})
print(f"✅ Baza noldan boshlandi. Anime: {left['anime_db']}, bo'lim: {left['season_db']}. Belgi: {mark}")
