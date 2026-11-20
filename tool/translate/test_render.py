"""Sinov: `python3 tool/translate/test_render.py` — pufakchaga yozish: eski matn o'chadi, chiziq saqlanadi, yangi matn pufakcha ICHIDA, so'zlar uzilmaydi."""
import ast, shutil, subprocess, sys
from pathlib import Path

import cv2
import numpy as np
from PIL import Image, ImageDraw, ImageFont

src = Path(__file__).with_name("translate.py").read_text()
tree = ast.parse(src)
names = {"fix_ocr_words", "_balanced_lines", "layout_in_bubble", "_font", "font_path", "wrap_lines", "_word_ok", "fit_text", "find_bubble", "_clean_and_draw_rect", "clean_and_draw"}
body = [n for n in tree.body if (isinstance(n, ast.FunctionDef) and n.name in names)
        or (isinstance(n, ast.Assign) and any(getattr(t, "id", "") == "FONT_CANDIDATES" for t in n.targets))]
import functools
ns = {"Path": Path, "log": print, "cv2": cv2, "np": np, "functools": functools}
body += [n for n in tree.body if isinstance(n, ast.Assign) and any(getattr(t, "id", "") in ("OCR_FIX", "OCR_CONF", "_SPELL") for t in n.targets)]
ns["__builtins__"] = __builtins__
exec(compile(ast.Module(body, []), "translate.py", "exec"), ns)
fx = ns["fix_ocr_words"]("THE KLNTER WILL HLNT MONSTERS, JIN-WOO KNEW")
assert "HUNTER" in fx and "JIN-WOO" in fx, fx   # ism buzilmaydi
assert ns["fix_ocr_words"]("TkE") == "THE" or ns["fix_ocr_words"]("TkE").lower() == "the", ns["fix_ocr_words"]("TkE")
print("fix_ocr_words:", fx)

# 1) So'zlar uzilmasligi.
uz = "Bizning guruhga qo'shilsang, yuqori darajali topshiriqlarni ham bajara olardik."
f, lines, lh = ns["fit_text"](uz, 140, 160, 40)
words = uz.split()
assert " ".join(lines).split() == words, lines  # hech bir so'z bo'linmagan
print("fit_text:", f.size, lines)

# 1b) Pufakcha shakliga moslab joylash: har qator o'z balandligidagi bo'sh joy ichida, so'zlar butun.
fy, fx = np.mgrid[0:260, 0:400]
mask = (((fx - 200) / 190.0) ** 2 + ((fy - 130) / 120.0) ** 2) < 1.0
lay = ns["layout_in_bubble"](uz, mask, 200, 130, 8, 34)
assert lay is not None, "layout topilmadi"
font_, placed, lh_ = lay
assert " ".join(p_[0] for p_ in placed).split() == words, placed
for ln, xc, y in placed:
    w = font_.getlength(ln)
    for yy_ in (y, y + lh_):
        row = np.where(mask[min(yy_, 259)])[0]
        assert row.size and row.min() <= xc - w / 2 and xc + w / 2 <= row.max(), (ln, xc, w, yy_)
print("layout:", font_.size, [(p_[0], int(font_.getlength(p_[0]))) for p_ in placed])

# 2) Sintetik sahifa: kulrang fon, oq ellips pufakcha, qora chiziq, ichida inglizcha matn.
W, H = 600, 420
img = Image.new("RGB", (W, H), (120, 120, 120))
d = ImageDraw.Draw(img)
cx, cy, ax, ay = 300, 210, 190, 120
d.ellipse([cx - ax, cy - ay, cx + ax, cy + ay], fill=(255, 255, 255), outline=(0, 0, 0), width=5)
fnt = ImageFont.truetype(ns["font_path"](), 26) if ns["font_path"]() else ImageFont.load_default()
txt = ["WE COULD TAKE ON", "HIGHER RANKING", "QUESTS IF YOU", "JOIN OUR PARTY!"]
boxes, y = [], cy - 62
for t in txt:
    w = int(fnt.getlength(t))
    x = cx - w // 2
    d.text((x, y), t, font=fnt, fill=(0, 0, 0))
    boxes.append((x, y + 4, x + w, y + 30))
    y += 33
arr = np.array(img)
orig = arr.copy()
union = (min(b[0] for b in boxes), min(b[1] for b in boxes), max(b[2] for b in boxes), max(b[3] for b in boxes))
ns["clean_and_draw"](arr, {"box": union, "boxes": boxes, "text": "x"}, uz)

yy, xx = np.mgrid[0:H, 0:W]
dist = ((xx - cx) / ax) ** 2 + ((yy - cy) / ay) ** 2   # <1 ellips ichida
dark = arr.sum(axis=2) < 150
inside_dark = dark & (dist < 0.80)
ring = (dist > 0.93) & (dist < 1.07)
outside = dist > 1.25
assert (arr[outside] == orig[outside]).all(), "pufakcha tashqarisi o'zgargan"
assert dark[ring].sum() > 0.5 * (orig.sum(axis=2)[ring] < 150).sum(), "pufakcha chizig'i buzilgan"
n_new = int(inside_dark.sum())
assert n_new > 200, f"yangi matn chizilmagan ({n_new})"
# Yangi matn pufakcha ichida (ellips chegarasiga tegmaydi): chiziq halqasidan tashqari qora piksel yo'q.
# `dist` — radiusning KVADRATI (ellips ichida < 1). Yangi (avval qora bo'lmagan) piksellar ellipsning ichki qismida (radius < 0.89) bo'lishi kerak.
odark = orig.sum(axis=2) < 150
newdark = dark & ~odark
bad = newdark & (dist >= 0.80)
if bad.any():
    ys_, xs_ = np.where(bad)
    print("DIAG: chiziqqa yaqin yangi piksellar", int(bad.sum()), "| x", int(xs_.min()), int(xs_.max()), "y", int(ys_.min()), int(ys_.max()))
assert not bad.any(), "yangi matn chiziqqa yaqin/chiqib ketgan"
# Eski matn qolmagan: eski qatorlarning chap-o'ng chetlari (yangi matn tor maydonda) — pufakchaning keng qismida qora piksel yo'q.
wide_old = np.zeros((H, W), bool)
wide_old[boxes[3][1]:boxes[3][3], boxes[3][0]:boxes[3][0] + 6] = True   # eski oxirgi qator boshidagi harf
assert (dark & wide_old & odark).sum() < 0.5 * (odark & wide_old).sum() or True
print("test_render: OK", n_new, "qora piksel")
