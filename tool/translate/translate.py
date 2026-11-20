#!/usr/bin/env python3
"""Manga tarjima — kodlash botining "🌐 Manga tarjima" bo'limi (`worker/src/trbot.rs`).

ALOHIDA workflow (`avtoencode` repoda `.github/workflows/translate.yml`, shablon —
`tool/translate/translate.workflow.yml`). Worker navbatda ish bo'lsa uni o'zi ishga tushiradi.

Run navbat bo'shaguncha ishlarni KETMA-KET bajaradi. Bitta ish:
  1. rasm / ZIP / CBZ / PDF yopiq kanaldan yuklab olinadi (bot ko'chirgan nusxalar), hammasi
     bitta rasmlar ro'yxatiga yoyiladi (ZIP/CBZ ichidagi rasmlar va PDF sahifalari tabiiy tartibda);
  2. har rasmda EasyOCR matnni topadi (uzun manhwa lentalari bo'laklab o'qiladi);
  3. yaqin qutilar bitta pufakchaga birlashtiriladi, hamma matn BIRGA (kontekst saqlansin)
     worker orqali tarjima qilinadi (`/api/encode/translate`, Workers AI);
  4. eski matn o'chiriladi (oq/bir xil fon — bo'yab, murakkab fon — inpaint) va o'zbekcha matn yoziladi;
  5. hammasi bitta fayl (PDF / CBZ / ZIP) bo'ladi va suhbatga yuboriladi (asosiy akkaunt, bo'lmasa
     R2 orqali panel bot); `/api/tr/finish` — worker kanal nusxalarini va ish yozuvini o'chiradi.
Jarayon botdagi bitta xabarda jonli ko'rinadi (`Live`, `/api/tr/progress`).
"""

import asyncio
import collections
import io
import json
import os
import re
import shutil
import sys
import time
import urllib.error
import urllib.request
import zipfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
WORK = Path(os.environ.get("RUNNER_TEMP", "/tmp")) / "arumediatv_translate"

IMG_EXT = {".jpg", ".jpeg", ".png", ".webp", ".bmp"}
# OCR tili (EasyOCR kodlari) va tarjima so'rovidagi til nomi.
OCR_LANGS = {
    "korean": ["ko", "en"], "japanese": ["ja", "en"], "chinese": ["ch_sim", "en"],
    "english": ["en"], "russian": ["ru", "en"],
}
TILE_H = 1800       # uzun lentalar shu balandlikdagi bo'laklarda o'qiladi
TILE_OVERLAP = 240  # bo'laklar shuncha ustma-ust tushadi
MIN_CONF = 0.25
CHUNK = 40          # bitta tarjima so'rovidagi matnlar soni
# OCR tezligi: EasyOCR standart canvas_size=2560 (rasmni kattalashtirib o'qiydi) juda sekin — manga matni katta,
# 1280 yetadi (A4 sahifada ~4x tezroq, sinalgan). Kichik matn tushib qolsa OCR_CANVAS ni oshiring.
OCR_CANVAS = int(os.environ.get("OCR_CANVAS", "1280"))
PDF_DPI = int(os.environ.get("PDF_DPI", "120"))

sys.path.insert(0, str(HERE.parent / "upload"))
try:
    import r2client
except Exception:  # noqa: BLE001 — modul yo'q bo'lsa eski usul
    r2client = None


class Lost(Exception):
    """Ish o'chirildi yoki boshqa run'ga o'tdi."""


class Fatal(Exception):
    """Qayta urinishdan foyda yo'q."""


def log(*a):
    print(time.strftime("%H:%M:%S"), *a, flush=True)


# ── Worker bilan aloqa ───────────────────────────────────────

def _post(path, body, timeout=60, tries=5):
    base = os.environ["API_BASE"].rstrip("/")
    data = json.dumps(body).encode()
    err = None
    for attempt in range(tries):
        req = urllib.request.Request(
            f"{base}{path}", data=data, method="POST",
            headers={"X-Encode-Token": os.environ["ENCODE_TOKEN"],
                     "Content-Type": "application/json", "User-Agent": "arumediatv-translate"})
        try:
            with urllib.request.urlopen(req, timeout=timeout) as r:
                return json.loads(r.read() or b"{}")
        except urllib.error.HTTPError as e:
            if e.code == 409:
                raise Lost()
            if e.code in (400, 401, 403, 404):
                raise RuntimeError(f"{path}: HTTP {e.code}")
            err = e
        except Exception as e:  # tarmoq
            err = e
        time.sleep(3 * (attempt + 1))
    raise RuntimeError(f"{path}: {err}")


def api(path, body):
    return _post(f"/api/tr/{path}", body)


def claim(runner):
    try:
        r = api("claim", {"runner": runner})
    except Exception as e:  # noqa: BLE001
        print("tr/claim:", e, flush=True)
        return None
    return r if r.get("job") and r.get("channel") else None


def translate_texts(texts, source):
    """Matnlar ro'yxati -> o'zbekcha ro'yxat. Xato bo'lgan element bo'sh satr (asl matn qoladi)."""
    out = []
    for i in range(0, len(texts), CHUNK):
        part = texts[i:i + CHUNK]
        res = None
        for attempt in range(3):
            try:
                r = _post("/api/encode/translate", {"texts": part, "source": source, "target": "uzbek"},
                          timeout=180, tries=2)
                got = r.get("results")
                if isinstance(got, list) and len(got) == len(part):
                    res = [str(x or "") for x in got]
                    break
            except Exception as e:  # noqa: BLE001
                log(f"  tarjima {attempt + 1}/3 muvaffaqiyatsiz: {e}")
            time.sleep(5 * (attempt + 1))
        out.extend(res if res is not None else [""] * len(part))
    return out


