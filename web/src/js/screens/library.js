// KUTUBXONA — `lib/screens/library_screen.dart`, `history_screen.dart`
// (onlayn qismi) va `favorites_screen.dart`.
//
// Ilovada uchta oyna bor: Tarix, Yuklanmalar, Sevimlilar. Saytda
// yuklab olish yo'q, shu sabab "Yuklanmalar" oynasi (DownloadsList va
// unga tegishli hamma narsa) ATAYLAB olib tashlangan.
//
// Oynalar `PageView` dagidek: tugma bosilsa ham, barmoq bilan surilsa
// ham almashadi; har oynaning ro'yxat o'rni saqlanadi.
//
// To'xtagan joydagi kadr faqat ilovada yasaladi — saytda bo'lim posteri.

import { icon, spinner, bindTap, glass, seasonCardHtml, bindSeasonCards, cardWidth, paidBadge, dialog } from '../ui.js';
import { imageUrl } from '../api.js';
import { esc } from '../format.js';
import { hooks } from '../hooks.js';
import { push, back } from '../router.js';
import { seasonsRepo } from '../seasons.js';
import { watchHistory } from '../services/history.js';
import { favorites } from '../services/favorites.js';

// ── UMUMIY YORDAMCHILAR ─────────────────────────────────────────

const two = (v) => `${v}`.padStart(2, '0');

/** `12:34` — daqiqa:soniya (soat ajratilmaydi). */
function clock(ms) {
  const total = Math.floor((ms || 0) / 1000);
  return `${two(Math.floor(total / 60))}:${two(total % 60)}`;
}

/** `12:46/01/01/2026` — soat/kun/oy/yil. */
function fmtDate(ms) {
  if (!(ms > 0)) return '';
  const d = new Date(ms);
  return `${two(d.getHours())}:${two(d.getMinutes())}/${two(d.getDate())}/${two(d.getMonth() + 1)}/${d.getFullYear()}`;
}

/** `43,21%` */
function percent(v) {
  const x = Number.isNaN(v) ? 0 : Math.min(100, Math.max(0, v));
  return `${x.toFixed(2).replace('.', ',')}%`;
}

/** Bo'lim aniq pullikmi (`seasonIsPaid` — umumiy ro'yxatdan). */
function isPaidIds(a, s) {
  const row = seasonsRepo.find(a, s);
  return !!row && row.free === false;
}

function paidMark(item, compact = false) {
  return isPaidIds(item.animeId, item.seasonId)
    ? `<div class="lib-paid">${paidBadge(compact)}</div>` : '';
}

/** Tarixdagi yozuvdan pleyer uchun "bo'lim". */
function seasonOf(item) {
  return {
    anime_id: item.animeId,
    season_id: item.seasonId,
    bolim_id: item.bolimId,
    nomi: item.seasonName,
    anime_name: item.animeName,
    photo_url: item.seasonPhoto || item.animePhoto,
  };
}

/** Qismni AYNAN to'xtagan joyidan ochadi. */
function openEpisode(item) {
  hooks.openSeasonIds(item.animeId, item.seasonId, item.epizodId, {
    startAtMs: item.positionMs,
    season: seasonOf(item),
  });
}

function posterHtml(url) {
  const u = imageUrl(url);
  return `<div class="lib-poster">${u ? `<img alt="" decoding="async" loading="lazy" src="${esc(u)}">` : ''}</div>`;
}

function bindPosters(root) {
  root.querySelectorAll('.lib-poster img').forEach((img) => {
    const ok = () => img.classList.add('ok');
    if (img.complete && img.naturalWidth > 0) ok();
    else {
      img.addEventListener('load', ok, { once: true });
      img.addEventListener('error', () => img.remove(), { once: true });
    }
  });
}

function shadowText(text, { size, weight = 500, alpha = 1, align = 'right' }) {
  if (!text) return '';
  return `<div class="lib-st" style="font-size:${size}px;font-weight:${weight};color:rgba(255,255,255,${alpha});text-align:${align}">${esc(text)}</div>`;
}

