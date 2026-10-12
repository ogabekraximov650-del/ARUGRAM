// "Kesh tozalanmoqda" animatsiyasi — `lib/widgets/trash_chick.dart` ning web (Canvas 2D) nusxasi.
// Ilovadagi sahna, vaqt jadvali va chizish qadamma-qadam ko'chirilgan: Telegram jo'jasi (`utyan`)
// chiqindini qutiga tashlaydi; sahna chiqindi HAJMIGA qarab 4 xil (500 MB / 2 GB / 5 GB / undan katta).
// Jo'ja shakllari: `chick-parts.js` (`lib/widgets/utyan_parts.dart` dan avtomatik).
import { CHICK } from './chick-parts.js';

// ── SAHNA O'LCHAMLARI ──
const W = 1360, H = 680, FLOOR = 600;
const WALL_R = 262;
const DL = 78, DR = 222, DT = 318;
const DC = (DL + DR) / 2;
const BL = 1186, BR = 1300, BT = 462;
const BC = (BL + BR) / 2;
const K = 0.40;
const HW = 306 * K / 2;
const PX = 294, PY = 392, CX = 300, HEAD_C = 302;
const STOP_X = BL - HW + 6;
const DRAG_X = BL - 18 + HW;
const MB = 1024 * 1024;
const PI = Math.PI;

const clamp = (v, a, b) => Math.max(a, Math.min(b, v));
const seg = (t, a, b) => clamp((t - a) / (b - a), 0, 1);
const lerp = (a, b, t) => a + (b - a) * t;
const eio = (x) => (x < 0.5 ? 4 * x * x * x : 1 - Math.pow(-2 * x + 2, 3) / 2);
const eout = (x) => 1 - Math.pow(1 - x, 3);
const sio = (x) => -(Math.cos(PI * x) - 1) / 2;
const backOut = (x, s = 1.7) => 1 + (s + 1) * Math.pow(x - 1, 3) + s * Math.pow(x - 1, 2);

const tierOf = (b) => (b < 500 * MB ? 1 : b < 2048 * MB ? 2 : b < 5120 * MB ? 3 : 4);
const durationOf = (tier) => [0, 7.0, 13.5, 16.5, 22.5][tier];
export const chickDuration = (bytes) => Math.round(durationOf(tierOf(bytes)) * 1000);

function bagSize(bytes) {
  const g = bytes / MB;
  if (g < 500) return lerp(34, 52, clamp(g / 500, 0, 1));
  if (g < 2048) return lerp(58, 92, clamp((g - 500) / 1548, 0, 1));
  if (g < 5120) return lerp(158, 190, clamp((g - 2048) / 3072, 0, 1));
  return 290;
}

// ── Vektor yordamchilari ──
const P = (x, y) => ({ x, y });
const add = (a, b) => P(a.x + b.x, a.y + b.y);
const sub = (a, b) => P(a.x - b.x, a.y - b.y);
const mul = (a, k) => P(a.x * k, a.y * k);
const dist = (a) => Math.hypot(a.x, a.y);
const lerp2 = (a, b, t) => P(lerp(a.x, b.x, t), lerp(a.y, b.y, t));
const polar = (p, deg, l) => P(p.x + Math.cos(deg * PI / 180) * l, p.y + Math.sin(deg * PI / 180) * l);
const capTo = (sh, p, maxL) => { const d = dist(sub(p, sh)); if (d <= maxL || d === 0) return p; return add(sh, mul(sub(p, sh), maxL / d)); };

function walk(t, t0, t1, x0, x1, stride) {
  const u = seg(t, t0, t1);
  const steps = Math.max(1, Math.round(Math.abs(x1 - x0) / stride));
  return { x: lerp(x0, x1, u), ph: u > 0 && u < 1 ? u * steps : -1 };
}

// ── VAQT JADVALI ──
class S {
  constructor(bytes, sec) {
    this.tier = tierOf(bytes); this.sec = sec; this.bh = bagSize(bytes); this.bw = this.bh * 0.82;
    Object.assign(this, {
      door: 0, x: DC, yaw: 0, ph: -1, stride: 1, lean: 0, lift: 0, sx: 1, sy: 1, rot: 0, sweat: 0, shake: 0, tears: 0,
      wind: 0, fling: 0, grip: 0, hop: 0, flail: 0, wipeBrow: 0, puff: -1, spark: -1, binWob: 0, flyT0: 0, flyT1: 0, flyPeak: 0,
      pivotR: false, happy: false, dragging: false, stuck: false, holding: false, sob: false, rubEye: false, rubAlt: false,
      flyBack: 0, jerk: 0, dragU: 0, dropAt: 99, depth: 0, throwArm: null, eyes: 'calm', bag: 'hand',
      hasEnd: false, endStuck: false, endFrom: 0, endAt: 0, endOut: 0, endPop: 0, endS: null,
      bagPx: NaN, bagAng: PI / 2, bagSq: 1, side: false, knot: null,
    });
    const t = sec;
    if (this.tier === 1) this.tier1(t); else if (this.tier === 2) this.tier2(t); else if (this.tier === 3) this.tier3(t); else this.tier4(t);
  }

  tier1(t) {
    this.door = eio(seg(t, 0, 0.7)) - eio(seg(t, 6.1, 6.9));
    this.x = DC + 18;
    this.throwArm = 'a';
    this.depth = -0.3 - 0.4 * eio(seg(t, 5.8, 6.2));
    let y = lerp(0, 1, eio(seg(t, 0.85, 1.15)));
    y = lerp(y, -1, eio(seg(t, 1.95, 2.35)));
    y = lerp(y, 1, eio(seg(t, 2.85, 3.15)));
    y = lerp(y, 0, eio(seg(t, 5.55, 5.85)));
    this.yaw = y;
    this.lean = 0.13 * Math.sin(PI * seg(t, 1.1, 1.95)) - 0.05 * Math.sin(PI * seg(t, 2.3, 2.85));
    this.grip = eio(seg(t, 0.75, 1.05)) - eio(seg(t, 4.75, 5.0));
    this.wind = sio(seg(t, 3.2, 3.6)) * (1 - seg(t, 3.6, 3.68));
    this.fling = eout(seg(t, 3.6, 3.72)) * (1 - sio(seg(t, 4.0, 4.4)));
    this.flyT0 = 3.68; this.flyT1 = 4.85; this.flyPeak = 300;
    if (t >= this.flyT0) this.bag = t < this.flyT1 ? 'fly' : 'bin';
    this.binWob = t > this.flyT1 ? Math.exp(-(t - this.flyT1) * 5) * Math.sin((t - this.flyT1) * 28) : 0;
    this.puff = seg(t, 4.85, 5.5);
    this.spark = seg(t, 4.9, 5.8);
    this.hop = t > 4.95 && t < 5.5 ? Math.sin(seg(t, 4.95, 5.5) * PI) : 0;
    this.happy = t > 4.9 && t < 5.7;
    this.eyes = this.happy ? 'happy' : 'calm';
  }

