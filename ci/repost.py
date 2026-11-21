#!/usr/bin/env python3
# ci/repost.py — `ci/REPOST` dagi "<anime> <bo'lim> <qism>" (bazadagi ID'lar) uchun e'lonni kanalga QAYTA joylaydi (saqlangan kadrlar bilan) va natijani/xatoni chop etadi.
import os, urllib.error, urllib.request
API = os.environ.get("API_BASE", "https://arumediatv.uzcom.workers.dev").rstrip("/")
TOKEN = os.environ["ENCODE_TOKEN"].strip()
for line in open("ci/REPOST").read().splitlines():
    parts = line.split("#")[0].split()
    if len(parts) != 3 or not all(p.isdigit() for p in parts):
        continue
    a, s, e = parts
    req = urllib.request.Request(f"{API}/api/encode/repost?a={a}&s={s}&e={e}", data=b"", method="POST",
                                 headers={"X-Encode-Token": TOKEN, "User-Agent": "arumediatv-encoder"})
    try:
        with urllib.request.urlopen(req, timeout=180) as r:
            print(f"{a}/{s}/{e}: {r.status} {r.read()[:300]!r}")
    except urllib.error.HTTPError as ex:
        print(f"{a}/{s}/{e}: HTTP {ex.code} {ex.read()[:400]!r}")
