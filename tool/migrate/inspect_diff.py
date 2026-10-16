"""Farq qilgan qismlar bo'yicha batafsil (faqat o'qish): eski va yangi bazada encode_jobs/encode_q/epizod_db/tg_files/r2_files qatorlari."""
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


def run(db, sql, args=()):
    body = json.dumps({"requests": [{"type": "execute", "stmt": {"sql": sql, "args": [{"type": "integer", "value": str(a)} if isinstance(a, int) else {"type": "text", "value": a} for a in args]}}, {"type": "close"}]}).encode()
    req = urllib.request.Request(db["url"] + "/v2/pipeline", data=body, headers={"Authorization": "Bearer " + db["token"], "Content-Type": "application/json"})
    res = json.loads(urllib.request.urlopen(req, timeout=60).read())["results"][0]
    if res["type"] != "ok":
        return [{"xato": res["error"]["message"][:80]}]
    r = res["response"]["result"]
    cols = [c["name"] for c in r["cols"]]
    return [dict(zip(cols, [c.get("value") for c in row])) for row in r["rows"]]


def short(d):
    return {k: (str(v)[:40] if v is not None else None) for k, v in d.items() if k not in ("origin", "file_key", "key_360p", "key_480p", "key_720p", "key_1080p", "fmp4_key_360p", "fmp4_key_480p", "fmp4_key_720p", "fmp4_key_1080p")}


for (a, s, e) in ((3, 1, 454), (9, 1, 599), (9, 1, 600), (9, 1, 601)):
    print(f"\n######## {a}/{s}/{e}")
    for name, db in (("ESKI", OLD), ("YANGI", NEW)):
        j = run(db, "SELECT state, done, queued_at FROM encode_jobs WHERE anime_id=? AND season_id=? AND epizod_id=?", (a, s, e))
        q = run(db, "SELECT quality, state, attempts, queued_at FROM encode_q WHERE anime_id=? AND season_id=? AND epizod_id=? ORDER BY quality", (a, s, e))
        ep = run(db, "SELECT url_360p, url_480p, url_720p, url_1080p, fmp4_url_360p, fmp4_url_480p, fmp4_url_720p, fmp4_url_1080p FROM epizod_db WHERE anime_id=? AND season_id=? AND epizod_id=?", (a, s, e))
        print(f"[{name}] encode_jobs: {j}")
        print(f"[{name}] encode_q   : {q}")
        print(f"[{name}] epizod_db  : {[{k: v for k, v in r.items() if v} for r in ep]}")
    r2 = run(NEW, "SELECT file_name, kind, state, attempts, size FROM r2_files WHERE anime_id=? AND season_id=? AND epizod_id=?", (a, s, e))
    print(f"[YANGI] r2_files   : {r2}")
print("\n== ESKI post_jobs ==")
print([short(r) for r in run(OLD, "SELECT * FROM post_jobs")])
print("== YANGI r2_files holati ==", run(NEW, "SELECT state, COUNT(*) AS n FROM r2_files GROUP BY state"))
