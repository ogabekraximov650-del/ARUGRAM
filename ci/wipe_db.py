#!/usr/bin/env python3
# ci/wipe_db.py — Turso bazasini BIR MARTA butunlay tozalaydi.
#
# TALAB (foydalanuvchi, 2026-09): "shifrlab yuklash tizimi ishlasa
# bazadagi barcha ma'lumotlarni tozalab tashla va epizod_db
# ustunini qayta tuz".
#
# `deploy-worker.yml` uni YANGI worker deploy qilingandan KEYIN
# chaqiradi va so'ng worker'ni QAYTA deploy qiladi — yangi
# izolyatlar jadvallarni yangi sxemada yaratadi (`init_db`).
# QOIDA (KEYINGI_VAZIFA.md): avval yangi worker, keyin tozalash —
# aks holda eski worker eski sxemani qaytarib yozib qo'yardi.
#
# TAKRORLANMAYDI: tozalash belgisi (`ci/WIPE_DB_ONCE` ichidagi
# satr) bazaning o'ziga yoziladi (`app_config.wipe_done`). Fayl
# repoda qolib ketsa ham, keyingi deploy'da baza QAYTA
# tozalanmaydi — faqat belgi o'zgarsa.
import json
import os
import sys
import urllib.request

url = os.environ.get("TURSO_URL", "").strip()
token = os.environ.get("TURSO_TOKEN", "").strip()
mark = open("ci/WIPE_DB_ONCE").read().strip()
if not url or not token or not mark:
    sys.exit("TURSO_URL / TURSO_TOKEN / belgi yo'q")
if url.startswith("libsql://"):
    url = "https://" + url[len("libsql://"):]


def run(*sqls):
    reqs = [{"type": "execute", "stmt": {"sql": s}} for s in sqls]
    reqs.append({"type": "close"})
    req = urllib.request.Request(
        url + "/v2/pipeline",
        data=json.dumps({"requests": reqs}).encode(),
        headers={"Authorization": "Bearer " + token, "Content-Type": "application/json"},
    )
    with urllib.request.urlopen(req, timeout=60) as r:
        out = json.load(r)["results"]
    for x in out[:-1]:
        if x["type"] == "error":
            raise RuntimeError(x["error"]["message"])
    return [x["response"]["result"] for x in out[:-1]]


def rows(res):
    return [[c.get("value") for c in r] for r in res["rows"]]


tables = [r[0] for r in rows(run(
    "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'"
)[0])]
if "app_config" in tables:
    done = rows(run("SELECT cfg_value FROM app_config WHERE cfg_key='wipe_done'")[0])
    if done and done[0][0] == mark:
        print(f"Baza '{mark}' belgisi bilan allaqachon tozalangan — o'tkazib yuborildi.")
        sys.exit(0)

print("O'chiriladigan jadvallar:", ", ".join(tables) or "(yo'q)")
if tables:
    run("PRAGMA foreign_keys=OFF", *[f'DROP TABLE IF EXISTS "{t}"' for t in tables])

left = rows(run("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'")[0])
if left:
    sys.exit(f"Tozalanmay qoldi: {left}")

# Belgi — xuddi worker'dagi `app_config` sxemasida.
run(
    "CREATE TABLE IF NOT EXISTS app_config (cfg_key TEXT PRIMARY KEY, cfg_value TEXT)",
    f"INSERT OR REPLACE INTO app_config (cfg_key, cfg_value) VALUES ('wipe_done', '{mark}')",
)
print(f"✅ Baza tozalandi ({len(tables)} ta jadval). Belgi: {mark}")
