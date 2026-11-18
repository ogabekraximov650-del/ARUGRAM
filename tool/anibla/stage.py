"""UZ: Anibla videosini Telegram'ga qismlab qo'yish uchun toza (tarmoqsiz) yordamchilar. 2 GB dan katta video BAYT bo'yicha ~1,9 GB li qismlarga bo'linadi;
butun fayl BITTA kalit bilan AES-128-CTR da shifrlanadi (hisoblagich fayl bo'ylab davom etadi), shuning uchun qismlarni ketma-ket ulash (`cat`) va
bir marta ochish asl faylni beradi. Qism chegarasi 16 baytga karrali (CTR bloki).
EN: pure (network-free) helpers for staging an Anibla video in Telegram. A video over 2 GB is split by BYTES into ~1.9 GB parts; the whole file is
    encrypted with ONE AES-128-CTR key (counter continues across the file), so concatenating the parts and decrypting once yields the original.
RU: chistye pomoshchniki dlya razmeshcheniya video v Telegram po chastyam (>2 GB), odin klyuch AES-128-CTR na ves fail.
"""
from pathlib import Path

from cryptography.hazmat.primitives.ciphers import Cipher, algorithms, modes

# Telegram oddiy akkaunt chegarasi 2000 MiB; zaxira bilan 1900 MiB (1 MiB = 16 ga karrali).
PART = 1900 * 1048576
BLOCK = 8 * 1048576


def plan_parts(size: int, part: int = PART) -> list[tuple[int, int]]:
    """`(boshlanish, uzunlik)` ro'yxati; oxirgi qism qisqaroq bo'lishi mumkin."""
    if size <= 0:
        return []
    if part % 16:
        raise ValueError("qism hajmi 16 ga karrali bo'lishi shart")
    return [(o, min(part, size - o)) for o in range(0, size, part)]


def encrypt_range(src, key: bytes, offset: int, length: int, dst) -> int:
    """`src` ning [offset, offset+length) oralig'ini AES-128-CTR (hisoblagich `offset//16` dan) bilan shifrlab `dst` ga yozadi. Qaytadi: yozilgan bayt."""
    if offset % 16:
        raise ValueError("offset 16 ga karrali bo'lishi shart")
    enc = Cipher(algorithms.AES(key), modes.CTR((offset // 16).to_bytes(16, "big"))).encryptor()
    done = 0
    with open(src, "rb") as f, open(dst, "wb") as out:
        f.seek(offset)
        while done < length:
            chunk = f.read(min(BLOCK, length - done))
            if not chunk:
                break
            out.write(enc.update(chunk))
            done += len(chunk)
    return done


def join_decrypt(parts, key: bytes, dst) -> int:
    """Qismlarni tartib bilan ulab (offset 0 dan) shifrini ochadi va `dst` ga yozadi. Qaytadi: yozilgan bayt."""
    dec = Cipher(algorithms.AES(key), modes.CTR((0).to_bytes(16, "big"))).decryptor()
    total = 0
    with open(dst, "wb") as out:
        for p in parts:
            with open(p, "rb") as f:
                while True:
                    chunk = f.read(BLOCK)
                    if not chunk:
                        break
                    out.write(dec.update(chunk))
                    total += len(chunk)
    return total
