#!/usr/bin/env python3
# ci/enc_diag.py — kodlash navbati tashxisi (FAQAT O'QISH): encode_jobs va encode_q qatorlari, app_config dagi encode_* belgilari.
import json, os, urllib.request, time

url = os.environ["TURSO_URL"].strip()
token = os.environ["TURSO_TOKEN"].strip()
if url.startswith("libsql://"):
    url = "https://" + url[len("libsql://"):]


def q(sql):
    req = urllib.request.Request(url + "/v2/pipeline", data=json.dumps({"requests": [{"type": "execute", "stmt": {"sql": sql}}, {"type": "close"}]}).encode(),
                                 headers={"Authorization": "Bearer " + token, "Content-Type": "application/json"})
    with urllib.request.urlopen(req, timeout=60) as r:
        x = json.load(r)["results"][0]
    if x["type"] == "error":
        return [["xato", x["error"]["message"][:100]]]
    return [[c.get("value") for c in r] for r in x["response"]["result"]["rows"]]


now = int(time.time() * 1000)
print("now(ms):", now)
print("\n== encode_jobs (anime, bo'lim, qism, state, done, runner, lease_in_s, attempts, error, progress, queued_at)")
for r in q("SELECT anime_id,season_id,epizod_id,state,done,runner,lease_until,attempts,substr(error,1,80),substr(progress,1,60),queued_at FROM encode_jobs"):
    r = list(r)
    try:
        r[6] = (int(r[6]) - now) // 1000
    except Exception:
        pass
    print(r)
print("\n== encode_q (anime, bo'lim, qism, sifat, state, runner, lease_in_s, attempts, error, queued_at)")
for r in q("SELECT anime_id,season_id,epizod_id,quality,state,runner,lease_until,attempts,substr(error,1,100),queued_at FROM encode_q ORDER BY quality"):
    r = list(r)
    try:
        r[6] = (int(r[6]) - now) // 1000
    except Exception:
        pass
    print(r)
print("\n== app_config encode*/sess_ok*/kicked")
for r in q("SELECT cfg_key, CASE WHEN LENGTH(cfg_value)<40 THEN cfg_value ELSE substr(cfg_value,1,12)||'...' END FROM app_config WHERE cfg_key LIKE 'encode%' OR cfg_key LIKE 'sess_ok%' OR cfg_key LIKE '%kicked%'"):
    print(r)

print("\n== vd_files (name, size, chunks, state, layout, stored, tries, lease_in_s, created_ago_s)")
for r in q("SELECT name,size,chunks,state,layout,CASE WHEN chunk_ids='' THEN 0 ELSE LENGTH(chunk_ids)-LENGTH(REPLACE(chunk_ids,',',''))+1 END,tries,lease,created FROM vd_files ORDER BY created"):
    r = list(r)
    try:
        r[7] = (int(r[7]) - now) // 1000
        r[8] = (now - int(r[8])) // 1000
    except Exception:
        pass
    print(r)
print("\n== vd_mig (name, state, tries, err)")
for r in q("SELECT name,state,tries,substr(err,1,80) FROM vd_mig"):
    print(r)
print("\n== epizod_db (anime, bo'lim, qism, url_360p, url_480p, url_720p, url_1080p, origin_video)")
for r in q("SELECT anime_id,season_id,epizod_id,url_360p,url_480p,url_720p,url_1080p,origin_video FROM epizod_db"):
    print(r)
