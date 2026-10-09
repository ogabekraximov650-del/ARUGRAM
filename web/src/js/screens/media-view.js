// Yozishmadagi rasm/video — `lib/screens/media_view_screen.dart`.
//
// Bu ASOSIY PLEYER EMAS: videoda faqat play/pause va pastda progress
// chizig'i, rasmda esa barmoq bilan kattalashtirish (1..4x,
// `InteractiveViewer`). Fayl worker'dan EMAS, Telegram orqali keladi
// (`tg/media.js` -> `mediaUrl`).
//
//   openMediaView({ url, type })  — `url`: xabardagi `media_url` (yoki
//                                   fayl nomi), `type`: 'image' | 'video'.
//
// Shu yerda umumiy yordamchi ham bor (support chat ishlatadi):
//   loadMedia(name) -> Promise<objectURL>  (bir fayl bir marta yuklanadi)
//   isTgNotReady(e), tgRetry() — Telegram'ga hali kirilmagan holat.

import * as tgMedia from '../tg/media.js';
import { push } from '../router.js';
import { icon, spinner, bindAppBar } from '../ui.js';

const cache = new Map(); // nom -> Promise<objectURL>

/** Telegram'ga hali kirilmagan (`tg_not_ready`). */
export function isTgNotReady(e) {
  const m = `${e?.message ?? e ?? ''}`;
  return m.includes('tg_not_ready');
}

/** Faylni Telegram'dan oladi (ochilgan, deshifrlangan `blob:` manzil). */
export function loadMedia(name, opts = {}) {
  const key = `${name}`;
  let p = cache.get(key);
  if (!p) {
    p = Promise.resolve().then(() => tgMedia.mediaUrl(key, opts));
    cache.set(key, p);
    // Xato — keyingi urinishda qaytadan.
    p.catch(() => { if (cache.get(key) === p) cache.delete(key); });
  }
  return p;
}

/** Telegram'ga kirishni so'raydi (bosilganda). `true` — tayyor. */
export async function tgRetry() {
  try { return (await tgMedia.ensureTelegram?.()) !== false; } catch (_) { return false; }
}

function nameOf(url) {
  const last = `${url ?? ''}`.split('?')[0].split('/').pop() || '';
  return /^[A-Za-z0-9._-]+$/.test(last) ? last : '';
}

function clock(sec) {
  const s = Math.max(0, Math.floor(sec || 0));
  const two = (n) => String(n).padStart(2, '0');
  const h = Math.floor(s / 3600);
  const m = Math.floor(s / 60);
  if (h > 0) return `${h}:${two(m % 60)}:${two(s % 60)}`;
  return `${two(m)}:${two(s % 60)}`;
}

export function openMediaView({ url = '', type = 'image', name = '' } = {}) {
  push((el) => build(el, name || nameOf(url), type));
}

