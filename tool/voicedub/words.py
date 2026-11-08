#!/usr/bin/env python3
"""UZ: So'zlarning HAQIQIY vaqti (millisekundda) — faster-whisper word_timestamps. EN: true per-word timestamps (ms). RU: точные тайминги слов (мс).

Usage: words.py AUDIO OUT.json [LANG]    OUT.json — {"language": "en", "words": [{"w": "We", "s": 1650, "e": 1973, "p": 0.98}, ...]}
"""
import json
import os
import sys

from faster_whisper import WhisperModel


def main():
    audio, out = sys.argv[1], sys.argv[2]
    lang = sys.argv[3] if len(sys.argv) > 3 and sys.argv[3] else None
    model = WhisperModel(os.environ.get("WORDS_MODEL", "small"), device="cpu", compute_type="int8", cpu_threads=os.cpu_count() or 2)
    segments, info = model.transcribe(audio, language=lang, word_timestamps=True, vad_filter=True, beam_size=5)
    words = []
    for seg in segments:
        for w in seg.words or []:
            t = (w.word or "").strip()
            if t:
                words.append({"w": t, "s": int(round(w.start * 1000)), "e": int(round(w.end * 1000)), "p": round(float(w.probability or 0), 3)})
    json.dump({"language": info.language, "words": words}, open(out, "w", encoding="utf-8"), ensure_ascii=False)
    print(f"so'zlar: {len(words)}", flush=True)


if __name__ == "__main__":
    main()
