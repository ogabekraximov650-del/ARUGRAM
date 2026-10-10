// Video bo'laklarining DISK keshi (IndexedDB `aru-chunks`) — ilovadagi shifrlangan bo'lak keshi kabi.
//
// Bo'laklar Telegram'dan KELGAN holida (shifrlangan, kalitsiz o'qib bo'lmaydi) saqlanadi:
// qayta ochish, orqaga surish va uzilgan joydan davom etish tarmoqsiz, darhol.
//
// HAJM: standart "Avtomatik" — qurilma/brauzer ruxsat bergancha (`navigator.storage.estimate`).
// Eski bo'laklar FAQAT yangisi sig'masa o'chadi (eng uzoq ishlatilmagani birinchi, kerakli
// hajmgacha, ortig'i emas). Sozlamalarda qat'iy chegara (500 MB ... 5 GB) tanlash va keshni
// tozalash mumkin (`screens/settings.js`). IndexedDB bo'lmasa (maxfiy rejim) — jim o'tkaziladi.
// Mtcute'ga bog'liq emas: sozlamalar ekrani uni yuklamasin.

const LS_LIMIT = 'aru_cache_limit'; // 'auto' | bayt
let dbP = null;
let idx = null; // Map: kalit -> {size, at}
let total = 0;
let loading = null;
let quotaAt = 0;
let quota = 0;
let usage = 0;

function db() {
  if (!dbP) {
    dbP = new Promise((res, rej) => {
      try {
        const r = indexedDB.open('aru-chunks', 1);
        r.onupgradeneeded = () => { r.result.createObjectStore('c'); r.result.createObjectStore('m'); };
        r.onsuccess = () => res(r.result);
        r.onerror = () => rej(r.error);
      } catch (e) { rej(e); }
    }).catch(() => null);
  }
  return dbP;
}
const req = (r) => new Promise((res) => { r.onsuccess = () => res(r.result); r.onerror = () => res(undefined); });
const txDone = (tx) => new Promise((res) => { tx.oncomplete = () => res(true); tx.onerror = () => res(false); tx.onabort = () => res(false); });

async function index() {
  if (idx) return idx;
  if (!loading) {
    loading = (async () => {
      const m = new Map(); total = 0;
      const d = await db();
      if (d) {
        const keys = await req(d.transaction('m').objectStore('m').getAllKeys());
        const vals = await req(d.transaction('m').objectStore('m').getAll());
        (keys || []).forEach((k, i) => { const v = vals?.[i] || {}; m.set(k, { size: v.size || 0, at: v.at || 0 }); total += v.size || 0; });
      }
      idx = m;
      return m;
    })();
  }
  return loading;
}

export function cacheLimitSetting() {
  try { return localStorage.getItem(LS_LIMIT) || 'auto'; } catch (_) { return 'auto'; }
}
export async function setCacheLimitSetting(v) {
  try { localStorage.setItem(LS_LIMIT, `${v}`); } catch (_) { /* */ }
  const lim = Number(v);
  if (Number.isFinite(lim) && lim > 0) await shrinkTo(lim); // yangi (kichik) chegaradan oshganini darhol tozalaydi
}

async function refreshQuota() {
  if (Date.now() - quotaAt < 15000) return;
  quotaAt = Date.now();
  try { const e = await navigator.storage?.estimate?.(); quota = e?.quota || 0; usage = e?.usage || 0; } catch (_) { /* */ }
}

async function evictOldest(d) {
  let oldK = null; let oldAt = Infinity;
  for (const [k, v] of idx) if (v.at < oldAt) { oldAt = v.at; oldK = k; }
  if (oldK == null) return 0;
  const size = idx.get(oldK).size;
  const tx = d.transaction(['c', 'm'], 'readwrite');
  tx.objectStore('c').delete(oldK); tx.objectStore('m').delete(oldK);
  await txDone(tx);
  idx.delete(oldK); total -= size; usage = Math.max(0, usage - size);
  return size;
}

async function shrinkTo(limit) {
  const d = await db(); if (!d) return;
  await index();
  while (total > limit && idx.size) { if (!(await evictOldest(d))) break; }
}

/** Yangi `size` bayt uchun joy bormi — yo'q bo'lsa FAQAT kerakli miqdorda eskisini o'chiradi. */
async function makeRoom(d, size) {
  const set = cacheLimitSetting();
  if (set !== 'auto') {
    const lim = Number(set);
    while (idx.size && total + size > lim) { if (!(await evictOldest(d))) break; }
    return;
  }
  await refreshQuota();
  // Avtomatik: brauzer ruxsatining ~95% gacha; to'lib borsa (yoki baho yo'q) — Quota xatosi kutiladi.
  while (idx.size && quota > 0 && usage + size > quota * 0.95) { if (!(await evictOldest(d))) break; }
}

export async function cacheGet(key, len) {
  try {
    const d = await db(); if (!d) return null;
    const buf = await req(d.transaction('c').objectStore('c').get(key));
    if (!buf || buf.byteLength !== len) return null;
    const m = await index(); const e = m.get(key);
    if (e) { e.at = Date.now(); d.transaction('m', 'readwrite').objectStore('m').put({ size: e.size, at: e.at }, key); }
    return new Uint8Array(buf);
  } catch (_) { return null; }
}

export async function cachePut(key, u8) {
  try {
    const d = await db(); if (!d) return;
    const m = await index();
    if (m.has(key)) return;
    const size = u8.byteLength;
    const buf = u8.slice().buffer; // subarray butun xotirani nusxalamasin
    await makeRoom(d, size);
    for (let attempt = 0; attempt < 8; attempt++) {
      const tx = d.transaction(['c', 'm'], 'readwrite');
      const at = Date.now();
      tx.objectStore('c').put(buf, key); tx.objectStore('m').put({ size, at }, key);
      if (await txDone(tx)) { m.set(key, { size, at }); total += size; usage += size; return; }
      // Joy yetmadi (QuotaExceededError) — eng eskisini o'chirib, qayta urinamiz.
      if (!m.size || !(await evictOldest(d))) return;
    }
  } catch (_) { /* */ }
}

/** `{used, count, setting, quota, usage}` — sozlamalar ekrani uchun. */
export async function cacheStats() {
  const m = await index();
  await refreshQuota();
  return { used: total, count: m.size, setting: cacheLimitSetting(), quota, usage };
}

export async function cacheClear() {
  const d = await db();
  const m = await index();
  if (d) {
    const tx = d.transaction(['c', 'm'], 'readwrite');
    tx.objectStore('c').clear(); tx.objectStore('m').clear();
    await txDone(tx);
  }
  m.clear(); total = 0; quotaAt = 0;
}
