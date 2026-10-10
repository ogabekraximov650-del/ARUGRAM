// Admin bilan yozishma — `lib/screens/support_chat_screen.dart` (foydalanuvchi
// tomoni) va u ishlatadigan Telegram uslubidagi bo'laklar:
// `tg_bubble.dart` (pufak, kun ajratgichi, vaqt), `tg_reply.dart` (javob,
// iqtibos, surib javob berish), `tg_chat_background.dart` (fon),
// `tg_waveform.dart` (ovoz to'lqini), `tg_record_button.dart` (yuborish /
// ovoz tugmasi va yozish doirasi), `tg_composer.dart` (emoji paneli —
// `tg-emoji.js`).
//
//   openSupport()  — o'z suhbatim (profildagi "Admin bilan bog'lanish").
//
// Fayllar (rasm, video, ovoz, hujjat) Telegram orqali yuklanadi va
// ko'rsatiladi (`tg/media.js`: `uploadFile`, `mediaUrl`). Telegram'ga hali
// kirilmagan bo'lsa — bosib yuklanadigan joy, bosilganda `ensureTelegram()`.
//
// Mini App'da YO'Q (sababi hisobotda): dumaloq video yozish, stiker/GIF
// (ilova to'plamlari), video kadr (thumbnail), admin tanlash/o'chirish
// (admin paneli ko'chirilmaydi).

import * as tgMedia from '../tg/media.js';
import { push, back } from '../router.js';
import { icon, spinner, toast, haptic, C } from '../ui.js';
import { esc } from '../format.js';
import { currentUser } from '../api.js';
import {
  ChatController, unreadBadge, hasMedia, isVoice, isInline, isViewable, fileNameOf,
} from '../services/support.js';
import { richText, plainEmojiText, emojiButtonHtml, bindEmojiInput } from './tg-emoji.js';
import { renderPackMedia } from './packs.js';
import { openMediaView, loadMedia, isTgNotReady, tgRetry } from './media-view.js';

// Telegram qorong'i mavzusi ranglari (aksentga moslangan).
const PILL = 'rgba(36,31,28,0.94)'; // 0xF0241F1C
const OUT_META = '#E2BE9C';
const TITLE = "Admin bilan bog'lanish";
const HEAD_H = 56;

// ══════════════════════════════════════════════════════════════
//  YORDAMCHILAR (tg_reply / tg_bubble / tg_waveform)
// ══════════════════════════════════════════════════════════════

const RE = /^\[re:([A-Za-z0-9_-]{1,40})\]/;

/** `[asl xabar id, qolgan matn]` */
export function tgSplitReply(body) {
  const m = RE.exec(body || '');
  if (!m) return [null, body || ''];
  return [m[1], body.slice(m[0].length)];
}
export const tgWithReply = (id, text) => (id ? `[re:${id}]${text}` : text);

const WF = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdef';

/** Ovoz balandliklari (0..1) -> `[wf:...]` (100 belgi). */
export function tgEncodeWaveform(levels) {
  if (!levels.length) return '';
  const peak = Math.max(...levels);
  const k = peak <= 0 ? 0 : 31 / peak;
  let out = '[wf:';
  for (let i = 0; i < 100; i++) {
    const a = Math.floor((i * levels.length) / 100);
    const b = Math.max(a + 1, Math.floor(((i + 1) * levels.length) / 100));
    let m = 0;
    for (let j = a; j < b && j < levels.length; j++) m = Math.max(m, levels[j]);
    out += WF[Math.min(31, Math.max(0, Math.round(m * k)))];
  }
  return `${out}]`;
}

export function tgDecodeWaveform(body) {
  if (!body || !body.startsWith('[wf:') || !body.endsWith(']')) return null;
  const s = body.slice(4, -1);
  const out = [];
  for (const ch of s) {
    const v = WF.indexOf(ch);
    if (v < 0) return null;
    out.push(v);
  }
  return out.length ? out : null;
}
export const tgIsWaveformBody = (b) => tgDecodeWaveform(b) != null;

const MONTHS = ['yanvar', 'fevral', 'mart', 'aprel', 'may', 'iyun', 'iyul', 'avgust', 'sentabr', 'oktabr', 'noyabr', 'dekabr'];
const two = (n) => String(n).padStart(2, '0');

export function tgDayLabel(ms) {
  const d = new Date(ms);
  const now = new Date();
  const today = new Date(now.getFullYear(), now.getMonth(), now.getDate());
  const day = new Date(d.getFullYear(), d.getMonth(), d.getDate());
  const diff = Math.round((today - day) / 86400000);
  if (diff === 0) return 'Bugun';
  if (diff === 1) return 'Kecha';
  const s = `${d.getDate()}-${MONTHS[d.getMonth()]}`;
  return d.getFullYear() === now.getFullYear() ? s : `${s}, ${d.getFullYear()}`;
}
export function tgSameDay(a, b) {
  const x = new Date(a); const y = new Date(b);
  return x.getFullYear() === y.getFullYear() && x.getMonth() === y.getMonth() && x.getDate() === y.getDate();
}
export function tgTime(ms) {
  if (ms <= 0) return '';
  const d = new Date(ms);
  return `${two(d.getHours())}:${two(d.getMinutes())}`;
}
export function voiceClock(ms) {
  const s = Math.max(0, Math.floor(ms / 1000));
  return `${Math.floor(s / 60)}:${two(s % 60)}`;
}

let measureCtx = null;
function textWidth(t, font) {
  try {
    measureCtx = measureCtx || document.createElement('canvas').getContext('2d');
    measureCtx.font = font;
    return measureCtx.measureText(t).width;
  } catch (_) { return t.length * 7; }
}

/** Ikkita ✓ (Telegram `msg_check_s` / `msg_halfcheck`). */
function checksSvg(color, seen) {
  return `<svg class="tgc-chk" width="17" height="11" viewBox="0 0 17 11" fill="none" stroke="${color}" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round">
    <path d="M0.8 5.8L3.8 8.8L10.4 1.6"/>${seen ? '<path d="M7.6 8.4L8.2 8.8L14.8 1.6"/>' : ''}</svg>`;
}
function sendState(pending, seen, color) {
  if (pending) return icon('schedule', { size: 12, color, cls: 'tgc-clock' });
  return checksSvg(color, seen);
}

// Pufak dumi (`MessageDrawable`): 8 dp chekinish ichida, 6 dp yoy.
const TAIL = '<svg class="tgc-tail" width="9" height="11" viewBox="0 0 9 11"><path d="M0 0L1 0A6.5 10 0 0 0 6.71 9.93L6.4 11L0 11Z"/></svg>';

/** Pufak burchaklari (`TgBubbleShape.pathFor`). */
function bubbleRadius({ out, tail, topNear, bottomNear, media }) {
  const R = 17; const N = 6;
  const drawTail = tail && !media;
  const tSide = topNear ? N : R;
  const bSide = drawTail ? 0 : (bottomNear ? N : R);
  // tartib: chap-tepa, o'ng-tepa, o'ng-past, chap-past
  return out ? `${R}px ${tSide}px ${bSide}px ${R}px` : `${tSide}px ${R}px ${R}px ${bSide}px`;
}

// ══════════════════════════════════════════════════════════════
//  OVOZ IJROCHISI (`VoicePlayer`): bitta, boshqa xabar bosilsa to'xtaydi
// ══════════════════════════════════════════════════════════════

const localFiles = new Map(); // o'zi yuborgan ovoz: nom -> blob: manzil

const voice = {
  audio: null,
  id: null,
  opening: false,
  subs: new Set(),
  emit() { this.subs.forEach((f) => { try { f(); } catch (_) { /* */ } }); },
  isCurrent(id) { return this.id === id; },
  isPlaying(id) { return this.id === id && this.audio && !this.audio.paused && !this.opening; },
  pos(id) { return this.id === id && this.audio ? this.audio.currentTime * 1000 : 0; },
  dur(id) { return this.id === id && this.audio && Number.isFinite(this.audio.duration) ? this.audio.duration * 1000 : 0; },
  ensure() {
    if (this.audio) return this.audio;
    const a = new Audio();
    a.preload = 'auto';
    ['play', 'pause', 'timeupdate', 'ended', 'loadedmetadata', 'durationchange'].forEach((ev) => a.addEventListener(ev, () => this.emit()));
    this.audio = a;
    return a;
  },
  async toggle(id, url) {
    const a = this.ensure();
    if (this.id === id && !this.opening) {
      if (a.paused) { if (a.ended) a.currentTime = 0; a.play().catch(() => {}); } else a.pause();
      this.emit();
      return;
    }
    this.id = id;
    this.opening = true;
    this.emit();
    try { a.pause(); } catch (_) { /* */ }
    const name = fileNameOf(url);
    try {
      const src = localFiles.get(name) || await loadMedia(name);
      if (this.id !== id) return;
      a.src = src;
      this.opening = false;
      this.emit();
      await a.play();
    } catch (e) {
      if (this.id !== id) return;
      this.opening = false;
      this.id = null;
      this.emit();
      if (isTgNotReady(e)) {
        if (await tgRetry()) this.toggle(id, url);
        return;
      }
      toast('Ovozli xabar ochilmadi');
    }
  },
  seek(id, ms) { if (this.id === id && this.audio) { this.audio.currentTime = ms / 1000; this.emit(); } },
  stop() {
    if (this.audio) { try { this.audio.pause(); } catch (_) { /* */ } }
    this.id = null;
    this.opening = false;
    this.emit();
  },
};

