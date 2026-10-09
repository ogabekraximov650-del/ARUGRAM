// Umumiy UI bo'laklari — ilovadagi (Flutter) vidjetlarning web nusxasi.
//
//   icon('star', {fill:true, size:12, color:'#FFC93C'})  -> Icons.*_rounded
//   spinner(size, stroke, color)                         -> CircularProgressIndicator
//   bindTap(el, fn)                                      -> GlassTappable (0.95 ga kichrayadi)
//   ripple(el)                                           -> InkWell to'lqini
//   toast(text)                                          -> SnackBar
//   dialog({...}) / confirmDialog(...)                   -> showDialog + Glass
//   sheet(builder)                                       -> showModalBottomSheet
//   appBar({title, back, actions})                       -> ilovadagi sahifa sarlavhasi
//   seasonCardHtml(season, width) + bindSeasonCards(...) -> SeasonCard
//
// Ranglar `css/app.css` dagi o'zgaruvchilar (AppColors).

import { imageUrl } from './api.js';
import { esc, formatCompact, formatCount, toInt } from './format.js';
import { back as routerBack } from './router.js';

export const C = {
  bg: '#0A0A0C', surface: '#101012', card: '#151517', cardAlt: '#1B1B1E',
  accent: '#C2410C', accent2: '#E2620F', accent3: '#FFC93C', accentTint: '#281914',
  text: '#F2F2F3', textDim: '#9A9AA0', textFaint: '#6B6B70',
  success: '#4ADE80', gold: '#FFC93C', danger: '#E5484D', telegram: '#229ED9',
};

/** Material ikonka. `fill: true` — Flutter'ning `*_rounded` (to'la) shakli. */
export function icon(name, { fill = true, size = 24, color = '', cls = '', style = '' } = {}) {
  const st = `font-size:${size}px;width:${size}px;height:${size}px;${color ? `color:${color};` : ''}${style}`;
  return `<span class="ic${fill ? ' fill' : ''}${cls ? ' ' + cls : ''}" style="${st}">${name}</span>`;
}

/** Aylanuvchi yuklanish belgisi. */
export function spinner(size = 36, stroke = 2, color = C.accent) {
  return `<div class="spinner" style="width:${size}px;height:${size}px;border-width:${stroke}px;border-top-color:${color};border-right-color:${color}"></div>`;
}

/** `GlassTappable`: bosilganda 0.95, qo'yib yuborilganda bosiladi (surish emas). */
export function bindTap(el, onTap, { scale = true } = {}) {
  if (!el) return;
  if (scale) el.classList.add('tap');
  let sx = 0; let sy = 0; let live = false;
  const up = () => el.classList.remove('down');
  el.addEventListener('pointerdown', (e) => {
    sx = e.clientX; sy = e.clientY; live = true;
    if (scale) el.classList.add('down');
  });
  el.addEventListener('pointermove', (e) => {
    if (live && (Math.abs(e.clientX - sx) > 10 || Math.abs(e.clientY - sy) > 10)) { live = false; up(); }
  });
  el.addEventListener('pointerup', (e) => { up(); if (live) { live = false; onTap(e); } });
  el.addEventListener('pointercancel', () => { live = false; up(); });
  el.addEventListener('pointerleave', () => { live = false; up(); });
}

/** InkWell to'lqini (bosilgan joydan yoyiladi). `el` ga `position:relative;overflow:hidden` kerak. */
export function ripple(el, onTap) {
  if (!el) return;
  el.classList.add('ink');
  el.addEventListener('pointerdown', (e) => {
    const r = el.getBoundingClientRect();
    const d = Math.max(r.width, r.height) * 2;
    const s = document.createElement('span');
    s.className = 'ink-wave';
    s.style.width = s.style.height = `${d}px`;
    s.style.left = `${e.clientX - r.left - d / 2}px`;
    s.style.top = `${e.clientY - r.top - d / 2}px`;
    el.appendChild(s);
    setTimeout(() => s.remove(), 600);
  });
  if (onTap) {
    let sx = 0; let sy = 0;
    el.addEventListener('pointerdown', (e) => { sx = e.clientX; sy = e.clientY; });
    el.addEventListener('click', (e) => {
      if (Math.abs(e.clientX - sx) > 10 || Math.abs(e.clientY - sy) > 10) return;
      onTap(e);
    });
  }
}

