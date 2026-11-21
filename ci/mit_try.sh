#!/usr/bin/env bash
# ci/mit_try.sh — SINOV: zyddnys/manga-image-translator (GPL-3.0) ni ALOHIDA dastur sifatida ishga tushirib, faqat matn topish + LaMa bilan TOZALASH sifatini ko'rish.
# Tarjima o'chirilgan (matnni o'zimiz qo'lda yozamiz). Natija `mit-out/` (artifact). Hech narsa bizning kodga qo'shilmaydi.
set +e
ROOT=$(pwd)
mkdir -p mit-out/in mit-out/out
python3 - <<'PY'
import zipfile, glob, os
for zp in sorted(glob.glob("ci/testdata/*.zip")):
    ch = os.path.basename(zp)[:-4]
    if ch not in open("ci/TRANSLATE_TEST").read():
        continue
    with zipfile.ZipFile(zp) as z:
        for i, n in enumerate(sorted(z.namelist())[:3]):
            open(f"mit-out/in/{ch}_{i}.jpg", "wb").write(z.read(n))
print(os.listdir("mit-out/in"))
PY
git clone --depth 1 https://github.com/zyddnys/manga-image-translator mit
cd mit
python3 -m venv ../mitenv
../mitenv/bin/pip install -q --upgrade pip
../mitenv/bin/pip install -q torch torchvision --index-url https://download.pytorch.org/whl/cpu
../mitenv/bin/pip install -r requirements.txt 2>&1 | tail -15
echo "=== HELP"
../mitenv/bin/python -m manga_translator --help 2>&1 | head -80
echo "=== LOCAL HELP"
../mitenv/bin/python -m manga_translator local --help 2>&1 | head -80
echo "=== RUN"
../mitenv/bin/python -m manga_translator local -i "$ROOT/mit-out/in" -o "$ROOT/mit-out/out" --translator none --inpainter lama_large --use-gpu=false 2>&1 | tail -60
echo "=== OUT"
ls -la "$ROOT/mit-out/out" | head
