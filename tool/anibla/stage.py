"""UZ: Anibla videosini Telegram'ga qismlab qo'yish uchun toza (tarmoqsiz) yordamchilar. 2 GB dan katta video BAYT bo'yicha ~1,9 GB li qismlarga bo'linadi;
butun fayl BITTA kalit bilan AES-128-CTR da shifrlanadi (hisoblagich fayl bo'ylab davom etadi), shuning uchun qismlarni ketma-ket ulash (`cat`) va
bir marta ochish asl faylni beradi. Qism chegarasi 16 baytga karrali (CTR bloki).
EN: pure (network-free) helpers for staging an Anibla video in Telegram. A video over 2 GB is split by BYTES into ~1.9 GB parts; the whole file is
    encrypted with ONE AES-128-CTR key (counter continues across the file), so concatenating the parts and decrypting once yields the original.
RU: chistye pomoshchniki dlya razmeshcheniya video v Telegram po chastyam (>2 GB), odin klyuch AES-128-CTR na ves fail.
"""
import shutil
import subprocess
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


# ── Telegram'da OCHILADIGAN qismlar (2026-11): vaqt bo'yicha bo'lingan oddiy mp4 (shifrsiz — asl video ≤2 GB bo'lganda ham shunday) ──
# UZ: 2 GB dan katta video kalit-kadr chegaralarida (`-c copy`, qayta kodlashsiz) ~1,7 GB li mp4 qismlarga bo'linadi; har qism Telegram'da o'zi ochiladigan video.
#     Kodlovchi qismlarni ffmpeg `concat` bilan ulaydi. EN: a >2 GB video is stream-copied into ~1.7 GB playable mp4 segments; the encoder rejoins them with ffmpeg concat.
#     RU: video >2 GB rezhetsya (-c copy) na vosproizvodimye mp4-chasti ~1,7 GB; kodirovshchik skleivaet ikh ffmpeg concat.
PLAY_TARGET = 1700 * 1048576
PLAY_MAX = 2000 * 1048576


def split_playable(src, outdir, duration: float, size: int, target: int = PLAY_TARGET, limit: int = PLAY_MAX) -> list[Path]:
    """`src` ni `outdir/part%03d.mp4` qismlarga bo'ladi; birortasi `limit` dan oshsa vaqtni qisqartirib qayta urinadi. Qaytadi: tartiblangan yo'llar."""
    if duration <= 0 or size <= 0:
        raise ValueError("davomiylik/hajm noma'lum")
    outdir = Path(outdir)
    seg = max(5, int(duration * target / size))
    for _ in range(5):
        shutil.rmtree(outdir, ignore_errors=True)
        outdir.mkdir(parents=True)
        r = subprocess.run(["ffmpeg", "-y", "-loglevel", "error", "-i", str(src), "-map", "0:v:0", "-map", "0:a?", "-c", "copy", "-f", "segment",
                            "-segment_time", str(seg), "-reset_timestamps", "1", "-segment_format", "mp4",
                            "-segment_format_options", "movflags=+faststart", str(outdir / "part%03d.mp4")], capture_output=True, text=True)
        parts = sorted(outdir.glob("part*.mp4"))
        if r.returncode == 0 and len(parts) > 1 and all(0 < p.stat().st_size <= limit for p in parts):
            return parts
        if r.returncode == 0 and len(parts) == 1:
            seg = max(5, seg // 2)  # bitta qism chiqdi — bo'linmadi
            continue
        seg = max(5, int(seg * 0.7))
    raise RuntimeError("video ochiladigan qismlarga bo'linmadi")


def concat_parts(paths, dst) -> None:
    """Qismlarni (`split_playable` natijasi) `-c copy` bilan bitta mp4 ga ulaydi."""
    lst = Path(str(dst) + ".list")
    lst.write_text("".join("file '" + str(Path(p).resolve()).replace("'", "'\\''") + "'\n" for p in paths))
    r = subprocess.run(["ffmpeg", "-y", "-loglevel", "error", "-f", "concat", "-safe", "0", "-i", str(lst), "-c", "copy", str(dst)], capture_output=True, text=True)
    lst.unlink(missing_ok=True)
    if r.returncode != 0 or not Path(dst).exists() or Path(dst).stat().st_size == 0:
        raise RuntimeError("qismlar ulanmadi: " + (r.stderr or "")[-200:])