let toastTimer = 0;
/** SnackBar o'rnida (pastda, panel ustida). */
export function toast(text, ms = 2600) {
  const app = document.getElementById('app');
  let el = app.querySelector(':scope > .toast');
  if (!el) {
    el = document.createElement('div');
    el.className = 'toast';
    app.appendChild(el);
  }
  el.textContent = text;
  requestAnimationFrame(() => el.classList.add('show'));
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => el.classList.remove('show'), ms);
}

/**
 * Dialog (`showDialog` + `Glass`). `content` — HTML yoki (el)=>void.
 * `actions`: [{text, primary?, danger?, value}] — bosilgan tugma `value` si
 * bilan Promise yechiladi; fon bosilsa — `null`.
 */
export function dialog({ title = '', content = '', actions = [], dismissible = true, cls = '' } = {}) {
  return new Promise((resolve) => {
    const app = document.getElementById('app');
    const wrap = document.createElement('div');
    wrap.className = 'dlg-wrap';
    wrap.innerHTML = `<div class="dlg ${cls}">
      ${title ? `<div class="dlg-title">${title}</div>` : ''}
      <div class="dlg-body"></div>
      ${actions.length ? '<div class="dlg-actions"></div>' : ''}
    </div>`;
    const body = wrap.querySelector('.dlg-body');
    if (typeof content === 'function') content(body, close); else body.innerHTML = content;
    const acts = wrap.querySelector('.dlg-actions');
    actions.forEach((a) => {
      const b = document.createElement('button');
      b.className = `btn ${a.primary ? 'btn-filled' : 'btn-text'}${a.danger ? ' danger' : ''}`;
      b.textContent = a.text;
      b.addEventListener('click', () => close(a.value ?? a.text));
      acts.appendChild(b);
    });
    function close(v) {
      wrap.classList.remove('in');
      setTimeout(() => wrap.remove(), 180);
      resolve(v);
    }
    wrap.addEventListener('click', (e) => { if (e.target === wrap && dismissible) close(null); });
    app.appendChild(wrap);
    requestAnimationFrame(() => wrap.classList.add('in'));
  });
}

export function confirmDialog(title, text, { ok = 'Ha', cancel = 'Bekor qilish', danger = false } = {}) {
  return dialog({
    title,
    content: `<div class="dlg-text">${text}</div>`,
    actions: [{ text: cancel, value: false }, { text: ok, value: true, primary: !danger, danger }],
  }).then((v) => v === true);
}

/** Matn kiritish dialogi. */
export function promptDialog(title, { value = '', placeholder = '', ok = 'Saqlash', type = 'text', maxLength = 0 } = {}) {
  let input;
  return dialog({
    title,
    content: (el) => {
      el.innerHTML = `<input class="field" type="${type}" placeholder="${esc(placeholder)}" value="${esc(value)}" ${maxLength ? `maxlength="${maxLength}"` : ''}>`;
      input = el.querySelector('input');
      setTimeout(() => input.focus(), 50);
    },
    actions: [{ text: 'Bekor qilish', value: null }, { text: ok, value: '__ok', primary: true }],
  }).then((v) => (v === '__ok' ? input.value : null));
}

/**
 * Pastdan chiqadigan oyna (`showModalBottomSheet`). `builder(el, close)`.
 * Promise `close(v)` qiymati bilan yechiladi.
 */
export function sheet(builder, { cls = '' } = {}) {
  return new Promise((resolve) => {
    const app = document.getElementById('app');
    const wrap = document.createElement('div');
    wrap.className = 'sheet-wrap';
    wrap.innerHTML = `<div class="sheet ${cls}"><div class="sheet-handle"></div><div class="sheet-body"></div></div>`;
    const body = wrap.querySelector('.sheet-body');
    function close(v) {
      wrap.classList.remove('in');
      setTimeout(() => wrap.remove(), 220);
      resolve(v ?? null);
    }
    wrap.addEventListener('click', (e) => { if (e.target === wrap) close(null); });
    builder(body, close);
    app.appendChild(wrap);
    requestAnimationFrame(() => wrap.classList.add('in'));
  });
}

