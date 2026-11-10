// Videoni yangi tizimdan (`vd`) bo'laklab olib <video> ga beradi (MSE).
//
// Android ilovada bu ishni ExoPlayer pleyerning o'zi qiladi. Brauzerda, ayniqsa iPhone'dagi Telegram ichida, bunday
// "o'rtadagi server" (Service Worker) yo'q — shu sabab teskari: biz o'zimiz oddiy MP4 (faststart) jadvallarini o'qib
// (`vd/remux.js`), har ~4 soniyalik bo'lakni fMP4 `moof+mdat` qilib, `SourceBuffer.appendBuffer` bilan beramiz.
// iPhone (iOS 17.1+) da `ManagedMediaSource`, qolganlarida `MediaSource`. Serverda fMP4 nusxa YO'Q (Telegram'da joy ikki baravar bo'lmaydi).
//
//   const eng = createEngine(videoEl, { info, onState, onError, onStall, shouldPlay, ahead })
//   await eng.ready;     eng.duration;   eng.holding;   eng.cancelHold();   eng.destroy();
//
// Sekin internet uchun:
//   * oldinda bir nechta bo'lak PARALLEL so'raladi (tarmoq bo'sh turmaydi; baytlar 1 MiB bo'laklarda, <= 4 tasi bitta so'rovda);
//   * qotib qolsa (`waiting`) pleyer pauzada "ushlab turiladi" va bufer
//     yetarli to'lgach (3 s, keyin 6 s, 10 s ...) davom etadi — har soniyada
//     to'xtab-yurish o'rniga bitta qisqa kutish;
//   * `onStall({count, kbps, needKbps})` — pleyer sifatni pasaytirishi uchun.

import { openVdFile, dbg } from '../vd/store.js';
import { buildLayout, buildFragment, segSpans } from '../vd/remux.js';

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

/** Faylning boshidan `moov` qutisini topadi (faststart — boshida; aks holda sarlavhalar bo'ylab sakrab, oxiridan). */
async function readMoov(file) {
  let pos = 0;
  for (let i = 0; i < 64 && pos < file.size; i++) {
    const h = await file.read(pos, 16);
    if (h.length < 8) break;
    let size = u32(h, 0); let hdr = 8;
    if (size === 1) { size = u64(h, 8); hdr = 16; } else if (size === 0) size = file.size - pos;
    if (type(h, 4) === 'moov') return file.read(pos, size);
    if (size < hdr) break;
    pos += size;
  }
  throw new Error('bad_mp4');
}

// ── Dvigatel ────────────────────────────────────────────────────────

export function engineSupported() {
  return !!(window.ManagedMediaSource || window.MediaSource);
}

/**
 * `info` — `vdInfo` natijasi (`{name, size, key}`).
 * `ahead` — oldindan yuklanadigan oyna (soniya): 20 s (ilova buferi 15..30 s).
 */
