// Katalog — `lib/screens/catalog_screen.dart`.
//
// Bosh sahifa bilan BITTA ro'yxat (`seasonsRepo`) — Katalogga o'tish
// qo'shimcha so'rov emas. Saralash va filtrlash telefonda bajariladi.
//
// Tepada logotip + "Katalog", tagida qo'lda suriladigan tugmalar
// qatori, ostida barmoq bilan surib o'tkaziladigan sahifalar
// (`PageView`). "Filtrlash" tugmasi (`CatalogFilterBar`) pastki panelga
// 10 px yopishib turadi va FAQAT Katalog ochiq bo'lganda ko'rinadi.
//
// Oflaynga xos qismlar ("Offline" belgisi, yuklab olinganlar filtri)
// ATAYLAB yo'q: Mini App faqat onlayn.

import { icon, spinner, seasonCardHtml, bindSeasonCards, cardWidth, ripple, sheet } from '../ui.js';
import { esc, toInt } from '../format.js';
import { seasonsRepo } from '../seasons.js';
import { hooks } from '../hooks.js';

const TABS = [
  { key: 'hammasi', label: 'Barchasi' },
  { key: 'reyting', label: 'Reytingi baland' },
  { key: 'korilgan', label: "Eng ko'p ko'rilgan" },
  { key: 'ongoing', label: 'Ongoing' },
  { key: 'tugallangan', label: 'Tugallangan' },
  { key: 'filmlar', label: 'Filmlar' },
  { key: 'ova', label: 'OVA qismlar' },
];

const rating = (s) => {
  const c = toInt(s.rating_count);
  return c <= 0 ? 0 : toInt(s.rating_sum) / c;
};
const genresOf = (s) => `${s.janri ?? ''}`.split(',').map((e) => e.trim()).filter(Boolean);
const trimStr = (v) => `${v ?? ''}`.trim();

/** `CatalogFilter` — janrlar orasida YOKI, yillar orasida YOKI, ular orasida VA. */
function makeFilter(genres = [], years = []) {
  const g = new Set(genres);
  const y = new Set(years);
  return {
    genres: g,
    years: y,
    get isEmpty() { return g.size === 0 && y.size === 0; },
    get count() { return g.size + y.size; },
    allows(s) {
      if (g.size && !genresOf(s).some((x) => g.has(x))) return false;
      if (y.size && !y.has(trimStr(s.yili))) return false;
      return true;
    },
  };
}

function listFor(key, filter) {
  const all = seasonsRepo.items;
  const rows = filter.isEmpty ? all.slice() : all.filter((s) => filter.allows(s));
  switch (key) {
    case 'reyting':
      return rows.filter((s) => rating(s) > 0).sort((a, b) => rating(b) - rating(a));
    case 'korilgan':
      return rows.filter((s) => toInt(s.views_total) > 0)
        .sort((a, b) => toInt(b.views_total) - toInt(a.views_total));
    case 'ongoing':
      return rows.filter((s) => trimStr(s.holati) === 'Davom etmoqda');
    case 'tugallangan':
      return rows.filter((s) => trimStr(s.holati) === 'Tugallangan');
    case 'filmlar':
      return rows.filter((s) => trimStr(s.turi).toUpperCase() === 'FILM');
    case 'ova':
      return rows.filter((s) => trimStr(s.turi).toUpperCase() === 'OVA');
    default:
      return rows;
  }
}

const easeOutCubic = (t) => 1 - (1 - t) ** 3;

/** `el.scrollLeft` ni `to` ga silliq suradi (`animateTo`). */
function animateScroll(el, to, ms, onDone) {
  const from = el.scrollLeft;
  const t0 = performance.now();
  let raf = 0;
  const step = (now) => {
    const t = Math.min(1, (now - t0) / ms);
    el.scrollLeft = from + (to - from) * easeOutCubic(t);
    if (t < 1) raf = requestAnimationFrame(step);
    else onDone?.();
  };
  raf = requestAnimationFrame(step);
  return () => cancelAnimationFrame(raf);
}

