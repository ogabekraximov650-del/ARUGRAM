// Oddiy MP4 (faststart: `moov` boshida) -> fMP4 bo'laklar, BRAUZERDA (MSE uchun). Serverda fMP4 nusxa saqlanmaydi.
//
// `buildLayout(moovBytes)` MP4 jadvallaridan (stts, ctts, stss, stsz, stsc, stco) hamma namuna (sample) joyini hisoblaydi va
//   * `init`  — ftyp + moov (bo'sh jadvallar + mvex), `SourceBuffer` ga birinchi beriladi;
//   * `segs`  — ~4 soniyalik bo'laklar (video kalit kadrdan kalit kadrgacha, audio shu vaqt oralig'i): `{time, dur, ranges}`;
//   * `duration` — soniyada.
// `buildFragment(layout, seg, read)` bitta bo'lak uchun `moof + mdat` yasaydi; `read(pos, len)` — faylning OCHILGAN baytlari.
// Bir bo'lak = bitta `moof` ichida hamma treklar (video + audio) — bitta `SourceBuffer`.
//
// Tahrirlash ro'yxati (`edts`) tashlanadi; video uchun uning `media_time` si kompozitsiya vaqtidan ayiriladi (B-kadrli videoda
// birinchi kadr 0 da chiqsin). Audio "priming" (~23 ms) hisobga olinmaydi.

const rd32 = (b, o) => ((b[o] << 24) >>> 0) + (b[o + 1] << 16) + (b[o + 2] << 8) + b[o + 3];
const rdI32 = (b, o) => (b[o] << 24) | (b[o + 1] << 16) | (b[o + 2] << 8) | b[o + 3];
const rd64 = (b, o) => rd32(b, o) * 4294967296 + rd32(b, o + 4);
const tp = (b, o) => String.fromCharCode(b[o], b[o + 1], b[o + 2], b[o + 3]);

/** `[start, end)` ichidagi qutilar: `{type, start, end, body}` (`body` — sarlavhadan keyin). */
function kids(b, start, end) {
  const out = [];
  let i = start;
  while (i + 8 <= end) {
    let size = rd32(b, i);
    let hdr = 8;
    if (size === 1) { size = rd64(b, i + 8); hdr = 16; } else if (size === 0) size = end - i;
    if (size < hdr || i + size > end) break;
    out.push({ type: tp(b, i + 4), start: i, end: i + size, body: i + hdr });
    i += size;
  }
  return out;
}
const sub = (b, box, t) => kids(b, box.body, box.end).find((x) => x.type === t) || null;
const subs = (b, box, t) => kids(b, box.body, box.end).filter((x) => x.type === t);
const at = (b, box, ...path) => { let c = box; for (const t of path) { c = c && sub(b, c, t); } return c; };

// ── Yozish yordamchilari ──
function box(type, ...parts) {
  let n = 8;
  for (const p of parts) n += p.length;
  const out = new Uint8Array(n);
  const dv = new DataView(out.buffer);
  dv.setUint32(0, n);
  for (let i = 0; i < 4; i++) out[4 + i] = type.charCodeAt(i);
  let o = 8;
  for (const p of parts) { out.set(p, o); o += p.length; }
  return out;
}
const u32b = (...v) => { const o = new Uint8Array(v.length * 4); const dv = new DataView(o.buffer); v.forEach((x, i) => dv.setUint32(i * 4, x >>> 0)); return o; };
const raw = (b, x) => b.subarray(x.start, x.end);

/** `x` qutini nusxalab, `off` (qutining tanasidan hisoblanadi) joydagi `len` baytni nolga tushiradi. */
function zeroed(b, x, off, len) {
  const c = b.slice(x.start, x.end);
  c.fill(0, (x.body - x.start) + off, (x.body - x.start) + off + len);
  return c;
}

