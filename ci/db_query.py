#!/usr/bin/env python3
# ci/db_query.py — FAQAT O'QISH: `ci/DB_QUERY.sql` dagi SELECT/PRAGMA so'rovlarini Ample bazasida bajarib, natijani logga chiqaradi.
# Kalit/token/sir ustunlari (nomida key, token, secret) qiymati yashiriladi. Boshqa turdagi so'rovlar rad etiladi.
import json, os, re, sys, urllib.request

url = os.environ["TURSO_URL"].strip().rstrip("/")
tok = os.environ["TURSO_TOKEN"].strip()
text = open("ci/DB_QUERY.sql").read()
stmts = [s.strip() for s in re.split(r";\s*(?:\n|$)", text) if s.strip() and not s.strip().startswith("--")]
SECRET = re.compile(r"key|token|secret|vkey", re.I)

def run(sql):
    req = urllib.request.Request(url + "/v2/pipeline", data=json.dumps({"requests": [{"type": "execute", "stmt": {"sql": sql}}, {"type": "close"}]}).encode(),
                                 headers={"Authorization": "Bearer " + tok, "Content-Type": "application/json"})
    r = json.load(urllib.request.urlopen(req, timeout=60))["results"][0]
    if r["type"] == "error":
        raise RuntimeError(r["error"]["message"])
    res = r["response"]["result"]
    return [c["name"] for c in res["cols"]], [[c.get("value") for c in row] for row in res["rows"]]

for sql in stmts:
    head = sql.lstrip().split(None, 1)[0].upper()
    print("\n>>", sql.replace("\n", " ")[:200])
    if head not in ("SELECT", "PRAGMA", "WITH"):
        print("   rad etildi: faqat SELECT/PRAGMA")
        continue
    try:
        cols, rows = run(sql)
    except Exception as e:  # noqa: BLE001
        print("   xato:", e)
        continue
    print("   ustunlar:", cols, "| qatorlar:", len(rows))
    for row in rows[:200]:
        print("   ", {c: ("***" if SECRET.search(c) and v else v) for c, v in zip(cols, row)})
