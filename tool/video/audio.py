#!/usr/bin/env python3
"""Videoning ovozi: yumshoq fon musiqasi + animatsiya bilan bir vaqtda ishlaydigan ovoz effektlari.

Hamma vaqtlar `timeline.json` dagi belgilardan (`marks`) olinadi — ya'ni `scenes.html` bilan BIR XIL manba.
Faqat numpy kerak. Natija: 44,1 kHz stereo 16-bit WAV.

Ishlatish:  python3 audio.py <chiqish.wav>
"""
import json
import sys
import wave
from pathlib import Path

import numpy as np

HERE = Path(__file__).resolve().parent
TL = json.loads((HERE / "timeline.json").read_text(encoding="utf-8"))
M = TL["marks"]
DUR = float(TL["duration"])
SR = 44100
N = int(DUR * SR) + SR
rng = np.random.default_rng(7)

L = np.zeros(N, dtype=np.float64)
R = np.zeros(N, dtype=np.float64)


def put(sig, t, gain=1.0, pan=0.0):
    """sig ni t soniyaga qo'shadi. pan: -1 (chap) .. +1 (o'ng)."""
    i = int(t * SR)
    if i < 0 or i >= N:
        return
    e = min(N, i + len(sig))
    s = sig[: e - i] * gain
    lg = np.cos((pan + 1) * np.pi / 4)
    rg = np.sin((pan + 1) * np.pi / 4)
    L[i:e] += s * lg
    R[i:e] += s * rg


def env_ad(n, a, d, curve=3.0):
    t = np.arange(n) / SR
    atk = np.clip(t / max(a, 1e-4), 0, 1)
    dec = np.exp(-curve * t / max(d, 1e-4))
    return atk * dec


def sine(f, dur, phase=0.0):
    t = np.arange(int(dur * SR)) / SR
    return np.sin(2 * np.pi * f * t + phase)


def lowpass_noise(dur, f0, f1):
    """Oq shovqin, kesish chastotasi f0 dan f1 gacha o'zgaradi (bir qutbli filtr)."""
    n = int(dur * SR)
    x = rng.standard_normal(n)
    y = np.zeros(n)
    fc = np.linspace(f0, f1, n)
    a = 1 - np.exp(-2 * np.pi * fc / SR)
    s = 0.0
    for i in range(n):
        s += a[i] * (x[i] - s)
        y[i] = s
    return y / (np.max(np.abs(y)) + 1e-9)


# ───────────────────────── Effektlar ─────────────────────────
def click(t, gain=0.5, f=1500):
    n = int(0.06 * SR)
    noise = rng.standard_normal(n) * np.exp(-np.arange(n) / (0.002 * SR))
    tone = sine(f, 0.06) * env_ad(n, 0.0005, 0.02)
    put(noise * 0.4 + tone, t, gain)


def tick(t, gain=0.25, f=2600):
    n = int(0.03 * SR)
    put(sine(f, 0.03) * env_ad(n, 0.0005, 0.008), t, gain)


def key_tick(t, gain=0.22):
    n = int(0.05 * SR)
    noise = rng.standard_normal(n) * np.exp(-np.arange(n) / (0.0015 * SR))
    put(noise * 0.5 + sine(900, 0.05) * env_ad(n, 0.0004, 0.012) * 0.6, t, gain, pan=rng.uniform(-.2, .2))


def whoosh(t, dur=0.9, up=True, gain=0.35, pan=0.0):
    f0, f1 = (300, 5200) if up else (5200, 300)
    y = lowpass_noise(dur, f0, f1)
    tt = np.linspace(0, 1, len(y))
    e = np.sin(np.pi * tt) ** 2
    put(y * e, t, gain, pan)


def pip(t, f=880, gain=0.22, dur=0.18, pan=0.0):
    n = int(dur * SR)
    s = (sine(f, dur) + 0.4 * sine(f * 2, dur)) * env_ad(n, 0.004, dur * 0.35)
    put(s, t, gain, pan)


