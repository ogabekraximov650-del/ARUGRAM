// Bosh sahifa — `lib/screens/home_screen.dart` (onlayn qismi).
//
// Oflaynga xos qismlar ("Offline" belgisi, "Keshdan ko'rsatilmoqda",
// yuklab olinganlar filtri) ATAYLAB yo'q: Mini App faqat onlayn.
// Bo'limlar ro'yxati (`SeasonsRepo`) `seasons.js` da — Katalog ham
// shuni ishlatadi.

import { icon, spinner, seasonCardHtml, bindSeasonCards, cardWidth } from './ui.js';
import { seasonsRepo } from './seasons.js';
import { hooks } from './hooks.js';

export function createHome(page) {
  page.innerHTML = `
    <div class="home-head">
      <div>
        <img class="logo" src="assets/aru-mark.png" alt="ARU">
        <div class="sub">Anime dunyosi</div>
      </div>
      <div class="spacer"></div>
      <div class="bell">${icon('notifications', { fill: false, size: 22, color: 'rgba(255,255,255,0.7)' })}</div>
    </div>
    <div class="section-title">Ommabop anime</div>
    <div class="home-body"></div>
    <div class="bottom-space"></div>`;
  const body = page.querySelector('.home-body');

  function render() {
    const seasons = seasonsRepo.items;
    if (seasonsRepo.loading && seasons.length === 0) {
      body.innerHTML = `<div class="center-box">${spinner(36, 2)}</div>`;
      return;
    }
    if (seasons.length === 0) {
      body.innerHTML = `<div class="center-box">${icon('movie', { fill: false, size: 52, color: 'rgba(255,255,255,0.2)' })}
        <div class="msg">${seasonsRepo.failed ? 'Internetni tekshiring' : 'Anime topilmadi'}</div></div>`;
      return;
    }
    const w = cardWidth(page.clientWidth);
    body.innerHTML = `<div class="grid">${seasons.map((s) => seasonCardHtml(s, w)).join('')}</div>`;
    bindSeasonCards(body, seasons, (s) => hooks.openSeason(s));
  }

  let lastW = 0;
  window.addEventListener('resize', () => {
    const w = Math.round(page.clientWidth);
    if (w !== lastW) { lastW = w; render(); }
  });
  seasonsRepo.listen(render);
  render();
  seasonsRepo.load();
  return {};
}
