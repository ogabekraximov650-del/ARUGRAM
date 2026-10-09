// fMP4 videoni Telegram'dan bo'laklab olib <video> ga beradi (MSE).
//
// Android ilovada bu ishni ExoPlayer + `AruDataSource` qiladi (pleyer o'zi
// bayt so'raydi). Brauzerda, ayniqsa iPhone'dagi Telegram ichida, bunday
// "o'rtadagi server" (Service Worker) yo'q — shu sabab teskari: biz o'zimiz
// fayl bo'laklarini (moof+mdat fragmentlari) olib, ochib, pleyerga
// `SourceBuffer.appendBuffer` bilan beramiz. iPhone (iOS 17.1+) da
// `ManagedMediaSource`, qolganlarida `MediaSource`.
//
// Fayl tuzilishi (`tool/encode/run.py` -> `make_fmp4`):
//   ftyp | moov (bo'sh jadvallar) | sidx (video) | sidx (audio) | moof+mdat ...
// `sidx` — "qaysi soniya qaysi baytda" jadvali: surish (seek) shu bilan.
//
//   const eng = createEngine(videoEl, { name, key, onState, onError, ahead })
//   await eng.ready;     eng.duration;   eng.destroy();

import { openFile } from '../tg/media.js';

const HEAD_PROBE = 256 * 1024;

// ── MP4 qutilari ────────────────────────────────────────────────────

function u32(b, o) { return ((b[o] << 24) >>> 0) + (b[o + 1] << 16) + (b[o + 2] << 8) + b[o + 3]; }
function u64(b, o) { return u32(b, o) * 4294967296 + u32(b, o + 4); }
function type(b, o) { return String.fromCharCode(b[o], b[o + 1], b[o + 2], b[o + 3]); }

/** `[{type, start, size, hdr}]` — `b` ichida, `base` — fayldagi joyi. */
function boxes(b, start = 0, end = b.length) {
  const out = [];
  let i = start;
  while (i + 8 <= end) {
    let size = u32(b, i);
    const t = type(b, i + 4);
    let hdr = 8;
    if (size === 1) { size = u64(b, i + 8); hdr = 16; } else if (size === 0) { size = end - i; }
    if (size < hdr) break;
    out.push({ type: t, start: i, size, hdr });
    i += size;
  }
  return out;
}

function child(b, box, t) {
  return boxes(b, box.start + box.hdr, Math.min(b.length, box.start + box.size)).find((x) => x.type === t) || null;
}

function path(b, box, ...types) {
  let cur = box;
  for (const t of types) { cur = cur && child(b, cur, t); }
  return cur;
}

/** Trek turi va kodek satri (MSE `addSourceBuffer` uchun). */
function trackCodecs(b, moov) {
  const out = [];
  for (const trak of boxes(b, moov.start + moov.hdr, moov.start + moov.size).filter((x) => x.type === 'trak')) {
    const stsd = path(b, trak, 'mdia', 'minf', 'stbl', 'stsd');
    if (!stsd) continue;
    // stsd: full box (4) + entry_count (4), keyin namunaviy yozuv.
    const entry = boxes(b, stsd.start + stsd.hdr + 8, stsd.start + stsd.size)[0];
    if (!entry) continue;
    const fourcc = entry.type;
    if (fourcc === 'hvc1' || fourcc === 'hev1') {
      // VisualSampleEntry: 8 (SampleEntry) + 70 baytdan keyin ichki qutilar.
      const hv = boxes(b, entry.start + 8 + 78, entry.start + entry.size).find((x) => x.type === 'hvcC');
      out.push(hv ? hevcCodec(fourcc, b, hv.start + hv.hdr) : `${fourcc}.1.6.L120.90`);
    } else if (fourcc === 'avc1' || fourcc === 'avc3') {
      const av = boxes(b, entry.start + 8 + 78, entry.start + entry.size).find((x) => x.type === 'avcC');
      if (av) {
        const p = av.start + av.hdr;
        const hex = (v) => v.toString(16).padStart(2, '0');
        out.push(`${fourcc}.${hex(b[p + 1])}${hex(b[p + 2])}${hex(b[p + 3])}`);
      } else out.push('avc1.640028');
    } else if (fourcc === 'vp09') {
      out.push('vp09.00.10.08');
    } else if (fourcc === 'av01') {
      out.push('av01.0.08M.08');
    } else if (fourcc === 'mp4a') {
      out.push('mp4a.40.2');
    } else if (fourcc === 'Opus' || fourcc === 'opus') {
      out.push('opus');
    }
  }
  return out;
}