/** To'lqin chizig'i (`SeekBarWaveform`): har 3 px da eni 2 px chiziq. */
function paintWave(canvas, wave, progress, played, rest) {
  const w = canvas.clientWidth || 140;
  const h = 30;
  const dpr = window.devicePixelRatio || 1;
  if (canvas.width !== Math.round(w * dpr)) { canvas.width = Math.round(w * dpr); canvas.height = Math.round(h * dpr); }
  const g = canvas.getContext('2d');
  g.setTransform(dpr, 0, 0, dpr, 0, 0);
  g.clearRect(0, 0, w, h);
  const count = Math.floor(w / 3);
  if (count <= 0) return;
  const split = progress * count;
  g.lineWidth = 2;
  g.lineCap = 'round';
  for (let i = 0; i < count; i++) {
    const v = wave && wave.length ? wave[Math.min(wave.length - 1, Math.floor((i * wave.length) / count))] : 0;
    const hh = (7 * v) / 31;
    const x = i * 3 + 1;
    g.strokeStyle = i < split ? played : rest;
    g.beginPath();
    g.moveTo(x, h / 2 - hh);
    g.lineTo(x, h / 2 + hh);
    g.stroke();
  }
}

// ══════════════════════════════════════════════════════════════
//  YUKLASH HALQASI (`SpinRing`)
// ══════════════════════════════════════════════════════════════

function spinRing(size = 52, font = 13) {
  const el = document.createElement('div');
  el.className = 'tgc-ring';
  el.style.width = el.style.height = `${size}px`;
  const stroke = 3;
  const r = (size - stroke) / 2;
  const c = size / 2;
  el.innerHTML = `<svg width="${size}" height="${size}"><circle cx="${c}" cy="${c}" r="${r}" fill="none" stroke="rgba(255,255,255,0.18)" stroke-width="${stroke}"/>
    <path fill="none" stroke="#fff" stroke-width="${stroke}" stroke-linecap="round"/></svg><span class="pc" style="font-size:${font}px"></span>`;
  const path = el.querySelector('path');
  const pc = el.querySelector('.pc');
  let target = 0; let shown = 0; let rate = 0; let prevT = 0; let prevAt = performance.now();
  let angle = 0; let last = 0; let known = false;
  function frame(now) {
    if (!el.isConnected && last) return;
    const dt = last ? (now - last) / 1000 : 0;
    last = now;
    angle = (angle + (dt * 2 * Math.PI) / 1.6) % (2 * Math.PI);
    if (shown < target) {
      const gap = target - shown;
      shown = Math.min(target, shown + Math.max(rate * dt, gap * dt * 1.5));
    }
    const full = shown >= 0.999;
    const sweep = Math.max(shown, 0.04) * 2 * Math.PI;
    if (full) {
      path.setAttribute('d', `M${c} ${c - r}A${r} ${r} 0 1 1 ${c - 0.01} ${c - r}`);
    } else {
      const a0 = angle - Math.PI / 2;
      const a1 = a0 + sweep;
      const x0 = c + r * Math.cos(a0); const y0 = c + r * Math.sin(a0);
      const x1 = c + r * Math.cos(a1); const y1 = c + r * Math.sin(a1);
      path.setAttribute('d', `M${x0} ${y0}A${r} ${r} 0 ${sweep > Math.PI ? 1 : 0} 1 ${x1} ${y1}`);
    }
    pc.textContent = known ? `${Math.floor(shown * 100)}` : '';
    requestAnimationFrame(frame);
  }
  requestAnimationFrame(frame);
  return {
    el,
    set(p) {
      known = true;
      const t = Math.min(1, Math.max(0, p));
      if (t < prevT) { shown = t; rate = 0; } else if (t > prevT) {
        const now = performance.now();
        const dt = (now - prevAt) / 1000;
        if (dt > 0.05) { const rr = (t - prevT) / dt; rate = rate === 0 ? rr : rate * 0.7 + rr * 0.3; prevAt = now; }
      }
      prevT = t; target = t;
    },
  };
}

// ══════════════════════════════════════════════════════════════
//  YOZISH DOIRASI (`_RecordCircle`, `BlobDrawable`)
// ══════════════════════════════════════════════════════════════

class Blob {
  constructor(n) {
    this.n = n;
    this.l = (4 / 3) * Math.tan(Math.PI / (2 * n));
    this.r = new Array(n).fill(0); this.a = new Array(n).fill(0);
    this.rn = new Array(n).fill(0); this.an = new Array(n).fill(0);
    this.p = new Array(n).fill(0); this.sp = new Array(n).fill(0);
    this.minR = 0; this.maxR = 0; this.amplitude = 0; this.to = 0; this.diff = 0;
  }
  r100() { return (Math.floor(Math.random() * 200) - 100) / 100; }
  gen(r, a, i) {
    const angleDif = (360 / this.n) * 0.05;
    r[i] = this.minR + Math.abs(this.r100()) * (this.maxR - this.minR);
    a[i] = (360 / this.n) * i + this.r100() * angleDif;
    this.sp[i] = 0.017 + 0.003 * Math.abs(this.r100());
  }
  generate() { for (let i = 0; i < this.n; i++) { this.gen(this.r, this.a, i); this.gen(this.rn, this.an, i); this.p[i] = 0; } }
  update(amp, scale) {
    for (let i = 0; i < this.n; i++) {
      this.p[i] += this.sp[i] * 0.8 + amp * this.sp[i] * 8.2 * scale;
      if (this.p[i] >= 1) { this.p[i] = 0; this.r[i] = this.rn[i]; this.a[i] = this.an[i]; this.gen(this.rn, this.an, i); }
    }
  }
  setValue(v, big) {
    this.to = v;
    const speed = big ? 1 - 0.65 : 1 - 0.45;
    this.diff = v > this.amplitude
      ? (v - this.amplitude) / (100 + (big ? 300 : 400) * speed)
      : (v - this.amplitude) / (100 + 500 * speed);
  }
  updateAmplitude(dt) {
    if (this.to === this.amplitude) return;
    this.amplitude += this.diff * dt;
    if ((this.diff > 0 && this.amplitude > this.to) || (this.diff < 0 && this.amplitude < this.to)) this.amplitude = this.to;
  }
  draw(g) {
    g.beginPath();
    const rot = (x, y, a) => [x * Math.cos(a) - y * Math.sin(a), x * Math.sin(a) + y * Math.cos(a)];
    for (let i = 0; i < this.n; i++) {
      const j = i + 1 < this.n ? i + 1 : 0;
      const p = this.p[i]; const pn = this.p[j];
      const r1 = this.r[i] * (1 - p) + this.rn[i] * p;
      const r2 = this.r[j] * (1 - pn) + this.rn[j] * pn;
      const a1 = ((this.a[i] * (1 - p) + this.an[i] * p) * Math.PI) / 180;
      const a2 = ((this.a[j] * (1 - pn) + this.an[j] * pn) * Math.PI) / 180;
      const l = this.l * (Math.min(r1, r2) + (Math.max(r1, r2) - Math.min(r1, r2)) / 2);
      const s0 = rot(0, -r1, a1); const s1 = rot(l, -r1, a1);
      const e0 = rot(0, -r2, a2); const e1 = rot(-l, -r2, a2);
      if (i === 0) g.moveTo(s0[0], s0[1]);
      g.bezierCurveTo(s1[0], s1[1], e1[0], e1[1], e0[0], e0[1]);
    }
    g.fill();
  }
}