def chime(t, f=659.25, gain=0.28, pan=0.0):
    dur = 1.8
    n = int(dur * SR)
    s = np.zeros(n)
    for k, (m, g) in enumerate([(1, 1.0), (2.0, .45), (3.01, .22), (4.2, .1)]):
        s += g * sine(f * m, dur) * env_ad(n, 0.003, 0.55 / (1 + 0.5 * k))
    put(s, t, gain, pan)


def success(t, gain=0.28):
    chime(t, 523.25, gain * .9)
    chime(t + 0.12, 659.25, gain)
    chime(t + 0.26, 783.99, gain)


def thunk(t, gain=0.5):
    dur = 0.35
    n = int(dur * SR)
    tt = np.arange(n) / SR
    f = 150 * np.exp(-tt * 9) + 55
    ph = 2 * np.pi * np.cumsum(f) / SR
    put(np.sin(ph) * env_ad(n, 0.002, 0.12), t, gain)
    click(t, 0.2, 900)


def deny(t, gain=0.3):
    for k in range(2):
        n = int(0.14 * SR)
        s = (np.sign(sine(180, 0.14)) * 0.4 + sine(180, 0.14)) * env_ad(n, 0.003, 0.08)
        put(s, t + k * 0.17, gain)


def scramble(t, dur=1.2, gain=0.16):
    k = 0
    tt = 0.0
    while tt < dur:
        pip(t + tt, float(rng.choice([700, 880, 1040, 1320, 1580, 1760])), gain, 0.05, pan=rng.uniform(-.6, .6))
        tt += 0.04 + 0.03 * rng.random()
        k += 1


def toggle_sound(t, on=True):
    click(t, 0.35, 1900 if on else 1300)
    pip(t + 0.02, 1500 if on else 900, 0.12, 0.08)


