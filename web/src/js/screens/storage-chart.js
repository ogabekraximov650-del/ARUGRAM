// Halqa diagramma — `lib/screens/storage_screen.dart` dagi `_CacheChart` / `_ChartPainter` ning web (Canvas 2D)
// nusxasi: bo'laklar ulangan holda siljiydi (3200 ms), zarrachalar, bosilganda 9 px kattalashish,
// halqa tashqarisidagi foizlar (ustma-ust tushmaydi), o'rtada hajm, "tayyor" yashil belgi, tozalashda bo'shash.

const RADIUS = 86, THICKNESS = 38, BOX = 270;
const GRAY = '#8A8A8E';
const PI = Math.PI;
const clamp = (v, a, b) => Math.max(a, Math.min(b, v));
const lerp = (a, b, t) => a + (b - a) * t;
const easeIO = (x) => (x < 0.5 ? 4 * x * x * x : 1 - Math.pow(-2 * x + 2, 3) / 2);

const hex = (h) => [parseInt(h.slice(1, 3), 16), parseInt(h.slice(3, 5), 16), parseInt(h.slice(5, 7), 16)];
const blend = (h, over, a) => { const c = hex(h); const o = hex(over); return c.map((v, i) => Math.round(v * (1 - a) + o[i] * a)); };
const rgb = (c, a = 1) => `rgba(${c[0]},${c[1]},${c[2]},${a})`;

export const pctText = (p) => (p > 0 && p < 0.005 ? '<0.01%' : `${p.toFixed(2)}%`);

/** Foizlar yuzdan bir aniqlikda, jami 100 (`_roundPercents`). */
export function roundPercents(shares) {
  const raw = shares.map((s) => s * 10000); const out = raw.map((r) => Math.floor(r));
  let left = Math.round(raw.reduce((a, b) => a + b, 0)) - out.reduce((a, b) => a + b, 0);
  raw.map((_, i) => i).sort((a, b) => (raw[b] - out[b]) - (raw[a] - out[a])).forEach((i) => { if (left > 0) { out[i]++; left--; } });
  return out.map((o) => o / 100);
}

function shares(slices) {
  const list = slices.filter((s) => s.bytes > 0);
  const sum = list.reduce((a, s) => a + s.bytes, 0);
  const out = new Map();
  if (sum <= 0) return out;
  let under = 0; let minus = 0;
  for (const s of list) { const p = s.bytes / sum; if (p > 0 && p < 0.02) { under++; minus += p; } }
  const percents = roundPercents(list.map((s) => s.bytes / sum));
  list.forEach((s, i) => {
    let p = s.bytes / sum;
    const textScale = p < 0.08 || percents[i] >= 100 ? 0.85 : 1.0;
    if (p < 0.02) p = 0.02; else p *= 1 - (0.02 * under - minus);
    out.set(s.label, { frac: p, pct: percents[i], textScale });
  });
  return out;
}

export class Chart {
  constructor(host, { onPress = () => {}, font = 'sans-serif', format = (b) => `${b} B` } = {}) {
    this.format = format;
    this.cv = document.createElement('canvas');
    this.cv.style.cssText = `display:block;width:100%;height:${BOX}px;touch-action:pan-y`;
    host.appendChild(this.cv);
    this.onPress = onPress; this.font = font;
    this.colors = {}; this.order = []; this.from = new Map(); this.to = new Map();
    this.fromBytes = 0; this.toBytes = 0; this.moveAt = -1e9; this.drain = null;
    this.loading = true; this.complete = false; this.slices = [];
    this.pressed = null; this.target = null; this.pressAt = 0; this.pressDir = 0; this.pressVal = 0;
    this.t0 = performance.now(); this.raf = 0; this.dead = false; this.labelsCache = null;
    const pos = (e) => { const r = this.cv.getBoundingClientRect(); return { x: e.clientX - r.left, y: e.clientY - r.top, w: r.width }; };
    const down = (e) => { if (this.interactive()) this.setPressed(this.hit(pos(e))); };
    this.cv.addEventListener('pointerdown', down);
    this.cv.addEventListener('pointermove', (e) => { if (e.buttons || e.pointerType === 'touch') down(e); });
    const up = () => this.setPressed(null);
    this.cv.addEventListener('pointerup', up); this.cv.addEventListener('pointercancel', up); this.cv.addEventListener('pointerleave', up);
    const frame = () => { if (this.dead) return; this.paint(); this.raf = requestAnimationFrame(frame); };
    this.raf = requestAnimationFrame(frame);
  }