export function createEngine(video, { info, ahead: aheadOpt = 60, behind = 10, startAt = 0, onState = () => {}, onError = () => {}, onStall = () => {}, shouldPlay = () => true }) {
  const MS = window.ManagedMediaSource || window.MediaSource;
  let file = null;
  let ms = null;
  let sb = null;
  let segs = [];
  let layout = null;
  let init = null;
  let next = 0; // keyingi yuklanadigan fragment
  let pumping = false;
  let destroyed = false;
  let streaming = true; // ManagedMediaSource "hozir yuklash mumkin" belgisi
  let objectUrl = '';
  const MAX_AHEAD = 90; // bufer 1.5 daqiqadan oshmasin (xotira)
  const PREFETCH_BYTES = 12 * 1024 * 1024; // oldindan parallel so'raladigan hajm
  let ahead = Math.min(MAX_AHEAD, aheadOpt);
  let duration = 0;
  let gen = 0; // seek'dan keyin eski yuklashlar tashlanadi
  let gapT = 0;
  // Qotish (rebuffer) boshqaruvi.
  let played = false; // birinchi `playing` bo'ldimi
  let holding = false; // qotgani uchun pauzada ushlab turilibdi
  let stalls = 0;
  const RESUME_GOALS = [3, 6, 10, 15];

  const st = (s, extra = {}) => { if (!destroyed) onState({ state: s, ...extra }); };

  const segBytes = (s) => segSpans(layout, s).reduce((a, [, l]) => a + l, 0);
  /** Bir nechta bo'lakning bayt oraliqlari BIRLASHTIRILIB so'raladi — qo'shni 1 MiB bo'laklar bitta so'rovda keladi (worker so'rovlari kam). */
  function prefetchSegs(list) {
    const sp = [];
    for (const s of list) sp.push(...segSpans(layout, s));
    sp.sort((a, b) => a[0] - b[0]);
    const merged = [];
    for (const [p, l] of sp) {
      const last = merged[merged.length - 1];
      if (last && p <= last[0] + last[1]) last[1] = Math.max(last[0] + last[1], p + l) - last[0]; else merged.push([p, l]);
    }
    for (const [p, l] of merged) file.prefetch(p, l);
  }

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

  let seekReq = 0;
  let focus = null; // sek tayyorlanayotgan nuqta (video hali eski joyda o'ynayapti)
  function bufferedAheadAt(t) {
    const r = video.buffered;
    for (let i = 0; i < r.length; i++) if (r.start(i) <= t + 0.05 && r.end(i) >= t) return r.end(i) - t;
    return 0;
  }
  async function trimBehind() {
    const cut = Math.min(video.currentTime, focus ?? Infinity) - behind;
    if (cut > 1 && sb.buffered.length && sb.buffered.start(0) < cut) await removeAsync(0, cut);
  }

  async function pump() {
    if (pumping || destroyed) return;
    pumping = true;
    const my = gen;
    try {
      while (!destroyed && my === gen && next < segs.length) {
        if (!streaming) break;
        if (bufferedAheadAt(focus ?? video.currentTime) > ahead) break;
        const seg = segs[next];
        st(holding || video.readyState < 3 ? 'buffering' : 'playing');
        // Oldindagi fragmentlar ham parallel so'rab qo'yiladi (tarmoq bo'sh turmasin,
        // sekin/uzoq ulanishda bir vaqtda bir nechta so'rov tezlikni oshiradi).
        // Chegara: `ahead` soniya va PREFETCH_BYTES — ortiqcha yuklanmasin.
        {
          const tEnd = (focus ?? video.currentTime) + ahead;
          let bytes = segBytes(seg);
          const group = [seg];
          for (let k = next + 1; k < segs.length && bytes < PREFETCH_BYTES && segs[k].time < tEnd; k++) {
            group.push(segs[k]);
            bytes += segBytes(segs[k]);
          }
          prefetchSegs(group);
        }
        const data = await buildFragment(layout, seg, next + 1, (p, l) => file.read(p, l));
        if (destroyed || my !== gen) break;
        for (;;) {
          try {
            await appendAsync(data);
            break;
          } catch (e) {
            if (e?.name === 'QuotaExceededError') {
              // Xotira to'ldi: orqadagini o'chirib, oldinga oynani kichraytiramiz.
              ahead = Math.max(12, Math.floor(ahead * 0.7));
              await trimBehind();
              await removeAsync(video.currentTime + ahead, duration);
              await new Promise((r) => setTimeout(r, 500));
              if (destroyed || my !== gen) break;
              continue;
            }
            throw e;
          }
        }
        if (destroyed || my !== gen) break;
        next += 1;
        if (next % 4 === 0) await trimBehind();
      }
      if (!destroyed && next >= segs.length && ms.readyState === 'open' && !sb.updating) {
        try { ms.endOfStream(); } catch (_) { /* */ }
      }
    } catch (e) {
      // Sek tufayli eskirgan sikl (`my !== gen`) yoki bekor qilingan so'rov — xato emas: yangi sikl davom etadi.
      // (Tez-tez sek qilinganda shu xato "Videoni ochib bo'lmadi" chiqarardi.)
      const stale = my !== gen || /^cancelled$|aborted/i.test(`${e?.message || e}`);
      if (!destroyed && !stale) onError(e);
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
    try { file?.cancelPrefetch?.(); } catch (_) { /* */ }
    next = segIndexAt(t);
    st('buffering');
    pump();
  }

  const onTime = () => { if (!pumping) pump(); };
  // Qotdi: video pauzada ushlab turiladi, bufer maqsadga yetgach davom etadi
  // (aks holda brauzer har 0.3 s ma'lumotda yurib, yana to'xtaydi).
  const onWaiting = () => {
    st('buffering');
    if (!played || holding || video.seeking || focus != null || video.ended) return;
    if (duration - video.currentTime < 1) return;
    stalls += 1;
    holding = true;
    try { video.pause(); } catch (_) { /* */ }
    try { onStall({ count: stalls, kbps: file?.rateKbps?.() || 0, needKbps: needKbps() }); } catch (_) { /* */ }
    pump();
  };
  const onPlaying = () => { played = true; st('playing'); };
  function needKbps() { return duration > 0 && file ? Math.round((file.size * 8) / duration / 1000) : 0; }
  function checkHold() {
    if (!holding) return;
    if (!shouldPlay()) { holding = false; return; }
    const goal = RESUME_GOALS[Math.min(stalls - 1, RESUME_GOALS.length - 1)];
    const left = Math.max(0.3, duration - video.currentTime - 0.3);
    if (bufferedAhead() >= Math.min(goal, left) || (next >= segs.length && !pumping)) {
      holding = false;
      st('playing');
      video.play().catch(() => {});
    } else st('buffering');
  }

  const ready = (async () => {
    st('loading');
    file = openVdFile(info);
    dbg('fayl boshi o\'qilmoqda');
    const moovBytes = await readMoov(file);
    dbg('jadval o\'qildi');
    if (destroyed) return;
    layout = buildLayout(moovBytes);
    init = layout.init;
    segs = layout.segs;
    duration = layout.duration;
    // Boshlanadigan bo'lakni DARHOL yuklab boshlaymiz (MSE tayyorlanishini kutmasdan).
    { const f0 = segs[segIndexAt(startAt || 0)]; if (f0) prefetchSegs([f0]); }
    const codecs = trackCodecs(moovBytes, boxes(moovBytes).find((x) => x.type === 'moov'));
    const mime = `video/mp4; codecs="${codecs.join(', ')}"`;
    if (!MS || !MS.isTypeSupported(mime)) {
      const e = new Error('codec_unsupported');
      e.mime = mime;
      throw e;
    }
    dbg(`pleyer tayyorlanmoqda (${mime.slice(0, 50)})`);
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
      if (destroyed) return;
      checkHold();
      if (video.readyState >= 3) { if (!pumping && next < segs.length && bufferedAhead() <= ahead) pump(); return; }
      const t = video.currentTime; const r = video.buffered;
      for (let i = 0; i < r.length; i++) {
        if (r.start(i) > t && r.start(i) - t < 1.5) { try { video.currentTime = r.start(i) + 0.02; } catch (_) { /* */ } return; }
      }
      if (!pumping && next < segs.length && bufferedAhead() <= ahead) pump();
    }, 250);
  })();
  ready.catch((e) => { if (!destroyed) onError(e); });

  return {
    ready,
    format: 'mp4',
    get duration() { return duration; },
    /** Qotgani uchun pauzada ushlab turilibdimi (foydalanuvchi pauzasi emas). */
    get holding() { return holding; },
    /** Foydalanuvchi pauza bosdi — ushlab turish bekor (o'zi davom etmaydi). */
    cancelHold() { holding = false; },
    /** Yuklash tezligi va videoning o'rtacha bitreyti (kbit/s). */
    health() { return { kbps: file?.rateKbps?.() || 0, needKbps: needKbps(), stalls }; },
    /** Boshlanish nuqtasi (to'xtagan joydan davom etish). */
    startAt(sec) {
      ready.then(() => { video.currentTime = Math.max(0, Math.min(sec, duration - 1)); });
    },
    /**
     * Sek: video eski joyda 1x da o'ynashda davom etadi, `t` atrofida bufer yig'iladi
     * (>= 2 s yoki fayl oxirigacha); tayyor bo'lgach `true` (eskirgan so'rov — `false`).
     */
    requestSeek(t) {
      const my = ++seekReq;
      focus = t;
      gen += 1;
      try { file?.cancelPrefetch?.(); } catch (_) { /* */ }
      next = segIndexAt(t);
      pump();
      return new Promise((resolve) => {
        const t0 = Date.now();
        const iv = setInterval(() => {
          if (destroyed || my !== seekReq) { clearInterval(iv); resolve(false); return; }
          if (bufferedAheadAt(t) >= Math.min(1.2, Math.max(0.3, duration - t - 0.3)) || Date.now() - t0 > 20000) {
            clearInterval(iv); if (my === seekReq) focus = null; resolve(true);
          }
        }, 80);
      });
    },
    setAhead(sec) { ahead = Math.min(MAX_AHEAD, sec); pump(); },
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
export const _test = { boxes, trackCodecs };
