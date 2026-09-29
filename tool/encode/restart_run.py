#!/usr/bin/env python3
"""Avto-kodlash repo'sida ishlayotgan run'ni to'xtatib, yangisini ishga tushiradi.

`.github/workflows/restart-autoencode.yml` ishga tushiradi:
  1. `avtoencode` repodagi ishlayotgan/kutayotgan `encode.yml` run'larini
     bekor qiladi;
  2. worker'da "ishlayapti" turgan ishlarni ijarasiz navbatga qaytaradi
     (`/api/encode/release`) — yangi run ularni darhol oladi;
  3. `encode.yml` ni yangidan ishga tushiradi.
Token: NEW_GH_TOKEN secret'i yoki `gh_token.enc` (ENCODE_TOKEN bilan shifrlangan).
"""

import json
import os
import subprocess
import sys
import time
import urllib.error
import urllib.request
from pathlib import Path

HERE = Path(__file__).parent
REPO = (HERE / "gh_repo.txt").read_text().strip()
API_BASE = (os.environ.get("API_BASE") or "https://arugram.uzcom.workers.dev").rstrip("/")
ENCODE_TOKEN = "".join(os.environ.get("ENCODE_TOKEN", "").split())


def token():
    t = os.environ.get("NEW_GH_TOKEN", "").strip()
    enc = HERE / "gh_token.enc"
    if not t and enc.exists():
        r = subprocess.run(
            ["openssl", "enc", "-d", "-aes-256-cbc", "-pbkdf2", "-iter", "200000",
             "-pass", "env:K", "-in", str(enc)],
            env={**os.environ, "K": ENCODE_TOKEN}, capture_output=True)
        t = r.stdout.decode().strip()
    if not t:
        sys.exit("::error::Token yo'q")
    return t


TOKEN = token()


def gh(method, path, body=None, ok=(200, 201, 202, 204)):
    req = urllib.request.Request(
        "https://api.github.com" + path, method=method,
        data=json.dumps(body).encode() if body is not None else None,
        headers={"Authorization": f"Bearer {TOKEN}", "Accept": "application/vnd.github+json",
                 "Content-Type": "application/json", "User-Agent": "arugram-restart"})
    try:
        with urllib.request.urlopen(req, timeout=60) as r:
            raw = r.read()
            return r.status, (json.loads(raw) if raw else {})
    except urllib.error.HTTPError as e:
        raw = e.read()
        try:
            data = json.loads(raw)
        except Exception:
            data = {"message": raw.decode(errors="replace")[:200]}
        if e.code in ok:
            return e.code, data
        sys.exit(f"::error::{method} {path} -> {e.code}: {data.get('message')}")


def active_runs():
    out = []
    for st in ("in_progress", "queued", "waiting", "pending", "requested"):
        _, v = gh("GET", f"/repos/{REPO}/actions/workflows/encode.yml/runs?status={st}&per_page=30")
        out += [r["id"] for r in v.get("workflow_runs", [])]
    return sorted(set(out))


def main():
    ids = active_runs()
    print(f"Ishlayotgan/kutayotgan run'lar: {ids or 'yo`q'}")
    for i in ids:
        code, _ = gh("POST", f"/repos/{REPO}/actions/runs/{i}/cancel", ok=(202, 409))
        print(f"  run {i}: bekor qilish so'rovi -> {code}")
    # Bekor qilinguncha kutamiz (ko'pi bilan ~2 daqiqa), so'ng majburan.
    for n in range(24):
        left = active_runs()
        if not left:
            break
        if n == 12:
            for i in left:
                gh("POST", f"/repos/{REPO}/actions/runs/{i}/force-cancel", ok=(202, 409))
        time.sleep(5)
    print("Bekor qilingandan keyin qolgan run'lar:", active_runs() or "yo`q")

    req = urllib.request.Request(
        f"{API_BASE}/api/encode/release", method="POST", data=b"{}",
        headers={"X-Encode-Token": ENCODE_TOKEN, "Content-Type": "application/json",
                 "User-Agent": "arugram-restart"})
    try:
        with urllib.request.urlopen(req, timeout=60) as r:
            print("Worker: ishlar navbatga qaytarildi", r.status)
    except urllib.error.HTTPError as e:
        print(f"::warning::Worker release -> {e.code} {e.read()[:150]!r}")

    code, _ = gh("POST", f"/repos/{REPO}/actions/workflows/encode.yml/dispatches",
                 {"ref": "main"}, ok=(204,))
    print("Yangi run ishga tushirildi" if code == 204 else f"dispatch -> {code}")


main()
