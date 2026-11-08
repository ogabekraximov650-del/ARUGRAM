#!/usr/bin/env python3
"""Dublyaj — kodlash botining "🗣 Dublyaj" bo'limi (`worker/src/dubbot.rs`).

ALOHIDA workflow (`avtoencode` repoda `.github/workflows/dub.yml`, shablon — `tool/dub/dub.workflow.yml`).
Worker navbatda ish bo'lsa uni o'zi ishga tushiradi. Bitta run — bitta ish (video yoki uning bir necha bo'lagi).

IKKI REJIM (ish `mode`):
  * auto   — transkripsiya -> NLLB tarjima -> dublyaj, hammasi bitta ishda;
  * manual — 1-bosqich (`phase=transcribe`): gaplar va HAR BIR SO'ZNING vaqti (millisekund) botga yuboriladi, foydalanuvchi tarjimani
             o'zi yuboradi; 2-bosqich (`phase=dub`): shu tarjima bilan dublyaj.

UZUN VIDEO BO'LAKLAB: gaplar `PART_S` soniyalik bo'laklarga (gap chegarasida) bo'linadi; har bo'lak alohida dublyaj qilinib kanalga
yuklanadi va botga yoziladi (`/api/dub/state`). 6 soatlik chegara yaqinlashsa run to'xtab (`partial`), worker yangi run ochadi va
qolgan bo'laklardan davom etadi; oxirgi run bo'laklarni ulab (ffmpeg concat) bitta video qiladi.

Jarayon: kanaldan yuklash -> (transkripsiya: `tool/voicedub/run.py`, so'z vaqtlari `words.py`) -> (tarjima `translate.py`, NLLB 1.3B) ->
VoiceStudio (CPU) dublyaj (`run.py`: OmniVoice klon ovozi, ffmpeg) -> kanalga yuklash -> `/api/dub/finish`.
FOIZ ACTIONS'NING O'ZIDAN: har bosqich uchun umumiy foiz botdagi bitta xabarda jonli yangilanadi (`/api/dub/progress`).
"""

import asyncio
import collections
import json
import os
import re
import shutil
import signal
import subprocess
import sys
import time
import urllib.error
import urllib.request
from pathlib import Path

HERE = Path(__file__).resolve().parent
WORK = Path(os.environ.get("WORK_ROOT", os.environ.get("RUNNER_TEMP", "/tmp"))) / "arumediatv_dub"
# ARUGRAM repodan olingan `tool/voicedub` (driver `run.py` shu yerda).
VD = Path(os.environ.get("VD_DIR", HERE.parent / "voicedub"))
# Actions job chegarasi 6 soat: run boshlanishi (JOB_T0, workflow yozadi) + byudjet; oxirgi 8 daqiqa yuborish uchun zaxira.
JOB_T0 = float(os.environ.get("JOB_T0") or time.time())
JOB_BUDGET_S = float(os.environ.get("JOB_BUDGET_MIN", "335")) * 60
SEND_RESERVE_S = 8 * 60
PART_S = float(os.environ.get("PART_S", "300"))          # bo'lak uzunligi (soniya, gap chegarasida)
VPY = VD / "voicestudio" / ".venv" / "bin" / "python"      # VoiceStudio venv (faster-whisper, transformers shu yerda)


class Lost(Exception):
    """Ish o'chirildi yoki boshqa run'ga o'tdi."""


class Fatal(Exception):
    """Qayta urinishdan foyda yo'q."""


def log(*a):
    print(time.strftime("%H:%M:%S"), *a, flush=True)


# ── Worker bilan aloqa ───────────────────────────────────────

