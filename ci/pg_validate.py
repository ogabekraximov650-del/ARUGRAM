#!/usr/bin/env python3
"""Worker'dagi SQL buyruqlarini Supabase `arumediatv.tr()` orqali Postgres'ga o'girib, EXPLAIN bilan tekshiradi (hech narsa bajarilmaydi/yozilmaydi).
UZ: statik SQL matnlar worker/src/*.rs dan yig'iladi; `?` lar NULL bilan almashtiriladi. Faqat xatolar chop etiladi.
EN: validates translated statements with EXPLAIN. Env: PG_JSON."""
import glob, json, os, re, sys
import psycopg2

cfg = json.loads(os.environ["PG_JSON"])
ref, region = cfg["ref"], cfg.get("region", "ap-northeast-1")
conn = None
for h, p, u in [(f"aws-0-{region}.pooler.supabase.com", 5432, f"{cfg['user']}.{ref}"), (f"aws-1-{region}.pooler.supabase.com", 5432, f"{cfg['user']}.{ref}")]:
    try:
        conn = psycopg2.connect(host=h, port=p, user=u, password=cfg["password"], dbname=cfg["db"], connect_timeout=15, sslmode="require"); break
    except Exception as e:
        print("ulanmadi", h, str(e)[:100].replace(cfg["password"], "***"))
if not conn: sys.exit("ulanib bo'lmadi")
conn.autocommit = True
cur = conn.cursor()
cur.execute("set search_path to arumediatv, pg_catalog, public")

kw = re.compile(r'^\s*(select|insert|update|delete|with|replace)\b', re.I)
sqls = set()
for f in glob.glob("worker/src/*.rs"):
    s = open(f, encoding="utf8").read()
    for m in re.finditer(r'"((?:[^"\\]|\\.)*)"', s, re.S):
        t = m.group(1).replace("\\\n", "").replace("\\n", "\n").replace('\\"', '"').replace("\\'", "'")
        t = re.sub(r"\s+", " ", t).strip()
        if kw.match(t) and len(t) > 12: sqls.add(t)
static = sorted(x for x in sqls if "{" not in x)
dyn = len(sqls) - len(static)
nulls = json.dumps([{"type": "null"}] * 30)
bad = 0
for q in static:
    try:
        cur.execute("select arumediatv.tr(%s, %s::json)", (q, nulls))
        pg = cur.fetchone()[0]
        cur.execute("explain " + pg)
    except Exception as e:
        bad += 1
        print("XATO:", str(e).strip().split("\n")[0][:160])
        print("   SQL:", q[:220])
print(f"jami statik: {len(static)}, dinamik (o'tkazildi): {dyn}, xato: {bad}")
sys.exit(0)
