# UZ: Neon (Hyperdrive/HTTP) / D1 / Turso kechikishini worker orqali o'lchaydi, oxirida Neon sovuq startini ham.
# EN: Measures Neon (Hyperdrive/HTTP) / D1 / Turso latency through the worker, finally Neon cold start.
import json, os, sys, threading, time, urllib.request
from statistics import mean

W = os.environ['WORKER_URL'].rstrip('/')


def call(engine, op, n=40, seed=1, timeout=90):
    rq = urllib.request.Request(f"{W}/run?engine={engine}&op={op}&n={n}&seed={seed}", headers={'User-Agent': 'dbbench2/1.0'})
    t0 = time.time()
    try:
        with urllib.request.urlopen(rq, timeout=timeout) as r:
            j = json.load(r)
    except Exception as e:
        return {'error': str(e)[:120]}, (time.time() - t0) * 1000
    return j, (time.time() - t0) * 1000


def pct(a, p):
    a = sorted(a)
    return a[min(len(a) - 1, int(len(a) * p))] if a else float('nan')


def line(label, ms, errs, conn, extra=''):
    print(f"{label:<22} n={len(ms):<4} mean={mean(ms) if ms else float('nan'):>7.1f} p50={pct(ms,.5):>6} p95={pct(ms,.95):>6} p99={pct(ms,.99):>6} err={errs:<3} connect≈{conn:>5.0f}ms {extra}", flush=True)


ENGINES = ['neon_hd', 'neon_http', 'd1', 'turso']
print("== A) Ketma-ket o'lchov (worker ichida, har chaqiruvda 40 so'rov, 5 chaqiruv) ==", flush=True)
for e in ENGINES:
    for op in ('read', 'write', 'heavy'):
        # isitish (tashlanadi)
        j0, _ = call(e, op, n=5, seed=99)
        if 'skipped' in j0:
            print(f"{e+' '+op:<22} o'tkazildi: {j0['skipped']}", flush=True)
            continue
        ms, errs, conns, last = [], 0, [], ''
        for s in range(1, 6):
            j, wall = call(e, op, seed=s)
            if 'ms' not in j:
                errs += 40
                last = j.get('error', '')
                continue
            ms += j['ms']; errs += j.get('errors', 0); conns.append(j.get('connect_ms', 0))
            last = j.get('last_error') or j.get('error') or last
        line(f"{e} {op}", ms, errs, mean(conns) if conns else 0, f"| {last}" if last else '')

print("\n== B) Parallel yuk: 4 mijoz x 15 soniya (o'qish) ==", flush=True)
for e in ENGINES:
    recs, lock = [], threading.Lock()
    end = time.time() + 15
    def cl(i, e=e):
        k = 0
        while time.time() < end:
            k += 1
            j, _ = call(e, 'read', seed=i * 100 + k)
            with lock:
                recs.append(j)
    ts = [threading.Thread(target=cl, args=(i,)) for i in range(4)]
    [t.start() for t in ts]; [t.join() for t in ts]
    ms = [m for j in recs for m in j.get('ms', [])]
    errs = sum(j.get('errors', 0) for j in recs) + 40 * sum(1 for j in recs if 'ms' not in j and 'skipped' not in j)
    if all('skipped' in j for j in recs):
        print(f"{e:<22} o'tkazildi", flush=True)
        continue
    line(f"{e} read x4", ms, errs, 0, f"| {len(ms)/15:.0f} so'rov/s")

print("\n== C) Neon sovuq start (330 s bekor turgach) ==", flush=True)
time.sleep(330)
for e in ('neon_hd', 'neon_http'):
    j, wall = call(e, 'read', n=3, seed=7)
    ms = j.get('ms', [])
    print(f"{e:<12} birinchi so'rovlar (ms): {ms}  ulanish: {j.get('connect_ms')}  umumiy chaqiruv: {wall:.0f} ms  {j.get('error','')}", flush=True)
