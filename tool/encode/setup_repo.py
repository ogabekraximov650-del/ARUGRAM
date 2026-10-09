#!/usr/bin/env python3
"""Yangi GitHub akkauntda avto-kodlash repo'sini yaratadi va sozlaydi.

`.github/workflows/setup-autoencode.yml` ishga tushiradi. Qiladigan ishlari:
  1. NEW_GH_TOKEN qaysi akkauntniki ekanini aniqlaydi (eski akkaunt bo'lsa —
     to'xtaydi: adashib eski akkauntda repo ochilmasin);
  2. bo'sh repo yaratadi (bor bo'lsa — o'shani ishlatadi);
  3. run.py, requirements.txt, session.enc, 4 ta workflow'ni (`encode.yml`,
     `packs.yml`, `post.yml`, `anibla.yml`) va ularning fayllarini yuklaydi;
  4. secret'larni (TG_API_ID, TG_API_HASH, ENCODE_TOKEN, API_BASE) shifrlab
     o'rnatadi — qiymatlar logda ko'rinmaydi;
  5. kodlashni ishga tushirmaydi (buni worker qiladi).
"""

import base64
import hashlib
import json
import os
import sys
import time
import urllib.error
import urllib.request
from pathlib import Path

from nacl import encoding, public



def _token():
    """`gh_token.enc` (ENCODE_TOKEN bilan shifrlangan) yoki NEW_GH_TOKEN secret'i.

    Shifrlangan fayl USTUN: akkaunt almashganda faqat shu fayl yangilanadi,
    eski NEW_GH_TOKEN secret'i (eski akkaunt) unga xalaqit bermasin.
    """
    t = ""
    enc = HERE / "gh_token.enc"
    if enc.exists():
        import subprocess
        k = "".join(os.environ.get("ENCODE_TOKEN", "").split())
        r = subprocess.run(
            ["openssl", "enc", "-d", "-aes-256-cbc", "-pbkdf2", "-iter", "200000",
             "-pass", "env:K", "-in", str(enc)],
            env={**os.environ, "K": k}, capture_output=True)
        t = r.stdout.decode().strip()
        print("Token shifrlangan fayldan olindi" if t else "gh_token.enc ochilmadi")
    if not t:
        t = os.environ.get("NEW_GH_TOKEN", "").strip()
    if not t:
        sys.exit("::error::Token yo'q (NEW_GH_TOKEN secret'i yoki gh_token.enc)")
    return t


HERE = Path(__file__).parent
PACKS = HERE.parent / "packs"
POST = HERE.parent / "post"
ANIBLA = HERE.parent / "anibla"
TOKEN = _token()
# Repo nomi: qo'lda berilgani yoki `gh_repo.txt` (`owner/repo` yoki faqat `repo`).
NAME = (os.environ.get("REPO_NAME", "").strip()
        or (HERE / "gh_repo.txt").read_text().strip().split("/")[-1]
        or "avtoencode")
# Bo'sh (masalan `push` bilan avtomatik sinxronlash) — ko'rinish O'ZGARTIRILMAYDI.
PRIVATE_RAW = os.environ.get("REPO_PRIVATE", "").strip()
PRIVATE = PRIVATE_RAW != "false"
ALLOW_SAME = os.environ.get("ALLOW_SAME_ACCOUNT", "true") != "false"
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
    if PRIVATE_RAW and bool(info.get("private")) != PRIVATE:
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
    # "Post kodlash" (kodlash botining ikkinchi bo'limi) — ALOHIDA workflow `post.yml`.
    files[".github/workflows/post.yml"] = POST / "post.workflow.yml"
    for src in sorted(POST.iterdir()):
        if src.is_file() and src.suffix in (".py", ".sh", ".png"):
            files[f"tool/post/{src.name}"] = src
    # "Anibla yuklash" — ALOHIDA workflow `anibla.yml` (`creds.enc` KO'CHIRILMAYDI).
    files[".github/workflows/anibla.yml"] = ANIBLA / "anibla.workflow.yml"
    files["tool/anibla/download.py"] = ANIBLA / "download.py"
    # fMP4 nusxalar (Mini App) — ALOHIDA workflow `fmp4.yml`.
    files[".github/workflows/fmp4.yml"] = HERE.parent / "fmp4" / "fmp4.workflow.yml"
    files["tool/fmp4/run.py"] = HERE.parent / "fmp4" / "run.py"
    for dest, src in files.items():
        raw = src.read_bytes()
        # 409 — shu payt boshqa workflow (`sync-packs.yml`) ham shu repoga
        # yozyapti: qayta o'qib, yana urinadi.
        for attempt in range(6):
            body = {"message": f"Avto-kodlash: {dest}",
                    "content": base64.b64encode(raw).decode(),
                    "branch": "main"}
            code, cur = api("GET", f"{repo}/contents/{dest}?ref=main", ok=(200, 404))
            if code == 200:
                if cur.get("sha") == hashlib.sha1(b"blob %d\0" % len(raw) + raw).hexdigest():
                    print(f"O'zgarmagan: {dest}")
                    break
                body["sha"] = cur["sha"]
            code, _ = api("PUT", f"{repo}/contents/{dest}", body, ok=(200, 201, 409))
            if code != 409:
                print(f"Yuklandi: {dest}")
                break
            time.sleep(2 + attempt * 2)
        else:
            sys.exit(f"::error::{dest} yuklanmadi (409 takrorlandi)")

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
