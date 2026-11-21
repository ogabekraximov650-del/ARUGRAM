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
import functools
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


GLOSSARY = {"Gate": "Darvoza", "Hunter": "Ovchi", "Hunters": "Ovchilar", "Dungeon": "Zindon", "Quest": "Topshiriq", "Quests": "Topshiriqlar",
            "Rank": "Daraja", "Level up": "Daraja oshdi", "Raid": "Reyd", "Boss": "Boss"}
_TIDY = {"\u2010": "-", "\u2011": "-", "\u2012": "-", "\u2013": "-", "\u2014": "-", "\u02bb": "'", "\u02bc": "'", "\u2018": "'", "\u2019": "'",
         "\u201c": '"', "\u201d": '"', "\u00a0": " ", "\u200b": "", "\u2060": "", "\u00ad": ""}


def tidy_uz(t: str) -> str:
    """Tarjima matnidagi shriftda yo'q (kvadrat bo'lib chiqadigan) belgilarni oddiylariga almashtiradi: defis variantlari, tipografik tirnoq/apostrof."""
    for a, b in _TIDY.items():
        t = t.replace(a, b)
    for v in set(GLOSSARY.values()):   # lug'at so'zlari gap o'rtasida kichik harf bilan ("zindon", "topshiriq")
        t = re.sub(r"(?<![.!?]\s)(?<!^)\b" + re.escape(v) + r"(?=\W|$)", lambda m: m.group(0).lower(), t)
    for i, c in enumerate(t):   # gap boshidagi birinchi harf katta bo'lsin ("[sizning kunlik..." -> "[Sizning kunlik...")
        if c.isalpha():
            t = t[:i] + c.upper() + t[i + 1:]
            break
    return t


def translate_texts(texts, source):
    """Matnlar ro'yxati -> o'zbekcha ro'yxat. Xato bo'lgan element bo'sh satr (asl matn qoladi)."""
    def norm(t):   # OCR katta harfli/aralash yozuvni gap shakliga keltirish (model aniqroq tarjima qiladi)
        t = (t or "").strip()
        letters = [c for c in t if c.isalpha()]
        if letters and sum(1 for c in letters if c.isupper()) / len(letters) > 0.25:
            t = t.lower()
            t = re.sub(r"\bi\b", "I", t)
            t = t[:1].upper() + t[1:]
        return t
    texts = [norm(t) for t in texts]
    out = []
    for i in range(0, len(texts), CHUNK):
        part = texts[i:i + CHUNK]
        res = None
        for attempt in range(3):
            try:
                r = _post("/api/encode/translate", {"texts": part, "source": source, "target": "uzbek", "glossary": GLOSSARY},
                          timeout=180, tries=2)
                got = r.get("results")
                if isinstance(got, list) and len(got) == len(part):
                    res = [tidy_uz(str(x or "").replace('\\"', '"').replace("\\", "")).strip() for x in got]   # JSON qochirish belgilari (\") tushib qolmasin
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
    owners = []   # har rasm qaysi yuborilgan faylga (pages indeksi) tegishli
    for i, (p, m) in enumerate(zip(pages, msgs), 1):
        if not m or m.empty or not (m.photo or m.document):
            raise Fatal(f"{i}-fayl kanalda topilmadi")
        live.pages("⬇️ Fayllar yuklab olinmoqda", i, len(pages))
        got = await app.download_media(m, file_name=str(raw / f"{i:05d}.bin"))
        if not got or Path(got).stat().st_size == 0:
            raise RuntimeError(f"{i}-fayl yuklab olinmadi")
        part = expand_file(Path(got), p.get("k") or "img", str(p.get("n") or ""), imgs, len(out) + 1)
        out.extend(part)
        owners.extend([i - 1] * len(part))
    if not out:
        raise Fatal("fayllar ichida rasm topilmadi")
    return out, owners


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
        out.append({"box": union, "boxes": boxes, "text": text, "conf": sum(d["conf"] for d in g) / len(g)})
    out.sort(key=lambda g: (g["box"][1], g["box"][0]))
    return out


def reread_box(img, box, reader) -> str:
    """Bitta qutini bir necha usulda qayta o'qiydi (birinchi mazmunli natija olinadi): 2x kattalashtirish; 3x kulrang+kontrast; Otsu ikkilik; qora-oq inversiya.
    Qiyshiq (kursiv) yoki xira shriftdagi pufakcha matni uchun."""
    import cv2
    import numpy as np
    H, W = img.shape[:2]
    crop = img[max(0, box[1] - 8):min(H, box[3] + 8), max(0, box[0] - 8):min(W, box[2] + 8)]
    if crop.size == 0:
        return ""
    gray = cv2.cvtColor(crop, cv2.COLOR_RGB2GRAY)
    big2 = cv2.resize(crop, None, fx=2.0, fy=2.0, interpolation=cv2.INTER_CUBIC)
    big3 = cv2.resize(gray, None, fx=3.0, fy=3.0, interpolation=cv2.INTER_CUBIC)
    clahe = cv2.createCLAHE(clipLimit=2.5, tileGridSize=(4, 4)).apply(big3)
    _, otsu = cv2.threshold(big3, 0, 255, cv2.THRESH_BINARY + cv2.THRESH_OTSU)
    tries = [big2, clahe, otsu, 255 - otsu, cv2.copyMakeBorder(otsu, 30, 30, 30, 30, cv2.BORDER_CONSTANT, value=255)]
    for im in tries:
        for conf in (0.2, 0.08):
            try:
                res = reader.readtext(im, detail=1, paragraph=False)
            except Exception as e:  # noqa: BLE001
                log(f"  qayta OCR xato ({e})")
                break
            t = " ".join(t for _, t, c in sorted(res, key=lambda r: (r[0][0][1], r[0][0][0])) if c >= conf).strip()
            if len(re.sub(r"[^A-Za-z\u0400-\u04ff\u3040-\u30ff\u4e00-\u9fff\uac00-\ud7a3]", "", t)) >= 3:
                return t
    return ""


