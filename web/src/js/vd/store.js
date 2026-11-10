// Yangi tizim (`vd`, ARUDRIVE uslubi) fayllari — Mini App tomoni. Android ilovadagi `vd/VdApi.kt` + `VdBlocks.kt` ning nusxasi:
//   * `/api/vd/info` (sessiya bilan) — hajm va ochish kaliti; kalit faqat kirgan foydalanuvchiga;
//   * `/api/warm/vd~<nom>?w=N` — 18 MiB oyna Telegram'dan Cloudflare keshiga isitiladi;
//   * `/api/play/vd~<nom>` + `Range` — shifrlangan baytlar; brauzer AES-128-CTR bilan ochadi (WebCrypto);
//   * 1 MiB bo'laklar IndexedDB'da (shifrlangan holda, `chunk-cache.js`) — qayta ochish tarmoqsiz.
// Telegram hisobi KERAK EMAS.

import { api, apiPost } from '../api.js';
import { cacheGet, cachePut } from '../tg/chunk-cache.js';
import { ctrApply } from '../tg/ctr.js';

/** Tashxis izi: pleyer ekranida (yuklanayotganda) ko'rinadi — qaysi bosqichda to'xtagani aniq bo'lsin. */
export function dbg(msg) {
  try { window.__vdDbg = `${msg}`; window.dispatchEvent(new CustomEvent('vd-dbg')); } catch (_) { /* */ }
}

export const BLOCK = 1024 * 1024;
export const WINDOW = 18 * 1024 * 1024;

const infos = new Map(); // nom -> {name,size,key,state,chunks,stored}
const infoWait = new Map();
const infoErr = new Map(); // nom -> oxirgi xato (server javob bermadi / rad etdi) — "tayyor emas" dan farqlash uchun

/** Oxirgi `vdInfo` xatosi (`null` — xato yo'q: fayl shunchaki yangi tizimda yo'q yoki hali tayyor emas). */
export const vdInfoError = (name) => infoErr.get(name) || null;

export const playable = (i) => !!i && i.size > 0 && `${i.key}`.length === 32 && (i.state === 'ready' || i.stored > 0);

function norm(name, f) {
  return { name, size: Number(f.size) || 0, key: `${f.key ?? ''}`, state: `${f.state ?? ''}`, chunks: Number(f.chunks) || 0, stored: Number(f.stored) || 0 };
}

/** Fayl ma'lumoti (`null` — yangi tizimda yo'q / hali tayyor emas). `fresh` — keshni o'tkazib so'raydi. */
export async function vdInfo(name, { fresh = false } = {}) {
  if (!name) return null;
  const c = infos.get(name);
  if (c && !fresh && c.state === 'ready') return c;
  if (infoWait.has(name)) return infoWait.get(name);
  const p = (async () => {
    try {
      dbg('ma\'lumot so\'ralmoqda');
      const j = await apiPost('/api/vd/info', { names: [name] });
      infoErr.delete(name);
      dbg(`ma'lumot: ${j?.files?.[name] ? `${j.files[name].state} ${j.files[name].size}` : 'yo\'q'}`);
      const f = j?.files?.[name];
      if (!f) return null;
      const i = norm(name, f);
      if (!playable(i)) return null;
      infos.set(name, i);
      return i;
    } catch (e) {
      infoErr.set(name, e);
      dbg(`ma'lumot xato: ${e?.status || ''} ${e?.message || e}`);
      return infos.get(name) || null;
    } finally {
      infoWait.delete(name);
    }
  })();
  infoWait.set(name, p);
  return p;
}

const warmed = new Set();
const warming = new Map();

/** Oyna keshga isitilguncha kutadi (bo'lak hali Telegram'ga saqlanmagan bo'lsa "pending" — 10 daqiqagacha qayta so'raydi). */
async function warm(name, w, force = false, signal) {
  const key = `${name}:${w}`;
  if (!force && warmed.has(key)) return;
  if (!force && warming.has(key)) return warming.get(key);
  const run = (async () => {
    const end = Date.now() + 10 * 60_000;
    let first = force;
    for (;;) {
      if (signal?.aborted) throw new Error('cancelled');
      let st = '';
      try {
        const t0 = Date.now();
        dbg(`isitilmoqda: oyna ${w}`);
        const tick = setInterval(() => dbg(`isitilmoqda: oyna ${w} (${Math.round((Date.now() - t0) / 1000)} s)`), 1000);
        let j;
        try { j = await api(`/api/warm/vd~${encodeURIComponent(name)}?w=${w}${first ? '&force=1' : ''}`, { signal }); } finally { clearInterval(tick); }
        st = `${j?.status ?? j?.raw ?? ''}`;
        dbg(`oyna ${w}: ${st.slice(0, 40)} (${Math.round((Date.now() - t0) / 1000)} s)`);
      } catch (e) {
        if (e?.name === 'AbortError') throw new Error('cancelled');
        throw e;
      }
      first = false;
      if (st === 'cached' || st === 'warmed') { warmed.add(key); return; }
      if (!st.startsWith('pending') || Date.now() > end) throw new Error('warm_failed');
      await new Promise((r) => setTimeout(r, 3000));
    }
  })();
  if (!force) warming.set(key, run);
  try { await run; } finally { warming.delete(key); }
}