function recordCircle() {
  const cv = document.createElement('canvas');
  cv.className = 'tgr-circle';
  const dpr = window.devicePixelRatio || 1;
  cv.width = 300 * dpr; cv.height = 300 * dpr;
  const g = cv.getContext('2d');
  const big = new Blob(12); big.minR = 50; big.maxR = 50 + 12 * 0.6; big.generate();
  const tiny = new Blob(11); tiny.minR = 47; tiny.maxR = 47 + 15 * 0.6; tiny.generate();
  const st = { enter: 0, enterAt: performance.now(), amp: 0, ampTo: 0, ampStep: 0, idle: 0, idleUp: true, slideDx: 0, slide: 1, lockMove: 0 };
  let last = 0; let raf = 0; let alive = true;
  function frame(now) {
    if (!alive) return;
    const dt = Math.min(50, last ? now - last : 0);
    last = now;
    st.enter = Math.min(1, (now - st.enterAt) / 360);
    if (st.amp !== st.ampTo) {
      st.amp += st.ampStep * dt;
      if ((st.ampStep > 0 && st.amp > st.ampTo) || (st.ampStep < 0 && st.amp < st.ampTo)) st.amp = st.ampTo;
    }
    big.updateAmplitude(dt); big.update(big.amplitude, 1.01);
    tiny.updateAmplitude(dt); tiny.update(tiny.amplitude, 1.02);
    if (st.idleUp) { st.idle += 0.01; if (st.idle > 1) { st.idle = 1; st.idleUp = false; } } else { st.idle -= 0.01; if (st.idle < 0) { st.idle = 0; st.idleUp = true; } }
    paint();
    raf = requestAnimationFrame(frame);
  }
  function paint() {
    g.setTransform(dpr, 0, 0, dpr, 0, 0);
    g.clearRect(0, 0, 300, 300);
    const cx = 150; const cy = 230;
    const s = st.enter;
    const sc = s <= 0.5 ? s / 0.5 : s <= 0.75 ? 1 - ((s - 0.5) / 0.25) * 0.1 : 0.9 + ((s - 0.75) / 0.25) * 0.1;
    const slideScale = 0.7 + st.slide * 0.3;
    const radius = (41 + 30 * st.amp) * sc * slideScale;
    const x = cx + st.slideDx;
    const slide1 = st.slide > 0.7 ? 1 : st.slide / 0.7;
    const enter = 1 - (1 - Math.min(1, Math.max(0, sc))) ** 2; // easeOut
    if (slide1 > 0) {
      let k = sc * slide1 * enter * (0.878 + 1.4 * big.amplitude);
      g.save(); g.translate(x, cy); g.scale(k, k); g.fillStyle = 'rgba(194,65,12,0.30)'; big.draw(g); g.restore();
      k = sc * slide1 * enter * (0.926 + 1.4 * tiny.amplitude);
      g.save(); g.translate(x, cy); g.scale(k, k); g.fillStyle = 'rgba(194,65,12,0.15)'; tiny.draw(g); g.restore();
    }
    g.fillStyle = C.accent;
    g.beginPath(); g.arc(x, cy, Math.max(0, radius), 0, Math.PI * 2); g.fill();
    const fs = 26 * Math.min(1, Math.max(0, sc));
    if (fs > 1) {
      g.fillStyle = '#fff';
      g.font = `${fs}px 'Material Symbols Rounded'`;
      g.textAlign = 'center'; g.textBaseline = 'middle';
      g.fillText('mic', x, cy);
    }
    // Qulf (`ControlsView`).
    const lm = Math.min(1, Math.max(0, st.lockMove));
    const move = 1 - lm;
    const lockH = 36 + 14 * move;
    const lockTop = cy - 170 + 60 + 30 * (1 - sc) - lm * 57 + move * st.idle * -8;
    const alpha = Math.min(1, Math.max(0, st.slide)) * Math.min(1, Math.max(0, sc));
    if (alpha > 0.01) {
      rrect(g, cx - 18, lockTop, 36, lockH, 18);
      g.fillStyle = `rgba(38,41,46,${alpha})`; g.fill();
      g.lineWidth = 1; g.strokeStyle = `rgba(255,255,255,${0.1 * alpha})`; g.stroke();
      const mid = lockTop + lockH / 2 - 8 + 2 + 2 * move;
      rrect(g, cx - 6.5, mid + 3 - 5, 13, 10, 2.5);
      g.fillStyle = `rgba(255,255,255,${0.85 * alpha})`; g.fill();
      const top = mid - 2;
      g.save();
      g.translate(cx, top); g.rotate((9 * move * Math.PI) / 180); g.translate(-cx, -top);
      g.beginPath();
      g.moveTo(cx - 4, top); g.lineTo(cx - 4, top - 3);
      g.arc(cx, top - 3, 4, Math.PI, 0);
      g.lineTo(cx + 4, top - 3 + 3 * (1 - move));
      g.lineWidth = 1.7; g.lineCap = 'round'; g.strokeStyle = `rgba(255,255,255,${0.85 * alpha})`; g.stroke();
      g.restore();
      if (move > 0.05) {
        const ay = lockTop + lockH - 10;
        g.beginPath(); g.moveTo(cx - 4, ay + 2); g.lineTo(cx, ay - 2); g.lineTo(cx + 4, ay + 2);
        g.strokeStyle = `rgba(255,255,255,${0.6 * alpha * move})`; g.lineWidth = 1.7; g.stroke();
      }
    }
  }
  raf = requestAnimationFrame(frame);
  return {
    el: cv,
    setAmp(v) {
      const a = Math.min(1, Math.max(0, v));
      big.setValue(a, true); tiny.setValue(a, false);
      st.ampTo = a;
      st.ampStep = (st.ampTo - st.amp) / (100 + 500 * 0.55);
    },
    set(o) { Object.assign(st, o); },
    destroy() { alive = false; cancelAnimationFrame(raf); cv.remove(); },
  };
}

function rrect(g, x, y, w, h, r) {
  g.beginPath();
  g.moveTo(x + r, y);
  g.arcTo(x + w, y, x + w, y + h, r);
  g.arcTo(x + w, y + h, x, y + h, r);
  g.arcTo(x, y + h, x, y, r);
  g.arcTo(x, y, x + w, y, r);
  g.closePath();
}

// ══════════════════════════════════════════════════════════════
//  POPUP MENYU (`showMenu`)
// ══════════════════════════════════════════════════════════════

function popupMenu(items, x, y, { bg = '#26272B', radius = 12, itemH = 48, iconSize = 22, gap = 18, iconAlpha = 0.8 } = {}) {
  return new Promise((resolve) => {
    const app = document.getElementById('app');
    const wrap = document.createElement('div');
    wrap.className = 'tgm-wrap';
    wrap.innerHTML = `<div class="tgm" style="background:${bg};border-radius:${radius}px">${items.map((it, i) => `
      <div class="tgm-it" data-i="${i}" style="height:${itemH}px">
        ${icon(it.icon, { fill: it.fill !== false, size: iconSize, color: it.color ? hexA(it.color, iconAlpha) : `rgba(255,255,255,${iconAlpha})` })}
        <span style="margin-left:${gap}px;color:${it.color || '#fff'}">${esc(it.label)}</span></div>`).join('')}</div>`;
    app.appendChild(wrap);
    const m = wrap.querySelector('.tgm');
    const vw = app.clientWidth; const vh = app.clientHeight;
    const w = m.offsetWidth; const h = m.offsetHeight;
    m.style.left = `${Math.max(8, Math.min(x, vw - w - 8))}px`;
    m.style.top = `${Math.max(8, Math.min(y, vh - h - 8))}px`;
    requestAnimationFrame(() => wrap.classList.add('in'));
    const close = (v) => { wrap.classList.remove('in'); setTimeout(() => wrap.remove(), 150); resolve(v); };
    wrap.addEventListener('click', (e) => {
      const it = e.target.closest('.tgm-it');
      close(it ? items[Number(it.dataset.i)].value : null);
    });
  });
}
function hexA(hex, a) {
  const n = parseInt(hex.slice(1), 16);
  return `rgba(${(n >> 16) & 255},${(n >> 8) & 255},${n & 255},${a})`;
}

// ══════════════════════════════════════════════════════════════
//  EKRAN
// ══════════════════════════════════════════════════════════════

const MIMES = {
  jpg: 'image/jpeg', jpeg: 'image/jpeg', png: 'image/png', webp: 'image/webp', gif: 'image/gif', heic: 'image/heic',
  mp4: 'video/mp4', mov: 'video/quicktime', mkv: 'video/x-matroska', webm: 'video/webm', '3gp': 'video/3gpp',
  mp3: 'audio/mpeg', m4a: 'audio/mp4', ogg: 'audio/ogg', wav: 'audio/wav', flac: 'audio/flac',
  pdf: 'application/pdf', zip: 'application/zip', txt: 'text/plain', apk: 'application/vnd.android.package-archive',
  doc: 'application/msword', xls: 'application/vnd.ms-excel',
  docx: 'application/vnd.openxmlformats-officedocument.wordprocessingml.document',
  xlsx: 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet',
};

export function openSupport() {
  push((el) => buildChat(el));
}