def _post(path, body, timeout=60, tries=5):
    base = os.environ["API_BASE"].rstrip("/")
    data = json.dumps(body).encode()
    err = None
    for attempt in range(tries):
        req = urllib.request.Request(
            f"{base}{path}", data=data, method="POST",
            headers={"X-Encode-Token": os.environ["ENCODE_TOKEN"],
                     "Content-Type": "application/json", "User-Agent": "arumediatv-dub"})
        try:
            with urllib.request.urlopen(req, timeout=timeout) as r:
                return json.loads(r.read() or b"{}")
        except urllib.error.HTTPError as e:
            if e.code == 409:
                raise Lost()
            if e.code in (400, 401, 403, 404):
                raise RuntimeError(f"{path}: HTTP {e.code}")
            err = e
        except Exception as e:  # tarmoq
            err = e
        time.sleep(3 * (attempt + 1))
    raise RuntimeError(f"{path}: {err}")


def api(path, body):
    return _post(f"/api/dub/{path}", body)


def claim(runner):
    try:
        r = api("claim", {"runner": runner})
    except Exception as e:  # noqa: BLE001
        print("dub/claim:", e, flush=True)
        return None
    return r if r.get("job") and r.get("channel") else None


# ── Jonli holat (foiz Actions'dan) ───────────────────────────

def hms(sec):
    sec = max(0, int(sec))
    h, m, s = sec // 3600, sec % 3600 // 60, sec % 60
    return f"{h}:{m:02}:{s:02}" if h else f"{m:02}:{s:02}"


def bar(pct, width=12):
    full = int(round(max(0.0, min(100.0, pct)) / 100 * width))
    return "▓" * full + "░" * (width - full)


def mb(n):
    return f"{n / 1048576:.1f} MB"


class Live:
    """Botdagi jonli holat xabari (`/api/dub/progress`): umumiy foiz, bosqich, o'tgan va taxminiy qolgan vaqt.
    Har 8 soniyada bittadan ko'p yuborilmaydi (bosqich almashganda darhol)."""

    def __init__(self, r):
        job = r["job"]
        self.base = {"id": int(job["id"]), "name": str(job.get("name") or ""), "chat": int(job["chat"]),
                     "attempt": int(job.get("attempt") or 1), "status_msg": int(r.get("status_msg") or 0)}
        self.steps = []
        self.last = 0.0
        self.last_stage = ""
        self.t0 = time.time()
        self.pipe_t0 = None

    def done(self, line):
        self.steps.append("✅ " + line)

    def send(self, line, force=False):
        now = time.time()
        if not self.base["status_msg"] or (not force and now - self.last < 8):
            return
        self.last = now
        text = "\n".join(self.steps + [line, "", f"⏱ jami: {hms(now - self.t0)}"])
        try:
            api("progress", {**self.base, "text": text})
        except Exception as e:  # noqa: BLE001
            print("dub/progress:", e, flush=True)

    def pipeline(self, pct, stage):
        """Umumiy foiz (0..100) va bosqich matni."""
        if self.pipe_t0 is None:
            self.pipe_t0 = time.time()
        el = time.time() - self.pipe_t0
        eta = el * (100 - pct) / pct if pct >= 12 else 0
        eta_txt = f" · qoldi ~{hms(eta)}" if eta else ""
        changed = stage.split(":")[0] != self.last_stage
        self.last_stage = stage.split(":")[0]
        self.send(f"{bar(pct)} {pct:.0f}%\n{stage}\no'tdi {hms(el)}{eta_txt}", force=changed)


# ── ffprobe ──────────────────────────────────────────────────

def probe(path):
    try:
        out = subprocess.check_output(
            ["ffprobe", "-v", "error", "-select_streams", "v:0", "-show_entries", "stream=width,height:format=duration",
             "-of", "json", str(path)], stderr=subprocess.DEVNULL).decode()
        j = json.loads(out)
        st = (j.get("streams") or [{}])[0]
        return int(st.get("width") or 0), int(st.get("height") or 0), float((j.get("format") or {}).get("duration") or 0)
    except Exception:  # noqa: BLE001
        return 0, 0, 0.0


# ── VoiceStudio (driver) va yordamchi skriptlar ──────────────