/** Bo'sh holat: `_EmptyState` / `_EmptyFavorites`. */
function emptyBox({ loading, ic, text, sub = '', fill = true }) {
  return `<div class="empty-glass">${glass(`
    ${loading ? spinner(26, 2.4, 'rgba(255,255,255,0.7)') : icon(ic, { fill, size: 46, color: 'rgba(255,255,255,0.54)' })}
    <div class="eg-t">${loading ? 'Yuklanmoqda...' : text}</div>
    ${!loading && sub ? `<div class="eg-s">${sub}</div>` : ''}`, { radius: 20, pad: '24px' })}</div>`;
}

/** Bosish + uzoq bosish (GestureDetector onTap / onLongPress). */
function bindPress(el, onTap, onLong) {
  let timer = 0; let sx = 0; let sy = 0; let live = false; let fired = false;
  const cancel = () => { clearTimeout(timer); live = false; };
  el.addEventListener('pointerdown', (e) => {
    sx = e.clientX; sy = e.clientY; live = true; fired = false;
    if (onLong) {
      timer = setTimeout(() => { if (live) { live = false; fired = true; onLong(); } }, 500);
    }
  });
  el.addEventListener('pointermove', (e) => {
    if (live && (Math.abs(e.clientX - sx) > 10 || Math.abs(e.clientY - sy) > 10)) cancel();
  });
  el.addEventListener('pointerup', () => {
    const was = live; cancel();
    if (was && !fired) onTap();
  });
  el.addEventListener('pointercancel', cancel);
  el.addEventListener('pointerleave', cancel);
  el.addEventListener('contextmenu', (e) => {
    e.preventDefault();
    if (onLong && !fired) { cancel(); fired = true; onLong(); }
  });
}

/** "Rostdan ham bu tarixni o'chirib tashlaysizmi?" */
function confirmRemove(item) {
  dialog({
    cls: 'lib-dlg',
    content: (el, close) => {
      el.innerHTML = `
        <div class="lib-cf">
          ${icon('delete', { fill: false, size: 42, color: 'rgba(255,255,255,0.7)' })}
          <div class="lib-cf-t">Rostdan ham bu tarixni o'chirib tashlaysizmi?</div>
          <div class="lib-cf-s">${esc(`${item.title} · ${item.bolimNumber}-bo'lim ${item.epizodNumber}-qism`)}</div>
          <div class="lib-cf-row">
            <button class="btn btn-outline lib-no">Yo'q</button>
            <button class="btn btn-filled lib-yes">Ha, o'chirilsin</button>
          </div>
        </div>`;
      el.querySelector('.lib-no').addEventListener('click', () => close(false));
      el.querySelector('.lib-yes').addEventListener('click', () => close(true));
    },
  }).then((ok) => { if (ok === true) watchHistory.remove(item); });
}

// ── ANIME QATORI ────────────────────────────────────────────────

function animeRowHtml(item) {
  return `<div class="lib-arow glass">
    <div class="lib-frame lib-frame-a">${posterHtml(item.poster)}${paidMark(item)}</div>
    <div class="lib-arow-t">${esc(item.title)}</div>
    <div class="lib-arow-s">${esc(`Oxirgi marta ${item.bolimNumber}-bo'lim ${item.epizodNumber}-qismni ko'rdingiz`)}</div>
    <div class="lib-arow-d">${esc(`Sana: ${fmtDate(item.updatedAt)}`)}</div>
  </div>`;
}

// ── QISM QATORI ─────────────────────────────────────────────────

function episodeRowHtml(item) {
  const p = (item.progress * 100).toFixed(3);
  return `<div class="lib-erow">
    ${posterHtml(item.poster)}
    ${paidMark(item)}
    <div class="lib-etexts">
      <div class="lib-eleft">
        ${shadowText(item.title, { size: 11.5, weight: 700, align: 'left' })}
        ${shadowText(`${item.bolimNumber}-bo'lim ${item.epizodNumber}-qism`, { size: 10.5, align: 'left' })}
        ${shadowText(`sana: ${fmtDate(item.updatedAt)}`, { size: 10, alpha: 0.85, align: 'left' })}
      </div>
      ${shadowText(`${percent(item.percent)} | ${clock(item.positionMs)}/${clock(item.durationMs)}`, { size: 10.5, weight: 600, align: 'right' })}
    </div>
    <div class="lib-bar"><div style="width:${p}%"></div></div>
  </div>`;
}

function bindEpisodeRows(root, rows) {
  root.querySelectorAll('.lib-erow').forEach((el, i) => {
    bindPress(el, () => openEpisode(rows[i]), () => confirmRemove(rows[i]));
  });
  bindPosters(root);
}

// ── BITTA ANIMENING QISMLARI (`AnimeHistoryScreen`) ─────────────

function openAnimeHistory(animeId, title) {
  push((el) => {
    el.innerHTML = `
      <div class="lib-ah-head">
        <div class="lib-ah-back glass">${icon('arrow_back', { size: 24, color: '#fff' })}</div>
        <div class="lib-ah-t">${esc(title || 'Tomosha tarixi')}</div>
      </div>
      <div class="scroll lib-ah-list"></div>`;
    bindTap(el.querySelector('.lib-ah-back'), () => back(), { scale: false });
    const list = el.querySelector('.lib-ah-list');
    let sig = '';
    const render = () => {
      const rows = watchHistory.episodesOf(animeId);
      const s = rows.map((r) => `${r.animeId}:${r.seasonId}:${r.epizodId}:${r.positionMs}:${r.updatedAt}`).join('|')
        + `#${seasonsRepo.items.length}`;
      if (s === sig) return;
      sig = s;
      if (!rows.length) {
        list.innerHTML = emptyBox({ loading: false, ic: 'history', text: "Hali hech narsa ko'rilmagan" });
        return;
      }
      list.innerHTML = `<div class="lib-list lib-list-ah">${rows.map(episodeRowHtml).join('')}</div>`;
      bindEpisodeRows(list, rows);
    };
    const offH = watchHistory.listen(render);
    const offS = seasonsRepo.listen(render);
    render();
    return { dispose() { offH(); offS(); } };
  }, { transition: 'slide' });
}