def find_missed(img, groups, reader=None):
    """OCR topolmagan matnli pufakcha/ramkalarni sun'iy intellektsiz topadi: sahifadagi OCH (>215) bog'langan sohalar orasidan pufakchaga o'xshaganlari
    (yopiq, to'ldirilganligi yuqori), ichida matnga o'xshash mayda "teshiklar" (harflar) bor, lekin mavjud OCR qutilari bilan qoplanmaganlari.
    Topilganlar guruh sifatida qaytadi (`reader` berilsa kesim kattalashtirib OCR qilinadi; o'qilmasa matn bo'sh)."""
    import cv2
    import numpy as np
    H, W = img.shape[:2]
    gray = cv2.cvtColor(img, cv2.COLOR_RGB2GRAY)
    bright = (gray > 215).astype(np.uint8)
    n, lab, stats, _ = cv2.connectedComponentsWithStats(bright, connectivity=4)
    covered = [g["box"] for g in groups]

    def cov(bx):
        a = max(1, (bx[2] - bx[0]) * (bx[3] - bx[1]))
        tot = 0
        for c in covered:
            iw, ih = min(bx[2], c[2]) - max(bx[0], c[0]), min(bx[3], c[3]) - max(bx[1], c[1])
            if iw > 0 and ih > 0:
                tot += iw * ih
        return tot / a
    out = []
    for i in range(1, n):
        x, y, w, h, area = stats[i]
        if area < 1500 or w < 40 or h < 30 or w > 0.7 * W or h > 0.7 * H or x == 0 or y == 0 or x + w >= W or y + h >= H:
            continue
        if area / float(w * h) < 0.45:   # pufakcha ellips/to'rtburchak — to'liq
            continue
        comp = (lab[y:y + h, x:x + w] == i)
        inv = (~comp).astype(np.uint8)
        ff = np.pad(inv, 1, constant_values=1)
        mask = np.zeros((ff.shape[0] + 2, ff.shape[1] + 2), np.uint8)
        cv2.floodFill(ff, mask, (0, 0), 2)
        holes = (ff[1:-1, 1:-1] != 2) & ~comp
        if holes.sum() < 0.004 * w * h:
            continue
        hn, hl, hs, _ = cv2.connectedComponentsWithStats(holes.astype(np.uint8), connectivity=8)
        boxes = []
        for j in range(1, hn):
            hx, hy, hw, hh, ha = hs[j]
            # Harfga o'xshash: mayda, juda cho'zinchoq emas, pufakchaning katta qismi emas.
            if 6 <= hh <= 0.35 * h and 2 <= hw <= 0.9 * w and ha >= 6:
                boxes.append((x + hx, y + hy, x + hx + hw, y + hy + hh))
        if len(boxes) < 3:
            continue
        union = (min(b[0] for b in boxes), min(b[1] for b in boxes), max(b[2] for b in boxes), max(b[3] for b in boxes))
        if (union[2] - union[0]) * (union[3] - union[1]) < 0.03 * w * h:
            continue
        if cov(union) >= 0.4:
            continue
        text = reread_box(img, union, reader) if reader is not None else ""
        out.append({"box": union, "boxes": boxes, "text": text})
        covered.append(union)
    return out


# ── Chizish ──────────────────────────────────────────────────

