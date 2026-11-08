#!/usr/bin/env python3
"""Vaqtinchalik tashxis: ikkinchi akkauntdagi (`avtoencode`) repoda dublyaj workflow'i va run'lari holati.
Token `tool/encode/gh_token.enc` dan ochiladi. Chiqishga token, email va akkaunt nomi YOZILMAYDI (bu repo ochiq — log hammaga ko'rinadi)."""
import io
import json
import os
import re
import subprocess
import sys
import urllib.error
import urllib.request
import zipfile
from pathlib import Path

ENC = Path(__file__).resolve().parent.parent / "encode"


def token():
    k = os.environ.get("ENCODE_TOKEN", "").strip()
    r = subprocess.run(["openssl", "enc", "-d", "-aes-256-cbc", "-pbkdf2", "-iter", "200000", "-pass", "env:K", "-in", str(ENC / "gh_token.enc")],
                       env={**os.environ, "K": k}, capture_output=True)
    return r.stdout.decode().strip()


T = token()
if not T:
    sys.exit("token ochilmadi")
print(f"::add-mask::{T}")


def api(path, raw=False):
    req = urllib.request.Request(path if path.startswith("http") else "https://api.github.com" + path,
                                 headers={"Authorization": f"Bearer {T}", "Accept": "application/vnd.github+json", "User-Agent": "arumediatv-check"})
    try:
        with urllib.request.urlopen(req, timeout=60) as r:
            data = r.read()
            return r.status, (data if raw else json.loads(data or b"{}"))
    except urllib.error.HTTPError as e:
        return e.code, {"message": e.read().decode(errors="replace")[:200]}


def clean(line):
    line = re.sub(r"\b[A-Za-z0-9_\-+/=]{32,}\b", "<yashirildi>", line)
    return re.sub(r"https?://\S+", lambda m: m.group(0) if "github.com" in m.group(0) else "<url>", line)[:220]


owner = None
code, me = api("/user")
if code == 200:
    owner = me["login"]
repo = (ENC / "gh_repo.txt").read_text().strip()
full = repo if "/" in repo else f"{owner}/{repo}"
print("repo: (ikkinchi akkaunt) /", full.split("/")[-1])

code, v = api(f"/repos/{full}")
print("repo kirish:", code, "| private:", v.get("private"), "| default:", v.get("default_branch"))
for path in (".github/workflows/dub.yml", "tool/dub/dub.py", "tool/dub/ticker.sh", "tool/encode/session.enc"):
    c, _ = api(f"/repos/{full}/contents/{path}")
    print(f"  fayl {path}: {c}")

code, v = api(f"/repos/{full}/actions/workflows")
print("workflow'lar:", [(w["path"].split("/")[-1], w["state"]) for w in v.get("workflows", [])] if code == 200 else (code, v.get("message")))
code, v = api(f"/repos/{full}/actions/workflows/dub.yml/runs?per_page=6")
runs = v.get("workflow_runs", []) if code == 200 else []
print("dub.yml run'lari:", code, len(runs))
for r in runs:
    print(f"  #{r['run_number']} {r['event']} {r['status']}/{r['conclusion']} {r['created_at']} (id {r['id']})")
if runs:
    r = runs[0]
    c, j = api(f"/repos/{full}/actions/runs/{r['id']}/jobs")
    for job in j.get("jobs", []):
        print(f"  job '{job['name']}': {job['status']}/{job['conclusion']}")
        for s in job["steps"]:
            if s["status"] != "pending":
                print(f"    {s['number']:>2} {s['name'][:60]:<60} {s['status']}/{s['conclusion']}")
        bad = [s for s in job["steps"] if s["conclusion"] == "failure"]
        if bad:
            c2, zdata = api(f"/repos/{full}/actions/runs/{r['id']}/logs", raw=True)
            if c2 == 200:
                z = zipfile.ZipFile(io.BytesIO(zdata))
                for s in bad:
                    name = next((n for n in z.namelist() if n.endswith(f"/{s['number']}_{s['name']}.txt") or f"_{s['name']}" in n and n.startswith(f"{job['name']}/")), None)
                    if name:
                        lines = z.read(name).decode(errors="replace").splitlines()[-30:]
                        print(f"  --- '{s['name']}' log oxiri (tozalangan) ---")
                        for ln in lines:
                            print("   ", clean(ln))
            else:
                print("  log olinmadi:", c2)
