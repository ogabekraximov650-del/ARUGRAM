// Bosh sahifa — `lib/screens/home_screen.dart` (onlayn qismi).
//
// Oflaynga xos qismlar ("Offline" belgisi, "Keshdan ko'rsatilmoqda",
// yuklab olinganlar filtri) ATAYLAB yo'q: Mini App faqat onlayn.

import { api, imageUrl } from './api.js';
import { esc, formatCompact, formatCount, toInt } from './format.js';

const CACHE_KEY = 'aru_seasons_v1';

function readCache() {
  try {
    const raw = localStorage.getItem(CACHE_KEY);
    const v = raw ? JSON.parse(raw) : null;
    return Array.isArray(v) ? v : null;
  } catch (_) { return null; }
}

function writeCache(list) {
  try { localStorage.setItem(CACHE_KEY, JSON.stringify(list)); } catch (_) { /* */ }
}

/** `seasonIsPaid` — server `free` maydonini beradi. */
const isPaid = (s) => s.free === false;

function ageColor(yosh) {
  // 18+ qizil, 16+ to'q sariq, qolgani oltin (alpha 0.92).
  if (yosh >= 18) return 'rgba(229,72,77,0.92)';
  if (yosh >= 16) return 'rgba(226,98,15,0.92)';
  return 'rgba(255,201,60,0.92)';
}

const paidBadge = () =>
  '<span class="paid-badge"><span class="ic fill">workspace_premium</span><span class="t">Pullik</span></span>';

/** `SeasonCard` — kartochka HTML'i. `w` — kartochka eni (px). */
export function seasonCardHtml(s, w) {
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

  const row1 = `
    <div class="badge-row">
      ${rCount > 0 ? `<span class="card-badge"><span class="ic fill gold">star</span><span class="t">${rating.toFixed(1)}</span></span>` : ''}
      <span class="grow"></span>
      ${views > 0 ? `<span class="card-badge"><span class="ic fill">visibility</span><span class="t">${esc(formatCompact(views))}</span></span>` : ''}
    </div>`;
  const row2 = yosh > 0 || paid ? `
    <div class="badge-row">
      ${yosh > 0 ? `<span class="age-badge" style="background:${ageColor(yosh)}">${yosh}+</span>` : ''}
      <span class="grow"></span>
      ${paid ? paidBadge() : ''}
    </div>` : '';

  return `
    <div class="season-card tap">
      <div class="ph">${photo ? '<div class="spinner"></div>' : '<span class="ic">movie</span>'}</div>
      ${photo ? `<img class="poster" alt="" decoding="async" loading="lazy" src="${esc(photo)}">` : ''}
      <div class="badges">${row1}${row2}</div>
      <div class="card-foot" style="min-height:${textHeight.toFixed(1)}px">
        ${tag ? `<div class="tag" style="font-size:${tagSize.toFixed(2)}px">${esc(tag)}</div>` : ''}
        <div class="name" style="font-size:${nameSize.toFixed(2)}px;-webkit-line-clamp:${lines}">${esc(name)}</div>
      </div>
    </div>`;
}

/** `GlassTappable`: bosilganda 0.95 ga kichrayadi, qo'yib yuborilganda bosiladi. */
export function bindTap(el, onTap) {
  let sx = 0;
  let sy = 0;
  let live = false;
  const up = () => { el.classList.remove('down'); };
  el.addEventListener('pointerdown', (e) => {
    sx = e.clientX; sy = e.clientY; live = true;
    el.classList.add('down');
  });
  el.addEventListener('pointermove', (e) => {
    if (live && (Math.abs(e.clientX - sx) > 10 || Math.abs(e.clientY - sy) > 10)) {
      live = false; up();
    }
  });
  el.addEventListener('pointerup', () => {
    up();
    if (live) { live = false; onTap(); }
  });
  el.addEventListener('pointercancel', () => { live = false; up(); });
  el.addEventListener('pointerleave', () => { live = false; up(); });
}

/** Rasm yuklangach ko'rsatish, xato bo'lsa — belgisi. */
function bindPosters(root) {
  root.querySelectorAll('img.poster').forEach((img) => {
    const ph = img.previousElementSibling;
    const done = () => { img.classList.add('ok'); };
    const fail = () => {
      img.remove();
      if (ph) ph.innerHTML = '<span class="ic">movie</span>';
    };
    if (img.complete && img.naturalWidth > 0) done();
    else {
      img.addEventListener('load', done, { once: true });
      img.addEventListener('error', fail, { once: true });
    }
  });
}

export function createHome(page, { onOpenSeason }) {
  let seasons = readCache() || [];
  let loading = seasons.length === 0;
  let failed = false;

  page.innerHTML = `
    <div class="home-head">
      <div>
        <img class="logo" src="assets/aru-mark.png" alt="ARU">
        <div class="sub">Anime dunyosi</div>
      </div>
      <div class="spacer"></div>
      <div class="bell"><span class="ic">notifications</span></div>
    </div>
    <div class="section-title">Ommabop anime</div>
    <div class="home-body"></div>
    <div class="bottom-space"></div>`;
  const body = page.querySelector('.home-body');

  function cardWidth() {
    const w = page.clientWidth || window.innerWidth;
    return (w - 32 - 14) / 2;
  }

  function render() {
    if (loading) {
      body.innerHTML = '<div class="center-box"><div class="spinner"></div></div>';
      return;
    }
    if (seasons.length === 0) {
      body.innerHTML = `<div class="center-box"><span class="ic">movie</span>
        <div class="msg">${failed ? "Internetni tekshiring" : 'Anime topilmadi'}</div></div>`;
      return;
    }
    const w = cardWidth();
    body.innerHTML = `<div class="grid">${seasons.map((s) => seasonCardHtml(s, w)).join('')}</div>`;
    body.querySelectorAll('.season-card').forEach((el, i) => bindTap(el, () => onOpenSeason(seasons[i])));
    bindPosters(body);
  }

  async function fetchList() {
    try {
      const list = await api('/api/seasons');
      if (Array.isArray(list)) {
        seasons = list;
        writeCache(list);
        failed = false;
      }
    } catch (_) {
      failed = true;
    }
    loading = false;
    render();
  }

  let lastW = 0;
  window.addEventListener('resize', () => {
    const w = Math.round(cardWidth());
    if (w !== lastW) { lastW = w; render(); }
  });

  render();
  fetchList();
  return { refresh: fetchList };
}