  destroy() { this.dead = true; cancelAnimationFrame(this.raf); }
  interactive() { return !this.loading && !this.complete && this.drain == null; }
  now() { return performance.now(); }
  moveValue() { return clamp((this.now() - this.moveAt) / 3200, 0, 1); }
  t() { return easeIO(this.moveValue()); }
  bytesNow() { return lerp(this.fromBytes, this.toBytes, this.t()); }
  left() { return 1 - clamp(this.drain ?? 0, 0, 1); }

  currentShares() {
    const t = this.t(); const out = new Map();
    for (const k of this.order) {
      const a = this.from.get(k), b = this.to.get(k);
      if (!a && !b) continue;
      const s = {
        frac: lerp(a?.frac ?? 0, b?.frac ?? 0, t), pct: lerp(a?.pct ?? 0, b?.pct ?? 0, t),
        textScale: lerp(a?.textScale ?? b.textScale, b?.textScale ?? a.textScale, t),
      };
      if (s.frac > 0.00001) out.set(k, s);
    }
    return out;
  }

  mergeOrder() {
    const keep = new Set([...this.from.keys(), ...this.to.keys()]);
    const order = this.order.filter((k) => keep.has(k));
    const fresh = this.slices.filter((s) => keep.has(s.label) && !order.includes(s.label)).sort((a, b) => a.bytes - b.bytes);
    order.push(...fresh.map((s) => s.label));
    for (const k of keep) if (!order.includes(k)) order.push(k);
    this.order = order;
  }

  sameShares(a, b) {
    if (a.size !== b.size) return false;
    for (const [k, v] of a) { const o = b.get(k); if (!o || Math.abs(o.frac - v.frac) > 0.00001 || Math.abs(o.pct - v.pct) > 0.001) return false; }
    return true;
  }

  /** `didUpdateWidget`: yangi ma'lumot. */
  set({ slices, selectedBytes, loading, complete, drain }) {
    const old = this.drain;
    this.loading = !!loading; this.complete = !!complete;
    for (const s of slices) this.colors[s.label] = s.color;
    this.slices = slices;
    this.drain = drain ?? null;
    if (this.drain != null) {
      if (old == null) {
        const now = this.currentShares(); const bytes = this.bytesNow();
        this.moveAt = -1e9; this.from = now; this.to = now; this.fromBytes = bytes; this.toBytes = bytes;
      }
      return;
    }
    const next = shares(slices); const bytes = selectedBytes;
    if (old != null) {
      this.from = new Map(); this.fromBytes = 0; this.to = next; this.toBytes = bytes; this.mergeOrder(); this.moveAt = this.now(); return;
    }
    if (!this.sameShares(next, this.to) || bytes !== this.toBytes) {
      this.fromBytes = this.bytesNow(); this.toBytes = bytes; this.from = this.currentShares(); this.to = next; this.mergeOrder(); this.moveAt = this.now();
    }
  }

  visible() {
    const f = this.drain == null ? 1 : this.left();
    const sh = this.currentShares(); const out = new Map(); let at = 0;
    for (const k of this.order) {
      const s = sh.get(k); if (!s) continue;
      const half = s.frac * f * 180; const alpha = clamp(s.frac / 0.02, 0, 1);
      out.set(k, { center: at + half, half, pct: s.pct * f, textAlpha: alpha, textScale: s.textScale });
      at += half * 2;
    }
    return out;
  }
  visibleBytes() { return this.drain == null ? this.bytesNow() : this.bytesNow() * this.left(); }

  hit(p) {
    const cx = p.w / 2, cy = BOX / 2; const dx = p.x - cx, dy = p.y - cy; const r = Math.hypot(dx, dy);
    if (r <= RADIUS - THICKNESS || r >= RADIUS + 14) return null;
    let a = Math.atan2(dy, dx) * 180 / PI; if (a < 0) a += 360;
    for (const [k, v] of this.visible()) if (a >= v.center - v.half && a <= v.center + v.half) return k;
    return null;
  }

  setPressed(label) {
    if (label === this.target) return;
    this.target = label; this.onPress(label);
    const cur = this.pressNow();
    this.pressFrom = cur; this.pressAt = this.now();
    if (label != null) this.pressed = label;
    this.pressTo = label == null ? 0 : 1;
  }
  pressNow() {
    if (this.pressTo == null) return 0;
    const u = clamp((this.now() - this.pressAt) / 200, 0, 1);
    return lerp(this.pressFrom ?? 0, this.pressTo, u);
  }