/** RFC 6381 HEVC satri: hvc1.<space><profile>.<compat>.<tier><level>.<cons...> */
function hevcCodec(fourcc, b, p) {
  const b1 = b[p + 1];
  const space = ['', 'A', 'B', 'C'][b1 >> 6];
  const tier = (b1 >> 5) & 1 ? 'H' : 'L';
  const profile = b1 & 0x1f;
  let compat = u32(b, p + 2);
  let rev = 0;
  for (let i = 0; i < 32; i++) { rev = (rev << 1) | (compat & 1); compat >>>= 1; }
  rev >>>= 0;
  const cons = [];
  for (let i = 0; i < 6; i++) cons.push(b[p + 6 + i]);
  while (cons.length && cons[cons.length - 1] === 0) cons.pop();
  const level = b[p + 12];
  const consStr = cons.length ? '.' + cons.map((x) => x.toString(16).toUpperCase()).join('.') : '';
  return `${fourcc}.${space}${profile}.${rev.toString(16).toUpperCase()}.${tier}${level}${consStr}`;
}

/** `sidx` -> fragmentlar ro'yxati. */
function parseSidx(b, box, fileOffset) {
  const p0 = box.start + box.hdr;
  const version = b[p0];
  const timescale = u32(b, p0 + 8);
  let ept; let first; let p;
  if (version === 0) { ept = u32(b, p0 + 12); first = u32(b, p0 + 16); p = p0 + 20; } else { ept = u64(b, p0 + 12); first = u64(b, p0 + 20); p = p0 + 28; }
  const count = (b[p + 2] << 8) | b[p + 3];
  p += 4;
  const segs = [];
  let off = fileOffset + box.start + box.size + first;
  let t = ept / timescale;
  for (let i = 0; i < count; i++) {
    const size = u32(b, p) & 0x7fffffff;
    const dur = u32(b, p + 4) / timescale;
    segs.push({ start: off, size, time: t, dur });
    off += size;
    t += dur;
    p += 12;
  }
  return { segs, refId: u32(b, p0 + 4) };
}

// ── Dvigatel ────────────────────────────────────────────────────────

export function engineSupported() {
  return !!(window.ManagedMediaSource || window.MediaSource);
}

/**
 * `name` — fMP4 fayl nomi (kanalda), `key` — AES-CTR kaliti (hex).
 * `ahead` — oldindan yuklanadigan oyna (soniya): 20 s (ilova buferi 15..30 s).
 */
