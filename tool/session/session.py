#!/usr/bin/env python3
"""Yangi Telegram sessiyasi yaratadi (kodlash botidagi "🔑 Yangi sessiya").

Worker (`worker/src/sessionbot.rs`) bu run'ni ishga tushiradi. Run:
  1. `/api/session/poll` dan telefon raqamini oladi (raqam workflow kiritmalarida YO'Q —
     ochiq repoda ham ko'rinmasin);
  2. raqamga Telegram kodini yuboradi (`code_sent`);
  3. foydalanuvchi botga yuborgan kodni (kerak bo'lsa 2FA parolni) poll orqali oladi;
  4. yangi sessiyani `ENCODE_TOKEN` bilan shifrlab `tool/encode/session.enc` ga yozadi va
     repoga commit qiladi (hamma workflow shu fayldan oladi) — `done`.
Xato bo'lsa `error` bilan sababi botga yuboriladi.
"""
import asyncio
import json
import os
import subprocess
import sys
import time
import urllib.error
import urllib.request
from pathlib import Path

from pyrogram import Client
from pyrogram.errors import (FloodWait, PasswordHashInvalid, PhoneCodeExpired, PhoneCodeInvalid,
                             PhoneNumberBanned, PhoneNumberInvalid, SessionPasswordNeeded)

API = os.environ["API_BASE"].rstrip("/")
TOKEN = "".join(os.environ["ENCODE_TOKEN"].split())
WORK = Path(os.environ.get("RUNNER_TEMP", "/tmp")) / "arugram_newsession"
SESS = WORK / "new_session"
OUT = Path("tool/encode/session.enc")

WAIT_PHONE = 300   # raqam kelguncha (s)
WAIT_CODE = 480    # kod kelguncha (s)
WAIT_PW = 300      # parol kelguncha (s)


def log(*a):
    print(time.strftime("%H:%M:%S"), *a, flush=True)


def call(path, body=None):
    data = json.dumps(body or {}).encode()
    err = None
    for attempt in range(4):
        req = urllib.request.Request(
            f"{API}/api/session/{path}", data=data, method="POST",
            headers={"X-Encode-Token": TOKEN, "Content-Type": "application/json",
                     "User-Agent": "arugram-session"})
        try:
            with urllib.request.urlopen(req, timeout=40) as r:
                return json.loads(r.read() or b"{}")
        except urllib.error.HTTPError as e:
            if e.code in (400, 401, 403, 404):
                raise RuntimeError(f"{path}: HTTP {e.code}")
            err = e
        except Exception as e:  # tarmoq
            err = e
        time.sleep(2 + attempt * 2)
    raise RuntimeError(f"{path}: {err}")


def report(event, text=""):
    try:
        call("report", {"event": event, "text": text})
    except Exception as e:  # noqa: BLE001
        log("report xato:", e)


class Stop(Exception):
    pass


def wait_state(want, timeout):
    """Worker holati `want` bo'lguncha kutadi (bo'sh holat — bekor qilingan)."""
    end = time.time() + timeout
    while time.time() < end:
        r = call("poll")
        st = r.get("state", "")
        if st == want:
            return r
        if st == "":
            raise Stop("bekor qilindi yoki vaqti o'tdi")
        time.sleep(3)
    raise Stop("vaqt tugadi (javob kelmadi)")


def run(cmd, **kw):
    return subprocess.run(cmd, check=True, capture_output=True, text=True, **kw)


