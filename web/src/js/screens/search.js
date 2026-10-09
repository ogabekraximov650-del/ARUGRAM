// Qidiruv — `lib/screens/search_screen.dart`.
//
// Ilovada qidiruv TELEFONDA bajariladi: bo'limlar ro'yxati
// (`/api/seasons`) bir marta olinadi va Rust'dagi
// `rust_search_filter` (`rust/src/search.rs`) nom (`nomi`) bo'yicha,
// katta-kichik harfga qaramay, "ichida bormi" deb filtrlaydi. Bu yerda
// o'sha mantiq JS'da (`searchFilter`). Ro'yxat Bosh sahifa bilan
// umumiy `seasonsRepo` dan olinadi — qo'shimcha so'rov yo'q.

import { icon, bindTap, bindImages, isPaid, paidBadge } from '../ui.js';
import { imageUrl } from '../api.js';
import { esc } from '../format.js';
import { seasonsRepo } from '../seasons.js';
import { hooks } from '../hooks.js';

/** `rust/src/search.rs` -> `rust_search_filter` ning aynan nusxasi. */
export function searchFilter(list, query) {
  const q = `${query ?? ''}`.toLowerCase();
  if (!q.trim()) return [];
  return list.filter((s) => {
    const name = s?.nomi;
    return typeof name === 'string' && name.toLowerCase().includes(q);
  });
}

function cardHtml(s) {
  const photo = `${s.photo_url ?? ''}`;
  const url = imageUrl(photo);
  return `<div class="srch-card">
    <div class="srch-img">
      ${url ? `<img class="srch-poster" alt="" decoding="async" loading="lazy" src="${esc(url)}">` : ''}
      ${!photo ? `<div class="srch-ph">${icon('movie', { fill: false, size: 40, color: 'rgba(255,255,255,0.7)' })}</div>` : ''}
      ${isPaid(s) ? `<div class="srch-paid">${paidBadge()}</div>` : ''}
    </div>
    <div class="srch-info">
      <div class="srch-name">${esc(s.nomi ?? '')}</div>
      <div class="srch-genre">${esc(s.janri ?? '')}</div>
    </div>
  </div>`;
}

export function createSearch(page) {
  page.style.overflow = 'hidden';
  page.innerHTML = `
    <div class="srch-root">
      <div class="srch-box glass">
        ${icon('search', { size: 24, color: 'rgba(255,255,255,0.7)' })}
        <input class="srch-input" type="search" enterkeyhint="search" autocomplete="off"
          autocorrect="off" spellcheck="false" placeholder="Anime qidirish...">
        <div class="srch-clear">${icon('close', { size: 20, color: 'rgba(255,255,255,0.7)' })}</div>
      </div>
      <div class="srch-body"></div>
    </div>`;
  const input = page.querySelector('.srch-input');
  const clearBtn = page.querySelector('.srch-clear');
  const body = page.querySelector('.srch-body');

  let results = [];
  let searching = false;
  let hasSearched = false;
  let seq = 0;

  function render() {
    clearBtn.classList.toggle('on', input.value.length > 0);
    if (searching) {
      body.innerHTML = `<div class="srch-center"><div class="spinner" style="width:36px;height:36px;border-width:4px;border-top-color:rgba(255,255,255,0.6);border-right-color:rgba(255,255,255,0.6)"></div></div>`;
      return;
    }
    if (results.length === 0) {
      const off = hasSearched;
      body.innerHTML = `<div class="srch-center">
        ${icon(off ? 'search_off' : 'search', { size: 48, color: 'rgba(255,255,255,0.24)' })}
        <div class="srch-msg">${off ? 'Qidiruv natijalari topilmadi' : 'Anime nomini yozib qidiruv qiling'}</div>
      </div>`;
      return;
    }
    const list = results;
    body.innerHTML = `<div class="srch-grid">${list.map(cardHtml).join('')}</div><div class="bottom-space"></div>`;
    body.querySelectorAll('.srch-card').forEach((el, i) => bindTap(el, () => hooks.openSeason(list[i])));
    bindImages(body, 'img.srch-poster');
  }

  async function search(query) {
    const my = ++seq;
    if (!query.trim()) {
      results = [];
      hasSearched = false;
      render();
      return;
    }
    if (seasonsRepo.items.length === 0) {
      searching = true;
      render();
      try { await seasonsRepo.load(); } catch (_) { /* */ }
      if (my !== seq) return;
      searching = false;
    }
    results = searchFilter(seasonsRepo.items, query);
    hasSearched = true;
    body.scrollTop = 0;
    render();
  }

  input.addEventListener('input', () => search(input.value));
  input.addEventListener('keydown', (e) => { if (e.key === 'Enter') input.blur(); });
  bindTap(clearBtn, () => {
    seq++;
    input.value = '';
    results = [];
    hasSearched = false;
    searching = false;
    render();
  }, { scale: false });

  render();
  return {
    onShow() {},
    onHide() { input.blur(); },
  };
}
