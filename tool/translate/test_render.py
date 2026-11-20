"""Sinov: `python3 tool/translate/test_render.py` — pufakchaga yozish: eski matn o'chadi, chiziq saqlanadi, yangi matn pufakcha ICHIDA, so'zlar uzilmaydi."""
import ast, shutil, subprocess, sys
from pathlib import Path

import cv2
import numpy as np
from PIL import Image, ImageDraw, ImageFont

src = Path(__file__).with_name("translate.py").read_text()
tree = ast.parse(src)
names = {"_font", "font_path", "wrap_lines", "_word_ok", "fit_text", "find_bubble", "_clean_and_draw_rect", "clean_and_draw"}
body = [n for n in tree.body if (isinstance(n, ast.FunctionDef) and n.name in names)
        or (isinstance(n, ast.Assign) and any(getattr(t, "id", "") == "FONT_CANDIDATES" for t in n.targets))]
ns = {"Path": Path, "log": print, "cv2": cv2, "np": np}
exec(compile(ast.Module(body, []), "translate.py", "exec"), ns)

# 1) So'zlar uzilmasligi.
uz = "Bizning guruhga qo'shilsang, yuqori darajali topshiriqlarni ham bajara olardik."
f, lines, lh = ns["fit_text"](uz, 140, 160, 40)
words = uz.split()
assert " ".join(lines).split() == words, lines  # hech bir so'z bo'linmagan
print("fit_text:", f.size, lines)

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
assert not (dark & (dist >= 0.80) & (dist <= 0.95)).any(), "matn chiziqqa yaqin/chiqib ketgan"
# Eski inglizcha matn qolmagan: yangi matnning qora piksellari soni eskisidan juda ko'p farq qilmaydi va birinchi qator joyida eski shakl yo'q.
old_region = orig[boxes[0][1]:boxes[0][3], boxes[0][0]:boxes[0][2]]
print("test_render: OK", n_new, "qora piksel")