function parseTrack(b, trak) {
  const tkhd = sub(b, trak, 'tkhd');
  const mdia = sub(b, trak, 'mdia');
  const mdhd = mdia && sub(b, mdia, 'mdhd');
  const hdlr = mdia && sub(b, mdia, 'hdlr');
  const minf = mdia && sub(b, mdia, 'minf');
  const stbl = minf && sub(b, minf, 'stbl');
  if (!tkhd || !mdhd || !hdlr || !stbl) return null;
  const v1 = b[tkhd.body] === 1;
  const id = rd32(b, tkhd.body + (v1 ? 20 : 12));
  const m1 = b[mdhd.body] === 1;
  const timescale = rd32(b, mdhd.body + (m1 ? 20 : 12));
  const handler = tp(b, hdlr.body + 8);
  const T = (t) => sub(b, stbl, t);
  const stsz = T('stsz'); const stts = T('stts'); const stsc = T('stsc');
  const stco = T('stco') || T('co64');
  if (!stsz || !stts || !stsc || !stco || !T('stsd')) return null;
  const n = rd32(b, stsz.body + 8);
  const sizes = new Uint32Array(n);
  const cst = rd32(b, stsz.body + 4);
  if (cst) sizes.fill(cst); else for (let i = 0; i < n; i++) sizes[i] = rd32(b, stsz.body + 12 + i * 4);
  const dts = new Float64Array(n); const dur = new Uint32Array(n);
  { let k = 0; let t = 0; const c = rd32(b, stts.body + 4);
    for (let e = 0; e < c && k < n; e++) {
      const cnt = rd32(b, stts.body + 8 + e * 8); const d = rd32(b, stts.body + 12 + e * 8);
      for (let j = 0; j < cnt && k < n; j++) { dts[k] = t; dur[k] = d; t += d; k++; }
    } }
  const cto = new Int32Array(n);
  const ctts = T('ctts');
  if (ctts) { let k = 0; const c = rd32(b, ctts.body + 4);
    for (let e = 0; e < c && k < n; e++) {
      const cnt = rd32(b, ctts.body + 8 + e * 8); const o = rdI32(b, ctts.body + 12 + e * 8);
      for (let j = 0; j < cnt && k < n; j++) cto[k++] = o;
    } }
  const sync = new Uint8Array(n);
  const stss = T('stss');
  if (stss) { const c = rd32(b, stss.body + 4); for (let e = 0; e < c; e++) { const s = rd32(b, stss.body + 8 + e * 4) - 1; if (s >= 0 && s < n) sync[s] = 1; } } else sync.fill(1);
  // Namuna joylari: chunk -> namunalar (stsc) + chunk offset (stco/co64).
  const off = new Float64Array(n);
  { const is64 = stco.type === 'co64';
    const nch = rd32(b, stco.body + 4);
    const sc = rd32(b, stsc.body + 4);
    let k = 0;
    for (let c = 0; c < nch && k < n; c++) {
      let per = 0;
      for (let e = 0; e < sc; e++) { if (rd32(b, stsc.body + 8 + e * 12) <= c + 1) per = rd32(b, stsc.body + 12 + e * 12); else break; }
      let o = is64 ? rd64(b, stco.body + 8 + c * 8) : rd32(b, stco.body + 8 + c * 4);
      for (let j = 0; j < per && k < n; j++) { off[k] = o; o += sizes[k]; k++; }
    } }
  // Tahrirlash ro'yxati: birinchi haqiqiy yozuvning `media_time` si.
  let mediaTime = 0;
  const elst = at(b, trak, 'edts', 'elst');
  if (elst) {
    const ev = b[elst.body]; const c = rd32(b, elst.body + 4);
    const step = ev === 1 ? 20 : 12;
    for (let e = 0; e < c; e++) {
      const p = elst.body + 8 + e * step;
      const neg = ev === 1 ? (b[p + 8] & 0x80) !== 0 : (b[p + 4] & 0x80) !== 0; // -1 — bo'sh tahrir
      const mt = ev === 1 ? rd64(b, p + 8) : rd32(b, p + 4);
      if (!neg) { mediaTime = mt; break; }
    }
  }
  return { id, timescale, handler, n, sizes, dts, dur, cto, sync, off, mediaTime, trak, tkhd, mdia, mdhd, hdlr, minf, stbl, v1, m1 };
}

const SEG_SEC = 4;