  tier2(t) {
    this.door = eio(seg(t, 0, 0.7)) - eio(seg(t, 12.7, 13.4));
    let y = lerp(0, 1, eio(seg(t, 0.75, 0.95)));
    y = lerp(y, 0, eio(seg(t, 7.0, 7.25)));
    y = lerp(y, -1, eio(seg(t, 7.9, 8.2)));
    y = lerp(y, -2, eio(seg(t, 12.35, 12.6)));
    this.yaw = y;
    this.depth = -0.6 * (1 - eio(seg(t, 0.35, 0.85))) - 0.6 * eio(seg(t, 12.5, 12.95));
    if (t > 0.35 && t < 0.85) this.ph = seg(t, 0.35, 0.85) * 2;
    let w = walk(t, 0.85, 5.6, DC, STOP_X - 60, 58);
    this.x = w.x;
    if (t > 0.85) this.ph = w.ph;
    this.wind = sio(seg(t, 5.75, 6.15)) * (1 - seg(t, 6.15, 6.22));
    this.fling = eout(seg(t, 6.15, 6.27)) * (1 - sio(seg(t, 6.6, 7.0)));
    this.flyT0 = 6.22; this.flyT1 = 6.85; this.flyPeak = 120;
    if (t >= this.flyT0) this.bag = t < this.flyT1 ? 'fly' : 'bin';
    this.binWob = t > this.flyT1 ? Math.exp(-(t - this.flyT1) * 5) * Math.sin((t - this.flyT1) * 28) : 0;
    this.puff = seg(t, 6.85, 7.5);
    this.spark = seg(t, 6.9, 7.8);
    this.hop = t > 7.1 && t < 7.75 ? Math.sin(seg(t, 7.1, 7.75) * PI) : 0;
    this.happy = t > 6.9 && t < 7.9;
    this.eyes = this.happy ? 'happy' : 'calm';
    if (t > 8.2) { w = walk(t, 8.2, 12.35, STOP_X - 60, DC, 58); this.x = w.x; this.ph = w.ph; }
  }

  tier3(t) {
    this.door = eio(seg(t, 0, 0.7)) - eio(seg(t, 15.8, 16.4));
    const xSide = DR + 80;
    let y = -2.0;
    y = lerp(y, -1, eio(seg(t, 1.3, 1.7)));
    y = lerp(y, 0, eio(seg(t, 9.0, 9.35)));
    y = lerp(y, -1, eio(seg(t, 10.6, 10.9)));
    y = lerp(y, -2, eio(seg(t, 15.2, 15.45)));
    this.yaw = y;
    this.depth = -0.7 * (1 - eio(seg(t, 0.5, 1.3))) - 0.7 * eio(seg(t, 15.35, 15.8));
    this.x = DC;
    if (t > 0.5 && t < 1.3) this.ph = seg(t, 0.5, 1.3) * 2;
    if (t >= 1.3) { const w = walk(t, 1.3, 1.9, DC, xSide, 40); this.x = w.x; this.ph = w.ph; }
    this.bag = 'drag';
    this.hasEnd = true; this.endFrom = 0.5; this.endAt = 1.9; this.endOut = 2.3;
    this.dragging = t >= 1.6 && t < 8.6;
    this.holding = t >= 0.5 && t < 8.6;
    if (t >= 2.3) { const w = walk(t, 2.3, 8.6, xSide, DRAG_X, 34); this.x = w.x; this.ph = w.ph; this.stride = 0.6; }
    this.lean = this.dragging ? 0.16 + 0.03 * Math.sin(t * 9) : 0;
    this.eyes = this.dragging ? 'strain' : 'calm';
    this.sweat = this.dragging ? 1.0 : Math.max(0, 1 - (t - 8.7) * 1.5);
    this.dropAt = 8.6;
    if (t > 8.7 && t < 10.4) {
      const u = seg(t, 8.7, 10.4);
      this.sy = 1 - 0.06 * Math.sin(u * PI); this.sx = 1 + 0.05 * Math.sin(u * PI);
      this.wipeBrow = Math.sin(u * PI);
    }
    if (t > 10.9) { const w = walk(t, 10.9, 15.2, DRAG_X, DC, 58); this.x = w.x; this.ph = w.ph; this.stride = 1; }
  }

  tier4(t) {
    this.door = eio(seg(t, 0, 0.8)) - eio(seg(t, 21.7, 22.4));
    this.bag = 'drag';
    this.pivotR = true;
    const xSide = DR + 90;
    let y = -2.0;
    y = lerp(y, -1, eio(seg(t, 1.5, 1.9)));
    y = lerp(y, 0, eio(seg(t, 6.5, 6.9)));
    y = lerp(y, -1, eio(seg(t, 8.0, 8.3)));
    y = lerp(y, 0, eio(seg(t, 16.2, 16.5)));
    y = lerp(y, -1, eio(seg(t, 17.4, 17.7)));
    y = lerp(y, -2, eio(seg(t, 21.0, 21.3)));
    this.yaw = y;
    this.depth = -0.7 * (1 - eio(seg(t, 0.6, 1.5))) - 0.7 * eio(seg(t, 21.2, 21.7));
    this.x = DC;
    if (t > 0.6 && t < 1.5) this.ph = seg(t, 0.6, 1.5) * 2;
    this.hasEnd = true; this.endStuck = true; this.endFrom = 0.6; this.endPop = 4.45;
    if (t < 4.45) {
      if (t >= 1.5) { const w = walk(t, 1.5, 2.1, DC, xSide, 40); this.x = w.x; this.ph = w.ph; }
      this.jerk = Math.max(0, Math.sin(seg(t, 2.1, 4.45) * PI * 6)) * seg(t, 2.1, 4.45);
      if (t >= 2.1) this.x = xSide + this.jerk * 10;
      this.lean = lerp(0.08, 0.3, eio(seg(t, 2.1, 2.5))) + 0.08 * this.jerk;
      this.shake = t > 2.1 ? 1.0 : 0;
      this.eyes = t > 2.1 ? 'strain' : 'calm';
      this.sweat = t > 2.3 ? 1.0 : 0;
      this.stuck = t >= 2.1;
      this.holding = true;
      this.dragging = t >= 1.5;
      return;
    }
    const tFall = 4.45, tHit = 4.8, lie = 1.32;
    let r;
    if (t < tHit) {
      const u = seg(t, tFall, tHit);
      r = 0.3 + (lie - 0.3) * u * u;
      this.flyBack = u * u;
    } else {
      const u = t - tHit;
      r = lie - 0.3 * Math.exp(-u * 5) * Math.abs(Math.sin(u * 11));
      this.flyBack = 1;
    }
    r = lerp(r, 0.28, eio(seg(t, 6.0, 6.6)));
    r = lerp(r, 0, eio(seg(t, 7.4, 7.9)));
    this.rot = r;
    if (t > tHit && t < tHit + 0.22) {
      const kq = Math.sin(seg(t, tHit, tHit + 0.22) * PI);
      this.sy *= 1 - 0.14 * kq; this.sx *= 1 + 0.1 * kq;
    }
    this.eyes = t < tHit + 0.1 ? 'calm' : 'cry';
    this.sob = t > 5.1 && t < 7.4;
    this.flail = t < 5.0 ? Math.sin(seg(t, tFall, 5.0) * PI) : 0;
    this.rubEye = (t > 6.9 && t < 7.5) || (t > 16.0 && t < 21.0);
    this.rubAlt = t < 7.5;
    this.tears = t > 5.0 ? 1.0 : 0;
    this.x = xSide;
    if (t >= 8.4) {
      this.dragU = seg(t, 8.4, 16.0);
      this.ph = walk(t, 8.4, 16.0, 0, 700, 28).ph;
      this.stride = 0.5;
      this.dragging = t < 16.0;
      this.lean = this.dragging ? 0.22 + 0.04 * Math.sin(t * 8) : 0;
      this.shake = this.dragging ? 0.4 : 0;
    }
    this.holding = t >= 8.0 && t < 16.0;
    if (t > 16.0 && t < 17.4) {
      const v = seg(t, 16, 17.4);
      this.sy = 1 - 0.07 * Math.sin(v * PI); this.sx = 1 + 0.05 * Math.sin(v * PI);
      this.sob = true;
    }
    this.dropAt = 16.0;
    if (t > 17.7) { const w3 = walk(t, 17.7, 21.0, DRAG_X, DC, 40); this.x = w3.x; this.ph = w3.ph; this.stride = 0.75; }
  }
}

