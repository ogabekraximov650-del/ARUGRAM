"""
"Yashil o'tloq" — 1 daqiqalik anime uslubidagi qisqa film.

Qiz personaj yam-yashil o'tloqda shamolda turibdi. Butun tasvir va musiqa
faqat kod orqali yaratiladi (Pillow + numpy + ffmpeg).

Ishga tushirish:
    pip install numpy pillow scipy imageio-ffmpeg
    python3 anime/make_anime.py            # to'liq video
    python3 anime/make_anime.py --still 20 # bitta kadrni PNG qilib saqlash
"""
import math
import os
import subprocess
import sys
from multiprocessing import Pool

import numpy as np
from PIL import Image, ImageDraw, ImageFilter, ImageFont

W, H = 1280, 720
SS = 2                      # supersampling (silliq chiziqlar uchun)
SW, SH = W * SS, H * SS
FPS = 24
DUR = 60.0
NFR = int(FPS * DUR)
HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(HERE, "yashil_otloq.mp4")
HORIZON_WY = -150.0


# ---------------------------------------------------------------- yordamchilar
def lerp(a, b, t):
    return a + (b - a) * t


def clamp(x, a=0.0, b=1.0):
    return max(a, min(b, x))


def ease(t):
    t = clamp(t)
    return 0.5 - 0.5 * math.cos(math.pi * t)


def mix(c1, c2, t):
    return tuple(int(round(lerp(a, b, t))) for a, b in zip(c1, c2))


def wind(t):
    return 0.55 + 0.25 * math.sin(t * 0.6) + 0.12 * math.sin(t * 1.7 + 1.0) + 0.08 * math.sin(t * 3.1)


def ellipse(cx, cy, rx, ry, n=28):
    return [(cx + rx * math.cos(2 * math.pi * i / n), cy + ry * math.sin(2 * math.pi * i / n)) for i in range(n)]


def stroke(cl, hw):
    """Markaziy chiziq va yarim-qalinliklardan ko'pburchak yasaydi."""
    left, right = [], []
    n = len(cl)
    for i in range(n):
        x0, y0 = cl[max(i - 1, 0)]
        x1, y1 = cl[min(i + 1, n - 1)]
        dx, dy = x1 - x0, y1 - y0
        L = math.hypot(dx, dy) or 1.0
        nx, ny = -dy / L, dx / L
        w = hw[i] if isinstance(hw, (list, tuple)) else hw
        left.append((cl[i][0] + nx * w, cl[i][1] + ny * w))
        right.append((cl[i][0] - nx * w, cl[i][1] - ny * w))
    return left + right[::-1]


def bez(p0, p1, p2, n=10):
    out = []
    for i in range(n + 1):
        t = i / n
        a, b, c = (1 - t) ** 2, 2 * (1 - t) * t, t * t
        out.append((a * p0[0] + b * p1[0] + c * p2[0], a * p0[1] + b * p1[1] + c * p2[1]))
    return out


# ---------------------------------------------------------------- ranglar
SKIN = (255, 231, 218)
SKIN_SH = (242, 196, 186)
LINE = (86, 52, 58)
LASH = (38, 22, 32)
HAIR = (66, 42, 54)
HAIR_SH = (44, 28, 40)
HAIR_HI = (132, 96, 112)
DRESS = (252, 252, 255)
DRESS_SH = (205, 214, 238)
RIBBON = (226, 74, 96)
SHOES = (150, 96, 70)


# ---------------------------------------------------------------- kamera
SHOTS = [(0.0, 14.0), (14.0, 30.0), (30.0, 60.0)]
XF = 0.9  # kadrlar orasidagi o'tish (soniya)


def cam_shot(i, t):
    if i == 0:  # osmondan pastga — o'tloq va qiz
        u = ease((t - 0.5) / 11.5)
        return (lerp(-60, -150, u), lerp(-640, -95, u), lerp(1.0, 1.08, u))
    if i == 1:  # to'liq bo'y
        u = (t - 14) / 16
        return (lerp(-45, -30, u), lerp(-106, -110, u), lerp(2.5, 2.85, ease(u)))
    # yuz yaqindan, keyin orqaga chekinib osmonga
    if t < 44:
        u = (t - 30) / 14
        return (lerp(-9, -5, u), -179.0, lerp(8.4, 9.2, u))
    u = ease((t - 44) / 12)
    z = math.exp(lerp(math.log(9.2), math.log(0.95), u))
    return (lerp(-5, -170, u), lerp(-179, -430, ease((t - 46) / 10)), z)


# ---------------------------------------------------------------- oldindan tayyorlanadigan qatlamlar
def make_cloud(seed, cw=1000, ch=420):
    rng = np.random.default_rng(seed)
    m = Image.new("L", (cw, ch), 0)
    d = ImageDraw.Draw(m)
    base = ch * 0.78
    for _ in range(26):
        x = rng.uniform(cw * 0.12, cw * 0.88)
        k = 1 - abs(x - cw / 2) / (cw / 2)
        r = rng.uniform(40, 70) + 110 * k * rng.uniform(0.6, 1.0)
        y = base - r * rng.uniform(0.3, 0.9)
        d.ellipse([x - r, y - r, x + r, y + r], fill=255)
    d.rectangle([0, base, cw, ch], fill=0)
    m = m.filter(ImageFilter.GaussianBlur(7))
    a = np.asarray(m).astype(np.float32) / 255.0
    # yuqorisi oq, pasti ko'kimtir soya
    shade = np.asarray(m.filter(ImageFilter.GaussianBlur(40))).astype(np.float32) / 255.0
    yy = np.linspace(0, 1, ch)[:, None]
    lit = np.clip(1.25 - yy * 1.1 + (1 - shade) * 0.5, 0, 1)
    top = np.array([255, 255, 255], np.float32)
    bot = np.array([212, 222, 244], np.float32)
    rgb = bot + (top - bot) * lit[..., None]
    rgba = np.dstack([rgb, np.clip(a * 1.15, 0, 1) * 255]).astype(np.uint8)
    return Image.fromarray(rgba, "RGBA")


