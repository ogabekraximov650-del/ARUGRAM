#!/usr/bin/env python3
"""Yangi GitHub akkauntda avto-kodlash repo'sini yaratadi va sozlaydi.

`.github/workflows/setup-autoencode.yml` ishga tushiradi. Qiladigan ishlari:
  1. NEW_GH_TOKEN qaysi akkauntniki ekanini aniqlaydi (eski akkaunt bo'lsa —
     to'xtaydi: adashib eski akkauntda repo ochilmasin);
  2. bo'sh repo yaratadi (bor bo'lsa — o'shani ishlatadi);
  3. run.py, requirements.txt, session.enc va workflow'ni yuklaydi;
  4. secret'larni (TG_API_ID, TG_API_HASH, ENCODE_TOKEN, API_BASE) shifrlab
     o'rnatadi — qiymatlar logda ko'rinmaydi;
  5. kodlashni ishga tushirmaydi (buni worker qiladi).
"""

import base64
import json
import os
import sys
import time
import urllib.error
import urllib.request
from pathlib import Path

from nacl import encoding, public



def _token():
    """NEW_GH_TOKEN secret'i yoki `gh_token.enc` (ENCODE_TOKEN bilan shifrlangan)."""
    t = os.environ.get("NEW_GH_TOKEN", "").strip()
    enc = HERE / "gh_token.enc"
    if not t and enc.exists():
        import subprocess
        k = "".join(os.environ.get("ENCODE_TOKEN", "").split())
        r = subprocess.run(
            ["openssl", "enc", "-d", "-aes-256-cbc", "-pbkdf2", "-iter", "200000",
             "-pass", "env:K", "-in", str(enc)],
            env={**os.environ, "K": k}, capture_output=True)
        t = r.stdout.decode().strip()
        print("Token shifrlangan fayldan olindi" if t else "gh_token.enc ochilmadi")
    if not t:
        sys.exit("::error::Token yo'q (NEW_GH_TOKEN secret'i yoki gh_token.enc)")
    return t


HERE = Path(__file__).parent
PACKS = HERE.parent / "packs"
TOKEN = _token()
NAME = os.environ.get("REPO_NAME", "avtoencode").strip() or "avtoencode"
PRIVATE = os.environ.get("REPO_PRIVATE", "true") == "true"
ALLOW_SAME = os.environ.get("ALLOW_SAME_ACCOUNT", "false") == "true"
OLD_OWNER = os.environ.get("GITHUB_REPOSITORY_OWNER", "")
def api(method, path, body=None, ok=(200, 201, 204)):
    req = urllib.request.Request(
        "https://api.github.com" + path,
        method=method,
        data=json.dumps(body).encode() if body is not None else None,
        headers={
            "Authorization": f"Bearer {TOKEN}",
            "Accept": "application/vnd.github+json",
            "Content-Type": "application/json",
            "User-Agent": "arugram-setup",
        },
    )
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
    _, me = api("GET", "/user")
    login = me["login"]
    print(f"Token akkaunti: {login}")
    if login.lower() == OLD_OWNER.lower() and not ALLOW_SAME:
        sys.exit(f"::error::Token '{login}' akkauntiniki — bu ESKI akkaunt. "
                 "Yangi akkaunt tokenini NEW_GH_TOKEN secret'iga qo'ying.")

    code, _ = api("POST", "/user/repos",
                  {"name": NAME, "private": PRIVATE,
                   "description": "Avto-kodlash (H.265)"}, ok=(201, 422))
    print("Repo yaratildi" if code == 201 else "Repo bor — o'shanga yuklanadi")
    repo = f"/repos/{login}/{NAME}"
    # Mavjud reponing ko'rinishi so'ralganiga moslanadi (public/private).
    _, info = api("GET", repo)
    if bool(info.get("private")) != PRIVATE:
        api("PATCH", repo, {"private": PRIVATE})
        print("Repo endi " + ("private" if PRIVATE else "PUBLIC"))

    files = {
        ".github/workflows/encode.yml": HERE / "avtoencode.workflow.yml",
        "tool/encode/run.py": HERE / "run.py",
        # To'plamlar (emoji/GIF/stiker) — ALOHIDA workflow (`packs.yml`).
        ".github/workflows/packs.yml": PACKS / "packs.workflow.yml",
        "tool/packs/run.py": PACKS / "run.py",
        "tool/packs/arupack.py": PACKS / "arupack.py",
        "tool/packs/arunorm.py": PACKS / "arunorm.py",
        "tool/packs/requirements.txt": PACKS / "requirements.txt",
        "tool/encode/requirements.txt": HERE / "requirements.txt",
        "tool/encode/session.enc": HERE / "session.enc",
    }
    for dest, src in files.items():
        body = {"message": f"Avto-kodlash: {dest}",
                "content": base64.b64encode(src.read_bytes()).decode(),
                "branch": "main"}
        code, cur = api("GET", f"{repo}/contents/{dest}?ref=main", ok=(200, 404))
        if code == 200:
            body["sha"] = cur["sha"]
        api("PUT", f"{repo}/contents/{dest}", body)
        print(f"Yuklandi: {dest}")

    _, pk = api("GET", f"{repo}/actions/secrets/public-key")
    box = public.SealedBox(public.PublicKey(pk["key"].encode(), encoding.Base64Encoder()))
    for name in ("TG_API_ID", "TG_API_HASH", "ENCODE_TOKEN", "API_BASE"):
        val = os.environ.get(name, "").strip()
        if not val:
            print(f"O'tkazildi (qiymat yo'q): {name}")
            continue
        enc = base64.b64encode(box.encrypt(val.encode())).decode()
        api("PUT", f"{repo}/actions/secrets/{name}",
            {"encrypted_value": enc, "key_id": pk["key_id"]})
        print(f"Secret o'rnatildi: {name}")

    # Kodlashni bu skript ISHGA TUSHIRMAYDI: uni Cloudflare worker o'zi
    # navbatga qarab ishga tushiradi (aks holda ishlab turgan run bilan
    # ikkinchisi to'qnashardi).
    print(f"Tayyor: https://github.com/{login}/{NAME}")


main()