function buildChat(el) {
  const chat = new ChatController();
  el.classList.add('sup');
  el.innerHTML = `
    <div class="sup-bg"><div class="g1"></div><div class="g2"></div><div class="pat"></div></div>
    <div class="sup-main">
      <div class="sup-area">
        <div class="sup-list"><div class="sup-items"></div><div class="sup-tail"></div></div>
        <div class="sup-down">${icon('keyboard_arrow_down', { size: 28, color: '#fff' })}</div>
      </div>
      <div class="sup-bottom"></div>
    </div>
    <div class="sup-head"></div>`;

  const listEl = el.querySelector('.sup-list');
  const itemsEl = el.querySelector('.sup-items');
  const tailEl = el.querySelector('.sup-tail');
  const downEl = el.querySelector('.sup-down');
  const headEl = el.querySelector('.sup-head');
  const bottomEl = el.querySelector('.sup-bottom');

  let closed = false;
  let replyTo = null;
  let searching = false;
  let hits = [];
  let hit = 0;
  let flash = null;
  let sending = false;
  let uploading = null; // {type, file, name, ms, ring}
  let pullBusy = false;
  const rows = new Map(); // id -> {sig, el}

  const isMine = (m) => (chat.isAdminView ? m.fromAdmin : !m.fromAdmin);
  const nameOf = (m) => (isMine(m) ? 'Siz' : TITLE);

  function snippet(m) {
    const body = tgSplitReply(m.body)[1];
    if (hasMedia(m)) {
      const label = { voice: 'Ovozli xabar', round: 'Video xabar', sticker: 'Stiker', gif: 'GIF', file: m.body || 'Fayl', video: 'Video' }[m.mediaType] || 'Rasm';
      if (m.mediaType === 'file' || isVoice(m) || !body) return label;
      return `${label}, ${plainEmojiText(body)}`;
    }
    return plainEmojiText(body).replace(/\n/g, ' ');
  }

  // ── SARLAVHA ──
  function paintHead() {
    if (searching) {
      headEl.innerHTML = `
        <div class="sup-pill sup-rb" data-a="close">${icon('arrow_back', { size: 26, color: '#fff' })}</div>
        <div class="sup-pill sup-mid search"><input type="text" placeholder="Qidiruv" enterkeyhint="search"></div>`;
      const inp = headEl.querySelector('input');
      inp.addEventListener('input', () => runSearch(inp.value));
      setTimeout(() => inp.focus(), 50);
      headEl.querySelector('[data-a="close"]').addEventListener('click', closeSearch);
      return;
    }
    headEl.innerHTML = `
      <div class="sup-pill sup-rb" data-a="back">${icon('arrow_back', { size: 26, color: '#fff' })}</div>
      <div class="sup-pill sup-mid">
        <div class="sup-ava"><img src="assets/aru-mark.png" alt=""></div>
        <div class="sup-tt"><div class="t">${esc(TITLE)}</div><div class="s">yordam xizmati</div></div>
      </div>
      <div class="sup-pill sup-rb" data-a="more">${icon('more_vert', { size: 26, color: '#fff' })}</div>`;
    headEl.querySelector('[data-a="back"]').addEventListener('click', () => back());
    const more = headEl.querySelector('[data-a="more"]');
    more.addEventListener('click', async () => {
      const r = more.getBoundingClientRect();
      const v = await popupMenu([{ value: 'search', icon: 'search', label: 'Qidiruv' }], r.right, r.bottom,
        { bg: '#2A2420', radius: 14, itemH: 50, iconSize: 24, gap: 22, iconAlpha: 0.85 });
      if (v === 'search' && !closed) { searching = true; paintHead(); paintBottom(); }
    });
  }

  // ── QIDIRUV ──
  function runSearch(q) {
    const s = q.trim().toLowerCase();
    const out = [];
    if (s) {
      for (const m of [...chat.items].reverse()) {
        const body = plainEmojiText(tgSplitReply(m.body)[1]).toLowerCase();
        if (body.includes(s) && !tgIsWaveformBody(body)) out.push(m.id);
      }
    }
    hits = out; hit = 0;
    paintSearchBar(q);
    if (out.length) goTo(out[0]);
  }
  function closeSearch() {
    searching = false; hits = []; hit = 0;
    paintHead(); paintBottom();
  }
  function paintSearchBar(q = headEl.querySelector('input')?.value || '') {
    const bar = bottomEl.querySelector('.sup-sbar');
    if (!bar) return;
    const n = hits.length;
    bar.querySelector('.t').textContent = !q.trim() ? '' : n === 0 ? 'Hech narsa topilmadi' : `${hit + 1} / ${n}`;
    bar.querySelector('.up').disabled = !(hit < n - 1);
    bar.querySelector('.dn').disabled = !(hit > 0);
  }

  // ── PASTKI QISM: yozish qatori yoki qidiruv paneli ──
  let composer = null;
  function paintBottom() {
    if (searching) {
      composer?.hide();
      bottomEl.querySelector('.sup-sbar')?.remove();
      bottomEl.insertAdjacentHTML('beforeend', `<div class="sup-sbar"><div class="t"></div>
        <button class="icon-btn up">${icon('keyboard_arrow_up', { size: 24 })}</button>
        <button class="icon-btn dn">${icon('keyboard_arrow_down', { size: 24 })}</button></div>`);
      const bar = bottomEl.querySelector('.sup-sbar');
      bar.querySelector('.up').addEventListener('click', () => { if (hit < hits.length - 1) { hit++; paintSearchBar(); goTo(hits[hit]); } });
      bar.querySelector('.dn').addEventListener('click', () => { if (hit > 0) { hit--; paintSearchBar(); goTo(hits[hit]); } });
      paintSearchBar('');
    } else {
      bottomEl.querySelector('.sup-sbar')?.remove();
      if (!composer) composer = buildComposer();
      composer.show();
    }
  }

  // ── RO'YXAT ──
  function render() {
    if (closed) return;
    const items = chat.items;
    if (chat.isLoading) {
      itemsEl.innerHTML = `<div class="sup-center">${spinner(36, 2, C.accent)}</div>`;
      rows.clear();
      tailEl.innerHTML = '';
      return;
    }
    if (!items.length && !uploading) {
      rows.clear();
      itemsEl.innerHTML = `<div class="sup-center"><div class="sup-empty">
        ${icon('support_agent', { size: 54, color: 'rgba(255,255,255,0.2)' })}
        <div class="t">${esc(chat.error ?? 'Savolingiz bormi? Yozing — admin javob beradi.')}</div></div></div>`;
      paintTail();
      return;
    }
    itemsEl.querySelector('.sup-center')?.remove();
    const byId = new Map(items.map((m) => [m.id, m]));
    const keep = new Set();
    const order = [];
    items.forEach((m, i) => {
      const next = items[i + 1];
      const prev = items[i - 1];
      const lastOfGroup = !next || next.fromAdmin !== m.fromAdmin || !tgSameDay(next.createdAt, m.createdAt);
      const newDay = !prev || !tgSameDay(prev.createdAt, m.createdAt);
      const topNear = !newDay && prev.fromAdmin === m.fromAdmin;
      const rid = tgSplitReply(m.body)[0];
      const orig = rid ? byId.get(rid) : null;
      const quoteSig = rid ? (orig ? `${orig.id}|${orig.body}|${orig.mediaType}` : 'x') : '';
      const sig = [m.id, m.pending, m.seen, m.body, m.mediaUrl, lastOfGroup, newDay, topNear, quoteSig].join('§');
      let r = rows.get(m.id);
      if (!r || r.sig !== sig) {
        const elx = buildRow(m, { lastOfGroup, newDay, topNear, rid, orig });
        if (r) r.el.replaceWith(elx);
        r = { sig, el: elx };
        rows.set(m.id, r);
      }
      keep.add(m.id);
      order.push(r.el);
    });
    for (const [id, r] of rows) if (!keep.has(id)) { r.el.remove(); rows.delete(id); }
    // Joyida turganlarga tegilmaydi (video/ovoz to'xtab qolmasin).
    order.forEach((node, i) => {
      const at = itemsEl.children[i];
      if (at !== node) itemsEl.insertBefore(node, at || null);
    });
    while (itemsEl.children.length > order.length) itemsEl.lastElementChild.remove();
    paintTail();
  }

  function paintTail() {
    tailEl.innerHTML = '';
    if (uploading) tailEl.appendChild(uploadingBubble(uploading));
    if (pullBusy) tailEl.insertAdjacentHTML('beforeend', `<div class="sup-pull">${spinner(20, 2, 'rgba(255,255,255,0.54)')}</div>`);
  }

  function timeRow(m, mine, onMedia) {
    const c = onMedia ? '#fff' : mine ? OUT_META : 'rgba(255,255,255,0.45)';
    return `<span class="tgc-time" style="color:${c}">${tgTime(m.createdAt)}${mine ? `<span class="ss">${sendState(m.pending, m.seen, onMedia ? '#fff' : OUT_META)}</span>` : ''}</span>`;
  }
  const timePill = (m, mine) => `<span class="tgc-tpill">${timeRow(m, mine, true)}</span>`;

  function buildRow(m, { lastOfGroup, newDay, topNear, rid, orig }) {
    const mine = isMine(m);
    const bottomNear = !lastOfGroup;
    const row = document.createElement('div');
    row.className = 'tgc-rowwrap';
    row.dataset.id = m.id;
    const body = tgSplitReply(m.body)[1];
    const hasText = !!body && m.mediaType !== 'file' && !(isVoice(m) && tgIsWaveformBody(body));
    const quote = rid ? `<div class="tgc-quote" style="--qc:${mine ? '#fff' : C.accent2}">
        <div class="bar"></div><div class="qb"><div class="qn">${esc(orig ? nameOf(orig) : 'Xabar')}</div>
        <div class="qt">${richText(orig ? snippet(orig) : "o'chirilgan")}</div></div></div>` : '';
    const bare = hasMedia(m) && isInline(m) && !m.body && !rid;
    let inner;
    if (bare) {
      inner = `<div class="tgc-bare ${mine ? 'out' : 'in'}"><div class="md"></div><div style="height:4px"></div>${timePill(m, mine)}</div>`;
    } else if (isViewable(m) && !hasText && !rid) {
      inner = `<div class="tgc-mediaonly ${mine ? 'out' : 'in'}"><div class="clip" style="border-radius:${bubbleRadius({ out: mine, topNear, bottomNear, media: true })}"><div class="md"></div>${timePill(m, mine)}</div></div>`;
    } else {
      const shape = { out: mine, tail: !bottomNear, topNear, bottomNear, media: false };
      const pad = isViewable(m) ? '3px 3px 6px 3px' : '6px 10px 6px 11px';
      const tw = textWidth(tgTime(m.createdAt), '12px Roboto, sans-serif') + (mine ? 20 : 0) + 8;
      inner = `<div class="tgc-bubble ${mine ? 'out' : 'in'}${isViewable(m) ? ' viewable' : ''}" style="background:${mine ? '#7A4A2A' : '#2A2420'};border-radius:${bubbleRadius(shape)};padding:${pad};--tc:${mine ? '#7A4A2A' : '#2A2420'}">
        ${shape.tail ? TAIL : ''}
        ${quote ? `<div class="qw">${quote}</div>` : ''}
        ${hasMedia(m) ? '<div class="md"></div>' : ''}
        ${hasText
    ? `<div class="tgc-text${isViewable(m) ? ' pv' : ''}${hasMedia(m) ? ' pt' : ''}">${richText(body)}<span class="sp" style="width:${tw.toFixed(1)}px"></span>${timeRow(m, mine, false)}</div>`
    : `<div class="tgc-trow${isViewable(m) ? ' pv' : ''}">${timeRow(m, mine, false)}</div>`}
      </div>`;
    }
    row.innerHTML = `${newDay ? `<div class="tgc-date"><span>${esc(tgDayLabel(m.createdAt))}</span></div>` : ''}
      <div class="tgc-row ${mine ? 'out' : 'in'}" style="padding-bottom:${bottomNear ? 2 : 8}px">
        <div class="tgc-swipe"><div class="ic-w">${icon('reply', { size: 20, color: '#fff' })}</div></div>
        <div class="tgc-content">${inner}</div>
      </div>`;
    const md = row.querySelector('.md');
    if (md && hasMedia(m)) fillMedia(md, m, mine);
    row.querySelector('.tgc-quote')?.addEventListener('click', (e) => { e.stopPropagation(); goTo(rid); });
    bindRowGestures(row, m);
    return row;
  }

  // ── MEDIA ──
  function mediaPlaceholder(box, retry, cls = '') {
    box.innerHTML = `<div class="tgc-tap ${cls}"><div class="b">${icon('download', { size: 26, color: '#fff' })}</div></div>`;
    box.querySelector('.tgc-tap').addEventListener('click', async (e) => {
      e.stopPropagation();
      box.innerHTML = `<div class="tgc-load ${cls}">${spinner(22, 2, 'rgba(255,255,255,0.54)')}</div>`;
      await tgRetry();
      if (!closed) retry();
    });
  }

  function fillMedia(md, m, mine) {
    const name = fileNameOf(m.mediaUrl);
    if (isVoice(m)) { voiceBubble(md, m, mine); return; }
    if (m.mediaType === 'file') { fileBubble(md, m, mine); return; }
    if (m.mediaType === 'round') { roundBubble(md, m); return; }
    if (m.mediaType === 'sticker' || m.mediaType === 'gif') {
      renderPackMedia(md, name, m.mediaType);
      return;
    }
    const video = m.mediaType === 'video';
    md.classList.add('tgc-thumb');
    const open = () => openMediaView({ url: m.mediaUrl, type: m.mediaType });
    if (video) {
      md.innerHTML = `<div class="tgc-vid"><div class="play">${icon('play_arrow', { size: 32, color: '#fff' })}</div></div>`;
      md.addEventListener('click', open);
      return;
    }
    const load = () => {
      md.innerHTML = `<div class="tgc-load">${spinner(22, 2, 'rgba(255,255,255,0.54)')}</div>`;
      loadMedia(name).then((src) => {
        if (closed) return;
        const img = new Image();
        img.className = 'tgc-img';
        img.alt = '';
        img.onload = () => { md.innerHTML = ''; md.appendChild(img); };
        img.onerror = () => { md.innerHTML = `<div class="tgc-load">${icon('broken_image', { fill: false, size: 24, color: 'rgba(255,255,255,0.38)' })}</div>`; };
        img.src = src;
      }).catch((e) => {
        if (closed) return;
        if (isTgNotReady(e)) mediaPlaceholder(md, load);
        else md.innerHTML = `<div class="tgc-load">${icon('broken_image', { fill: false, size: 24, color: 'rgba(255,255,255,0.38)' })}</div>`;
      });
    };
    md.addEventListener('click', (e) => { if (md.querySelector('img')) open(); else if (!md.querySelector('.tgc-tap')) e.stopPropagation(); });
    load();
  }

  function voiceBubble(md, m, mine) {
    md.innerHTML = `<div class="tgc-voice">
      <div class="btn" style="background:${mine ? '#B07C57' : C.accent2}"></div>
      <div class="col"><canvas class="wave" height="30"></canvas><div class="clk" style="color:${mine ? OUT_META : 'rgba(255,255,255,0.55)'}"></div></div>
    </div>`;
    const btn = md.querySelector('.btn');
    const cv = md.querySelector('.wave');
    const clk = md.querySelector('.clk');
    const wave = tgDecodeWaveform(m.body);
    let lastIcon = '';
    const update = () => {
      if (closed) return;
      if (!md.isConnected) { voiceUpdaters.delete(update); return; }
      const playing = voice.isPlaying(m.id);
      const opening = voice.opening && voice.id === m.id;
      const real = voice.dur(m.id);
      const total = real > 0 ? real : m.mediaMs;
      const pos = Math.min(voice.pos(m.id), total > 0 ? total : 0);
      const current = voice.isCurrent(m.id) && total > 0;
      const st = opening ? 'o' : playing ? 'p' : 's';
      if (st !== lastIcon) {
        lastIcon = st;
        btn.innerHTML = opening ? spinner(24, 2, '#fff') : icon(playing ? 'pause' : 'play_arrow', { size: 28, color: '#fff' });
      }
      paintWave(cv, wave, current ? pos / total : 0, mine ? '#F6E2CF' : '#fff', mine ? 'rgba(226,190,156,0.55)' : 'rgba(255,255,255,0.35)');
      clk.textContent = voiceClock(current ? pos : total);
    };
    btn.addEventListener('click', (e) => { e.stopPropagation(); voice.toggle(m.id, m.mediaUrl); });
    cv.addEventListener('pointerdown', (e) => {
      const total = voice.dur(m.id) || m.mediaMs;
      if (!voice.isCurrent(m.id) || total <= 0) return;
      e.stopPropagation();
      const r = cv.getBoundingClientRect();
      voice.seek(m.id, Math.min(1, Math.max(0, (e.clientX - r.left) / r.width)) * total);
    });
    voiceUpdaters.add(update);
    requestAnimationFrame(update);
  }

  function fileBubble(md, m, mine) {
    const nm = m.body || '';
    const dot = nm.lastIndexOf('.');
    const ext = dot > 0 ? nm.slice(dot + 1).toUpperCase() : '';
    md.innerHTML = `<div class="tgc-file"><div class="c" style="background:${mine ? 'rgba(255,255,255,0.22)' : C.accent}">${icon('insert_drive_file', { size: 24, color: '#fff' })}</div>
      <div class="col"><div class="n">${esc(nm || 'Fayl')}</div>${ext ? `<div class="e">${esc(ext)}</div>` : ''}</div></div>`;
    const c = md.querySelector('.c');
    let busy = false;
    const openFile = async () => {
      if (busy) return;
      busy = true;
      c.innerHTML = spinner(20, 2, '#fff');
      try {
        const src = await loadMedia(fileNameOf(m.mediaUrl));
        const a = document.createElement('a');
        a.href = src;
        a.download = (nm || fileNameOf(m.mediaUrl)).replace(/[\\/:*?"<>|]/g, '_');
        document.body.appendChild(a);
        a.click();
        a.remove();
      } catch (e) {
        if (isTgNotReady(e)) { busy = false; c.innerHTML = icon('insert_drive_file', { size: 24, color: '#fff' }); if (await tgRetry()) openFile(); return; }
        toast(`Fayl ochilmadi: ${e?.message || e}`);
      }
      busy = false;
      c.innerHTML = icon('insert_drive_file', { size: 24, color: '#fff' });
    };
    md.querySelector('.tgc-file').addEventListener('click', (e) => { e.stopPropagation(); openFile(); });
  }

  function roundBubble(md, m) {
    const short = Math.min(window.innerWidth, window.innerHeight);
    const size = Math.min(320, Math.max(160, short * 0.6));
    const playSize = Math.min(420, Math.max(200, short * 0.92 - 16));
    md.innerHTML = `<div class="tgc-round" style="width:${size}px;height:${size}px">
      <div class="in"><div class="tgc-load">${spinner(24, 2, 'rgba(255,255,255,0.54)')}</div></div>
      <svg class="prog"><circle fill="none" stroke="#fff" stroke-width="3"/></svg>
      <div class="pill"><span class="t">${voiceClock(m.mediaMs)}</span>${icon('volume_off', { size: 13, color: '#fff' })}</div></div>`;
    const box = md.querySelector('.tgc-round');
    const inner = box.querySelector('.in');
    const prog = box.querySelector('.prog');
    const circ = prog.querySelector('circle');
    const pillT = box.querySelector('.pill .t');
    const pillI = box.querySelector('.pill .ic');
    let v = null; let sound = false;
    const layout = () => {
      const s = sound ? playSize : size;
      box.style.width = box.style.height = `${s}px`;
      prog.setAttribute('width', s); prog.setAttribute('height', s);
      circ.setAttribute('cx', s / 2); circ.setAttribute('cy', s / 2); circ.setAttribute('r', (s - 3) / 2);
      const len = Math.PI * (s - 3);
      circ.style.strokeDasharray = `${len}`;
      prog.style.display = sound ? '' : 'none';
      pillI.style.display = sound ? 'none' : '';
    };
    const tick = () => {
      if (!v) return;
      const total = Number.isFinite(v.duration) && v.duration > 0 ? v.duration * 1000 : m.mediaMs;
      const pos = v.currentTime * 1000;
      pillT.textContent = voiceClock(sound ? total - pos : total);
      if (sound && total > 0) {
        const s = playSize;
        const len = Math.PI * (s - 3);
        circ.style.strokeDashoffset = `${len * (1 - Math.min(1, pos / total))}`;
      }
    };
    const load = () => {
      inner.innerHTML = `<div class="tgc-load">${spinner(24, 2, 'rgba(255,255,255,0.54)')}</div>`;
      const nm = fileNameOf(m.mediaUrl);
      Promise.resolve(localFiles.get(nm) || loadMedia(nm)).then((src) => {
        if (closed) return;
        v = document.createElement('video');
        v.muted = true; v.loop = true; v.playsInline = true; v.autoplay = true;
        v.setAttribute('playsinline', '');
        v.src = src;
        v.addEventListener('timeupdate', tick);
        v.addEventListener('ended', () => {
          if (!sound) return;
          sound = false; v.muted = true; v.loop = true; v.currentTime = 0; v.play().catch(() => {});
          layout(); tick();
        });
        inner.innerHTML = '';
        inner.appendChild(v);
        v.play().catch(() => {});
      }).catch((e) => {
        if (closed) return;
        if (isTgNotReady(e)) mediaPlaceholder(inner, load, 'round');
        else inner.innerHTML = `<div class="tgc-load">${icon('refresh', { size: 30, color: 'rgba(255,255,255,0.6)' })}</div>`;
      });
    };
    box.addEventListener('click', (e) => {
      e.stopPropagation();
      if (!v) { if (!inner.querySelector('.tgc-tap')) load(); return; }
      if (!sound) {
        voice.stop();
        sound = true; v.loop = false; v.currentTime = 0; v.muted = false; v.play().catch(() => {});
      } else if (!v.paused) v.pause(); else v.play().catch(() => {});
      layout(); tick();
    });
    layout();
    load();
  }

  // ── YUKLANAYOTGAN FAYL ──
  function uploadingBubble(u) {
    const wrap = document.createElement('div');
    wrap.className = 'tgc-up';
    const image = u.type === 'image';
    const wide = image || u.type === 'video';
    wrap.innerHTML = `<div class="b${wide ? ' wide' : ''}"><div class="m"></div>
      <div class="pend">${sendState(true, false, 'rgba(255,255,255,0.7)')}</div></div>`;
    const mEl = wrap.querySelector('.m');
    const ring = u.ring;
    if (image) {
      mEl.className = 'm img';
      mEl.innerHTML = `<img alt="" src="${u.preview}"><div class="dim"></div>`;
      mEl.appendChild(ring.el);
    } else if (u.type === 'video') {
      mEl.className = 'm vid';
      mEl.appendChild(ring.el);
    } else {
      mEl.className = 'm row';
      mEl.appendChild(ring.el);
      mEl.insertAdjacentHTML('beforeend', `<div class="t">${esc(u.type === 'file' ? u.file.name : voiceClock(u.ms))}</div>`);
    }
    return wrap;
  }

  // ── IMO-ISHORALAR: bosib turish menyusi, surib javob ──
  function bindRowGestures(row, m) {
    const content = row.querySelector('.tgc-content');
    const swipe = row.querySelector('.tgc-swipe');
    let sx = 0; let sy = 0; let dx = 0; let mode = null; let lp = 0; let armed = false; let pid = null;
    const reset = (animate) => {
      content.style.transition = animate ? 'transform 200ms' : '';
      content.style.transform = '';
      swipe.style.opacity = '0';
      swipe.style.transform = 'scale(0.5)';
      dx = 0;
    };
    row.addEventListener('pointerdown', (e) => {
      if (e.button > 0) return;
      sx = e.clientX; sy = e.clientY; mode = null; armed = false; pid = e.pointerId;
      content.style.transition = '';
      clearTimeout(lp);
      lp = setTimeout(() => { lp = 0; if (mode === null) { mode = 'menu'; openMsgMenu(m, sx, sy); } }, 500);
    });
    row.addEventListener('pointermove', (e) => {
      if (e.pointerId !== pid) return;
      const mx = e.clientX - sx; const my = e.clientY - sy;
      if (mode === null && (Math.abs(mx) > 8 || Math.abs(my) > 8)) {
        clearTimeout(lp); lp = 0;
        mode = Math.abs(mx) > Math.abs(my) && mx < 0 && !m.pending ? 'swipe' : 'scroll';
        if (mode === 'swipe') { try { row.setPointerCapture(e.pointerId); } catch (_) { /* */ } }
      }
      if (mode !== 'swipe') return;
      dx = Math.min(0, Math.max(-90, mx));
      const p = Math.min(1, -dx / 50);
      content.style.transform = `translateX(${dx}px)`;
      swipe.style.opacity = `${p}`;
      swipe.style.transform = `scale(${0.5 + 0.5 * p})`;
      const a = -dx >= 50;
      if (a !== armed) { armed = a; if (a) haptic('light'); }
    });
    const end = () => {
      clearTimeout(lp); lp = 0;
      if (mode === 'swipe') { if (armed) startReply(m); reset(true); }
      mode = null; pid = null;
    };
    row.addEventListener('pointerup', end);
    row.addEventListener('pointercancel', () => { clearTimeout(lp); lp = 0; if (mode === 'swipe') reset(true); mode = null; pid = null; });
    row.addEventListener('contextmenu', (e) => { e.preventDefault(); if (mode !== 'menu') openMsgMenu(m, e.clientX, e.clientY); });
  }

  async function openMsgMenu(m, x, y) {
    haptic('medium');
    const text = tgSplitReply(m.body)[1];
    const canCopy = !hasMedia(m) && !!text;
    const items = [];
    if (!m.pending) items.push({ value: 'reply', icon: 'reply', label: 'Javob berish' });
    if (canCopy) items.push({ value: 'copy', icon: 'content_copy', label: 'Nusxa olish' });
    if (!items.length) return;
    const v = await popupMenu(items, x, y);
    if (closed || !v) return;
    if (v === 'reply') startReply(m);
    if (v === 'copy') {
      try { await navigator.clipboard.writeText(plainEmojiText(text)); } catch (_) {
        const ta = document.createElement('textarea'); ta.value = plainEmojiText(text); document.body.appendChild(ta); ta.select();
        try { document.execCommand('copy'); } catch (__) { /* */ } ta.remove();
      }
      toast('Nusxa olindi');
    }
  }

  function startReply(m) {
    if (m.pending) return;
    haptic('light');
    replyTo = m;
    composer?.paintReply();
    composer?.focus();
  }

  /** Iqtibos bosildi — asl xabarga o'tiladi va u bir zum yoritiladi. */
  function goTo(id) {
    const r = rows.get(id);
    if (!r) { toast('Asl xabar topilmadi'); return; }
    const top = r.el.offsetTop - listEl.clientHeight * 0.35;
    listEl.scrollTo({ top: Math.max(0, top), behavior: 'smooth' });
    rows.forEach((x) => x.el.classList.remove('flash'));
    flash = id;
    r.el.classList.add('flash');
    setTimeout(() => { if (flash === id) { r.el.classList.remove('flash'); flash = null; } }, 1200);
  }

  // ── PASTGA TUSHISH ──
  function atBottomGap() { return listEl.scrollHeight - listEl.clientHeight - listEl.scrollTop; }
  function toBottom(jump = false) {
    requestAnimationFrame(() => {
      const gap = atBottomGap();
      if (!jump && gap > 300) return;
      listEl.scrollTo({ top: listEl.scrollHeight, behavior: jump ? 'auto' : 'smooth' });
    });
  }
  listEl.addEventListener('scroll', () => {
    downEl.classList.toggle('on', atBottomGap() > 400);
  }, { passive: true });
  downEl.addEventListener('click', () => listEl.scrollTo({ top: listEl.scrollHeight, behavior: 'smooth' }));

  // ── PASTDAN TORTIB YANGILASH ──
  let pull = 0; let ty = 0;
  async function pullRefresh() {
    if (pullBusy) return;
    pullBusy = true; paintTail(); toBottom(true);
    await chat.load({ force: true });
    await unreadBadge.refresh();
    pullBusy = false;
    if (!closed) paintTail();
  }
  listEl.addEventListener('touchstart', (e) => { ty = e.touches[0].clientY; pull = 0; }, { passive: true });
  listEl.addEventListener('touchmove', (e) => {
    const y = e.touches[0].clientY;
    const d = ty - y; ty = y;
    if (atBottomGap() <= 1 && d > 0) { pull += d; if (pull > 90) { pull = 0; pullRefresh(); } }
  }, { passive: true });
  listEl.addEventListener('wheel', (e) => {
    if (atBottomGap() <= 1 && e.deltaY > 0) { pull += e.deltaY; if (pull > 240) { pull = 0; pullRefresh(); } }
  }, { passive: true });

  // ══════════════════════════════════════════════════════════
  //  YOZISH QATORI
  // ══════════════════════════════════════════════════════════
  function buildComposer() {
    const wrap = document.createElement('div');
    wrap.className = 'sup-comp';
    wrap.innerHTML = `
      <div class="sup-reply"></div>
      <div class="sup-crow">
        <div class="sup-field">
          ${emojiButtonHtml()}
          <textarea rows="1" maxlength="2000" placeholder="Xabar"></textarea>
          <div class="sup-attach">${icon('attach_file', { size: 23, color: 'rgba(255,255,255,0.55)' })}</div>
        </div>
        <div class="sup-recinfo">
          <div class="dot"></div><div class="tm">0:00,00</div>
          <div class="mid"><div class="slide"><span class="sh">${'<span class="ic fill" style="font-size:20px">chevron_left</span>'}Bekor qilish uchun suring</span></div>
            <button class="cancel">BEKOR QILISH</button></div>
        </div>
        <div class="tgr"><div class="tgr-btn"><span class="snd">${icon('send', { size: 24, color: '#fff' })}</span><span class="mic">${icon('mic', { size: 26, color: '#fff' })}</span></div><div class="tgr-hint">Ovozli xabar yozish uchun bosib turing.</div></div>
      </div>
      <div class="sup-panel"></div>
      <input type="file" class="sup-file" multiple hidden>`;
    bottomEl.appendChild(wrap);
    const ta = wrap.querySelector('textarea');
    const replyEl = wrap.querySelector('.sup-reply');
    const attach = wrap.querySelector('.sup-attach');
    const fileIn = wrap.querySelector('.sup-file');
    const rec = wrap.querySelector('.tgr');
    const recBtn = wrap.querySelector('.tgr-btn');
    const hintEl = wrap.querySelector('.tgr-hint');
    const info = wrap.querySelector('.sup-recinfo');
    const slideEl = info.querySelector('.slide');
    const cancelBtn = info.querySelector('.cancel');
    const tmEl = info.querySelector('.tm');
    const emoji = bindEmojiInput({ button: wrap.querySelector('.tge-btn'), panelHost: wrap.querySelector('.sup-panel'), input: ta, onPickMedia: (p) => sendPack(p) });

    const blank = () => !ta.value.trim();
    function autosize() {
      ta.style.height = 'auto';
      ta.style.height = `${Math.min(ta.scrollHeight, 6 * 20)}px`;
    }
    function paintBtn() {
      const send = (!blank() && !recording) || locked;
      recBtn.classList.toggle('send', send);
      recBtn.classList.toggle('busy', sending);
      recBtn.querySelector('.snd').innerHTML = sending ? spinner(22, 2, '#fff') : icon('send', { size: 24, color: '#fff' });
    }
    ta.addEventListener('input', () => { autosize(); paintBtn(); });

    function paintReply() {
      if (!replyTo || recording) { replyEl.innerHTML = ''; replyEl.classList.remove('on'); return; }
      replyEl.classList.add('on');
      replyEl.innerHTML = `<div class="sup-rbar">
        ${icon('reply', { size: 24, color: C.accent2 })}<div class="ln"></div>
        <div class="col"><div class="n">${esc(nameOf(replyTo))}</div><div class="t">${richText(snippet(replyTo))}</div></div>
        <button class="icon-btn x">${icon('close', { size: 24, color: 'rgba(255,255,255,0.6)' })}</button></div>`;
      replyEl.querySelector('.x').addEventListener('click', () => { replyTo = null; paintReply(); });
    }

    async function sendPack(p) {
      if (sending) return;
      sending = true; paintBtn();
      const reply = replyTo;
      const err = await chat.send(tgWithReply(reply?.id, ''), { mediaFile: `pk_${p.pack}_${p.item}`, mediaType: p.kind });
      sending = false;
      if (closed) return;
      if (err) { paintBtn(); toast(err); return; }
      replyTo = null; paintReply(); paintBtn();
      emoji.close();
      toBottom();
    }

    async function send() {
      if (sending) return;
      const text = ta.value.trim();
      if (!text) return;
      sending = true; paintBtn();
      const reply = replyTo;
      const err = await chat.send(tgWithReply(reply?.id, text));
      sending = false;
      if (closed) return;
      if (err) { paintBtn(); toast(err); return; }
      replyTo = null; paintReply();
      ta.value = ''; autosize(); paintBtn();
      toBottom();
    }

    // ── 📎 BIRIKTIRISH ──
    attach.addEventListener('click', () => { if (!uploading) { emoji.close(); fileIn.click(); } });
    fileIn.addEventListener('change', async () => {
      const files = [...(fileIn.files || [])];
      fileIn.value = '';
      for (let i = 0; i < files.length; i++) {
        const f = files[i];
        const t = (f.type || '').startsWith('image/') ? 'image' : (f.type || '').startsWith('video/') ? 'video' : 'file';
        const dot = f.name.lastIndexOf('.');
        let ext = dot > 0 ? f.name.slice(dot + 1).toLowerCase() : '';
        if (!ext || ext.length > 5 || !/^[a-z0-9]+$/.test(ext)) ext = t === 'video' ? 'mp4' : t === 'image' ? 'jpg' : 'bin';
        const ms = t === 'video' ? await videoMs(f) : 0;
        // Fayl xabarida matn o'rnida uning nomi; izoh — birinchisiga.
        uploadAndSend({ file: f, ext, type: t, ms, body: t === 'file' ? f.name : (i === 0 ? null : '') });
      }
    });

    // ── 🎤 OVOZLI XABAR ──
    let recording = false; let locked = false; let holding = false; let starting = false; let abort = false;
    let pressed = false; let holdT = 0; let lockedNow = false; let startPt = { x: 0, y: 0 }; let dxv = 0;
    let circle = null; let hintT = 0;
    let media = null; let chunks = []; let recStart = 0; let levels = []; let ampT = 0; let audioCtx = null; let autoStop = 0; let recRaf = 0;
    const distCanMove = () => Math.min(140, window.innerWidth * 0.35);

    function setHolding(v) {
      if (v === holding) return;
      holding = v;
      recBtn.classList.toggle('hold', v);
      if (v) {
        circle = recordCircle();
        rec.appendChild(circle.el);
      } else {
        circle?.destroy(); circle = null; dxv = 0;
      }
    }
    function setRecUi() {
      wrap.classList.toggle('recording', recording);
      info.classList.toggle('locked', locked);
      paintReply();
      paintBtn();
    }
    function onDrag(dx) {
      slideEl.style.transform = `translateX(${dx}px)`;
      slideEl.style.opacity = `${Math.min(1, Math.max(0, 1 + dx / 120))}`;
    }

    async function startRecording() {
      if (recording) return false;
      let stream;
      try {
        stream = await navigator.mediaDevices.getUserMedia({ audio: true });
      } catch (_) {
        toast('Mikrofonga ruxsat berilmadi');
        return false;
      }
      voice.stop();
      try {
        const types = ['audio/mp4', 'audio/webm;codecs=opus', 'audio/webm', 'audio/ogg;codecs=opus'];
        const mt = types.find((t) => window.MediaRecorder?.isTypeSupported?.(t)) || '';
        media = new MediaRecorder(stream, mt ? { mimeType: mt, audioBitsPerSecond: 64000 } : {});
        chunks = [];
        media.addEventListener('dataavailable', (e) => { if (e.data && e.data.size) chunks.push(e.data); });
        media.start(250);
        levels = [];
        // Ovoz balandligi (Telegram: RMS * 32767 / 1800).
        try {
          audioCtx = new (window.AudioContext || window.webkitAudioContext)();
          const src = audioCtx.createMediaStreamSource(stream);
          const an = audioCtx.createAnalyser();
          an.fftSize = 2048;
          src.connect(an);
          const buf = new Float32Array(an.fftSize);
          ampT = setInterval(() => {
            an.getFloatTimeDomainData(buf);
            let s = 0; for (let i = 0; i < buf.length; i++) s += buf[i] * buf[i];
            const lin = Math.sqrt(s / buf.length);
            levels.push(lin);
            circle?.setAmp((lin * 32767) / 1800);
          }, 100);
        } catch (_) { /* to'lqinsiz */ }
        recStart = Date.now();
        recording = true;
        setRecUi();
        const tickT = () => {
          if (!recording) return;
          const ms = Date.now() - recStart;
          tmEl.textContent = `${Math.floor(ms / 60000)}:${two(Math.floor(ms / 1000) % 60)},${two(Math.floor((ms % 1000) / 10))}`;
          recRaf = requestAnimationFrame(tickT);
        };
        tickT();
        // 5 daqiqadan uzun ovozli xabar — o'zi to'xtaydi.
        autoStop = setTimeout(() => stopRec(true), 5 * 60_000);
        return true;
      } catch (e) {
        stream.getTracks().forEach((t) => t.stop());
        toast(`Yozib bo'lmadi: ${e?.message || e}`);
        return false;
      }
    }

    async function stopRecording(sendIt) {
      if (!recording) return;
      locked = false;
      clearTimeout(autoStop); clearInterval(ampT); cancelAnimationFrame(recRaf);
      const len = Date.now() - recStart;
      recording = false;
      setRecUi();
      onDrag(0);
      const mr = media; media = null;
      try { audioCtx?.close(); } catch (_) { /* */ }
      audioCtx = null;
      const blob = await new Promise((resolve) => {
        if (!mr || mr.state === 'inactive') { resolve(null); return; }
        mr.addEventListener('stop', () => resolve(new Blob(chunks, { type: mr.mimeType || 'audio/webm' })), { once: true });
        try { mr.stop(); } catch (_) { resolve(null); }
      });
      try { mr?.stream.getTracks().forEach((t) => t.stop()); } catch (_) { /* */ }
      if (!sendIt || !blob) return;
      if (len < 700) { toast('Juda qisqa'); return; }
      const type = blob.type || 'audio/webm';
      const ext = type.includes('mp4') ? 'm4a' : type.includes('ogg') ? 'ogg' : 'webm';
      const file = new File([blob], `voice.${ext}`, { type: type.split(';')[0] });
      uploadAndSend({ file, ext, type: 'voice', ms: len, body: tgEncodeWaveform(levels) });
    }

    function stopRec(sendIt) {
      locked = false; dxv = 0;
      setHolding(false);
      onDrag(0);
      stopRecording(sendIt);
    }

    function showHint() {
      clearTimeout(hintT);
      hintEl.classList.add('on');
      hintT = setTimeout(() => hintEl.classList.remove('on'), 2000);
    }

    recBtn.addEventListener('pointerdown', (e) => {
      if (sending) return;
      if ((!blank() && !recording) || locked) return;
      try { recBtn.setPointerCapture(e.pointerId); } catch (_) { /* */ }
      startPt = { x: e.clientX, y: e.clientY };
      lockedNow = false; abort = false; pressed = true;
      clearTimeout(holdT);
      holdT = setTimeout(async () => {
        holdT = 0;
        emoji.close();
        ta.blur();
        starting = true;
        setHolding(true);
        haptic('light');
        const ok = await startRecording();
        starting = false;
        if (closed) return;
        if (!ok) {
          setHolding(false); onDrag(0);
          if (locked || lockedNow) stopRec(false);
          lockedNow = false;
          return;
        }
        if (abort || (!pressed && !locked && !lockedNow)) {
          abort = false; setHolding(false); onDrag(0); stopRec(false);
        }
      }, 150);
    });
    recBtn.addEventListener('pointermove', (e) => {
      if (!holding || locked) return;
      const dx = e.clientX - startPt.x; const dy = e.clientY - startPt.y;
      const dist = distCanMove();
      const slide = Math.min(1, Math.max(0, 1 + dx / dist));
      if (slide >= 0.7 && -dy >= 57) {
        haptic('medium');
        lockedNow = true;
        setHolding(false); onDrag(0);
        locked = true; setRecUi();
        return;
      }
      dxv = Math.min(0, Math.max(-dist, dx));
      circle?.set({ slideDx: dxv, slide, lockMove: Math.min(57, Math.max(0, -dy)) / 57 });
      onDrag(dxv);
      if (slide <= 0) {
        setHolding(false); onDrag(0);
        if (starting) abort = true; else stopRec(false);
      }
    });
    recBtn.addEventListener('pointerup', () => {
      pressed = false;
      if (lockedNow) { lockedNow = false; return; }
      if (sending) return;
      if (!blank() && !recording) { send(); return; }
      if (locked) { stopRec(true); return; }
      if (holdT) { clearTimeout(holdT); holdT = 0; showHint(); return; }
      if (starting) { abort = true; setHolding(false); onDrag(0); return; }
      if (holding) {
        const cancel = 1 + dxv / distCanMove() < 0.45;
        setHolding(false); onDrag(0);
        stopRec(!cancel);
      }
    });
    recBtn.addEventListener('pointercancel', () => {
      pressed = false;
      clearTimeout(holdT); holdT = 0;
      if (starting) { abort = true; setHolding(false); onDrag(0); return; }
      if (holding) { setHolding(false); onDrag(0); stopRec(false); }
    });
    cancelBtn.addEventListener('click', () => stopRec(false));

    paintBtn();
    return {
      show() { wrap.style.display = ''; },
      hide() { wrap.style.display = 'none'; emoji.close(); },
      paintReply,
      focus() { ta.focus(); },
      setUploading(v) { attach.classList.toggle('dis', v); },
      dispose() {
        clearTimeout(holdT); clearTimeout(hintT);
        if (recording) stopRecording(false);
        circle?.destroy();
      },
    };
  }

  // ── FAYLNI TELEGRAM'GA YUKLASH VA YUBORISH (navbat bilan) ──
  let upQueue = Promise.resolve();
  function uploadAndSend(job) {
    const run = upQueue.then(() => uploadNow(job));
    upQueue = run.catch(() => {});
    return run;
  }

  async function uploadNow({ file, ext, type, ms = 0, body = null }) {
    if (closed) return;
    const me = currentUser()?.id ?? 0;
    const name = `chat_${me}_${Date.now()}.${ext}`;
    const ring = spinRing(type === 'image' || type === 'video' ? 52 : 38, type === 'image' || type === 'video' ? 13 : 10);
    const preview = type === 'image' ? URL.createObjectURL(file) : '';
    uploading = { type, file, ms, ring, preview };
    composer?.setUploading(true);
    render();
    toBottom();
    try {
      const up = async () => tgMedia.uploadFile(file, name, { onProgress: (sent, total) => ring.set(total > 0 ? sent / total : 0) });
      let res;
      try {
        res = await up();
      } catch (e) {
        if (!isTgNotReady(e)) throw e;
        if (!(await tgRetry())) throw e;
        res = await up();
      }
      const fileName = res?.name || name;
      if (type === 'voice' || type === 'round') localFiles.set(fileName, URL.createObjectURL(file));
      const ta = bottomEl.querySelector('textarea');
      const text = body ?? (type === 'voice' || type === 'round' ? '' : (ta?.value.trim() || ''));
      const err = await chat.send(text, { mediaFile: fileName, mediaType: type, mediaMs: ms });
      if (closed) return;
      if (err) toast(err);
      else {
        if (body == null && type !== 'voice' && type !== 'round' && ta) { ta.value = ''; ta.dispatchEvent(new Event('input')); }
        toBottom();
      }
    } catch (e) {
      if (!closed) toast(`Yuborilmadi: ${isTgNotReady(e) ? "Telegram'ga kirilmagan" : (e?.message || e)}`);
    } finally {
      if (preview) setTimeout(() => URL.revokeObjectURL(preview), 1000);
      uploading = null;
      if (!closed) { composer?.setUploading(false); render(); }
    }
  }

  // ── MA'LUMOT ──
  const voiceUpdaters = new Set();
  const onVoice = () => voiceUpdaters.forEach((f) => f());
  voice.subs.add(onVoice);

  let seen = 0;
  const unsub = chat.listen(() => {
    render();
    const n = chat.items.length;
    if (n > seen) { seen = n; toBottom(); }
  });

  paintHead();
  paintBottom();
  chat.loadFromDisk();
  seen = chat.items.length;
  render();
  toBottom(true);
  chat.load().then(() => { seen = chat.items.length; toBottom(true); });
  chat.startPolling();

  return {
    onBack() {
      if (searching) { closeSearch(); return false; }
      return true;
    },
    dispose() {
      closed = true;
      unsub();
      voice.subs.delete(onVoice);
      voice.stop();
      composer?.dispose();
      chat.dispose();
      unreadBadge.refresh();
    },
  };
}

/** Videoning uzunligi (ms) — fayl metama'lumotidan. */
function videoMs(file) {
  return new Promise((resolve) => {
    let done = false;
    const url = URL.createObjectURL(file);
    const v = document.createElement('video');
    const fin = (ms) => { if (done) return; done = true; URL.revokeObjectURL(url); resolve(ms); };
    v.preload = 'metadata';
    v.onloadedmetadata = () => fin(Number.isFinite(v.duration) ? Math.round(v.duration * 1000) : 0);
    v.onerror = () => fin(0);
    setTimeout(() => fin(0), 3000);
    v.src = url;
  });
}
