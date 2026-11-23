"""ARUMEDIATVRS: HarmonyOS uslubidagi animatsiyalar PROTOTIPI (video).

Ilovaning HAQIQIY ekranlari (`cargo test` snapshotlari, 2x) olinadi va ustida animatsiyalar (prujina fizikasi, bosish effekti,
sahifa o'tishi, umumiy element o'tishi, yorliq va pastki menyu, ro'yxat, dialog) ko'rsatiladi. Bu video ilovaning O'ZIDA hali yo'q harakatlarni
oldindan ko'rsatadi (prototip). Chiqish: MP4 (ffmpeg, libx264), 1080x1920, 30 kadr/s.

Ishlatish: python make_video.py <snapshot_papkasi> <chiqish.mp4>
Tezkor sinov: VIDEO_FAST=1 (har sahna 1.5 s, past sifat).
"""
import math
import os
import subprocess
import sys

import numpy as np
from PIL import Image, ImageDraw, ImageFont

SNAP, OUT = sys.argv[1], sys.argv[2]
FAST = os.environ.get("VIDEO_FAST") == "1"
W, H, FPS = 1080, 1920, 30
HERE = os.path.dirname(os.path.abspath(__file__))
FONTS = os.path.join(HERE, "..", "assets", "fonts")
ACCENT = (226, 98, 15)
BG = (10, 10, 12)


def font(size, bold=True):
    return ImageFont.truetype(os.path.join(FONTS, "Roboto-Bold.ttf" if bold else "Roboto-Regular.ttf"), size)


def load(name):
    return Image.open(os.path.join(SNAP, name)).convert("RGB")


# ── Fizika / egri chiziqlar ──
def spring(t, freq=2.0, zeta=0.55):
    """0 dan 1 gacha prujina (so'nuvchi tebranish): zeta < 1 — biroz oshib qaytadi."""
    if t <= 0:
        return 0.0
    wn = freq * 2 * math.pi
    wd = wn * math.sqrt(1 - zeta * zeta)
    return 1 - math.exp(-zeta * wn * t) * (math.cos(wd * t) + zeta / math.sqrt(1 - zeta * zeta) * math.sin(wd * t))


def ease(t):
    t = max(0.0, min(1.0, t))
    return t * t * (3 - 2 * t)


def clamp01(x):
    return max(0.0, min(1.0, x))


def lerp(a, b, p):
    return a + (b - a) * p


# ── Rasm yordamchilari ──
_mask_cache = {}


def rmask(w, h, r):
    k = (w, h, r)
    if k not in _mask_cache:
        m = Image.new("L", (w * 2, h * 2), 0)
        ImageDraw.Draw(m).rounded_rectangle((0, 0, w * 2 - 1, h * 2 - 1), radius=r * 2, fill=255)
        _mask_cache[k] = m.resize((w, h), Image.LANCZOS)
    return _mask_cache[k]


def paste(canvas, img, cx, cy, sx=1.0, sy=None, alpha=1.0, radius=0):
    """`img` ni markazi (cx, cy) da `sx`/`sy` masshtab va shaffoflik bilan joylaydi."""
    sy = sx if sy is None else sy
    w, h = max(2, int(img.width * sx)), max(2, int(img.height * sy))
    im = img if (w, h) == img.size else img.resize((w, h), Image.BILINEAR)
    m = rmask(w, h, int(radius)) if radius > 0 else None
    if alpha < 0.999:
        base = m if m is not None else Image.new("L", (w, h), 255)
        m = base.point(lambda v: int(v * alpha))
    canvas.paste(im, (int(cx - w / 2), int(cy - h / 2)), m)


def fill_rect(img, box, color):
    ImageDraw.Draw(img).rectangle(box, fill=color)


def text_center(d, xy, s, f, fill):
    d.text(xy, s, font=f, fill=fill, anchor="mm")