/** `moov` — Uint8Array (qutining o'zi, sarlavhasi bilan, 0 dan boshlanadi). */
export function buildLayout(b) {
  const top = kids(b, 0, b.length);
  const moov = top.find((x) => x.type === 'moov');
  if (!moov) throw new Error('bad_mp4');
  const tracks = subs(b, moov, 'trak').map((t) => parseTrack(b, t)).filter((t) => t && t.n > 0 && (t.handler === 'vide' || t.handler === 'soun'));
  if (!tracks.length) throw new Error('bad_mp4');
  const ref = tracks.find((t) => t.handler === 'vide') || tracks[0];
  const sec = (t, i) => t.dts[i] / t.timescale;
  const end = (t) => (t.dts[t.n - 1] + t.dur[t.n - 1]) / t.timescale;
  const duration = Math.max(...tracks.map(end));
  // Bo'lak chegaralari: etalon trekning kalit kadrlarida, ~SEG_SEC dan keyin.
  const bounds = [0];
  let last = 0;
  for (let i = 1; i < ref.n; i++) {
    if (ref.sync[i] && sec(ref, i) - last >= SEG_SEC) { bounds.push(i); last = sec(ref, i); }
  }
  bounds.push(ref.n);
  const lowerBound = (t, time) => { // dts/timescale >= time bo'lgan birinchi namuna
    let lo = 0; let hi = t.n;
    while (lo < hi) { const m = (lo + hi) >> 1; if (t.dts[m] / t.timescale < time - 1e-9) lo = m + 1; else hi = m; }
    return lo;
  };
  const segs = [];
  for (let s = 0; s + 1 < bounds.length; s++) {
    const a = bounds[s]; const e = bounds[s + 1];
    const t0 = s === 0 ? 0 : sec(ref, a);
    const t1 = e >= ref.n ? Infinity : sec(ref, e);
    const ranges = tracks.map((t) => {
      if (t === ref) return [a, e];
      return [s === 0 ? 0 : lowerBound(t, t0), e >= ref.n ? t.n : lowerBound(t, t1)];
    });
    if (!ranges.some((r) => r[1] > r[0])) continue;
    segs.push({ time: t0, dur: (e >= ref.n ? duration : sec(ref, e)) - t0, ranges });
  }

  // ── Boshlang'ich bo'lak: ftyp + moov (bo'sh jadvallar, mvex) ──
  const emptyTables = (stsd) => [
    stsd,
    box('stts', u32b(0, 0)), box('stsc', u32b(0, 0)), box('stsz', u32b(0, 0, 0)), box('stco', u32b(0, 0)),
  ];
  const trakOut = tracks.map((t) => {
    const mv1 = t.m1 ? 24 : 16;
    const tkhd = zeroed(b, t.tkhd, t.v1 ? 28 : 20, t.v1 ? 8 : 4);
    const mdhd = zeroed(b, t.mdhd, mv1 + 0, t.m1 ? 8 : 4);
    const minfParts = kids(b, t.minf.body, t.minf.end).filter((x) => x.type !== 'stbl').map((x) => raw(b, x));
    const stsd = raw(b, sub(b, t.stbl, 'stsd'));
    const minf = box('minf', ...minfParts, box('stbl', ...emptyTables(stsd)));
    return box('trak', tkhd, box('mdia', mdhd, raw(b, t.hdlr), minf));
  });
  const mvhdBox = sub(b, moov, 'mvhd');
  const mvhd = zeroed(b, mvhdBox, b[mvhdBox.body] === 1 ? 24 : 16, b[mvhdBox.body] === 1 ? 8 : 4);
  const trex = tracks.map((t) => box('trex', u32b(0, t.id, 1, 0, 0, 0)));
  const ftyp = box('ftyp', new Uint8Array([...'iso5'].map((c) => c.charCodeAt(0))), u32b(512),
    new Uint8Array([...'iso5iso6mp41'].map((c) => c.charCodeAt(0))));
  const init = concat([ftyp, box('moov', mvhd, ...trakOut, box('mvex', ...trex))]);
  return { init, tracks, segs, duration, ref };
}

function concat(list) {
  let n = 0; for (const x of list) n += x.length;
  const out = new Uint8Array(n); let o = 0;
  for (const x of list) { out.set(x, o); o += x.length; }
  return out;
}

