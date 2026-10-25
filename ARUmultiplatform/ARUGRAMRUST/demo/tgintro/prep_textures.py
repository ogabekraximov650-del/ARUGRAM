"""UZ: Telegram intro teksturalarini (webp, xxhdpi) xom RGBA fayllarga aylantiradi. EN: Converts the intro textures (webp, xxhdpi) into raw RGBA files. RU: Превращает текстуры интро (webp, xxhdpi) в сырые RGBA-файлы.
Ishlatish / Usage: python3 prep_textures.py <res_dir> <out_dir>"""
import os, struct, sys
from PIL import Image
res, out = sys.argv[1], sys.argv[2]
os.makedirs(out, exist_ok=True)
NAMES = ["fast_arrow_shadow", "fast_arrow", "fast_body", "fast_spiral", "ic_bubble_dot", "ic_bubble", "ic_cam_lens", "ic_cam", "ic_pencil", "ic_pin",
         "ic_smile_eye", "ic_smile", "ic_videocam", "knot_down", "knot_up", "powerful_infinity_white", "powerful_infinity", "powerful_mask",
         "powerful_star", "private_door", "private_screw", "tg_plane"]
for n in NAMES:
    im = Image.open(os.path.join(res, "drawable-xxhdpi", f"intro_{n}.webp")).convert("RGBA")
    open(os.path.join(out, n + ".rgba"), "wb").write(struct.pack("<II", *im.size) + im.tobytes())
print("tayyor", len(NAMES))