def run_driver(src, lang, outdir, live, scale=None, env_extra=None):
    """`tool/voicedub/run.py` — `PROGRESS <foiz>\\t<matn>` qatorlari holat xabariga (scale(pv) -> umumiy foiz), qolgani log'ga."""
    env = {**os.environ, "VS_API": os.environ.get("VS_API", "http://127.0.0.1:8000"), **(env_extra or {})}
    p = subprocess.Popen([sys.executable, str(VD / "run.py"), str(src), lang, str(outdir)],
                         stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, bufsize=1, env=env)
    tail = collections.deque(maxlen=30)
    for line in p.stdout:
        line = line.rstrip()
        if not line:
            continue
        tail.append(line)
        if line.startswith("PROGRESS "):
            pct, _, txt = line[9:].partition("\t")
            try:
                live.pipeline(scale(float(pct)) if scale else float(pct), txt)
            except ValueError:
                pass
            log("  " + txt)
        else:
            log("  " + line)
    return p.wait(), list(tail)


def run_tool(script, *args, env_extra=None):
    """VoiceStudio venv'ida `tool/voicedub/<script>` ni ishga tushiradi; chiqishi log'ga."""
    p = subprocess.Popen([str(VPY), str(VD / script), *map(str, args)], stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True,
                         bufsize=1, env={**os.environ, **(env_extra or {})})
    for line in p.stdout:
        line = line.rstrip()
        if line and not line.startswith("PROGRESS"):
            log("  " + line)
    code = p.wait()
    if code != 0:
        raise RuntimeError(f"{script} xato bilan tugadi (kod {code})")


def fmt_ms(ms):
    ms = int(ms)
    return f"{ms // 60000:02}:{ms % 60000 // 1000:02}.{ms % 1000:03}"


def attach_words(segs, words):
    """Har so'z o'rtasi qaysi gap oralig'iga tushsa, o'sha gapga (yo'q bo'lsa — eng yaqiniga)."""
    out = [{**s, "words": []} for s in segs]
    for w in words:
        mid = (w["s"] + w["e"]) / 2000.0
        best, bd = None, 1e9
        for o in out:
            d = 0 if o["start"] - 0.25 <= mid <= o["end"] + 0.25 else min(abs(mid - o["start"]), abs(mid - o["end"]))
            if d < bd:
                best, bd = o, d
        if best is not None:
            best["words"].append(w)
    return out


def human_text(name, lang, dur, segs):
    """Foydalanuvchiga fayl: har gap + HAR SO'Z millisekund vaqti bilan."""
    lines = [f"MATN: {name}", f"Til: {lang} · Davomiyligi: {fmt_ms(dur * 1000)} · Gaplar: {len(segs)}",
             'Tarjimani "#raqam matn" ko\'rinishida yuboring (masalan: "#1 Salom dunyo").', "=" * 60, ""]
    for n, s in enumerate(segs, 1):
        lines.append(f"#{n}  [{fmt_ms(s['start'] * 1000)} – {fmt_ms(s['end'] * 1000)}]  {int(s['start'] * 1000)}–{int(s['end'] * 1000)} ms")
        lines.append("   " + " ".join(f"{w['w']}({w['s']}–{w['e']})" for w in s["words"]) if s["words"] else "   " + s["text"])
        lines.append("")
    return "\n".join(lines)


def chat_text(segs):
    return "\n\n".join(f"#{n} {s['text']}" for n, s in enumerate(segs, 1))


def split_parts(segs, dur):
    """Gaplarni ~PART_S soniyalik bo'laklarga bo'ladi (gap chegarasida). Har bo'lak: (i0, i1, t0, t1) — gaplar [i0, i1), vaqt oralig'i
    [t0, t1): chegara qo'shni gaplar orasining o'rtasida, shuning uchun bo'laklar bo'shliqsiz ulanadi."""
    groups, cur = [], []
    for i, s in enumerate(segs):
        if cur and s["end"] - segs[cur[0]]["start"] > PART_S:
            groups.append(cur)
            cur = []
        cur.append(i)
    if cur:
        groups.append(cur)
    parts = []
    for gi, g in enumerate(groups):
        t0 = 0.0 if gi == 0 else (segs[groups[gi - 1][-1]]["end"] + segs[g[0]]["start"]) / 2
        t1 = dur if gi == len(groups) - 1 else (segs[g[-1]]["end"] + segs[groups[gi + 1][0]]["start"]) / 2
        parts.append((g[0], g[-1] + 1, t0, t1))
    return parts


