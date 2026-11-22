// Qism kadri (thumbnail): KERAK BO'LGANDA brauzerning o'zida yasaladi va FAQAT shu brauzer keshida (IndexedDB `aru-thumbs`) saqlanadi —
// serverga, bazaga, Telegram'ga HECH QAYERGA yuborilmaydi.
// Qanday: pleyer dvigateli (`player/engine.js`) yashirin <video> ga videoning ~30% nuqtasidagi bo'lakni (kalit kadrdan) beradi, kadr
// canvas -> kichik JPEG. Baytlar odatdagi yo'l bilan olinadi (ko'rilgan qism keshda bo'lsa tarmoq ketmaydi). H.265 ni brauzer dekodlay
// olmasa — `null` (muqova ko'rinadi). Bir vaqtda bitta kadr yasaladi.

import { vdInfo, vdInfoError } from './store.js';
import { createEngine, engineSupported } from '../player/engine.js';
import { cacheCountPrefix } from '../tg/chunk-cache.js';
import { fileNameOf } from '../player/source.js';

const QS = ['1080p', '720p', '480p', '360p'];
const W = 360;
const MAX_ENTRIES = 400;
const POINT = 0.3;

let dbP = null;
function db() {
  if (!dbP) {
    dbP = new Promise((res) => {
      try {
        const r = indexedDB.open('aru-thumbs', 1);
        r.onupgradeneeded = () => { const s = r.result.createObjectStore('t'); s.createIndex('at', 'at'); };
        r.onsuccess = () => res(r.result);
        r.onerror = () => res(null);
      } catch (_) { res(null); }
    });
  }
  return dbP;
}
const rq = (r) => new Promise((res) => { r.onsuccess = () => res(r.result); r.onerror = () => res(undefined); });

const FAIL_MS = 7 * 24 * 3600 * 1000;
/** Saqlangan kadr (`Blob`), `'fail'` (yaqinda yasab bo'lmagan: qayta urinib trafik sarflanmasin) yoki `null`. */
async function readStored(name) {
  const d = await db(); if (!d) return null;
  const v = await rq(d.transaction('t').objectStore('t').get(name));
  if (v?.fail) return Date.now() - (v.at || 0) < FAIL_MS ? 'fail' : null;
  return v?.blob || null;
}
async function markFail(name) {
  const d = await db(); if (!d) return;
  try { d.transaction('t', 'readwrite').objectStore('t').put({ fail: true, at: Date.now() }, name); } catch (_) { /* */ }
}
async function store(name, blob) {
  const d = await db(); if (!d) return;
  try {
    const tx = d.transaction('t', 'readwrite'); const st = tx.objectStore('t');
    st.put({ blob, at: Date.now() }, name);
    const n = await rq(st.count());
    if (n > MAX_ENTRIES) {
      let drop = n - MAX_ENTRIES + 40;
      st.index('at').openCursor().onsuccess = (e) => { const c = e.target.result; if (c && drop-- > 0) { c.delete(); c.continue(); } };
    }
  } catch (_) { /* */ }
}

const urls = new Map(); // nom -> Promise<objectURL|null>
let chain = Promise.resolve();

async function make(name) {
  if (!engineSupported()) return null;
  let info = null;
  try { info = await vdInfo(name); } catch (_) { return null; }
  if (!info || vdInfoError(name)) return null;
  const video = document.createElement('video');
  video.muted = true; video.playsInline = true; video.preload = 'auto';
  let eng = null;
  try {
    eng = createEngine(video, { info, noAhead: true, ahead: 2, shouldPlay: () => false });
    await new Promise((res, rej) => {
      const t = setTimeout(() => rej(new Error('timeout')), 25000);
      video.addEventListener('loadedmetadata', () => { clearTimeout(t); res(); }, { once: true });
      video.addEventListener('error', () => { clearTimeout(t); rej(new Error('video_error')); }, { once: true });
    });
    const dur = Number.isFinite(video.duration) ? video.duration : 0;
    const t = dur > 4 ? dur * POINT : 0.5;
    await eng.requestSeek(t);
    video.currentTime = t;
    await new Promise((res, rej) => {
      const to = setTimeout(() => rej(new Error('seek_timeout')), 20000);
      const ok = () => { if (video.readyState >= 2) { clearTimeout(to); res(); } };
      video.addEventListener('seeked', ok); video.addEventListener('loadeddata', ok); video.addEventListener('canplay', ok);
      ok();
    });
    const vw = video.videoWidth || 16; const vh = video.videoHeight || 9;
    const c = document.createElement('canvas');
    c.width = W; c.height = Math.max(1, Math.round((W * vh) / vw));
    c.getContext('2d').drawImage(video, 0, 0, c.width, c.height);
    const blob = await new Promise((r) => c.toBlob(r, 'image/jpeg', 0.72));
    return blob || null;
  } catch (_) {
    return null;
  } finally {
    try { eng?.destroy(); } catch (_) { /* */ }
  }
}

/** Fayl nomi bo'yicha kadr manzili (`blob:`) yoki `null`. Natija brauzer keshida; qayta so'ralsa darhol. */
export function thumbOfFile(name) {
  if (!name) return Promise.resolve(null);
  let p = urls.get(name);
  if (p) return p;
  p = (async () => {
    const saved = await readStored(name);
    if (saved === 'fail') return null;
    if (saved) return URL.createObjectURL(saved);
    // Navbat: bir vaqtda bitta (telefon qotmasin, trafik kam bo'lsin).
    const run = chain.then(() => make(name));
    chain = run.catch(() => {});
    const blob = await run;
    if (!blob) { markFail(name); return null; }
    store(name, blob);
    return URL.createObjectURL(blob);
  })();
  urls.set(name, p);
  return p;
}

/** Qism uchun: keshdagi eng ko'p yuklangan sifat (tarmoq kam ketsin), bo'lmasa eng kichigi. */
export async function thumbOfEpisode(ep, { exclude = '' } = {}) {
  const names = QS.map((q) => fileNameOf(ep?.[`url_${q}`])).filter((n) => n && n !== exclude);
  if (!names.length) return null;
  const done = names.map((n) => thumbOfFile.cached?.(n)).find(Boolean);
  if (done) return done;
  let best = names[names.length - 1]; let most = 0;
  for (const n of names) {
    const { bytes } = await cacheCountPrefix(`vd:${n}#`);
    if (bytes > most) { most = bytes; best = n; }
  }
  // Allaqachon saqlangan kadr bo'lsa o'sha.
  for (const n of names) { const s = await readStored(n); if (s && s !== 'fail') return URL.createObjectURL(s); }
  if ((await Promise.all(names.map(readStored))).some((x) => x === 'fail')) return null;
  return thumbOfFile(best);
}
