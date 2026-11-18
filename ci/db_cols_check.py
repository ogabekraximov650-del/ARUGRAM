#!/usr/bin/env python3
# ci/db_cols_check.py — FAQAT O'QISH: jadvalning ustunlari tartibi va qatorlar soni (Ample bazasi).
import json, os, sys, urllib.request
url = os.environ["TURSO_URL"].strip().rstrip("/")
tok = os.environ["TURSO_TOKEN"].strip()
def q(sql):
    req = urllib.request.Request(url + "/v2/pipeline", data=json.dumps({"requests": [{"type": "execute", "stmt": {"sql": sql}}, {"type": "close"}]}).encode(),
                                 headers={"Authorization": "Bearer " + tok, "Content-Type": "application/json"})
    r = json.load(urllib.request.urlopen(req, timeout=60))["results"][0]
    if r["type"] == "error":
        return [["xato", r["error"]["message"]]]
    return [[c.get("value") for c in row] for row in r["response"]["result"]["rows"]]
for t in sys.argv[1:] or ["epizod_db"]:
    cols = [r[1] for r in q(f"PRAGMA table_info('{t}')")]
    print(f"{t}: {len(cols)} ustun, qatorlar: {q(f'SELECT COUNT(*) FROM {t}')[0][0]}")
    for i, c in enumerate(cols, 1):
        print(f"  {i:2}. {c}")
# Baza hajmi (faqat o'qish): sahifalar, bo'sh sahifalar (DELETE'dan keyin faylda qoladi, VACUUM bo'shatadi), jadvallar kesimida.
try:
    pc = int(q("PRAGMA page_count")[0][0]); ps = int(q("PRAGMA page_size")[0][0]); fl = int(q("PRAGMA freelist_count")[0][0])
    print(f"BAZA HAJMI: {pc * ps / 1048576:.2f} MB ({pc} sahifa x {ps} bayt); bo'sh sahifalar: {fl} ({fl * ps / 1048576:.2f} MB)")
    r = q("SELECT name, SUM(pgsize) AS b FROM dbstat GROUP BY name ORDER BY b DESC LIMIT 15")
    for row in r:
        print(f"   {row[0]}: {int(row[1]) / 1048576:.2f} MB" if len(row) > 1 and str(row[1]).isdigit() else f"   {row}")
except Exception as e:
    print("hajm olinmadi:", e)
print("jadvallar:", [r[0] for r in q("SELECT name FROM sqlite_master WHERE type='table' AND name LIKE 'epizod_db%'")])