# ── Ish ──────────────────────────────────────────────────────

async def download_doc(app, channel, msg_id, dst):
    m = await app.get_messages(channel, int(msg_id))
    if not m or m.empty:
        raise Fatal(f"kanalda xabar {msg_id} topilmadi")
    p = await app.download_media(m, file_name=str(dst))
    if not p:
        raise RuntimeError(f"xabar {msg_id} yuklab olinmadi")
    return Path(p)


async def process(app, r, runner):
    channel = int(r["channel"])
    job = r["job"]
    jid, name, lang = int(job["id"]), str(job.get("name") or "video"), str(job.get("lang") or "uz")
    mode, tr_msg = str(job.get("mode") or "auto"), int(job.get("tr_msg") or 0)
    tl = job.get("tl") or {}
    parts_done = {int(p["k"]): int(p["msg"]) for p in (job.get("parts") or [])}
    live = Live(r)
    ident = {"runner": runner, "id": jid, "status_msg": live.base["status_msg"]}
    shutil.rmtree(WORK, ignore_errors=True)
    WORK.mkdir(parents=True)
    t_run0 = time.time()
    try:
        log(f"🗣 Dublyaj #{jid}: {name} -> {lang} [{mode}] (urinish {job.get('attempt')})")
        live.send("⬇️ Video yuklab olinmoqda...", force=True)
        msg = await app.get_messages(channel, int(job["ch_msg"]))
        if not msg or msg.empty or not (msg.video or msg.document):
            raise Fatal("video kanalda topilmadi")
        t0 = time.time()

        def dl_cb(cur, total):
            live.send(f"⬇️ Video yuklab olinmoqda\n{bar(cur * 100 / max(total, 1))} {cur * 100 / max(total, 1):.0f}% · {mb(cur)} / {mb(total)}")

        src = await app.download_media(msg, file_name=str(WORK / "src.bin"), progress=dl_cb)
        if not src or Path(src).stat().st_size == 0:
            raise RuntimeError("video yuklab olinmadi")
        src = Path(src)
        w, h, dur = probe(src)
        live.done(f"Yuklab olindi: {mb(src.stat().st_size)} · {w}×{h} · {hms(dur)} ({hms(time.time() - t0)})")

        clip = WORK / "clip.mp4"   # VoiceStudio uchun oddiy mp4
        subprocess.run(["ffmpeg", "-hide_banner", "-loglevel", "error", "-y", "-i", str(src), "-c:v", "libx264", "-preset", "veryfast",
                        "-crf", "26", "-vf", "scale='min(854,iw)':-2", "-c:a", "aac", "-ar", "44100", str(clip)], check=True)
        src.unlink(missing_ok=True)
        _, _, dur = probe(clip)

        # ── 1) TRANSKRIPSIYA (gaplar + har so'zning ms vaqti) ──────────
        if not tr_msg:
            out_t = WORK / "tr"
            live.pipeline(0, "VoiceStudio ishga tushmoqda")
            scale_t = (lambda pv: pv * (0.18 if mode == "auto" else 0.9))
            code, tail = await asyncio.to_thread(run_driver, clip, lang, out_t, live, scale_t, {"VS_TRANSCRIBE_ONLY": "1"})
            tj = out_t / "transcript.json"
            if code != 0 or not tj.exists():
                raise RuntimeError("transkripsiya bajarilmadi: " + (tail[-1] if tail else f"kod {code}")[:200])
            tdata = json.load(open(tj, encoding="utf-8"))
            segs = [{"start": float(s["start"]), "end": float(s["end"]), "text": (s["text"] or "").strip(),
                     "speaker_id": s.get("speaker_id") or "Speaker 1"} for s in tdata["segments"] if (s.get("text") or "").strip()]
            if not segs:
                raise Fatal("videoda nutq topilmadi")
            src_lang = tdata.get("source_lang") or "en"
            live.pipeline(0.9 * 100 if mode == "manual" else 19, "So'zlarning aniq vaqti (millisekund) aniqlanmoqda")
            wav = WORK / "audio.wav"
            subprocess.run(["ffmpeg", "-hide_banner", "-loglevel", "error", "-y", "-i", str(clip), "-vn", "-ac", "1", "-ar", "16000", str(wav)], check=True)
            wj = WORK / "words.json"
            await asyncio.to_thread(run_tool, "words.py", wav, wj, src_lang)
            words = json.load(open(wj, encoding="utf-8"))["words"]
            segs = attach_words(segs, words)
            full = {"language": src_lang, "duration": dur, "segments": segs}
            tj_path = WORK / f"transcript_{jid}.json"
            json.dump(full, open(tj_path, "w", encoding="utf-8"), ensure_ascii=False)
            sent = await app.send_document(channel, str(tj_path), caption=f"dub_tr_{jid}")
            tr_msg = int(sent.id)
            live.done(f"Transkripsiya: {len(segs)} ta gap, {len(words)} ta so'z vaqti ({hms(time.time() - t_run0)})")

            if mode == "manual":
                txt_path = WORK / f"matn_{jid}.txt"
                txt_path.write_text(human_text(name, src_lang, dur, segs), encoding="utf-8")
                sent2 = await app.send_document(channel, str(txt_path), caption=f"dub_txt_{jid}", file_name=f"matn_{jid}.txt")
                api("finish", {**ident, "ok": True, "phase": "transcribe", "tr_msg": tr_msg, "txt_msg": int(sent2.id), "nseg": len(segs),
                               "text": chat_text(segs), "src_lang": src_lang,
                               "summary": f"{len(segs)} ta gap · {len(words)} ta so'z · {hms(dur)}"})
                log(f"  ✅ transkripsiya #{jid} yuborildi — tarjima kutilmoqda")
                return

            # auto: NLLB bilan tarjima
            live.pipeline(20, f"Tarjima modeli (NLLB 1.3B) yuklanmoqda va {lang} tiliga tarjima qilinmoqda")
            tin, tout = WORK / "tin.json", WORK / "tout.json"
            json.dump([{"text": s["text"]} for s in segs], open(tin, "w", encoding="utf-8"), ensure_ascii=False)
            await asyncio.to_thread(run_tool, "translate.py", tin, tout, src_lang, lang)
            tr = json.load(open(tout, encoding="utf-8"))
            if len(tr) != len(segs) or any(not t.strip() for t in tr):
                raise RuntimeError("tarjima to'liq emas")
            tl = {str(i + 1): t for i, t in enumerate(tr)}
            api("state", {**ident, "tr_msg": tr_msg, "nseg": len(segs), "tl": tl})
            live.done("Tarjima tayyor")
        else:
            tj_path = await download_doc(app, channel, tr_msg, WORK / "transcript.json")
            full = json.load(open(tj_path, encoding="utf-8"))
            segs = full["segments"]

        # ── 2) DUBLYAJ (bo'laklab) ─────────────────────────────────
        for n in range(1, len(segs) + 1):
            if not str(tl.get(str(n)) or "").strip():
                raise Fatal(f"#{n} gap uchun tarjima yo'q")
        parts = split_parts(segs, dur)
        K = len(parts)
        base = 28.0 if mode == "auto" and not job.get("tr_msg") else 0.0
        rate = None   # sekund / video soniyasi (shu run'da o'lchangan)
        did_this_run = 0
        for k, (i0, i1, p0, p1) in enumerate(parts):
            if k in parts_done:
                continue
            span = p1 - p0
            remaining = JOB_T0 + JOB_BUDGET_S - SEND_RESERVE_S - time.time()
            est = span * (rate if rate else 14.0)
            if did_this_run and est > remaining:
                log(f"  vaqt yetmaydi (kerak ~{est / 60:.0f} daq, qoldi {remaining / 60:.0f} daq) — run to'xtaydi, davom ettiriladi")
                api("finish", {**ident, "ok": True, "partial": True})
                return
            part_clip = WORK / f"part_{k}.mp4"
            subprocess.run(["ffmpeg", "-hide_banner", "-loglevel", "error", "-y", "-ss", f"{p0:.3f}", "-t", f"{span:.3f}", "-i", str(clip),
                            "-c:v", "libx264", "-preset", "veryfast", "-crf", "24", "-c:a", "aac", "-ar", "44100", str(part_clip)], check=True)
            sj = WORK / f"part_{k}.json"
            json.dump([{"start": max(0.0, s["start"] - p0), "end": max(0.01, s["end"] - p0), "text": tl[str(i + 1)],
                        "speaker_id": s.get("speaker_id") or "Speaker 1"} for i, s in zip(range(i0, i1), segs[i0:i1])],
                      open(sj, "w", encoding="utf-8"), ensure_ascii=False)
            out_p = WORK / f"out_{k}"
            lo = base + (99 - base) * k / K
            hi = base + (99 - base) * (k + 1) / K
            tag = f"bo'lak {k + 1}/{K}" if K > 1 else ""
            live.pipeline(lo, f"VoiceStudio ishga tushmoqda {tag}".strip())
            tp = time.time()
            code, tail = await asyncio.to_thread(
                run_driver, part_clip, lang, out_p, live,
                (lambda pv, lo=lo, hi=hi: lo + (hi - lo) * pv / 100), {"VS_SEGMENTS": str(sj), "VS_TRANSCRIBE_ONLY": "0"})
            outv = out_p / f"dubbed_{lang}.mp4"
            if code != 0 or not outv.exists():
                raise RuntimeError(f"dublyaj bajarilmadi ({tag}): " + (tail[-1] if tail else f"kod {code}")[:200])
            api("check", ident)
            ow, oh, od = probe(outv)

            def up_cb(cur, total, tag=tag):
                live.send(f"⬆️ Tayyor video yuborilmoqda {tag}\n{bar(cur * 100 / max(total, 1))} {cur * 100 / max(total, 1):.0f}% · {mb(cur)} / {mb(total)}")

            sent = await app.send_video(channel, str(outv), caption=f"dub_part_{jid}_{k}", supports_streaming=True, duration=int(od),
                                        width=ow, height=oh, file_name=f"dub_{jid}_{lang}_{k}.mp4", progress=up_cb)
            parts_done[k] = int(sent.id)
            did_this_run += 1
            sp = (time.time() - tp) / max(span, 1.0)
            rate = sp if rate is None else (rate + sp) / 2
            api("state", {**ident, "parts": [{"k": kk, "msg": mm} for kk, mm in sorted(parts_done.items())]})
            live.done(f"Dublyaj {tag}: {hms(span)} video, {hms(time.time() - tp)} ish".strip())
            shutil.rmtree(out_p, ignore_errors=True)
            part_clip.unlink(missing_ok=True)

        # ── 3) YAKUNIY VIDEO ────────────────────────────────────────
        if K == 1:
            res_msg = parts_done[0]
        else:
            live.send("🔗 Bo'laklar ulanmoqda...", force=True)
            lst = WORK / "concat.txt"
            files = []
            for k in range(K):
                pth = await download_doc(app, channel, parts_done[k], WORK / f"dl_{k}.mp4")
                files.append(pth)
            lst.write_text("".join(f"file '{p}'\n" for p in files), encoding="utf-8")
            final = WORK / f"dub_{jid}_{lang}.mp4"
            subprocess.run(["ffmpeg", "-hide_banner", "-loglevel", "error", "-y", "-f", "concat", "-safe", "0", "-i", str(lst), "-c:v", "libx264",
                            "-preset", "veryfast", "-crf", "23", "-c:a", "aac", "-movflags", "+faststart", str(final)], check=True)
            fw, fh, fd = probe(final)
            sent = await app.send_video(channel, str(final), caption=f"dub_{jid}_{lang}", supports_streaming=True, duration=int(fd),
                                        width=fw, height=fh, file_name=final.name,
                                        progress=lambda c, t: live.send(f"⬆️ Yakuniy video yuborilmoqda\n{bar(c * 100 / max(t, 1))} {c * 100 / max(t, 1):.0f}%"))
            res_msg = int(sent.id)
        summary = f"{lang} · {hms(dur)} · {K} bo'lak · jami {hms(time.time() - live.t0)}"
        api("finish", {**ident, "ok": True, "res_msg": int(res_msg), "summary": summary})
        log(f"  ✅ dublyaj #{jid} yuborildi ({summary})")
    except Lost:
        log(f"  dublyaj #{jid} o'chirildi yoki boshqa run'ga o'tdi — yuborilmadi")
    except Fatal as ex:
        log(f"  dublyaj #{jid} XATO (qayta urinilmaydi): {ex}")
        try:
            api("finish", {**ident, "ok": False, "fatal": True, "error": str(ex)})
        except Exception:  # noqa: BLE001
            pass
    except Exception as ex:  # noqa: BLE001
        log(f"  dublyaj #{jid} XATO: {type(ex).__name__}: {ex}")
        try:
            api("finish", {**ident, "ok": False, "error": f"{type(ex).__name__}: {ex}"[:300]})
        except Exception:  # noqa: BLE001
            pass
    finally:
        shutil.rmtree(WORK, ignore_errors=True)


