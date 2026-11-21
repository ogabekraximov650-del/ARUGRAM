#!/usr/bin/env python3
# ci/upload_speed.py — SINOV (saqlash kanaliga 1 ta vaqtincha hujjat yuboriladi va o'chiriladi): telefon -> worker -> Telegram yuklash tezligi, R2'siz.
# `ci/UPLOAD_SPEED` o'zgarsa `db-query.yml` ishga tushiradi. Har bo'lak 18 MiB tasodifiy bayt (shifrlangan fayl kabi). Ikki o'lchov:
#   tg=0 — faqat worker'ga yetkazish (mijoz -> worker);  tg=1 — worker uni Telegram'ga ham yuboradi (`recv_ms` — worker qabul qilgani, `tg_ms` — Telegram'ga yuborishi).
import os, statistics, sys, time, urllib.request, json

API = os.environ.get("API_BASE", "https://arumediatv.uzcom.workers.dev").rstrip("/")
TOKEN = os.environ["ENCODE_TOKEN"].strip()
SIZE = 18 * 1024 * 1024
ROUNDS = int(os.environ.get("ROUNDS", "4"))


def once(tg: int):
    data = os.urandom(SIZE)
    req = urllib.request.Request(f"{API}/api/encode/uptest?tg={tg}", data=data, method="POST",
                                 headers={"X-Encode-Token": TOKEN, "Content-Type": "application/octet-stream", "User-Agent": "arumediatv-speedtest"})
    t = time.time()
    with urllib.request.urlopen(req, timeout=300) as r:
        j = json.loads(r.read() or b"{}")
    return time.time() - t, j


for tg in (0, 1):
    print(f"\n== {'faqat worker' if tg == 0 else 'worker + Telegram'} ({ROUNDS} marta x {SIZE // 1048576} MiB) ==")
    speeds = []
    for i in range(ROUNDS):
        try:
            dt, j = once(tg)
        except Exception as e:  # noqa: BLE001
            print(f"  {i + 1}: XATO {type(e).__name__}: {str(e)[:200]}")
            continue
        if j.get("error"):
            print(f"  {i + 1}: worker xatosi: {j}")
            continue
        mbs = SIZE / 1048576 / dt
        speeds.append(mbs)
        extra = f", worker qabul {j.get('recv_ms')} ms" + (f", Telegram'ga {j.get('tg_ms')} ms ({SIZE / 1048576 / max(1, j['tg_ms']) * 1000:.1f} MB/s)" if tg else "")
        print(f"  {i + 1}: jami {dt:.1f} s -> {mbs:.1f} MB/s{extra}")
    if speeds:
        print(f"  o'rtacha {statistics.mean(speeds):.1f} MB/s, eng sekin {min(speeds):.1f}, eng tez {max(speeds):.1f}")
