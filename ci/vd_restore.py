#!/usr/bin/env python3
# ci/vd_restore.py — tozalashdan oldingi zaxiradan (JSONL) FAQAT ruxsat etilgan `app_config` kalitlarini jonli bazaga qaytaradi.
# Qiymatlar logga chiqmaydi (faqat kalit nomlari).
import json
import os
import sys
import urllib.request

ALLOW = {"anibla_creds", "anibla_token", "encbot_admins"}
src = sys.argv[1]
url = os.environ["TURSO_URL"].strip()
token = os.environ["TURSO_TOKEN"].strip()
if url.startswith("libsql://"):
    url = "https://" + url[len("libsql://"):]


def run(*stmts):
    reqs = [{"type": "execute", "stmt": ({"sql": s} if isinstance(s, str) else {"sql": s[0], "args": [{"type": "text", "value": a} for a in s[1]]})} for s in stmts]
    reqs.append({"type": "close"})
    req = urllib.request.Request(url + "/v2/pipeline", data=json.dumps({"requests": reqs}).encode(),
                                 headers={"Authorization": "Bearer " + token, "Content-Type": "application/json"})
    with urllib.request.urlopen(req, timeout=60) as r:
        out = json.load(r)["results"]
    for x in out[:-1]:
        if x["type"] == "error":
            raise RuntimeError(x["error"]["message"])


names, rows = [], {}
for line in open(src):
    d = json.loads(line)
    if d["t"] != "app_config":
        continue
    r = dict(zip(d["cols"], d["row"]))
    names.append(r["cfg_key"])
    if r["cfg_key"] in ALLOW and r.get("cfg_value"):
        rows[r["cfg_key"]] = r["cfg_value"]
print("Zaxiradagi sozlama kalitlari:", sorted(names))
for k, v in rows.items():
    run(("INSERT INTO app_config (cfg_key,cfg_value) VALUES (?,?) ON CONFLICT(cfg_key) DO UPDATE SET cfg_value=excluded.cfg_value", [k, v]))
print("Qaytarildi:", sorted(rows))
