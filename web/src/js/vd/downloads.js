// Yuklab olish (ilovadagi `vd/VdDownloads.kt` kabi): video 18 MiB oynalarda yuklanadi, shifrlangan holda
// qurilma keshiga (IndexedDB, `tg/chunk-cache.js`) yoziladi va kesh to'lganda ham o'chmaydi (`setPinned`).
// Pleyer shu keshdan (internetsiz ham) o'qiydi. Ko'rilgan joy qayta yuklanmaydi (bo'lak keshda bo'lsa o'tkaziladi).

import { vdInfo, openVdFile, BLOCK, WINDOW } from './store.js';
import { cacheCountPrefix, cacheDeletePrefix, setPinned } from '../tg/chunk-cache.js';

const KEY = 'aru_dl_done';
const read = () => { try { return JSON.parse(localStorage.getItem(KEY) || '{}') || {}; } catch (_) { return {}; } };
const write = (o) => { try { localStorage.setItem(KEY, JSON.stringify(o)); } catch (_) { /* */ } };

const stats = new Map(); // nom -> {total, got, run, queued, retry, speed, done}
const jobs = new Map(); // nom -> {abort}
const queue = [];
let active = 0;
const MAX_FILES = 2;
const PAR = 3; // bir faylda parallel oynalar (har biri <= 18 MiB, ichida 4 MiB so'rovlar)

export const dlKey = (name) => `vd:${name}#`;
const emit = (name) => window.dispatchEvent(new CustomEvent('aru-dl', { detail: { name } }));
const done0 = () => read();
setPinned(Object.keys(done0()));

export function dlStat(name) {
  const s = stats.get(name);
  if (s) return s;
  const d = done0()[name];
  return d ? { total: d, got: d, run: false, queued: false, retry: false, speed: 0, done: true } : { total: 0, got: 0, run: false, queued: false, retry: false, speed: 0, done: false };
}
export const dlComplete = (name) => !!done0()[name];

async function startNext() {
  while (active < MAX_FILES && queue.length) {
    const name = queue.shift();
    active++;
    run(name).finally(() => { active--; startNext(); });
  }
}

async function run(name) {
  const st = stats.get(name) || { total: 0, got: 0 };
  const ac = new AbortController();
  jobs.set(name, ac);
  Object.assign(st, { run: true, queued: false, retry: false, done: false });
  stats.set(name, st); emit(name);
  let file = null;
  try {
    let info = null;
    for (let i = 0; i < 40 && !info && !ac.signal.aborted; i++) {
      info = await vdInfo(name, { fresh: true });
      if (!info) await new Promise((r) => setTimeout(r, 3000));
    }
    if (!info) throw new Error('no_info');
    st.total = info.size;
    file = openVdFile(info);
    // Ko'rilgan joyni o'tkazish: keshdagi bo'laklar sanaladi.
    st.got = Math.min(info.size, (await cacheCountPrefix(dlKey(name))).bytes);
    const n = Math.ceil(info.size / WINDOW);
    let nextW = 0; let fail = 0;
    const t0 = Date.now(); const got0 = st.got;
    const worker = async () => {
      for (;;) {
        if (ac.signal.aborted) return;
        const w = nextW++;
        if (w >= n) return;
        const pos = w * WINDOW; const len = Math.min(WINDOW, info.size - pos);
        for (let attempt = 0; ; attempt++) {
          if (ac.signal.aborted) return;
          try {
            // Oyna 4 MiB bo'laklarda ketma-ket (har biri ichida worker parallel emas, 3 oyna parallel).
            for (let o = 0; o < len; o += 4 * BLOCK) {
              if (ac.signal.aborted) return;
              const l = Math.min(4 * BLOCK, len - o);
              await file.read(pos + o, l);
              st.got = Math.min(st.total, st.got + l);
              st.speed = Math.round(((st.got - got0) / Math.max(1, (Date.now() - t0) / 1000)));
              emit(name);
            }
            st.retry = false;
            break;
          } catch (e) {
            if (ac.signal.aborted) return;
            st.retry = true; emit(name); fail++;
            if (attempt >= 5 || fail > 40) throw e;
            await new Promise((r) => setTimeout(r, 1500 * (attempt + 1)));
          }
        }
      }
    };
    await Promise.all(Array.from({ length: PAR }, worker));
    if (ac.signal.aborted) throw new Error('paused');
    await new Promise((r) => setTimeout(r, 1200)); // oxirgi bo'laklar diskka yozilib bo'lsin
    const d = done0(); d[name] = info.size; write(d); setPinned(Object.keys(d));
    Object.assign(st, { run: false, done: true, got: info.size, speed: 0 });
  } catch (e) {
    Object.assign(st, { run: false, retry: false, speed: 0 });
    if (`${e?.message}` !== 'paused') st.error = `${e?.message || e}`;
  } finally {
    try { file?.close(); } catch (_) { /* */ }
    jobs.delete(name);
    emit(name);
  }
}

/** Yuklashni boshlaydi (navbatga qo'yadi). */
export function dlStart(name) {
  if (!name || jobs.has(name) || queue.includes(name) || dlComplete(name)) return;
  const st = stats.get(name) || { total: 0, got: 0, run: false, queued: false, retry: false, speed: 0, done: false };
  st.queued = true; st.error = ''; stats.set(name, st); emit(name);
  queue.push(name);
  startNext();
}

export function dlPause(name) {
  const i = queue.indexOf(name);
  if (i >= 0) { queue.splice(i, 1); const st = stats.get(name); if (st) st.queued = false; emit(name); }
  jobs.get(name)?.abort();
}

/** Yuklangan nusxani o'chiradi (kesh bo'laklari ham). */
export async function dlRemove(name) {
  dlPause(name);
  const d = done0(); delete d[name]; write(d); setPinned(Object.keys(d));
  stats.delete(name);
  await cacheDeletePrefix(dlKey(name));
  emit(name);
}

/** Qisman yuklangan (to'liq emas) nusxa uchun keshdagi hajm: sahifa ochilganda holatni tiklash. */
export async function dlRefresh(name, total = 0) {
  if (dlComplete(name) || jobs.has(name)) return dlStat(name);
  const { bytes } = await cacheCountPrefix(dlKey(name));
  const st = stats.get(name) || { total, got: 0, run: false, queued: false, retry: false, speed: 0, done: false };
  st.total = total || st.total; st.got = Math.min(st.total || bytes, bytes);
  stats.set(name, st); emit(name);
  return st;
}
