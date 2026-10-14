"""Ikkinchi akkauntdagi avtoencode repo'sining oxirgi run loglarini shu job logiga chiqaradi.

Token: tool/encode/gh_token.enc (kalit — ENCODE_TOKEN). Faqat o'qish. Loglar GitHub tomonidan maskalangan.
"""
import io
import json
import os
import subprocess
import urllib.request
import zipfile
from pathlib import Path

ENC = Path("tool/encode/gh_token.enc")
REPO = Path("tool/encode/gh_repo.txt").read_text().strip().split("/")[-1]
RUNS = int(os.environ.get("DIAG_RUNS", "12"))
TAIL = int(os.environ.get("DIAG_TAIL", "60"))

k = "".join(os.environ.get("ENCODE_TOKEN", "").split())
r = subprocess.run(["openssl", "enc", "-d", "-aes-256-cbc", "-pbkdf2", "-iter", "200000",
                    "-pass", "env:K", "-in", str(ENC)], env={**os.environ, "K": k}, capture_output=True)
TOKEN = r.stdout.decode().strip()
if not TOKEN:
    raise SystemExit("gh_token.enc ochilmadi")


def get(path, raw=False):
    req = urllib.request.Request(path if path.startswith("http") else "https://api.github.com" + path,
                                 headers={"Authorization": f"Bearer {TOKEN}", "Accept": "application/vnd.github+json"})
    with urllib.request.urlopen(req, timeout=60) as f:
        b = f.read()
    return b if raw else json.loads(b)


login = get("/user")["login"]
print(f"akkaunt: {login}, repo: {REPO}")
runs = get(f"/repos/{login}/{REPO}/actions/runs?per_page={RUNS}")["workflow_runs"]
for run in runs:
    print(f"\n######## #{run['run_number']} {run['name']} | {run['status']}/{run['conclusion']} | {run['created_at']} | id={run['id']}")
    try:
        jobs = get(f"/repos/{login}/{REPO}/actions/runs/{run['id']}/jobs?per_page=30")["jobs"]
    except Exception as ex:  # noqa: BLE001
        print("  jobs olinmadi:", ex)
        continue
    for j in jobs:
        print(f"  --- job {j['name']}: {j['status']}/{j['conclusion']}")
        for st in j.get("steps", []):
            if st.get("conclusion") not in (None, "success", "skipped"):
                print(f"      qadam xato: {st['name']} -> {st['conclusion']}")
    try:
        z = zipfile.ZipFile(io.BytesIO(get(f"/repos/{login}/{REPO}/actions/runs/{run['id']}/logs", raw=True)))
    except Exception as ex:  # noqa: BLE001
        print("  loglar olinmadi:", ex)
        continue
    for n in sorted(z.namelist()):
        if "/" not in n:
            continue
        lines = z.read(n).decode("utf-8", "replace").splitlines()
        print(f"  >>> {n} ({len(lines)} qator), oxirgi {TAIL}:")
        for ln in lines[-TAIL:]:
            print("    " + ln[:300])