SESSION = str(HERE / "pyro_session")
RUNNER = f"{os.environ.get('GITHUB_RUN_ID', 'local')}-{os.environ.get('GITHUB_RUN_ATTEMPT', '1')}"
CURRENT = None


def session_api_id():
    try:
        import sqlite3
        c = sqlite3.connect(f"file:{SESSION}.session?mode=ro", uri=True)
        return int(c.execute("SELECT api_id FROM sessions").fetchone()[0] or 0)
    except Exception:  # noqa: BLE001
        return 0


def on_cancel(signum, frame):
    """Run bekor qilindi — ish navbatga qaytadi."""
    if CURRENT:
        try:
            api("finish", {"runner": RUNNER, "id": CURRENT, "ok": False, "cancelled": True})
        except Exception:  # noqa: BLE001
            pass
        print("Run bekor qilindi — ish navbatga qaytarildi", flush=True)
    os._exit(0)


async def main():
    global CURRENT
    import pyrogram.utils
    from pyrogram import Client
    pyrogram.utils.MIN_CHANNEL_ID = -1009999999999
    api_id = session_api_id() or int(os.environ["TG_API_ID"])
    app = Client(SESSION, api_id=api_id, api_hash=os.environ["TG_API_HASH"], no_updates=True)
    done = 0
    async with app:
        async for _ in app.get_dialogs():  # kanal peer keshida bo'lsin
            pass
        r = claim(RUNNER)
        if r:
            CURRENT = int(r["job"]["id"])
            try:
                await process(app, r, RUNNER)
                done += 1
            finally:
                CURRENT = None
    log(f"Run tugadi: {done} ta ish.")
    out = os.environ.get("GITHUB_OUTPUT")
    if out and done:
        with open(out, "a") as f:
            f.write("worked=1\n")


if __name__ == "__main__":
    signal.signal(signal.SIGINT, on_cancel)
    signal.signal(signal.SIGTERM, on_cancel)
    asyncio.run(main())
    sys.exit(0)