def save_session():
    f = SESS.with_suffix(".session")
    if not f.exists() or f.read_bytes()[:15] != b"SQLite format 3":
        raise RuntimeError("sessiya fayli yaratilmadi")
    env = {**os.environ, "K": TOKEN}
    OUT.parent.mkdir(parents=True, exist_ok=True)
    subprocess.run(["openssl", "enc", "-aes-256-cbc", "-pbkdf2", "-iter", "200000", "-salt",
                    "-pass", "env:K", "-in", str(f), "-out", str(OUT)], check=True, env=env)
    # Tekshiruv: qayta ochilsa SQLite bo'lishi kerak.
    back = subprocess.run(["openssl", "enc", "-d", "-aes-256-cbc", "-pbkdf2", "-iter", "200000",
                           "-pass", "env:K", "-in", str(OUT)], check=True, env=env,
                          capture_output=True).stdout
    if back[:15] != b"SQLite format 3":
        raise RuntimeError("shifrlangan sessiya tekshiruvdan o'tmadi")
    run(["git", "config", "user.name", "arugram-session"])
    run(["git", "config", "user.email", "session@users.noreply.github.com"])
    run(["git", "add", str(OUT)])
    run(["git", "commit", "-m", "Yangi Telegram sessiyasi (bot orqali)"])
    for attempt in range(4):
        p = subprocess.run(["git", "push", "origin", "HEAD:main"], capture_output=True, text=True)
        if p.returncode == 0:
            return
        subprocess.run(["git", "pull", "--rebase", "origin", "main"], capture_output=True, text=True)
        time.sleep(2 + attempt * 2)
    raise RuntimeError("repoga yozib bo'lmadi: " + p.stderr[-200:])


async def main():
    WORK.mkdir(parents=True, exist_ok=True)
    for f in WORK.glob("new_session*"):
        f.unlink()
    r = wait_state("wait_send", WAIT_PHONE)
    phone = "+" + "".join(c for c in r.get("phone", "") if c.isdigit())
    log("kod so'ralmoqda")
    app = Client(str(SESS), api_id=int(os.environ["TG_API_ID"]),
                 api_hash=os.environ["TG_API_HASH"], no_updates=True)
    await app.connect()
    try:
        try:
            sent = await app.send_code(phone)
        except PhoneNumberInvalid:
            raise RuntimeError("telefon raqami noto'g'ri")
        except PhoneNumberBanned:
            raise RuntimeError("bu raqam Telegram'da bloklangan")
        except FloodWait as e:
            raise RuntimeError(f"Telegram cheklovi: {e.value} soniyadan keyin urinib ko'ring")
        kind = str(getattr(sent.type, "name", sent.type)).lower()
        report("code_sent", "(Telegram ilovasiga)" if "app" in kind else f"({kind})")
        me = None
        code_ok = False
        while me is None:
            if not code_ok:
                r = wait_state("wait_sign", WAIT_CODE)
                try:
                    res = await app.sign_in(phone, sent.phone_code_hash, r["code"])
                    if res is False:
                        raise RuntimeError("bu raqam Telegram'da ro'yxatdan o'tmagan")
                    me = res
                    code_ok = True
                except PhoneCodeInvalid:
                    report("bad_code")
                    continue
                except PhoneCodeExpired:
                    raise RuntimeError("kod eskirgan — yangidan boshlang")
                except SessionPasswordNeeded:
                    code_ok = True
                    report("need_password")
            if me is None:
                r = wait_state("wait_pw", WAIT_PW)
                try:
                    me = await app.check_password(r["pw"])
                except PasswordHashInvalid:
                    report("bad_password")
        name = " ".join(x for x in (getattr(me, "first_name", ""), getattr(me, "last_name", "")) if x)
        try:
            await app.storage.save()
        except Exception:  # noqa: BLE001
            pass
    finally:
        try:
            await app.disconnect()
        except Exception:  # noqa: BLE001
            pass
    save_session()
    log("sessiya saqlandi:", name)
    report("done", f"({name})" if name else "")


if __name__ == "__main__":
    try:
        asyncio.run(main())
    except Stop as e:
        log("to'xtadi:", e)
        report("error", str(e))
        sys.exit(0)
    except Exception as e:  # noqa: BLE001
        log("XATO:", e)
        report("error", str(e))
        sys.exit(1)
