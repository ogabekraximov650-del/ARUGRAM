#!/usr/bin/env python3
"""uzcoders-lottie natijasini rlottie (Telegram'ning o'zi ishlatadigan chizuvchi) bilan solishtiradi.

Ishlatish: compare.py <tgs_papka> <bizning_png_papka> <o'lcham> [hisobot_papkasi]
Bizning PNG'lar: `render_dir` yaratadi (`<nom>_0/1/2.png`, kadrlar 0%, 33%, 66%).
Chiqish: umumiy statistika, eng yomon fayllar, rad etilgan fayllar sabablari; `sheet.png` — yonma-yon taqqoslash.
Meyor: bir pikselda 3 kanaldan eng kattasi farqi 40 dan oshsa — "farq" piksel.
"""

import gzip
import io
import json
import statistics
import sys
from pathlib import Path

import numpy as np
from PIL import Image
from rlottie_python import LottieAnimation

FRACS = (0.0, 0.33, 0.66)


def comp(img: Image.Image) -> np.ndarray:
    a = np.asarray(img.convert("RGBA")).astype(float)
    al = a[..., 3:4] / 255.0
    return a[..., :3] * al + 128 * (1 - al)


def main():
    tgs_dir, mine_dir, size = Path(sys.argv[1]), Path(sys.argv[2]), int(sys.argv[3])
    rep = Path(sys.argv[4]) if len(sys.argv) > 4 else Path("report")
    rep.mkdir(parents=True, exist_ok=True)
    rows, missing, sheet = [], [], []
    for f in sorted(tgs_dir.glob("*.tgs")):
        mine = [mine_dir / f"{f.stem}_{k}.png" for k in range(3)]
        if not all(m.exists() for m in mine):
            missing.append(f.stem)
            continue
        try:
            anim = LottieAnimation.from_tgs(str(f))
            total = anim.lottie_animation_get_totalframe()
        except Exception as e:  # rlottie ham ocha olmadi
            print(f"rlottie ochmadi {f.stem}: {e}")
            continue
        for k, fr in enumerate(FRACS):
            ref = anim.render_pillow_frame(frame_num=min(int(total * fr), total - 1), width=size, height=size)
            m = Image.open(mine[k]).convert("RGBA")
            if m.size != ref.size:
                m = m.resize(ref.size)
            A, B = comp(ref), comp(m)
            d = np.abs(A - B).max(axis=2)
            rows.append((f.stem, k, float(d.mean()), float((d > 40).mean() * 100)))
            if k == 1 and len(sheet) < 48:
                sheet.append((f.stem, ref, m, d))
    print(f"solishtirildi: {len(rows)} kadr ({len(rows) // 3} fayl); bizda chizilmadi: {len(missing)}")
    if rows:
        print(f"o'rtacha farq (0..255): median {statistics.median(r[2] for r in rows):.3f}, "
              f"farqli piksellar % : median {statistics.median(r[3] for r in rows):.3f}")
        for thr in (1, 3, 5, 10):
            print(f">{thr}% farqli piksel: {sum(1 for r in rows if r[3] > thr)} kadr")
        print("ENG YOMON 15:")
        for r in sorted(rows, key=lambda r: -r[3])[:15]:
            print(f"  {r[0]} kadr{r[1]}: o'rtacha {r[2]:.2f}, farqli {r[3]:.2f}%")
    if sheet:
        S = 128
        w = Image.new("RGB", (S * 3, S * len(sheet)), (60, 60, 60))
        for i, (_, ref, m, d) in enumerate(sheet):
            for c, im in enumerate((ref, m)):
                bg = Image.new("RGBA", im.size, (128, 128, 128, 255))
                bg.alpha_composite(im.convert("RGBA"))
                w.paste(bg.convert("RGB").resize((S, S)), (S * c, S * i))
            dd = Image.fromarray((d * 3).clip(0, 255).astype("uint8")).convert("RGB").resize((S, S))
            w.paste(dd, (S * 2, S * i))
        w.save(rep / "sheet.png")
        (rep / "sheet_names.txt").write_text("\n".join(n for n, *_ in sheet))
    (rep / "rows.json").write_text(json.dumps(rows))
    if missing:
        print("bizda chizilmaganlar:", ", ".join(missing[:40]))


if __name__ == "__main__":
    main()
