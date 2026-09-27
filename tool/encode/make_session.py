#!/usr/bin/env python3
"""Avto-kodlash uchun Telegram sessiyasini yaratadi (O'Z qurilmangizda).

`.github/workflows/encode.yml` kanal egasining Pyrogram sessiyasini
`PYRO_SESSION_B64_1/2/3` secret'laridan tiklaydi. Bu skript o'sha uchta
qiymatni tayyorlaydi.

XAVFSIZLIK: sessiya — hisobingizga TO'LIQ kirish kaliti. Skriptni faqat
o'zingizning telefoningiz (Termux) yoki kompyuteringizda ishga tushiring;
kodni va chiqqan qiymatlarni hech kimga yubormang — faqat GitHub
secret'lariga qo'ying.

Ishlatish:
    pip install pyrogram==2.0.106 tgcrypto==1.2.5
    python3 make_session.py            # qiymatlar ekranga va 3 ta .txt faylga
    python3 make_session.py --send     # + fayllar Saqlangan xabarlarga

Kirish: telefon raqam, Telegram'ga kelgan kod va (bo'lsa) ikki bosqichli
parol so'raladi. api_id / api_hash — my.telegram.org dan.
"""

import base64
import os
import sys
from pathlib import Path

from pyrogram import Client

NAME = "arugram_encode"


def main():
    api_id = int(os.environ.get("TG_API_ID") or input("api_id: ").strip())
    api_hash = (os.environ.get("TG_API_HASH") or input("api_hash: ")).strip()
    send = "--send" in sys.argv

    with Client(NAME, api_id=api_id, api_hash=api_hash) as app:
        me = app.get_me()
        print(f"\nKirildi: {me.first_name} (id {me.id})")
        # Sessiya fayli yopilgandan keyin o'qiladi.

    raw = Path(f"{NAME}.session").read_bytes()
    b64 = base64.b64encode(raw).decode()
    n = (len(b64) + 2) // 3
    parts = [b64[i * n:(i + 1) * n] for i in range(3)]
    files = []
    for i, p in enumerate(parts, 1):
        f = Path(f"PYRO_SESSION_B64_{i}.txt")
        f.write_text(p)
        files.append(f)
        print(f"\nPYRO_SESSION_B64_{i} ({len(p)} belgi) -> {f}")

    if send:
        with Client(NAME, api_id=api_id, api_hash=api_hash) as app:
            for f in files:
                app.send_document("me", str(f), caption=f.stem)
        print("\nSaqlangan xabarlarga yuborildi. GitHub'ga qo'ygach ularni o'chiring.")

    print("\nGitHub -> ARUGRAM -> Settings -> Secrets and variables -> Actions:")
    print("  PYRO_SESSION_B64_1/2/3 = fayllardagi qiymatlar.")
    print(f"Ish tugagach {NAME}.session va .txt fayllarni o'chiring.")


if __name__ == "__main__":
    main()