// ── 2D AFFIN ──
class Aff {
  constructor() { this.a = 1; this.b = 0; this.c = 0; this.d = 1; this.e = 0; this.f = 0; }
  translate(x, y) { this.e += this.a * x + this.c * y; this.f += this.b * x + this.d * y; return this; }
  scale(sx, sy) { this.a *= sx; this.b *= sx; this.c *= sy; this.d *= sy; return this; }
  rotate(r) {
    const co = Math.cos(r), si = Math.sin(r);
    const na = this.a * co + this.c * si, nb = this.b * co + this.d * si;
    const nc = -this.a * si + this.c * co, nd = -this.b * si + this.d * co;
    this.a = na; this.b = nb; this.c = nc; this.d = nd; return this;
  }
  map(p) { return P(this.a * p.x + this.c * p.y + this.e, this.b * p.x + this.d * p.y + this.f); }
  unmap(p) {
    const det = this.a * this.d - this.b * this.c; const x = p.x - this.e, y = p.y - this.f;
    return P((this.d * x - this.c * y) / det, (-this.b * x + this.a * y) / det);
  }
}

// ── JO'JA SHAKLLARI ──
const argc = (op) => (op === 1 ? 6 : op === 3 ? 0 : 2);
function mix(a, b, t) {
  const o = a.slice(); let i = 0;
  while (i < a.length) { const n = argc(a[i++]); for (let k = 0; k < n; k++, i++) o[i] = a[i] + (b[i] - a[i]) * t; }
  return o;
}
function bbox(a) {
  let x0 = 1e9, y0 = 1e9, x1 = -1e9, y1 = -1e9, i = 0;
  while (i < a.length) {
    const n = argc(a[i++]);
    for (let k = 0; k < n; k += 2) { x0 = Math.min(x0, a[i + k]); x1 = Math.max(x1, a[i + k]); y0 = Math.min(y0, a[i + k + 1]); y1 = Math.max(y1, a[i + k + 1]); }
    i += n;
  }
  return { x0, y0, x1, y1, cx: (x0 + x1) / 2, cy: (y0 + y1) / 2, w: x1 - x0 };
}
function xform(a, cx, sx, dx, dy) {
  const o = a.slice(); let i = 0;
  while (i < a.length) {
    const n = argc(a[i++]);
    for (let k = 0; k < n; k += 2) { o[i + k] = cx + (a[i + k] - cx) * sx + dx; o[i + k + 1] = a[i + k + 1] + dy; }
    i += n;
  }
  return o;
}
function flipY(a) {
  const cy = bbox(a).cy; const o = a.slice(); let i = 0;
  while (i < a.length) { const n = argc(a[i++]); for (let k = 0; k < n; k += 2) o[i + k + 1] = 2 * cy - a[i + k + 1] + 6; i += n; }
  return o;
}
function pathOf(a) {
  const p = new Path2D(); let i = 0;
  while (i < a.length) {
    const op = a[i++];
    if (op === 0) { p.moveTo(a[i], a[i + 1]); i += 2; }
    else if (op === 1) { p.bezierCurveTo(a[i], a[i + 1], a[i + 2], a[i + 3], a[i + 4], a[i + 5]); i += 6; }
    else if (op === 2) { p.lineTo(a[i], a[i + 1]); i += 2; }
    else p.closePath();
  }
  return p;
}
const PARTS = Object.fromEntries(CHICK.map((p) => [p.n, p]));
const FACE_SWAP = { eye: 'eye_2', eye_2: 'eye', beak: 'beak', beak_bl: 'beak_bl', mouth: 'mouth' };

function pose(name, yaw) {
  const part = PARTS[name];
  if (yaw < -1) {
    const base = pose(name, -1);
    if (!(name in FACE_SWAP)) return base;
    return xform(base, 0, 1, -(-1 - yaw) * 170, 0);
  }
  if (yaw >= 0) return mix(part.a, part.t, yaw);
  const u = -yaw; const ra = bbox(part.a); const swap = FACE_SWAP[name]; let rb, tx;
  if (swap) { rb = bbox(PARTS[swap].t); tx = 2 * HEAD_C - rb.cx; }
  else { rb = bbox(part.t); tx = ra.cx - (rb.cx - ra.cx); }
  return xform(part.a, ra.cx, lerp(1, rb.w / ra.w, u), (tx - ra.cx) * u, (rb.cy - ra.cy) * u);
}

// ── QO'LLAR ──
const SH_Y = 250, SH_DX = 96, ARM_L = 62;

function arms(s, m) {
  const y = s.yaw, ys = Math.max(-1, y);
  const kk = 1 - 0.5 * Math.abs(ys), off = 34 * ys;
  const shA = P(CX - SH_DX * kk + off, SH_Y), shB = P(CX + SH_DX * kk + off, SH_Y);
  const face = y < -0.05 ? -1 : 1;
  const mir = (d) => (face > 0 ? d : 180 - d);
  let aA = 115, aB = 65;
  if (s.ph >= 0) {
    const ph = s.ph % 1, n = Math.floor(s.ph);
    const sw = Math.sin(ph * PI) * (n % 2 === 1 ? 1 : -1) * 24;
    aA += sw; aB -= sw;
  }
  let a = polar(shA, aA, ARM_L), b = polar(shB, aB, ARM_L);
  const nearIsB = face > 0;
  const throwB = s.throwArm != null ? s.throwArm === 'b' : nearIsB;
  if (s.bag === 'hand' || s.fling > 0) {
    const s0 = throwB ? shB : shA;
    const hy = Math.min(320, PY - 0.9 * s.bh / K - 26);
    const hold = capTo(s0, P(s0.x + face * 58, hy), ARM_L * 1.25);
    const back = polar(s0, mir(160), ARM_L * 0.95);
    const up = polar(s0, mir(-55), ARM_L);
    let h = lerp2(hold, back, s.wind);
    h = lerp2(h, up, s.fling);
    if (throwB) b = h; else a = h;
  }
  if (s.grip > 0) {
    const p = m.unmap(P(DR + 8, FLOOR - 62));
    b = lerp2(b, capTo(shB, p, ARM_L * 2.2), s.grip);
  }
  if (s.happy) {
    const w = Math.sin(s.sec * 22) * 14;
    a = polar(shA, -125 + w, ARM_L); b = polar(shB, -55 - w, ARM_L);
  }
  const knot = s.knot;
  if ((s.dragging || s.stuck || s.holding) && knot) {
    const tr = s.shake * Math.sin(s.sec * 50) * 5;
    const k = m.unmap(knot);
    const cap = s.endS ? ARM_L * 1.35 : ARM_L * 1.9;
    a = capTo(shA, P(k.x + 8, k.y + 12 + tr), cap);
    b = capTo(shB, P(k.x - 8, k.y - 12 - tr), cap);
  }
  if (s.flail > 0) {
    const w = Math.sin(s.sec * 30) * 30;
    a = lerp2(a, polar(shA, -140 + w, ARM_L), s.flail);
    b = lerp2(b, polar(shB, -40 - w, ARM_L), s.flail);
  }
  if (s.rubEye && !s.dragging) {
    const r = Math.sin(s.sec * 16) * 10;
    const e1 = bbox(pose('eye', y)), e2 = bbox(pose('eye_2', y));
    const ha = capTo(shA, P(e1.cx + r, e1.cy + 20), ARM_L * 1.7);
    const hb = capTo(shB, P(e2.cx - r, e2.cy + 20), ARM_L * 1.7);
    if (s.rubAlt) { if (Math.floor(s.sec / 0.7) % 2 === 1) a = ha; else b = hb; }
    else if (nearIsB) b = hb; else a = ha;
  }
  if (s.wipeBrow > 0) {
    const h = P(HEAD_C + Math.sin(s.sec * 10) * 30, 62);
    a = lerp2(a, capTo(shA, h, ARM_L * 1.8), s.wipeBrow);
  }
  return { shA, shB, a, b, face };
}

