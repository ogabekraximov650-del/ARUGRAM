#!/usr/bin/env python3
# ci/ep_wipe.py — FAQAT QISM VIDEOLARINI tozalash (foydalanuvchi talabi, 2026-11): kanal xabarlari, R2/kesh, `epizod_db` qatorlari, kodlash navbati.
# Rasm, avatar, foydalanuvchi, to'lov, anime/bo'lim yozuvlari QOLADI.
#   backup — ta'sirlanadigan jadvallar JSONL ga yoziladi (workflow shifrlab artifact qiladi). Xato bo'lsa hech narsa o'chmaydi.
#   wipe   — worker kanal/R2 ni tozalaydi (`stage: episodes`), so'ng `epizod_db`, `encode_jobs`, `encode_q` bo'shatiladi; belgi bazaga yoziladi (BIR MARTA).
import json, os, sys, time, urllib.error, urllib.request

TABLES = ["epizod_db", "encode_jobs", "encode_q", "vd_queue", "tg_files"]
url = os.environ.get("TURSO_URL", "").strip()
token = os.environ.get("TURSO_TOKEN", "").strip()
api = os.environ.get("API_BASE", "https://arumediatv.uzcom.workers.dev").rstrip("/")
enc = "".join(os.environ.get("ENCODE_TOKEN", "").split())
mark = open("ci/EP_WIPE_ONCE").read().strip()
mode = sys.argv[1] if len(sys.argv) > 1 else ""
if not url or not token or not mark or not enc or mode not in ("backup", "wipe"):
    sys.exit("TURSO_URL / TURSO_TOKEN / ENCODE_TOKEN / belgi yo'q yoki rejim noto'g'ri (backup|wipe)")
if url.startswith("libsql://"):
    url = "https://" + url[len("libsql://"):]


def run(*stmts):
    reqs = [{"type": "execute", "stmt": {"sql": s}} for s in stmts] + [{"type": "close"}]
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
        except urllib.error.HTTPError as e:
            last = f"HTTP {e.code}: {e.read()[:300]!r}"
        except Exception as e:  # noqa: BLE001
            last = e
        time.sleep(5 * (i + 1))
    raise RuntimeError(f"worker: {last}")


tabs = [r[0] for r in rows(run("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'")[0])]
if "app_config" in tabs:
    done = rows(run("SELECT cfg_value FROM app_config WHERE cfg_key='ep_wipe_done'")[0])
    if done and done[0][0] == mark:
        print(f"'{mark}' belgisi bilan allaqachon bajarilgan — o'tkazib yuborildi.")
        open(os.environ.get("GITHUB_OUTPUT", os.devnull), "a").write("skip=1\n")
        sys.exit(0)
if "anime_db" not in tabs or int(rows(run("SELECT COUNT(*) FROM anime_db")[0])[0][0]) == 0 or "epizod_db" not in tabs:
    sys.exit("anime_db bo'sh yoki epizod_db yo'q — noto'g'ri baza? To'xtatildi.")
print("Qatorlar:", {t: int(rows(run(f'SELECT COUNT(*) FROM "{t}"')[0])[0][0]) for t in TABLES if t in tabs})

if mode == "backup":
    out = os.path.join(os.environ.get("RUNNER_TEMP", "/tmp"), "ep_backup.jsonl")
    with open(out, "w") as f:
        for t in TABLES:
            if t not in tabs:
                continue
            off = 0
            while True:
                res = run(f'SELECT * FROM "{t}" LIMIT 1000 OFFSET {off}')[0]
                cols = [c["name"] for c in res["cols"]]
                rs = rows(res)
                if not rs:
                    break
                for r in rs:
                    f.write(json.dumps({"t": t, "cols": cols, "row": r}, ensure_ascii=False) + "\n")
                off += 1000
    print("Zaxira yozildi:", out)
    open(os.environ.get("GITHUB_OUTPUT", os.devnull), "a").write("ok=1\n")
    sys.exit(0)

print("1) Saqlash/migratsiya run'larini bekor qilish:", worker({"stage": "cancel"}))
time.sleep(10)
for i in range(600):
    r = worker({"stage": "episodes"})
    print(f"   kanal/R2: o'chirildi {r.get('deleted')}, xato {r.get('failed')}, qoldi {r.get('left')} (epizod_db {r.get('epizod_db')}, vd_queue {r.get('vd_queue')}, tg_files {r.get('tg_files')}) {r.get('err') or ''}")
    if int(r.get("left", 1)) == 0:
        break
    if int(r.get("failed", 0)) > 0 and i > 5:
        sys.exit("Kanal xabarlari o'chmayapti (xato) — jadvallar tegilmadi.")
else:
    sys.exit("To'liq o'chmadi (aylanish tugadi) — jadvallar tegilmadi.")
print("2) Jadvallarni bo'shatish")
for t in ("epizod_db", "encode_jobs", "encode_q"):
    if t in tabs:
        run(f'DELETE FROM "{t}"')
        print("   tozalandi:", t)
cols = [r[1] for r in rows(run('PRAGMA table_info("season_db")')[0])]
sets = [f"{c}=0" for c in ("views_total", "watch_ms_total", "epizod_count") if c in cols]
if sets:
    run("UPDATE season_db SET " + ", ".join(sets))
run("CREATE TABLE IF NOT EXISTS app_config (cfg_key TEXT PRIMARY KEY, cfg_value TEXT)",
    f"INSERT OR REPLACE INTO app_config (cfg_key, cfg_value) VALUES ('ep_wipe_done', '{mark}')")
print("3) 'O'zgarish bormi?' belgilari:", worker({"stage": "marks", "tables": ["epizod_db", "season_db", "encode_jobs", "encode_q"]}))
print(f"✅ Qism videolari tozalandi. Belgi: {mark}")
