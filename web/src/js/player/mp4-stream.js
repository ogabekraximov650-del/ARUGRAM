// Oddiy MP4 (`url_<sifat>`) ni <video> ga to'g'ridan-to'g'ri beradi — Service Worker orqali
// (`src/sw.js`): brauzer o'zi "Range" so'raydi, baytlarni biz Telegram'dan olib ochamiz
// (`openFile`). Android (Chrome WebView) uchun asosiy yo'l; MSE kerak emas, o'zi bufer qiladi,
// o'zi suradi. Service Worker ishlamasa (iPhone'dagi Telegram va h.k.) — fMP4 + MSE (`engine.js`).
//
//   await mp4Streaming()                       — bu qurilmada ishlaydimi (bir marta tekshiriladi)
//   createMp4Stream(video, { name, startAt, onState, onError }) -> engine bilan bir xil interfeys

import { openFile } from '../tg/media.js';

let swState = null; // Promise<boolean>
const sources = new Map(); // id -> {file, size}
let wired = false;

function wire() {
  if (wired) return;
  wired = true;
  navigator.serviceWorker.addEventListener('message', async (ev) => {
    const d = ev.data || {};
    if (d.t !== 'aru-stat' && d.t !== 'aru-read') return;
    const port = ev.ports?.[0];
    const src = sources.get(d.id);
    try {
      if (!src) throw new Error('no_source');
      if (d.t === 'aru-stat') { port.postMessage({ size: src.file.size }); return; }
      const buf = await src.file.read(d.offset, d.length);
      // Keyingi bo'laklar oldindan (parallel) — qotmasin.
      src.file.prefetch(d.offset + d.length, 6 * 1024 * 1024);
      const ab = buf.byteOffset === 0 && buf.byteLength === buf.buffer.byteLength ? buf.buffer : buf.slice().buffer;
      port.postMessage({ buf: ab }, [ab]);
    } catch (e) { port?.postMessage({ error: `${e?.message || e}` }); }
  });
}

/** Service Worker ro'yxatdan o'tib, sahifani boshqarayaptimi (bir marta). */
export function mp4Streaming() {
  if (swState) return swState;
  swState = (async () => {
    try {
      if (!('serviceWorker' in navigator) || !window.isSecureContext) return false;
      await navigator.serviceWorker.register('sw.js', { scope: './' });
      await Promise.race([navigator.serviceWorker.ready, new Promise((_, j) => setTimeout(() => j(new Error('sw_timeout')), 4000))]);
      if (!navigator.serviceWorker.controller) {
        await Promise.race([
          new Promise((res) => navigator.serviceWorker.addEventListener('controllerchange', res, { once: true })),
          new Promise((res) => setTimeout(res, 2500)),
        ]);
      }
      if (!navigator.serviceWorker.controller) return false;
      wire();
      return true;
    } catch (_) { return false; }
  })();
  return swState;
}

let seq = 0;
export function createMp4Stream(video, { name, startAt = 0, onState = () => {}, onError = () => {} }) {
  let destroyed = false;
  const id = `${Date.now().toString(36)}${(seq++).toString(36)}`;
  let file = null;
  const st = (s) => { if (!destroyed) onState({ state: s }); };
  const onWaiting = () => st('buffering');
  const onPlaying = () => st('playing');
  const onErr = () => {
    const c = video.error;
    // Manba (SW/Telegram) xatosi — pleyer qayta urinadi (player-screen: MEDIA_ERR).
    if (!destroyed) onError(new Error(`MEDIA_ERR_${c?.code ?? 0} ${c?.message || ''}`));
  };
  const ready = (async () => {
    st('loading');
    file = await openFile(name);
    if (destroyed) return;
    sources.set(id, { file });
    // Boshi (moov) va birinchi bo'laklar darhol.
    file.prefetch(0, 4 * 1024 * 1024);
    video.addEventListener('waiting', onWaiting);
    video.addEventListener('playing', onPlaying);
    video.addEventListener('error', onErr);
    video.src = `/__aru/mp4/${id}.mp4`;
    await new Promise((res, rej) => {
      const ok = () => { video.removeEventListener('loadedmetadata', ok); video.removeEventListener('error', bad); res(); };
      const bad = () => { video.removeEventListener('loadedmetadata', ok); video.removeEventListener('error', bad); rej(new Error('mp4_open_failed')); };
      video.addEventListener('loadedmetadata', ok);
      video.addEventListener('error', bad);
    });
    if (startAt > 0) { try { video.currentTime = Math.max(0, Math.min(startAt, Math.max(0, (video.duration || startAt + 1) - 1))); } catch (_) { /* */ } }
  })();
  ready.catch((e) => { if (!destroyed) onError(e); });
  return {
    ready,
    get duration() { return video.duration || 0; },
    get holding() { return false; },
    cancelHold() {},
    health() { return { kbps: 0, needKbps: 0, stalls: 0 }; },
    startAt(sec) { ready.then(() => { video.currentTime = Math.max(0, sec); }); },
    setAhead() {},
    destroy() {
      destroyed = true;
      video.removeEventListener('waiting', onWaiting);
      video.removeEventListener('playing', onPlaying);
      video.removeEventListener('error', onErr);
      try { video.pause(); } catch (_) { /* */ }
      try { video.removeAttribute('src'); video.load(); } catch (_) { /* */ }
      sources.delete(id);
      try { file?.close(); } catch (_) { /* */ }
    },
  };
}