function matrixOf(s) {
  let rot = s.rot, lift = s.lift, sx = s.sx, sy = s.sy;
  if (s.ph >= 0) {
    const ph = s.ph % 1, n = Math.floor(s.ph);
    lift += Math.sin(ph * PI) * 16 * s.stride;
    rot += (n % 2 === 1 ? 1 : -1) * Math.sin(ph * PI) * 0.06 * s.stride;
    const land = ph < 0.15 ? 1 - ph / 0.15 : ph > 0.85 ? (ph - 0.85) / 0.15 : 0;
    sy *= 1 - 0.06 * land; sx *= 1 + 0.05 * land;
  }
  const dir = s.yaw < 0 ? -1 : 1;
  if (s.wind > 0 || s.fling > 0) {
    sy *= 1 - 0.08 * s.wind + 0.06 * s.fling;
    sx *= 1 + 0.07 * s.wind - 0.04 * s.fling;
    rot += (-0.10 * s.wind + 0.12 * s.fling) * dir;
  }
  if (s.hop > 0) { lift += s.hop * 46; sy *= 1 + 0.06 * s.hop; sx *= 1 - 0.04 * s.hop; }
  rot += s.lean;
  if (s.shake > 0) rot += Math.sin(s.sec * 55) * 0.025 * s.shake;
  if (s.sob) {
    const c = (s.sec * 1.7) % 1;
    const v = c < 0.18 ? c / 0.18 : Math.pow(1 - (c - 0.18) / 0.82, 2);
    sy *= 1 + 0.06 * v; sx *= 1 - 0.03 * v;
    if (s.pivotR) rot += 0.03 * v;
  }
  sx *= 1 + 0.1 * s.depth; sy *= 1 + 0.1 * s.depth;
  lift -= 14 * s.depth;
  const m = new Aff();
  if (s.pivotR) m.translate(s.x + HW, FLOOR - lift).rotate(rot).translate(-HW, 0);
  else m.translate(s.x, FLOOR - lift).rotate(rot);
  m.scale(K * sx, K * sy).translate(-PX, -PY);
  return m;
}

const tipOf = (sh, hand) => { const dv = sub(hand, sh); const d = dist(dv) === 0 ? 1 : dist(dv); return add(sh, mul(dv, (d + 20) / d)); };
const bagAt = (s, tip) => P(tip.x, tip.y + 0.4 * s.bh / K);
const carryOf = (s, ar) => (s.bag === 'hand' || s.fling > 0 ? (s.throwArm ?? (ar.face > 0 ? 'b' : 'a')) : null);
function handWorld(s) {
  const m = matrixOf(s); const ar = arms(s, m);
  const k = carryOf(s, ar) ?? (ar.face > 0 ? 'b' : 'a');
  const tip = k === 'b' ? tipOf(ar.shB, ar.b) : tipOf(ar.shA, ar.a);
  return m.map(bagAt(s, tip));
}
function pullWorld(s) { const m = matrixOf(s); const ar = arms(s, m); return m.map(lerp2(ar.a, ar.b, 0.5)); }

// ── QOP HOLATI ──
const H_OFF = HW * 0.55;
const bagLen = (s) => 0.9 * s.bh * (1 + (1 - s.bagSq) * 0.35);
const bagWW = (s) => s.bw * s.bagSq;
const knotY = (s) => FLOOR - bagWW(s) / 2 * 0.85 + 2;
function setKnot(s, knotX) { s.bagAng = PI / 2; s.bagPx = knotX - bagLen(s); s.knot = P(knotX, knotY(s)); }

function endState(s) {
  if (!s.hasEnd) return null;
  const t = s.sec, d = s.bw; let sc, inside = true;
  if (s.endStuck) {
    if (t < s.endFrom) return null;
    sc = lerp(0.78, 1, eio(seg(t, s.endFrom, 2.1))) + 0.03 * s.jerk;
    if (t >= s.endPop) { inside = false; sc = lerp(1, 1.05, eout(seg(t, s.endPop, s.endPop + 0.1))); }
  } else {
    if (t < s.endFrom || t >= s.endOut) return null;
    sc = lerp(0.75, 1, eio(seg(t, s.endFrom, s.endAt)));
    if (t >= s.endAt) { inside = false; sc = 1; }
  }
  const tip = s.endStuck ? seg(t, s.endPop, s.endPop + 0.35) : (t >= s.endAt ? seg(t, s.endAt, s.endOut) : 0);
  if (tip >= 1) return null;
  const ry = d / 2 * 0.92 * sc;
  return { cx: DC, cy: FLOOR - ry + 2 - (inside ? (1 - sc) * 40 : 0), d, sc, ry, tip, inside };
}

function resolve(s) {
  if (s.bag !== 'drag') return s;
  const t = s.sec;
  const e = s.endS = endState(s);
  s.side = false; s.knot = null;
  if (e && (s.holding || s.stuck)) s.knot = P(e.cx + 18, e.cy);
  if (s.tier === 3) {
    if (t >= s.endAt) {
      s.side = t >= s.endOut;
      setKnot(s, (t < s.dropAt ? s.x : DRAG_X) - H_OFF);
      if (t >= s.dropAt) s.knot = null;
    }
    return s;
  }
  const pop = s.endPop;
  if (t < pop) return s;
  s.side = t >= pop + 0.35;
  const restPx = 65;
  s.bagSq = lerp(0.86, 1, backOut(seg(t, pop, pop + 0.4), 2.5));
  s.bagAng = PI / 2;
  s.bagPx = lerp(restPx - 70, restPx, eout(seg(t, pop, pop + 0.7)));
  const xSide = DR + 90, xLand = xSide + 80;
  s.x = lerp(xSide, xLand, s.flyBack);
  if (t >= 8.0) {
    const xGrab = restPx + bagLen(s) + H_OFF;
    if (t < 8.4) {
      s.x = lerp(xLand, xGrab, eio(seg(t, 8.0, 8.4)));
      s.knot = P(restPx + bagLen(s), knotY(s));
    } else {
      s.x = lerp(xGrab, DRAG_X, s.dragU);
      setKnot(s, (t < s.dropAt ? s.x : DRAG_X) - H_OFF);
      if (t >= s.dropAt) s.knot = null;
    }
  }
  if (t > 17.7) s.x = walk(t, 17.7, 21.0, DRAG_X, DC, 40).x;
  return s;
}

// ── RANG VA CHIZISH YORDAMCHILARI ──
const col = (v) => `rgba(${(v >>> 16) & 255},${(v >>> 8) & 255},${v & 255},${(((v >>> 24) & 255) / 255).toFixed(3)})`;
const rgba = (r, g, b, a) => `rgba(${r},${g},${b},${a})`;
function lerpCol(a, b, t) {
  const ch = (v, sh) => (v >>> sh) & 255;
  return `rgb(${Math.round(lerp(ch(a, 16), ch(b, 16), t))},${Math.round(lerp(ch(a, 8), ch(b, 8), t))},${Math.round(lerp(ch(a, 0), ch(b, 0), t))})`;
}
function linG(ctx, x0, y0, x1, y1, stops) { const g = ctx.createLinearGradient(x0, y0, x1, y1); stops.forEach(([o, c]) => g.addColorStop(o, c)); return g; }
const fill = (ctx, path, style, rule) => { ctx.fillStyle = style; rule ? ctx.fill(path, rule) : ctx.fill(path); };
function stroke(ctx, path, w, style, cap = 'butt', join = 'miter') {
  ctx.lineWidth = w; ctx.strokeStyle = style; ctx.lineCap = cap; ctx.lineJoin = join; ctx.stroke(path);
}
function rectP(x, y, w, h) { const p = new Path2D(); p.rect(x, y, w, h); return p; }
function ovalP(cx, cy, w, h) { const p = new Path2D(); p.ellipse(cx, cy, Math.max(0, w / 2), Math.max(0, h / 2), 0, 0, 2 * PI); return p; }
function line(ctx, x0, y0, x1, y1, w, style, cap = 'butt') { const p = new Path2D(); p.moveTo(x0, y0); p.lineTo(x1, y1); stroke(ctx, p, w, style, cap); }
function circleP(cx, cy, r) { const p = new Path2D(); p.arc(cx, cy, r, 0, 2 * PI); return p; }
const INK = '#121317';

