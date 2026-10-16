"""Ikkinchi akkauntdagi avtoencode repo'sida HAMMA workflow'larni o'chiradi va ishlayotgan run'larni bekor qiladi.

Baza ko'chirilayotganda yozuvlar to'xtashi uchun. Token: tool/encode/gh_token.enc (kalit — ENCODE_TOKEN).
`python3 stop_encode.py stop`   — to'xtatadi (default)
`python3 stop_encode.py start`  — qayta yoqadi
"""
import json
import os
import subprocess
import sys
import urllib.error
import urllib.request
from pathlib import Path

MODE = sys.argv[1] if len(sys.argv) > 1 else "stop"
ENC = Path("tool/encode/gh_token.enc")
REPO_TXT = Path("tool/encode/gh_repo.txt").read_text().strip()

k = "".join(os.environ.get("ENCODE_TOKEN", "").split())
r = subprocess.run(["openssl", "enc", "-d", "-aes-256-cbc", "-pbkdf2", "-iter", "200000", "-pass", "env:K", "-in", str(ENC)],
                   env={**os.environ, "K": k}, capture_output=True)
TOKEN = r.stdout.decode().strip()
if not TOKEN:
    raise SystemExit("gh_token.enc ochilmadi")


def call(method, path, body=None):
    req = urllib.request.Request("https://api.github.com" + path, method=method,
                                 data=json.dumps(body).encode() if body is not None else None,
                                 headers={"Authorization": f"Bearer {TOKEN}", "Accept": "application/vnd.github+json",
                                          "User-Agent": "arumediatv-migrate"})
    try:
        with urllib.request.urlopen(req, timeout=60) as f:
            b = f.read()
            return f.status, (json.loads(b) if b else {})
    except urllib.error.HTTPError as e:
        return e.code, {"error": e.read().decode()[:200]}


login = call("GET", "/user")[1]["login"]
repo = REPO_TXT if "/" in REPO_TXT else f"{login}/{REPO_TXT}"
print(f"akkaunt: {login}, repo: {repo}, rejim: {MODE}")

_, wf = call("GET", f"/repos/{repo}/actions/workflows?per_page=100")
for w in wf.get("workflows", []):
    if MODE == "stop":
        st, _ = call("PUT", f"/repos/{repo}/actions/workflows/{w['id']}/disable")
    else:
        st, _ = call("PUT", f"/repos/{repo}/actions/workflows/{w['id']}/enable")
    print(f"  {w['path']}: {w['state']} -> {MODE} (HTTP {st})")

if MODE == "stop":
    for status in ("in_progress", "queued"):
        _, runs = call("GET", f"/repos/{repo}/actions/runs?status={status}&per_page=100")
        for run in runs.get("workflow_runs", []):
            st, _ = call("POST", f"/repos/{repo}/actions/runs/{run['id']}/cancel")
            print(f"  run #{run['run_number']} {run['name']} ({status}): bekor (HTTP {st})")
    # Tekshiruv
    import time
    time.sleep(20)
    left = 0
    for status in ("in_progress", "queued"):
        _, runs = call("GET", f"/repos/{repo}/actions/runs?status={status}&per_page=100")
        left += len(runs.get("workflow_runs", []))
    print(f"yakun: ishlayotgan/navbatdagi run qoldi = {left}")