function build(el, name, type) {
  const isVideo = type === 'video';
  el.classList.add('mv');
  el.innerHTML = `
    <div class="mv-body"></div>
    <div class="appbar mv-bar"><button class="icon-btn appbar-back" aria-label="Orqaga">${icon('arrow_back', { size: 24 })}</button></div>`;
  bindAppBar(el);
  const body = el.querySelector('.mv-body');
  let closed = false;
  let video = null;
  let cleanup = () => {};

  function loading() {
    body.innerHTML = `<div class="mv-center">${spinner(34, 2, 'rgba(255,255,255,0.54)')}</div>`;
  }

  function notReady(retry) {
    body.innerHTML = `<div class="mv-center"><button class="mv-load">${icon('download', { size: 30, color: '#fff' })}</button></div>`;
    body.querySelector('.mv-load').addEventListener('click', async () => {
      loading();
      await tgRetry();
      if (!closed) retry();
    });
  }

  // ── RASM ──
  async function openImage() {
    loading();
    let src;
    try {
      src = await loadMedia(name);
    } catch (e) {
      if (closed) return;
      if (isTgNotReady(e)) { notReady(openImage); return; }
      body.innerHTML = `<div class="mv-center">${icon('broken_image', { fill: false, size: 54, color: 'rgba(255,255,255,0.3)' })}</div>`;
      return;
    }
    if (closed) return;
    body.innerHTML = '<div class="mv-zoom"><img alt="" draggable="false"></div>';
    const box = body.querySelector('.mv-zoom');
    const img = box.querySelector('img');
    img.onerror = () => { box.innerHTML = `<div class="mv-center">${icon('broken_image', { fill: false, size: 54, color: 'rgba(255,255,255,0.3)' })}</div>`; };
    img.src = src;
    cleanup = bindZoom(box, img);
  }

  // ── VIDEO ──
  async function openVideo() {
    loading();
    let src;
    try {
      src = await loadMedia(name);
    } catch (e) {
      if (closed) return;
      if (isTgNotReady(e)) { notReady(openVideo); return; }
      showError();
      return;
    }
    if (closed) return;
    body.innerHTML = `
      <div class="mv-vid">
        <div class="mv-frame"><video playsinline preload="auto"></video>
          <div class="mv-tap"><div class="mv-pp">${icon('play_arrow', { size: 36, color: '#fff' })}</div></div>
        </div>
      </div>
      <div class="mv-ctl">
        <div class="mv-slider"><div class="tr"><div class="fill"></div></div><div class="th"></div></div>
        <div class="mv-times"><span class="a">00:00</span><span class="b">00:00</span></div>
      </div>`;
    video = body.querySelector('video');
    const pp = body.querySelector('.mv-pp');
    const slider = body.querySelector('.mv-slider');
    const fill = slider.querySelector('.fill');
    const th = slider.querySelector('.th');
    const ta = body.querySelector('.mv-times .a');
    const tb = body.querySelector('.mv-times .b');
    let dragging = false;
    let shownPos = 0;
    const setPlaying = (p) => {
      pp.classList.toggle('hide', p);
      pp.querySelector('.ic').textContent = p ? 'pause' : 'play_arrow';
    };
    const paint = () => {
      const d = video.duration || 0;
      const f = d > 0 ? Math.min(1, Math.max(0, shownPos / d)) : 0;
      fill.style.width = `${f * 100}%`;
      th.style.left = `${f * 100}%`;
      ta.textContent = clock(shownPos);
      tb.textContent = clock(d);
    };
    video.addEventListener('loadedmetadata', paint);
    video.addEventListener('error', () => { if (!closed) showError(); });
    const tick = setInterval(() => {
      if (!dragging) shownPos = video.currentTime;
      setPlaying(!video.paused);
      paint();
    }, 250);
    body.querySelector('.mv-tap').addEventListener('click', () => {
      const was = !video.paused;
      if (was) video.pause(); else video.play().catch(() => {});
      setPlaying(!was);
    });
    // Sek FAQAT qo'yib yuborilganda (surish davomida faqat tutqich).
    const at = (e) => {
      const r = slider.getBoundingClientRect();
      const f = Math.min(1, Math.max(0, (e.clientX - r.left) / r.width));
      return f * (video.duration || 0);
    };
    slider.addEventListener('pointerdown', (e) => {
      dragging = true;
      slider.setPointerCapture(e.pointerId);
      shownPos = at(e); paint();
    });
    slider.addEventListener('pointermove', (e) => { if (dragging) { shownPos = at(e); paint(); } });
    const end = (e) => {
      if (!dragging) return;
      dragging = false;
      shownPos = at(e);
      try { video.currentTime = shownPos; } catch (_) { /* */ }
      paint();
    };
    slider.addEventListener('pointerup', end);
    slider.addEventListener('pointercancel', () => { dragging = false; });
    video.src = src;
    video.volume = 1;
    video.play().then(() => setPlaying(true)).catch(() => setPlaying(false));
    cleanup = () => { clearInterval(tick); try { video.pause(); } catch (_) { /* */ } };
  }

  function showError() {
    cleanup();
    cleanup = () => {};
    body.innerHTML = `<div class="mv-center col">
      <div class="mv-err">Videoni ochib bo'lmadi</div>
      <button class="mv-retry">${icon('refresh', { size: 18 })}<span>Qayta urinish</span></button></div>`;
    body.querySelector('.mv-retry').addEventListener('click', () => openVideo());
  }

  if (isVideo) openVideo(); else openImage();

  return {
    dispose() {
      closed = true;
      cleanup();
      if (video) { try { video.removeAttribute('src'); video.load(); } catch (_) { /* */ } }
    },
  };
}

/** `InteractiveViewer(minScale: 1, maxScale: 4)`: barmoq bilan kattalashtirish va surish. */
function bindZoom(box, img) {
  let scale = 1; let tx = 0; let ty = 0;
  const pts = new Map();
  let start = null;
  const apply = () => { img.style.transform = `translate(${tx}px,${ty}px) scale(${scale})`; };
  const clampPan = () => {
    const r = box.getBoundingClientRect();
    const mx = (r.width * (scale - 1)) / 2;
    const my = (r.height * (scale - 1)) / 2;
    tx = Math.min(mx, Math.max(-mx, tx));
    ty = Math.min(my, Math.max(-my, ty));
  };
  const snapshot = () => {
    const p = [...pts.values()];
    if (p.length >= 2) {
      const d = Math.hypot(p[0].x - p[1].x, p[0].y - p[1].y);
      start = { d, scale, tx, ty, cx: (p[0].x + p[1].x) / 2, cy: (p[0].y + p[1].y) / 2 };
    } else if (p.length === 1) {
      start = { x: p[0].x, y: p[0].y, tx, ty };
    } else start = null;
  };
  box.addEventListener('pointerdown', (e) => { box.setPointerCapture(e.pointerId); pts.set(e.pointerId, { x: e.clientX, y: e.clientY }); snapshot(); });
  box.addEventListener('pointermove', (e) => {
    if (!pts.has(e.pointerId)) return;
    pts.set(e.pointerId, { x: e.clientX, y: e.clientY });
    const p = [...pts.values()];
    if (p.length >= 2 && start?.d) {
      const d = Math.hypot(p[0].x - p[1].x, p[0].y - p[1].y);
      scale = Math.min(4, Math.max(1, start.scale * (d / start.d)));
      const cx = (p[0].x + p[1].x) / 2; const cy = (p[0].y + p[1].y) / 2;
      tx = start.tx + (cx - start.cx); ty = start.ty + (cy - start.cy);
    } else if (p.length === 1 && start && start.x != null && scale > 1) {
      tx = start.tx + (p[0].x - start.x); ty = start.ty + (p[0].y - start.y);
    }
    clampPan(); apply();
  });
  const up = (e) => { pts.delete(e.pointerId); snapshot(); };
  box.addEventListener('pointerup', up);
  box.addEventListener('pointercancel', up);
  box.addEventListener('wheel', (e) => {
    e.preventDefault();
    scale = Math.min(4, Math.max(1, scale * (e.deltaY < 0 ? 1.1 : 0.9)));
    clampPan(); apply();
  }, { passive: false });
  return () => {};
}
