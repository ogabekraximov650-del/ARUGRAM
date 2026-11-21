"""Sinov (sun'iy intellektsiz usullar): davriy fon tiklash, qora/rangli narrator ramkasi, OCR topmagan pufakchani topish."""
import ast, functools
from pathlib import Path

import cv2
import numpy as np
from PIL import Image, ImageDraw, ImageFont

src = Path(__file__).with_name("translate.py").read_text()
tree = ast.parse(src)
names = {"_med1d", "smooth_fill", "_ocr_candidates", "merge_same_bubble", "inpaint_texture", "inpaint_mask", "find_missed", "find_bubble", "clean_and_draw", "_clean_and_draw_rect", "layout_in_bubble",
         "_balanced_lines", "_font", "font_path", "wrap_lines", "_word_ok", "fit_text"}
body = [n for n in tree.body if (isinstance(n, ast.FunctionDef) and n.name in names)
        or (isinstance(n, ast.Assign) and any(getattr(t, "id", "") == "FONT_CANDIDATES" for t in n.targets))]
import re
ns = {"Path": Path, "log": print, "cv2": cv2, "np": np, "functools": functools, "re": re}
exec(compile(ast.Module(body, []), "translate.py", "exec"), ns)

# 1) Davriy fon (yarim ton): davriy usul Telea'dan aniqroq bo'lishi kerak.
for name, pat in (("nuqta-to'r 6px", lambda x, y: ((x % 6) < 2) & ((y % 6) < 2)),
                  ("diagonal to'r 8px", lambda x, y: ((x + y) % 8) < 2)):
    h = w = 140
    yy, xx = np.mgrid[0:h, 0:w]
    gt = np.where(pat(xx, yy), 40, 225).astype(np.uint8)
    gt = np.stack([gt] * 3, axis=2)
    img = Image.fromarray(gt.copy())
    d = ImageDraw.Draw(img)
    f = ImageFont.truetype(ns["font_path"](), 18) if ns["font_path"]() else ImageFont.load_default()
    d.text((20, 60), "HORRIFYING?", font=f, fill=(0, 0, 0))
    noisy = np.array(img)
    mask = (noisy.astype(int) != gt.astype(int)).any(axis=2)
    mask = cv2.dilate(mask.astype(np.uint8), np.ones((5, 5), np.uint8)).astype(bool)
    r = ns["inpaint_texture"](noisy, mask)
    assert r is not None, f"{name}: davr topilmadi"
    out, left = r
    tel = cv2.inpaint(noisy, mask.astype(np.uint8) * 255, 3, cv2.INPAINT_TELEA)
    e_per = np.abs(out.astype(int) - gt.astype(int))[mask & ~left].mean()
    e_tel = np.abs(tel.astype(int) - gt.astype(int))[mask].mean()
    print(f"{name}: davriy xato {e_per:.1f} | Telea xato {e_tel:.1f} | qolgan {int(left.sum())} piksel")
    assert e_per < e_tel, (name, e_per, e_tel)

# 2) To'q (qora) narrator ramkasi: oq matn, yangi matn ham oq.
img = Image.new("RGB", (420, 220), (150, 150, 150))
d = ImageDraw.Draw(img)
d.rectangle([60, 70, 360, 150], fill=(30, 30, 30), outline=(0, 0, 0), width=3)
f = ImageFont.truetype(ns["font_path"](), 22) if ns["font_path"]() else ImageFont.load_default()
d.text((100, 95), "THE NEXT DAY", font=f, fill=(255, 255, 255))
arr = np.array(img)
orig = arr.copy()
box = (100, 97, 100 + int(f.getlength("THE NEXT DAY")), 120)
ns["clean_and_draw"](arr, {"box": box, "boxes": [box], "text": "THE NEXT DAY"}, "Ertasi kuni")
outside = np.ones(arr.shape[:2], bool)
outside[60:160, 50:370] = False
assert (arr[outside] == orig[outside]).all(), "ramka tashqarisi o'zgargan"
inner = arr[78:142, 68:352]
white_new = int((inner.sum(axis=2) > 600).sum())
assert white_new > 150, f"oq matn chizilmagan ({white_new})"
print("to'q ramka: OK", white_new)

