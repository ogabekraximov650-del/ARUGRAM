"""Foydalanuvchi yuklagan rasmni to'plamga mos YENGIL WebP ga aylantiradi.

Maqsad — kuchsiz telefonlar ko'tara olsin:
  * o'lcham cheklanadi (stiker 512, emoji 128, GIF 480 px);
  * animatsiya sekundiga ko'pi bilan 20 kadr (ortiqcha kadrlar tashlanadi,
    vaqti oldingi kadrga qo'shiladi — tezlik o'zgarmaydi);
  * uzunlik va kadrlar soni cheklanadi (juda og'ir animatsiya RAD etiladi,
    kesib tashlanmaydi — egasiga sababi aytiladi);
  * har elementga kichik statik rasm (thumb) yasaladi: to'plam oynasida
    faqat shu ko'rinadi, animatsiya esa faqat kerak bo'lganda ochiladi.

Kiruvchi turlar: PNG, JPEG, GIF, WebP (animatsiyali ham). Fayl turi kengaytmadan
emas, BAYTLARIDAN aniqlanadi. Chiqish hajmi <= 5 MB (`arupack.MAX_ITEM`).
"""

import io
from dataclasses import dataclass

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


def normalize(raw: bytes, kind: str, emoji: str = "") -> Result:
    """Foydalanuvchi faylini to'plam elementiga aylantiradi (yoki `Rejected`)."""
    if kind not in LIMITS:
        raise Rejected("noto'g'ri tur")
    lim = LIMITS[kind]
    fmt = sniff(raw)
    if not fmt:
        raise Rejected("fayl turi qo'llanmaydi (PNG, JPG, GIF yoki WebP kerak)")
    if len(raw) > arupack.MAX_ITEM:
        raise Rejected("fayl 5 MB dan katta")
    try:
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