// ── RASSOM ──
function arm(ctx, sh, hand) {
  const dv = sub(hand, sh); const l = dist(dv) + 24;
  ctx.save(); ctx.translate(sh.x, sh.y); ctx.rotate(Math.atan2(dv.y, dv.x));
  const p = new Path2D();
  p.moveTo(-8, -30); p.quadraticCurveTo(l * 0.45, -36, l * 0.72, -26);
  p.bezierCurveTo(l + 8, -18, l + 8, 18, l * 0.72, 24);
  p.quadraticCurveTo(l * 0.45, 32, -8, 30);
  fill(ctx, p, linG(ctx, 0, -30, 0, 30, [[0, '#FFE066'], [1, '#FFC81F']]));
  stroke(ctx, p, 10, '#FA9016', 'round', 'round');
  const hl = new Path2D(); hl.moveTo(l * 0.22, -17); hl.quadraticCurveTo(l * 0.45, -22, l * 0.62, -16);
  stroke(ctx, hl, 8, 'rgba(255,255,255,0.902)', 'round', 'round');
  ctx.restore();
}

function part(ctx, name, flat) {
  const p = PARTS[name]; const path = pathOf(flat);
  if (p.f) fill(ctx, path, p.f);
  if (p.s) stroke(ctx, path, p.w, p.s, p.b ? 'butt' : 'round', 'round');
}

function bag(ctx, c, w, h, sq, rot, local = false) {
  ctx.save(); ctx.translate(c.x, c.y); ctx.rotate(rot);
  const ww = w * sq, hh = h * (1 + (1 - sq) * 0.35);
  const lw = local ? 10 : clamp(h * 0.035, 3.2, 7);
  const top = -hh / 2, bot = hh / 2, neck = ww * 0.13;
  const body = new Path2D();
  body.moveTo(-neck, top + hh * 0.16);
  body.bezierCurveTo(-ww * 0.62, top + hh * 0.26, -ww * 0.6, bot - hh * 0.02, -ww * 0.32, bot);
  body.bezierCurveTo(-ww * 0.12, bot + hh * 0.04, ww * 0.12, bot + hh * 0.04, ww * 0.32, bot);
  body.bezierCurveTo(ww * 0.6, bot - hh * 0.02, ww * 0.62, top + hh * 0.26, neck, top + hh * 0.16);
  body.closePath();
  fill(ctx, body, linG(ctx, -ww / 2, 0, ww / 2, 0, [[0, '#4A4F58'], [1, '#2C2F35']]));
  stroke(ctx, body, lw, INK, 'round', 'round');
  const kt = top + hh * 0.16;
  const knot = new Path2D();
  knot.moveTo(-neck, kt);
  knot.bezierCurveTo(-ww * 0.3, top + hh * 0.04, -ww * 0.3, top - hh * 0.06, -ww * 0.17, top - hh * 0.05);
  knot.bezierCurveTo(-ww * 0.1, top - hh * 0.04, -ww * 0.04, top + hh * 0.06, 0, top + hh * 0.1);
  knot.bezierCurveTo(ww * 0.04, top + hh * 0.06, ww * 0.1, top - hh * 0.04, ww * 0.17, top - hh * 0.05);
  knot.bezierCurveTo(ww * 0.3, top - hh * 0.06, ww * 0.3, top + hh * 0.04, neck, kt);
  knot.closePath();
  fill(ctx, knot, '#3D4149');
  stroke(ctx, knot, lw, INK, 'round', 'round');
  fill(ctx, ovalP(0, kt - hh * 0.01, ww * 0.2, hh * 0.07), INK);
  const f1 = new Path2D(); f1.moveTo(-ww * 0.05, top + hh * 0.28); f1.quadraticCurveTo(-ww * 0.14, top + hh * 0.55, -ww * 0.08, top + hh * 0.78);
  stroke(ctx, f1, lw * 0.65, 'rgba(18,19,23,0.749)', 'round');
  const f2 = new Path2D(); f2.moveTo(ww * 0.2, top + hh * 0.3); f2.quadraticCurveTo(ww * 0.28, top + hh * 0.55, ww * 0.2, top + hh * 0.74);
  stroke(ctx, f2, lw * 0.65, 'rgba(18,19,23,0.749)', 'round');
  const f3 = new Path2D(); f3.moveTo(-ww * 0.36, top + hh * 0.42); f3.quadraticCurveTo(-ww * 0.4, top + hh * 0.6, -ww * 0.3, top + hh * 0.78);
  stroke(ctx, f3, lw * 0.8, 'rgba(255,255,255,0.451)', 'round');
  ctx.restore();
}

function chick(ctx, s) {
  const m = matrixOf(s);
  const lift = s.lift + (s.ph >= 0 ? Math.sin((s.ph % 1) * PI) * 16 : 0) + s.hop * 46;
  const lie = s.pivotR ? Math.sin(s.rot) : 0;
  fill(ctx, ovalP(s.x + lie * HW, FLOOR + 3 + 14 * s.depth, 2 * (HW * 1.05 * (1 - lift / 250) + lie * HW * 0.6), 18),
    `rgba(0,0,0,${clamp(0.3 - lift / 300, 0, 1).toFixed(3)})`);
  ctx.save();
  ctx.transform(m.a, m.b, m.c, m.d, m.e, m.f);
  const y = s.yaw; const ar = arms(s, m);
  const near = ar.face > 0 ? 'b' : 'a'; const carry = carryOf(s, ar); const gripB = s.grip > 0.5;
  const backV = y < -1.3;
  if (backV) { arm(ctx, ar.shA, ar.a); arm(ctx, ar.shB, ar.b); }
  else if (Math.abs(y) > 0.35) {
    const far = near === 'b' ? 'a' : 'b';
    if (!(far === 'b' && gripB) && far !== carry) { if (far === 'a') arm(ctx, ar.shA, ar.a); else arm(ctx, ar.shB, ar.b); }
  }
  for (const n of ['body', 'head_bl3', 'head', 'head_bl1', 'head_bl2']) part(ctx, n, pose(n, y));
  ctx.save();
  if (y < -1) { const cp = pathOf(pose('body', y)); cp.addPath(pathOf(pose('head', y))); ctx.clip(cp); }
  for (const n of ['beak', 'beak_bl', 'mouth']) part(ctx, n, pose(n, y));
  const e1 = pose('eye', y), e2 = pose('eye_2', y); const c1 = bbox(e1), c2 = bbox(e2);
  const ink = (p, w, c = '#000') => stroke(ctx, p, w, c, 'round', 'round');
  if (s.eyes === 'happy') { part(ctx, 'eye', flipY(e1)); part(ctx, 'eye_2', flipY(e2)); }
  else if (s.eyes === 'strain') {
    for (const [c, d] of [[c1, 1], [c2, -1]]) {
      const p = new Path2D(); p.moveTo(c.cx - 16 * d, c.cy - 14); p.lineTo(c.cx + 12 * d, c.cy); p.lineTo(c.cx - 16 * d, c.cy + 14); ink(p, 12);
    }
  } else { part(ctx, 'eye', e1); part(ctx, 'eye_2', e2); }
  if (s.eyes === 'cry' || s.eyes === 'strain') {
    const kq = s.eyes === 'strain' ? -1 : 1;
    for (const [c, d] of [[c1, 1], [c2, -1]]) {
      const p = new Path2D(); p.moveTo(c.cx - 26 * d, c.cy - 34 + 10 * kq); p.lineTo(c.cx + 18 * d, c.cy - 38 - 6 * kq); ink(p, 10);
    }
  }
  if (s.tears > 0) {
    for (const [c, d] of [[c1, -1], [c2, 1]]) {
      const p = new Path2D(); p.moveTo(c.cx + 16 * d, c.cy + 8); p.quadraticCurveTo(c.cx + 28 * d, c.cy + 40, c.cx + 24 * d, c.cy + 70);
      ink(p, 11, 'rgba(120,200,255,0.902)');
    }
  }
  if (s.sweat > 0) {
    const ph = (s.sec * 1.6) % 1; const px = y < 0 ? 185 : 420, py = 50 + ph * 60;
    const drop = new Path2D(); drop.moveTo(px, py - 28); drop.quadraticCurveTo(px + 18, py, px, py + 12); drop.quadraticCurveTo(px - 18, py, px, py - 28); drop.closePath();
    const alpha = (s.sweat * (1 - ph * 0.6)).toFixed(3);
    fill(ctx, drop, `rgba(143,211,255,${alpha})`); ink(drop, 6, `rgba(61,143,196,${alpha})`);
  }
  ctx.restore();
  if (backV) { ctx.restore(); return; }
  if (s.bag === 'hand' && carry) {
    const tip = carry === 'b' ? tipOf(ar.shB, ar.b) : tipOf(ar.shA, ar.a);
    const sw = (s.ph >= 0 ? 0.16 * Math.sin(s.ph * PI) : 0.05 * Math.sin(s.sec * 3)) + s.wind * 0.5 * ar.face;
    ctx.save(); ctx.translate(tip.x, tip.y); ctx.rotate(sw);
    bag(ctx, P(0, 0.4 * s.bh / K), s.bw / K, s.bh / K, 1, 0, true);
    ctx.restore();
  }
  const side = Math.abs(y) > 0.35;
  if (!side || near === 'a' || carry === 'a') arm(ctx, ar.shA, ar.a);
  if ((!side || near === 'b' || carry === 'b') && !gripB) arm(ctx, ar.shB, ar.b);
  ctx.restore();
}

