#!/usr/bin/env python3
"""UZ: Gaplarni NLLB bilan tarjima qiladi (VoiceStudio venv'ida). EN: translate segments with NLLB. RU: перевод сегментов через NLLB.

Usage: translate.py IN.json OUT.json SRC_LANG TARGET_LANG
  IN.json  — [{"text": "..."}, ...]    OUT.json — ["tarjima", ...]
Model: env NLLB_MODEL (standart facebook/nllb-200-distilled-1.3B; sinovda 600M'dan yaxshi chiqdi).
"""
import json
import os
import sys

import torch
from transformers import AutoModelForSeq2SeqLM, AutoTokenizer

FLORES = {
    "en": "eng_Latn", "uz": "uzn_Latn", "ru": "rus_Cyrl", "tr": "tur_Latn", "es": "spa_Latn", "de": "deu_Latn", "fr": "fra_Latn",
    "it": "ita_Latn", "pt": "por_Latn", "ar": "arb_Arab", "zh": "zho_Hans", "ja": "jpn_Jpan", "ko": "kor_Hang", "hi": "hin_Deva",
    "fa": "pes_Arab", "kk": "kaz_Cyrl", "ky": "kir_Cyrl", "tg": "tgk_Cyrl", "az": "azj_Latn", "tk": "tuk_Latn", "uk": "ukr_Cyrl",
    "pl": "pol_Latn", "nl": "nld_Latn", "id": "ind_Latn", "vi": "vie_Latn", "th": "tha_Thai", "bn": "ben_Beng", "ur": "urd_Arab",
}


def main():
    src_json, out_json, src, tgt = sys.argv[1:5]
    repo = os.environ.get("NLLB_MODEL", "facebook/nllb-200-distilled-1.3B")
    fs, ft = FLORES.get(src.lower(), "eng_Latn"), FLORES.get(tgt.lower())
    if not ft:
        raise SystemExit(f"NLLB: noma'lum maqsad til kodi: {tgt}")
    segs = json.load(open(src_json, encoding="utf-8"))
    torch.set_num_threads(os.cpu_count() or 2)
    tok = AutoTokenizer.from_pretrained(repo, src_lang=fs)
    model = AutoModelForSeq2SeqLM.from_pretrained(repo, torch_dtype=torch.float32).eval()
    out = []
    for i, s in enumerate(segs):
        text = (s.get("text") or "").strip()
        if not text:
            out.append("")
            continue
        enc = tok(text, return_tensors="pt", truncation=True, max_length=256)
        with torch.no_grad():
            gen = model.generate(**enc, forced_bos_token_id=tok.convert_tokens_to_ids(ft), num_beams=4, max_new_tokens=200)
        out.append(tok.batch_decode(gen, skip_special_tokens=True)[0].strip())
        print(f"PROGRESS {i + 1}/{len(segs)}", flush=True)
    json.dump(out, open(out_json, "w", encoding="utf-8"), ensure_ascii=False, indent=1)


if __name__ == "__main__":
    main()