  paint() {
    const cv = this.cv; const dpr = window.devicePixelRatio || 1;
    const w = cv.clientWidth || 360; const h = BOX;
    if (cv.width !== Math.round(w * dpr) || cv.height !== Math.round(h * dpr)) { cv.width = Math.round(w * dpr); cv.height = Math.round(h * dpr); }
    const ctx = cv.getContext('2d'); ctx.setTransform(dpr, 0, 0, dpr, 0, 0); ctx.clearRect(0, 0, w, h);
    const c = { x: w / 2, y: h / 2 }; const time = (this.now() - this.t0) / 1000 / 10;
    if (this.loading) return this.paintLoading(ctx, c, time);
    if (this.complete) return this.paintComplete(ctx, c);
    const all = this.visible();
    const keys = [...all.keys()].sort((a, b) => (a === this.pressed ? 1 : 0) - (b === this.pressed ? 1 : 0));
    const pt = this.pressNow();
    for (const k of keys) {
      const s = all.get(k); if (s.half <= 0) continue;
      const grow = k === this.pressed ? 9 * pt : 0;
      this.paintSector(ctx, c, s, this.colors[k] || '#E8B730', RADIUS + grow, time);
    }
    this.paintLabels(ctx, c, all);
    this.paintCenter(ctx, c);
  }

  paintSector(ctx, c, s, color, outer, time) {
    const inner = RADIUS - THICKNESS; const full = s.half * 2 >= 359;
    const path = new Path2D();
    if (full) { path.arc(c.x, c.y, outer, 0, 2 * PI); path.moveTo(c.x + inner, c.y); path.arc(c.x, c.y, inner, 0, 2 * PI); }
    else {
      const from = (s.center - s.half) * PI / 180; const sweep = (s.half * 2 + 0.5) * PI / 180;
      path.arc(c.x, c.y, outer, from, from + sweep); path.arc(c.x, c.y, inner, from + sweep, from, true); path.closePath();
    }
    const light = blend(color, '#FFFFFF', 0x30 / 255); const dark = blend(color, '#000000', 3 / 255);
    const g = ctx.createRadialGradient(c.x, c.y, 0, c.x, c.y, RADIUS);
    g.addColorStop(0.3, rgb(light)); g.addColorStop(1, rgb(dark));
    ctx.fillStyle = g; full ? ctx.fill(path, 'evenodd') : ctx.fill(path);
    ctx.save(); ctx.clip(path, full ? 'evenodd' : 'nonzero');
    this.paintParticles(ctx, c, s, inner, outer, time);
    ctx.restore();
  }

  paintParticles(ctx, c, s, inner, outer, time) {
    const step = 7, sz = 5, sqrt2 = Math.SQRT2;
    const start = (((s.center - s.half) % 360) + 360) % 360; const end = start + s.half * 2;
    for (let i = Math.floor(start / step); i <= Math.ceil(end / step); i++) {
      const angle = i * step;
      const t = (((time + 100) * (1 + (Math.sin(angle * 2000) + 1) * 0.25)) % 1);
      const r = lerp(inner - sz * sqrt2, outer + sz * sqrt2, t); const rad = angle * PI / 180;
      const px = c.x + Math.cos(rad) * r, py = c.y + Math.sin(rad) * r;
      const wave = 0.25 * (Math.sin(t * PI) - 1) + 1;
      const alpha = clamp(0.65 * (-1.75 * Math.abs(t - 0.5) + 1) * wave, 0, 1);
      if (alpha <= 0) continue;
      const scale = 0.75 * wave * (0.8 + (Math.sin(angle) + 1) * 0.25);
      ctx.fillStyle = `rgba(255,255,255,${alpha.toFixed(3)})`; ctx.beginPath(); ctx.arc(px, py, 2.4 * scale, 0, 2 * PI); ctx.fill();
    }
  }

  labelPos(c, angle, tw, th) {
    const dx = Math.cos(angle * PI / 180), dy = Math.sin(angle * PI / 180);
    const gap = 10 + Math.abs(dx) * tw / 2 + Math.abs(dy) * th / 2;
    return { x: c.x + dx * (RADIUS + 9 + gap), y: c.y + dy * (RADIUS + 9 + gap) };
  }