# ───────────────────────── Musiqa ─────────────────────────
def music():
    bpm = 76.0
    beat = 60.0 / bpm
    bar = 4 * beat
    prog = [  # Am – F – C – G (A minor, yumshoq)
        ([220.00, 261.63, 329.63], 110.00),
        ([174.61, 220.00, 261.63], 87.31),
        ([196.00, 261.63, 329.63], 130.81),
        ([196.00, 246.94, 293.66], 98.00),
    ]
    arp = [0, 1, 2, 1, 2, 1, 0, 1]
    t = 0.0
    bi = 0
    while t < DUR:
        chord, bass = prog[(bi // 2) % 4]
        dur = bar * 2 if bi % 2 == 0 else 0
        if dur:
            n = int((dur + 1.2) * SR)
            pad = np.zeros(n)
            for f in chord:
                for det in (-0.6, 0.0, 0.7):
                    pad += sine(f * 2 ** (det / 1200), dur + 1.2) * 0.33
            e = np.minimum(np.arange(n) / (1.4 * SR), 1) * np.clip((n - np.arange(n)) / (1.2 * SR), 0, 1)
            put(pad * e, t, 0.050, pan=-0.1)
            bn = int((dur + 0.4) * SR)
            b = sine(bass, dur + 0.4) * (np.minimum(np.arange(bn) / (0.3 * SR), 1)) * np.clip((bn - np.arange(bn)) / (0.4 * SR), 0, 1)
            put(b, t, 0.070)
        # arpeggio (har sakkizlik)
        for k in range(8):
            f = chord[arp[k]] * 2
            tt = t + k * beat / 2 * (1.0)
            if tt >= DUR:
                break
            if bi % 2 == 0 or k >= 0:
                nn = int(0.9 * SR)
                s = (sine(f, 0.9) + 0.3 * sine(f * 2, 0.9)) * env_ad(nn, 0.004, 0.3)
                put(s, tt, 0.026, pan=0.25 if k % 2 else -0.25)
        t += bar
        bi += 1


def typing(a, b, n, shift=0.0):
    """scenes.html dagi `typeInto` bilan bir xil: harf chiqish vaqtlari."""
    for k in range(1, n + 1):
        key_tick(a + (k / (n + 0.999)) * (b - a) + shift)


# ───────────────────────── Vaqt jadvali ─────────────────────────
def cues():
    m = M
    # sahnalar almashishi
    whoosh(m["intro_logo"] - 0.1, 1.0, True, 0.30)
    chime(m["intro_logo"] + 0.7, 523.25, 0.24)
    chime(m["intro_sub"], 659.25, 0.16)
    for i in range(4):
        pip(m["intro_feat"] + i * 0.35, 880 + i * 110, 0.12, 0.12, pan=-0.5 + i / 3)
    whoosh(m["intro_end"] - 0.2, 1.1, True, 0.34)
    for key in ("s1", "s2", "s3", "s4", "s5", "s6"):
        whoosh(m[key] - 0.05, 0.8, True, 0.20, pan=0.3)

    # 1 — kirish
    whoosh(m["s1_phone"], 0.9, False, 0.22, pan=-0.4)
    pip(m["s1_dia_phone"], 740, 0.16, pan=-0.5)
    pip(m["s1_dia_tg"], 988, 0.16, pan=0.5)
    typing(m["s1_ph0"], m["s1_ph1"], 12)
    click(m["s1_btn1"], 0.45)
    pip(m["s1_pk1"], 1175, 0.18, pan=0.0)
    whoosh(m["s1_code_scr"], 0.5, True, 0.16, pan=0.3)
    pip(m["s1_pk2"] + 1.8, 1319, 0.22, pan=-0.3)
    chime(m["s1_tg_msg"], 880, 0.14)
    typing(m["s1_co0"], m["s1_co1"], 5)
    click(m["s1_btn2"], 0.45)
    pip(m["s1_pk3"], 1175, 0.18)
    whoosh(m["s1_pass_scr"], 0.5, True, 0.16, pan=0.3)
    typing(m["s1_pw0"], m["s1_pw1"], 6, 0.0)
    click(m["s1_btn3"], 0.45)
    pip(m["s1_pk4"], 1175, 0.18)
    success(m["s1_sess"], 0.26)
    thunk(m["s1_lock"], 0.5)
    pip(m["s1_lock"] + 2.2, 1046, 0.14)
    whoosh(m["s1_token"] - 0.1, 0.9, True, 0.22, pan=0.4)
    pip(m["s1_token"] + 0.6, 880, 0.18, pan=-0.3)
    pip(m["s1_token"] + 2.2, 1175, 0.18, pan=0.3)
    pip(m["s1_token"] + 4.0, 1319, 0.18, pan=0.4)
    pip(m["s1_token"] + 5.9, 1568, 0.20, pan=-0.3)
    success(m["s1_token"] + 7.2, 0.16)
    for i in range(4):
        pip(m["s1_limit"] + 0.2 + i * 0.6, 659 + i * 110, 0.20, 0.2, pan=-0.6 + i * 0.4)
    deny(m["s1_limit"] + 0.2 + 4 * 0.6)

    # 2 — bosh sahifa
    whoosh(m["s2_phone"], 0.8, False, 0.18, pan=-0.4)
    for i in range(4):
        pip(m["s2"] + 0.8 + i * 1.2, 784 + i * 98, 0.14, 0.15, pan=0.4)
    whoosh(m["s2_scroll0"], 4.0, True, 0.10, pan=-0.2)
    click(m["s2_search"], 0.4)
    whoosh(m["s2_search"] + 0.05, 0.5, True, 0.12)
    for i, n in enumerate([m["s2_cache"], m["s2_cache"] + 0.4, m["s2_cache"] + 0.8]):
        pip(n, 700 + i * 120, 0.16, pan=-0.4 + i * 0.4)
    for i in range(6):
        tick(m["s2_cache"] + 2 + i * 0.9 + 0.5, 0.22, 2400)
    pip(m["s2_cache"] + 5.8, 600, 0.22)

    # 3 — pleyer
    whoosh(m["s3_phone"] + 3.2, 0.9, True, 0.22, pan=-0.3)
    for k, key in enumerate(["s3_chan", "s3_rust", "s3_play", "s3_disk"]):
        pip(m[key] - (0.6 if key == "s3_disk" else 0), 700 + k * 120, 0.14, pan=-0.5 + k * 0.33)
    click(m["s3_tap"], 0.5)
    pip(m["s3_req"], 1046, 0.2, pan=-0.2)
    tick(m["s3_req"] + 1.8, 0.3)
    toggle_sound(m["s3_dec"], True)
    thunk(m["s3_disk"] + 1.6, 0.30)
    pip(m["s3_plyr"] + 1.6, 1319, 0.2, pan=0.4)
    for i in range(10):
        tick(m["s3_plyr"] + 1.8 + i * 0.25, 0.12, 2000 + 60 * i)
    whoosh(m["s3_win0"], 1.2, True, 0.14)
    for i in range(10):
        tick(m["s3_win0"] + 1 + i * 0.5, 0.10, 2300)
    whoosh(m["s3_seek"], 0.9, True, 0.30, pan=0.3)
    tick(m["s3_seek"] + 0.8, 0.35)
    success(m["s3_hist"], 0.20)
    chime(m["s3_hist"] + 3, 784, 0.16)
    deny(m["s3_off"], 0.22)
    chime(m["s3_off"] + 1.8, 880, 0.20)
    whoosh(m["s3_dial"], 0.9, True, 0.30)
    for i, a in enumerate([m["s3_dial"] + 0.8, m["s3_time"], m["s3_time"] + 4, m["s3_time"] + 8]):
        pip(a, 740 + i * 120, 0.15, pan=0.4)
    # yarim aylana aylanishi (3 / -5 / +2 qism) — har qismda tebranish "chiq"
    sp = m["s3_spin"]
    for k in range(3):
        tick(sp + 0.55 * (k + 1) * 2.2 / 3.0 + 0.3, 0.34, 1800)
    for k in range(5):
        tick(sp + 3.6 + 2.0 * (k + 0.5) / 5.0, 0.34, 1800)
    for k in range(2):
        tick(sp + 7.0 + 1.4 * (k + 0.5) / 2.0, 0.34, 1800)
    tm = m["s3_time"]
    for k in range(11):
        tick(tm + 3.0 * (k + 0.5) / 11.0, 0.24, 2600)
    for k in range(5):
        tick(tm + 5 + 2.5 * (k + 0.5) / 5.0, 0.24, 2600)
    click(tm + 3.1, 0.3, 1100)
    click(tm + 7.6, 0.3, 1100)

    # 4 — xavfsizlik
    whoosh(m["s4_file"] - 0.2, 0.7, True, 0.18)
    pip(m["s4_file"], 740, 0.16, pan=-0.3)
    scramble(m["s4_enc"] + 0.4, 1.4, 0.15)
    thunk(m["s4_enc"] + 2.0, 0.40)
    pip(m["s4_up"], 988, 0.18, pan=0.3)
    chime(m["s4_up"] + 0.6, 587.33, 0.15)
    whoosh(m["s4_disk"] - 0.3, 0.7, True, 0.20, pan=-0.3)
    pip(m["s4_disk"], 698, 0.16, pan=-0.3)
    thunk(m["s4_disk"] + 0.5, 0.30)
    chime(m["s4_ks"], 784, 0.22)
    pip(m["s4_ks"] + 0.4, 1175, 0.12, pan=0.4)
    toggle_sound(m["s4_keys"], True)
    pip(m["s4_keys"] + 0.3, 1046, 0.14)
    whoosh(m["s4_sync"] - 0.3, 0.7, True, 0.20, pan=0.3)
    for i in range(6):
        tick(m["s4_sync"] + 0.3 + i * 0.12, 0.16, 2200 + 120 * i)
    thunk(m["s4_sync"] + 1.2, 0.30)
    pip(m["s4_nofile"], 880, 0.18)
    chime(m["s4_nofile"] + 0.4, 659, 0.16)
    pip(m["s4_sec"], 1046, 0.18)
    chime(m["s4_sec"] + 0.4, 880, 0.18)
    for i in range(3):
        success(m["s4_sum"] + i * 0.35, 0.12)
    whoosh(m["s4_sum"] - 0.1, 0.8, True, 0.2)

    # 5 — foydalanuvchi xavfsizligi
    whoosh(m["s5_phone"] - 0.2, 0.8, False, 0.20, pan=0.4)
    pip(m["s5_tog1"], 840, 0.16, pan=0.3)
    toggle_sound(m["s5_tog1"] + 1.2, True)
    toggle_sound(m["s5_tog1"] + 1.5, True)
    pip(m["s5_tog2"], 940, 0.16, pan=0.3)
    toggle_sound(m["s5_tog2"] + 1.2, True)
    pip(m["s5_dev"], 740, 0.16, pan=0.3)
    chime(m["s5_dev"] + 0.1, 698, 0.16)
    pip(m["s5_dev"] + 2.2, 880, 0.16, pan=0.3)
    chime(m["s5_dev"] + 2.3, 740, 0.16)
    whoosh(m["s5_logout"] - 0.2, 0.6, True, 0.20, pan=0.3)
    thunk(m["s5_logout"] + 0.2, 0.40)
    pip(m["s5_logout"] + 0.4, 600, 0.16)
    whoosh(m["s5_consent"] - 0.2, 0.6, True, 0.20, pan=0.3)
    toggle_sound(m["s5_consent"] + 0.3, True)
    success(m["s5_consent"] + 0.5, 0.18)

    # 6 — xulosa
    whoosh(m["s6_i1"] - 0.2, 0.7, True, 0.18)
    for i, k in enumerate(["s6_i1", "s6_i2", "s6_i3", "s6_i4"]):
        pip(m[k], 784 + i * 110, 0.20, 0.22, pan=0.4)
        chime(m[k] + 0.05, 523.25 * (1 + i * 0.125), 0.14)
    whoosh(m["s6_logo"] - 0.2, 1.2, True, 0.30)
    success(m["s6_logo"] + 0.5, 0.30)
    chime(m["s6_logo"] + 1.4, 1046.5, 0.22)


def main():
    out = Path(sys.argv[1] if len(sys.argv) > 1 else "audio.wav")
    music()
    mus_l, mus_r = L.copy(), R.copy()
    L[:] = 0
    R[:] = 0
    cues()
    fade = np.minimum(np.arange(N) / (3.0 * SR), 1) * np.clip((N - np.arange(N)) / (4.0 * SR), 0, 1)
    ml = mus_l * fade + L
    mr = mus_r * fade + R
    # yumshoq cheklash va me'yorlash (-1 dBFS)
    peak = max(np.max(np.abs(ml)), np.max(np.abs(mr)), 1e-9)
    g = 0.89 / peak
    ml, mr = np.tanh(ml * g * 1.15) / np.tanh(1.15), np.tanh(mr * g * 1.15) / np.tanh(1.15)
    n = int(DUR * SR)
    pcm = np.empty((n, 2), dtype="<i2")
    pcm[:, 0] = np.clip(ml[:n] * 32767 * 0.95, -32768, 32767).astype("<i2")
    pcm[:, 1] = np.clip(mr[:n] * 32767 * 0.95, -32768, 32767).astype("<i2")
    with wave.open(str(out), "wb") as w:
        w.setnchannels(2)
        w.setsampwidth(2)
        w.setframerate(SR)
        w.writeframes(pcm.tobytes())
    print("audio:", out, "%.1f s" % (n / SR))


if __name__ == "__main__":
    main()
