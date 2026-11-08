#!/usr/bin/env python3
"""UZ: NLLB variantlarini bir xil gaplarda solishtirish (inglizcha -> o'zbekcha). EN: compare NLLB variants on the same sentences. RU: сравнение вариантов NLLB."""
import os
import sys
import time

import torch
from transformers import AutoModelForSeq2SeqLM, AutoTokenizer

SENT = [
    "We begin this year and this administration, in the midst of an unprecedented crisis that calls for unprecedented action.",
    "Just this week, we saw more people file for unemployment than at any time in the last 26 years.",
    "And experts agree that if nothing is done, the unemployment rate could reach double digits.",
    "Our economy could fall a trillion dollars short of its full capacity,",
    "which translates into more than $12,000 in lost income for a family of four.",
]
MODELS = os.environ.get("MODELS", "facebook/nllb-200-distilled-600M,facebook/nllb-200-distilled-1.3B,facebook/nllb-200-1.3B").split(",")
torch.set_num_threads(os.cpu_count() or 2)
for repo in MODELS:
    t0 = time.time()
    tok = AutoTokenizer.from_pretrained(repo, src_lang="eng_Latn")
    model = AutoModelForSeq2SeqLM.from_pretrained(repo, torch_dtype=torch.float32).eval()
    print(f"\n=== {repo} (yuklandi {time.time() - t0:.0f}s)", flush=True)
    t1 = time.time()
    for s in SENT:
        enc = tok(s, return_tensors="pt")
        with torch.no_grad():
            out = model.generate(**enc, forced_bos_token_id=tok.convert_tokens_to_ids("uzn_Latn"), num_beams=4, max_new_tokens=128)
        print("  EN:", s)
        print("  UZ:", tok.batch_decode(out, skip_special_tokens=True)[0], flush=True)
    print(f"  (tarjima {time.time() - t1:.0f}s)", flush=True)
    del model, tok
