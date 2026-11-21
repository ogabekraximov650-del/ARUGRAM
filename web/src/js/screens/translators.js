// Tarjimonlar — Android `ui/catalog/TranslatorViews.kt` (Katalogning "Tarjimonlar" bo'limi va tarjimon sahifasi).
//
// `/api/translators` — ro'yxat; `/api/translators/:id/page` — { translator, stats{anime,episodes,views,rating}, seasons[] }.
// Oxirgi javob localStorage'da (darhol ko'rsatiladi, yangisi fonda keladi) — ilovadagi `TranslatorRepo` kabi.

import { api, imageUrl } from '../api.js';
import { esc, formatCount, toInt } from '../format.js';
import { push, back } from '../router.js';
import { icon, spinner, bindTap, bindSeasonCards, seasonCardHtml, cardWidth } from '../ui.js';
import { hooks } from '../hooks.js';

const LIST_KEY = 'aru_translators_v1';
const pageKey = (id) => `aru_translator_page_${id}`;
const read = (k) => { try { return JSON.parse(localStorage.getItem(k) || 'null'); } catch (_) { return null; } };
const write = (k, v) => { try { localStorage.setItem(k, JSON.stringify(v)); } catch (_) { /* */ } };

let listMem = null;
let listAt = 0;

/** Tarjimonlar ro'yxati: 5 daqiqa xotirada; tarmoq yo'q bo'lsa oxirgi nusxa. `null` — umuman yo'q. */
export async function loadTranslators() {
  if (listMem && Date.now() - listAt < 300000) return listMem;
  try {
    const l = await api('/api/translators');
    if (Array.isArray(l)) {
      listMem = l.filter((t) => toInt(t.id) > 0 && `${t.name ?? ''}`.trim());
      listAt = Date.now();
      write(LIST_KEY, listMem);
      return listMem;
    }
  } catch (_) { /* */ }
  return listMem || read(LIST_KEY);
}

/** Dumaloq tarjimon logotipi; rasm yo'q bo'lsa nomning bosh harflari. */
export function translatorLogoHtml(t, size) {
  const initials = `${t.name ?? ''}`.split(/[\s.\-]+/).filter(Boolean).slice(0, 2).map((x) => x[0].toUpperCase()).join('') || '?';
  const u = t.photo_url ? imageUrl(t.photo_url) : '';
  return `<span class="tr-ava" style="width:${size}px;height:${size}px">
    <span class="tr-ini" style="font-size:${(size / 2.4).toFixed(1)}px">${esc(initials)}</span>
    ${u ? `<img alt="" src="${esc(u)}" onerror="this.remove()">` : ''}</span>`;
}

const CHEVRON = `<svg class="tr-chev" width="9" height="16" viewBox="0 0 9 16"><path d="M1.1 1.1 L7.9 8 L1.1 14.9" fill="none" stroke="#fff" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"/></svg>`;

/** Oxirgi qo'shilgan `n` ta anime (har anime bir marta). */
function latestAnime(seasons, trId, n = 2) {
  const seen = new Set();
  const out = [];
  for (const s of seasons) {
    if (toInt(s.tr_id) !== trId || seen.has(s.anime_id)) continue;
    seen.add(s.anime_id);
    out.push(s);
    if (out.length >= n) break;
  }
  return out;
}

