"""Eski va yangi Turso bazasi orasidagi FARQNI chop etadi (faqat o'qish): har jadval uchun faqat eskida / faqat yangida qatorlar soni.
Maxfiy qiymatlar chop etilmaydi: faqat jadval nomi, sonlar va (ayrim jadvallar uchun) birinchi 3 ustun (kalit)."""
import json
import os
import subprocess
import time
import urllib.request

ENC_KEY = "".join(os.environ.get("ENCODE_TOKEN", "").split())
r = subprocess.run(["openssl", "enc", "-d", "-aes-256-cbc", "-pbkdf2", "-iter", "200000", "-pass", "env:K",
                    "-in", "tool/migrate/new_turso.enc"], env={**os.environ, "K": ENC_KEY}, capture_output=True)
NEW = json.loads(r.stdout.decode())


def norm(u):
    u = u.strip().rstrip("/")
    return "https://" + u[len("libsql://"):] if u.startswith("libsql://") else u


OLD = {"url": norm(os.environ["TURSO_URL"]), "token": os.environ["TURSO_TOKEN"].strip()}
NEW["url"] = norm(NEW["url"])


def run(db, sql, args=()):
    body = json.dumps({"requests": [{"type": "execute", "stmt": {"sql": sql, "args": list(args)}}, {"type": "close"}]}).encode()
    for i in range(4):
        try:
            req = urllib.request.Request(db["url"] + "/v2/pipeline", data=body, headers={"Authorization": "Bearer " + db["token"], "Content-Type": "application/json"})
            res = json.loads(urllib.request.urlopen(req, timeout=120).read())["results"][0]
            if res["type"] != "ok":
                raise RuntimeError(res["error"]["message"])
            return res["response"]["result"]
        except Exception:  # noqa: BLE001
            if i == 3:
                raise
            time.sleep(2 * (i + 1))


def rows(db, table):
    out, after = [], 0
    while True:
        res = run(db, f'SELECT rowid AS __rid, * FROM "{table}" WHERE rowid > ? ORDER BY rowid LIMIT 1000', [{"type": "integer", "value": str(after)}])
        if not res["rows"]:
            return out
        for row in res["rows"]:
            after = int(row[0]["value"])
            out.append(tuple(json.dumps(c, sort_keys=True) for c in row[1:]))
        if len(res["rows"]) < 1000:
            return out


tables = [r_[0]["value"] for r_ in run(OLD, "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' AND name NOT LIKE '_cf_%' ORDER BY name")["rows"]]
SHOW_KEYS = {"tg_files", "epizod_db", "encode_jobs", "encode_q", "r2_files", "announced", "fmp4_jobs"}
print(f"{'jadval':<22}{'eski':>7}{'yangi':>7}{'faqat eskida':>14}{'faqat yangida':>15}")
for t in tables:
    try:
        a, b = rows(OLD, t), rows(NEW, t)
    except Exception as e:  # noqa: BLE001
        print(f"{t:<22} xato: {str(e)[:60]}")
        continue
    sa, sb = set(a), set(b)
    oo, nn = [x for x in a if x not in sb], [x for x in b if x not in sa]
    flag = "" if not oo and not nn else "  <-- FARQ"
    print(f"{t:<22}{len(a):>7}{len(b):>7}{len(oo):>14}{len(nn):>15}{flag}")
    if (oo or nn) and t in SHOW_KEYS:
        for name, lst in (("eskida", oo), ("yangida", nn)):
            for x in lst[:3]:
                print(f"     {name}: " + " | ".join(json.loads(c).get("value", "null")[:34] if isinstance(json.loads(c), dict) and json.loads(c).get("value") is not None else "null" for c in x[:4]))