  paintLabels(ctx, c, all) {
    const items = [];
    for (const [k, s] of all) {
      if (s.half <= 0 || s.textAlpha <= 0.01 || s.pct <= 0) continue;
      const col = blend(this.colors[k] || '#E8B730', '#FFFFFF', 0x33 / 255);
      const fs = 14.5 * s.textScale; ctx.font = `700 ${fs}px ${this.font}`;
      const text = pctText(s.pct); const tw = ctx.measureText(text).width; const th = fs * 1.2;
      items.push({ home: ((s.center % 360) + 360) % 360, angle: ((s.center % 360) + 360) % 360, text, tw, th, fs, col, a: s.textAlpha });
    }
    if (!items.length) return;
    items.sort((a, b) => a.home - b.home);
    const rectAt = (l, ang) => { const p = this.labelPos(c, ang, l.tw, l.th); return { x0: p.x - l.tw / 2 - 3, y0: p.y - l.th / 2 - 3, x1: p.x + l.tw / 2 + 3, y1: p.y + l.th / 2 + 3 }; };
    const ov = (a, b) => a.x0 < b.x1 && a.x1 > b.x0 && a.y0 < b.y1 && a.y1 > b.y0;
    const step = 1.5, maxShift = 70, n = items.length;
    for (let iter = 0; iter < 120 && n > 1; iter++) {
      let moved = false;
      for (let i = 0; i < n; i++) {
        const j = (i + 1) % n; if (n === 2 && i === 1) break;
        const a = items[i], b = items[j];
        if (!ov(rectAt(a, a.angle), rectAt(b, b.angle))) continue;
        if (Math.abs(a.angle - step - a.home) <= maxShift) a.angle -= step;
        if (Math.abs(b.angle + step - b.home) <= maxShift) b.angle += step;
        moved = true;
      }
      if (!moved) break;
    }
    for (const l of items) {
      const pos = this.labelPos(c, l.angle, l.tw, l.th); const col = rgb(l.col, l.a);
      if (Math.abs(l.angle - l.home) > 4) {
        const home = l.home * PI / 180;
        const fx = c.x + Math.cos(home) * (RADIUS + 3), fy = c.y + Math.sin(home) * (RADIUS + 3);
        const tx = pos.x - c.x, ty = pos.y - c.y; const td = Math.hypot(tx, ty);
        const k = td - (l.tw / 2 * Math.abs(tx / td) + l.th / 2 * Math.abs(ty / td)) - 3;
        ctx.strokeStyle = rgb(l.col, l.a * 0.55); ctx.lineWidth = 1.2; ctx.lineCap = 'round';
        ctx.beginPath(); ctx.moveTo(fx, fy); ctx.lineTo(c.x + tx / td * k, c.y + ty / td * k); ctx.stroke();
      }
      ctx.font = `700 ${l.fs}px ${this.font}`; ctx.fillStyle = col; ctx.textAlign = 'center'; ctx.textBaseline = 'middle';
      ctx.fillText(l.text, pos.x, pos.y);
    }
  }

  paintCenter(ctx, c) {
    const bytes = Math.round(this.visibleBytes());
    const text = this.format(bytes).split(' '); const top = text[0]; const bottom = text.slice(1).join(' ');
    ctx.textAlign = 'center'; ctx.textBaseline = 'middle';
    ctx.font = `700 32px ${this.font}`; ctx.fillStyle = '#fff'; ctx.fillText(top, c.x, c.y - 5);
    ctx.font = `12px ${this.font}`; ctx.fillStyle = GRAY; ctx.fillText(bottom, c.x, c.y + 22);
  }

  paintLoading(ctx, c, time) {
    ctx.lineWidth = THICKNESS; ctx.strokeStyle = 'rgba(255,255,255,0.08)';
    ctx.beginPath(); ctx.arc(c.x, c.y, RADIUS - THICKNESS / 2, 0, 2 * PI); ctx.stroke();
    const t = time * 10; const start = (t * 2 * PI) % (2 * PI); const sweep = PI * (0.35 + 0.25 * Math.sin(t * PI));
    ctx.strokeStyle = 'rgba(255,255,255,0.12)'; ctx.lineCap = 'round';
    ctx.beginPath(); ctx.arc(c.x, c.y, RADIUS - THICKNESS / 2, start, start + sweep); ctx.stroke();
  }

  paintComplete(ctx, c) {
    const w = 10, d = RADIUS * 2; const left = c.x - RADIUS, top = c.y - RADIUS;
    const g = ctx.createLinearGradient(0, c.y - 100, 0, c.y + 100);
    g.addColorStop(0, 'rgba(110,213,86,0)'); g.addColorStop(0.07, '#6ED556'); g.addColorStop(0.93, '#41BA71'); g.addColorStop(1, 'rgba(65,186,113,0)');
    ctx.strokeStyle = g; ctx.lineWidth = w; ctx.lineCap = 'round'; ctx.lineJoin = 'round';
    ctx.beginPath(); ctx.arc(c.x, c.y, RADIUS - w / 2, 0, 2 * PI); ctx.stroke();
    ctx.beginPath(); ctx.moveTo(left + d * 0.348, top + d * 0.538); ctx.lineTo(left + d * 0.447, top + d * 0.636); ctx.lineTo(left + d * 0.678, top + d * 0.402); ctx.stroke();
  }
}