function dragBag(ctx, s) {
  if (s.bag !== 'drag' || Number.isNaN(s.bagPx) || !s.side) return;
  const sq = s.bagSq, hh = s.bh * (1 + (1 - sq) * 0.35), ww = s.bw * sq;
  const py = FLOOR - (ww / 2) * Math.abs(Math.sin(s.bagAng)) * 0.85 + 2;
  ctx.save(); ctx.translate(s.bagPx, py); ctx.rotate(s.bagAng);
  bag(ctx, P(0, -hh / 2), s.bw, s.bh, sq, 0);
  ctx.restore();
}

const FOLDS = [[0.3, 0.88, 0.3], [1.05, 0.62, -0.25], [1.7, 0.8, 0.4], [2.55, 0.7, -0.3], [3.3, 0.9, 0.2], [4.0, 0.55, -0.35], [4.75, 0.78, 0.3], [5.5, 0.66, -0.2]];
function bagEnd(ctx, s, e) {
  let rx = e.d / 2 * e.sc, ry = e.ry, cx = e.cx, cy = e.cy;
  if (e.tip > 0 && !Number.isNaN(s.bagPx)) {
    const sq = s.bagSq, hh = s.bh * (1 + (1 - sq) * 0.35), ww = s.bw * sq;
    const u = eout(e.tip), th = u * PI / 2; const sn = Math.sin(th), cs = Math.cos(th);
    const py = FLOOR - ww / 2 * 0.85 + 2; const c = lerp(e.cx, s.bagPx + hh / 2, u);
    cy = lerp(cy, py, u); ry = lerp(ry, ww / 2 * 0.9, u);
    fill(ctx, ovalP(c, FLOOR + 2, 2 * (hh / 2 * sn + rx * cs) * 0.95, 20), 'rgba(0,0,0,0.302)');
    if (sn > 0.02) {
      ctx.save(); ctx.translate(c, py); ctx.scale(sn, 1); ctx.translate(-hh / 2, 0); ctx.rotate(PI / 2);
      bag(ctx, P(0, -hh / 2), s.bw, s.bh, sq, 0);
      ctx.restore();
    }
    rx *= cs; cx = c + hh / 2 * sn;
    if (rx < 2) return;
  } else {
    if (rx <= 0 || ry <= 0) return;
    fill(ctx, ovalP(cx, FLOOR + 2, 2 * rx * 0.95, 20), 'rgba(0,0,0,0.302)');
  }
  ctx.save(); ctx.translate(cx, cy);
  const oval = ovalP(0, 0, 2 * rx, 2 * ry);
  const g = ctx.createRadialGradient(-rx * 0.3, -ry * 0.35, rx * 0.1, 0, 0, rx);
  g.addColorStop(0, '#555A64'); g.addColorStop(1, '#25282D');
  fill(ctx, oval, g);
  stroke(ctx, oval, 6, INK);
  for (const [a, l, bend] of FOLDS) {
    const p = new Path2D();
    p.moveTo(Math.cos(a) * rx * l, Math.sin(a) * ry * l);
    p.quadraticCurveTo(Math.cos(a + bend) * rx * l * 0.55, Math.sin(a + bend) * ry * l * 0.55, Math.cos(a) * rx * 0.12, Math.sin(a) * ry * 0.12);
    stroke(ctx, p, 4, 'rgba(18,19,23,0.749)', 'round');
  }
  const arc = new Path2D(); arc.ellipse(0, 0, rx * 0.78, ry * 0.78, 0, PI * 1.08, PI * 1.08 + PI * 0.3);
  stroke(ctx, arc, 6, 'rgba(255,255,255,0.349)');
  fill(ctx, circleP(0, 0, ry * 0.13), INK);
  ctx.restore();
}

function neck(ctx, s) {
  const e = s.endS;
  if (!e || !(s.holding || s.stuck)) return;
  const h = pullWorld(s); const a = P(e.cx, e.cy);
  const c = P((a.x + h.x) / 2, (a.y + h.y) / 2 + (s.stuck ? 4 : 14));
  const at = (u) => add(add(mul(a, (1 - u) * (1 - u)), mul(c, 2 * u * (1 - u))), mul(h, u * u));
  const w0 = Math.min(54, e.ry * 0.42), w1 = 14; const left = [], right = []; const n = 14;
  for (let i = 0; i <= n; i++) {
    const u = i / n; const p = at(u);
    const dv = sub(at(Math.min(1, u + 0.01)), at(Math.max(0, u - 0.01))); const d = dist(dv) === 0 ? 1 : dist(dv);
    const nv = P(-dv.y / d, dv.x / d); const w = w1 + (w0 - w1) * Math.pow(1 - u, 2.2);
    left.push(add(p, mul(nv, w / 2))); right.push(sub(p, mul(nv, w / 2)));
  }
  const body = new Path2D(); body.moveTo(left[0].x, left[0].y);
  left.slice(1).forEach((p) => body.lineTo(p.x, p.y));
  right.slice().reverse().forEach((p) => body.lineTo(p.x, p.y));
  body.closePath();
  fill(ctx, body, '#34383F'); stroke(ctx, body, 5, INK, 'butt', 'round');
  for (const k of [0.3, 0.7]) {
    const f = new Path2D();
    for (let i = 0; i <= 10; i++) { const q = lerp2(left[i], right[i], k); if (i === 0) f.moveTo(q.x, q.y); else f.lineTo(q.x, q.y); }
    stroke(ctx, f, 3, 'rgba(18,19,23,0.702)', 'round');
  }
  fill(ctx, circleP(h.x, h.y, 11), '#3D4149'); stroke(ctx, circleP(h.x, h.y, 11), 4, INK);
}