// ── PASTGA TORTIB YANGILASH (`RefreshIndicator`) ────────────────

function bindPullRefresh(pane, onRefresh) {
  const ind = document.createElement('div');
  ind.className = 'lib-ptr';
  ind.innerHTML = '<div class="lib-ptr-arc"></div>';
  pane.appendChild(ind);
  const arc = ind.firstChild;
  let sy = 0; let sx = 0; let pulling = false; let dy = 0; let busy = false; let decided = false;
  const TRIGGER = 80;
  const set = (d, anim) => {
    ind.style.transition = anim ? 'transform 200ms, opacity 200ms' : 'none';
    const y = Math.min(d, 120) * 0.6;
    ind.style.transform = `translate(-50%, ${y - 40}px)`;
    ind.style.opacity = d > 4 ? '1' : '0';
    arc.style.transform = `rotate(${Math.min(d, TRIGGER) * 3.6}deg)`;
  };
  pane.addEventListener('touchstart', (e) => {
    if (busy || pane.scrollTop > 0) return;
    sy = e.touches[0].clientY; sx = e.touches[0].clientX;
    pulling = true; decided = false; dy = 0;
  }, { passive: true });
  pane.addEventListener('touchmove', (e) => {
    if (!pulling) return;
    const ddy = e.touches[0].clientY - sy;
    const ddx = e.touches[0].clientX - sx;
    if (!decided) {
      if (Math.abs(ddy) < 6 && Math.abs(ddx) < 6) return;
      decided = true;
      if (Math.abs(ddx) > Math.abs(ddy) || ddy < 0) { pulling = false; return; }
    }
    dy = Math.max(0, ddy);
    set(dy, false);
  }, { passive: true });
  const end = async () => {
    if (!pulling) return;
    pulling = false;
    if (dy < TRIGGER) { set(0, true); return; }
    busy = true;
    ind.classList.add('spin');
    set(TRIGGER, true);
    try {
      await Promise.race([onRefresh(), new Promise((r) => setTimeout(r, 25000))]);
    } catch (_) { /* */ }
    ind.classList.remove('spin');
    busy = false;
    set(0, true);
  };
  pane.addEventListener('touchend', end);
  pane.addEventListener('touchcancel', end);
}

// ── SAHIFA ──────────────────────────────────────────────────────

const TITLES = ['Tarix', 'Sevimlilar'];