def make_sun(size=760):
    yy, xx = np.mgrid[0:size, 0:size].astype(np.float32)
    r = np.hypot(xx - size / 2, yy - size / 2)
    a = np.clip(1.4 * np.exp(-(r / 55) ** 2) + 0.5 * np.exp(-(r / 190) ** 2) + 0.18 * np.exp(-(r / 330) ** 2), 0, 1)
    a *= np.clip(1 - r / (size / 2), 0, 1) ** 0.5
    rgb = np.zeros((size, size, 3), np.float32)
    rgb[...] = (255, 250, 228)
    return Image.fromarray(np.dstack([rgb, a * 255]).astype(np.uint8), "RGBA")


CLOUDS = [make_cloud(s) for s in (1, 2, 3, 4)]
SUN = make_sun()
CLOUD_POS = [  # (wx, balandlik, masshtab, tezlik, sprite)
    (-760, 330, 1.00, 6.0, 0), (-180, 500, 0.80, 5.0, 1), (420, 300, 1.15, 7.0, 2),
    (980, 540, 0.75, 4.0, 3), (-1300, 580, 0.90, 5.0, 1), (160, 150, 0.55, 8.0, 3),
    (-450, 700, 0.70, 4.5, 2), (700, 760, 0.85, 4.0, 0),
]

# o't barglari va gullar
_rng = np.random.default_rng(7)
NG = 5200
G_X = _rng.uniform(-1500, 1100, NG)
G_D = _rng.uniform(0, 1, NG) ** 1.5
G_Y = HORIZON_WY + G_D * 540
G_H = 3 + G_D * 24 * _rng.uniform(0.7, 1.3, NG)
G_W = 0.7 + G_D * 2.6
G_PH = _rng.uniform(0, 6.28, NG)
_gc_far = np.array([160, 212, 112])
_gc_near = np.array([58, 140, 52])
G_COL = (_gc_far + (_gc_near - _gc_far) * G_D[:, None] + _rng.normal(0, 14, (NG, 3))).clip(0, 255).astype(int)
G_TYPE = (_rng.uniform(0, 1, NG) < 0.07).astype(int)  # 1 = gul
FLOWER_COLS = [(255, 255, 255), (255, 214, 226), (255, 236, 130), (200, 190, 255)]
G_FC = _rng.integers(0, len(FLOWER_COLS), NG)
_ord = np.argsort(G_Y)
G_X, G_D, G_Y, G_H, G_W, G_PH, G_COL, G_TYPE, G_FC = (a[_ord] for a in (G_X, G_D, G_Y, G_H, G_W, G_PH, G_COL, G_TYPE, G_FC))

# ekran oldidagi gulbarglar va nur zarrachalari
NP = 34
P_X0 = _rng.uniform(0, W + 200, NP)
P_Y0 = _rng.uniform(-50, H + 50, NP)
P_VX = _rng.uniform(35, 90, NP)
P_VY = _rng.uniform(8, 30, NP)
P_S = _rng.uniform(3, 9, NP)
P_PH = _rng.uniform(0, 6.28, NP)
P_COL = [mix((255, 200, 215), (255, 246, 250), _rng.uniform()) for _ in range(NP)]
NM = 35
M_X0 = _rng.uniform(0, W, NM)
M_Y0 = _rng.uniform(0, H, NM)
M_PH = _rng.uniform(0, 6.28, NM)

# post-effektlar
_yy, _xx = np.mgrid[0:H, 0:W].astype(np.float32)
VIGNETTE = (1 - 0.14 * (((_xx - W / 2) / (W / 2)) ** 2 + ((_yy - H / 2) / (H / 2)) ** 2) ** 1.4).clip(0.8, 1)[..., None]
LEAK = (np.exp(-(((_xx - W * 1.05) / 520) ** 2 + ((_yy + 80) / 420) ** 2))[..., None] * np.array([60, 42, 18], np.float32))

try:
    FONT_BIG = ImageFont.truetype("/usr/share/fonts/truetype/dejavu/DejaVuSerif-Bold.ttf", 64)
    FONT_SM = ImageFont.truetype("/usr/share/fonts/truetype/dejavu/DejaVuSerif.ttf", 26)
except OSError:
    FONT_BIG = FONT_SM = ImageFont.load_default()


# ---------------------------------------------------------------- fon
def sky(hy, z):
    zs = 1 + (z - 1) * 0.08
    rows = (np.arange(SH) + 0.5) / SS
    el = (hy - rows) / zs
    stops = [-50, 0, 200, 720]
    cols = np.array([(225, 240, 250), (218, 238, 250), (138, 194, 246), (52, 116, 214)], np.float32)
    col = np.stack([np.interp(el, stops, cols[:, c]) for c in range(3)], 1)
    arr = np.broadcast_to(col[:, None, :], (SH, SW, 3)).astype(np.uint8)
    return Image.fromarray(np.ascontiguousarray(arr), "RGB")


def draw_ridge(d, cx, z, hy, p, prof, color, rim=None):
    zp = 1 + (z - 1) * p
    xs = np.arange(-20, W + 21, 8, dtype=np.float64)
    wx = (xs - W / 2) / zp + cx * p
    ys = hy - prof(wx) * zp
    pts = [(x * SS, y * SS) for x, y in zip(xs, ys)]
    bottom = (hy + 60 * z) * SS
    d.polygon(pts + [((W + 20) * SS, bottom), (-20 * SS, bottom)], fill=color)
    if rim:
        d.line(pts, fill=rim, width=max(1, int(2 * zp)))
    return zp


def far_prof(x):
    return 100 + 48 * np.sin(x * 0.0035 + 0.7) + 24 * np.sin(x * 0.009 + 2) + 8 * np.sin(x * 0.031)


def mid_prof(x):
    return 40 + 17 * np.sin(x * 0.0065 + 1.3) + 9 * np.sin(x * 0.019)


def near_prof(x):
    return 16 + 9 * np.sin(x * 0.011 + 0.4) + 4 * np.sin(x * 0.027 + 1)


def ground_prof(x):
    return 5 * np.sin(x * 0.009 + 0.3) + 3 * np.sin(x * 0.023)


TREES = [-620, -560, -330, 140, 185, 610, 1000, -1050]