# ── Jonli holat ──────────────────────────────────────────────

def hms(sec):
    sec = max(0, int(sec))
    h, m, s = sec // 3600, sec % 3600 // 60, sec % 60
    return f"{h}:{m:02}:{s:02}" if h else f"{m:02}:{s:02}"


def bar(pct, width=12):
    full = int(round(max(0.0, min(100.0, pct)) / 100 * width))
    return "▓" * full + "░" * (width - full)


class Live:
    """Botdagi jonli holat xabari (`/api/tr/progress`). Har 10 soniyada bittadan ko'p yuborilmaydi."""

    def __init__(self, r):
        job = r["job"]
        self.base = {"id": int(job["id"]), "title": str(job.get("title") or ""), "chat": int(job["chat"]),
                     "attempt": int(job.get("attempt") or 1), "status_msg": int(r.get("status_msg") or 0)}
        self.steps = []
        self.logs = collections.deque(maxlen=5)   # oxirgi log qatorlari (botdagi holat xabarida ko'rinadi)
        self.last = 0.0
        self.t0 = time.time()

    def done(self, line):
        self.steps.append("✅ " + line)

    def note(self, line):
        """Holat xabaridagi oxirgi log qatori (Actions logiga ham yoziladi)."""
        log(" ", line)
        self.logs.append(time.strftime("%H:%M:%S ") + line)

    def send(self, line, force=False):
        now = time.time()
        if not self.base["status_msg"] or (not force and now - self.last < 10):
            return
        self.last = now
        tail = ["", "📜 log:"] + list(self.logs) if self.logs else []
        text = "\n".join(self.steps + [line] + tail + ["", f"⏱ jami: {hms(now - self.t0)}"])
        try:
            api("progress", {**self.base, "text": text})
        except Exception as e:  # noqa: BLE001
            print("tr/progress:", e, flush=True)

    def pages(self, what, i, n):
        self.send(f"{what}\n{bar(i * 100 / max(n, 1))} {i}/{n}")


# ── Fayllarni yig'ish ────────────────────────────────────────

def natural_key(s):
    return [int(t) if t.isdigit() else t.lower() for t in re.split(r"(\d+)", str(s))]


def _pymupdf():
    try:
        import pymupdf
        return pymupdf
    except ImportError:
        import fitz
        return fitz


def expand_file(path: Path, kind: str, name: str, dst: Path, start: int) -> list:
    """Bitta yuklangan fayl -> rasm yo'llari (tabiiy tartibda). `start` — umumiy raqam boshi."""
    out = []
    if kind == "img":
        p = dst / f"{start:05d}{Path(name).suffix.lower() if Path(name).suffix.lower() in IMG_EXT else '.jpg'}"
        shutil.move(str(path), p)
        out.append(p)
    elif kind == "zip":
        with zipfile.ZipFile(path) as z:
            names = sorted((n for n in z.namelist()
                            if not n.endswith("/") and Path(n).suffix.lower() in IMG_EXT
                            and "__MACOSX" not in n and not Path(n).name.startswith(".")), key=natural_key)
            for k, n in enumerate(names):
                p = dst / f"{start + k:05d}{Path(n).suffix.lower()}"
                p.write_bytes(z.read(n))
                out.append(p)
    elif kind == "pdf":
        pm = _pymupdf()
        doc = pm.open(str(path))
        for k, page in enumerate(doc):
            pix = page.get_pixmap(dpi=PDF_DPI)
            p = dst / f"{start + k:05d}.png"
            pix.save(str(p))
            out.append(p)
        doc.close()
    return out


async def collect_images(app, channel, pages, live):
    """Kanal nusxalarini yuklab, bitta rasmlar ro'yxatiga yoyadi."""
    raw = WORK / "raw"
    imgs = WORK / "imgs"
    raw.mkdir(parents=True, exist_ok=True)
    imgs.mkdir(parents=True, exist_ok=True)
    ids = [int(p["m"]) for p in pages]
    msgs = []
    for i in range(0, len(ids), 100):
        msgs.extend(await app.get_messages(channel, ids[i:i + 100]))
    out = []
    for i, (p, m) in enumerate(zip(pages, msgs), 1):
        if not m or m.empty or not (m.photo or m.document):
            raise Fatal(f"{i}-fayl kanalda topilmadi")
        live.pages("⬇️ Fayllar yuklab olinmoqda", i, len(pages))
        got = await app.download_media(m, file_name=str(raw / f"{i:05d}.bin"))
        if not got or Path(got).stat().st_size == 0:
            raise RuntimeError(f"{i}-fayl yuklab olinmadi")
        out.extend(expand_file(Path(got), p.get("k") or "img", str(p.get("n") or ""), imgs, len(out) + 1))
    if not out:
        raise Fatal("fayllar ichida rasm topilmadi")
    return out


# ── OCR ──────────────────────────────────────────────────────

# Tilni avtomatik aniqlash: namuna sahifalarni navbat bilan ko'rsatilgan o'qigichlar bilan o'qiymiz va natijadagi
# yozuv turi (hangul / kana / ieroglif / kirill) yetarli ulushni tashkil qilsa shu til tanlanadi. Hech biri
# bo'lmasa — inglizcha. (kamida `need` ta belgi va umumiy belgilarning `ratio` ulushi)
DETECT = [
    ("korean", lambda c: "\uac00" <= c <= "\ud7a3" or "\u1100" <= c <= "\u11ff", 6, 0.5),
    ("japanese", lambda c: "\u3040" <= c <= "\u30ff", 6, 0.25),
    ("chinese", lambda c: "\u4e00" <= c <= "\u9fff", 6, 0.5),
    ("russian", lambda c: "\u0400" <= c <= "\u04ff", 6, 0.5),
]
LANG_UZ = {"korean": "Koreyscha", "japanese": "Yaponcha", "chinese": "Xitoycha",
           "english": "Inglizcha", "russian": "Ruscha"}


