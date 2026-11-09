#!/usr/bin/env python3
# ci/db_audit.py — Turso bazasi AUDITI (FAQAT O'QISH): jadvallar, qatorlar soni, ustunlar, indekslar, katta matn ustunlari, app_config kalitlari (qiymatsiz).
import json, os, sys, urllib.request

url = os.environ["TURSO_URL"].strip()
token = os.environ["TURSO_TOKEN"].strip()
if url.startswith("libsql://"):
    url = "https://" + url[len("libsql://"):]


def run(*stmts):
    reqs = [{"type": "execute", "stmt": {"sql": s}} for s in stmts] + [{"type": "close"}]
    req = urllib.request.Request(url + "/v2/pipeline", data=json.dumps({"requests": reqs}).encode(),
                                 headers={"Authorization": "Bearer " + token, "Content-Type": "application/json"})
    with urllib.request.urlopen(req, timeout=120) as r:
        out = json.load(r)["results"]
    res = []
    for x in out[:-1]:
        if x["type"] == "error":
            res.append(None)
            print("  xato:", x["error"]["message"][:120])
        else:
            res.append([[c.get("value") for c in r] for r in x["response"]["result"]["rows"]])
    return res


tabs = [r[0] for r in run("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name")[0]]
print(f"JADVALLAR: {len(tabs)}")
for t in tabs:
    cnt = run(f'SELECT COUNT(*) FROM "{t}"')[0]
    cols = run(f'PRAGMA table_info("{t}")')[0] or []
    idx = run(f'PRAGMA index_list("{t}")')[0] or []
    n = cnt[0][0] if cnt else "?"
    print(f"\n== {t}: {n} qator, {len(cols)} ustun, {len(idx)} indeks")
    print("   ustunlar:", ", ".join(f"{c[1]}:{c[2]}" for c in cols))
    if idx:
        print("   indekslar:", ", ".join(i[1] for i in idx))
    if cnt and int(cnt[0][0] or 0) > 0:
        txt = [c[1] for c in cols if (c[2] or "").upper() in ("TEXT", "BLOB", "")]
        if txt:
            sel = ", ".join(f'ROUND(AVG(LENGTH("{c}")),0)' for c in txt)
            mx = ", ".join(f'MAX(LENGTH("{c}"))' for c in txt)
            a = run(f'SELECT {sel} FROM "{t}"', f'SELECT {mx} FROM "{t}"')
            if a[0] and a[1]:
                big = [(c, a[0][0][i], a[1][0][i]) for i, c in enumerate(txt) if a[1][0][i] and int(a[1][0][i]) >= 200]
                if big:
                    print("   katta matn ustunlari (o'rtacha/eng katta bayt):", ", ".join(f"{c} {x}/{y}" for c, x, y in big))
        # bo'sh (NULL/'') ustunlar — hech qachon to'ldirilmagan
        empty = []
        for c in cols:
            r = run(f'SELECT COUNT(*) FROM "{t}" WHERE "{c[1]}" IS NOT NULL AND "{c[1]}" != \'\' AND "{c[1]}" != 0')[0]
            if r is not None and r[0][0] == "0":
                empty.append(c[1])
        if empty:
            print("   BUTUNLAY BO'SH ustunlar:", ", ".join(empty))

if "app_config" in tabs:
    print("\napp_config kalitlari (qiymat uzunligi bilan):")
    for k, ln in run("SELECT cfg_key, LENGTH(cfg_value) FROM app_config ORDER BY cfg_key")[0]:
        print(f"  {k}  ({ln})")
