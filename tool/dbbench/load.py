# UZ: Yuk generatori: Cloudflare worker orqali va to'g'ridan-to'g'ri Ample'ga. Tartib: avval Postgres, keyin libSQL.
# EN: Load generator: through the Cloudflare worker and directly to Ample. Order: Postgres first, then libSQL.
import http.client, json, os, threading, time, urllib.parse, urllib.request

WORKER = os.environ['WORKER_URL'].rstrip('/')
AMPLE = os.environ['AMPLE_URL'].rstrip('/')
TOKEN = os.environ['CHAOS_TOKEN']
NAME = {'pg': 'Postgres', 'lite': 'libSQL'}


def pct(a, p):
    if not a:
        return float('nan')
    a = sorted(a)
    return a[min(len(a) - 1, int(len(a) * p))]


def row(label, c, total, errs, elapsed, lat):
    rps = total / elapsed if elapsed > 0 else 0
    print(f"{label:<22} C={c:<4} req={total:<6} rps={rps:>7.0f} err={errs:<5} p50={pct(lat, .5):>6.0f} p95={pct(lat, .95):>6.0f} p99={pct(lat, .99):>6.0f} ms", flush=True)


def via_worker(engine, op, C, secs, n=45, kill_after=None):
    recs, lock = [], threading.Lock()
    start = time.time()
    deadline = start + secs
    kill = {}

    def client(i):
        k = 0
        while time.time() < deadline:
            k += 1
            t0 = time.time()
            try:
                rq = urllib.request.Request(f"{WORKER}/run?engine={engine}&op={op}&n={n}&seed={i * 1000 + k}", headers={'User-Agent': 'dbbench/1.0'})
                with urllib.request.urlopen(rq, timeout=60) as r:
                    j = json.load(r)
                ms, ok = j['ms'], j['ok']
            except Exception:
                ms, ok = [], '0' * n
            with lock:
                recs.append((t0, time.time(), ms, ok))

    def killer():
        time.sleep(kill_after)
        try:
            urllib.request.urlopen(urllib.request.Request(f"{AMPLE}/chaos/{engine}?token={TOKEN}", headers={'User-Agent': 'dbbench/1.0'}), timeout=30).read()
        except Exception as e:
            print('   chaos call failed:', e)
        kill['t'] = time.time()

    ts = [threading.Thread(target=client, args=(i,)) for i in range(C)]
    if kill_after is not None:
        ts.append(threading.Thread(target=killer))
    for t in ts:
        t.start()
    for t in ts:
        t.join()
    total = sum(len(r[3]) for r in recs)
    errs = sum(r[3].count('0') for r in recs)
    lat = [m for r in recs for m, o in zip(r[2], r[3]) if o == '1']
    elapsed = max(r[1] for r in recs) - start if recs else secs
    return recs, total, errs, elapsed, lat, kill.get('t')


def direct(engine, op, C, secs):
    host = urllib.parse.urlparse(AMPLE).netloc
    lat, errs, lock = [], [0], threading.Lock()
    start = time.time()
    deadline = start + secs

    def client(i):
        conn = http.client.HTTPSConnection(host, timeout=30)
        k = 0
        while time.time() < deadline:
            k += 1
            t0 = time.time()
            try:
                conn.request('GET', f"/{engine}/{op}?id={(i * 7919 + k * 104729) % 500000 + 1}")
                r = conn.getresponse()
                r.read()
                good = r.status == 200
            except Exception:
                good = False
                try:
                    conn.close()
                except Exception:
                    pass
                conn = http.client.HTTPSConnection(host, timeout=30)
            dt = (time.time() - t0) * 1000
            with lock:
                if good:
                    lat.append(dt)
                else:
                    errs[0] += 1

    ts = [threading.Thread(target=client, args=(i,)) for i in range(C)]
    for t in ts:
        t.start()
    for t in ts:
        t.join()
    return len(lat) + errs[0], errs[0], time.time() - start, lat


def chaos(engine):
    print(f"-- chaos: {NAME[engine]} jarayoni 12-soniyada kill -9 (C=3, 40 s, yozish) --", flush=True)
    recs, total, errs, elapsed, lat, tk = via_worker(engine, 'write', 3, 40, n=20, kill_after=12)
    ev = []
    for t0, t1, ms, ok in recs:
        for j, o in enumerate(ok):
            ev.append((t0 + (j + 1) * (t1 - t0) / max(1, len(ok)), o))
    bad = [t for t, o in ev if o == '0']
    row(f"{NAME[engine]} chaos", 3, total, errs, elapsed, lat)
    if tk and bad:
        print(f"   kill vaqti: t+0s; birinchi xato: +{min(bad) - tk:.1f}s; oxirgi xato: +{max(bad) - tk:.1f}s; xatolar: {len(bad)}/{total}", flush=True)
        tail = [o for t, o in ev if t > max(bad)]
        print(f"   oxirgi xatodan keyin muvaffaqiyatli so'rovlar: {tail.count('1')}", flush=True)
    else:
        print(f"   xato yo'q yoki kill qayd etilmadi (xatolar: {len(bad)})", flush=True)


def main():
    t = time.time()
    r = via_worker('ping', 'read', 4, 8)
    row('ping (baza yo\'q)', 4, r[1], r[2], r[3], r[4])
    for engine in ('pg', 'lite'):
        print(f"\n===== {NAME[engine]} =====", flush=True)
        print("-- Cloudflare worker orqali (bepul reja: ~1000 chaqiruv/daq cheklovi, shu sababli past C) --", flush=True)
        for op, levels in (('read', (2, 4, 6)), ('write', (2, 4, 6)), ('hot', (4,)), ('heavy', (4,))):
            for C in levels:
                _, total, errs, elapsed, lat, _ = via_worker(engine, op, C, 12)
                row(f"{NAME[engine]} {op}", C, total, errs, elapsed, lat)
        if os.environ.get('DIRECT', '1') == '1':
            print("-- to'g'ridan-to'g'ri Actions -> Ample (worker'siz, yuqori parallel) --", flush=True)
            for op, levels in (('read', (50, 100, 200)), ('write', (50, 100)), ('hot', (50,)), ('heavy', (20,))):
                for C in levels:
                    total, errs, elapsed, lat = direct(engine, op, C, 8)
                    row(f"{NAME[engine]} {op} direct", C, total, errs, elapsed, lat)
        chaos(engine)
        time.sleep(10)
    print(f"\njami vaqt: {time.time() - t:.0f}s")


main()
