// ARUmediaTV — Telegram Mini App. Ilovaning onlayn nusxasi.
//
// Tuzilishi `lib/screens/root_screen.dart` dagidek: 5 ta sahifa bir
// vaqtda quriladi (IndexedStack), pastki panel faqat qaysi biri
// ko'rinishini almashtiradi.

import { auth } from './api.js';
import { createBottomNav } from './nav.js';
import { createHome } from './home.js';

const tg = window.Telegram?.WebApp;
const app = document.getElementById('app');

function setupTelegram() {
  if (!tg) return;
  try {
    tg.ready();
    tg.expand();
    tg.setHeaderColor?.('#0A0A0C');
    tg.setBackgroundColor?.('#0A0A0C');
    tg.setBottomBarColor?.('#0A0A0C');
    // Ro'yxatni surganda Mini App yopilib ketmasin.
    tg.disableVerticalSwipes?.();
  } catch (_) { /* eski Telegram — e'tiborsiz */ }
}

let toastTimer = 0;
export function toast(text) {
  let el = app.querySelector('.toast');
  if (!el) {
    el = document.createElement('div');
    el.className = 'toast';
    app.appendChild(el);
  }
  el.textContent = text;
  requestAnimationFrame(() => el.classList.add('show'));
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => el.classList.remove('show'), 2200);
}

function soonPage(page, icon, title) {
  page.innerHTML = `<div class="soon"><span class="ic">${icon}</span>
    <div class="msg">${title} — keyingi bosqichda</div></div>`;
}

function gate(text) {
  app.innerHTML = `<div class="gate"><img src="assets/aru-mark.png" alt="ARU">
    <div class="msg">${text}</div></div>`;
}

async function main() {
  setupTelegram();
  if (!tg?.initData) {
    gate("ARUmediaTV'ni Telegram ichida oching:<br>botga /start yuboring va <b>«ARUmediaTV'ni ochish»</b> tugmasini bosing.");
    return;
  }
  try {
    await auth();
  } catch (_) {
    gate("Ulanib bo'lmadi. Internetni tekshirib, Mini App'ni qayta oching.");
    return;
  }

  const pagesEl = document.createElement('div');
  pagesEl.className = 'pages';
  app.appendChild(pagesEl);
  const pages = [0, 1, 2, 3, 4].map((i) => {
    const p = document.createElement('div');
    p.className = 'page' + (i === 0 ? ' active' : '');
    pagesEl.appendChild(p);
    return p;
  });

  createHome(pages[0], {
    onOpenSeason: () => toast("Anime sahifasi keyingi bosqichda qo'shiladi"),
  });
  soonPage(pages[1], 'search', 'Qidiruv');
  soonPage(pages[2], 'grid_view', 'Katalog');
  soonPage(pages[3], 'folder', 'Kutubxona');
  soonPage(pages[4], 'person', 'Profil');

  createBottomNav(app, (i) => {
    pages.forEach((p, k) => p.classList.toggle('active', k === i));
  });
}

main();