export function createEngine(video, { name, key = '', ahead = 20, behind = 10, startAt = 0, onState = () => {}, onError = () => {} }) {
  const MS = window.ManagedMediaSource || window.MediaSource;
  let file = null;
  let ms = null;
  let sb = null;
  let segs = [];
  let init = null;
  let next = 0; // keyingi yuklanadigan fragment
  let pumping = false;
  let destroyed = false;
  let streaming = true; // ManagedMediaSource "hozir yuklash mumkin" belgisi
  let objectUrl = '';
  let duration = 0;
  let gen = 0; // seek'dan keyin eski yuklashlar tashlanadi
  let gapT = 0;

  const st = (s, extra = {}) => { if (!destroyed) onState({ state: s, ...extra }); };

  function segIndexAt(t) {
    if (!segs.length) return 0;
    let lo = 0; let hi = segs.length - 1;
    while (lo < hi) {
      const mid = (lo + hi + 1) >> 1;
      if (segs[mid].time <= t + 0.001) lo = mid; else hi = mid - 1;
    }
    return lo;
  }

  function appendAsync(data) {
    return new Promise((resolve, reject) => {
      const done = () => { sb.removeEventListener('updateend', done); sb.removeEventListener('error', fail); resolve(); };
      const fail = () => { sb.removeEventListener('updateend', done); sb.removeEventListener('error', fail); reject(new Error('append_error')); };
      sb.addEventListener('updateend', done);
      sb.addEventListener('error', fail);
      try { sb.appendBuffer(data); } catch (e) { sb.removeEventListener('updateend', done); sb.removeEventListener('error', fail); reject(e); }
    });
  }

  function removeAsync(a, b) {
    return new Promise((resolve) => {
      if (b <= a) { resolve(); return; }
      const done = () => { sb.removeEventListener('updateend', done); resolve(); };
      sb.addEventListener('updateend', done);
      try { sb.remove(a, b); } catch (_) { sb.removeEventListener('updateend', done); resolve(); }
    });
  }

  function bufferedAhead() {
    const t = video.currentTime;
    const r = video.buffered;
    for (let i = 0; i < r.length; i++) {
      if (r.start(i) <= t + 0.3 && r.end(i) >= t) return r.end(i) - t;
    }
    return 0;
  }

  async function trimBehind() {
    const cut = video.currentTime - behind;
    if (cut > 1 && sb.buffered.length && sb.buffered.start(0) < cut) await removeAsync(0, cut);
  }

  async function pump() {
    if (pumping || destroyed) return;
    pumping = true;
    const my = gen;
    try {
      while (!destroyed && my === gen && next < segs.length) {
        if (!streaming) break;
        if (bufferedAhead() > ahead) break;
        const seg = segs[next];
        st(video.readyState < 3 ? 'buffering' : 'playing');
        // Keyingi fragmentni ham oldindan so'rab qo'yamiz (tarmoq bo'sh turmasin).
        const n2 = segs[next + 1];
        if (n2) file.prefetch(n2.start, n2.size);
        const data = await file.read(seg.start, seg.size);
        if (destroyed || my !== gen) break;
        try {
          await appendAsync(data);
        } catch (e) {
          if (e?.name === 'QuotaExceededError') {
            await trimBehind();
            await removeAsync(video.currentTime + ahead, duration);
            continue;
          }
          throw e;
        }
        next += 1;
        if (next % 4 === 0) await trimBehind();
      }
      if (!destroyed && next >= segs.length && ms.readyState === 'open' && !sb.updating) {
        try { ms.endOfStream(); } catch (_) { /* */ }
      }
    } catch (e) {
      if (!destroyed) onError(e);
    } finally {
      pumping = false;
      // Surishdan keyin eski sikl tugadi — yangi joydan davom etamiz.
      if (!destroyed && my !== gen) pump();
    }
  }

  async function onSeeking() {
    if (!sb || destroyed) return;
    const t = video.currentTime;
    // Allaqachon buferda bo'lsa — hech narsa qilinmaydi.
    const r = video.buffered;
    for (let i = 0; i < r.length; i++) if (r.start(i) <= t && r.end(i) > t + 1) { next = Math.max(next, segIndexAt(r.end(i) - 0.05)); pump(); return; }
    gen += 1;
    next = segIndexAt(t);
    st('buffering');
    pump();
  }

  const onTime = () => { if (!pumping) pump(); };
  const onWaiting = () => st('buffering');
  const onPlaying = () => st('playing');

  const ready = (async () => {
    st('loading');
    file = await openFile(name, { key });
    if (destroyed) return;
    // Bosh qism: ftyp + moov + sidx (odatda bir necha KB).
    let head = await file.read(0, Math.min(HEAD_PROBE, file.size));
    let top = boxes(head);
    let moov = top.find((x) => x.type === 'moov');
    if (moov && moov.start + moov.size > head.length) {
      head = await file.read(0, Math.min(file.size, moov.start + moov.size + 64 * 1024));
      top = boxes(head);
      moov = top.find((x) => x.type === 'moov');
    }
    if (!moov) throw new Error('bad_fmp4');
    const ftyp = top.find((x) => x.type === 'ftyp');
    const initEnd = moov.start + moov.size;
    init = head.slice(ftyp ? ftyp.start : 0, initEnd);
    const sidxBoxes = top.filter((x) => x.type === 'sidx' && x.start + x.size <= head.length);
    if (!sidxBoxes.length) throw new Error('no_sidx');
    // Video trek indeksi (birinchi sidx — ffmpeg video trekdan boshlaydi).
    segs = parseSidx(head, sidxBoxes[0], 0).segs;
    duration = segs.reduce((a, s) => a + s.dur, 0);
    const codecs = trackCodecs(head, moov);
    const mime = `video/mp4; codecs="${codecs.join(', ')}"`;
    if (!MS || !MS.isTypeSupported(mime)) {
      const e = new Error('codec_unsupported');
      e.mime = mime;
      throw e;
    }
    ms = new MS();
    if (window.ManagedMediaSource && MS === window.ManagedMediaSource) {
      video.disableRemotePlayback = true;
      ms.addEventListener('startstreaming', () => { streaming = true; pump(); });
      ms.addEventListener('endstreaming', () => { streaming = false; });
    }
    objectUrl = URL.createObjectURL(ms);
    video.src = objectUrl;
    await new Promise((res) => ms.addEventListener('sourceopen', res, { once: true }));
    if (destroyed) return;
    sb = ms.addSourceBuffer(mime);
    sb.mode = 'segments';
    try { ms.duration = duration; } catch (_) { /* */ }
    await appendAsync(init);
    // Boshlanish nuqtasi OLDINDAN qo'yiladi — bufer 0 dan emas, kerakli joydan to'ladi.
    if (startAt > 0) { try { video.currentTime = Math.max(0, Math.min(startAt, Math.max(0, duration - 1))); } catch (_) { /* */ } }
    video.addEventListener('seeking', onSeeking);
    video.addEventListener('timeupdate', onTime);
    video.addEventListener('waiting', onWaiting);
    video.addEventListener('playing', onPlaying);
    next = segIndexAt(video.currentTime || 0);
    pump();
    // Bo'shliqdan sakrash: bufer joriy joydan biroz keyin boshlansa (B-kadr/tekislash)
    // video qotib qolmasin.
    gapT = setInterval(() => {
      if (destroyed || video.readyState >= 3) return;
      const t = video.currentTime; const r = video.buffered;
      for (let i = 0; i < r.length; i++) {
        if (r.start(i) > t && r.start(i) - t < 1.5) { try { video.currentTime = r.start(i) + 0.02; } catch (_) { /* */ } return; }
      }
      if (!pumping && next < segs.length && bufferedAhead() <= ahead) pump();
    }, 500);
  })();
  ready.catch((e) => { if (!destroyed) onError(e); });

  return {
    ready,
    get duration() { return duration; },
    /** Boshlanish nuqtasi (to'xtagan joydan davom etish). */
    startAt(sec) {
      ready.then(() => { video.currentTime = Math.max(0, Math.min(sec, duration - 1)); });
    },
    setAhead(sec) { ahead = sec; pump(); },
    destroy() {
      destroyed = true;
      clearInterval(gapT);
      gen += 1;
      video.removeEventListener('seeking', onSeeking);
      video.removeEventListener('timeupdate', onTime);
      video.removeEventListener('waiting', onWaiting);
      video.removeEventListener('playing', onPlaying);
      try { video.pause(); } catch (_) { /* */ }
      try { video.removeAttribute('src'); video.load(); } catch (_) { /* */ }
      if (objectUrl) URL.revokeObjectURL(objectUrl);
      try { file?.close(); } catch (_) { /* */ }
    },
  };
}

// Sinov uchun (Node): MP4 qutilarini o'qish.
export const _test = { boxes, parseSidx, trackCodecs };
