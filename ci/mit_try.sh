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
        for i, n in enumerate(sorted(z.namelist())[:5]):
            open(f"mit-out/in/{ch}_{i}.jpg", "wb").write(z.read(n))
print(os.listdir("mit-out/in"))
PY
cd "$MIT_DIR"
PY="$MIT_PY"
echo "=== CONFIG-HELP"
$PY -m manga_translator config-help 2>&1 | grep -v onnxruntime | head -230 | cut -c1-200
cat > "$ROOT/mit-out/cfg.json" <<'JSON'
{"translator": {"translator": "original", "target_lang": "ENG", "no_text_lang_skip": true},
 "detector": {"detector": "default", "detection_size": 2048},
 "inpainter": {"inpainter": "lama_large", "inpainting_size": 2048}}
JSON
echo "=== RUN (prep-manual + saqlash)"
$PY -m manga_translator local -i "$ROOT/mit-out/in" -o "$ROOT/mit-out/out" --prep-manual --save-text --config-file "$ROOT/mit-out/cfg.json" 2>&1 | grep -v onnxruntime | tail -60 | cut -c1-220
echo "=== OUT"
find "$ROOT/mit-out/out" -type f | head -30