const STALL_MS = 20_000; // shuncha vaqt bitta bayt ham kelmasa so'rov uziladi va qayta uriniladi

/** Javob tanasini oqim bilan o'qiydi: jarayon ekranda ko'rinadi, to'xtab qolsa (`STALL_MS`) uziladi. */
async function readBody(r, expect, label, t0, touch) {
  const reader = r.body.getReader();
  const out = new Uint8Array(expect);
  let got = 0;
  for (;;) {
    const { done, value } = await reader.read();
    if (done) break;
    if (got + value.length > expect) throw new Error('chunk_long');
    out.set(value, got); got += value.length;
    touch();
    dbg(`${label}: ${(got / 1048576).toFixed(1)}/${(expect / 1048576).toFixed(1)} MB (${Math.round((Date.now() - t0) / 1000)} s)`);
  }
  if (got !== expect) throw new Error('chunk_short');
  return out;
}

/** Shifrlangan `[start, end]` (ikkala chegara ham kiradi) baytlar — bitta so'rov. */
async function fetchRange(name, start, end, signal) {
  const w = Math.floor(start / WINDOW);
  const label = `yuklanmoqda ${(start / 1048576).toFixed(0)}-${((end + 1) / 1048576).toFixed(0)} MB`;
  for (let attempt = 0; attempt < 3; attempt++) {
    const ac = new AbortController();
    const onAbort = () => ac.abort();
    signal?.addEventListener('abort', onAbort, { once: true });
    let last = Date.now(); let stalled = false; let watch = false; // isitish uzoq davom etishi mumkin — kuzatuv faqat bayt so'rovida
    const iv = setInterval(() => { if (watch && Date.now() - last > STALL_MS) { stalled = true; ac.abort(); } }, 2000);
    try {
      await warm(name, w, attempt > 0, ac.signal);
      last = Date.now(); watch = true;
      const t0 = Date.now();
      dbg(`${label}: so'ralmoqda`);
      const r = await api(`/api/play/vd~${encodeURIComponent(name)}`, { headers: { Range: `bytes=${start}-${end}` }, raw: true, signal: ac.signal });
      last = Date.now();
      return await readBody(r, end - start + 1, label, t0, () => { last = Date.now(); });
    } catch (e) {
      if (signal?.aborted) throw new Error('cancelled');
      const why = stalled ? `javob ${STALL_MS / 1000} s kelmadi` : `${e?.status || ''} ${e?.message || e}`;
      dbg(`xato (${attempt + 1}/3): ${why}`);
      if (attempt === 2) throw new Error(stalled ? 'chunk_timeout' : (e?.message || 'download_failed'));
      await new Promise((r) => setTimeout(r, 600 * (attempt + 1)));
    } finally {
      clearInterval(iv);
      signal?.removeEventListener('abort', onAbort);
    }
  }
  throw new Error('download_failed');
}

/**
 * Pleyer uchun fayl: `size`, `read(pos, len)` (OCHILGAN baytlar), `prefetch(pos, len)`, `cancelPrefetch()`, `rateKbps()`, `close()`.
 * Bo'laklar 1 MiB; bir so'rovda ketma-ket yetishmayotgan bo'laklar (ko'pi bilan 4 ta, bitta 18 MiB oyna ichida) olinadi, ortig'i emas.
 */
