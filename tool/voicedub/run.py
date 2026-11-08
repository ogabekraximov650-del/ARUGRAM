#!/usr/bin/env python3
"""UZ: VoiceStudio backend (HTTP API) orqali dublyaj: video -> transkripsiya -> o'zbekchaga tarjima -> asl ovozni klonlab ovozlash -> videoga qo'yish.
EN: Drives the VoiceStudio backend over HTTP: ingest -> transcribe (with speaker voice clones) -> translate -> generate -> mux.
RU: Управляет бэкендом VoiceStudio по HTTP: загрузка -> транскрипция (с клонами голосов) -> перевод -> озвучка -> сборка.

Usage: run.py VIDEO TARGET_LANG OUTDIR
"""
import http.client
import json
import os
import sys
import time
import urllib.parse

API = os.environ.get("VS_API", "http://127.0.0.1:8000")
T0 = time.time()


def log(msg):
    print(f"[{time.time() - T0:7.1f}s] {msg}", flush=True)


def conn():
    u = urllib.parse.urlparse(API)
    return http.client.HTTPConnection(u.hostname, u.port or 80, timeout=7200)


def call(method, path, body=None, files=None, raw=False):
    c = conn()
    headers = {}
    payload = None
    if files:
        boundary = "----vsdub%d" % int(time.time() * 1000)
        parts = []
        for k, v in (body or {}).items():
            parts.append(f'--{boundary}\r\nContent-Disposition: form-data; name="{k}"\r\n\r\n{v}\r\n'.encode())
        for k, (fn, data, ct) in files.items():
            parts.append(f'--{boundary}\r\nContent-Disposition: form-data; name="{k}"; filename="{fn}"\r\nContent-Type: {ct}\r\n\r\n'.encode() + data + b"\r\n")
        parts.append(f"--{boundary}--\r\n".encode())
        payload = b"".join(parts)
        headers["Content-Type"] = f"multipart/form-data; boundary={boundary}"
    elif body is not None:
        payload = json.dumps(body).encode()
        headers["Content-Type"] = "application/json"
    c.request(method, path, payload, headers)
    r = c.getresponse()
    data = r.read()
    if r.status >= 400:
        raise RuntimeError(f"{method} {path} -> {r.status}: {data[:600].decode(errors='replace')}")
    if raw:
        return data
    try:
        return json.loads(data)
    except Exception:
        return data


def sse(path):
    """Yield (event_name, data_dict) from an SSE endpoint. Qator-qator o'qiladi: voqea keldi — darhol qaytadi (jonli foiz uchun)."""
    c = conn()
    c.request("GET", path)
    r = c.getresponse()
    if r.status >= 400:
        raise RuntimeError(f"GET {path} -> {r.status}: {r.read()[:400]!r}")
    name, data = None, None
    while True:
        line = r.readline()
        if not line:
            break
        line = line.rstrip(b"\r\n")
        if line.startswith(b"event:"):
            name = line[6:].strip().decode()
        elif line.startswith(b"data:"):
            try:
                data = json.loads(line[5:].strip())
            except Exception:
                data = {"raw": line[5:].decode(errors="replace")}
        elif line == b"":
            if data is not None:
                yield (name or (data.get("type") if isinstance(data, dict) else None) or "message"), data
            name, data = None, None


def prog(pct, text):
    """Umumiy jarayon foizi (0..100) — Actions skripti (`tool/dub/dub.py`) shu qatorlarni o'qib botdagi xabarni yangilaydi."""
    print(f"PROGRESS {max(0.0, min(100.0, pct)):.1f}\t{text}", flush=True)


def safe_name(speaker_id):
    out = []
    for ch in speaker_id.lower():
        if ch.isalnum():
            out.append(ch)
        elif ch in (" ", "-"):
            out.append("_")
    return "".join(out) or "speaker"