/** Katalogning "Tarjimonlar" sahifasi tanasi. */
export async function renderTranslatorsTab(body, seasons) {
  body.innerHTML = `<div class="cat-loading">${spinner(32, 2)}</div>`;
  const list = await loadTranslators();
  if (list === null) {
    body.innerHTML = `<div class="cat-loading">${spinner(32, 2)}</div>`;
    return;
  }
  const shown = list.filter((t) => toInt(t.anime_count) > 0 || seasons.some((s) => toInt(s.tr_id) === toInt(t.id)));
  if (shown.length === 0) {
    body.innerHTML = `<div class="cat-empty">${icon('menu', { size: 48, color: 'rgba(255,255,255,0.2)' })}<div class="msg">Hozircha tarjimonlar yo'q</div></div>`;
    return;
  }
  const w = cardWidth(body.clientWidth);
  body.innerHTML = `<div class="tr-list">${shown.map((t) => {
    const latest = latestAnime(seasons, toInt(t.id));
    return `<div class="tr-item" data-id="${toInt(t.id)}">
      <div class="tr-row">${translatorLogoHtml(t, 40)}<div class="tr-name">${esc(t.name)}</div>${CHEVRON}</div>
      <div class="tr-cards">${latest.map((s) => seasonCardHtml(s, w)).join('')}</div>
    </div>`;
  }).join('')}</div><div class="cat-bottom-space"></div>`;
  body.querySelectorAll('.tr-item').forEach((el, i) => {
    bindTap(el.querySelector('.tr-row'), () => openTranslator(shown[i]), { scale: false });
    const latest = latestAnime(seasons, toInt(shown[i].id));
    bindSeasonCards(el.querySelector('.tr-cards'), latest, (s) => hooks.openSeason(s));
  });
}

function statTile(label, value) {
  return `<div class="tr-stat"><div class="tr-stat-l">${esc(label)}</div><div class="tr-stat-v">${esc(value)}</div></div>`;
}

/** Tarjimon sahifasi (`TranslatorScreen`). */
export function openTranslator(translator) {
  push((el) => {
    el.innerHTML = `<div class="tr-page"><div class="scroll tr-scroll"></div>
      <button class="tr-back" aria-label="Orqaga">${icon('arrow_back', { size: 24, color: '#fff' })}</button></div>`;
    el.querySelector('.tr-back').addEventListener('click', () => back());
    const scroll = el.querySelector('.tr-scroll');
    const id = toInt(translator.id);
    let data = read(pageKey(id));
    let failed = false;
    let loading = true;

    function paint() {
      const t = data?.translator || translator;
      const st = data?.stats || {};
      const ready = !!data;
      const seasons = data?.seasons || [];
      const w = cardWidth(scroll.clientWidth);
      const rating = Number(st.rating) || 0;
      scroll.innerHTML = `<div class="tr-head">
          ${translatorLogoHtml(t, 96)}
          <div class="tr-h-name">${esc(t.name ?? '')}</div>
          ${`${t.bio ?? ''}`.trim() ? `<div class="tr-h-bio">${esc(`${t.bio}`.trim())}</div>` : ''}
        </div>
        <div class="tr-stats">
          ${statTile('Jami anime', ready ? `${toInt(st.anime)}` : '—')}
          ${statTile('Jami qismlar', ready ? `${toInt(st.episodes)}` : '—')}
          ${statTile("Ko'rishlar", ready ? formatCount(toInt(st.views)) : '—')}
          ${statTile("O'rtacha baho", ready && rating > 0 ? `★ ${rating.toFixed(1)}` : '—')}
        </div>
        <div class="tr-h-sec">Animelar</div>
        ${seasons.length ? `<div class="grid tr-grid">${seasons.map((s) => seasonCardHtml(s, w)).join('')}</div>`
    : `<div class="cat-empty">${loading && !failed ? spinner(32, 2) : icon('menu', { size: 48, color: 'rgba(255,255,255,0.2)' })}${
      loading && !failed ? '' : `<div class="msg">${failed ? "Ma'lumot yuklanmadi" : "Bu tarjimonning animelari hali yo'q"}</div>`}</div>`}
        <div class="bottom-space"></div>`;
      bindSeasonCards(scroll, seasons, (s) => hooks.openSeason(s));
    }

    paint();
    api(`/api/translators/${id}/page`).then((p) => {
      if (p && p.translator) { data = p; write(pageKey(id), p); }
      failed = !data;
    }).catch(() => { failed = !data; }).finally(() => { loading = false; paint(); });
    return {};
  });
}
