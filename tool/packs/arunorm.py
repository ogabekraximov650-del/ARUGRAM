"""Foydalanuvchi yuklagan rasmni to'plamga mos YENGIL WebP ga aylantiradi.

Maqsad — kuchsiz telefonlar ko'tara olsin:
  * o'lcham cheklanadi (stiker 512, emoji 128, GIF 480 px);
  * animatsiya sekundiga ko'pi bilan 20 kadr (ortiqcha kadrlar tashlanadi,
    vaqti oldingi kadrga qo'shiladi — tezlik o'zgarmaydi);
  * uzunlik va kadrlar soni cheklanadi (juda og'ir animatsiya RAD etiladi,
    kesib tashlanmaydi — egasiga sababi aytiladi);
  * har elementga kichik statik rasm (thumb) yasaladi: to'plam oynasida
    faqat shu ko'rinadi, animatsiya esa faqat kerak bo'lganda ochiladi.

Kiruvchi turlar: PNG, JPEG, GIF, WebP (animatsiyali ham) va VIDEO (MP4/MOV,
WebM). Fayl turi kengaytmadan emas, BAYTLARIDAN aniqlanadi. Video `ffmpeg`
bilan kadrlarga ajratiladi (foydalanuvchi ilovada tanlagan bo'lak — `trim`
— kesib olinadi, ovoz tashlanadi, emoji uchun markazdan kvadrat kesiladi),
so'ng rasm animatsiyasi bilan bir xil yo'l. Chiqish hajmi <= 5 MB
(`arupack.MAX_ITEM`).
"""

import io
import subprocess
import tempfile
from dataclasses import dataclass
from pathlib import Path

from PIL import Image, ImageSequence, UnidentifiedImageError

import arupack

# Ochiq holatda shuncha piksel/kadr o'qiladi — "dekompressiya bombasi"dan himoya.
Image.MAX_IMAGE_PIXELS = 40_000_000
MAX_SOURCE_SIDE = 4096

LIMITS = {
    #           eng katta tomon, kadrlar, soniya, yumshoq hajm chegarasi
    "sticker": dict(side=512, frames=150, seconds=8.0, soft=512 * 1024),
    "emoji":   dict(side=128, frames=90,  seconds=5.0, soft=256 * 1024),
    "gif":     dict(side=480, frames=200, seconds=15.0, soft=3 * 1024 * 1024),
}
MAX_FPS = 20
THUMB_SIDE = 96
QUALITIES = (80, 65, 50, 40, 30)
SCALES = (1.0, 0.8, 0.65, 0.5)


class Rejected(Exception):
    """Element to'plamga mos emas — matn egasiga ko'rsatiladi."""


@dataclass
class Result:
    data: bytes
    thumb: bytes
    animated: bool
    w: int
    h: int


def sniff(b: bytes) -> str:
    if b[:8] == b"\x89PNG\r\n\x1a\n":
        return "png"
    if b[:3] == b"\xff\xd8\xff":
        return "jpeg"
    if b[:6] in (b"GIF87a", b"GIF89a"):
        return "gif"
    if b[:4] == b"RIFF" and b[8:12] == b"WEBP":
        return "webp"
    if b[4:8] == b"ftyp":
        return "mp4"
    if b[:4] == b"\x1a\x45\xdf\xa3":
        return "webm"
    return ""


def _fit(w: int, h: int, side: int):
    k = min(1.0, side / max(w, h))
    return max(1, round(w * k)), max(1, round(h * k))


def _frames(im: Image.Image, lim: dict):
    """(RGBA kadr, davomiyligi ms) ro'yxati — kadrlar/uzunlik chegarasi bilan."""
    frames, total = [], 0.0
    for fr in ImageSequence.Iterator(im):
        d = fr.info.get("duration", im.info.get("duration", 100))
        d = float(d) if d and d > 0 else 100.0
        # 10 ms dan qisqa kadrlar brauzerlarda 100 ms ga aylanadi — biz ham.
        if d < 20:
            d = 100.0
        frames.append((fr.convert("RGBA"), d))
        total += d
        if len(frames) > lim["frames"]:
            raise Rejected(f"kadrlar juda ko'p (ko'pi bilan {lim['frames']} ta)")
        if total / 1000.0 > lim["seconds"]:
            raise Rejected(f"animatsiya juda uzun (ko'pi bilan {lim['seconds']:.0f} soniya)")
    return frames


VIDEO_FPS = 15


def _run(cmd, timeout=180):
    return subprocess.run(cmd, capture_output=True, timeout=timeout, check=True)


