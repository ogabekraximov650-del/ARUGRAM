#!/usr/bin/env python3
"""UZ: zaxira/tiklash E2E sinovi (soxta Telegram bilan): arzon yozuv yuklanmaydi, muhim yozuv yuklanadi, 24 bo'lakdan keyin birlashadi, snapshot eskilarini o'chiradi,
disk yo'qolganda to'liq tiklanadi. Ishga tushirish: `python3 tests/e2e.py target/release/tgstore-jr`. EN: end-to-end backup/recovery test against a fake Telegram."""
import json, os, shutil, subprocess, sys, tempfile, time, urllib.request

BIN = sys.argv[1]; HERE = os.path.dirname(os.path.abspath(__file__))
MOCK, SVC = 8191, 8192
STORE, ADMIN = "storekey1234567890", "adminkey1234567890"
tmp = tempfile.mkdtemp(prefix="jre2e"); data = os.path.join(tmp, "data")
procs = []

def call(port, path, body=None, key=STORE, method=None):
    req = urllib.request.Request(f"http://127.0.0.1:{port}{path}", data=json.dumps(body).encode() if body is not None else None, method=method or ("POST" if body is not None else "GET"), headers={"Authorization": "Bearer " + key, "Content-Type": "application/json"})
    return json.load(urllib.request.urlopen(req, timeout=60))

def sql(q, args=()):
    a = [{"type": "text", "value": str(x)} if isinstance(x, str) else {"type": "integer", "value": str(x)} for x in args]
    r = call(SVC, "/v2/pipeline", {"requests": [{"type": "execute", "stmt": {"sql": q, "args": a}}, {"type": "close"}]})
    res = r["results"][0]
    assert res["type"] == "ok", res
    return [[c.get("value") for c in row] for row in res["response"]["result"]["rows"]]

def state(): return call(MOCK, "/_state", key="x")

def start_mock():
    p = subprocess.Popen([sys.executable, os.path.join(HERE, "mock_tg.py"), str(MOCK)]); procs.append(p); time.sleep(0.6); return p

def start_svc():
    env = dict(os.environ, PORT=str(SVC), JR_DIR=data, JR_ARCH_DIR=os.path.join(tmp, "arch"), STORE_KEY=STORE, ADMIN_KEY=ADMIN, JR_KEY="ab" * 32,
               TG_BOT_TOKEN="tok", TG_CHANNEL_ID="-100", TG_API_BASE=f"http://127.0.0.1:{MOCK}", JR_CRIT_SECS="2", JR_IDLE_SECS="100000", JR_CHUNK_BYTES="6000")
    p = subprocess.Popen([BIN], env=env, stdout=open(os.path.join(tmp, "svc.log"), "a"), stderr=subprocess.STDOUT); procs.append(p)
    for _ in range(100):
        try: urllib.request.urlopen(f"http://127.0.0.1:{SVC}/status", timeout=1); return p
        except Exception: time.sleep(0.2)
    raise SystemExit("servis ishga tushmadi")

def ok(cond, msg):
    print(("OK   " if cond else "XATO ") + msg)
    if not cond: finish(1)

def finish(code):
    for p in procs:
        try: p.kill()
        except Exception: pass
    print("log:", os.path.join(tmp, "svc.log")); sys.exit(code)

start_mock(); svc = start_svc()
sql("create table kv(k integer primary key, v text)"); sql("create table stats_hourly(h text primary key, n integer)")
time.sleep(17)  # birinchi ro'yxat (manifest) yuklanadi
s0 = state(); ok(s0["pinned"] is not None, f"ro'yxat pin qilindi, xabarlar: {s0['messages']}")

# 1) faqat arzon yozuv — yuklanmasligi kerak
for i in range(5): sql("insert or replace into stats_hourly values (?,?)", (f"h{i}", i))
time.sleep(20)
s1 = state(); ok(s1["reqs"] == s0["reqs"], f"arzon yozuv Telegram'ga yuklanmadi (so'rovlar {s0['reqs']} -> {s1['reqs']})")
st = call(SVC, "/status", {}, key=ADMIN); ok(st["dirty"], f"arzon yozuv yuklanishni kutyapti (dirty={st['dirty']})")

# 2) muhim yozuv — ~2 s + sikl ichida yuklanadi (arzon yozuvlar ham birga ketadi)
sql("insert into kv values (1,'birinchi')")
time.sleep(20)
s2 = state(); ok(s2["reqs"] > s1["reqs"], f"muhim yozuv yuklandi (so'rovlar {s1['reqs']} -> {s2['reqs']})")
ok(not call(SVC, "/status", {}, key=ADMIN)["dirty"], "yuklangach dirty bo'sh")

# 3) 40 ta kichik bo'lak: 24 dan keyin birlashishi kerak (xabarlar soni cheklangan)
for i in range(2, 42):
    sql("insert into kv values (?,?)", (i, "x" * 20)); call(SVC, "/replicate", {}, key=ADMIN, method="POST")
s3 = state(); ok(s3["messages"] <= 40, f"bo'laklar birlashtirildi/aylantirildi: xabarlar {s3['messages']} (40 ta yuklashdan keyin)")

# 4) snapshot eski bo'laklarni o'chiradi
r = call(SVC, "/snapshot", {}, key=ADMIN, method="POST"); s4 = state()
ok(s4["messages"] <= 4, f"snapshotdan keyin kanal toza: xabarlar {s4['messages']} {s4['names']}")
ok(any(n.startswith("snap_") for n in s4["names"]), "snapshot yuklangan")

# 5) snapshotdan keyin yana yozuv (arzon + muhim) va flush
sql("insert into kv values (100,'snapshotdan keyin')"); sql("insert or replace into stats_hourly values ('h99',99)"); call(SVC, "/replicate", {}, key=ADMIN, method="POST")
before = sql("select count(*), (select count(*) from stats_hourly) from kv")[0]

# 6) disk yo'qoladi -> tiklash
svc.kill(); svc.wait(); shutil.rmtree(data); svc = start_svc(); time.sleep(1)
after = sql("select count(*), (select count(*) from stats_hourly) from kv")[0]
ok(after == before, f"tiklashdan keyin ma'lumot bir xil: {before} == {after}")
ok(sql("select v from kv where k=100")[0][0] == "snapshotdan keyin", "snapshotdan keyingi yozuv tiklandi")

print("Telegram:", json.dumps(call(SVC, "/status", {}, key=ADMIN)["telegram_stats"]))
shutil.rmtree(tmp, ignore_errors=True); finish(0)