def main():
    video, target, outdir = sys.argv[1], sys.argv[2], sys.argv[3]
    os.makedirs(outdir, exist_ok=True)

    # 0. Model litsenziyalari (sinov uchun ephemeral muhitda qabul qilinadi; OmniVoice va NLLB vaznlari CC-BY-NC — faqat shaxsiy sinov)
    for repo in os.environ.get("VS_MODELS", "k2-fsa/OmniVoice,eustlb/higgs-audio-v2-tokenizer,Systran/faster-whisper-small").split(","):
        try:
            st = call("GET", "/models/licenses/acceptance/" + repo)
            log(f"litsenziya {repo}: {st.get('state')} (kerak={st.get('required')}, litsenziya={st.get('license')})")
            if st.get("required") and not st.get("accepted"):
                r = call("POST", "/models/licenses/accept", {"repo_id": repo, "fingerprint": st["fingerprint"], "accepted": True})
                log(f"  qabul qilindi: {r.get('state')}")
        except Exception as e:  # noqa: BLE001
            log(f"  litsenziya {repo}: {e}")

    # 1. Yuklash
    data = open(video, "rb").read()
    log(f"yuklash: {video} ({len(data) / 1e6:.1f} MB)")
    prog(1, "Video VoiceStudio'ga yuklanmoqda")
    res = call("POST", "/dub/upload", files={"video": (os.path.basename(video), data, "video/mp4")}, body={"input_type": "video"})
    job_id, task_id = res["job_id"], res["task_id"]
    log(f"job={job_id} task={task_id}")

    # 2. Tayyorlash (ovoz ajratish, Demucs)
    for name, ev in sse(f"/tasks/stream/{task_id}"):
        t = ev.get("type", name)
        if t in ("extract_start", "demucs_start", "scene_start", "demucs_done", "scene_done", "cached", "warning"):
            log(f"  prep: {t} {json.dumps(ev)[:160] if t == 'warning' else ''}")
        if t == "extract_start":
            prog(3, "Ovoz ajratib olinmoqda")
        elif t == "demucs_start":
            prog(4, "Ovoz va fon (musiqa) ajratilmoqda")
        elif t == "demucs_progress":
            prog(4 + 6 * float(ev.get("percent") or 0) / 100, f"Ovoz va fon ajratilmoqda {float(ev.get('percent') or 0):.0f}%")
        if t == "error":
            raise SystemExit(f"prep xato: {json.dumps(ev)[:800]}")
        if t == "ready":
            log(f"  prep tayyor: davomiyligi {ev.get('duration')} s")
            prog(10, "Tayyorlandi")
            break

    # 3. Transkripsiya + spiker ovozlari (klon namunalari)
    log("transkripsiya (ASR + diarizatsiya + ovoz namunalari)...")
    prog(11, "Nutq matnga aylantirilmoqda (Whisper)")
    final = None
    for name, ev in sse(f"/dub/transcribe-stream/{job_id}"):
        if name in ("start", "ping", "segments"):
            if name == "start":
                log(f"  start: {json.dumps(ev)[:160]}")
            elif name == "segments":
                prog(17, "Gaplar ajratilmoqda, spikerlar aniqlanmoqda")
            continue
        if name in ("warning", "error"):
            log(f"  {name}: {json.dumps(ev)[:700]}")
        if name == "error":
            raise SystemExit("transkripsiya xato: " + json.dumps(ev)[:600])
        if name == "final":
            final = ev
        if name == "done":
            break
    if not final:
        raise SystemExit("transkripsiya natija bermadi")
    segs = final["segments"]
    src_lang = final.get("source_lang")
    if os.environ.get("VS_TRANSCRIBE_ONLY") == "1":
        # Faqat transkripsiya (qo'lda tarjima / bo'laklab ishlash): gaplar va ularning vaqti saqlanadi, tarjima va ovozlash yo'q.
        json.dump({"segments": [{k: s.get(k) for k in ("id", "start", "end", "text", "speaker_id")} for s in segs],
                   "source_lang": src_lang, "duration": None}, open(os.path.join(outdir, "transcript.json"), "w"), ensure_ascii=False, indent=1)
        prog(100, f"Transkripsiya tayyor: {len(segs)} ta gap")
        log("TRANSKRIPSIYA TUGADI")
        return
    prog(22, f"Matn tayyor: {len(segs)} ta gap, til: {src_lang}")
    log(f"  {len(segs)} segment, manba til={src_lang}, spikerlar={[s.get('speaker_id') for s in segs][:8]}")
    log(f"  klon manbalari: {json.dumps(final.get('cast_sources'))[:300]}")
    for s in segs:
        log(f"   [{s['start']:.1f}-{s['end']:.1f}] {s.get('speaker_id')}: {s.get('text')}")

    # 4. Tarjima (o'zbekcha). Tayyor tarjima fayli bo'lsa (VS_TRANSLATIONS) — shu ishlatiladi (qo'lda yozilgan, vaqtga sig'adigan),
    # aks holda VoiceStudio tarjimoni (VS_TRANSLATE, standart nllb).
    pre = None
    sf = os.environ.get("VS_SEGMENTS")   # tayyor gaplar (vaqt + MATN): [{"start","end","text","speaker_id"?}] — ASR gaplari o'rniga ishlatiladi
    if sf and os.path.exists(sf):
        given = json.load(open(sf, encoding="utf-8"))
        log(f"tayyor gaplar ishlatilmoqda: {len(given)} ta ({sf})")
        segs = [{"id": i, "start": float(g["start"]), "end": float(g["end"]), "text": g["text"],
                 "speaker_id": g.get("speaker_id") or (segs[0].get("speaker_id") if segs else None) or "Speaker 1"} for i, g in enumerate(given)]
        pre = [g["text"] for g in given]
    tf = os.environ.get("VS_TRANSLATIONS")
    if pre is None and tf and os.path.exists(tf):
        pre = json.load(open(tf, encoding="utf-8"))
        if len(pre) != len(segs):
            log(f"  tayyor tarjima soni mos emas ({len(pre)} != {len(segs)}) — VoiceStudio tarjimoniga o'tiladi")
            pre = None
    tr = {}
    merged = []
    if pre is not None:
        log(f"tayyor tarjima ishlatilmoqda ({tf})")
        for s, txt in zip(segs, pre):
            log(f"   -> {txt}")
            merged.append({**s, "text": txt})
    else:
        log(f"tarjima -> {target} ...")
        prog(23, f"{target} tiliga tarjima qilinmoqda")
        tr = call("POST", "/dub/translate", {
            "segments": [{"id": s["id"], "text": s["text"], "target_lang": target,
                          "slot_seconds": s["end"] - s["start"], "start": s["start"], "end": s["end"]} for s in segs],
            "target_lang": target, "job_id": job_id, "provider": os.environ.get("VS_TRANSLATE", "nllb"),
            "source_lang": src_lang, "quality": "fast",
        })
        by_id = {t["id"]: t for t in tr.get("translated", [])}
        for s in segs:
            t = by_id.get(s["id"]) or {}
            if t.get("error") or not t.get("text"):
                raise SystemExit(f"tarjima muvaffaqiyatsiz [{s['id']}]: {t.get('error') or 'matn yo`q'}")
            log(f"   -> {t['text']}")
            merged.append({**s, "text": t["text"]})
    prog(30, "Tarjima tayyor")
    json.dump({"source": segs, "translated": merged, "raw": tr}, open(os.path.join(outdir, "segments.json"), "w"), ensure_ascii=False, indent=1)

    # 5. Ovozlash: har bir qator o'z spikerining klonlangan ovozi bilan
    log("ovozlash (klon) ...")
    gen = call("POST", f"/dub/generate/{job_id}", {
        "segments": [{
            "start": s["start"], "end": s["end"], "text": s["text"], "target_lang": target,
            "profile_id": "auto:" + safe_name(s.get("speaker_id") or "Speaker 1"),
            "effect_preset": os.environ.get("VS_EFFECT", "raw"),
        } for s in merged],
        "num_step": int(os.environ.get("VS_NUM_STEP", "32")),
        "language": "Uzbek", "language_code": target,
        "timing_strategy": os.environ.get("VS_TIMING", "strict_slot"),
        "voice_match": "consistent",
    })
    ok = False
    for name, ev in sse(f"/tasks/stream/{gen['task_id']}"):
        t = ev.get("type", name)
        if t == "progress":
            cur, tot = int(ev.get("current") or 0), max(1, int(ev.get("total") or 1))
            log(f"  segment {cur}/{tot}")
            prog(30 + 65 * cur / tot, f"Ovoz klonlanib gaplar aytilmoqda: {cur}/{tot}")
        elif t in ("error", "warning"):
            log(f"  {t}: {json.dumps(ev)[:600]}")
        if t == "done":
            log(f"  tayyor: {json.dumps(ev)[:400]}")
            ok = True
            break
        if t in ("cancelled", "error") and t == "cancelled":
            break
    if not ok:
        raise SystemExit("ovozlash tugamadi")

    # 6. Videoga qo'yish va yuklab olish
    log("eksport (video + dublyaj + fon) ...")
    prog(96, "Video va dublyaj birlashtirilmoqda")
    mp4 = call("GET", f"/dub/download/{job_id}?preserve_bg=true&default_track={target}&include_tracks={target}", raw=True)
    open(os.path.join(outdir, f"dubbed_{target}.mp4"), "wb").write(mp4)
    log(f"  video: {len(mp4) / 1e6:.2f} MB")
    try:
        wav = call("GET", f"/dub/download-audio/{job_id}?lang={target}&preserve_bg=false", raw=True)
        open(os.path.join(outdir, f"dubbed_{target}_voice.wav"), "wb").write(wav)
    except Exception as e:  # noqa: BLE001
        log(f"  ovoz fayli olinmadi: {e}")
    prog(100, "Tayyor")
    log("TUGADI")


if __name__ == "__main__":
    main()
