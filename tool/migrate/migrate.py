"""Turso bazasini yangi Turso bazasiga ko'chiradi — GitHub Actions to'g'ridan-to'g'ri ikkala bazaga ulanadi (worker ishtirok etmaydi).

Manba: TURSO_URL / TURSO_TOKEN (GitHub Secrets, `environment: secret`). Manzil: tool/migrate/new_turso.enc
(JSON {"url","token"}, AES-256-CBC/PBKDF2, kalit — ENCODE_TOKEN). Hech narsa chop etilmaydi.
Qayta ishga tushirsa ham xavfsiz: `INSERT OR REPLACE`.
"""
import base64
import json
import os
import subprocess
import sys
import time
import urllib.error
import urllib.request

ENC_KEY = "".join(os.environ.get("ENCODE_TOKEN", "").split())
r = subprocess.run(["openssl", "enc", "-d", "-aes-256-cbc", "-pbkdf2", "-iter", "200000", "-pass", "env:K",
                    "-in", "tool/migrate/new_turso.enc"], env={**os.environ, "K": ENC_KEY}, capture_output=True)
NEW = json.loads(r.stdout.decode() or "{}")
if not NEW.get("url") or not NEW.get("token"):
    raise SystemExit("new_turso.enc ochilmadi")


def norm(u):
    u = u.strip().rstrip("/")
    return "https://" + u[len("libsql://"):] if u.startswith("libsql://") else u


OLD = {"url": norm(os.environ.get("TURSO_URL", "")), "token": os.environ.get("TURSO_TOKEN", "").strip()}
NEW["url"] = norm(NEW["url"])
if not OLD["url"] or not OLD["token"]:
    raise SystemExit("TURSO_URL / TURSO_TOKEN (eski baza) yo'q")
if OLD["url"] == NEW["url"]:
    raise SystemExit("eski va yangi baza bir xil")


def pipeline(db, stmts, tries=5):
    reqs = [{"type": "execute", "stmt": {"sql": s, "args": a}} for s, a in stmts] + [{"type": "close"}]
    body = json.dumps({"requests": reqs}).encode()
    last = None
    for i in range(tries):
        req = urllib.request.Request(db["url"] + "/v2/pipeline", data=body,
                                     headers={"Authorization": "Bearer " + db["token"], "Content-Type": "application/json"})
        try:
            with urllib.request.urlopen(req, timeout=180) as f:
                res = json.loads(f.read())["results"][: len(stmts)]
            return [(None, x["response"]["result"]) if x["type"] == "ok" else (x["error"]["message"], None) for x in res]
        except Exception as e:  # noqa: BLE001
            last = str(e)
            time.sleep(2 * (i + 1))
    raise SystemExit(f"so'rov yiqildi: {last}")


def rows(res):
    cols = [c["name"] for c in res["cols"]]
    return [dict(zip(cols, r)) for r in res["rows"]]


def val(c):
    return c.get("value") if c["type"] != "null" else None


def ident_ok(s):
    return bool(s) and all(ch.isalnum() or ch == "_" for ch in s)


def add_ine(sql):
    up = sql.strip().upper()
    for kw in ("CREATE UNIQUE INDEX ", "CREATE INDEX ", "CREATE TABLE ", "CREATE TRIGGER ", "CREATE VIEW "):
        if up.startswith(kw):
            rest = sql.strip()[len(kw):]
            return sql.strip() if rest.upper().startswith("IF NOT EXISTS") else sql.strip()[: len(kw)] + "IF NOT EXISTS " + rest
    return sql.strip()


# 1) Sxema
err, res = pipeline(OLD, [("SELECT type, name, sql FROM sqlite_master WHERE sql IS NOT NULL AND name NOT LIKE 'sqlite_%' AND name NOT LIKE '_cf_%'", [])])[0]
if err:
    raise SystemExit(f"eski baza o'qilmadi: {err}")
objs = [{k: val(v) for k, v in zip(["type", "name", "sql"], r)} for r in res["rows"]]
stmts = [(add_ine(o["sql"]), []) for kind in ("table", "index", "view", "trigger") for o in objs if o["type"] == kind]
errs = [(s[0][:70], e) for s, (e, _) in zip(stmts, pipeline(NEW, stmts)) if e]
print(f"sxema: {len(stmts)} buyruq, xato: {len(errs)}")
for e in errs[:10]:
    print("   ", e)

# 2) Jadvallar
tables = [o["name"] for o in objs if o["type"] == "table" and ident_ok(o["name"])]
print(f"{len(tables)} ta jadval")
total_rows = 0
for t in tables:
    err, c = pipeline(OLD, [(f'SELECT COUNT(*) AS n FROM "{t}"', [])])[0]
    n = int(val(c["rows"][0][0])) if not err else -1
    if n <= 0:
        print(f"  {t}: {n if n < 0 else 0} qator")
        continue
    after, copied, started = 0, 0, time.time()
    while True:
        err, d = pipeline(OLD, [(f'SELECT rowid AS __rid, * FROM "{t}" WHERE rowid > ? ORDER BY rowid LIMIT 500',
                                  [{"type": "integer", "value": str(after)}])])[0]
        if err:
            print(f"  ❌ {t}: {err}")
            break
        if not d["rows"]:
            break
        names = [c["name"] for c in d["cols"]][1:]
        sql = f'INSERT OR REPLACE INTO "{t}" ({",".join(chr(34) + x + chr(34) for x in names)}) VALUES ({",".join("?" * len(names))})'
        batch, size = [], 0
        for row in d["rows"]:
            after = int(val(row[0]))
            args = row[1:]
            size += sum(len(json.dumps(a)) for a in args)
            batch.append((sql, args))
        out = pipeline(NEW, batch)
        bad = [e for e, _ in out if e]
        if bad:
            print(f"  ❌ {t}: {bad[0]}")
            break
        copied += len(batch)
        if len(d["rows"]) < 500:
            break
    total_rows += copied
    print(f"  {t}: {copied}/{n} qator ({time.time() - started:.0f} s)")

# 3) Solishtirish
print("\n== Solishtirish (eski / yangi) ==")
bad = 0
for t in tables:
    a = pipeline(OLD, [(f'SELECT COUNT(*) FROM "{t}"', [])])[0]
    b = pipeline(NEW, [(f'SELECT COUNT(*) FROM "{t}"', [])])[0]
    na = int(val(a[1]["rows"][0][0])) if a[1] else -1
    nb = int(val(b[1]["rows"][0][0])) if b[1] else -1
    ok = na == nb
    bad += 0 if ok else 1
    print(f"  {'✅' if ok else '❌'} {t}: {na} / {nb}")
print(f"NATIJA: {'hammasi teng' if bad == 0 else str(bad) + ' ta jadval teng emas'}; jami ko'chirilgan qator: {total_rows}")
sys.exit(0 if bad == 0 else 1)