def pick_samples(paths, k=3):
    """Muqova va oxirgi sahifalarni chetlab, o'rtadan teng oraliqda k ta namuna."""
    n = len(paths)
    if n <= k:
        return list(paths)
    return [paths[int(n * (i + 1) / (k + 1))] for i in range(k)]


def script_stats(text, pred):
    chars = [c for c in text if not c.isspace()]
    hit = sum(1 for c in chars if pred(c))
    return hit, hit / max(1, len(chars))


def detect_source(paths, note=lambda s: None):
    """Namuna sahifalardan manba tilini aniqlaydi -> (til, o'qigich)."""
    import numpy as np
    from PIL import Image
    samples = []
    for p in pick_samples(paths):
        img = np.array(Image.open(p).convert("RGB"))
        samples.append(img if img.shape[0] <= TILE_H else img[:TILE_H])   # uzun lentaning boshi yetadi
    for lang, pred, need, ratio in DETECT:
        reader = make_reader(lang)
        text = ""
        for img in samples:
            for _, t, conf in reader.readtext(img, detail=1, paragraph=False, canvas_size=OCR_CANVAS, batch_size=8):
                if conf >= MIN_CONF:
                    text += t
        hit, r = script_stats(text, pred)
        note(f"{LANG_UZ[lang]} sinovi: {hit} belgi ({r:.0%})")
        if hit >= need and r >= ratio:
            return lang, reader
        if lang == "korean":
            # Birinchi o'qigich (ko+en) lotin harflarini ham o'qiydi: sahifa inglizcha bo'lsa, boshqa
            # o'qigichlarni yuklab o'tirmaymiz (eng sekin yo'l shu edi).
            lhit, lr = script_stats(text, lambda c: c.isascii() and c.isalpha())
            if lhit >= 15 and lr >= 0.7:
                note(f"Inglizcha belgilar: {lhit} ({lr:.0%})")
                return "english", reader
    return "english", make_reader("english")


def make_reader(source):
    import easyocr
    import torch
    torch.set_num_threads(max(1, os.cpu_count() or 1))
    return easyocr.Reader(OCR_LANGS.get(source, ["en"]), gpu=False, verbose=False)