def _video_frames(raw: bytes, kind: str, lim: dict, trim):
    """Video -> (RGBA kadr, ms) ro'yxati va manba o'lchami. `trim` — (boshi_ms,
    oxiri_ms) yoki None."""
    with tempfile.TemporaryDirectory() as d:
        src = Path(d) / "in.bin"
        src.write_bytes(raw)
        try:
            out = _run(["ffprobe", "-v", "error", "-select_streams", "v:0",
                        "-show_entries", "stream=width,height:format=duration",
                        "-of", "csv=p=0", str(src)], 60).stdout.decode().split()
            wh, dur = out[0].split(","), None
            w0, h0 = int(wh[0]), int(wh[1])
            dur = float(out[1].split(",")[-1]) if len(out) > 1 else float(wh[2])
        except Exception:
            raise Rejected("video o'qilmadi")
        if w0 <= 0 or h0 <= 0 or max(w0, h0) > MAX_SOURCE_SIDE or dur <= 0:
            raise Rejected("video o'lchami yoki davomiyligi noto'g'ri")
        start, end = 0.0, dur
        if trim:
            start = max(0.0, min(dur, trim[0] / 1000.0))
            end = max(start, min(dur, trim[1] / 1000.0))
        length = end - start
        if length < 0.2:
            raise Rejected("tanlangan bo'lak juda qisqa")
        if length > lim["seconds"] + 0.05:
            raise Rejected(f"video juda uzun (ko'pi bilan {lim['seconds']:.0f} soniya)")
        side = lim["side"]
        vf = []
        if kind == "emoji":
            vf.append("crop='min(iw,ih)':'min(iw,ih)'")
        vf.append(f"fps={VIDEO_FPS}")
        vf.append(f"scale='min({side},iw)':'min({side},ih)':force_original_aspect_ratio=decrease")
        try:
            _run(["ffmpeg", "-v", "error", "-y", "-ss", f"{start:.3f}", "-t", f"{length:.3f}",
                  "-i", str(src), "-an", "-vf", ",".join(vf),
                  "-frames:v", str(lim["frames"] + 1), str(Path(d) / "f%04d.png")], 180)
        except Exception:
            raise Rejected("videoni qayta ishlab bo'lmadi")
        files = sorted(Path(d).glob("f*.png"))
        if not files:
            raise Rejected("videodan kadr olinmadi")
        if len(files) > lim["frames"]:
            raise Rejected(f"kadrlar juda ko'p (ko'pi bilan {lim['frames']} ta)")
        frames = []
        for f in files:
            with Image.open(f) as im:
                frames.append((im.convert("RGBA"), 1000.0 / VIDEO_FPS))
        w1, h1 = frames[0][0].size
        return frames, w1, h1


def _drop_fast(frames):
    """Sekundiga MAX_FPS dan ortiq kadrlarni tashlaydi (vaqt saqlanadi)."""
    step = 1000.0 / MAX_FPS
    out = []
    for img, d in frames:
        if out and out[-1][1] < step:
            out[-1][1] += d
            continue
        out.append([img, d])
    return out


def _encode(frames, scale, quality, side_w, side_h):
    w, h = max(1, round(side_w * scale)), max(1, round(side_h * scale))
    imgs = [f[0].resize((w, h), Image.LANCZOS) if (w, h) != f[0].size else f[0]
            for f in frames]
    buf = io.BytesIO()
    if len(imgs) == 1:
        imgs[0].save(buf, "WEBP", quality=quality, method=4)
    else:
        imgs[0].save(buf, "WEBP", save_all=True, append_images=imgs[1:],
                     duration=[int(f[1]) for f in frames], loop=0,
                     quality=quality, method=4, minimize_size=False)
    return buf.getvalue(), w, h


def normalize(raw: bytes, kind: str, emoji: str = "", trim=None) -> Result:
    """Foydalanuvchi faylini to'plam elementiga aylantiradi (yoki `Rejected`).
    `trim` — video uchun (boshi_ms, oxiri_ms)."""
    if kind not in LIMITS:
        raise Rejected("noto'g'ri tur")
    lim = LIMITS[kind]
    fmt = sniff(raw)
    if not fmt:
        raise Rejected("fayl turi qo'llanmaydi (PNG, JPG, GIF yoki WebP kerak)")
    if len(raw) > arupack.MAX_ITEM:
        raise Rejected("fayl 5 MB dan katta")
    try:
        if fmt in ("mp4", "webm"):
            frames, w0, h0 = _video_frames(raw, kind, lim, trim)
        else:
            im = Image.open(io.BytesIO(raw))
            w0, h0 = im.size
            if w0 <= 0 or h0 <= 0 or max(w0, h0) > MAX_SOURCE_SIDE:
                raise Rejected(f"rasm o'lchami juda katta (ko'pi bilan {MAX_SOURCE_SIDE} px)")
            frames = _frames(im, lim)
    except Rejected:
        raise
    except (UnidentifiedImageError, OSError, ValueError, SyntaxError, EOFError,
            Image.DecompressionBombError) as e:
        raise Rejected(f"fayl o'qilmadi ({type(e).__name__})")

    if not frames:
        raise Rejected("rasm bo'sh")
    animated = len(frames) > 1
    if animated:
        frames = _drop_fast(frames)
        animated = len(frames) > 1
    fw, fh = _fit(w0, h0, lim["side"])
    if kind == "emoji" and abs(fw - fh) > max(fw, fh) * 0.15:
        # Emoji matn ichida kvadrat katakda turadi — cho'zilgan rasm rad etiladi.
        raise Rejected("emoji kvadrat bo'lishi kerak")

    best = None
    for scale in SCALES:
        for q in QUALITIES:
            data, w, h = _encode(frames, scale, q, fw, fh)
            if best is None or len(data) < len(best[0]):
                best = (data, w, h)
            if len(data) <= lim["soft"]:
                best = (data, w, h)
                break
        else:
            continue
        break
    data, w, h = best
    if len(data) > arupack.MAX_ITEM:
        raise Rejected("yengillashtirilgandan keyin ham 5 MB dan katta")

    tw, th = _fit(w0, h0, THUMB_SIDE)
    tb = io.BytesIO()
    frames[0][0].resize((tw, th), Image.LANCZOS).save(tb, "WEBP", quality=60, method=4)
    return Result(data=data, thumb=tb.getvalue(), animated=animated, w=w, h=h)