function tearDrops(ctx, s) {
  if (s.tears <= 0) return;
  const m = matrixOf(s); const g = 2400;
  for (const [n, d, off] of [['eye', -1, 0], ['eye_2', 1, 0.31]]) {
    const c = bbox(pose(n, s.yaw)); const p = m.map(P(c.cx + 24 * d, c.cy + 72));
    for (let i = 0; i < 3; i++) {
      const age = (s.sec + off + i * 0.23) % 0.69;
      const yy = p.y + 0.5 * g * age * age, xx = p.x + d * 30 * age;
      if (yy < FLOOR) fill(ctx, ovalP(xx, yy, 7, 10), 'rgba(150,215,255,0.949)');
      else {
        const tg = Math.sqrt(Math.max(0, 2 * (FLOOR - p.y) / g)); const a = age - tg;
        if (a < 0.15) {
          const k = a / 0.15; const p2 = new Path2D();
          p2.ellipse(p.x + d * 30 * tg, FLOOR, 4 + 10 * k, 2 + 2 * k, 0, PI, 2 * PI);
          stroke(ctx, p2, 2.5, `rgba(150,215,255,${clamp(1 - k, 0, 1).toFixed(3)})`);
        }
      }
    }
  }
}

function flying(ctx, s, a) {
  if (s.bag !== 'fly') return;
  const u = seg(s.sec, s.flyT0, s.flyT1);
  const x = lerp(a.x, BC, u); const y = lerp(a.y, BT + 6, u) - 4 * s.flyPeak * u * (1 - u);
  ctx.save();
  if (u > 0.7) { ctx.beginPath(); ctx.rect(0, 0, W, BT + 2); ctx.clip(); }
  const st = 1 + 0.12 * Math.sin(u * PI);
  bag(ctx, P(x, y), s.bw / st, s.bh * st, 1, u * 0.9 + Math.sin(u * PI) * 0.35);
  ctx.restore();
}

function ground(ctx) {
  fill(ctx, rectP(0, FLOOR, W, H - FLOOR), '#1F1A16');
  fill(ctx, rectP(0, FLOOR, W, 8), '#2B241E');
}

function fence(ctx) {
  const top = FLOOR - 150, step = 74; const ink = '#4A2A12';
  for (const yy of [top + 40, top + 108]) {
    const r = rectP(WALL_R, yy, W - WALL_R, 18);
    fill(ctx, r, '#7A4A26'); stroke(ctx, r, 4, ink);
  }
  for (let x = WALL_R + 20; x < W + step; x += step) {
    const p = new Path2D();
    p.moveTo(x, FLOOR); p.lineTo(x, top + 14); p.lineTo(x + 20, top); p.lineTo(x + 40, top + 14); p.lineTo(x + 40, FLOOR); p.closePath();
    fill(ctx, p, linG(ctx, x, 0, x + 40, 0, [[0, '#9C6436'], [1, '#83512A']]));
    stroke(ctx, p, 5, ink, 'butt', 'round');
    line(ctx, x + 9, top + 24, x + 9, FLOOR - 14, 4, 'rgba(255,220,170,0.22)');
    fill(ctx, circleP(x + 20, top + 49, 3.5), ink);
    fill(ctx, circleP(x + 20, top + 117, 3.5), ink);
  }
  fill(ctx, rectP(WALL_R, top - 2, W - WALL_R, FLOOR - top + 2), 'rgba(20,20,20,0.278)');
}

function houseBack(ctx, s) {
  const r = rectP(DL, DT, DR - DL, FLOOR - DT);
  fill(ctx, r, linG(ctx, 0, DT, 0, FLOOR, [[0, '#1D140E'], [1, '#3A271B']]));
  fill(ctx, r, `rgba(255,200,120,${(0.25 * s.door).toFixed(3)})`);
  const sill = rectP(DL - 14, FLOOR - 3, DR - DL + 28, 10);
  fill(ctx, sill, '#9A9792');
  fill(ctx, rectP(DL - 14, FLOOR - 3, DR - DL + 28, 3), '#B5B2AC');
  stroke(ctx, sill, 3, '#5A5855');
}

function houseFront(ctx) {
  const wall = new Path2D();
  wall.moveTo(-10, FLOOR); wall.lineTo(-10, 170); wall.lineTo(WALL_R, 262); wall.lineTo(WALL_R, FLOOR); wall.closePath();
  wall.rect(DL, DT, DR - DL, FLOOR - DT);
  fill(ctx, wall, linG(ctx, 0, 0, WALL_R, 0, [[0, '#E9C79A'], [1, '#D4AB7A']]), 'evenodd');
  ctx.save(); ctx.clip(wall, 'evenodd');
  for (let yy = 200; yy < FLOOR; yy += 34) line(ctx, -10, yy, WALL_R, yy, 4, 'rgba(150,100,64,0.349)');
  ctx.restore();
  line(ctx, WALL_R, 262, WALL_R, FLOOR, 8, '#7A4A26');
  for (const [x0, x1] of [[-10, DL - 14], [DR + 14, WALL_R]]) {
    const base = rectP(x0, FLOOR - 22, x1 - x0, 22);
    fill(ctx, base, '#8C8A86'); stroke(ctx, base, 5, '#5A5855');
  }
  for (const [x, y, w, h] of [[DL - 14, DT - 14, 14, FLOOR - DT + 14], [DR, DT - 14, 14, FLOOR - DT + 14], [DL - 14, DT - 14, DR - DL + 28, 14]]) {
    const r = rectP(x, y, w, h); fill(ctx, r, '#AA5A1F'); stroke(ctx, r, 7, '#6D3605');
  }
  const roofEdge = new Path2D();
  roofEdge.moveTo(-20, 110); roofEdge.lineTo(WALL_R + 46, 262); roofEdge.lineTo(WALL_R + 46, 286); roofEdge.lineTo(-20, 140); roofEdge.closePath();
  fill(ctx, roofEdge, '#C4463A'); stroke(ctx, roofEdge, 7, '#7A2620', 'butt', 'round');
  const tiles = new Path2D();
  tiles.moveTo(-20, 30); tiles.lineTo(WALL_R + 46, 262); tiles.lineTo(-20, 262); tiles.closePath();
  fill(ctx, tiles, '#D3554A');
  ctx.save(); ctx.clip(tiles);
  for (let i = 0; i < 8; i++) { const o = i * 34; line(ctx, -20, 30 + o, WALL_R + 46, 262 + o, 5, 'rgba(122,38,32,0.702)'); }
  ctx.restore();
  const edge = new Path2D(); edge.moveTo(-20, 30); edge.lineTo(WALL_R + 46, 262);
  stroke(ctx, edge, 7, '#7A2620', 'butt', 'round');
}

function doorPanel(ctx, s) {
  const phi = s.door * 1.95; const x0 = DL, y0 = DT, y1 = FLOOR, pw = DR - DL;
  const xf = x0 + pw * Math.cos(phi); const yc = (y0 + y1) / 2; const hh = (y1 - y0) / 2 * (1 + Math.sin(phi) * 0.12);
  const q = new Path2D(); q.moveTo(x0, y0); q.lineTo(xf, yc - hh); q.lineTo(xf, yc + hh); q.lineTo(x0, y1); q.closePath();
  const back = Math.cos(phi) < 0; const lit = Math.abs(Math.cos(phi));
  const c0 = back ? lerpCol(0xFF7A3F14, 0xFFA8612B, lit) : lerpCol(0xFFA8612B, 0xFFD98A45, lit);
  const c1 = back ? lerpCol(0xFF6A3510, 0xFF93531F, lit) : lerpCol(0xFF93531F, 0xFFC7773A, lit);
  fill(ctx, q, linG(ctx, x0, 0, xf === x0 ? x0 + 1 : xf, 0, [[0, c0], [1, c1]]));
  const at = (u, v) => P(lerp(x0, xf, u), lerp(lerp(y0, yc - hh, u), lerp(y1, yc + hh, u), v));
  if (Math.abs(xf - x0) > 10) {
    for (const v of [[0.08, 0.42], [0.52, 0.92]]) {
      const p = new Path2D();
      p.moveTo(at(0.16, v[0]).x, at(0.16, v[0]).y); p.lineTo(at(0.84, v[0]).x, at(0.84, v[0]).y);
      p.lineTo(at(0.84, v[1]).x, at(0.84, v[1]).y); p.lineTo(at(0.16, v[1]).x, at(0.16, v[1]).y); p.closePath();
      fill(ctx, p, 'rgba(0,0,0,0.102)'); stroke(ctx, p, 5, 'rgba(109,54,5,0.902)', 'butt', 'round');
    }
    const kc = at(0.86, 0.55); const kr = ovalP(kc.x, kc.y, 2 * (8 * lit + 2), 16);
    fill(ctx, kr, '#FFD527'); stroke(ctx, kr, 4, '#6D3605');
  }
  stroke(ctx, q, 7, '#6D3605', 'butt', 'round');
}