/**
 * Ichki sahifa sarlavhasi (orqaga tugmasi + nom + o'ng tomonda tugmalar).
 * `actions`: HTML. Qaytaradi: HTML; orqaga tugmasini `bindAppBar(el)` ulaydi.
 */
export function appBar({ title = '', sub = '', actions = '', backBtn = true } = {}) {
  return `<div class="appbar">
    ${backBtn ? `<button class="icon-btn appbar-back" aria-label="Orqaga">${icon('arrow_back', { size: 22 })}</button>` : ''}
    <div class="appbar-title"><div class="t">${title}</div>${sub ? `<div class="s">${sub}</div>` : ''}</div>
    <div class="appbar-actions">${actions}</div>
  </div>`;
}

export function bindAppBar(el, onBack = () => routerBack()) {
  el.querySelector('.appbar-back')?.addEventListener('click', onBack);
}

/** Glass karta HTML (`Glass` vidjeti). */
export function glass(inner, { radius = 22, pad = '', cls = '', style = '' } = {}) {
  return `<div class="glass ${cls}" style="border-radius:${radius}px;${pad ? `padding:${pad};` : ''}${style}">${inner}</div>`;
}

// ── Bo'lim kartochkasi (`SeasonCard`, `home_screen.dart`) ──────────

function ageColor(yosh) {
  if (yosh >= 18) return 'rgba(229,72,77,0.92)';
  if (yosh >= 16) return 'rgba(226,98,15,0.92)';
  return 'rgba(255,201,60,0.92)';
}

export const isPaid = (s) => s && s.free === false;

export const paidBadge = (compact = false) => (compact
  ? `<span class="paid-badge compact">${icon('workspace_premium', { size: 13, color: C.gold })}</span>`
  : `<span class="paid-badge">${icon('workspace_premium', { size: 12, color: C.gold })}<span class="t">Pullik</span></span>`);

export function ageBadge(yosh) {
  return `<span class="age-badge" style="background:${ageColor(yosh)}">${yosh}+</span>`;
}

/**
 * `w` — kartochka eni (px). `opts.badges` (rasm ustidagi belgilar),
 * `opts.corner` (o'ng yuqoridagi qo'shimcha HTML).
 */
export function seasonCardHtml(s, w, { badges = true, corner = '' } = {}) {
  const photo = imageUrl(s.photo_url);
  const name = `${s.nomi ?? ''}`;
  const bolim = toInt(s.bolim_id);
  const eps = toInt(s.epizod_count);
  const rCount = toInt(s.rating_count);
  const rSum = toInt(s.rating_sum);
  const rating = rCount > 0 ? rSum / rCount : 0;
  const views = toInt(s.views_total);
  const yosh = toInt(s.yosh);
  const paid = isPaid(s);
  const nameSize = Math.min(18, Math.max(12.5, w * 0.085));
  const tagSize = Math.min(13.5, Math.max(10, nameSize * 0.78));
  const lines = name.length > 26 && w >= 150 ? 3 : 2;
  const textHeight = nameSize * 1.22 * lines + tagSize * 1.3 + 16;
  const tag = [bolim > 0 ? `${bolim}-bo'lim` : '', eps > 0 ? `${formatCount(eps)} ta qism` : '']
    .filter(Boolean).join(' · ');
  let top = '';
  if (badges) {
    const row1 = `<div class="badge-row">
      ${rCount > 0 ? `<span class="card-badge">${icon('star', { size: 12, color: C.gold })}<span class="t">${rating.toFixed(1)}</span></span>` : ''}
      <span class="grow"></span>
      ${views > 0 ? `<span class="card-badge">${icon('visibility', { size: 12, color: 'rgba(255,255,255,0.7)' })}<span class="t">${esc(formatCompact(views))}</span></span>` : ''}
    </div>`;
    const row2 = yosh > 0 || paid ? `<div class="badge-row">
      ${yosh > 0 ? ageBadge(yosh) : ''}<span class="grow"></span>${paid ? paidBadge() : ''}</div>` : '';
    top = `<div class="badges">${row1}${row2}</div>`;
  } else if (paid) {
    top = `<div class="badges">${paidBadge()}</div>`;
  }
  return `
    <div class="season-card">
      <div class="ph">${photo ? spinner(36, 2) : icon('movie', { fill: false, size: 36, color: 'rgba(255,255,255,0.38)' })}</div>
      ${photo ? `<img class="poster" alt="" decoding="async" loading="lazy" src="${esc(photo)}">` : ''}
      ${top}
      ${corner ? `<div class="card-corner">${corner}</div>` : ''}
      <div class="card-foot" style="min-height:${textHeight.toFixed(1)}px">
        ${tag ? `<div class="tag" style="font-size:${tagSize.toFixed(2)}px">${esc(tag)}</div>` : ''}
        <div class="name" style="font-size:${nameSize.toFixed(2)}px;-webkit-line-clamp:${lines}">${esc(name)}</div>
      </div>
    </div>`;
}