FONT_CANDIDATES = [
    "/usr/share/fonts/opentype/comic-neue/ComicNeue-Bold.otf",         # komiks uslubi (apt: fonts-comic-neue, OFL)
    "/usr/share/fonts/truetype/comic-neue/ComicNeue-Bold.ttf",
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


@functools.lru_cache(maxsize=256)
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


def _med1d(a, k):
    """Chekka qatoridagi keskin sakrashlarni (harf piksellari) tekislaydi: N x 3 massivda k kenglikdagi sirpanuvchi mediana."""
    import numpy as np
    n = a.shape[0]
    if n < 3 or k < 3:
        return a
    k = min(k | 1, n if n % 2 else n - 1)
    h = k // 2
    pad = np.pad(a, ((h, h), (0, 0)), mode="edge")
    win = np.stack([pad[i:i + n] for i in range(k)], axis=0)
    return np.median(win, axis=0)


def smooth_fill(img, x0, y0, x1, y1, t=4):
    """To'rtburchak (x0,y0)-(x1,y1) ni uning CHEKKALARIDAN silliq to'ldiradi (Coons yamog'i: to'rt chekka qatordan bilinear interpolyatsiya).
    Chekka qatorlari mediana bilan tozalanadi (harf chekkasiga tushgan piksellar yo'l-yo'l dog' qoldirmasin). Gradient yoki tekis fonda eski yozuv qutisi
    izsiz yo'qoladi (Telea katta to'rtburchakda ko'rinadigan dog' qoldirardi)."""
    import numpy as np
    H, W = img.shape[:2]
    x0, y0, x1, y1 = max(t, x0), max(t, y0), min(W - 1 - t, x1), min(H - 1 - t, y1)
    if x1 - x0 < 2 or y1 - y0 < 2:
        return False
    a = img.astype(np.float32)
    kx, ky = max(9, (x1 - x0) // 6), max(9, (y1 - y0) // 4)
    top = _med1d(np.median(a[y0 - t:y0, x0:x1 + 1], axis=0), kx)
    bot = _med1d(np.median(a[y1 + 1:y1 + 1 + t, x0:x1 + 1], axis=0), kx)
    left = _med1d(np.median(a[y0:y1 + 1, x0 - t:x0], axis=1), ky)
    right = _med1d(np.median(a[y0:y1 + 1, x1 + 1:x1 + 1 + t], axis=1), ky)
    h, w = y1 - y0 + 1, x1 - x0 + 1
    u = np.linspace(0, 1, w, dtype=np.float32)[None, :, None]
    v = np.linspace(0, 1, h, dtype=np.float32)[:, None, None]
    TL, TR = (top[0] + left[0]) / 2, (top[-1] + right[0]) / 2
    BL, BR = (bot[0] + left[-1]) / 2, (bot[-1] + right[-1]) / 2
    Lc = (1 - u) * left[:, None, :] + u * right[:, None, :]
    Tc = (1 - v) * top[None, :, :] + v * bot[None, :, :]
    bil = (1 - u) * (1 - v) * TL + u * (1 - v) * TR + (1 - u) * v * BL + u * v * BR
    img[y0:y1 + 1, x0:x1 + 1] = np.clip(Lc + Tc - bil, 0, 255).astype(np.uint8)
    return True


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
    gray_r = cv2.cvtColor(np.ascontiguousarray(region), cv2.COLOR_RGB2GRAY)
    lap = np.abs(cv2.Laplacian(gray_r, cv2.CV_32F))
    ring_lap = float(lap[ring].mean()) if ring.any() else 99.0
    mask = np.zeros((H, W), np.uint8)
    for (bx0, by0, bx1, by1) in grp["boxes"]:
        cv2.rectangle(mask, (max(0, bx0 - pad), max(0, by0 - pad)), (min(W - 1, bx1 + pad), min(H - 1, by1 + pad)), 255, -1)
    stroke = None
    rp = region[ring] if ring.any() else region.reshape(-1, 3)
    near = float((np.abs(rp.astype(np.int16) - bg.astype(np.int16)).max(axis=1) <= 14).mean())
    if near >= 0.85:
        # Tekis fon (oq/bir xil rang): quti fon rangi bilan to'ldiriladi (chekkadagi harf piksellari interpolyatsiyani buzmasin — "xira dog'" bo'lmasin).
        img[mask > 0] = bg.astype(np.uint8)
    elif ring_lap < 4.0:
        # Silliq fon (gradient): har bir quti chekkalaridan silliq to'ldiriladi.
        for (bx0, by0, bx1, by1) in grp["boxes"]:
            smooth_fill(img, bx0 - pad - 3, by0 - pad - 3, bx1 + pad + 3, by1 + pad + 3)
    else:
        # Murakkab fon: matn belgilari FON FARQI bo'yicha topiladi (har pikselning atrofdagi mediana rangidan farqi) — rangli, yaltiroq yozuvlar ham;
        # keyin davriy fon (screentone) usuli yoki Telea bilan tiklanadi.
        k = max(15, min(71, int(fs * 2.5) // 2 * 2 + 1))
        mx = k // 2 + 2
        ex0, ey0, ex1, ey1 = max(0, rx0 - mx), max(0, ry0 - mx), min(W, rx1 + mx), min(H, ry1 + mx)
        big = np.ascontiguousarray(img[ey0:ey1, ex0:ex1])
        med = cv2.medianBlur(big, k)
        diff = np.abs(big.astype(np.int16) - med.astype(np.int16)).max(axis=2)
        strokes = np.zeros_like(mask)
        strokes[ey0:ey1, ex0:ex1] = (diff > 24).astype(np.uint8)
        strokes = cv2.bitwise_and(strokes, mask)
        strokes = cv2.dilate(strokes, np.ones((5, 5), np.uint8), iterations=1)
        inpaint_mask(img, strokes > 0)
    # Yozuv rangi: tiklangan joyning yorqinligiga qarab (och fonda qora, to'qda oq); murakkab fonda qarama-qarshi chiziq bilan.
    after = img[mask > 0]
    lum = float(np.median(after.mean(axis=1))) if after.size else float(bg.mean())
    fg = (0, 0, 0) if lum > 140 else (255, 255, 255)
    if ring_lap >= 4.0:
        stroke = (255, 255, 255) if lum > 140 else (0, 0, 0)
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


def _balanced_lines(words, font, avails):
    """So'zlarni `len(avails)` qatorga shunday bo'ladiki, har qator o'z kengligiga (avails[i]) sig'sin va qatorlar bir tekis to'lsin
    (yig'indi (bo'sh joy)^2 eng kam). Imkoni bo'lmasa None."""
    n, m = len(avails), len(words)
    if m == 0 or n == 0 or m < n:
        return None
    INF = float("inf")
    sp = font.getlength(" ")
    wl = [font.getlength(w) for w in words]
    pre = [0.0]
    for w in wl:
        pre.append(pre[-1] + w)

    def seg(a, b):  # words[a:b] kengligi
        return pre[b] - pre[a] + sp * (b - a - 1)
    dp = [[INF] * (m + 1) for _ in range(n + 1)]
    bk = [[0] * (m + 1) for _ in range(n + 1)]
    dp[0][0] = 0.0
    for i in range(1, n + 1):
        for j in range(i, m + 1):
            for a in range(i - 1, j):
                if dp[i - 1][a] == INF:
                    continue
                w = seg(a, j)
                if w > avails[i - 1]:
                    continue
                c = dp[i - 1][a] + (avails[i - 1] - w) ** 2
                if c < dp[i][j]:
                    dp[i][j], bk[i][j] = c, a
    if dp[n][m] == INF:
        return None
    out, j = [], m
    for i in range(n, 0, -1):
        a = bk[i][j]
        out.append(" ".join(words[a:j]))
        j = a
    return out[::-1]


def layout_in_bubble(uz, filled, cx, cy, pad, max_size, min_size=9):
    """Matnni pufakcha SHAKLIGA moslab joylaydi: har qator o'z balandligidagi haqiqiy bo'sh kenglikda (chiziqdan pad masofada), markazi shu
    qatordagi pufakcha markazida. Qaytadi: (font, [(qator, markaz_x, y)]) yoki None. So'zlar hech qachon uzilmaydi."""
    import numpy as np
    words = uz.split()
    if not words:
        return None
    ys = np.where(filled.any(axis=1))[0]
    top_all, bot_all = int(ys.min()) + pad, int(ys.max()) - pad
    Hh, Ww = filled.shape
    L = np.zeros(Hh, int)
    R = np.zeros(Hh, int)
    for y in range(Hh):
        row = filled[y]
        if not row[cx]:
            continue
        l = cx
        while l > 0 and row[l - 1]:
            l -= 1
        r = cx
        while r < Ww - 1 and row[r + 1]:
            r += 1
        L[y], R[y] = l, r

    def span(y0, y1):
        y0, y1 = max(0, y0), min(Hh - 1, y1)
        if y1 < y0:
            return 0, 0
        ls, rs = L[y0:y1 + 1], R[y0:y1 + 1]
        if (rs <= ls).any():
            return 0, 0
        return int(ls.max()) + pad, int(rs.min()) - pad

    for size in range(max(min_size, min(max_size, 90)), min_size - 1, -1):
        f = _font(size)
        lh = int(size * 1.12)
        if any(f.getlength(w) > (R[cy] - L[cy]) - 2 * pad for w in words):
            continue
        # Boshlang'ich taxmin: qatorlar soni to'rtburchak kengligi bo'yicha, keyin pufakcha shakli bilan aniqlanadi.
        n = max(1, int(sum(f.getlength(w) + f.getlength(" ") for w in words) // max(1, (R[cy] - L[cy]) - 2 * pad)) + 1)
        for _ in range(6):
            if n * lh > bot_all - top_all:
                break
            top = max(top_all, min(cy - n * lh // 2, bot_all - n * lh))
            spans = [span(top + i * lh, top + (i + 1) * lh) for i in range(n)]
            avails = [r - l for l, r in spans]
            if any(a <= 0 for a in avails):
                n += 1
                continue
            lines = _balanced_lines(words, f, avails)
            if lines is not None:
                placed = []
                for i, ln in enumerate(lines):
                    l, r = spans[i]
                    placed.append((ln, (l + r) / 2.0, top + i * lh))
                return f, placed, lh
            n += 1
    return None


SFX_WORDS = {"gong", "swoosh", "whoosh", "beep", "stab", "klang", "klong", "clang", "thud", "boom", "bang", "crash", "haah", "huff", "tring", "ding", "slash",
             "crack", "thump", "splat", "bam", "pow", "zzz", "cooing", "kreak", "creak", "bzzt", "thwack", "slam", "swish", "whack", "pant", "gasp"}
SHORT_OK = {"no", "ok", "oh", "ah", "uh", "um", "hm", "so", "go", "me", "we", "he", "it", "is", "to", "do", "be", "by", "on", "at", "in", "of", "or",
            "an", "as", "if", "my", "up", "us", "am", "hi", "a", "i"}
JUNK_WORDS = (".com", "scanlat", "feedback is", "editor", "quality check", "cleaner", "raw provider", "www.", "http", "discord", "patreon", "facebook", "twitter", "instagram", "scanlation", "scans", "translator", "proofreader",
              "typesetter", "redrawer", "credit:", "join our", "read on", "@gmail", "support us", "recruit", "staff")
OCR_CONF = {frozenset(x) for x in (("h", "k"), ("u", "l"), ("u", "n"), ("l", "i"), ("o", "0"), ("c", "e"), ("e", "o"), ("a", "o"), ("r", "p"), ("t", "l"),
                                   ("m", "n"), ("n", "h"), ("d", "o"), ("g", "6"), ("s", "5"), ("b", "6"), ("b", "h"), ("v", "y"), ("u", "v"), ("a", "4"),
                                   ("i", "1"), ("l", "1"), ("f", "t"), ("c", "o"), ("r", "n"), ("q", "g"))}
OCR_FIX = {"slng": "sung", "sune": "sung", "sng": "sung", "tke": "the", "tbe": "the", "tne": "the", "whal": "what", "wnat": "what", "bul": "but", "yau": "you", "yov": "you"}
_SPELL = None


def keep_group(g) -> bool:
    """Tarjima qilishga arziydigan matnmi: kamida 2 ta harf, belgilar ko'p emas, ishonch yetarli, reklama/kredit emas."""
    t = (g.get("text") or "").strip()
    if not t:
        return True   # bo'sh (OCR o'qimagan) pufakcha — foydalanuvchi rasmga qarab yozadi
    letters = sum(1 for c in t if c.isalpha())
    if letters < 2:
        return False
    words = [w.lower() for w in re.findall(r"[A-Za-z]+", t)]
    if words and all(w in SFX_WORDS for w in words):
        return False   # ovoz effekti (BEEP, GONG...) — tarjima qilinmaydi
    if letters <= 2 and "".join(words) not in SHORT_OK:
        return False   # 1-2 harfli shovqin (Yz, Yo)
    if sum(1 for c in t if c.isdigit()) / max(1, len(t)) > 0.35:
        return False
    sym = sum(1 for c in t if not (c.isalnum() or c.isspace() or c in ".,!?'\"-:;()~…"))
    if sym / max(1, len(t)) > 0.2:
        return False
    if g.get("conf", 1.0) < 0.30:
        return False
    low = t.lower()
    return not any(w in low for w in JUNK_WORDS)


def _ocr_candidates(low):
    """`low` so'zidan OCR chalkashligi (OCR_CONF) bo'yicha 1-3 ta harfni almashtirib hosil bo'lgan LUG'ATDAGI so'zlar."""
    import itertools
    alts = {}
    for i, ch in enumerate(low):
        a = [o for pair in OCR_CONF for o in pair if ch in pair and o != ch]
        if a:
            alts[i] = sorted(set(a))
    out = set()
    pos = list(alts)
    for k in (1, 2, 3):
        for comb in itertools.combinations(pos, k):
            for rep in itertools.product(*(alts[i] for i in comb)):
                w = list(low)
                for i, r in zip(comb, rep):
                    w[i] = r
                w = "".join(w)
                if w in _SPELL:
                    out.add(w)
    return out


def fix_ocr_words(text: str) -> str:
    """Inglizcha OCR xatolarini lug'at bo'yicha tuzatadi (sun'iy intellektsiz): faqat OCR HARF CHALKASHLIGIGA o'xshash almashtirishlar (H/K, U/L, P/R...)
    bilan: KLNTER -> HUNTER, SOppy -> SORRY. So'z lug'atda bo'lmasa yoki juda kam uchrasa (soppy) va nomzod ancha ko'p uchrasa almashtiriladi.
    5+ harfli, defis/apostrofga yopishmagan so'zlar (ismlar buzilmasin) va kichik `OCR_FIX` jadvali (TkE -> THE, SLNG -> SUNG)."""
    global _SPELL
    import re as _re
    try:
        if _SPELL is None:
            from spellchecker import SpellChecker
            _SPELL = SpellChecker(distance=1)
    except Exception:  # noqa: BLE001
        return text

    def freq(w):
        try:
            return _SPELL.word_usage_frequency(w)
        except Exception:  # noqa: BLE001
            return _SPELL[w]

    def fix(m):
        w = m.group(0)
        pre, post = text[max(0, m.start() - 1):m.start()], text[m.end():m.end() + 1]
        low = w.lower()
        if low in OCR_FIX:
            new = OCR_FIX[low]
        elif len(w) >= 5 and pre not in ("-", "'") and post not in ("-", "'"):   # 4 harfli so'zlar (HAAH, HUFF) faqat `OCR_FIX` jadvali bilan
            f0 = freq(low) if low in _SPELL else 0.0
            best, new = -1.0, None
            for cand in _ocr_candidates(low):
                f1 = freq(cand)
                if f1 > best:
                    best, new = f1, cand
            if not new or (f0 > 0 and best < 300 * f0):
                return w
        else:
            return w
        if w.isupper():
            return new.upper()
        if w[:1].isupper():
            return new.capitalize()
        return new
    return _re.sub(r"[A-Za-z]+", fix, text)


def postprocess_groups(img, groups, reader, source):
    """OCR guruhlarini yakunlaydi: OCR topmagan pufakchalarni qo'shadi, keraksizlarini (reklama, kredit, shovqin) olib tashlaydi, inglizcha OCR xatolarini tuzatadi."""
    try:
        extra = find_missed(img, groups, reader)
    except Exception as e:  # noqa: BLE001
        log(f"  qo'shimcha pufakcha izlashda xato ({e})")
        extra = []
    out = []
    for g in sorted(groups + extra, key=lambda x: (x["box"][1], x["box"][0])):
        if not (g.get("text") or "").strip() and reader is not None:
            try:
                g = dict(g, text=reread_box(img, g["box"], reader))
            except Exception as e:  # noqa: BLE001
                log(f"  bo'sh blokni qayta o'qishda xato ({e})")
        if g.get("text"):
            g = dict(g, text=re.sub(r"\s{2,}", " ", re.sub(r"[@#$%^&*{}<>|\\_=+`]", "", g["text"])).strip())
        if source == "english" and g.get("text"):
            g = dict(g, text=fix_ocr_words(g["text"]))
        if not keep_group(g):   # tuzatishdan KEYIN tekshiriladi (masalan "Coong" -> "Cooing" ovoz effekti)
            continue
        out.append(g)
    return out


def find_bubble(img, box):
    """Matn qutisi (x0,y0,x1,y1) turgan BIR TEKIS FONLI pufakcha/ramkani topadi (oq pufakcha, rangli yoki qora narrator ramkasi):
    (to'ldirilgan niqob, matn izlari niqobi, (ox, oy) kesim boshlanishi, fon rangi RGB, fon to'qmi) yoki None.
    Fon = matn qutisidagi eng ko'p uchraydigan yorqinlik; pufakcha = shu fonga yaqin eng katta bog'langan soha (chiqish yo'lagi yo'q — kesim
    chetiga tegmaydi). Matn harflari shu sohadagi "teshiklar": ularni fon rangi bilan to'ldirsak, chiziqqa tegmasdan eski matn o'chadi."""
    import cv2
    import numpy as np
    H, W = img.shape[:2]
    x0, y0, x1, y1 = box
    m = int(3 * max(x1 - x0, y1 - y0) + 60)
    cx0, cy0, cx1, cy1 = max(0, x0 - m), max(0, y0 - m), min(W, x1 + m), min(H, y1 + m)
    crop = img[cy0:cy1, cx0:cx1]
    gray = cv2.cvtColor(crop, cv2.COLOR_RGB2GRAY)
    ry0, ry1, rx0, rx1 = y0 - cy0, y1 - cy0, x0 - cx0, x1 - cx0
    sub = gray[ry0:ry1, rx0:rx1]
    if sub.size < 30:
        return None
    hist = np.bincount((sub // 8).ravel(), minlength=32)
    mode = int(hist.argmax())
    bg = float(np.median(sub[(sub // 8) == mode]))
    if bg >= 215:
        sel = gray > 215                    # oq/och fon (toza oq pufakcha)
    else:
        sel = np.abs(gray.astype(np.int16) - int(bg)) <= 14   # tusli yoki qora ramka
    n, lab = cv2.connectedComponents(sel.astype(np.uint8), connectivity=4)
    roi = lab[ry0:ry1, rx0:rx1]
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
    if filled[ry0:ry1, rx0:rx1].mean() < 0.85:
        return None
    bgc = tuple(int(v) for v in np.median(crop[comp], axis=0))
    return filled, holes, (cx0, cy0), bgc, bg < 110


def inpaint_texture(crop, mask):
    """Takrorlanuvchi fon (yarim ton nuqtalari/to'r — "screentone") uchun sun'iy intellektsiz tiklash: atrofdagi fon avtokorrelyatsiyasidan DAVR
    (dx,dy) topiladi, niqoblangan piksellar davr masofasidagi qo'shni (niqobsiz) piksellardan nusxalanadi. Davr topilmasa None.
    crop: RGB uint8, mask: bool (True — tiklanadigan)."""
    import numpy as np
    h, w = mask.shape
    if h < 24 or w < 24 or mask.mean() > 0.5:
        return None
    g = crop.astype(np.float32).mean(axis=2)
    mean = g[~mask].mean()
    g0 = np.where(mask, mean, g) - mean
    F = np.fft.rfft2(g0)
    ac = np.fft.irfft2(F * np.conj(F), s=g0.shape)
    ac /= max(ac[0, 0], 1e-6)
    cands = []
    for dy in range(0, min(h // 2, 31)):
        for dx in range(-min(w // 2, 31), min(w // 2, 31) + 1):
            if dy == 0 and dx <= 0:
                continue
            if abs(dx) + dy < 3:
                continue
            v = ac[dy % h, dx % w]
            if v > 0.35:
                cands.append((float(v), dx, dy))
    if not cands:
        return None
    cands.sort(reverse=True)
    lags = [(cands[0][1], cands[0][2])]
    for v, dx, dy in cands[1:]:   # birinchisiga parallel bo'lmagan ikkinchi yo'nalish
        d0 = lags[0]
        if abs(dx * d0[1] - dy * d0[0]) > 2 and len(lags) < 2:
            lags.append((dx, dy))
    out = crop.copy()
    remaining = mask.copy()
    known = ~mask
    ys, xs = np.mgrid[0:h, 0:w]
    for k in range(1, 7):
        for (dx, dy) in lags:
            for sgn in (1, -1):
                sx, sy = xs + sgn * k * dx, ys + sgn * k * dy
                ok = remaining & (sx >= 0) & (sx < w) & (sy >= 0) & (sy < h)
                src_ok = np.zeros_like(ok)
                src_ok[ok] = known[sy[ok], sx[ok]]
                sel = ok & src_ok
                if sel.any():
                    out[sel] = out[sy[sel], sx[sel]]   # AVVAL tiklanganlardan ham nusxa olinadi (asl `crop` da hali yozuv piksellari bor!)
                    remaining &= ~sel
                    known = known | sel
        if not remaining.any():
            break
    return out, remaining


def inpaint_mask(img, mask):
    """Niqob (butun rasm o'lchamida bool) ostidagi pikselni tiklaydi: avval davriy fon usuli, qolganini OpenCV Telea bilan. `img` joyida o'zgaradi."""
    import cv2
    import numpy as np
    ys, xs = np.where(mask)
    if ys.size == 0:
        return
    H, W = mask.shape
    m = 36
    y0, y1, x0, x1 = max(0, ys.min() - m), min(H, ys.max() + m + 1), max(0, xs.min() - m), min(W, xs.max() + m + 1)
    crop = img[y0:y1, x0:x1].copy()
    cm = mask[y0:y1, x0:x1]
    res = None
    left = cm
    try:
        r = inpaint_texture(crop, cm)
        if r is not None:
            res, left = r
    except Exception as e:  # noqa: BLE001
        log(f"  davriy tiklash xatosi ({e}) — Telea")
    base = res if res is not None else crop
    if left.any():
        base = cv2.inpaint(base, (left.astype(np.uint8) * 255), 3, cv2.INPAINT_TELEA)
    img[y0:y1, x0:x1][cm] = base[cm]


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
    filled, holes, (ox, oy), bgc, dark_bg = found
    # 1) Eski matn: teshiklar (kengaytirilgan) pufakcha ichida FON RANGI bilan bo'yaladi.
    er = cv2.dilate(holes.astype(np.uint8), np.ones((3, 3), np.uint8), iterations=2).astype(bool) & filled
    sub = img[oy:oy + er.shape[0], ox:ox + er.shape[1]]
    # Yozuv atrofidagi yarim shaffof "soya" (antialias) ham o'chadi: fonga yaqin bo'lmagan, lekin pufakcha ichidagi piksellar.
    lum = 0.299 * bgc[0] + 0.587 * bgc[1] + 0.114 * bgc[2]
    g_sub = cv2.cvtColor(np.ascontiguousarray(sub), cv2.COLOR_RGB2GRAY).astype(np.float32)
    halo = filled & (np.abs(g_sub - lum) > 14)
    er = er | cv2.dilate(halo.astype(np.uint8), np.ones((3, 3), np.uint8), iterations=1).astype(bool) & filled
    # Pufakcha ichi bir tekis (oq) bo'lsa — matn atrofi to'liq fon rangiga bo'yaladi (JPEG shovqini/"ruh" izlari qolmasin).
    rest = filled & ~er
    if rest.any() and float(g_sub[rest].std()) < 9.0:
        bxm = np.zeros(filled.shape, bool)
        m0 = max(4, int(0.15 * max(x1 - x0, y1 - y0)))
        bxm[max(0, y0 - oy - m0):y1 - oy + m0, max(0, x0 - ox - m0):x1 - ox + m0] = True
        er = er | (bxm & filled)
    sub[er] = bgc
    ink = (255, 255, 255) if dark_bg else (0, 0, 0)
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
    pad = max(4, int(min(rx1 - rx0, ry1 - ry0) * 0.09))  # chiziqqa yopishmasin
    max_font = int(fs * 1.35) + 4
    pil = Image.fromarray(img)
    d = ImageDraw.Draw(pil)
    lay = None
    try:
        lay = layout_in_bubble(uz, filled, cx, cy, max(3, pad // 2), max_font)
    except Exception as e:  # noqa: BLE001
        log(f"  pufakcha shakliga joylab bo'lmadi ({e}) — to'rtburchak")
    if lay is not None:
        font, placed, lh = lay
        for ln, xc, y in placed:
            d.text((ox + xc - font.getlength(ln) / 2, oy + y), ln, font=font, fill=ink)
    else:
        tw, th = (rx1 - rx0) - 2 * pad, (ry1 - ry0) - 2 * pad
        font, lines, lh = fit_text(uz, tw, th, max_size=max_font)
        total = len(lines) * lh
        y = oy + ry0 + pad + max(0, (th - total) // 2)
        for ln in lines:
            w = font.getlength(ln)
            x = ox + rx0 + pad + max(0, (tw - w) / 2)
            d.text((x, y), ln, font=font, fill=ink)
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

def merge_same_bubble(img, groups, uz):
    """Bir XIL pufakcha/ramka ichida bir necha OCR guruhi bo'lsa (masalan tizim oynasining ikki qatori), ularning tarjimasi BIRLASHTIRILIB bir marta
    chiziladi — aks holda matnlar bir-birining ustiga tushadi. Qaytadi: (guruhlar, tarjimalar)."""
    out_g, out_u, seen = [], [], {}
    for g, u in zip(groups, uz):
        key = None
        if u and u.strip():
            try:
                f = find_bubble(img, g["box"])
            except Exception:  # noqa: BLE001
                f = None
            if f is not None:
                import numpy as np
                ys, xs = np.where(f[0])
                key = (int(xs.min()) + f[2][0], int(ys.min()) + f[2][1], int(xs.max()) + f[2][0], int(ys.max()) + f[2][1], int(f[0].sum()))
        if key is not None and key in seen:
            i = seen[key]
            a = out_g[i]
            out_g[i] = {"box": (min(a["box"][0], g["box"][0]), min(a["box"][1], g["box"][1]), max(a["box"][2], g["box"][2]), max(a["box"][3], g["box"][3])),
                        "boxes": list(a["boxes"]) + list(g["boxes"]), "text": a.get("text", "")}
            out_u[i] = (out_u[i].strip() + " " + u.strip()).strip()
            continue
        if key is not None:
            seen[key] = len(out_g)
        out_g.append(g)
        out_u.append(u)
    # 2-o'tish: pufakcha topilmasa ham YAQIN qo'shni ikki qator (bir xil ramka/abzats) birlashtiriladi: x bo'yicha ustma-ust, oraliq ≤ 1.6 qator
    # balandligi va oraliq TEKIS (orada chiziq/rasm yo'q) bo'lsa. Aks holda matnlar bir-birining ustiga tushadi.
    import numpy as np
    order = sorted(range(len(out_g)), key=lambda i: out_g[i]["box"][1])
    merged = {}
    gray = img.mean(axis=2) if img.ndim == 3 else img
    for k in range(len(order) - 1):
        i, j = order[k], order[k + 1]
        i = merged.get(i, i)
        a, b = out_g[i]["box"], out_g[j]["box"]
        if not (out_u[i] and out_u[i].strip() and out_u[j] and out_u[j].strip()) or i == j:
            continue
        ox = min(a[2], b[2]) - max(a[0], b[0])
        gap = b[1] - a[3]
        hmin = max(1, min(a[3] - a[1], b[3] - b[1]))
        if ox >= 0.5 * max(1, min(a[2] - a[0], b[2] - b[0])) and 0 <= gap <= 1.6 * hmin:
            xs0, xs1 = max(a[0], b[0]), min(a[2], b[2])
            strip = gray[a[3]:b[1] + 1, xs0:xs1] if gap > 1 else None
            if strip is None or strip.size == 0 or float(strip.std()) < 14:
                ga = out_g[i]
                out_g[i] = {"box": (min(a[0], b[0]), min(a[1], b[1]), max(a[2], b[2]), max(a[3], b[3])),
                            "boxes": list(ga["boxes"]) + list(out_g[j]["boxes"]), "text": ga.get("text", "")}
                out_u[i] = (out_u[i].strip() + " " + out_u[j].strip()).strip()
                merged[j] = i
                out_u[j] = ""
    keep = [t for t in range(len(out_g)) if t not in merged]
    return [out_g[t] for t in keep], [out_u[t] for t in keep]


MIT_DIR = os.environ.get("MIT_DIR", "")
MIT_PY = os.environ.get("MIT_PY", "")


def mit_clean(paths, work: Path) -> dict:
    """Sahifalarni ALOHIDA dastur — zyddnys/manga-image-translator (GPL-3.0; faqat ishga tushiriladi, kodi bizga qo'shilmaydi) — bilan TOZALAYDI:
    matn topish + LaMa inpaint, `--prep-manual` (tarjimasiz, matni olib tashlangan bo'sh sahifalar). Qaytadi: {sahifa yo'li: tozalangan rasm yo'li}.
    `MIT_DIR`/`MIT_PY` berilmagan yoki xato bo'lsa bo'sh lug'at (o'zimizning tozalash usuli ishlaydi)."""
    if not (MIT_DIR and MIT_PY and Path(MIT_PY).exists() and Path(MIT_DIR).exists()):
        return {}
    try:
        inp, out = work / "in", work / "out"
        shutil.rmtree(work, ignore_errors=True)
        inp.mkdir(parents=True)
        out.mkdir(parents=True)
        for p in paths:
            shutil.copy(p, inp / f"{p.stem}{p.suffix}")
        cfg = work / "cfg.json"
        cfg.write_text(json.dumps({
            "translator": {"translator": "original", "target_lang": "ENG", "no_text_lang_skip": True},
            "detector": {"detector": "default", "detection_size": 2048},
            "inpainter": {"inpainter": "lama_large", "inpainting_size": 2048}}))
        r = subprocess.run([MIT_PY, "-m", "manga_translator", "local", "-i", str(inp), "-o", str(out), "--prep-manual", "-f", "png",
                            "--config-file", str(cfg)], cwd=MIT_DIR, capture_output=True, text=True, timeout=3 * 3600)
        log("  MIT tozalash:", r.returncode, (r.stdout or r.stderr or "")[-200:].replace("\n", " | "))
        res = {}
        for p in paths:
            hits = [f for f in out.glob(f"{p.stem}.*") if "-orig" not in f.name]
            if hits:
                res[p] = hits[0]
        return res
    except Exception as e:  # noqa: BLE001
        log(f"  MIT tozalash xatosi ({type(e).__name__}: {e}) — o'zimizning usul")
        return {}


def render_all(paths, per_page_groups, uz_by_page, out_dir: Path, cleaned=None):
    """Har rasmga tarjimani chizib, JPEG qilib saqlaydi. `cleaned` — `mit_clean` natijasi: tarjima qilinadigan pufakchalar atrofi shu tozalangan
    rasmdan olinadi (qolgan joylar o'zgarmaydi), keyin yangi matn chiziladi. -> yangi yo'llar."""
    import numpy as np
    from PIL import Image
    outs = []
    cleaned = cleaned or {}
    for i, p in enumerate(paths):
        img = np.array(Image.open(p).convert("RGB"))
        cl = None
        if p in cleaned:
            try:
                cl = np.array(Image.open(cleaned[p]).convert("RGB"))
                if cl.shape != img.shape:
                    cl = None
            except Exception:  # noqa: BLE001
                cl = None
        groups_i, uz_i = merge_same_bubble(img, per_page_groups[i], uz_by_page[i])
        def _norm(t):
            return "".join(c for c in (t or "").lower() if c.isalnum())
        for grp, uz in zip(groups_i, uz_i):
            if uz and uz.strip() and _norm(uz) != _norm(grp.get("text")):   # tarjima asl bilan bir xil (ovoz effekti, nom) — tegilmaydi
                try:
                    if cl is not None:
                        H, W = img.shape[:2]
                        m = 10
                        x0, y0, x1, y1 = grp["box"]
                        x0, y0, x1, y1 = max(0, x0 - m), max(0, y0 - m), min(W, x1 + m), min(H, y1 + m)
                        img[y0:y1, x0:x1] = cl[y0:y1, x0:x1]
                    clean_and_draw(img, grp, tidy_uz(uz.strip()))
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


def build_each(outs, owners, pages, dst: Path) -> list:
    """'Alohida-alohida': har yuborilgan fayl uchun o'zi kabi tur: rasm -> rasm, PDF -> PDF, ZIP/CBZ -> ZIP/CBZ.
    Qaytadi: [(fayl yo'li, nom)]. Tarjima qilinmagan (rasmsiz) fayllar o'tkaziladi."""
    from collections import OrderedDict
    groups = OrderedDict()
    for path, o in zip(outs, owners):
        groups.setdefault(o, []).append(path)
    dst.mkdir(parents=True, exist_ok=True)
    res, used = [], set()
    for o, files in groups.items():
        meta = pages[o] if o < len(pages) else {}
        name = str(meta.get("n") or "")
        kind = meta.get("k") or "img"
        stem = re.sub(r'[\\/:*?"<>|]', "", Path(name).stem).strip()[:50] or f"fayl{o + 1}"
        if stem in used:
            stem = f"{stem}_{o + 1}"
        used.add(stem)
        if kind == "pdf":
            f = build_output(files, "pdf", f"{stem}_uz", dst)
        elif kind == "zip":
            f = build_output(files, "cbz" if name.lower().endswith(".cbz") else "zip", f"{stem}_uz", dst)
        else:
            f = dst / f"{stem}_uz{files[0].suffix}"
            shutil.copy(files[0], f)
        res.append((f, f.name))
    return res


async def deliver(app, chat, outs, owners, job, fmt, title, jid, live):
    """Tayyor sahifalarni yuboradi: 'each' — har fayl alohida, aks holda bitta fayl. Qaytadi: (jami bayt, fayllar soni)."""
    if str(job.get("out") or "all") == "each":
        items = await asyncio.to_thread(build_each, outs, owners, job["pages"], WORK / "each")
        total = 0
        for k, (f, nm) in enumerate(items, 1):
            api("check", {"runner": job["_runner"], "id": jid})
            live.send(f"⬆️ Yuborilmoqda {k}/{len(items)}...", force=True)
            await send_result(app, chat, f, f"\U0001F310 {title} · {k}/{len(items)}" if len(items) > 1 else f"\U0001F310 {title}", jid, live)
            total += f.stat().st_size
        live.done(f"Fayllar tayyor: {len(items)} ta, {total / 1048576:.1f} MB ({len(outs)} sahifa)")
        return total, len(items)
    final = await asyncio.to_thread(build_output, outs, fmt, title, WORK)
    size = final.stat().st_size
    live.done(f"Fayl tayyor: {size / 1048576:.1f} MB ({len(outs)} sahifa)")
    api("check", {"runner": job["_runner"], "id": jid})
    live.send("⬆️ Yuborilmoqda...", force=True)
    await send_result(app, chat, final, f"\U0001F310 {title}"[:1024], jid, live)
    return size, 1


async def process(app, r, runner):
    channel = int(r["channel"])
    job = r["job"]
    jid, title, source = int(job["id"]), str(job.get("title") or "Tarjima"), str(job.get("src") or "korean")
    fmt = str(job.get("fmt") or "pdf")
    chat = int(job["chat"])
    job["_runner"] = runner
    live = Live(r)
    ident = {"runner": runner, "id": jid, "status_msg": live.base["status_msg"]}
    shutil.rmtree(WORK, ignore_errors=True)
    WORK.mkdir(parents=True)
    try:
        log(f"\U0001F310 Tarjima #{jid}: {title} ({source}, {len(job['pages'])} fayl, urinish {job.get('attempt')})")
        live.send("⬇️ Fayllar yuklab olinmoqda...", force=True)
        paths, owners = await collect_images(app, channel, job["pages"], live)
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
            cleaned = await asyncio.to_thread(mit_clean, paths, WORK / "mit")
            outs = await asyncio.to_thread(render_all, paths, groups, uz_pages, out_dir, cleaned)
            size, _n = await deliver(app, chat, outs, owners, job, fmt, title, jid, live)
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
                grp_i = group_dets(ocr_image(reader, img), source)
                n0 = len(grp_i)
                grp_i = postprocess_groups(img, grp_i, reader, source)
                if len(grp_i) != n0:
                    live.note(f"{i}-sahifa: {n0} blok -> {len(grp_i)} (qo'shimcha topildi / keraksizlari olib tashlandi)")
                res.append(grp_i)
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
            unread = "(o'qilmadi — rasmga qarang)"
            texts = [f"{i + 1}.{j}# {g['text'] or unread}" for i, page in enumerate(groups) for j, g in enumerate(page, 1)]
            api("check", ident)
            live.send("⬆️ Yuborilmoqda...", force=True)
            await send_result(app, chat, final, f"\u270D {title} — pufakchalar raqamlandi ({len(texts)} ta)", jid, live)
            api("finish", {**ident, "wait": True, "src": source, "boxes": json.dumps(serialize_groups(groups), separators=(",", ":")),
                           "pages": len(paths), "blocks": len(texts), "texts": texts})
            log(f"  ✍ #{jid}: {len(texts)} pufakcha raqamlandi, tarjima kutilmoqda")
            return

        live.send(f"\U0001F310 Tarjima qilinmoqda ({len(flat)} blok)...", force=True)
        uz_flat = await asyncio.to_thread(translate_texts, flat, source) if flat else []
        uz_flat = [u if t.strip() else "" for t, u in zip(flat, uz_flat)]   # o'qilmagan (bo'sh) pufakchalar tarjimasiz qoladi
        bad = sum(1 for t, x in zip(flat, uz_flat) if t.strip() and not x.strip())
        if flat and bad == sum(1 for t in flat if t.strip()) and bad > 0:
            raise RuntimeError("tarjima xizmati javob bermadi (Workers AI)")
        live.done(f"Tarjima qilindi" + (f" ({bad} blok tarjimasiz qoldi)" if bad else ""))
        it = iter(uz_flat)
        uz_pages = [[next(it) for _ in page] for page in groups]

        live.send("\U0001F3A8 Rasmlarga yozilmoqda...", force=True)
        out_dir = WORK / "out"
        out_dir.mkdir()
        cleaned = await asyncio.to_thread(mit_clean, paths, WORK / "mit")
        outs = await asyncio.to_thread(render_all, paths, groups, uz_pages, out_dir, cleaned)
        size, _n = await deliver(app, chat, outs, owners, job, fmt, title, jid, live)   # yuborishdan oldin `check` (admin o'chirgan bo'lsa — yuborilmaydi)
        log(f"  tayyor: {_n} fayl, {size / 1048576:.1f} MB")
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
