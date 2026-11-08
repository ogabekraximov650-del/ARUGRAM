"""UZ: Dublyaj ovozini asl aktyor ohangi/ovoz balandligiga moslash (CPU, librosa). Har gap uchun: (1) ovoz balandligi (dB) konturi,
(2) gap darajasi (baqiriq/pichirlash), (3) ovoz tonining (pitch) spikerning o'rtachasiga nisbatan siljishi.
EN: Match the dubbed voice to the original actor's loudness contour, per-line level and pitch shift (CPU, librosa).
RU: Podgonka dublirovannogo golosa pod gromkost, uroven i visotu originala (CPU, librosa).

Usage: prosody.py <src_video> <segments.json> <voice.wav> <mix.wav> <dubbed.mp4> <out.mp4>
"""
import json
import subprocess
import sys
import tempfile
import time

import librosa
import numpy as np
import soundfile as sf

HOP = 0.02
ANA_SR = 16000
PITCH_BUDGET_S = float(__import__("os").environ.get("PROSODY_PITCH_BUDGET", "900"))


def log(m):
    print("  [prosody] " + m, flush=True)


def env_db(y, sr):
    n = int(sr * HOP)
    if len(y) < n * 2:
        return np.full(2, -80.0)
    fr = librosa.util.frame(y, frame_length=n * 2, hop_length=n)
    e = 20 * np.log10(np.sqrt((fr ** 2).mean(axis=0)) + 1e-5)
    k = 7
    return np.convolve(np.pad(e, k // 2, mode="edge"), np.ones(k) / k, mode="valid")


def active_median(e):
    thr = e.max() - 25
    a = e[e > thr]
    return float(np.median(a)) if len(a) else float(np.median(e))


def f0_median(y, sr):
    if len(y) < sr * 0.3:
        return None
    f0, vf, _ = librosa.pyin(y, fmin=65, fmax=450, sr=sr, frame_length=1024, hop_length=320)
    v = f0[vf & ~np.isnan(f0)]
    return float(np.median(v)) if len(v) >= 5 else None


def main():
    src, segf, voicef, mixf, dubbed, out = sys.argv[1:7]
    t0 = time.time()
    segs = json.load(open(segf))["translated"]
    voice, sr = sf.read(voicef, always_2d=True, dtype="float32")
    mix, sr2 = sf.read(mixf, always_2d=True, dtype="float32")
    if sr2 != sr:
        raise SystemExit("namuna chastotasi mos emas")
    n = min(len(voice), len(mix))
    voice, mix = voice[:n], mix[:n]
    if mix.shape[1] != voice.shape[1]:
        voice = np.repeat(voice[:, :1], mix.shape[1], axis=1)
    bg = mix - voice
    with tempfile.TemporaryDirectory() as td:
        ow = td + "/o.wav"
        subprocess.run(["ffmpeg", "-hide_banner", "-loglevel", "error", "-y", "-i", src, "-vn", "-ac", "1", "-ar", str(ANA_SR), ow], check=True)
        orig, _ = sf.read(ow, dtype="float32")
    vmono = librosa.resample(voice.mean(axis=1), orig_sr=sr, target_sr=ANA_SR)

    info = []
    for s in segs:
        a, b = float(s["start"]), float(s["end"])
        yo = orig[int(a * ANA_SR):int(b * ANA_SR)]
        yd = vmono[int(a * ANA_SR):int((b + 0.3) * ANA_SR)]
        if len(yo) < ANA_SR * 0.2 or len(yd) < ANA_SR * 0.2:
            info.append(None)
            continue
        eo, ed = env_db(yo, ANA_SR), env_db(yd, ANA_SR)
        info.append({"spk": s.get("speaker_id") or "S", "eo": eo, "ed": ed, "mo": active_median(eo), "md": active_median(ed),
                     "fo": None, "fd": None, "yo": yo, "yd": yd})
    ok = [i for i in info if i]
    if not ok:
        raise SystemExit("moslashtirish uchun gap topilmadi")
    gmo = float(np.median([i["mo"] for i in ok]))
    gmd = float(np.median([i["md"] for i in ok]))

    # Ovoz tonini (pitch) o'lchash — vaqt byudjeti bilan / pitch measurement with a time budget
    for i in ok:
        if time.time() - t0 > PITCH_BUDGET_S:
            break
        i["fo"], i["fd"] = f0_median(i["yo"], ANA_SR), f0_median(i["yd"], ANA_SR)
    per = {}
    for i in ok:
        if i["fo"] and i["fd"]:
            per.setdefault(i["spk"], ([], []))
            per[i["spk"]][0].append(i["fo"])
            per[i["spk"]][1].append(i["fd"])
    base = {k: (float(np.median(v[0])), float(np.median(v[1]))) for k, v in per.items() if len(v[0]) >= 3}

    res = voice.copy()
    nch, nsh = 0, 0
    for s, i in zip(segs, info):
        if not i:
            continue
        a = int(float(s["start"]) * sr)
        b = min(n, int((float(s["end"]) + 0.3) * sr))
        seg = voice[a:b]
        if len(seg) < sr * 0.2:
            continue
        # (3) pitch
        shift = 0.0
        if i["fo"] and i["fd"] and i["spk"] in base:
            bo, bd = base[i["spk"]]
            shift = float(np.clip(0.7 * (12 * np.log2(i["fo"] / bo) - 12 * np.log2(i["fd"] / bd)), -3.0, 3.0))
        if abs(shift) >= 0.3:
            seg = np.stack([librosa.effects.pitch_shift(seg[:, c], sr=sr, n_steps=shift) for c in range(seg.shape[1])], axis=1)
            nsh += 1
        # (1)+(2) loudness contour + line level
        rel_o, rel_d = i["eo"] - i["mo"], i["ed"] - i["md"]
        t = np.linspace(0, 1, len(seg))
        co = np.interp(t, np.linspace(0, 1, len(rel_o)), rel_o)
        cd = np.interp(t, np.linspace(0, 1, len(rel_d)), rel_d)
        contour = np.clip(0.6 * (co - cd), -6.0, 6.0)
        level = np.clip(0.8 * ((i["mo"] - gmo) - (i["md"] - gmd)), -6.0, 6.0)
        gain = 10 ** ((contour + level) / 20)
        fade = min(len(seg) // 4, int(0.01 * sr))
        if fade > 0:
            gain[:fade] *= np.linspace(0, 1, fade)
            gain[-fade:] *= np.linspace(1, 0, fade)
        res[a:b] = seg * gain[:, None]
        nch += 1
    outmix = bg + res
    pk = float(np.abs(outmix).max())
    if pk > 0.98:
        outmix *= 0.98 / pk
    with tempfile.TemporaryDirectory() as td:
        w = td + "/new.wav"
        sf.write(w, outmix, sr)
        subprocess.run(["ffmpeg", "-hide_banner", "-loglevel", "error", "-y", "-i", dubbed, "-i", w, "-map", "0:v:0", "-map", "1:a:0",
                        "-c:v", "copy", "-c:a", "aac", "-b:a", "160k", "-shortest", out], check=True)
    log(f"gaplar: {nch}, ton siljitilgan: {nsh}, vaqt: {time.time() - t0:.0f}s")


if __name__ == "__main__":
    main()