const MOUTH = { cx: BC, cy: BT, w: BR - BL + 16, h: 26 };
function binBack(ctx, s) {
  ctx.save(); ctx.translate(BC, BT); ctx.rotate(s.binWob * 0.06); ctx.translate(-BC, -BT);
  fill(ctx, ovalP(MOUTH.cx, MOUTH.cy, MOUTH.w, MOUTH.h), '#163F49');
  ctx.restore();
}
function binFront(ctx, s) {
  ctx.save(); ctx.translate(BC, FLOOR); ctx.rotate(s.binWob * 0.06); ctx.translate(-BC, -FLOOR);
  const body = new Path2D();
  body.moveTo(BL, BT); body.lineTo(BR, BT); body.lineTo(BR - 12, FLOOR - 14);
  body.quadraticCurveTo(BR - 14, FLOOR, BR - 30, FLOOR); body.lineTo(BL + 30, FLOOR);
  body.quadraticCurveTo(BL + 14, FLOOR, BL + 12, FLOOR - 14); body.closePath();
  fill(ctx, body, linG(ctx, BL, 0, BR, 0, [[0, '#58B3C9'], [0.55, '#3F97AD'], [1, '#2E7B8F']]));
  const ink = '#1B4D59';
  stroke(ctx, body, 7, ink, 'round', 'round');
  for (const f of [0.3, 0.5, 0.7]) {
    const x = lerp(BL + 18, BR - 18, f);
    line(ctx, x, BT + 26, lerp(x, BC, 0.08), FLOOR - 22, 5, 'rgba(27,77,89,0.749)', 'round');
  }
  line(ctx, BL + 18, BT + 24, BL + 24, BT + 84, 6, 'rgba(255,255,255,0.549)', 'round');
  stroke(ctx, ovalP(MOUTH.cx, MOUTH.cy, MOUTH.w, MOUTH.h), 7, ink, 'round', 'round');
  ctx.restore();
}

function puffFx(ctx, s) {
  if (!(s.puff > 0 && s.puff < 1)) return;
  const a = 1 - eio(s.puff), k = eout(s.puff);
  ctx.save(); ctx.globalAlpha = 0.9 * a;
  for (const d of [[-48, -6, 13, 1.0], [-22, -26, 17, 0.8], [6, -30, 16, 0.9], [32, -20, 14, 1.0], [54, -4, 11, 1.1]]) {
    fill(ctx, circleP(BC + d[0] * (1 + k * 0.7 * d[3]), BT - 10 + d[1] * (0.6 + k * 0.9), Math.max(0, d[2] * (0.5 + k * 0.8) * (1 - 0.3 * s.puff))), '#C7CCC1');
  }
  ctx.restore();
}

function sparksFx(ctx, s) {
  if (!(s.spark > 0 && s.spark < 1)) return;
  const pts = [[BL - 14, BT - 80, 1.0], [BR + 8, BT - 60, 0.8], [BC + 14, BT - 115, 1.1]];
  for (let i = 0; i < pts.length; i++) {
    const u = clamp(s.spark * 1.4 - i * 0.18, 0, 1);
    if (u <= 0 || u >= 1) continue;
    const r = Math.sin(u * PI) * 15 * pts[i][2];
    ctx.save(); ctx.translate(pts[i][0], pts[i][1] - u * 22); ctx.rotate(u * 1.5);
    const star = new Path2D();
    for (let j = 0; j < 8; j++) {
      const an = j * PI / 4; const rr = j % 2 === 1 ? r * 0.35 : r;
      if (j === 0) star.moveTo(Math.cos(an) * rr, Math.sin(an) * rr); else star.lineTo(Math.cos(an) * rr, Math.sin(an) * rr);
    }
    star.closePath(); fill(ctx, star, '#FFD527');
    ctx.restore();
  }
}

function paintScene(ctx, size, bytes, sec) {
  const k = Math.min(size.w / W, size.h / H);
  ctx.save();
  ctx.translate((size.w - W * k) / 2, (size.h - H * k) / 2); ctx.scale(k, k);
  ctx.beginPath(); ctx.rect(0, 0, W, H); ctx.clip();
  const s = resolve(new S(bytes, sec));
  let release = null;
  if (s.bag === 'fly' || s.bag === 'bin') release = handWorld(new S(bytes, s.flyT0 - 0.0001));
  const chickIn = s.depth < -0.02; const e = s.endS; const overBin = s.x > BL - HW;
  ground(ctx); fence(ctx); houseBack(ctx, s); binBack(ctx, s);
  if (e && e.inside) bagEnd(ctx, s, e);
  if (chickIn) { neck(ctx, s); chick(ctx, s); }
  houseFront(ctx); doorPanel(ctx, s);
  if (e && !e.inside) bagEnd(ctx, s, e);
  dragBag(ctx, s);
  if (!chickIn) neck(ctx, s);
  if (!chickIn && !overBin) chick(ctx, s);
  tearDrops(ctx, s);
  if (s.grip > 0.5) {
    const m = matrixOf(s); const ar = arms(s, m);
    ctx.save(); ctx.beginPath(); ctx.rect(0, 0, DR, H); ctx.clip();
    ctx.transform(m.a, m.b, m.c, m.d, m.e, m.f);
    arm(ctx, ar.shB, ar.b);
    ctx.restore();
  }
  if (release) flying(ctx, s, release);
  binFront(ctx, s);
  if (overBin) chick(ctx, s);
  puffFx(ctx, s); sparksFx(ctx, s);
  ctx.restore();
}

/** Animatsiyani [host] ichiga qo'yadi (bir marta o'ynaydi); to'xtatish funksiyasini qaytaradi. */
export function mountChick(host, bytes) {
  const cv = document.createElement('canvas');
  cv.style.cssText = 'width:100%;height:100%;display:block';
  host.innerHTML = ''; host.appendChild(cv);
  const dur = durationOf(tierOf(bytes));
  let raf = 0; let t0 = 0;
  const frame = (now) => {
    if (!t0) t0 = now;
    const sec = Math.min(dur, (now - t0) / 1000);
    const dpr = window.devicePixelRatio || 1;
    const w = host.clientWidth || 340, h = host.clientHeight || 170;
    if (cv.width !== Math.round(w * dpr) || cv.height !== Math.round(h * dpr)) { cv.width = Math.round(w * dpr); cv.height = Math.round(h * dpr); }
    const ctx = cv.getContext('2d');
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, w, h);
    try { paintScene(ctx, { w, h }, bytes, sec); } catch (_) { /* kadr tashlanadi */ }
    raf = requestAnimationFrame(frame);
  };
  raf = requestAnimationFrame(frame);
  return () => cancelAnimationFrame(raf);
}

/** Sinov uchun: berilgan soniyadagi kadrni chizadi. */
export function drawChickAt(canvas, bytes, sec) {
  const ctx = canvas.getContext('2d');
  ctx.clearRect(0, 0, canvas.width, canvas.height);
  paintScene(ctx, { w: canvas.width, h: canvas.height }, bytes, sec);
}
