"""Ko'chirishdan keyin eski worker (eski baza) bilan yangi worker (yangi baza) bir vaqtda ishlagan davrdagi FARQNI tuzatadi.

Sabab: bot webhook'lari bir necha soat eski worker'ga qarab turdi — u yuborilgan fayllarni ESKI bazaga yozdi (kanalga ko'chirdi) va
umumiy R2 omboridan faylni o'chirdi, yangi baza esa buni bilmadi (R2 qatori bor, obyekt yo'q -> "HTTP 404").

Tuzatish (hammasi qayta ishga tushirilsa ham xavfsiz):
  1. tg_files, epizod_db, encode_jobs, post_jobs: eskida bor/farq qilgan qatorlar yangiga o'tkaziladi (INSERT OR REPLACE).
  2. Eskida "tayyor" (done) sifatlar uchun yangidagi encode_q qatori 'done' qilinadi.
  3. Yangi bazadagi r2_files qatori endi tg_files'da bor fayl uchun — nusxa ortiqcha: worker `/api/r2/done` obyekt va qatorni o'chiradi.
  4. R2'da obyekti yo'q va tg_files'da yo'q r2_files qatori o'chiriladi; shu sifatning fMP4 nusxasi fmp4_jobs navbatiga qo'yiladi.
  5. Foydalanuvchi eski worker'da boshlagan panel holati (anibla_*, post_last_src) ko'chiriladi.
"""
import json
import os
import subprocess
import time
import urllib.request

K = "".join(os.environ.get("ENCODE_TOKEN", "").split())
NEW = json.loads(subprocess.run(["openssl", "enc", "-d", "-aes-256-cbc", "-pbkdf2", "-iter", "200000", "-pass", "env:K", "-in", "tool/migrate/new_turso.enc"],
                                env={**os.environ, "K": K}, capture_output=True).stdout.decode())
norm = lambda u: ("https://" + u.strip().rstrip("/")[len("libsql://"):]) if u.strip().startswith("libsql://") else u.strip().rstrip("/")
OLD = {"url": norm(os.environ["TURSO_URL"]), "token": os.environ["TURSO_TOKEN"].strip()}
NEW["url"] = norm(NEW["url"])
WORKER = os.environ.get("WORKER_URL", "https://arumediatv.uzcom.workers.dev").rstrip("/")
PK = {"tg_files": ["file_name"], "epizod_db": ["anime_id", "season_id", "epizod_id"], "encode_jobs": ["anime_id", "season_id", "epizod_id"], "post_jobs": ["id"]}


def pipeline(db, stmts):
    reqs = [{"type": "execute", "stmt": {"sql": s, "args": a}} for s, a in stmts] + [{"type": "close"}]
    body = json.dumps({"requests": reqs}).encode()
    last = None
    for i in range(4):
        try:
            req = urllib.request.Request(db["url"] + "/v2/pipeline", data=body, headers={"Authorization": "Bearer " + db["token"], "Content-Type": "application/json"})
            res = json.loads(urllib.request.urlopen(req, timeout=120).read())["results"][: len(stmts)]
            out = []
            for x in res:
                if x["type"] != "ok":
                    raise RuntimeError(x["error"]["message"])
                out.append(x["response"]["result"])
            return out
        except Exception as e:  # noqa: BLE001
            last = e
            time.sleep(2 * (i + 1))
    raise SystemExit(f"baza so'rovi yiqildi: {last}")


def select(db, sql, args=()):
    r = pipeline(db, [(sql, list(args))])[0]
    cols = [c["name"] for c in r["cols"]]
    return cols, r["rows"]


def tint(v):
    return {"type": "integer", "value": str(v)}


def ttext(v):
    return {"type": "text", "value": v}


def val(c):
    return c.get("value") if c["type"] != "null" else None


def canon(row):
    return json.dumps(row, sort_keys=True)


# ── 1. Qatorlarni yangiga o'tkazish ─────────────────────────────
changed = {}
for t, pk in PK.items():
    ocols, orows = select(OLD, f'SELECT * FROM "{t}"')
    ncols, nrows = select(NEW, f'SELECT * FROM "{t}"')
    nset = {canon(r) for r in nrows}
    diff = [r for r in orows if canon(r) not in nset]
    changed[t] = [dict(zip(ocols, r)) for r in diff]
    if not diff:
        print(f"{t}: farq yo'q")
        continue
    sql = f'INSERT OR REPLACE INTO "{t}" ({",".join(chr(34) + c + chr(34) for c in ocols)}) VALUES ({",".join("?" * len(ocols))})'
    pipeline(NEW, [(sql, r) for r in diff])
    print(f"{t}: {len(diff)} qator yangiga o'tkazildi")