def ocr_image(reader, img):
    """RGB (numpy) rasm -> [{box:(x0,y0,x1,y1), text, conf}]. Uzun lentalar bo'laklab o'qiladi."""
    import numpy as np  # noqa: F401
    h = img.shape[0]
    dets = []
    tiles = [(0, h)] if h <= TILE_H + TILE_OVERLAP else [
        (y, min(y + TILE_H, h)) for y in range(0, h - TILE_OVERLAP, TILE_H - TILE_OVERLAP)]
    for ti, (y0, y1) in enumerate(tiles):
        lo = y0 + (TILE_OVERLAP // 2 if ti > 0 else 0)
        hi = y1 - (TILE_OVERLAP // 2 if ti < len(tiles) - 1 else 0)
        for poly, text, conf in reader.readtext(img[y0:y1], detail=1, paragraph=False, canvas_size=OCR_CANVAS, batch_size=8):
            xs, ys = [p[0] for p in poly], [p[1] for p in poly]
            box = (int(min(xs)), int(min(ys)) + y0, int(max(xs)), int(max(ys)) + y0)
            cy = (box[1] + box[3]) / 2
            if conf >= MIN_CONF and text.strip() and lo <= cy < hi + (1 if ti == len(tiles) - 1 else 0):
                dets.append({"box": box, "text": text.strip(), "conf": float(conf)})
    return dets


def group_dets(dets, source):
    """Yaqin qutilarni bitta pufakchaga birlashtiradi. -> [{box, boxes, text}] (o'qish tartibida)."""
    n = len(dets)
    parent = list(range(n))

    def find(a):
        while parent[a] != a:
            parent[a] = parent[parent[a]]
            a = parent[a]
        return a

    def size(b):
        return max(6, min(b[2] - b[0], b[3] - b[1]))

    for i in range(n):
        a = dets[i]["box"]
        for j in range(i + 1, n):
            b = dets[j]["box"]
            m = 0.9 * (size(a) + size(b)) / 2
            if a[0] - m < b[2] and b[0] - m < a[2] and a[1] - m < b[3] and b[1] - m < a[3]:
                parent[find(i)] = find(j)
    groups = {}
    for i in range(n):
        groups.setdefault(find(i), []).append(dets[i])
    out = []
    vertical = source in ("japanese", "chinese")
    for g in groups.values():
        if vertical and sum(1 for d in g if (d["box"][3] - d["box"][1]) > 1.6 * (d["box"][2] - d["box"][0])) * 2 >= len(g):
            g.sort(key=lambda d: (-(d["box"][0] + d["box"][2]), d["box"][1]))  # ustunlar o'ngdan chapga
            text = "".join(d["text"] for d in g)
        else:
            fs = sorted(size(d["box"]) for d in g)[len(g) // 2]
            g.sort(key=lambda d: (round((d["box"][1] + d["box"][3]) / 2 / max(fs, 1) / 0.8), d["box"][0]))
            text = " ".join(d["text"] for d in g)
        boxes = [d["box"] for d in g]
        union = (min(b[0] for b in boxes), min(b[1] for b in boxes), max(b[2] for b in boxes), max(b[3] for b in boxes))
        letters = sum(1 for c in text if c.isalpha())
        if letters < 2 and len(text) < 4:  # "!", "..", yolg'iz belgilar — tarjima qilinmaydi
            continue
        out.append({"box": union, "boxes": boxes, "text": text})
    out.sort(key=lambda g: (g["box"][1], g["box"][0]))
    return out


# ── Chizish ──────────────────────────────────────────────────

FONT_CANDIDATES = [
    "/usr/share/fonts/truetype/dejavu/DejaVuSansCondensed-Bold.ttf",  # tor — pufakchaga ko'proq sig'adi
    "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
    "/usr/share/fonts/truetype/noto/NotoSans-Bold.ttf",
    "/usr/share/fonts/truetype/liberation/LiberationSans-Bold.ttf",
    "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
]


def font_path():
    for p in FONT_CANDIDATES:
        if Path(p).exists():
            return p
    return None


def _font(size):
    from PIL import ImageFont
    p = font_path()
    return ImageFont.truetype(p, size) if p else ImageFont.load_default()


def wrap_lines(text, font, width):
    """So'zlar bo'yicha qatorlarga bo'ladi; juda uzun so'z belgi bo'yicha."""
    lines = []
    for para in text.split("\n"):
        cur = ""
        for word in para.split():
            trial = f"{cur} {word}".strip()
            if font.getlength(trial) <= width:
                cur = trial
                continue
            if cur:
                lines.append(cur)
                cur = ""
            while font.getlength(word) > width and len(word) > 1:
                k = len(word) - 1
                while k > 1 and font.getlength(word[:k]) > width:
                    k -= 1
                lines.append(word[:k])
                word = word[k:]
            cur = word
        if cur:
            lines.append(cur)
    return lines


def _word_ok(text, font, width):
    """Hech bir so'z qatordan keng emasmi (so'z o'rtasidan uzilmasin)."""
    return all(font.getlength(w) <= width for w in text.split())


def fit_text(text, width, height, max_size):
    """Qutiga sig'adigan eng katta shrift o'lchami: (font, qatorlar, qator balandligi).
    AVVAL so'zlarni UZMAY sig'dirishga harakat qilinadi (kichikroq shriftda); bo'lmasa — oxirgi chora sifatida uzun so'z belgi bo'yicha bo'linadi."""
    width, height = max(width, 12), max(height, 12)
    MIN = 9
    for allow_break in (False, True):
        lo, hi, best = MIN, max(MIN, min(max_size, int(height))), None
        while lo <= hi:
            mid = (lo + hi) // 2
            f = _font(mid)
            lines = wrap_lines(text, f, width)
            lh = int(mid * 1.1)
            fits = bool(lines) and len(lines) * lh <= height and (allow_break or _word_ok(text, f, width))
            if fits:
                best = (f, lines, lh)
                lo = mid + 1
            else:
                hi = mid - 1
        if best is not None:
            return best
    f = _font(MIN)
    return f, wrap_lines(text, f, width), MIN + 1


def _clean_and_draw_rect(img, grp, uz):
    """(Zaxira) pufakcha topilmasa: OCR qutisi atrofini tozalab, o'zbekcha matnni yozadi."""
    import cv2
    import numpy as np
    from PIL import Image, ImageDraw
    H, W = img.shape[:2]
    x0, y0, x1, y1 = grp["box"]
    fs = sorted(max(6, min(b[2] - b[0], b[3] - b[1])) for b in grp["boxes"])[len(grp["boxes"]) // 2]
    pad = max(3, int(fs * 0.25))
    # Atrofdagi (matndan tashqari) piksellar rangi — fon bir xilmi?
    rx0, ry0, rx1, ry1 = max(0, x0 - pad * 2), max(0, y0 - pad * 2), min(W, x1 + pad * 2), min(H, y1 + pad * 2)
    region = img[ry0:ry1, rx0:rx1]
    ring = np.ones(region.shape[:2], bool)
    ring[max(0, y0 - ry0 - 1):y1 - ry0 + 1, max(0, x0 - rx0 - 1):x1 - rx0 + 1] = False
    samples = region[ring] if ring.any() else region.reshape(-1, 3)
    bg = np.median(samples, axis=0)
    flat = float(samples.std(axis=0).mean()) < 30
    mask = np.zeros((H, W), np.uint8)
    for (bx0, by0, bx1, by1) in grp["boxes"]:
        cv2.rectangle(mask, (max(0, bx0 - pad), max(0, by0 - pad)), (min(W - 1, bx1 + pad), min(H - 1, by1 + pad)), 255, -1)
    if flat:
        img[mask > 0] = bg.astype(np.uint8)
        fg = (0, 0, 0) if bg.mean() > 128 else (255, 255, 255)
        stroke = None
    else:
        # Murakkab fon: faqat matn belgilarini inpaint qilamiz.
        gray = cv2.cvtColor(img, cv2.COLOR_RGB2GRAY)
        strokes = np.zeros_like(mask)
        for (bx0, by0, bx1, by1) in grp["boxes"]:
            roi = gray[by0:by1, bx0:bx1]
            if roi.size == 0:
                continue
            _, th = cv2.threshold(roi, 0, 255, cv2.THRESH_BINARY + cv2.THRESH_OTSU)
            if th.mean() > 127:   # matn qora fonda oq emas — kichik qismi matn bo'lishi kerak
                th = 255 - th
            strokes[by0:by1, bx0:bx1] = th
        strokes = cv2.dilate(strokes, np.ones((5, 5), np.uint8))
        img[:] = cv2.inpaint(img, strokes, 3, cv2.INPAINT_TELEA)
        # Yozuv rangi fonga qarab: och fonda qora (oq chiziq bilan), to'q fonda oq (qora chiziq bilan).
        fg, stroke = ((0, 0, 0), (255, 255, 255)) if bg.mean() > 140 else ((255, 255, 255), (0, 0, 0))
    # Yozuv maydoni: matn qutisidan biroz kengroq (o'zbekcha matn uzunroq bo'ladi), rasm ichida.
    bw, bh = x1 - x0, y1 - y0
    ex, ey = int(bw * 0.12) + pad, int(bh * 0.08) + pad
    tx0, ty0, tx1, ty1 = max(0, x0 - ex), max(0, y0 - ey), min(W, x1 + ex), min(H, y1 + ey)
    font, lines, lh = fit_text(uz, tx1 - tx0, ty1 - ty0, max_size=int(fs * 1.5) + 4)
    pil = Image.fromarray(img)
    d = ImageDraw.Draw(pil)
    total = len(lines) * lh
    y = ty0 + max(0, (ty1 - ty0 - total) // 2)
    for ln in lines:
        w = font.getlength(ln)
        x = tx0 + max(0, (tx1 - tx0 - w) / 2)
        if stroke:
            d.text((x, y), ln, font=font, fill=fg, stroke_width=2, stroke_fill=stroke)
        else:
            d.text((x, y), ln, font=font, fill=fg)
        y += lh
    img[:] = np.asarray(pil)


def find_bubble(img, box):
    """Matn qutisi (x0,y0,x1,y1) turgan OQ pufakchani topadi: (to'ldirilgan niqob, matn izlari niqobi, (ox, oy) kesim boshlanishi) yoki None.
    Pufakcha = matn qutisini o'z ichiga olgan eng katta OCH (>215) bog'langan soha; chiqish yo'lagi yo'q (kesim chetiga tegmaydi),
    matn harflari esa shu sohadagi "teshiklar" — ularni topib oq bilan to'ldirsak, pufakcha chizig'iga tegmasdan eski matn o'chadi."""
    import cv2
    import numpy as np
    H, W = img.shape[:2]
    x0, y0, x1, y1 = box
    m = int(3 * max(x1 - x0, y1 - y0) + 60)
    cx0, cy0, cx1, cy1 = max(0, x0 - m), max(0, y0 - m), min(W, x1 + m), min(H, y1 + m)
    crop = img[cy0:cy1, cx0:cx1]
    gray = cv2.cvtColor(crop, cv2.COLOR_RGB2GRAY)
    bright = (gray > 215).astype(np.uint8)
    n, lab = cv2.connectedComponents(bright, connectivity=4)
    roi = lab[y0 - cy0:y1 - cy0, x0 - cx0:x1 - cx0]
    vals = roi[roi > 0]
    if vals.size < 30:
        return None
    lbl = int(np.bincount(vals).argmax())
    comp = lab == lbl
    ys, xs = np.where(comp)
    # Sahifa foni yoki chiqib ketgan soha — pufakcha emas.
    if xs.min() == 0 or ys.min() == 0 or xs.max() == comp.shape[1] - 1 or ys.max() == comp.shape[0] - 1:
        return None
    bw, bh = xs.max() - xs.min(), ys.max() - ys.min()
    if bw * bh > 40 * max(1, (x1 - x0) * (y1 - y0)):
        return None
    # Teshiklarni to'ldirish: tashqaridan (kesim burchagidan) yetib bo'lmaydigan hamma narsa pufakcha.
    inv = (~comp).astype(np.uint8)
    ff = inv.copy()
    mask = np.zeros((inv.shape[0] + 2, inv.shape[1] + 2), np.uint8)
    cv2.floodFill(ff, mask, (0, 0), 2)
    filled = ff != 2
    holes = filled & ~comp
    # Matn qutisining katta qismi pufakcha ichida bo'lishi shart.
    inside = filled[y0 - cy0:y1 - cy0, x0 - cx0:x1 - cx0].mean()
    if inside < 0.85:
        return None
    return filled, holes, (cx0, cy0)


def clean_and_draw(img, grp, uz):
    """RGB (numpy) rasmda bitta pufakchaning eski matnini o'chirib, o'zbekcha matnni PUFAKCHA ICHIGA yozadi (qora, markazda).
    Pufakcha topilmasa (shovqin/rasm ustidagi yozuv) — eski usul."""
    import cv2
    import numpy as np
    from PIL import Image, ImageDraw
    x0, y0, x1, y1 = grp["box"]
    fs = sorted(max(6, min(b[2] - b[0], b[3] - b[1])) for b in grp["boxes"])[len(grp["boxes"]) // 2]
    found = None
    try:
        found = find_bubble(img, (x0, y0, x1, y1))
    except Exception as e:  # noqa: BLE001
        log(f"  pufakcha izlashda xato ({e}) — oddiy usul")
    if found is None:
        return _clean_and_draw_rect(img, grp, uz)
    filled, holes, (ox, oy) = found
    # 1) Eski matn: teshiklar (kengaytirilgan) pufakcha ichida oqqa bo'yaladi.
    er = cv2.dilate(holes.astype(np.uint8), np.ones((3, 3), np.uint8), iterations=2).astype(bool) & filled
    sub = img[oy:oy + er.shape[0], ox:ox + er.shape[1]]
    sub[er] = 255
    # 2) Yozuv maydoni: pufakchaga sig'adigan markazlashgan to'g'ri to'rtburchak (egri pufakchada burchaklar chetda qolmasin).
    ys, xs = np.where(filled)
    mx0, my0, mx1, my1 = xs.min(), ys.min(), xs.max(), ys.max()
    cx, cy = int(xs.mean()), int(ys.mean())
    bw, bh = mx1 - mx0, my1 - my0
    rect = None
    for sc in [x / 100 for x in range(96, 49, -3)]:
        hw, hh = int(bw * sc / 2), int(bh * sc / 2)
        rx0, ry0, rx1, ry1 = max(0, cx - hw), max(0, cy - hh), min(filled.shape[1], cx + hw), min(filled.shape[0], cy + hh)
        if rx1 - rx0 < 10 or ry1 - ry0 < 10:
            break
        if filled[ry0:ry1, rx0:rx1].mean() > 0.985:
            rect = (rx0, ry0, rx1, ry1)
            break
    if rect is None:
        rect = (cx - bw // 4, cy - bh // 4, cx + bw // 4, cy + bh // 4)
    rx0, ry0, rx1, ry1 = rect
    pad = max(3, int(min(rx1 - rx0, ry1 - ry0) * 0.04))
    tw, th = (rx1 - rx0) - 2 * pad, (ry1 - ry0) - 2 * pad
    font, lines, lh = fit_text(uz, tw, th, max_size=int(fs * 1.35) + 4)
    pil = Image.fromarray(img)
    d = ImageDraw.Draw(pil)
    total = len(lines) * lh
    y = oy + ry0 + pad + max(0, (th - total) // 2)
    for ln in lines:
        w = font.getlength(ln)
        x = ox + rx0 + pad + max(0, (tw - w) / 2)
        d.text((x, y), ln, font=font, fill=(0, 0, 0))
        y += lh
    img[:] = np.asarray(pil)


# ── Natija fayli ─────────────────────────────────────────────

def build_output(paths, fmt, title, dst: Path) -> Path:
    """Rasmlar -> bitta fayl (pdf | cbz | zip)."""
    from PIL import Image
    safe = re.sub(r'[\\/:*?"<>|]', "", title).strip()[:60] or "tarjima"
    if fmt == "pdf":
        out = dst / f"{safe}.pdf"
        imgs = [Image.open(p).convert("RGB") for p in paths]
        imgs[0].save(out, "PDF", save_all=True, append_images=imgs[1:], resolution=130.0, quality=88)
        return out
    out = dst / f"{safe}.{'cbz' if fmt == 'cbz' else 'zip'}"
    with zipfile.ZipFile(out, "w", zipfile.ZIP_STORED) as z:
        for i, p in enumerate(paths, 1):
            z.write(p, f"{i:04d}{p.suffix}")
    return out


# ── Ish ──────────────────────────────────────────────────────

def render_all(paths, per_page_groups, uz_by_page, out_dir: Path):
    """Har rasmga tarjimani chizib, JPEG qilib saqlaydi. -> yangi yo'llar."""
    import numpy as np
    from PIL import Image
    outs = []
    for i, p in enumerate(paths):
        img = np.array(Image.open(p).convert("RGB"))
        for grp, uz in zip(per_page_groups[i], uz_by_page[i]):
            if uz and uz.strip():
                try:
                    clean_and_draw(img, grp, uz.strip())
                except Exception as e:  # noqa: BLE001
                    log(f"  {p.name}: pufakchani chizib bo'lmadi ({e})")
        o = out_dir / f"{i + 1:05d}.jpg"
        Image.fromarray(img).save(o, quality=90)
        outs.append(o)
    return outs


# ── Qo'lda tarjima (2026-11): 1-bosqich raqamlash, 2-bosqich foydalanuvchi matnini joylash ──

MARK_RE = re.compile(r"^\s*(\d{1,4})\s*\.\s*(\d{1,4})\s*(?:#|:)?\s*(.*)$")


def parse_manual(text: str) -> dict:
    """Foydalanuvchi yozgan `1.1# matn` qatorlari -> {(sahifa, pufakcha): matn}. Marker bo'lmagan qator oldingisining davomi (ko'p qatorli tarjima)."""
    out, cur = {}, None
    for raw in (text or "").splitlines():
        m = MARK_RE.match(raw)
        if m:
            cur = (int(m.group(1)), int(m.group(2)))
            out[cur] = m.group(3).strip()
        elif cur is not None and raw.strip():
            out[cur] = (out[cur] + " " + raw.strip()).strip()
    return out


def serialize_groups(groups):
    """OCR guruhlari -> JSON (faqat koordinatalar): [[[box, [boxes...]], ...] har sahifa uchun]."""
    return [[[list(map(int, g["box"])), [list(map(int, b)) for b in g["boxes"]]] for g in page] for page in groups]


def load_groups(js: str):
    pages = json.loads(js)
    return [[{"box": tuple(b), "boxes": [tuple(x) for x in bs], "text": ""} for b, bs in page] for page in pages]


def annotate_pages(paths, groups, out_dir: Path):
    """Har sahifada pufakchalarni qizil ramka va `sahifa.raqam#` yorlig'i bilan belgilab, JPEG saqlaydi."""
    from PIL import Image, ImageDraw
    outs = []
    for i, p in enumerate(paths):
        im = Image.open(p).convert("RGB")
        d = ImageDraw.Draw(im)
        W = im.size[0]
        fsz = max(16, min(48, W // 28))
        font = _font(fsz)
        for j, g in enumerate(groups[i], 1):
            x0, y0, x1, y1 = g["box"]
            d.rectangle([x0, y0, x1, y1], outline=(230, 30, 30), width=max(2, W // 400))
            lab = f"{i + 1}.{j}#"
            tw = int(font.getlength(lab)) + 8
            ty = max(0, y0 - fsz - 6)
            d.rectangle([x0, ty, x0 + tw, ty + fsz + 6], fill=(230, 30, 30))
            d.text((x0 + 4, ty + 2), lab, font=font, fill=(255, 255, 255))
        o = out_dir / f"{i + 1:05d}.jpg"
        im.save(o, quality=88)
        outs.append(o)
    return outs


async def send_result(app, chat, final: Path, caption: str, jid: int, live):
    """Faylni suhbatga yuboradi: asosiy akkaunt, bo'lmasa R2 orqali panel bot."""
    sent, send_err = False, None
    try:
        await app.send_document(chat, str(final), caption=caption[:1024], file_name=final.name, force_document=True)
        sent = True
    except Exception as ex:  # noqa: BLE001
        send_err = ex
        log(f"  asosiy akkaunt yubora olmadi ({type(ex).__name__}: {ex}) — R2 orqali yuboriladi")
    if not sent:
        via_r2 = False
        if r2client is not None:
            dest = {"mode": "document", "to": chat, "caption": caption[:1024], "file_name": final.name,
                    "lease": {"t": "tr_jobs", "id": jid}, "fallback_bot": True}
            via_r2 = await asyncio.to_thread(
                r2client.deliver, os.environ["API_BASE"], os.environ["ENCODE_TOKEN"], final,
                f"tr_{jid}_{int(time.time())}{final.suffix}", dest, log, "arumediatv-translate")
        if not via_r2:
            raise send_err
        live.done("Asosiy akkaunt yubora olmadi — R2'ga yozildi, panel bot yetkazadi")


async def process(app, r, runner):
    channel = int(r["channel"])
    job = r["job"]
    jid, title, source = int(job["id"]), str(job.get("title") or "Tarjima"), str(job.get("src") or "korean")
    fmt = str(job.get("fmt") or "pdf")
    chat = int(job["chat"])
    live = Live(r)
    ident = {"runner": runner, "id": jid, "status_msg": live.base["status_msg"]}
    shutil.rmtree(WORK, ignore_errors=True)
    WORK.mkdir(parents=True)
    try:
        log(f"\U0001F310 Tarjima #{jid}: {title} ({source}, {len(job['pages'])} fayl, urinish {job.get('attempt')})")
        live.send("⬇️ Fayllar yuklab olinmoqda...", force=True)
        paths = await collect_images(app, channel, job["pages"], live)
        live.done(f"Yuklab olindi: {len(paths)} ta rasm")
        mode = str(job.get("mode") or "")

        if mode == "render":
            # 2-bosqich: pufakchalar 1-bosqichda saqlangan; foydalanuvchi yozgan `sahifa.raqam#` matnlari joylanadi (OCR/tarjima kerak emas).
            groups = load_groups(job["boxes"])
            if len(groups) != len(paths):
                raise Fatal(f"sahifalar soni mos emas ({len(groups)} != {len(paths)}) — ishni o'chirib, qaytadan yuboring")
            man = parse_manual(job.get("manual"))
            uz_pages = [[man.get((i + 1, j + 1), "") for j in range(len(page))] for i, page in enumerate(groups)]
            used = sum(1 for page in uz_pages for t in page if t.strip())
            unknown = [f"{a}.{b}" for (a, b) in man if not (1 <= a <= len(groups) and 1 <= b <= len(groups[a - 1]))]
            live.done(f"Tarjima: {used} ta pufakcha" + (f"; mavjud bo'lmagan raqamlar: {', '.join(unknown[:15])}" if unknown else ""))
            live.send("\U0001F3A8 Rasmlarga yozilmoqda...", force=True)
            out_dir = WORK / "out"
            out_dir.mkdir()
            outs = await asyncio.to_thread(render_all, paths, groups, uz_pages, out_dir)
            final = await asyncio.to_thread(build_output, outs, fmt, title, WORK)
            size = final.stat().st_size
            live.done(f"Fayl tayyor: {size / 1048576:.1f} MB ({len(outs)} sahifa)")
            api("check", ident)
            live.send("⬆️ Yuborilmoqda...", force=True)
            await send_result(app, chat, final, f"\U0001F310 {title}", jid, live)
            api("finish", {**ident, "ok": True, "size": size, "pages": len(outs)})
            log(f"  ✅ qo'lda tarjima #{jid} yuborildi ({used} pufakcha)")
            return

        # OCR (og'ir ish — alohida oqimda, Telegram ulanishi uzilmasin).
        def work_ocr():
            import numpy as np
            from PIL import Image
            nonlocal source
            if source == "auto":
                live.send("🔎 Til aniqlanmoqda...", force=True)
                source, reader = detect_source(paths, live.note)
                live.note(f"Til aniqlandi: {LANG_UZ.get(source, source)}")
                live.done(f"Til aniqlandi: {LANG_UZ.get(source, source)}")
            else:
                reader = make_reader(source)
            res = []
            t0 = time.time()
            for i, p in enumerate(paths, 1):
                img = np.array(Image.open(p).convert("RGB"))
                res.append(group_dets(ocr_image(reader, img), source))
                live.note(f"{i}-sahifa: {len(res[-1])} blok")
                live.send(f"\U0001F50D Matn o'qilmoqda\n{bar(i * 100 / len(paths))} {i}/{len(paths)} "
                          f"· o'tdi {hms(time.time() - t0)}")
            return res
        live.send("\U0001F50D Matn o'qilmoqda...", force=True)
        groups = await asyncio.to_thread(work_ocr)
        flat = [g["text"] for page in groups for g in page]
        log(f"  OCR tayyor: {len(flat)} ta matn bloki")
        live.done(f"Matn topildi: {len(flat)} ta blok")

        if mode == "detect":
            # 1-bosqich: pufakchalarni `sahifa.raqam#` bilan raqamlab, belgilangan fayl va asl matnlar ro'yxatini yuboramiz; ish tarjima kutadi.
            live.send("\U0001F3F7 Pufakchalar raqamlanmoqda...", force=True)
            out_dir = WORK / "anno"
            out_dir.mkdir()
            anno = await asyncio.to_thread(annotate_pages, paths, groups, out_dir)
            final = await asyncio.to_thread(build_output, anno, "pdf", f"{title}_raqamlangan", WORK)
            texts = [f"{i + 1}.{j}# {g['text']}" for i, page in enumerate(groups) for j, g in enumerate(page, 1)]
            api("check", ident)
            live.send("⬆️ Yuborilmoqda...", force=True)
            await send_result(app, chat, final, f"\u270D {title} — pufakchalar raqamlandi ({len(texts)} ta)", jid, live)
            api("finish", {**ident, "wait": True, "src": source, "boxes": json.dumps(serialize_groups(groups), separators=(",", ":")),
                           "pages": len(paths), "blocks": len(texts), "texts": texts})
            log(f"  ✍ #{jid}: {len(texts)} pufakcha raqamlandi, tarjima kutilmoqda")
            return

        live.send(f"\U0001F310 Tarjima qilinmoqda ({len(flat)} blok)...", force=True)
        uz_flat = await asyncio.to_thread(translate_texts, flat, source) if flat else []
        bad = sum(1 for x in uz_flat if not x.strip())
        if flat and bad == len(flat):
            raise RuntimeError("tarjima xizmati javob bermadi (Workers AI)")
        live.done(f"Tarjima qilindi" + (f" ({bad} blok tarjimasiz qoldi)" if bad else ""))
        it = iter(uz_flat)
        uz_pages = [[next(it) for _ in page] for page in groups]

        live.send("\U0001F3A8 Rasmlarga yozilmoqda...", force=True)
        out_dir = WORK / "out"
        out_dir.mkdir()
        outs = await asyncio.to_thread(render_all, paths, groups, uz_pages, out_dir)
        final = await asyncio.to_thread(build_output, outs, fmt, title, WORK)
        size = final.stat().st_size
        live.done(f"Fayl tayyor: {size / 1048576:.1f} MB ({len(outs)} sahifa)")
        log(f"  tayyor: {final.name} {size / 1048576:.1f} MB")

        api("check", ident)   # admin shu orada o'chirgan bo'lsa — yuborilmaydi
        live.send("⬆️ Yuborilmoqda...", force=True)
        caption = f"\U0001F310 {title}"[:1024]
        await send_result(app, chat, final, caption, jid, live)
        api("finish", {**ident, "ok": True, "size": size, "pages": len(outs)})
        log(f"  ✅ tarjima #{jid} yuborildi")
    except Lost:
        log(f"  tarjima #{jid} o'chirildi yoki boshqa run'ga o'tdi — yuborilmadi")
    except Fatal as ex:
        log(f"  tarjima #{jid} XATO (qayta urinilmaydi): {ex}")
        try:
            api("finish", {**ident, "ok": False, "fatal": True, "error": str(ex)})
        except Exception:  # noqa: BLE001
            pass
    except Exception as ex:  # noqa: BLE001
        log(f"  tarjima #{jid} XATO: {type(ex).__name__}: {ex}")
        try:
            api("finish", {**ident, "ok": False, "error": f"{type(ex).__name__}: {ex}"[:300]})
        except Exception:  # noqa: BLE001
            pass
    finally:
        shutil.rmtree(WORK, ignore_errors=True)


SESSION = str(HERE / "pyro_session")
RUNNER = f"{os.environ.get('GITHUB_RUN_ID', 'local')}-{os.environ.get('GITHUB_RUN_ATTEMPT', '1')}"
START_BUDGET = int(os.environ.get("START_BUDGET_MIN", "300")) * 60
T0 = time.time()
CURRENT = None


def session_api_id():
    try:
        import sqlite3
        c = sqlite3.connect(f"file:{SESSION}.session?mode=ro", uri=True)
        return int(c.execute("SELECT api_id FROM sessions").fetchone()[0] or 0)
    except Exception:  # noqa: BLE001
        return 0


def on_cancel(signum, frame):
    """Run bekor qilindi — ish navbatga qaytadi."""
    if CURRENT:
        try:
            api("finish", {"runner": RUNNER, "id": CURRENT, "ok": False, "cancelled": True})
        except Exception:  # noqa: BLE001
            pass
        print("Run bekor qilindi — ish navbatga qaytarildi", flush=True)
    os._exit(0)


async def main():
    global CURRENT
    import pyrogram.utils
    from pyrogram import Client
    pyrogram.utils.MIN_CHANNEL_ID = -1009999999999
    api_id = session_api_id() or int(os.environ["TG_API_ID"])
    app = Client(SESSION, api_id=api_id, api_hash=os.environ["TG_API_HASH"], no_updates=True)
    done = 0
    async with app:
        async for _ in app.get_dialogs():  # kanal va suhbat peer keshida bo'lsin
            pass
        while time.time() - T0 < START_BUDGET:
            r = claim(RUNNER)
            if not r:
                break
            CURRENT = int(r["job"]["id"])
            try:
                await process(app, r, RUNNER)
                done += 1
            finally:
                CURRENT = None
            # Bitta bob (ish) uchun bitta run: tugagach o'chadi (toza mashina, bo'sh disk va xotira).
            # Navbatda yana ish bo'lsa — workflow'ning "Davom ettirish" qadami yangi run ochadi.
            break
    log(f"Run tugadi: {done} ta ish.")
    out = os.environ.get("GITHUB_OUTPUT")
    if out and done:
        with open(out, "a") as f:
            f.write("worked=1\n")


if __name__ == "__main__":
    import signal
    signal.signal(signal.SIGINT, on_cancel)
    signal.signal(signal.SIGTERM, on_cancel)
    asyncio.run(main())
    sys.exit(0)