# 3) OCR topmagan pufakcha.
img = Image.new("RGB", (600, 420), (120, 120, 120))
d = ImageDraw.Draw(img)
d.ellipse([150, 100, 450, 320], fill=(255, 255, 255), outline=(0, 0, 0), width=5)
f = ImageFont.truetype(ns["font_path"](), 28) if ns["font_path"]() else ImageFont.load_default()
for i, t in enumerate(["HOW DID THIS", "HAPPEN...?"]):
    w_ = int(f.getlength(t))
    d.text((300 - w_ // 2, 170 + i * 40), t, font=f, fill=(0, 0, 0))
arr = np.array(img)
found = ns["find_missed"](arr, [], None)
assert len(found) == 1, f"topilmagan pufakcha soni: {len(found)}"
b = found[0]["box"]
assert 150 < b[0] and b[2] < 450 and 100 < b[1] and b[3] < 320, b
none = ns["find_missed"](arr, [{"box": b}], None)
assert none == [], "qoplangan pufakcha qayta topildi"
print("find_missed: OK", b)
# 4) Gradient fonda yaltiroq (rangli) yozuv: butunlay o'chadi (rangli piksellar qolmaydi).
H, W = 220, 420
yy, xx = np.mgrid[0:H, 0:W]
grad = np.clip(yy / H * 255, 0, 255).astype(np.uint8)
img = Image.fromarray(np.stack([grad, grad, grad], axis=2))
d = ImageDraw.Draw(img)
f = ImageFont.truetype(ns["font_path"](), 40) if ns["font_path"]() else ImageFont.load_default()
d.text((120, 90), "GATE", font=f, fill=(40, 90, 255))
arr = np.array(img)
orig = arr.copy()
bx = (120, 95, 120 + int(f.getlength("GATE")), 135)
ns["clean_and_draw"](arr, {"box": bx, "boxes": [bx], "text": "GATE"}, "Darvoza")
blue = (arr[..., 2].astype(int) - arr[..., 0].astype(int)) > 80
if blue.any():
    ys_, xs_ = np.where(blue)
    print("DIAG: rangli qoldiq", int(blue.sum()), "x", int(xs_.min()), int(xs_.max()), "y", int(ys_.min()), int(ys_.max()), "| quti", bx,
          "| asl matn piksellari", int(((orig[..., 2].astype(int) - orig[..., 0].astype(int)) > 80).sum()))
assert int(blue.sum()) == 0, f"rangli yozuv qoldigi: {int(blue.sum())}"
print("gradient: OK")

# 5) Bir pufakchada ikki OCR guruhi: tarjimalar birlashadi.
img = Image.new("RGB", (420, 220), (150, 150, 150))
d = ImageDraw.Draw(img)
d.rectangle([40, 50, 380, 170], fill=(30, 30, 30))
f = ImageFont.truetype(ns["font_path"](), 22) if ns["font_path"]() else ImageFont.load_default()
d.text((70, 70), "YOUR DAILY QUESTS", font=f, fill=(255, 255, 255))
d.text((70, 120), "ACCEPT?", font=f, fill=(255, 255, 255))
arr = np.array(img)
g1 = {"box": (70, 72, 70 + int(f.getlength("YOUR DAILY QUESTS")), 96), "boxes": [(70, 72, 250, 96)], "text": "a"}
g2 = {"box": (70, 122, 70 + int(f.getlength("ACCEPT?")), 146), "boxes": [(70, 122, 150, 146)], "text": "b"}
mg, mu = ns["merge_same_bubble"](arr, [g1, g2], ["Kunlik topshiriq", "Qabul qilasizmi?"])
assert len(mg) == 1 and mu[0] == "Kunlik topshiriq Qabul qilasizmi?", (len(mg), mu)
# 5b) Pufakcha topilmasa ham yaqin ikki qator (tekis fonda) birlashadi; orada chiziq bo'lsa — birlashmaydi.
img2 = np.full((300, 420, 3), 90, np.uint8)
ga = {"box": (60, 60, 300, 90), "boxes": [(60, 60, 300, 90)], "text": "a"}
gb = {"box": (60, 105, 200, 135), "boxes": [(60, 105, 200, 135)], "text": "b"}
mg2, mu2 = ns["merge_same_bubble"](img2, [ga, gb], ["Birinchi", "Ikkinchi"])
assert len(mg2) == 1 and mu2[0] == "Birinchi Ikkinchi", (len(mg2), mu2)
img3 = img2.copy()
img3[93:103, 40:360] = 0   # orada qora chiziq
mg3, mu3 = ns["merge_same_bubble"](img3, [ga, gb], ["Birinchi", "Ikkinchi"])
assert len(mg3) == 2, len(mg3)
print("merge_same_bubble: OK")
# 6) Coons silliq to'ldirish: vertikal gradient aniq tiklanadi (xato <= 3).
H, W = 200, 300
yy, xx = np.mgrid[0:H, 0:W]
g = np.clip(yy * 255 / H, 0, 255).astype(np.uint8)
gt = np.stack([g, g, g], axis=2)
arr = gt.copy()
arr[60:110, 80:220] = (10, 200, 50)
ns["smooth_fill"](arr, 80, 60, 219, 109)
err = np.abs(arr.astype(int) - gt.astype(int))[60:110, 80:220].mean()
assert err < 3.0, f"Coons xatosi katta: {err:.2f}"
print(f"smooth_fill: OK (xato {err:.2f})")
print("test_classic: OK")