# ── 2. Eskida tayyor sifatlar -> yangidagi encode_q 'done' ─────────
for j in changed["encode_jobs"]:
    a, s, e = (int(val(j[k])) for k in ("anime_id", "season_id", "epizod_id"))
    done = [q for q in (val(j["done"]) or "").split(",") if q]
    for q in done:
        pipeline(NEW, [("UPDATE encode_q SET state='done', runner='', lease_until=0, progress='', error='' WHERE anime_id=? AND season_id=? AND epizod_id=? AND quality=? AND state<>'done'",
                        [tint(a), tint(s), tint(e), ttext(q)])])
    print(f"encode_q {a}/{s}/{e}: tayyor sifatlar {done}")

# ── 3-4. R2 qatorlari ─────────────────────────────────────────
_, r2rows = select(NEW, "SELECT file_name, anime_id, season_id, epizod_id, quality, kind, state FROM r2_files")
_, tgrows = select(NEW, "SELECT file_name FROM tg_files WHERE msg_id>0")
committed = {val(r[0]) for r in tgrows}
for row in r2rows:
    name, a, s, e, q, kind, state = (val(c) for c in row)
    if name in committed:
        # Telegram'da allaqachon bor — R2 nusxasi ortiqcha. Worker obyekt va qatorni o'chiradi.
        req = urllib.request.Request(WORKER + "/api/r2/done", data=json.dumps({"file": name}).encode(), method="POST",
                                     headers={"X-Encode-Token": K, "Content-Type": "application/json", "User-Agent": "arumediatv-reconcile"})
        try:
            print(f"r2 {name}: tg_files'da bor ->", urllib.request.urlopen(req, timeout=60).read().decode()[:80])
        except Exception as ex:  # noqa: BLE001
            print(f"r2 {name}: /api/r2/done xato: {ex}")
        continue
    # Obyekt yo'qmi? (worker `diag` buni `rows_without_object` deb ko'rsatadi) — yuklovchi 404 olayotgan qatorlar
    req = urllib.request.Request(WORKER + "/api/r2/diag", data=b"{}", method="POST",
                                 headers={"X-Encode-Token": K, "Content-Type": "application/json", "User-Agent": "arumediatv-reconcile"})
    diag = json.loads(urllib.request.urlopen(req, timeout=60).read())
    missing = set((diag.get("audit") or {}).get("rows_without_object") or [])
    if name in missing:
        pipeline(NEW, [("DELETE FROM r2_files WHERE file_name=?", [ttext(name)])])
        print(f"r2 {name}: obyekt yo'q, qator o'chirildi")
        if kind == "fmp4":
            pipeline(NEW, [("INSERT INTO fmp4_jobs (anime_id, season_id, epizod_id, quality, queued_at) VALUES (?,?,?,?,?) "
                            "ON CONFLICT DO UPDATE SET state='queued', attempts=0, error='', runner='', lease_until=0",
                            [tint(a), tint(s), tint(e), ttext(q), tint(int(time.time() * 1000))])])
            print(f"fmp4_jobs: {a}/{s}/{e} {q} navbatga qo'yildi")

# ── 5. Panel holati ───────────────────────────────────────────
for k in ("anibla_nav", "anibla_target", "anibla_token", "post_last_src"):
    _, rows = select(OLD, "SELECT cfg_value FROM app_config WHERE cfg_key=?", [ttext(k)])
    if rows:
        pipeline(NEW, [("INSERT INTO app_config (cfg_key, cfg_value) VALUES (?,?) ON CONFLICT(cfg_key) DO UPDATE SET cfg_value=excluded.cfg_value", [ttext(k), rows[0][0]])])
        print(f"app_config.{k}: ko'chirildi")

# ── Tekshiruv ─────────────────────────────────────────────────
print("\n== Tekshiruv ==")
bad = 0
for t in PK:
    _, o = select(OLD, f'SELECT * FROM "{t}"')
    _, n = select(NEW, f'SELECT * FROM "{t}"')
    nset = {canon(r) for r in n}
    left = [r for r in o if canon(r) not in nset]
    bad += len(left)
    print(f"  {t}: eskida qolgan farq = {len(left)}")
_, r2 = select(NEW, "SELECT state, COUNT(*) FROM r2_files GROUP BY state")
print("  yangi r2_files:", [(val(a), val(b)) for a, b in r2])
print("NATIJA:", "hammasi mos" if bad == 0 else f"{bad} qator mos emas")
