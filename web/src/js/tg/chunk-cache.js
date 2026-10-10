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
//
// Uch xil yozuv (hammasi bitta hajm hisobida, bitta LRU):
//   * video bo'laklari `c` — Telegram'dan kelgan (shifrlangan) holida;
//   * kichik fayllar `f` (rasm, ovoz, avatar) — to'liq, qurilma kaliti bilan AES-GCM;
//   * fayl ma'lumoti (`size`, ochish kaliti) — o'sha kalit bilan shifrlangan, shu sabab
//     keshdagi fayl Telegram'ga bot nusxasini kutmasdan DARHOL ochiladi.
// Qurilma kaliti — shu brauzerdan chiqmaydigan (`extractable: false`) AES-GCM kaliti.

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
        const r = indexedDB.open('aru-chunks', 2);
        r.onupgradeneeded = () => {
          for (const n of ['c', 'm', 'f', 'k']) if (!r.result.objectStoreNames.contains(n)) r.result.createObjectStore(n);
        };
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
  const tx = d.transaction(['c', 'm', 'f'], 'readwrite');
  tx.objectStore('c').delete(oldK); tx.objectStore('f').delete(oldK); tx.objectStore('m').delete(oldK);
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
    const tx = d.transaction(['c', 'm', 'f'], 'readwrite');
    tx.objectStore('c').clear(); tx.objectStore('f').clear(); tx.objectStore('m').clear();
    await txDone(tx);
  }
  m.clear(); total = 0; quotaAt = 0;
}

// ── Kichik fayllar va fayl ma'lumoti (qurilma kaliti bilan shifrlangan) ──────────
let devKeyP = null;
function devKey() {
  if (!devKeyP) {
    devKeyP = (async () => {
      const d = await db(); if (!d) return null;
      let k = await req(d.transaction('k').objectStore('k').get('dev'));
      if (!k) {
        k = await crypto.subtle.generateKey({ name: 'AES-GCM', length: 256 }, false, ['encrypt', 'decrypt']);
        const tx = d.transaction('k', 'readwrite'); tx.objectStore('k').put(k, 'dev'); await txDone(tx);
      }
      return k;
    })().catch(() => null);
  }
  return devKeyP;
}
async function seal(bytes) {
  const k = await devKey(); if (!k) return null;
  const iv = crypto.getRandomValues(new Uint8Array(12));
  const ct = new Uint8Array(await crypto.subtle.encrypt({ name: 'AES-GCM', iv }, k, bytes));
  const out = new Uint8Array(12 + ct.length); out.set(iv); out.set(ct, 12);
  return out.buffer;
}
async function unseal(buf) {
  const k = await devKey(); if (!k || !buf) return null;
  const u = new Uint8Array(buf);
  try { return new Uint8Array(await crypto.subtle.decrypt({ name: 'AES-GCM', iv: u.subarray(0, 12) }, k, u.subarray(12))); } catch (_) { return null; }
}

async function putSealed(key, buf) {
  const d = await db(); if (!d || !buf) return;
  const m = await index();
  const size = buf.byteLength;
  if (m.has(key)) { total -= m.get(key).size; m.delete(key); }
  await makeRoom(d, size);
  for (let attempt = 0; attempt < 8; attempt++) {
    const tx = d.transaction(['f', 'm'], 'readwrite');
    const at = Date.now();
    tx.objectStore('f').put(buf, key); tx.objectStore('m').put({ size, at }, key);
    if (await txDone(tx)) { m.set(key, { size, at }); total += size; usage += size; return; }
    if (!m.size || !(await evictOldest(d))) return;
  }
}
async function getSealed(key) {
  const d = await db(); if (!d) return null;
  const buf = await req(d.transaction('f').objectStore('f').get(key));
  if (!buf) return null;
  const m = await index(); const e = m.get(key);
  if (e) { e.at = Date.now(); d.transaction('m', 'readwrite').objectStore('m').put({ size: e.size, at: e.at }, key); }
  return unseal(buf);
}

/** Kichik fayl (rasm, ovoz): `{bytes, type}` yoki null. */
export async function fileGet(name) {
  try {
    const u = await getSealed(`f:${name}`);
    if (!u || u.length < 2) return null;
    const tl = u[0]; const type = new TextDecoder().decode(u.subarray(1, 1 + tl));
    return { bytes: u.subarray(1 + tl), type };
  } catch (_) { return null; }
}
export async function filePut(name, bytes, type) {
  try {
    const t = new TextEncoder().encode(type || '');
    const buf = new Uint8Array(1 + t.length + bytes.length);
    buf[0] = t.length; buf.set(t, 1); buf.set(bytes, 1 + t.length);
    await putSealed(`f:${name}`, await seal(buf));
  } catch (_) { /* */ }
}
/** Fayl ma'lumoti: `{size, key}` (video/to'plam fayllari Telegram'ga so'rovsiz ochilishi uchun). */
export async function metaGet(name) {
  try {
    const u = await getSealed(`f:meta:${name}`);
    const m = u ? JSON.parse(new TextDecoder().decode(u)) : null;
    // Eski yozuvda ochish kaliti bor edi — diskdan o'chiriladi (kalitlar faqat onlayn olinadi).
    if (m && 'key' in m) { const clean = { size: m.size }; metaPut(name, clean); return clean; }
    return m;
  } catch (_) { return null; }
}
export async function metaPut(name, meta) {
  try { await putSealed(`f:meta:${name}`, await seal(new TextEncoder().encode(JSON.stringify(meta)))); } catch (_) { /* */ }
}

/** Hisobdan chiqishda: hamma kesh va qurilma kaliti o'chadi (keyingi hisob eskisini ko'rmasin). */
export async function cacheWipe() {
  try { await cacheClear(); } catch (_) { /* */ }
  try {
    const d = await db();
    if (d) { const tx = d.transaction('k', 'readwrite'); tx.objectStore('k').clear(); await txDone(tx); }
  } catch (_) { /* */ }
  devKeyP = null;
}