export function openVdFile(info) {
  const { name, size } = info;
  const key = info.key;
  const nBlocks = Math.ceil(size / BLOCK);
  const mem = new Map(); // idx -> ochilgan bo'lak (LRU)
  const inflight = new Map(); // idx -> Promise<Uint8Array>
  let ctl = new AbortController();
  const MEM_MAX = 48;
  let bytesGot = 0; let t0 = 0; let lastKbps = 0;
  const blockLen = (i) => Math.min(BLOCK, size - i * BLOCK);
  const ckey = (i) => `vd:${name}#${i}`;

  function remember(i, u8) {
    mem.delete(i); mem.set(i, u8);
    while (mem.size > MEM_MAX) mem.delete(mem.keys().next().value);
  }

  async function fromDisk(i) {
    const enc = await cacheGet(ckey(i), blockLen(i));
    return enc ? ctrApply(key, i * BLOCK, enc) : null;
  }

  const perWin = WINDOW / BLOCK;

  /** `idx` dan boshlab `count` ta ketma-ket bo'lak BITTA so'rovda olinadi (worker so'rovlari kam bo'lsin). */
  function startRun(idx, count) {
    const ids = Array.from({ length: count }, (_, k) => idx + k);
    const my = ctl.signal;
    const start = idx * BLOCK;
    const end = Math.min(size, (idx + count) * BLOCK) - 1;
    const whole = (async () => {
      const t = performance.now();
      const enc = await fetchRange(name, start, end, my);
      bytesGot += enc.length;
      const dt = (performance.now() - t) / 1000;
      if (dt > 0.05) lastKbps = Math.round((enc.length * 8) / dt / 1000);
      return enc;
    })();
    ids.forEach((i, n) => {
      const pr = (async () => {
        try {
          const enc = await whole;
          const part = enc.subarray(n * BLOCK, n * BLOCK + blockLen(i));
          const plain = await ctrApply(key, i * BLOCK, part);
          cachePut(ckey(i), part);
          remember(i, plain);
          return plain;
        } finally { inflight.delete(i); }
      })();
      pr.catch(() => {}); // kutayotgan bo'lmasa ham "unhandled" chiqmasin
      inflight.set(i, pr);
    });
  }

  /** `first..last` bo'laklari (ochilgan): xotira -> disk -> tarmoq; yetishmaydigan ketma-ketlari (<= 4 ta, bitta oyna ichida) bitta so'rovda. */
  async function getBlocks(first, last) {
    const res = new Array(last - first + 1);
    const need = [];
    for (let i = first; i <= last; i++) {
      const m = mem.get(i);
      if (m) { remember(i, m); res[i - first] = m; } else if (inflight.has(i)) res[i - first] = inflight.get(i); else need.push(i);
    }
    const disk = await Promise.all(need.map((i) => fromDisk(i)));
    const miss = [];
    need.forEach((i, k) => {
      if (disk[k]) { remember(i, disk[k]); res[i - first] = disk[k]; } else if (inflight.has(i)) res[i - first] = inflight.get(i); else miss.push(i);
    });
    for (let k = 0; k < miss.length;) {
      let e = k;
      while (e + 1 < miss.length && miss[e + 1] === miss[e] + 1 && e - k < 3 && Math.floor(miss[e + 1] / perWin) === Math.floor(miss[k] / perWin)) e++;
      if (!t0) t0 = performance.now();
      startRun(miss[k], e - k + 1);
      for (let j = k; j <= e; j++) res[miss[j] - first] = inflight.get(miss[j]);
      k = e + 1;
    }
    return Promise.all(res);
  }

  return {
    size,
    async read(pos, len) {
      if (pos < 0 || len <= 0 || pos >= size) return new Uint8Array(0);
      const end = Math.min(size, pos + len);
      const first = Math.floor(pos / BLOCK); const last = Math.floor((end - 1) / BLOCK);
      const got = await getBlocks(first, last);
      if (first === last) return got[0].subarray(pos - first * BLOCK, end - first * BLOCK);
      const out = new Uint8Array(end - pos);
      let o = 0;
      got.forEach((b, k) => {
        const i = first + k;
        const s = i === first ? pos - first * BLOCK : 0;
        const e = i === last ? end - last * BLOCK : b.length;
        out.set(b.subarray(s, e), o); o += e - s;
      });
      return out;
    },
    /** Oldindan so'rab qo'yish (natija kutilmaydi). */
    prefetch(pos, len) {
      if (pos >= size || len <= 0) return;
      const end = Math.min(size, pos + len);
      getBlocks(Math.floor(pos / BLOCK), Math.floor((end - 1) / BLOCK)).catch(() => {});
    },
    cancelPrefetch() {
      // Hali boshlanmagan so'rovlar bekor qilinadi; yangi nuqtadan davom etish uchun yangi signal.
      try { ctl.abort(); } catch (_) { /* */ }
      ctl = new AbortController();
    },
    rateKbps() { return lastKbps; },
    close() { try { ctl.abort(); } catch (_) { /* */ } mem.clear(); },
  };
}

/** Rasm uchun ochilgan baytlar (<= 18 MiB, bitta bo'lak). `imginfo` — sessiyasiz, kirmagan foydalanuvchiga ham. */
const imgMeta = new Map();
export async function vdImageInfo(names) {
  const need = [...new Set(names)].filter((n) => !imgMeta.has(n));
  for (let i = 0; i < need.length; i += 100) {
    const part = need.slice(i, i + 100);
    let files = {};
    try { files = (await apiPost('/api/vd/imginfo', { names: part }))?.files || {}; } catch (_) { /* */ }
    for (const n of part) imgMeta.set(n, files[n] ? norm(n, files[n]) : null);
  }
  return names.map((n) => imgMeta.get(n) || null);
}

export async function vdImageBytes(info) {
  const r = await api(`/api/vd/img/${encodeURIComponent(info.name)}`, { raw: true });
  const enc = new Uint8Array(await r.arrayBuffer());
  if (info.size && enc.length !== info.size) throw new Error('img_size');
  return ctrApply(info.key, 0, enc);
}