// ── Filtr oynasi (`_FilterSheet`) ─────────────────────────────

function chipsHtml(values, selected, kind) {
  return `<div class="cat-chips">${values.map((v, i) => `<div class="cat-chip${selected.has(v) ? ' sel' : ''}" data-k="${kind}" data-i="${i}">${esc(v)}</div>`).join('')}</div>`;
}

function openFilterSheet(current) {
  const genres = seasonsRepo.genres;
  const years = seasonsRepo.years;
  const selG = new Set(current.genres);
  const selY = new Set(current.years);
  return sheet((el, close) => {
    el.innerHTML = `<div class="cat-fs glass">
      <div class="cat-fs-handle"></div>
      <div class="cat-fs-title">Filtrlash</div>
      <div class="cat-fs-scroll">
        ${genres.length ? `<div class="cat-fs-sec">Janrlar</div>${chipsHtml(genres, selG, 'g')}<div style="height:18px"></div>` : ''}
        ${years.length ? `<div class="cat-fs-sec">Yillar</div>${chipsHtml(years, selY, 'y')}` : ''}
        ${!genres.length && !years.length ? '<div class="cat-fs-none">Hozircha filtrlaydigan narsa yo\'q.</div>' : ''}
      </div>
      <div class="cat-fs-btns">
        <button class="btn cat-fs-clear">Tozalash</button>
        <button class="btn cat-fs-apply">Filtrlash</button>
      </div>
    </div>`;
    el.querySelectorAll('.cat-chip').forEach((c) => {
      c.addEventListener('click', () => {
        const isG = c.dataset.k === 'g';
        const set = isG ? selG : selY;
        const v = (isG ? genres : years)[+c.dataset.i];
        if (!set.delete(v)) set.add(v);
        c.classList.toggle('sel', set.has(v));
      });
    });
    const clr = el.querySelector('.cat-fs-clear');
    const app = el.querySelector('.cat-fs-apply');
    ripple(clr, () => close(makeFilter()));
    ripple(app, () => close(makeFilter([...selG], [...selY])));
  }, { cls: 'cat-fsheet' });
}

// ── "Filtrlash" tugmasi (`CatalogFilterBar` + `_FilterButton`) ─

function createFilterBar(onOpen, onClear) {
  const app = document.getElementById('app');
  const bar = document.createElement('div');
  bar.className = 'cat-fbar';
  app.appendChild(bar);
  let count = 0;

  function paint() {
    const on = count > 0;
    bar.innerHTML = `<div class="cat-fbtn${on ? ' on' : ''}">
      <div class="cat-fbtn-main">${icon('tune', { size: 18, color: '#fff' })}<span class="t">${on ? `Filtrlash · ${count}` : 'Filtrlash'}</span></div>
      ${on ? `<div class="cat-fbtn-x">${icon('close', { size: 19, color: '#fff' })}</div>` : ''}
    </div>`;
    ripple(bar.querySelector('.cat-fbtn-main'), () => onOpen());
    const x = bar.querySelector('.cat-fbtn-x');
    if (x) ripple(x, () => onClear());
  }

  return {
    setCount(c) { count = c; paint(); },
    show() { bar.classList.add('show'); },
    hide() { bar.classList.remove('show'); },
  };
}

// ── Ekran ─────────────────────────────────────────────────────

