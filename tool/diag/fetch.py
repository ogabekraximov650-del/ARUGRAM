"""Ikkinchi akkauntdagi avtoencode repo'sining oxirgi run loglarini shu job logiga chiqaradi.

Token: tool/encode/gh_token.enc (kalit — ENCODE_TOKEN). Faqat o'qish. Loglar GitHub tomonidan maskalangan.
"""
import io
import json
import os
import subprocess
import urllib.error
import urllib.request
import zipfile
from pathlib import Path

ENC = Path("tool/encode/gh_token.enc")
REPO = Path("tool/encode/gh_repo.txt").read_text().strip().split("/")[-1]
RUNS = int(os.environ.get("DIAG_RUNS", "8"))
TAIL = int(os.environ.get("DIAG_TAIL", "60"))

k = "".join(os.environ.get("ENCODE_TOKEN", "").split())
r = subprocess.run(["openssl", "enc", "-d", "-aes-256-cbc", "-pbkdf2", "-iter", "200000",
                    "-pass", "env:K", "-in", str(ENC)], env={**os.environ, "K": k}, capture_output=True)
TOKEN = r.stdout.decode().strip()
if not TOKEN:
    raise SystemExit("gh_token.enc ochilmadi")


class _NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, *a, **k):
        return None


def get(path, raw=False):
    if raw and "/logs" in path:
        # Yo'naltirilgan manzilga Authorization yuborilmasin (Azure 401 beradi).
        try:
            urllib.request.build_opener(_NoRedirect).open(urllib.request.Request(
                "https://api.github.com" + path,
                headers={"Authorization": f"Bearer {TOKEN}", "Accept": "application/vnd.github+json"}), timeout=60)
        except urllib.error.HTTPError as e:
            loc = e.headers.get("Location")
            if e.code not in (301, 302, 303, 307, 308) or not loc:
                raise
            with urllib.request.urlopen(loc, timeout=120) as f:
                return f.read()
    req = urllib.request.Request(path if path.startswith("http") else "https://api.github.com" + path,
                                 headers={"Authorization": f"Bearer {TOKEN}", "Accept": "application/vnd.github+json"})
    with urllib.request.urlopen(req, timeout=60) as f:
        b = f.read()
    return b if raw else json.loads(b)


login = get("/user")["login"]
print(f"akkaunt: {login}, repo: {REPO}")
runs = get(f"/repos/{login}/{REPO}/actions/runs?per_page={RUNS}")["workflow_runs"]
print("== Run ro'yxati ==")
for run in runs:
    print(f"#{run['run_number']} {run['name']} | {run['status']}/{run['conclusion']} | {run['created_at']} | id={run['id']}")
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
    if "R2" in run["name"] or "ukl" in run["name"].lower():
        for j in jobs:
            try:
                txt = get(f"/repos/{login}/{REPO}/actions/jobs/{j['id']}/logs", raw=True).decode("utf-8", "replace")
            except Exception as ex:  # noqa: BLE001
                print("  job logi olinmadi:", ex)
                continue
            lines = [l for l in txt.splitlines() if "##[" not in l and "[36;1m" not in l and "Node.js" not in l]
            print(f"  >>> JOB LOG {j['name']} ({len(lines)} qator), oxirgi 250:")
            for ln in lines[-250:]:
                print("    " + ln[:300])
        continue
    try:
        z = zipfile.ZipFile(io.BytesIO(get(f"/repos/{login}/{REPO}/actions/runs/{run['id']}/logs", raw=True)))
    except Exception as ex:  # noqa: BLE001
        print("  loglar olinmadi:", ex)
        continue
    for n in sorted(z.namelist()):
        if "/" not in n:
            continue
        lines = [l for l in z.read(n).decode("utf-8", "replace").splitlines()
                 if "##[" not in l and "[36;1m" not in l and "Node.js" not in l and "ENCODE_TOKEN" not in l]
        up = "R2" in run["name"] or "ukl" in run["name"].lower() or "upload" in run["name"].lower()
        tail = TAIL * 3 if up else 12
        print(f"  >>> {n} ({len(lines)} qator), oxirgi {tail}:")
        for ln in lines[-tail:]:
            print("    " + ln[:300])
