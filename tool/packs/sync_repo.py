#!/usr/bin/env python3
"""`tool/packs/` fayllarini avtoencode reposiga (boshqa akkaunt) yangilaydi.

MUHIM: to'plam Actions'i (`packs.yml`) BOSHQA akkauntdagi repoda ishlaydi va
uning kodi o'sha yerga NUSXA qilib qo'yilgan. Bu repodagi o'zgarish o'z-o'zidan
o'sha yerga o'tmaydi — shu skript o'tkazadi (`sync-packs.yml`, `tool/packs/**`
o'zgarganda avtomatik). Faqat o'zgargan fayllar yuklanadi.

Token: `tool/encode/gh_token.enc` (ENCODE_TOKEN bilan shifrlangan) yoki NEW_GH_TOKEN.
"""
import base64
import json
import os
import subprocess
import sys
import urllib.error
import urllib.request
from pathlib import Path

HERE = Path(__file__).parent
ENC = HERE.parent / "encode"
REPO = (ENC / "gh_repo.txt").read_text().strip()

FILES = {
    ".github/workflows/packs.yml": HERE / "packs.workflow.yml",
    "tool/packs/run.py": HERE / "run.py",
    "tool/packs/arupack.py": HERE / "arupack.py",
    "tool/packs/arunorm.py": HERE / "arunorm.py",
    "tool/packs/requirements.txt": HERE / "requirements.txt",
}


def token() -> str:
    t = os.environ.get("NEW_GH_TOKEN", "").strip()
    enc = ENC / "gh_token.enc"
    if not t and enc.exists():
        k = "".join(os.environ.get("ENCODE_TOKEN", "").split())
        r = subprocess.run(
            ["openssl", "enc", "-d", "-aes-256-cbc", "-pbkdf2", "-iter", "200000",
             "-pass", "env:K", "-in", str(enc)],
            env={**os.environ, "K": k}, capture_output=True)
        t = r.stdout.decode().strip()
    if not t:
        sys.exit("::error::Token yo'q (gh_token.enc ochilmadi va NEW_GH_TOKEN yo'q)")
    return t


TOKEN = token()


def api(method, path, body=None, ok=(200, 201)):
    req = urllib.request.Request(
        "https://api.github.com" + path, method=method,
        data=json.dumps(body).encode() if body is not None else None,
        headers={"Authorization": f"Bearer {TOKEN}",
                 "Accept": "application/vnd.github+json",
                 "Content-Type": "application/json", "User-Agent": "arugram-sync"})
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


def main():
    changed = 0
    for dest, src in FILES.items():
        new = src.read_bytes()
        code, cur = api("GET", f"/repos/{REPO}/contents/{dest}?ref=main", ok=(200, 404))
        if code == 200:
            old = base64.b64decode(cur.get("content", "").replace("\n", ""))
            if old == new:
                print(f"O'zgarmagan: {dest}")
                continue
        body = {"message": f"Sinxronlash: {dest}",
                "content": base64.b64encode(new).decode(), "branch": "main"}
        if code == 200:
            body["sha"] = cur["sha"]
        api("PUT", f"/repos/{REPO}/contents/{dest}", body)
        changed += 1
        print(f"Yangilandi: {dest}")
    print(f"Tayyor: {changed} ta fayl yangilandi")


if __name__ == "__main__":
    main()
