#!/usr/bin/env python3
# ci/translate_test.py — SINOV: `ci/testdata/*.zip` (bob sahifalari) ni to'liq avto-rejimda o'tkazadi (OCR -> Workers AI tarjima -> rasmga yozish)
# va natijani `translate-test-out/` ga (artifact) saqlaydi: <bob>/result/*.jpg, <bob>/numbered/*.jpg, <bob>/texts.json. Maqsad — natijani KO'ZDAN kechirib xatolarni topish.
import importlib.util, json, os, shutil, sys, time, zipfile
from pathlib import Path

spec = importlib.util.spec_from_file_location("tr", "tool/translate/translate.py")
tr = importlib.util.module_from_spec(spec)
spec.loader.exec_module(tr)

only = (open("ci/TRANSLATE_TEST").read().split() if os.path.exists("ci/TRANSLATE_TEST") else [])
only = [x for x in only if x and not x.startswith("#")]
OUT = Path("translate-test-out")
shutil.rmtree(OUT, ignore_errors=True)
OUT.mkdir()
source = "english"
reader = tr.make_reader(source)
import numpy as np
from PIL import Image

for zp in sorted(Path("ci/testdata").glob("*.zip")):
    ch = zp.stem
    if only and ch not in only:
        continue
    t0 = time.time()
    work = OUT / ch
    (work / "src").mkdir(parents=True)
    paths = []
    with zipfile.ZipFile(zp) as z:
        for n in sorted(z.namelist()):
            p = work / "src" / n
            p.write_bytes(z.read(n))
            paths.append(p)
    groups = []
    for i, p in enumerate(paths, 1):
        img = np.array(Image.open(p).convert("RGB"))
        g = tr.group_dets(tr.ocr_image(reader, img), source)
        try:
            extra = tr.find_missed(img, g, reader)
            g = sorted(g + extra, key=lambda x: (x["box"][1], x["box"][0]))
        except Exception as e:  # noqa: BLE001
            print("find_missed xato:", e)
        groups.append(g)
        print(f"{ch} sahifa {i}: {len(g)} blok ({time.time() - t0:.0f}s)", flush=True)
    flat = [g["text"] for page in groups for g in page]
    uz_flat = tr.translate_texts([t if t.strip() else "" for t in flat], source) if flat else []
    uz_flat = [u if t.strip() else "" for t, u in zip(flat, uz_flat)]
    it = iter(uz_flat)
    uz_pages = [[next(it) for _ in page] for page in groups]
    (work / "result").mkdir()
    (work / "numbered").mkdir()
    tr.render_all(paths, groups, uz_pages, work / "result")
    tr.annotate_pages(paths, groups, work / "numbered")
    shutil.rmtree(work / "src")
    (work / "texts.json").write_text(json.dumps(
        [[{"n": f"{i + 1}.{j}", "en": g["text"], "uz": uz_pages[i][j - 1], "box": list(map(int, g["box"]))} for j, g in enumerate(page, 1)]
         for i, page in enumerate(groups)], ensure_ascii=False, indent=1))
    print(f"✅ {ch}: {len(paths)} sahifa, {len(flat)} blok, {time.time() - t0:.0f}s", flush=True)