export function createCatalog(page) {
  page.style.overflow = 'hidden';
  page.innerHTML = `
    <div class="cat-root">
      <div class="home-head">
        <div>
          <img class="logo" src="assets/aru-mark.png" alt="ARU">
          <div class="sub">Katalog</div>
        </div>
      </div>
      <div class="cat-tabs">${TABS.map((t, i) => `<div class="cat-tab${i === 0 ? ' sel' : ''}" data-i="${i}">${esc(t.label)}</div>`).join('')}</div>
      <div class="cat-pager">${TABS.map(() => `<div class="cat-pane"><div class="cat-ptr"><div class="cat-ptr-c">${spinner(20, 2.5)}</div></div><div class="cat-pane-body"></div></div>`).join('')}</div>
    </div>`;

  const tabsEl = page.querySelector('.cat-tabs');
  const tabEls = [...tabsEl.querySelectorAll('.cat-tab')];
  const pager = page.querySelector('.cat-pager');
  const panes = [...pager.querySelectorAll('.cat-pane')];
  const bodies = panes.map((p) => p.querySelector('.cat-pane-body'));

  let index = 0;
  let filter = makeFilter();
  let dirty = TABS.map(() => true);
  let stopPagerAnim = null;
  let stopTabAnim = null;

  // ── bitta tabning ro'yxati (`_Grid`) ──
  function renderPane(i) {
    dirty[i] = false;
    const body = bodies[i];
    const rows = listFor(TABS[i].key, filter);
    if (seasonsRepo.loading && rows.length === 0) {
      body.innerHTML = `<div class="cat-loading">${spinner(36, 2)}</div>`;
      return;
    }
    if (rows.length === 0) {
      const f = !filter.isEmpty;
      body.innerHTML = `<div class="cat-empty">
        ${icon(f ? 'filter_alt_off' : 'movie', { fill: f, size: 52, color: 'rgba(255,255,255,0.2)' })}
        <div class="msg">${f ? 'Bu filtrga mos anime topilmadi' : "Bu bo'limda hozircha anime yo'q"}</div>
      </div>`;
      return;
    }
    const w = cardWidth(page.clientWidth);
    body.innerHTML = `<div class="cat-grid">${rows.map((s) => seasonCardHtml(s, w)).join('')}</div><div class="cat-bottom-space"></div>`;
    bindSeasonCards(body, rows, (s) => hooks.openSeason(s));
  }

  /** Ko'rinib turgan (va qo'shni) sahifalarni chizadi. */
  function renderNear() {
    const pos = pager.clientWidth ? pager.scrollLeft / pager.clientWidth : index;
    const lo = Math.floor(pos) - 1;
    const hi = Math.ceil(pos) + 1;
    for (let i = Math.max(0, lo); i <= Math.min(TABS.length - 1, hi); i++) {
      if (dirty[i]) renderPane(i);
    }
  }

  function invalidate() {
    dirty = TABS.map(() => true);
    renderNear();
  }

  // ── tepadagi tugmalar ──
  function paintTabs() {
    tabEls.forEach((el, i) => el.classList.toggle('sel', i === index));
  }

  /** Tanlangan tugmani o'rtaga suradi (`ensureVisible`, alignment 0.5). */
  function revealTab(i) {
    const el = tabEls[i];
    if (!el) return;
    const max = tabsEl.scrollWidth - tabsEl.clientWidth;
    let to = el.offsetLeft + el.offsetWidth / 2 - tabsEl.clientWidth / 2;
    to = Math.max(0, Math.min(max, to));
    stopTabAnim?.();
    stopTabAnim = animateScroll(tabsEl, to, 260);
  }

  function onPageChanged(i) {
    if (i === index) return;
    index = i;
    paintTabs();
    revealTab(i);
  }

  function goTo(i) {
    if (i === index && !stopPagerAnim) return;
    const w = pager.clientWidth;
    stopPagerAnim?.();
    pager.classList.add('anim');
    // Oraliq sahifalar ham surilganda bo'sh ko'rinmasin.
    for (let k = Math.min(i, index); k <= Math.max(i, index); k++) if (dirty[k]) renderPane(k);
    stopPagerAnim = animateScroll(pager, i * w, 280, () => {
      pager.classList.remove('anim');
      stopPagerAnim = null;
    });
  }

  tabEls.forEach((el, i) => el.addEventListener('click', () => goTo(i)));

  pager.addEventListener('scroll', () => {
    const w = pager.clientWidth;
    if (!w) return;
    onPageChanged(Math.max(0, Math.min(TABS.length - 1, Math.round(pager.scrollLeft / w))));
    renderNear();
  }, { passive: true });
  // Foydalanuvchi barmoq qo'ysa — dasturiy surish to'xtaydi.
  pager.addEventListener('touchstart', () => {
    if (stopPagerAnim) { stopPagerAnim(); stopPagerAnim = null; pager.classList.remove('anim'); }
  }, { passive: true });

  // ── tortib yangilash (`RefreshIndicator`) ──
  panes.forEach((pane) => bindPullToRefresh(pane, () => seasonsRepo.fetch()));

  // ── filtr ──
  async function openFilter() {
    const res = await openFilterSheet(filter);
    if (!res) return;
    filter = res;
    bar.setCount(filter.count);
    invalidate();
  }
  function clearFilter() {
    filter = makeFilter();
    bar.setCount(0);
    invalidate();
  }
  const bar = createFilterBar(openFilter, clearFilter);
  bar.setCount(0);

  let lastW = 0;
  window.addEventListener('resize', () => {
    const w = Math.round(page.clientWidth);
    if (w && w !== lastW) {
      lastW = w;
      pager.scrollLeft = index * pager.clientWidth;
      invalidate();
    }
  });

  seasonsRepo.listen(invalidate);
  invalidate();
  seasonsRepo.load();

  return {
    onShow() {
      bar.show();
      if (pager.clientWidth) pager.scrollLeft = index * pager.clientWidth;
      renderNear();
    },
    onHide() { bar.hide(); },
  };
}