/** Bo'lakning fayldagi bayt oraliqlari (oldindan olish uchun): `[[pos, len], ...]`. */
export function segSpans(layout, seg) {
  const out = [];
  layout.tracks.forEach((t, k) => {
    const [a, e] = seg.ranges[k];
    if (e <= a) return;
    const start = t.off[a]; const end = t.off[e - 1] + t.sizes[e - 1];
    out.push([start, end - start]);
  });
  return out;
}

const SYNC_FLAGS = 0x02000000; const NONSYNC_FLAGS = 0x01010000;

/** Bitta bo'lak: `moof + mdat`. `read(pos, len)` — Promise<Uint8Array> (ochilgan baytlar). */
export async function buildFragment(layout, seg, seq, read) {
  const parts = [];
  for (let k = 0; k < layout.tracks.length; k++) {
    const t = layout.tracks[k]; const [a, e] = seg.ranges[k];
    if (e <= a) continue;
    const start = t.off[a]; const end = t.off[e - 1] + t.sizes[e - 1];
    if (end - start > 96 * 1024 * 1024) throw new Error('segment_too_big');
    const bytes = await read(start, end - start);
    if (bytes.length !== end - start) throw new Error('short_read');
    parts.push({ t, a, e, bytes, start });
  }
  // moof o'lchami: moof sarlavhasi (8) + mfhd (16) + har traf: 8 + tfhd (16) + tfdt (20) + trun (20 + 16*n).
  let moofSize = 8 + 16;
  for (const p of parts) moofSize += 8 + 16 + 20 + 20 + 16 * (p.e - p.a);
  // mdat'ga faqat namuna baytlari yoziladi (oraliqdagi begona baytlarsiz).
  let dataTotal = 0;
  for (const p of parts) { let s = 0; for (let i = p.a; i < p.e; i++) s += p.t.sizes[i]; p.data = s; dataTotal += s; }
  const buf = new Uint8Array(moofSize + 8 + dataTotal);
  const dv = new DataView(buf.buffer);
  const w4 = (o, v) => dv.setUint32(o, v >>> 0);
  const wT = (o, t) => { for (let i = 0; i < 4; i++) buf[o + i] = t.charCodeAt(i); };
  let o = 0;
  w4(o, moofSize); wT(o + 4, 'moof'); o += 8;
  w4(o, 16); wT(o + 4, 'mfhd'); w4(o + 8, 0); w4(o + 12, seq); o += 16;
  let dataPos = moofSize + 8;
  for (const p of parts) {
    const n = p.e - p.a;
    const trafSize = 8 + 16 + 20 + 20 + 16 * n;
    w4(o, trafSize); wT(o + 4, 'traf'); o += 8;
    w4(o, 16); wT(o + 4, 'tfhd'); w4(o + 8, 0x020000); w4(o + 12, p.t.id); o += 16;
    w4(o, 20); wT(o + 4, 'tfdt'); w4(o + 8, 0x01000000);
    dv.setUint32(o + 12, Math.floor(p.t.dts[p.a] / 4294967296)); dv.setUint32(o + 16, p.t.dts[p.a] % 4294967296); o += 20;
    w4(o, 20 + 16 * n); wT(o + 4, 'trun'); w4(o + 8, 0x01000F01); w4(o + 12, n); w4(o + 16, dataPos); o += 20;
    const shift = p.t.handler === 'vide' ? p.t.mediaTime : 0;
    for (let i = p.a; i < p.e; i++) {
      w4(o, p.t.dur[i]); w4(o + 4, p.t.sizes[i]);
      w4(o + 8, p.t.sync[i] ? SYNC_FLAGS : NONSYNC_FLAGS);
      dv.setInt32(o + 12, p.t.cto[i] - shift);
      o += 16;
    }
    // Namuna baytlari mdat ichiga.
    let dp = dataPos;
    for (let i = p.a; i < p.e; i++) {
      const s = p.t.off[i] - p.start;
      buf.set(p.bytes.subarray(s, s + p.t.sizes[i]), dp);
      dp += p.t.sizes[i];
    }
    dataPos = dp;
  }
  w4(moofSize, 8 + dataTotal); wT(moofSize + 4, 'mdat');
  return buf;
}
