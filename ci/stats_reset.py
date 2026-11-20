#!/usr/bin/env python3
# ci/stats_reset.py — BIR MARTA: foydalanuvchi statistikasini nolga tushiradi (Ample bazasida). Foydalanuvchi talabi (2026-11-20): "Barcha foydalanuvchi statistikasini nolga tushir".
# Nolga tushadi: tomosha tarixi (`watch_history_db`), ko'rishlar soni va tomosha vaqti (anime/bo'lim hisoblagichlari), trafik (`users_db.traffic_bytes`),
# umumiy statistika jadvallari (`stats_hourly`, `stats_daily`). TEGILMAYDI: foydalanuvchilar, balans/obuna, sevimlilar, baholar, izohlar, anime/bo'lim/qismlar.
import json, os, sys, urllib.request

url = os.environ["TURSO_URL"].strip().rstrip("/")
tok = os.environ["TURSO_TOKEN"].strip()
mark = open("ci/STATS_RESET_ONCE").read().strip()


def run(*stmts):
    reqs = [{"type": "execute", "stmt": {"sql": s}} for s in stmts] + [{"type": "close"}]
    req = urllib.request.Request(url + "/v2/pipeline", data=json.dumps({"requests": reqs}).encode(), headers={"Authorization": "Bearer " + tok, "Content-Type": "application/json"})
    out = json.load(urllib.request.urlopen(req, timeout=120))["results"]
    for x in out[:-1]:
        if x["type"] == "error":
            raise RuntimeError(x["error"]["message"])
    return [x["response"]["result"] for x in out[:-1]]


def rows(res):
    return [[c.get("value") for c in r] for r in res["rows"]]


tabs = [r[0] for r in rows(run("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'")[0])]
if "app_config" in tabs:
    d = rows(run("SELECT cfg_value FROM app_config WHERE cfg_key='stats_reset_done'")[0])
    if d and d[0][0] == mark:
        print(f"'{mark}' belgisi bilan allaqachon bajarilgan — o'tkazib yuborildi.")
        sys.exit(0)
if "users_db" not in tabs or int(rows(run("SELECT COUNT(*) FROM users_db")[0])[0][0]) == 0:
    sys.exit("users_db yo'q yoki bo'sh — noto'g'ri baza? To'xtatildi.")


def count(t):
    return int(rows(run(f'SELECT COUNT(*) FROM "{t}"')[0])[0][0]) if t in tabs else -1


print("Oldin: watch_history_db", count("watch_history_db"), "| stats_hourly", count("stats_hourly"), "| stats_daily", count("stats_daily"),
      "| traffic yig'indisi", rows(run("SELECT COALESCE(SUM(traffic_bytes),0) FROM users_db")[0])[0][0])
for t in ("watch_history_db", "stats_hourly", "stats_daily"):
    if t in tabs:
        run(f'DELETE FROM "{t}"')
        print("   tozalandi:", t)
run("UPDATE users_db SET traffic_bytes=0")
print("   users_db.traffic_bytes = 0")
ZERO = {"views_total", "watch_ms_total", "views", "watch_ms", "view_count", "watch_ms_sum"}
for t in ("anime_db", "season_db", "epizod_db"):
    if t not in tabs:
        continue
    cols = [r[1] for r in rows(run(f'PRAGMA table_info("{t}")')[0])]
    sets = [f"{c}=0" for c in cols if c in ZERO]
    if sets:
        run(f"UPDATE {t} SET " + ", ".join(sets))
        print("   hisoblagichlar 0:", t, sets)
run("CREATE TABLE IF NOT EXISTS app_config (cfg_key TEXT PRIMARY KEY, cfg_value TEXT)",
    f"INSERT OR REPLACE INTO app_config (cfg_key, cfg_value) VALUES ('stats_reset_done', '{mark}')")
print("Keyin: watch_history_db", count("watch_history_db"), "| stats_hourly", count("stats_hourly"), "| stats_daily", count("stats_daily"),
      "| traffic yig'indisi", rows(run("SELECT COALESCE(SUM(traffic_bytes),0) FROM users_db")[0])[0][0])
print(f"✅ Foydalanuvchi statistikasi nolga tushirildi. Belgi: {mark}")