/** Rasmlar yuklanishini kuzatadi (yuklangach ko'rsatish, xato — belgi). */
export function bindImages(root, sel = 'img.poster') {
  root.querySelectorAll(sel).forEach((img) => {
    const ph = img.previousElementSibling;
    const done = () => img.classList.add('ok');
    const fail = () => {
      img.remove();
      if (ph) ph.innerHTML = icon('movie', { fill: false, size: 36, color: 'rgba(255,255,255,0.38)' });
    };
    if (img.complete && img.naturalWidth > 0) done();
    else {
      img.addEventListener('load', done, { once: true });
      img.addEventListener('error', fail, { once: true });
    }
  });
}

/** Grid'dagi kartochkalarni bosiladigan qiladi. */
export function bindSeasonCards(root, seasons, onOpen) {
  root.querySelectorAll('.season-card').forEach((el, i) => bindTap(el, () => onOpen(seasons[i])));
  bindImages(root);
}

/** Ikki ustunli grid uchun kartochka eni. */
export function cardWidth(containerWidth, pad = 16, gap = 14) {
  const w = containerWidth || window.innerWidth;
  return (w - pad * 2 - gap) / 2;
}

/** Bo'sh holat (ikonka + matn) — `Glass` ichida, ilovadagidek. */
export function emptyGlass(ic, text, sub = '') {
  return `<div class="empty-glass">${glass(`
    ${icon(ic, { fill: false, size: 46, color: 'rgba(255,255,255,0.54)' })}
    <div class="eg-t">${text}</div>${sub ? `<div class="eg-s">${sub}</div>` : ''}`, { radius: 20, pad: '24px' })}</div>`;
}

/** Avatar (rasm bo'lmasa — ism harfi, ilovadagidek gradient). */
export function avatarHtml(url, name, size = 40) {
  const u = imageUrl(url);
  const letter = esc((`${name || '?'}`.trim()[0] || '?').toUpperCase());
  return `<span class="avatar" style="width:${size}px;height:${size}px;font-size:${Math.round(size * 0.42)}px">
    ${u ? `<img src="${esc(u)}" alt="" onerror="this.remove()">` : ''}<span class="av-l">${letter}</span></span>`;
}

/** Ko'p qatorli matnni HTML qilib (yangi qatorlar bilan). */
export function multiline(s) { return esc(s).replace(/\n/g, '<br>'); }

/** Telegram ilovasi ichida havolani ochish. */
export function openLink(url) {
  const tg = window.Telegram?.WebApp;
  try {
    if (/^https:\/\/t\.me\//.test(url) && tg?.openTelegramLink) { tg.openTelegramLink(url); return; }
    if (tg?.openLink) { tg.openLink(url); return; }
  } catch (_) { /* */ }
  window.open(url, '_blank');
}

/** Telegram tebranishi (haptic). */
export function haptic(kind = 'light') {
  try { window.Telegram?.WebApp?.HapticFeedback?.impactOccurred(kind); } catch (_) { /* */ }
}
