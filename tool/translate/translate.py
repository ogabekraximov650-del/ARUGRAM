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
        self.last = 0.0
        self.t0 = time.time()

    def done(self, line):
        self.steps.append("✅ " + line)

    def send(self, line, force=False):
        now = time.time()
        if not self.base["status_msg"] or (not force and now - self.last < 10):
            return
        self.last = now
        text = "\n".join(self.steps + [line, "", f"⏱ jami: {hms(now - self.t0)}"])
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


def fit_text(text, width, height, max_size):
    """Qutiga sig'adigan eng katta shrift o'lchami: (font, qatorlar, qator balandligi)."""
    width, height = max(width, 12), max(height, 12)
    lo, hi, best = 8, max(8, min(max_size, int(height))), None
    while lo <= hi:
        mid = (lo + hi) // 2
        f = _font(mid)
        lines = wrap_lines(text, f, width)
        lh = int(mid * 1.12)
        if lines and len(lines) * lh <= height:
            best = (f, lines, lh)
            lo = mid + 1
        else:
            hi = mid - 1
    if best is None:
        f = _font(8)
        return f, wrap_lines(text, f, width), 9
    return best


def clean_and_draw(img, grp, uz):
    """RGB (numpy) rasmda bitta pufakchaning eski matnini o'chirib, o'zbekcha matnni yozadi."""
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
        fg, stroke = (255, 255, 255), (0, 0, 0)
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

        # OCR (og'ir ish — alohida oqimda, Telegram ulanishi uzilmasin).
        def work_ocr():
            import numpy as np
            from PIL import Image
            reader = make_reader(source)
            res = []
            t0 = time.time()
            for i, p in enumerate(paths, 1):
                img = np.array(Image.open(p).convert("RGB"))
                res.append(group_dets(ocr_image(reader, img), source))
                live.send(f"\U0001F50D Matn o'qilmoqda\n{bar(i * 100 / len(paths))} {i}/{len(paths)} "
                          f"· o'tdi {hms(time.time() - t0)}")
            return res
        live.send("\U0001F50D Matn o'qilmoqda...", force=True)
        groups = await asyncio.to_thread(work_ocr)
        flat = [g["text"] for page in groups for g in page]
        log(f"  OCR tayyor: {len(flat)} ta matn bloki")
        live.done(f"Matn topildi: {len(flat)} ta blok")

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
        sent = False
        send_err = None
        try:
            await app.send_document(chat, str(final), caption=caption, file_name=final.name, force_document=True)
            sent = True
        except Exception as ex:  # noqa: BLE001
            send_err = ex
            log(f"  asosiy akkaunt yubora olmadi ({type(ex).__name__}: {ex}) — R2 orqali yuboriladi")
        if not sent:
            via_r2 = False
            if r2client is not None:
                dest = {"mode": "document", "to": chat, "caption": caption, "file_name": final.name,
                        "lease": {"t": "tr_jobs", "id": jid}, "fallback_bot": True}
                via_r2 = await asyncio.to_thread(
                    r2client.deliver, os.environ["API_BASE"], os.environ["ENCODE_TOKEN"], final,
                    f"tr_{jid}_{int(time.time())}{final.suffix}", dest, log, "arumediatv-translate")
            if not via_r2:
                raise send_err
            live.done("Asosiy akkaunt yubora olmadi — R2'ga yozildi, panel bot yetkazadi")
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
    log(f"Run tugadi: {done} ta ish.")


if __name__ == "__main__":
    import signal
    signal.signal(signal.SIGINT, on_cancel)
    signal.signal(signal.SIGTERM, on_cancel)
    asyncio.run(main())
    sys.exit(0)