export function createLibrary(page) {
  watchHistory.loadFromDisk();
  favorites.loadFromDisk();

  page.innerHTML = `
    <div class="lib">
      <div class="lib-head">Kutubxona</div>
      <div class="lib-tabs">${TITLES.map((t, i) => `<div class="lib-tab${i === 0 ? ' active' : ''}" data-i="${i}"><span>${t}</span></div>`).join('')}</div>
      <div class="lib-pager">
        <div class="lib-pane" data-p="0"><div class="lib-hist"></div></div>
        <div class="lib-pane" data-p="1"><div class="lib-fav"></div></div>
      </div>
    </div>`;
  const pager = page.querySelector('.lib-pager');
  const panes = [...page.querySelectorAll('.lib-pane')];
  const tabs = [...page.querySelectorAll('.lib-tab')];
  const histEl = page.querySelector('.lib-hist');
  const favEl = page.querySelector('.lib-fav');
  let tab = 0;

  const setTab = (i) => {
    if (i === tab) return;
    tab = i;
    tabs.forEach((t, k) => t.classList.toggle('active', k === i));
  };
  tabs.forEach((t, i) => t.addEventListener('click', () => {
    if (i === tab) return;
    setTab(i);
    pager.scrollTo({ left: i * pager.clientWidth, behavior: 'smooth' });
  }));
  pager.addEventListener('scroll', () => {
    const w = pager.clientWidth || 1;
    setTab(Math.round(pager.scrollLeft / w));
  }, { passive: true });

  bindPullRefresh(panes[0], () => watchHistory.load({ force: true }));
  bindPullRefresh(panes[1], () => favorites.load({ force: true }));

  // ── Tarix (faqat anime bo'yicha) ──
  let hSig = '';
  function renderHistory() {
    const rows = watchHistory.byAnime;
    const s = `${watchHistory.isLoading}|${seasonsRepo.items.length}|`
      + rows.map((r) => `${r.animeId}:${r.seasonId}:${r.epizodId}:${r.epizodNumber}:${r.updatedAt}:${r.poster}:${r.title}`).join('|');
    if (s === hSig) return;
    hSig = s;
    if (!rows.length) {
      histEl.innerHTML = emptyBox({ loading: watchHistory.isLoading, ic: 'history', text: "Hali hech narsa ko'rilmagan" });
      return;
    }
    histEl.innerHTML = `<div class="lib-list">${rows.map(animeRowHtml).join('')}</div>`;
    histEl.querySelectorAll('.lib-arow').forEach((el, i) => {
      const it = rows[i];
      bindTap(el, () => openAnimeHistory(it.animeId, it.title), { scale: false });
    });
    bindPosters(histEl);
  }

  // ── Sevimlilar ──
  let fSig = '';
  function renderFavs() {
    const rows = favorites.items;
    const w = cardWidth(page.clientWidth);
    const s = `${favorites.isLoading}|${Math.round(w)}|${JSON.stringify(rows)}`;
    if (s === fSig) return;
    fSig = s;
    if (!rows.length) {
      favEl.innerHTML = emptyBox({
        loading: favorites.isLoading,
        ic: 'favorite',
        text: "Hali sevimli bo'lim qo'shilmagan",
        sub: 'Pleyerdagi yurakchani bosing',
        fill: false,
      });
      return;
    }
    favEl.innerHTML = `<div class="grid lib-grid">${rows.map((r) => seasonCardHtml(r, w)).join('')}</div>`;
    bindSeasonCards(favEl, rows, (season) => hooks.openSeason(season));
  }

  watchHistory.listen(renderHistory);
  favorites.listen(renderFavs);
  seasonsRepo.listen(() => { renderHistory(); renderFavs(); });
  let lastW = 0;
  window.addEventListener('resize', () => {
    const w = Math.round(page.clientWidth);
    if (w !== lastW) {
      lastW = w;
      renderFavs();
      pager.scrollLeft = tab * pager.clientWidth;
    }
  });
  renderHistory();
  renderFavs();

  return {
    onShow() {
      // `root_screen.dart` -> `_onTabTap`: aynan tugma bosilganda.
      watchHistory.load();
      favorites.load();
      renderFavs();
    },
    onHide() {},
  };
}