def wrap(s, f, maxw, d):
    out, cur = [], ""
    for wd in s.split(" "):
        t = (cur + " " + wd).strip()
        if d.textlength(t, font=f) <= maxw:
            cur = t
        else:
            out.append(cur)
            cur = wd
    out.append(cur)
    return out


# ── Fon va telefon ramkasi ──
def base_canvas():
    a = np.zeros((H, W, 3), np.uint8)
    top, bot = np.array([14, 14, 18]), np.array([6, 6, 8])
    for y in range(H):
        a[y, :] = top + (bot - top) * (y / H)
    return Image.fromarray(a)


BASE = base_canvas()
PX, PY = 180, 350  # telefon ekrani chap-yuqori burchagi
SW, SH = 720, 1280


def new_frame(phone=True):
    f = BASE.copy()
    if phone:
        d = ImageDraw.Draw(f)
        d.rounded_rectangle((PX - 18, PY - 18, PX + SW + 18, PY + SH + 18), radius=70, fill=(28, 28, 32), outline=(60, 60, 66), width=3)
    return f


def put_screen(f, screen):
    f.paste(screen, (PX, PY), rmask(SW, SH, 54))


def caption(f, title, lines, t, d_in=0.5):
    """Tepada sarlavha, pastda izoh; sahna boshida yumshoq paydo bo'ladi."""
    a = clamp01(t / d_in)
    layer = Image.new("RGBA", (W, H), (0, 0, 0, 0))
    d = ImageDraw.Draw(layer)
    al = int(255 * ease(a))
    off = int((1 - ease(a)) * 24)
    ft = font(54)
    tl = wrap(title, ft, W - 120, d)
    y = 110 + off
    for ln in tl:
        d.text((W // 2, y), ln, font=ft, fill=(255, 255, 255, al), anchor="mm")
        y += 66
    fb = font(38, False)
    y = PY + SH + 70 + off
    for s in lines:
        for ln in wrap(s, fb, W - 140, d):
            d.text((W // 2, y), ln, font=fb, fill=(200, 200, 206, al), anchor="mm")
            y += 50
    f.paste(layer, (0, 0), layer)


def finger(f, x, y, p):
    """Barmoq izi (yarim shaffof doira); p — bosilish kuchi 0..1."""
    r = int(46 + 12 * p)
    layer = Image.new("RGBA", (r * 2 + 4, r * 2 + 4), (0, 0, 0, 0))
    ImageDraw.Draw(layer).ellipse((2, 2, r * 2 + 2, r * 2 + 2), fill=(255, 255, 255, int(70 + 70 * p)), outline=(255, 255, 255, 200), width=4)
    f.paste(layer, (int(x - r - 2), int(y - r - 2)), layer)


# ── Ekranlar ──
HOME, SEARCH, CAT = load("6_home.png"), load("7_search.png"), load("8_catalog.png")
LIB0 = load("10_lib0.png")
PROFILE, DIALOG = load("11_profile.png"), load("12_dialog.png")
S_INFO, S_EPS, S_SEC, S_COM = load("13_season_info.png"), load("13_season_eps.png"), load("13_season_sec.png"), load("13_season_com.png")
TAB_SCREENS = [HOME, SEARCH, CAT, LIB0, PROFILE]


class Scene:
    def __init__(self, name, dur):
        self.name, self.dur = name, (1.5 if FAST else dur)


frames_written = 0
proc = None


def start_ffmpeg():
    global proc
    crf = "34" if FAST else "21"
    proc = subprocess.Popen(
        ["ffmpeg", "-y", "-loglevel", "error", "-f", "rawvideo", "-pix_fmt", "rgb24", "-s", f"{W}x{H}", "-r", str(FPS), "-i", "-",
         "-c:v", "libx264", "-preset", "veryfast", "-crf", crf, "-pix_fmt", "yuv420p", "-movflags", "+faststart", OUT],
        stdin=subprocess.PIPE,
    )


def emit(f):
    global frames_written
    proc.stdin.write(f.tobytes())
    frames_written += 1


def run(scene, fn):
    n = int(scene.dur * FPS)
    for i in range(n):
        emit(fn(i / FPS, scene.dur))


# ═══════════ SAHNALAR ═══════════
def s_intro(t, dur):
    f = new_frame()
    p = spring(t - 0.2, 1.6, 0.6)
    put_screen_scaled = HOME
    paste(f, put_screen_scaled, PX + SW // 2, PY + SH // 2, 0.78 + 0.22 * p, alpha=clamp01(t / 0.6), radius=54)
    d = ImageDraw.Draw(f)
    a = ease(t / 1.0)
    d.text((W // 2, 110 + int((1 - a) * 30)), "ARUMEDIATVRS", font=font(78), fill=(255, 255, 255), anchor="mm")
    d.text((W // 2, 200), "Silliq harakat: HarmonyOS tuyg'usi", font=font(44, False), fill=ACCENT, anchor="mm")
    caption_lines = ["Bu video ilovaning HAQIQIY ekranlari ustida animatsiyalar PROTOTIPINI ko'rsatadi.", "Ular ilovaning o'zida hali yo'q — Rust ilovada qanday bo'lishini oldindan ko'rasiz."]
    fb = font(38, False)
    y = PY + SH + 70
    for s in caption_lines:
        for ln in wrap(s, fb, W - 140, d):
            d.text((W // 2, y), ln, font=fb, fill=(200, 200, 206), anchor="mm")
            y += 50
    return f


def lane(f, y, label, pos, color):
    d = ImageDraw.Draw(f)
    d.rounded_rectangle((120, y - 12, W - 120, y + 12), radius=12, fill=(40, 40, 46))
    d.text((120, y - 70), label, font=font(40), fill=(230, 230, 235), anchor="lm")
    x = 120 + 45 + (W - 240 - 90) * pos
    d.ellipse((x - 45, y - 45, x + 45, y + 45), fill=color)


def graph(f, box, curves, cursor):
    d = ImageDraw.Draw(f)
    x0, y0, x1, y1 = box
    d.rounded_rectangle(box, radius=24, fill=(20, 20, 24), outline=(50, 50, 56), width=2)
    yb = lambda v: y1 - 30 - (y1 - y0 - 60) * (v / 1.3)
    d.line((x0 + 20, yb(1.0), x1 - 20, yb(1.0)), fill=(70, 70, 78), width=2)
    for pts, col in curves:
        xy = [(x0 + 30 + (x1 - x0 - 60) * i / (len(pts) - 1), yb(v)) for i, v in enumerate(pts)]
        d.line(xy, fill=col, width=6, joint="curve")
    cx = x0 + 30 + (x1 - x0 - 60) * cursor
    d.line((cx, y0 + 15, cx, y1 - 15), fill=(255, 255, 255), width=2)


def s_spring(t, dur):
    f = new_frame(phone=False)
    cyc = 3.2
    lt = t % cyc
    half = dur / 2
    ease_curve = [ease(i / 60) for i in range(61)]
    if t < half:
        title = "Prujina nima? Oddiy va prujinali harakat"
        # chapga-o'ngga: ease vs spring
        forward = int(t / cyc) % 2 == 0
        k = clamp01(lt / 1.4)
        ep = ease(k)
        sp = spring(lt, 1.7, 0.5)
        if not forward:
            ep, sp = 1 - ep, 1 - sp
        lane(f, 640, "Oddiy (ease): yumshoq to'xtaydi", ep, (140, 140, 150))
        lane(f, 980, "Prujina (spring): oshib, qaytib to'xtaydi", sp, ACCENT)
        sc = [spring(i / 60 * 2.0, 1.7, 0.5) for i in range(61)]
        graph(f, (90, 1180, W - 90, 1640), [(ease_curve, (140, 140, 150)), (sc, ACCENT)], clamp01(lt / 2.0))
        lines = ["Kulrang: oddiy — to'xtash joyida sekinlashadi, jonsiz tuyuladi.", "To'q sariq: prujina — nishondan biroz o'tib qaytadi, jonli va tabiiy."]
    else:
        title = "Prujina sozlamasi: yumshoq, o'rtacha, tez"
        variants = [("Yumshoq (past qattiqlik)", 1.1, 0.45), ("O'rtacha", 1.8, 0.55), ("Tez va qattiq", 3.0, 0.7)]
        forward = int((t - half) / cyc) % 2 == 0
        for j, (lab, fq, z) in enumerate(variants):
            sp = spring(lt, fq, z)
            lane(f, 560 + j * 330, lab, sp if forward else 1 - sp, [(150, 150, 255), ACCENT, (90, 220, 150)][j])
        lines = ["Qattiqlik va so'nish qiymatlari bilan his o'zgaradi: kichik tugma uchun tez, katta sahifa uchun yumshoq prujina."]
    caption(f, title, lines, t % half if t >= half else t)
    return f


CARD1 = (34, 258, 346, 738)
CARD2 = (376, 258, 688, 738)


def s_press(t, dur):
    f = new_frame()
    screen = HOME.copy()
    cyc = 4.0
    lt = t % cyc
    card, nm = (CARD1, 0) if int(t / cyc) % 2 == 0 else (CARD2, 1)
    crop = HOME.crop(card)
    # bosish: 0.6 s da bosiladi, 2.2 s da qo'yib yuboriladi
    if lt < 0.6:
        s = 1.0
        fp = 0.0
    elif lt < 2.2:
        s = lerp(1.0, 0.94, spring(lt - 0.6, 3.5, 0.8))
        fp = clamp01((lt - 0.6) / 0.15)
    else:
        s = lerp(0.94, 1.0, spring(lt - 2.2, 2.2, 0.38))
        fp = max(0.0, 1 - (lt - 2.2) / 0.2)
    fill_rect(screen, card, BG)
    cx, cy = (card[0] + card[2]) / 2, (card[1] + card[3]) / 2
    paste(screen, crop, cx, cy, s, radius=40)
    put_screen(f, screen)
    if fp > 0.02 or 0.6 <= lt < 2.2:
        finger(f, PX + cx, PY + cy + 60, fp if lt < 2.2 else fp)
    msg = "Bosilganda karta kichrayadi (94%)" if 0.6 <= lt < 2.2 else ("Qo'yib yuborilganda prujina bilan qaytadi" if lt >= 2.2 else "Karta tinch turibdi")
    caption(f, "Bosish effekti", [msg, "Tugma bosilganini ko'z darhol sezadi: javob tez va jonli."], t, 0.5)
    return f


def s_page(t, dur):
    f = new_frame()
    # fazalar: 0..3 kutish, 3 oldinga, 8 ushlab, 11..16 orqaga (barmoq bilan tortish), 16.. tugash
    u = t / dur * 26.0 if not FAST else t * 10
    scr = Image.new("RGB", (SW, SH), BG)
    prog = 0.0  # 0 — bosh sahifa, 1 — ichki sahifa
    note = "Bosh sahifa"
    fx = None
    if u < 3:
        prog = 0
    elif u < 9:
        prog = spring(u - 3, 1.5, 0.62)
        note = "Yangi sahifa o'ng tomondan prujina bilan kiradi, orqadagi sahifa kichrayib xiralashadi"
    elif u < 12:
        prog = 1
        note = "Ichki sahifa"
    elif u < 15:
        # barmoq chetdan tortadi: sahifa barmoq ortidan boradi
        k = ease((u - 12) / 3.0)
        prog = 1 - 0.55 * k
        fx = (PX + 30 + 380 * k, PY + 640)
        note = "Orqaga: sahifa barmoq ortidan ergashadi"
    elif u < 21:
        base_p = 0.45
        prog = base_p * (1 - spring(u - 15, 1.7, 0.7))
        fx = None
        note = "Qo'yib yuborilganda prujina bilan joyiga qaytadi"
    else:
        prog = 0
    back_s = 1 - 0.08 * prog
    back_x = -110 * prog
    paste(scr, HOME, SW / 2 + back_x, SH / 2, back_s, radius=int(40 * prog))
    # xiralashish
    dim = Image.new("RGB", (SW, SH), (0, 0, 0))
    ov = rmask(SW, SH, 1).point(lambda v: int(v * 0.45 * prog))
    scr.paste(dim, (0, 0), ov)
    fx0 = lerp(SW, 0, prog)
    scr.paste(S_EPS, (int(fx0), 0))
    if prog > 0.001:
        # chap chetida soya
        sh = Image.new("RGBA", (40, SH), (0, 0, 0, 0))
        dd = ImageDraw.Draw(sh)
        for i in range(40):
            dd.line((i, 0, i, SH), fill=(0, 0, 0, int(90 * (i / 40) ** 2 * clamp01(prog * 2))))
        scr.paste(sh, (int(fx0) - 40, 0), sh)
    put_screen(f, scr)
    if fx:
        finger(f, fx[0], fx[1], 1.0)
    caption(f, "Sahifa o'tishi (parallaks)", [note, "Sahifalar orasida bog'liqlik sezilib turadi: qaerdan kelib, qaerga ketgani aniq."], t, 0.5)
    return f


def s_shared(t, dur):
    f = new_frame()
    scr = Image.new("RGB", (SW, SH), BG)
    u = (t / dur) * 22 if not FAST else t * 9
    # 0..4 kutish, 4..11 ochilish, 11..15 ushlab, 15..21 yopilish
    if u < 4:
        p = 0.0
    elif u < 11:
        p = spring(u - 4, 1.4, 0.7)
    elif u < 15:
        p = 1.0
    else:
        p = 1 - spring(u - 15, 1.4, 0.75)
    p = max(0.0, p)
    card = CARD1
    hero = (32, 120, 688, 489)
    cr = HOME.crop(card)
    hr = S_INFO.crop(hero)
    # uy ekrani xiralashadi
    paste(scr, HOME, SW / 2, SH / 2, 1 - 0.06 * p)
    shade = Image.new("RGB", (SW, SH), (0, 0, 0))
    scr.paste(shade, (0, 0), rmask(SW, SH, 1).point(lambda v: int(v * 0.5 * p)))
    # ichki sahifa (kontent) — kartadan ochilayotganda paydo bo'ladi
    a_page = clamp01((p - 0.35) / 0.5)
    if a_page > 0:
        page = S_INFO.copy()
        fill_rect(page, hero, BG)  # rasm joyi bo'sh: uni kartaning o'zi to'ldiradi
        m = rmask(SW, SH, 1).point(lambda v: int(v * a_page))
        scr.paste(page, (0, 0), m)
    cx = lerp((card[0] + card[2]) / 2, (hero[0] + hero[2]) / 2, p)
    cy = lerp((card[1] + card[3]) / 2, (hero[1] + hero[3]) / 2, p)
    w = lerp(card[2] - card[0], hero[2] - hero[0], p)
    h = lerp(card[3] - card[1], hero[3] - hero[1], p)
    blend = Image.blend(cr.resize((int(w), int(h)), Image.BILINEAR), hr.resize((int(w), int(h)), Image.BILINEAR), ease(clamp01(p * 1.1)))
    paste(scr, blend, cx, cy, 1.0, radius=int(lerp(40, 34, p)))
    put_screen(f, scr)
    note = "Karta bosildi" if u < 4 else ("Rasm joyidan o'sib, yangi sahifaning tepasiga aylanadi" if u < 11 else ("Sahifa ochiq" if u < 15 else "Orqaga: rasm kartaga qaytib joylashadi"))
    caption(f, "Umumiy element o'tishi", [note, "Ko'z rasmni kuzatib boradi, shuning uchun sahifa 'sakramaydi'."], t, 0.5)
    return f


TAB_BAR = (38, 527, 682, 591)
TAB_LABELS = ["Ma'lumot", "Qismlar", "Bo'limlar", "Izohlar"]
TAB_IMGS = [S_INFO, S_EPS, S_SEC, S_COM]


def draw_tabbar(scr, pos):
    """Yorliq paneli: faol 'tabletka' `pos` (0..3 kasr) bo'yicha silliq suriladi."""
    d = ImageDraw.Draw(scr)
    x0, y0, x1, y1 = TAB_BAR
    d.rectangle(TAB_BAR, fill=(10, 10, 12))
    tw = (x1 - x0) / 4
    px = x0 + tw * pos
    d.rounded_rectangle((px, y0, px + tw, y1), radius=22, fill=(200, 60, 12))
    for i, lab in enumerate(TAB_LABELS):
        act = clamp01(1 - abs(pos - i))
        col = tuple(int(lerp(140, 255, act) if True else 255) for _ in range(3))
        d.text((x0 + tw * i + tw / 2, (y0 + y1) / 2), lab, font=font(30, act > 0.5), fill=col, anchor="mm")


def s_tabs(t, dur):
    f = new_frame()
    u = t / dur * 18 if not FAST else t * 8
    # 3 s dan boshlab har 3.5 s da keyingi yorliq
    idx = clamp01((u - 2) / 3.5)
    steps = [(2, 0, 1), (5.5, 1, 2), (9, 2, 3), (12.5, 3, 2), (15.5, 2, 0)]
    pos = 0.0
    a, b, p = 0, 0, 1.0
    for s0, fr, to in steps:
        if u >= s0:
            a, b = fr, to
            p = spring(u - s0, 2.0, 0.66)
            pos = lerp(fr, to, p)
    if u < 2:
        a = b = 0
        pos = 0
    scr = S_INFO.copy()
    # kontent: pastdagi qism (yorliqlar ostidan)
    cut = 600
    base = Image.new("RGB", (SW, SH - cut), BG)
    ia, ib = TAB_IMGS[a].crop((0, cut, SW, SH)), TAB_IMGS[b].crop((0, cut, SW, SH))
    dirn = 1 if b >= a else -1
    off = lerp(0, SW * dirn, clamp01(p)) if a != b else 0
    base.paste(ia, (int(-off), 0))
    if a != b:
        base.paste(ib, (int(SW * dirn - off), 0))
    scr.paste(base, (0, cut))
    draw_tabbar(scr, pos)
    put_screen(f, scr)
    caption(f, "Yorliqlar almashishi", ["Faol 'tabletka' yorliqdan yorliqqa prujina bilan sirpanadi, kontent yon tomonga suriladi.", "Barmoq bilan chapga-o'ngga surib ham o'tish mumkin."], t, 0.5)
    return f


NAV_Y0, NAV_Y1 = 1112, 1256
SLOT_X = [107, 233, 360, 486, 611]


def nav_glyphs():
    """Pastki menyu belgilarini ekran suratlaridan kesib oladi (faol — oq, nofaol — kulrang)."""
    act, ina = [], []
    for j in range(5):
        cx = SLOT_X[j]
        a_img = TAB_SCREENS[j].crop((cx - 56, 1130, cx + 56, 1240))
        arr = np.array(a_img).astype(int)
        m = (arr.min(axis=2) > 150).astype(np.uint8) * 255
        act.append((a_img, Image.fromarray(m)))
        other = TAB_SCREENS[(j + 1) % 5]
        i_img = other.crop((cx - 56, 1130, cx + 56, 1240))
        arr = np.array(i_img).astype(int)
        lum = arr.sum(axis=2) / 3
        m = ((lum > 60)).astype(np.uint8) * 255
        ina.append((i_img, Image.fromarray(m)))
    return act, ina


ACT, INA = nav_glyphs()


def draw_nav(scr, pos):
    d = ImageDraw.Draw(scr)
    bar = (34, NAV_Y0, 686, NAV_Y1)
    d.rounded_rectangle(bar, radius=70, fill=(16, 16, 18), outline=(40, 40, 44), width=2)
    px = lerp(SLOT_X[0], SLOT_X[4], pos / 4)
    d.rounded_rectangle((px - 56, 1132, px + 56, 1236), radius=40, fill=(200, 60, 12))
    for j in range(5):
        a = clamp01(1 - abs(pos - j))
        cx = SLOT_X[j]
        img, m = ACT[j] if a > 0.5 else INA[j]
        mm = m.point(lambda v: int(v * (0.35 + 0.65 * a)))
        white = Image.new("RGB", img.size, (255, 255, 255) if a > 0.5 else (122, 122, 128))
        scr.paste(white, (cx - 56, 1130), mm)


def s_nav(t, dur):
    f = new_frame()
    u = t / dur * 20 if not FAST else t * 9
    order = [0, 1, 2, 3, 4, 0]
    pos, a, b, p = 0.0, 0, 0, 1.0
    t0 = 2.0
    for i in range(5):
        s0 = t0 + 3.0 * i
        if u >= s0:
            a, b = order[i], order[i + 1]
            p = spring(u - s0, 1.8, 0.62)
            pos = lerp(a, b, p)
    scr = Image.new("RGB", (SW, SH), BG)
    ia, ib = TAB_SCREENS[a], TAB_SCREENS[b]
    off = lerp(0, 90, clamp01(p)) if a != b else 0
    ca = clamp01(1 - p * 1.4) if a != b else 1.0
    scr.paste(ib if p > 0.5 else ia, (0, 0))
    if a != b and p <= 0.5:
        pass
    if a != b:
        mix = ease(clamp01(p * 1.3))
        scr.paste(Image.blend(ia, ib, mix), (0, 0))
    draw_nav(scr, pos)
    put_screen(f, scr)
    caption(f, "Pastki menyu", ["Tabletka tab'lar orasida prujina bilan oqib o'tadi, ekranlar yumshoq almashadi.", "Belgilar faol bo'lganda oq rangga va yozuvga aylanadi."], t, 0.5)
    return f


def s_list(t, dur):
    f = new_frame()
    scr = HOME.copy()
    cards = [CARD1, CARD2, (34, 770, 346, 1110), (376, 770, 688, 1110)]
    u = t / dur * 16 if not FAST else t * 6
    cyc = 8.0
    lu = u % cyc
    for i, c in enumerate(cards):
        fill_rect(scr, c, BG)
    for i, c in enumerate(cards):
        delay = 0.7 + 0.28 * i
        p = spring(lu - delay, 2.0, 0.62)
        al = clamp01((lu - delay) / 0.35)
        if lu > 6.5:  # chiqish
            al = clamp01(1 - (lu - 6.5) / 1.0)
            p = 1
        crop = HOME.crop(c)
        cx, cy = (c[0] + c[2]) / 2, (c[1] + c[3]) / 2 + (1 - p) * 120
        paste(scr, crop, cx, cy, lerp(0.9, 1.0, min(1.1, p)), alpha=al, radius=40)
    # sarlavha va menyu o'rnida qoladi (HOME dagi tepa va pastki qismlar)
    top = HOME.crop((0, 0, SW, 250))
    scr.paste(top, (0, 0))
    nav = HOME.crop((0, 1110, SW, SH))
    scr.paste(nav, (0, 1110))
    put_screen(f, scr)
    caption(f, "Ro'yxat animatsiyasi", ["Kartalar birin-ketin pastdan ko'tarilib, prujina bilan o'z joyiga o'tiradi.", "Kechikish har kartada oz-ozdan oshadi — to'lqin effekti."], t, 0.5)
    return f


def s_dialog(t, dur):
    f = new_frame()
    u = t / dur * 14 if not FAST else t * 6
    # 2..7 ochiladi, 7..10 ushlab, 10..13 yopiladi
    if u < 2:
        p = 0.0
    elif u < 8:
        p = spring(u - 2, 2.2, 0.6)
    elif u < 10:
        p = 1.0
    else:
        p = max(0.0, 1 - ease((u - 10) / 1.2))
    scr = PROFILE.copy()
    dim = Image.new("RGB", (SW, SH), (0, 0, 0))
    scr.paste(dim, (0, 0), rmask(SW, SH, 1).point(lambda v: int(v * 0.6 * clamp01(p))))
    panel = (126, 463, 594, 816)
    crop = DIALOG.crop(panel)
    if p > 0.001:
        cx, cy = (panel[0] + panel[2]) / 2, (panel[1] + panel[3]) / 2
        paste(scr, crop, cx, cy, lerp(0.86, 1.0, min(1.15, p)), alpha=clamp01(p * 2.2), radius=50)
    put_screen(f, scr)
    caption(f, "Dialog oynasi", ["Oyna markazdan biroz kattalashib, prujina bilan ochiladi; orqa fon asta qorayadi.", "Yopilganda tezroq va yumshoqroq ketadi."], t, 0.5)
    return f


def s_outro(t, dur):
    f = new_frame(phone=False)
    d = ImageDraw.Draw(f)
    a = ease(t / 0.8)
    d.text((W // 2, 170), "Xulosa va keyingi qadamlar", font=font(64), fill=(255, 255, 255), anchor="mm")
    items = [
        "1. Prujina animatsiya yadrosi (kutubxonaga)",
        "2. Bosish effekti va sahifa o'tishlari",
        "3. Yorliqlar, pastki menyu, dialog va ro'yxat",
        "4. Umumiy element o'tishi (rasm kattalashib ochiladi)",
        "5. Blur/shisha faqat kuchli telefonlarda (avtomatik)",
    ]
    y = 360
    f46 = font(42, False)
    for i, s in enumerate(items):
        al = clamp01((t - 0.5 - 0.6 * i) / 0.6)
        col = tuple(int(c * al + 14 * (1 - al)) for c in (235, 235, 240))
        for k, ln in enumerate(wrap(s, f46, W - 180, d)):
            d.text((90, y + k * 54 + (1 - al) * 20), ln, font=f46, fill=col, anchor="lm")
        y += 140
    fb = font(38, False)
    y = 1180
    for s in ["Kuchsiz telefonlarda (Redmi 9C) silliqlik saqlanishi uchun og'ir effektlar (blur, katta soyalar) ishlatilmaydi.",
              "Har bir animatsiya telefonda o'lchanib, 60 kadr/s ichida qolishi tekshiriladi."]:
        for ln in wrap(s, fb, W - 160, d):
            d.text((W // 2, y), ln, font=fb, fill=(190, 190, 198), anchor="mm")
            y += 54
        y += 20
    d.text((W // 2, 1660), "ARUMEDIATVRS", font=font(70), fill=ACCENT, anchor="mm")
    return f


def main():
    start_ffmpeg()
    scenes = [
        (Scene("intro", 6), s_intro),
        (Scene("spring", 20), s_spring),
        (Scene("press", 16), s_press),
        (Scene("page", 26), s_page),
        (Scene("shared", 22), s_shared),
        (Scene("tabs", 18), s_tabs),
        (Scene("nav", 20), s_nav),
        (Scene("list", 16), s_list),
        (Scene("dialog", 14), s_dialog),
        (Scene("outro", 10), s_outro),
    ]
    for sc, fn in scenes:
        run(sc, fn)
        print("sahna tayyor:", sc.name, flush=True)
    proc.stdin.close()
    proc.wait()
    print(f"Tayyor: {OUT} ({frames_written} kadr, {frames_written / FPS:.0f} s)")


if __name__ == "__main__":
    main()