/**
 * `RefreshIndicator`: ro'yxat eng tepada turganda pastga tortilsa
 * aylana chiqadi; yetarlicha tortilsa — yangilanadi.
 */
function bindPullToRefresh(pane, onRefresh) {
  const ind = pane.querySelector('.cat-ptr');
  const circle = pane.querySelector('.cat-ptr-c');
  const TRIGGER = 100;
  let y0 = 0;
  let x0 = 0;
  let pulling = false;
  let dy = 0;
  let busy = false;

  const set = (d, anim = false) => {
    const t = Math.min(1, d / TRIGGER);
    ind.style.transition = anim ? 'transform 200ms, opacity 200ms' : 'none';
    ind.style.transform = `translateY(${Math.min(d, TRIGGER) * 0.8}px)`;
    ind.style.opacity = `${t}`;
    circle.style.transform = `rotate(${d * 3}deg)`;
  };

  pane.addEventListener('touchstart', (e) => {
    if (busy || pane.scrollTop > 0) { pulling = false; return; }
    y0 = e.touches[0].clientY;
    x0 = e.touches[0].clientX;
    pulling = true;
    dy = 0;
  }, { passive: true });
  pane.addEventListener('touchmove', (e) => {
    if (!pulling) return;
    const d = e.touches[0].clientY - y0;
    const dx = Math.abs(e.touches[0].clientX - x0);
    if (dy === 0 && dx > Math.abs(d)) { pulling = false; return; }
    if (d <= 0 || pane.scrollTop > 0) { if (dy) set(0); dy = 0; return; }
    dy = d * 0.6;
    set(dy);
  }, { passive: true });
  const end = async () => {
    if (!pulling) return;
    pulling = false;
    if (dy >= TRIGGER) {
      busy = true;
      set(TRIGGER, true);
      circle.classList.add('spin');
      try { await onRefresh(); } catch (_) { /* */ }
      circle.classList.remove('spin');
      busy = false;
    }
    dy = 0;
    set(0, true);
  };
  pane.addEventListener('touchend', end);
  pane.addEventListener('touchcancel', end);
}