def draw_background(img, t, cx, cy, z, hy):
    # quyosh
    zs = 1 + (z - 1) * 0.03
    sx, sy = (380 - cx * 0.03) * zs + W / 2, hy - 560 * zs
    if -400 < sx < W + 400 and -400 < sy < H + 400:
        img.paste(SUN, (int(sx * SS - SUN.width / 2), int(sy * SS - SUN.height / 2)), SUN)
    # bulutlar
    zc = 1 + (z - 1) * 0.06
    for wx, el, sc, sp, k in CLOUD_POS:
        spr = CLOUDS[k]
        s = sc * zc
        cw, ch = int(spr.width * s), int(spr.height * s)
        x = ((wx + sp * t - cx * 0.06) * zc + W / 2) * SS
        y = (hy - el * zc) * SS
        if x + cw / 2 < 0 or x - cw / 2 > SW or y + ch / 2 < 0 or y - ch / 2 > SH:
            continue
        r = spr.resize((cw, ch), Image.BILINEAR)
        img.paste(r, (int(x - cw / 2), int(y - ch / 2)), r)

    d = ImageDraw.Draw(img)
    draw_ridge(d, cx, z, hy, 0.08, far_prof, (158, 190, 224), (190, 214, 238))
    zp = draw_ridge(d, cx, z, hy, 0.22, mid_prof, (122, 182, 128))
    for tx in TREES:  # tepalikdagi daraxtlar
        e = float(mid_prof(np.array([tx]))[0])
        x = ((tx - cx * 0.22) * zp + W / 2) * SS
        y = (hy - e * zp) * SS
        r = 13 * zp * SS
        if -3 * r < x < SW + 3 * r:
            d.rectangle([x - r * 0.12, y - r * 0.9, x + r * 0.12, y + r * 0.3], fill=(96, 80, 70))
            for ox, oy, rr, c in ((-0.55, -1.3, 0.75, (74, 132, 90)), (0.55, -1.3, 0.75, (74, 132, 90)),
                                  (0, -1.9, 0.9, (84, 146, 98)), (-0.2, -2.1, 0.45, (110, 170, 116))):
                cx_, cy_, rr_ = x + ox * r, y + oy * r, rr * r
                d.ellipse([cx_ - rr_, cy_ - rr_, cx_ + rr_, cy_ + rr_], fill=c)
    draw_ridge(d, cx, z, hy, 0.45, near_prof, (108, 176, 90), (140, 198, 110))

    # yaqin o'tloq (gradient + niqob)
    rows = (np.arange(SH) + 0.5) / SS
    dep = (rows - hy) / z
    stops = [-20, 0, 150, 450]
    cols = np.array([(150, 206, 108), (150, 206, 108), (96, 172, 72), (58, 132, 48)], np.float32)
    col = np.stack([np.interp(dep, stops, cols[:, c]) for c in range(3)], 1)
    grad = Image.fromarray(np.ascontiguousarray(np.broadcast_to(col[:, None, :], (SH, SW, 3)).astype(np.uint8)))
    mask = Image.new("L", (SW, SH), 0)
    md = ImageDraw.Draw(mask)
    xs = np.arange(-20, W + 21, 8, dtype=np.float64)
    wx = (xs - W / 2) / z + cx
    ys = hy - ground_prof(wx) * z
    md.polygon([(x * SS, y * SS) for x, y in zip(xs, ys)] + [((W + 20) * SS, SH + 10), (-20 * SS, SH + 10)], fill=255)
    img.paste(grad, (0, 0), mask)
    # bulut soyalari o'tloq ustida sekin suzadi
    sh = Image.new("L", (SW, SH), 0)
    sd = ImageDraw.Draw(sh)
    for bx, by, br in ((-300, 60, 140), (350, 180, 200), (900, 40, 120)):
        x = ((bx + 14 * t - cx) * z + W / 2) * SS
        y = ((by - cy) * z + H / 2) * SS
        rx, ry = br * z * SS, br * 0.25 * z * SS
        if x + rx > 0 and x - rx < SW and y + ry > 0 and y - ry < SH:
            sd.ellipse([x - rx, y - ry, x + rx, y + ry], fill=70)
    if sh.getbbox():
        sh = Image.fromarray((np.asarray(sh.resize((SW // 8, SH // 8)).filter(ImageFilter.GaussianBlur(6)).resize((SW, SH)), dtype=np.uint8) * (np.asarray(mask) > 0)).astype(np.uint8))
        img.paste((40, 100, 40), (0, 0), sh)


def draw_grass(d, t, cx, cy, z, front):
    sel = (G_Y >= 0) if front else (G_Y < 0)
    bx = ((G_X - cx) * z + W / 2) * SS
    by = ((G_Y - cy) * z + H / 2) * SS
    h = G_H * z * SS
    vis = sel & (bx > -h) & (bx < SW + h) & (by > 0) & (by - h < SH)
    if not vis.any():
        return
    wd = wind(t)
    ang = wd * 0.45 * (0.6 + 0.4 * G_D) + 0.2 * np.sin(2.3 * t - G_X * 0.02 + G_PH)
    for i in np.nonzero(vis)[0]:
        x, y, hh, a = bx[i], by[i], h[i], ang[i]
        w = G_W[i] * z * SS
        if G_TYPE[i]:
            hh *= 0.75
            tx, ty = x + hh * math.sin(a * 0.7), y - hh * math.cos(a * 0.7)
            d.line([(x, y), (tx, ty)], fill=(70, 140, 60), width=max(1, int(w * 0.4)))
            r = max(1.0, w * 1.3)
            fc = FLOWER_COLS[G_FC[i]]
            for k in range(5):
                pa = k * 1.2566 + t * 0.3
                px, py = tx + math.cos(pa) * r * 0.8, ty + math.sin(pa) * r * 0.8
                d.ellipse([px - r * 0.6, py - r * 0.6, px + r * 0.6, py + r * 0.6], fill=fc)
            d.ellipse([tx - r * 0.45, ty - r * 0.45, tx + r * 0.45, ty + r * 0.45], fill=(250, 200, 60))
            continue
        sa, ca = math.sin(a), math.cos(a)
        tx, ty = x + hh * sa, y - hh * ca
        mx, my = x + hh * 0.5 * math.sin(a * 0.45), y - hh * 0.5 * math.cos(a * 0.45)
        c = tuple(G_COL[i])
        d.polygon([(x - w / 2, y), (mx - w * 0.36, my), (tx, ty), (mx + w * 0.36, my), (x + w / 2, y)], fill=c)


# ---------------------------------------------------------------- qiz personaj
class GT:
    """Qiz koordinatalari (oyoq = 0, yuqoriga +) -> ekran piksellari."""

    def __init__(self, cx, cy, z, head_rot):
        self.cx, self.cy, self.z = cx, cy, z
        self.c, self.s = math.cos(head_rot), math.sin(head_rot)

    def P(self, x, y, head=False):
        if head:
            dx, dy = x, y - 163
            x, y = dx * self.c - dy * self.s, 163 + dx * self.s + dy * self.c
        return (((x - self.cx) * self.z + W / 2) * SS, ((-y - self.cy) * self.z + H / 2) * SS)


def blink_open(t):
    o = 1.0
    for tb in (4.6, 10.8, 17.3, 21.0, 26.4, 31.6, 35.2, 39.3, 42.4, 47.0, 52.5):
        o = min(o, clamp((abs(t - tb) - 0.05) / 0.1))
    return o


def draw_girl(d, t, cx, cy, z):
    wd = wind(t)
    g = GT(cx, cy, z, 0.035 * math.sin(t * 0.55) + 0.015 * math.sin(t * 1.3))
    lw = max(1, int(round(0.32 * z * SS)))
    breath = 0.35 * math.sin(t * 1.6)

    def poly(pts, fill, outline=None, head=False, width=None):
        q = [g.P(x, y, head) for x, y in pts]
        if outline:
            d.polygon(q, fill=fill, outline=outline, width=width or lw)
        else:
            d.polygon(q, fill=fill)

    # yerga tushgan soya
    sx0, sy0 = g.P(6, 0)
    rx, ry = 26 * z * SS, 4.5 * z * SS
    d.ellipse([sx0 - rx, sy0 - ry, sx0 + rx, sy0 + ry], fill=(66, 128, 52))

    # orqa sochlar
    poly(ellipse(0, 176, 19.5, 23), HAIR, LINE, head=True)
    locks = [(-11, 0.0, 88, 8.5), (-7, 1.3, 97, 9), (-2.5, 2.1, 101, 9), (2.5, 2.9, 99, 9), (7, 3.7, 95, 9), (11, 4.4, 86, 8.5)]
    for x0, ph, L, wr in locks:
        cl, ws = [], []
        for i in range(18):
            s = i / 17
            x = x0 + math.copysign(9, x0) * s ** 0.7 + wd * 28 * s ** 1.6 + 3.4 * math.sin(2.1 * t + ph - s * 2.6) * s ** 1.2
            y = 180 - L * s + wd * 11 * s ** 2
            cl.append((x, y))
            ws.append(wr * (0.55 + 0.45 * clamp(s / 0.25)) * (1 - s) ** 0.6 + 0.3)
        poly(stroke(cl, ws), HAIR, LINE)
    for x0, ph in ((-5, 1.0), (4, 2.5), (10, 3.3)):  # sochdagi qorong'i iplar
        cl = [(x0 + wd * 22 * (i / 10) ** 1.6 + 2.5 * math.sin(2.1 * t + ph - i / 4), 180 - 80 * i / 10 + wd * 9 * (i / 10) ** 2) for i in range(11)]
        poly(stroke(cl, [0.5 * (1 - i / 10) + 0.1 for i in range(11)]), HAIR_SH)

    # oyoqlar
    for sd_ in (-1, 1):
        poly(stroke([(sd_ * 5.4, 66), (sd_ * 5.0, 36), (sd_ * 4.6, 6)], [3.5, 2.7, 2.0]), SKIN, LINE)
        poly(ellipse(sd_ * 5.6, 3.2, 4.3, 2.6), SHOES, LINE)

    # yubka
    shx = wd * 7
    hem_y = 60
    L0, R0 = (-27 + shx * 0.6, hem_y + 1), (29 + shx, hem_y + wd * 5)
    left = bez((-8.5, 128), (-15, 100), L0, 10)
    hem = []
    for i in range(1, 20):
        u = i / 20
        x = lerp(L0[0], R0[0], u)
        y = lerp(L0[1], R0[1], u) + 1.8 * math.sin(i * 1.3 + t * 3.4) * (0.4 + 0.6 * wd) + 1.2 * math.sin(i * 0.5 - t * 1.7)
        hem.append((x, y))
    right = bez(R0, (16 + shx * 0.3, 100), (8.5, 128), 10)
    skirt = left + hem + right
    poly(skirt, DRESS, LINE)
    poly(bez((-8.5, 128), (-15, 100), L0, 8) + bez((L0[0] + 9, hem_y + 1), (-6, 98), (-4, 128), 8), DRESS_SH)
    for k in (4, 8, 12, 16):
        hx, hy_ = hem[k]
        fx = lerp(-6, 7, k / 19)
        poly(stroke(bez((fx, 126), (lerp(fx, hx, 0.5) - 1, 94), (hx, hy_ + 1.5), 6), 0.28), DRESS_SH)

    # qo'llar
    sway = 0.4 * math.sin(t * 0.9)
    for sd_ in (-1, 1):
        cl = [(sd_ * 14.6, 150 + breath * 0.3), (sd_ * 16.8, 131), (sd_ * 16.2 + sway * (sd_ > 0), 113)]
        poly(stroke(cl, [3.0, 2.5, 2.0]), SKIN, LINE)
        poly(ellipse(sd_ * 16.2 + sway * (sd_ > 0), 110.8, 2.5, 3.1), SKIN, LINE)

    # bo'yin
    poly([(-3.6, 168), (3.6, 168), (4.0, 160), (6.5, 159.2), (0, 153.5), (-6.5, 159.2), (-4.0, 160)], SKIN)
    poly([(-3.6, 167.5), (3.6, 167.5), (3.7, 163.5), (0, 161.8), (-3.7, 163.5)], SKIN_SH)

    # ko'ylak ustki qismi
    b = breath * 0.25
    poly([(-13.5, 157 + b), (-6, 159.2 + b), (0, 154.5 + b), (6, 159.2 + b), (13.5, 157 + b), (12, 149), (9.5, 139), (8.6, 128.5),
          (-8.6, 128.5), (-9.5, 139), (-12, 149)], DRESS, LINE)
    poly([(-13.5, 157 + b), (-9.5, 156.5), (-7.8, 142), (-7.2, 129), (-8.6, 128.5), (-9.5, 139), (-12, 149)], DRESS_SH)
    poly([(-8.9, 131.5), (8.9, 131.5), (8.6, 127.4), (-8.6, 127.4)], RIBBON, LINE)
    # belbog' bantigi va shamolda hilpiragan uchlari
    k0 = (-5.5, 129.5)
    poly([k0, (-11.5, 134.5), (-12, 126.5)], RIBBON, LINE)
    poly([k0, (0.5, 134.5), (0.8, 126.5)], RIBBON, LINE)
    for off, ln in ((0, 22), (2.5, 18)):
        cl = [(k0[0] + off * 0.4 + wd * 10 * (i / 8) + 1.8 * math.sin(3 * t + i * 0.8 + off), k0[1] - ln * i / 8 + wd * 3 * (i / 8) ** 2) for i in range(9)]
        poly(stroke(cl, [1.1] * 8 + [0.4]), RIBBON, LINE)
    poly(ellipse(k0[0], k0[1], 1.6, 1.8, 14), (196, 54, 78), LINE)
    for sd_ in (-1, 1):  # puflangan yenglar
        poly(ellipse(sd_ * 14.8, 151.5 + b, 4.7, 6.0), DRESS, LINE)

    # --- bosh
    arc = [(14.3 * math.cos(a), 183 + 14.3 * math.sin(a)) for a in np.linspace(0, math.pi, 14)]
    jaw = [(-14.3, 183), (-13.8, 177), (-11.2, 171.2), (-6.5, 167.0), (-2.4, 165.3), (0, 165.0),
           (2.4, 165.3), (6.5, 167.0), (11.2, 171.2), (13.8, 177), (14.3, 183)]
    poly(arc + jaw, SKIN, LINE, head=True)

    # peshanadagi soch soyasi va old sochlar (bangs)
    fl = 0.5 * math.sin(t * 2.7)
    tips = [(-15.4, 169.5), (-12.9, 180.8), (-9.6, 186.8), (-7.3, 180.0), (-4.6, 187.6), (-1.9, 181.4), (1.1, 188.2),
            (3.8, 180.5), (6.8, 187.4), (9.2, 181.2), (12.1, 186.8), (13.8, 177.5), (15.6, 170.0)]
    tips = [(x + (wd * 0.9 + fl * 0.4) * clamp((188 - y) / 10), y) for x, y in tips]
    top = [(16.9 * math.cos(a), 184 + 16.9 * math.sin(a)) for a in np.linspace(-0.2, math.pi + 0.2, 20)]
    poly(top + [(x + 0.5, y - 1.5) for x, y in tips], SKIN_SH, head=True)

    # ko'zlar
    o = blink_open(t)
    look_u = 0.25 * math.sin(t * 0.4)
    look_v = 0.9 * clamp((t - 46) / 4)
    for side in (-1, 1):
        draw_eye(poly, g, side, o, look_u * side, look_v, lw)

    # yonoq qizarishi, burun, og'iz
    smile = 0.35 + 0.65 * ease((t - 33) / 5)
    for side in (-1, 1):
        poly(ellipse(side * 8.5, 172.1, 2.9, 1.25, 20), (255, 206, 206), head=True)
        for k in range(3):
            x = side * 8.5 + (k - 1) * 1.0
            poly(stroke([(x - 0.35, 171.6), (x + 0.35, 172.6)], 0.12), (238, 150, 160), head=True)
    poly(stroke([(0.8, 173.4), (0.35, 172.3)], 0.16), (214, 158, 150), head=True)
    m = [(u, 168.5 - smile * 0.65 * (1 - (u / 2.2) ** 2)) for u in np.linspace(-2.2, 2.2, 9)]
    poly(stroke(m, 0.22), (150, 70, 78), head=True)

    # old sochlar
    poly(top + tips, HAIR, LINE, head=True)
    hi = [(12.8 * math.cos(a), 184 + 12.8 * math.sin(a)) for a in np.linspace(1.0, 2.2, 14)]
    poly(stroke(hi, [0.2 + 0.5 * math.sin(math.pi * i / 13) for i in range(14)]), HAIR_HI, head=True)
    for x0 in (-7.3, -1.9, 3.8, 9.2):
        poly(stroke([(x0 * 1.1, 197.5), (x0, 183.5)], 0.18), HAIR_SH, head=True)
    for side in (-1, 1):  # yuzni o'rab turgan yon sochlar
        cl = []
        for i in range(12):
            s = i / 11
            x = side * (14.9 + 1.4 * s) + wd * (2.5 if side < 0 else 9) * s ** 1.5 + 1.2 * math.sin(2.4 * t + side + s * 2) * s
            cl.append((x, 191 - 44 * s))
        poly(stroke(cl, [2.7 * (0.3 + 0.7 * clamp(i / 3)) * (1 - i / 11) ** 0.6 + 0.2 for i in range(12)]), HAIR, LINE, head=True)
    for side in (-1, 1):  # qoshlar
        poly(stroke([(side * (2.8 + 6 * u), 186.2 + 0.9 * math.sin(u * math.pi)) for u in np.linspace(0, 1, 7)], 0.22), (104, 70, 80), head=True)

    # soch lentasi
    K = (13.2, 192.8)
    poly([K, (18.4, 197.2), (18.9, 190.2)], RIBBON, LINE, head=True)
    poly([K, (10.6, 198.6), (8.9, 194.3)], RIBBON, LINE, head=True)
    for off in (0, 1.6):
        cl = [(K[0] + 0.8 + off + wd * 5 * (i / 6) + 0.8 * math.sin(3.2 * t + i + off), K[1] - 1 - 7 * i / 6) for i in range(7)]
        poly(stroke(cl, [0.8] * 6 + [0.3]), RIBBON, LINE, head=True)
    poly(ellipse(K[0], K[1], 1.4, 1.5, 12), (196, 54, 78), LINE, head=True)


def draw_eye(poly, g, side, o, lu, lv, lw):
    ex, ey = side * 6.3, 177.2

    def E(u, v):
        return (ex + side * u, ey - 1.0 + (v + 1.0) * o)

    def up(u):
        return 3.0 - 0.24 * (u - 0.4) ** 2

    def lo(u):
        return -3.4 + 0.12 * (u - 0.4) ** 2

    if o < 0.25:  # yumuq ko'z
        cl = [(ex + side * u, ey - 1.0 - 0.9 * (1 - ((u - 0.4) / 4.0) ** 2)) for u in np.linspace(-3.4, 4.4, 12)]
        poly(stroke(cl, 0.45), LASH, head=True)
        return
    us = np.linspace(-3.5, 4.3, 14)
    sclera = [E(u, up(u)) for u in us] + [E(u, lo(u)) for u in us[::-1]]
    poly(sclera, (255, 255, 255), head=True)
    poly([E(u, up(u)) for u in us] + [E(u, up(u) - 1.1) for u in us[::-1]], (206, 210, 230), head=True)
    ic_u, ic_v = 0.3 + lu, -0.4 + lv

    def ell(cu, cv, rx, ry, col):
        poly([E(cu + rx * math.cos(a), cv + ry * math.sin(a)) for a in np.linspace(0, 2 * math.pi, 24, endpoint=False)], col, head=True)

    ell(ic_u, ic_v, 2.8, 3.45, (32, 62, 98))
    ell(ic_u, ic_v - 0.9, 2.3, 2.4, (52, 112, 152))
    ell(ic_u, ic_v - 1.9, 1.8, 1.05, (112, 202, 216))
    ell(ic_u, ic_v + 0.1, 1.15, 1.6, (16, 26, 46))
    poly([E(u, up(u)) for u in us] + [E(u, up(u) - 0.8) for u in us[::-1]], (26, 44, 76), head=True)
    # iris tashqarisini teri bilan yopish
    poly([E(u, up(u) + 0.05) for u in us] + [E(4.6, 5.5), E(-3.8, 5.5)], SKIN, head=True)
    poly([E(u, lo(u) - 0.02) for u in us] + [E(4.6, -5.8), E(-3.8, -5.8)], SKIN, head=True)
    # kiprik va qovoqlar
    ul = np.linspace(-3.7, 4.4, 16)
    poly(stroke([E(u, up(u) + 0.2) for u in ul], [0.3 + 0.5 * clamp((u + 3.7) / 8.1) ** 1.5 for u in ul]), LASH, head=True)
    poly([E(4.0, up(4.0) + 0.6), E(5.5, 0.5), E(4.4, up(4.4) - 0.3)], LASH, head=True)
    poly(stroke([E(u, lo(u) - 0.05) for u in np.linspace(0.4, 3.9, 8)], 0.17), LINE, head=True)
    poly(stroke([E(u, up(u) + 1.35) for u in np.linspace(-1.4, 3.2, 8)], 0.12), (204, 146, 138), head=True)
    # yaltiroq nuqtalar
    ell(ic_u - 1.0, ic_v + 1.3, 0.95, 1.0, (255, 255, 255))
    ell(ic_u + 1.2, ic_v - 1.8, 0.45, 0.45, (255, 255, 255))


# ---------------------------------------------------------------- kadr
def render_scene(t, cam):
    cx, cy, z = cam
    hy = (HORIZON_WY - cy) * z + H / 2
    img = sky(hy, z)
    draw_background(img, t, cx, cy, z, hy)
    d = ImageDraw.Draw(img)
    draw_grass(d, t, cx, cy, z, front=False)
    draw_girl(d, t, cx, cy, z)
    draw_grass(d, t, cx, cy, z, front=True)
    return img


def draw_particles(img, t):
    d = ImageDraw.Draw(img)
    wd = wind(t)
    for i in range(NM):  # yorug'lik zarralari
        x = (M_X0[i] + 12 * t + 18 * math.sin(t * 0.4 + M_PH[i])) % W
        y = (M_Y0[i] - 6 * t + 10 * math.sin(t * 0.7 + M_PH[i])) % H
        a = 0.5 + 0.5 * math.sin(t * 2 + M_PH[i] * 3)
        r = 1.5 + 1.5 * a
        d.ellipse([x - r, y - r, x + r, y + r], fill=mix((255, 250, 210), (255, 255, 255), a))
    for i in range(NP):  # gulbarglar
        x = (P_X0[i] + P_VX[i] * t * (0.5 + wd)) % (W + 200) - 100
        y = (P_Y0[i] + P_VY[i] * t + 14 * math.sin(t * 1.3 + P_PH[i])) % (H + 100) - 50
        s = P_S[i]
        a = t * 2.2 + P_PH[i]
        sq = 0.35 + 0.65 * abs(math.sin(t * 1.7 + P_PH[i]))
        pts = []
        for k in range(10):
            th = 2 * math.pi * k / 10
            px, py = s * math.cos(th), s * 0.55 * sq * math.sin(th)
            pts.append((x + px * math.cos(a) - py * math.sin(a), y + px * math.sin(a) + py * math.cos(a)))
        d.polygon(pts, fill=P_COL[i])


def post(img, t):
    small = img.resize((W // 8, H // 8), Image.BILINEAR).filter(ImageFilter.GaussianBlur(2.5)).resize((W, H), Image.BILINEAR)
    a = np.asarray(img).astype(np.float32) / 255
    b = np.asarray(small).astype(np.float32) / 255
    out = 1 - (1 - a) * (1 - b * 0.3)            # yumshoq nur (bloom)
    out = out * 255 + LEAK
    out *= VIGNETTE
    if t < 1.5:
        out *= t / 1.5
    fade = clamp((t - 56.0) / 2.2)
    if fade > 0:
        out = out * (1 - 0.8 * fade) + 255 * 0.8 * fade
    img = Image.fromarray(out.clip(0, 255).astype(np.uint8))
    if fade > 0:
        ta = clamp((t - 57.0) / 1.2)
        over = Image.new("RGBA", (W, H), (0, 0, 0, 0))
        od = ImageDraw.Draw(over)
        col = (70, 120, 90, int(255 * ta))
        for txt, f, y in (("Yashil o'tloq", FONT_BIG, H / 2 - 40), ("~ shamol, quyosh va sokin kun ~", FONT_SM, H / 2 + 40)):
            w_ = od.textlength(txt, font=f)
            od.text(((W - w_) / 2, y - f.size / 2), txt, font=f, fill=col)
        img = Image.alpha_composite(img.convert("RGBA"), over).convert("RGB")
    return img


def render_frame(n):
    t = n / FPS
    si = max(i for i, (s, _) in enumerate(SHOTS) if t >= s)
    img = render_scene(t, cam_shot(si, t))
    if si > 0 and t < SHOTS[si][0] + XF:
        prev = render_scene(t, cam_shot(si - 1, t))
        img = Image.blend(prev, img, ease((t - SHOTS[si][0]) / XF))
    draw_particles_ss = img.resize((W, H), Image.LANCZOS)
    draw_particles(draw_particles_ss, t)
    return post(draw_particles_ss, t).tobytes()


# ---------------------------------------------------------------- musiqa
SR = 44100
BPM = 80
BEAT = 60 / BPM
BAR = BEAT * 4


def mfreq(m):
    return 440.0 * 2 ** ((m - 69) / 12)


def adsr(n, a, r, sr=SR):
    env = np.ones(n)
    na, nr = int(a * sr), int(r * sr)
    if na:
        env[:na] = np.linspace(0, 1, na)
    if nr:
        env[-nr:] *= np.linspace(1, 0, nr)
    return env


def piano(f, dur, vel):
    n = int((dur + 0.6) * SR)
    t = np.arange(n) / SR
    y = np.zeros(n)
    for k in range(1, 9):
        fk = k * f * math.sqrt(1 + 0.0004 * k * k)
        if fk > SR / 2.2:
            break
        y += (1 / k ** 1.3) * np.sin(2 * np.pi * fk * t) * np.exp(-t * (0.9 + 0.55 * k) * (f / 260) ** 0.3)
    env = adsr(n, 0.004, 0.0)
    rel = np.ones(n)
    k0 = int(dur * SR)
    rel[k0:] = np.exp(-np.arange(n - k0) / (0.12 * SR))
    return y * env * rel * vel


def flute(f, dur, vel):
    n = int((dur + 0.15) * SR)
    t = np.arange(n) / SR
    vib = 1 + 0.004 * np.sin(2 * np.pi * 5.2 * t) * np.clip((t - 0.25) / 0.3, 0, 1)
    ph = 2 * np.pi * f * np.cumsum(vib) / SR
    y = np.sin(ph) + 0.22 * np.sin(2 * ph) + 0.06 * np.sin(3 * ph)
    rng = np.random.default_rng(int(f * 10))
    y += 0.05 * np.convolve(rng.normal(0, 1, n), np.ones(20) / 20, "same")
    return y * adsr(n, 0.06, 0.16) * vel


def bell(f, dur, vel):
    n = int((dur + 2.0) * SR)
    t = np.arange(n) / SR
    y = np.zeros(n)
    for r, a, dcy in ((1, 1, 1.4), (2.76, 0.45, 3.0), (5.4, 0.25, 5.0), (8.93, 0.12, 8.0)):
        if f * r < SR / 2.2:
            y += a * np.sin(2 * np.pi * f * r * t) * np.exp(-t * dcy)
    return y * adsr(n, 0.002, 0.05) * vel


def strings(freqs, dur, vel):
    from scipy.signal import lfilter
    n = int((dur + 1.2) * SR)
    t = np.arange(n) / SR
    y = np.zeros(n)
    for f in freqs:
        for det in (-0.004, 0.0, 0.0045):
            ff = f * (1 + det)
            for k in range(1, 9):
                y += np.sin(2 * np.pi * ff * k * t + det * 100 * k) / k
    b = 1 - math.exp(-2 * math.pi * 1400 / SR)
    y = lfilter([b], [1, b - 1], y)
    y = lfilter([b], [1, b - 1], y)
    return y * adsr(n, 0.9, 1.2) * vel / (len(freqs) * 3)


def add(buf, sig, t0, pan=0.0):
    i = int(t0 * SR)
    j = min(len(buf), i + len(sig))
    if j <= i:
        return
    s = sig[: j - i]
    buf[i:j, 0] += s * math.cos((pan + 1) * math.pi / 4)
    buf[i:j, 1] += s * math.sin((pan + 1) * math.pi / 4)


CHORDS = {"G": (43, 4), "A": (45, 4), "F#m": (42, 3), "Bm": (47, 3), "Em": (40, 3), "D": (38, 4)}
PROG = ["G", "A", "F#m", "Bm"] * 2 + ["Em", "A", "D", "Bm"] + ["G", "A", "F#m", "Bm"] + ["G", "A", "D", "D"]
_A = [(71, 1), (69, .5), (71, .5), (74, 2)]
_B = [(73, 1), (74, .5), (76, .5), (69, 2)]
_C = [(78, 1.5), (76, .5), (73, 1), (69, 1)]
_D = [(74, 1), (73, .5), (71, 2.5)]
MELODY = {5: _A, 6: _B, 7: _C, 8: _D,
          9: [(67, 1), (71, 1), (76, 1), (74, 1)], 10: [(73, 1.5), (71, .5), (69, 2)],
          11: [(66, 1), (69, 1), (74, 1.5), (76, .5)], 12: [(78, 3), (None, 1)],
          13: _A, 14: _B, 15: _C, 16: _D,
          17: [(74, 1), (76, 1), (78, 1), (79, 1)], 18: [(81, 2), (79, .5), (78, .5), (76, 1)],
          19: [(78, 4)], 20: [(74, 4)]}


def make_music(path):
    from scipy.signal import fftconvolve, lfilter
    n = int((DUR + 0.5) * SR)
    buf = np.zeros((n, 2))
    rng = np.random.default_rng(3)
    for bi, name in enumerate(PROG):
        bar = bi + 1
        t0 = bi * BAR
        root, third = CHORDS[name]
        r3 = root + 12
        tones = [r3, r3 + 7, r3 + 12, r3 + 12 + third, r3 + 19, r3 + 12 + third, r3 + 12, r3 + 7]
        # pianino arpedjiosi
        steps = 8 if bar < 20 else 4
        for k in range(steps):
            dt = BEAT / 2 if bar < 20 else BEAT
            vel = (0.16 if bar <= 4 else 0.13) * (1.15 if k % 4 == 0 else 1.0) * rng.uniform(0.9, 1.05)
            add(buf, piano(mfreq(tones[k]), dt * 1.8, vel), t0 + k * dt + rng.uniform(0, 0.01), pan=-0.25)
        if bar == 20:
            add(buf, piano(mfreq(r3 + 12 + third), 3.5, 0.12), t0 + BAR * 0.99, pan=0.1)
        # bass
        if bar >= 3:
            add(buf, piano(mfreq(root), BAR, 0.22), t0, pan=0.0)
            if bar >= 13 and bar < 20:
                add(buf, piano(mfreq(root), BAR / 2, 0.14), t0 + BAR / 2, pan=0.0)
        # torli cholg'ular (pad)
        sv = 0.05 if bar <= 4 else (0.07 if bar <= 12 else 0.11)
        if bar == 20:
            sv = 0.08
        add(buf, strings([mfreq(r3), mfreq(r3 + third), mfreq(r3 + 7), mfreq(r3 + 12)], BAR * (1.6 if bar == 20 else 1.0), sv), t0, pan=0.3)
        # kuy (nay) va qo'ng'iroqchalar
        if bar in MELODY:
            b = 0.0
            for m, ln in MELODY[bar]:
                if m is not None:
                    dur_ = ln * BEAT * (1.4 if bar == 20 else 1.0)
                    add(buf, flute(mfreq(m), dur_, 0.19), t0 + b * BEAT, pan=0.15)
                    if 13 <= bar <= 19 and b % 1 == 0:
                        add(buf, bell(mfreq(m + 12), 0.2, 0.06), t0 + b * BEAT, pan=0.45)
                b += ln
        # yengil ritm
        if 13 <= bar <= 18:
            for k in range(8):
                nn = int(0.06 * SR)
                sh = rng.normal(0, 1, nn) * np.exp(-np.arange(nn) / (0.012 * SR))
                sh = np.diff(sh, prepend=0)
                add(buf, sh * (0.05 if k % 2 else 0.028), t0 + k * BEAT / 2, pan=0.35)
            for k in (0, 2):
                nn = int(0.35 * SR)
                tt = np.arange(nn) / SR
                kick = np.sin(2 * np.pi * (48 * tt + 40 * (1 - np.exp(-tt * 30)) / 30)) * np.exp(-tt * 9)
                add(buf, kick * 0.2, t0 + k * BEAT)

    # shamol shovqini va qushlar
    wn = rng.normal(0, 1, n)
    b_ = 1 - math.exp(-2 * math.pi * 500 / SR)
    wn = lfilter([b_], [1, b_ - 1], wn)
    wn = lfilter([b_], [1, b_ - 1], wn)
    tt = np.arange(n) / SR
    wenv = 0.55 + 0.25 * np.sin(tt * 0.6) + 0.12 * np.sin(tt * 1.7 + 1.0)
    buf[:, 0] += wn * wenv * 0.06
    buf[:, 1] += np.roll(wn, 3000) * wenv * 0.06
    for tb in (2.2, 6.8, 9.1, 19.5, 27.0, 41.3, 49.8, 55.5):
        for rep in range(rng.integers(2, 5)):
            nn = int(0.09 * SR)
            ct = np.arange(nn) / SR
            f0 = rng.uniform(2800, 3600)
            ch = np.sin(2 * np.pi * (f0 * ct + 9000 * ct ** 2)) * np.sin(np.pi * ct / ct[-1]) ** 2
            add(buf, ch * 0.03, tb + rep * 0.13, pan=rng.uniform(-0.8, 0.8))

    # reverb
    ir_n = int(2.4 * SR)
    ir_t = np.arange(ir_n) / SR
    wet = np.zeros_like(buf)
    for c in range(2):
        ir = np.random.default_rng(10 + c).normal(0, 1, ir_n) * np.exp(-ir_t * 2.8)
        ir = lfilter([0.3], [1, -0.7], ir)
        ir /= np.sqrt(np.sum(ir ** 2))
        wet[:, c] = fftconvolve(buf[:, c], ir)[:n]
    mixd = buf * 0.8 + wet * 0.45
    fade_in = np.clip(tt / 1.5, 0, 1)
    fade_out = np.clip((DUR - tt) / 3.0, 0, 1)
    mixd *= (fade_in * fade_out)[:, None]
    mixd = mixd[: int(DUR * SR)]
    mixd = np.tanh(mixd / np.max(np.abs(mixd)) * 1.1) * 0.9
    pcm = (mixd * 32767).astype(np.int16)
    import wave
    with wave.open(path, "wb") as wf:
        wf.setnchannels(2)
        wf.setsampwidth(2)
        wf.setframerate(SR)
        wf.writeframes(pcm.tobytes())


# ---------------------------------------------------------------- asosiy
def ffmpeg_exe():
    try:
        import imageio_ffmpeg
        return imageio_ffmpeg.get_ffmpeg_exe()
    except ImportError:
        return "ffmpeg"


def main():
    if len(sys.argv) > 2 and sys.argv[1] == "--still":
        t = float(sys.argv[2])
        img = Image.frombytes("RGB", (W, H), render_frame(int(t * FPS)))
        out = sys.argv[3] if len(sys.argv) > 3 else os.path.join(HERE, f"still_{t:05.1f}.png")
        img.save(out)
        print(out)
        return
    wav = os.path.join(HERE, "musiqa.wav")
    print("musiqa yaratilmoqda...")
    make_music(wav)
    print("kadrlar chizilmoqda...")
    ff = ffmpeg_exe()
    proc = subprocess.Popen([ff, "-y", "-loglevel", "error", "-f", "rawvideo", "-pix_fmt", "rgb24", "-s", f"{W}x{H}",
                             "-r", str(FPS), "-i", "-", "-i", wav, "-c:v", "libx264", "-preset", "medium", "-crf", "20",
                             "-pix_fmt", "yuv420p", "-c:a", "aac", "-b:a", "192k", "-shortest", "-movflags", "+faststart", OUT],
                            stdin=subprocess.PIPE)
    with Pool(os.cpu_count()) as pool:
        for i, fr in enumerate(pool.imap(render_frame, range(NFR), chunksize=4)):
            proc.stdin.write(fr)
            if i % 120 == 0:
                print(f"  {i}/{NFR}", flush=True)
    proc.stdin.close()
    proc.wait()
    print("tayyor:", OUT)


if __name__ == "__main__":
    main()
